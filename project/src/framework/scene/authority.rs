use bevy::prelude::*;

use super::{
    id::{SceneId, SceneSessionId},
    lifecycle::SceneAuthorityMode,
};

#[derive(Clone, Debug, Message, PartialEq, Eq)]
pub struct SceneAuthorityReadyRequest {
    pub scene_id: SceneId,
    pub session_id: SceneSessionId,
    pub authority_mode: SceneAuthorityMode,
    pub content_version: Option<String>,
    pub seed: Option<u64>,
}

#[derive(Clone, Debug, Message, PartialEq, Eq)]
pub struct SceneAuthorityReadyStatus {
    pub scene_id: SceneId,
    pub session_id: SceneSessionId,
    pub status: SceneAuthorityReadyState,
    pub content_version: Option<String>,
    pub message: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SceneAuthorityReadyState {
    Pending,
    Ready,
    Rejected,
}

pub trait SceneAuthorityAdapter: Send + Sync + 'static {
    fn request_scene_ready(&mut self, request: SceneAuthorityReadyRequest);
}
