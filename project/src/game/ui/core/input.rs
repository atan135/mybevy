use bevy::prelude::*;

use crate::game::ui::core::{UiPanelId, UiPanelKind, UiPanelRoot, UiPanelSystems};

pub(in crate::game) struct UiInputPlugin;

impl Plugin for UiInputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiInputState>()
            .configure_sets(Update, UiInputSystems::Update)
            .add_systems(
                Update,
                update_ui_input_state
                    .in_set(UiInputSystems::Update)
                    .after(UiPanelSystems::Commands),
            );
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
pub(in crate::game) enum UiInputSystems {
    Update,
}

#[derive(Debug, Default, Resource)]
pub(in crate::game) struct UiInputState {
    pub pointer_blocked: bool,
    pub focused_panel: Option<UiPanelId>,
    pub top_blocking_panel: Option<UiPanelId>,
}

fn update_ui_input_state(
    mut input_state: ResMut<UiInputState>,
    buttons: Query<&Interaction, With<Button>>,
    panels: Query<&UiPanelRoot>,
) {
    let top_blocking_panel = panels
        .iter()
        .find(|panel| panel.kind == UiPanelKind::BlockingOverlay)
        .or_else(|| panels.iter().find(|panel| panel.kind == UiPanelKind::Modal))
        .map(|panel| panel.id);

    input_state.focused_panel = top_blocking_panel;
    input_state.top_blocking_panel = top_blocking_panel;
    input_state.pointer_blocked = top_blocking_panel.is_some()
        || buttons
            .iter()
            .any(|interaction| matches!(*interaction, Interaction::Pressed | Interaction::Hovered));
}
