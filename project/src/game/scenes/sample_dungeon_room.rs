use bevy::prelude::*;

pub(in crate::game::scenes) const SAMPLE_DUNGEON_ROOM_SCENE_ID: &str = "sample.dungeon_room";

pub(super) struct SampleDungeonRoomPlugin;

impl Plugin for SampleDungeonRoomPlugin {
    fn build(&self, _app: &mut App) {
        let _ = SAMPLE_DUNGEON_ROOM_SCENE_ID;
    }
}
