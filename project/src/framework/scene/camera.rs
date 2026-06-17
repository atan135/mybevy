use bevy::prelude::*;

use super::id::{SceneAnchorId, SceneSessionId};
use super::root::SceneOwned;

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

    pub fn is_session(&self, session_id: &SceneSessionId) -> bool {
        &self.session_id == session_id
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
        Self::gameplay_2d()
    }
}

impl SceneCameraConfig {
    pub fn new(mode: SceneCameraMode) -> Self {
        let mut config = match mode {
            SceneCameraMode::UiOnly2d => Self::ui_only_2d(),
            SceneCameraMode::Gameplay2d => Self::gameplay_2d(),
            SceneCameraMode::Gameplay3d => Self::gameplay_3d(),
            SceneCameraMode::Fixed3d => Self::fixed_3d(),
            SceneCameraMode::DebugFree => Self::debug_free(),
        };
        config.mode = mode;
        config
    }

    pub fn ui_only_2d() -> Self {
        Self {
            mode: SceneCameraMode::UiOnly2d,
            transform: Transform::default(),
            projection: SceneCameraProjection::Default2d,
            target: None,
        }
    }

    pub fn gameplay_2d() -> Self {
        Self {
            mode: SceneCameraMode::Gameplay2d,
            transform: Transform::default(),
            projection: SceneCameraProjection::Default2d,
            target: None,
        }
    }

    pub fn gameplay_3d() -> Self {
        Self {
            mode: SceneCameraMode::Gameplay3d,
            transform: default_scene_camera_3d_transform(),
            projection: SceneCameraProjection::Default3d,
            target: None,
        }
    }

    pub fn fixed_3d() -> Self {
        Self {
            mode: SceneCameraMode::Fixed3d,
            transform: default_scene_camera_3d_transform(),
            projection: SceneCameraProjection::Default3d,
            target: None,
        }
    }

    pub fn debug_free() -> Self {
        Self {
            mode: SceneCameraMode::DebugFree,
            transform: default_scene_camera_3d_transform(),
            projection: SceneCameraProjection::Default3d,
            target: None,
        }
    }

    pub fn with_transform(mut self, transform: Transform) -> Self {
        self.transform = transform;
        self
    }

    pub fn with_projection(mut self, projection: SceneCameraProjection) -> Self {
        self.projection = projection;
        self
    }

    pub fn with_target(mut self, target: impl Into<SceneAnchorId>) -> Self {
        self.target = Some(target.into());
        self
    }

    pub fn without_target(mut self) -> Self {
        self.target = None;
        self
    }

    pub fn requires_world_camera(&self) -> bool {
        self.mode.requires_world_camera()
    }

