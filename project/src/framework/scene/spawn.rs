use std::{collections::HashMap, fmt};

use bevy::{math::EulerRot, prelude::*};
use serde::Deserialize;

use super::{
    id::{SceneAnchorId, SceneId, SceneSessionId, SceneSpawnPointId},
    lifecycle::SceneSessionInfo,
};

#[derive(Clone, Debug, PartialEq)]
pub struct SceneSpawnPoint {
    pub id: SceneSpawnPointId,
    pub transform: Transform,
    pub tags: Vec<String>,
}

impl SceneSpawnPoint {
    pub fn new(id: impl Into<SceneSpawnPointId>, transform: Transform) -> Self {
        Self {
            id: id.into(),
            transform,
            tags: Vec::new(),
        }
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|candidate| candidate == tag)
    }

    pub fn to_transform(&self) -> Transform {
        self.transform
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneAnchor {
    pub id: SceneAnchorId,
    pub transform: Transform,
    pub tags: Vec<String>,
}

impl SceneAnchor {
    pub fn new(id: impl Into<SceneAnchorId>, transform: Transform) -> Self {
        Self {
            id: id.into(),
            transform,
            tags: Vec::new(),
        }
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|candidate| candidate == tag)
    }

    pub fn to_transform(&self) -> Transform {
        self.transform
    }
}

#[derive(Clone, Debug, Default, Resource)]
pub struct SceneSpawnRegistry {
    sessions: HashMap<SceneSessionId, SceneSpawnSessionIndex>,
}

impl SceneSpawnRegistry {
    pub fn session(&self, session_id: &SceneSessionId) -> Option<&SceneSpawnSessionIndex> {
        self.sessions.get(session_id)
    }

    pub fn contains_session(&self, session_id: &SceneSessionId) -> bool {
        self.sessions.contains_key(session_id)
    }

    pub fn sessions(&self) -> impl Iterator<Item = &SceneSpawnSessionIndex> {
        self.sessions.values()
    }

    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    pub fn spawn_point(
        &self,
        session_id: &SceneSessionId,
        spawn_point_id: &SceneSpawnPointId,
    ) -> Result<&SceneSpawnPoint, SceneSpawnLookupError> {
        self.lookup_session(session_id)?.spawn_point(spawn_point_id)
    }

    pub fn anchor(
        &self,
        session_id: &SceneSessionId,
        anchor_id: &SceneAnchorId,
    ) -> Result<&SceneAnchor, SceneSpawnLookupError> {
        self.lookup_session(session_id)?.anchor(anchor_id)
    }

    pub fn default_spawn(
        &self,
        session: &SceneSessionInfo,
    ) -> Result<&SceneSpawnPoint, SceneSpawnLookupError> {
        let index = self.lookup_session(&session.session_id)?;
        let spawn_point_id = session
            .spawn_point
            .as_ref()
            .or_else(|| index.default_spawn_id())
            .ok_or_else(|| SceneSpawnLookupError::DefaultSpawnMissing {
                session_id: session.session_id.clone(),
            })?;

        index.spawn_point(spawn_point_id)
    }

    pub fn default_spawn_for_session(
        &self,
        session_id: &SceneSessionId,
    ) -> Result<&SceneSpawnPoint, SceneSpawnLookupError> {
        self.lookup_session(session_id)?.default_spawn()
    }

