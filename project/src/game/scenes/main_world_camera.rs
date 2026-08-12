//! Main-world camera intent state.
//!
//! The scene framework owns the actual camera transform. This module stores
//! the validated orbit intent that later input adapters apply to the current
//! session's [`SceneCameraRig`] configuration.

use bevy::prelude::*;

use crate::framework::scene::prelude::SceneSessionId;

pub(super) const MAIN_WORLD_CAMERA_DEFAULT_YAW_RADIANS: f32 = 0.0;
pub(super) const MAIN_WORLD_CAMERA_DEFAULT_PITCH_RADIANS: f32 = std::f32::consts::FRAC_PI_4;
pub(super) const MAIN_WORLD_CAMERA_MIN_PITCH_RADIANS: f32 = 20.0_f32.to_radians();
pub(super) const MAIN_WORLD_CAMERA_MAX_PITCH_RADIANS: f32 = 75.0_f32.to_radians();
pub(super) const MAIN_WORLD_CAMERA_DEFAULT_DISTANCE: f32 = 2.0;
pub(super) const MAIN_WORLD_CAMERA_MIN_DISTANCE: f32 = 1.5;
pub(super) const MAIN_WORLD_CAMERA_MAX_DISTANCE: f32 = 12.0;
pub(super) const MAIN_WORLD_CAMERA_DEFAULT_LOOK_AT_HEIGHT: f32 = 0.25;
pub(super) const MAIN_WORLD_CAMERA_MIN_LOOK_AT_HEIGHT: f32 = 0.0;
pub(super) const MAIN_WORLD_CAMERA_MAX_LOOK_AT_HEIGHT: f32 = 2.0;
pub(super) const MAIN_WORLD_CAMERA_DEFAULT_POSITION_LERP: f32 = 0.25;
pub(super) const MAIN_WORLD_CAMERA_DEFAULT_ROTATION_LERP: f32 = 0.25;
pub(super) const MAIN_WORLD_CAMERA_MIN_LERP: f32 = 0.0;
pub(super) const MAIN_WORLD_CAMERA_MAX_LERP: f32 = 1.0;
pub(super) const MAIN_WORLD_CAMERA_FOV_Y_RADIANS: f32 = 0.82;
pub(super) const MAIN_WORLD_CAMERA_NEAR: f32 = 0.02;
pub(super) const MAIN_WORLD_CAMERA_FAR: f32 = 800.0;

pub(super) struct MainWorldCameraPlugin;

impl Plugin for MainWorldCameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MainWorldCameraOrbitState>();
    }
}

/// Session-bound, user-adjustable intent for the main-world follow camera.
///
/// `scene_session_id` and `generation` prevent a later controller from
/// carrying orbit state across a re-entry or authority recovery boundary.
#[derive(Clone, Debug, PartialEq, Resource)]
pub(in crate::game) struct MainWorldCameraOrbitState {
    pub yaw_radians: f32,
    pub pitch_radians: f32,
    pub distance: f32,
    pub look_at_height: f32,
    pub position_lerp: f32,
    pub rotation_lerp: f32,
    pub scene_session_id: Option<SceneSessionId>,
    pub generation: u64,
}

impl Default for MainWorldCameraOrbitState {
    fn default() -> Self {
        Self {
            yaw_radians: MAIN_WORLD_CAMERA_DEFAULT_YAW_RADIANS,
            pitch_radians: MAIN_WORLD_CAMERA_DEFAULT_PITCH_RADIANS,
            distance: MAIN_WORLD_CAMERA_DEFAULT_DISTANCE,
            look_at_height: MAIN_WORLD_CAMERA_DEFAULT_LOOK_AT_HEIGHT,
            position_lerp: MAIN_WORLD_CAMERA_DEFAULT_POSITION_LERP,
            rotation_lerp: MAIN_WORLD_CAMERA_DEFAULT_ROTATION_LERP,
            scene_session_id: None,
            generation: 0,
        }
    }
}

impl MainWorldCameraOrbitState {
    pub fn reset_for_session(&mut self, scene_session_id: SceneSessionId, generation: u64) {
        *self = Self {
            scene_session_id: Some(scene_session_id),
            generation,
            ..Self::default()
        };
    }

