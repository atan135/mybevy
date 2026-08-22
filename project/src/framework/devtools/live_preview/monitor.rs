use bevy::{
    camera::{RenderTarget, visibility::RenderLayers},
    prelude::*,
    window::{WindowRef, WindowResolution},
};
use std::time::Instant;

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
const MONITOR_PERFORMANCE_SAMPLE_INTERVAL_MS: u128 = 500;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LivePreviewMonitorRuntimeMode {
    #[default]
    Closed,
    MainWindow,
    DedicatedWindow,
}

#[derive(Debug, Resource)]
pub struct LivePreviewMonitorPerformance {
    last_sample_at: Instant,
    pub mode: LivePreviewMonitorRuntimeMode,
    pub all_entity_count: u64,
    pub monitor_entity_count: u64,
    pub measurement_cpu_us: u64,
    pub collector_cpu_us: u64,
    pub memory_estimate_bytes: u64,
    pub sample_count: u64,
}

impl Default for LivePreviewMonitorPerformance {
    fn default() -> Self {
        Self {
            last_sample_at: Instant::now(),
            mode: LivePreviewMonitorRuntimeMode::Closed,
            all_entity_count: 0,
            monitor_entity_count: 0,
            measurement_cpu_us: 0,
            collector_cpu_us: 0,
            memory_estimate_bytes: (super::redaction::LIVE_PREVIEW_MAX_EXPORT_BYTES as u64)
                + (super::timeline::LIVE_PREVIEW_TIMELINE_MAX_CAPACITY as u64)
                    * (super::timeline::LIVE_PREVIEW_TIMELINE_MAX_SUMMARY_CHARS as u64
                        + super::timeline::LIVE_PREVIEW_TIMELINE_MAX_DETAIL_CHARS as u64),
            sample_count: 0,
        }
    }
}

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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LivePreviewMonitorTimelineSeverityFilter {
    #[default]
    All,
    Info,
    Warning,
    Error,
    Critical,
}

