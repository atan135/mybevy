use std::{collections::HashMap, fmt};

use bevy::prelude::*;

use super::{
    id::{SceneId, SceneSpawnPointId},
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
        }
    }

    pub fn pure_ui(scene_id: impl Into<SceneId>) -> Self {
        Self {
            kind: SceneKind::Ui,
            ..Self::new(scene_id, SceneKind::Ui)
        }
    }

    pub fn with_world_root(mut self) -> Self {
        self.has_world_root = true;
        self
    }
}

#[derive(Clone, Debug, Default, Resource)]
pub struct SceneRegistry {
    definitions: HashMap<SceneId, SceneDefinition>,
    fallback_scene: Option<SceneId>,
}

impl SceneRegistry {
    pub fn register(&mut self, definition: SceneDefinition) -> Result<(), SceneRegistrationError> {
        if definition.scene_id.is_empty() {
            return Err(SceneRegistrationError::EmptySceneId);
        }

        if self.definitions.contains_key(&definition.scene_id) {
            return Err(SceneRegistrationError::DuplicateSceneId(
                definition.scene_id.clone(),
            ));
        }

        self.definitions
            .insert(definition.scene_id.clone(), definition);
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

    pub fn set_fallback_scene(&mut self, scene_id: impl Into<SceneId>) {
        self.fallback_scene = Some(scene_id.into());
    }

    pub fn fallback_scene(&self) -> Option<&SceneId> {
        self.fallback_scene.as_ref()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SceneRegistrationError {
    EmptySceneId,
    DuplicateSceneId(SceneId),
}

impl fmt::Display for SceneRegistrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySceneId => formatter.write_str("scene id must not be empty"),
            Self::DuplicateSceneId(scene_id) => {
                write!(formatter, "scene id is already registered: {scene_id}")
            }
        }
    }
}
