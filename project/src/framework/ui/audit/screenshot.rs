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
const SCREENSHOT_TIMEOUT_FRAMES: u32 = 300;

pub(crate) struct UiScreenshotPlugin;

impl Plugin for UiScreenshotPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(UiAuditScreenshotConfig::from_env())
            .init_resource::<UiScreenshotFrameClock>()
            .init_resource::<UiScreenshotRequestIds>()
            .init_resource::<UiScreenshotPending>()
            .add_message::<UiScreenshotCommand>()
            .add_message::<UiScreenshotEvent>()
            .add_systems(First, advance_ui_screenshot_frame)
            .configure_sets(
                Update,
                (
                    UiScreenshotSystems::ManualInput,
                    UiScreenshotSystems::Commands,
                    UiScreenshotSystems::Timeout,
                )
                    .chain(),
            )
            .configure_sets(
                Update,
                UiScreenshotSystems::ManualInput.after(UiPanelSystems::Commands),
            )
            .add_systems(
                Update,
                request_manual_screenshot
                    .run_if(manual_screenshot_enabled)
                    .in_set(UiScreenshotSystems::ManualInput),
            )
            .add_systems(
                Update,
                handle_ui_screenshot_commands.in_set(UiScreenshotSystems::Commands),
            )
            .add_systems(
                Update,
                expire_pending_ui_screenshot.in_set(UiScreenshotSystems::Timeout),
            );
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
pub(super) enum UiScreenshotSystems {
    ManualInput,
    Commands,
    Timeout,
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

#[derive(Clone, Debug, Message, PartialEq, Eq)]
pub(crate) enum UiScreenshotCommand {
    Capture { path: PathBuf, label: String },
}

#[derive(Clone, Debug, Message, PartialEq, Eq)]
pub(crate) enum UiScreenshotEvent {
    Saved(UiScreenshotSaved),
    Failed(UiScreenshotFailed),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UiScreenshotSaved {
    pub request: UiScreenshotRequestRecord,
    pub captured_size: (u32, u32),
    pub completion_frame: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UiScreenshotFailed {
    pub request: UiScreenshotRequestRecord,
    pub captured_size: Option<(u32, u32)>,
    pub completion_frame: u64,
    pub reason: UiScreenshotFailureReason,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UiScreenshotRequestRecord {
    pub request_id: UiScreenshotRequestId,
    pub label: String,
    pub path: PathBuf,
    pub display_path: PathBuf,
    pub target_window: Option<Entity>,
    pub logical_size: Option<(u32, u32)>,
    pub physical_size: Option<(u32, u32)>,
    pub request_frame: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct UiScreenshotRequestId(u64);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum UiScreenshotFailureReason {
    AlreadyPending {
        active_request_id: UiScreenshotRequestId,
        active_path: PathBuf,
    },
    CaptureInProgress {
        entity: Entity,
    },
    PrimaryWindowUnavailable,
    InvalidPath {
        error: String,
    },
    PathStatusUnavailable {
        error: String,
    },
    PathAlreadyExists,
    DirectoryCreateFailed {
        error: String,
    },
    SaveFailed {
        error: String,
    },
    CaptureTimedOut {
        waited_frames: u32,
        timeout_frames: u32,
    },
}

impl fmt::Display for UiScreenshotFailureReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyPending {
                active_request_id,
                active_path,
            } => write!(
                formatter,
                "request already pending: active_id={}, active_path={}",
                active_request_id,
                active_path.display()
            ),
            Self::CaptureInProgress { entity } => {
                write!(
                    formatter,
                    "screenshot capture already in progress on {entity}"
                )
            }
            Self::PrimaryWindowUnavailable => formatter.write_str("primary window is unavailable"),
            Self::InvalidPath { error } => write!(formatter, "invalid path: {error}"),
            Self::PathStatusUnavailable { error } => {
                write!(formatter, "path status unavailable: {error}")
            }
            Self::PathAlreadyExists => formatter.write_str("target path already exists"),
            Self::DirectoryCreateFailed { error } => {
                write!(formatter, "directory create failed: {error}")
            }
            Self::SaveFailed { error } => write!(formatter, "save failed: {error}"),
            Self::CaptureTimedOut {
                waited_frames,
                timeout_frames,
            } => write!(
                formatter,
                "screenshot capture timed out: waited_frames={waited_frames}, timeout_frames={timeout_frames}"
            ),
        }
    }
}

impl fmt::Display for UiScreenshotRequestId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

#[derive(Clone, Debug, Default, Resource, PartialEq, Eq)]
struct UiScreenshotFrameClock {
    frame: u64,
}

#[derive(Clone, Debug, Default, Resource, PartialEq, Eq)]
struct UiScreenshotRequestIds {
    next: u64,
}

impl UiScreenshotRequestIds {
    fn next(&mut self) -> UiScreenshotRequestId {
        self.next = self.next.saturating_add(1);
        UiScreenshotRequestId(self.next)
    }
}

#[derive(Clone, Debug, Default, Resource, PartialEq, Eq)]
struct UiScreenshotPending {
    active: Option<PendingUiScreenshot>,
}

impl UiScreenshotPending {
    fn start(&mut self, entity: Entity, request: UiScreenshotRequestRecord) {
        self.active = Some(PendingUiScreenshot {
            entity,
            request,
            waited_frames: 0,
            timeout_frames: SCREENSHOT_TIMEOUT_FRAMES,
        });
    }