impl LivePreviewMonitorTimelineSeverityFilter {
    fn next(self) -> Self {
        match self {
            Self::All => Self::Info,
            Self::Info => Self::Warning,
            Self::Warning => Self::Error,
            Self::Error => Self::Critical,
            Self::Critical => Self::All,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Critical => "critical",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LivePreviewMonitorTimelineDomainFilter {
    #[default]
    All,
    Ui,
    Player,
    Scene,
    Network,
    Performance,
    SourceHealth,
}

impl LivePreviewMonitorTimelineDomainFilter {
    fn next(self) -> Self {
        match self {
            Self::All => Self::Ui,
            Self::Ui => Self::Player,
            Self::Player => Self::Scene,
            Self::Scene => Self::Network,
            Self::Network => Self::Performance,
            Self::Performance => Self::SourceHealth,
            Self::SourceHealth => Self::All,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Ui => "ui",
            Self::Player => "player",
            Self::Scene => "scene",
            Self::Network => "network",
            Self::Performance => "performance",
            Self::SourceHealth => "source_health",
        }
    }
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
    pub timeline_severity_filter: LivePreviewMonitorTimelineSeverityFilter,
    pub timeline_domain_filter: LivePreviewMonitorTimelineDomainFilter,
    pub scroll_offsets: [f32; 7],
    pub focused_tab: LivePreviewMonitorTab,
    pub scroll_restore_pending: bool,
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
            timeline_severity_filter: LivePreviewMonitorTimelineSeverityFilter::All,
            timeline_domain_filter: LivePreviewMonitorTimelineDomainFilter::All,
            scroll_offsets: [0.0; 7],
            focused_tab: LivePreviewMonitorTab::Overview,
            scroll_restore_pending: false,
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
struct LivePreviewMonitorBody(LivePreviewMonitorTab);

#[derive(Component)]
struct LivePreviewMonitorScroll(LivePreviewMonitorTab);

pub(crate) struct LivePreviewMonitorPlugin;

impl Plugin for LivePreviewMonitorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LivePreviewMonitorState>()
            .init_resource::<LivePreviewMonitorPerformance>()
            .add_systems(
                Update,
                (
                    handle_monitor_keys,
                    sync_monitor_target,
                    sync_monitor_scroll,
                    refresh_monitor_view,
                    measure_monitor_performance,
                )
                    .chain(),
            );
    }
}

fn measure_monitor_performance(
    state: Res<LivePreviewMonitorState>,
    budget: Res<super::LivePreviewPerformanceBudget>,
    mut performance: ResMut<LivePreviewMonitorPerformance>,
    entities: Query<Entity>,
    roots: Query<Entity, With<LivePreviewMonitorRoot>>,
    windows: Query<Entity, With<LivePreviewMonitorWindow>>,
    cameras: Query<Entity, With<LivePreviewMonitorCamera>>,
    scrolls: Query<Entity, With<LivePreviewMonitorScroll>>,
) {
    if performance.last_sample_at.elapsed().as_millis() < MONITOR_PERFORMANCE_SAMPLE_INTERVAL_MS {
        return;
    }
    let started_at = Instant::now();
    performance.last_sample_at = started_at;
    performance.mode = if !state.enabled {
        LivePreviewMonitorRuntimeMode::Closed
    } else {
        match state.target {
            LivePreviewMonitorTarget::GameWindow => LivePreviewMonitorRuntimeMode::MainWindow,
            LivePreviewMonitorTarget::DedicatedWindow => {
                LivePreviewMonitorRuntimeMode::DedicatedWindow
            }
        }
    };
    performance.all_entity_count = entities.iter().count() as u64;
    performance.monitor_entity_count = (roots.iter().count()
        + windows.iter().count()
        + cameras.iter().count()
        + scrolls.iter().count()) as u64;
    performance.collector_cpu_us = budget.last_collector_time_us();
    performance.measurement_cpu_us =
        started_at.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;
    performance.sample_count = performance.sample_count.saturating_add(1);
}

fn handle_monitor_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<LivePreviewMonitorState>,
    hub: Res<super::LivePreviewSnapshotHub>,
    policy: Res<super::LivePreviewPolicy>,
) {
    if !(*policy).is_enabled() {
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
    if keys.just_pressed(KeyCode::F8) && state.enabled {
        state.last_export = Some(redacted_export(&hub.read()));
    }
    if keys.just_pressed(KeyCode::F5) {
        apply_ui_shortcut(&mut state, KeyCode::F5);
    }
    if keys.just_pressed(KeyCode::F6) {
        apply_ui_shortcut(&mut state, KeyCode::F6);
    }
    if keys.just_pressed(KeyCode::PageUp) {
        apply_timeline_shortcut(&mut state, KeyCode::PageUp);
    }
    if keys.just_pressed(KeyCode::PageDown) {
        apply_timeline_shortcut(&mut state, KeyCode::PageDown);
    }
    if keys.just_pressed(KeyCode::Tab) && state.enabled {
        state.tab = state.tab.next();
        state.focused_tab = state.tab;
        state.scroll_restore_pending = true;
    }
}

fn apply_timeline_shortcut(state: &mut LivePreviewMonitorState, key: KeyCode) {
    if state.tab != LivePreviewMonitorTab::Timeline {
        return;
    }
    match key {
        KeyCode::PageUp => state.timeline_severity_filter = state.timeline_severity_filter.next(),
        KeyCode::PageDown => state.timeline_domain_filter = state.timeline_domain_filter.next(),
        _ => {}
    }
}

fn apply_ui_shortcut(state: &mut LivePreviewMonitorState, key: KeyCode) {
    if state.tab != LivePreviewMonitorTab::Ui {
        return;
    }
    match key {
        KeyCode::F5 => state.panel_filter = state.panel_filter.next(),
        KeyCode::F6 => state.highlight_panels = !state.highlight_panels,
        _ => {}
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
            state.tab,
        ));
        state.scroll_restore_pending = true;
    }
}

