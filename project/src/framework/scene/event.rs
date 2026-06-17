use std::fmt;

use bevy::prelude::*;

use super::{
    id::{SceneAssetId, SceneChunkId, SceneId, SceneLayerId, SceneSessionId},
    lifecycle::SceneLifecycleState,
    loading::SceneLoadProgress,
    root::SceneLayerState,
};

#[derive(Clone, Debug, Message, PartialEq)]
pub enum SceneEvent {
    Resolving(SceneResolving),
    LoadProgress(SceneLoadProgress),
    Instantiating(SceneInstantiating),
    Entered(SceneEntered),
    Ready(SceneReady),
    ExitStarted(SceneExitStarted),
    Exited(SceneExited),
    LayerLoaded(SceneLayerStatusEvent),
    LayerUnloaded(SceneLayerStatusEvent),
    ChunkLoaded(SceneChunkStatusEvent),
    ChunkUnloaded(SceneChunkStatusEvent),
    Failed(SceneFailure),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneResolving {
    pub scene_id: SceneId,
    pub session_id: Option<SceneSessionId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneInstantiating {
    pub scene_id: SceneId,
    pub session_id: SceneSessionId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneEntered {
    pub scene_id: SceneId,
    pub session_id: SceneSessionId,
    pub content_version: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneReady {
    pub scene_id: SceneId,
    pub session_id: SceneSessionId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneExitStarted {
    pub scene_id: SceneId,
    pub session_id: SceneSessionId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneExited {
    pub scene_id: SceneId,
    pub session_id: SceneSessionId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneLayerStatusEvent {
    pub scene_id: SceneId,
    pub session_id: SceneSessionId,
    pub layer_id: SceneLayerId,
    pub state: SceneLayerState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneChunkStatusEvent {
    pub scene_id: SceneId,
    pub session_id: SceneSessionId,
    pub chunk_id: SceneChunkId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneFailure {
    pub kind: SceneFailureKind,
    pub scene_id: Option<SceneId>,
    pub session_id: Option<SceneSessionId>,
    pub content_version: Option<String>,
    pub state: SceneLifecycleState,
    pub asset_id: Option<SceneAssetId>,
    pub asset_path: Option<String>,
    pub message: Option<String>,
}

impl SceneFailure {
    pub fn new(kind: SceneFailureKind, state: SceneLifecycleState) -> Self {
        Self {
            kind,
            scene_id: None,
            session_id: None,
            content_version: None,
            state,
            asset_id: None,
            asset_path: None,
            message: None,
        }
    }

    pub fn message_key(&self) -> &'static str {
        self.kind.message_key()
    }
}

impl fmt::Display for SceneFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "scene failure {:?} while {:?}",
            self.kind, self.state
        )?;

        if let Some(scene_id) = &self.scene_id {
            write!(formatter, " scene_id={scene_id}")?;
        }

        if let Some(session_id) = &self.session_id {
            write!(formatter, " session_id={session_id}")?;
        }

        if let Some(asset_path) = &self.asset_path {
            write!(formatter, " asset_path={asset_path}")?;
        }

        if let Some(message) = &self.message {
            write!(formatter, " message={message}")?;
        }

        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SceneFailureKind {
    SceneNotFound,
    ManifestLoadFailed,
    ManifestParseFailed,
    ManifestVersionUnsupported,
    ContentVersionMissing,
    ContentHashMismatch,
    RequiredAssetMissing,
    AssetLoadFailed,
    SceneInstanceFailed,
    SpawnPointMissing,
    CameraSetupFailed,
    AuthorityRejected,
    NetworkTimeout,
    OutOfMemoryRisk,
    Unknown,
}

impl SceneFailureKind {
    pub fn message_key(self) -> &'static str {
        match self {
            Self::SceneNotFound => "scene.error.not_found",
            Self::ManifestLoadFailed => "scene.error.manifest_load_failed",
            Self::ManifestParseFailed => "scene.error.manifest_parse_failed",
            Self::ManifestVersionUnsupported => "scene.error.manifest_version_unsupported",
            Self::ContentVersionMissing => "scene.error.content_version_missing",
            Self::ContentHashMismatch => "scene.error.content_hash_mismatch",
            Self::RequiredAssetMissing => "scene.error.required_asset_missing",
            Self::AssetLoadFailed => "scene.error.asset_load_failed",
            Self::SceneInstanceFailed => "scene.error.instance_failed",
            Self::SpawnPointMissing => "scene.error.spawn_point_missing",
            Self::CameraSetupFailed => "scene.error.camera_setup_failed",
            Self::AuthorityRejected => "scene.error.authority_rejected",
            Self::NetworkTimeout => "scene.error.network_timeout",
            Self::OutOfMemoryRisk => "scene.error.out_of_memory_risk",
            Self::Unknown => "scene.error.unknown",
        }
    }
}
