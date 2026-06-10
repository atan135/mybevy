use bevy::{
    camera::{RenderTarget, visibility::RenderLayers},
    picking::Pickable,
    prelude::*,
    window::{WindowRef, WindowResolution},
};

use crate::game::ui::{
    core::{
        UiInputState, UiInputSystems, UiLayer, UiLayerRoot, UiPanelId, UiPanelKind, UiPanelRoot,
        UiPanelSystems, focus::UiFocusState,
    },
    style::{
        UiFontAssets, UiTheme,
        theme::{
            UiThemeBackgroundRole, UiThemeBorderRole, UiThemePanelNodeRole, UiThemeRootNodeRole,
            UiThemeTextColorRole, UiThemeTextStyleRole,
        },
    },
    widgets::{screen_label, screen_title},
};

const UI_DEBUG_TARGET_ENV: &str = "MYBEVY_UI_DEBUG_TARGET";
const UI_DEBUG_RENDER_LAYER: usize = 31;

pub(in crate::game) struct UiDebugPlugin;

impl Plugin for UiDebugPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiDebugState>().add_systems(
            Update,
            (
                handle_ui_debug_keys,
                sync_ui_debug_panel,
                refresh_ui_debug_text.after(UiInputSystems::Update),
                sync_ui_debug_panel_highlights,
            )
                .chain()
                .after(UiPanelSystems::Commands),
        );
    }
}

#[derive(Debug, Resource)]
struct UiDebugState {
    enabled: bool,
    root: Option<Entity>,
    target: UiDebugDisplayTarget,
    window: Option<Entity>,
    camera: Option<Entity>,
    frozen: bool,
    panel_filter: UiDebugPanelFilter,
    highlight_panels: bool,
    frozen_body: Option<String>,
}

impl Default for UiDebugState {
    fn default() -> Self {
        Self {
            enabled: false,
            root: None,
            target: initial_ui_debug_display_target(),
            window: None,
            camera: None,
            frozen: false,
            panel_filter: UiDebugPanelFilter::default(),
            highlight_panels: false,
            frozen_body: None,
        }
    }
}

#[derive(Component)]
struct UiDebugRoot;

#[derive(Component)]
struct UiDebugWindow;

#[derive(Component)]
struct UiDebugCamera;

#[derive(Component)]
struct UiDebugText;

