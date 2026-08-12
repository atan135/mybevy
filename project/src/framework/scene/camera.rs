use bevy::prelude::*;

use super::id::{SceneAnchorId, SceneSessionId};
use super::root::SceneOwned;
use super::spawn::SceneSpawnRegistry;

pub const SCENE_CAMERA_3D_ORDER: isize = -1;
pub const SCENE_CAMERA_2D_ORDER: isize = 0;
pub const SCENE_CAMERA_LOCAL_PLAYER_TARGET_TAG: &str = "local_player";
pub const SCENE_CAMERA_PRIMARY_ACTOR_TARGET_TAG: &str = "primary_actor";

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

#[derive(Clone, Debug, Component, PartialEq, Eq)]
pub struct SceneCameraTarget {
    pub session_id: SceneSessionId,
    pub tag: Option<String>,
    pub priority: i32,
}

impl SceneCameraTarget {
    pub fn new(session_id: impl Into<SceneSessionId>) -> Self {
        Self {
            session_id: session_id.into(),
            tag: None,
            priority: 0,
        }
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    pub fn is_session(&self, session_id: &SceneSessionId) -> bool {
        &self.session_id == session_id
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tag.as_deref() == Some(tag)
    }
}

#[derive(Clone, Debug, Component, PartialEq)]
pub struct SceneCameraRuntimeState {
    goal: Transform,
    tween_from: Transform,
    tween_elapsed_seconds: f32,
}

impl SceneCameraRuntimeState {
    fn new(initial: Transform) -> Self {
        Self {
            goal: initial,
            tween_from: initial,
            tween_elapsed_seconds: 0.0,
        }
    }

    fn start_tween(&mut self, from: Transform, goal: Transform) {
        self.goal = goal;
        self.tween_from = from;
        self.tween_elapsed_seconds = 0.0;
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneCameraConfig {
    pub mode: SceneCameraMode,
    pub transform: Transform,
    pub projection: SceneCameraProjection,
    pub target: Option<SceneAnchorId>,
    pub follow: Option<SceneCameraFollowConfig>,
    pub animation: SceneCameraAnimationConfig,
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
            SceneCameraMode::FollowTarget => Self::follow_target(),
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
            follow: None,
            animation: SceneCameraAnimationConfig::default(),
        }
    }

    pub fn gameplay_2d() -> Self {
        Self {
            mode: SceneCameraMode::Gameplay2d,
            transform: Transform::default(),
            projection: SceneCameraProjection::Default2d,
            target: None,
            follow: None,
            animation: SceneCameraAnimationConfig::default(),
        }
    }

    pub fn gameplay_3d() -> Self {
        Self {
            mode: SceneCameraMode::Gameplay3d,
            transform: default_scene_camera_3d_transform(),
            projection: SceneCameraProjection::Default3d,
            target: None,
            follow: None,
            animation: SceneCameraAnimationConfig::default(),
        }
    }

    pub fn fixed_3d() -> Self {
        Self {
            mode: SceneCameraMode::Fixed3d,
            transform: default_scene_camera_3d_transform(),
            projection: SceneCameraProjection::Default3d,
            target: None,
            follow: None,
            animation: SceneCameraAnimationConfig::default(),
        }
    }

    pub fn follow_target() -> Self {
        Self {
            mode: SceneCameraMode::FollowTarget,
            transform: default_scene_camera_3d_transform(),
            projection: SceneCameraProjection::Default3d,
            target: None,
            follow: Some(SceneCameraFollowConfig::default()),
            animation: SceneCameraAnimationConfig::default(),
        }
    }

