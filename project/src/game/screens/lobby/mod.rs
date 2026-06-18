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
use crate::game::{navigation::AppUiMode, scenes::SAMPLE_DUNGEON_ROOM_SCENE_ID};

pub(super) struct LobbyScreensPlugin;

impl Plugin for LobbyScreensPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<game_list::SampleDungeonRoomEntryState>()
            .add_systems(
                OnEnter(AppUiMode::Lobby),
                (
                    reset_sample_dungeon_room_entry_state,
                    game_list::setup_game_list_screen,
                ),
            )
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
    mut sample_scene_entry: ResMut<game_list::SampleDungeonRoomEntryState>,
    mut overlay_commands: MessageWriter<UiOverlayCommand>,
) {
    for event in scene_events.read() {
        match event {
            SceneEvent::Entered(entered)
                if entered.scene_id.as_str() == SAMPLE_DUNGEON_ROOM_SCENE_ID =>
            {
                sample_scene_entry.clear();
            }
            SceneEvent::Failed(failure)
                if failure
                    .scene_id
                    .as_ref()
                    .is_some_and(|scene_id| scene_id.as_str() == SAMPLE_DUNGEON_ROOM_SCENE_ID) =>
            {
                sample_scene_entry.clear();
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

fn reset_sample_dungeon_room_entry_state(
    mut sample_scene_entry: ResMut<game_list::SampleDungeonRoomEntryState>,
) {
    sample_scene_entry.clear();
}
