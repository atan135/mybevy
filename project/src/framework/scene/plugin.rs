use bevy::prelude::*;

use super::{
    command::SceneCommand, debug::SceneDebugConfig, event::SceneEvent, lifecycle::SceneRuntime,
    registry::SceneRegistry, trigger::SceneTriggerEvent,
};

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SceneCommand>()
            .add_message::<SceneEvent>()
            .add_message::<SceneTriggerEvent>()
            .init_resource::<SceneRuntime>()
            .init_resource::<SceneRegistry>()
            .init_resource::<SceneDebugConfig>();
    }
}
