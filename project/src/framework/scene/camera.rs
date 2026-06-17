use bevy::prelude::*;

use super::id::{SceneAnchorId, SceneSessionId};

#[derive(Clone, Debug, Component, PartialEq)]
pub struct SceneCameraRig {
    pub session_id: SceneSessionId,
    pub config: SceneCameraConfig,
}

impl SceneCameraRig {
    pub fn new(session_id: impl Into<SceneSessionId>, config: SceneCameraConfig) -> Self {
        Self {
            session_id: session_id.into(),
            config,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneCameraConfig {
    pub mode: SceneCameraMode,
    pub transform: Transform,
    pub projection: SceneCameraProjection,
    pub target: Option<SceneAnchorId>,
}

impl Default for SceneCameraConfig {
    fn default() -> Self {
        Self {
            mode: SceneCameraMode::Gameplay2d,
            transform: Transform::default(),
            projection: SceneCameraProjection::Default2d,
            target: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SceneCameraMode {
    UiOnly2d,
    #[default]
    Gameplay2d,
    Gameplay3d,
    Fixed3d,
    DebugFree,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SceneCameraProjection {
    Default2d,
    Default3d,
    Orthographic2d {
        scale: f32,
    },
    Perspective3d {
        fov_y_radians: f32,
        near: f32,
        far: f32,
    },
}
