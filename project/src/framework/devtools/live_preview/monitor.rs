use bevy::{
    camera::{RenderTarget, visibility::RenderLayers},
    prelude::*,
    window::{WindowRef, WindowResolution},
};

use crate::framework::ui::{
    core::{UiLayer, UiLayerRoot, UiMetrics, UiViewport},
    style::{
        UiFontAssets, UiTheme,
        theme::{
            UiThemeBackgroundRole, UiThemeBorderRole, UiThemePanelNodeRole, UiThemeRootNodeRole,
            UiThemeTextColorRole, UiThemeTextStyleRole,
        },
    },
    widgets::{UiScrollViewConfig, screen_label, screen_title, ui_scroll_column_bundle},
};

const MONITOR_RENDER_LAYER: usize = 30;
const MONITOR_WINDOW_WIDTH: u32 = 720;
const MONITOR_WINDOW_HEIGHT: u32 = 760;
const MONITOR_MAX_TEXT_CHARS: usize = 12_000;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LivePreviewMonitorTab {
    #[default]
    Overview,
    Ui,
    Player,
    Scene,
    Network,
    Performance,
    Timeline,
}

impl LivePreviewMonitorTab {
    const ALL: [Self; 7] = [
        Self::Overview,
        Self::Ui,
        Self::Player,
        Self::Scene,
        Self::Network,
        Self::Performance,
        Self::Timeline,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Ui => "UI",
            Self::Player => "Player",
            Self::Scene => "Scene",
            Self::Network => "Network",
            Self::Performance => "Performance",
            Self::Timeline => "Timeline",
        }
    }

    fn next(self) -> Self {
        let index = Self::ALL.iter().position(|tab| *tab == self).unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LivePreviewMonitorTarget {
    #[default]
    GameWindow,
    DedicatedWindow,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LivePreviewMonitorPanelFilter {
    #[default]
    All,
    ActiveOnly,
    BlockingOnly,
}

impl LivePreviewMonitorPanelFilter {
    fn next(self) -> Self {
        match self {
            Self::All => Self::ActiveOnly,
            Self::ActiveOnly => Self::BlockingOnly,
            Self::BlockingOnly => Self::All,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::ActiveOnly => "active",
            Self::BlockingOnly => "blocking",
        }
    }
}

impl LivePreviewMonitorTarget {
    fn label(self) -> &'static str {
        match self {
            Self::GameWindow => "main",
            Self::DedicatedWindow => "dedicated",
        }
    }

    fn next_supported(self) -> Self {
        match self {
            Self::GameWindow if supports_dedicated_window() => Self::DedicatedWindow,
            _ => Self::GameWindow,
        }
    }
}

#[derive(Clone, Debug, Resource)]
pub struct LivePreviewMonitorState {
    pub enabled: bool,
    pub frozen: bool,
    pub target: LivePreviewMonitorTarget,
    pub tab: LivePreviewMonitorTab,
    pub panel_filter: LivePreviewMonitorPanelFilter,
    pub highlight_panels: bool,
    pub root: Option<Entity>,
    pub window: Option<Entity>,
    pub camera: Option<Entity>,
    pub last_export: Option<String>,
}

impl Default for LivePreviewMonitorState {
    fn default() -> Self {
        Self {
            enabled: false,
            frozen: false,
            target: LivePreviewMonitorTarget::GameWindow,
            tab: LivePreviewMonitorTab::Overview,
            panel_filter: LivePreviewMonitorPanelFilter::All,
            highlight_panels: false,
            root: None,
            window: None,
            camera: None,
            last_export: None,
        }
    }
}

#[derive(Component)]
struct LivePreviewMonitorRoot;

#[derive(Component)]
struct LivePreviewMonitorWindow;

#[derive(Component)]
struct LivePreviewMonitorCamera;

#[derive(Component)]
struct LivePreviewMonitorHeader;

#[derive(Component)]
struct LivePreviewMonitorTabs;

#[derive(Component)]
struct LivePreviewMonitorBody;

pub(crate) struct LivePreviewMonitorPlugin;

impl Plugin for LivePreviewMonitorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LivePreviewMonitorState>().add_systems(
            Update,
            (
                handle_monitor_keys,
                sync_monitor_target,
                refresh_monitor_view,
            )
                .chain(),
        );
    }
}

