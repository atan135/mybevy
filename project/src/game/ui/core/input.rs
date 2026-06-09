use bevy::{picking::hover::Hovered, prelude::*};

use crate::game::ui::{
    core::{
        UiFocusSystems, UiPanelId, UiPanelKind, UiPanelRoot, UiPanelSystems, focus::UiFocusState,
    },
    widgets::{UiScrollView, UiTextInput},
};

pub(in crate::game) struct UiInputPlugin;

impl Plugin for UiInputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiInputState>()
            .configure_sets(Update, UiInputSystems::Update)
            .add_systems(
                Update,
                update_ui_input_state
                    .in_set(UiInputSystems::Update)
                    .after(UiPanelSystems::Commands)
                    .after(UiFocusSystems::SyncFocusedMarkers),
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
    focus_state: Res<UiFocusState>,
    buttons: Query<&Interaction, With<Button>>,
    scroll_views: Query<&Hovered, With<UiScrollView>>,
    text_inputs: Query<(), With<UiTextInput>>,
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
        || focus_state
            .focused_entity
            .is_some_and(|entity| text_inputs.contains(entity))
        || buttons
            .iter()
            .any(|interaction| matches!(*interaction, Interaction::Pressed | Interaction::Hovered))
        || scroll_views.iter().any(|hovered| hovered.0);
}