fn sync_monitor_scroll(
    state: ResMut<LivePreviewMonitorState>,
    mut scrolls: Query<(
        &LivePreviewMonitorScroll,
        &mut ScrollPosition,
        &mut Visibility,
    )>,
) {
    let mut state = state;
    let restore = state.scroll_restore_pending;
    for (scroll, mut position, mut visibility) in &mut scrolls {
        if scroll.0 == state.tab {
            position.0.y = sync_scroll_offset(&mut state, scroll.0, position.0.y, restore);
            *visibility = Visibility::Visible;
        } else {
            sync_scroll_offset(&mut state, scroll.0, position.0.y, false);
            *visibility = Visibility::Hidden;
        }
    }
    state.scroll_restore_pending = false;
}

fn sync_scroll_offset(
    state: &mut LivePreviewMonitorState,
    tab: LivePreviewMonitorTab,
    observed: f32,
    restore: bool,
) -> f32 {
    let index = tab_index(tab);
    if restore && tab == state.tab {
        state.scroll_offsets[index].max(0.0)
    } else {
        let observed = observed.max(0.0);
        state.scroll_offsets[index] = observed;
        observed
    }
}

fn tab_index(tab: LivePreviewMonitorTab) -> usize {
    LivePreviewMonitorTab::ALL
        .iter()
        .position(|candidate| *candidate == tab)
        .unwrap_or(0)
}

fn refresh_monitor_view(
    state: Res<LivePreviewMonitorState>,
    hub: Res<super::LivePreviewSnapshotHub>,
    mut headers: Query<&mut Text, With<LivePreviewMonitorHeader>>,
    mut tabs: Query<&mut Text, With<LivePreviewMonitorTabs>>,
    mut bodies: Query<(&LivePreviewMonitorBody, &mut Text)>,
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
    let body = monitor_body(&snapshot, &state);
    if let Some((_, mut text)) = bodies.iter_mut().find(|(body, _)| body.0 == state.tab) {
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
    active_tab: LivePreviewMonitorTab,
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
        for tab in LivePreviewMonitorTab::ALL {
            root.spawn((
                ui_scroll_column_bundle(
                    UiScrollViewConfig::new(metrics.control_gap).with_max_height(
                        (viewport.logical_height - metrics.page_padding * 3.0).max(280.0),
                    ),
                ),
                LivePreviewMonitorScroll(tab),
                if tab == active_tab {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                },
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
                    LivePreviewMonitorBody(tab),
                    Pickable::IGNORE,
                ));
            });
        }
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

fn monitor_body(snapshot: &super::LivePreviewSnapshot, state: &LivePreviewMonitorState) -> String {
    let body = match state.tab {
        LivePreviewMonitorTab::Overview => overview_body(snapshot),
        LivePreviewMonitorTab::Ui => ui_body(snapshot, state),
        LivePreviewMonitorTab::Player => player_body(snapshot),
        LivePreviewMonitorTab::Scene => scene_body(snapshot),
        LivePreviewMonitorTab::Network => network_body(snapshot),
        LivePreviewMonitorTab::Performance => performance_body(snapshot),
        LivePreviewMonitorTab::Timeline => timeline_body(snapshot, state),
    };
    truncate_text(body)
}

