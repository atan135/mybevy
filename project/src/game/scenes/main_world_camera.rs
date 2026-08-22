//! Main-world camera intent state.
//!
//! The scene framework owns the actual camera transform. This module stores
//! the validated orbit intent that later input adapters apply to the current
//! session's [`SceneCameraRig`] configuration.

use std::collections::HashMap;

use bevy::{
    input::mouse::{MouseScrollUnit, MouseWheel},
    input::touch::{TouchInput, TouchPhase},
    prelude::*,
    window::{CursorMoved, PrimaryWindow, WindowFocused},
};

use crate::{
    framework::scene::prelude::{
        SCENE_CAMERA_LOCAL_PLAYER_TARGET_TAG, SCENE_CAMERA_PRIMARY_ACTOR_TARGET_TAG,
        SceneCameraAnimationConfig, SceneCameraFollowTargetSource, SceneCameraMode,
        SceneCameraProjection, SceneCameraRig, SceneCameraRuntimeState, SceneCameraTarget,
        SceneSessionId, update_scene_cameras,
    },
    framework::ui::core::{UiInputState, UiInputSystems},
    game::scenes::main_world_entry::MainWorldEntryState,
};

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
const MAIN_WORLD_CAMERA_DESKTOP_YAW_RADIANS_PER_LOGICAL_PIXEL: f32 = 0.006;
const MAIN_WORLD_CAMERA_DESKTOP_PITCH_RADIANS_PER_LOGICAL_PIXEL: f32 = 0.005;
const MAIN_WORLD_CAMERA_DESKTOP_DISTANCE_PER_WHEEL_LINE: f32 = 0.35;
pub(super) const MAIN_WORLD_TOUCH_MOVE_REGION_FRACTION: f32 = 0.4;

pub(super) struct MainWorldCameraPlugin;

impl Plugin for MainWorldCameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MainWorldCameraOrbitState>()
            .init_resource::<MainWorldCameraRigAdapterRuntime>()
            .init_resource::<MainWorldDesktopOrbitRuntime>()
            .init_resource::<MainWorldTouchOrbitRuntime>()
            .init_resource::<ButtonInput<MouseButton>>()
            .add_message::<CursorMoved>()
            .add_message::<MouseWheel>()
            .add_message::<WindowFocused>()
            .add_message::<TouchInput>()
            .add_systems(
                Update,
                (
                    update_main_world_desktop_orbit
                        .after(UiInputSystems::Update)
                        .before(sync_main_world_camera_rig),
                    update_main_world_touch_orbit
                        .after(UiInputSystems::Update)
                        .before(sync_main_world_camera_rig),
                    sync_main_world_camera_rig.before(update_scene_cameras),
                ),
            );
    }
}

/// Stable ownership of the desktop mouse until the matching button release or
/// a gameplay/UI lifecycle gate revokes it. Touch input will use the same
/// ownership boundary with pointer-specific captures in a later stage.
#[derive(Default, Resource)]
struct MainWorldDesktopOrbitRuntime {
    capture: Option<MainWorldDesktopMouseCapture>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct MainWorldDesktopMouseCapture {
    window: Entity,
    last_cursor_position: Option<Vec2>,
}

/// Initial touch ownership is shared with main-world movement. The owner is
/// fixed at touch start; crossing screen regions never transfers it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MainWorldTouchOwner {
    Ui,
    Move,
    CameraOrbit,
    CameraPinch,
}

/// Classifies an initial main-world touch before either camera or movement
/// code mutates local gesture state. UI always wins over gameplay ownership.
pub(super) fn main_world_touch_owner(
    ui_blocks_gameplay: bool,
    viewport_size: Vec2,
    position: Vec2,
) -> MainWorldTouchOwner {
    if ui_blocks_gameplay || !viewport_size.is_finite() || !position.is_finite() {
        MainWorldTouchOwner::Ui
    } else if position.x < viewport_size.x * MAIN_WORLD_TOUCH_MOVE_REGION_FRACTION {
        MainWorldTouchOwner::Move
    } else {
        MainWorldTouchOwner::CameraOrbit
    }
}

#[derive(Clone, Copy, Debug)]
struct MainWorldTouchCapture {
    owner: MainWorldTouchOwner,
    position: Vec2,
}

#[derive(Default, Resource)]
struct MainWorldTouchOrbitRuntime {
    window: Option<Entity>,
    viewport_size: Vec2,
    captures: HashMap<u64, MainWorldTouchCapture>,
    pinch_distance: Option<f32>,
}

impl MainWorldTouchOrbitRuntime {
    fn reset(&mut self) {
        *self = Self::default();
    }
}

impl MainWorldDesktopOrbitRuntime {
    fn release(&mut self) {
        self.capture = None;
    }

    fn begin(&mut self, window: Entity, cursor_position: Option<Vec2>) {
        self.capture = Some(MainWorldDesktopMouseCapture {
            window,
            last_cursor_position: cursor_position,
        });
    }

    #[cfg(test)]
    fn is_capturing(&self) -> bool {
        self.capture.is_some()
    }
}

#[derive(Default, Resource)]
struct MainWorldCameraRigAdapterRuntime {
    session_id: Option<SceneSessionId>,
    generation: u64,
    target_entity: Option<Entity>,
}

impl MainWorldCameraRigAdapterRuntime {
    fn reset(&mut self) {
        *self = Self::default();
    }

    fn requires_smoothing_reset(
        &self,
        session_id: &SceneSessionId,
        generation: u64,
        target_entity: Option<Entity>,
    ) -> bool {
        self.session_id.as_ref() != Some(session_id)
            || self.generation != generation
            || self.target_entity != target_entity
    }

    fn bind(&mut self, session_id: SceneSessionId, generation: u64, target_entity: Option<Entity>) {
        self.session_id = Some(session_id);
        self.generation = generation;
        self.target_entity = target_entity;
    }
}

/// Session-bound, user-adjustable intent for the main-world follow camera.
///
/// `scene_session_id` and `generation` prevent a later controller from
/// carrying orbit state across a re-entry or authority recovery boundary.
#[derive(Clone, Debug, PartialEq, Resource)]
pub(in crate::game) struct MainWorldCameraOrbitState {
    pub follow_player: bool,
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
            follow_player: true,
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

