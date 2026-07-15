use std::{
    env, fs,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

use bevy::{
    app::AppExit,
    asset::{AssetPlugin, LoadState, UntypedHandle},
    prelude::*,
    ui::IsDefaultUiCamera,
    window::WindowResolution,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::{
    UI_DOCUMENT_MAX_BYTES, UiAssetKind, UiAssetSource, UiDocument, UiDocumentBuildState,
    UiDocumentId, UiDocumentInputMode, UiDocumentLayer, UiDocumentPanel, UiDocumentPlatform,
    UiDocumentPreviewCommand, UiDocumentPreviewRegistration, UiDocumentReloadEvent,
    UiDocumentReloadStatus, UiDocumentRequestId, UiDocumentRuntime, UiDocumentRuntimeSystems,
    UiDocumentSourcePath, UiDocumentSourceRoot, UiPageState, UiSafeAreaClass, UiTargetProfile,
    ValidatedUiDocument,
};
use crate::framework::ui::{
    audit::{UiScreenshotCommand, UiScreenshotEvent, UiScreenshotFailureReason},
    core::UiFrameworkPlugin,
    style::{UiFontResolution, UiFontResolutionStatus},
};

const PREVIEW_OWNER: &str = "ui_generation_standalone_preview";
const MIN_DIMENSION: u32 = 64;
const MAX_DIMENSION: u32 = 4096;
const MIN_TIMEOUT_FRAMES: u32 = 60;
const MAX_TIMEOUT_FRAMES: u32 = 3600;
const DEFAULT_STABLE_FRAMES: u32 = 30;

#[derive(Clone, Debug)]
pub struct UiDocumentStandalonePreviewOptions {
    pub document_path: PathBuf,
    pub screenshot_path: PathBuf,
    pub result_path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub timeout_frames: u32,
    pub stable_frames: u32,
}

impl UiDocumentStandalonePreviewOptions {
    pub fn parse_env_args() -> Result<Self, UiDocumentStandalonePreviewError> {
        Self::parse_args(env::args_os().skip(1))
    }

    fn parse_args(
        arguments: impl IntoIterator<Item = std::ffi::OsString>,
    ) -> Result<Self, UiDocumentStandalonePreviewError> {
        let mut arguments = arguments.into_iter();
        let mut document_path = None;
        let mut screenshot_path = None;
        let mut result_path = None;
        let mut width = 390;
        let mut height = 844;
        let mut timeout_frames = 1200;
        let mut stable_frames = DEFAULT_STABLE_FRAMES;
        while let Some(flag) = arguments.next() {
            let flag = flag.to_str().ok_or_else(|| {
                setup_error(
                    UiDocumentStandalonePreviewFailureKind::ConfigurationInvalid,
                    "UI_DOCUMENT_PREVIEW_ARGUMENT_INVALID",
                    "preview arguments must be valid UTF-8",
                )
            })?;
            let value = arguments.next().ok_or_else(|| {
                setup_error(
                    UiDocumentStandalonePreviewFailureKind::ConfigurationInvalid,
                    "UI_DOCUMENT_PREVIEW_ARGUMENT_MISSING",
                    "each preview option requires exactly one value",
                )
            })?;
            match flag {
                "--document" => document_path = Some(PathBuf::from(value)),
                "--screenshot" => screenshot_path = Some(PathBuf::from(value)),
                "--result" => result_path = Some(PathBuf::from(value)),
                "--width" => width = parse_u32(value, "width")?,
                "--height" => height = parse_u32(value, "height")?,
                "--timeout-frames" => timeout_frames = parse_u32(value, "timeout frames")?,
                "--stable-frames" => stable_frames = parse_u32(value, "stable frames")?,
                _ => {
                    return Err(setup_error(
                        UiDocumentStandalonePreviewFailureKind::ConfigurationInvalid,
                        "UI_DOCUMENT_PREVIEW_ARGUMENT_UNKNOWN",
                        "preview command contains an unknown option",
                    ));
                }
            }
        }
        let options = Self {
            document_path: required_path(document_path, "document")?,
            screenshot_path: required_path(screenshot_path, "screenshot")?,
            result_path: required_path(result_path, "result")?,
            width,
            height,
            timeout_frames,
            stable_frames,
        };
        options.validate()?;
        Ok(options)
    }

    fn validate(&self) -> Result<(), UiDocumentStandalonePreviewError> {
        if !(MIN_DIMENSION..=MAX_DIMENSION).contains(&self.width)
            || !(MIN_DIMENSION..=MAX_DIMENSION).contains(&self.height)
            || !(MIN_TIMEOUT_FRAMES..=MAX_TIMEOUT_FRAMES).contains(&self.timeout_frames)
            || self.stable_frames == 0
            || self.stable_frames >= self.timeout_frames
            || self.document_path == self.screenshot_path
            || self.document_path == self.result_path
            || self.screenshot_path == self.result_path
        {
            return Err(setup_error(
                UiDocumentStandalonePreviewFailureKind::ConfigurationInvalid,
                "UI_DOCUMENT_PREVIEW_CONFIGURATION_INVALID",
                "preview paths, dimensions, timeout, or stable-frame budget are invalid",
            ));
        }
        for output in [&self.screenshot_path, &self.result_path] {
            if output.exists()
                || output
                    .parent()
                    .is_none_or(|parent| parent.as_os_str().is_empty())
            {
                return Err(setup_error(
                    UiDocumentStandalonePreviewFailureKind::OutputConflict,
                    "UI_DOCUMENT_PREVIEW_OUTPUT_CONFLICT",
                    "preview outputs must be new files with explicit parent directories",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiDocumentStandalonePreviewFailureKind {
    ConfigurationInvalid,
    DocumentUnreadable,
    DocumentInvalid,
    ResourceMissing,
    ResourceFailed,
    ResourceTimeout,
    RuntimeFailed,
    ScreenshotFailed,
    ScreenshotTimeout,
    OutputConflict,
    OutputWriteFailed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UiDocumentStandalonePreviewError {
    pub kind: UiDocumentStandalonePreviewFailureKind,
    pub code: String,
    pub detail: String,
}

impl std::fmt::Display for UiDocumentStandalonePreviewError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.detail)
    }
}

impl std::error::Error for UiDocumentStandalonePreviewError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum PreviewStatus {
    Passed,
    Failed,
}

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct PreviewResult {
    protocol_version: u32,
    status: PreviewStatus,
    document_id: String,
    canonical_document_sha256: String,
    width: u32,
    height: u32,
    elapsed_frames: u32,
    stable_frames: u32,
    screenshot_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    captured_size: Option<(u32, u32)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    failure: Option<UiDocumentStandalonePreviewError>,
}

#[derive(Resource)]
struct PreviewConfig {
    options: UiDocumentStandalonePreviewOptions,
    source_json: String,
    validated: ValidatedUiDocument,
    document_id: UiDocumentId,
    canonical_document_sha256: String,
}

#[derive(Default, Resource)]
struct PreviewDriver {
    elapsed_frames: u32,
    stable_frames: u32,
    screenshot_requested: bool,
    terminal: bool,
    request_id: Option<UiDocumentRequestId>,
}

#[derive(Default, Resource)]
struct PreviewAssetHandles(Vec<(String, UntypedHandle)>);

pub fn run_ui_document_standalone_preview(
    options: UiDocumentStandalonePreviewOptions,
) -> Result<(), UiDocumentStandalonePreviewError> {
    options.validate()?;
    let (source_json, validated, document_id, canonical_document_sha256) =
        load_preview_document(&options.document_path)?;
    validate_packaged_resources(validated.document())?;

    let asset_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                file_path: asset_root.to_string_lossy().into_owned(),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "UiDocument standalone preview".to_owned(),
                    resolution: WindowResolution::new(options.width, options.height),
                    resizable: false,
                    ..default()
                }),
                ..default()
            }),
    )
    .add_plugins(UiFrameworkPlugin)
    .insert_resource(ClearColor(Color::srgb_u8(10, 15, 18)))
    .insert_resource(PreviewConfig {
        options,
        source_json,
        validated,
        document_id,
        canonical_document_sha256,
    })
    .init_resource::<PreviewDriver>()
    .init_resource::<PreviewAssetHandles>()
    .add_systems(Startup, setup_preview)
    .add_systems(
        Update,
        drive_preview.after(UiDocumentRuntimeSystems::Reconcile),
    );
    app.run();
    Ok(())
}

fn load_preview_document(
    path: &Path,
) -> Result<(String, ValidatedUiDocument, UiDocumentId, String), UiDocumentStandalonePreviewError> {
    let metadata = fs::metadata(path).map_err(|_| {
        setup_error(
            UiDocumentStandalonePreviewFailureKind::DocumentUnreadable,
            "UI_DOCUMENT_PREVIEW_DOCUMENT_UNREADABLE",
            "preview document metadata is unavailable",
        )
    })?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > UI_DOCUMENT_MAX_BYTES as u64 {
        return Err(setup_error(
            UiDocumentStandalonePreviewFailureKind::DocumentUnreadable,
            "UI_DOCUMENT_PREVIEW_DOCUMENT_SIZE_INVALID",
            "preview document must be a bounded nonempty regular file",
        ));
    }
    let source_json = fs::read_to_string(path).map_err(|_| {
        setup_error(
            UiDocumentStandalonePreviewFailureKind::DocumentUnreadable,
            "UI_DOCUMENT_PREVIEW_DOCUMENT_UNREADABLE",
            "preview document is not readable UTF-8 JSON",
        )
    })?;
    let validated = UiDocument::parse_and_validate_json(&source_json).map_err(|error| {
        setup_error(
            UiDocumentStandalonePreviewFailureKind::DocumentInvalid,
            error.code(),
            "preview document failed formal validation",
        )
    })?;
    let document_id = validated.document().document_id.clone();
    let canonical = validated
        .document()
        .to_canonical_json_pretty()
        .map_err(|_| {
            setup_error(
                UiDocumentStandalonePreviewFailureKind::DocumentInvalid,
                "UI_DOCUMENT_PREVIEW_CANONICALIZATION_FAILED",
                "preview document could not be canonicalized",
            )
        })?;
    let canonical_document_sha256 = format!("{:x}", Sha256::digest(canonical.as_bytes()));
    Ok((
        source_json,
        validated,
        document_id,
        canonical_document_sha256,
    ))
}

fn validate_packaged_resources(
    document: &UiDocument,
) -> Result<(), UiDocumentStandalonePreviewError> {
    let project_root = fs::canonicalize(env!("CARGO_MANIFEST_DIR")).map_err(|_| {
        setup_error(
            UiDocumentStandalonePreviewFailureKind::ResourceMissing,
            "UI_DOCUMENT_PREVIEW_PROJECT_ROOT_UNAVAILABLE",
            "project root cannot be resolved",
        )
    })?;
    let assets_root = fs::canonicalize(project_root.join("assets")).map_err(|_| {
        setup_error(
            UiDocumentStandalonePreviewFailureKind::ResourceMissing,
            "UI_DOCUMENT_PREVIEW_ASSET_ROOT_UNAVAILABLE",
            "project asset root cannot be resolved",
        )
    })?;
    for entry in document.assets.values() {
        let UiAssetSource::Packaged { path } = &entry.source else {
            if matches!(entry.source, UiAssetSource::ContentCache { .. }) {
                return Err(setup_error(
                    UiDocumentStandalonePreviewFailureKind::ResourceMissing,
                    "UI_DOCUMENT_PREVIEW_CONTENT_CACHE_UNAVAILABLE",
                    "standalone preview cannot resolve a content-cache asset",
                ));
            }
            continue;
        };
        let candidate = fs::canonicalize(assets_root.join(path)).map_err(|_| {
            setup_error(
                UiDocumentStandalonePreviewFailureKind::ResourceMissing,
                "UI_DOCUMENT_PREVIEW_RESOURCE_MISSING",
                "a declared packaged preview resource is missing",
            )
        })?;
        if !candidate.starts_with(&assets_root) || !candidate.is_file() || candidate == assets_root
        {
            return Err(setup_error(
                UiDocumentStandalonePreviewFailureKind::ResourceMissing,
                "UI_DOCUMENT_PREVIEW_RESOURCE_OUTSIDE_ASSETS",
                "a declared packaged preview resource is outside the project asset root",
            ));
        }
    }
    Ok(())
}

fn setup_preview(
    mut commands: Commands,
    config: Res<PreviewConfig>,
    asset_server: Res<AssetServer>,
    mut preview_commands: MessageWriter<UiDocumentPreviewCommand>,
) {
    commands.spawn((
        Camera2d,
        IsDefaultUiCamera,
        Name::new("UiDocumentPreviewCamera"),
    ));
    let mut handles = Vec::new();
    for (id, entry) in &config.validated.document().assets {
        let UiAssetSource::Packaged { path } = &entry.source else {
            continue;
        };
        let handle = match entry.kind {
            UiAssetKind::Image | UiAssetKind::Icon | UiAssetKind::Atlas => {
                asset_server.load::<Image>(path.clone()).untyped()
            }
            UiAssetKind::Font => asset_server.load::<Font>(path.clone()).untyped(),
            UiAssetKind::Material => continue,
        };
        handles.push((id.to_string(), handle));
    }
    commands.insert_resource(PreviewAssetHandles(handles));
    let target_profile = UiTargetProfile::new(
        config.options.width as f32,
        config.options.height as f32,
        UiSafeAreaClass::None,
        UiDocumentInputMode::MouseKeyboard,
        platform(),
    )
    .expect("validated preview dimensions are positive");
    preview_commands.write(UiDocumentPreviewCommand::Register(
        UiDocumentPreviewRegistration {
            document_id: config.document_id.clone(),
            owner: PREVIEW_OWNER.to_owned(),
            source_path: UiDocumentSourcePath::new(
                UiDocumentSourceRoot::Authoring,
                "stage8/standalone-preview.json",
            )
            .expect("static standalone preview source path is valid"),
            source_json: config.source_json.clone(),
            panel: UiDocumentPanel::Page,
            layer: UiDocumentLayer::Page,
            target_profile,
            page_state: UiPageState::initial(),
            owner_alive: true,
            host_bindings: default(),
            watch: false,
            open_on_register: true,
            audit_profiles: vec!["standalone".to_owned()],
        },
    ));
}

#[allow(clippy::too_many_arguments)]
fn drive_preview(
    mut driver: ResMut<PreviewDriver>,
    config: Res<PreviewConfig>,
    handles: Res<PreviewAssetHandles>,
    asset_server: Res<AssetServer>,
    runtime: Res<UiDocumentRuntime>,
    fonts: Query<&UiFontResolution>,
    mut reload_events: MessageReader<UiDocumentReloadEvent>,
    mut screenshot_events: MessageReader<UiScreenshotEvent>,
    mut screenshot_commands: MessageWriter<UiScreenshotCommand>,
    mut app_exit: MessageWriter<AppExit>,
) {
    if driver.terminal {
        return;
    }
    driver.elapsed_frames = driver.elapsed_frames.saturating_add(1);

    for event in reload_events.read() {
        if event.0.document_id != config.document_id || event.0.owner != PREVIEW_OWNER {
            continue;
        }
        if let Some(request_id) = event.0.request_id {
            driver.request_id = Some(request_id);
        }
        if event.0.status == UiDocumentReloadStatus::Failed {
            finish_preview(
                &mut driver,
                &config,
                PreviewStatus::Failed,
                None,
                Some(setup_error(
                    UiDocumentStandalonePreviewFailureKind::RuntimeFailed,
                    event
                        .0
                        .error
                        .as_ref()
                        .map(|error| error.code.as_str())
                        .unwrap_or("UI_DOCUMENT_PREVIEW_RELOAD_FAILED"),
                    "formal UiDocument preview registration failed",
                )),
                &mut app_exit,
            );
            return;
        }
    }

    for event in screenshot_events.read() {
        match event {
            UiScreenshotEvent::Saved(saved)
                if saved.request.path == config.options.screenshot_path =>
            {
                finish_preview(
                    &mut driver,
                    &config,
                    PreviewStatus::Passed,
                    Some(saved.captured_size),
                    None,
                    &mut app_exit,
                );
                return;
            }
            UiScreenshotEvent::Failed(failed)
                if failed.request.path == config.options.screenshot_path =>
            {
                finish_preview(
                    &mut driver,
                    &config,
                    PreviewStatus::Failed,
                    failed.captured_size,
                    Some(setup_error(
                        UiDocumentStandalonePreviewFailureKind::ScreenshotFailed,
                        screenshot_failure_code(&failed.reason),
                        "standalone preview screenshot failed",
                    )),
                    &mut app_exit,
                );
                return;
            }
            _ => {}
        }
    }

    let record = driver
        .request_id
        .and_then(|request_id| runtime.record(request_id));
    if let Some(record) = record
        && record.state == UiDocumentBuildState::Failed
    {
        finish_preview(
            &mut driver,
            &config,
            PreviewStatus::Failed,
            None,
            Some(setup_error(
                UiDocumentStandalonePreviewFailureKind::RuntimeFailed,
                record
                    .failure_code
                    .as_deref()
                    .unwrap_or("UI_DOCUMENT_PREVIEW_RUNTIME_FAILED"),
                "formal UiDocument runtime rejected the standalone preview",
            )),
            &mut app_exit,
        );
        return;
    }

    if !driver.screenshot_requested
        && record.is_some_and(|record| record.state == UiDocumentBuildState::Committed)
    {
        let mut resources_pending = false;
        for (_, handle) in &handles.0 {
            match asset_server.get_load_state(handle.id()) {
                Some(LoadState::Loaded) => {}
                Some(LoadState::Failed(_)) => {
                    finish_preview(
                        &mut driver,
                        &config,
                        PreviewStatus::Failed,
                        None,
                        Some(setup_error(
                            UiDocumentStandalonePreviewFailureKind::ResourceFailed,
                            "UI_DOCUMENT_PREVIEW_RESOURCE_LOAD_FAILED",
                            "a declared preview resource failed to load",
                        )),
                        &mut app_exit,
                    );
                    return;
                }
                _ => resources_pending = true,
            }
        }
        for resolution in &fonts {
            match &resolution.status {
                UiFontResolutionStatus::Loading { .. }
                | UiFontResolutionStatus::GlyphReplacement { loading: true, .. } => {
                    resources_pending = true;
                }
                UiFontResolutionStatus::InvalidStyle(_) | UiFontResolutionStatus::Unavailable => {
                    finish_preview(
                        &mut driver,
                        &config,
                        PreviewStatus::Failed,
                        None,
                        Some(setup_error(
                            UiDocumentStandalonePreviewFailureKind::ResourceFailed,
                            "UI_DOCUMENT_PREVIEW_FONT_UNAVAILABLE",
                            "a preview text node has no usable font resource",
                        )),
                        &mut app_exit,
                    );
                    return;
                }
                _ => {}
            }
        }
        if resources_pending {
            driver.stable_frames = 0;
        } else {
            driver.stable_frames = driver.stable_frames.saturating_add(1);
            if driver.stable_frames >= config.options.stable_frames {
                screenshot_commands.write(UiScreenshotCommand::Capture {
                    path: config.options.screenshot_path.clone(),
                    label: "ui_document_standalone_preview".to_owned(),
                });
                driver.screenshot_requested = true;
            }
        }
    }

    if driver.elapsed_frames >= config.options.timeout_frames {
        let (kind, code, detail) = if driver.screenshot_requested {
            (
                UiDocumentStandalonePreviewFailureKind::ScreenshotTimeout,
                "UI_DOCUMENT_PREVIEW_SCREENSHOT_TIMEOUT",
                "standalone screenshot did not complete within the frame budget",
            )
        } else {
            (
                UiDocumentStandalonePreviewFailureKind::ResourceTimeout,
                "UI_DOCUMENT_PREVIEW_RESOURCE_TIMEOUT",
                "runtime, fonts, or images did not become ready within the frame budget",
            )
        };
        finish_preview(
            &mut driver,
            &config,
            PreviewStatus::Failed,
            None,
            Some(setup_error(kind, code, detail)),
            &mut app_exit,
        );
    }
}

fn finish_preview(
    driver: &mut PreviewDriver,
    config: &PreviewConfig,
    status: PreviewStatus,
    captured_size: Option<(u32, u32)>,
    failure: Option<UiDocumentStandalonePreviewError>,
    app_exit: &mut MessageWriter<AppExit>,
) {
    driver.terminal = true;
    let result = PreviewResult {
        protocol_version: 1,
        status,
        document_id: config.document_id.to_string(),
        canonical_document_sha256: config.canonical_document_sha256.clone(),
        width: config.options.width,
        height: config.options.height,
        elapsed_frames: driver.elapsed_frames,
        stable_frames: driver.stable_frames,
        screenshot_path: config
            .options
            .screenshot_path
            .to_string_lossy()
            .into_owned(),
        captured_size,
        failure,
    };
    if let Err(error) = write_result_no_clobber(&config.options.result_path, &result) {
        error!("standalone UiDocument preview result write failed: {error}");
    }
    app_exit.write(AppExit::Success);
}

fn write_result_no_clobber(
    path: &Path,
    result: &PreviewResult,
) -> Result<(), UiDocumentStandalonePreviewError> {
    let parent = path.parent().ok_or_else(|| {
        setup_error(
            UiDocumentStandalonePreviewFailureKind::OutputWriteFailed,
            "UI_DOCUMENT_PREVIEW_RESULT_PARENT_INVALID",
            "preview result path has no parent",
        )
    })?;
    fs::create_dir_all(parent).map_err(|_| {
        setup_error(
            UiDocumentStandalonePreviewFailureKind::OutputWriteFailed,
            "UI_DOCUMENT_PREVIEW_RESULT_DIRECTORY_FAILED",
            "preview result directory could not be created",
        )
    })?;
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|_| {
            setup_error(
                UiDocumentStandalonePreviewFailureKind::OutputConflict,
                "UI_DOCUMENT_PREVIEW_RESULT_CREATE_FAILED",
                "preview result could not be created without overwrite",
            )
        })?;
    let bytes = serde_json::to_vec_pretty(result).map_err(|_| {
        setup_error(
            UiDocumentStandalonePreviewFailureKind::OutputWriteFailed,
            "UI_DOCUMENT_PREVIEW_RESULT_SERIALIZE_FAILED",
            "preview result could not be serialized",
        )
    })?;
    file.write_all(&bytes)
        .and_then(|_| file.write_all(b"\n"))
        .map_err(|_| {
            setup_error(
                UiDocumentStandalonePreviewFailureKind::OutputWriteFailed,
                "UI_DOCUMENT_PREVIEW_RESULT_WRITE_FAILED",
                "preview result write did not complete",
            )
        })?;
    file.sync_all().map_err(|_| {
        setup_error(
            UiDocumentStandalonePreviewFailureKind::OutputWriteFailed,
            "UI_DOCUMENT_PREVIEW_RESULT_SYNC_FAILED",
            "preview result could not be flushed",
        )
    })
}

