use std::{
    env, fmt, fs,
    path::{Path, PathBuf},
};

use bevy::{app::AppExit, prelude::*, window::PrimaryWindow};
use serde::Serialize;

use crate::framework::ui::{
    audit::screenshot::{
        UiScreenshotEvent, UiScreenshotPlugin, UiScreenshotSystems, absolute_display_path,
        current_unix_timestamp_seconds, read_bool, sanitize_filename_segment,
    },
    core::{
        UiCurrentOwner, UiHeightClass, UiInputMode, UiOrientation, UiOwnerId, UiPanelKind,
        UiPanelRoot, UiViewport, UiWidthClass, stats::UiStats,
    },
    widgets::{
        UiScrollAuditAnchorId, UiScrollAuditId, UiScrollAuditMetrics, UiScrollAuditPosition,
        UiScrollView, scroll_audit_metrics, scroll_audit_position_reached, set_scroll_audit_anchor,
        set_scroll_audit_position,
    },
};

const ENV_UI_AUDIT: &str = "MYBEVY_UI_AUDIT";
const ENV_UI_AUDIT_SCREEN: &str = "MYBEVY_UI_AUDIT_SCREEN";
const ENV_UI_AUDIT_OUTPUT: &str = "MYBEVY_UI_AUDIT_OUTPUT";
const ENV_UI_AUDIT_STATES: &str = "MYBEVY_UI_AUDIT_STATES";
const ENV_UI_AUDIT_EXIT_ON_FINISH: &str = "MYBEVY_UI_AUDIT_EXIT_ON_FINISH";
const DEFAULT_AUDIT_OUTPUT_ROOT: &str = "../summary/ui-audit";

// These MYBEVY_UI_AUDIT_* variables belong only to the first-stage local one-shot mode.
const INITIAL_CAPTURE_STATE: &str = "initial";
const VISUAL_FOUNDATION_CAPTURE_STATE: &str = "visual_foundation";
const IMAGE_FIT_CAPTURE_STATE: &str = "image_fit";
const IMAGE_MODES_CAPTURE_STATE: &str = "image_modes";
const IMAGE_TILING_CAPTURE_STATE: &str = "image_tiling";
const IMAGE_ATLAS_CAPTURE_STATE: &str = "image_atlas";
const SCROLL_TOP_CAPTURE_STATE: &str = "top";
const SCROLL_MIDDLE_CAPTURE_STATE: &str = "middle";
const SCROLL_BOTTOM_CAPTURE_STATE: &str = "bottom";
const STABLE_WAIT_FRAMES: u32 = 5;
const PANEL_READY_TIMEOUT_FRAMES: u32 = 300;
const STABLE_TIMEOUT_FRAMES: u32 = 120;
const SCREENSHOT_TIMEOUT_FRAMES: u32 = 300;

pub(crate) struct UiAuditPlugin;

impl Plugin for UiAuditPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(UiScreenshotPlugin)
            .init_resource::<UiAuditScreenRegistry>()
            .insert_resource(UiAuditConfig::from_env())
            .insert_resource(UiAuditRuntime::default())
            .add_message::<UiAuditRouteCommand>()
            .configure_sets(
                Update,
                UiAuditSystems::Driver.after(UiScreenshotSystems::Timeout),
            )
            .add_systems(
                Update,
                drive_local_ui_audit
                    .run_if(local_ui_audit_enabled)
                    .in_set(UiAuditSystems::Driver),
            );
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
enum UiAuditSystems {
    Driver,
}

#[derive(Clone, Debug, Default, Resource)]
pub(crate) struct UiAuditScreenRegistry {
    screens: Vec<UiAuditScreen>,
}

impl UiAuditScreenRegistry {
    pub(crate) fn register(&mut self, screen: UiAuditScreen) {
        if let Some(existing) = self
            .screens
            .iter_mut()
            .find(|existing| existing.canonical == screen.canonical)
        {
            *existing = screen;
        } else {
            self.screens.push(screen);
        }
    }

    pub(crate) fn register_recipe(&mut self, recipe: UiAuditScreenRecipe) {
        self.register(recipe.screen);
    }

