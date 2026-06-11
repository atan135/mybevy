use bevy::{
    camera::{RenderTarget, visibility::RenderLayers},
    picking::Pickable,
    prelude::*,
    window::{WindowRef, WindowResolution},
};

use crate::game::ui::{
    core::{
        UiInputState, UiInputSystems, UiLayer, UiLayerRoot, UiPanelId, UiPanelKind, UiPanelRoot,
        UiPanelSystems, focus::UiFocusState, stats::UiStats,
    },
    style::{
        UiFontAssets, UiTheme,
        theme::{
            UiThemeBackgroundRole, UiThemeBorderRole, UiThemePanelNodeRole, UiThemeRootNodeRole,
            UiThemeTextColorRole, UiThemeTextStyleRole,
        },
    },
    widgets::{UiScrollView, screen_label, screen_title},
};

const UI_DEBUG_TARGET_ENV: &str = "MYBEVY_UI_DEBUG_TARGET";
const UI_DEBUG_RENDER_LAYER: usize = 31;
const UI_DEBUG_TREE_MAX_LINES: usize = 24;
const UI_DEBUG_LAYOUT_BOUNDS_MAX_LINES: usize = 16;
const UI_DEBUG_COPY_LOG_PREVIEW_CHARS: usize = 8_000;
const UI_DEBUG_GAME_BODY_MAX_HEIGHT: f32 = 560.0;

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
    copy_requested: bool,
    last_display_text: Option<String>,
    last_copied_text: Option<String>,
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
            copy_requested: false,
            last_display_text: None,
            last_copied_text: None,
        }
    }
}

#[derive(Component)]
struct UiDebugRoot;

#[derive(Component)]
struct UiDebugNode;

#[derive(Component)]
struct UiDebugWindow;

#[derive(Component)]
struct UiDebugCamera;

#[derive(Component)]
struct UiDebugText;

#[derive(Component)]
struct UiDebugHeaderText;

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

    if key_codes.just_pressed(KeyCode::F8) {
        debug_state.copy_requested = true;
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
    stats: Res<UiStats>,
    panels: Query<(
        Entity,
        &UiPanelRoot,
        Option<&Visibility>,
        Option<&InheritedVisibility>,
        Option<&ZIndex>,
    )>,
    ui_nodes: Query<
        (
            Entity,
            Option<&Name>,
            Option<&ChildOf>,
            Option<&UiLayerRoot>,
            Option<&UiPanelRoot>,
            Option<&Visibility>,
            Option<&InheritedVisibility>,
        ),
        With<Node>,
    >,
    layout_nodes: Query<
        (
            Entity,
            Option<&Name>,
            &ComputedNode,
            &UiGlobalTransform,
            Option<&UiLayerRoot>,
            Option<&UiPanelRoot>,
            Option<&Visibility>,
            Option<&InheritedVisibility>,
        ),
        (With<Node>, Without<UiDebugNode>),
    >,
    mut texts: ParamSet<(
        Query<&mut Text, With<UiDebugHeaderText>>,
        Query<&mut Text, With<UiDebugText>>,
    )>,
) {
    let (header, body, display) = build_ui_debug_display_parts(&mut debug_state, |debug_state| {
        build_ui_debug_body(
            debug_state,
            &input_state,
            &focus_state,
            &stats,
            &panels,
            &ui_nodes,
            &layout_nodes,
        )
    });

    debug_state.last_display_text = Some(display.clone());

    if debug_state.copy_requested {
        copy_ui_debug_display_text(&mut debug_state, &display);
    }

    {
        let mut header_texts = texts.p0();
        if let Ok(mut text) = header_texts.single_mut() {
            let header = header.join("\n");
            if text.0 != header {
                text.0 = header;
            }
        }
    }

    {
        let mut body_texts = texts.p1();
        if let Ok(mut text) = body_texts.single_mut()
            && text.0 != body
        {
            text.0 = body;
        }
    }
}