#[derive(Component)]
struct UiDebugPanelHighlight {
    original_border: Option<BorderColor>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum UiDebugPanelFilter {
    #[default]
    All,
    ActivePanelsOnly,
    BlockingPanelsOnly,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum UiDebugDisplayTarget {
    #[default]
    GameWindow,
    DedicatedWindow,
}

impl UiDebugDisplayTarget {
    fn label(self) -> &'static str {
        match self {
            Self::GameWindow => "game window",
            Self::DedicatedWindow => "debug window",
        }
    }

    fn next_supported(self) -> Self {
        match self {
            Self::GameWindow if supports_dedicated_debug_window() => Self::DedicatedWindow,
            _ => Self::GameWindow,
        }
    }
}

impl UiDebugPanelFilter {
    fn next(self) -> Self {
        match self {
            Self::All => Self::ActivePanelsOnly,
            Self::ActivePanelsOnly => Self::BlockingPanelsOnly,
            Self::BlockingPanelsOnly => Self::All,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::ActivePanelsOnly => "active panels only",
            Self::BlockingPanelsOnly => "blocking panels only",
        }
    }
}

fn handle_ui_debug_keys(
    key_codes: Res<ButtonInput<KeyCode>>,
    mut debug_state: ResMut<UiDebugState>,
) {
    if key_codes.just_pressed(KeyCode::F3) {
        debug_state.enabled = !debug_state.enabled;
    }

    if key_codes.just_pressed(KeyCode::F4) {
        debug_state.frozen = !debug_state.frozen;
    }

    if key_codes.just_pressed(KeyCode::F5) {
        debug_state.panel_filter = debug_state.panel_filter.next();
    }

    if key_codes.just_pressed(KeyCode::F6) {
        debug_state.highlight_panels = !debug_state.highlight_panels;
    }

    if key_codes.just_pressed(KeyCode::F7) {
        let next_target = debug_state.target.next_supported();
        if debug_state.target != next_target {
            debug_state.target = next_target;
            debug_state.root = None;
        }
    }
}

fn sync_ui_debug_panel(
    mut commands: Commands,
    theme: Res<UiTheme>,
    fonts: Res<UiFontAssets>,
    mut debug_state: ResMut<UiDebugState>,
    debug_roots: Query<Entity, With<UiDebugRoot>>,
    debug_windows: Query<Entity, With<UiDebugWindow>>,
    debug_cameras: Query<Entity, With<UiDebugCamera>>,
) {
    debug_state.target = normalize_ui_debug_display_target(debug_state.target);

    if let Some(root) = debug_state.root
        && !debug_roots.contains(root)
    {
        debug_state.root = None;
    }

    if let Some(window) = debug_state.window
        && !debug_windows.contains(window)
    {
        cleanup_ui_debug_roots(&mut commands, &debug_roots);
        cleanup_ui_debug_dedicated_target(&mut commands, &debug_windows, &debug_cameras);
        debug_state.enabled = false;
        debug_state.root = None;
        debug_state.window = None;
        debug_state.camera = None;
        return;
    }

    if let Some(camera) = debug_state.camera
        && !debug_cameras.contains(camera)
    {
        debug_state.camera = None;
        debug_state.root = None;
    }

    if !debug_state.enabled {
        cleanup_ui_debug_roots(&mut commands, &debug_roots);
        cleanup_ui_debug_dedicated_target(&mut commands, &debug_windows, &debug_cameras);
        debug_state.root = None;
        debug_state.window = None;
        debug_state.camera = None;
        return;
    }

    if debug_state.root.is_none() {
        cleanup_ui_debug_roots(&mut commands, &debug_roots);
    }

    let target_camera = match debug_state.target {
        UiDebugDisplayTarget::GameWindow => {
            cleanup_ui_debug_dedicated_target(&mut commands, &debug_windows, &debug_cameras);
            debug_state.window = None;
            debug_state.camera = None;
            None
        }
        UiDebugDisplayTarget::DedicatedWindow => {
            if debug_state.window.is_none() || debug_state.camera.is_none() {
                cleanup_ui_debug_dedicated_target(&mut commands, &debug_windows, &debug_cameras);
                let (window, camera) = spawn_ui_debug_window(&mut commands);
                debug_state.window = Some(window);
                debug_state.camera = Some(camera);
                debug_state.root = None;
            }

            debug_state.camera
        }
    };

    if debug_state.root.is_none() {
        debug_state.root = Some(spawn_ui_debug_panel(
            &mut commands,
            &theme,
            &fonts,
            debug_state.target,
            target_camera,
        ));
    }
}

fn refresh_ui_debug_text(
    mut debug_state: ResMut<UiDebugState>,
    input_state: Res<UiInputState>,
    focus_state: Res<UiFocusState>,
    panels: Query<(
        Entity,
        &UiPanelRoot,
        Option<&Visibility>,
        Option<&InheritedVisibility>,
    )>,
    mut texts: Query<&mut Text, With<UiDebugText>>,
) {
    let Ok(mut text) = texts.single_mut() else {
        return;
    };

    let header = ui_debug_header_lines(&debug_state);
    let body = if debug_state.frozen {
        debug_state.frozen_body.clone().unwrap_or_else(|| {
            build_ui_debug_body(&debug_state, &input_state, &focus_state, &panels)
        })
    } else {
        let body = build_ui_debug_body(&debug_state, &input_state, &focus_state, &panels);
        debug_state.frozen_body = Some(body.clone());
        body
    };

    let display = if body.is_empty() {
        header.join("\n")
    } else {
        format!("{}\n{}", header.join("\n"), body)
    };

    if text.0 != display {
        text.0 = display;
    }
}

fn ui_debug_header_lines(debug_state: &UiDebugState) -> Vec<String> {
    vec![
        format!(
            "debug: target={} freeze={} filter={} highlight={}",
            debug_state.target.label(),
            on_off_label(debug_state.frozen),
            debug_state.panel_filter.label(),
            on_off_label(debug_state.highlight_panels),
        ),
        "keys: F3 toggle panel | F4 freeze | F5 filter | F6 highlight | F7 target".to_string(),
        String::new(),
    ]
}

fn build_ui_debug_body(
    debug_state: &UiDebugState,
    input_state: &UiInputState,
    focus_state: &UiFocusState,
    panels: &Query<(
        Entity,
        &UiPanelRoot,
        Option<&Visibility>,
        Option<&InheritedVisibility>,
    )>,
) -> String {
    let mut lines = vec![
        format!("pointer_blocked: {}", input_state.pointer_blocked),
        format!("block_reason: {}", input_state.pointer_block_reason),
        format!("route_summary: {}", input_state.route_summary),
        format!("focused_panel: {:?}", input_state.focused_panel),
        format!("top_blocking_panel: {:?}", input_state.top_blocking_panel),
        format!("focused_entity: {:?}", focus_state.focused_entity),
        "route history:".to_string(),
    ];

    if input_state.route_history.is_empty() {
        lines.push("  none".to_string());
    } else {
        for entry in &input_state.route_history {
            lines.push(format!("  #{:03} {}", entry.id, entry.summary));
        }
    }

    lines.extend(["panels:".to_string()]);

    let mut panel_entries = panels.iter().collect::<Vec<_>>();
    panel_entries.sort_by_key(|(entity, panel, _, _)| {
        (
            panel_kind_order(panel.kind),
            panel_id_order(panel.id),
            *entity,
        )
    });

    let panel_entries = panel_entries
        .into_iter()
        .filter(|(_, panel, visibility, inherited_visibility)| {
            panel_matches_filter(
                debug_state.panel_filter,
                panel.kind,
                is_panel_active(*visibility, *inherited_visibility),
            )
        })
        .collect::<Vec<_>>();

    if panel_entries.is_empty() {
        lines.push("  none".to_string());
    } else {
        for (entity, panel, visibility, _) in panel_entries {
            lines.push(format!(
                "  {:?} {:?} owner={:?} visible={} entity={:?}",
                panel.id,
                panel.kind,
                panel.owner_mode,
                visibility_label(visibility),
                entity,
            ));
        }
    }

    lines.join("\n")
}

fn sync_ui_debug_panel_highlights(
    mut commands: Commands,
    debug_state: Res<UiDebugState>,
    mut panels: Query<(
        Entity,
        &UiPanelRoot,
        Option<&Visibility>,
        Option<&InheritedVisibility>,
        Option<&mut BorderColor>,
        Option<&mut UiDebugPanelHighlight>,
    )>,
) {
    if !debug_state.enabled && !debug_state.highlight_panels {
        return;
    }

    for (entity, _panel, visibility, inherited_visibility, border_color, highlight) in &mut panels {
        let should_highlight = debug_state.enabled
            && debug_state.highlight_panels
            && is_panel_active(visibility, inherited_visibility);

        match (should_highlight, border_color, highlight) {
            (true, Some(mut border_color), Some(mut highlight)) => {
                if *border_color != ui_debug_highlight_border_color() {
                    highlight.original_border = Some(*border_color);
                }
                *border_color = ui_debug_highlight_border_color();
            }
            (true, Some(mut border_color), None) => {
                let original_border = *border_color;
                *border_color = ui_debug_highlight_border_color();
                commands.entity(entity).insert(UiDebugPanelHighlight {
                    original_border: Some(original_border),
                });
            }
            (true, None, None) => {
                commands.entity(entity).insert((
                    ui_debug_highlight_border_color(),
                    UiDebugPanelHighlight {
                        original_border: None,
                    },
                ));
            }
            (true, None, Some(_)) => {
                commands
                    .entity(entity)
                    .insert(ui_debug_highlight_border_color());
            }
            (false, Some(mut border_color), Some(highlight)) => {
                if let Some(original_border) = highlight.original_border {
                    *border_color = original_border;
                } else {
                    commands.entity(entity).remove::<BorderColor>();
                }
                commands.entity(entity).remove::<UiDebugPanelHighlight>();
            }
            (false, None, Some(_)) => {
                commands.entity(entity).remove::<UiDebugPanelHighlight>();
            }
            (false, _, None) => {}
        }
    }
}

fn spawn_ui_debug_panel(
    commands: &mut Commands,
    theme: &UiTheme,
    fonts: &UiFontAssets,
    target: UiDebugDisplayTarget,
    target_camera: Option<Entity>,
) -> Entity {
    let node = ui_debug_panel_node(theme, target);

    let mut root = commands.spawn((
        UiDebugRoot,
        UiLayerRoot {
            layer: UiLayer::Debug,
        },
        UiThemeRootNodeRole::Debug,
        UiThemePanelNodeRole::Debug,
        node,
        ZIndex(250),
        BackgroundColor(theme.colors.panel_background),
        BorderColor::all(theme.colors.panel_border),
        UiThemeBackgroundRole::Panel,
        UiThemeBorderRole::Panel,
        Pickable::IGNORE,
    ));

    if let Some(target_camera) = target_camera {
        root.insert(UiTargetCamera(target_camera));
    }

    root.with_children(|root| {
        root.spawn((
            Node {
                width: percent(100),
                ..default()
            },
            screen_title(
                theme,
                fonts,
                "UI Input Debug",
                UiThemeTextStyleRole::Caption,
            ),
            Pickable::IGNORE,
        ));
        root.spawn((
            Node {
                width: percent(100),
                ..default()
            },
            screen_label(
                theme,
                fonts,
                "",
                UiThemeTextStyleRole::Caption,
                UiThemeTextColorRole::Primary,
            ),
            UiDebugText,
            Pickable::IGNORE,
        ));
    })
    .id()
}

fn ui_debug_panel_node(theme: &UiTheme, target: UiDebugDisplayTarget) -> Node {
    let overlay_padding = px(theme.layout.overlay_padding);

    let mut node = Node {
        position_type: PositionType::Absolute,
        left: overlay_padding,
        top: overlay_padding,
        flex_direction: FlexDirection::Column,
        row_gap: px(theme.layout.row_gap),
        padding: UiRect::all(px(14)),
        border: UiRect::all(px(theme.panel.border)),
        border_radius: BorderRadius::all(px(theme.panel.radius)),
        ..default()
    };

    match target {
        UiDebugDisplayTarget::GameWindow => {
            node.width = px(430);
            node.max_width = percent(94);
        }
        UiDebugDisplayTarget::DedicatedWindow => {
            node.right = overlay_padding;
            node.bottom = overlay_padding;
            node.width = auto();
            node.max_width = percent(100);
        }
    }

    node
}

fn spawn_ui_debug_window(commands: &mut Commands) -> (Entity, Entity) {
    let window = commands
        .spawn((
            UiDebugWindow,
            Window {
                title: "MyBevy UI Debug".to_string(),
                resolution: WindowResolution::new(560, 720),
                ..default()
            },
        ))
        .id();

    let camera = commands
        .spawn((
            UiDebugCamera,
            Camera2d,
            RenderLayers::layer(UI_DEBUG_RENDER_LAYER),
            RenderTarget::Window(WindowRef::Entity(window)),
        ))
        .id();

    (window, camera)
}

fn cleanup_ui_debug_roots(commands: &mut Commands, debug_roots: &Query<Entity, With<UiDebugRoot>>) {
    for root in debug_roots {
        commands.entity(root).try_despawn();
    }
}

fn cleanup_ui_debug_dedicated_target(
    commands: &mut Commands,
    debug_windows: &Query<Entity, With<UiDebugWindow>>,
    debug_cameras: &Query<Entity, With<UiDebugCamera>>,
) {
    for camera in debug_cameras {
        commands.entity(camera).try_despawn();
    }

    for window in debug_windows {
        commands.entity(window).try_despawn();
    }
}

fn initial_ui_debug_display_target() -> UiDebugDisplayTarget {
    std::env::var(UI_DEBUG_TARGET_ENV)
        .ok()
        .and_then(|value| parse_ui_debug_display_target(&value))
        .map(normalize_ui_debug_display_target)
        .unwrap_or_default()
}

fn parse_ui_debug_display_target(value: &str) -> Option<UiDebugDisplayTarget> {
    match value.trim().to_ascii_lowercase().as_str() {
        "game" | "main" | "main-window" | "game-window" | "inline" => {
            Some(UiDebugDisplayTarget::GameWindow)
        }
        "window" | "debug-window" | "dedicated" | "popout" | "external" => {
            Some(UiDebugDisplayTarget::DedicatedWindow)
        }
        _ => None,
    }
}

fn normalize_ui_debug_display_target(target: UiDebugDisplayTarget) -> UiDebugDisplayTarget {
    match target {
        UiDebugDisplayTarget::DedicatedWindow if !supports_dedicated_debug_window() => {
            UiDebugDisplayTarget::GameWindow
        }
        _ => target,
    }
}

fn supports_dedicated_debug_window() -> bool {
    !cfg!(target_os = "android")
}

fn visibility_label(visibility: Option<&Visibility>) -> &'static str {
    match visibility {
        Some(Visibility::Hidden) => "hidden",
        Some(Visibility::Inherited) => "inherited",
        Some(Visibility::Visible) | None => "visible",
    }
}

