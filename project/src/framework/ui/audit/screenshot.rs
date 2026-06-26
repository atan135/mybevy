use std::{
    env, fmt, fs,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use bevy::{
    image::ImageFormat,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured},
    window::PrimaryWindow,
};

use crate::framework::ui::core::{UiCurrentOwner, UiPanelSystems};

const ENV_MANUAL_SCREENSHOT: &str = "MYBEVY_UI_AUDIT_MANUAL_SCREENSHOT";
const ENV_MANUAL_SCREENSHOT_OUTPUT: &str = "MYBEVY_UI_AUDIT_MANUAL_OUTPUT";
const DEFAULT_MANUAL_SCREENSHOT_DIR: &str = "../summary/ui-audit/manual";
const DEFAULT_SCREEN_LABEL: &str = "unknown_screen";
const MANUAL_SCREENSHOT_TIMEOUT_FRAMES: u32 = 300;

pub(crate) struct UiAuditPlugin;

impl Plugin for UiAuditPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(UiAuditScreenshotConfig::from_env())
            .init_resource::<UiManualScreenshotPending>()
            .add_systems(
                Update,
                (
                    request_manual_screenshot.after(UiPanelSystems::Commands),
                    expire_pending_manual_screenshot,
                )
                    .chain()
                    .run_if(manual_screenshot_enabled),
            );
    }
}

#[derive(Clone, Debug, Resource, PartialEq, Eq)]
pub(crate) struct UiAuditScreenshotConfig {
    manual_enabled: bool,
    manual_output_dir: PathBuf,
}

impl Default for UiAuditScreenshotConfig {
    fn default() -> Self {
        Self {
            manual_enabled: default_manual_screenshot_enabled(),
            manual_output_dir: PathBuf::from(DEFAULT_MANUAL_SCREENSHOT_DIR),
        }
    }
}

impl UiAuditScreenshotConfig {
    pub(crate) fn from_env() -> Self {
        Self::from_env_reader(|key| env::var(key).ok())
    }