fn build_ui_debug_display_parts<F>(
    debug_state: &mut UiDebugState,
    build_body: F,
) -> (Vec<String>, String, String)
where
    F: FnOnce(&UiDebugState) -> String,
{
    let header = ui_debug_header_lines(debug_state);
    let body = if debug_state.frozen {
        debug_state
            .frozen_body
            .clone()
            .unwrap_or_else(|| build_body(debug_state))
    } else {
        let body = build_body(debug_state);
        debug_state.frozen_body = Some(body.clone());
        body
    };

    let display = compose_ui_debug_display_text(&header, &body);
    (header, body, display)
}

fn compose_ui_debug_display_text(header: &[String], body: &str) -> String {
    if body.is_empty() {
        header.join("\n")
    } else {
        format!("{}\n{}", header.join("\n"), body)
    }
}

fn copy_ui_debug_display_text(debug_state: &mut UiDebugState, display_text: &str) {
    debug_state.last_copied_text = Some(display_text.to_string());
    debug_state.copy_requested = false;
    let preview = debug_copy_log_preview(display_text, UI_DEBUG_COPY_LOG_PREVIEW_CHARS);
    info!(
        "UI debug state copied to internal buffer ({} bytes):\n{}",
        display_text.len(),
        preview,
    );
}

fn debug_copy_log_preview(display_text: &str, max_chars: usize) -> String {
    let mut preview = display_text.chars().take(max_chars).collect::<String>();
    if display_text.chars().count() > max_chars {
        preview.push_str("\n... truncated ...");
    }
    preview
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
        "keys: F3 toggle panel | F4 freeze | F5 filter | F6 highlight | F7 target | F8 copy"
            .to_string(),
        String::new(),
    ]
}

fn build_ui_debug_body(
    debug_state: &UiDebugState,
    input_state: &UiInputState,
    focus_state: &UiFocusState,
    stats: &UiStats,
    panels: &Query<(
        Entity,
        &UiPanelRoot,
        Option<&Visibility>,
        Option<&InheritedVisibility>,
        Option<&ZIndex>,
    )>,
    ui_nodes: &Query<
        (
            Entity,
            Option<&Name>,
            Option<&ChildOf>,
            Option<&UiLayerRoot>,
            Option<&UiPanelRoot>,
            Option<&Visibility>,
            Option<&InheritedVisibility>,
        ),
        With<Node>,
    >,
    layout_nodes: &Query<
        (
            Entity,
            Option<&Name>,
            &ComputedNode,
            &UiGlobalTransform,
            Option<&UiLayerRoot>,
            Option<&UiPanelRoot>,
            Option<&Visibility>,
            Option<&InheritedVisibility>,
        ),
        (With<Node>, Without<UiDebugNode>),
    >,
) -> String {
    let mut lines = vec![
        format!("pointer_blocked: {}", input_state.pointer_blocked),
        format!("block_reason: {}", input_state.pointer_block_reason),
        format!("route_summary: {}", input_state.route_summary),
        format!("focused_panel: {:?}", input_state.focused_panel),
        format!("top_blocking_panel: {:?}", input_state.top_blocking_panel),
        format!("focused_entity: {:?}", focus_state.focused_entity),
    ];

    lines.extend(ui_stats_debug_lines(stats));
    lines.extend([String::new(), "route history:".to_string()]);

    if input_state.route_history.is_empty() {
        lines.push("  none".to_string());
    } else {
        for entry in &input_state.route_history {
            lines.push(format!("  #{:03} {}", entry.id, entry.summary));
        }
    }

    let panel_entries = collect_panel_debug_entries(panels);
    lines.push(String::new());
    lines.extend(panel_list_debug_lines(
        debug_state.panel_filter,
        &panel_entries,
    ));
    lines.push(String::new());
    lines.extend(panel_stack_debug_lines(&panel_entries));
    lines.push(String::new());
    lines.extend(ui_tree_debug_lines(
        &collect_ui_tree_debug_entries(ui_nodes),
        UI_DEBUG_TREE_MAX_LINES,
    ));
    lines.push(String::new());
    lines.extend(layout_bounds_debug_lines(
        &collect_layout_debug_entries(layout_nodes),
        UI_DEBUG_LAYOUT_BOUNDS_MAX_LINES,
    ));

    lines.join("\n")
}

