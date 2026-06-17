use bevy::{math::EulerRot, prelude::*};

use super::id::{SceneAnchorId, SceneSpawnPointId};

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
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneSpawnPointManifest {
    pub id: SceneSpawnPointId,
    pub position: [f32; 3],
    pub rotation_degrees: [f32; 3],
    pub tags: Vec<String>,
}

impl SceneSpawnPointManifest {
    pub fn transform(&self) -> Transform {
        transform_from_position_rotation(self.position, self.rotation_degrees)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneAnchorManifest {
    pub id: SceneAnchorId,
    pub position: [f32; 3],
    pub rotation_degrees: [f32; 3],
    pub tags: Vec<String>,
}

impl SceneAnchorManifest {
    pub fn transform(&self) -> Transform {
        transform_from_position_rotation(self.position, self.rotation_degrees)
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