fn ui_body(snapshot: &super::LivePreviewSnapshot, state: &LivePreviewMonitorState) -> String {
    let Some(value) = snapshot.ui.value.as_ref() else {
        return section_body("UI", snapshot.ui.status, None);
    };
    let panels = value
        .panels
        .iter()
        .filter(|panel| match state.panel_filter {
            LivePreviewMonitorPanelFilter::All => true,
            LivePreviewMonitorPanelFilter::ActiveOnly => panel.active == Some(true),
            LivePreviewMonitorPanelFilter::BlockingOnly => {
                panel.kind.as_deref() == Some("blocking_overlay")
            }
        })
        .map(|panel| {
            format!(
                "{} kind={} active={} z={}{}",
                panel.id.as_str(),
                panel.kind.as_deref().unwrap_or("-"),
                bool_label(panel.active),
                optional_u64(panel.z_index.map(|value| value as u64)),
                if state.highlight_panels && panel.active == Some(true) {
                    " highlight"
                } else {
                    ""
                }
            )
        })
        .collect::<Vec<_>>();
    let viewport = value.viewport.as_ref().map_or_else(
        || "unavailable".to_owned(),
        |viewport| {
            format!(
                "{}x{} scale={} preview={} class={}/{} orientation={} input={} safe={:?}",
                format_float(viewport.logical_width),
                format_float(viewport.logical_height),
                format_float(viewport.device_scale),
                format_float(viewport.preview_scale),
                viewport.width_class,
                viewport.height_class,
                viewport.orientation,
                viewport.input_mode,
                viewport.safe_area,
            )
        },
    );
    let metrics = value.metrics.as_ref().map_or_else(
        || "unavailable".to_owned(),
        |metrics| {
            format!(
                "padding={} panel={} gap={} section_gap={} touch={} font={} max_width={}",
                format_float(metrics.page_padding),
                format_float(metrics.panel_padding),
                format_float(metrics.control_gap),
                format_float(metrics.section_gap),
                format_float(metrics.touch_target_min),
                format_float(metrics.font_body),
                format_float(metrics.content_max_width),
            )
        },
    );
    format!(
        "UI\nstatus={}\nscreen={} owner={}\nviewport={}\nmetrics={}\ninput pointer_blocked={} reason={} route={} blocking={}\nfocus panel={} node={}\nstats nodes={}/{} text={} panels={} kinds={}\npanel filter={} highlight={} visible={}/{}\ntree panels:\n{}\nlayout document={} schema={} status={} source={} error={}",
        status_label(snapshot.ui.status),
        value.canonical_screen.as_deref().unwrap_or("unavailable"),
        value.owner.as_deref().unwrap_or("unavailable"),
        viewport,
        metrics,
        bool_label(value.pointer_blocked),
        safe_text(value.block_reason.as_deref(), "-"),
        safe_text(value.route_summary.as_deref(), "-"),
        safe_text(value.blocking_reason.as_deref(), "-"),
        value.focus_panel_id.as_ref().map_or("-", |id| id.as_str()),
        value.focus_node_id.as_ref().map_or("-", |id| id.as_str()),
        optional_u64(value.ui_node_count),
        optional_u64(value.visible_ui_node_count),
        optional_u64(value.text_node_count),
        optional_u64(value.panel_count),
        value.panel_kind_counts.as_ref().map_or_else(
            || "-".to_owned(),
            |counts| format!(
                "page={} hud={} floating={} modal={} blocking={}",
                counts.page, counts.hud, counts.floating, counts.modal, counts.blocking_overlay
            ),
        ),
        state.panel_filter.label(),
        if state.highlight_panels { "on" } else { "off" },
        panels.len(),
        value.panels.len(),
        if panels.is_empty() {
            "-".to_owned()
        } else {
            panels.join("\n")
        },
        value.document_id.as_ref().map_or("-", |id| id.as_str()),
        value
            .document_schema_version
            .map_or("-".to_owned(), |version| version.to_string()),
        safe_text(value.document_status.as_deref(), "-"),
        safe_text(value.document_source.as_deref(), "-"),
        safe_text(value.document_error.as_deref(), "-"),
    )
}

fn player_body(snapshot: &super::LivePreviewSnapshot) -> String {
    let Some(value) = snapshot.player.value.as_ref() else {
        return section_body("Player", snapshot.player.status, None);
    };
    let attrs = value.attributes.as_ref().map_or_else(
        || "unavailable".to_owned(),
        |attrs| {
            format!(
                "affinity[e={},f={},w={},wind={}] mastery[e={},f={},w={},wind={}]",
                attrs.affinity.earth,
                attrs.affinity.fire,
                attrs.affinity.water,
                attrs.affinity.wind,
                attrs.mastery.earth,
                attrs.mastery.fire,
                attrs.mastery.water,
                attrs.mastery.wind,
            )
        },
    );
    format!(
        "Player\nstatus={}\nidentity character={} name={} world={} selection={}\nattributes={} source={} freshness={} refreshed_ms={} push_seq={} revision={}\nposition={} direction={} movement={} authority_frame={}",
        status_label(snapshot.player.status),
        value
            .character_id
            .as_ref()
            .map_or("unavailable", |id| id.as_str()),
        safe_text(value.display_name.as_deref(), "unavailable"),
        value
            .world_id
            .as_ref()
            .map_or("unavailable", |id| id.as_str()),
        safe_text(value.selection_state.as_deref(), "unavailable"),
        attrs,
        safe_text(value.attributes_source.as_deref(), "-"),
        safe_text(value.attributes_freshness.as_deref(), "-"),
        optional_u64(value.attributes_snapshot_refreshed_at_ms),
        optional_u64(value.attributes_push_sequence),
        optional_u64(value.attributes_revision),
        vector3(value.position),
        vector3(value.direction),
        safe_text(value.movement_state.as_deref(), "unavailable"),
        optional_u64(value.authority_frame),
    )
}