fn screenshot_failure_code(reason: &UiScreenshotFailureReason) -> &'static str {
    match reason {
        UiScreenshotFailureReason::PathAlreadyExists => "UI_DOCUMENT_PREVIEW_SCREENSHOT_CONFLICT",
        UiScreenshotFailureReason::CaptureTimedOut { .. } => {
            "UI_DOCUMENT_PREVIEW_SCREENSHOT_TIMEOUT"
        }
        _ => "UI_DOCUMENT_PREVIEW_SCREENSHOT_FAILED",
    }
}

fn platform() -> UiDocumentPlatform {
    if cfg!(target_os = "macos") {
        UiDocumentPlatform::Macos
    } else if cfg!(target_os = "linux") {
        UiDocumentPlatform::Linux
    } else {
        UiDocumentPlatform::Windows
    }
}

fn parse_u32(
    value: std::ffi::OsString,
    name: &str,
) -> Result<u32, UiDocumentStandalonePreviewError> {
    value
        .to_str()
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| {
            setup_error(
                UiDocumentStandalonePreviewFailureKind::ConfigurationInvalid,
                "UI_DOCUMENT_PREVIEW_NUMBER_INVALID",
                &format!("preview {name} must be an unsigned integer"),
            )
        })
}

fn required_path(
    value: Option<PathBuf>,
    name: &str,
) -> Result<PathBuf, UiDocumentStandalonePreviewError> {
    value.ok_or_else(|| {
        setup_error(
            UiDocumentStandalonePreviewFailureKind::ConfigurationInvalid,
            "UI_DOCUMENT_PREVIEW_PATH_MISSING",
            &format!("preview {name} path is required"),
        )
    })
}

