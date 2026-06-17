use serde::{Deserialize, Deserializer};
use std::{
    collections::HashSet,
    fmt, fs, io,
    path::{Path, PathBuf},
};

use super::{
    id::{
        SCENE_ID_ALLOWED_CHARACTERS, SceneAssetId, SceneId, SceneIdError, SceneLayerId,
        SceneSpawnPointId,
    },
    loading::SceneLoadingPolicy,
    registry::SceneKind,
    spawn::{SceneAnchorManifest, SceneSpawnPointManifest},
    trigger::SceneTriggerManifest,
};

/// Scene manifests currently support exactly this version.
pub const SCENE_MANIFEST_VERSION: &str = "1";

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SceneManifest {
    pub version: String,
    pub scene_id: SceneId,
    pub kind: SceneKind,
    #[serde(default)]
    pub entry: SceneManifestEntry,
    #[serde(default)]
    pub layers: Vec<SceneLayerManifest>,
    #[serde(default)]
    pub spawn_points: Vec<SceneSpawnPointManifest>,
    #[serde(default)]
    pub anchors: Vec<SceneAnchorManifest>,
    #[serde(default)]
    pub triggers: Vec<SceneTriggerManifest>,
}

impl Default for SceneManifest {
    fn default() -> Self {
        Self::new(SceneId::from("scene"), SceneKind::default())
    }
}

impl SceneManifest {
    pub fn new(scene_id: impl Into<SceneId>, kind: SceneKind) -> Self {
        Self {
            version: SCENE_MANIFEST_VERSION.to_string(),
            scene_id: scene_id.into(),
            kind,
            entry: SceneManifestEntry::default(),
            layers: Vec::new(),
            spawn_points: Vec::new(),
            anchors: Vec::new(),
            triggers: Vec::new(),
        }
    }

    pub fn with_entry(mut self, entry: SceneManifestEntry) -> Self {
        self.entry = entry;
        self
    }

    pub fn with_layer(mut self, layer: SceneLayerManifest) -> Self {
        self.layers.push(layer);
        self
    }

    pub fn with_spawn_point(mut self, spawn_point: SceneSpawnPointManifest) -> Self {
        self.spawn_points.push(spawn_point);
        self
    }

    pub fn with_anchor(mut self, anchor: SceneAnchorManifest) -> Self {
        self.anchors.push(anchor);
        self
    }

    pub fn with_trigger(mut self, trigger: SceneTriggerManifest) -> Self {
        self.triggers.push(trigger);
        self
    }

    pub fn is_supported_version(version: &str) -> bool {
        version == SCENE_MANIFEST_VERSION
    }

    pub fn load_first_package_ron(
        manifest_path: impl AsRef<str>,
    ) -> Result<Self, SceneManifestLoadError> {
        let manifest_path = manifest_path.as_ref();
        validate_asset_relative_path(manifest_path)
            .map_err(SceneManifestLoadError::UnsafeManifestPath)?;

        let fs_path = first_package_manifest_fs_path(manifest_path)
            .ok_or_else(|| SceneManifestLoadError::ManifestNotFound(manifest_path.to_string()))?;

        let manifest_source =
            fs::read_to_string(&fs_path).map_err(|source| SceneManifestLoadError::ReadFailed {
                path: fs_path.clone(),
                source,
            })?;

        let manifest = ron::from_str::<Self>(&manifest_source).map_err(|source| {
            SceneManifestLoadError::ParseFailed {
                path: fs_path,
                source,
            }
        })?;

        manifest
            .validate_basic()
            .map_err(SceneManifestLoadError::ValidationFailed)?;

        Ok(manifest)
    }

