use bevy::prelude::*;

use super::id::{SceneId, SceneLayerId, SceneSessionId};

#[derive(Clone, Debug, Component, PartialEq, Eq)]
pub struct SceneRoot {
    pub scene_id: SceneId,
    pub session_id: SceneSessionId,
}

impl SceneRoot {
    pub fn new(scene_id: impl Into<SceneId>, session_id: impl Into<SceneSessionId>) -> Self {
        Self {
            scene_id: scene_id.into(),
            session_id: session_id.into(),
        }
    }
}

#[derive(Clone, Debug, Component, PartialEq, Eq)]
pub struct SceneLayerRoot {
    pub session_id: SceneSessionId,
    pub layer_id: SceneLayerId,
    pub state: SceneLayerState,
    pub required: bool,
}

impl SceneLayerRoot {
    pub fn new(session_id: impl Into<SceneSessionId>, layer_id: impl Into<SceneLayerId>) -> Self {
        Self {
            session_id: session_id.into(),
            layer_id: layer_id.into(),
            state: SceneLayerState::default(),
            required: true,
        }
    }

    pub fn optional(mut self) -> Self {
        self.required = false;
        self
    }
}

#[derive(Clone, Debug, Component, PartialEq, Eq)]
pub struct SceneOwned {
    pub session_id: SceneSessionId,
}

impl SceneOwned {
    pub fn new(session_id: impl Into<SceneSessionId>) -> Self {
        Self {
            session_id: session_id.into(),
        }
    }
}

#[derive(Clone, Debug, Component, PartialEq, Eq)]
pub struct SceneRuntimeRoot {
    pub session_id: SceneSessionId,
}

impl SceneRuntimeRoot {
    pub fn new(session_id: impl Into<SceneSessionId>) -> Self {
        Self {
            session_id: session_id.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SceneLayerState {
    #[default]
    Registered,
    Loading,
    Loaded,
    Active,
    Unloading,
    Failed,
}
