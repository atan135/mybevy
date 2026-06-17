use bevy::prelude::*;

use super::{
    event::SceneFailure,
    id::{SceneId, SceneSessionId},
    lifecycle::{SceneLifecycleState, SceneRuntime},
    root::SceneEntityCounts,
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
    pub entity_counts: SceneEntityCounts,
    pub scene_owned_entities: usize,
    pub layer_count: usize,
    pub last_error: Option<SceneFailure>,
}

impl SceneDebugSnapshot {
    pub fn from_runtime(runtime: &SceneRuntime) -> Self {
        let session = runtime.active().or(runtime.pending());

        Self {
            scene_id: session.map(|session| session.scene_id.clone()),
            session_id: session.map(|session| session.session_id.clone()),
            state: runtime.state(),
            last_error: runtime.last_error().cloned(),
            ..Default::default()
        }
    }

    pub fn with_entity_counts(mut self, entity_counts: SceneEntityCounts) -> Self {
        self.scene_owned_entities = entity_counts.total_scene_owned;
        self.layer_count = entity_counts.layer_roots;
        self.entity_counts = entity_counts;
        self
    }
}
