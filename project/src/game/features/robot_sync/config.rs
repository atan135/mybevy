use bevy::prelude::*;

use crate::{framework::scene::prelude::SceneId, game::scenes::ROBOT_SYNC_ARENA_SCENE_ID};

#[derive(Clone, Debug, Resource, PartialEq, Eq)]
pub(in crate::game::features::robot_sync) struct RobotSyncConfig {
    pub(in crate::game::features::robot_sync) scene_id: SceneId,
}

impl Default for RobotSyncConfig {
    fn default() -> Self {
        Self {
            scene_id: SceneId::from(ROBOT_SYNC_ARENA_SCENE_ID),
        }
    }
}

impl RobotSyncConfig {
    pub(in crate::game::features::robot_sync) fn is_robot_sync_scene(
        &self,
        scene_id: &SceneId,
    ) -> bool {
        self.scene_id.as_str() == scene_id.as_str()
    }
}