fn scene_body(snapshot: &super::LivePreviewSnapshot) -> String {
    let Some(value) = snapshot.scene.value.as_ref() else {
        return section_body("Scene", snapshot.scene.status, None);
    };
    let layers = value
        .layers
        .iter()
        .map(|layer| {
            format!(
                "{} session={} state={} required={}",
                layer.id.as_str(),
                layer.session_id.as_str(),
                safe_text(layer.state.as_deref(), "-"),
                bool_label(layer.required),
            )
        })
        .collect::<Vec<_>>();
    format!(
        "Scene\nstatus={} active={} session={} pending={} pending_session={} ready={} ready_session={}\nlifecycle={} scene_status={} loading={} policy={} required={}/{} optional={}/{} failed={} message={} authority={} version={} seed={}\nlayers count={} ids={} entities={} roots={} runtime_roots={} recent_error={} adapter={}\n{}",
        status_label(snapshot.scene.status),
        value.active_scene_id.as_ref().map_or("-", |id| id.as_str()),
        value
            .active_session_id
            .as_ref()
            .map_or("-", |id| id.as_str()),
        value
            .pending_scene_id
            .as_ref()
            .map_or("-", |id| id.as_str()),
        value
            .pending_session_id
            .as_ref()
            .map_or("-", |id| id.as_str()),
        value.ready_scene_id.as_ref().map_or("-", |id| id.as_str()),
        value
            .ready_session_id
            .as_ref()
            .map_or("-", |id| id.as_str()),
        safe_text(value.lifecycle.as_deref(), "-"),
        safe_text(value.scene_status.as_deref(), "-"),
        safe_text(value.loading_phase.as_deref(), "-"),
        safe_text(value.loading_policy.as_deref(), "-"),
        optional_u64(value.required_loaded),
        optional_u64(value.required_total),
        optional_u64(value.optional_loaded),
        optional_u64(value.optional_total),
        optional_u64(value.optional_failed),
        safe_text(value.loading_message_key.as_deref(), "-"),
        safe_text(value.authority_mode.as_deref(), "-"),
        safe_text(value.content_version.as_deref(), "-"),
        optional_u64(value.seed),
        optional_u64(value.layer_count),
        if value.layer_ids.is_empty() {
            "-".to_owned()
        } else {
            value
                .layer_ids
                .iter()
                .map(|id| id.as_str())
                .collect::<Vec<_>>()
                .join(",")
        },
        optional_u64(value.scene_entity_count),
        optional_u64(value.scene_root_count),
        optional_u64(value.runtime_root_count),
        safe_text(value.recent_error.as_deref(), "-"),
        safe_text(value.adapter_summary.as_deref(), "-"),
        if layers.is_empty() {
            "layers: -".to_owned()
        } else {
            format!("layers:\n{}", layers.join("\n"))
        },
    )
}

