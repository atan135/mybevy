use bevy::{prelude::*, ui::UiSystems};

use crate::game::ui::{
    core::{UiPanelKind, UiPanelRoot},
    widgets::{DisabledButton, FocusableButton, FocusedButton, LoadingButton},
};

pub(in crate::game) struct UiFocusPlugin;

impl Plugin for UiFocusPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiFocusState>()
            .configure_sets(
                Update,
                UiFocusSystems::SyncFocusedMarkers.before(UiFocusSystems::Visuals),
            )
            .add_systems(
                PreUpdate,
                update_keyboard_button_activation.after(UiSystems::Focus),
            )
            .add_systems(
                Update,
                (
                    navigate_focus_with_tab,
                    repair_invalid_focus,
                    sync_focused_button_markers,
                )
                    .chain()
                    .in_set(UiFocusSystems::SyncFocusedMarkers),
            );
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
pub(in crate::game) enum UiFocusSystems {
    SyncFocusedMarkers,
    Visuals,
}

#[derive(Debug, Default, Resource)]
pub(in crate::game) struct UiFocusState {
    pub focused_entity: Option<Entity>,
    keyboard_pressed_entity: Option<Entity>,
}

#[derive(Clone, Copy, Debug)]
struct FocusCandidate {
    entity: Entity,
    panel: Option<Entity>,
}

type FocusableButtonFilter = (
    With<Button>,
    With<FocusableButton>,
    Without<DisabledButton>,
    Without<LoadingButton>,
);

fn navigate_focus_with_tab(
    key_codes: Res<ButtonInput<KeyCode>>,
    mut focus_state: ResMut<UiFocusState>,
    buttons: Query<(Entity, Option<&InheritedVisibility>), FocusableButtonFilter>,
    panels: Query<(Entity, &UiPanelRoot, Option<&ZIndex>)>,
    parents: Query<&ChildOf>,
) {
    if !key_codes.just_pressed(KeyCode::Tab) {
        return;
    }

    let candidates = focus_candidates(&buttons, &panels, &parents);
    if candidates.is_empty() {
        focus_state.focused_entity = None;
        return;
    }

    let moving_backward = key_codes.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    focus_state.focused_entity =
        next_focus_entity(&candidates, focus_state.focused_entity, moving_backward);
}

fn repair_invalid_focus(
    mut focus_state: ResMut<UiFocusState>,
    buttons: Query<(Entity, Option<&InheritedVisibility>), FocusableButtonFilter>,
    panels: Query<(Entity, &UiPanelRoot, Option<&ZIndex>)>,
    parents: Query<&ChildOf>,
) {
    let Some(focused_entity) = focus_state.focused_entity else {
        return;
    };

    let candidates = focus_candidates(&buttons, &panels, &parents);
    if candidates
        .iter()
        .any(|candidate| candidate.entity == focused_entity)
    {
        return;
    }

    focus_state.focused_entity = candidates.first().map(|candidate| candidate.entity);
}

fn update_keyboard_button_activation(
    key_codes: Res<ButtonInput<KeyCode>>,
    mut focus_state: ResMut<UiFocusState>,
    mut buttons: Query<&mut Interaction, FocusableButtonFilter>,
) {
    if let Some(entity) = focus_state.keyboard_pressed_entity.take()
        && let Ok(mut interaction) = buttons.get_mut(entity)
        && *interaction == Interaction::Pressed
    {
        *interaction = Interaction::None;
    }

    if !key_codes.just_pressed(KeyCode::Enter) && !key_codes.just_pressed(KeyCode::Space) {
        return;
    }

    let Some(focused_entity) = focus_state.focused_entity else {
        return;
    };

    if let Ok(mut interaction) = buttons.get_mut(focused_entity) {
        *interaction = Interaction::Pressed;
        focus_state.keyboard_pressed_entity = Some(focused_entity);
    }
}

