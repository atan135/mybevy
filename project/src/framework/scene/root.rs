use bevy::prelude::*;

use super::id::{SceneId, SceneLayerId, SceneSessionId};

pub const SCENE_DEFAULT_LAYER_ID: &str = "base";

#[derive(Clone, Debug, Component, PartialEq, Eq)]
pub struct SceneRoot {
    pub scene_id: SceneId,
    pub session_id: SceneSessionId,
}

impl SceneRoot {
    pub fn new(scene_id: impl Into<SceneId>, session_id: impl Into<SceneSessionId>) -> Self {
        Self {
            scene_id: scene_id.into(),
            session_id: session_id.into(),
        }
    }

    pub fn is_session(&self, session_id: &SceneSessionId) -> bool {
        &self.session_id == session_id
    }
}

#[derive(Clone, Debug, Component, PartialEq, Eq)]
pub struct SceneLayerRoot {
    pub session_id: SceneSessionId,
    pub layer_id: SceneLayerId,
    pub state: SceneLayerState,
    pub required: bool,
}

impl SceneLayerRoot {
    pub fn new(session_id: impl Into<SceneSessionId>, layer_id: impl Into<SceneLayerId>) -> Self {
        Self {
            session_id: session_id.into(),
            layer_id: layer_id.into(),
            state: SceneLayerState::default(),
            required: true,
        }
    }

    pub fn with_state(mut self, state: SceneLayerState) -> Self {
        self.state = state;
        self
    }

    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    pub fn optional(mut self) -> Self {
        self.required = false;
        self
    }

    pub fn is_session(&self, session_id: &SceneSessionId) -> bool {
        &self.session_id == session_id
    }
}

#[derive(Clone, Debug, Component, PartialEq, Eq)]
pub struct SceneOwned {
    pub session_id: SceneSessionId,
}

impl SceneOwned {
    pub fn new(session_id: impl Into<SceneSessionId>) -> Self {
        Self {
            session_id: session_id.into(),
        }
    }

    pub fn is_session(&self, session_id: &SceneSessionId) -> bool {
        &self.session_id == session_id
    }
}

#[derive(Clone, Debug, Component, PartialEq, Eq)]
pub struct SceneRuntimeRoot {
    pub session_id: SceneSessionId,
}

impl SceneRuntimeRoot {
    pub fn new(session_id: impl Into<SceneSessionId>) -> Self {
        Self {
            session_id: session_id.into(),
        }
    }

