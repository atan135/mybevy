use bevy::{picking::Pickable, prelude::*};

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

#[derive(Debug, Default, Resource)]
struct UiDebugState {
    enabled: bool,
    root: Option<Entity>,
    frozen: bool,
    panel_filter: UiDebugPanelFilter,
    highlight_panels: bool,
}

#[derive(Component)]
struct UiDebugRoot;

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
}

fn sync_ui_debug_panel(
    mut commands: Commands,
    theme: Res<UiTheme>,
    fonts: Res<UiFontAssets>,
    mut debug_state: ResMut<UiDebugState>,
    debug_roots: Query<Entity, With<UiDebugRoot>>,
) {
    if let Some(root) = debug_state.root
        && !debug_roots.contains(root)
    {
        debug_state.root = None;
    }

    if !debug_state.enabled {
        for root in &debug_roots {
            commands.entity(root).try_despawn();
        }
        debug_state.root = None;
        return;
    }

    if debug_state.root.is_none() {
        debug_state.root = Some(spawn_ui_debug_panel(&mut commands, &theme, &fonts));
    }
}

fn refresh_ui_debug_text(
    debug_state: Res<UiDebugState>,
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

    if debug_state.frozen {
        return;
    }

    let mut lines = vec![
        format!(
            "debug: freeze={} filter={} highlight={}",
            on_off_label(debug_state.frozen),
            debug_state.panel_filter.label(),
            on_off_label(debug_state.highlight_panels),
        ),
        "keys: F3 toggle panel | F4 freeze | F5 filter | F6 highlight".to_string(),
        String::new(),
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

    text.0 = lines.join("\n");
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

fn spawn_ui_debug_panel(commands: &mut Commands, theme: &UiTheme, fonts: &UiFontAssets) -> Entity {
    commands
        .spawn((
            UiDebugRoot,
            UiLayerRoot {
                layer: UiLayer::Debug,
            },
            UiThemeRootNodeRole::Debug,
            UiThemePanelNodeRole::Debug,
            Node {
                position_type: PositionType::Absolute,
                left: px(theme.layout.overlay_padding),
                top: px(theme.layout.overlay_padding),
                width: px(430),
                max_width: percent(94),
                flex_direction: FlexDirection::Column,
                row_gap: px(theme.layout.row_gap),
                padding: UiRect::all(px(14)),
                border: UiRect::all(px(theme.panel.border)),
                border_radius: BorderRadius::all(px(theme.panel.radius)),
                ..default()
            },
            ZIndex(250),
            BackgroundColor(theme.colors.panel_background),
            BorderColor::all(theme.colors.panel_border),
            UiThemeBackgroundRole::Panel,
            UiThemeBorderRole::Panel,
            Pickable::IGNORE,
        ))
        .with_children(|root| {
            root.spawn((
                screen_title(
                    theme,
                    fonts,
                    "UI Input Debug",
                    UiThemeTextStyleRole::Caption,
                ),
                Pickable::IGNORE,
            ));
            root.spawn((
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
}