fn setup_error(
    kind: UiDocumentStandalonePreviewFailureKind,
    code: impl Into<String>,
    detail: impl Into<String>,
) -> UiDocumentStandalonePreviewError {
    UiDocumentStandalonePreviewError {
        kind,
        code: code.into(),
        detail: detail.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn standalone_preview_args_are_closed_and_bounded() {
        let arguments = [
            "--document",
            "input/page.json",
            "--screenshot",
            "out/page.png",
            "--result",
            "out/result.json",
            "--width",
            "390",
            "--height",
            "844",
            "--timeout-frames",
            "600",
            "--stable-frames",
            "30",
        ]
        .into_iter()
        .map(OsString::from);
        let options = UiDocumentStandalonePreviewOptions::parse_args(arguments).unwrap();
        assert_eq!(options.width, 390);
        assert_eq!(options.height, 844);

        let unknown = ["--unknown", "value"].into_iter().map(OsString::from);
        assert_eq!(
            UiDocumentStandalonePreviewOptions::parse_args(unknown)
                .unwrap_err()
                .code,
            "UI_DOCUMENT_PREVIEW_ARGUMENT_UNKNOWN"
        );
    }

    #[test]
    fn missing_packaged_resource_is_a_stable_failure() {
        let source = r#"{
          "schema_version": 1,
          "document_id": "preview.missing_resource",
          "assets": {
            "missing": {
              "kind": "image",
              "source": {"kind": "packaged", "path": "ui/fixtures/missing.png"}
            }
          },
          "root": {
            "type": "image",
            "id": "preview.image",
            "asset": "missing",
            "layout": {"width": {"px": 64}, "height": {"px": 64}}
          }
        }"#;
        let validated = UiDocument::parse_and_validate_json(source).unwrap();
        let error = validate_packaged_resources(validated.document()).unwrap_err();
        assert_eq!(
            error.kind,
            UiDocumentStandalonePreviewFailureKind::ResourceMissing
        );
        assert_eq!(error.code, "UI_DOCUMENT_PREVIEW_RESOURCE_MISSING");
    }

    #[test]
    fn result_writer_never_clobbers_existing_evidence() {
        let directory = env::temp_dir().join(format!(
            "mybevy-standalone-preview-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&directory).unwrap();
        let path = directory.join("result.json");
        let result = PreviewResult {
            protocol_version: 1,
            status: PreviewStatus::Passed,
            document_id: "preview.test".to_owned(),
            canonical_document_sha256: "a".repeat(64),
            width: 390,
            height: 844,
            elapsed_frames: 60,
            stable_frames: 30,
            screenshot_path: "preview.png".to_owned(),
            captured_size: Some((390, 844)),
            failure: None,
        };
        write_result_no_clobber(&path, &result).unwrap();
        let original = fs::read(&path).unwrap();
        assert!(write_result_no_clobber(&path, &result).is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
        fs::remove_file(path).unwrap();
        fs::remove_dir(directory).unwrap();
    }
}