fn network_body(snapshot: &super::LivePreviewSnapshot) -> String {
    let Some(value) = snapshot.network.value.as_ref() else {
        return section_body("Network", snapshot.network.status, None);
    };
    format!(
        "Network\nstatus={} session={} login={} registration={} character_selection={}\nconnection={} connected={} authenticated={} transport={} room={} pending={} reconnecting={} phase={}\nendpoint kind={} environment={} detail=redacted\nreceive last_success_ms={} error_category={}\nauthority endpoint={} role={} epoch={} frame={} activity_age_ms={} sync_health={}",
        status_label(snapshot.network.status),
        value.session_status.as_deref().unwrap_or("-"),
        value.login_status.as_deref().unwrap_or("-"),
        value.registration_status.as_deref().unwrap_or("-"),
        value.character_selection_status.as_deref().unwrap_or("-"),
        value.connection_state.as_deref().unwrap_or("unavailable"),
        bool_label(value.connected),
        bool_label(value.authenticated),
        value.transport.as_deref().unwrap_or("-"),
        value.room_id.as_ref().map_or("-", |id| id.as_str()),
        optional_u64(value.pending_request_count.map(u64::from)),
        bool_label(value.reconnecting),
        value.reconnect_phase.as_deref().unwrap_or("-"),
        value.endpoint_kind.as_deref().unwrap_or("-"),
        value.endpoint_environment.as_deref().unwrap_or("-"),
        optional_u64(value.last_successful_receive_ms),
        value.last_error_category.as_deref().unwrap_or("-"),
        value.authority_endpoint_kind.as_deref().unwrap_or("-"),
        value.authority_role.as_deref().unwrap_or("-"),
        optional_u64(value.authority_epoch),
        optional_u64(value.authority_frame),
        optional_u64(value.authority_last_activity_age_ms),
        value.authority_sync_health.as_deref().unwrap_or("-"),
    )
}

fn performance_body(snapshot: &super::LivePreviewSnapshot) -> String {
    let value = snapshot.performance.value.as_ref();
    let stale = snapshot
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
                        super::PreviewDataStatus::Unavailable | super::PreviewDataStatus::Failed
                    )
                })
                .count()
        })
        .unwrap_or(0);
    format!(
        "Performance\nstatus={} fps={} frame_time_ms={} collector_time_us={}\ncounts ui_nodes={} scene_entities={} timeline_entries={} timeline_capacity={}\nsections ui={}#{} player={}#{} scene={}#{} network={}#{} performance={}#{} source_health={}#{}\nsource stale_or_failed={} timeline_history=fixed_capacity",
        status_label(snapshot.performance.status),
        value.map_or("-".to_owned(), |value| format_number(value.fps)),
        value.map_or("-".to_owned(), |value| format_number(value.frame_time_ms)),
        value.map_or("-".to_owned(), |value| optional_u64(
            value.collector_time_us
        )),
        value.map_or("-".to_owned(), |value| optional_u64(value.ui_node_count)),
        value.map_or("-".to_owned(), |value| optional_u64(
            value.scene_entity_count
        )),
        value.map_or("-".to_owned(), |value| optional_u64(
            value.timeline_entry_count
        )),
        snapshot.timeline.capacity,
        status_label(snapshot.ui.status),
        optional_u64(snapshot.ui.revision),
        status_label(snapshot.player.status),
        optional_u64(snapshot.player.revision),
        status_label(snapshot.scene.status),
        optional_u64(snapshot.scene.revision),
        status_label(snapshot.network.status),
        optional_u64(snapshot.network.revision),
        status_label(snapshot.performance.status),
        optional_u64(snapshot.performance.revision),
        status_label(snapshot.source_health.status),
        optional_u64(snapshot.source_health.revision),
        stale,
    )
}

fn timeline_body(snapshot: &super::LivePreviewSnapshot, state: &LivePreviewMonitorState) -> String {
    let events = filtered_timeline_events(
        &snapshot.timeline.events,
        state.timeline_severity_filter.label(),
        state.timeline_domain_filter.label(),
    );
    let lines = events
        .iter()
        .map(|event| {
            format!(
                "{}ms seq={} [{} / {}] x{} {}{}",
                event.timestamp_ms,
                event.snapshot_sequence,
                event.severity,
                event.event_type,
                event.repeat_count,
                super::redaction::redacted_text(&event.summary),
                event.detail.as_ref().map_or_else(String::new, |detail| {
                    format!(" ({})", super::redaction::redacted_text(detail))
                }),
            )
        })
        .collect::<Vec<_>>();
    format!(
        "Timeline\nseverity={} domain={}\ncapacity={} entries={} filtered={}{}\n{}",
        state.timeline_severity_filter.label(),
        state.timeline_domain_filter.label(),
        snapshot.timeline.capacity,
        snapshot.timeline.events.len(),
        events.len(),
        if (snapshot.timeline.events.len() as u64) >= snapshot.timeline.capacity
            && snapshot.timeline.capacity > 0
        {
            " fixed-capacity: oldest entries evicted"
        } else {
            ""
        },
        if lines.is_empty() {
            "history: empty".to_owned()
        } else {
            lines.join("\n")
        },
    )
}