    /// Replaces invalid values with the frozen defaults and clamps all bounded
    /// values before they can be copied into a scene camera rig.
    pub fn sanitize(&mut self) {
        self.yaw_radians = normalize_yaw(self.yaw_radians);
        self.pitch_radians = clamp_finite(
            self.pitch_radians,
            MAIN_WORLD_CAMERA_DEFAULT_PITCH_RADIANS,
            MAIN_WORLD_CAMERA_MIN_PITCH_RADIANS,
            MAIN_WORLD_CAMERA_MAX_PITCH_RADIANS,
        );
        self.distance = clamp_finite(
            self.distance,
            MAIN_WORLD_CAMERA_DEFAULT_DISTANCE,
            MAIN_WORLD_CAMERA_MIN_DISTANCE,
            MAIN_WORLD_CAMERA_MAX_DISTANCE,
        );
        self.look_at_height = clamp_finite(
            self.look_at_height,
            MAIN_WORLD_CAMERA_DEFAULT_LOOK_AT_HEIGHT,
            MAIN_WORLD_CAMERA_MIN_LOOK_AT_HEIGHT,
            MAIN_WORLD_CAMERA_MAX_LOOK_AT_HEIGHT,
        );
        self.position_lerp = clamp_finite(
            self.position_lerp,
            MAIN_WORLD_CAMERA_DEFAULT_POSITION_LERP,
            MAIN_WORLD_CAMERA_MIN_LERP,
            MAIN_WORLD_CAMERA_MAX_LERP,
        );
        self.rotation_lerp = clamp_finite(
            self.rotation_lerp,
            MAIN_WORLD_CAMERA_DEFAULT_ROTATION_LERP,
            MAIN_WORLD_CAMERA_MIN_LERP,
            MAIN_WORLD_CAMERA_MAX_LERP,
        );
    }

    pub fn has_valid_values(&self) -> bool {
        self.yaw_radians.is_finite()
            && (-std::f32::consts::PI..=std::f32::consts::PI).contains(&self.yaw_radians)
            && value_is_in_range(
                self.pitch_radians,
                MAIN_WORLD_CAMERA_MIN_PITCH_RADIANS,
                MAIN_WORLD_CAMERA_MAX_PITCH_RADIANS,
            )
            && value_is_in_range(
                self.distance,
                MAIN_WORLD_CAMERA_MIN_DISTANCE,
                MAIN_WORLD_CAMERA_MAX_DISTANCE,
            )
            && value_is_in_range(
                self.look_at_height,
                MAIN_WORLD_CAMERA_MIN_LOOK_AT_HEIGHT,
                MAIN_WORLD_CAMERA_MAX_LOOK_AT_HEIGHT,
            )
            && value_is_in_range(
                self.position_lerp,
                MAIN_WORLD_CAMERA_MIN_LERP,
                MAIN_WORLD_CAMERA_MAX_LERP,
            )
            && value_is_in_range(
                self.rotation_lerp,
                MAIN_WORLD_CAMERA_MIN_LERP,
                MAIN_WORLD_CAMERA_MAX_LERP,
            )
    }
}

fn clamp_finite(value: f32, default: f32, minimum: f32, maximum: f32) -> f32 {
    if value.is_finite() {
        value.clamp(minimum, maximum)
    } else {
        default
    }
}