    pub fn spawn_points_with_tag<'a>(
        &'a self,
        session_id: &SceneSessionId,
        tag: &str,
    ) -> Result<Vec<&'a SceneSpawnPoint>, SceneSpawnLookupError> {
        Ok(self.lookup_session(session_id)?.spawn_points_with_tag(tag))
    }

    pub fn anchors_with_tag<'a>(
        &'a self,
        session_id: &SceneSessionId,
        tag: &str,
    ) -> Result<Vec<&'a SceneAnchor>, SceneSpawnLookupError> {
        Ok(self.lookup_session(session_id)?.anchors_with_tag(tag))
    }

    pub fn debug_items(&self) -> impl Iterator<Item = SceneSpawnDebugItem<'_>> {
        self.sessions
            .values()
            .flat_map(|session| session.debug_items())
    }

    pub(crate) fn set_session_index(&mut self, index: SceneSpawnSessionIndex) {
        self.sessions.insert(index.session_id.clone(), index);
    }

    pub(crate) fn clear_session(&mut self, session_id: &SceneSessionId) {
        self.sessions.remove(session_id);
    }

    fn lookup_session(
        &self,
        session_id: &SceneSessionId,
    ) -> Result<&SceneSpawnSessionIndex, SceneSpawnLookupError> {
        self.session(session_id)
            .ok_or_else(|| SceneSpawnLookupError::SessionMissing {
                session_id: session_id.clone(),
            })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneSpawnSessionIndex {
    scene_id: SceneId,
    session_id: SceneSessionId,
    default_spawn: Option<SceneSpawnPointId>,
    spawn_points: HashMap<SceneSpawnPointId, SceneSpawnPoint>,
    anchors: HashMap<SceneAnchorId, SceneAnchor>,
}

impl SceneSpawnSessionIndex {
    pub fn empty(scene_id: impl Into<SceneId>, session_id: impl Into<SceneSessionId>) -> Self {
        Self {
            scene_id: scene_id.into(),
            session_id: session_id.into(),
            default_spawn: None,
            spawn_points: HashMap::new(),
            anchors: HashMap::new(),
        }
    }

    pub fn scene_id(&self) -> &SceneId {
        &self.scene_id
    }

    pub fn session_id(&self) -> &SceneSessionId {
        &self.session_id
    }

    pub fn default_spawn_id(&self) -> Option<&SceneSpawnPointId> {
        self.default_spawn.as_ref()
    }

    pub fn spawn_point_count(&self) -> usize {
        self.spawn_points.len()
    }

    pub fn anchor_count(&self) -> usize {
        self.anchors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.spawn_points.is_empty() && self.anchors.is_empty()
    }

    pub fn contains_spawn_point(&self, spawn_point_id: &SceneSpawnPointId) -> bool {
        self.spawn_points.contains_key(spawn_point_id)
    }

    pub fn contains_anchor(&self, anchor_id: &SceneAnchorId) -> bool {
        self.anchors.contains_key(anchor_id)
    }

    pub fn spawn_point(
        &self,
        spawn_point_id: &SceneSpawnPointId,
    ) -> Result<&SceneSpawnPoint, SceneSpawnLookupError> {
        self.spawn_points.get(spawn_point_id).ok_or_else(|| {
            SceneSpawnLookupError::SpawnPointMissing {
                session_id: self.session_id.clone(),
                spawn_point_id: spawn_point_id.clone(),
            }
        })
    }

    pub fn anchor(&self, anchor_id: &SceneAnchorId) -> Result<&SceneAnchor, SceneSpawnLookupError> {
        self.anchors
            .get(anchor_id)
            .ok_or_else(|| SceneSpawnLookupError::AnchorMissing {
                session_id: self.session_id.clone(),
                anchor_id: anchor_id.clone(),
            })
    }

    pub fn default_spawn(&self) -> Result<&SceneSpawnPoint, SceneSpawnLookupError> {
        let spawn_point_id = self.default_spawn.as_ref().ok_or_else(|| {
            SceneSpawnLookupError::DefaultSpawnMissing {
                session_id: self.session_id.clone(),
            }
        })?;

        self.spawn_point(spawn_point_id)
    }

    pub fn spawn_points(&self) -> impl Iterator<Item = &SceneSpawnPoint> {
        self.spawn_points.values()
    }

    pub fn anchors(&self) -> impl Iterator<Item = &SceneAnchor> {
        self.anchors.values()
    }

    pub fn spawn_points_with_tag(&self, tag: &str) -> Vec<&SceneSpawnPoint> {
        self.spawn_points
            .values()
            .filter(|spawn_point| spawn_point.has_tag(tag))
            .collect()
    }

    pub fn anchors_with_tag(&self, tag: &str) -> Vec<&SceneAnchor> {
        self.anchors
            .values()
            .filter(|anchor| anchor.has_tag(tag))
            .collect()
    }

    pub fn debug_items(&self) -> impl Iterator<Item = SceneSpawnDebugItem<'_>> {
        let spawn_items = self
            .spawn_points
            .values()
            .map(|spawn_point| SceneSpawnDebugItem {
                scene_id: &self.scene_id,
                session_id: &self.session_id,
                kind: SceneSpawnDebugKind::SpawnPoint,
                id: spawn_point.id.as_str(),
                transform: &spawn_point.transform,
                tags: &spawn_point.tags,
                is_default_spawn: self.default_spawn.as_ref() == Some(&spawn_point.id),
            });

        let anchor_items = self.anchors.values().map(|anchor| SceneSpawnDebugItem {
            scene_id: &self.scene_id,
            session_id: &self.session_id,
            kind: SceneSpawnDebugKind::Anchor,
            id: anchor.id.as_str(),
            transform: &anchor.transform,
            tags: &anchor.tags,
            is_default_spawn: false,
        });

        spawn_items.chain(anchor_items)
    }

    pub(crate) fn from_manifest_parts(
        scene_id: SceneId,
        session_id: SceneSessionId,
        default_spawn: Option<SceneSpawnPointId>,
        spawn_points: &[SceneSpawnPointManifest],
        anchors: &[SceneAnchorManifest],
    ) -> Self {
        let mut index = Self::empty(scene_id, session_id);
        index.default_spawn = default_spawn;

        for spawn_point in spawn_points {
            index.insert_spawn_point(spawn_point.spawn_point());
        }

        for anchor in anchors {
            index.insert_anchor(anchor.anchor());
        }

        index
    }

    pub(crate) fn validate_default_spawn(&self) -> Result<(), SceneSpawnLookupError> {
        if let Some(default_spawn) = &self.default_spawn
            && !self.spawn_points.contains_key(default_spawn)
        {
            return Err(SceneSpawnLookupError::SpawnPointMissing {
                session_id: self.session_id.clone(),
                spawn_point_id: default_spawn.clone(),
            });
        }

        Ok(())
    }

    fn insert_spawn_point(&mut self, spawn_point: SceneSpawnPoint) {
        self.spawn_points
            .insert(spawn_point.id.clone(), spawn_point);
    }

    fn insert_anchor(&mut self, anchor: SceneAnchor) {
        self.anchors.insert(anchor.id.clone(), anchor);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SceneSpawnDebugKind {
    SpawnPoint,
    Anchor,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneSpawnDebugItem<'a> {
    pub scene_id: &'a SceneId,
    pub session_id: &'a SceneSessionId,
    pub kind: SceneSpawnDebugKind,
    pub id: &'a str,
    pub transform: &'a Transform,
    pub tags: &'a [String],
    pub is_default_spawn: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SceneSpawnLookupError {
    SessionMissing {
        session_id: SceneSessionId,
    },
    DefaultSpawnMissing {
        session_id: SceneSessionId,
    },
    SpawnPointMissing {
        session_id: SceneSessionId,
        spawn_point_id: SceneSpawnPointId,
    },
    AnchorMissing {
        session_id: SceneSessionId,
        anchor_id: SceneAnchorId,
    },
}

impl fmt::Display for SceneSpawnLookupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SessionMissing { session_id } => {
                write!(
                    formatter,
                    "scene spawn index is missing for session: {session_id}"
                )
            }
            Self::DefaultSpawnMissing { session_id } => {
                write!(
                    formatter,
                    "default scene spawn point is not defined for session: {session_id}"
                )
            }
            Self::SpawnPointMissing {
                session_id,
                spawn_point_id,
            } => {
                write!(
                    formatter,
                    "scene spawn point is missing for session {session_id}: {spawn_point_id}"
                )
            }
            Self::AnchorMissing {
                session_id,
                anchor_id,
            } => {
                write!(
                    formatter,
                    "scene anchor is missing for session {session_id}: {anchor_id}"
                )
            }
        }
    }
}

