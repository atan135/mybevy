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

    pub fn spawn_point(&self) -> SceneSpawnPoint {
        SceneSpawnPoint {
            id: self.id.clone(),
            transform: self.transform(),
            tags: self.tags.clone(),
        }
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
