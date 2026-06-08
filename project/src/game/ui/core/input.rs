use bevy::prelude::*;

use crate::game::ui::overlays::UiModalRoot;

pub(in crate::game) struct UiInputPlugin;

impl Plugin for UiInputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiInputState>()
            .configure_sets(Update, UiInputSystems::Update)
            .add_systems(Update, update_ui_input_state.in_set(UiInputSystems::Update));
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
pub(in crate::game) enum UiInputSystems {
    Update,
}

#[derive(Debug, Default, Resource)]
pub(in crate::game) struct UiInputState {
    pub pointer_blocked: bool,
}

fn update_ui_input_state(
    mut input_state: ResMut<UiInputState>,
    buttons: Query<&Interaction, With<Button>>,
    modals: Query<(), With<UiModalRoot>>,
) {
    input_state.pointer_blocked = !modals.is_empty()
        || buttons
            .iter()
            .any(|interaction| matches!(*interaction, Interaction::Pressed | Interaction::Hovered));
}