fn ui_stats_debug_lines(stats: &UiStats) -> Vec<String> {
    vec![
        "ui stats:".to_string(),
        format!(
            "  nodes: total={} visible={} text={}",
            stats.ui_node_count, stats.visible_ui_node_count, stats.text_node_count,
        ),
        format!(
            "  panels: total={} page={} hud={} floating={} modal={} blocking={}",
            stats.panel_count,
            stats.panel_kind_counts.page,
            stats.panel_kind_counts.hud,
            stats.panel_kind_counts.floating,
            stats.panel_kind_counts.modal,
            stats.panel_kind_counts.blocking_overlay,
        ),
    ]
}

#[derive(Clone, Debug)]
struct UiDebugPanelEntry {
    entity: Entity,
    id: UiPanelId,
    kind: UiPanelKind,
    owner_mode: String,
    visible: &'static str,
    active: bool,
    z_index: i32,
}

#[derive(Clone, Debug)]
struct UiDebugTreeEntry {
    entity: Entity,
    name: Option<String>,
    parent: Option<Entity>,
    layer: Option<UiLayer>,
    panel_id: Option<UiPanelId>,
    panel_kind: Option<UiPanelKind>,
    visible: &'static str,
    inherited_visible: &'static str,
}

#[derive(Clone, Debug)]
struct UiDebugLayoutEntry {
    entity: Entity,
    name: Option<String>,
    size: Vec2,
    center: Vec2,
    scale: Vec2,
    rotation: f32,
    layer: Option<UiLayer>,
    panel_id: Option<UiPanelId>,
    panel_kind: Option<UiPanelKind>,
    visible: &'static str,
    inherited_visible: &'static str,
    stack_index: u32,
}

fn collect_panel_debug_entries(
    panels: &Query<(
        Entity,
        &UiPanelRoot,
        Option<&Visibility>,
        Option<&InheritedVisibility>,
        Option<&ZIndex>,
    )>,
) -> Vec<UiDebugPanelEntry> {
    let mut entries = panels
        .iter()
        .map(
            |(entity, panel, visibility, inherited_visibility, z_index)| UiDebugPanelEntry {
                entity,
                id: panel.id,
                kind: panel.kind,
                owner_mode: format!("{:?}", panel.owner_mode),
                visible: visibility_label(visibility),
                active: is_panel_active(visibility, inherited_visibility),
                z_index: z_index_value(z_index),
            },
        )
        .collect::<Vec<_>>();

    sort_panel_debug_entries(&mut entries);
    entries
}

fn sort_panel_debug_entries(entries: &mut [UiDebugPanelEntry]) {
    entries.sort_by_key(|entry| {
        (
            panel_kind_order(entry.kind),
            entry.z_index,
            panel_id_order(entry.id),
            entry.entity,
        )
    });
}

fn panel_list_debug_lines(
    filter: UiDebugPanelFilter,
    panel_entries: &[UiDebugPanelEntry],
) -> Vec<String> {
    let mut lines = vec![format!("panels ({})", filter.label())];
    let entries = panel_entries
        .iter()
        .filter(|entry| panel_matches_filter(filter, entry.kind, entry.active))
        .collect::<Vec<_>>();

    if entries.is_empty() {
        lines.push("  none".to_string());
        return lines;
    }

    for entry in entries {
        lines.push(format!(
            "  {:?} {:?} owner={} visible={} active={} z={} entity={:?}",
            entry.id,
            entry.kind,
            entry.owner_mode,
            entry.visible,
            entry.active,
            entry.z_index,
            entry.entity,
        ));
    }

    lines
}