fn filtered_timeline_events<'a>(
    events: &'a [super::LivePreviewTimelineEventPreview],
    severity: &str,
    domain: &str,
) -> Vec<&'a super::LivePreviewTimelineEventPreview> {
    events
        .iter()
        .filter(|event| {
            (severity == "all" || event.severity == severity)
                && (domain == "all" || event.event_type == domain)
        })
        .collect()
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
        safe_text(
            ui.and_then(|value| value.canonical_screen.as_deref()),
            "unavailable"
        ),
        safe_text(ui.and_then(|value| value.owner.as_deref()), "unavailable"),
        safe_text(
            player.and_then(|value| value.display_name.as_deref()),
            "unavailable"
        ),
        safe_text(
            network.and_then(|value| value.room_id.as_ref().map(|id| id.as_str())),
            "unavailable",
        ),
        safe_text(
            scene.and_then(|value| value.active_scene_id.as_ref().map(|id| id.as_str())),
            "unavailable",
        ),
        safe_text(
            network.and_then(|value| value.connection_state.as_deref()),
            "unavailable",
        ),
        safe_text(
            network.and_then(|value| value.authority_endpoint_kind.as_deref()),
            "unavailable",
        ),
        safe_text(
            network.and_then(|value| value.authority_sync_health.as_deref()),
            "unavailable",
        ),
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

fn status_label(status: super::PreviewDataStatus) -> String {
    format!("{status:?}").to_ascii_lowercase()
}

fn safe_text(value: Option<&str>, fallback: &str) -> String {
    value.map_or_else(|| fallback.to_owned(), super::redaction::redacted_text)
}

fn bool_label(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "yes",
        Some(false) => "no",
        None => "unavailable",
    }
}

fn optional_u64(value: Option<u64>) -> String {
    value.map_or_else(|| "-".to_owned(), |value| value.to_string())
}

fn format_float(value: f32) -> String {
    if value.is_finite() {
        format!("{value:.1}")
    } else {
        "-".to_owned()
    }
}

fn vector3(value: Option<[f32; 3]>) -> String {
    value.map_or_else(
        || "unavailable".to_owned(),
        |value| {
            format!(
                "[{},{},{}]",
                format_float(value[0]),
                format_float(value[1]),
                format_float(value[2])
            )
        },
    )
}

fn format_number(value: Option<f32>) -> String {
    value.map_or_else(|| "-".to_owned(), format_float)
}

fn truncate_text(mut value: String) -> String {
    if value.chars().count() > MONITOR_MAX_TEXT_CHARS {
        value = value.chars().take(MONITOR_MAX_TEXT_CHARS).collect();
        value.push_str("\n... truncated ...");
    }
    value
}