    pub fn follow_offset(&self) -> Vec3 {
        let horizontal_distance = self.distance * self.pitch_radians.cos();
        Vec3::new(
            horizontal_distance * self.yaw_radians.sin(),
            self.distance * self.pitch_radians.sin(),
            horizontal_distance * self.yaw_radians.cos(),
        )
    }

    pub fn look_at_offset(&self) -> Vec3 {
        Vec3::Y * self.look_at_height
    }
}

fn update_main_world_desktop_orbit(
    entry: Option<Res<MainWorldEntryState>>,
    ui_input: Option<Res<UiInputState>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    primary_window: Query<(Entity, &Window), With<PrimaryWindow>>,
    mut cursor_moves: MessageReader<CursorMoved>,
    mut mouse_wheels: MessageReader<MouseWheel>,
    mut focus_events: MessageReader<WindowFocused>,
    mut orbit: ResMut<MainWorldCameraOrbitState>,
    mut desktop_runtime: ResMut<MainWorldDesktopOrbitRuntime>,
) {
    let Some(entry) = entry else {
        desktop_runtime.release();
        return;
    };
    let Some((window_entity, window)) = primary_window.iter().next() else {
        desktop_runtime.release();
        return;
    };
    let ui_blocks_gameplay = ui_input.is_some_and(|input| input.blocks_gameplay_pointer());
    let gameplay_active = entry.allows_gameplay_input() && !ui_blocks_gameplay;
    let primary_window_lost_focus = focus_events
        .read()
        .any(|event| event.window == window_entity && !event.focused);

    if !gameplay_active || primary_window_lost_focus {
        cursor_moves.clear();
        mouse_wheels.clear();
        desktop_runtime.release();
        return;
    }

    let cursor_position = window.cursor_position();
    if mouse_buttons.just_pressed(MouseButton::Right) {
        desktop_runtime.begin(window_entity, cursor_position);
    }
    if mouse_buttons.just_released(MouseButton::Right) || !mouse_buttons.pressed(MouseButton::Right)
    {
        desktop_runtime.release();
    }

    if let Some(capture) = desktop_runtime.capture.as_mut() {
        if capture.window != window_entity {
            desktop_runtime.release();
        } else {
            for cursor_move in cursor_moves.read() {
                if cursor_move.window != capture.window {
                    continue;
                }
                if let Some(previous_position) = capture.last_cursor_position {
                    apply_desktop_orbit_drag(&mut orbit, cursor_move.position - previous_position);
                }
                capture.last_cursor_position = Some(cursor_move.position);
            }
        }
    } else {
        cursor_moves.clear();
    }

    for wheel in mouse_wheels.read() {
        if wheel.window == window_entity {
            apply_desktop_orbit_wheel(&mut orbit, wheel.y, wheel.unit);
        }
    }
}

fn apply_desktop_orbit_drag(orbit: &mut MainWorldCameraOrbitState, logical_delta: Vec2) {
    if !logical_delta.is_finite() {
        return;
    }

    orbit.yaw_radians -= logical_delta.x * MAIN_WORLD_CAMERA_DESKTOP_YAW_RADIANS_PER_LOGICAL_PIXEL;
    orbit.pitch_radians +=
        logical_delta.y * MAIN_WORLD_CAMERA_DESKTOP_PITCH_RADIANS_PER_LOGICAL_PIXEL;
    orbit.sanitize();
}

fn apply_desktop_orbit_wheel(
    orbit: &mut MainWorldCameraOrbitState,
    wheel_delta_y: f32,
    unit: MouseScrollUnit,
) {
    if !wheel_delta_y.is_finite() {
        return;
    }

    let line_delta = match unit {
        MouseScrollUnit::Line => wheel_delta_y,
        MouseScrollUnit::Pixel => wheel_delta_y / MouseScrollUnit::SCROLL_UNIT_CONVERSION_FACTOR,
    };
    orbit.distance -= line_delta * MAIN_WORLD_CAMERA_DESKTOP_DISTANCE_PER_WHEEL_LINE;
    orbit.sanitize();
}

fn update_main_world_touch_orbit(
    entry: Option<Res<MainWorldEntryState>>,
    ui_input: Option<Res<UiInputState>>,
    primary_window: Query<(Entity, &Window), With<PrimaryWindow>>,
    mut touch_events: MessageReader<TouchInput>,
    mut focus_events: MessageReader<WindowFocused>,
    mut orbit: ResMut<MainWorldCameraOrbitState>,
    mut touch_runtime: ResMut<MainWorldTouchOrbitRuntime>,
) {
    let Some(entry) = entry else {
        touch_events.clear();
        touch_runtime.reset();
        return;
    };
    let Some((window_entity, window)) = primary_window.iter().next() else {
        touch_events.clear();
        touch_runtime.reset();
        return;
    };
    let window_size = window.size();
    let focus_lost = focus_events
        .read()
        .any(|event| event.window == window_entity && !event.focused);
    let ui_blocks_gameplay = ui_input
        .as_ref()
        .is_some_and(|input| input.blocks_gameplay_pointer());
    let gate_closed = !entry.allows_gameplay_input()
        || focus_lost
        || window_size.x <= 0.0
        || window_size.y <= 0.0;
    if gate_closed {
        touch_events.clear();
        touch_runtime.reset();
        return;
    }

    if touch_runtime.window != Some(window_entity) || touch_runtime.viewport_size != window_size {
        touch_runtime.reset();
        touch_runtime.window = Some(window_entity);
        touch_runtime.viewport_size = window_size;
    }
    if ui_blocks_gameplay {
        touch_runtime
            .captures
            .retain(|_, capture| capture.owner == MainWorldTouchOwner::Ui);
        touch_runtime.pinch_distance = None;
    }

    for event in touch_events.read() {
        if event.window != window_entity || !event.position.is_finite() {
            continue;
        }
        match event.phase {
            TouchPhase::Started => {
                let owner = main_world_touch_owner(ui_blocks_gameplay, window_size, event.position);
                touch_runtime.captures.insert(
                    event.id,
                    MainWorldTouchCapture {
                        owner,
                        position: event.position,
                    },
                );
                if owner == MainWorldTouchOwner::CameraOrbit
                    && touch_runtime
                        .captures
                        .values()
                        .filter(|capture| capture.owner == MainWorldTouchOwner::CameraOrbit)
                        .count()
                        >= 2
                {
                    for capture in touch_runtime.captures.values_mut() {
                        if capture.owner == MainWorldTouchOwner::CameraOrbit {
                            capture.owner = MainWorldTouchOwner::CameraPinch;
                        }
                    }
                    touch_runtime.pinch_distance = camera_touch_distance(&touch_runtime);
                }
            }
            TouchPhase::Moved => {
                let Some(capture) = touch_runtime.captures.get_mut(&event.id) else {
                    continue;
                };
                let delta = event.position - capture.position;
                capture.position = event.position;
                if capture.owner == MainWorldTouchOwner::CameraOrbit {
                    apply_desktop_orbit_drag(&mut orbit, delta);
                }
                if capture.owner == MainWorldTouchOwner::CameraPinch {
                    let previous_distance = touch_runtime.pinch_distance;
                    touch_runtime.pinch_distance = camera_touch_distance(&touch_runtime);
                    if let (Some(previous), Some(current)) =
                        (previous_distance, touch_runtime.pinch_distance)
                    {
                        orbit.distance -= (current - previous) * 0.01;
                        orbit.sanitize();
                    }
                }
            }
            TouchPhase::Ended | TouchPhase::Canceled => {
                let was_pinch = touch_runtime
                    .captures
                    .get(&event.id)
                    .is_some_and(|capture| capture.owner == MainWorldTouchOwner::CameraPinch);
                touch_runtime.captures.remove(&event.id);
                if was_pinch {
                    let remaining_camera = touch_runtime
                        .captures
                        .values_mut()
                        .find(|capture| capture.owner == MainWorldTouchOwner::CameraPinch);
                    if let Some(capture) = remaining_camera {
                        capture.owner = MainWorldTouchOwner::CameraOrbit;
                    }
                    touch_runtime.pinch_distance = camera_touch_distance(&touch_runtime);
                }
            }
        }
    }
}