fn normalize_yaw(value: f32) -> f32 {
    if value.is_finite() {
        (value + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
    } else {
        MAIN_WORLD_CAMERA_DEFAULT_YAW_RADIANS
    }
}

fn value_is_in_range(value: f32, minimum: f32, maximum: f32) -> bool {
    value.is_finite() && (minimum..=maximum).contains(&value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::scene::prelude::{
        SceneCameraFollowTargetSource, SceneCameraMode, SceneCameraProjection, SceneManifest,
    };

    #[test]
    fn default_orbit_state_freezes_the_first_main_world_camera_parameters() {
        let state = MainWorldCameraOrbitState::default();

        assert_eq!(state.yaw_radians, MAIN_WORLD_CAMERA_DEFAULT_YAW_RADIANS);
        assert_eq!(state.pitch_radians, MAIN_WORLD_CAMERA_DEFAULT_PITCH_RADIANS);
        assert_eq!(state.distance, MAIN_WORLD_CAMERA_DEFAULT_DISTANCE);
        assert_eq!(
            state.look_at_height,
            MAIN_WORLD_CAMERA_DEFAULT_LOOK_AT_HEIGHT
        );
        assert_eq!(state.position_lerp, MAIN_WORLD_CAMERA_DEFAULT_POSITION_LERP);
        assert_eq!(state.rotation_lerp, MAIN_WORLD_CAMERA_DEFAULT_ROTATION_LERP);
        assert_eq!(state.scene_session_id, None);
        assert_eq!(state.generation, 0);
        assert!(state.has_valid_values());
    }

    #[test]
    fn orbit_state_clamps_ranges_and_recovers_from_non_finite_input() {
        let mut state = MainWorldCameraOrbitState {
            yaw_radians: f32::NAN,
            pitch_radians: 100.0,
            distance: f32::INFINITY,
            look_at_height: -1.0,
            position_lerp: -0.5,
            rotation_lerp: 2.0,
            ..Default::default()
        };

        state.sanitize();

        assert_eq!(state.yaw_radians, MAIN_WORLD_CAMERA_DEFAULT_YAW_RADIANS);
        assert_eq!(state.pitch_radians, MAIN_WORLD_CAMERA_MAX_PITCH_RADIANS);
        assert_eq!(state.distance, MAIN_WORLD_CAMERA_DEFAULT_DISTANCE);
        assert_eq!(state.look_at_height, MAIN_WORLD_CAMERA_MIN_LOOK_AT_HEIGHT);
        assert_eq!(state.position_lerp, MAIN_WORLD_CAMERA_MIN_LERP);
        assert_eq!(state.rotation_lerp, MAIN_WORLD_CAMERA_MAX_LERP);
        assert!(state.has_valid_values());
    }

    #[test]
    fn reset_for_session_discards_previous_orbit_and_binds_the_new_generation() {
        let mut state = MainWorldCameraOrbitState {
            yaw_radians: 1.0,
            distance: 7.0,
            ..Default::default()
        };

        state.reset_for_session(SceneSessionId::from("main-world-2"), 9);

        assert_eq!(
            state,
            MainWorldCameraOrbitState {
                scene_session_id: Some(SceneSessionId::from("main-world-2")),
                generation: 9,
                ..Default::default()
            }
        );
    }

    #[test]
    fn main_world_manifest_uses_a_primary_actor_follow_camera_with_frozen_projection() {
        let manifest =
            SceneManifest::load_first_package_ron("scenes/main_world/scene.ron").unwrap();
        let config = manifest.entry.camera.unwrap().into_config();
        let follow = config.follow.unwrap();

        assert_eq!(config.mode, SceneCameraMode::FollowTarget);
        assert_eq!(
            follow.target_source,
            SceneCameraFollowTargetSource::PrimaryActor
        );
        assert_eq!(
            follow.position_lerp,
            MAIN_WORLD_CAMERA_DEFAULT_POSITION_LERP
        );
        assert_eq!(
            follow.rotation_lerp,
            MAIN_WORLD_CAMERA_DEFAULT_ROTATION_LERP
        );
        assert_eq!(
            follow.look_at_offset,
            Vec3::Y * MAIN_WORLD_CAMERA_DEFAULT_LOOK_AT_HEIGHT
        );
        let SceneCameraProjection::Perspective3d {
            fov_y_radians,
            near,
            far,
        } = config.projection
        else {
            panic!("main world follow camera must use a perspective projection");
        };
        assert_eq!(fov_y_radians, MAIN_WORLD_CAMERA_FOV_Y_RADIANS);
        assert_eq!(near, MAIN_WORLD_CAMERA_NEAR);
        assert_eq!(far, MAIN_WORLD_CAMERA_FAR);
    }
}
