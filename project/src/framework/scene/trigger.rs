use bevy::prelude::*;

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

#[derive(Clone, Debug, PartialEq)]
pub struct SceneTriggerManifest {
    pub id: SceneTriggerId,
    pub shape: SceneTriggerShapeManifest,
    pub position: [f32; 3],
    pub rotation_degrees: [f32; 3],
    pub event: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SceneTriggerShapeManifest {
    Circle2d { radius: f32 },
    Box2d { half_extents: [f32; 2] },
    Box3d { half_extents: [f32; 3] },
}
