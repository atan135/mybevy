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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        framework::scene::prelude::{
            SceneContentSource, SceneId, SceneKind, ScenePlugin, SceneRegistry,
        },
        game::navigation::GameRouteCommand,
    };
    use bevy::asset::AssetPlugin;

    #[test]
    fn scene_plugins_register_sample_dungeon_room_from_first_package_catalog() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin))
            .add_message::<GameRouteCommand>()
            .add_plugins(GameScenesPlugin);

        app.update();

        let registry = app.world().resource::<SceneRegistry>();
        let scene_id = SceneId::from(SAMPLE_DUNGEON_ROOM_SCENE_ID);
        let definition = registry.get(&scene_id).unwrap();

        assert_eq!(definition.scene_id, scene_id);
        assert_eq!(definition.kind, SceneKind::Dungeon);
        assert!(definition.has_world_root);
        assert_eq!(
            definition.manifest_path.as_deref(),
            Some("scenes/sample_dungeon_room/scene.ron")
        );
        assert_eq!(
            definition.content_source,
            SceneContentSource::FirstPackage {
                manifest_path: "scenes/sample_dungeon_room/scene.ron".to_string()
            }
        );
    }
}
