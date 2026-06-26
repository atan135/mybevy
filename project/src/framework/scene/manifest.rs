use bevy::{math::EulerRot, prelude::*};
use serde::{Deserialize, Deserializer};
use std::{
    collections::HashSet,
    fmt, fs, io,
    path::{Path, PathBuf},
};

use super::{
    camera::{
        SceneCameraAnimationConfig, SceneCameraConfig, SceneCameraEasing, SceneCameraFollowConfig,
        SceneCameraFollowTargetSource, SceneCameraMode, SceneCameraProjection,
        default_scene_camera_3d_transform,
    },
    id::{
        SCENE_ID_ALLOWED_CHARACTERS, SceneAssetId, SceneId, SceneIdError, SceneLayerId,
        SceneSpawnPointId,
    },
    loading::SceneLoadingPolicy,
    registry::SceneKind,
    spawn::{SceneAnchorManifest, SceneSpawnPointManifest},
    streaming::SceneChunkManifest,
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
    #[serde(default)]
    pub chunks: Vec<SceneChunkManifest>,
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
            chunks: Vec::new(),
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

    pub fn with_chunk(mut self, chunk: SceneChunkManifest) -> Self {
        self.chunks.push(chunk);
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

        let mut chunk_ids = HashSet::new();
        for (chunk_index, chunk) in self.chunks.iter().enumerate() {
            if chunk.zone_id.is_empty() {
                return Err(SceneManifestError::EmptyChunkZoneId { index: chunk_index });
            }

            if chunk.region_id.is_empty() {
                return Err(SceneManifestError::EmptyChunkRegionId { index: chunk_index });
            }

            if chunk.chunk_id.is_empty() {
                return Err(SceneManifestError::EmptyChunkId { index: chunk_index });
            }

            if !chunk_ids.insert(chunk.chunk_id.clone()) {
                return Err(SceneManifestError::DuplicateChunkId(chunk.chunk_id.clone()));
            }

            if !chunk.bounds.is_valid() {
                return Err(SceneManifestError::InvalidChunkBounds {
                    chunk_id: chunk.chunk_id.clone(),
                });
            }
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
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

#[derive(Clone, Debug, PartialEq)]
pub struct SceneCameraRef {
    id: String,
    config: SceneCameraConfig,
}

impl SceneCameraRef {
    pub fn new(value: impl Into<String>) -> Self {
        let id = value.into();
        let config = scene_camera_config_from_ref_id(&id);
        Self { id, config }
    }

    pub fn as_str(&self) -> &str {
        &self.id
    }

    pub fn into_string(self) -> String {
        self.id
    }

    pub fn is_empty(&self) -> bool {
        self.id.is_empty()
    }

    pub fn config(&self) -> &SceneCameraConfig {
        &self.config
    }

    pub fn into_config(self) -> SceneCameraConfig {
        self.config
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

impl<'de> Deserialize<'de> for SceneCameraRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum CameraRefDef {
            Id(String),
            Config(SceneCameraManifest),
        }

        match CameraRefDef::deserialize(deserializer)? {
            CameraRefDef::Id(id) => Ok(Self::new(id)),
            CameraRefDef::Config(camera) => Ok(camera.into_ref()),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default)]
pub struct SceneCameraManifest {
    pub id: Option<String>,
    pub mode: SceneCameraMode,
    pub position: Option<[f32; 3]>,
    #[serde(alias = "rotation")]
    pub rotation_degrees: Option<[f32; 3]>,
    pub projection: Option<SceneCameraProjectionManifest>,
    pub target: Option<super::id::SceneAnchorId>,
    pub follow: Option<SceneCameraFollowManifest>,
    pub animation: Option<SceneCameraAnimationManifest>,
}

impl Default for SceneCameraManifest {
    fn default() -> Self {
        Self {
            id: None,
            mode: SceneCameraMode::Gameplay2d,
            position: None,
            rotation_degrees: None,
            projection: None,
            target: None,
            follow: None,
            animation: None,
        }
    }
}

impl SceneCameraManifest {
    pub fn config(&self) -> SceneCameraConfig {
        let mut config = SceneCameraConfig::new(self.mode);

        if self.position.is_some() || self.rotation_degrees.is_some() {
            config.transform =
                scene_camera_manifest_transform(self.mode, self.position, self.rotation_degrees);
        }

        config.projection = self
            .projection
            .map(SceneCameraProjectionManifest::projection)
            .unwrap_or_else(|| SceneCameraProjection::for_mode(self.mode));
        config.target = self.target.clone();
        config.follow = self
            .follow
            .as_ref()
            .map(SceneCameraFollowManifest::follow_config)
            .or(config.follow);
        config.animation = self
            .animation
            .map(SceneCameraAnimationManifest::animation_config)
            .unwrap_or(config.animation);
        config
    }

    pub fn into_ref(self) -> SceneCameraRef {
        let id = self
            .id
            .clone()
            .unwrap_or_else(|| scene_camera_ref_id_for_mode(self.mode).to_string());
        SceneCameraRef {
            id,
            config: self.config(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default)]
pub struct SceneCameraFollowManifest {
    pub target_source: SceneCameraFollowTargetSourceManifest,
    pub target: Option<super::id::SceneAnchorId>,
    pub offset: Option<[f32; 3]>,
    pub look_at_offset: Option<[f32; 3]>,
    pub position_lerp: Option<f32>,
    pub rotation_lerp: Option<f32>,
    pub min_visible_targets: Option<usize>,
    pub visible_target_padding: Option<f32>,
}

impl Default for SceneCameraFollowManifest {
    fn default() -> Self {
        Self {
            target_source: SceneCameraFollowTargetSourceManifest::SceneTarget,
            target: None,
            offset: None,
            look_at_offset: None,
            position_lerp: None,
            rotation_lerp: None,
            min_visible_targets: None,
            visible_target_padding: None,
        }
    }
}

impl SceneCameraFollowManifest {
    fn follow_config(&self) -> SceneCameraFollowConfig {
        let default_config = SceneCameraFollowConfig::default();

        SceneCameraFollowConfig {
            target_source: self
                .target_source
                .target_source(self.target.clone())
                .unwrap_or(default_config.target_source),
            offset: self
                .offset
                .map(Vec3::from_array)
                .unwrap_or(default_config.offset),
            look_at_offset: self
                .look_at_offset
                .map(Vec3::from_array)
                .unwrap_or(default_config.look_at_offset),
            position_lerp: self.position_lerp.unwrap_or(default_config.position_lerp),
            rotation_lerp: self.rotation_lerp.unwrap_or(default_config.rotation_lerp),
            min_visible_targets: self
                .min_visible_targets
                .unwrap_or(default_config.min_visible_targets),
            visible_target_padding: self
                .visible_target_padding
                .unwrap_or(default_config.visible_target_padding),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SceneCameraFollowTargetSourceManifest {
    #[default]
    SceneTarget,
    Anchor,
    PrimaryActor,
    AllParticipants,
}

impl SceneCameraFollowTargetSourceManifest {
    fn target_source(
        self,
        target: Option<super::id::SceneAnchorId>,
    ) -> Option<SceneCameraFollowTargetSource> {
        match self {
            Self::SceneTarget => Some(SceneCameraFollowTargetSource::SceneTarget),
            Self::Anchor => target.map(SceneCameraFollowTargetSource::Anchor),
            Self::PrimaryActor => Some(SceneCameraFollowTargetSource::PrimaryActor),
            Self::AllParticipants => Some(SceneCameraFollowTargetSource::AllParticipants),
        }
    }
}

impl<'de> Deserialize<'de> for SceneCameraFollowTargetSourceManifest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match normalize_manifest_token(&value).as_str() {
            "scenetarget" | "scene" | "target" => Self::SceneTarget,
            "anchor" | "sceneanchor" => Self::Anchor,
            "primaryactor" | "player" | "localplayer" => Self::PrimaryActor,
            "allparticipants" | "participants" | "players" => Self::AllParticipants,
            _ => Self::SceneTarget,
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(default)]
pub struct SceneCameraAnimationManifest {
    pub enabled: Option<bool>,
    #[serde(alias = "duration")]
    pub duration_seconds: Option<f32>,
    pub easing: SceneCameraEasing,
}

impl Default for SceneCameraAnimationManifest {
    fn default() -> Self {
        Self {
            enabled: None,
            duration_seconds: None,
            easing: SceneCameraEasing::SmoothStep,
        }
    }
}

impl SceneCameraAnimationManifest {
    fn animation_config(self) -> SceneCameraAnimationConfig {
        let default_config = SceneCameraAnimationConfig::default();

        SceneCameraAnimationConfig {
            enabled: self.enabled.unwrap_or(default_config.enabled),
            duration_seconds: self
                .duration_seconds
                .unwrap_or(default_config.duration_seconds),
            easing: self.easing,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SceneCameraProjectionManifest {
    Default2d,
    Default3d,
    Orthographic2d {
        scale: f32,
    },
    Perspective3d {
        #[serde(alias = "fov_y")]
        fov_y_radians: f32,
        near: f32,
        far: f32,
    },
}

impl SceneCameraProjectionManifest {
    fn projection(self) -> SceneCameraProjection {
        match self {
            Self::Default2d => SceneCameraProjection::Default2d,
            Self::Default3d => SceneCameraProjection::Default3d,
            Self::Orthographic2d { scale } => SceneCameraProjection::Orthographic2d { scale },
            Self::Perspective3d {
                fov_y_radians,
                near,
                far,
            } => SceneCameraProjection::Perspective3d {
                fov_y_radians,
                near,
                far,
            },
        }
    }
}

impl<'de> Deserialize<'de> for SceneCameraMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match normalize_manifest_token(&value).as_str() {
            "uionly2d" | "ui2d" | "ui" => Self::UiOnly2d,
            "gameplay2d" | "world2d" | "2d" => Self::Gameplay2d,
            "gameplay3d" | "world3d" | "3d" => Self::Gameplay3d,
            "fixed3d" => Self::Fixed3d,
            "followtarget" | "follow" | "targetfollow" => Self::FollowTarget,
            "debugfree" | "free" | "freecamera" => Self::DebugFree,
            _ => Self::Gameplay2d,
        })
    }
}

impl<'de> Deserialize<'de> for SceneCameraEasing {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match normalize_manifest_token(&value).as_str() {
            "linear" => Self::Linear,
            "smoothstep" | "smooth" => Self::SmoothStep,
            "easeinout" | "easeinoutquad" => Self::EaseInOut,
            _ => Self::SmoothStep,
        })
    }
}

fn scene_camera_config_from_ref_id(id: &str) -> SceneCameraConfig {
    SceneCameraConfig::new(scene_camera_mode_from_ref_id(id))
}

fn scene_camera_mode_from_ref_id(id: &str) -> SceneCameraMode {
    match normalize_manifest_token(id).as_str() {
        "uionly2d" | "ui2d" | "ui" => SceneCameraMode::UiOnly2d,
        "gameplay3d" | "world3d" | "3d" => SceneCameraMode::Gameplay3d,
        "fixed3d" => SceneCameraMode::Fixed3d,
        "followtarget" | "follow" | "targetfollow" => SceneCameraMode::FollowTarget,
        "debugfree" | "free" | "freecamera" => SceneCameraMode::DebugFree,
        _ => SceneCameraMode::Gameplay2d,
    }
}

fn scene_camera_ref_id_for_mode(mode: SceneCameraMode) -> &'static str {
    match mode {
        SceneCameraMode::UiOnly2d => "ui_only_2d",
        SceneCameraMode::Gameplay2d => "gameplay_2d",
        SceneCameraMode::Gameplay3d => "gameplay_3d",
        SceneCameraMode::Fixed3d => "fixed_3d",
        SceneCameraMode::FollowTarget => "follow_target",
        SceneCameraMode::DebugFree => "debug_free",
    }
}

fn scene_camera_manifest_transform(
    mode: SceneCameraMode,
    position: Option<[f32; 3]>,
    rotation_degrees: Option<[f32; 3]>,
) -> Transform {
    let default_transform = if mode.is_3d() {
        default_scene_camera_3d_transform()
    } else {
        Transform::default()
    };

    let translation = position
        .map(Vec3::from_array)
        .unwrap_or(default_transform.translation);
    let rotation = rotation_degrees
        .map(|rotation_degrees| {
            Quat::from_euler(
                EulerRot::XYZ,
                rotation_degrees[0].to_radians(),
                rotation_degrees[1].to_radians(),
                rotation_degrees[2].to_radians(),
            )
        })
        .unwrap_or(default_transform.rotation);

    Transform {
        translation,
        rotation,
        scale: default_transform.scale,
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
    EmptyChunkZoneId {
        index: usize,
    },
    EmptyChunkRegionId {
        index: usize,
    },
    EmptyChunkId {
        index: usize,
    },
    DuplicateChunkId(super::id::SceneChunkId),
    InvalidChunkBounds {
        chunk_id: super::id::SceneChunkId,
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
            Self::EmptyChunkZoneId { index } => {
                write!(
                    formatter,
                    "scene chunk zone id must not be empty at index: {index}"
                )
            }
            Self::EmptyChunkRegionId { index } => {
                write!(
                    formatter,
                    "scene chunk region id must not be empty at index: {index}"
                )
            }
            Self::EmptyChunkId { index } => {
                write!(
                    formatter,
                    "scene chunk id must not be empty at index: {index}"
                )
            }
            Self::DuplicateChunkId(chunk_id) => {
                write!(formatter, "scene chunk id is duplicated: {chunk_id}")
            }
            Self::InvalidChunkBounds { chunk_id } => {
                write!(
                    formatter,
                    "scene chunk bounds are invalid for chunk: {chunk_id}"
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::scene::id::{SceneAnchorId, SceneChunkId, SceneSpawnPointId};
    use crate::framework::scene::spawn::SceneSpawnPointManifest;
    use crate::framework::scene::streaming::{SceneChunkBounds, SceneChunkManifest};

    #[test]
    fn validate_basic_rejects_duplicate_chunk_ids() {
        let manifest = SceneManifest::new("scene", SceneKind::World)
            .with_chunk(SceneChunkManifest::new(
                "zone",
                "region",
                "chunk",
                SceneChunkBounds::new(Vec3::ZERO, Vec3::ONE),
            ))
            .with_chunk(SceneChunkManifest::new(
                "zone",
                "region",
                "chunk",
                SceneChunkBounds::new(Vec3::ONE, Vec3::splat(2.0)),
            ));

        assert_eq!(
            manifest.validate_basic(),
            Err(SceneManifestError::DuplicateChunkId(SceneChunkId::from(
                "chunk"
            )))
        );
    }

    #[test]
    fn validate_basic_rejects_empty_scene_id() {
        let manifest = SceneManifest::new("", SceneKind::World);

        assert_eq!(
            manifest.validate_basic(),
            Err(SceneManifestError::EmptySceneId)
        );
    }

    #[test]
    fn validate_basic_rejects_missing_default_spawn() {
        let manifest = SceneManifest::new("scene", SceneKind::World)
            .with_entry(SceneManifestEntry::new().with_default_spawn("missing"));

        assert_eq!(
            manifest.validate_basic(),
            Err(SceneManifestError::DefaultSpawnMissing(
                SceneSpawnPointId::from("missing")
            ))
        );
    }

    #[test]
    fn validate_basic_accepts_defined_default_spawn() {
        let manifest = SceneManifest::new("scene", SceneKind::World)
            .with_entry(SceneManifestEntry::new().with_default_spawn("start"))
            .with_spawn_point(SceneSpawnPointManifest::new("start", [0.0, 0.0, 0.0]));

        assert_eq!(manifest.validate_basic(), Ok(()));
    }

    #[test]
    fn validate_basic_rejects_required_layer_without_assets() {
        let manifest = SceneManifest::new("scene", SceneKind::World)
            .with_layer(SceneLayerManifest::new("base"));

        assert_eq!(
            manifest.validate_basic(),
            Err(SceneManifestError::RequiredLayerWithoutAssets(
                SceneLayerId::from("base")
            ))
        );
    }

    #[test]
    fn validate_basic_rejects_empty_and_unsafe_asset_paths() {
        let empty_path_manifest = SceneManifest::new("scene", SceneKind::World).with_layer(
            SceneLayerManifest::new("base").with_asset(SceneAssetRef::new(
                "mesh",
                SceneAssetKind::GltfScene,
                "",
            )),
        );
        assert_eq!(
            empty_path_manifest.validate_basic(),
            Err(SceneManifestError::EmptyAssetPath {
                layer_id: SceneLayerId::from("base"),
                asset_id: SceneAssetId::from("mesh"),
            })
        );

        let unsafe_path_manifest = SceneManifest::new("scene", SceneKind::World).with_layer(
            SceneLayerManifest::new("base").with_asset(SceneAssetRef::new(
                "mesh",
                SceneAssetKind::GltfScene,
                "../outside.glb",
            )),
        );
        assert_eq!(
            unsafe_path_manifest.validate_basic(),
            Err(SceneManifestError::UnsafeAssetPath {
                layer_id: SceneLayerId::from("base"),
                asset_id: SceneAssetId::from("mesh"),
                path: "../outside.glb".to_string(),
            })
        );
    }

    #[test]
    fn camera_manifest_keeps_fixed_camera_compatibility() {
        let manifest = ron::from_str::<SceneManifest>(
            r#"(
                version: "1",
                scene_id: SceneId("scene.camera_fixed"),
                kind: "world",
                entry: (
                    camera: Some((
                        id: Some("camera.fixed"),
                        mode: "fixed3d",
                        position: Some((1.0, 2.0, 3.0)),
                        rotation: Some((-30.0, 0.0, 0.0)),
                        projection: Some((kind: "perspective3d", fov_y: 0.8, near: 0.1, far: 100.0)),
                        target: Some("anchor.camera_target"),
                    )),
                ),
            )"#,
        )
        .unwrap();

        let camera = manifest.entry.camera.unwrap();
        let config = camera.config();

        assert_eq!(camera.as_str(), "camera.fixed");
        assert_eq!(config.mode, SceneCameraMode::Fixed3d);
        assert!(config.is_3d());
        assert_eq!(config.transform.translation, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(
            config.target.as_ref().map(|target| target.as_str()),
            Some("anchor.camera_target")
        );
        assert_eq!(config.follow, None);
        assert_eq!(config.animation, SceneCameraAnimationConfig::default());
    }

    #[test]
    fn camera_manifest_parses_follow_camera_config() {
        let manifest = ron::from_str::<SceneManifest>(
            r#"(
                version: "1",
                scene_id: SceneId("scene.camera_follow"),
                kind: "world",
                entry: (
                    camera: Some((
                        mode: "follow_target",
                        follow: Some((
                            target_source: "anchor",
                            target: Some("anchor.player"),
                            offset: Some((0.0, 7.0, 14.0)),
                            look_at_offset: Some((0.0, 1.5, 0.0)),
                            position_lerp: Some(0.35),
                            rotation_lerp: Some(0.5),
                            min_visible_targets: Some(2),
                            visible_target_padding: Some(3.0),
                        )),
                    )),
                ),
            )"#,
        )
        .unwrap();

        let camera = manifest.entry.camera.unwrap();
        let config = camera.config();
        let follow = config.follow.as_ref().unwrap();

        assert_eq!(camera.as_str(), "follow_target");
        assert_eq!(config.mode, SceneCameraMode::FollowTarget);
        assert!(config.is_3d());
        assert_eq!(
            follow.target_source,
            SceneCameraFollowTargetSource::Anchor(SceneAnchorId::from("anchor.player"))
        );
        assert_eq!(follow.offset, Vec3::new(0.0, 7.0, 14.0));
        assert_eq!(follow.look_at_offset, Vec3::new(0.0, 1.5, 0.0));
        assert_eq!(follow.position_lerp, 0.35);
        assert_eq!(follow.rotation_lerp, 0.5);
        assert_eq!(follow.min_visible_targets, 2);
        assert_eq!(follow.visible_target_padding, 3.0);
    }

    #[test]
    fn camera_manifest_parses_animation_config() {
        let manifest = ron::from_str::<SceneManifest>(
            r#"(
                version: "1",
                scene_id: SceneId("scene.camera_animation"),
                kind: "world",
                entry: (
                    camera: Some((
                        mode: "gameplay3d",
                        animation: Some((
                            enabled: Some(true),
                            duration: Some(0.75),
                            easing: "ease_in_out",
                        )),
                    )),
                ),
            )"#,
        )
        .unwrap();

        let config = manifest.entry.camera.unwrap().into_config();

        assert_eq!(config.mode, SceneCameraMode::Gameplay3d);
        assert_eq!(
            config.animation,
            SceneCameraAnimationConfig {
                enabled: true,
                duration_seconds: 0.75,
                easing: SceneCameraEasing::EaseInOut,
            }
        );
    }

    #[test]
    fn camera_manifest_defaults_unknown_and_missing_fields() {
        let unknown_mode = SceneCameraRef::new("unknown_camera_mode");
        assert_eq!(unknown_mode.config().mode, SceneCameraMode::Gameplay2d);
        assert_eq!(
            SceneCameraRef::new("gameplay3d").config().mode,
            SceneCameraMode::Gameplay3d
        );
        assert_eq!(
            SceneCameraRef::new("fixed3d").config().mode,
            SceneCameraMode::Fixed3d
        );

        let manifest = ron::from_str::<SceneManifest>(
            r#"(
                version: "1",
                scene_id: SceneId("scene.camera_defaults"),
                kind: "world",
                entry: (
                    camera: Some((
                        mode: "does_not_exist",
                        follow: Some((target_source: "does_not_exist")),
                        animation: Some((easing: "does_not_exist")),
                    )),
                ),
            )"#,
        )
        .unwrap();

        let config = manifest.entry.camera.unwrap().into_config();
        let follow = config.follow.as_ref().unwrap();

        assert_eq!(config.mode, SceneCameraMode::Gameplay2d);
        assert_eq!(
            follow.target_source,
            SceneCameraFollowTargetSource::SceneTarget
        );
        assert_eq!(follow.offset, SceneCameraFollowConfig::default().offset);
        assert_eq!(config.animation.easing, SceneCameraEasing::SmoothStep);
        assert_eq!(config.animation.enabled, false);
    }
}
