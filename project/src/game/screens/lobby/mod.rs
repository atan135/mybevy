mod game_list;

use bevy::prelude::*;

use crate::framework::{
    scene::prelude::SceneEvent,
    ui::{
        core::UiPanelSystems,
        i18n::UiI18n,
        overlays::{UiOverlayCommand, UiToast},
    },
};
use crate::game::{
    navigation::{AppUiMode, GameRouteCommand},
    scenes::SAMPLE_DUNGEON_ROOM_SCENE_ID,
};

pub(super) struct LobbyScreensPlugin;

impl Plugin for LobbyScreensPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppUiMode::Lobby), game_list::setup_game_list_screen)
            .add_systems(
                Update,
                (
                    game_list::handle_game_list_buttons,
                    handle_sample_dungeon_room_scene_events,
                )
                    .before(UiPanelSystems::Commands)
                    .run_if(in_state(AppUiMode::Lobby)),
            );
    }
}

fn handle_sample_dungeon_room_scene_events(
    i18n: Res<UiI18n>,
    mut scene_events: MessageReader<SceneEvent>,
    mut route_commands: MessageWriter<GameRouteCommand>,
    mut overlay_commands: MessageWriter<UiOverlayCommand>,
) {
    for event in scene_events.read() {
        match event {
            SceneEvent::Entered(entered)
                if entered.scene_id.as_str() == SAMPLE_DUNGEON_ROOM_SCENE_ID =>
            {
                route_commands.write(GameRouteCommand::ChangeMode(AppUiMode::SampleScene));
            }
            SceneEvent::Failed(failure)
                if failure
                    .scene_id
                    .as_ref()
                    .is_some_and(|scene_id| scene_id.as_str() == SAMPLE_DUNGEON_ROOM_SCENE_ID) =>
            {
                overlay_commands.write(UiOverlayCommand::ShowToast(UiToast::new_key(
                    &i18n,
                    "lobby.sample_scene.toast.failed",
                    "Failed to enter sample scene",
                )));
            }
            _ => {}
        }
    }
}