fn handle_monitor_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<LivePreviewMonitorState>,
    hub: Res<super::LivePreviewSnapshotHub>,
) {
    if !cfg!(debug_assertions) || cfg!(target_os = "android") {
        state.enabled = false;
        return;
    }
    if keys.just_pressed(KeyCode::F3) {
        state.enabled = !state.enabled;
    }
    if keys.just_pressed(KeyCode::F4) {
        state.frozen = !state.frozen;
    }
    if keys.just_pressed(KeyCode::F7) {
        state.target = state.target.next_supported();
        state.root = None;
    }
    if keys.just_pressed(KeyCode::F5) {
        state.panel_filter = state.panel_filter.next();
    }
    if keys.just_pressed(KeyCode::F6) {
        state.highlight_panels = !state.highlight_panels;
    }
    if keys.just_pressed(KeyCode::F8) {
        state.last_export = Some(redacted_export(&hub.read()));
    }
    if keys.just_pressed(KeyCode::Tab) && state.enabled {
        state.tab = state.tab.next();
    }
}

fn sync_monitor_target(
    mut commands: Commands,
    theme: Res<UiTheme>,
    metrics: Res<UiMetrics>,
    viewport: Res<UiViewport>,
    fonts: Res<UiFontAssets>,
    mut state: ResMut<LivePreviewMonitorState>,
    roots: Query<Entity, With<LivePreviewMonitorRoot>>,
    windows: Query<Entity, With<LivePreviewMonitorWindow>>,
    cameras: Query<Entity, With<LivePreviewMonitorCamera>>,
) {
    if !cfg!(debug_assertions) || cfg!(target_os = "android") {
        if state.enabled {
            state.enabled = false;
        }
    }
    let normalized_target = match state.target {
        LivePreviewMonitorTarget::DedicatedWindow if !supports_dedicated_window() => {
            LivePreviewMonitorTarget::GameWindow
        }
        target => target,
    };
    if normalized_target != state.target {
        state.target = normalized_target;
        state.root = None;
    }
    if let Some(root) = state.root
        && !roots.contains(root)
    {
        state.root = None;
    }
    if !state.enabled {
        despawn_monitor_roots(&mut commands, &roots, &windows, &cameras);
        state.root = None;
        state.window = None;
        state.camera = None;
        return;
    }
    let target_camera = match state.target {
        LivePreviewMonitorTarget::GameWindow => {
            despawn_monitor_window(&mut commands, &windows, &cameras);
            state.window = None;
            state.camera = None;
            None
        }
        LivePreviewMonitorTarget::DedicatedWindow => {
            let valid_window = state.window.is_some_and(|window| windows.contains(window));
            let valid_camera = state.camera.is_some_and(|camera| cameras.contains(camera));
            if !valid_window || !valid_camera {
                despawn_monitor_window(&mut commands, &windows, &cameras);
                let (window, camera) = spawn_monitor_window(&mut commands);
                state.window = Some(window);
                state.camera = Some(camera);
                state.root = None;
            }
            state.camera
        }
    };
    if state.root.is_none() {
        for root in roots.iter() {
            commands.entity(root).try_despawn();
        }
        state.root = Some(spawn_monitor_root(
            &mut commands,
            &theme,
            &metrics,
            &viewport,
            &fonts,
            state.target,
            target_camera,
        ));
    }
}

