use std::{collections::HashMap, fmt};

use bevy::prelude::*;

use super::{
    id::{SCENE_ID_ALLOWED_CHARACTERS, SceneId, SceneIdError, SceneSpawnPointId},
    loading::SceneLoadingPolicy,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SceneKind {
    Boot,
    Ui,
    Lobby,
    #[default]
    Gameplay,
    Dungeon,
    World,
    Arena,
    Dev,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneDefinition {
    pub scene_id: SceneId,
    pub kind: SceneKind,
    pub has_world_root: bool,
    pub default_spawn: Option<SceneSpawnPointId>,
    pub manifest_path: Option<String>,
    pub loading_policy: SceneLoadingPolicy,
    pub content_version: Option<String>,
    pub content_source: SceneContentSource,
}

impl SceneDefinition {
    pub fn new(scene_id: impl Into<SceneId>, kind: SceneKind) -> Self {
        Self {
            scene_id: scene_id.into(),
            kind,
            has_world_root: false,
            default_spawn: None,
            manifest_path: None,
            loading_policy: SceneLoadingPolicy::default(),
            content_version: None,
            content_source: SceneContentSource::RegisteredMetadata,
        }
    }

    pub fn pure_ui(scene_id: impl Into<SceneId>) -> Self {
        Self::new(scene_id, SceneKind::Ui)
    }

    pub fn first_package_manifest(
        scene_id: impl Into<SceneId>,
        kind: SceneKind,
        manifest_path: impl Into<String>,
    ) -> Self {
        let manifest_path = manifest_path.into();
        Self {
            has_world_root: true,
            manifest_path: Some(manifest_path.clone()),
            content_source: SceneContentSource::FirstPackage { manifest_path },
            ..Self::new(scene_id, kind)
        }
    }

    pub fn with_world_root(mut self) -> Self {
        self.has_world_root = true;
        self
    }

    pub fn without_world_root(mut self) -> Self {
        self.has_world_root = false;
        self
    }

    pub fn with_default_spawn(mut self, default_spawn: impl Into<SceneSpawnPointId>) -> Self {
        self.default_spawn = Some(default_spawn.into());
        self
    }

    pub fn with_manifest_path(mut self, manifest_path: impl Into<String>) -> Self {
        let manifest_path = manifest_path.into();
        self.manifest_path = Some(manifest_path.clone());
        self.content_source = SceneContentSource::FirstPackage { manifest_path };
        self
    }

    pub fn with_loading_policy(mut self, loading_policy: SceneLoadingPolicy) -> Self {
        self.loading_policy = loading_policy;
        self
    }

    pub fn with_content_version(mut self, content_version: impl Into<String>) -> Self {
        self.content_version = Some(content_version.into());
        self
    }

    pub fn with_content_source(mut self, content_source: SceneContentSource) -> Self {
        self.manifest_path = content_source.manifest_path().map(str::to_string);
        if let Some(content_version) = content_source.content_version() {
            self.content_version = Some(content_version.to_string());
        }
        self.content_source = content_source;
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SceneContentSource {
    #[default]
    RegisteredMetadata,
    FirstPackage {
        manifest_path: String,
    },
    ContentCache {
        manifest_path: String,
        content_version: Option<String>,
    },
}

impl SceneContentSource {
    pub fn manifest_path(&self) -> Option<&str> {
        match self {
            Self::RegisteredMetadata => None,
            Self::FirstPackage { manifest_path } | Self::ContentCache { manifest_path, .. } => {
                Some(manifest_path)
            }
        }
    }

    pub fn content_version(&self) -> Option<&str> {
        match self {
            Self::RegisteredMetadata | Self::FirstPackage { .. } => None,
            Self::ContentCache {
                content_version, ..
            } => content_version.as_deref(),
        }
    }
}

#[derive(Clone, Debug, Default, Resource)]
pub struct SceneRegistry {
    definitions: HashMap<SceneId, SceneDefinition>,
    fallback_scene: Option<SceneId>,
}

impl SceneRegistry {
    pub fn register(&mut self, definition: SceneDefinition) -> Result<(), SceneRegistrationError> {
        validate_definition(&definition)?;

        if self.definitions.contains_key(&definition.scene_id) {
            return Err(SceneRegistrationError::DuplicateSceneId(
                definition.scene_id.clone(),
            ));
        }

        self.definitions
            .insert(definition.scene_id.clone(), definition);
        Ok(())
    }

    pub fn register_pure_ui(
        &mut self,
        scene_id: impl Into<SceneId>,
    ) -> Result<(), SceneRegistrationError> {
        self.register(SceneDefinition::pure_ui(scene_id))
    }

    pub fn register_manifest_scene(
        &mut self,
        scene_id: impl Into<SceneId>,
        kind: SceneKind,
        manifest_path: impl Into<String>,
    ) -> Result<(), SceneRegistrationError> {
        self.register(SceneDefinition::first_package_manifest(
            scene_id,
            kind,
            manifest_path,
        ))
    }

    pub fn register_fallback_scene(
        &mut self,
        definition: SceneDefinition,
    ) -> Result<(), SceneRegistrationError> {
        let scene_id = definition.scene_id.clone();
        self.register(definition)?;
        self.fallback_scene = Some(scene_id);
        Ok(())
    }

    pub fn get(&self, scene_id: &SceneId) -> Option<&SceneDefinition> {
        self.definitions.get(scene_id)
    }

    pub fn contains(&self, scene_id: &SceneId) -> bool {
        self.definitions.contains_key(scene_id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &SceneDefinition> {
        self.definitions.values()
    }

    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }

    pub fn set_fallback_scene(
        &mut self,
        scene_id: impl Into<SceneId>,
    ) -> Result<(), SceneRegistrationError> {
        let scene_id = scene_id.into();
        validate_scene_id(&scene_id)?;

        if !self.contains(&scene_id) {
            return Err(SceneRegistrationError::FallbackSceneNotRegistered(scene_id));
        }

        self.fallback_scene = Some(scene_id);
        Ok(())
    }

    pub fn fallback_scene(&self) -> Option<&SceneId> {
        self.fallback_scene.as_ref()
    }

    pub fn fallback_definition(&self) -> Option<&SceneDefinition> {
        self.fallback_scene()
            .and_then(|scene_id| self.get(scene_id))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SceneRegistrationError {
    EmptySceneId,
    InvalidSceneIdFormat(SceneId),
    DuplicateSceneId(SceneId),
    EmptyManifestPath(SceneId),
    FallbackSceneNotRegistered(SceneId),
}

impl fmt::Display for SceneRegistrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySceneId => formatter.write_str("scene id must not be empty"),
            Self::InvalidSceneIdFormat(scene_id) => {
                write!(
                    formatter,
                    "scene id has invalid format: {scene_id}; allowed characters are {SCENE_ID_ALLOWED_CHARACTERS}"
                )
            }
            Self::DuplicateSceneId(scene_id) => {
                write!(formatter, "scene id is already registered: {scene_id}")
            }
            Self::EmptyManifestPath(scene_id) => {
                write!(
                    formatter,
                    "manifest path must not be empty for scene: {scene_id}"
                )
            }
            Self::FallbackSceneNotRegistered(scene_id) => {
                write!(
                    formatter,
                    "fallback scene must be registered first: {scene_id}"
                )
            }
        }
    }
}

impl std::error::Error for SceneRegistrationError {}

impl From<SceneIdError> for SceneRegistrationError {
    fn from(error: SceneIdError) -> Self {
        match error {
            SceneIdError::Empty => Self::EmptySceneId,
            SceneIdError::InvalidFormat(value) => Self::InvalidSceneIdFormat(SceneId::from(value)),
        }
    }
}

fn validate_definition(definition: &SceneDefinition) -> Result<(), SceneRegistrationError> {
    validate_scene_id(&definition.scene_id)?;

    if definition
        .manifest_path
        .as_deref()
        .is_some_and(str::is_empty)
        || definition
            .content_source
            .manifest_path()
            .is_some_and(str::is_empty)
    {
        return Err(SceneRegistrationError::EmptyManifestPath(
            definition.scene_id.clone(),
        ));
    }

    Ok(())
}

fn validate_scene_id(scene_id: &SceneId) -> Result<(), SceneRegistrationError> {
    scene_id.validate().map_err(SceneRegistrationError::from)
}