    fn from_env_reader(mut read: impl FnMut(&str) -> Option<String>) -> Self {
        let defaults = Self::default();
        let manual_enabled =
            read_bool(&mut read, ENV_MANUAL_SCREENSHOT).unwrap_or(defaults.manual_enabled);
        let manual_output_dir = read(ENV_MANUAL_SCREENSHOT_OUTPUT)
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or(defaults.manual_output_dir);

        Self {
            manual_enabled,
            manual_output_dir,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct UiManualScreenshotRequest {
    path: PathBuf,
    display_path: PathBuf,
    screen_label: String,
    logical_size: (u32, u32),
    physical_size: (u32, u32),
}

#[derive(Clone, Debug, Default, Resource, PartialEq, Eq)]
struct UiManualScreenshotPending {
    active: Option<PendingManualScreenshot>,
}

impl UiManualScreenshotPending {
    fn start(&mut self, entity: Entity, request: UiManualScreenshotRequest) {
        self.active = Some(PendingManualScreenshot {
            entity,
            request,
            waited_frames: 0,
            timeout_frames: MANUAL_SCREENSHOT_TIMEOUT_FRAMES,
        });
    }

    fn clear_if_entity(&mut self, entity: Entity) {
        if self
            .active
            .as_ref()
            .is_some_and(|active| active.entity == entity)
        {
            self.active = None;
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PendingManualScreenshot {
    entity: Entity,
    request: UiManualScreenshotRequest,
    waited_frames: u32,
    timeout_frames: u32,
}

fn manual_screenshot_enabled(config: Res<UiAuditScreenshotConfig>) -> bool {
    config.manual_enabled
}

fn request_manual_screenshot(
    mut commands: Commands,
    key_codes: Res<ButtonInput<KeyCode>>,
    config: Res<UiAuditScreenshotConfig>,
    current_owner: Res<UiCurrentOwner>,
    primary_window: Query<&Window, With<PrimaryWindow>>,
    active_screenshots: Query<Entity, With<Screenshot>>,
    mut pending: ResMut<UiManualScreenshotPending>,
) {
    if !key_codes.just_pressed(KeyCode::F9) {
        return;
    }

    if let Some(active) = pending.active.as_ref() {
        error!(
            "ui audit manual screenshot failed before capture: previous request still pending: path={}, screen={}, waited_frames={}",
            active.request.display_path.display(),
            active.request.screen_label,
            active.waited_frames
        );
        return;
    }

    if let Some(active) = active_screenshots.iter().next() {
        error!(
            "ui audit manual screenshot failed before capture: screenshot already in progress on entity {active}"
        );
        return;
    }

    let Ok(window) = primary_window.single() else {
        error!("ui audit manual screenshot failed before capture: primary window is unavailable");
        return;
    };

    let screen_label = current_owner
        .owner
        .map(|owner| owner.as_str())
        .unwrap_or(DEFAULT_SCREEN_LABEL);
    let request = build_manual_screenshot_request(
        &config,
        screen_label,
        WindowCaptureSize::from_window(window),
        current_unix_timestamp_seconds(),
    );

    match ensure_parent_dir(&request.path) {
        Ok(()) => {
            info!(
                "ui audit manual screenshot requested: path={}, screen={}, logical={}x{}, physical={}x{}",
                request.display_path.display(),
                request.screen_label,
                request.logical_size.0,
                request.logical_size.1,
                request.physical_size.0,
                request.physical_size.1
            );
            let entity = spawn_primary_window_screenshot(&mut commands, request.clone());
            pending.start(entity, request);
        }
        Err(error) => {
            error!(
                "ui audit manual screenshot failed before capture: path={}, error={error}",
                request.display_path.display()
            );
        }
    }
}

fn spawn_primary_window_screenshot(
    commands: &mut Commands,
    request: UiManualScreenshotRequest,
) -> Entity {
    commands
        .spawn(Screenshot::primary_window())
        .observe(
            move |captured: On<ScreenshotCaptured>,
                  mut pending: ResMut<UiManualScreenshotPending>| {
                save_manual_screenshot(captured.image.clone(), &request);
                pending.clear_if_entity(captured.entity);
            },
        )
        .id()
}

fn expire_pending_manual_screenshot(
    mut commands: Commands,
    mut pending: ResMut<UiManualScreenshotPending>,
) {
    let Some(active) = pending.active.as_mut() else {
        return;
    };

    active.waited_frames = next_pending_waited_frames(active.waited_frames);
    if !pending_manual_screenshot_timed_out(active.waited_frames, active.timeout_frames) {
        return;
    }

    let expired = pending
        .active
        .take()
        .expect("pending screenshot should exist");
    error!(
        "ui audit manual screenshot failed: screenshot capture timed out: path={}, screen={}, waited_frames={}, timeout_frames={}",
        expired.request.display_path.display(),
        expired.request.screen_label,
        expired.waited_frames,
        expired.timeout_frames
    );
    commands.entity(expired.entity).try_despawn();
}

fn save_manual_screenshot(image: Image, request: &UiManualScreenshotRequest) {
    let captured_size = (image.width(), image.height());
    let path = &request.path;

    let result = image
        .try_into_dynamic()
        .map_err(|error| UiAuditScreenshotSaveError::ImageConversion(error.to_string()))
        .and_then(|dynamic_image| {
            ImageFormat::Png
                .as_image_crate_format()
                .ok_or_else(|| UiAuditScreenshotSaveError::ImageFormat("png is unavailable".into()))
                .map(|format| (dynamic_image, format))
        })
        .and_then(|(dynamic_image, format)| {
            dynamic_image
                .to_rgb8()
                .save_with_format(path, format)
                .map_err(|error| UiAuditScreenshotSaveError::Io(error.to_string()))
        });

    match result {
        Ok(()) => info!(
            "ui audit manual screenshot saved: path={}, captured={}x{}, requested logical={}x{}, requested physical={}x{}",
            request.display_path.display(),
            captured_size.0,
            captured_size.1,
            request.logical_size.0,
            request.logical_size.1,
            request.physical_size.0,
            request.physical_size.1
        ),
        Err(error) => error!(
            "ui audit manual screenshot failed: path={}, captured={}x{}, error={error}",
            request.display_path.display(),
            captured_size.0,
            captured_size.1
        ),
    }
}

fn build_manual_screenshot_request(
    config: &UiAuditScreenshotConfig,
    screen_label: &str,
    window_size: WindowCaptureSize,
    timestamp_seconds: u64,
) -> UiManualScreenshotRequest {
    let screen_label = sanitize_filename_segment(screen_label);
    let timestamp = format_unix_timestamp_compact(timestamp_seconds);
    let filename = format!(
        "{timestamp}_{screen_label}_logical-{}x{}_physical-{}x{}.png",
        window_size.logical_width,
        window_size.logical_height,
        window_size.physical_width,
        window_size.physical_height
    );
    let path = config.manual_output_dir.join(filename);

    UiManualScreenshotRequest {
        display_path: absolute_display_path(&path),
        path,
        screen_label,
        logical_size: (window_size.logical_width, window_size.logical_height),
        physical_size: (window_size.physical_width, window_size.physical_height),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WindowCaptureSize {
    logical_width: u32,
    logical_height: u32,
    physical_width: u32,
    physical_height: u32,
}

impl WindowCaptureSize {
    fn from_window(window: &Window) -> Self {
        Self {
            logical_width: rounded_dimension(window.resolution.width()),
            logical_height: rounded_dimension(window.resolution.height()),
            physical_width: window.resolution.physical_width(),
            physical_height: window.resolution.physical_height(),
        }
    }
}

fn rounded_dimension(value: f32) -> u32 {
    value.round().max(0.0) as u32
}

fn ensure_parent_dir(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    Ok(())
}

fn absolute_display_path(path: &Path) -> PathBuf {
    let base = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    absolute_display_path_from_base(&base, path)
}

fn absolute_display_path_from_base(base: &Path, path: &Path) -> PathBuf {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    };
    normalize_path_lexically(&path)
}

fn normalize_path_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() && !path.is_absolute() {
                    normalized.push(component.as_os_str());
                }
            }
        }
    }

    normalized
}

fn sanitize_filename_segment(value: &str) -> String {
    let mut sanitized = String::with_capacity(value.len());
    let mut last_was_separator = false;

    for character in value.trim().chars() {
        let is_allowed = character.is_ascii_alphanumeric() || matches!(character, '-' | '_');
        let next = if is_allowed {
            character.to_ascii_lowercase()
        } else {
            '_'
        };

        if next == '_' {
            if last_was_separator {
                continue;
            }
            last_was_separator = true;
        } else {
            last_was_separator = false;
        }

        sanitized.push(next);
    }

    let sanitized = sanitized.trim_matches('_');
    if sanitized.is_empty() {
        DEFAULT_SCREEN_LABEL.to_owned()
    } else {
        sanitized.to_owned()
    }
}

fn current_unix_timestamp_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn format_unix_timestamp_compact(seconds_since_epoch: u64) -> String {
    seconds_since_epoch.to_string()
}

fn default_manual_screenshot_enabled() -> bool {
    cfg!(all(
        debug_assertions,
        not(target_os = "android"),
        not(target_arch = "wasm32")
    ))
}

fn read_bool(read: &mut impl FnMut(&str) -> Option<String>, key: &str) -> Option<bool> {
    read(key).and_then(|value| match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "on" | "yes" | "enabled" => Some(true),
        "0" | "false" | "off" | "no" | "disabled" => Some(false),
        _ => None,
    })
}

fn next_pending_waited_frames(waited_frames: u32) -> u32 {
    waited_frames.saturating_add(1)
}

fn pending_manual_screenshot_timed_out(waited_frames: u32, timeout_frames: u32) -> bool {
    waited_frames > timeout_frames
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum UiAuditScreenshotSaveError {
    ImageConversion(String),
    ImageFormat(String),
    Io(String),
}

impl fmt::Display for UiAuditScreenshotSaveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ImageConversion(error) => write!(formatter, "image conversion error: {error}"),
            Self::ImageFormat(error) => write!(formatter, "image format error: {error}"),
            Self::Io(error) => write!(formatter, "io error: {error}"),
        }
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

    #[test]
    fn config_uses_default_manual_directory() {
        let config = UiAuditScreenshotConfig::from_env_reader(env_reader(&[]));

        assert_eq!(
            config.manual_output_dir,
            PathBuf::from(DEFAULT_MANUAL_SCREENSHOT_DIR)
        );
    }

    #[test]
    fn config_parses_manual_enabled_env_values() {
        for value in ["1", "true", "on", "yes", "enabled"] {
            let config = UiAuditScreenshotConfig::from_env_reader(env_reader(&[(
                ENV_MANUAL_SCREENSHOT,
                value,
            )]));
            assert!(
                config.manual_enabled,
                "{value} should enable manual screenshots"
            );
        }

        for value in ["0", "false", "off", "no", "disabled"] {
            let config = UiAuditScreenshotConfig::from_env_reader(env_reader(&[(
                ENV_MANUAL_SCREENSHOT,
                value,
            )]));
            assert!(
                !config.manual_enabled,
                "{value} should disable manual screenshots"
            );
        }
    }

    #[test]
    fn config_ignores_invalid_manual_enabled_env_value() {
        let config = UiAuditScreenshotConfig::from_env_reader(env_reader(&[(
            ENV_MANUAL_SCREENSHOT,
            "maybe",
        )]));

        assert_eq!(config.manual_enabled, default_manual_screenshot_enabled());
    }

    #[test]
    fn config_parses_manual_output_directory() {
        let config = UiAuditScreenshotConfig::from_env_reader(env_reader(&[(
            ENV_MANUAL_SCREENSHOT_OUTPUT,
            "../summary/ui-audit/custom",
        )]));

        assert_eq!(
            config.manual_output_dir,
            PathBuf::from("../summary/ui-audit/custom")
        );
    }

    #[test]
    fn sanitize_filename_segment_removes_path_and_symbol_characters() {
        assert_eq!(
            sanitize_filename_segment(" UI:Gallery / Detail*Page? "),
            "ui_gallery_detail_page"
        );
        assert_eq!(sanitize_filename_segment("../.."), DEFAULT_SCREEN_LABEL);
        assert_eq!(
            sanitize_filename_segment("fangyuan-home_01"),
            "fangyuan-home_01"
        );
    }

    #[test]
    fn manual_screenshot_filename_includes_timestamp_screen_and_sizes() {
        let config = UiAuditScreenshotConfig {
            manual_enabled: true,
            manual_output_dir: PathBuf::from("../summary/ui-audit/manual"),
        };
        let request = build_manual_screenshot_request(
            &config,
            "ui/gallery",
            WindowCaptureSize {
                logical_width: 360,
                logical_height: 800,
                physical_width: 720,
                physical_height: 1600,
            },
            1_782_400_000,
        );

        assert_eq!(
            request.path,
            PathBuf::from(
                "../summary/ui-audit/manual/1782400000_ui_gallery_logical-360x800_physical-720x1600.png"
            )
        );
    }

    #[test]
    fn absolute_display_path_resolves_relative_segments() {
        let display_path = absolute_display_path_from_base(
            Path::new("C:/project/mybevy/project"),
            Path::new("../summary/ui-audit/manual/shot.png"),
        );

        assert_eq!(
            display_path,
            PathBuf::from("C:/project/mybevy/summary/ui-audit/manual/shot.png")
        );
    }

    #[test]
    fn manual_screenshot_request_records_absolute_display_path() {
        let config = UiAuditScreenshotConfig {
            manual_enabled: true,
            manual_output_dir: PathBuf::from("../summary/ui-audit/manual"),
        };
        let request = build_manual_screenshot_request(
            &config,
            "ui_gallery",
            WindowCaptureSize {
                logical_width: 360,
                logical_height: 800,
                physical_width: 360,
                physical_height: 800,
            },
            1_782_400_000,
        );

        assert!(request.display_path.is_absolute());
        assert!(request.display_path.ends_with(
            "summary/ui-audit/manual/1782400000_ui_gallery_logical-360x800_physical-360x800.png"
        ));
    }

    #[test]
    fn pending_timeout_requires_waiting_past_limit() {
        assert!(!pending_manual_screenshot_timed_out(300, 300));
        assert!(pending_manual_screenshot_timed_out(301, 300));
        assert_eq!(next_pending_waited_frames(u32::MAX), u32::MAX);
    }

    #[test]
    fn pending_state_clears_matching_entity_only() {
        let mut pending = UiManualScreenshotPending::default();
        let request = UiManualScreenshotRequest {
            path: PathBuf::from("../summary/ui-audit/manual/shot.png"),
            display_path: PathBuf::from("C:/project/mybevy/summary/ui-audit/manual/shot.png"),
            screen_label: "ui_gallery".to_owned(),
            logical_size: (360, 800),
            physical_size: (360, 800),
        };
        let entity = Entity::from_raw_u32(7).unwrap();
        let other_entity = Entity::from_raw_u32(8).unwrap();

        pending.start(entity, request);
        pending.clear_if_entity(other_entity);
        assert!(pending.active.is_some());

        pending.clear_if_entity(entity);
        assert!(pending.active.is_none());
    }
}
