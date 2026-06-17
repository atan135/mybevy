use std::collections::VecDeque;

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

const UI_INPUT_ROUTE_HISTORY_LIMIT: usize = 12;

#[derive(Debug, Resource)]
pub(in crate::game) struct UiInputState {
    pub pointer_blocked: bool,
    pub focused_panel: Option<UiPanelId>,
    pub top_blocking_panel: Option<UiPanelId>,
    pub pointer_block_reason: String,
    pub route_summary: String,
    pub route_history: VecDeque<UiInputRouteHistoryEntry>,
    next_history_id: u64,
    last_snapshot: Option<UiInputRouteSnapshot>,
}

impl Default for UiInputState {
    fn default() -> Self {
        let snapshot = UiInputRouteSnapshot::default();

        Self {
            pointer_blocked: snapshot.pointer_blocked,
            focused_panel: snapshot.focused_panel,
            top_blocking_panel: snapshot.top_blocking_panel,
            pointer_block_reason: snapshot.block_reason.summary(),
            route_summary: snapshot.summary(),
            route_history: VecDeque::default(),
            next_history_id: 0,
            last_snapshot: Some(snapshot),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::game) struct UiInputRouteHistoryEntry {
    pub id: u64,
    pub summary: String,
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
        .map(|panel| (panel.id, panel.kind));
    let focused_text_input = focus_state
        .focused_entity
        .is_some_and(|entity| text_inputs.contains(entity));
    let mut hovered_button = false;
    let mut pressed_button = false;
    for interaction in &buttons {
        match *interaction {
            Interaction::Pressed => pressed_button = true,
            Interaction::Hovered => hovered_button = true,
            Interaction::None => {}
        }
    }
    let hovered_scroll_view = scroll_views.iter().any(|hovered| hovered.0);

    input_state.apply_snapshot(resolve_ui_input_route(UiInputRouteSignals {
        top_blocking_panel,
        focused_text_input,
        pressed_button,
        hovered_button,
        hovered_scroll_view,
    }));
}

impl UiInputState {
    fn apply_snapshot(&mut self, snapshot: UiInputRouteSnapshot) {
        self.pointer_blocked = snapshot.pointer_blocked;
        self.focused_panel = snapshot.focused_panel;
        self.top_blocking_panel = snapshot.top_blocking_panel;
        self.pointer_block_reason = snapshot.block_reason.summary();
        self.route_summary = snapshot.summary();

        if self.last_snapshot.as_ref() != Some(&snapshot) {
            self.route_history.push_front(UiInputRouteHistoryEntry {
                id: self.next_history_id,
                summary: self.route_summary.clone(),
            });
            self.next_history_id += 1;

            while self.route_history.len() > UI_INPUT_ROUTE_HISTORY_LIMIT {
                self.route_history.pop_back();
            }

            self.last_snapshot = Some(snapshot);
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct UiInputRouteSignals {
    top_blocking_panel: Option<(UiPanelId, UiPanelKind)>,
    focused_text_input: bool,
    pressed_button: bool,
    hovered_button: bool,
    hovered_scroll_view: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct UiInputRouteSnapshot {
    pointer_blocked: bool,
    focused_panel: Option<UiPanelId>,
    top_blocking_panel: Option<UiPanelId>,
    block_reason: UiInputBlockReason,
}

impl Default for UiInputRouteSnapshot {
    fn default() -> Self {
        Self {
            pointer_blocked: false,
            focused_panel: None,
            top_blocking_panel: None,
            block_reason: UiInputBlockReason::None,
        }
    }
}

impl UiInputRouteSnapshot {
    fn summary(&self) -> String {
        format!(
            "blocked={} reason={} focused_panel={} top_blocking_panel={}",
            self.pointer_blocked,
            self.block_reason.summary(),
            optional_panel_id_label(self.focused_panel),
            optional_panel_id_label(self.top_blocking_panel),
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum UiInputBlockReason {
    None,
    BlockingPanel { id: UiPanelId, kind: UiPanelKind },
    FocusedTextInput,
    PressedButton,
    HoveredButton,
    HoveredScrollView,
}

impl UiInputBlockReason {
    fn summary(&self) -> String {
        match self {
            Self::None => "none".to_string(),
            Self::BlockingPanel { id, kind } => format!("{id} {kind:?}"),
            Self::FocusedTextInput => "focused text input".to_string(),
            Self::PressedButton => "pressed button".to_string(),
            Self::HoveredButton => "hovered button".to_string(),
            Self::HoveredScrollView => "hovered scroll view".to_string(),
        }
    }
}

fn optional_panel_id_label(id: Option<UiPanelId>) -> String {
    id.map(|id| id.to_string())
        .unwrap_or_else(|| "none".to_string())
}

fn resolve_ui_input_route(signals: UiInputRouteSignals) -> UiInputRouteSnapshot {
    let block_reason = if let Some((id, kind)) = signals.top_blocking_panel {
        UiInputBlockReason::BlockingPanel { id, kind }
    } else if signals.focused_text_input {
        UiInputBlockReason::FocusedTextInput
    } else if signals.pressed_button {
        UiInputBlockReason::PressedButton
    } else if signals.hovered_button {
        UiInputBlockReason::HoveredButton
    } else if signals.hovered_scroll_view {
        UiInputBlockReason::HoveredScrollView
    } else {
        UiInputBlockReason::None
    };
    let top_blocking_panel = signals.top_blocking_panel.map(|(id, _)| id);

    UiInputRouteSnapshot {
        pointer_blocked: block_reason != UiInputBlockReason::None,
        focused_panel: top_blocking_panel,
        top_blocking_panel,
        block_reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::ui::core::UI_PANEL_GLOBAL_LOADING;

    #[test]
    fn route_reason_prefers_blocking_panel_then_text_then_button_then_scroll() {
        let snapshot = resolve_ui_input_route(UiInputRouteSignals {
            top_blocking_panel: Some((UI_PANEL_GLOBAL_LOADING, UiPanelKind::BlockingOverlay)),
            focused_text_input: true,
            pressed_button: true,
            hovered_button: true,
            hovered_scroll_view: true,
        });
        assert_eq!(
            snapshot.block_reason,
            UiInputBlockReason::BlockingPanel {
                id: UI_PANEL_GLOBAL_LOADING,
                kind: UiPanelKind::BlockingOverlay,
            },
        );

        let snapshot = resolve_ui_input_route(UiInputRouteSignals {
            focused_text_input: true,
            pressed_button: true,
            hovered_button: true,
            hovered_scroll_view: true,
            ..default()
        });
        assert_eq!(snapshot.block_reason, UiInputBlockReason::FocusedTextInput);

        let snapshot = resolve_ui_input_route(UiInputRouteSignals {
            pressed_button: true,
            hovered_button: true,
            hovered_scroll_view: true,
            ..default()
        });
        assert_eq!(snapshot.block_reason, UiInputBlockReason::PressedButton);

        let snapshot = resolve_ui_input_route(UiInputRouteSignals {
            hovered_button: true,
            hovered_scroll_view: true,
            ..default()
        });
        assert_eq!(snapshot.block_reason, UiInputBlockReason::HoveredButton);

        let snapshot = resolve_ui_input_route(UiInputRouteSignals {
            hovered_scroll_view: true,
            ..default()
        });
        assert_eq!(snapshot.block_reason, UiInputBlockReason::HoveredScrollView);
    }

    #[test]
    fn route_history_records_only_state_changes() {
        let mut state = UiInputState::default();
        let hovered = resolve_ui_input_route(UiInputRouteSignals {
            hovered_button: true,
            ..default()
        });
        state.apply_snapshot(hovered.clone());
        state.apply_snapshot(hovered);
        assert_eq!(state.route_history.len(), 1);

        state.apply_snapshot(resolve_ui_input_route(UiInputRouteSignals {
            hovered_scroll_view: true,
            ..default()
        }));
        assert_eq!(state.route_history.len(), 2);
        assert!(
            state.route_history[0]
                .summary
                .contains("hovered scroll view")
        );
    }

    #[test]
    fn route_history_keeps_recent_entries() {
        let mut state = UiInputState::default();

        for index in 0..(UI_INPUT_ROUTE_HISTORY_LIMIT + 3) {
            let snapshot = if index % 2 == 0 {
                resolve_ui_input_route(UiInputRouteSignals {
                    hovered_button: true,
                    ..default()
                })
            } else {
                resolve_ui_input_route(UiInputRouteSignals {
                    hovered_scroll_view: true,
                    ..default()
                })
            };
            state.apply_snapshot(snapshot);
        }

        assert_eq!(state.route_history.len(), UI_INPUT_ROUTE_HISTORY_LIMIT);
        assert_eq!(
            state.route_history[0].id,
            UI_INPUT_ROUTE_HISTORY_LIMIT as u64 + 2
        );
        assert_eq!(state.route_history.back().unwrap().id, 3);
    }
}
