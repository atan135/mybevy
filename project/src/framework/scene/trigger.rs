use bevy::prelude::*;
use serde::{Deserialize, Deserializer};

use super::id::{SceneSessionId, SceneTriggerId};

#[derive(Clone, Debug, Component, PartialEq)]
pub struct SceneTrigger {
    pub trigger_id: SceneTriggerId,
    pub shape: SceneTriggerShape,
    pub event: String,
    pub enabled: bool,
    pub session_id: SceneSessionId,
}

impl SceneTrigger {
    pub fn new(
        session_id: impl Into<SceneSessionId>,
        trigger_id: impl Into<SceneTriggerId>,
        shape: SceneTriggerShape,
        event: impl Into<String>,
    ) -> Self {
        Self {
            trigger_id: trigger_id.into(),
            shape,
            event: event.into(),
            enabled: true,
            session_id: session_id.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SceneTriggerShape {
    Circle2d { radius: f32 },
    Box2d { half_extents: Vec2 },
    Box3d { half_extents: Vec3 },
}

#[derive(Clone, Debug, Message, PartialEq)]
pub struct SceneTriggerEvent {
    pub trigger_id: SceneTriggerId,
    pub activator: Option<Entity>,
    pub action: SceneTriggerAction,
    pub session_id: SceneSessionId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SceneTriggerAction {
    Enter,
    Exit,
    Stay,
    Interact,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default)]
pub struct SceneTriggerManifest {
    pub id: SceneTriggerId,
    pub shape: SceneTriggerShapeManifest,
    pub position: [f32; 3],
    #[serde(alias = "rotation")]
    pub rotation_degrees: [f32; 3],
    pub event: String,
}

impl Default for SceneTriggerManifest {
    fn default() -> Self {
        Self::new(
            SceneTriggerId::from(""),
            SceneTriggerShapeManifest::default(),
            "",
        )
    }
}

impl SceneTriggerManifest {
    pub fn new(
        id: impl Into<SceneTriggerId>,
        shape: SceneTriggerShapeManifest,
        event: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            shape,
            position: [0.0, 0.0, 0.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            event: event.into(),
        }
    }

    pub fn with_position(mut self, position: [f32; 3]) -> Self {
        self.position = position;
        self
    }

    pub fn with_rotation_degrees(mut self, rotation_degrees: [f32; 3]) -> Self {
        self.rotation_degrees = rotation_degrees;
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SceneTriggerShapeManifest {
    Circle2d { radius: f32 },
    Box2d { half_extents: [f32; 2] },
    Box3d { half_extents: [f32; 3] },
}

impl Default for SceneTriggerShapeManifest {
    fn default() -> Self {
        Self::Box2d {
            half_extents: [0.0, 0.0],
        }
    }
}

impl<'de> Deserialize<'de> for SceneTriggerShapeManifest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        enum Shape {
            Circle2d { radius: f32 },
            Circle { radius: f32 },
            Box2d { half_extents: [f32; 2] },
            Box3d { half_extents: [f32; 3] },
            Box { half_extents: [f32; 3] },
        }

        Ok(match Shape::deserialize(deserializer)? {
            Shape::Circle2d { radius } | Shape::Circle { radius } => Self::Circle2d { radius },
            Shape::Box2d { half_extents } => Self::Box2d { half_extents },
            Shape::Box3d { half_extents } | Shape::Box { half_extents } => {
                Self::Box3d { half_extents }
            }
        })
    }
}

impl SceneTriggerShapeManifest {
    pub fn circle2d(radius: f32) -> Self {
        Self::Circle2d { radius }
    }

    pub fn box2d(half_extents: [f32; 2]) -> Self {
        Self::Box2d { half_extents }
    }

    pub fn box3d(half_extents: [f32; 3]) -> Self {
        Self::Box3d { half_extents }
    }

    pub fn shape(&self) -> SceneTriggerShape {
        match self {
            Self::Circle2d { radius } => SceneTriggerShape::Circle2d { radius: *radius },
            Self::Box2d { half_extents } => SceneTriggerShape::Box2d {
                half_extents: Vec2::from_array(*half_extents),
            },
            Self::Box3d { half_extents } => SceneTriggerShape::Box3d {
                half_extents: Vec3::from_array(*half_extents),
            },
        }
    }
}