fn on_off_label(value: bool) -> &'static str {
    if value { "on" } else { "off" }
}

fn is_panel_active(
    visibility: Option<&Visibility>,
    inherited_visibility: Option<&InheritedVisibility>,
) -> bool {
    visibility.is_none_or(|visibility| *visibility != Visibility::Hidden)
        && inherited_visibility.is_none_or(|visibility| visibility.get())
}

fn panel_matches_filter(filter: UiDebugPanelFilter, kind: UiPanelKind, active: bool) -> bool {
    match filter {
        UiDebugPanelFilter::All => true,
        UiDebugPanelFilter::ActivePanelsOnly => active,
        UiDebugPanelFilter::BlockingPanelsOnly => {
            active && matches!(kind, UiPanelKind::Modal | UiPanelKind::BlockingOverlay)
        }
    }
}

fn ui_debug_highlight_border_color() -> BorderColor {
    BorderColor::all(Color::srgb(1.0, 0.82, 0.16))
}

fn panel_kind_order(kind: UiPanelKind) -> u8 {
    match kind {
        UiPanelKind::Page => 0,
        UiPanelKind::Hud => 1,
        UiPanelKind::Floating => 2,
        UiPanelKind::Modal => 3,
        UiPanelKind::BlockingOverlay => 4,
    }
}