impl std::error::Error for SceneSpawnLookupError {}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default)]
pub struct SceneSpawnPointManifest {
    pub id: SceneSpawnPointId,
    pub position: [f32; 3],
    #[serde(alias = "rotation")]
    pub rotation_degrees: [f32; 3],
    pub tags: Vec<String>,
}

impl Default for SceneSpawnPointManifest {
    fn default() -> Self {
        Self::new(SceneSpawnPointId::from(""), [0.0, 0.0, 0.0])
    }
}

impl SceneSpawnPointManifest {
    pub fn new(id: impl Into<SceneSpawnPointId>, position: [f32; 3]) -> Self {
        Self {
            id: id.into(),
            position,
            rotation_degrees: [0.0, 0.0, 0.0],
            tags: Vec::new(),
        }
    }

    pub fn with_rotation_degrees(mut self, rotation_degrees: [f32; 3]) -> Self {
        self.rotation_degrees = rotation_degrees;
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }

    pub fn transform(&self) -> Transform {
        transform_from_position_rotation(self.position, self.rotation_degrees)
    }

    pub fn to_transform(&self) -> Transform {
        self.transform()
    }

    pub fn spawn_point(&self) -> SceneSpawnPoint {
        SceneSpawnPoint {
            id: self.id.clone(),
            transform: self.transform(),
            tags: self.tags.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default)]
pub struct SceneAnchorManifest {
    pub id: SceneAnchorId,
    pub position: [f32; 3],
    #[serde(alias = "rotation")]
    pub rotation_degrees: [f32; 3],
    pub tags: Vec<String>,
}

impl Default for SceneAnchorManifest {
    fn default() -> Self {
        Self::new(SceneAnchorId::from(""), [0.0, 0.0, 0.0])
    }
}

impl SceneAnchorManifest {
    pub fn new(id: impl Into<SceneAnchorId>, position: [f32; 3]) -> Self {
        Self {
            id: id.into(),
            position,
            rotation_degrees: [0.0, 0.0, 0.0],
            tags: Vec::new(),
        }
    }

    pub fn with_rotation_degrees(mut self, rotation_degrees: [f32; 3]) -> Self {
        self.rotation_degrees = rotation_degrees;
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }

    pub fn transform(&self) -> Transform {
        transform_from_position_rotation(self.position, self.rotation_degrees)
    }

    pub fn to_transform(&self) -> Transform {
        self.transform()
    }

    pub fn anchor(&self) -> SceneAnchor {
        SceneAnchor {
            id: self.id.clone(),
            transform: self.transform(),
            tags: self.tags.clone(),
        }
    }
}

pub fn transform_from_position_rotation(
    position: [f32; 3],
    rotation_degrees: [f32; 3],
) -> Transform {
    Transform {
        translation: Vec3::from_array(position),
        rotation: Quat::from_euler(
            EulerRot::XYZ,
            rotation_degrees[0].to_radians(),
            rotation_degrees[1].to_radians(),
            rotation_degrees[2].to_radians(),
        ),
        ..Default::default()
    }
}

pub fn scene_spawn_point_transform(spawn_point: &SceneSpawnPoint) -> Transform {
    spawn_point.to_transform()
}

pub fn scene_anchor_transform(anchor: &SceneAnchor) -> Transform {
    anchor.to_transform()
}