    pub fn is_session(&self, session_id: &SceneSessionId) -> bool {
        &self.session_id == session_id
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SceneLayerState {
    #[default]
    Registered,
    Loading,
    Loaded,
    Active,
    Unloading,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneWorldRoots {
    pub root: Entity,
    pub default_layer_root: Entity,
    pub runtime_root: Entity,
}

impl SceneWorldRoots {
    pub fn entities(self) -> [Entity; 3] {
        [self.root, self.default_layer_root, self.runtime_root]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SceneEntityCounts {
    pub total_scene_owned: usize,
    pub scene_roots: usize,
    pub layer_roots: usize,
    pub runtime_roots: usize,
}

impl SceneEntityCounts {
    pub fn is_empty(self) -> bool {
        self.total_scene_owned == 0
            && self.scene_roots == 0
            && self.layer_roots == 0
            && self.runtime_roots == 0
    }

    pub fn has_entities(self) -> bool {
        !self.is_empty()
    }

    pub fn has_residual_entities(self) -> bool {
        self.has_entities()
    }
}

pub fn scene_root_bundle(
    scene_id: impl Into<SceneId>,
    session_id: impl Into<SceneSessionId>,
) -> impl Bundle {
    let scene_id = scene_id.into();
    let session_id = session_id.into();
    let name = format!("SceneRoot({scene_id})");

    (
        SceneRoot::new(scene_id, session_id.clone()),
        SceneOwned::new(session_id),
        Name::new(name),
    )
}

pub fn scene_layer_root_bundle(
    session_id: impl Into<SceneSessionId>,
    layer_id: impl Into<SceneLayerId>,
    state: SceneLayerState,
    required: bool,
) -> impl Bundle {
    let session_id = session_id.into();
    let layer_id = layer_id.into();
    let name = format!("SceneLayerRoot({layer_id})");

    (
        SceneLayerRoot::new(session_id.clone(), layer_id)
            .with_state(state)
            .required(required),
        SceneOwned::new(session_id),
        Name::new(name),
    )
}

pub fn scene_runtime_root_bundle(session_id: impl Into<SceneSessionId>) -> impl Bundle {
    let session_id = session_id.into();

    (
        SceneRuntimeRoot::new(session_id.clone()),
        SceneOwned::new(session_id),
        Name::new("SceneRuntimeRoot"),
    )
}

pub fn spawn_scene_root(
    commands: &mut Commands,
    scene_id: &SceneId,
    session_id: &SceneSessionId,
) -> Entity {
    commands
        .spawn(scene_root_bundle(scene_id.clone(), session_id.clone()))
        .id()
}

pub fn spawn_scene_layer_root(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    layer_id: impl Into<SceneLayerId>,
    state: SceneLayerState,
    required: bool,
) -> Entity {
    let layer_root = commands
        .spawn(scene_layer_root_bundle(
            session_id.clone(),
            layer_id,
            state,
            required,
        ))
        .id();
    commands.entity(parent).add_child(layer_root);
    layer_root
}

pub fn spawn_scene_default_layer_root(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
) -> Entity {
    spawn_scene_layer_root(
        commands,
        parent,
        session_id,
        SCENE_DEFAULT_LAYER_ID,
        SceneLayerState::Active,
        true,
    )
}

pub fn spawn_scene_runtime_root(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
) -> Entity {
    let runtime_root = commands
        .spawn(scene_runtime_root_bundle(session_id.clone()))
        .id();
    commands.entity(parent).add_child(runtime_root);
    runtime_root
}

pub fn spawn_scene_world_roots(
    commands: &mut Commands,
    scene_id: &SceneId,
    session_id: &SceneSessionId,
) -> SceneWorldRoots {
    let root = spawn_scene_root(commands, scene_id, session_id);
    let default_layer_root = spawn_scene_default_layer_root(commands, root, session_id);
    let runtime_root = spawn_scene_runtime_root(commands, root, session_id);

    SceneWorldRoots {
        root,
        default_layer_root,
        runtime_root,
    }
}

pub(crate) fn despawn_scene_session_entities(
    commands: &mut Commands,
    session_id: &SceneSessionId,
    scene_roots: &Query<(Entity, &SceneRoot)>,
    owned_entities: &Query<(Entity, &SceneOwned)>,
) -> usize {
    let mut root_entities = Vec::new();
    let mut despawn_requests = 0;

    for (entity, root) in scene_roots.iter() {
        if root.is_session(session_id) {
            root_entities.push(entity);
            commands.entity(entity).try_despawn();
            despawn_requests += 1;
        }
    }

    for (entity, owned) in owned_entities.iter() {
        if owned.is_session(session_id) && !root_entities.contains(&entity) {
            commands.entity(entity).try_despawn();
            despawn_requests += 1;
        }
    }

    despawn_requests
}

pub fn count_scene_entities(
    owned_entities: &Query<&SceneOwned>,
    scene_roots: &Query<&SceneRoot>,
    layer_roots: &Query<&SceneLayerRoot>,
    runtime_roots: &Query<&SceneRuntimeRoot>,
) -> SceneEntityCounts {
    SceneEntityCounts {
        total_scene_owned: owned_entities.iter().count(),
        scene_roots: scene_roots.iter().count(),
        layer_roots: layer_roots.iter().count(),
        runtime_roots: runtime_roots.iter().count(),
    }
}

pub fn count_scene_entities_for_session(
    session_id: &SceneSessionId,
    owned_entities: &Query<&SceneOwned>,
    scene_roots: &Query<&SceneRoot>,
    layer_roots: &Query<&SceneLayerRoot>,
    runtime_roots: &Query<&SceneRuntimeRoot>,
) -> SceneEntityCounts {
    SceneEntityCounts {
        total_scene_owned: owned_entities
            .iter()
            .filter(|owned| owned.is_session(session_id))
            .count(),
        scene_roots: scene_roots
            .iter()
            .filter(|root| root.is_session(session_id))
            .count(),
        layer_roots: layer_roots
            .iter()
            .filter(|root| root.is_session(session_id))
            .count(),
        runtime_roots: runtime_roots
            .iter()
            .filter(|root| root.is_session(session_id))
            .count(),
    }
}
