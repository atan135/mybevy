use bevy::prelude::*;

use super::{
    event::SceneFailure,
    id::{SceneId, SceneSessionId, SceneSpawnPointId},
};

#[derive(Clone, Debug, Resource, PartialEq)]
pub struct SceneRuntime {
    pub active: Option<SceneSessionInfo>,
    pub pending: Option<SceneSessionInfo>,
    pub state: SceneLifecycleState,
    pub last_error: Option<SceneFailure>,
}

impl Default for SceneRuntime {
    fn default() -> Self {
        Self {
            active: None,
            pending: None,
            state: SceneLifecycleState::Idle,
            last_error: None,
        }
    }
}

impl SceneRuntime {
    pub fn active(&self) -> Option<&SceneSessionInfo> {
        self.active.as_ref()
    }

    pub fn pending(&self) -> Option<&SceneSessionInfo> {
        self.pending.as_ref()
    }

    pub fn state(&self) -> SceneLifecycleState {
        self.state
    }

    pub fn last_error(&self) -> Option<&SceneFailure> {
        self.last_error.as_ref()
    }

    pub fn is_active_scene(&self, scene_id: &SceneId) -> bool {
        self.active
            .as_ref()
            .is_some_and(|session| &session.scene_id == scene_id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneSessionInfo {
    pub scene_id: SceneId,
    pub session_id: SceneSessionId,
    pub authority_mode: SceneAuthorityMode,
    pub content_version: Option<String>,
    pub spawn_point: Option<SceneSpawnPointId>,
    pub seed: Option<u64>,
    pub entered_at_seconds: Option<u64>,
}

impl SceneSessionInfo {
    pub fn new(scene_id: impl Into<SceneId>, session_id: impl Into<SceneSessionId>) -> Self {
        Self {
            scene_id: scene_id.into(),
            session_id: session_id.into(),
            authority_mode: SceneAuthorityMode::default(),
            content_version: None,
            spawn_point: None,
            seed: None,
            entered_at_seconds: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SceneAuthorityMode {
    #[default]
    Local,
    LocalHost,
    Remote,
    External,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SceneLifecycleState {
    #[default]
    Idle,
    Resolving,
    Downloading,
    LoadingAssets,
    Instantiating,
    Activating,
    Active,
    Suspending,
    Deactivating,
    Unloading,
    Failed,
}