fn sync_focused_button_markers(
    mut commands: Commands,
    focus_state: Res<UiFocusState>,
    focused_buttons: Query<Entity, With<FocusedButton>>,
    focusable_buttons: Query<(), FocusableButtonFilter>,
) {
    let active_focus = focus_state
        .focused_entity
        .filter(|entity| focusable_buttons.get(*entity).is_ok());

    for entity in &focused_buttons {
        if Some(entity) != active_focus {
            commands.entity(entity).remove::<FocusedButton>();
        }
    }

    if let Some(entity) = active_focus
        && !focused_buttons.contains(entity)
    {
        commands.entity(entity).insert(FocusedButton);
    }
}

fn focus_candidates(
    buttons: &Query<(Entity, Option<&InheritedVisibility>), FocusableButtonFilter>,
    panels: &Query<(Entity, &UiPanelRoot, Option<&ZIndex>)>,
    parents: &Query<&ChildOf>,
) -> Vec<FocusCandidate> {
    let active_panel = active_focus_panel(panels);
    let mut candidates = buttons
        .iter()
        .filter(|(_, inherited_visibility)| {
            inherited_visibility.is_none_or(|visibility| visibility.get())
        })
        .filter_map(|(entity, _)| {
            let panel = nearest_panel(entity, panels, parents);
            if active_panel.is_some() && panel != active_panel {
                return None;
            }

            Some(FocusCandidate { entity, panel })
        })
        .collect::<Vec<_>>();

    if active_panel.is_none() {
        if let Some(fallback_panel) = highest_panel_with_buttons(&candidates, panels) {
            candidates.retain(|candidate| candidate.panel == Some(fallback_panel));
        }
    }

    candidates.sort_by_key(|candidate| candidate.entity);
    candidates
}

fn active_focus_panel(panels: &Query<(Entity, &UiPanelRoot, Option<&ZIndex>)>) -> Option<Entity> {
    panels
        .iter()
        .filter(|(_, panel, _)| {
            matches!(
                panel.kind,
                UiPanelKind::BlockingOverlay | UiPanelKind::Modal
            )
        })
        .max_by_key(|(entity, panel, z_index)| {
            (
                panel_kind_order(panel.kind),
                panel_order_key(*entity, z_index),
            )
        })
        .map(|(entity, _, _)| entity)
}

fn highest_panel_with_buttons(
    candidates: &[FocusCandidate],
    panels: &Query<(Entity, &UiPanelRoot, Option<&ZIndex>)>,
) -> Option<Entity> {
    candidates
        .iter()
        .filter_map(|candidate| {
            let panel_entity = candidate.panel?;
            let (_, panel, z_index) = panels.get(panel_entity).ok()?;
            Some((
                panel_entity,
                panel_order_key(panel_entity, &z_index),
                panel.kind,
            ))
        })
        .max_by_key(|(entity, order, kind)| (panel_kind_order(*kind), *order, *entity))
        .map(|(entity, _, _)| entity)
}

fn nearest_panel(
    entity: Entity,
    panels: &Query<(Entity, &UiPanelRoot, Option<&ZIndex>)>,
    parents: &Query<&ChildOf>,
) -> Option<Entity> {
    if panels.get(entity).is_ok() {
        return Some(entity);
    }

    parents
        .iter_ancestors(entity)
        .find(|ancestor| panels.get(*ancestor).is_ok())
}

fn next_focus_entity(
    candidates: &[FocusCandidate],
    focused_entity: Option<Entity>,
    moving_backward: bool,
) -> Option<Entity> {
    let current_index = focused_entity
        .and_then(|focused_entity| {
            candidates
                .iter()
                .position(|candidate| candidate.entity == focused_entity)
        })
        .unwrap_or_else(|| {
            if moving_backward {
                candidates.len()
            } else {
                usize::MAX
            }
        });

    let next_index = if moving_backward {
        current_index
            .checked_sub(1)
            .unwrap_or_else(|| candidates.len().saturating_sub(1))
    } else if current_index == usize::MAX || current_index + 1 >= candidates.len() {
        0
    } else {
        current_index + 1
    };

    candidates.get(next_index).map(|candidate| candidate.entity)
}

fn panel_order_key(entity: Entity, z_index: &Option<&ZIndex>) -> (i32, Entity) {
    (z_index.map_or(0, |z_index| z_index.0), entity)
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