fn camera_touch_distance(runtime: &MainWorldTouchOrbitRuntime) -> Option<f32> {
    let mut positions = runtime
        .captures
        .values()
        .filter(|capture| capture.owner == MainWorldTouchOwner::CameraPinch)
        .map(|capture| capture.position);
    let first = positions.next()?;
    let second = positions.next()?;
    Some(first.distance(second))
}

fn sync_main_world_camera_rig(
    entry: Option<Res<MainWorldEntryState>>,
    mut orbit: ResMut<MainWorldCameraOrbitState>,
    mut adapter_runtime: ResMut<MainWorldCameraRigAdapterRuntime>,
    camera_targets: Query<(Entity, &SceneCameraTarget)>,
    mut scene_cameras: Query<
        (
            &mut SceneCameraRig,
            &Transform,
            &mut SceneCameraRuntimeState,
        ),
        With<Camera3d>,
    >,
) {
    let Some(entry) = entry else {
        adapter_runtime.reset();
        *orbit = MainWorldCameraOrbitState::default();
        return;
    };
    let Some(session_id) = entry.scene_session_id.as_ref() else {
        adapter_runtime.reset();
        *orbit = MainWorldCameraOrbitState::default();
        return;
    };
    if !entry.retains_main_world_visuals() {
        adapter_runtime.reset();
        *orbit = MainWorldCameraOrbitState::default();
        return;
    }

    if orbit.scene_session_id.as_ref() != Some(session_id) || orbit.generation != entry.generation {
        orbit.reset_for_session(session_id.clone(), entry.generation);
    } else {
        orbit.sanitize();
    }

    let target_entity = orbit
        .follow_player
        .then(|| primary_actor_target_for_session(session_id, &camera_targets))
        .flatten();
    let reset_smoothing =
        adapter_runtime.requires_smoothing_reset(session_id, entry.generation, target_entity);
    let mut applied = false;

    for (mut rig, transform, mut runtime) in &mut scene_cameras {
        if !rig.is_session(session_id) || !is_main_world_camera_rig(&rig) {
            continue;
        }
        if !orbit.follow_player {
            rig.config.mode = SceneCameraMode::Fixed3d;
            rig.config.transform = *transform;
            rig.config.animation = SceneCameraAnimationConfig::default();
            adapter_runtime.reset();
            continue;
        }
        rig.config.mode = SceneCameraMode::FollowTarget;
        let Some(follow) = rig.config.follow.as_mut() else {
            continue;
        };
        follow.offset = orbit.follow_offset();
        follow.look_at_offset = orbit.look_at_offset();
        follow.position_lerp = if reset_smoothing {
            1.0
        } else {
            orbit.position_lerp
        };
        follow.rotation_lerp = if reset_smoothing {
            1.0
        } else {
            orbit.rotation_lerp
        };
        rig.config.projection = SceneCameraProjection::Perspective3d {
            fov_y_radians: MAIN_WORLD_CAMERA_FOV_Y_RADIANS,
            near: MAIN_WORLD_CAMERA_NEAR,
            far: MAIN_WORLD_CAMERA_FAR,
        };
        rig.config.animation = SceneCameraAnimationConfig::default();
        if reset_smoothing {
            runtime.reset(*transform);
        }
        applied = true;
    }

    if applied {
        adapter_runtime.bind(session_id.clone(), entry.generation, target_entity);
    }
}

fn is_main_world_camera_rig(rig: &SceneCameraRig) -> bool {
    matches!(
        rig.config.mode,
        SceneCameraMode::FollowTarget | SceneCameraMode::Fixed3d
    ) && rig
        .config
        .follow
        .as_ref()
        .is_some_and(|follow| follow.target_source == SceneCameraFollowTargetSource::PrimaryActor)
}

