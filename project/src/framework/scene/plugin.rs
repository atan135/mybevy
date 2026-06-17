use bevy::prelude::*;

use super::{
    command::SceneCommand,
    debug::SceneDebugConfig,
    event::SceneEvent,
    lifecycle::{SceneRuntime, poll_scene_asset_loads, process_scene_lifecycle_commands},
    loading::{
        SceneAssetLoadQueue, SceneLoadingUiConfig, SceneLoadingUiState, sync_scene_loading_ui,
    },
    registry::SceneRegistry,
    spawn::SceneSpawnRegistry,
    trigger::{
        SceneTriggerCommand, SceneTriggerEvent, detect_scene_triggers,
        process_scene_trigger_commands,
    },
};

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SceneCommand>()
            .add_message::<SceneEvent>()
            .add_message::<SceneTriggerCommand>()
            .add_message::<SceneTriggerEvent>()
            .init_resource::<SceneRuntime>()
            .init_resource::<SceneAssetLoadQueue>()
            .init_resource::<SceneLoadingUiConfig>()
            .init_resource::<SceneLoadingUiState>()
            .init_resource::<SceneRegistry>()
            .init_resource::<SceneSpawnRegistry>()
            .init_resource::<SceneDebugConfig>()
            .add_systems(
                Update,
                (
                    process_scene_lifecycle_commands,
                    poll_scene_asset_loads,
                    process_scene_trigger_commands,
                    detect_scene_triggers,
                    sync_scene_loading_ui,
                )
                    .chain(),
            );
    }
}