fn panel_id_order(id: UiPanelId) -> u8 {
    match id {
        UiPanelId::LoginPage => 0,
        UiPanelId::GameListPage => 1,
        UiPanelId::UiGalleryPage => 2,
        UiPanelId::GalleryFloating => 3,
        UiPanelId::TouchRippleHud => 4,
        UiPanelId::TouchRipplePause => 5,
        UiPanelId::TouchRippleSettings => 6,
        UiPanelId::GlobalLoading => 7,
        UiPanelId::ConfirmModal => 8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_panel_filter_cycles_through_modes() {
        assert_eq!(
            UiDebugPanelFilter::All.next(),
            UiDebugPanelFilter::ActivePanelsOnly
        );
        assert_eq!(
            UiDebugPanelFilter::ActivePanelsOnly.next(),
            UiDebugPanelFilter::BlockingPanelsOnly
        );
        assert_eq!(
            UiDebugPanelFilter::BlockingPanelsOnly.next(),
            UiDebugPanelFilter::All
        );
    }

    #[test]
    fn debug_panel_filter_matches_expected_panel_sets() {
        assert!(panel_matches_filter(
            UiDebugPanelFilter::All,
            UiPanelKind::Hud,
            false
        ));
        assert!(!panel_matches_filter(
            UiDebugPanelFilter::ActivePanelsOnly,
            UiPanelKind::Hud,
            false
        ));
        assert!(panel_matches_filter(
            UiDebugPanelFilter::ActivePanelsOnly,
            UiPanelKind::Hud,
            true
        ));
        assert!(panel_matches_filter(
            UiDebugPanelFilter::BlockingPanelsOnly,
            UiPanelKind::Modal,
            true
        ));
        assert!(panel_matches_filter(
            UiDebugPanelFilter::BlockingPanelsOnly,
            UiPanelKind::BlockingOverlay,
            true
        ));
        assert!(!panel_matches_filter(
            UiDebugPanelFilter::BlockingPanelsOnly,
            UiPanelKind::Modal,
            false
        ));
        assert!(!panel_matches_filter(
            UiDebugPanelFilter::BlockingPanelsOnly,
            UiPanelKind::Floating,
            true
        ));
    }

    #[test]
    fn debug_highlight_border_is_uniform() {
        let border = ui_debug_highlight_border_color();

        assert_eq!(border.top, border.right);
        assert_eq!(border.right, border.bottom);
        assert_eq!(border.bottom, border.left);
    }

    #[test]
    fn debug_display_target_parses_known_values() {
        assert_eq!(
            parse_ui_debug_display_target("game"),
            Some(UiDebugDisplayTarget::GameWindow)
        );
        assert_eq!(
            parse_ui_debug_display_target("main-window"),
            Some(UiDebugDisplayTarget::GameWindow)
        );
        assert_eq!(
            parse_ui_debug_display_target("window"),
            Some(UiDebugDisplayTarget::DedicatedWindow)
        );
        assert_eq!(
            parse_ui_debug_display_target("popout"),
            Some(UiDebugDisplayTarget::DedicatedWindow)
        );
        assert_eq!(parse_ui_debug_display_target("unknown"), None);
    }

    #[test]
    fn debug_display_target_cycles_to_supported_mode() {
        let next = UiDebugDisplayTarget::GameWindow.next_supported();

        if supports_dedicated_debug_window() {
            assert_eq!(next, UiDebugDisplayTarget::DedicatedWindow);
        } else {
            assert_eq!(next, UiDebugDisplayTarget::GameWindow);
        }

        assert_eq!(
            UiDebugDisplayTarget::DedicatedWindow.next_supported(),
            UiDebugDisplayTarget::GameWindow
        );
    }

    #[test]
    fn debug_panel_node_uses_wide_layout_for_dedicated_window() {
        let theme = UiTheme::default();
        let game_node = ui_debug_panel_node(&theme, UiDebugDisplayTarget::GameWindow);
        let window_node = ui_debug_panel_node(&theme, UiDebugDisplayTarget::DedicatedWindow);

        assert_eq!(game_node.width, px(430));
        assert_eq!(game_node.right, Val::Auto);
        assert_eq!(window_node.width, Val::Auto);
        assert_eq!(window_node.right, px(theme.layout.overlay_padding));
        assert_eq!(window_node.bottom, px(theme.layout.overlay_padding));
    }
}
