use std::fmt;

use super::{
    id::{SceneAssetId, SceneId, SceneLayerId, SceneSpawnPointId},
    loading::SceneLoadingPolicy,
    registry::SceneKind,
    spawn::{SceneAnchorManifest, SceneSpawnPointManifest},
    trigger::SceneTriggerManifest,
};

pub const SCENE_MANIFEST_VERSION: &str = "1";

#[derive(Clone, Debug, PartialEq)]
pub struct SceneManifest {
    pub version: String,
    pub scene_id: SceneId,
    pub kind: SceneKind,
    pub entry: SceneManifestEntry,
    pub layers: Vec<SceneLayerManifest>,
    pub spawn_points: Vec<SceneSpawnPointManifest>,
    pub anchors: Vec<SceneAnchorManifest>,
    pub triggers: Vec<SceneTriggerManifest>,
}

impl SceneManifest {
    pub fn validate_basic(&self) -> Result<(), SceneManifestError> {
        if self.version != SCENE_MANIFEST_VERSION {
            return Err(SceneManifestError::UnsupportedVersion(self.version.clone()));
        }

        if self.scene_id.is_empty() {
            return Err(SceneManifestError::EmptySceneId);
        }

        if let Some(default_spawn) = &self.entry.default_spawn {
            let has_default_spawn = self
                .spawn_points
                .iter()
                .any(|spawn| &spawn.id == default_spawn);
            if !has_default_spawn {
                return Err(SceneManifestError::DefaultSpawnMissing(
                    default_spawn.clone(),
                ));
            }
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SceneManifestEntry {
    pub default_spawn: Option<SceneSpawnPointId>,
    pub camera: Option<String>,
    pub loading_policy: SceneLoadingPolicy,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneLayerManifest {
    pub id: SceneLayerId,
    pub required: bool,
    pub assets: Vec<SceneAssetRef>,
}

impl SceneLayerManifest {
    pub fn new(id: impl Into<SceneLayerId>) -> Self {
        Self {
            id: id.into(),
            required: true,
            assets: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneAssetRef {
    pub id: SceneAssetId,
    pub kind: SceneAssetKind,
    pub path: String,
    pub label: Option<String>,
}

impl SceneAssetRef {
    pub fn new(id: impl Into<SceneAssetId>, kind: SceneAssetKind, path: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind,
            path: path.into(),
            label: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SceneAssetKind {
    GltfScene,
    Image,
    Audio,
    Ron,
    Json,
    Other(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SceneManifestError {
    EmptySceneId,
    UnsupportedVersion(String),
    DefaultSpawnMissing(SceneSpawnPointId),
}

impl fmt::Display for SceneManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySceneId => formatter.write_str("scene manifest scene_id must not be empty"),
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported scene manifest version: {version}")
            }
            Self::DefaultSpawnMissing(spawn_id) => {
                write!(formatter, "default spawn point is not defined: {spawn_id}")
            }
        }
    }
}