    pub fn validate_basic(&self) -> Result<(), SceneManifestError> {
        if !Self::is_supported_version(&self.version) {
            return Err(SceneManifestError::UnsupportedVersion {
                found: self.version.clone(),
                expected: SCENE_MANIFEST_VERSION,
            });
        }

        self.scene_id.validate().map_err(SceneManifestError::from)?;

        if self
            .entry
            .camera
            .as_ref()
            .is_some_and(SceneCameraRef::is_empty)
        {
            return Err(SceneManifestError::EmptyCameraRef {
                scene_id: self.scene_id.clone(),
            });
        }

        let mut layer_ids = HashSet::new();
        let mut asset_ids = HashSet::new();

        for (layer_index, layer) in self.layers.iter().enumerate() {
            if layer.id.is_empty() {
                return Err(SceneManifestError::EmptyLayerId { index: layer_index });
            }

            if !layer_ids.insert(layer.id.clone()) {
                return Err(SceneManifestError::DuplicateLayerId(layer.id.clone()));
            }

            if layer.required && layer.assets.is_empty() {
                return Err(SceneManifestError::RequiredLayerWithoutAssets(
                    layer.id.clone(),
                ));
            }

            for (asset_index, asset) in layer.assets.iter().enumerate() {
                if asset.id.is_empty() {
                    return Err(SceneManifestError::EmptyAssetId {
                        layer_id: layer.id.clone(),
                        index: asset_index,
                    });
                }

                if !asset_ids.insert(asset.id.clone()) {
                    return Err(SceneManifestError::DuplicateAssetId {
                        layer_id: layer.id.clone(),
                        asset_id: asset.id.clone(),
                    });
                }

                if asset.kind.is_empty() {
                    return Err(SceneManifestError::EmptyAssetKind {
                        layer_id: layer.id.clone(),
                        asset_id: asset.id.clone(),
                    });
                }

                if asset.path.trim().is_empty() {
                    return Err(SceneManifestError::EmptyAssetPath {
                        layer_id: layer.id.clone(),
                        asset_id: asset.id.clone(),
                    });
                }

                if is_unsafe_asset_path(&asset.path) {
                    return Err(SceneManifestError::UnsafeAssetPath {
                        layer_id: layer.id.clone(),
                        asset_id: asset.id.clone(),
                        path: asset.path.clone(),
                    });
                }
            }
        }

        let mut spawn_point_ids = HashSet::new();
        for (spawn_index, spawn) in self.spawn_points.iter().enumerate() {
            if spawn.id.is_empty() {
                return Err(SceneManifestError::EmptySpawnPointId { index: spawn_index });
            }

            if !spawn_point_ids.insert(spawn.id.clone()) {
                return Err(SceneManifestError::DuplicateSpawnPointId(spawn.id.clone()));
            }
        }

        if let Some(default_spawn) = &self.entry.default_spawn {
            if default_spawn.is_empty() {
                return Err(SceneManifestError::EmptyDefaultSpawn);
            }

            if !spawn_point_ids.contains(default_spawn) {
                return Err(SceneManifestError::DefaultSpawnMissing(
                    default_spawn.clone(),
                ));
            }
        }

        let mut anchor_ids = HashSet::new();
        for (anchor_index, anchor) in self.anchors.iter().enumerate() {
            if anchor.id.is_empty() {
                return Err(SceneManifestError::EmptyAnchorId {
                    index: anchor_index,
                });
            }

            if !anchor_ids.insert(anchor.id.clone()) {
                return Err(SceneManifestError::DuplicateAnchorId(anchor.id.clone()));
            }
        }

        let mut trigger_ids = HashSet::new();
        for (trigger_index, trigger) in self.triggers.iter().enumerate() {
            if trigger.id.is_empty() {
                return Err(SceneManifestError::EmptyTriggerId {
                    index: trigger_index,
                });
            }

            if !trigger_ids.insert(trigger.id.clone()) {
                return Err(SceneManifestError::DuplicateTriggerId(trigger.id.clone()));
            }

            if trigger.event.trim().is_empty() {
                return Err(SceneManifestError::EmptyTriggerEvent {
                    trigger_id: trigger.id.clone(),
                });
            }
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct SceneManifestEntry {
    pub default_spawn: Option<SceneSpawnPointId>,
    pub camera: Option<SceneCameraRef>,
    pub loading_policy: SceneLoadingPolicy,
}

impl SceneManifestEntry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_default_spawn(mut self, default_spawn: impl Into<SceneSpawnPointId>) -> Self {
        self.default_spawn = Some(default_spawn.into());
        self
    }

    pub fn with_camera(mut self, camera: impl Into<SceneCameraRef>) -> Self {
        self.camera = Some(camera.into());
        self
    }

    pub fn with_loading_policy(mut self, loading_policy: SceneLoadingPolicy) -> Self {
        self.loading_policy = loading_policy;
        self
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Hash)]
pub struct SceneCameraRef(String);

impl SceneCameraRef {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<&str> for SceneCameraRef {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for SceneCameraRef {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl AsRef<str> for SceneCameraRef {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for SceneCameraRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct SceneLayerManifest {
    pub id: SceneLayerId,
    pub required: bool,
    pub assets: Vec<SceneAssetRef>,
}

impl Default for SceneLayerManifest {
    fn default() -> Self {
        Self::new(SceneLayerId::from(""))
    }
}

impl SceneLayerManifest {
    pub fn new(id: impl Into<SceneLayerId>) -> Self {
        Self {
            id: id.into(),
            required: true,
            assets: Vec::new(),
        }
    }

    pub fn optional(id: impl Into<SceneLayerId>) -> Self {
        Self {
            required: false,
            ..Self::new(id)
        }
    }

    pub fn with_required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    pub fn with_asset(mut self, asset: SceneAssetRef) -> Self {
        self.assets.push(asset);
        self
    }

    pub fn add_asset(&mut self, asset: SceneAssetRef) {
        self.assets.push(asset);
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct SceneAssetRef {
    pub id: SceneAssetId,
    pub kind: SceneAssetKind,
    pub path: String,
    pub label: Option<String>,
}

impl Default for SceneAssetRef {
    fn default() -> Self {
        Self::new(
            SceneAssetId::from(""),
            SceneAssetKind::Other(String::new()),
            "",
        )
    }
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

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
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

impl Default for SceneAssetKind {
    fn default() -> Self {
        Self::Other(String::new())
    }
}

impl SceneAssetKind {
    pub fn other(value: impl Into<String>) -> Self {
        Self::Other(value.into())
    }

    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Other(value) if value.trim().is_empty())
    }
}

impl<'de> Deserialize<'de> for SceneAssetKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match normalize_manifest_token(&value).as_str() {
            "gltfscene" | "gltf" | "glb" => Self::GltfScene,
            "image" | "texture" => Self::Image,
            "audio" | "sound" => Self::Audio,
            "ron" => Self::Ron,
            "json" => Self::Json,
            _ => Self::Other(value),
        })
    }
}

pub(crate) fn validate_asset_relative_path(path: &str) -> Result<(), SceneManifestPathError> {
    if path.trim().is_empty() {
        return Err(SceneManifestPathError::Empty);
    }

    if is_unsafe_asset_path(path) {
        return Err(SceneManifestPathError::UnsafePath(path.to_string()));
    }

    Ok(())
}

pub(crate) fn asset_path_with_label(asset: &SceneAssetRef) -> String {
    match asset.label.as_deref() {
        Some(label) if !label.is_empty() => format!("{}#{label}", asset.path),
        _ => asset.path.clone(),
    }
}

pub(crate) fn normalize_manifest_token(value: &str) -> String {
    value
        .trim()
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|character| !matches!(character, '-' | '_' | ' '))
        .collect()
}

fn first_package_manifest_fs_path(manifest_path: &str) -> Option<PathBuf> {
    first_package_asset_root_candidates()
        .into_iter()
        .map(|root| root.join(Path::new(manifest_path)))
        .find(|candidate| candidate.is_file())
}

fn first_package_asset_root_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(current_dir) = std::env::current_dir() {
        candidates.push(current_dir.join("assets"));
        candidates.push(current_dir.join("project").join("assets"));
    }
    candidates.push(PathBuf::from("assets"));
    candidates.push(PathBuf::from("project").join("assets"));
    candidates
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SceneManifestError {
    EmptySceneId,
    InvalidSceneIdFormat(SceneId),
    UnsupportedVersion {
        found: String,
        expected: &'static str,
    },
    EmptyCameraRef {
        scene_id: SceneId,
    },
    EmptyLayerId {
        index: usize,
    },
    DuplicateLayerId(SceneLayerId),
    RequiredLayerWithoutAssets(SceneLayerId),
    EmptyAssetId {
        layer_id: SceneLayerId,
        index: usize,
    },
    DuplicateAssetId {
        layer_id: SceneLayerId,
        asset_id: SceneAssetId,
    },
    EmptyAssetKind {
        layer_id: SceneLayerId,
        asset_id: SceneAssetId,
    },
    EmptyAssetPath {
        layer_id: SceneLayerId,
        asset_id: SceneAssetId,
    },
    UnsafeAssetPath {
        layer_id: SceneLayerId,
        asset_id: SceneAssetId,
        path: String,
    },
    EmptyDefaultSpawn,
    DefaultSpawnMissing(SceneSpawnPointId),
    EmptySpawnPointId {
        index: usize,
    },
    DuplicateSpawnPointId(SceneSpawnPointId),
    EmptyAnchorId {
        index: usize,
    },
    DuplicateAnchorId(super::id::SceneAnchorId),
    EmptyTriggerId {
        index: usize,
    },
    DuplicateTriggerId(super::id::SceneTriggerId),
    EmptyTriggerEvent {
        trigger_id: super::id::SceneTriggerId,
    },
}

#[derive(Debug)]
pub enum SceneManifestLoadError {
    UnsafeManifestPath(SceneManifestPathError),
    ManifestNotFound(String),
    ReadFailed {
        path: PathBuf,
        source: io::Error,
    },
    ParseFailed {
        path: PathBuf,
        source: ron::error::SpannedError,
    },
    ValidationFailed(SceneManifestError),
}

impl fmt::Display for SceneManifestLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsafeManifestPath(error) => write!(formatter, "{error}"),
            Self::ManifestNotFound(path) => {
                write!(
                    formatter,
                    "scene manifest was not found under the first package assets root: {path}"
                )
            }
            Self::ReadFailed { path, source } => {
                write!(
                    formatter,
                    "failed to read scene manifest at {}: {source}",
                    path.display()
                )
            }
            Self::ParseFailed { path, source } => {
                write!(
                    formatter,
                    "failed to parse scene manifest RON at {}: {source}",
                    path.display()
                )
            }
            Self::ValidationFailed(error) => {
                write!(formatter, "scene manifest validation failed: {error}")
            }
        }
    }
}