fn redacted_export(snapshot: &super::LivePreviewSnapshot) -> String {
    super::redaction::redacted_json(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::devtools::live_preview::{
        LivePreviewSnapshot, LivePreviewTimelineEventPreview, NetworkPreviewState, PreviewFailure,
        PreviewSection, StablePreviewId,
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
    fn f5_f6_only_change_ui_panel_state() {
        let mut state = LivePreviewMonitorState {
            tab: LivePreviewMonitorTab::Network,
            ..Default::default()
        };
        apply_ui_shortcut(&mut state, KeyCode::F5);
        apply_ui_shortcut(&mut state, KeyCode::F6);
        assert_eq!(state.panel_filter, LivePreviewMonitorPanelFilter::All);
        assert!(!state.highlight_panels);

        state.tab = LivePreviewMonitorTab::Ui;
        apply_ui_shortcut(&mut state, KeyCode::F5);
        apply_ui_shortcut(&mut state, KeyCode::F6);
        assert_eq!(
            state.panel_filter,
            LivePreviewMonitorPanelFilter::ActiveOnly
        );
        assert!(state.highlight_panels);
    }

    #[test]
    fn timeline_filters_cycle_without_using_ui_shortcuts() {
        let mut state = LivePreviewMonitorState {
            tab: LivePreviewMonitorTab::Timeline,
            ..Default::default()
        };
        apply_timeline_shortcut(&mut state, KeyCode::PageUp);
        apply_timeline_shortcut(&mut state, KeyCode::PageDown);
        assert_eq!(
            state.timeline_severity_filter,
            LivePreviewMonitorTimelineSeverityFilter::Info
        );
        assert_eq!(
            state.timeline_domain_filter,
            LivePreviewMonitorTimelineDomainFilter::Ui
        );
    }

    #[test]
    fn tab_scroll_offsets_restore_independently() {
        let mut state = LivePreviewMonitorState {
            tab: LivePreviewMonitorTab::Ui,
            scroll_offsets: [0.0, 12.0, 34.0, 0.0, 0.0, 0.0, 0.0],
            ..Default::default()
        };
        assert_eq!(
            sync_scroll_offset(&mut state, LivePreviewMonitorTab::Ui, 99.0, true),
            12.0
        );
        state.tab = LivePreviewMonitorTab::Player;
        assert_eq!(
            sync_scroll_offset(&mut state, LivePreviewMonitorTab::Player, 0.0, true),
            34.0
        );
        assert_eq!(
            state.scroll_offsets[tab_index(LivePreviewMonitorTab::Ui)],
            12.0
        );
        assert_eq!(
            state.scroll_offsets[tab_index(LivePreviewMonitorTab::Player)],
            34.0
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

    #[test]
    fn timeline_presenter_filters_by_severity_and_domain_in_stable_order() {
        let events = vec![
            LivePreviewTimelineEventPreview {
                event_type: "network".to_owned(),
                severity: "warning".to_owned(),
                timestamp_ms: 2,
                summary: "reconnecting".to_owned(),
                ..Default::default()
            },
            LivePreviewTimelineEventPreview {
                event_type: "ui".to_owned(),
                severity: "info".to_owned(),
                timestamp_ms: 1,
                summary: "screen changed".to_owned(),
                ..Default::default()
            },
        ];
        let filtered = filtered_timeline_events(&events, "warning", "network");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].summary, "reconnecting");
        let mut snapshot = LivePreviewSnapshot::default();
        snapshot.timeline.capacity = 1;
        snapshot.timeline.events = events;
        let body = timeline_body(
            &snapshot,
            &LivePreviewMonitorState {
                timeline_severity_filter: LivePreviewMonitorTimelineSeverityFilter::Warning,
                timeline_domain_filter: LivePreviewMonitorTimelineDomainFilter::Network,
                ..Default::default()
            },
        );
        assert!(body.contains("severity=warning domain=network"));
        assert!(body.contains("fixed-capacity"));
    }

    #[test]
    fn presenters_fail_closed_and_bound_long_text() {
        let mut snapshot = LivePreviewSnapshot::default();
        snapshot.ui = PreviewSection::failed(PreviewFailure::new("failed", "diagnostic"));
        let state = LivePreviewMonitorState::default();
        assert!(
            monitor_body(
                &snapshot,
                &LivePreviewMonitorState {
                    tab: LivePreviewMonitorTab::Ui,
                    ..state.clone()
                }
            )
            .contains("status=failed")
        );
        let long = "x".repeat(MONITOR_MAX_TEXT_CHARS * 2);
        snapshot
            .timeline
            .events
            .push(LivePreviewTimelineEventPreview {
                summary: long,
                ..Default::default()
            });
        let timeline_state = LivePreviewMonitorState {
            tab: LivePreviewMonitorTab::Timeline,
            ..state
        };
        assert!(
            monitor_body(&snapshot, &timeline_state).chars().count() <= MONITOR_MAX_TEXT_CHARS + 32
        );
    }
}
