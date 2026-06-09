use bevy::{picking::Pickable, prelude::*};

use crate::game::ui::{
    core::{
        UiInputState, UiInputSystems, UiLayer, UiLayerRoot, UiPanelId, UiPanelKind, UiPanelRoot,
        UiPanelSystems, focus::UiFocusState,
    },
    style::{
        UiFontAssets, UiTheme,
        theme::{UiThemeBackgroundRole, UiThemeBorderRole, UiThemeTextColorRole},
    },
    widgets::{screen_label, screen_title},
};

pub(in crate::game) struct UiDebugPlugin;

impl Plugin for UiDebugPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiDebugState>().add_systems(
            Update,
            (
                toggle_ui_debug_panel,
                sync_ui_debug_panel,
                refresh_ui_debug_text.after(UiInputSystems::Update),
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
}

#[derive(Component)]
struct UiDebugRoot;

#[derive(Component)]
struct UiDebugText;

fn toggle_ui_debug_panel(
    key_codes: Res<ButtonInput<KeyCode>>,
    mut debug_state: ResMut<UiDebugState>,
) {
    if key_codes.just_pressed(KeyCode::F3) {
        debug_state.enabled = !debug_state.enabled;
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

    let mut lines = vec![
        format!("pointer_blocked: {}", input_state.pointer_blocked),
        format!("focused_panel: {:?}", input_state.focused_panel),
        format!("top_blocking_panel: {:?}", input_state.top_blocking_panel),
        format!("focused_entity: {:?}", focus_state.focused_entity),
        "visible panels:".to_string(),
    ];

    let mut visible_panels = panels
        .iter()
        .filter(|(_, _, visibility, inherited_visibility)| {
            visibility.is_none_or(|visibility| *visibility != Visibility::Hidden)
                && inherited_visibility.is_none_or(|visibility| visibility.get())
        })
        .collect::<Vec<_>>();
    visible_panels.sort_by_key(|(entity, panel, _, _)| {
        (
            panel_kind_order(panel.kind),
            panel_id_order(panel.id),
            *entity,
        )
    });

    if visible_panels.is_empty() {
        lines.push("  none".to_string());
    } else {
        for (entity, panel, visibility, _) in visible_panels {
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

fn spawn_ui_debug_panel(commands: &mut Commands, theme: &UiTheme, fonts: &UiFontAssets) -> Entity {
    commands
        .spawn((
            UiDebugRoot,
            UiLayerRoot {
                layer: UiLayer::Debug,
            },
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
                screen_title(theme, fonts, "UI Input Debug", theme.text.caption),
                Pickable::IGNORE,
            ));
            root.spawn((
                screen_label(
                    theme,
                    fonts,
                    "",
                    theme.text.caption,
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
