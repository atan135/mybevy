use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    env, fmt, fs,
    path::{Path, PathBuf},
    time::Duration,
};

use bevy::{
    app::AppExit, ecs::system::SystemParam, prelude::*, time::TimeUpdateStrategy,
    window::PrimaryWindow,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::framework::ui::{
    audit::screenshot::{
        UiScreenshotEvent, UiScreenshotPlugin, UiScreenshotSystems, absolute_display_path,
        current_unix_timestamp_seconds, read_bool, sanitize_filename_segment,
    },
    audit::semantic::{UiAuditSemanticTree, UiAuditSemanticWorld, collect_semantic_tree},
    core::{
        UiAnimationDebugSnapshot, UiCurrentOwner, UiHeightClass, UiInputMode, UiMotionPolicy,
        UiOrientation, UiOwnerId, UiPanelKind, UiPanelRoot, UiSafeAreaStatus, UiViewport,
        UiWidthClass, stats::UiStats,
    },
    document::{
        UiDocumentInstanceId, UiDocumentNodeAuditMarker, UiDocumentResolvedStyleMarker,
        UiDocumentRuntimeRoot,
    },
    i18n::UiI18n,
    style::{
        UiFontResolution, UiResolvedEffectDebugSnapshot, UiResolvedStyleDebugSnapshot,
        UiTextStyleToken, UiThemeSource,
    },
    visual::{UiVisualBudgetProfile, UiVisualBudgetReport, UiVisualBudgetUsage},
    widgets::{
        DisabledButton, FocusedButton, UiBadge, UiControlFlags, UiControlMeta, UiControlState,
        UiImageStatus, UiImageWidget, UiProgress, UiScrollAuditAnchorId, UiScrollAuditId,
        UiScrollAuditMetrics, UiScrollAuditPosition, UiScrollView, UiTooltip, UiTooltipTone,
        resolve_control_state, scroll_audit_metrics, scroll_audit_position_reached,
        set_scroll_audit_anchor, set_scroll_audit_position,
    },
};

const ENV_UI_AUDIT: &str = "MYBEVY_UI_AUDIT";
const ENV_UI_AUDIT_SCREEN: &str = "MYBEVY_UI_AUDIT_SCREEN";
const ENV_UI_AUDIT_OUTPUT: &str = "MYBEVY_UI_AUDIT_OUTPUT";
const ENV_UI_AUDIT_STATES: &str = "MYBEVY_UI_AUDIT_STATES";
const ENV_UI_AUDIT_EXIT_ON_FINISH: &str = "MYBEVY_UI_AUDIT_EXIT_ON_FINISH";
const ENV_UI_AUDIT_DETERMINISTIC: &str = "MYBEVY_UI_AUDIT_DETERMINISTIC";
const ENV_UI_AUDIT_TARGET_LOGICAL_WIDTH: &str = "MYBEVY_UI_AUDIT_TARGET_LOGICAL_WIDTH";
const ENV_UI_AUDIT_TARGET_LOGICAL_HEIGHT: &str = "MYBEVY_UI_AUDIT_TARGET_LOGICAL_HEIGHT";
const ENV_UI_AUDIT_TARGET_PHYSICAL_WIDTH: &str = "MYBEVY_UI_AUDIT_TARGET_PHYSICAL_WIDTH";
const ENV_UI_AUDIT_TARGET_PHYSICAL_HEIGHT: &str = "MYBEVY_UI_AUDIT_TARGET_PHYSICAL_HEIGHT";
const ENV_UI_AUDIT_TARGET_DEVICE_SCALE: &str = "MYBEVY_UI_AUDIT_TARGET_DEVICE_SCALE";
const ENV_UI_AUDIT_LOCALE: &str = "MYBEVY_UI_AUDIT_LOCALE";
const ENV_UI_AUDIT_THEME: &str = "MYBEVY_UI_AUDIT_THEME";
const ENV_UI_AUDIT_RANDOM_SEED: &str = "MYBEVY_UI_AUDIT_RANDOM_SEED";
const ENV_UI_AUDIT_FROZEN_TIME_SECONDS: &str = "MYBEVY_UI_AUDIT_FROZEN_TIME_SECONDS";
const ENV_UI_AUDIT_ANIMATION_PROGRESS: &str = "MYBEVY_UI_AUDIT_ANIMATION_PROGRESS";
const ENV_UI_AUDIT_DYNAMIC_POLICY: &str = "MYBEVY_UI_AUDIT_DYNAMIC_POLICY";
const ENV_UI_AUDIT_STABLE_FIXTURE_ID: &str = "MYBEVY_UI_AUDIT_STABLE_FIXTURE_ID";
const ENV_UI_AUDIT_DYNAMIC_MASK_ID: &str = "MYBEVY_UI_AUDIT_DYNAMIC_MASK_ID";
const ENV_UI_AUDIT_REPEAT_CAPTURES: &str = "MYBEVY_UI_AUDIT_REPEAT_CAPTURES";
const ENV_UI_AUDIT_GIT_COMMIT: &str = "MYBEVY_UI_AUDIT_GIT_COMMIT";
const DEFAULT_AUDIT_OUTPUT_ROOT: &str = "../summary/ui-audit";
const DEFAULT_AUDIT_LOCALE: &str = "zh_cn";
const DEFAULT_AUDIT_THEME: &str = "default";
const DEFAULT_STABLE_FIXTURE_ID: &str = "repository_static_data";
const MAX_REPEAT_CAPTURES: u32 = 8;

// These MYBEVY_UI_AUDIT_* variables belong only to the first-stage local one-shot mode.
const INITIAL_CAPTURE_STATE: &str = "initial";
const VISUAL_FOUNDATION_CAPTURE_STATE: &str = "visual_foundation";
const VISUAL_ACCEPTANCE_CAPTURE_STATE: &str = "visual_acceptance";
const IMAGE_FIT_CAPTURE_STATE: &str = "image_fit";
const IMAGE_MODES_CAPTURE_STATE: &str = "image_modes";
const IMAGE_TILING_CAPTURE_STATE: &str = "image_tiling";
const IMAGE_ATLAS_CAPTURE_STATE: &str = "image_atlas";
const TYPOGRAPHY_CAPTURE_STATE: &str = "typography";
const TYPOGRAPHY_OVERFLOW_CAPTURE_STATE: &str = "typography_overflow";
const ICONS_CAPTURE_STATE: &str = "icons";
const ICON_STATES_CAPTURE_STATE: &str = "icon_states";
const STYLE_SCOPES_CAPTURE_STATE: &str = "style_scopes";
const EFFECTS_CAPTURE_STATE: &str = "effects";
const ANIMATIONS_CAPTURE_STATE: &str = "animations";
const COMPONENTS_CAPTURE_STATE: &str = "components";
const COMPONENT_CHECKBOXES_CAPTURE_STATE: &str = "component_checkboxes";
const COMPONENT_TOGGLES_CAPTURE_STATE: &str = "component_toggles";
const COMPONENT_SEGMENTED_CAPTURE_STATE: &str = "component_segmented";
const COMPONENT_OVERLAYS_CAPTURE_STATE: &str = "component_overlays";
const COMPONENT_TOOLTIP_CAPTURE_STATE: &str = "component_tooltip";
const SCROLL_TOP_CAPTURE_STATE: &str = "top";
const SCROLL_MIDDLE_CAPTURE_STATE: &str = "middle";
const SCROLL_BOTTOM_CAPTURE_STATE: &str = "bottom";
// First-use UI gradient and box-shadow pipelines can need several render frames to become visible.
const STABLE_WAIT_FRAMES: u32 = 30;
const PANEL_READY_TIMEOUT_FRAMES: u32 = 300;
const STABLE_TIMEOUT_FRAMES: u32 = 120;
const SCREENSHOT_TIMEOUT_FRAMES: u32 = 300;

pub(crate) struct UiAuditPlugin;

impl Plugin for UiAuditPlugin {
    fn build(&self, app: &mut App) {
        let config = UiAuditConfig::from_env();
        configure_deterministic_runtime(app, config.enabled, &config.determinism);
        let determinism_context = UiAuditDeterminismContext::from(&config.determinism);
        app.add_plugins(UiScreenshotPlugin)
            .init_resource::<UiAuditScreenRegistry>()
            .insert_resource(config)
            .insert_resource(determinism_context)
            .insert_resource(UiAuditRuntime::default())
            .add_message::<UiAuditRouteCommand>()
            .add_message::<UiAuditCaptureStateApplied>()
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

fn configure_deterministic_runtime(
    app: &mut App,
    audit_enabled: bool,
    config: &UiAuditDeterminismConfig,
) {
    if audit_enabled && config.enabled {
        app.init_resource::<Time<Virtual>>();
        if valid_frozen_time(config.frozen_time_seconds) {
            freeze_virtual_time(
                &mut app.world_mut().resource_mut::<Time<Virtual>>(),
                config.frozen_time_seconds,
            );
        }
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO))
            .insert_resource(UiMotionPolicy::Disabled);
    }
}

fn freeze_virtual_time(time: &mut Time<Virtual>, frozen_time_seconds: f64) {
    *time = Time::<Virtual>::default();
    time.advance_by(Duration::from_secs_f64(frozen_time_seconds));
    time.advance_by(Duration::ZERO);
    time.pause();
}

fn valid_frozen_time(value: f64) -> bool {
    value.is_finite() && value >= 0.0
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

#[derive(Clone, Debug, PartialEq)]
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

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct UiAuditScreenRecipe {
    pub screen: UiAuditScreen,
}

impl UiAuditScreenRecipe {
    pub(crate) const fn new(screen: UiAuditScreen) -> Self {
        Self { screen }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct UiAuditRecipe {
    pub captures: &'static [UiAuditCaptureRecipe],
    pub ready: Option<UiAuditReadyCondition>,
    pub reference: UiAuditReferenceRecipe,
}

impl UiAuditRecipe {
    pub(crate) const fn new(captures: &'static [UiAuditCaptureRecipe]) -> Self {
        Self {
            captures,
            ready: None,
            reference: UiAuditReferenceRecipe::DEFAULT,
        }
    }

    #[allow(dead_code)]
    pub(crate) const fn with_ready(mut self, ready: UiAuditReadyCondition) -> Self {
        self.ready = Some(ready);
        self
    }

    pub(crate) const fn with_reference(mut self, reference: UiAuditReferenceRecipe) -> Self {
        self.reference = reference;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct UiAuditReferenceRecipe {
    pub target_viewport: UiAuditTargetViewport,
    pub locale: &'static str,
    pub theme: &'static str,
    pub random_seed: Option<u64>,
    pub frozen_time_seconds: f64,
    pub animation_progress: f32,
    pub dynamic_content: UiAuditDynamicContentRecipe,
}

impl UiAuditReferenceRecipe {
    const DEFAULT: Self = Self {
        target_viewport: UiAuditTargetViewport::RuntimeProfile,
        locale: DEFAULT_AUDIT_LOCALE,
        theme: DEFAULT_AUDIT_THEME,
        random_seed: None,
        frozen_time_seconds: 0.0,
        animation_progress: 1.0,
        dynamic_content: UiAuditDynamicContentRecipe::StableFixture(DEFAULT_STABLE_FIXTURE_ID),
    };

    pub(crate) const fn new() -> Self {
        Self::DEFAULT
    }

    pub(crate) const fn with_target_viewport(
        mut self,
        target_viewport: UiAuditTargetViewport,
    ) -> Self {
        self.target_viewport = target_viewport;
        self
    }

    pub(crate) const fn with_locale(mut self, locale: &'static str) -> Self {
        self.locale = locale;
        self
    }

    pub(crate) const fn with_theme(mut self, theme: &'static str) -> Self {
        self.theme = theme;
        self
    }

    pub(crate) const fn with_random_seed(mut self, random_seed: Option<u64>) -> Self {
        self.random_seed = random_seed;
        self
    }

    pub(crate) const fn with_frozen_time_seconds(mut self, frozen_time_seconds: f64) -> Self {
        self.frozen_time_seconds = frozen_time_seconds;
        self
    }

    pub(crate) const fn with_animation_progress(mut self, animation_progress: f32) -> Self {
        self.animation_progress = animation_progress;
        self
    }

    pub(crate) const fn with_dynamic_content(
        mut self,
        dynamic_content: UiAuditDynamicContentRecipe,
    ) -> Self {
        self.dynamic_content = dynamic_content;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UiAuditTargetViewport {
    RuntimeProfile,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UiAuditDynamicContentRecipe {
    StableFixture(&'static str),
    ExplicitMask(&'static str),
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
    OwnerDocument,
}

#[derive(Clone, Debug, Resource, Serialize, PartialEq)]
pub(crate) struct UiAuditDeterminismContext {
    pub enabled: bool,
    pub random_seed: Option<u64>,
    pub frozen_time_seconds: f64,
    pub animation_progress: f32,
    pub dynamic_policy: String,
    pub stable_fixture_id: Option<String>,
    pub dynamic_mask_id: Option<String>,
}

impl From<&UiAuditDeterminismConfig> for UiAuditDeterminismContext {
    fn from(config: &UiAuditDeterminismConfig) -> Self {
        Self {
            enabled: config.enabled,
            random_seed: config.random_seed,
            frozen_time_seconds: config.frozen_time_seconds,
            animation_progress: config.animation_progress,
            dynamic_policy: config.dynamic_policy.as_str().to_owned(),
            stable_fixture_id: config.stable_fixture_id.clone(),
            dynamic_mask_id: config.dynamic_mask_id.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct UiAuditTargetViewportConfig {
    logical_width: f32,
    logical_height: f32,
    physical_width: u32,
    physical_height: u32,
    device_scale: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum UiAuditDynamicPolicy {
    #[default]
    StableFixture,
    ExplicitMask,
}

impl UiAuditDynamicPolicy {
    const fn as_str(self) -> &'static str {
        match self {
            Self::StableFixture => "stable_fixture",
            Self::ExplicitMask => "explicit_mask",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct UiAuditDeterminismConfig {
    enabled: bool,
    target_viewport: Option<UiAuditTargetViewportConfig>,
    locale: String,
    theme: String,
    random_seed: Option<u64>,
    frozen_time_seconds: f64,
    animation_progress: f32,
    dynamic_policy: UiAuditDynamicPolicy,
    stable_fixture_id: Option<String>,
    dynamic_mask_id: Option<String>,
    repeat_captures: u32,
    git_commit: Option<String>,
    overrides: UiAuditDeterminismOverrides,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct UiAuditDeterminismOverrides {
    target_viewport: bool,
    locale: bool,
    theme: bool,
    random_seed: bool,
    frozen_time_seconds: bool,
    animation_progress: bool,
    dynamic_policy: bool,
    stable_fixture_id: bool,
    dynamic_mask_id: bool,
}

impl Default for UiAuditDeterminismConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            target_viewport: None,
            locale: DEFAULT_AUDIT_LOCALE.to_owned(),
            theme: DEFAULT_AUDIT_THEME.to_owned(),
            random_seed: None,
            frozen_time_seconds: 0.0,
            animation_progress: 1.0,
            dynamic_policy: UiAuditDynamicPolicy::StableFixture,
            stable_fixture_id: Some(DEFAULT_STABLE_FIXTURE_ID.to_owned()),
            dynamic_mask_id: None,
            repeat_captures: 1,
            git_commit: None,
            overrides: UiAuditDeterminismOverrides::default(),
        }
    }
}

#[derive(Clone, Debug, Resource)]
struct UiAuditConfig {
    enabled: bool,
    screen: Option<String>,
    output_root: PathBuf,
    states: Vec<UiAuditCaptureState>,
    states_from_env: bool,
    exit_on_finish: bool,
    determinism: UiAuditDeterminismConfig,
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
            determinism: UiAuditDeterminismConfig::default(),
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
        let (mut determinism, determinism_error) = parse_determinism_config(&mut read, enabled);
        if !enabled {
            determinism.enabled = false;
        }

        let (states, states_from_env, state_error) = match read(ENV_UI_AUDIT_STATES) {
            Some(value) => {
                let (states, error) = parse_capture_states(&value);
                (states, true, error)
            }
            None => (vec![UiAuditCaptureState::Initial], false, None),
        };
        let config_error = if enabled {
            state_error.or(determinism_error).or_else(|| {
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
            determinism,
            config_error,
        }
    }
}

fn parse_determinism_config(
    read: &mut impl FnMut(&str) -> Option<String>,
    _audit_enabled: bool,
) -> (UiAuditDeterminismConfig, Option<UiAuditFailureKind>) {
    let mut config = UiAuditDeterminismConfig::default();
    let mut invalid = false;
    config.enabled = read_bool(read, ENV_UI_AUDIT_DETERMINISTIC).unwrap_or(false);

    let (locale_supplied, locale) = parse_optional_env::<String>(read, ENV_UI_AUDIT_LOCALE)
        .unwrap_or_else(|()| {
            invalid = true;
            (true, None)
        });
    config.overrides.locale = locale_supplied;
    config.locale = locale
        .unwrap_or_else(|| DEFAULT_AUDIT_LOCALE.to_owned())
        .to_ascii_lowercase()
        .replace('-', "_");

    let (theme_supplied, theme) = parse_optional_env::<String>(read, ENV_UI_AUDIT_THEME)
        .unwrap_or_else(|()| {
            invalid = true;
            (true, None)
        });
    config.overrides.theme = theme_supplied;
    config.theme = theme.unwrap_or_else(|| DEFAULT_AUDIT_THEME.to_owned());

    let (random_seed_supplied, random_seed) =
        parse_optional_env::<u64>(read, ENV_UI_AUDIT_RANDOM_SEED).unwrap_or_else(|()| {
            invalid = true;
            (true, None)
        });
    config.overrides.random_seed = random_seed_supplied;
    config.random_seed = random_seed;

    let (frozen_time_supplied, frozen_time) =
        parse_optional_env::<f64>(read, ENV_UI_AUDIT_FROZEN_TIME_SECONDS).unwrap_or_else(|()| {
            invalid = true;
            (true, None)
        });
    config.overrides.frozen_time_seconds = frozen_time_supplied;
    config.frozen_time_seconds = frozen_time.unwrap_or_default();

    let (animation_progress_supplied, animation_progress) =
        parse_optional_env::<f32>(read, ENV_UI_AUDIT_ANIMATION_PROGRESS).unwrap_or_else(|()| {
            invalid = true;
            (true, None)
        });
    config.overrides.animation_progress = animation_progress_supplied;
    config.animation_progress = animation_progress.unwrap_or(1.0);

    let (dynamic_policy_supplied, dynamic_policy) =
        parse_optional_env::<String>(read, ENV_UI_AUDIT_DYNAMIC_POLICY).unwrap_or_else(|()| {
            invalid = true;
            (true, None)
        });
    config.overrides.dynamic_policy = dynamic_policy_supplied;
    config.dynamic_policy = match dynamic_policy
        .unwrap_or_else(|| "stable_fixture".to_owned())
        .to_ascii_lowercase()
        .replace('-', "_")
        .as_str()
    {
        "stable_fixture" => UiAuditDynamicPolicy::StableFixture,
        "explicit_mask" => UiAuditDynamicPolicy::ExplicitMask,
        _ => {
            invalid = true;
            UiAuditDynamicPolicy::StableFixture
        }
    };

    let (stable_fixture_supplied, stable_fixture_id) =
        parse_optional_env::<String>(read, ENV_UI_AUDIT_STABLE_FIXTURE_ID).unwrap_or_else(|()| {
            invalid = true;
            (true, None)
        });
    config.overrides.stable_fixture_id = stable_fixture_supplied;
    config.stable_fixture_id =
        stable_fixture_id.or_else(|| Some(DEFAULT_STABLE_FIXTURE_ID.to_owned()));

    let (dynamic_mask_supplied, dynamic_mask_id) =
        parse_optional_env::<String>(read, ENV_UI_AUDIT_DYNAMIC_MASK_ID).unwrap_or_else(|()| {
            invalid = true;
            (true, None)
        });
    config.overrides.dynamic_mask_id = dynamic_mask_supplied;
    config.dynamic_mask_id = dynamic_mask_id;

    let (_, repeat_captures) = parse_optional_env::<u32>(read, ENV_UI_AUDIT_REPEAT_CAPTURES)
        .unwrap_or_else(|()| {
            invalid = true;
            (true, None)
        });
    config.repeat_captures = repeat_captures.unwrap_or(1);

    let (_, git_commit) = parse_optional_env::<String>(read, ENV_UI_AUDIT_GIT_COMMIT)
        .unwrap_or_else(|()| {
            invalid = true;
            (true, None)
        });
    config.git_commit = git_commit;

    let (logical_width_supplied, logical_width) =
        parse_optional_env::<f32>(read, ENV_UI_AUDIT_TARGET_LOGICAL_WIDTH).unwrap_or_else(|()| {
            invalid = true;
            (true, None)
        });
    let (logical_height_supplied, logical_height) =
        parse_optional_env::<f32>(read, ENV_UI_AUDIT_TARGET_LOGICAL_HEIGHT).unwrap_or_else(|()| {
            invalid = true;
            (true, None)
        });
    let (physical_width_supplied, physical_width) =
        parse_optional_env::<u32>(read, ENV_UI_AUDIT_TARGET_PHYSICAL_WIDTH).unwrap_or_else(|()| {
            invalid = true;
            (true, None)
        });
    let (physical_height_supplied, physical_height) =
        parse_optional_env::<u32>(read, ENV_UI_AUDIT_TARGET_PHYSICAL_HEIGHT).unwrap_or_else(|()| {
            invalid = true;
            (true, None)
        });
    let (device_scale_supplied, device_scale) =
        parse_optional_env::<f32>(read, ENV_UI_AUDIT_TARGET_DEVICE_SCALE).unwrap_or_else(|()| {
            invalid = true;
            (true, None)
        });
    let viewport_parts = [
        logical_width_supplied,
        logical_height_supplied,
        physical_width_supplied,
        physical_height_supplied,
        device_scale_supplied,
    ];
    if viewport_parts.iter().all(|present| *present) {
        config.overrides.target_viewport = true;
        config.target_viewport = Some(UiAuditTargetViewportConfig {
            logical_width: logical_width.unwrap_or_default(),
            logical_height: logical_height.unwrap_or_default(),
            physical_width: physical_width.unwrap_or_default(),
            physical_height: physical_height.unwrap_or_default(),
            device_scale: device_scale.unwrap_or_default(),
        });
    } else if viewport_parts.iter().any(|present| *present) {
        invalid = true;
    }

    (config, invalid.then_some(UiAuditFailureKind::ConfigInvalid))
}

fn validate_determinism_config(config: &UiAuditDeterminismConfig, audit_enabled: bool) -> bool {
    if !config.enabled {
        return true;
    }

    let viewport_values_valid = config.target_viewport.is_some_and(|viewport| {
        viewport.logical_width.is_finite()
            && viewport.logical_width > 0.0
            && viewport.logical_height.is_finite()
            && viewport.logical_height > 0.0
            && viewport.physical_width > 0
            && viewport.physical_height > 0
            && viewport.device_scale.is_finite()
            && viewport.device_scale > 0.0
    });
    let dynamic_policy_valid = match config.dynamic_policy {
        UiAuditDynamicPolicy::StableFixture => config
            .stable_fixture_id
            .as_deref()
            .is_some_and(|id| !id.trim().is_empty()),
        UiAuditDynamicPolicy::ExplicitMask => config
            .dynamic_mask_id
            .as_deref()
            .is_some_and(|id| !id.trim().is_empty()),
    };
    let commit_valid = config.git_commit.as_deref().is_none_or(|commit| {
        commit == "unknown"
            || ((7..=64).contains(&commit.len())
                && commit.bytes().all(|byte| byte.is_ascii_hexdigit()))
    });
    audit_enabled
        && viewport_values_valid
        && !config.locale.is_empty()
        && !config.theme.trim().is_empty()
        && valid_frozen_time(config.frozen_time_seconds)
        && (config.animation_progress - 1.0).abs() <= f32::EPSILON
        && dynamic_policy_valid
        && (2..=MAX_REPEAT_CAPTURES).contains(&config.repeat_captures)
        && commit_valid
}

fn parse_optional_env<T: std::str::FromStr>(
    read: &mut impl FnMut(&str) -> Option<String>,
    key: &str,
) -> Result<(bool, Option<T>), ()> {
    let Some(value) = read(key) else {
        return Ok((false, None));
    };
    let value = value.trim();
    if value.is_empty() {
        return Err(());
    }
    value
        .parse()
        .map(|parsed| (true, Some(parsed)))
        .map_err(|_| ())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UiAuditCaptureState {
    Initial,
    VisualFoundation,
    VisualAcceptance,
    ImageFit,
    ImageModes,
    ImageTiling,
    ImageAtlas,
    Typography,
    TypographyOverflow,
    Icons,
    IconStates,
    StyleScopes,
    Effects,
    Animations,
    Components,
    ComponentCheckboxes,
    ComponentToggles,
    ComponentSegmented,
    ComponentOverlays,
    ComponentTooltip,
    Top,
    Middle,
    Bottom,
}

impl UiAuditCaptureState {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Initial => INITIAL_CAPTURE_STATE,
            Self::VisualFoundation => VISUAL_FOUNDATION_CAPTURE_STATE,
            Self::VisualAcceptance => VISUAL_ACCEPTANCE_CAPTURE_STATE,
            Self::ImageFit => IMAGE_FIT_CAPTURE_STATE,
            Self::ImageModes => IMAGE_MODES_CAPTURE_STATE,
            Self::ImageTiling => IMAGE_TILING_CAPTURE_STATE,
            Self::ImageAtlas => IMAGE_ATLAS_CAPTURE_STATE,
            Self::Typography => TYPOGRAPHY_CAPTURE_STATE,
            Self::TypographyOverflow => TYPOGRAPHY_OVERFLOW_CAPTURE_STATE,
            Self::Icons => ICONS_CAPTURE_STATE,
            Self::IconStates => ICON_STATES_CAPTURE_STATE,
            Self::StyleScopes => STYLE_SCOPES_CAPTURE_STATE,
            Self::Effects => EFFECTS_CAPTURE_STATE,
            Self::Animations => ANIMATIONS_CAPTURE_STATE,
            Self::Components => COMPONENTS_CAPTURE_STATE,
            Self::ComponentCheckboxes => COMPONENT_CHECKBOXES_CAPTURE_STATE,
            Self::ComponentToggles => COMPONENT_TOGGLES_CAPTURE_STATE,
            Self::ComponentSegmented => COMPONENT_SEGMENTED_CAPTURE_STATE,
            Self::ComponentOverlays => COMPONENT_OVERLAYS_CAPTURE_STATE,
            Self::ComponentTooltip => COMPONENT_TOOLTIP_CAPTURE_STATE,
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
    first_capture_hashes: BTreeMap<String, String>,
    screenshot_evidence: Option<UiAuditScreenshotEvidence>,
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
        stable_frames: u32,
        last_signature: Option<u64>,
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
    determinism: UiAuditDeterminismConfig,
    captures: Vec<UiAuditCapturePlan>,
}

#[derive(Clone, Debug, PartialEq)]
struct UiAuditCapturePlan {
    index: usize,
    state: UiAuditCaptureState,
    screenshot_path: PathBuf,
    metadata_path: PathBuf,
    repetition_index: u32,
    repetition_total: u32,
    target_viewport: Option<UiAuditTargetViewportConfig>,
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
    DocumentNotReady,
    LocaleNotReady,
    ThemeNotReady,
    FontNotReady,
    ImageNotReady,
    UnstableUi,
    ScreenshotSizeMismatch,
    NondeterministicCapture,
    ScreenshotFailed,
    ScrollTargetMissing,
    ScrollTargetUnreachable,
    ConfigInvalid,
    OutputWriteFailed,
}

impl Default for UiAuditFailureKind {
    fn default() -> Self {
        Self::PanelNotReady
    }
}

impl UiAuditFailureKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::ScreenNotFound => "screen_not_found",
            Self::PanelNotReady => "panel_not_ready",
            Self::DocumentNotReady => "document_not_ready",
            Self::LocaleNotReady => "locale_not_ready",
            Self::ThemeNotReady => "theme_not_ready",
            Self::FontNotReady => "font_not_ready",
            Self::ImageNotReady => "image_not_ready",
            Self::UnstableUi => "unstable_ui",
            Self::ScreenshotSizeMismatch => "screenshot_size_mismatch",
            Self::NondeterministicCapture => "nondeterministic_capture",
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
    SizeMismatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct UiAuditStepInput {
    readiness: UiAuditReadiness,
    screenshot_status: UiAuditScreenshotStatus,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct UiAuditReadiness {
    target_root: Option<Entity>,
    target_document_instance_id: Option<u64>,
    panel_ready: bool,
    document_ready: bool,
    target_ready: bool,
    target_not_ready_failure: UiAuditFailureKind,
    locale_ready: bool,
    theme_ready: bool,
    fonts_ready: bool,
    images_ready: bool,
    animations_ready: bool,
    viewport_ready: bool,
    signature: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct UiAuditScreenshotEvidence {
    captured_size: (u32, u32),
    requested_logical_size: Option<(u32, u32)>,
    requested_physical_size: Option<(u32, u32)>,
    request_frame: u64,
    completion_frame: u64,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct UiAuditCaptureArtifactMetadata {
    sha256: String,
    byte_length: u64,
    captured_width: u32,
    captured_height: u32,
    requested_logical_width: Option<u32>,
    requested_logical_height: Option<u32>,
    requested_physical_width: Option<u32>,
    requested_physical_height: Option<u32>,
    request_frame: u64,
    completion_frame: u64,
    exact_match_with_first_repetition: Option<bool>,
}

fn local_ui_audit_enabled(config: Res<UiAuditConfig>) -> bool {
    config.enabled
}

#[derive(SystemParam)]
struct UiAuditMetadataWorld<'w, 's> {
    current_owner: Res<'w, UiCurrentOwner>,
    viewport: Res<'w, UiViewport>,
    safe_area_status: Res<'w, UiSafeAreaStatus>,
    stats: Res<'w, UiStats>,
    motion_policy: Res<'w, UiMotionPolicy>,
    virtual_time: ResMut<'w, Time<Virtual>>,
    i18n: Res<'w, UiI18n>,
    theme_source: Res<'w, UiThemeSource>,
    image_assets: Res<'w, Assets<Image>>,
    panels: Query<'w, 's, (Entity, &'static UiPanelRoot)>,
    document_roots: Query<'w, 's, (Entity, &'static UiDocumentRuntimeRoot)>,
    parents: Query<'w, 's, &'static ChildOf>,
    document_nodes: Query<
        'w,
        's,
        (
            Entity,
            &'static UiDocumentNodeAuditMarker,
            &'static UiDocumentResolvedStyleMarker,
        ),
    >,
    style_resolutions: Query<
        'w,
        's,
        (
            Entity,
            Option<&'static Name>,
            &'static UiResolvedStyleDebugSnapshot,
        ),
    >,
    effect_resolutions: Query<
        'w,
        's,
        (
            Entity,
            Option<&'static Name>,
            &'static UiResolvedEffectDebugSnapshot,
        ),
    >,
    animation_snapshots: Query<
        'w,
        's,
        (
            Entity,
            Option<&'static Name>,
            &'static UiAnimationDebugSnapshot,
        ),
    >,
    image_snapshots: Query<
        'w,
        's,
        (
            Entity,
            Option<&'static Name>,
            &'static ImageNode,
            Option<&'static UiImageWidget>,
            Option<&'static UiImageStatus>,
        ),
    >,
    font_snapshots: Query<
        'w,
        's,
        (
            Entity,
            Option<&'static Name>,
            &'static UiTextStyleToken,
            &'static UiFontResolution,
        ),
    >,
    control_snapshots: Query<
        'w,
        's,
        (
            Entity,
            Option<&'static Name>,
            &'static UiControlMeta,
            Option<&'static Interaction>,
            Option<&'static UiControlFlags>,
            Has<FocusedButton>,
            Has<DisabledButton>,
            Option<&'static UiBadge>,
            Option<&'static UiProgress>,
            Option<&'static UiTooltip>,
        ),
    >,
    primary_window: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    semantic: UiAuditSemanticWorld<'w, 's>,
}

fn drive_local_ui_audit(
    mut runtime: ResMut<UiAuditRuntime>,
    config: Res<UiAuditConfig>,
    mut determinism_context: ResMut<UiAuditDeterminismContext>,
    registry: Res<UiAuditScreenRegistry>,
    mut metadata_world: UiAuditMetadataWorld,
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
    mut capture_state_writer: MessageWriter<UiAuditCaptureStateApplied>,
    mut screenshot_writer: MessageWriter<crate::framework::ui::audit::UiScreenshotCommand>,
    mut screenshot_events: MessageReader<UiScreenshotEvent>,
    mut app_exit: MessageWriter<AppExit>,
) {
    if matches!(runtime.phase, UiAuditPhase::Finish) {
        request_exit_if_needed(&mut runtime, &config, &mut app_exit);
        return;
    }

    if runtime.plan.is_none() {
        let plan = match prepare_runtime_plan(&config, &registry, &metadata_world.primary_window) {
            Ok(plan) => plan,
            Err(error) => {
                let failure = error.failure;
                let detail = Some(error.detail);
                if let Err(error) = write_planless_failure_outputs(
                    &config,
                    &metadata_world.primary_window,
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
        if plan.determinism.enabled {
            freeze_virtual_time(
                &mut metadata_world.virtual_time,
                plan.determinism.frozen_time_seconds,
            );
        }
        *determinism_context = UiAuditDeterminismContext::from(&plan.determinism);
        runtime.plan = Some(plan);
    }

    let current_capture = current_capture_plan(&runtime).cloned();
    let screenshot_status = consume_screenshot_status(
        &mut screenshot_events,
        current_capture.as_ref(),
        &mut runtime.screenshot_evidence,
    );
    let readiness = runtime
        .plan
        .as_ref()
        .map(|plan| collect_ui_audit_readiness(plan, &metadata_world))
        .unwrap_or_default();
    let phase = std::mem::take(&mut runtime.phase);
    let (next_phase, action) = advance_audit_phase(
        phase,
        UiAuditStepInput {
            readiness,
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
                Ok(()) => {
                    capture_state_writer.write(UiAuditCaptureStateApplied {
                        state: capture.state,
                    });
                }
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
            runtime.screenshot_evidence = None;
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
                let artifact = match build_capture_artifact_metadata(
                    &capture,
                    runtime.screenshot_evidence.as_ref(),
                    runtime
                        .first_capture_hashes
                        .get(capture.state.as_str())
                        .map(String::as_str),
                ) {
                    Ok(artifact) => artifact,
                    Err((failure, detail)) => {
                        runtime.phase = UiAuditPhase::Failed(failure);
                        runtime.result = Some(UiAuditCaptureResult {
                            status: UiAuditRunStatus::Failed,
                            failure: Some(failure),
                            detail: Some(detail.clone()),
                        });
                        if let Err(error) = write_failure_outputs(
                            &plan,
                            &runtime.manifest_entries,
                            &capture,
                            failure,
                            Some(&detail),
                        ) {
                            error!("ui audit failure output write failed: {error}");
                        }
                        request_exit_if_needed(&mut runtime, &config, &mut app_exit);
                        return;
                    }
                };
                let first_hash = runtime
                    .first_capture_hashes
                    .entry(capture.state.as_str().to_owned())
                    .or_insert_with(|| artifact.sha256.clone())
                    .clone();
                let scroll = capture_scroll_metadata(&capture, &mut scroll_targets);
                let metadata = build_capture_metadata(
                    &plan,
                    &capture,
                    scroll.as_ref(),
                    &metadata_world.viewport,
                    &metadata_world.safe_area_status,
                    &metadata_world.stats,
                    &metadata_world.current_owner,
                    &metadata_world.panels,
                    &metadata_world.document_nodes,
                    &metadata_world.style_resolutions,
                    &metadata_world.effect_resolutions,
                    &metadata_world.motion_policy,
                    &metadata_world.animation_snapshots,
                    &metadata_world.control_snapshots,
                    &metadata_world.image_snapshots,
                    &metadata_world.font_snapshots,
                    &metadata_world.image_assets,
                    &metadata_world.semantic,
                    metadata_world.primary_window.single().ok(),
                    &metadata_world.i18n,
                    &metadata_world.theme_source,
                    &metadata_world.virtual_time,
                    readiness,
                    artifact.clone(),
                );
                match write_capture_metadata(&capture, &metadata) {
                    Ok(()) => {
                        if capture.repetition_index > 0 && artifact.sha256 != first_hash {
                            let failure = UiAuditFailureKind::NondeterministicCapture;
                            let detail = format!(
                                "state '{}' repetition {} sha256 {} differs from first repetition {}",
                                capture.state.as_str(),
                                capture.repetition_index + 1,
                                artifact.sha256,
                                first_hash
                            );
                            runtime.phase = UiAuditPhase::Failed(failure);
                            runtime.result = Some(UiAuditCaptureResult {
                                status: UiAuditRunStatus::Failed,
                                failure: Some(failure),
                                detail: Some(detail.clone()),
                            });
                            if let Err(error) = write_failure_outputs(
                                &plan,
                                &runtime.manifest_entries,
                                &capture,
                                failure,
                                Some(&detail),
                            ) {
                                error!("ui audit failure output write failed: {error}");
                            }
                            request_exit_if_needed(&mut runtime, &config, &mut app_exit);
                            return;
                        }
                        runtime
                            .manifest_entries
                            .push(UiAuditManifestEntry::success_with_artifact(
                                &plan, &capture, &artifact,
                            ));
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

#[derive(Debug)]
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
    let determinism = resolve_determinism_for_screen(
        &config.determinism,
        screen.recipe.map(|recipe| recipe.reference),
        primary_window,
    )?;
    if !validate_determinism_config(&determinism, config.enabled) {
        return Err(UiAuditPlanError {
            failure: UiAuditFailureKind::ConfigInvalid,
            detail: "invalid deterministic audit configuration after applying screen recipe"
                .to_owned(),
        });
    }

    Ok(plan_audit_paths_with_determinism(
        &config.output_root,
        resolved,
        &device,
        screen.recipe.and_then(|recipe| recipe.ready),
        determinism,
        &captures,
    ))
}

fn resolve_determinism_for_screen(
    base: &UiAuditDeterminismConfig,
    reference: Option<UiAuditReferenceRecipe>,
    primary_window: &Query<&Window, With<PrimaryWindow>>,
) -> Result<UiAuditDeterminismConfig, UiAuditPlanError> {
    let mut resolved = base.clone();
    if !resolved.enabled {
        return Ok(resolved);
    }

    let reference = reference.unwrap_or(UiAuditReferenceRecipe::DEFAULT);
    if !resolved.overrides.target_viewport {
        resolved.target_viewport = Some(match reference.target_viewport {
            UiAuditTargetViewport::RuntimeProfile => primary_window
                .single()
                .ok()
                .map(target_viewport_from_window)
                .ok_or_else(|| UiAuditPlanError {
                    failure: UiAuditFailureKind::ConfigInvalid,
                    detail: "deterministic audit requires a primary window to resolve the runtime profile viewport"
                        .to_owned(),
                })?,
        });
    }
    if !resolved.overrides.locale {
        resolved.locale = normalize_locale(reference.locale);
    }
    if !resolved.overrides.theme {
        resolved.theme = reference.theme.trim().to_owned();
    }
    if !resolved.overrides.random_seed {
        resolved.random_seed = reference.random_seed;
    }
    if !resolved.overrides.frozen_time_seconds {
        resolved.frozen_time_seconds = reference.frozen_time_seconds;
    }
    if !resolved.overrides.animation_progress {
        resolved.animation_progress = reference.animation_progress;
    }
    if !resolved.overrides.dynamic_policy {
        match reference.dynamic_content {
            UiAuditDynamicContentRecipe::StableFixture(id) => {
                resolved.dynamic_policy = UiAuditDynamicPolicy::StableFixture;
                if !resolved.overrides.stable_fixture_id {
                    resolved.stable_fixture_id = Some(id.to_owned());
                }
                if !resolved.overrides.dynamic_mask_id {
                    resolved.dynamic_mask_id = None;
                }
            }
            UiAuditDynamicContentRecipe::ExplicitMask(id) => {
                resolved.dynamic_policy = UiAuditDynamicPolicy::ExplicitMask;
                if !resolved.overrides.dynamic_mask_id {
                    resolved.dynamic_mask_id = Some(id.to_owned());
                }
                if !resolved.overrides.stable_fixture_id {
                    resolved.stable_fixture_id = None;
                }
            }
        }
    }
    Ok(resolved)
}

fn target_viewport_from_window(window: &Window) -> UiAuditTargetViewportConfig {
    UiAuditTargetViewportConfig {
        logical_width: window.resolution.width(),
        logical_height: window.resolution.height(),
        physical_width: window.resolution.physical_width(),
        physical_height: window.resolution.physical_height(),
        device_scale: window.resolution.scale_factor(),
    }
}

fn normalize_locale(locale: &str) -> String {
    locale.trim().to_ascii_lowercase().replace('-', "_")
}

fn plan_audit_paths(
    output_root: &Path,
    screen: UiAuditResolvedScreen,
    device: &str,
    ready_condition: Option<UiAuditReadyCondition>,
    captures: &[UiAuditCaptureRecipe],
) -> UiAuditRunPlan {
    plan_audit_paths_with_determinism(
        output_root,
        screen,
        device,
        ready_condition,
        UiAuditDeterminismConfig::default(),
        captures,
    )
}

fn plan_audit_paths_with_determinism(
    output_root: &Path,
    screen: UiAuditResolvedScreen,
    device: &str,
    ready_condition: Option<UiAuditReadyCondition>,
    determinism: UiAuditDeterminismConfig,
    captures: &[UiAuditCaptureRecipe],
) -> UiAuditRunPlan {
    let screen_segment = sanitize_filename_segment(&screen.canonical);
    let device_segment = sanitize_filename_segment(device);
    let repetition_total = if determinism.enabled {
        determinism.repeat_captures
    } else {
        1
    };
    let capture_plans = captures
        .iter()
        .flat_map(|capture| {
            (0..repetition_total).map(|repetition_index| (*capture, repetition_index))
        })
        .enumerate()
        .map(|(index, (capture, repetition_index))| {
            plan_capture_paths(
                output_root,
                &screen_segment,
                &device_segment,
                index,
                capture,
                repetition_index,
                repetition_total,
                determinism.target_viewport,
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
        determinism,
        captures: capture_plans,
    }
}

fn plan_capture_paths(
    output_root: &Path,
    screen_segment: &str,
    device_segment: &str,
    index: usize,
    capture: UiAuditCaptureRecipe,
    repetition_index: u32,
    repetition_total: u32,
    target_viewport: Option<UiAuditTargetViewportConfig>,
) -> UiAuditCapturePlan {
    let state_segment = sanitize_filename_segment(capture.state.as_str());
    let repetition_suffix = (repetition_total > 1)
        .then(|| format!("-repeat-{:02}", repetition_index + 1))
        .unwrap_or_default();
    let file_stem = format!("{index:02}-{state_segment}{repetition_suffix}");

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
        repetition_index,
        repetition_total,
        target_viewport,
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
            if input.readiness.target_ready {
                (UiAuditPhase::ApplyCaptureState, None)
            } else if waited_frames >= PANEL_READY_TIMEOUT_FRAMES {
                let failure = input.readiness.target_not_ready_failure;
                (
                    UiAuditPhase::Failed(failure),
                    Some(UiAuditPureAction::Fail(failure)),
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
            UiAuditPhase::WaitForStable {
                waited_frames: 0,
                stable_frames: 0,
                last_signature: None,
            },
            Some(UiAuditPureAction::ApplyCaptureState),
        ),
        UiAuditPhase::WaitForStable {
            waited_frames,
            stable_frames,
            last_signature,
        } => {
            if !input.readiness.target_ready {
                (
                    UiAuditPhase::Failed(UiAuditFailureKind::UnstableUi),
                    Some(UiAuditPureAction::Fail(UiAuditFailureKind::UnstableUi)),
                )
            } else if waited_frames >= STABLE_TIMEOUT_FRAMES {
                let failure = if !input.readiness.locale_ready {
                    UiAuditFailureKind::LocaleNotReady
                } else if !input.readiness.theme_ready {
                    UiAuditFailureKind::ThemeNotReady
                } else if !input.readiness.fonts_ready {
                    UiAuditFailureKind::FontNotReady
                } else if !input.readiness.images_ready {
                    UiAuditFailureKind::ImageNotReady
                } else if !input.readiness.viewport_ready {
                    UiAuditFailureKind::ScreenshotSizeMismatch
                } else {
                    UiAuditFailureKind::UnstableUi
                };
                (
                    UiAuditPhase::Failed(failure),
                    Some(UiAuditPureAction::Fail(failure)),
                )
            } else {
                let resources_ready = input.readiness.locale_ready
                    && input.readiness.theme_ready
                    && input.readiness.fonts_ready
                    && input.readiness.images_ready
                    && input.readiness.animations_ready
                    && input.readiness.viewport_ready;
                let next_stable_frames =
                    if resources_ready && last_signature == Some(input.readiness.signature) {
                        stable_frames.saturating_add(1)
                    } else if resources_ready {
                        1
                    } else {
                        0
                    };
                if next_stable_frames >= STABLE_WAIT_FRAMES {
                    return (
                        UiAuditPhase::RequestScreenshot,
                        Some(UiAuditPureAction::RequestScreenshot),
                    );
                }
                (
                    UiAuditPhase::WaitForStable {
                        waited_frames: waited_frames.saturating_add(1),
                        stable_frames: next_stable_frames,
                        last_signature: resources_ready.then_some(input.readiness.signature),
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
            UiAuditScreenshotStatus::SizeMismatch => (
                UiAuditPhase::Failed(UiAuditFailureKind::ScreenshotSizeMismatch),
                Some(UiAuditPureAction::Fail(
                    UiAuditFailureKind::ScreenshotSizeMismatch,
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
            UiAuditScreenshotStatus::SizeMismatch => (
                UiAuditPhase::Failed(UiAuditFailureKind::ScreenshotSizeMismatch),
                Some(UiAuditPureAction::Fail(
                    UiAuditFailureKind::ScreenshotSizeMismatch,
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
    evidence: &mut Option<UiAuditScreenshotEvidence>,
) -> UiAuditScreenshotStatus {
    let Some(capture) = capture else {
        return UiAuditScreenshotStatus::Pending;
    };
    let mut status = UiAuditScreenshotStatus::Pending;
    for event in screenshot_events.read() {
        match event {
            UiScreenshotEvent::Saved(saved) if saved.request.path == capture.screenshot_path => {
                *evidence = Some(UiAuditScreenshotEvidence {
                    captured_size: saved.captured_size,
                    requested_logical_size: saved.request.logical_size,
                    requested_physical_size: saved.request.physical_size,
                    request_frame: saved.request.request_frame,
                    completion_frame: saved.completion_frame,
                });
                let request_matches = saved
                    .request
                    .physical_size
                    .is_some_and(|requested| requested == saved.captured_size);
                let target_matches = capture
                    .target_physical_size()
                    .is_none_or(|target| target == saved.captured_size);
                status = if request_matches && target_matches {
                    UiAuditScreenshotStatus::Saved
                } else {
                    UiAuditScreenshotStatus::SizeMismatch
                };
            }
            UiScreenshotEvent::Failed(failed) if failed.request.path == capture.screenshot_path => {
                status = UiAuditScreenshotStatus::Failed;
            }
            _ => {}
        }
    }
    status
}

impl UiAuditCapturePlan {
    fn target_physical_size(&self) -> Option<(u32, u32)> {
        self.target_viewport
            .map(|viewport| (viewport.physical_width, viewport.physical_height))
    }
}

fn collect_ui_audit_readiness(
    plan: &UiAuditRunPlan,
    world: &UiAuditMetadataWorld,
) -> UiAuditReadiness {
    let target_panel = world
        .panels
        .iter()
        .filter(|(_, panel)| panel.owner == Some(plan.screen.owner))
        .min_by_key(|(entity, _)| entity.index());
    let panel_ready = target_panel.is_some();
    let target_document = world
        .document_roots
        .iter()
        .filter(|(_, root)| root.owner == plan.screen.owner.as_str())
        .max_by_key(|(entity, root)| (root.generation, entity.index()));
    let document_ready = target_document.is_some_and(|(root_entity, root)| {
        document_instance_ready(
            root_entity,
            root.instance_id,
            &world.document_nodes,
            &world.parents,
        )
    });
    let (target_root, target_ready, target_not_ready_failure) = match plan.ready_condition {
        Some(UiAuditReadyCondition::OwnerPanel) => (
            target_panel.map(|(entity, _)| entity),
            panel_ready,
            UiAuditFailureKind::PanelNotReady,
        ),
        Some(UiAuditReadyCondition::OwnerDocument) => (
            target_document.map(|(entity, _)| entity),
            document_ready,
            UiAuditFailureKind::DocumentNotReady,
        ),
        None => {
            if let Some((entity, _)) = target_panel {
                (Some(entity), true, UiAuditFailureKind::PanelNotReady)
            } else {
                (
                    target_document.map(|(entity, _)| entity),
                    document_ready,
                    UiAuditFailureKind::PanelNotReady,
                )
            }
        }
    };
    let locale_ready = !plan.determinism.enabled
        || requested_locale_is_active(&plan.determinism.locale, world.i18n.locale());
    let theme_ready = !plan.determinism.enabled
        || requested_theme_is_loaded(&plan.determinism.theme, &world.theme_source);
    let fonts_ready = world
        .font_snapshots
        .iter()
        .filter(|(entity, _, _, _)| {
            target_root.is_some_and(|root| entity_is_within_target(*entity, root, &world.parents))
        })
        .all(|(_, _, _, resolution)| ui_font_resource_ready(&resolution.status));
    let images_ready = scoped_images_ready(
        target_root,
        &world.parents,
        &world.image_snapshots,
        &world.image_assets,
    );
    let animations_ready = !plan.determinism.enabled
        || world
            .animation_snapshots
            .iter()
            .filter(|(entity, _, _)| {
                target_root
                    .is_some_and(|root| entity_is_within_target(*entity, root, &world.parents))
            })
            .all(|(_, _, snapshot)| {
                snapshot.policy == UiMotionPolicy::Disabled.as_str()
                    && snapshot
                        .tracks
                        .iter()
                        .all(|track| track.state == "finished")
            });
    let viewport_ready = plan.determinism.target_viewport.is_none_or(|target| {
        world.primary_window.single().ok().is_some_and(|window| {
            dimensions_approximately_equal(window.resolution.width(), target.logical_width)
                && dimensions_approximately_equal(window.resolution.height(), target.logical_height)
                && window.resolution.physical_width() == target.physical_width
                && window.resolution.physical_height() == target.physical_height
                && dimensions_approximately_equal(
                    window.resolution.scale_factor(),
                    target.device_scale,
                )
        })
    });
    let signature = build_readiness_signature(plan, world, target_root, target_document);

    UiAuditReadiness {
        target_root,
        target_document_instance_id: target_document
            .filter(|(entity, _)| Some(*entity) == target_root)
            .map(|(_, root)| root.instance_id.0),
        panel_ready,
        document_ready,
        target_ready,
        target_not_ready_failure,
        locale_ready,
        theme_ready,
        fonts_ready,
        images_ready,
        animations_ready,
        viewport_ready,
        signature,
    }
}

fn entity_is_within_target(entity: Entity, target_root: Entity, parents: &Query<&ChildOf>) -> bool {
    let mut current = entity;
    for _ in 0..=1024 {
        if current == target_root {
            return true;
        }
        let Ok(parent) = parents.get(current) else {
            return false;
        };
        current = parent.parent();
    }
    false
}

fn document_instance_ready(
    root_entity: Entity,
    instance_id: UiDocumentInstanceId,
    document_nodes: &Query<(
        Entity,
        &UiDocumentNodeAuditMarker,
        &UiDocumentResolvedStyleMarker,
    )>,
    parents: &Query<&ChildOf>,
) -> bool {
    document_nodes.iter().any(|(entity, marker, _)| {
        marker.instance_id == instance_id && entity_is_within_target(entity, root_entity, parents)
    })
}

fn scoped_images_ready(
    target_root: Option<Entity>,
    parents: &Query<&ChildOf>,
    image_snapshots: &Query<(
        Entity,
        Option<&Name>,
        &ImageNode,
        Option<&UiImageWidget>,
        Option<&UiImageStatus>,
    )>,
    image_assets: &Assets<Image>,
) -> bool {
    image_snapshots
        .iter()
        .filter(|(entity, _, _, _, _)| {
            target_root.is_some_and(|root| entity_is_within_target(*entity, root, parents))
        })
        .all(|(_, _, image_node, widget, status)| {
            ui_image_resource_ready(
                widget.is_some(),
                status.copied(),
                image_assets.contains(image_node.image.id()),
            )
        })
}

fn requested_theme_is_loaded(requested: &str, source: &UiThemeSource) -> bool {
    let loaded = source.loaded_file_name();
    requested_theme_file_is_loaded(requested, loaded.as_deref(), source.using_fallback())
}

fn requested_locale_is_active(requested: &str, active: &str) -> bool {
    normalize_locale(requested) == normalize_locale(active)
}

fn requested_theme_file_is_loaded(
    requested: &str,
    loaded_file_name: Option<&str>,
    using_fallback: bool,
) -> bool {
    if using_fallback {
        return false;
    }
    let requested = requested.trim();
    if requested.is_empty() {
        return false;
    }
    let requested_file = Path::new(requested)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| requested.to_owned());
    let expected_file = if Path::new(&requested_file).extension().is_some() {
        requested_file
    } else {
        format!("{requested_file}.ron")
    };
    loaded_file_name.is_some_and(|loaded| loaded.eq_ignore_ascii_case(&expected_file))
}

fn ui_font_resource_ready(status: &super::super::style::UiFontResolutionStatus) -> bool {
    !matches!(status.as_str(), "loading" | "unavailable")
}

fn ui_image_resource_ready(
    tracked_widget: bool,
    status: Option<UiImageStatus>,
    asset_resolved: bool,
) -> bool {
    if !tracked_widget {
        return asset_resolved;
    }
    matches!(
        status,
        Some(UiImageStatus::Ready { .. } | UiImageStatus::Invalid(_))
    )
}

fn dimensions_approximately_equal(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() <= 0.05
}

fn build_readiness_signature(
    plan: &UiAuditRunPlan,
    world: &UiAuditMetadataWorld,
    target_root: Option<Entity>,
    target_document: Option<(Entity, &UiDocumentRuntimeRoot)>,
) -> u64 {
    let mut evidence = Vec::new();
    evidence.push(format!("screen={}", plan.screen.canonical));
    evidence.push(format!("locale={}", world.i18n.locale()));
    evidence.push(format!(
        "theme={}:{}",
        world.theme_source.loaded_file_name().unwrap_or_default(),
        world.theme_source.using_fallback()
    ));
    for (entity, panel) in world
        .panels
        .iter()
        .filter(|(entity, _)| target_root.is_some_and(|root| *entity == root))
    {
        evidence.push(format!(
            "panel={entity:?}:{}:{}:{}",
            panel.id,
            panel_kind_name(panel.kind),
            panel.owner.map(|owner| owner.as_str()).unwrap_or_default()
        ));
    }
    if let Some((entity, root)) = target_document.filter(|(entity, _)| {
        target_root.is_some_and(|target| entity_is_within_target(*entity, target, &world.parents))
    }) {
        evidence.push(format!(
            "document={entity:?}:{}:{}:{}:{}:{}",
            root.owner, root.document_id, root.instance_id.0, root.schema_version, root.generation
        ));
        for (node_entity, marker, _) in world.document_nodes.iter().filter(|(node, marker, _)| {
            marker.instance_id == root.instance_id
                && target_root
                    .is_some_and(|target| entity_is_within_target(*node, target, &world.parents))
        }) {
            evidence.push(format!(
                "document_node={node_entity:?}:{}:{}",
                marker.instance_id.0, marker.node_id
            ));
        }
    }
    for (_, name, _, resolution) in world.font_snapshots.iter().filter(|(entity, _, _, _)| {
        target_root.is_some_and(|root| entity_is_within_target(*entity, root, &world.parents))
    }) {
        evidence.push(format!(
            "font={}:{:?}:{}",
            name.map(Name::as_str).unwrap_or_default(),
            resolution.face,
            resolution.status.as_str()
        ));
    }
    for (_, name, image_node, _, status) in
        world.image_snapshots.iter().filter(|(entity, _, _, _, _)| {
            target_root.is_some_and(|root| entity_is_within_target(*entity, root, &world.parents))
        })
    {
        evidence.push(format!(
            "image={}:{}:{}",
            name.map(Name::as_str).unwrap_or_default(),
            image_node.image.id(),
            status.map_or("untracked", |status| status.code())
        ));
    }
    for (_, name, snapshot) in world.animation_snapshots.iter().filter(|(entity, _, _)| {
        target_root.is_some_and(|root| entity_is_within_target(*entity, root, &world.parents))
    }) {
        evidence.push(format!(
            "animation={}:{}",
            name.map(Name::as_str).unwrap_or_default(),
            serde_json::to_string(snapshot).unwrap_or_else(|_| "invalid".to_owned())
        ));
    }
    if let Ok(window) = world.primary_window.single() {
        evidence.push(format!(
            "window={:.3}x{:.3}:{}x{}:{:.3}",
            window.resolution.width(),
            window.resolution.height(),
            window.resolution.physical_width(),
            window.resolution.physical_height(),
            window.resolution.scale_factor()
        ));
    }
    evidence.sort();
    let digest = Sha256::digest(evidence.join("\n").as_bytes());
    u64::from_be_bytes(
        digest[..8]
            .try_into()
            .expect("sha256 prefix is eight bytes"),
    )
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
        UiAuditFailureKind::DocumentNotReady => plan.map(|plan| {
            format!(
                "target owner '{}' did not finish declarative document construction before timeout",
                plan.screen.owner
            )
        }),
        UiAuditFailureKind::LocaleNotReady => plan.map(|plan| {
            format!(
                "requested locale '{}' was not active before timeout",
                plan.determinism.locale
            )
        }),
        UiAuditFailureKind::ThemeNotReady => plan.map(|plan| {
            format!(
                "requested theme '{}' was not loaded from its theme file before timeout",
                plan.determinism.theme
            )
        }),
        UiAuditFailureKind::FontNotReady => {
            Some("one or more UI fonts remained loading or unavailable before timeout".to_owned())
        }
        UiAuditFailureKind::ImageNotReady => {
            Some("one or more UI images remained unresolved before timeout".to_owned())
        }
        UiAuditFailureKind::UnstableUi => plan.map(|plan| {
            format!(
                "target owner '{}' disappeared before stable capture",
                plan.screen.owner
            )
        }),
        UiAuditFailureKind::ScreenshotFailed => {
            Some(format!("screenshot status ended as {screenshot_status:?}"))
        }
        UiAuditFailureKind::ScreenshotSizeMismatch => plan.and_then(|plan| {
            plan.determinism.target_viewport.map(|target| {
                format!(
                    "screenshot did not match target viewport logical {:.3}x{:.3}, physical {}x{}, scale {:.3}",
                    target.logical_width,
                    target.logical_height,
                    target.physical_width,
                    target.physical_height,
                    target.device_scale
                )
            })
        }),
        UiAuditFailureKind::NondeterministicCapture => capture.map(|capture| {
            format!(
                "state '{}' repetition {} differed from the first exact PNG hash",
                capture.state.as_str(),
                capture.repetition_index + 1
            )
        }),
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
    } else if value.eq_ignore_ascii_case(VISUAL_ACCEPTANCE_CAPTURE_STATE) {
        Some(UiAuditCaptureState::VisualAcceptance)
    } else if value.eq_ignore_ascii_case(IMAGE_FIT_CAPTURE_STATE) {
        Some(UiAuditCaptureState::ImageFit)
    } else if value.eq_ignore_ascii_case(IMAGE_MODES_CAPTURE_STATE) {
        Some(UiAuditCaptureState::ImageModes)
    } else if value.eq_ignore_ascii_case(IMAGE_TILING_CAPTURE_STATE) {
        Some(UiAuditCaptureState::ImageTiling)
    } else if value.eq_ignore_ascii_case(IMAGE_ATLAS_CAPTURE_STATE) {
        Some(UiAuditCaptureState::ImageAtlas)
    } else if value.eq_ignore_ascii_case(TYPOGRAPHY_CAPTURE_STATE) {
        Some(UiAuditCaptureState::Typography)
    } else if value.eq_ignore_ascii_case(TYPOGRAPHY_OVERFLOW_CAPTURE_STATE) {
        Some(UiAuditCaptureState::TypographyOverflow)
    } else if value.eq_ignore_ascii_case(ICONS_CAPTURE_STATE) {
        Some(UiAuditCaptureState::Icons)
    } else if value.eq_ignore_ascii_case(ICON_STATES_CAPTURE_STATE) {
        Some(UiAuditCaptureState::IconStates)
    } else if value.eq_ignore_ascii_case(STYLE_SCOPES_CAPTURE_STATE) {
        Some(UiAuditCaptureState::StyleScopes)
    } else if value.eq_ignore_ascii_case(EFFECTS_CAPTURE_STATE) {
        Some(UiAuditCaptureState::Effects)
    } else if value.eq_ignore_ascii_case(ANIMATIONS_CAPTURE_STATE) {
        Some(UiAuditCaptureState::Animations)
    } else if value.eq_ignore_ascii_case(COMPONENTS_CAPTURE_STATE) {
        Some(UiAuditCaptureState::Components)
    } else if value.eq_ignore_ascii_case(COMPONENT_CHECKBOXES_CAPTURE_STATE) {
        Some(UiAuditCaptureState::ComponentCheckboxes)
    } else if value.eq_ignore_ascii_case(COMPONENT_TOGGLES_CAPTURE_STATE) {
        Some(UiAuditCaptureState::ComponentToggles)
    } else if value.eq_ignore_ascii_case(COMPONENT_SEGMENTED_CAPTURE_STATE) {
        Some(UiAuditCaptureState::ComponentSegmented)
    } else if value.eq_ignore_ascii_case(COMPONENT_OVERLAYS_CAPTURE_STATE) {
        Some(UiAuditCaptureState::ComponentOverlays)
    } else if value.eq_ignore_ascii_case(COMPONENT_TOOLTIP_CAPTURE_STATE) {
        Some(UiAuditCaptureState::ComponentTooltip)
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

fn build_capture_artifact_metadata(
    capture: &UiAuditCapturePlan,
    evidence: Option<&UiAuditScreenshotEvidence>,
    first_repetition_sha256: Option<&str>,
) -> Result<UiAuditCaptureArtifactMetadata, (UiAuditFailureKind, String)> {
    let evidence = evidence.ok_or_else(|| {
        (
            UiAuditFailureKind::ScreenshotFailed,
            "saved screenshot did not retain capture evidence".to_owned(),
        )
    })?;
    let bytes = fs::read(&capture.screenshot_path).map_err(|error| {
        (
            UiAuditFailureKind::ScreenshotFailed,
            format!(
                "could not read saved screenshot '{}': {error}",
                capture.screenshot_path.display()
            ),
        )
    })?;
    let sha256 = format!("{:x}", Sha256::digest(&bytes));
    let requested_logical = evidence.requested_logical_size;
    let requested_physical = evidence.requested_physical_size;
    Ok(UiAuditCaptureArtifactMetadata {
        sha256: sha256.clone(),
        byte_length: bytes.len() as u64,
        captured_width: evidence.captured_size.0,
        captured_height: evidence.captured_size.1,
        requested_logical_width: requested_logical.map(|size| size.0),
        requested_logical_height: requested_logical.map(|size| size.1),
        requested_physical_width: requested_physical.map(|size| size.0),
        requested_physical_height: requested_physical.map(|size| size.1),
        request_frame: evidence.request_frame,
        completion_frame: evidence.completion_frame,
        exact_match_with_first_repetition: first_repetition_sha256.map(|first| first == sha256),
    })
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
    safe_area_status: &UiSafeAreaStatus,
    stats: &UiStats,
    current_owner: &UiCurrentOwner,
    panels: &Query<(Entity, &UiPanelRoot)>,
    document_nodes: &Query<(
        Entity,
        &UiDocumentNodeAuditMarker,
        &UiDocumentResolvedStyleMarker,
    )>,
    style_resolutions: &Query<(Entity, Option<&Name>, &UiResolvedStyleDebugSnapshot)>,
    effect_resolutions: &Query<(Entity, Option<&Name>, &UiResolvedEffectDebugSnapshot)>,
    motion_policy: &UiMotionPolicy,
    animation_snapshots: &Query<(Entity, Option<&Name>, &UiAnimationDebugSnapshot)>,
    control_snapshots: &Query<(
        Entity,
        Option<&Name>,
        &UiControlMeta,
        Option<&Interaction>,
        Option<&UiControlFlags>,
        Has<FocusedButton>,
        Has<DisabledButton>,
        Option<&UiBadge>,
        Option<&UiProgress>,
        Option<&UiTooltip>,
    )>,
    image_snapshots: &Query<(
        Entity,
        Option<&Name>,
        &ImageNode,
        Option<&UiImageWidget>,
        Option<&UiImageStatus>,
    )>,
    font_snapshots: &Query<(Entity, Option<&Name>, &UiTextStyleToken, &UiFontResolution)>,
    image_assets: &Assets<Image>,
    semantic_world: &UiAuditSemanticWorld,
    primary_window: Option<&Window>,
    i18n: &UiI18n,
    theme_source: &UiThemeSource,
    virtual_time: &Time<Virtual>,
    readiness: UiAuditReadiness,
    artifact: UiAuditCaptureArtifactMetadata,
) -> UiAuditMetadata {
    let style_resolutions = collect_style_resolution_metadata(style_resolutions);
    let document_nodes = collect_document_node_metadata(document_nodes);
    let effect_resolutions = collect_effect_resolution_metadata(effect_resolutions);
    let animation_snapshots = collect_animation_snapshot_metadata(animation_snapshots);
    let control_snapshots = collect_control_snapshot_metadata(control_snapshots);
    let (image_snapshots, image_accounting) =
        collect_image_snapshot_metadata(image_snapshots, image_assets);
    let font_snapshots = collect_font_snapshot_metadata(font_snapshots);
    let semantic_tree = collect_semantic_tree(semantic_world, readiness.target_root, viewport);
    let visual_summary = build_visual_summary(
        &style_resolutions,
        &effect_resolutions,
        &animation_snapshots,
        &control_snapshots,
        &image_snapshots,
        &font_snapshots,
    );
    let visual_budget = build_visual_budget(viewport, stats, image_accounting, &effect_resolutions);
    UiAuditMetadata {
        screen: plan.screen.canonical.clone(),
        requested_screen: plan.screen.requested.clone(),
        state: capture.state.as_str().to_owned(),
        device: plan.device.clone(),
        screenshot_path: absolute_display_path(&capture.screenshot_path)
            .to_string_lossy()
            .into_owned(),
        application: UiAuditApplicationMetadata {
            package_name: env!("CARGO_PKG_NAME"),
            package_version: env!("CARGO_PKG_VERSION"),
            git_commit: plan
                .determinism
                .git_commit
                .clone()
                .unwrap_or_else(|| "unknown".to_owned()),
            git_commit_source: plan
                .determinism
                .git_commit
                .as_ref()
                .map_or("unavailable", |_| "runner_environment"),
        },
        capture_identity: UiAuditCaptureIdentityMetadata {
            state: capture.state.as_str().to_owned(),
            repetition_index: capture.repetition_index + 1,
            repetition_total: capture.repetition_total,
        },
        environment: UiAuditEnvironmentMetadata {
            requested_locale: plan.determinism.locale.clone(),
            actual_locale: i18n.locale().to_owned(),
            requested_theme: plan.determinism.theme.clone(),
            loaded_theme_file: theme_source.loaded_file_name(),
            theme_fallback: theme_source.using_fallback(),
        },
        determinism: UiAuditDeterminismMetadata {
            enabled: plan.determinism.enabled,
            random_seed: plan.determinism.random_seed,
            frozen_time_seconds: plan.determinism.frozen_time_seconds,
            actual_virtual_elapsed_seconds: virtual_time.elapsed_secs_f64(),
            actual_virtual_delta_seconds: virtual_time.delta_secs_f64(),
            clock_control: if plan.determinism.enabled {
                "manual_zero_delta"
            } else {
                "runtime"
            },
            requested_animation_progress: plan.determinism.animation_progress,
            actual_motion_policy: motion_policy.as_str(),
            dynamic_policy: plan.determinism.dynamic_policy.as_str(),
            stable_fixture_id: plan.determinism.stable_fixture_id.clone(),
            dynamic_mask_id: plan.determinism.dynamic_mask_id.clone(),
        },
        resource_readiness: UiAuditResourceReadinessMetadata::from(readiness),
        artifact,
        scroll: scroll.cloned(),
        viewport: UiAuditViewportMetadata::new(*viewport, *safe_area_status),
        current_page: current_owner.owner.map(|owner| owner.as_str().to_owned()),
        panels: panels
            .iter()
            .map(|(_, panel)| UiAuditPanelMetadata::from(panel))
            .collect(),
        document_nodes,
        style_resolutions,
        effect_resolutions,
        motion_policy: motion_policy.as_str().to_owned(),
        animation_snapshots,
        control_snapshots,
        image_snapshots,
        font_snapshots,
        semantic_tree,
        visual_summary,
        visual_budget,
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
    application: UiAuditApplicationMetadata,
    capture_identity: UiAuditCaptureIdentityMetadata,
    environment: UiAuditEnvironmentMetadata,
    determinism: UiAuditDeterminismMetadata,
    resource_readiness: UiAuditResourceReadinessMetadata,
    artifact: UiAuditCaptureArtifactMetadata,
    scroll: Option<UiAuditScrollMetadata>,
    viewport: UiAuditViewportMetadata,
    current_page: Option<String>,
    panels: Vec<UiAuditPanelMetadata>,
    document_nodes: Vec<UiAuditDocumentNodeMetadata>,
    style_resolutions: Vec<UiAuditStyleResolutionMetadata>,
    effect_resolutions: Vec<UiAuditEffectResolutionMetadata>,
    motion_policy: String,
    animation_snapshots: Vec<UiAuditAnimationSnapshotMetadata>,
    control_snapshots: Vec<UiAuditControlSnapshotMetadata>,
    image_snapshots: Vec<UiAuditImageSnapshotMetadata>,
    font_snapshots: Vec<UiAuditFontSnapshotMetadata>,
    semantic_tree: UiAuditSemanticTree,
    visual_summary: UiAuditVisualSummary,
    visual_budget: UiVisualBudgetReport,
    window: Option<UiAuditWindowMetadata>,
    stats: UiAuditStatsMetadata,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct UiAuditApplicationMetadata {
    package_name: &'static str,
    package_version: &'static str,
    git_commit: String,
    git_commit_source: &'static str,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct UiAuditCaptureIdentityMetadata {
    state: String,
    repetition_index: u32,
    repetition_total: u32,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct UiAuditEnvironmentMetadata {
    requested_locale: String,
    actual_locale: String,
    requested_theme: String,
    loaded_theme_file: Option<String>,
    theme_fallback: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
struct UiAuditDeterminismMetadata {
    enabled: bool,
    random_seed: Option<u64>,
    frozen_time_seconds: f64,
    actual_virtual_elapsed_seconds: f64,
    actual_virtual_delta_seconds: f64,
    clock_control: &'static str,
    requested_animation_progress: f32,
    actual_motion_policy: &'static str,
    dynamic_policy: &'static str,
    stable_fixture_id: Option<String>,
    dynamic_mask_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct UiAuditResourceReadinessMetadata {
    target_root_entity: Option<String>,
    target_document_instance_id: Option<u64>,
    panel_ready: bool,
    document_ready: bool,
    locale_ready: bool,
    theme_ready: bool,
    fonts_ready: bool,
    images_ready: bool,
    animations_ready: bool,
    viewport_ready: bool,
    stable_signature: u64,
}

impl From<UiAuditReadiness> for UiAuditResourceReadinessMetadata {
    fn from(readiness: UiAuditReadiness) -> Self {
        Self {
            target_root_entity: readiness.target_root.map(|entity| format!("{entity:?}")),
            target_document_instance_id: readiness.target_document_instance_id,
            panel_ready: readiness.panel_ready,
            document_ready: readiness.document_ready,
            locale_ready: readiness.locale_ready,
            theme_ready: readiness.theme_ready,
            fonts_ready: readiness.fonts_ready,
            images_ready: readiness.images_ready,
            animations_ready: readiness.animations_ready,
            viewport_ready: readiness.viewport_ready,
            stable_signature: readiness.signature,
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
struct UiAuditDocumentNodeMetadata {
    entity: String,
    document_id: String,
    schema_version: u32,
    node_id: String,
    document_path: String,
    source_path: String,
    effective_style: super::super::document::UiResolvedStyle,
}

fn collect_document_node_metadata(
    nodes: &Query<(
        Entity,
        &UiDocumentNodeAuditMarker,
        &UiDocumentResolvedStyleMarker,
    )>,
) -> Vec<UiAuditDocumentNodeMetadata> {
    let mut values = nodes
        .iter()
        .map(|(entity, marker, style)| UiAuditDocumentNodeMetadata {
            entity: format!("{entity:?}"),
            document_id: marker.document_id.as_str().to_owned(),
            schema_version: marker.schema_version,
            node_id: marker.node_id.as_str().to_owned(),
            document_path: marker.document_path.clone(),
            source_path: marker.source_path.clone(),
            effective_style: style.0.clone(),
        })
        .collect::<Vec<_>>();
    values.sort_by(|left, right| {
        left.document_id
            .cmp(&right.document_id)
            .then_with(|| left.node_id.cmp(&right.node_id))
            .then_with(|| left.entity.cmp(&right.entity))
    });
    values
}

#[derive(Clone, Debug, Serialize, PartialEq)]
struct UiAuditStyleResolutionMetadata {
    entity: String,
    name: Option<String>,
    snapshot: UiResolvedStyleDebugSnapshot,
}

fn collect_style_resolution_metadata(
    resolutions: &Query<(Entity, Option<&Name>, &UiResolvedStyleDebugSnapshot)>,
) -> Vec<UiAuditStyleResolutionMetadata> {
    let mut values = resolutions
        .iter()
        .map(|(entity, name, snapshot)| UiAuditStyleResolutionMetadata {
            entity: format!("{entity:?}"),
            name: name.map(|name| name.as_str().to_owned()),
            snapshot: snapshot.clone(),
        })
        .collect::<Vec<_>>();
    values.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.entity.cmp(&right.entity))
    });
    values
}

#[derive(Clone, Debug, Serialize, PartialEq)]
struct UiAuditEffectResolutionMetadata {
    entity: String,
    name: Option<String>,
    snapshot: UiResolvedEffectDebugSnapshot,
}

fn collect_effect_resolution_metadata(
    resolutions: &Query<(Entity, Option<&Name>, &UiResolvedEffectDebugSnapshot)>,
) -> Vec<UiAuditEffectResolutionMetadata> {
    let mut values = resolutions
        .iter()
        .map(|(entity, name, snapshot)| UiAuditEffectResolutionMetadata {
            entity: format!("{entity:?}"),
            name: name.map(|name| name.as_str().to_owned()),
            snapshot: snapshot.clone(),
        })
        .collect::<Vec<_>>();
    values.sort_by(|left, right| {
        left.name
            .as_deref()
            .unwrap_or_default()
            .cmp(right.name.as_deref().unwrap_or_default())
            .then_with(|| left.entity.cmp(&right.entity))
    });
    values
}

#[derive(Clone, Debug, Serialize, PartialEq)]
struct UiAuditAnimationSnapshotMetadata {
    entity: String,
    name: Option<String>,
    snapshot: UiAnimationDebugSnapshot,
}

fn collect_animation_snapshot_metadata(
    snapshots: &Query<(Entity, Option<&Name>, &UiAnimationDebugSnapshot)>,
) -> Vec<UiAuditAnimationSnapshotMetadata> {
    let mut values = snapshots
        .iter()
        .map(
            |(entity, name, snapshot)| UiAuditAnimationSnapshotMetadata {
                entity: format!("{entity:?}"),
                name: name.map(|name| name.as_str().to_owned()),
                snapshot: snapshot.clone(),
            },
        )
        .collect::<Vec<_>>();
    values.sort_by(|left, right| {
        left.name
            .as_deref()
            .unwrap_or_default()
            .cmp(right.name.as_deref().unwrap_or_default())
            .then_with(|| left.entity.cmp(&right.entity))
    });
    values
}

#[derive(Clone, Debug, Serialize, PartialEq)]
struct UiAuditControlSnapshotMetadata {
    entity: String,
    name: Option<String>,
    control_id: String,
    kind: String,
    state: String,
    selected: bool,
    disabled: bool,
    loading: bool,
    empty: bool,
    error: bool,
}

fn collect_control_snapshot_metadata(
    snapshots: &Query<(
        Entity,
        Option<&Name>,
        &UiControlMeta,
        Option<&Interaction>,
        Option<&UiControlFlags>,
        Has<FocusedButton>,
        Has<DisabledButton>,
        Option<&UiBadge>,
        Option<&UiProgress>,
        Option<&UiTooltip>,
    )>,
) -> Vec<UiAuditControlSnapshotMetadata> {
    let mut values = snapshots
        .iter()
        .map(
            |(
                entity,
                name,
                meta,
                interaction,
                flags,
                focused,
                disabled_marker,
                badge,
                progress,
                tooltip,
            )| {
                let flags = flags.copied().unwrap_or_default();
                let state = badge
                    .map(|badge| badge.state)
                    .or_else(|| progress.map(|progress| progress.state))
                    .or_else(|| {
                        tooltip.map(|tooltip| {
                            if disabled_marker {
                                UiControlState::Disabled
                            } else if tooltip.tone == UiTooltipTone::Error {
                                UiControlState::Error
                            } else {
                                UiControlState::Normal
                            }
                        })
                    })
                    .unwrap_or_else(|| {
                        resolve_control_state(
                            interaction.copied().unwrap_or(Interaction::None),
                            focused,
                            flags,
                        )
                    });
                UiAuditControlSnapshotMetadata {
                    entity: format!("{entity:?}"),
                    name: name.map(|name| name.as_str().to_owned()),
                    control_id: meta.id.as_str().to_owned(),
                    kind: format!("{:?}", meta.kind).to_ascii_lowercase(),
                    state: format!("{state:?}").to_ascii_lowercase(),
                    selected: flags.selected || state == UiControlState::Selected,
                    disabled: flags.disabled || state == UiControlState::Disabled,
                    loading: flags.loading || state == UiControlState::Loading,
                    empty: flags.empty || state == UiControlState::Empty,
                    error: flags.error || state == UiControlState::Error,
                }
            },
        )
        .collect::<Vec<_>>();
    values.sort_by(|left, right| {
        left.control_id
            .cmp(&right.control_id)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.entity.cmp(&right.entity))
    });
    values
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct UiAuditImageSnapshotMetadata {
    entity: String,
    name: Option<String>,
    presentation: String,
    node_image_mode: &'static str,
    status: &'static str,
    asset_resolved: bool,
    decoded_bytes_estimate: Option<usize>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct UiAuditImageAccounting {
    unique_asset_count: usize,
    decoded_bytes_estimate: usize,
    unresolved_asset_count: usize,
}

fn collect_image_snapshot_metadata(
    snapshots: &Query<(
        Entity,
        Option<&Name>,
        &ImageNode,
        Option<&UiImageWidget>,
        Option<&UiImageStatus>,
    )>,
    image_assets: &Assets<Image>,
) -> (Vec<UiAuditImageSnapshotMetadata>, UiAuditImageAccounting) {
    let mut unique_assets = HashSet::new();
    let mut accounting = UiAuditImageAccounting::default();
    let mut values = snapshots
        .iter()
        .map(|(entity, name, image_node, widget, status)| {
            let asset = image_assets.get(image_node.image.id());
            if unique_assets.insert(image_node.image.id()) {
                accounting.unique_asset_count += 1;
                if let Some(decoded_bytes) =
                    asset.and_then(|image| image.data.as_ref()).map(Vec::len)
                {
                    accounting.decoded_bytes_estimate = accounting
                        .decoded_bytes_estimate
                        .saturating_add(decoded_bytes);
                } else {
                    accounting.unresolved_asset_count += 1;
                }
            }
            UiAuditImageSnapshotMetadata {
                entity: format!("{entity:?}"),
                name: name.map(|name| name.as_str().to_owned()),
                presentation: widget
                    .map(|widget| widget.presentation_kind().as_str().to_owned())
                    .unwrap_or_else(|| node_image_mode_name(&image_node.image_mode).to_owned()),
                node_image_mode: node_image_mode_name(&image_node.image_mode),
                status: status.map_or("untracked", |status| status.code()),
                asset_resolved: asset.is_some(),
                decoded_bytes_estimate: asset.and_then(|image| image.data.as_ref()).map(Vec::len),
            }
        })
        .collect::<Vec<_>>();
    values.sort_by(|left, right| {
        left.name
            .as_deref()
            .unwrap_or_default()
            .cmp(right.name.as_deref().unwrap_or_default())
            .then_with(|| left.presentation.cmp(&right.presentation))
            .then_with(|| left.entity.cmp(&right.entity))
    });
    (values, accounting)
}

fn node_image_mode_name(mode: &NodeImageMode) -> &'static str {
    match mode {
        NodeImageMode::Auto => "auto",
        NodeImageMode::Stretch => "stretch",
        NodeImageMode::Sliced(_) => "sliced",
        NodeImageMode::Tiled { .. } => "tiled",
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
struct UiAuditFontSnapshotMetadata {
    entity: String,
    name: Option<String>,
    requested_role: &'static str,
    requested_family: String,
    requested_weight: String,
    resolved_family: String,
    resolved_weight: String,
    status: &'static str,
}

fn collect_font_snapshot_metadata(
    snapshots: &Query<(Entity, Option<&Name>, &UiTextStyleToken, &UiFontResolution)>,
) -> Vec<UiAuditFontSnapshotMetadata> {
    let mut values = snapshots
        .iter()
        .map(
            |(entity, name, token, resolution)| UiAuditFontSnapshotMetadata {
                entity: format!("{entity:?}"),
                name: name.map(|name| name.as_str().to_owned()),
                requested_role: token.font_role.as_str(),
                requested_family: format!("{:?}", token.font_family).to_ascii_lowercase(),
                requested_weight: format!("{:?}", token.font_weight).to_ascii_lowercase(),
                resolved_family: format!("{:?}", resolution.face.family).to_ascii_lowercase(),
                resolved_weight: format!("{:?}", resolution.face.weight).to_ascii_lowercase(),
                status: resolution.status.as_str(),
            },
        )
        .collect::<Vec<_>>();
    values.sort_by(|left, right| {
        left.name
            .as_deref()
            .unwrap_or_default()
            .cmp(right.name.as_deref().unwrap_or_default())
            .then_with(|| left.requested_role.cmp(right.requested_role))
            .then_with(|| left.entity.cmp(&right.entity))
    });
    values
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
struct UiAuditVisualSummary {
    image_modes: BTreeMap<String, usize>,
    image_statuses: BTreeMap<String, usize>,
    style_scopes: BTreeMap<String, usize>,
    style_variants: BTreeMap<String, usize>,
    font_roles: BTreeMap<String, usize>,
    font_resolution_statuses: BTreeMap<String, usize>,
    effect_count: usize,
    effect_fallback_count: usize,
    material_request_count: usize,
    animation_policy_states: BTreeMap<String, usize>,
    animation_track_states: BTreeMap<String, usize>,
    animation_track_count: usize,
    paused_animation_track_count: usize,
    layout_reflow_track_count: usize,
    control_kinds: BTreeMap<String, usize>,
    control_states: BTreeMap<String, usize>,
}

fn build_visual_summary(
    styles: &[UiAuditStyleResolutionMetadata],
    effects: &[UiAuditEffectResolutionMetadata],
    animations: &[UiAuditAnimationSnapshotMetadata],
    controls: &[UiAuditControlSnapshotMetadata],
    images: &[UiAuditImageSnapshotMetadata],
    fonts: &[UiAuditFontSnapshotMetadata],
) -> UiAuditVisualSummary {
    let mut summary = UiAuditVisualSummary::default();
    for image in images {
        increment_count(&mut summary.image_modes, &image.presentation);
        increment_count(&mut summary.image_statuses, image.status);
    }
    for style in styles {
        for scope in &style.snapshot.scopes {
            increment_count(&mut summary.style_scopes, scope);
        }
        for entry in &style.snapshot.entries {
            if let Some(variant) = &entry.requested_variant {
                increment_count(&mut summary.style_variants, variant);
            }
        }
    }
    for font in fonts {
        increment_count(&mut summary.font_roles, font.requested_role);
        increment_count(&mut summary.font_resolution_statuses, font.status);
    }
    summary.effect_count = effects.len();
    summary.effect_fallback_count = effects
        .iter()
        .filter(|effect| effect.snapshot.fallback)
        .count();
    summary.material_request_count = effects
        .iter()
        .filter(|effect| effect.snapshot.material.is_some())
        .count();
    for animation in animations {
        increment_count(
            &mut summary.animation_policy_states,
            &animation.snapshot.policy,
        );
        for track in &animation.snapshot.tracks {
            increment_count(&mut summary.animation_track_states, &track.state);
            summary.animation_track_count += 1;
            summary.paused_animation_track_count += usize::from(track.paused);
            summary.layout_reflow_track_count += usize::from(track.causes_layout_reflow);
        }
    }
    for control in controls {
        increment_count(&mut summary.control_kinds, &control.kind);
        increment_count(&mut summary.control_states, &control.state);
    }
    summary
}

fn increment_count(map: &mut BTreeMap<String, usize>, key: impl AsRef<str>) {
    *map.entry(key.as_ref().to_owned()).or_default() += 1;
}

fn build_visual_budget(
    viewport: &UiViewport,
    stats: &UiStats,
    image_accounting: UiAuditImageAccounting,
    effects: &[UiAuditEffectResolutionMetadata],
) -> UiVisualBudgetReport {
    let additional_effect_draw_call_upper_bound = effects
        .iter()
        .map(|effect| u64::from(effect.snapshot.budget.applied_draw_call_upper_bound))
        .sum::<u64>();
    let custom_material_ids = effects
        .iter()
        .filter_map(|effect| {
            effect
                .snapshot
                .material
                .as_ref()
                .map(|material| &material.id)
        })
        .collect::<BTreeSet<_>>();
    let effect_overdraw_layers_upper_bound = effects
        .iter()
        .map(|effect| u64::from(effect.snapshot.budget.overdraw_layers))
        .max()
        .unwrap_or_default();
    let usage = UiVisualBudgetUsage {
        node_count: stats.ui_node_count as u64,
        decoded_image_bytes_estimate: image_accounting.decoded_bytes_estimate as u64,
        unresolved_image_asset_count: image_accounting.unresolved_asset_count as u64,
        render_primitive_estimate: (stats.visible_ui_node_count as u64)
            .saturating_add(additional_effect_draw_call_upper_bound),
        additional_effect_draw_call_upper_bound,
        material_count_estimate: u64::from(stats.visible_ui_node_count > 0)
            .saturating_add(custom_material_ids.len() as u64),
        effect_overdraw_layers_upper_bound,
    };
    UiVisualBudgetReport::evaluate(
        UiVisualBudgetProfile::for_width_class(viewport.width_class),
        usage,
    )
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

impl UiAuditViewportMetadata {
    fn new(viewport: UiViewport, safe_area_status: UiSafeAreaStatus) -> Self {
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
                source: safe_area_status.source.as_str(),
                revision: safe_area_status.revision,
                physical: safe_area_status.physical.map(|physical| {
                    UiAuditPhysicalSafeAreaMetadata {
                        left: physical.left,
                        right: physical.right,
                        top: physical.top,
                        bottom: physical.bottom,
                    }
                }),
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
    source: &'static str,
    revision: u64,
    physical: Option<UiAuditPhysicalSafeAreaMetadata>,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
struct UiAuditPhysicalSafeAreaMetadata {
    left: u32,
    right: u32,
    top: u32,
    bottom: u32,
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
    repetition_index: u32,
    repetition_total: u32,
    screenshot_sha256: Option<String>,
    screenshot_byte_length: Option<u64>,
    status: UiAuditRunStatus,
    failure: Option<String>,
    detail: Option<String>,
}

impl UiAuditManifestEntry {
    #[cfg(test)]
    fn success(plan: &UiAuditRunPlan, capture: &UiAuditCapturePlan) -> Self {
        Self::new(plan, capture, UiAuditRunStatus::Passed, None, None, None)
    }

    fn success_with_artifact(
        plan: &UiAuditRunPlan,
        capture: &UiAuditCapturePlan,
        artifact: &UiAuditCaptureArtifactMetadata,
    ) -> Self {
        Self::new(
            plan,
            capture,
            UiAuditRunStatus::Passed,
            None,
            None,
            Some(artifact),
        )
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
            None,
        )
    }

    fn new(
        plan: &UiAuditRunPlan,
        capture: &UiAuditCapturePlan,
        status: UiAuditRunStatus,
        failure: Option<&str>,
        detail: Option<&str>,
        artifact: Option<&UiAuditCaptureArtifactMetadata>,
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
            repetition_index: capture.repetition_index + 1,
            repetition_total: capture.repetition_total,
            screenshot_sha256: artifact.map(|artifact| artifact.sha256.clone()),
            screenshot_byte_length: artifact.map(|artifact| artifact.byte_length),
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

#[derive(Clone, Copy, Debug, Message, PartialEq, Eq)]
pub(crate) struct UiAuditCaptureStateApplied {
    pub state: UiAuditCaptureState,
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
    use bevy::ecs::system::SystemState;
    use bevy::window::WindowResolution;
    use std::str::FromStr;

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
                readiness: UiAuditReadiness {
                    panel_ready: target_panel_ready,
                    target_ready: target_panel_ready,
                    locale_ready: true,
                    theme_ready: true,
                    fonts_ready: true,
                    images_ready: true,
                    animations_ready: true,
                    viewport_ready: true,
                    signature: 1,
                    ..default()
                },
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
    fn deterministic_config_binds_environment_viewport_and_closed_policy() {
        let config = UiAuditConfig::from_env_reader(
            env_reader(&[
                (ENV_UI_AUDIT, "1"),
                (ENV_UI_AUDIT_SCREEN, "ui-generated-acceptance"),
                (ENV_UI_AUDIT_DETERMINISTIC, "1"),
                (ENV_UI_AUDIT_TARGET_LOGICAL_WIDTH, "360"),
                (ENV_UI_AUDIT_TARGET_LOGICAL_HEIGHT, "800"),
                (ENV_UI_AUDIT_TARGET_PHYSICAL_WIDTH, "720"),
                (ENV_UI_AUDIT_TARGET_PHYSICAL_HEIGHT, "1600"),
                (ENV_UI_AUDIT_TARGET_DEVICE_SCALE, "2"),
                (ENV_UI_AUDIT_LOCALE, "ZH-CN"),
                (ENV_UI_AUDIT_THEME, "default"),
                (ENV_UI_AUDIT_RANDOM_SEED, "42"),
                (ENV_UI_AUDIT_FROZEN_TIME_SECONDS, "0"),
                (ENV_UI_AUDIT_ANIMATION_PROGRESS, "1"),
                (ENV_UI_AUDIT_DYNAMIC_POLICY, "stable_fixture"),
                (ENV_UI_AUDIT_STABLE_FIXTURE_ID, "acceptance_data"),
                (ENV_UI_AUDIT_REPEAT_CAPTURES, "2"),
                (ENV_UI_AUDIT_GIT_COMMIT, "0123456789abcdef"),
            ]),
            100,
        );

        assert!(config.config_error.is_none());
        assert!(config.determinism.enabled);
        assert_eq!(config.determinism.locale, "zh_cn");
        assert_eq!(config.determinism.random_seed, Some(42));
        assert_eq!(config.determinism.repeat_captures, 2);
        assert_eq!(
            config.determinism.target_viewport,
            Some(UiAuditTargetViewportConfig {
                logical_width: 360.0,
                logical_height: 800.0,
                physical_width: 720,
                physical_height: 1600,
                device_scale: 2.0,
            })
        );
    }

    #[test]
    fn deterministic_config_requires_mask_when_dynamic_content_is_not_fixture_data() {
        let config = UiAuditConfig::from_env_reader(
            env_reader(&[
                (ENV_UI_AUDIT, "1"),
                (ENV_UI_AUDIT_SCREEN, "ui-gallery"),
                (ENV_UI_AUDIT_DETERMINISTIC, "1"),
                (ENV_UI_AUDIT_TARGET_LOGICAL_WIDTH, "360"),
                (ENV_UI_AUDIT_TARGET_LOGICAL_HEIGHT, "800"),
                (ENV_UI_AUDIT_TARGET_PHYSICAL_WIDTH, "720"),
                (ENV_UI_AUDIT_TARGET_PHYSICAL_HEIGHT, "1600"),
                (ENV_UI_AUDIT_TARGET_DEVICE_SCALE, "2"),
                (ENV_UI_AUDIT_DYNAMIC_POLICY, "explicit_mask"),
                (ENV_UI_AUDIT_REPEAT_CAPTURES, "2"),
            ]),
            100,
        );

        assert!(config.config_error.is_none());
        assert!(!validate_determinism_config(&config.determinism, true));
    }

    #[test]
    fn deterministic_runtime_injects_elapsed_time_with_zero_delta_and_terminal_animation_policy() {
        let mut app = App::new();
        let config = UiAuditDeterminismConfig {
            enabled: true,
            frozen_time_seconds: 123.5,
            ..default()
        };

        configure_deterministic_runtime(&mut app, true, &config);

        assert!(matches!(
            app.world().resource::<TimeUpdateStrategy>(),
            TimeUpdateStrategy::ManualDuration(duration) if duration.is_zero()
        ));
        assert_eq!(
            *app.world().resource::<UiMotionPolicy>(),
            UiMotionPolicy::Disabled
        );
        let time = app.world().resource::<Time<Virtual>>();
        assert!((time.elapsed_secs_f64() - 123.5).abs() <= f64::EPSILON);
        assert_eq!(time.delta(), Duration::ZERO);
        assert!(time.is_paused());
    }

    #[test]
    fn deterministic_environment_alone_does_not_change_ordinary_runtime() {
        let parsed = UiAuditConfig::from_env_reader(
            env_reader(&[
                (ENV_UI_AUDIT_DETERMINISTIC, "1"),
                (ENV_UI_AUDIT_FROZEN_TIME_SECONDS, "123.5"),
            ]),
            100,
        );
        assert!(!parsed.enabled);
        assert!(!parsed.determinism.enabled);
        assert!(!UiAuditDeterminismContext::from(&parsed.determinism).enabled);

        let mut app = App::new();
        app.init_resource::<Time<Virtual>>()
            .insert_resource(TimeUpdateStrategy::Automatic)
            .insert_resource(UiMotionPolicy::Reduced);
        app.world_mut()
            .resource_mut::<Time<Virtual>>()
            .advance_by(Duration::from_secs(42));
        let before_elapsed = app.world().resource::<Time<Virtual>>().elapsed();
        let before_delta = app.world().resource::<Time<Virtual>>().delta();

        configure_deterministic_runtime(
            &mut app,
            false,
            &UiAuditDeterminismConfig {
                enabled: true,
                frozen_time_seconds: 123.5,
                ..default()
            },
        );

        assert!(matches!(
            app.world().resource::<TimeUpdateStrategy>(),
            TimeUpdateStrategy::Automatic
        ));
        let time = app.world().resource::<Time<Virtual>>();
        assert_eq!(time.elapsed(), before_elapsed);
        assert_eq!(time.delta(), before_delta);
        assert!(!time.is_paused());
        assert_eq!(
            *app.world().resource::<UiMotionPolicy>(),
            UiMotionPolicy::Reduced
        );
    }

    #[test]
    fn deterministic_config_rejects_blank_or_malformed_numeric_values() {
        for (bad_key, bad_value) in [
            (ENV_UI_AUDIT_RANDOM_SEED, "not-a-seed"),
            (ENV_UI_AUDIT_FROZEN_TIME_SECONDS, "NaN?"),
            (ENV_UI_AUDIT_ANIMATION_PROGRESS, "done"),
            (ENV_UI_AUDIT_REPEAT_CAPTURES, ""),
            (ENV_UI_AUDIT_TARGET_LOGICAL_WIDTH, "wide"),
            (ENV_UI_AUDIT_TARGET_LOGICAL_HEIGHT, "tall"),
            (ENV_UI_AUDIT_TARGET_PHYSICAL_WIDTH, "wide"),
            (ENV_UI_AUDIT_TARGET_PHYSICAL_HEIGHT, "tall"),
            (ENV_UI_AUDIT_TARGET_DEVICE_SCALE, "retina"),
        ] {
            let value = |key, valid| if key == bad_key { bad_value } else { valid };
            let config = UiAuditConfig::from_env_reader(
                env_reader(&[
                    (ENV_UI_AUDIT, "1"),
                    (ENV_UI_AUDIT_SCREEN, "ui-gallery"),
                    (ENV_UI_AUDIT_DETERMINISTIC, "1"),
                    (
                        ENV_UI_AUDIT_TARGET_LOGICAL_WIDTH,
                        value(ENV_UI_AUDIT_TARGET_LOGICAL_WIDTH, "360"),
                    ),
                    (
                        ENV_UI_AUDIT_TARGET_LOGICAL_HEIGHT,
                        value(ENV_UI_AUDIT_TARGET_LOGICAL_HEIGHT, "800"),
                    ),
                    (
                        ENV_UI_AUDIT_TARGET_PHYSICAL_WIDTH,
                        value(ENV_UI_AUDIT_TARGET_PHYSICAL_WIDTH, "720"),
                    ),
                    (
                        ENV_UI_AUDIT_TARGET_PHYSICAL_HEIGHT,
                        value(ENV_UI_AUDIT_TARGET_PHYSICAL_HEIGHT, "1600"),
                    ),
                    (
                        ENV_UI_AUDIT_TARGET_DEVICE_SCALE,
                        value(ENV_UI_AUDIT_TARGET_DEVICE_SCALE, "2"),
                    ),
                    (
                        ENV_UI_AUDIT_RANDOM_SEED,
                        value(ENV_UI_AUDIT_RANDOM_SEED, "42"),
                    ),
                    (
                        ENV_UI_AUDIT_FROZEN_TIME_SECONDS,
                        value(ENV_UI_AUDIT_FROZEN_TIME_SECONDS, "123.5"),
                    ),
                    (
                        ENV_UI_AUDIT_ANIMATION_PROGRESS,
                        value(ENV_UI_AUDIT_ANIMATION_PROGRESS, "1"),
                    ),
                    (
                        ENV_UI_AUDIT_REPEAT_CAPTURES,
                        value(ENV_UI_AUDIT_REPEAT_CAPTURES, "2"),
                    ),
                ]),
                100,
            );

            assert_eq!(
                config.config_error,
                Some(UiAuditFailureKind::ConfigInvalid),
                "{bad_key}={bad_value:?} must not silently fall back"
            );
        }
    }

    #[test]
    fn deterministic_config_requires_finite_non_negative_frozen_time() {
        let base = UiAuditDeterminismConfig {
            enabled: true,
            target_viewport: Some(UiAuditTargetViewportConfig {
                logical_width: 390.0,
                logical_height: 844.0,
                physical_width: 780,
                physical_height: 1688,
                device_scale: 2.0,
            }),
            repeat_captures: 2,
            ..default()
        };
        let valid = UiAuditDeterminismConfig {
            frozen_time_seconds: 123.5,
            ..base.clone()
        };
        assert!(validate_determinism_config(&valid, true));
        for frozen_time_seconds in [-1.0, f64::NAN, f64::INFINITY] {
            let invalid = UiAuditDeterminismConfig {
                frozen_time_seconds,
                ..base.clone()
            };
            assert!(!validate_determinism_config(&invalid, true));
        }
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
    fn config_accepts_visual_acceptance_capture_state() {
        let config = UiAuditConfig::from_env_reader(
            env_reader(&[
                (ENV_UI_AUDIT, "1"),
                (ENV_UI_AUDIT_SCREEN, "ui-gallery"),
                (ENV_UI_AUDIT_STATES, "visual_acceptance"),
            ]),
            100,
        );

        assert_eq!(config.states, vec![UiAuditCaptureState::VisualAcceptance]);
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
    fn config_accepts_typography_capture_states() {
        let config = UiAuditConfig::from_env_reader(
            env_reader(&[
                (ENV_UI_AUDIT, "1"),
                (ENV_UI_AUDIT_SCREEN, "ui-gallery"),
                (ENV_UI_AUDIT_STATES, "typography,typography_overflow"),
            ]),
            100,
        );

        assert_eq!(
            config.states,
            vec![
                UiAuditCaptureState::Typography,
                UiAuditCaptureState::TypographyOverflow,
            ]
        );
        assert!(config.states_from_env);
        assert!(config.config_error.is_none());
    }

    #[test]
    fn config_accepts_icon_capture_states() {
        let config = UiAuditConfig::from_env_reader(
            env_reader(&[
                (ENV_UI_AUDIT, "1"),
                (ENV_UI_AUDIT_SCREEN, "ui-gallery"),
                (ENV_UI_AUDIT_STATES, "icons,icon_states"),
            ]),
            100,
        );

        assert_eq!(
            config.states,
            vec![UiAuditCaptureState::Icons, UiAuditCaptureState::IconStates]
        );
        assert!(config.states_from_env);
        assert!(config.config_error.is_none());
    }

    #[test]
    fn config_accepts_style_scope_capture_state() {
        let config = UiAuditConfig::from_env_reader(
            env_reader(&[
                (ENV_UI_AUDIT, "1"),
                (ENV_UI_AUDIT_SCREEN, "ui-gallery"),
                (ENV_UI_AUDIT_STATES, "style_scopes"),
            ]),
            100,
        );

        assert_eq!(config.states, vec![UiAuditCaptureState::StyleScopes]);
        assert!(config.states_from_env);
        assert!(config.config_error.is_none());
    }

    #[test]
    fn config_accepts_effects_capture_state() {
        let config = UiAuditConfig::from_env_reader(
            env_reader(&[
                (ENV_UI_AUDIT, "1"),
                (ENV_UI_AUDIT_SCREEN, "ui-gallery"),
                (ENV_UI_AUDIT_STATES, "effects"),
            ]),
            100,
        );

        assert_eq!(config.states, vec![UiAuditCaptureState::Effects]);
        assert!(config.states_from_env);
        assert!(config.config_error.is_none());
    }

    #[test]
    fn config_accepts_animations_capture_state() {
        let config = UiAuditConfig::from_env_reader(
            env_reader(&[
                (ENV_UI_AUDIT, "1"),
                (ENV_UI_AUDIT_SCREEN, "ui-gallery"),
                (ENV_UI_AUDIT_STATES, "animations"),
            ]),
            100,
        );

        assert_eq!(config.states, vec![UiAuditCaptureState::Animations]);
        assert!(config.states_from_env);
        assert!(config.config_error.is_none());
    }

    #[test]
    fn config_accepts_component_capture_states() {
        let config = UiAuditConfig::from_env_reader(
            env_reader(&[
                (ENV_UI_AUDIT, "1"),
                (ENV_UI_AUDIT_SCREEN, "ui-gallery"),
                (
                    ENV_UI_AUDIT_STATES,
                    "components,component_checkboxes,component_toggles,component_segmented,component_overlays,component_tooltip",
                ),
            ]),
            100,
        );

        assert_eq!(
            config.states,
            vec![
                UiAuditCaptureState::Components,
                UiAuditCaptureState::ComponentCheckboxes,
                UiAuditCaptureState::ComponentToggles,
                UiAuditCaptureState::ComponentSegmented,
                UiAuditCaptureState::ComponentOverlays,
                UiAuditCaptureState::ComponentTooltip,
            ]
        );
        assert!(config.states_from_env);
        assert!(config.config_error.is_none());
    }

    #[test]
    fn audit_metadata_collects_control_snapshots_in_stable_id_order() {
        let mut world = World::new();
        world.spawn((
            Name::new("control-z"),
            UiControlMeta::new(
                crate::framework::ui::widgets::UiControlId::new("z.control"),
                crate::framework::ui::widgets::UiControlKind::Dropdown,
            ),
            Interaction::Hovered,
            UiControlFlags {
                error: true,
                ..default()
            },
        ));
        world.spawn((
            Name::new("control-a"),
            UiControlMeta::new(
                crate::framework::ui::widgets::UiControlId::new("a.control"),
                crate::framework::ui::widgets::UiControlKind::Badge,
            ),
            UiBadge {
                state: crate::framework::ui::widgets::UiControlState::Selected,
            },
        ));
        world.spawn((
            Name::new("tooltip-disabled"),
            UiControlMeta::new(
                crate::framework::ui::widgets::UiControlId::new("tooltip.disabled"),
                crate::framework::ui::widgets::UiControlKind::Tooltip,
            ),
            UiTooltip {
                text: "Unavailable".to_owned(),
                tone: UiTooltipTone::Error,
            },
            DisabledButton,
        ));
        let mut state = SystemState::<
            Query<(
                Entity,
                Option<&Name>,
                &UiControlMeta,
                Option<&Interaction>,
                Option<&UiControlFlags>,
                Has<FocusedButton>,
                Has<DisabledButton>,
                Option<&UiBadge>,
                Option<&UiProgress>,
                Option<&UiTooltip>,
            )>,
        >::new(&mut world);
        let query = state.get(&world);

        let metadata = collect_control_snapshot_metadata(&query);

        assert_eq!(metadata.len(), 3);
        assert_eq!(metadata[0].control_id, "a.control");
        assert_eq!(metadata[0].state, "selected");
        assert!(metadata[0].selected);
        assert_eq!(metadata[1].control_id, "tooltip.disabled");
        assert_eq!(metadata[1].state, "disabled");
        assert!(metadata[1].disabled);
        assert_eq!(metadata[2].control_id, "z.control");
        assert_eq!(metadata[2].state, "error");
        assert!(metadata[2].error);
    }

    #[test]
    fn audit_metadata_collects_resolved_style_snapshots_in_stable_order() {
        let mut world = World::new();
        world.spawn((
            Name::new("style-z"),
            UiResolvedStyleDebugSnapshot {
                scopes: vec!["scope.z".to_owned()],
                entries: Vec::new(),
            },
        ));
        world.spawn((
            Name::new("style-a"),
            UiResolvedStyleDebugSnapshot {
                scopes: vec!["scope.a".to_owned()],
                entries: Vec::new(),
            },
        ));
        let mut state =
            SystemState::<Query<(Entity, Option<&Name>, &UiResolvedStyleDebugSnapshot)>>::new(
                &mut world,
            );
        let query = state.get(&world);

        let metadata = collect_style_resolution_metadata(&query);

        assert_eq!(metadata.len(), 2);
        assert_eq!(metadata[0].name.as_deref(), Some("style-a"));
        assert_eq!(metadata[0].snapshot.scopes, vec!["scope.a"]);
        assert_eq!(metadata[1].name.as_deref(), Some("style-z"));
        let json = serde_json::to_string(&metadata).unwrap();
        assert!(json.contains("style_resolutions") || json.contains("scope.a"));
    }

    #[test]
    fn audit_metadata_collects_declarative_source_and_effective_style_stably() {
        let mut world = World::new();
        let document_id =
            crate::framework::ui::document::UiDocumentId::from_str("audit.document").unwrap();
        for (index, node) in ["audit.z", "audit.a"].into_iter().enumerate() {
            world.spawn((
                UiDocumentNodeAuditMarker {
                    instance_id: crate::framework::ui::document::UiDocumentInstanceId(7),
                    document_id: document_id.clone(),
                    schema_version: 1,
                    node_id: crate::framework::ui::document::UiNodeId::from_str(node).unwrap(),
                    document_path: format!("$.root.children[{index}]"),
                    source_path: "ui/documents/approved/audit/page.json".to_owned(),
                },
                UiDocumentResolvedStyleMarker(
                    crate::framework::ui::document::UiResolvedStyle::default(),
                ),
            ));
        }
        let mut state = SystemState::<
            Query<(
                Entity,
                &UiDocumentNodeAuditMarker,
                &UiDocumentResolvedStyleMarker,
            )>,
        >::new(&mut world);
        let query = state.get(&world);
        let metadata = collect_document_node_metadata(&query);

        assert_eq!(metadata.len(), 2);
        assert_eq!(metadata[0].node_id, "audit.a");
        assert_eq!(metadata[1].node_id, "audit.z");
        assert!(metadata.iter().all(|node| {
            node.document_id == "audit.document"
                && node.schema_version == 1
                && node.source_path == "ui/documents/approved/audit/page.json"
        }));
    }

    #[test]
    fn audit_metadata_collects_resolved_effect_snapshots_in_stable_order() {
        let snapshot = |request: &str, fallback| UiResolvedEffectDebugSnapshot {
            request: request.to_owned(),
            resolved_preset: request.to_owned(),
            applied_components: vec!["box_shadow".to_owned()],
            material: None,
            budget: crate::framework::ui::style::UiEffectBudgetSnapshot::default(),
            fallback,
            error: fallback.then(|| "ui_material_shader_unavailable".to_owned()),
        };
        let mut world = World::new();
        world.spawn((Name::new("effect-z"), snapshot("gallery.z", true)));
        world.spawn((Name::new("effect-a"), snapshot("gallery.a", false)));
        let mut state =
            SystemState::<Query<(Entity, Option<&Name>, &UiResolvedEffectDebugSnapshot)>>::new(
                &mut world,
            );
        let query = state.get(&world);

        let metadata = collect_effect_resolution_metadata(&query);

        assert_eq!(metadata.len(), 2);
        assert_eq!(metadata[0].name.as_deref(), Some("effect-a"));
        assert_eq!(metadata[0].snapshot.request, "gallery.a");
        assert_eq!(metadata[1].name.as_deref(), Some("effect-z"));
        assert!(metadata[1].snapshot.fallback);
        let json = serde_json::to_string(&metadata).unwrap();
        assert!(json.contains("ui_material_shader_unavailable"));
        assert!(json.contains("requested_draw_call_upper_bound"));
    }

    #[test]
    fn audit_metadata_collects_animation_snapshots_in_stable_order() {
        let snapshot = |id: &str| UiAnimationDebugSnapshot {
            policy: "full".to_owned(),
            tracks: vec![crate::framework::ui::core::UiAnimationTrackDebugSnapshot {
                id: id.to_owned(),
                target: "transform_scale".to_owned(),
                state: "running".to_owned(),
                raw_progress: 0.625,
                eased_progress: 0.625,
                paused: true,
                causes_layout_reflow: false,
            }],
        };
        let mut world = World::new();
        world.spawn((Name::new("animation-z"), snapshot("gallery.z")));
        world.spawn((Name::new("animation-a"), snapshot("gallery.a")));
        let mut state =
            SystemState::<Query<(Entity, Option<&Name>, &UiAnimationDebugSnapshot)>>::new(
                &mut world,
            );
        let query = state.get(&world);

        let metadata = collect_animation_snapshot_metadata(&query);

        assert_eq!(metadata.len(), 2);
        assert_eq!(metadata[0].name.as_deref(), Some("animation-a"));
        assert_eq!(metadata[0].snapshot.tracks[0].id, "gallery.a");
        assert_eq!(metadata[1].name.as_deref(), Some("animation-z"));
        let json = serde_json::to_string(&metadata).unwrap();
        assert!(json.contains("raw_progress"));
        assert!(json.contains("causes_layout_reflow"));
    }

    #[test]
    fn audit_metadata_collects_image_modes_and_deduplicates_memory() {
        use crate::framework::ui::widgets::{UiImageFit, UiImageSize, ui_image};

        let mut image_assets = Assets::<Image>::default();
        let mut image = Image::default();
        image.data = Some(vec![255; 16]);
        let handle = image_assets.add(image);
        let mut world = World::new();
        let contain = world
            .spawn(ui_image(
                handle.clone(),
                UiImageFit::Contain,
                UiImageSize::FixedBox {
                    width: 20.0,
                    height: 20.0,
                },
            ))
            .id();
        world.entity_mut(contain).insert(Name::new("image-b"));
        let cover = world
            .spawn(ui_image(
                handle,
                UiImageFit::cover(crate::framework::ui::widgets::UiImageFocus::CENTER),
                UiImageSize::FixedBox {
                    width: 20.0,
                    height: 20.0,
                },
            ))
            .id();
        world.entity_mut(cover).insert(Name::new("image-a"));
        let mut state = SystemState::<
            Query<(
                Entity,
                Option<&Name>,
                &ImageNode,
                Option<&UiImageWidget>,
                Option<&UiImageStatus>,
            )>,
        >::new(&mut world);
        let query = state.get(&world);

        let (metadata, accounting) = collect_image_snapshot_metadata(&query, &image_assets);

        assert_eq!(metadata.len(), 2);
        assert_eq!(metadata[0].name.as_deref(), Some("image-a"));
        assert_eq!(metadata[0].presentation, "cover");
        assert_eq!(metadata[1].presentation, "contain");
        assert_eq!(accounting.unique_asset_count, 1);
        assert_eq!(accounting.decoded_bytes_estimate, 16);
        assert_eq!(accounting.unresolved_asset_count, 0);
    }

    #[test]
    fn audit_metadata_collects_font_roles_without_text_content() {
        use crate::framework::ui::style::{
            UiFontFamily, UiFontResolutionStatus, UiFontRole, UiFontWeight, fonts::UiFontFaceKey,
        };

        let mut world = World::new();
        world.spawn((
            Name::new("font-body"),
            UiTextStyleToken {
                font_role: UiFontRole::Body,
                font_family: UiFontFamily::ProductCjk,
                font_weight: UiFontWeight::Regular,
                font_size: 18.0,
                line_height: crate::framework::ui::style::UiTextLineHeight::Relative(1.2),
                alignment: crate::framework::ui::style::UiTextAlignment::Left,
                wrap: crate::framework::ui::style::UiTextWrap::WordOrCharacter,
                truncation: crate::framework::ui::style::UiTextTruncation::None,
            },
            UiFontResolution {
                face: UiFontFaceKey::new(UiFontFamily::ProductCjk, UiFontWeight::Regular),
                rendered_source: "private text is not emitted".to_owned(),
                status: UiFontResolutionStatus::Ready,
            },
        ));
        let mut state = SystemState::<
            Query<(Entity, Option<&Name>, &UiTextStyleToken, &UiFontResolution)>,
        >::new(&mut world);
        let query = state.get(&world);

        let metadata = collect_font_snapshot_metadata(&query);
        let json = serde_json::to_string(&metadata).unwrap();

        assert_eq!(metadata[0].requested_role, "body");
        assert_eq!(metadata[0].status, "ready");
        assert!(!json.contains("private text"));
    }

    #[test]
    fn audit_visual_budget_reuses_effect_planning_values() {
        let effects = vec![UiAuditEffectResolutionMetadata {
            entity: "1v0".to_owned(),
            name: Some("effect".to_owned()),
            snapshot: UiResolvedEffectDebugSnapshot {
                request: "gallery.effect".to_owned(),
                resolved_preset: "gallery.effect".to_owned(),
                applied_components: vec!["box_shadow".to_owned()],
                material: None,
                budget: crate::framework::ui::style::UiEffectBudgetSnapshot {
                    requested_draw_call_upper_bound: 3,
                    applied_draw_call_upper_bound: 2,
                    overdraw_layers: 2,
                    shadow_layers: 1,
                    gradient_stops: 0,
                },
                fallback: false,
                error: None,
            },
        }];
        let viewport =
            UiViewport::from_device_logical_size(360.0, 800.0, UiInputMode::MouseTouch, default());
        let stats = UiStats {
            ui_node_count: 1_100,
            visible_ui_node_count: 1_000,
            ..default()
        };
        let report = build_visual_budget(
            &viewport,
            &stats,
            UiAuditImageAccounting {
                unique_asset_count: 4,
                decoded_bytes_estimate: 24 * 1024 * 1024,
                unresolved_asset_count: 1,
            },
            &effects,
        );

        assert_eq!(
            report.status,
            crate::framework::ui::visual::UiVisualBudgetStatus::Passed
        );
        assert_eq!(report.usage.additional_effect_draw_call_upper_bound, 2);
        assert_eq!(report.usage.effect_overdraw_layers_upper_bound, 2);
        assert_eq!(report.usage.unresolved_image_asset_count, 1);
        assert!(report.accounting.contains("not measured GPU"));
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
            UiAuditFailureKind::DocumentNotReady.as_str(),
            "document_not_ready"
        );
        assert_eq!(
            UiAuditFailureKind::LocaleNotReady.as_str(),
            "locale_not_ready"
        );
        assert_eq!(
            UiAuditFailureKind::ThemeNotReady.as_str(),
            "theme_not_ready"
        );
        assert_eq!(UiAuditFailureKind::FontNotReady.as_str(), "font_not_ready");
        assert_eq!(
            UiAuditFailureKind::ImageNotReady.as_str(),
            "image_not_ready"
        );
        assert_eq!(
            UiAuditFailureKind::ScreenshotSizeMismatch.as_str(),
            "screenshot_size_mismatch"
        );
        assert_eq!(
            UiAuditFailureKind::NondeterministicCapture.as_str(),
            "nondeterministic_capture"
        );
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
    fn screen_reference_recipe_populates_run_plan_without_matching_environment_fields() {
        const RECIPE_CAPTURES: &[UiAuditCaptureRecipe] = &[UiAuditCaptureRecipe::initial()];
        let reference = UiAuditReferenceRecipe::DEFAULT
            .with_target_viewport(UiAuditTargetViewport::RuntimeProfile)
            .with_locale("en-US")
            .with_theme("night")
            .with_random_seed(Some(77))
            .with_frozen_time_seconds(123.5)
            .with_animation_progress(1.0)
            .with_dynamic_content(UiAuditDynamicContentRecipe::ExplicitMask("clock-region"));
        let mut registry = UiAuditScreenRegistry::default();
        registry.register(
            UiAuditScreen::new(
                "recipe_screen",
                &["recipe-screen"],
                UiOwnerId::new("recipe_screen"),
            )
            .with_recipe(UiAuditRecipe::new(RECIPE_CAPTURES).with_reference(reference)),
        );
        let config = UiAuditConfig::from_env_reader(
            env_reader(&[
                (ENV_UI_AUDIT, "1"),
                (ENV_UI_AUDIT_SCREEN, "recipe-screen"),
                (ENV_UI_AUDIT_DETERMINISTIC, "1"),
                (ENV_UI_AUDIT_REPEAT_CAPTURES, "2"),
            ]),
            100,
        );
        assert!(config.config_error.is_none());

        let mut world = World::new();
        world.spawn((
            Window {
                resolution: WindowResolution::new(780, 1688).with_scale_factor_override(2.0),
                ..default()
            },
            PrimaryWindow,
        ));
        let mut state = SystemState::<Query<&Window, With<PrimaryWindow>>>::new(&mut world);
        let windows = state.get(&world);
        let expected_viewport = target_viewport_from_window(windows.single().unwrap());

        let plan = prepare_runtime_plan(&config, &registry, &windows).unwrap();

        assert_eq!(plan.determinism.target_viewport, Some(expected_viewport));
        assert_eq!(plan.determinism.locale, "en_us");
        assert_eq!(plan.determinism.theme, "night");
        assert_eq!(plan.determinism.random_seed, Some(77));
        assert_eq!(plan.determinism.frozen_time_seconds, 123.5);
        assert_eq!(
            plan.determinism.dynamic_policy,
            UiAuditDynamicPolicy::ExplicitMask
        );
        assert_eq!(
            plan.determinism.dynamic_mask_id.as_deref(),
            Some("clock-region")
        );
        assert_eq!(plan.captures.len(), 2);
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
    fn deterministic_plan_expands_repetitions_with_stable_identity() {
        let plan = plan_audit_paths_with_determinism(
            Path::new("../summary/ui-audit/repeat-run"),
            resolved_test_screen(),
            "phone-small",
            None,
            UiAuditDeterminismConfig {
                enabled: true,
                repeat_captures: 2,
                ..default()
            },
            &[UiAuditCaptureRecipe::initial()],
        );

        assert_eq!(plan.captures.len(), 2);
        assert_eq!(plan.captures[0].repetition_index, 0);
        assert_eq!(plan.captures[1].repetition_index, 1);
        assert_eq!(plan.captures[1].repetition_total, 2);
        assert!(
            plan.captures[0]
                .screenshot_path
                .ends_with("00-initial-repeat-01.png")
        );
        assert!(
            plan.captures[1]
                .screenshot_path
                .ends_with("01-initial-repeat-02.png")
        );
    }

    #[test]
    fn repeated_capture_uses_exact_png_sha256_evidence() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("test clock should follow epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "mybevy-ui-audit-repeat-{}-{unique}",
            std::process::id()
        ));
        let plan = plan_audit_paths_with_determinism(
            &root,
            resolved_test_screen(),
            "phone-small",
            None,
            UiAuditDeterminismConfig {
                enabled: true,
                repeat_captures: 2,
                ..default()
            },
            &[UiAuditCaptureRecipe::initial()],
        );
        let evidence = UiAuditScreenshotEvidence {
            captured_size: (2, 2),
            requested_logical_size: Some((2, 2)),
            requested_physical_size: Some((2, 2)),
            request_frame: 10,
            completion_frame: 11,
        };
        fs::create_dir_all(
            plan.captures[0]
                .screenshot_path
                .parent()
                .expect("capture path has a parent"),
        )
        .unwrap();
        fs::write(&plan.captures[0].screenshot_path, b"exact-png-a").unwrap();
        fs::write(&plan.captures[1].screenshot_path, b"exact-png-a").unwrap();
        let first = build_capture_artifact_metadata(&plan.captures[0], Some(&evidence), None)
            .expect("first artifact should hash");
        let matching = build_capture_artifact_metadata(
            &plan.captures[1],
            Some(&evidence),
            Some(&first.sha256),
        )
        .expect("matching artifact should hash");
        assert_eq!(matching.exact_match_with_first_repetition, Some(true));

        fs::write(&plan.captures[1].screenshot_path, b"exact-png-b").unwrap();
        let changed = build_capture_artifact_metadata(
            &plan.captures[1],
            Some(&evidence),
            Some(&first.sha256),
        )
        .expect("changed artifact should still hash");
        assert_eq!(changed.exact_match_with_first_repetition, Some(false));
        fs::remove_dir_all(root).unwrap();
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
    fn state_machine_distinguishes_document_build_timeout() {
        assert_eq!(
            advance_audit_phase(
                UiAuditPhase::WaitForScreen {
                    waited_frames: PANEL_READY_TIMEOUT_FRAMES,
                },
                UiAuditStepInput {
                    readiness: UiAuditReadiness {
                        target_not_ready_failure: UiAuditFailureKind::DocumentNotReady,
                        ..default()
                    },
                    screenshot_status: UiAuditScreenshotStatus::Pending,
                },
            ),
            (
                UiAuditPhase::Failed(UiAuditFailureKind::DocumentNotReady),
                Some(UiAuditPureAction::Fail(
                    UiAuditFailureKind::DocumentNotReady,
                )),
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
                UiAuditPhase::WaitForStable {
                    waited_frames: 0,
                    stable_frames: 0,
                    last_signature: None,
                },
                Some(UiAuditPureAction::ApplyCaptureState)
            )
        );
    }

    #[test]
    fn state_machine_waits_fixed_stable_frames_before_screenshot() {
        assert_eq!(
            step(
                UiAuditPhase::WaitForStable {
                    waited_frames: 4,
                    stable_frames: 4,
                    last_signature: Some(1),
                },
                true,
                UiAuditScreenshotStatus::Pending
            ),
            (
                UiAuditPhase::WaitForStable {
                    waited_frames: 5,
                    stable_frames: 5,
                    last_signature: Some(1),
                },
                None
            )
        );
        assert_eq!(
            step(
                UiAuditPhase::WaitForStable {
                    waited_frames: STABLE_WAIT_FRAMES,
                    stable_frames: STABLE_WAIT_FRAMES - 1,
                    last_signature: Some(1),
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
                UiAuditPhase::WaitForStable {
                    waited_frames: 2,
                    stable_frames: 2,
                    last_signature: Some(1),
                },
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
    fn state_machine_classifies_resource_and_viewport_timeouts_independently() {
        let timeout_phase = UiAuditPhase::WaitForStable {
            waited_frames: STABLE_TIMEOUT_FRAMES,
            stable_frames: 0,
            last_signature: None,
        };
        let readiness = |locale_ready, theme_ready, fonts_ready, images_ready, viewport_ready| {
            UiAuditReadiness {
                panel_ready: true,
                target_ready: true,
                locale_ready,
                theme_ready,
                fonts_ready,
                images_ready,
                animations_ready: true,
                viewport_ready,
                ..default()
            }
        };
        for (readiness, failure) in [
            (
                readiness(false, true, true, true, true),
                UiAuditFailureKind::LocaleNotReady,
            ),
            (
                readiness(true, false, true, true, true),
                UiAuditFailureKind::ThemeNotReady,
            ),
            (
                readiness(true, true, false, true, true),
                UiAuditFailureKind::FontNotReady,
            ),
            (
                readiness(true, true, true, false, true),
                UiAuditFailureKind::ImageNotReady,
            ),
            (
                readiness(true, true, true, true, false),
                UiAuditFailureKind::ScreenshotSizeMismatch,
            ),
        ] {
            assert_eq!(
                advance_audit_phase(
                    timeout_phase.clone(),
                    UiAuditStepInput {
                        readiness,
                        screenshot_status: UiAuditScreenshotStatus::Pending,
                    },
                ),
                (
                    UiAuditPhase::Failed(failure),
                    Some(UiAuditPureAction::Fail(failure)),
                )
            );
        }
    }

    #[test]
    fn resource_gate_distinguishes_loading_from_stable_fallback_and_invalid_states() {
        use crate::framework::ui::{style::UiFontResolutionStatus, widgets::UiImageError};

        assert!(ui_font_resource_ready(&UiFontResolutionStatus::Ready));
        assert!(!ui_font_resource_ready(&UiFontResolutionStatus::Loading {
            used_fallback: true,
        }));
        assert!(!ui_font_resource_ready(
            &UiFontResolutionStatus::Unavailable
        ));
        assert!(ui_image_resource_ready(
            true,
            Some(UiImageStatus::Ready {
                source_size: UVec2::new(10, 10),
            }),
            true,
        ));
        assert!(ui_image_resource_ready(
            true,
            Some(UiImageStatus::Invalid(UiImageError::ZeroContainerSize)),
            false,
        ));
        assert!(!ui_image_resource_ready(
            true,
            Some(UiImageStatus::Loading),
            false,
        ));
        assert!(!ui_image_resource_ready(
            true,
            Some(UiImageStatus::Failed),
            false,
        ));
        assert!(ui_image_resource_ready(false, None, true));
    }

    #[test]
    fn scoped_resource_gate_blocks_a_pending_image_under_the_target_root() {
        let mut world = World::new();
        let target_root = world.spawn_empty().id();
        world.spawn((
            ImageNode::new(Handle::<Image>::default()),
            ChildOf(target_root),
        ));
        let mut state = SystemState::<(
            Query<&ChildOf>,
            Query<(
                Entity,
                Option<&Name>,
                &ImageNode,
                Option<&UiImageWidget>,
                Option<&UiImageStatus>,
            )>,
        )>::new(&mut world);
        let (parents, images) = state.get(&world);

        assert!(!scoped_images_ready(
            Some(target_root),
            &parents,
            &images,
            &Assets::<Image>::default(),
        ));
    }

    #[test]
    fn scoped_resource_gate_ignores_a_pending_image_under_an_unrelated_root() {
        let mut world = World::new();
        let target_root = world.spawn_empty().id();
        let unrelated_root = world.spawn_empty().id();
        world.spawn((
            ImageNode::new(Handle::<Image>::default()),
            ChildOf(unrelated_root),
        ));
        let mut state = SystemState::<(
            Query<&ChildOf>,
            Query<(
                Entity,
                Option<&Name>,
                &ImageNode,
                Option<&UiImageWidget>,
                Option<&UiImageStatus>,
            )>,
        )>::new(&mut world);
        let (parents, images) = state.get(&world);

        assert!(scoped_images_ready(
            Some(target_root),
            &parents,
            &images,
            &Assets::<Image>::default(),
        ));
    }

    #[test]
    fn document_readiness_does_not_cross_satisfy_instances_with_the_same_document_id() {
        let mut world = World::new();
        let first_root = world.spawn_empty().id();
        let second_root = world.spawn_empty().id();
        let document_id =
            crate::framework::ui::document::UiDocumentId::from_str("audit.same").unwrap();
        world.spawn((
            UiDocumentNodeAuditMarker {
                instance_id: UiDocumentInstanceId(1),
                document_id,
                schema_version: 1,
                node_id: crate::framework::ui::document::UiNodeId::from_str("audit.node").unwrap(),
                document_path: "$.root".to_owned(),
                source_path: "audit.json".to_owned(),
            },
            UiDocumentResolvedStyleMarker(
                crate::framework::ui::document::UiResolvedStyle::default(),
            ),
            ChildOf(first_root),
        ));
        let mut state = SystemState::<(
            Query<(
                Entity,
                &UiDocumentNodeAuditMarker,
                &UiDocumentResolvedStyleMarker,
            )>,
            Query<&ChildOf>,
        )>::new(&mut world);
        let (nodes, parents) = state.get(&world);

        assert!(document_instance_ready(
            first_root,
            UiDocumentInstanceId(1),
            &nodes,
            &parents,
        ));
        assert!(!document_instance_ready(
            second_root,
            UiDocumentInstanceId(2),
            &nodes,
            &parents,
        ));
    }

    #[test]
    fn locale_and_theme_readiness_require_the_requested_loaded_sources() {
        assert!(requested_locale_is_active("zh-CN", "zh_cn"));
        assert!(!requested_locale_is_active("en_us", "zh_cn"));
        assert!(requested_theme_file_is_loaded(
            "default",
            Some("default.ron"),
            false,
        ));
        assert!(!requested_theme_file_is_loaded("default", None, true,));
        assert!(!requested_theme_file_is_loaded(
            "night",
            Some("default.ron"),
            false,
        ));
        assert!(requested_theme_file_is_loaded(
            "night",
            Some("night.ron"),
            false,
        ));
    }

    #[test]
    fn state_machine_resets_stability_on_signature_change() {
        let result = advance_audit_phase(
            UiAuditPhase::WaitForStable {
                waited_frames: 10,
                stable_frames: 9,
                last_signature: Some(1),
            },
            UiAuditStepInput {
                readiness: UiAuditReadiness {
                    panel_ready: true,
                    target_ready: true,
                    locale_ready: true,
                    theme_ready: true,
                    fonts_ready: true,
                    images_ready: true,
                    animations_ready: true,
                    viewport_ready: true,
                    signature: 2,
                    ..default()
                },
                screenshot_status: UiAuditScreenshotStatus::Pending,
            },
        );

        assert_eq!(
            result,
            (
                UiAuditPhase::WaitForStable {
                    waited_frames: 11,
                    stable_frames: 1,
                    last_signature: Some(2),
                },
                None,
            )
        );
    }

    #[test]
    fn state_machine_classifies_saved_size_mismatch() {
        assert_eq!(
            step(
                UiAuditPhase::WaitForScreenshot { waited_frames: 1 },
                true,
                UiAuditScreenshotStatus::SizeMismatch,
            ),
            (
                UiAuditPhase::Failed(UiAuditFailureKind::ScreenshotSizeMismatch),
                Some(UiAuditPureAction::Fail(
                    UiAuditFailureKind::ScreenshotSizeMismatch,
                )),
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
