use bevy::prelude::*;

use super::{
    id::{SceneId, SceneLayerId, SceneSessionId, SceneSpawnPointId},
    lifecycle::SceneAuthorityMode,
};

#[derive(Clone, Debug, Message, PartialEq)]
pub enum SceneCommand {
    Enter(SceneEnterRequest),
    Exit(SceneExitRequest),
    Switch(SceneSwitchRequest),
    Preload(ScenePreloadRequest),
    Unload(SceneUnloadRequest),
    ReloadCurrent(SceneReloadRequest),
    SetLayerEnabled(SceneLayerCommand),
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneEnterRequest {
    pub scene_id: SceneId,
    pub session_id: Option<SceneSessionId>,
    pub spawn_point: Option<SceneSpawnPointId>,
    pub content_version: Option<String>,
    pub transition: SceneTransition,
    pub authority_mode: SceneAuthorityMode,
    pub seed: Option<u64>,
}

impl SceneEnterRequest {
    pub fn new(scene_id: impl Into<SceneId>) -> Self {
        Self {
            scene_id: scene_id.into(),
            session_id: None,
            spawn_point: None,
            content_version: None,
            transition: SceneTransition::default(),
            authority_mode: SceneAuthorityMode::default(),
            seed: None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SceneExitRequest {
    pub scene_id: Option<SceneId>,
    pub session_id: Option<SceneSessionId>,
    pub transition: SceneTransition,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneSwitchRequest {
    pub exit: SceneExitRequest,
    pub enter: SceneEnterRequest,
}

impl SceneSwitchRequest {
    pub fn new(scene_id: impl Into<SceneId>) -> Self {
        Self {
            exit: SceneExitRequest::default(),
            enter: SceneEnterRequest::new(scene_id),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScenePreloadRequest {
    pub scene_id: SceneId,
    pub content_version: Option<String>,
}

impl ScenePreloadRequest {
    pub fn new(scene_id: impl Into<SceneId>) -> Self {
        Self {
            scene_id: scene_id.into(),
            content_version: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneUnloadRequest {
    pub scene_id: SceneId,
    pub content_version: Option<String>,
}

impl SceneUnloadRequest {
    pub fn new(scene_id: impl Into<SceneId>) -> Self {
        Self {
            scene_id: scene_id.into(),
            content_version: None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SceneReloadRequest {
    pub session_id: Option<SceneSessionId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneLayerCommand {
    pub layer_id: SceneLayerId,
    pub enabled: bool,
}

impl SceneLayerCommand {
    pub fn new(layer_id: impl Into<SceneLayerId>, enabled: bool) -> Self {
        Self {
            layer_id: layer_id.into(),
            enabled,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SceneTransition {
    #[default]
    Instant,
    Loading,
    Fade,
}