    pub fn is_3d(&self) -> bool {
        self.mode.is_3d()
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

impl SceneCameraMode {
    pub fn requires_world_camera(self) -> bool {
        !matches!(self, Self::UiOnly2d)
    }

    pub fn is_2d(self) -> bool {
        matches!(self, Self::UiOnly2d | Self::Gameplay2d)
    }

    pub fn is_3d(self) -> bool {
        matches!(self, Self::Gameplay3d | Self::Fixed3d | Self::DebugFree)
    }
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

impl SceneCameraProjection {
    pub fn for_mode(mode: SceneCameraMode) -> Self {
        if mode.is_3d() {
            Self::Default3d
        } else {
            Self::Default2d
        }
    }

    pub fn to_bevy_projection(self, mode: SceneCameraMode) -> Projection {
        match self {
            Self::Default2d => Projection::Orthographic(OrthographicProjection::default_2d()),
            Self::Default3d => Projection::Perspective(PerspectiveProjection::default()),
            Self::Orthographic2d { scale } => Projection::Orthographic(OrthographicProjection {
                scale,
                ..OrthographicProjection::default_2d()
            }),
            Self::Perspective3d {
                fov_y_radians,
                near,
                far,
            } => Projection::Perspective(PerspectiveProjection {
                fov: fov_y_radians,
                near,
                far,
                near_clip_plane: Vec4::new(0.0, 0.0, -1.0, -near),
                ..PerspectiveProjection::default()
            }),
        }
        .coerce_for_mode(mode)
    }
}

trait SceneProjectionModeCoerce {
    fn coerce_for_mode(self, mode: SceneCameraMode) -> Self;
}

impl SceneProjectionModeCoerce for Projection {
    fn coerce_for_mode(self, mode: SceneCameraMode) -> Self {
        match (mode.is_3d(), self) {
            (true, Projection::Orthographic(_)) => {
                Projection::Orthographic(OrthographicProjection::default_3d())
            }
            (false, Projection::Perspective(_) | Projection::Custom(_)) => {
                Projection::Orthographic(OrthographicProjection::default_2d())
            }
            (_, projection) => projection,
        }
    }
}

pub fn default_scene_camera_config_for_world(has_world_root: bool) -> Option<SceneCameraConfig> {
    has_world_root.then(SceneCameraConfig::gameplay_2d)
}

pub fn default_scene_camera_2d_config() -> SceneCameraConfig {
    SceneCameraConfig::gameplay_2d()
}

pub fn default_scene_camera_3d_config() -> SceneCameraConfig {
    SceneCameraConfig::gameplay_3d()
}

pub fn default_scene_camera_3d_transform() -> Transform {
    Transform::from_xyz(0.0, 6.0, 12.0).looking_at(Vec3::ZERO, Vec3::Y)
}

pub fn scene_has_camera_for_session(
    session_id: &SceneSessionId,
    scene_cameras: &Query<&SceneCameraRig>,
) -> bool {
    scene_cameras.iter().any(|rig| rig.is_session(session_id))
}

pub fn ensure_scene_camera(
    commands: &mut Commands,
    session_id: &SceneSessionId,
    config: &SceneCameraConfig,
    scene_cameras: &Query<&SceneCameraRig>,
) -> Option<Entity> {
    if !config.requires_world_camera() || scene_has_camera_for_session(session_id, scene_cameras) {
        return None;
    }

    Some(spawn_scene_camera(commands, session_id, config.clone()))
}

pub fn spawn_default_scene_camera_2d(
    commands: &mut Commands,
    session_id: &SceneSessionId,
) -> Entity {
    spawn_scene_camera(commands, session_id, default_scene_camera_2d_config())
}

pub fn spawn_default_scene_camera_3d(
    commands: &mut Commands,
    session_id: &SceneSessionId,
) -> Entity {
    spawn_scene_camera(commands, session_id, default_scene_camera_3d_config())
}

pub fn spawn_scene_camera(
    commands: &mut Commands,
    session_id: &SceneSessionId,
    config: SceneCameraConfig,
) -> Entity {
    if config.is_3d() {
        spawn_scene_camera_3d(commands, session_id, config)
    } else {
        spawn_scene_camera_2d(commands, session_id, config)
    }
}

fn spawn_scene_camera_2d(
    commands: &mut Commands,
    session_id: &SceneSessionId,
    config: SceneCameraConfig,
) -> Entity {
    let transform = config.transform;
    let projection = config.projection.to_bevy_projection(config.mode);

    commands
        .spawn((
            Camera2d,
            transform,
            projection,
            SceneCameraRig::new(session_id.clone(), config),
            SceneOwned::new(session_id.clone()),
            Name::new("SceneCamera2d"),
        ))
        .id()
}

fn spawn_scene_camera_3d(
    commands: &mut Commands,
    session_id: &SceneSessionId,
    config: SceneCameraConfig,
) -> Entity {
    let transform = config.transform;
    let projection = config.projection.to_bevy_projection(config.mode);

    commands
        .spawn((
            Camera3d::default(),
            transform,
            projection,
            SceneCameraRig::new(session_id.clone(), config),
            SceneOwned::new(session_id.clone()),
            Name::new("SceneCamera3d"),
        ))
        .id()
}
