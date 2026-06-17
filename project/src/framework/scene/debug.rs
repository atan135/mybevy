use bevy::prelude::*;

use super::{
    event::SceneFailure,
    id::{SceneId, SceneSessionId},
    lifecycle::SceneLifecycleState,
};

#[derive(Clone, Debug, Resource, PartialEq)]
pub struct SceneDebugConfig {
    pub enabled: bool,
    pub log_lifecycle: bool,
    pub simulate_slow_loading_seconds: Option<f32>,
    pub simulate_failure: Option<SceneDebugFailure>,
}

impl Default for SceneDebugConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            log_lifecycle: false,
            simulate_slow_loading_seconds: None,
            simulate_failure: None,
        }
    }
}

impl SceneDebugConfig {
    pub fn from_env() -> Self {
        let enabled = std::env::var("MYBEVY_SCENE_DEBUG")
            .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "on" | "ON"))
            .unwrap_or(false);

        Self {
            enabled,
            log_lifecycle: enabled,
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SceneDebugFailure {
    ManifestLoad,
    AssetLoad,
    CameraSetup,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SceneDebugSnapshot {
    pub scene_id: Option<SceneId>,
    pub session_id: Option<SceneSessionId>,
    pub state: SceneLifecycleState,
    pub scene_owned_entities: usize,
    pub layer_count: usize,
    pub last_error: Option<SceneFailure>,
}