    fn take_if_entity(&mut self, entity: Entity) -> Option<PendingUiScreenshot> {
        if self
            .active
            .as_ref()
            .is_some_and(|active| active.entity == entity)
        {
            self.active.take()
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PendingUiScreenshot {
    entity: Entity,
    request: UiScreenshotRequestRecord,
    waited_frames: u32,
    timeout_frames: u32,
}

fn manual_screenshot_enabled(config: Res<UiAuditScreenshotConfig>) -> bool {
    config.manual_enabled
}

fn advance_ui_screenshot_frame(mut clock: ResMut<UiScreenshotFrameClock>) {
    clock.frame = clock.frame.saturating_add(1);
}

fn request_manual_screenshot(
    key_codes: Res<ButtonInput<KeyCode>>,
    config: Res<UiAuditScreenshotConfig>,
    current_owner: Res<UiCurrentOwner>,
    primary_window: Query<&Window, With<PrimaryWindow>>,
    mut screenshot_commands: MessageWriter<UiScreenshotCommand>,
) {
    if !key_codes.just_pressed(KeyCode::F9) {
        return;
    }

    let label = current_owner
        .owner
        .map(|owner| owner.as_str())
        .unwrap_or(DEFAULT_SCREEN_LABEL);
    let label = sanitize_filename_segment(label);
    let window_size = primary_window
        .single()
        .ok()
        .map(WindowCaptureSize::from_window)
        .unwrap_or_default();
    let path = build_manual_screenshot_path(
        &config,
        &label,
        window_size,
        current_unix_timestamp_seconds(),
    );

    screenshot_commands.write(UiScreenshotCommand::Capture { path, label });
}

fn handle_ui_screenshot_commands(
    mut commands: Commands,
    mut screenshot_commands: MessageReader<UiScreenshotCommand>,
    mut screenshot_events: MessageWriter<UiScreenshotEvent>,
    mut request_ids: ResMut<UiScreenshotRequestIds>,
    clock: Res<UiScreenshotFrameClock>,
    primary_window: Query<(Entity, &Window), With<PrimaryWindow>>,
    active_screenshots: Query<Entity, With<Screenshot>>,
    mut pending: ResMut<UiScreenshotPending>,
) {
    for command in screenshot_commands.read() {
        let UiScreenshotCommand::Capture { path, label } = command;
        let mut request =
            build_screenshot_request_record(request_ids.next(), path.clone(), label, clock.frame);

        if let Some(active) = pending.active.as_ref() {
            fail_screenshot_request(
                &mut screenshot_events,
                request,
                None,
                clock.frame,
                UiScreenshotFailureReason::AlreadyPending {
                    active_request_id: active.request.request_id,
                    active_path: active.request.display_path.clone(),
                },
            );
            continue;
        }

        if let Some(active) = active_screenshots.iter().next() {
            fail_screenshot_request(
                &mut screenshot_events,
                request,
                None,
                clock.frame,
                UiScreenshotFailureReason::CaptureInProgress { entity: active },
            );
            continue;
        }

        let Ok((window_entity, window)) = primary_window.single() else {
            fail_screenshot_request(
                &mut screenshot_events,
                request,
                None,
                clock.frame,
                UiScreenshotFailureReason::PrimaryWindowUnavailable,
            );
            continue;
        };

        request = request.with_window(window_entity, WindowCaptureSize::from_window(window));

        if let Err(reason) = prepare_screenshot_path(&request.path) {
            fail_screenshot_request(&mut screenshot_events, request, None, clock.frame, reason);
            continue;
        }

        info!(
            "ui audit screenshot requested: id={}, label={}, path={}, target_window={:?}, request_frame={}",
            request.request_id,
            request.label,
            request.display_path.display(),
            request.target_window,
            request.request_frame
        );

        let entity = spawn_screenshot_capture(&mut commands, request.clone(), window_entity);
        pending.start(entity, request);
    }
}

fn spawn_screenshot_capture(
    commands: &mut Commands,
    request: UiScreenshotRequestRecord,
    target_window: Entity,
) -> Entity {
    commands
        .spawn(Screenshot::window(target_window))
        .observe(
            move |captured: On<ScreenshotCaptured>,
                  clock: Res<UiScreenshotFrameClock>,
                  mut pending: ResMut<UiScreenshotPending>,
                  mut screenshot_events: MessageWriter<UiScreenshotEvent>| {
                let Some(active) = pending.take_if_entity(captured.entity) else {
                    warn!(
                        "ui audit screenshot capture ignored after pending state cleared: id={}, path={}",
                        request.request_id,
                        request.display_path.display()
                    );
                    return;
                };

                save_captured_screenshot(
                    captured.image.clone(),
                    active.request,
                    clock.frame,
                    &mut screenshot_events,
                );
            },
        )
        .id()
}

fn expire_pending_ui_screenshot(
    mut commands: Commands,
    clock: Res<UiScreenshotFrameClock>,
    mut pending: ResMut<UiScreenshotPending>,
    mut screenshot_events: MessageWriter<UiScreenshotEvent>,
) {
    let Some(active) = pending.active.as_mut() else {
        return;
    };

    active.waited_frames = next_pending_waited_frames(active.waited_frames);
    if !pending_screenshot_timed_out(active.waited_frames, active.timeout_frames) {
        return;
    }

    let expired = pending
        .active
        .take()
        .expect("pending screenshot should exist");
    commands.entity(expired.entity).try_despawn();
    fail_screenshot_request(
        &mut screenshot_events,
        expired.request,
        None,
        clock.frame,
        UiScreenshotFailureReason::CaptureTimedOut {
            waited_frames: expired.waited_frames,
            timeout_frames: expired.timeout_frames,
        },
    );
}

fn save_captured_screenshot(
    image: Image,
    request: UiScreenshotRequestRecord,
    completion_frame: u64,
    screenshot_events: &mut MessageWriter<UiScreenshotEvent>,
) {
    let captured_size = (image.width(), image.height());

    match save_screenshot_image(image, &request) {
        Ok(()) => {
            info!(
                "ui audit screenshot saved: id={}, label={}, path={}, captured={}x{}, request_frame={}, completion_frame={}",
                request.request_id,
                request.label,
                request.display_path.display(),
                captured_size.0,
                captured_size.1,
                request.request_frame,
                completion_frame
            );
            screenshot_events.write(UiScreenshotEvent::Saved(UiScreenshotSaved {
                request,
                captured_size,
                completion_frame,
            }));
        }
        Err(reason) => {
            fail_screenshot_request(
                screenshot_events,
                request,
                Some(captured_size),
                completion_frame,
                reason,
            );
        }
    }
}

fn fail_screenshot_request(
    screenshot_events: &mut MessageWriter<UiScreenshotEvent>,
    request: UiScreenshotRequestRecord,
    captured_size: Option<(u32, u32)>,
    completion_frame: u64,
    reason: UiScreenshotFailureReason,
) {
    error!(
        "ui audit screenshot failed: id={}, label={}, path={}, request_frame={}, completion_frame={}, reason={}",
        request.request_id,
        request.label,
        request.display_path.display(),
        request.request_frame,
        completion_frame,
        reason
    );
    screenshot_events.write(UiScreenshotEvent::Failed(UiScreenshotFailed {
        request,
        captured_size,
        completion_frame,
        reason,
    }));
}

fn build_screenshot_request_record(
    request_id: UiScreenshotRequestId,
    path: PathBuf,
    label: &str,
    request_frame: u64,
) -> UiScreenshotRequestRecord {
    UiScreenshotRequestRecord {
        request_id,
        label: sanitize_filename_segment(label),
        display_path: absolute_display_path(&path),
        path,
        target_window: None,
        logical_size: None,
        physical_size: None,
        request_frame,
    }
}

impl UiScreenshotRequestRecord {
    fn with_window(mut self, target_window: Entity, window_size: WindowCaptureSize) -> Self {
        self.target_window = Some(target_window);
        self.logical_size = Some((window_size.logical_width, window_size.logical_height));
        self.physical_size = Some((window_size.physical_width, window_size.physical_height));
        self
    }
}

fn build_manual_screenshot_path(
    config: &UiAuditScreenshotConfig,
    screen_label: &str,
    window_size: WindowCaptureSize,
    timestamp_seconds: u64,
) -> PathBuf {
    let screen_label = sanitize_filename_segment(screen_label);
    let timestamp = format_unix_timestamp_compact(timestamp_seconds);
    let filename = format!(
        "{timestamp}_{screen_label}_logical-{}x{}_physical-{}x{}.png",
        window_size.logical_width,
        window_size.logical_height,
        window_size.physical_width,
        window_size.physical_height
    );

    config.manual_output_dir.join(filename)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
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

fn prepare_screenshot_path(path: &Path) -> Result<(), UiScreenshotFailureReason> {
    if path.as_os_str().is_empty() {
        return Err(UiScreenshotFailureReason::InvalidPath {
            error: "path is empty".to_owned(),
        });
    }

    match path.try_exists() {
        Ok(true) => return Err(UiScreenshotFailureReason::PathAlreadyExists),
        Ok(false) => {}
        Err(error) => {
            return Err(UiScreenshotFailureReason::PathStatusUnavailable {
                error: error.to_string(),
            });
        }
    }

    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| {
            UiScreenshotFailureReason::DirectoryCreateFailed {
                error: error.to_string(),
            }
        })?;
    }

    Ok(())
}

fn save_screenshot_image(
    image: Image,
    request: &UiScreenshotRequestRecord,
) -> Result<(), UiScreenshotFailureReason> {
    match request.path.try_exists() {
        Ok(true) => return Err(UiScreenshotFailureReason::PathAlreadyExists),
        Ok(false) => {}
        Err(error) => {
            return Err(UiScreenshotFailureReason::PathStatusUnavailable {
                error: error.to_string(),
            });
        }
    }

    image
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
                .save_with_format(&request.path, format)
                .map_err(|error| UiAuditScreenshotSaveError::Io(error.to_string()))
        })
        .map_err(|error| UiScreenshotFailureReason::SaveFailed {
            error: error.to_string(),
        })
}

pub(super) fn absolute_display_path(path: &Path) -> PathBuf {
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

pub(super) fn sanitize_filename_segment(value: &str) -> String {
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

pub(super) fn current_unix_timestamp_seconds() -> u64 {
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

pub(super) fn read_bool(read: &mut impl FnMut(&str) -> Option<String>, key: &str) -> Option<bool> {
    read(key).and_then(|value| match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "on" | "yes" | "enabled" => Some(true),
        "0" | "false" | "off" | "no" | "disabled" => Some(false),
        _ => None,
    })
}

fn next_pending_waited_frames(waited_frames: u32) -> u32 {
    waited_frames.saturating_add(1)
}

fn pending_screenshot_timed_out(waited_frames: u32, timeout_frames: u32) -> bool {
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
    use bevy::{
        ecs::message::{MessageCursor, Messages},
        render::render_resource::{Extent3d, TextureDimension, TextureFormat},
        window::WindowResolution,
    };

    fn env_reader<'a>(values: &'a [(&'a str, &'a str)]) -> impl FnMut(&str) -> Option<String> + 'a {
        move |key| {
            values
                .iter()
                .find_map(|(candidate, value)| (*candidate == key).then(|| (*value).to_owned()))
        }
    }

    fn screenshot_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<UiScreenshotFrameClock>()
            .init_resource::<UiScreenshotRequestIds>()
            .init_resource::<UiScreenshotPending>()
            .add_message::<UiScreenshotCommand>()
            .add_message::<UiScreenshotEvent>()
            .add_systems(First, advance_ui_screenshot_frame)
            .add_systems(
                Update,
                (handle_ui_screenshot_commands, expire_pending_ui_screenshot).chain(),
            );
        app
    }