impl std::error::Error for SceneManifestLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::UnsafeManifestPath(error) => Some(error),
            Self::ReadFailed { source, .. } => Some(source),
            Self::ParseFailed { source, .. } => Some(source),
            Self::ValidationFailed(error) => Some(error),
            Self::ManifestNotFound(_) => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SceneManifestPathError {
    Empty,
    UnsafePath(String),
}

impl fmt::Display for SceneManifestPathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("scene manifest path must not be empty"),
            Self::UnsafePath(path) => write!(
                formatter,
                "scene manifest path must be relative to assets and stay inside assets: {path}"
            ),
        }
    }
}

impl std::error::Error for SceneManifestPathError {}

impl fmt::Display for SceneManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySceneId => formatter.write_str("scene manifest scene_id must not be empty"),
            Self::InvalidSceneIdFormat(scene_id) => {
                write!(
                    formatter,
                    "scene manifest scene_id has invalid format: {scene_id}; allowed characters are {SCENE_ID_ALLOWED_CHARACTERS}"
                )
            }
            Self::UnsupportedVersion { found, expected } => {
                write!(
                    formatter,
                    "unsupported scene manifest version: {found}; expected {expected}"
                )
            }
            Self::EmptyCameraRef { scene_id } => {
                write!(
                    formatter,
                    "scene manifest camera reference must not be empty for scene: {scene_id}"
                )
            }
            Self::EmptyLayerId { index } => {
                write!(
                    formatter,
                    "scene manifest layer id must not be empty at index: {index}"
                )
            }
            Self::DuplicateLayerId(layer_id) => {
                write!(
                    formatter,
                    "scene manifest layer id is duplicated: {layer_id}"
                )
            }
            Self::RequiredLayerWithoutAssets(layer_id) => {
                write!(
                    formatter,
                    "required scene layer must define at least one asset: {layer_id}"
                )
            }
            Self::EmptyAssetId { layer_id, index } => {
                write!(
                    formatter,
                    "scene asset id must not be empty in layer {layer_id} at index: {index}"
                )
            }
            Self::DuplicateAssetId { layer_id, asset_id } => {
                write!(
                    formatter,
                    "scene asset id is duplicated in layer {layer_id}: {asset_id}"
                )
            }
            Self::EmptyAssetKind { layer_id, asset_id } => {
                write!(
                    formatter,
                    "scene asset kind must not be empty for asset {asset_id} in layer {layer_id}"
                )
            }
            Self::EmptyAssetPath { layer_id, asset_id } => {
                write!(
                    formatter,
                    "scene asset path must not be empty for asset {asset_id} in layer {layer_id}"
                )
            }
            Self::UnsafeAssetPath {
                layer_id,
                asset_id,
                path,
            } => {
                write!(
                    formatter,
                    "scene asset path is unsafe for asset {asset_id} in layer {layer_id}: {path}"
                )
            }
            Self::EmptyDefaultSpawn => {
                formatter.write_str("default spawn point id must not be empty")
            }
            Self::DefaultSpawnMissing(spawn_id) => {
                write!(formatter, "default spawn point is not defined: {spawn_id}")
            }
            Self::EmptySpawnPointId { index } => {
                write!(
                    formatter,
                    "scene spawn point id must not be empty at index: {index}"
                )
            }
            Self::DuplicateSpawnPointId(spawn_id) => {
                write!(formatter, "scene spawn point id is duplicated: {spawn_id}")
            }
            Self::EmptyAnchorId { index } => {
                write!(
                    formatter,
                    "scene anchor id must not be empty at index: {index}"
                )
            }
            Self::DuplicateAnchorId(anchor_id) => {
                write!(formatter, "scene anchor id is duplicated: {anchor_id}")
            }
            Self::EmptyTriggerId { index } => {
                write!(
                    formatter,
                    "scene trigger id must not be empty at index: {index}"
                )
            }
            Self::DuplicateTriggerId(trigger_id) => {
                write!(formatter, "scene trigger id is duplicated: {trigger_id}")
            }
            Self::EmptyTriggerEvent { trigger_id } => {
                write!(
                    formatter,
                    "scene trigger event must not be empty for trigger: {trigger_id}"
                )
            }
        }
    }
}

impl std::error::Error for SceneManifestError {}

impl From<SceneIdError> for SceneManifestError {
    fn from(error: SceneIdError) -> Self {
        match error {
            SceneIdError::Empty => Self::EmptySceneId,
            SceneIdError::InvalidFormat(value) => Self::InvalidSceneIdFormat(SceneId::from(value)),
        }
    }
}

fn is_unsafe_asset_path(path: &str) -> bool {
    path.starts_with('/')
        || path.starts_with('\\')
        || path.contains('\\')
        || has_windows_drive_prefix(path)
        || path.split('/').any(|segment| segment == "..")
}

fn has_windows_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic()
}