    pub fn debug_free() -> Self {
        Self {
            mode: SceneCameraMode::DebugFree,
            transform: default_scene_camera_3d_transform(),
            projection: SceneCameraProjection::Default3d,
            target: None,
            follow: None,
            animation: SceneCameraAnimationConfig::default(),
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

    pub fn with_follow(mut self, follow: SceneCameraFollowConfig) -> Self {
        self.follow = Some(follow);
        self
    }

    pub fn without_follow(mut self) -> Self {
        self.follow = None;
        self
    }

    pub fn with_animation(mut self, animation: SceneCameraAnimationConfig) -> Self {
        self.animation = animation;
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

    /// Returns configuration that is safe to apply to a Bevy camera.
    ///
    /// Manifests are validated before scene creation, but game-layer input can
    /// update a rig at runtime. This guard prevents invalid input from
    /// publishing a non-finite transform or projection.
    pub fn sanitized(&self) -> Self {
        let default_config = Self::new(self.mode);

        Self {
            mode: self.mode,
            transform: sanitize_scene_camera_transform(self.transform, default_config.transform),
            projection: self.projection.sanitized(),
            target: self.target.clone(),
            follow: self.follow.as_ref().map(SceneCameraFollowConfig::sanitized),
            animation: self.animation.sanitized(),
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
    FollowTarget,
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
        matches!(
            self,
            Self::Gameplay3d | Self::Fixed3d | Self::FollowTarget | Self::DebugFree
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneCameraFollowConfig {
    pub target_source: SceneCameraFollowTargetSource,
    pub offset: Vec3,
    pub look_at_offset: Vec3,
    pub position_lerp: f32,
    pub rotation_lerp: f32,
    pub min_visible_targets: usize,
    pub visible_target_padding: f32,
}

impl Default for SceneCameraFollowConfig {
    fn default() -> Self {
        Self {
            target_source: SceneCameraFollowTargetSource::SceneTarget,
            offset: Vec3::new(0.0, 6.0, 12.0),
            look_at_offset: Vec3::ZERO,
            position_lerp: 1.0,
            rotation_lerp: 1.0,
            min_visible_targets: 1,
            visible_target_padding: 0.0,
        }
    }
}

impl SceneCameraFollowConfig {
    fn sanitized(&self) -> Self {
        let default_config = Self::default();

        Self {
            target_source: self.target_source.clone(),
            offset: sanitize_vec3(self.offset, default_config.offset),
            look_at_offset: sanitize_vec3(self.look_at_offset, default_config.look_at_offset),
            position_lerp: sanitize_unit_interval(self.position_lerp, default_config.position_lerp),
            rotation_lerp: sanitize_unit_interval(self.rotation_lerp, default_config.rotation_lerp),
            min_visible_targets: self.min_visible_targets,
            visible_target_padding: sanitize_non_negative(
                self.visible_target_padding,
                default_config.visible_target_padding,
            ),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum SceneCameraFollowTargetSource {
    #[default]
    SceneTarget,
    Anchor(SceneAnchorId),
    PrimaryActor,
    AllParticipants,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneCameraAnimationConfig {
    pub enabled: bool,
    pub duration_seconds: f32,
    pub easing: SceneCameraEasing,
}

impl Default for SceneCameraAnimationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            duration_seconds: 0.0,
            easing: SceneCameraEasing::SmoothStep,
        }
    }
}

impl SceneCameraAnimationConfig {
    fn sanitized(self) -> Self {
        let default_config = Self::default();
        Self {
            enabled: self.enabled,
            duration_seconds: sanitize_non_negative(
                self.duration_seconds,
                default_config.duration_seconds,
            ),
            easing: self.easing,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SceneCameraEasing {
    Linear,
    #[default]
    SmoothStep,
    EaseInOut,
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
        match self.sanitized() {
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

    fn sanitized(self) -> Self {
        match self {
            Self::Default2d | Self::Default3d => self,
            Self::Orthographic2d { scale } => Self::Orthographic2d {
                scale: sanitize_positive(scale, OrthographicProjection::default_2d().scale),
            },
            Self::Perspective3d {
                fov_y_radians,
                near,
                far,
            } => {
                let default_projection = PerspectiveProjection::default();
                let fov_y_radians = if fov_y_radians.is_finite()
                    && fov_y_radians > 0.0
                    && fov_y_radians < std::f32::consts::PI
                {
                    fov_y_radians
                } else {
                    default_projection.fov
                };
                let near = sanitize_positive(near, default_projection.near);
                let far = if far.is_finite() && far > near {
                    far
                } else {
                    default_projection.far.max(near + 1.0)
                };

                Self::Perspective3d {
                    fov_y_radians,
                    near,
                    far,
                }
            }
        }
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
    let config = config.sanitized();
    if config.is_3d() {
        spawn_scene_camera_3d(commands, session_id, config)
    } else {
        spawn_scene_camera_2d(commands, session_id, config)
    }
}

pub fn update_scene_cameras(
    time: Res<Time>,
    spawn_registry: Res<SceneSpawnRegistry>,
    mut scene_cameras: Query<(
        &SceneCameraRig,
        &mut Transform,
        &mut Projection,
        &mut SceneCameraRuntimeState,
    )>,
    camera_targets: Query<(&SceneCameraTarget, &GlobalTransform)>,
) {
    let delta_seconds = if time.delta_secs() > 0.0 {
        time.delta_secs()
    } else {
        1.0 / 60.0
    };

    for (rig, mut transform, mut projection, mut runtime) in &mut scene_cameras {
        let config = rig.config.sanitized();
        *projection = config.projection.to_bevy_projection(config.mode);
        let current = sanitize_scene_camera_transform(*transform, config.transform);

        let desired = scene_camera_desired_transform(
            &rig.session_id,
            &config,
            &current,
            &spawn_registry,
            &camera_targets,
        );
        *transform = scene_camera_apply_animation(
            &config.animation,
            &mut runtime,
            current,
            desired,
            delta_seconds,
        );
    }
}

fn scene_camera_desired_transform(
    session_id: &SceneSessionId,
    config: &SceneCameraConfig,
    current: &Transform,
    spawn_registry: &SceneSpawnRegistry,
    camera_targets: &Query<(&SceneCameraTarget, &GlobalTransform)>,
) -> Transform {
    if config.mode != SceneCameraMode::FollowTarget {
        return config.transform;
    }

    let Some(follow) = config.follow.as_ref() else {
        return config.transform;
    };

    let Some(target_transform) =
        resolve_scene_camera_target_transform(session_id, config, spawn_registry, camera_targets)
    else {
        return config.transform;
    };

    let look_at = target_transform.translation() + follow.look_at_offset;
    let desired_translation = target_transform.translation() + follow.offset;
    let position_lerp = follow.position_lerp.clamp(0.0, 1.0);
    let rotation_lerp = follow.rotation_lerp.clamp(0.0, 1.0);
    let translation = current.translation.lerp(desired_translation, position_lerp);
    let desired_rotation =
        scene_camera_look_at_rotation(translation, look_at).unwrap_or(current.rotation);

    sanitize_scene_camera_transform(
        Transform {
            translation,
            rotation: current.rotation.slerp(desired_rotation, rotation_lerp),
            scale: config.transform.scale,
        },
        config.transform,
    )
}

fn resolve_scene_camera_target_transform(
    session_id: &SceneSessionId,
    config: &SceneCameraConfig,
    spawn_registry: &SceneSpawnRegistry,
    camera_targets: &Query<(&SceneCameraTarget, &GlobalTransform)>,
) -> Option<GlobalTransform> {
    if !spawn_registry.contains_session(session_id) {
        return None;
    }

    let follow = config.follow.as_ref()?;
    match &follow.target_source {
        SceneCameraFollowTargetSource::Anchor(anchor_id) => spawn_registry
            .anchor(session_id, anchor_id)
            .ok()
            .map(|anchor| GlobalTransform::from(anchor.to_transform())),
        SceneCameraFollowTargetSource::SceneTarget => {
            resolve_scene_target_transform(session_id, config, spawn_registry, camera_targets)
        }
        SceneCameraFollowTargetSource::PrimaryActor => best_scene_camera_target_with_tags(
            session_id,
            &[
                SCENE_CAMERA_LOCAL_PLAYER_TARGET_TAG,
                SCENE_CAMERA_PRIMARY_ACTOR_TARGET_TAG,
            ],
            camera_targets,
        ),
        SceneCameraFollowTargetSource::AllParticipants => {
            average_scene_camera_target_transform(session_id, camera_targets)
        }
    }
}

fn resolve_scene_target_transform(
    session_id: &SceneSessionId,
    config: &SceneCameraConfig,
    spawn_registry: &SceneSpawnRegistry,
    camera_targets: &Query<(&SceneCameraTarget, &GlobalTransform)>,
) -> Option<GlobalTransform> {
    if let Some(target) = config.target.as_ref() {
        if let Ok(anchor) = spawn_registry.anchor(session_id, target) {
            return Some(GlobalTransform::from(anchor.to_transform()));
        }

        return best_scene_camera_target_with_tag(session_id, target.as_str(), camera_targets);
    }

    best_scene_camera_target(session_id, camera_targets)
}

fn best_scene_camera_target(
    session_id: &SceneSessionId,
    camera_targets: &Query<(&SceneCameraTarget, &GlobalTransform)>,
) -> Option<GlobalTransform> {
    camera_targets
        .iter()
        .filter(|(target, _)| target.is_session(session_id))
        .max_by_key(|(target, _)| target.priority)
        .map(|(_, transform)| *transform)
}

fn best_scene_camera_target_with_tag(
    session_id: &SceneSessionId,
    tag: &str,
    camera_targets: &Query<(&SceneCameraTarget, &GlobalTransform)>,
) -> Option<GlobalTransform> {
    camera_targets
        .iter()
        .filter(|(target, _)| target.is_session(session_id) && target.has_tag(tag))
        .max_by_key(|(target, _)| target.priority)
        .map(|(_, transform)| *transform)
}

fn best_scene_camera_target_with_tags(
    session_id: &SceneSessionId,
    tags: &[&str],
    camera_targets: &Query<(&SceneCameraTarget, &GlobalTransform)>,
) -> Option<GlobalTransform> {
    tags.iter()
        .find_map(|tag| best_scene_camera_target_with_tag(session_id, tag, camera_targets))
}

fn average_scene_camera_target_transform(
    session_id: &SceneSessionId,
    camera_targets: &Query<(&SceneCameraTarget, &GlobalTransform)>,
) -> Option<GlobalTransform> {
    let mut total = Vec3::ZERO;
    let mut count = 0.0;

    for (target, transform) in camera_targets.iter() {
        if target.is_session(session_id) {
            total += transform.translation();
            count += 1.0;
        }
    }

    (count > 0.0).then(|| GlobalTransform::from(Transform::from_translation(total / count)))
}

fn scene_camera_apply_animation(
    animation: &SceneCameraAnimationConfig,
    runtime: &mut SceneCameraRuntimeState,
    current: Transform,
    desired: Transform,
    delta_seconds: f32,
) -> Transform {
    if !animation.enabled || animation.duration_seconds <= 0.0 {
        runtime.goal = desired;
        runtime.tween_from = desired;
        runtime.tween_elapsed_seconds = animation.duration_seconds.max(0.0);
        return desired;
    }

    if !scene_camera_transform_nearly_eq(runtime.goal, desired) {
        runtime.start_tween(current, desired);
    }

    runtime.tween_elapsed_seconds =
        (runtime.tween_elapsed_seconds + delta_seconds).min(animation.duration_seconds);
    let amount = (runtime.tween_elapsed_seconds / animation.duration_seconds).clamp(0.0, 1.0);
    scene_camera_interpolate_transform(
        runtime.tween_from,
        runtime.goal,
        animation.easing.sample(amount),
    )
}

fn scene_camera_interpolate_transform(from: Transform, to: Transform, amount: f32) -> Transform {
    Transform {
        translation: from.translation.lerp(to.translation, amount),
        rotation: from.rotation.slerp(to.rotation, amount),
        scale: from.scale.lerp(to.scale, amount),
    }
}

fn scene_camera_transform_nearly_eq(left: Transform, right: Transform) -> bool {
    const EPSILON: f32 = 0.0001;

    left.translation.distance_squared(right.translation) <= EPSILON
        && left.rotation.dot(right.rotation).abs() >= 1.0 - EPSILON
        && left.scale.distance_squared(right.scale) <= EPSILON
}

fn scene_camera_look_at_rotation(translation: Vec3, look_at: Vec3) -> Option<Quat> {
    let direction = look_at - translation;
    let direction_length_squared = direction.length_squared();
    (direction.is_finite()
        && direction_length_squared.is_finite()
        && direction_length_squared > 0.000001)
        .then(|| {
            Transform::from_translation(translation)
                .looking_at(look_at, Vec3::Y)
                .rotation
        })
}

fn sanitize_scene_camera_transform(value: Transform, default: Transform) -> Transform {
    let translation = sanitize_vec3(value.translation, default.translation);
    let scale = sanitize_vec3(value.scale, default.scale);
    let rotation_length_squared = value.rotation.length_squared();
    let rotation = if value.rotation.is_finite()
        && rotation_length_squared.is_finite()
        && rotation_length_squared > 0.000001
    {
        value.rotation.normalize()
    } else {
        default.rotation
    };

    Transform {
        translation,
        rotation,
        scale,
    }
}

fn sanitize_vec3(value: Vec3, default: Vec3) -> Vec3 {
    value.is_finite().then_some(value).unwrap_or(default)
}

fn sanitize_unit_interval(value: f32, default: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        default
    }
}

fn sanitize_non_negative(value: f32, default: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        default
    }
}

fn sanitize_positive(value: f32, default: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        default
    }
}

impl SceneCameraEasing {
    fn sample(self, amount: f32) -> f32 {
        let amount = amount.clamp(0.0, 1.0);
        match self {
            Self::Linear => amount,
            Self::SmoothStep => amount * amount * (3.0 - 2.0 * amount),
            Self::EaseInOut => {
                if amount < 0.5 {
                    2.0 * amount * amount
                } else {
                    1.0 - (-2.0 * amount + 2.0).powi(2) / 2.0
                }
            }
        }
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
            Camera {
                order: scene_camera_order(config.mode),
                ..Default::default()
            },
            transform,
            projection,
            SceneCameraRig::new(session_id.clone(), config),
            SceneCameraRuntimeState::new(transform),
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
            Camera {
                order: scene_camera_order(config.mode),
                ..Default::default()
            },
            transform,
            projection,
            SceneCameraRig::new(session_id.clone(), config),
            SceneCameraRuntimeState::new(transform),
            SceneOwned::new(session_id.clone()),
            Name::new("SceneCamera3d"),
        ))
        .id()
}

fn scene_camera_order(mode: SceneCameraMode) -> isize {
    if mode.is_3d() {
        SCENE_CAMERA_3D_ORDER
    } else {
        SCENE_CAMERA_2D_ORDER
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::scene::spawn::{SceneAnchorManifest, SceneSpawnSessionIndex};
    use crate::framework::scene::{
        id::SceneId,
        root::{SceneRoot, despawn_scene_session_entities},
    };

    fn app_with_scene_camera(session_id: &str, config: SceneCameraConfig) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<SceneSpawnRegistry>()
            .add_systems(Update, update_scene_cameras);
        let session_id = SceneSessionId::from(session_id);
        let spawn_index = SceneSpawnSessionIndex::from_manifest_parts(
            SceneId::from("test_scene"),
            session_id.clone(),
            None,
            &[],
            &[],
        );
        app.world_mut()
            .resource_mut::<SceneSpawnRegistry>()
            .set_session_index(spawn_index);
        app.add_systems(Startup, move |mut commands: Commands| {
            spawn_scene_camera(&mut commands, &session_id, config.clone());
        });
        app.update();
        app
    }

    #[test]
    fn scene_camera_orders_keep_3d_below_2d_and_ui() {
        assert!(SCENE_CAMERA_3D_ORDER < SCENE_CAMERA_2D_ORDER);
    }

    #[test]
    fn spawned_3d_scene_camera_order_does_not_conflict_with_2d_order() {
        let mut app = app_with_scene_camera("scene-3d", SceneCameraConfig::gameplay_3d());

        let mut cameras = app
            .world_mut()
            .query_filtered::<(&Camera, &SceneCameraRig, &SceneOwned), With<Camera3d>>();
        let (camera, rig, owned) = cameras.single(app.world()).unwrap();

        assert_eq!(camera.order, SCENE_CAMERA_3D_ORDER);
        assert_ne!(camera.order, SCENE_CAMERA_2D_ORDER);
        assert_eq!(rig.session_id, SceneSessionId::from("scene-3d"));
        assert_eq!(owned.session_id, SceneSessionId::from("scene-3d"));
    }

    #[test]
    fn scene_camera_follow_target_uses_matching_target_tag() {
        let mut config = SceneCameraConfig::follow_target()
            .with_target("player")
            .with_follow(SceneCameraFollowConfig {
                offset: Vec3::new(1.0, 2.0, 3.0),
                look_at_offset: Vec3::ZERO,
                position_lerp: 1.0,
                rotation_lerp: 1.0,
                ..Default::default()
            });
        config.animation.enabled = false;

        let mut app = app_with_scene_camera("scene-a", config);
        app.world_mut().spawn((
            SceneCameraTarget::new("scene-a")
                .with_tag("player")
                .with_priority(10),
            Transform::from_xyz(4.0, 5.0, 6.0),
            GlobalTransform::from(Transform::from_xyz(4.0, 5.0, 6.0)),
        ));

        app.update();

        let mut cameras = app
            .world_mut()
            .query_filtered::<&Transform, With<SceneCameraRig>>();
        let transform = cameras.single(app.world()).unwrap();
        assert_eq!(transform.translation, Vec3::new(5.0, 7.0, 9.0));
    }

    #[test]
    fn scene_camera_follow_anchor_resolves_current_session_only() {
        let mut config = SceneCameraConfig::follow_target().with_follow(SceneCameraFollowConfig {
            target_source: SceneCameraFollowTargetSource::Anchor(SceneAnchorId::from("camera")),
            offset: Vec3::new(0.0, 1.0, 0.0),
            position_lerp: 1.0,
            rotation_lerp: 1.0,
            ..Default::default()
        });
        config.animation.enabled = false;

        let mut app = app_with_scene_camera("scene-a", config);
        let index_a = SceneSpawnSessionIndex::from_manifest_parts(
            SceneId::from("test_scene"),
            SceneSessionId::from("scene-a"),
            None,
            &[],
            &[SceneAnchorManifest::new("camera", [1.0, 2.0, 3.0])],
        );
        let index_b = SceneSpawnSessionIndex::from_manifest_parts(
            SceneId::from("test_scene"),
            SceneSessionId::from("scene-b"),
            None,
            &[],
            &[SceneAnchorManifest::new("camera", [100.0, 2.0, 3.0])],
        );
        {
            let mut registry = app.world_mut().resource_mut::<SceneSpawnRegistry>();
            registry.set_session_index(index_a);
            registry.set_session_index(index_b);
        }

        app.update();

        let mut cameras = app
            .world_mut()
            .query_filtered::<&Transform, With<SceneCameraRig>>();
        let transform = cameras.single(app.world()).unwrap();
        assert_eq!(transform.translation, Vec3::new(1.0, 3.0, 3.0));
    }

    #[test]
    fn scene_camera_animation_interpolates_between_current_and_desired() {
        let mut config = SceneCameraConfig::follow_target()
            .with_target("player")
            .with_follow(SceneCameraFollowConfig {
                offset: Vec3::ZERO,
                look_at_offset: Vec3::new(0.0, 0.0, -1.0),
                position_lerp: 1.0,
                rotation_lerp: 1.0,
                ..Default::default()
            })
            .with_animation(SceneCameraAnimationConfig {
                enabled: true,
                duration_seconds: 1.0,
                easing: SceneCameraEasing::Linear,
            });
        config.transform = Transform::from_xyz(0.0, 0.0, 0.0);

        let mut app = app_with_scene_camera("scene-a", config);
        app.world_mut().spawn((
            SceneCameraTarget::new("scene-a").with_tag("player"),
            Transform::from_xyz(10.0, 0.0, 0.0),
            GlobalTransform::from(Transform::from_xyz(10.0, 0.0, 0.0)),
        ));

        app.update();

        let mut cameras = app
            .world_mut()
            .query_filtered::<&Transform, With<SceneCameraRig>>();
        let transform = cameras.single(app.world()).unwrap();
        assert!(transform.translation.x > 0.0);
        assert!(transform.translation.x < 10.0);
    }

    #[test]
    fn scene_camera_missing_target_falls_back_to_config_transform() {
        let config = SceneCameraConfig::follow_target()
            .with_follow(SceneCameraFollowConfig {
                target_source: SceneCameraFollowTargetSource::PrimaryActor,
                ..Default::default()
            })
            .with_transform(Transform::from_xyz(2.0, 3.0, 4.0));
        let mut app = app_with_scene_camera("scene-a", config);

        app.update();

        let mut cameras = app
            .world_mut()
            .query_filtered::<&Transform, With<SceneCameraRig>>();
        let transform = cameras.single(app.world()).unwrap();
        assert_eq!(transform.translation, Vec3::new(2.0, 3.0, 4.0));
    }

    #[test]
    fn scene_camera_non_finite_runtime_config_keeps_transform_and_projection_finite() {
        let invalid_config = SceneCameraConfig::follow_target()
            .with_transform(Transform::from_xyz(2.0, 3.0, 4.0))
            .with_projection(SceneCameraProjection::Perspective3d {
                fov_y_radians: f32::NAN,
                near: f32::NAN,
                far: f32::NEG_INFINITY,
            })
            .with_follow(SceneCameraFollowConfig {
                target_source: SceneCameraFollowTargetSource::PrimaryActor,
                offset: Vec3::splat(f32::NAN),
                look_at_offset: Vec3::splat(f32::INFINITY),
                position_lerp: f32::NAN,
                rotation_lerp: f32::INFINITY,
                min_visible_targets: 1,
                visible_target_padding: f32::NAN,
            });
        let mut app = app_with_scene_camera("scene-a", SceneCameraConfig::follow_target());
        let camera = app
            .world_mut()
            .query_filtered::<Entity, With<SceneCameraRig>>()
            .single(app.world())
            .unwrap();
        {
            let mut entity = app.world_mut().entity_mut(camera);
            entity.get_mut::<SceneCameraRig>().unwrap().config = invalid_config;
            entity.get_mut::<Transform>().unwrap().rotation =
                Quat::from_xyzw(f32::MAX, 0.0, 0.0, 1.0);
        }

        app.update();

        let mut cameras = app
            .world_mut()
            .query_filtered::<(&Transform, &Projection), With<SceneCameraRig>>();
        let (transform, projection) = cameras.single(app.world()).unwrap();
        assert!(transform.translation.is_finite());
        assert!(transform.rotation.is_finite());
        assert!(transform.scale.is_finite());
        let Projection::Perspective(projection) = projection else {
            panic!("follow target cameras must preserve a perspective projection");
        };
        assert!(projection.fov.is_finite());
        assert!(projection.near.is_finite());
        assert!(projection.far.is_finite());
        assert!(projection.near > 0.0);
        assert!(projection.far > projection.near);
    }

    #[test]
    fn scene_camera_targets_are_session_isolated() {
        let config = SceneCameraConfig::follow_target()
            .with_target("player")
            .with_follow(SceneCameraFollowConfig {
                offset: Vec3::ZERO,
                position_lerp: 1.0,
                rotation_lerp: 1.0,
                ..Default::default()
            });
        let mut app = app_with_scene_camera("scene-a", config);
        app.world_mut().spawn((
            SceneCameraTarget::new("scene-b").with_tag("player"),
            Transform::from_xyz(50.0, 0.0, 0.0),
            GlobalTransform::from(Transform::from_xyz(50.0, 0.0, 0.0)),
        ));

        app.update();

        let mut cameras = app
            .world_mut()
            .query_filtered::<&Transform, With<SceneCameraRig>>();
        let transform = cameras.single(app.world()).unwrap();
        assert_eq!(
            transform.translation,
            default_scene_camera_3d_transform().translation
        );
    }

    #[test]
    fn scene_camera_is_cleaned_up_with_scene_session_entities() {
        let mut app = app_with_scene_camera("scene-a", SceneCameraConfig::gameplay_3d());
        app.world_mut().spawn((
            SceneRoot::new("test_scene", "scene-a"),
            SceneOwned::new("scene-a"),
        ));

        app.world_mut()
            .run_system_cached(
                |mut commands: Commands,
                 scene_roots: Query<(Entity, &SceneRoot)>,
                 owned_entities: Query<(Entity, &SceneOwned)>| {
                    despawn_scene_session_entities(
                        &mut commands,
                        &SceneSessionId::from("scene-a"),
                        &scene_roots,
                        &owned_entities,
                    );
                },
            )
            .unwrap();
        app.update();

        let mut cameras = app.world_mut().query::<&SceneCameraRig>();
        assert!(cameras.iter(app.world()).next().is_none());
    }
}