fn panel_stack_debug_lines(panel_entries: &[UiDebugPanelEntry]) -> Vec<String> {
    let mut lines = vec!["panel stack:".to_string()];
    let entries = panel_entries
        .iter()
        .filter(|entry| entry.active)
        .collect::<Vec<_>>();

    if entries.is_empty() {
        lines.push("  none".to_string());
        return lines;
    }

    lines.push("  bottom -> top (active panels):".to_string());
    for (index, entry) in entries.iter().enumerate() {
        lines.push(format!(
            "  [{:02}] {} {:?} owner={} z={} entity={:?}",
            index,
            panel_kind_label(entry.kind),
            entry.id,
            entry.owner_mode,
            entry.z_index,
            entry.entity,
        ));
    }

    lines
}

fn collect_ui_tree_debug_entries(
    ui_nodes: &Query<
        (
            Entity,
            Option<&Name>,
            Option<&ChildOf>,
            Option<&UiLayerRoot>,
            Option<&UiPanelRoot>,
            Option<&Visibility>,
            Option<&InheritedVisibility>,
        ),
        With<Node>,
    >,
) -> Vec<UiDebugTreeEntry> {
    let mut entries = ui_nodes
        .iter()
        .filter(|(_, _, _, layer, panel, _, _)| layer.is_some() || panel.is_some())
        .map(
            |(entity, name, parent, layer, panel, visibility, inherited_visibility)| {
                UiDebugTreeEntry {
                    entity,
                    name: name.map(|name| name.as_str().to_string()),
                    parent: parent.map(ChildOf::parent),
                    layer: layer.map(|layer| layer.layer),
                    panel_id: panel.map(|panel| panel.id),
                    panel_kind: panel.map(|panel| panel.kind),
                    visible: visibility_label(visibility),
                    inherited_visible: inherited_visibility_label(inherited_visibility),
                }
            },
        )
        .collect::<Vec<_>>();

    sort_ui_tree_debug_entries(&mut entries);
    entries
}

fn sort_ui_tree_debug_entries(entries: &mut [UiDebugTreeEntry]) {
    entries.sort_by_key(|entry| {
        (
            entry.layer.map_or(u8::MAX, ui_layer_order),
            entry.panel_kind.map_or(u8::MAX, panel_kind_order),
            entry.panel_id.map_or(u8::MAX, panel_id_order),
            entry.entity,
        )
    });
}

fn ui_tree_debug_lines(entries: &[UiDebugTreeEntry], max_lines: usize) -> Vec<String> {
    let mut lines = vec!["ui tree:".to_string()];

    if entries.is_empty() {
        lines.push("  none".to_string());
        return lines;
    }

    for entry in entries.iter().take(max_lines) {
        lines.push(ui_tree_entry_line(entry));
    }

    if entries.len() > max_lines {
        lines.push(format!(
            "  ... {} more root-like UI nodes",
            entries.len() - max_lines
        ));
    }

    lines
}

fn ui_tree_entry_line(entry: &UiDebugTreeEntry) -> String {
    format!(
        "  {:?} name={} parent={} layer={} panel={} visible={}/{}",
        entry.entity,
        entry.name.as_deref().unwrap_or("-"),
        option_entity_label(entry.parent),
        entry.layer.map_or("-", ui_layer_label),
        panel_tree_label(entry.panel_kind, entry.panel_id),
        entry.visible,
        entry.inherited_visible,
    )
}

