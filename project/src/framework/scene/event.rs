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

    pub fn with_scene(mut self, scene_id: impl Into<SceneId>) -> Self {
        self.scene_id = Some(scene_id.into());
        self
    }

    pub fn with_optional_scene(mut self, scene_id: Option<SceneId>) -> Self {
        self.scene_id = scene_id;
        self
    }

    pub fn with_session(mut self, session_id: impl Into<SceneSessionId>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    pub fn with_optional_session(mut self, session_id: Option<SceneSessionId>) -> Self {
        self.session_id = session_id;
        self
    }

    pub fn with_content_version(mut self, content_version: impl Into<String>) -> Self {
        self.content_version = Some(content_version.into());
        self
    }

    pub fn with_optional_content_version(mut self, content_version: Option<String>) -> Self {
        self.content_version = content_version;
        self
    }

    pub fn with_asset_id(mut self, asset_id: impl Into<SceneAssetId>) -> Self {
        self.asset_id = Some(asset_id.into());
        self
    }

    pub fn with_optional_asset_id(mut self, asset_id: Option<SceneAssetId>) -> Self {
        self.asset_id = asset_id;
        self
    }

    pub fn with_asset_path(mut self, asset_path: impl Into<String>) -> Self {
        self.asset_path = Some(asset_path.into());
        self
    }

    pub fn with_optional_asset_path(mut self, asset_path: Option<String>) -> Self {
        self.asset_path = asset_path;
        self
    }

    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    pub fn message_key(&self) -> &'static str {
        self.kind.message_key()
    }

    pub fn log_description(&self) -> String {
        let mut description = format!(
            "kind={:?} state={:?} ui_key={}",
            self.kind,
            self.state,
            self.message_key()
        );

        if let Some(scene_id) = &self.scene_id {
            description.push_str(&format!(" scene_id={scene_id}"));
        }

        if let Some(session_id) = &self.session_id {
            description.push_str(&format!(" session_id={session_id}"));
        }

        if let Some(content_version) = &self.content_version {
            description.push_str(&format!(" content_version={content_version}"));
        }

        if let Some(asset_id) = &self.asset_id {
            description.push_str(&format!(" asset_id={asset_id}"));
        }

        if let Some(asset_path) = &self.asset_path {
            description.push_str(&format!(" asset_path={asset_path}"));
        }

        if let Some(message) = &self.message {
            description.push_str(&format!(" message={message}"));
        }

        description
    }
}

impl fmt::Display for SceneFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "scene failure {}", self.log_description())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failure_builder_fills_diagnostic_fields() {
        let failure = SceneFailure::new(
            SceneFailureKind::RequiredAssetMissing,
            SceneLifecycleState::LoadingAssets,
        )
        .with_scene("arena")
        .with_session("arena-1")
        .with_content_version("v1")
        .with_asset_id("hero")
        .with_asset_path("scenes/arena/hero.png")
        .with_message("asset failed before load");

        assert_eq!(failure.message_key(), "scene.error.required_asset_missing");
        assert!(failure.log_description().contains("scene_id=arena"));
        assert!(failure.log_description().contains("session_id=arena-1"));
        assert!(failure.log_description().contains("content_version=v1"));
        assert!(
            failure
                .log_description()
                .contains("asset_path=scenes/arena/hero.png")
        );
    }
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