    fn spawn_primary_window(app: &mut App) -> Entity {
        app.world_mut()
            .spawn((
                Window {
                    resolution: WindowResolution::new(360, 800),
                    ..default()
                },
                PrimaryWindow,
            ))
            .id()
    }

    fn collect_messages<M: Message + Clone>(app: &App) -> Vec<M> {
        let messages = app.world().resource::<Messages<M>>();
        let mut cursor = MessageCursor::default();
        cursor.read(messages).cloned().collect()
    }

    fn unique_temp_path(name: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "mybevy-ui-screenshot-test-{}-{name}",
            current_unix_timestamp_seconds()
        ))
    }

    fn tiny_test_image() -> Image {
        Image::new_fill(
            Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[255, 0, 0, 255],
            TextureFormat::Rgba8UnormSrgb,
            default(),
        )
    }

    fn trigger_pending_capture(app: &mut App) {
        let entity = app
            .world()
            .resource::<UiScreenshotPending>()
            .active
            .as_ref()
            .expect("screenshot request should be pending")
            .entity;
        app.world_mut().trigger(ScreenshotCaptured {
            entity,
            image: tiny_test_image(),
        });
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
        let path = build_manual_screenshot_path(
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
            path,
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
    fn screenshot_request_records_absolute_display_path() {
        let request = build_screenshot_request_record(
            UiScreenshotRequestId(7),
            PathBuf::from("../summary/ui-audit/manual/shot.png"),
            "ui_gallery",
            12,
        );

        assert!(request.display_path.is_absolute());
        assert!(
            request
                .display_path
                .ends_with("summary/ui-audit/manual/shot.png")
        );
    }

    #[test]
    fn pending_timeout_requires_waiting_past_limit() {
        assert!(!pending_screenshot_timed_out(300, 300));
        assert!(pending_screenshot_timed_out(301, 300));
        assert_eq!(next_pending_waited_frames(u32::MAX), u32::MAX);
    }

    #[test]
    fn pending_state_takes_matching_entity_only() {
        let mut pending = UiScreenshotPending::default();
        let request = UiScreenshotRequestRecord {
            request_id: UiScreenshotRequestId(1),
            path: PathBuf::from("../summary/ui-audit/manual/shot.png"),
            display_path: PathBuf::from("C:/project/mybevy/summary/ui-audit/manual/shot.png"),
            label: "ui_gallery".to_owned(),
            target_window: None,
            logical_size: None,
            physical_size: None,
            request_frame: 1,
        };
        let entity = Entity::from_raw_u32(7).unwrap();
        let other_entity = Entity::from_raw_u32(8).unwrap();

        pending.start(entity, request);
        assert!(pending.take_if_entity(other_entity).is_none());
        assert!(pending.active.is_some());

        assert!(pending.take_if_entity(entity).is_some());
        assert!(pending.active.is_none());
    }

    #[test]
    fn capture_command_starts_pending_and_rejects_duplicate_in_same_frame() {
        let mut app = screenshot_test_app();
        let window = spawn_primary_window(&mut app);
        let path_a = unique_temp_path("duplicate-a.png");
        let path_b = unique_temp_path("duplicate-b.png");

        app.world_mut().write_message(UiScreenshotCommand::Capture {
            path: path_a.clone(),
            label: "first".to_owned(),
        });
        app.world_mut().write_message(UiScreenshotCommand::Capture {
            path: path_b,
            label: "second".to_owned(),
        });
        app.update();

        let pending = app.world().resource::<UiScreenshotPending>();
        assert!(pending.active.is_some());
        let active = pending.active.as_ref().unwrap();
        assert_eq!(active.request.path, path_a);
        assert_eq!(active.request.target_window, Some(window));
        assert_eq!(active.request.logical_size, Some((360, 800)));
        assert_eq!(active.request.request_frame, 1);

        let events = collect_messages::<UiScreenshotEvent>(&app);
        assert_eq!(events.len(), 1);
        let UiScreenshotEvent::Failed(failure) = &events[0] else {
            panic!("duplicate request should fail");
        };
        assert_eq!(
            failure.reason,
            UiScreenshotFailureReason::AlreadyPending {
                active_request_id: UiScreenshotRequestId(1),
                active_path: absolute_display_path(&path_a),
            }
        );
        assert_eq!(failure.request.request_id, UiScreenshotRequestId(2));
        assert_eq!(failure.request.request_frame, 1);
        assert_eq!(failure.completion_frame, 1);
    }

    #[test]
    fn capture_command_fails_when_target_path_already_exists() {
        let mut app = screenshot_test_app();
        let window = spawn_primary_window(&mut app);
        let path = unique_temp_path("existing.png");
        fs::write(&path, b"existing").expect("temp screenshot placeholder should be writable");

        app.world_mut().write_message(UiScreenshotCommand::Capture {
            path: path.clone(),
            label: "existing".to_owned(),
        });
        app.update();

        let pending = app.world().resource::<UiScreenshotPending>();
        assert!(pending.active.is_none());

        let events = collect_messages::<UiScreenshotEvent>(&app);
        assert_eq!(events.len(), 1);
        let UiScreenshotEvent::Failed(failure) = &events[0] else {
            panic!("existing path should fail");
        };
        assert_eq!(failure.reason, UiScreenshotFailureReason::PathAlreadyExists);
        assert_eq!(failure.request.path, path.clone());
        assert_eq!(failure.request.target_window, Some(window));
        assert_eq!(failure.request.logical_size, Some((360, 800)));

        fs::remove_file(path).ok();
    }

    #[test]
    fn capture_command_fails_when_parent_directory_cannot_be_created() {
        let mut app = screenshot_test_app();
        spawn_primary_window(&mut app);
        let file_parent = unique_temp_path("file-parent");
        fs::write(&file_parent, b"not a directory")
            .expect("temp parent placeholder should be writable");
        let path = file_parent.join("shot.png");

        app.world_mut().write_message(UiScreenshotCommand::Capture {
            path,
            label: "bad parent".to_owned(),
        });
        app.update();

        let events = collect_messages::<UiScreenshotEvent>(&app);
        assert_eq!(events.len(), 1);
        let UiScreenshotEvent::Failed(failure) = &events[0] else {
            panic!("bad parent should fail");
        };
        assert!(matches!(
            failure.reason,
            UiScreenshotFailureReason::DirectoryCreateFailed { .. }
        ));

        fs::remove_file(file_parent).ok();
    }

    #[test]
    fn capture_command_fails_without_primary_window() {
        let mut app = screenshot_test_app();
        let path = unique_temp_path("no-primary.png");

        app.world_mut().write_message(UiScreenshotCommand::Capture {
            path,
            label: "no primary".to_owned(),
        });
        app.update();

        let events = collect_messages::<UiScreenshotEvent>(&app);
        assert_eq!(events.len(), 1);
        let UiScreenshotEvent::Failed(failure) = &events[0] else {
            panic!("missing primary window should fail");
        };
        assert_eq!(
            failure.reason,
            UiScreenshotFailureReason::PrimaryWindowUnavailable
        );
        assert_eq!(failure.request.request_frame, 1);
        assert_eq!(failure.completion_frame, 1);
    }

    #[test]
    fn timeout_failure_emits_event_and_clears_pending() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<UiScreenshotFrameClock>()
            .init_resource::<UiScreenshotPending>()
            .add_message::<UiScreenshotEvent>()
            .add_systems(First, advance_ui_screenshot_frame)
            .add_systems(Update, expire_pending_ui_screenshot);
        let entity = app.world_mut().spawn_empty().id();
        let request = build_screenshot_request_record(
            UiScreenshotRequestId(42),
            unique_temp_path("timeout.png"),
            "timeout",
            1,
        );
        app.world_mut().resource_mut::<UiScreenshotPending>().active = Some(PendingUiScreenshot {
            entity,
            request: request.clone(),
            waited_frames: 0,
            timeout_frames: 0,
        });

        app.update();

        assert!(
            app.world()
                .resource::<UiScreenshotPending>()
                .active
                .is_none()
        );
        let events = collect_messages::<UiScreenshotEvent>(&app);
        assert_eq!(events.len(), 1);
        let UiScreenshotEvent::Failed(failure) = &events[0] else {
            panic!("timeout should fail");
        };
        assert_eq!(failure.request, request);
        assert_eq!(
            failure.reason,
            UiScreenshotFailureReason::CaptureTimedOut {
                waited_frames: 1,
                timeout_frames: 0,
            }
        );
        assert_eq!(failure.completion_frame, 1);
    }

    #[test]
    fn captured_save_reports_path_conflict_as_failed_event() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<UiScreenshotEvent>()
            .add_systems(
                Update,
                move |mut events: MessageWriter<UiScreenshotEvent>| {
                    let path = unique_temp_path("save-conflict.png");
                    fs::write(&path, b"existing")
                        .expect("temp screenshot placeholder should be writable");
                    let request = build_screenshot_request_record(
                        UiScreenshotRequestId(3),
                        path.clone(),
                        "save conflict",
                        10,
                    );

                    save_captured_screenshot(tiny_test_image(), request, 12, &mut events);
                    fs::remove_file(path).ok();
                },
            );

        app.update();

        let events = collect_messages::<UiScreenshotEvent>(&app);
        assert_eq!(events.len(), 1);
        let UiScreenshotEvent::Failed(failure) = &events[0] else {
            panic!("save conflict should fail");
        };
        assert_eq!(failure.reason, UiScreenshotFailureReason::PathAlreadyExists);
        assert_eq!(failure.captured_size, Some((1, 1)));
        assert_eq!(failure.request.request_frame, 10);
        assert_eq!(failure.completion_frame, 12);
    }

    #[test]
    fn command_capture_observer_saves_file_and_cleans_up_temp_artifact() {
        let mut app = screenshot_test_app();
        spawn_primary_window(&mut app);
        let path = unique_temp_path("command-saved.png");

        app.world_mut().write_message(UiScreenshotCommand::Capture {
            path: path.clone(),
            label: "command save".to_owned(),
        });
        app.update();
        assert!(!path.exists());

        trigger_pending_capture(&mut app);

        assert!(path.exists());
        let events = collect_messages::<UiScreenshotEvent>(&app);
        assert_eq!(events.len(), 1);
        let UiScreenshotEvent::Saved(saved) = &events[0] else {
            panic!("command capture should save");
        };
        assert_eq!(saved.request.label, "command_save");
        assert_eq!(saved.request.path, path.clone());
        assert_eq!(saved.captured_size, (1, 1));
        assert_eq!(saved.request.request_frame, 1);
        assert_eq!(saved.completion_frame, 1);
        assert!(
            app.world()
                .resource::<UiScreenshotPending>()
                .active
                .is_none()
        );

        fs::remove_file(path).expect("test screenshot artifact should be removable");
    }

    #[test]
    fn manual_f9_writes_capture_command_with_default_path() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<UiScreenshotCommand>()
            .init_resource::<ButtonInput<KeyCode>>()
            .insert_resource(UiAuditScreenshotConfig {
                manual_enabled: true,
                manual_output_dir: PathBuf::from("../summary/ui-audit/manual"),
            })
            .insert_resource(UiCurrentOwner {
                owner: Some(crate::framework::ui::core::UiOwnerId::new("ui/gallery")),
            })
            .add_systems(Update, request_manual_screenshot);
        spawn_primary_window(&mut app);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F9);

        app.update();

        let commands = collect_messages::<UiScreenshotCommand>(&app);
        assert_eq!(commands.len(), 1);
        let UiScreenshotCommand::Capture { path, label } = &commands[0];
        assert_eq!(label, "ui_gallery");
        assert!(path.starts_with("../summary/ui-audit/manual"));
        assert!(
            path.file_name()
                .and_then(|filename| filename.to_str())
                .is_some_and(|filename| filename
                    .contains("_ui_gallery_logical-360x800_physical-360x800.png"))
        );
    }

    #[test]
    fn manual_f9_capture_observer_saves_file_and_cleans_up_temp_artifact() {
        let output_dir = env::temp_dir().join(format!(
            "mybevy-ui-screenshot-f9-{}",
            current_unix_timestamp_seconds()
        ));
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<UiScreenshotFrameClock>()
            .init_resource::<UiScreenshotRequestIds>()
            .init_resource::<UiScreenshotPending>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<UiScreenshotCommand>()
            .add_message::<UiScreenshotEvent>()
            .insert_resource(UiAuditScreenshotConfig {
                manual_enabled: true,
                manual_output_dir: output_dir.clone(),
            })
            .insert_resource(UiCurrentOwner {
                owner: Some(crate::framework::ui::core::UiOwnerId::new("ui/gallery")),
            })
            .add_systems(First, advance_ui_screenshot_frame)
            .add_systems(
                Update,
                (
                    request_manual_screenshot,
                    handle_ui_screenshot_commands,
                    expire_pending_ui_screenshot,
                )
                    .chain(),
            );
        spawn_primary_window(&mut app);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F9);

        app.update();
        trigger_pending_capture(&mut app);

        let events = collect_messages::<UiScreenshotEvent>(&app);
        assert_eq!(events.len(), 1);
        let UiScreenshotEvent::Saved(saved) = &events[0] else {
            panic!("manual F9 capture should save");
        };
        assert_eq!(saved.request.label, "ui_gallery");
        assert!(saved.request.path.starts_with(&output_dir));
        assert!(saved.request.path.exists());
        assert_eq!(saved.captured_size, (1, 1));

        fs::remove_file(&saved.request.path).expect("test screenshot artifact should be removable");
        fs::remove_dir(output_dir).expect("test screenshot directory should be removable");
    }
}