fn collect_layout_debug_entries(
    layout_nodes: &Query<
        (
            Entity,
            Option<&Name>,
            &ComputedNode,
            &UiGlobalTransform,
            Option<&UiLayerRoot>,
            Option<&UiPanelRoot>,
            Option<&Visibility>,
            Option<&InheritedVisibility>,
        ),
        (With<Node>, Without<UiDebugNode>),
    >,
) -> Vec<UiDebugLayoutEntry> {
    let mut entries = layout_nodes
        .iter()
        .map(
            |(
                entity,
                name,
                computed,
                transform,
                layer,
                panel,
                visibility,
                inherited_visibility,
            )| {
                let (scale, rotation, center) = transform.to_scale_angle_translation();

                UiDebugLayoutEntry {
                    entity,
                    name: name.map(|name| name.as_str().to_string()),
                    size: computed.size(),
                    center,
                    scale,
                    rotation,
                    layer: layer.map(|layer| layer.layer),
                    panel_id: panel.map(|panel| panel.id),
                    panel_kind: panel.map(|panel| panel.kind),
                    visible: visibility_label(visibility),
                    inherited_visible: inherited_visibility_label(inherited_visibility),
                    stack_index: computed.stack_index(),
                }
            },
        )
        .collect::<Vec<_>>();

    sort_layout_debug_entries(&mut entries);
    entries
}

fn sort_layout_debug_entries(entries: &mut [UiDebugLayoutEntry]) {
    entries.sort_by_key(|entry| {
        (
            layout_entry_kind_order(entry),
            entry.layer.map_or(u8::MAX, ui_layer_order),
            entry.panel_kind.map_or(u8::MAX, panel_kind_order),
            entry.panel_id.map_or(u8::MAX, panel_id_order),
            entry.stack_index,
            entry.entity,
        )
    });
}

fn layout_bounds_debug_lines(entries: &[UiDebugLayoutEntry], max_lines: usize) -> Vec<String> {
    let mut lines = vec!["layout bounds:".to_string()];

    if entries.is_empty() {
        lines.push("  none".to_string());
        return lines;
    }

    for entry in entries.iter().take(max_lines) {
        lines.push(layout_bounds_entry_line(entry));
    }

    if entries.len() > max_lines {
        lines.push(format!("  ... {} more UI nodes", entries.len() - max_lines));
    }

    lines
}

fn layout_bounds_entry_line(entry: &UiDebugLayoutEntry) -> String {
    let top_left = entry.center - entry.size * 0.5;

    format!(
        "  {:?} name={} size={} top_left={} center={} scale={} rot={:.2} visible={}/{} layer={} panel={} stack={}",
        entry.entity,
        entry.name.as_deref().unwrap_or("-"),
        format_vec2_size(entry.size),
        format_vec2_point(top_left),
        format_vec2_point(entry.center),
        format_vec2_size(entry.scale),
        entry.rotation,
        entry.visible,
        entry.inherited_visible,
        entry.layer.map_or("-", ui_layer_label),
        panel_tree_label(entry.panel_kind, entry.panel_id),
        entry.stack_index,
    )
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
        UiDebugNode,
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
            UiDebugNode,
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
            UiDebugNode,
            Node {
                width: percent(100),
                ..default()
            },
            screen_label(
                theme,
                fonts,
                ui_debug_header_lines(&UiDebugState {
                    target,
                    ..default()
                })
                .join("\n"),
                UiThemeTextStyleRole::Caption,
                UiThemeTextColorRole::Primary,
            ),
            UiDebugHeaderText,
            Pickable::IGNORE,
        ));
        root.spawn((
            UiDebugNode,
            UiScrollView,
            ScrollPosition(Vec2::ZERO),
            ui_debug_body_scroll_node(target),
            Pickable {
                is_hoverable: true,
                should_block_lower: false,
            },
        ))
        .with_children(|body| {
            body.spawn((
                UiDebugNode,
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
        });
    })
    .id()
}

fn ui_debug_body_scroll_node(target: UiDebugDisplayTarget) -> Node {
    let mut node = Node {
        width: percent(100),
        flex_grow: 1.0,
        flex_direction: FlexDirection::Column,
        overflow: Overflow::scroll_y(),
        ..default()
    };

    if target == UiDebugDisplayTarget::GameWindow {
        node.max_height = px(UI_DEBUG_GAME_BODY_MAX_HEIGHT);
    }

    node
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
            node.max_height = percent(92);
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

fn inherited_visibility_label(visibility: Option<&InheritedVisibility>) -> &'static str {
    match visibility {
        Some(visibility) if visibility.get() => "inherited-visible",
        Some(_) => "inherited-hidden",
        None => "inherited-unknown",
    }
}