fn primary_actor_target_for_session(
    session_id: &SceneSessionId,
    camera_targets: &Query<(Entity, &SceneCameraTarget)>,
) -> Option<Entity> {
    [
        SCENE_CAMERA_LOCAL_PLAYER_TARGET_TAG,
        SCENE_CAMERA_PRIMARY_ACTOR_TARGET_TAG,
    ]
    .into_iter()
    .find_map(|tag| {
        camera_targets
            .iter()
            .filter(|(_, target)| target.is_session(session_id) && target.has_tag(tag))
            .max_by_key(|(_, target)| target.priority)
            .map(|(entity, _)| entity)
    })
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
        SceneCameraConfig, SceneCameraFollowConfig, SceneCameraFollowTargetSource, SceneCameraMode,
        SceneCameraProjection, SceneManifest, SceneSpawnRegistry, SceneSpawnSessionIndex,
        spawn_scene_camera,
    };
    use crate::game::scenes::main_world_entry::MainWorldEntryPhase;

    const TEST_SCENE_ID: &str = "world.main";

    fn active_camera_app(session_id: SceneSessionId, generation: u64) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<SceneSpawnRegistry>()
            .init_resource::<UiInputState>()
            .insert_resource(MainWorldEntryState {
                generation,
                phase: MainWorldEntryPhase::Active,
                scene_session_id: Some(session_id.clone()),
                ..Default::default()
            })
            .add_plugins(MainWorldCameraPlugin)
            .add_systems(Update, update_scene_cameras);
        app.world_mut()
            .resource_mut::<SceneSpawnRegistry>()
            .set_session_index(SceneSpawnSessionIndex::empty(TEST_SCENE_ID, session_id));
        app
    }

    fn active_desktop_camera_app() -> (App, Entity) {
        let session_id = SceneSessionId::from("main-world-desktop");
        let mut app = active_camera_app(session_id, 1);
        let window = app
            .world_mut()
            .spawn((PrimaryWindow, Window::default()))
            .id();
        app.world_mut()
            .resource_mut::<MainWorldEntryState>()
            .input_frozen = false;
        (app, window)
    }

    fn send_cursor_move(app: &mut App, window: Entity, position: Vec2) {
        app.world_mut().write_message(CursorMoved {
            window,
            position,
            delta: None,
        });
    }

    fn send_wheel(app: &mut App, window: Entity, y: f32, unit: MouseScrollUnit) {
        app.world_mut().write_message(MouseWheel {
            unit,
            x: 0.0,
            y,
            window,
        });
    }

    fn send_touch(app: &mut App, window: Entity, id: u64, phase: TouchPhase, position: Vec2) {
        app.world_mut().write_message(TouchInput {
            phase,
            position,
            window,
            force: None,
            id,
        });
    }

    fn touch_runtime(app: &App) -> &MainWorldTouchOrbitRuntime {
        app.world().resource::<MainWorldTouchOrbitRuntime>()
    }

    fn set_right_mouse_button(app: &mut App, pressed: bool) {
        let mut mouse_buttons = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
        if pressed {
            mouse_buttons.press(MouseButton::Right);
        } else {
            mouse_buttons.release(MouseButton::Right);
        }
    }

    fn clear_mouse_button_transients(app: &mut App) {
        let mut mouse_buttons = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
        mouse_buttons.clear_just_pressed(MouseButton::Right);
        mouse_buttons.clear_just_released(MouseButton::Right);
    }

    fn assert_desktop_capture(app: &App, expected: bool) {
        assert_eq!(
            app.world()
                .resource::<MainWorldDesktopOrbitRuntime>()
                .is_capturing(),
            expected
        );
    }

    fn register_scene_session(app: &mut App, session_id: SceneSessionId) {
        app.world_mut()
            .resource_mut::<SceneSpawnRegistry>()
            .set_session_index(SceneSpawnSessionIndex::empty(TEST_SCENE_ID, session_id));
    }

    fn schedule_follow_camera(
        app: &mut App,
        session_id: SceneSessionId,
        target_source: SceneCameraFollowTargetSource,
        offset: Vec3,
    ) {
        let config = SceneCameraConfig::follow_target().with_follow(SceneCameraFollowConfig {
            target_source,
            offset,
            look_at_offset: Vec3::new(9.0, 8.0, 7.0),
            position_lerp: 0.75,
            rotation_lerp: 0.5,
            ..Default::default()
        });
        app.add_systems(Startup, move |mut commands: Commands| {
            spawn_scene_camera(&mut commands, &session_id, config.clone());
        });
    }

    fn spawn_target(app: &mut App, session_id: &SceneSessionId, position: Vec3) -> Entity {
        let transform = Transform::from_translation(position);
        app.world_mut()
            .spawn((
                SceneCameraTarget::new(session_id.clone())
                    .with_tag(SCENE_CAMERA_LOCAL_PLAYER_TARGET_TAG),
                transform,
                GlobalTransform::from(transform),
            ))
            .id()
    }

    fn camera_for_session(
        app: &mut App,
        session_id: &SceneSessionId,
        target_source: SceneCameraFollowTargetSource,
    ) -> (SceneCameraRig, Transform, Projection) {
        let world = app.world_mut();
        let mut cameras = world.query::<(&SceneCameraRig, &Transform, &Projection)>();
        cameras
            .iter(world)
            .find(|(rig, _, _)| {
                rig.is_session(session_id)
                    && rig
                        .config
                        .follow
                        .as_ref()
                        .is_some_and(|follow| follow.target_source == target_source)
            })
            .map(|(rig, transform, projection)| (rig.clone(), *transform, projection.clone()))
            .unwrap()
    }

    fn assert_vec3_approx_eq(actual: Vec3, expected: Vec3) {
        assert!(
            actual.distance(expected) < 0.0001,
            "expected {expected:?}, got {actual:?}"
        );
    }

    #[test]
    fn default_orbit_state_freezes_the_first_main_world_camera_parameters() {
        let state = MainWorldCameraOrbitState::default();

        assert!(state.follow_player);
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
    fn orbit_offset_uses_yaw_pitch_and_distance_with_a_height_only_look_at() {
        let state = MainWorldCameraOrbitState {
            pitch_radians: std::f32::consts::FRAC_PI_4,
            distance: 2.0,
            ..Default::default()
        };

        assert_vec3_approx_eq(
            state.follow_offset(),
            Vec3::new(0.0, std::f32::consts::SQRT_2, std::f32::consts::SQRT_2),
        );
        assert_vec3_approx_eq(
            MainWorldCameraOrbitState {
                yaw_radians: std::f32::consts::FRAC_PI_2,
                ..state.clone()
            }
            .follow_offset(),
            Vec3::new(std::f32::consts::SQRT_2, std::f32::consts::SQRT_2, 0.0),
        );
        assert_eq!(state.look_at_offset(), Vec3::Y * state.look_at_height);
    }

    #[test]
    fn desktop_right_drag_updates_orbit_only_while_the_capture_is_held() {
        let (mut app, window) = active_desktop_camera_app();
        send_cursor_move(&mut app, window, Vec2::new(100.0, 100.0));
        set_right_mouse_button(&mut app, true);

        app.update();

        assert_desktop_capture(&app, true);
        let initial_orbit = app.world().resource::<MainWorldCameraOrbitState>().clone();
        clear_mouse_button_transients(&mut app);
        send_cursor_move(&mut app, window, Vec2::new(120.0, 90.0));
        app.update();

        let dragged_orbit = app.world().resource::<MainWorldCameraOrbitState>();
        assert_eq!(
            (dragged_orbit.yaw_radians
                - (initial_orbit.yaw_radians
                    - 20.0 * MAIN_WORLD_CAMERA_DESKTOP_YAW_RADIANS_PER_LOGICAL_PIXEL))
                .abs()
                < 0.000001,
            true
        );
        assert_eq!(
            (dragged_orbit.pitch_radians
                - (initial_orbit.pitch_radians
                    - 10.0 * MAIN_WORLD_CAMERA_DESKTOP_PITCH_RADIANS_PER_LOGICAL_PIXEL))
                .abs()
                < 0.000001,
            true
        );

        set_right_mouse_button(&mut app, false);
        app.update();
        assert_desktop_capture(&app, false);
        let released_orbit = app.world().resource::<MainWorldCameraOrbitState>().clone();
        clear_mouse_button_transients(&mut app);
        send_cursor_move(&mut app, window, Vec2::new(180.0, 40.0));
        app.update();
        assert_eq!(
            *app.world().resource::<MainWorldCameraOrbitState>(),
            released_orbit
        );
    }

    #[test]
    fn desktop_wheel_clamps_distance_and_normalizes_pixel_units() {
        let (mut app, window) = active_desktop_camera_app();
        app.update();
        send_wheel(&mut app, window, 2.0, MouseScrollUnit::Line);
        app.update();
        assert_eq!(
            app.world().resource::<MainWorldCameraOrbitState>().distance,
            MAIN_WORLD_CAMERA_MIN_DISTANCE
        );

        send_wheel(
            &mut app,
            window,
            MouseScrollUnit::SCROLL_UNIT_CONVERSION_FACTOR,
            MouseScrollUnit::Pixel,
        );
        app.update();
        assert_eq!(
            app.world().resource::<MainWorldCameraOrbitState>().distance,
            MAIN_WORLD_CAMERA_MIN_DISTANCE
        );

        send_wheel(&mut app, window, 10_000.0, MouseScrollUnit::Line);
        app.update();
        assert_eq!(
            app.world().resource::<MainWorldCameraOrbitState>().distance,
            MAIN_WORLD_CAMERA_MIN_DISTANCE
        );
        send_wheel(&mut app, window, -10_000.0, MouseScrollUnit::Line);
        app.update();
        assert_eq!(
            app.world().resource::<MainWorldCameraOrbitState>().distance,
            MAIN_WORLD_CAMERA_MAX_DISTANCE
        );
    }

    #[test]
    fn desktop_capture_releases_and_discards_input_when_gameplay_or_ui_gates_close() {
        let (mut app, window) = active_desktop_camera_app();
        send_cursor_move(&mut app, window, Vec2::new(10.0, 10.0));
        set_right_mouse_button(&mut app, true);
        app.update();
        assert_desktop_capture(&app, true);
        clear_mouse_button_transients(&mut app);
        let before_gate = app.world().resource::<MainWorldCameraOrbitState>().clone();
        app.world_mut()
            .resource_mut::<MainWorldEntryState>()
            .input_frozen = true;
        send_cursor_move(&mut app, window, Vec2::new(50.0, 10.0));
        send_wheel(&mut app, window, 1.0, MouseScrollUnit::Line);
        app.update();
        assert_desktop_capture(&app, false);
        assert_eq!(
            *app.world().resource::<MainWorldCameraOrbitState>(),
            before_gate
        );

        app.world_mut()
            .resource_mut::<MainWorldEntryState>()
            .input_frozen = false;
        app.world_mut().insert_resource(UiInputState::default());
        app.world_mut()
            .resource_mut::<UiInputState>()
            .pointer_blocked = true;
        set_right_mouse_button(&mut app, true);
        send_cursor_move(&mut app, window, Vec2::new(80.0, 10.0));
        send_wheel(&mut app, window, 1.0, MouseScrollUnit::Line);
        app.update();
        assert_desktop_capture(&app, false);
        assert_eq!(
            *app.world().resource::<MainWorldCameraOrbitState>(),
            before_gate
        );
    }

    #[test]
    fn desktop_capture_releases_on_focus_loss_or_non_active_scene() {
        let (mut app, window) = active_desktop_camera_app();
        send_cursor_move(&mut app, window, Vec2::new(10.0, 10.0));
        set_right_mouse_button(&mut app, true);
        app.update();
        assert_desktop_capture(&app, true);
        clear_mouse_button_transients(&mut app);
        app.world_mut().write_message(WindowFocused {
            window,
            focused: false,
        });
        app.update();
        assert_desktop_capture(&app, false);

        set_right_mouse_button(&mut app, false);
        app.update();
        clear_mouse_button_transients(&mut app);
        app.world_mut().resource_mut::<MainWorldEntryState>().phase =
            MainWorldEntryPhase::Recovering;
        set_right_mouse_button(&mut app, true);
        send_cursor_move(&mut app, window, Vec2::new(40.0, 10.0));
        app.update();
        assert_desktop_capture(&app, false);
    }

    #[test]
    fn touch_right_drag_updates_camera_but_move_owner_stays_out_of_camera() {
        let (mut app, window) = active_desktop_camera_app();
        app.update();

        let initial = app.world().resource::<MainWorldCameraOrbitState>().clone();
        send_touch(
            &mut app,
            window,
            1,
            TouchPhase::Started,
            Vec2::new(700.0, 300.0),
        );
        app.update();
        send_touch(
            &mut app,
            window,
            1,
            TouchPhase::Moved,
            Vec2::new(720.0, 280.0),
        );
        app.update();
        let orbit = app.world().resource::<MainWorldCameraOrbitState>();
        assert_ne!(orbit.yaw_radians, initial.yaw_radians);
        assert_ne!(orbit.pitch_radians, initial.pitch_radians);

        let before_move = orbit.clone();
        send_touch(
            &mut app,
            window,
            2,
            TouchPhase::Started,
            Vec2::new(100.0, 300.0),
        );
        app.update();
        send_touch(
            &mut app,
            window,
            2,
            TouchPhase::Moved,
            Vec2::new(160.0, 250.0),
        );
        app.update();
        assert_eq!(
            *app.world().resource::<MainWorldCameraOrbitState>(),
            before_move
        );
    }

    #[test]
    fn touch_owner_is_locked_and_two_camera_touches_pinch_distance() {
        let (mut app, window) = active_desktop_camera_app();
        app.update();

        send_touch(
            &mut app,
            window,
            1,
            TouchPhase::Started,
            Vec2::new(700.0, 300.0),
        );
        app.update();
        send_touch(
            &mut app,
            window,
            1,
            TouchPhase::Moved,
            Vec2::new(100.0, 300.0),
        );
        app.update();
        assert!(
            app.world()
                .resource::<MainWorldCameraOrbitState>()
                .yaw_radians
                != MAIN_WORLD_CAMERA_DEFAULT_YAW_RADIANS
        );
        assert_eq!(
            touch_runtime(&app)
                .captures
                .get(&1)
                .map(|capture| capture.owner),
            Some(MainWorldTouchOwner::CameraOrbit)
        );

        let before = app.world().resource::<MainWorldCameraOrbitState>().distance;
        send_touch(
            &mut app,
            window,
            2,
            TouchPhase::Started,
            Vec2::new(1000.0, 300.0),
        );
        app.update();
        send_touch(
            &mut app,
            window,
            1,
            TouchPhase::Moved,
            Vec2::new(650.0, 300.0),
        );
        app.update();
        assert!(app.world().resource::<MainWorldCameraOrbitState>().distance > before);
        assert!(
            touch_runtime(&app)
                .captures
                .values()
                .all(|capture| capture.owner == MainWorldTouchOwner::CameraPinch)
        );

        send_touch(
            &mut app,
            window,
            1,
            TouchPhase::Ended,
            Vec2::new(650.0, 300.0),
        );
        app.update();
        assert_eq!(
            touch_runtime(&app)
                .captures
                .get(&2)
                .map(|capture| capture.owner),
            Some(MainWorldTouchOwner::CameraOrbit)
        );
    }

    #[test]
    fn touch_ui_gate_cancels_existing_capture_and_marks_new_touch_ui_owned() {
        let (mut app, window) = active_desktop_camera_app();
        app.update();
        send_touch(
            &mut app,
            window,
            1,
            TouchPhase::Started,
            Vec2::new(700.0, 300.0),
        );
        app.update();
        app.world_mut()
            .resource_mut::<UiInputState>()
            .pointer_blocked = true;
        send_touch(
            &mut app,
            window,
            1,
            TouchPhase::Moved,
            Vec2::new(740.0, 260.0),
        );
        send_touch(
            &mut app,
            window,
            2,
            TouchPhase::Started,
            Vec2::new(800.0, 300.0),
        );
        app.update();
        assert!(
            touch_runtime(&app)
                .captures
                .values()
                .all(|capture| capture.owner == MainWorldTouchOwner::Ui)
        );
        let blocked_orbit = app.world().resource::<MainWorldCameraOrbitState>().clone();
        send_touch(
            &mut app,
            window,
            2,
            TouchPhase::Moved,
            Vec2::new(850.0, 240.0),
        );
        app.update();
        assert_eq!(
            *app.world().resource::<MainWorldCameraOrbitState>(),
            blocked_orbit
        );
    }

    #[test]
    fn touch_focus_loss_cancel_and_resize_clear_runtime() {
        let (mut app, window) = active_desktop_camera_app();
        app.update();
        send_touch(
            &mut app,
            window,
            1,
            TouchPhase::Started,
            Vec2::new(700.0, 300.0),
        );
        app.update();
        app.world_mut().write_message(WindowFocused {
            window,
            focused: false,
        });
        app.update();
        assert!(touch_runtime(&app).captures.is_empty());

        app.world_mut().write_message(WindowFocused {
            window,
            focused: true,
        });
        app.update();
        send_touch(
            &mut app,
            window,
            2,
            TouchPhase::Started,
            Vec2::new(700.0, 300.0),
        );
        app.update();
        app.world_mut()
            .entity_mut(window)
            .get_mut::<Window>()
            .unwrap()
            .resolution
            .set(1024.0, 768.0);
        send_touch(
            &mut app,
            window,
            2,
            TouchPhase::Canceled,
            Vec2::new(700.0, 300.0),
        );
        app.update();
        assert!(touch_runtime(&app).captures.is_empty());
    }

    #[test]
    fn active_adapter_updates_only_the_current_main_world_rig_before_scene_camera_update() {
        let session_id = SceneSessionId::from("main-world-a");
        let target_position = Vec3::new(10.0, 0.5, -2.0);
        let mut app = active_camera_app(session_id.clone(), 4);
        spawn_target(&mut app, &session_id, target_position);
        schedule_follow_camera(
            &mut app,
            session_id.clone(),
            SceneCameraFollowTargetSource::PrimaryActor,
            Vec3::new(99.0, 99.0, 99.0),
        );

        app.update();

        let (rig, transform, projection) = camera_for_session(
            &mut app,
            &session_id,
            SceneCameraFollowTargetSource::PrimaryActor,
        );
        let follow = rig.config.follow.unwrap();
        let expected_offset = MainWorldCameraOrbitState::default().follow_offset();
        assert_vec3_approx_eq(follow.offset, expected_offset);
        assert_eq!(
            follow.look_at_offset,
            Vec3::Y * MAIN_WORLD_CAMERA_DEFAULT_LOOK_AT_HEIGHT
        );
        assert_eq!(follow.position_lerp, 1.0);
        assert_eq!(follow.rotation_lerp, 1.0);
        assert_vec3_approx_eq(transform.translation, target_position + expected_offset);
        let Projection::Perspective(projection) = projection else {
            panic!("main world camera must remain perspective");
        };
        assert_eq!(projection.fov, MAIN_WORLD_CAMERA_FOV_Y_RADIANS);
        assert_eq!(projection.near, MAIN_WORLD_CAMERA_NEAR);
        assert_eq!(projection.far, MAIN_WORLD_CAMERA_FAR);
    }

    #[test]
    fn camera_follow_intent_freezes_the_current_rig_and_can_resume_following() {
        let session_id = SceneSessionId::from("main-world-follow-toggle");
        let target_position = Vec3::new(3.0, 0.0, 1.0);
        let mut app = active_camera_app(session_id.clone(), 1);
        spawn_target(&mut app, &session_id, target_position);
        schedule_follow_camera(
            &mut app,
            session_id.clone(),
            SceneCameraFollowTargetSource::PrimaryActor,
            Vec3::ZERO,
        );
        app.update();
        let followed_transform = camera_for_session(
            &mut app,
            &session_id,
            SceneCameraFollowTargetSource::PrimaryActor,
        )
        .1;

        app.world_mut()
            .resource_mut::<MainWorldCameraOrbitState>()
            .follow_player = false;
        app.update();
        let (frozen_rig, frozen_transform, _) = camera_for_session(
            &mut app,
            &session_id,
            SceneCameraFollowTargetSource::PrimaryActor,
        );
        assert_eq!(frozen_rig.config.mode, SceneCameraMode::Fixed3d);
        assert_eq!(frozen_transform, followed_transform);

        app.world_mut()
            .resource_mut::<MainWorldCameraOrbitState>()
            .follow_player = true;
        app.update();
        let (resumed_rig, _, _) = camera_for_session(
            &mut app,
            &session_id,
            SceneCameraFollowTargetSource::PrimaryActor,
        );
        assert_eq!(resumed_rig.config.mode, SceneCameraMode::FollowTarget);
    }

    #[test]
    fn active_adapter_leaves_other_session_and_non_primary_rigs_unchanged() {
        let session_a = SceneSessionId::from("main-world-a");
        let session_b = SceneSessionId::from("main-world-b");
        let other_offset = Vec3::new(7.0, 8.0, 9.0);
        let wrong_session_offset = Vec3::new(4.0, 5.0, 6.0);
        let mut app = active_camera_app(session_a.clone(), 1);
        register_scene_session(&mut app, session_b.clone());
        spawn_target(&mut app, &session_a, Vec3::new(3.0, 0.0, 1.0));
        schedule_follow_camera(
            &mut app,
            session_a.clone(),
            SceneCameraFollowTargetSource::PrimaryActor,
            Vec3::ZERO,
        );
        schedule_follow_camera(
            &mut app,
            session_a.clone(),
            SceneCameraFollowTargetSource::SceneTarget,
            other_offset,
        );
        schedule_follow_camera(
            &mut app,
            session_b.clone(),
            SceneCameraFollowTargetSource::PrimaryActor,
            wrong_session_offset,
        );
        let ui_camera = app
            .world_mut()
            .spawn((
                Camera2d,
                Projection::Orthographic(OrthographicProjection::default_2d()),
            ))
            .id();

        app.update();

        let (other_rig, _, _) = camera_for_session(
            &mut app,
            &session_a,
            SceneCameraFollowTargetSource::SceneTarget,
        );
        assert_eq!(other_rig.config.follow.unwrap().offset, other_offset);
        let (wrong_session_rig, _, _) = camera_for_session(
            &mut app,
            &session_b,
            SceneCameraFollowTargetSource::PrimaryActor,
        );
        assert_eq!(
            wrong_session_rig.config.follow.unwrap().offset,
            wrong_session_offset
        );
        assert!(matches!(
            app.world().entity(ui_camera).get::<Projection>(),
            Some(Projection::Orthographic(_))
        ));
    }

    #[test]
    fn target_replacement_resets_smoothing_for_one_update_then_restores_orbit_smoothing() {
        let session_id = SceneSessionId::from("main-world-a");
        let mut app = active_camera_app(session_id.clone(), 1);
        let first_target = spawn_target(&mut app, &session_id, Vec3::ZERO);
        schedule_follow_camera(
            &mut app,
            session_id.clone(),
            SceneCameraFollowTargetSource::PrimaryActor,
            Vec3::ZERO,
        );
        app.update();
        {
            let mut orbit = app.world_mut().resource_mut::<MainWorldCameraOrbitState>();
            orbit.position_lerp = 0.3;
            orbit.rotation_lerp = 0.4;
        }
        app.world_mut().despawn(first_target);
        spawn_target(&mut app, &session_id, Vec3::new(8.0, 0.0, 0.0));

        app.update();

        let (replacement_rig, _, _) = camera_for_session(
            &mut app,
            &session_id,
            SceneCameraFollowTargetSource::PrimaryActor,
        );
        let replacement_follow = replacement_rig.config.follow.unwrap();
        assert_eq!(replacement_follow.position_lerp, 1.0);
        assert_eq!(replacement_follow.rotation_lerp, 1.0);

        app.update();

        let (settled_rig, _, _) = camera_for_session(
            &mut app,
            &session_id,
            SceneCameraFollowTargetSource::PrimaryActor,
        );
        let settled_follow = settled_rig.config.follow.unwrap();
        assert_eq!(settled_follow.position_lerp, 0.3);
        assert_eq!(settled_follow.rotation_lerp, 0.4);
    }

    #[test]
    fn session_or_generation_change_resets_orbit_and_updates_only_the_new_session_rig() {
        let session_a = SceneSessionId::from("main-world-a");
        let session_b = SceneSessionId::from("main-world-b");
        let mut app = active_camera_app(session_a.clone(), 1);
        register_scene_session(&mut app, session_b.clone());
        spawn_target(&mut app, &session_a, Vec3::ZERO);
        spawn_target(&mut app, &session_b, Vec3::new(2.0, 0.0, 0.0));
        schedule_follow_camera(
            &mut app,
            session_a.clone(),
            SceneCameraFollowTargetSource::PrimaryActor,
            Vec3::ZERO,
        );
        schedule_follow_camera(
            &mut app,
            session_b.clone(),
            SceneCameraFollowTargetSource::PrimaryActor,
            Vec3::new(8.0, 8.0, 8.0),
        );
        app.update();
        {
            let mut orbit = app.world_mut().resource_mut::<MainWorldCameraOrbitState>();
            orbit.yaw_radians = std::f32::consts::FRAC_PI_2;
            orbit.distance = 6.0;
        }
        app.update();
        let changed_a_offset = camera_for_session(
            &mut app,
            &session_a,
            SceneCameraFollowTargetSource::PrimaryActor,
        )
        .0
        .config
        .follow
        .unwrap()
        .offset;
        assert_vec3_approx_eq(
            changed_a_offset,
            Vec3::new(
                6.0 * MAIN_WORLD_CAMERA_DEFAULT_PITCH_RADIANS.cos(),
                6.0 * MAIN_WORLD_CAMERA_DEFAULT_PITCH_RADIANS.sin(),
                0.0,
            ),
        );
        {
            let mut entry = app.world_mut().resource_mut::<MainWorldEntryState>();
            entry.generation = 2;
            entry.scene_session_id = Some(session_b.clone());
        }

        app.update();

        let orbit = app.world().resource::<MainWorldCameraOrbitState>();
        assert_eq!(orbit.scene_session_id.as_ref(), Some(&session_b));
        assert_eq!(orbit.generation, 2);
        assert_eq!(orbit.yaw_radians, MAIN_WORLD_CAMERA_DEFAULT_YAW_RADIANS);
        assert_eq!(orbit.distance, MAIN_WORLD_CAMERA_DEFAULT_DISTANCE);
        let reset_b_offset = camera_for_session(
            &mut app,
            &session_b,
            SceneCameraFollowTargetSource::PrimaryActor,
        )
        .0
        .config
        .follow
        .unwrap()
        .offset;
        assert_vec3_approx_eq(
            reset_b_offset,
            MainWorldCameraOrbitState::default().follow_offset(),
        );
        let retained_a_offset = camera_for_session(
            &mut app,
            &session_a,
            SceneCameraFollowTargetSource::PrimaryActor,
        )
        .0
        .config
        .follow
        .unwrap()
        .offset;
        assert_vec3_approx_eq(retained_a_offset, changed_a_offset);
    }

    #[test]
    fn recovery_preserves_orbit_and_reentry_starts_from_defaults() {
        let session_id = SceneSessionId::from("main-world-lifecycle");
        let mut app = active_camera_app(session_id.clone(), 1);
        spawn_target(&mut app, &session_id, Vec3::ZERO);
        schedule_follow_camera(
            &mut app,
            session_id.clone(),
            SceneCameraFollowTargetSource::PrimaryActor,
            Vec3::ZERO,
        );
        app.update();
        {
            let mut orbit = app.world_mut().resource_mut::<MainWorldCameraOrbitState>();
            orbit.yaw_radians = 1.2;
            orbit.distance = 7.0;
        }
        app.update();

        {
            let mut entry = app.world_mut().resource_mut::<MainWorldEntryState>();
            entry.phase = MainWorldEntryPhase::Recovering;
            entry.reconnect_requested = true;
        }
        app.update();
        assert_eq!(
            *app.world().resource::<MainWorldCameraOrbitState>(),
            MainWorldCameraOrbitState {
                yaw_radians: 1.2,
                distance: 7.0,
                scene_session_id: Some(session_id.clone()),
                generation: 1,
                ..Default::default()
            }
        );
        assert!(
            app.world()
                .resource::<MainWorldCameraRigAdapterRuntime>()
                .session_id
                .is_some()
        );

        app.world_mut().resource_mut::<MainWorldEntryState>().phase =
            MainWorldEntryPhase::WaitingSceneReady;
        app.update();
        assert_eq!(
            *app.world().resource::<MainWorldCameraOrbitState>(),
            MainWorldCameraOrbitState {
                yaw_radians: 1.2,
                distance: 7.0,
                scene_session_id: Some(session_id.clone()),
                generation: 1,
                ..Default::default()
            }
        );
        assert!(
            app.world()
                .resource::<MainWorldCameraRigAdapterRuntime>()
                .session_id
                .is_some()
        );

        {
            let mut entry = app.world_mut().resource_mut::<MainWorldEntryState>();
            entry.phase = MainWorldEntryPhase::Active;
            entry.reconnect_requested = false;
        }
        app.update();
        assert_eq!(
            *app.world().resource::<MainWorldCameraOrbitState>(),
            MainWorldCameraOrbitState {
                scene_session_id: Some(session_id),
                generation: 1,
                yaw_radians: 1.2,
                distance: 7.0,
                ..Default::default()
            }
        );
    }

    #[test]
    fn main_world_adapter_does_not_modify_preview_or_ui_cameras_and_keeps_one_world_rig() {
        let session_id = SceneSessionId::from("main-world-camera-conflict");
        let mut app = active_camera_app(session_id.clone(), 1);
        spawn_target(&mut app, &session_id, Vec3::ZERO);
        schedule_follow_camera(
            &mut app,
            session_id.clone(),
            SceneCameraFollowTargetSource::PrimaryActor,
            Vec3::ZERO,
        );
        let preview = app
            .world_mut()
            .spawn((
                Camera3d::default(),
                Transform::from_translation(Vec3::new(20.0, 21.0, 22.0)),
                Projection::Perspective(PerspectiveProjection {
                    fov: 0.47,
                    near: 0.11,
                    far: 99.0,
                    ..Default::default()
                }),
            ))
            .id();
        let preview_transform_before = *app.world().entity(preview).get::<Transform>().unwrap();
        let Projection::Perspective(preview_projection_before) =
            app.world().entity(preview).get::<Projection>().unwrap()
        else {
            panic!("preview camera should remain perspective");
        };
        let preview_projection_before = preview_projection_before.clone();

        app.update();
        assert_eq!(
            app.world_mut()
                .query::<(&SceneCameraRig, &Camera3d)>()
                .iter(app.world())
                .filter(|(rig, _)| rig.is_session(&session_id))
                .count(),
            1
        );
        assert_eq!(
            *app.world().entity(preview).get::<Transform>().unwrap(),
            preview_transform_before
        );
        let Projection::Perspective(preview_projection_after) =
            app.world().entity(preview).get::<Projection>().unwrap()
        else {
            panic!("preview camera should remain perspective");
        };
        assert_eq!(preview_projection_after.fov, preview_projection_before.fov);
        assert_eq!(
            preview_projection_after.near,
            preview_projection_before.near
        );
        assert_eq!(preview_projection_after.far, preview_projection_before.far);
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
