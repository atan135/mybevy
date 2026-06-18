//! Game-layer scene definitions and registration.
//!
//! Reusable scene framework code stays in `framework::scene`. Concrete game
//! scene ids, scene registration adapters, and scene-specific composition live
//! here as the project grows.

mod catalog;
mod sample_dungeon_room;

use bevy::prelude::*;

pub(in crate::game) use sample_dungeon_room::SAMPLE_DUNGEON_ROOM_SCENE_ID;

pub(in crate::game) struct GameScenesPlugin;

impl Plugin for GameScenesPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            catalog::GameSceneCatalogPlugin,
            sample_dungeon_room::SampleDungeonRoomPlugin,
        ));
    }
}
