use bevy::{prelude::*, ui::UiSystems};

use crate::game::ui::{
    core::{UiPanelKind, UiPanelRoot},
    widgets::{
        DisabledButton, DisabledTextInput, FocusableButton, FocusedButton, LoadingButton,
        UiTextInput,
    },
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
                    focus_interacted_button,
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
    Without<DisabledTextInput>,
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

fn focus_interacted_button(
    mut focus_state: ResMut<UiFocusState>,
    buttons: Query<(Entity, &Interaction), (Changed<Interaction>, FocusableButtonFilter)>,
) {
    for (entity, interaction) in &buttons {
        if *interaction == Interaction::Pressed {
            focus_state.focused_entity = Some(entity);
        }
    }
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
    mut buttons: Query<(Entity, &mut Interaction, Has<UiTextInput>), FocusableButtonFilter>,
) {
    if let Some(entity) = focus_state.keyboard_pressed_entity.take()
        && let Ok((_, mut interaction, _)) = buttons.get_mut(entity)
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

    if let Ok((_, mut interaction, is_text_input)) = buttons.get_mut(focused_entity)
        && !is_text_input
    {
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

    candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.entity));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::ui::core::UiPanelId;

    fn entity(index: u32) -> Entity {
        Entity::from_raw_u32(index).unwrap()
    }

    fn candidate(index: u32) -> FocusCandidate {
        FocusCandidate {
            entity: entity(index),
            panel: None,
        }
    }

    fn test_panel(kind: UiPanelKind) -> UiPanelRoot {
        UiPanelRoot {
            id: UiPanelId::UiGalleryPage,
            kind,
            owner_mode: None,
        }
    }

    fn spawn_focusable_button(world: &mut World) -> Entity {
        world
            .spawn((
                Button,
                FocusableButton,
                Interaction::None,
                InheritedVisibility::VISIBLE,
            ))
            .id()
    }

    fn test_app() -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<UiFocusState>()
            .add_systems(
                Update,
                (
                    focus_interacted_button,
                    navigate_focus_with_tab,
                    repair_invalid_focus,
                    sync_focused_button_markers,
                )
                    .chain(),
            );
        app
    }

    fn press_tab(app: &mut App, shift: bool) {
        {
            let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            input.press(KeyCode::Tab);
            if shift {
                input.press(KeyCode::ShiftLeft);
            }
        }
        app.update();
        {
            let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            input.release(KeyCode::Tab);
            input.release(KeyCode::ShiftLeft);
            input.clear();
        }
    }

    #[test]
    fn next_focus_entity_cycles_forward_and_backward() {
        let candidates = [candidate(1), candidate(2), candidate(3)];

        assert_eq!(next_focus_entity(&candidates, None, false), Some(entity(1)));
        assert_eq!(
            next_focus_entity(&candidates, Some(entity(1)), false),
            Some(entity(2))
        );
        assert_eq!(
            next_focus_entity(&candidates, Some(entity(3)), false),
            Some(entity(1))
        );
        assert_eq!(next_focus_entity(&candidates, None, true), Some(entity(3)));
        assert_eq!(
            next_focus_entity(&candidates, Some(entity(1)), true),
            Some(entity(3))
        );
    }

    #[test]
    fn tab_focus_skips_hidden_disabled_and_loading_buttons() {
        let mut app = test_app();
        let hidden = app
            .world_mut()
            .spawn((
                Button,
                FocusableButton,
                Interaction::None,
                InheritedVisibility::HIDDEN,
            ))
            .id();
        let disabled = app
            .world_mut()
            .spawn((
                Button,
                FocusableButton,
                DisabledButton,
                Interaction::None,
                InheritedVisibility::VISIBLE,
            ))
            .id();
        let loading = app
            .world_mut()
            .spawn((
                Button,
                FocusableButton,
                LoadingButton,
                Interaction::None,
                InheritedVisibility::VISIBLE,
            ))
            .id();
        let visible = spawn_focusable_button(app.world_mut());

        press_tab(&mut app, false);

        let focus_state = app.world().resource::<UiFocusState>();
        assert_eq!(focus_state.focused_entity, Some(visible));
        assert!(app.world().entity(visible).contains::<FocusedButton>());
        assert!(!app.world().entity(hidden).contains::<FocusedButton>());
        assert!(!app.world().entity(disabled).contains::<FocusedButton>());
        assert!(!app.world().entity(loading).contains::<FocusedButton>());
    }

    #[test]
    fn tab_focus_cycles_and_shift_tab_moves_backward() {
        let mut app = test_app();
        let first = spawn_focusable_button(app.world_mut());
        let second = spawn_focusable_button(app.world_mut());
        let third = spawn_focusable_button(app.world_mut());
        let mut expected_order = vec![first, second, third];
        expected_order.sort_by_key(|entity| std::cmp::Reverse(*entity));

        press_tab(&mut app, false);
        assert_eq!(
            app.world().resource::<UiFocusState>().focused_entity,
            Some(expected_order[0])
        );

        press_tab(&mut app, false);
        assert_eq!(
            app.world().resource::<UiFocusState>().focused_entity,
            Some(expected_order[1])
        );

        press_tab(&mut app, true);
        assert_eq!(
            app.world().resource::<UiFocusState>().focused_entity,
            Some(expected_order[0])
        );

        press_tab(&mut app, true);
        assert_eq!(
            app.world().resource::<UiFocusState>().focused_entity,
            Some(expected_order[2])
        );
    }

    #[test]
    fn modal_panel_limits_focus_to_its_own_buttons() {
        let mut app = test_app();
        let page_button = spawn_focusable_button(app.world_mut());
        let panel = app
            .world_mut()
            .spawn((test_panel(UiPanelKind::Modal), ZIndex(10)))
            .id();
        let modal_button = spawn_focusable_button(app.world_mut());
        app.world_mut().entity_mut(panel).add_child(modal_button);

        press_tab(&mut app, false);

        assert_eq!(
            app.world().resource::<UiFocusState>().focused_entity,
            Some(modal_button)
        );
        assert!(!app.world().entity(page_button).contains::<FocusedButton>());
        assert!(app.world().entity(modal_button).contains::<FocusedButton>());
    }
}