fn refresh_monitor_view(
    state: Res<LivePreviewMonitorState>,
    hub: Res<super::LivePreviewSnapshotHub>,
    mut headers: Query<&mut Text, With<LivePreviewMonitorHeader>>,
    mut tabs: Query<&mut Text, With<LivePreviewMonitorTabs>>,
    mut bodies: Query<&mut Text, With<LivePreviewMonitorBody>>,
) {
    if !state.enabled {
        return;
    }
    let snapshot = hub.read();
    let header = monitor_header(&state, &snapshot);
    let tab_line = LivePreviewMonitorTab::ALL
        .iter()
        .map(|tab| {
            if *tab == state.tab {
                format!("[{}]", tab.label())
            } else {
                tab.label().to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("  ");
    if let Ok(mut text) = headers.single_mut() {
        text.0 = header;
    }
    if let Ok(mut text) = tabs.single_mut() {
        text.0 = tab_line;
    }
    if state.frozen {
        return;
    }
    let body = monitor_body(&snapshot, state.tab);
    if let Ok(mut text) = bodies.single_mut() {
        text.0 = body;
    }
}

fn monitor_header(
    state: &LivePreviewMonitorState,
    snapshot: &super::LivePreviewSnapshot,
) -> String {
    format!(
        "LIVE PREVIEW  target={}  freeze={}  filter={}  highlight={}  seq={}  frame={}",
        state.target.label(),
        if state.frozen { "on" } else { "off" },
        state.panel_filter.label(),
        if state.highlight_panels { "on" } else { "off" },
        snapshot.sequence,
        snapshot.captured_frame,
    )
}

fn spawn_monitor_root(
    commands: &mut Commands,
    theme: &UiTheme,
    metrics: &UiMetrics,
    viewport: &UiViewport,
    fonts: &UiFontAssets,
    target: LivePreviewMonitorTarget,
    target_camera: Option<Entity>,
) -> Entity {
    let mut node = Node {
        position_type: PositionType::Absolute,
        left: px(metrics.page_padding),
        top: px(metrics.page_padding),
        flex_direction: FlexDirection::Column,
        row_gap: px(metrics.control_gap),
        padding: UiRect::all(px(theme.panel.padding)),
        border: UiRect::all(px(theme.panel.border)),
        border_radius: BorderRadius::all(px(theme.panel.radius)),
        ..default()
    };
    match target {
        LivePreviewMonitorTarget::GameWindow => {
            node.width = px((viewport.logical_width * 0.42).clamp(420.0, 720.0));
            node.max_height = percent(92.0);
        }
        LivePreviewMonitorTarget::DedicatedWindow => {
            node.left = px(metrics.page_padding);
            node.top = px(metrics.page_padding);
            node.width = percent(100.0);
            node.height = percent(100.0);
        }
    }
    let mut root = commands.spawn((
        LivePreviewMonitorRoot,
        UiLayerRoot {
            layer: UiLayer::Debug,
        },
        UiThemeRootNodeRole::Debug,
        UiThemePanelNodeRole::Debug,
        UiThemeBackgroundRole::Panel,
        UiThemeBorderRole::Panel,
        node,
        BackgroundColor(theme.colors.panel_background),
        BorderColor::all(theme.colors.panel_border),
        ZIndex(250),
        Pickable::IGNORE,
    ));
    if let Some(camera) = target_camera {
        root.insert(UiTargetCamera(camera));
    }
    root.with_children(|root| {
        root.spawn((
            screen_title(
                theme,
                fonts,
                "Live Preview Monitor",
                UiThemeTextStyleRole::Subtitle,
            ),
            LivePreviewMonitorHeader,
            Pickable::IGNORE,
        ));
        root.spawn((
            Node {
                width: percent(100.0),
                min_height: px(metrics.touch_target_min),
                ..default()
            },
            screen_label(
                theme,
                fonts,
                "[Overview]  UI  Player  Scene  Network  Performance  Timeline",
                UiThemeTextStyleRole::Caption,
                UiThemeTextColorRole::Primary,
            ),
            LivePreviewMonitorTabs,
            Pickable::IGNORE,
        ));
        root.spawn((
            ui_scroll_column_bundle(
                UiScrollViewConfig::new(metrics.control_gap).with_max_height(
                    (viewport.logical_height - metrics.page_padding * 3.0).max(280.0),
                ),
            ),
            Node {
                width: percent(100.0),
                flex_grow: 1.0,
                ..default()
            },
        ))
        .with_children(|body| {
            body.spawn((
                Node {
                    width: percent(100.0),
                    ..default()
                },
                screen_label(
                    theme,
                    fonts,
                    "",
                    UiThemeTextStyleRole::Caption,
                    UiThemeTextColorRole::Primary,
                ),
                LivePreviewMonitorBody,
                Pickable::IGNORE,
            ));
        });
    })
    .id()
}

fn spawn_monitor_window(commands: &mut Commands) -> (Entity, Entity) {
    let window = commands
        .spawn((
            LivePreviewMonitorWindow,
            Window {
                title: "MyBevy Live Preview".to_owned(),
                resolution: WindowResolution::new(MONITOR_WINDOW_WIDTH, MONITOR_WINDOW_HEIGHT),
                ..default()
            },
        ))
        .id();
    let camera = commands
        .spawn((
            LivePreviewMonitorCamera,
            Camera2d,
            RenderLayers::layer(MONITOR_RENDER_LAYER),
            RenderTarget::Window(WindowRef::Entity(window)),
        ))
        .id();
    (window, camera)
}

fn despawn_monitor_roots(
    commands: &mut Commands,
    roots: &Query<Entity, With<LivePreviewMonitorRoot>>,
    windows: &Query<Entity, With<LivePreviewMonitorWindow>>,
    cameras: &Query<Entity, With<LivePreviewMonitorCamera>>,
) {
    for root in roots {
        commands.entity(root).try_despawn();
    }
    despawn_monitor_window(commands, windows, cameras);
}

fn despawn_monitor_window(
    commands: &mut Commands,
    windows: &Query<Entity, With<LivePreviewMonitorWindow>>,
    cameras: &Query<Entity, With<LivePreviewMonitorCamera>>,
) {
    for camera in cameras {
        commands.entity(camera).try_despawn();
    }
    for window in windows {
        commands.entity(window).try_despawn();
    }
}

fn supports_dedicated_window() -> bool {
    !cfg!(target_os = "android")
}

fn monitor_body(snapshot: &super::LivePreviewSnapshot, tab: LivePreviewMonitorTab) -> String {
    let body = match tab {
        LivePreviewMonitorTab::Overview => overview_body(snapshot),
        LivePreviewMonitorTab::Ui => section_body(
            "UI",
            snapshot.ui.status,
            snapshot.ui.value.as_ref().map(|value| {
                format!(
                    "screen={} owner={} panels={}",
                    value.canonical_screen.as_deref().unwrap_or("-"),
                    value.owner.as_deref().unwrap_or("-"),
                    value
                        .panel_count
                        .map_or("-".to_owned(), |count| count.to_string())
                )
            }),
        ),
        LivePreviewMonitorTab::Player => section_body(
            "Player",
            snapshot.player.status,
            snapshot.player.value.as_ref().map(|value| {
                format!(
                    "character={} world={} movement={}",
                    value.character_id.as_ref().map_or("-", |id| id.as_str()),
                    value.world_id.as_ref().map_or("-", |id| id.as_str()),
                    value.movement_state.as_deref().unwrap_or("-")
                )
            }),
        ),
        LivePreviewMonitorTab::Scene => section_body(
            "Scene",
            snapshot.scene.status,
            snapshot.scene.value.as_ref().map(|value| {
                format!(
                    "active={} status={} lifecycle={}",
                    value.active_scene_id.as_ref().map_or("-", |id| id.as_str()),
                    value.scene_status.as_deref().unwrap_or("-"),
                    value.lifecycle.as_deref().unwrap_or("-")
                )
            }),
        ),
        LivePreviewMonitorTab::Network => section_body(
            "Network",
            snapshot.network.status,
            snapshot.network.value.as_ref().map(|value| {
                format!(
                    "connection={} room={} authority={} health={}",
                    value.connection_state.as_deref().unwrap_or("-"),
                    value.room_id.as_ref().map_or("-", |id| id.as_str()),
                    value.authority_endpoint_kind.as_deref().unwrap_or("-"),
                    value.authority_sync_health.as_deref().unwrap_or("-")
                )
            }),
        ),
        LivePreviewMonitorTab::Performance => section_body(
            "Performance",
            snapshot.performance.status,
            snapshot.performance.value.as_ref().map(|value| {
                format!(
                    "fps={} frame_ms={} timeline={}",
                    format_number(value.fps),
                    format_number(value.frame_time_ms),
                    value
                        .timeline_entry_count
                        .map_or("-".to_owned(), |count| count.to_string())
                )
            }),
        ),
        LivePreviewMonitorTab::Timeline => format!(
            "Timeline\nentries are available in the shared fixed-capacity timeline ({} source records).",
            snapshot
                .source_health
                .value
                .as_ref()
                .map_or(0, |health| health.sources.len())
        ),
    };
    truncate_text(body)
}

fn overview_body(snapshot: &super::LivePreviewSnapshot) -> String {
    let ui = snapshot.ui.value.as_ref();
    let player = snapshot.player.value.as_ref();
    let scene = snapshot.scene.value.as_ref();
    let network = snapshot.network.value.as_ref();
    let performance = snapshot.performance.value.as_ref();
    let warnings = snapshot
        .source_health
        .value
        .as_ref()
        .map(|health| {
            health
                .sources
                .iter()
                .filter(|source| {
                    matches!(
                        source.status,
                        super::PreviewDataStatus::Failed | super::PreviewDataStatus::Unavailable
                    )
                })
                .count()
        })
        .unwrap_or(0);
    format!(
        "Overview\nScreen: {}\nOwner: {}\nCharacter: {}\nRoom: {}\nScene: {}\nConnection: {}\nAuthority: {} / {}\nFPS: {}\nWarnings: {}",
        ui.and_then(|value| value.canonical_screen.as_deref())
            .unwrap_or("unavailable"),
        ui.and_then(|value| value.owner.as_deref())
            .unwrap_or("unavailable"),
        player
            .and_then(|value| value.display_name.as_deref())
            .unwrap_or("unavailable"),
        network
            .and_then(|value| value.room_id.as_ref().map(|id| id.as_str()))
            .unwrap_or("unavailable"),
        scene
            .and_then(|value| value.active_scene_id.as_ref().map(|id| id.as_str()))
            .unwrap_or("unavailable"),
        network
            .and_then(|value| value.connection_state.as_deref())
            .unwrap_or("unavailable"),
        network
            .and_then(|value| value.authority_endpoint_kind.as_deref())
            .unwrap_or("unavailable"),
        network
            .and_then(|value| value.authority_sync_health.as_deref())
            .unwrap_or("unavailable"),
        performance.map_or("unavailable".to_owned(), |value| format_number(value.fps)),
        warnings,
    )
}

fn section_body(name: &str, status: super::PreviewDataStatus, detail: Option<String>) -> String {
    format!(
        "{name}\nstatus={}\n{}",
        format!("{status:?}").to_ascii_lowercase(),
        detail.unwrap_or_else(|| "data unavailable".to_owned()),
    )
}

fn format_number(value: Option<f32>) -> String {
    value.map_or_else(|| "-".to_owned(), |value| format!("{value:.1}"))
}

fn truncate_text(mut value: String) -> String {
    if value.chars().count() > MONITOR_MAX_TEXT_CHARS {
        value = value.chars().take(MONITOR_MAX_TEXT_CHARS).collect();
        value.push_str("\n... truncated ...");
    }
    value
}

fn redacted_export(snapshot: &super::LivePreviewSnapshot) -> String {
    serde_json::to_string(snapshot).unwrap_or_else(|_| "{\"status\":\"export_failed\"}".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::devtools::live_preview::{
        LivePreviewSnapshot, NetworkPreviewState, PreviewSection, StablePreviewId,
    };

    #[test]
    fn monitor_tab_cycle_is_stable() {
        assert_eq!(
            LivePreviewMonitorTab::Overview.next(),
            LivePreviewMonitorTab::Ui
        );
        assert_eq!(
            LivePreviewMonitorTab::Timeline.next(),
            LivePreviewMonitorTab::Overview
        );
    }

    #[test]
    fn panel_filter_cycle_preserves_f5_f6_semantics() {
        assert_eq!(
            LivePreviewMonitorPanelFilter::All.next(),
            LivePreviewMonitorPanelFilter::ActiveOnly
        );
        assert_eq!(
            LivePreviewMonitorPanelFilter::ActiveOnly.next(),
            LivePreviewMonitorPanelFilter::BlockingOnly
        );
        assert_eq!(
            LivePreviewMonitorPanelFilter::BlockingOnly.next(),
            LivePreviewMonitorPanelFilter::All
        );
    }

    #[test]
    fn disabled_target_falls_back_to_game_window_on_android_contract() {
        assert_eq!(
            LivePreviewMonitorTarget::GameWindow.next_supported(),
            if supports_dedicated_window() {
                LivePreviewMonitorTarget::DedicatedWindow
            } else {
                LivePreviewMonitorTarget::GameWindow
            }
        );
    }

    #[test]
    fn overview_handles_missing_sections_without_layout_unbounded_text() {
        let mut snapshot = LivePreviewSnapshot::default();
        snapshot.network = PreviewSection::available(
            1,
            NetworkPreviewState {
                room_id: Some(StablePreviewId::from("room")),
                ..Default::default()
            },
        );
        let body = overview_body(&snapshot);
        assert!(body.contains("Screen: unavailable"));
        assert!(body.contains("Room: room"));
        assert!(body.chars().count() < MONITOR_MAX_TEXT_CHARS);
    }

    #[test]
    fn export_is_snapshot_only_and_does_not_add_credentials() {
        let snapshot = LivePreviewSnapshot::default();
        let exported = redacted_export(&snapshot);
        assert!(exported.contains("schema_version"));
        assert!(!exported.contains("token"));
    }

    #[test]
    fn frozen_header_remains_visible_while_body_can_stay_unchanged() {
        let snapshot = LivePreviewSnapshot::default();
        let state = LivePreviewMonitorState {
            frozen: true,
            ..Default::default()
        };
        assert!(monitor_header(&state, &snapshot).contains("freeze=on"));
    }
}