fn option_entity_label(entity: Option<Entity>) -> String {
    entity
        .map(|entity| format!("{entity:?}"))
        .unwrap_or_else(|| "-".to_string())
}

fn format_vec2_size(value: Vec2) -> String {
    format!("{:.1}x{:.1}", value.x, value.y)
}

fn format_vec2_point(value: Vec2) -> String {
    format!("({:.1},{:.1})", value.x, value.y)
}

fn panel_tree_label(kind: Option<UiPanelKind>, id: Option<UiPanelId>) -> String {
    match (kind, id) {
        (Some(kind), Some(id)) => format!("{}::{id:?}", panel_kind_label(kind)),
        (Some(kind), None) => panel_kind_label(kind).to_string(),
        (None, Some(id)) => format!("{id:?}"),
        (None, None) => "-".to_string(),
    }
}

fn z_index_value(z_index: Option<&ZIndex>) -> i32 {
    z_index.map_or(0, |z_index| z_index.0)
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

fn layout_entry_kind_order(entry: &UiDebugLayoutEntry) -> u8 {
    if entry.layer.is_some() || entry.panel_id.is_some() {
        0
    } else {
        1
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

fn panel_kind_label(kind: UiPanelKind) -> &'static str {
    match kind {
        UiPanelKind::Page => "Page",
        UiPanelKind::Hud => "Hud",
        UiPanelKind::Floating => "Floating",
        UiPanelKind::Modal => "Modal",
        UiPanelKind::BlockingOverlay => "Blocking",
    }
}

fn ui_layer_order(layer: UiLayer) -> u8 {
    match layer {
        UiLayer::Page => 0,
        UiLayer::Floating => 1,
        UiLayer::Modal => 2,
        UiLayer::Loading => 3,
        UiLayer::Toast => 4,
        UiLayer::Debug => 5,
    }
}

fn ui_layer_label(layer: UiLayer) -> &'static str {
    match layer {
        UiLayer::Page => "Page",
        UiLayer::Floating => "Floating",
        UiLayer::Modal => "Modal",
        UiLayer::Loading => "Loading",
        UiLayer::Toast => "Toast",
        UiLayer::Debug => "Debug",
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

    fn entity(index: u32) -> Entity {
        Entity::from_raw_u32(index).unwrap()
    }

    fn debug_panel_entry(
        index: u32,
        id: UiPanelId,
        kind: UiPanelKind,
        active: bool,
        z_index: i32,
    ) -> UiDebugPanelEntry {
        UiDebugPanelEntry {
            entity: entity(index),
            id,
            kind,
            owner_mode: "None".to_string(),
            visible: if active { "visible" } else { "hidden" },
            active,
            z_index,
        }
    }

    fn debug_tree_entry(index: u32, layer: UiLayer, id: UiPanelId) -> UiDebugTreeEntry {
        UiDebugTreeEntry {
            entity: entity(index),
            name: Some(format!("node-{index}")),
            parent: None,
            layer: Some(layer),
            panel_id: Some(id),
            panel_kind: Some(UiPanelKind::Page),
            visible: "visible",
            inherited_visible: "inherited-visible",
        }
    }

    fn debug_layout_entry(index: u32) -> UiDebugLayoutEntry {
        UiDebugLayoutEntry {
            entity: entity(index),
            name: Some(format!("layout-node-{index}")),
            size: Vec2::new(120.0 + index as f32, 48.0),
            center: Vec2::new(240.0, 96.0 + index as f32),
            scale: Vec2::ONE,
            rotation: 0.0,
            layer: Some(UiLayer::Page),
            panel_id: Some(UiPanelId::UiGalleryPage),
            panel_kind: Some(UiPanelKind::Page),
            visible: "visible",
            inherited_visible: "inherited-visible",
            stack_index: index,
        }
    }

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

    #[test]
    fn ui_stats_debug_lines_include_node_and_panel_counts() {
        let stats = UiStats {
            ui_node_count: 12,
            visible_ui_node_count: 9,
            text_node_count: 4,
            panel_count: 5,
            panel_kind_counts: crate::game::ui::core::stats::UiPanelKindCounts {
                page: 1,
                hud: 1,
                floating: 1,
                modal: 1,
                blocking_overlay: 1,
            },
        };

        assert_eq!(
            ui_stats_debug_lines(&stats),
            vec![
                "ui stats:",
                "  nodes: total=12 visible=9 text=4",
                "  panels: total=5 page=1 hud=1 floating=1 modal=1 blocking=1",
            ]
        );
    }

    #[test]
    fn panel_stack_lines_show_active_panels_in_debug_order() {
        let mut entries = vec![
            debug_panel_entry(4, UiPanelId::ConfirmModal, UiPanelKind::Modal, true, 100),
            debug_panel_entry(
                3,
                UiPanelId::GalleryFloating,
                UiPanelKind::Floating,
                false,
                80,
            ),
            debug_panel_entry(2, UiPanelId::UiGalleryPage, UiPanelKind::Page, true, 0),
            debug_panel_entry(
                5,
                UiPanelId::GlobalLoading,
                UiPanelKind::BlockingOverlay,
                true,
                120,
            ),
        ];

        sort_panel_debug_entries(&mut entries);
        let lines = panel_stack_debug_lines(&entries);

        assert_eq!(lines[0], "panel stack:");
        assert_eq!(lines[1], "  bottom -> top (active panels):");
        assert!(lines[2].contains("[00] Page UiGalleryPage"));
        assert!(lines[3].contains("[01] Modal ConfirmModal"));
        assert!(lines[4].contains("[02] Blocking GlobalLoading"));
        assert!(!lines.join("\n").contains("GalleryFloating"));
    }

    #[test]
    fn panel_list_lines_apply_filter_without_affecting_stack() {
        let entries = vec![
            debug_panel_entry(1, UiPanelId::UiGalleryPage, UiPanelKind::Page, true, 0),
            debug_panel_entry(
                2,
                UiPanelId::GalleryFloating,
                UiPanelKind::Floating,
                true,
                80,
            ),
            debug_panel_entry(3, UiPanelId::ConfirmModal, UiPanelKind::Modal, true, 100),
        ];

        let lines = panel_list_debug_lines(UiDebugPanelFilter::BlockingPanelsOnly, &entries);

        assert_eq!(lines[0], "panels (blocking panels only)");
        assert!(!lines.join("\n").contains("UiGalleryPage"));
        assert!(!lines.join("\n").contains("GalleryFloating"));
        assert!(lines.join("\n").contains("ConfirmModal"));
    }

    #[test]
    fn ui_tree_lines_truncate_long_root_like_lists() {
        let entries = vec![
            debug_tree_entry(1, UiLayer::Page, UiPanelId::LoginPage),
            debug_tree_entry(2, UiLayer::Page, UiPanelId::GameListPage),
            debug_tree_entry(3, UiLayer::Debug, UiPanelId::UiGalleryPage),
        ];

        let lines = ui_tree_debug_lines(&entries, 2);

        assert_eq!(lines[0], "ui tree:");
        assert_eq!(lines.len(), 4);
        assert!(lines[1].contains("node-1"));
        assert!(lines[2].contains("node-2"));
        assert_eq!(lines[3], "  ... 1 more root-like UI nodes");
    }

    #[test]
    fn ui_tree_entry_line_includes_layer_panel_and_parent() {
        let entry = UiDebugTreeEntry {
            entity: entity(7),
            name: Some("debug-root".to_string()),
            parent: Some(entity(1)),
            layer: Some(UiLayer::Debug),
            panel_id: Some(UiPanelId::ConfirmModal),
            panel_kind: Some(UiPanelKind::Modal),
            visible: "visible",
            inherited_visible: "inherited-visible",
        };

        let line = ui_tree_entry_line(&entry);

        assert!(line.contains("debug-root"));
        assert!(line.contains("layer=Debug"));
        assert!(line.contains("panel=Modal::ConfirmModal"));
        assert!(line.contains("parent="));
        assert!(line.contains("visible/inherited-visible"));
    }

    #[test]
    fn layout_bounds_entry_line_includes_bounds_visibility_layer_and_panel() {
        let entry = UiDebugLayoutEntry {
            entity: entity(8),
            name: Some("gallery-root".to_string()),
            size: Vec2::new(200.0, 80.0),
            center: Vec2::new(300.0, 140.0),
            scale: Vec2::ONE,
            rotation: 0.0,
            layer: Some(UiLayer::Page),
            panel_id: Some(UiPanelId::UiGalleryPage),
            panel_kind: Some(UiPanelKind::Page),
            visible: "visible",
            inherited_visible: "inherited-visible",
            stack_index: 12,
        };

        let line = layout_bounds_entry_line(&entry);

        assert!(line.contains("gallery-root"));
        assert!(line.contains("size=200.0x80.0"));
        assert!(line.contains("top_left=(200.0,100.0)"));
        assert!(line.contains("center=(300.0,140.0)"));
        assert!(line.contains("visible/inherited-visible"));
        assert!(line.contains("layer=Page"));
        assert!(line.contains("panel=Page::UiGalleryPage"));
        assert!(line.contains("stack=12"));
    }

    #[test]
    fn layout_bounds_lines_truncate_long_lists() {
        let entries = vec![
            debug_layout_entry(1),
            debug_layout_entry(2),
            debug_layout_entry(3),
            debug_layout_entry(4),
        ];

        let lines = layout_bounds_debug_lines(&entries, 2);

        assert_eq!(lines[0], "layout bounds:");
        assert_eq!(lines.len(), 4);
        assert!(lines[1].contains("layout-node-1"));
        assert!(lines[2].contains("layout-node-2"));
        assert_eq!(lines[3], "  ... 2 more UI nodes");
    }

    #[test]
    fn layout_bounds_lines_show_empty_state() {
        assert_eq!(
            layout_bounds_debug_lines(&[], 16),
            vec!["layout bounds:", "  none"]
        );
    }

    #[test]
    fn debug_header_lists_copy_shortcut() {
        let debug_state = UiDebugState::default();
        let header = ui_debug_header_lines(&debug_state);

        assert!(header[1].contains("F8 copy"));
    }

    #[test]
    fn debug_display_text_uses_frozen_body() {
        let mut debug_state = UiDebugState {
            frozen: true,
            frozen_body: Some("frozen body".to_string()),
            ..default()
        };

        let (_, _, display) =
            build_ui_debug_display_parts(&mut debug_state, |_| "live body".to_string());

        assert!(display.contains("freeze=on"));
        assert!(display.contains("frozen body"));
        assert!(!display.contains("live body"));
    }

    #[test]
    fn debug_copy_stores_display_text_and_clears_request() {
        let mut debug_state = UiDebugState {
            copy_requested: true,
            ..default()
        };

        copy_ui_debug_display_text(&mut debug_state, "debug text");

        assert!(!debug_state.copy_requested);
        assert_eq!(debug_state.last_copied_text.as_deref(), Some("debug text"));
    }

    #[test]
    fn debug_copy_log_preview_truncates_long_text() {
        assert_eq!(debug_copy_log_preview("abcdef", 8), "abcdef");
        assert_eq!(
            debug_copy_log_preview("abcdef", 3),
            "abc\n... truncated ..."
        );
    }
}
