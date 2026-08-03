mod host;
mod model;

#[cfg(test)]
mod tests;

use bevy::prelude::*;

use crate::framework::{
    scene::prelude::SceneEvent,
    ui::{
        core::{UiPanelCommand, UiPanelSystems, binding::UiBindingSystems},
        document::UiDocumentRuntimeSystems,
        i18n::UiI18n,
        overlays::{UiOverlayCommand, UiToast},
    },
};
use crate::game::{
    navigation::{AppUiMode, GameRouteCommand},
    scenes::{
        FANGYUAN_HOME_SCENE_ID, LOCKSTEP_SIM_ARENA_SCENE_ID, ROBOT_SYNC_ARENA_SCENE_ID,
        SAMPLE_DUNGEON_ROOM_SCENE_ID,
    },
};

pub(super) struct LobbyScreensPlugin;

impl Plugin for LobbyScreensPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<model::LobbyUiState>()
            .init_resource::<host::LobbyHostContract>()
            .add_systems(Startup, host::register_lobby_contract);

        #[cfg(all(debug_assertions, not(target_os = "android")))]
        app.add_systems(OnEnter(AppUiMode::Lobby), host::prepare_lobby_audit_fixture)
            .add_systems(
                OnExit(AppUiMode::Lobby),
                host::clear_lobby_audit_image_failure,
            );

        app.add_systems(OnExit(AppUiMode::Lobby), host::cleanup_lobby_screen)
            .add_systems(
                Update,
                host::finish_lobby_reload.run_if(in_state(AppUiMode::Lobby)),
            )
            .add_systems(
                Update,
                (
                    host::handle_lobby_document_actions.after(UiDocumentRuntimeSystems::Reconcile),
                    host::sync_lobby_document_bindings.before(UiBindingSystems::Apply),
                )
                    .chain()
                    .run_if(in_state(AppUiMode::Lobby)),
            )
            .add_systems(Update, host::follow_lobby_document_reload_events)
            .add_systems(
                Update,
                handle_lobby_scene_entry_events.before(UiPanelSystems::Commands),
            );
    }
}

fn handle_lobby_scene_entry_events(
    i18n: Res<UiI18n>,
    mut scene_events: MessageReader<SceneEvent>,
    mut ui_state: ResMut<model::LobbyUiState>,
    mut panel_commands: MessageWriter<UiPanelCommand>,
    mut overlay_commands: MessageWriter<UiOverlayCommand>,
    mut route_commands: MessageWriter<GameRouteCommand>,
) {
    for event in scene_events.read() {
        match event {
            SceneEvent::Entered(entered)
                if entered.scene_id.as_str() == SAMPLE_DUNGEON_ROOM_SCENE_ID =>
            {
                host::close_lobby_loading(&mut ui_state, &mut panel_commands);
            }
            SceneEvent::Entered(entered)
                if should_route_robot_sync_scene_entered(entered.scene_id.as_str()) =>
            {
                host::close_lobby_loading(&mut ui_state, &mut panel_commands);
                route_commands.write(GameRouteCommand::ChangeMode(AppUiMode::RobotSyncScene));
            }
            SceneEvent::Entered(entered)
                if should_route_fangyuan_home_entered(entered.scene_id.as_str()) =>
            {
                host::close_lobby_loading(&mut ui_state, &mut panel_commands);
                route_commands.write(GameRouteCommand::ChangeMode(AppUiMode::FangyuanHome));
            }
            SceneEvent::Failed(failure)
                if failure.scene_id.as_ref().is_some_and(|scene_id| {
                    matches!(
                        scene_id.as_str(),
                        SAMPLE_DUNGEON_ROOM_SCENE_ID
                            | ROBOT_SYNC_ARENA_SCENE_ID
                            | LOCKSTEP_SIM_ARENA_SCENE_ID
                            | FANGYUAN_HOME_SCENE_ID
                    )
                }) =>
            {
                host::close_lobby_loading(&mut ui_state, &mut panel_commands);
                warn!("failed to enter lobby scene: {}", failure.log_description());
                overlay_commands.write(UiOverlayCommand::ShowToast(UiToast::new_key(
                    &i18n,
                    "lobby.scene.toast.failed",
                    "Failed to enter game scene",
                )));
            }
            SceneEvent::Exited(exited)
                if matches!(
                    exited.scene_id.as_str(),
                    SAMPLE_DUNGEON_ROOM_SCENE_ID
                        | ROBOT_SYNC_ARENA_SCENE_ID
                        | LOCKSTEP_SIM_ARENA_SCENE_ID
                        | FANGYUAN_HOME_SCENE_ID
                ) =>
            {
                host::close_lobby_loading(&mut ui_state, &mut panel_commands);
            }
            _ => {}
        }
    }
}

fn should_route_robot_sync_scene_entered(scene_id: &str) -> bool {
    matches!(
        scene_id,
        ROBOT_SYNC_ARENA_SCENE_ID | LOCKSTEP_SIM_ARENA_SCENE_ID
    )
}

fn should_route_fangyuan_home_entered(scene_id: &str) -> bool {
    scene_id == FANGYUAN_HOME_SCENE_ID
}