    pub(crate) fn resolve(&self, value: &str) -> Option<&UiAuditScreen> {
        let normalized = normalize_screen_alias(value);
        self.screens.iter().find(|screen| {
            screen.canonical == normalized
                || screen
                    .aliases
                    .iter()
                    .any(|alias| normalize_screen_alias(alias) == normalized)
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UiAuditScreen {
    pub canonical: &'static str,
    pub aliases: &'static [&'static str],
    pub owner: UiOwnerId,
    pub recipe: Option<UiAuditRecipe>,
}

impl UiAuditScreen {
    pub(crate) const fn new(
        canonical: &'static str,
        aliases: &'static [&'static str],
        owner: UiOwnerId,
    ) -> Self {
        Self {
            canonical,
            aliases,
            owner,
            recipe: None,
        }
    }

    pub(crate) const fn with_recipe(mut self, recipe: UiAuditRecipe) -> Self {
        self.recipe = Some(recipe);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UiAuditScreenRecipe {
    pub screen: UiAuditScreen,
}

impl UiAuditScreenRecipe {
    pub(crate) const fn new(screen: UiAuditScreen) -> Self {
        Self { screen }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct UiAuditRecipe {
    pub captures: &'static [UiAuditCaptureRecipe],
    pub ready: Option<UiAuditReadyCondition>,
}

impl UiAuditRecipe {
    pub(crate) const fn new(captures: &'static [UiAuditCaptureRecipe]) -> Self {
        Self {
            captures,
            ready: None,
        }
    }

    #[allow(dead_code)]
    pub(crate) const fn with_ready(mut self, ready: UiAuditReadyCondition) -> Self {
        self.ready = Some(ready);
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct UiAuditCaptureRecipe {
    pub state: UiAuditCaptureState,
    pub scroll: Option<UiAuditScrollRecipe>,
}

impl UiAuditCaptureRecipe {
    pub(crate) const fn initial() -> Self {
        Self {
            state: UiAuditCaptureState::Initial,
            scroll: None,
        }
    }

    pub(crate) const fn scroll(
        state: UiAuditCaptureState,
        target_id: UiScrollAuditId,
        position: UiScrollAuditPosition,
    ) -> Self {
        Self {
            state,
            scroll: Some(UiAuditScrollRecipe {
                target_id,
                target: UiAuditScrollTarget::Position(position),
            }),
        }
    }

    pub(crate) const fn scroll_anchor(
        state: UiAuditCaptureState,
        target_id: UiScrollAuditId,
        anchor_id: UiScrollAuditAnchorId,
    ) -> Self {
        Self {
            state,
            scroll: Some(UiAuditScrollRecipe {
                target_id,
                target: UiAuditScrollTarget::Anchor(anchor_id),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct UiAuditScrollRecipe {
    pub target_id: UiScrollAuditId,
    pub target: UiAuditScrollTarget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UiAuditScrollTarget {
    Position(UiScrollAuditPosition),
    Anchor(UiScrollAuditAnchorId),
}

impl UiAuditScrollTarget {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Position(position) => position.as_str(),
            Self::Anchor(anchor) => anchor.as_str(),
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UiAuditReadyCondition {
    OwnerPanel,
}

#[derive(Clone, Debug, Resource)]
struct UiAuditConfig {
    enabled: bool,
    screen: Option<String>,
    output_root: PathBuf,
    states: Vec<UiAuditCaptureState>,
    states_from_env: bool,
    exit_on_finish: bool,
    config_error: Option<UiAuditFailureKind>,
}

impl Default for UiAuditConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            screen: None,
            output_root: PathBuf::from(DEFAULT_AUDIT_OUTPUT_ROOT),
            states: vec![UiAuditCaptureState::Initial],
            states_from_env: false,
            exit_on_finish: false,
            config_error: None,
        }
    }
}

impl UiAuditConfig {
    fn from_env() -> Self {
        Self::from_env_reader(|key| env::var(key).ok(), current_unix_timestamp_seconds())
    }

    fn from_env_reader(mut read: impl FnMut(&str) -> Option<String>, run_id: u64) -> Self {
        let enabled = read_bool(&mut read, ENV_UI_AUDIT).unwrap_or(false);
        let screen = read(ENV_UI_AUDIT_SCREEN)
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        let output_root = read(ENV_UI_AUDIT_OUTPUT)
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_AUDIT_OUTPUT_ROOT).join(run_id.to_string()));
        let exit_on_finish = read_bool(&mut read, ENV_UI_AUDIT_EXIT_ON_FINISH).unwrap_or(false);

        let (states, states_from_env, state_error) = match read(ENV_UI_AUDIT_STATES) {
            Some(value) => {
                let (states, error) = parse_capture_states(&value);
                (states, true, error)
            }
            None => (vec![UiAuditCaptureState::Initial], false, None),
        };
        let config_error = if enabled {
            state_error.or_else(|| {
                screen
                    .is_none()
                    .then_some(UiAuditFailureKind::ConfigInvalid)
            })
        } else {
            None
        };

        Self {
            enabled,
            screen,
            output_root,
            states,
            states_from_env,
            exit_on_finish,
            config_error,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UiAuditCaptureState {
    Initial,
    VisualFoundation,
    ImageFit,
    ImageModes,
    ImageTiling,
    ImageAtlas,
    Top,
    Middle,
    Bottom,
}

impl UiAuditCaptureState {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Initial => INITIAL_CAPTURE_STATE,
            Self::VisualFoundation => VISUAL_FOUNDATION_CAPTURE_STATE,
            Self::ImageFit => IMAGE_FIT_CAPTURE_STATE,
            Self::ImageModes => IMAGE_MODES_CAPTURE_STATE,
            Self::ImageTiling => IMAGE_TILING_CAPTURE_STATE,
            Self::ImageAtlas => IMAGE_ATLAS_CAPTURE_STATE,
            Self::Top => SCROLL_TOP_CAPTURE_STATE,
            Self::Middle => SCROLL_MIDDLE_CAPTURE_STATE,
            Self::Bottom => SCROLL_BOTTOM_CAPTURE_STATE,
        }
    }
}

#[derive(Clone, Debug, Default, Resource)]
struct UiAuditRuntime {
    phase: UiAuditPhase,
    plan: Option<UiAuditRunPlan>,
    capture_index: usize,
    manifest_entries: Vec<UiAuditManifestEntry>,
    result: Option<UiAuditCaptureResult>,
    exit_requested: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum UiAuditPhase {
    #[default]
    Init,
    EnterScreen,
    WaitForScreen {
        waited_frames: u32,
    },
    ApplyCaptureState,
    WaitForStable {
        waited_frames: u32,
    },
    RequestScreenshot,
    WaitForScreenshot {
        waited_frames: u32,
    },
    WriteCapture,
    Finish,
    Failed(UiAuditFailureKind),
}

#[derive(Clone, Debug, PartialEq)]
struct UiAuditRunPlan {
    screen: UiAuditResolvedScreen,
    output_root: PathBuf,
    manifest_path: PathBuf,
    report_path: PathBuf,
    device: String,
    ready_condition: Option<UiAuditReadyCondition>,
    captures: Vec<UiAuditCapturePlan>,
}

#[derive(Clone, Debug, PartialEq)]
struct UiAuditCapturePlan {
    index: usize,
    state: UiAuditCaptureState,
    screenshot_path: PathBuf,
    metadata_path: PathBuf,
    scroll: Option<UiAuditScrollRecipe>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct UiAuditResolvedScreen {
    requested: String,
    canonical: String,
    owner: UiOwnerId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct UiAuditCaptureResult {
    status: UiAuditRunStatus,
    failure: Option<UiAuditFailureKind>,
    detail: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum UiAuditRunStatus {
    Passed,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum UiAuditFailureKind {
    ScreenNotFound,
    PanelNotReady,
    UnstableUi,
    ScreenshotFailed,
    ScrollTargetMissing,
    ScrollTargetUnreachable,
    ConfigInvalid,
    OutputWriteFailed,
}

impl UiAuditFailureKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::ScreenNotFound => "screen_not_found",
            Self::PanelNotReady => "panel_not_ready",
            Self::UnstableUi => "unstable_ui",
            Self::ScreenshotFailed => "screenshot_failed",
            Self::ScrollTargetMissing => "scroll_target_missing",
            Self::ScrollTargetUnreachable => "scroll_target_unreachable",
            Self::ConfigInvalid => "config_invalid",
            Self::OutputWriteFailed => "output_write_failed",
        }
    }
}

impl fmt::Display for UiAuditFailureKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum UiAuditPureAction {
    RouteToScreen,
    ApplyCaptureState,
    RequestScreenshot,
    WriteCapture,
    Finish,
    Fail(UiAuditFailureKind),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UiAuditScreenshotStatus {
    Pending,
    Saved,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct UiAuditStepInput {
    target_panel_ready: bool,
    screenshot_status: UiAuditScreenshotStatus,
}

fn local_ui_audit_enabled(config: Res<UiAuditConfig>) -> bool {
    config.enabled
}

fn drive_local_ui_audit(
    mut runtime: ResMut<UiAuditRuntime>,
    config: Res<UiAuditConfig>,
    registry: Res<UiAuditScreenRegistry>,
    current_owner: Res<UiCurrentOwner>,
    viewport: Res<UiViewport>,
    stats: Res<UiStats>,
    panels: Query<&UiPanelRoot>,
    primary_window: Query<&Window, With<PrimaryWindow>>,
    mut scroll_targets: Query<
        (
            &UiScrollAuditId,
            &mut ScrollPosition,
            &ComputedNode,
            &UiGlobalTransform,
        ),
        With<UiScrollView>,
    >,
    scroll_anchors: Query<
        (&UiScrollAuditAnchorId, &ComputedNode, &UiGlobalTransform),
        Without<UiScrollView>,
    >,
    mut route_writer: MessageWriter<UiAuditRouteCommand>,
    mut screenshot_writer: MessageWriter<crate::framework::ui::audit::UiScreenshotCommand>,
    mut screenshot_events: MessageReader<UiScreenshotEvent>,
    mut app_exit: MessageWriter<AppExit>,
) {
    if matches!(runtime.phase, UiAuditPhase::Finish) {
        request_exit_if_needed(&mut runtime, &config, &mut app_exit);
        return;
    }

    if runtime.plan.is_none() {
        let plan = match prepare_runtime_plan(&config, &registry, &primary_window) {
            Ok(plan) => plan,
            Err(error) => {
                let failure = error.failure;
                let detail = Some(error.detail);
                if let Err(error) = write_planless_failure_outputs(
                    &config,
                    &primary_window,
                    failure,
                    detail.as_deref(),
                ) {
                    error!("ui audit failure output write failed: {error}");
                }
                runtime.phase = UiAuditPhase::Failed(failure);
                runtime.result = Some(UiAuditCaptureResult {
                    status: UiAuditRunStatus::Failed,
                    failure: Some(failure),
                    detail,
                });
                request_exit_if_needed(&mut runtime, &config, &mut app_exit);
                return;
            }
        };
        runtime.plan = Some(plan);
    }

    let screenshot_status =
        consume_screenshot_status(&mut screenshot_events, current_capture_plan(&runtime));
    let target_panel_ready = runtime
        .plan
        .as_ref()
        .is_some_and(|plan| target_owner_panel_ready(plan.screen.owner, &panels));
    let phase = std::mem::take(&mut runtime.phase);
    let (next_phase, action) = advance_audit_phase(
        phase,
        UiAuditStepInput {
            target_panel_ready,
            screenshot_status,
        },
    );
    runtime.phase = next_phase;

    match action {
        Some(UiAuditPureAction::RouteToScreen) => {
            if let Some(plan) = runtime.plan.as_ref() {
                route_writer.write(UiAuditRouteCommand {
                    screen: plan.screen.canonical.clone(),
                    owner: plan.screen.owner,
                });
            }
        }
        Some(UiAuditPureAction::ApplyCaptureState) => {
            let Some(capture) = current_capture_plan(&runtime).cloned() else {
                let failure = UiAuditFailureKind::ConfigInvalid;
                runtime.phase = UiAuditPhase::Failed(failure);
                runtime.result = Some(UiAuditCaptureResult {
                    status: UiAuditRunStatus::Failed,
                    failure: Some(failure),
                    detail: Some("no capture plan is available".to_owned()),
                });
                request_exit_if_needed(&mut runtime, &config, &mut app_exit);
                return;
            };

            match apply_capture_state(&capture, &mut scroll_targets, &scroll_anchors) {
                Ok(()) => {}
                Err((failure, detail)) => {
                    runtime.phase = UiAuditPhase::Failed(failure);
                    runtime.result = Some(UiAuditCaptureResult {
                        status: UiAuditRunStatus::Failed,
                        failure: Some(failure),
                        detail: Some(detail.clone()),
                    });
                    if let Some(plan) = runtime.plan.as_ref() {
                        if let Err(error) = write_failure_outputs(
                            plan,
                            &runtime.manifest_entries,
                            &capture,
                            failure,
                            Some(&detail),
                        ) {
                            error!("ui audit failure output write failed: {error}");
                        }
                    }
                    request_exit_if_needed(&mut runtime, &config, &mut app_exit);
                }
            }
        }
        Some(UiAuditPureAction::RequestScreenshot) => {
            if let (Some(plan), Some(capture)) =
                (runtime.plan.as_ref(), current_capture_plan(&runtime))
            {
                screenshot_writer.write(
                    crate::framework::ui::audit::UiScreenshotCommand::Capture {
                        path: capture.screenshot_path.clone(),
                        label: format!("{}_{}", plan.screen.canonical, capture.state.as_str()),
                    },
                );
            }
        }
        Some(UiAuditPureAction::WriteCapture) => {
            if let (Some(plan), Some(capture)) = (
                runtime.plan.as_ref().cloned(),
                current_capture_plan(&runtime).cloned(),
            ) {
                let scroll = capture_scroll_metadata(&capture, &mut scroll_targets);
                let metadata = build_capture_metadata(
                    &plan,
                    &capture,
                    scroll.as_ref(),
                    &viewport,
                    &stats,
                    &current_owner,
                    &panels,
                    primary_window.single().ok(),
                );
                match write_capture_metadata(&capture, &metadata) {
                    Ok(()) => {
                        runtime
                            .manifest_entries
                            .push(UiAuditManifestEntry::success(&plan, &capture));
                        runtime.capture_index = runtime.capture_index.saturating_add(1);
                        if runtime.capture_index >= plan.captures.len() {
                            let manifest = UiAuditManifest::new(runtime.manifest_entries.clone());
                            if let Err(error) = write_run_outputs(&plan, &manifest) {
                                error!("ui audit output write failed: {error}");
                                let failure = UiAuditFailureKind::OutputWriteFailed;
                                runtime.phase = UiAuditPhase::Failed(failure);
                                runtime.result = Some(UiAuditCaptureResult {
                                    status: UiAuditRunStatus::Failed,
                                    failure: Some(failure),
                                    detail: Some(error),
                                });
                                request_exit_if_needed(&mut runtime, &config, &mut app_exit);
                            } else {
                                runtime.result = Some(UiAuditCaptureResult {
                                    status: UiAuditRunStatus::Passed,
                                    failure: None,
                                    detail: None,
                                });
                            }
                        } else {
                            runtime.phase = UiAuditPhase::ApplyCaptureState;
                        }
                    }
                    Err(error) => {
                        error!("ui audit output write failed: {error}");
                        let failure = UiAuditFailureKind::OutputWriteFailed;
                        runtime.phase = UiAuditPhase::Failed(failure);
                        runtime.result = Some(UiAuditCaptureResult {
                            status: UiAuditRunStatus::Failed,
                            failure: Some(failure),
                            detail: Some(error),
                        });
                        request_exit_if_needed(&mut runtime, &config, &mut app_exit);
                    }
                }
            }
        }
        Some(UiAuditPureAction::Finish) => {
            info!("ui audit finished successfully");
            request_exit_if_needed(&mut runtime, &config, &mut app_exit);
        }
        Some(UiAuditPureAction::Fail(failure)) => {
            let detail = failure_detail(
                failure,
                runtime.plan.as_ref(),
                current_capture_plan(&runtime),
                screenshot_status,
            );
            runtime.result = Some(UiAuditCaptureResult {
                status: UiAuditRunStatus::Failed,
                failure: Some(failure),
                detail: detail.clone(),
            });
            if let (Some(plan), Some(capture)) =
                (runtime.plan.as_ref(), current_capture_plan(&runtime))
            {
                if let Err(error) = write_failure_outputs(
                    plan,
                    &runtime.manifest_entries,
                    capture,
                    failure,
                    detail.as_deref(),
                ) {
                    error!("ui audit failure output write failed: {error}");
                }
            }
            request_exit_if_needed(&mut runtime, &config, &mut app_exit);
        }
        None => {}
    }
}

fn current_capture_plan(runtime: &UiAuditRuntime) -> Option<&UiAuditCapturePlan> {
    runtime
        .plan
        .as_ref()
        .and_then(|plan| plan.captures.get(runtime.capture_index))
}

fn request_exit_if_needed(
    runtime: &mut UiAuditRuntime,
    config: &UiAuditConfig,
    app_exit: &mut MessageWriter<AppExit>,
) {
    if config.exit_on_finish && !runtime.exit_requested {
        runtime.exit_requested = true;
        app_exit.write(AppExit::Success);
    }
}

struct UiAuditPlanError {
    failure: UiAuditFailureKind,
    detail: String,
}

fn prepare_runtime_plan(
    config: &UiAuditConfig,
    registry: &UiAuditScreenRegistry,
    primary_window: &Query<&Window, With<PrimaryWindow>>,
) -> Result<UiAuditRunPlan, UiAuditPlanError> {
    if let Some(failure) = config.config_error {
        return Err(UiAuditPlanError {
            failure,
            detail: "invalid local audit configuration".to_owned(),
        });
    }

    let requested = config.screen.as_ref().ok_or_else(|| UiAuditPlanError {
        failure: UiAuditFailureKind::ConfigInvalid,
        detail: "screen alias is required when local UI audit is enabled".to_owned(),
    })?;
    let screen = registry
        .resolve(requested)
        .ok_or_else(|| UiAuditPlanError {
            failure: UiAuditFailureKind::ScreenNotFound,
            detail: format!("screen alias '{requested}' was not registered"),
        })?;
    let device = primary_window
        .single()
        .ok()
        .map(device_label_from_window)
        .unwrap_or_else(|| "local".to_owned());
    let resolved = UiAuditResolvedScreen {
        requested: requested.clone(),
        canonical: screen.canonical.to_owned(),
        owner: screen.owner,
    };
    let captures = resolve_capture_plans(&config.states, config.states_from_env, screen).map_err(
        |detail| UiAuditPlanError {
            failure: UiAuditFailureKind::ConfigInvalid,
            detail,
        },
    )?;

    Ok(plan_audit_paths(
        &config.output_root,
        resolved,
        &device,
        screen.recipe.and_then(|recipe| recipe.ready),
        &captures,
    ))
}

fn plan_audit_paths(
    output_root: &Path,
    screen: UiAuditResolvedScreen,
    device: &str,
    ready_condition: Option<UiAuditReadyCondition>,
    captures: &[UiAuditCaptureRecipe],
) -> UiAuditRunPlan {
    let screen_segment = sanitize_filename_segment(&screen.canonical);
    let device_segment = sanitize_filename_segment(device);
    let capture_plans = captures
        .iter()
        .enumerate()
        .map(|(index, capture)| {
            plan_capture_paths(
                output_root,
                &screen_segment,
                &device_segment,
                index,
                *capture,
            )
        })
        .collect();

    UiAuditRunPlan {
        screen,
        output_root: output_root.to_path_buf(),
        manifest_path: output_root.join("manifest.json"),
        report_path: output_root.join("report.md"),
        device: device_segment,
        ready_condition,
        captures: capture_plans,
    }
}

fn plan_capture_paths(
    output_root: &Path,
    screen_segment: &str,
    device_segment: &str,
    index: usize,
    capture: UiAuditCaptureRecipe,
) -> UiAuditCapturePlan {
    let state_segment = sanitize_filename_segment(capture.state.as_str());
    let file_stem = format!("{index:02}-{state_segment}");

    UiAuditCapturePlan {
        index,
        state: capture.state,
        screenshot_path: output_root
            .join("screenshots")
            .join(screen_segment)
            .join(device_segment)
            .join(format!("{file_stem}.png")),
        metadata_path: output_root
            .join("metadata")
            .join(screen_segment)
            .join(device_segment)
            .join(format!("{file_stem}.json")),
        scroll: capture.scroll,
    }
}

fn resolve_capture_plans(
    requested_states: &[UiAuditCaptureState],
    states_from_env: bool,
    screen: &UiAuditScreen,
) -> Result<Vec<UiAuditCaptureRecipe>, String> {
    let Some(recipe) = screen.recipe else {
        if states_from_env
            && requested_states
                .iter()
                .any(|state| *state != UiAuditCaptureState::Initial)
        {
            return Err(format!(
                "screen '{}' has no recipe for requested capture states: {}",
                screen.canonical,
                join_capture_state_names(requested_states)
            ));
        }
        return Ok(vec![UiAuditCaptureRecipe::initial()]);
    };

    if !states_from_env {
        if recipe.captures.is_empty() {
            return Err(format!(
                "screen '{}' recipe does not declare any capture states",
                screen.canonical
            ));
        }
        return Ok(recipe.captures.to_vec());
    }

    let mut captures = Vec::with_capacity(requested_states.len());
    for state in requested_states {
        if *state == UiAuditCaptureState::Initial {
            captures.push(UiAuditCaptureRecipe::initial());
            continue;
        }
        let Some(capture) = recipe
            .captures
            .iter()
            .find(|capture| capture.state == *state)
            .copied()
        else {
            return Err(format!(
                "screen '{}' recipe does not declare capture state '{}'",
                screen.canonical,
                state.as_str()
            ));
        };
        captures.push(capture);
    }

    Ok(captures)
}

fn join_capture_state_names(states: &[UiAuditCaptureState]) -> String {
    states
        .iter()
        .map(|state| state.as_str())
        .collect::<Vec<_>>()
        .join(",")
}

fn advance_audit_phase(
    phase: UiAuditPhase,
    input: UiAuditStepInput,
) -> (UiAuditPhase, Option<UiAuditPureAction>) {
    match phase {
        UiAuditPhase::Init => (
            UiAuditPhase::EnterScreen,
            Some(UiAuditPureAction::RouteToScreen),
        ),
        UiAuditPhase::EnterScreen => (UiAuditPhase::WaitForScreen { waited_frames: 0 }, None),
        UiAuditPhase::WaitForScreen { waited_frames } => {
            if input.target_panel_ready {
                (UiAuditPhase::ApplyCaptureState, None)
            } else if waited_frames >= PANEL_READY_TIMEOUT_FRAMES {
                (
                    UiAuditPhase::Failed(UiAuditFailureKind::PanelNotReady),
                    Some(UiAuditPureAction::Fail(UiAuditFailureKind::PanelNotReady)),
                )
            } else {
                (
                    UiAuditPhase::WaitForScreen {
                        waited_frames: waited_frames.saturating_add(1),
                    },
                    None,
                )
            }
        }
        UiAuditPhase::ApplyCaptureState => (
            UiAuditPhase::WaitForStable { waited_frames: 0 },
            Some(UiAuditPureAction::ApplyCaptureState),
        ),
        UiAuditPhase::WaitForStable { waited_frames } => {
            if !input.target_panel_ready {
                (
                    UiAuditPhase::Failed(UiAuditFailureKind::UnstableUi),
                    Some(UiAuditPureAction::Fail(UiAuditFailureKind::UnstableUi)),
                )
            } else if waited_frames >= STABLE_WAIT_FRAMES {
                (
                    UiAuditPhase::RequestScreenshot,
                    Some(UiAuditPureAction::RequestScreenshot),
                )
            } else if waited_frames >= STABLE_TIMEOUT_FRAMES {
                (
                    UiAuditPhase::Failed(UiAuditFailureKind::UnstableUi),
                    Some(UiAuditPureAction::Fail(UiAuditFailureKind::UnstableUi)),
                )
            } else {
                (
                    UiAuditPhase::WaitForStable {
                        waited_frames: waited_frames.saturating_add(1),
                    },
                    None,
                )
            }
        }
        UiAuditPhase::RequestScreenshot => match input.screenshot_status {
            UiAuditScreenshotStatus::Saved => (
                UiAuditPhase::WriteCapture,
                Some(UiAuditPureAction::WriteCapture),
            ),
            UiAuditScreenshotStatus::Failed => (
                UiAuditPhase::Failed(UiAuditFailureKind::ScreenshotFailed),
                Some(UiAuditPureAction::Fail(
                    UiAuditFailureKind::ScreenshotFailed,
                )),
            ),
            UiAuditScreenshotStatus::Pending => {
                (UiAuditPhase::WaitForScreenshot { waited_frames: 0 }, None)
            }
        },
        UiAuditPhase::WaitForScreenshot { waited_frames } => match input.screenshot_status {
            UiAuditScreenshotStatus::Saved => (
                UiAuditPhase::WriteCapture,
                Some(UiAuditPureAction::WriteCapture),
            ),
            UiAuditScreenshotStatus::Failed => (
                UiAuditPhase::Failed(UiAuditFailureKind::ScreenshotFailed),
                Some(UiAuditPureAction::Fail(
                    UiAuditFailureKind::ScreenshotFailed,
                )),
            ),
            UiAuditScreenshotStatus::Pending => {
                if waited_frames >= SCREENSHOT_TIMEOUT_FRAMES {
                    (
                        UiAuditPhase::Failed(UiAuditFailureKind::ScreenshotFailed),
                        Some(UiAuditPureAction::Fail(
                            UiAuditFailureKind::ScreenshotFailed,
                        )),
                    )
                } else {
                    (
                        UiAuditPhase::WaitForScreenshot {
                            waited_frames: waited_frames.saturating_add(1),
                        },
                        None,
                    )
                }
            }
        },
        UiAuditPhase::WriteCapture => (UiAuditPhase::Finish, Some(UiAuditPureAction::Finish)),
        UiAuditPhase::Finish => (UiAuditPhase::Finish, None),
        UiAuditPhase::Failed(failure) => (UiAuditPhase::Failed(failure), None),
    }
}

fn consume_screenshot_status(
    screenshot_events: &mut MessageReader<UiScreenshotEvent>,
    capture: Option<&UiAuditCapturePlan>,
) -> UiAuditScreenshotStatus {
    let Some(capture) = capture else {
        return UiAuditScreenshotStatus::Pending;
    };
    let mut status = UiAuditScreenshotStatus::Pending;
    for event in screenshot_events.read() {
        match event {
            UiScreenshotEvent::Saved(saved) if saved.request.path == capture.screenshot_path => {
                status = UiAuditScreenshotStatus::Saved;
            }
            UiScreenshotEvent::Failed(failed) if failed.request.path == capture.screenshot_path => {
                status = UiAuditScreenshotStatus::Failed;
            }
            _ => {}
        }
    }
    status
}

fn target_owner_panel_ready(owner: UiOwnerId, panels: &Query<&UiPanelRoot>) -> bool {
    panels.iter().any(|panel| panel.owner == Some(owner))
}

fn apply_capture_state(
    capture: &UiAuditCapturePlan,
    scroll_targets: &mut Query<
        (
            &UiScrollAuditId,
            &mut ScrollPosition,
            &ComputedNode,
            &UiGlobalTransform,
        ),
        With<UiScrollView>,
    >,
    scroll_anchors: &Query<
        (&UiScrollAuditAnchorId, &ComputedNode, &UiGlobalTransform),
        Without<UiScrollView>,
    >,
) -> Result<(), (UiAuditFailureKind, String)> {
    let Some(scroll) = capture.scroll else {
        return Ok(());
    };

    for (id, mut position, computed, transform) in scroll_targets.iter_mut() {
        if *id != scroll.target_id {
            continue;
        }
        let result = match scroll.target {
            UiAuditScrollTarget::Position(target) => {
                set_scroll_audit_position(&mut position, computed, target).and_then(|_| {
                    scroll_audit_position_reached(&position, computed, target)
                        .then_some(())
                        .ok_or(crate::framework::ui::widgets::UiScrollAuditSetError::Unreachable)
                })
            }
            UiAuditScrollTarget::Anchor(anchor_id) => {
                let Some((_, anchor_computed, anchor_transform)) =
                    scroll_anchors.iter().find(|(id, _, _)| **id == anchor_id)
                else {
                    return Err((
                        UiAuditFailureKind::ScrollTargetMissing,
                        format!(
                            "scroll anchor '{}' was not found for capture state '{}'",
                            anchor_id,
                            capture.state.as_str()
                        ),
                    ));
                };
                set_scroll_audit_anchor(
                    &mut position,
                    computed,
                    transform,
                    anchor_computed,
                    anchor_transform,
                )
                .map(|_| ())
            }
        };
        return result.map_err(|_| {
            (
                UiAuditFailureKind::ScrollTargetUnreachable,
                format!(
                    "scroll target '{}' cannot reach '{}' for capture state '{}'",
                    scroll.target_id,
                    scroll.target.as_str(),
                    capture.state.as_str()
                ),
            )
        });
    }

    Err((
        UiAuditFailureKind::ScrollTargetMissing,
        format!(
            "scroll target '{}' was not found for capture state '{}'",
            scroll.target_id,
            capture.state.as_str()
        ),
    ))
}

fn capture_scroll_metadata(
    capture: &UiAuditCapturePlan,
    scroll_targets: &mut Query<
        (
            &UiScrollAuditId,
            &mut ScrollPosition,
            &ComputedNode,
            &UiGlobalTransform,
        ),
        With<UiScrollView>,
    >,
) -> Option<UiAuditScrollMetadata> {
    let scroll = capture.scroll?;
    scroll_targets
        .iter_mut()
        .find(|(id, _, _, _)| **id == scroll.target_id)
        .map(|(id, position, computed, _)| {
            UiAuditScrollMetadata::from_metrics(
                *id,
                scroll_audit_metrics(&position, computed, UiScrollAuditPosition::Top),
                scroll.target,
            )
        })
}

fn failure_detail(
    failure: UiAuditFailureKind,
    plan: Option<&UiAuditRunPlan>,
    capture: Option<&UiAuditCapturePlan>,
    screenshot_status: UiAuditScreenshotStatus,
) -> Option<String> {
    match failure {
        UiAuditFailureKind::PanelNotReady => plan.map(|plan| {
            format!(
                "target owner '{}' did not produce a root panel before timeout",
                plan.screen.owner
            )
        }),
        UiAuditFailureKind::UnstableUi => plan.map(|plan| {
            format!(
                "target owner '{}' disappeared before stable capture",
                plan.screen.owner
            )
        }),
        UiAuditFailureKind::ScreenshotFailed => {
            Some(format!("screenshot status ended as {screenshot_status:?}"))
        }
        UiAuditFailureKind::ScrollTargetMissing => capture.and_then(|capture| {
            capture.scroll.map(|scroll| {
                format!(
                    "scroll target '{}' was not found for capture state '{}'",
                    scroll.target_id,
                    capture.state.as_str()
                )
            })
        }),
        UiAuditFailureKind::ScrollTargetUnreachable => capture.and_then(|capture| {
            capture.scroll.map(|scroll| {
                format!(
                    "scroll target '{}' cannot reach '{}' for capture state '{}'",
                    scroll.target_id,
                    scroll.target.as_str(),
                    capture.state.as_str()
                )
            })
        }),
        UiAuditFailureKind::ScreenNotFound
        | UiAuditFailureKind::ConfigInvalid
        | UiAuditFailureKind::OutputWriteFailed => None,
    }
}

fn parse_capture_states(value: &str) -> (Vec<UiAuditCaptureState>, Option<UiAuditFailureKind>) {
    let raw_states: Vec<_> = value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect();
    if raw_states.is_empty() {
        return (
            vec![UiAuditCaptureState::Initial],
            Some(UiAuditFailureKind::ConfigInvalid),
        );
    }

    let mut states = Vec::with_capacity(raw_states.len());
    for state in raw_states {
        let Some(parsed) = parse_capture_state(state) else {
            return (
                vec![UiAuditCaptureState::Initial],
                Some(UiAuditFailureKind::ConfigInvalid),
            );
        };
        states.push(parsed);
    }
    (states, None)
}

fn parse_capture_state(value: &str) -> Option<UiAuditCaptureState> {
    if value.eq_ignore_ascii_case(INITIAL_CAPTURE_STATE) {
        Some(UiAuditCaptureState::Initial)
    } else if value.eq_ignore_ascii_case(VISUAL_FOUNDATION_CAPTURE_STATE) {
        Some(UiAuditCaptureState::VisualFoundation)
    } else if value.eq_ignore_ascii_case(IMAGE_FIT_CAPTURE_STATE) {
        Some(UiAuditCaptureState::ImageFit)
    } else if value.eq_ignore_ascii_case(IMAGE_MODES_CAPTURE_STATE) {
        Some(UiAuditCaptureState::ImageModes)
    } else if value.eq_ignore_ascii_case(IMAGE_TILING_CAPTURE_STATE) {
        Some(UiAuditCaptureState::ImageTiling)
    } else if value.eq_ignore_ascii_case(IMAGE_ATLAS_CAPTURE_STATE) {
        Some(UiAuditCaptureState::ImageAtlas)
    } else if value.eq_ignore_ascii_case(SCROLL_TOP_CAPTURE_STATE) {
        Some(UiAuditCaptureState::Top)
    } else if value.eq_ignore_ascii_case(SCROLL_MIDDLE_CAPTURE_STATE) {
        Some(UiAuditCaptureState::Middle)
    } else if value.eq_ignore_ascii_case(SCROLL_BOTTOM_CAPTURE_STATE) {
        Some(UiAuditCaptureState::Bottom)
    } else {
        None
    }
}

fn normalize_screen_alias(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace('-', "_")
}

fn device_label_from_window(window: &Window) -> String {
    format!(
        "local-{}x{}-physical-{}x{}",
        rounded_dimension(window.resolution.width()),
        rounded_dimension(window.resolution.height()),
        window.resolution.physical_width(),
        window.resolution.physical_height()
    )
}

fn rounded_dimension(value: f32) -> u32 {
    value.round().max(0.0) as u32
}

fn write_capture_metadata(
    capture: &UiAuditCapturePlan,
    metadata: &UiAuditMetadata,
) -> Result<(), String> {
    write_json_file(&capture.metadata_path, metadata)
}

fn write_run_outputs(plan: &UiAuditRunPlan, manifest: &UiAuditManifest) -> Result<(), String> {
    write_json_file(&plan.manifest_path, &manifest)?;
    write_report(plan, &manifest)
}

fn write_failure_outputs(
    plan: &UiAuditRunPlan,
    completed_entries: &[UiAuditManifestEntry],
    capture: &UiAuditCapturePlan,
    failure: UiAuditFailureKind,
    detail: Option<&str>,
) -> Result<(), String> {
    let mut entries = completed_entries.to_vec();
    entries.push(UiAuditManifestEntry::failure(
        plan, capture, failure, detail,
    ));
    let manifest = UiAuditManifest::new(entries);
    write_run_outputs(plan, &manifest)
}

fn write_planless_failure_outputs(
    config: &UiAuditConfig,
    primary_window: &Query<&Window, With<PrimaryWindow>>,
    failure: UiAuditFailureKind,
    detail: Option<&str>,
) -> Result<(), String> {
    let requested_screen = config
        .screen
        .clone()
        .unwrap_or_else(|| "unknown_screen".to_owned());
    let canonical = sanitize_filename_segment(&requested_screen);
    let device = primary_window
        .single()
        .ok()
        .map(device_label_from_window)
        .unwrap_or_else(|| "local".to_owned());
    let captures = [UiAuditCaptureRecipe::initial()];
    let plan = plan_audit_paths(
        &config.output_root,
        UiAuditResolvedScreen {
            requested: requested_screen,
            canonical,
            owner: UiOwnerId::new("unknown"),
        },
        &device,
        None,
        &captures,
    );

    let capture = plan
        .captures
        .first()
        .ok_or_else(|| "planless failure capture plan missing".to_owned())?;
    write_failure_outputs(&plan, &[], capture, failure, detail)
}

fn write_json_file<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let json = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    fs::write(path, json).map_err(|error| error.to_string())
}

fn write_report(plan: &UiAuditRunPlan, manifest: &UiAuditManifest) -> Result<(), String> {
    if let Some(parent) = plan
        .report_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    fs::write(&plan.report_path, build_report_markdown(plan, manifest))
        .map_err(|error| error.to_string())
}

fn build_report_markdown(plan: &UiAuditRunPlan, manifest: &UiAuditManifest) -> String {
    let entry = &manifest.entries[0];
    let display_root = absolute_display_path(&plan.output_root);
    let mut report = String::new();
    report.push_str("# UI Audit Report\n\n");
    report.push_str(&format!("- Screen: `{}`\n", entry.screen));
    report.push_str(&format!("- Device: `{}`\n", entry.device));
    report.push_str(&format!("- Status: `{}`\n", manifest.status_string()));
    if let Some(failure) = &entry.failure {
        report.push_str(&format!("- Failure: `{failure}`\n"));
    }
    if let Some(detail) = &entry.detail {
        report.push_str(&format!("- Detail: {detail}\n"));
    }
    report.push('\n');
    report.push_str("| State | Status | Screenshot | Metadata |\n");
    report.push_str("| --- | --- | --- | --- |\n");
    for entry in &manifest.entries {
        let screenshot_link =
            markdown_relative_path(&display_root, Path::new(&entry.screenshot_path));
        let metadata_link = markdown_relative_path(&display_root, Path::new(&entry.metadata_path));
        report.push_str(&format!(
            "| `{}` | `{}` | [screenshot]({}) | [metadata]({}) |\n",
            entry.state,
            entry.status_string(),
            screenshot_link,
            metadata_link
        ));
    }
    report
}

fn markdown_relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn build_capture_metadata(
    plan: &UiAuditRunPlan,
    capture: &UiAuditCapturePlan,
    scroll: Option<&UiAuditScrollMetadata>,
    viewport: &UiViewport,
    stats: &UiStats,
    current_owner: &UiCurrentOwner,
    panels: &Query<&UiPanelRoot>,
    primary_window: Option<&Window>,
) -> UiAuditMetadata {
    UiAuditMetadata {
        screen: plan.screen.canonical.clone(),
        requested_screen: plan.screen.requested.clone(),
        state: capture.state.as_str().to_owned(),
        device: plan.device.clone(),
        screenshot_path: absolute_display_path(&capture.screenshot_path)
            .to_string_lossy()
            .into_owned(),
        scroll: scroll.cloned(),
        viewport: UiAuditViewportMetadata::from(*viewport),
        current_page: current_owner.owner.map(|owner| owner.as_str().to_owned()),
        panels: panels.iter().map(UiAuditPanelMetadata::from).collect(),
        window: primary_window.map(UiAuditWindowMetadata::from),
        stats: UiAuditStatsMetadata::from(stats),
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
struct UiAuditMetadata {
    screen: String,
    requested_screen: String,
    state: String,
    device: String,
    screenshot_path: String,
    scroll: Option<UiAuditScrollMetadata>,
    viewport: UiAuditViewportMetadata,
    current_page: Option<String>,
    panels: Vec<UiAuditPanelMetadata>,
    window: Option<UiAuditWindowMetadata>,
    stats: UiAuditStatsMetadata,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
struct UiAuditScrollMetadata {
    target_id: String,
    offset: f32,
    max_offset: f32,
    viewport_height: f32,
    content_height: f32,
    position: String,
}

impl UiAuditScrollMetadata {
    fn from_metrics(
        target_id: UiScrollAuditId,
        metrics: UiScrollAuditMetrics,
        target: UiAuditScrollTarget,
    ) -> Self {
        Self {
            target_id: target_id.as_str().to_owned(),
            offset: metrics.offset,
            max_offset: metrics.max_offset,
            viewport_height: metrics.viewport_height,
            content_height: metrics.content_height,
            position: target.as_str().to_owned(),
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
struct UiAuditViewportMetadata {
    logical_width: f32,
    logical_height: f32,
    window_logical_width: f32,
    window_logical_height: f32,
    device_width: f32,
    device_height: f32,
    device_scale: f32,
    preview_scale: f32,
    width_class: &'static str,
    height_class: &'static str,
    orientation: &'static str,
    input_mode: &'static str,
    safe_area: UiAuditSafeAreaMetadata,
}

impl From<UiViewport> for UiAuditViewportMetadata {
    fn from(viewport: UiViewport) -> Self {
        Self {
            logical_width: viewport.logical_width,
            logical_height: viewport.logical_height,
            window_logical_width: viewport.window_logical_width,
            window_logical_height: viewport.window_logical_height,
            device_width: viewport.device_width,
            device_height: viewport.device_height,
            device_scale: viewport.device_scale,
            preview_scale: viewport.preview_scale,
            width_class: width_class_name(viewport.width_class),
            height_class: height_class_name(viewport.height_class),
            orientation: orientation_name(viewport.orientation),
            input_mode: input_mode_name(viewport.input_mode),
            safe_area: UiAuditSafeAreaMetadata {
                left: viewport.safe_area.left,
                right: viewport.safe_area.right,
                top: viewport.safe_area.top,
                bottom: viewport.safe_area.bottom,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
struct UiAuditSafeAreaMetadata {
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct UiAuditPanelMetadata {
    id: String,
    kind: &'static str,
    owner: Option<String>,
}

impl From<&UiPanelRoot> for UiAuditPanelMetadata {
    fn from(panel: &UiPanelRoot) -> Self {
        Self {
            id: panel.id.as_str().to_owned(),
            kind: panel_kind_name(panel.kind),
            owner: panel.owner.map(|owner| owner.as_str().to_owned()),
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
struct UiAuditWindowMetadata {
    logical_width: f32,
    logical_height: f32,
    physical_width: u32,
    physical_height: u32,
    scale_factor: f32,
}

impl From<&Window> for UiAuditWindowMetadata {
    fn from(window: &Window) -> Self {
        Self {
            logical_width: window.resolution.width(),
            logical_height: window.resolution.height(),
            physical_width: window.resolution.physical_width(),
            physical_height: window.resolution.physical_height(),
            scale_factor: window.resolution.scale_factor(),
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
struct UiAuditStatsMetadata {
    ui_node_count: usize,
    visible_ui_node_count: usize,
    panel_count: usize,
    text_node_count: usize,
}

impl From<&UiStats> for UiAuditStatsMetadata {
    fn from(stats: &UiStats) -> Self {
        Self {
            ui_node_count: stats.ui_node_count,
            visible_ui_node_count: stats.visible_ui_node_count,
            panel_count: stats.panel_count,
            text_node_count: stats.text_node_count,
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct UiAuditManifest {
    mode: &'static str,
    entries: Vec<UiAuditManifestEntry>,
}

impl UiAuditManifest {
    fn new(entries: Vec<UiAuditManifestEntry>) -> Self {
        Self {
            mode: "local_once",
            entries,
        }
    }

    fn status_string(&self) -> &'static str {
        if self
            .entries
            .iter()
            .any(|entry| entry.status == UiAuditRunStatus::Failed)
        {
            "failed"
        } else {
            "passed"
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct UiAuditManifestEntry {
    screen: String,
    requested_screen: String,
    device: String,
    state: String,
    screenshot_path: String,
    metadata_path: String,
    scroll_target_id: Option<String>,
    scroll_position: Option<String>,
    status: UiAuditRunStatus,
    failure: Option<String>,
    detail: Option<String>,
}

impl UiAuditManifestEntry {
    fn success(plan: &UiAuditRunPlan, capture: &UiAuditCapturePlan) -> Self {
        Self::new(plan, capture, UiAuditRunStatus::Passed, None, None)
    }

    fn failure(
        plan: &UiAuditRunPlan,
        capture: &UiAuditCapturePlan,
        failure: UiAuditFailureKind,
        detail: Option<&str>,
    ) -> Self {
        Self::new(
            plan,
            capture,
            UiAuditRunStatus::Failed,
            Some(failure.as_str()),
            detail,
        )
    }

    fn new(
        plan: &UiAuditRunPlan,
        capture: &UiAuditCapturePlan,
        status: UiAuditRunStatus,
        failure: Option<&str>,
        detail: Option<&str>,
    ) -> Self {
        Self {
            screen: plan.screen.canonical.clone(),
            requested_screen: plan.screen.requested.clone(),
            device: plan.device.clone(),
            state: capture.state.as_str().to_owned(),
            screenshot_path: absolute_display_path(&capture.screenshot_path)
                .to_string_lossy()
                .into_owned(),
            metadata_path: absolute_display_path(&capture.metadata_path)
                .to_string_lossy()
                .into_owned(),
            scroll_target_id: capture
                .scroll
                .map(|scroll| scroll.target_id.as_str().to_owned()),
            scroll_position: capture
                .scroll
                .map(|scroll| scroll.target.as_str().to_owned()),
            status,
            failure: failure.map(str::to_owned),
            detail: detail.map(str::to_owned),
        }
    }

    const fn status_string(&self) -> &'static str {
        match self.status {
            UiAuditRunStatus::Passed => "passed",
            UiAuditRunStatus::Failed => "failed",
        }
    }
}

#[derive(Clone, Debug, Message, PartialEq, Eq)]
pub(crate) struct UiAuditRouteCommand {
    pub screen: String,
    pub owner: UiOwnerId,
}

fn width_class_name(value: UiWidthClass) -> &'static str {
    match value {
        UiWidthClass::Compact => "compact",
        UiWidthClass::Medium => "medium",
        UiWidthClass::Expanded => "expanded",
    }
}

fn height_class_name(value: UiHeightClass) -> &'static str {
    match value {
        UiHeightClass::Short => "short",
        UiHeightClass::Regular => "regular",
        UiHeightClass::Tall => "tall",
    }
}

fn orientation_name(value: UiOrientation) -> &'static str {
    match value {
        UiOrientation::Portrait => "portrait",
        UiOrientation::Landscape => "landscape",
    }
}

fn input_mode_name(value: UiInputMode) -> &'static str {
    match value {
        UiInputMode::MouseTouch => "mouse_touch",
        UiInputMode::Touch => "touch",
        UiInputMode::MouseKeyboard => "mouse_keyboard",
    }
}

fn panel_kind_name(value: UiPanelKind) -> &'static str {
    match value {
        UiPanelKind::Page => "page",
        UiPanelKind::Hud => "hud",
        UiPanelKind::Floating => "floating",
        UiPanelKind::Modal => "modal",
        UiPanelKind::BlockingOverlay => "blocking_overlay",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_reader<'a>(values: &'a [(&'a str, &'a str)]) -> impl FnMut(&str) -> Option<String> + 'a {
        move |key| {
            values
                .iter()
                .find_map(|(candidate, value)| (*candidate == key).then(|| (*value).to_owned()))
        }
    }

    fn step(
        phase: UiAuditPhase,
        target_panel_ready: bool,
        screenshot_status: UiAuditScreenshotStatus,
    ) -> (UiAuditPhase, Option<UiAuditPureAction>) {
        advance_audit_phase(
            phase,
            UiAuditStepInput {
                target_panel_ready,
                screenshot_status,
            },
        )
    }

    const TEST_SCROLL_ID: UiScrollAuditId = UiScrollAuditId::new("test.scroll");
    const TEST_SCROLL_CAPTURES: &[UiAuditCaptureRecipe] = &[
        UiAuditCaptureRecipe::scroll(
            UiAuditCaptureState::Top,
            TEST_SCROLL_ID,
            UiScrollAuditPosition::Top,
        ),
        UiAuditCaptureRecipe::scroll(
            UiAuditCaptureState::Middle,
            TEST_SCROLL_ID,
            UiScrollAuditPosition::Middle,
        ),
        UiAuditCaptureRecipe::scroll(
            UiAuditCaptureState::Bottom,
            TEST_SCROLL_ID,
            UiScrollAuditPosition::Bottom,
        ),
    ];
    const TEST_TOP_ONLY_CAPTURES: &[UiAuditCaptureRecipe] = &[UiAuditCaptureRecipe::scroll(
        UiAuditCaptureState::Top,
        TEST_SCROLL_ID,
        UiScrollAuditPosition::Top,
    )];

    fn resolved_test_screen() -> UiAuditResolvedScreen {
        UiAuditResolvedScreen {
            requested: "ui-gallery".to_owned(),
            canonical: "ui_gallery".to_owned(),
            owner: UiOwnerId::new("ui_gallery"),
        }
    }

    #[test]
    fn config_defaults_to_disabled_local_once_mode() {
        let config = UiAuditConfig::from_env_reader(env_reader(&[]), 100);

        assert!(!config.enabled);
        assert_eq!(
            config.output_root,
            PathBuf::from(DEFAULT_AUDIT_OUTPUT_ROOT).join("100")
        );
        assert_eq!(config.states, vec![UiAuditCaptureState::Initial]);
        assert!(!config.exit_on_finish);
        assert!(config.config_error.is_none());
    }

    #[test]
    fn config_reads_local_once_env_values() {
        let config = UiAuditConfig::from_env_reader(
            env_reader(&[
                (ENV_UI_AUDIT, "1"),
                (ENV_UI_AUDIT_SCREEN, "ui-gallery"),
                (ENV_UI_AUDIT_OUTPUT, "../summary/ui-audit/custom"),
                (ENV_UI_AUDIT_STATES, "initial"),
                (ENV_UI_AUDIT_EXIT_ON_FINISH, "true"),
            ]),
            100,
        );

        assert!(config.enabled);
        assert_eq!(config.screen.as_deref(), Some("ui-gallery"));
        assert_eq!(
            config.output_root,
            PathBuf::from("../summary/ui-audit/custom")
        );
        assert_eq!(config.states, vec![UiAuditCaptureState::Initial]);
        assert!(config.states_from_env);
        assert!(config.exit_on_finish);
        assert!(config.config_error.is_none());
    }

    #[test]
    fn config_accepts_scroll_capture_states() {
        let config = UiAuditConfig::from_env_reader(
            env_reader(&[
                (ENV_UI_AUDIT, "1"),
                (ENV_UI_AUDIT_SCREEN, "ui-gallery"),
                (ENV_UI_AUDIT_STATES, "top,middle,bottom"),
            ]),
            100,
        );

        assert_eq!(
            config.states,
            vec![
                UiAuditCaptureState::Top,
                UiAuditCaptureState::Middle,
                UiAuditCaptureState::Bottom
            ]
        );
        assert!(config.states_from_env);
        assert!(config.config_error.is_none());
    }

    #[test]
    fn config_accepts_visual_foundation_capture_state() {
        let config = UiAuditConfig::from_env_reader(
            env_reader(&[
                (ENV_UI_AUDIT, "1"),
                (ENV_UI_AUDIT_SCREEN, "ui-gallery"),
                (ENV_UI_AUDIT_STATES, "visual_foundation"),
            ]),
            100,
        );

        assert_eq!(config.states, vec![UiAuditCaptureState::VisualFoundation]);
        assert!(config.states_from_env);
        assert!(config.config_error.is_none());
    }

    #[test]
    fn config_accepts_image_fit_capture_state() {
        let config = UiAuditConfig::from_env_reader(
            env_reader(&[
                (ENV_UI_AUDIT, "1"),
                (ENV_UI_AUDIT_SCREEN, "ui-gallery"),
                (ENV_UI_AUDIT_STATES, "image_fit"),
            ]),
            100,
        );

        assert_eq!(config.states, vec![UiAuditCaptureState::ImageFit]);
        assert!(config.states_from_env);
        assert!(config.config_error.is_none());
    }

    #[test]
    fn config_accepts_image_modes_capture_state() {
        let config = UiAuditConfig::from_env_reader(
            env_reader(&[
                (ENV_UI_AUDIT, "1"),
                (ENV_UI_AUDIT_SCREEN, "ui-gallery"),
                (ENV_UI_AUDIT_STATES, "image_modes,image_tiling,image_atlas"),
            ]),
            100,
        );

        assert_eq!(
            config.states,
            vec![
                UiAuditCaptureState::ImageModes,
                UiAuditCaptureState::ImageTiling,
                UiAuditCaptureState::ImageAtlas,
            ]
        );
        assert!(config.states_from_env);
        assert!(config.config_error.is_none());
    }

    #[test]
    fn config_rejects_unknown_capture_states() {
        let config = UiAuditConfig::from_env_reader(
            env_reader(&[
                (ENV_UI_AUDIT, "1"),
                (ENV_UI_AUDIT_SCREEN, "ui-gallery"),
                (ENV_UI_AUDIT_STATES, "top,unknown"),
            ]),
            100,
        );

        assert_eq!(config.config_error, Some(UiAuditFailureKind::ConfigInvalid));
    }

    #[test]
    fn config_requires_screen_when_enabled() {
        let config = UiAuditConfig::from_env_reader(env_reader(&[(ENV_UI_AUDIT, "1")]), 100);

        assert_eq!(config.config_error, Some(UiAuditFailureKind::ConfigInvalid));
    }

    #[test]
    fn failure_kind_strings_are_stable() {
        assert_eq!(
            UiAuditFailureKind::ScreenNotFound.as_str(),
            "screen_not_found"
        );
        assert_eq!(
            UiAuditFailureKind::PanelNotReady.as_str(),
            "panel_not_ready"
        );
        assert_eq!(UiAuditFailureKind::UnstableUi.as_str(), "unstable_ui");
        assert_eq!(
            UiAuditFailureKind::ScreenshotFailed.as_str(),
            "screenshot_failed"
        );
        assert_eq!(
            UiAuditFailureKind::ScrollTargetMissing.as_str(),
            "scroll_target_missing"
        );
        assert_eq!(
            UiAuditFailureKind::ScrollTargetUnreachable.as_str(),
            "scroll_target_unreachable"
        );
    }

    #[test]
    fn registry_resolves_canonical_and_alias_names() {
        let mut registry = UiAuditScreenRegistry::default();
        registry.register(UiAuditScreen::new(
            "ui_gallery",
            &["ui-gallery", "gallery"],
            UiOwnerId::new("ui_gallery"),
        ));

        assert_eq!(
            registry.resolve("ui_gallery").map(|screen| screen.owner),
            Some(UiOwnerId::new("ui_gallery"))
        );
        assert_eq!(
            registry.resolve("ui-gallery").map(|screen| screen.owner),
            Some(UiOwnerId::new("ui_gallery"))
        );
        assert_eq!(
            registry.resolve("gallery").map(|screen| screen.owner),
            Some(UiOwnerId::new("ui_gallery"))
        );
        assert!(registry.resolve("missing").is_none());
    }

    #[test]
    fn path_plan_uses_multi_capture_layout() {
        let captures = [
            UiAuditCaptureRecipe::scroll(
                UiAuditCaptureState::Top,
                TEST_SCROLL_ID,
                UiScrollAuditPosition::Top,
            ),
            UiAuditCaptureRecipe::scroll(
                UiAuditCaptureState::Middle,
                TEST_SCROLL_ID,
                UiScrollAuditPosition::Middle,
            ),
            UiAuditCaptureRecipe::scroll(
                UiAuditCaptureState::Bottom,
                TEST_SCROLL_ID,
                UiScrollAuditPosition::Bottom,
            ),
        ];
        let plan = plan_audit_paths(
            Path::new("../summary/ui-audit/run-1"),
            resolved_test_screen(),
            "phone-small",
            None,
            &captures,
        );

        assert_eq!(
            plan.captures[0].screenshot_path,
            PathBuf::from(
                "../summary/ui-audit/run-1/screenshots/ui_gallery/phone-small/00-top.png"
            )
        );
        assert_eq!(
            plan.captures[1].metadata_path,
            PathBuf::from(
                "../summary/ui-audit/run-1/metadata/ui_gallery/phone-small/01-middle.json"
            )
        );
        assert_eq!(
            plan.captures[2].screenshot_path,
            PathBuf::from(
                "../summary/ui-audit/run-1/screenshots/ui_gallery/phone-small/02-bottom.png"
            )
        );
        assert_eq!(
            plan.manifest_path,
            PathBuf::from("../summary/ui-audit/run-1/manifest.json")
        );
        assert_eq!(
            plan.report_path,
            PathBuf::from("../summary/ui-audit/run-1/report.md")
        );
    }

    #[test]
    fn state_machine_routes_then_waits_for_panel() {
        assert_eq!(
            step(UiAuditPhase::Init, false, UiAuditScreenshotStatus::Pending),
            (
                UiAuditPhase::EnterScreen,
                Some(UiAuditPureAction::RouteToScreen)
            )
        );
        assert_eq!(
            step(
                UiAuditPhase::EnterScreen,
                false,
                UiAuditScreenshotStatus::Pending
            ),
            (UiAuditPhase::WaitForScreen { waited_frames: 0 }, None)
        );
    }

    #[test]
    fn state_machine_fails_when_panel_never_ready() {
        assert_eq!(
            step(
                UiAuditPhase::WaitForScreen {
                    waited_frames: PANEL_READY_TIMEOUT_FRAMES
                },
                false,
                UiAuditScreenshotStatus::Pending
            ),
            (
                UiAuditPhase::Failed(UiAuditFailureKind::PanelNotReady),
                Some(UiAuditPureAction::Fail(UiAuditFailureKind::PanelNotReady))
            )
        );
    }

    #[test]
    fn state_machine_applies_capture_state_after_panel_is_ready() {
        assert_eq!(
            step(
                UiAuditPhase::WaitForScreen { waited_frames: 2 },
                true,
                UiAuditScreenshotStatus::Pending
            ),
            (UiAuditPhase::ApplyCaptureState, None)
        );
        assert_eq!(
            step(
                UiAuditPhase::ApplyCaptureState,
                true,
                UiAuditScreenshotStatus::Pending
            ),
            (
                UiAuditPhase::WaitForStable { waited_frames: 0 },
                Some(UiAuditPureAction::ApplyCaptureState)
            )
        );
    }

    #[test]
    fn state_machine_waits_fixed_stable_frames_before_screenshot() {
        assert_eq!(
            step(
                UiAuditPhase::WaitForStable { waited_frames: 4 },
                true,
                UiAuditScreenshotStatus::Pending
            ),
            (UiAuditPhase::WaitForStable { waited_frames: 5 }, None)
        );
        assert_eq!(
            step(
                UiAuditPhase::WaitForStable {
                    waited_frames: STABLE_WAIT_FRAMES
                },
                true,
                UiAuditScreenshotStatus::Pending
            ),
            (
                UiAuditPhase::RequestScreenshot,
                Some(UiAuditPureAction::RequestScreenshot)
            )
        );
    }

    #[test]
    fn state_machine_classifies_unstable_ui_when_panel_disappears() {
        assert_eq!(
            step(
                UiAuditPhase::WaitForStable { waited_frames: 2 },
                false,
                UiAuditScreenshotStatus::Pending
            ),
            (
                UiAuditPhase::Failed(UiAuditFailureKind::UnstableUi),
                Some(UiAuditPureAction::Fail(UiAuditFailureKind::UnstableUi))
            )
        );
    }

    #[test]
    fn state_machine_writes_capture_after_saved_screenshot() {
        assert_eq!(
            step(
                UiAuditPhase::WaitForScreenshot { waited_frames: 2 },
                true,
                UiAuditScreenshotStatus::Saved
            ),
            (
                UiAuditPhase::WriteCapture,
                Some(UiAuditPureAction::WriteCapture)
            )
        );
    }

    #[test]
    fn state_machine_classifies_screenshot_failure() {
        assert_eq!(
            step(
                UiAuditPhase::WaitForScreenshot { waited_frames: 2 },
                true,
                UiAuditScreenshotStatus::Failed
            ),
            (
                UiAuditPhase::Failed(UiAuditFailureKind::ScreenshotFailed),
                Some(UiAuditPureAction::Fail(
                    UiAuditFailureKind::ScreenshotFailed
                ))
            )
        );
    }

    #[test]
    fn report_links_screenshot_and_metadata() {
        let captures = [UiAuditCaptureRecipe::initial()];
        let plan = plan_audit_paths(
            Path::new("../summary/ui-audit/run-1"),
            resolved_test_screen(),
            "phone-small",
            None,
            &captures,
        );
        let manifest = UiAuditManifest::new(vec![UiAuditManifestEntry::success(
            &plan,
            &plan.captures[0],
        )]);
        let report = build_report_markdown(&plan, &manifest);

        assert!(report.contains("[screenshot](screenshots/ui_gallery/phone-small/00-initial.png)"));
        assert!(report.contains("[metadata](metadata/ui_gallery/phone-small/00-initial.json)"));
    }

    #[test]
    fn report_lists_multiple_capture_entries() {
        let plan = plan_audit_paths(
            Path::new("../summary/ui-audit/run-1"),
            resolved_test_screen(),
            "phone-small",
            None,
            TEST_SCROLL_CAPTURES,
        );
        let manifest = UiAuditManifest::new(
            plan.captures
                .iter()
                .map(|capture| UiAuditManifestEntry::success(&plan, capture))
                .collect(),
        );
        let report = build_report_markdown(&plan, &manifest);

        assert!(report.contains("00-top.png"));
        assert!(report.contains("01-middle.png"));
        assert!(report.contains("02-bottom.png"));
    }

    #[test]
    fn recipe_defaults_to_declared_captures_when_states_are_not_from_env() {
        let screen =
            UiAuditScreen::new("ui_gallery", &["ui-gallery"], UiOwnerId::new("ui_gallery"))
                .with_recipe(UiAuditRecipe::new(TEST_SCROLL_CAPTURES));

        let captures =
            resolve_capture_plans(&[UiAuditCaptureState::Initial], false, &screen).unwrap();

        assert_eq!(captures, TEST_SCROLL_CAPTURES);
    }

    #[test]
    fn recipe_filters_explicit_capture_states() {
        let screen =
            UiAuditScreen::new("ui_gallery", &["ui-gallery"], UiOwnerId::new("ui_gallery"))
                .with_recipe(UiAuditRecipe::new(TEST_SCROLL_CAPTURES));

        let captures = resolve_capture_plans(
            &[UiAuditCaptureState::Bottom, UiAuditCaptureState::Top],
            true,
            &screen,
        )
        .unwrap();

        assert_eq!(captures.len(), 2);
        assert_eq!(captures[0].state, UiAuditCaptureState::Bottom);
        assert_eq!(captures[1].state, UiAuditCaptureState::Top);
    }

    #[test]
    fn recipe_rejects_scroll_state_when_screen_has_no_recipe() {
        let screen = UiAuditScreen::new("login", &["login"], UiOwnerId::new("login"));

        let error = resolve_capture_plans(&[UiAuditCaptureState::Bottom], true, &screen)
            .expect_err("scroll capture requires a recipe");

        assert!(error.contains("has no recipe"));
    }

    #[test]
    fn recipe_rejects_missing_declared_state() {
        let screen =
            UiAuditScreen::new("ui_gallery", &["ui-gallery"], UiOwnerId::new("ui_gallery"))
                .with_recipe(UiAuditRecipe::new(TEST_TOP_ONLY_CAPTURES));

        let error = resolve_capture_plans(&[UiAuditCaptureState::Bottom], true, &screen)
            .expect_err("missing recipe state should fail");

        assert!(error.contains("does not declare capture state 'bottom'"));
    }

    #[test]
    fn manifest_entry_records_scroll_target_and_position() {
        let plan = plan_audit_paths(
            Path::new("../summary/ui-audit/run-1"),
            resolved_test_screen(),
            "phone-small",
            None,
            TEST_SCROLL_CAPTURES,
        );

        let entry = UiAuditManifestEntry::success(&plan, &plan.captures[1]);

        assert_eq!(entry.scroll_target_id.as_deref(), Some("test.scroll"));
        assert_eq!(entry.scroll_position.as_deref(), Some("middle"));
        assert_eq!(entry.status, UiAuditRunStatus::Passed);
    }
}
