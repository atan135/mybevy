use bevy::prelude::*;
use std::time::Duration;

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

    pub fn active_scene_id(&self) -> Option<&SceneId> {
        self.active.as_ref().map(|session| &session.scene_id)
    }

    pub fn active_session_id(&self) -> Option<&SceneSessionId> {
        self.active.as_ref().map(|session| &session.session_id)
    }

    pub fn pending_scene_id(&self) -> Option<&SceneId> {
        self.pending.as_ref().map(|session| &session.scene_id)
    }

    pub fn pending_session_id(&self) -> Option<&SceneSessionId> {
        self.pending.as_ref().map(|session| &session.session_id)
    }

    pub fn has_active(&self) -> bool {
        self.active.is_some()
    }

    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    pub fn is_active_scene(&self, scene_id: &SceneId) -> bool {
        self.active
            .as_ref()
            .is_some_and(|session| &session.scene_id == scene_id)
    }

    pub fn is_pending_scene(&self, scene_id: &SceneId) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|session| &session.scene_id == scene_id)
    }

    pub fn is_idle(&self) -> bool {
        self.state.is_idle()
    }

    pub fn is_loading(&self) -> bool {
        self.state.is_loading()
    }

    pub fn is_transitioning(&self) -> bool {
        self.state.is_transitioning()
    }

    pub fn is_failed(&self) -> bool {
        self.state.is_failed()
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
    pub entered_at: Option<Duration>,
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
            entered_at: None,
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

impl SceneLifecycleState {
    pub fn is_idle(self) -> bool {
        matches!(self, Self::Idle)
    }

    pub fn is_loading(self) -> bool {
        matches!(
            self,
            Self::Resolving | Self::Downloading | Self::LoadingAssets | Self::Instantiating
        )
    }

    pub fn is_transitioning(self) -> bool {
        matches!(
            self,
            Self::Resolving
                | Self::Downloading
                | Self::LoadingAssets
                | Self::Instantiating
                | Self::Activating
                | Self::Deactivating
                | Self::Unloading
                | Self::Suspending
        )
    }

    pub fn is_active(self) -> bool {
        matches!(self, Self::Active)
    }

    pub fn is_failed(self) -> bool {
        matches!(self, Self::Failed)
    }
}
