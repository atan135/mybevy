//! Client movement runtime boundary for the public main world.
//!
//! This module owns no scene entry policy and does not yet simulate movement.
//! It establishes the bounded, generation-scoped state and schedule required
//! by later input, send, prediction, correction, and interpolation stages.

use std::collections::{HashMap, VecDeque};

use bevy::{
    input::touch::{TouchInput, TouchPhase},
    prelude::*,
    time::Fixed,
    transform::TransformSystems,
    window::{AppLifecycle, PrimaryWindow, WindowFocused},
};

use crate::{
    framework::{
        scene::prelude::SceneSessionId,
        ui::core::{UiInputState, UiInputSystems},
    },
    game::{
        myserver::MyServerUpdateSet,
        scenes::{
            main_world_camera::{
                MainWorldCameraOrbitState, MainWorldTouchOwner, main_world_touch_owner,
            },
            main_world_contract::{
                MAIN_WORLD_AUTHORITY_TICKS_PER_SECOND, MainWorldAuthorityFrame,
                MainWorldConfirmedFrame, MainWorldPredictedFrame, MainWorldRenderFrame,
                main_world_normalized_direction,
            },
            main_world_entry::{MainWorldEntryPhase, MainWorldEntryState, MainWorldEntryUpdateSet},
        },
    },
};

/// Maximum number of locally-sent authority inputs that can await a server
/// acknowledgement. At 20 Hz this preserves five seconds of replay context.
pub(in crate::game) const MAIN_WORLD_PREDICTION_HISTORY_CAPACITY: usize = 100;

/// Maximum number of authority samples retained for any one remote character.
/// At 20 Hz this gives interpolation a two-second bounded history window.
pub(in crate::game) const MAIN_WORLD_REMOTE_INTERPOLATION_CAPACITY: usize = 40;

/// Logical-pixel virtual-stick tuning. Values are deliberately independent of
/// render resolution so the stick retains a stable dead zone across devices.
pub(in crate::game) const MAIN_WORLD_TOUCH_JOYSTICK_RADIUS: f32 = 80.0;
pub(in crate::game) const MAIN_WORLD_TOUCH_JOYSTICK_DEAD_ZONE: f32 = 12.0;

/// Established ordering of the client movement pipeline. The named sets are
/// intentionally sparse in this stage: later stages fill them without moving
/// lifecycle or transform ownership back into the entry coordinator.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, SystemSet)]
pub(in crate::game) enum MainWorldMovementUpdateSet {
    /// Read validated network messages into authority baselines and queues.
    ConsumeAuthority,
    /// Collect desktop/touch intent after UI input ownership is resolved.
    CollectIntent,
    /// Build 20 Hz send commands after current-frame intent is known.
    DispatchInput,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, SystemSet)]
pub(in crate::game) enum MainWorldMovementFixedSet {
    /// Advance fixed local prediction from stored intent and input history.
    Predict,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, SystemSet)]
pub(in crate::game) enum MainWorldMovementPostUpdateSet {
    /// Write already-computed visual transforms before Bevy propagation.
    WriteTransforms,
}

/// Continuous planar player intent. It is a request for a later fixed-step
/// prediction/send stage, never an assertion that a Transform is authoritative.
#[derive(Clone, Copy, Debug, Default, PartialEq, Resource)]
pub(in crate::game) struct MainWorldMovementIntent {
    pub direction: Vec2,
    pub active: bool,
    /// Monotonically advances only when an active intent becomes idle. The
    /// stage-4 sender uses this to emit one `MOVE_STOP` per transition.
    pub stop_sequence: u64,
}

impl MainWorldMovementIntent {
    pub fn clear(&mut self) {
        self.direction = Vec2::ZERO;
        self.active = false;
    }

    fn set_direction(&mut self, direction: Vec2) {
        self.direction = direction;
        self.active = true;
    }

    fn request_stop(&mut self) {
        if self.active {
            self.stop_sequence = self.stop_sequence.wrapping_add(1).max(1);
        }
        self.clear();
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct MainWorldTouchMoveCapture {
    id: u64,
    origin: Vec2,
    position: Vec2,
}

/// Input-device state that is intentionally separate from prediction state.
/// A touch that begins in the move zone can never become a camera touch.
#[derive(Default, Resource)]
struct MainWorldMovementInputRuntime {
    window: Option<Entity>,
    viewport_size: Vec2,
    touch_move_capture: Option<MainWorldTouchMoveCapture>,
    keyboard_rearm_required: bool,
}

impl MainWorldMovementInputRuntime {
    fn reset_touch(&mut self) {
        self.touch_move_capture = None;
    }

    fn reset_all(&mut self) {
        *self = Self::default();
    }
}

/// Predicted local player state between authoritative snapshots.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(in crate::game) struct MainWorldPredictedState {
    pub frame: MainWorldPredictedFrame,
    pub position: Vec3,
    pub direction: Vec2,
    pub moving: bool,
}

/// An input retained until `EntityTransform.last_input_frame` confirms it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::game) struct MainWorldUnconfirmedInput {
    pub frame: MainWorldPredictedFrame,
    pub direction: Vec2,
    pub predicted_before: MainWorldPredictedState,
    pub predicted_after: MainWorldPredictedState,
}

/// Last accepted server state for the local character. Later correction code
/// replays only input history newer than `confirmed_frame` from this baseline.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(in crate::game) struct MainWorldAuthorityBaseline {
    pub frame: MainWorldAuthorityFrame,
    pub confirmed_frame: MainWorldConfirmedFrame,
    pub position: Vec3,
    pub direction: Vec2,
    pub moving: bool,
}

/// One authority sample for a remote player. Remote movement is always driven
/// from this cache and never from local input prediction.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(in crate::game) struct MainWorldRemoteSnapshot {
    pub frame: MainWorldAuthorityFrame,
    pub position: Vec3,
    pub direction: Vec2,
    pub moving: bool,
}

/// Per-character bounded interpolation queue. Duplicate frame IDs replace the
/// stored sample, retained late frames are inserted in authority order, and
/// samples older than the eviction window are rejected.
#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::game) struct MainWorldRemoteInterpolationBuffer {
    snapshots: VecDeque<MainWorldRemoteSnapshot>,
}

impl MainWorldRemoteInterpolationBuffer {
    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }

    pub fn snapshots(&self) -> &VecDeque<MainWorldRemoteSnapshot> {
        &self.snapshots
    }

    pub fn push(&mut self, snapshot: MainWorldRemoteSnapshot) -> bool {
        if let Some(existing) = self
            .snapshots
            .iter_mut()
            .find(|existing| existing.frame == snapshot.frame)
        {
            *existing = snapshot;
            return true;
        }
        if self
            .snapshots
            .front()
            .is_some_and(|oldest| snapshot.frame < oldest.frame)
        {
            return false;
        }
        let insert_at = self
            .snapshots
            .iter()
            .position(|existing| existing.frame > snapshot.frame)
            .unwrap_or(self.snapshots.len());
        self.snapshots.insert(insert_at, snapshot);
        if self.snapshots.len() > MAIN_WORLD_REMOTE_INTERPOLATION_CAPACITY {
            self.snapshots.pop_front();
        }
        true
    }
}

/// All movement state scoped to a main-world entry generation and scene
/// session. `clear` is deliberately the lifecycle cleanup operation used for
/// exits, failures, disconnect recovery, and generation changes.
#[derive(Clone, Debug, Resource)]
pub(in crate::game) struct MainWorldMovementRuntime {
    pub generation: u64,
    pub session_id: Option<SceneSessionId>,
    pub input_frozen: bool,
    pub render_frame: MainWorldRenderFrame,
    pub predicted: MainWorldPredictedState,
    pub authority_baseline: Option<MainWorldAuthorityBaseline>,
    pub unconfirmed_inputs: VecDeque<MainWorldUnconfirmedInput>,
    pub remote_interpolation: HashMap<String, MainWorldRemoteInterpolationBuffer>,
}

impl Default for MainWorldMovementRuntime {
    fn default() -> Self {
        Self {
            generation: 0,
            session_id: None,
            input_frozen: true,
            render_frame: MainWorldRenderFrame::default(),
            predicted: MainWorldPredictedState::default(),
            authority_baseline: None,
            unconfirmed_inputs: VecDeque::new(),
            remote_interpolation: HashMap::new(),
        }
    }
}

impl MainWorldMovementRuntime {
    pub fn allows_local_movement(&self) -> bool {
        !self.input_frozen && self.session_id.is_some()
    }

    pub fn bind_active_session(&mut self, generation: u64, session_id: SceneSessionId) {
        if self.generation != generation || self.session_id.as_ref() != Some(&session_id) {
            self.clear();
            self.generation = generation;
            self.session_id = Some(session_id);
        }
        self.input_frozen = false;
    }

    pub fn freeze(&mut self) {
        self.input_frozen = true;
    }

    pub fn clear(&mut self) {
        self.input_frozen = true;
        self.session_id = None;
        self.render_frame = MainWorldRenderFrame::default();
        self.predicted = MainWorldPredictedState::default();
        self.authority_baseline = None;
        self.unconfirmed_inputs.clear();
        self.remote_interpolation.clear();
    }

    pub fn push_unconfirmed_input(&mut self, input: MainWorldUnconfirmedInput) {
        self.unconfirmed_inputs.push_back(input);
        if self.unconfirmed_inputs.len() > MAIN_WORLD_PREDICTION_HISTORY_CAPACITY {
            self.unconfirmed_inputs.pop_front();
        }
    }

    pub fn remote_buffer_mut(
        &mut self,
        character_id: impl Into<String>,
    ) -> &mut MainWorldRemoteInterpolationBuffer {
        self.remote_interpolation
            .entry(character_id.into())
            .or_default()
    }
}

pub(in crate::game) struct MainWorldMovementPlugin;

impl Plugin for MainWorldMovementPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MainWorldMovementIntent>()
            .init_resource::<MainWorldMovementRuntime>()
            .init_resource::<MainWorldMovementInputRuntime>()
            .insert_resource(Time::<Fixed>::from_hz(f64::from(
                MAIN_WORLD_AUTHORITY_TICKS_PER_SECOND,
            )))
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<TouchInput>()
            .add_message::<WindowFocused>()
            .add_message::<AppLifecycle>()
            .configure_sets(
                Update,
                (
                    MainWorldMovementUpdateSet::ConsumeAuthority
                        .after(MainWorldEntryUpdateSet::Coordinator),
                    MainWorldMovementUpdateSet::CollectIntent
                        .after(MainWorldMovementUpdateSet::ConsumeAuthority),
                    MainWorldMovementUpdateSet::DispatchInput
                        .after(MainWorldMovementUpdateSet::CollectIntent)
                        .before(MyServerUpdateSet::CommandDispatch),
                ),
            )
            .configure_sets(FixedUpdate, MainWorldMovementFixedSet::Predict)
            .configure_sets(
                PostUpdate,
                MainWorldMovementPostUpdateSet::WriteTransforms.before(TransformSystems::Propagate),
            )
            .add_systems(
                Update,
                (
                    sync_main_world_movement_lifecycle
                        .in_set(MainWorldMovementUpdateSet::ConsumeAuthority),
                    collect_main_world_movement_intent
                        .after(UiInputSystems::Update)
                        .in_set(MainWorldMovementUpdateSet::CollectIntent),
                ),
            )
            .add_systems(
                FixedUpdate,
                maintain_main_world_movement_fixed_gate.in_set(MainWorldMovementFixedSet::Predict),
            )
            .add_systems(
                PostUpdate,
                advance_main_world_render_frame
                    .in_set(MainWorldMovementPostUpdateSet::WriteTransforms),
            );
    }
}

/// Applies entry lifecycle ownership before later movement stages inspect any
/// input, predict a position, or write a player transform.
fn sync_main_world_movement_lifecycle(
    entry: Option<Res<MainWorldEntryState>>,
    mut intent: ResMut<MainWorldMovementIntent>,
    mut runtime: ResMut<MainWorldMovementRuntime>,
    mut input_runtime: ResMut<MainWorldMovementInputRuntime>,
) {
    let Some(entry) = entry else {
        intent.request_stop();
        runtime.clear();
        input_runtime.reset_all();
        return;
    };
    let Some(session_id) = entry.scene_session_id.clone() else {
        intent.request_stop();
        runtime.clear();
        input_runtime.reset_all();
        runtime.generation = entry.generation;
        return;
    };

    if entry.phase == MainWorldEntryPhase::Active && !entry.input_frozen {
        runtime.bind_active_session(entry.generation, session_id);
        return;
    }

    intent.request_stop();
    runtime.clear();
    input_runtime.reset_all();
    runtime.generation = entry.generation;
}

/// Samples keyboard and left-region virtual-stick input into a world-relative
/// intent. It never mutates player state, prediction, or network commands.
#[allow(clippy::too_many_arguments)]
fn collect_main_world_movement_intent(
    entry: Option<Res<MainWorldEntryState>>,
    ui_input: Option<Res<UiInputState>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    primary_window: Query<(Entity, &Window), With<PrimaryWindow>>,
    camera_orbit: Option<Res<MainWorldCameraOrbitState>>,
    mut touch_events: MessageReader<TouchInput>,
    mut focus_events: MessageReader<WindowFocused>,
    mut lifecycle_events: MessageReader<AppLifecycle>,
    mut intent: ResMut<MainWorldMovementIntent>,
    mut input_runtime: ResMut<MainWorldMovementInputRuntime>,
    runtime: Res<MainWorldMovementRuntime>,
) {
    let Some(entry) = entry else {
        touch_events.clear();
        intent.request_stop();
        input_runtime.reset_all();
        return;
    };
    let Some((window_entity, window)) = primary_window.iter().next() else {
        touch_events.clear();
        intent.request_stop();
        input_runtime.reset_all();
        return;
    };

    let focus_lost = focus_events
        .read()
        .any(|event| event.window == window_entity && !event.focused);
    let backgrounded = lifecycle_events
        .read()
        .any(|event| matches!(event, AppLifecycle::WillSuspend | AppLifecycle::Suspended));
    let ui_blocks_gameplay = ui_input.is_some_and(|input| input.blocks_gameplay_pointer());
    let viewport_size = window.size();
    let gameplay_open = runtime.allows_local_movement()
        && entry.allows_gameplay_input()
        && !ui_blocks_gameplay
        && !focus_lost
        && !backgrounded
        && viewport_size.x > 0.0
        && viewport_size.y > 0.0;

    if input_runtime.window != Some(window_entity) || input_runtime.viewport_size != viewport_size {
        input_runtime.reset_touch();
        input_runtime.window = Some(window_entity);
        input_runtime.viewport_size = viewport_size;
    }
    if !gameplay_open {
        touch_events.clear();
        intent.request_stop();
        input_runtime.reset_touch();
        input_runtime.keyboard_rearm_required = true;
        return;
    }

    let touch_axis = update_main_world_touch_move_capture(
        &mut touch_events,
        window_entity,
        viewport_size,
        ui_blocks_gameplay,
        &mut input_runtime,
    );
    let keyboard_axis = keyboard_movement_axis(&keyboard);
    if input_runtime.keyboard_rearm_required {
        if keyboard_axis != Vec2::ZERO || touch_axis != Vec2::ZERO {
            return;
        }
        input_runtime.keyboard_rearm_required = false;
    }
    let local_axis =
        match main_world_normalized_direction((keyboard_axis + touch_axis).clamp_length_max(1.0)) {
            Ok(direction) => direction,
            Err(_) => {
                intent.request_stop();
                return;
            }
        };
    let camera = camera_orbit.as_deref().cloned().unwrap_or_default();
    let world_direction = main_world_camera_relative_direction(local_axis, camera.yaw_radians);
    intent.set_direction(world_direction);
}

fn keyboard_movement_axis(keyboard: &ButtonInput<KeyCode>) -> Vec2 {
    let forward = keyboard.pressed(KeyCode::KeyW) || keyboard.pressed(KeyCode::ArrowUp);
    let backward = keyboard.pressed(KeyCode::KeyS) || keyboard.pressed(KeyCode::ArrowDown);
    let left = keyboard.pressed(KeyCode::KeyA) || keyboard.pressed(KeyCode::ArrowLeft);
    let right = keyboard.pressed(KeyCode::KeyD) || keyboard.pressed(KeyCode::ArrowRight);
    Vec2::new(
        (right as i8 - left as i8) as f32,
        (forward as i8 - backward as i8) as f32,
    )
}

/// Maps local stick/keyboard axes (right, forward) to an XZ world direction
/// using the current follow-camera yaw. Pitch does not alter planar movement.
pub(in crate::game) fn main_world_camera_relative_direction(local_axis: Vec2, yaw: f32) -> Vec2 {
    if !local_axis.is_finite() || !yaw.is_finite() {
        return Vec2::ZERO;
    }
    let forward = Vec2::new(yaw.sin(), yaw.cos());
    let right = Vec2::new(forward.y, -forward.x);
    main_world_normalized_direction(right * local_axis.x + forward * local_axis.y)
        .unwrap_or(Vec2::ZERO)
}

fn update_main_world_touch_move_capture(
    touch_events: &mut MessageReader<TouchInput>,
    window_entity: Entity,
    viewport_size: Vec2,
    ui_blocks_gameplay: bool,
    input_runtime: &mut MainWorldMovementInputRuntime,
) -> Vec2 {
    for event in touch_events.read() {
        if event.window != window_entity || !event.position.is_finite() {
            continue;
        }
        match event.phase {
            TouchPhase::Started => {
                if input_runtime.touch_move_capture.is_none()
                    && main_world_touch_owner(ui_blocks_gameplay, viewport_size, event.position)
                        == MainWorldTouchOwner::Move
                {
                    input_runtime.touch_move_capture = Some(MainWorldTouchMoveCapture {
                        id: event.id,
                        origin: event.position,
                        position: event.position,
                    });
                }
            }
            TouchPhase::Moved => {
                if let Some(capture) = input_runtime.touch_move_capture.as_mut()
                    && capture.id == event.id
                {
                    capture.position = event.position;
                }
            }
            TouchPhase::Ended | TouchPhase::Canceled => {
                if input_runtime
                    .touch_move_capture
                    .is_some_and(|capture| capture.id == event.id)
                {
                    input_runtime.reset_touch();
                }
            }
        }
    }
    input_runtime
        .touch_move_capture
        .map_or(Vec2::ZERO, |capture| {
            main_world_virtual_joystick_axis(capture.position - capture.origin)
        })
}

/// Converts a virtual-stick displacement into continuous local movement. A
/// fixed dead zone prevents tiny touch jitter from producing `MOVE_DIR` later.
pub(in crate::game) fn main_world_virtual_joystick_axis(displacement: Vec2) -> Vec2 {
    if !displacement.is_finite() {
        return Vec2::ZERO;
    }
    let length = displacement.length();
    if length <= MAIN_WORLD_TOUCH_JOYSTICK_DEAD_ZONE {
        return Vec2::ZERO;
    }
    let magnitude = ((length - MAIN_WORLD_TOUCH_JOYSTICK_DEAD_ZONE)
        / (MAIN_WORLD_TOUCH_JOYSTICK_RADIUS - MAIN_WORLD_TOUCH_JOYSTICK_DEAD_ZONE))
        .clamp(0.0, 1.0);
    displacement.normalize_or_zero() * magnitude
}

/// Stage-2 fixed-update gate. It intentionally performs no simulation yet;
/// this verifies future prediction cannot run while entry lifecycle is frozen.
fn maintain_main_world_movement_fixed_gate(
    runtime: Res<MainWorldMovementRuntime>,
    intent: Res<MainWorldMovementIntent>,
) {
    if !runtime.allows_local_movement() || !intent.active {
        return;
    }
}

/// Rendering is a separate cadence from authority or prediction frames. Later
/// presentation smoothing uses this monotonically advancing visual frame only.
fn advance_main_world_render_frame(mut runtime: ResMut<MainWorldMovementRuntime>) {
    if runtime.session_id.is_some() {
        runtime.render_frame.0 = runtime.render_frame.0.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn active_entry(generation: u64, session_id: &str) -> MainWorldEntryState {
        MainWorldEntryState {
            generation,
            phase: MainWorldEntryPhase::Active,
            scene_session_id: Some(SceneSessionId::from(session_id)),
            input_frozen: false,
            ..Default::default()
        }
    }

    fn movement_app(entry: MainWorldEntryState) -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(entry)
            .add_plugins(MainWorldMovementPlugin);
        let window = app
            .world_mut()
            .spawn((PrimaryWindow, Window::default()))
            .id();
        (app, window)
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

    fn press_key(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
    }

    fn release_key(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(key);
    }

    fn intent(app: &App) -> MainWorldMovementIntent {
        *app.world().resource::<MainWorldMovementIntent>()
    }

    fn assert_vec2_approx_eq(actual: Vec2, expected: Vec2) {
        assert!(
            actual.distance(expected) < 0.0001,
            "expected {expected:?}, got {actual:?}"
        );
    }

    fn input(frame: u32) -> MainWorldUnconfirmedInput {
        let predicted = MainWorldPredictedState {
            frame: MainWorldPredictedFrame(frame),
            position: Vec3::new(frame as f32, 0.0, 0.0),
            direction: Vec2::X,
            moving: true,
        };
        MainWorldUnconfirmedInput {
            frame: MainWorldPredictedFrame(frame),
            direction: Vec2::X,
            predicted_before: predicted,
            predicted_after: predicted,
        }
    }

    fn remote_snapshot(frame: u32) -> MainWorldRemoteSnapshot {
        MainWorldRemoteSnapshot {
            frame: MainWorldAuthorityFrame(frame),
            position: Vec3::new(frame as f32, 0.0, 0.0),
            direction: Vec2::X,
            moving: true,
        }
    }

    #[test]
    fn resources_start_frozen_with_empty_bounded_collections() {
        let runtime = MainWorldMovementRuntime::default();
        assert!(runtime.input_frozen);
        assert!(!runtime.allows_local_movement());
        assert!(runtime.unconfirmed_inputs.is_empty());
        assert!(runtime.remote_interpolation.is_empty());
        assert_eq!(runtime.render_frame, MainWorldRenderFrame(0));
        assert_eq!(MAIN_WORLD_PREDICTION_HISTORY_CAPACITY, 100);
        assert_eq!(MAIN_WORLD_REMOTE_INTERPOLATION_CAPACITY, 40);
    }

    #[test]
    fn lifecycle_opens_only_for_active_unfrozen_entry_and_resets_intent() {
        let (mut app, _) = movement_app(active_entry(7, "main-world-7"));
        app.update();
        assert!(
            app.world()
                .resource::<MainWorldMovementRuntime>()
                .allows_local_movement()
        );

        {
            let mut intent = app.world_mut().resource_mut::<MainWorldMovementIntent>();
            intent.active = true;
            intent.direction = Vec2::X;
        }
        {
            let mut entry = app.world_mut().resource_mut::<MainWorldEntryState>();
            entry.input_frozen = true;
        }
        app.update();

        assert!(
            !app.world()
                .resource::<MainWorldMovementRuntime>()
                .allows_local_movement()
        );
        assert_eq!(intent(&app).direction, Vec2::ZERO);
        assert!(!intent(&app).active);
        assert_eq!(intent(&app).stop_sequence, 1);
    }

    #[test]
    fn prediction_history_evicts_oldest_input_at_capacity() {
        let mut runtime = MainWorldMovementRuntime::default();
        for frame in 0..=MAIN_WORLD_PREDICTION_HISTORY_CAPACITY as u32 {
            runtime.push_unconfirmed_input(input(frame));
        }
        assert_eq!(
            runtime.unconfirmed_inputs.len(),
            MAIN_WORLD_PREDICTION_HISTORY_CAPACITY
        );
        assert_eq!(
            runtime.unconfirmed_inputs.front().unwrap().frame,
            MainWorldPredictedFrame(1)
        );
        assert_eq!(
            runtime.unconfirmed_inputs.back().unwrap().frame,
            MainWorldPredictedFrame(MAIN_WORLD_PREDICTION_HISTORY_CAPACITY as u32)
        );
    }

    #[test]
    fn remote_buffers_replace_duplicates_reject_old_frames_and_evict_oldest() {
        let mut buffer = MainWorldRemoteInterpolationBuffer::default();
        assert!(buffer.push(remote_snapshot(10)));
        assert!(buffer.push(remote_snapshot(12)));
        assert!(buffer.push(remote_snapshot(11)));
        assert_eq!(
            buffer
                .snapshots()
                .iter()
                .map(|snapshot| snapshot.frame)
                .collect::<Vec<_>>(),
            vec![
                MainWorldAuthorityFrame(10),
                MainWorldAuthorityFrame(11),
                MainWorldAuthorityFrame(12),
            ]
        );
        for frame in 13..=(10 + MAIN_WORLD_REMOTE_INTERPOLATION_CAPACITY as u32) {
            assert!(buffer.push(remote_snapshot(frame)));
        }
        let mut replacement = remote_snapshot(20);
        replacement.moving = false;
        assert!(buffer.push(replacement));
        assert_eq!(buffer.len(), MAIN_WORLD_REMOTE_INTERPOLATION_CAPACITY);
        assert_eq!(
            buffer.snapshots().front().unwrap().frame,
            MainWorldAuthorityFrame(11)
        );
        assert!(
            !buffer
                .snapshots()
                .iter()
                .find(|snapshot| snapshot.frame == MainWorldAuthorityFrame(20))
                .unwrap()
                .moving
        );
        assert!(!buffer.push(remote_snapshot(9)));
        assert!(buffer.push(remote_snapshot(
            11 + MAIN_WORLD_REMOTE_INTERPOLATION_CAPACITY as u32
        )));
        assert_eq!(buffer.len(), MAIN_WORLD_REMOTE_INTERPOLATION_CAPACITY);
        assert_eq!(
            buffer.snapshots().front().unwrap().frame,
            MainWorldAuthorityFrame(12)
        );
        assert_eq!(
            buffer.snapshots().back().unwrap().frame,
            MainWorldAuthorityFrame(11 + MAIN_WORLD_REMOTE_INTERPOLATION_CAPACITY as u32)
        );
    }

    #[test]
    fn generation_change_disconnect_exit_and_failure_clear_scoped_runtime() {
        let (mut app, _) = movement_app(active_entry(1, "main-world-1"));
        app.update();
        {
            let mut runtime = app.world_mut().resource_mut::<MainWorldMovementRuntime>();
            runtime.push_unconfirmed_input(input(1));
            runtime.remote_buffer_mut("remote").push(remote_snapshot(1));
        }

        for phase in [
            MainWorldEntryPhase::Recovering,
            MainWorldEntryPhase::Exiting,
            MainWorldEntryPhase::Failed,
        ] {
            {
                let mut entry = app.world_mut().resource_mut::<MainWorldEntryState>();
                entry.phase = phase;
                entry.input_frozen = true;
            }
            app.update();
            let runtime = app.world().resource::<MainWorldMovementRuntime>();
            assert!(runtime.input_frozen);
            assert!(runtime.session_id.is_none());
            assert!(runtime.unconfirmed_inputs.is_empty());
            assert!(runtime.remote_interpolation.is_empty());
        }

        *app.world_mut().resource_mut::<MainWorldEntryState>() = active_entry(2, "main-world-2");
        app.update();
        let runtime = app.world().resource::<MainWorldMovementRuntime>();
        assert_eq!(runtime.generation, 2);
        assert_eq!(
            runtime.session_id,
            Some(SceneSessionId::from("main-world-2"))
        );
        assert!(runtime.unconfirmed_inputs.is_empty());
        assert!(runtime.remote_interpolation.is_empty());
    }

    #[test]
    fn render_frame_advances_only_for_a_bound_session() {
        let (mut app, _) = movement_app(MainWorldEntryState::default());
        app.update();
        assert_eq!(
            app.world()
                .resource::<MainWorldMovementRuntime>()
                .render_frame,
            MainWorldRenderFrame(0)
        );

        *app.world_mut().resource_mut::<MainWorldEntryState>() = active_entry(1, "main-world-1");
        app.update();
        assert_eq!(
            app.world()
                .resource::<MainWorldMovementRuntime>()
                .render_frame,
            MainWorldRenderFrame(1)
        );
    }

    fn freeze_entry_in_coordinator_set(mut entry: ResMut<MainWorldEntryState>) {
        entry.input_frozen = true;
    }

    #[test]
    fn movement_lifecycle_runs_after_entry_coordinator_and_fixed_time_is_20_hz() {
        let (mut app, _) = movement_app(active_entry(1, "main-world-1"));
        app.add_systems(
            Update,
            freeze_entry_in_coordinator_set.in_set(MainWorldEntryUpdateSet::Coordinator),
        );
        app.update();

        assert!(
            app.world()
                .resource::<MainWorldMovementRuntime>()
                .input_frozen
        );
        assert_eq!(
            app.world().resource::<Time<Fixed>>().timestep(),
            std::time::Duration::from_millis(50)
        );
    }

    #[test]
    fn keyboard_axes_normalize_diagonals_and_use_wasd_or_arrows() {
        let (mut app, _) = movement_app(active_entry(1, "main-world-1"));
        press_key(&mut app, KeyCode::KeyW);
        press_key(&mut app, KeyCode::KeyD);
        app.update();
        assert!(intent(&app).active);
        assert!((intent(&app).direction.length() - 1.0).abs() < f32::EPSILON);
        assert_vec2_approx_eq(intent(&app).direction, Vec2::new(1.0, 1.0).normalize());

        release_key(&mut app, KeyCode::KeyW);
        release_key(&mut app, KeyCode::KeyD);
        press_key(&mut app, KeyCode::ArrowLeft);
        press_key(&mut app, KeyCode::ArrowDown);
        app.update();
        assert_vec2_approx_eq(intent(&app).direction, Vec2::new(-1.0, -1.0).normalize());
    }

    #[test]
    fn camera_relative_mapping_rotates_local_axes_on_the_xz_plane() {
        assert_eq!(main_world_camera_relative_direction(Vec2::Y, 0.0), Vec2::Y);
        assert_vec2_approx_eq(
            main_world_camera_relative_direction(Vec2::Y, std::f32::consts::FRAC_PI_2),
            Vec2::X,
        );
        assert_eq!(main_world_camera_relative_direction(Vec2::X, 0.0), Vec2::X);
        assert_eq!(
            main_world_camera_relative_direction(Vec2::new(f32::NAN, 0.0), 0.0),
            Vec2::ZERO
        );
    }

    #[test]
    fn left_touch_stick_has_dead_zone_continuous_axis_and_locked_ownership() {
        assert_eq!(
            main_world_virtual_joystick_axis(Vec2::splat(1.0)),
            Vec2::ZERO
        );
        assert_eq!(
            main_world_virtual_joystick_axis(Vec2::new(MAIN_WORLD_TOUCH_JOYSTICK_DEAD_ZONE, 0.0)),
            Vec2::ZERO
        );
        assert_eq!(
            main_world_virtual_joystick_axis(Vec2::new(MAIN_WORLD_TOUCH_JOYSTICK_RADIUS, 0.0)),
            Vec2::X
        );

        let (mut app, window) = movement_app(active_entry(1, "main-world-1"));
        app.update();
        send_touch(
            &mut app,
            window,
            1,
            TouchPhase::Started,
            Vec2::new(100.0, 300.0),
        );
        app.update();
        send_touch(
            &mut app,
            window,
            1,
            TouchPhase::Moved,
            Vec2::new(180.0, 300.0),
        );
        app.update();
        assert_eq!(intent(&app).direction, Vec2::X);

        send_touch(
            &mut app,
            window,
            1,
            TouchPhase::Moved,
            Vec2::new(900.0, 300.0),
        );
        app.update();
        assert!(intent(&app).active);
        assert_eq!(intent(&app).direction, Vec2::X);

        send_touch(
            &mut app,
            window,
            1,
            TouchPhase::Ended,
            Vec2::new(900.0, 300.0),
        );
        app.update();
        assert!(!intent(&app).active);
        assert_eq!(intent(&app).stop_sequence, 1);
    }

    #[test]
    fn right_and_ui_touches_never_claim_movement_and_camera_uses_same_owner_rule() {
        let viewport = Vec2::new(1280.0, 720.0);
        assert_eq!(
            main_world_touch_owner(false, viewport, Vec2::new(100.0, 100.0)),
            MainWorldTouchOwner::Move
        );
        assert_eq!(
            main_world_touch_owner(false, viewport, Vec2::new(700.0, 100.0)),
            MainWorldTouchOwner::CameraOrbit
        );
        assert_eq!(
            main_world_touch_owner(true, viewport, Vec2::new(100.0, 100.0)),
            MainWorldTouchOwner::Ui
        );

        let (mut app, window) = movement_app(active_entry(1, "main-world-1"));
        app.update();
        send_touch(
            &mut app,
            window,
            1,
            TouchPhase::Started,
            Vec2::new(800.0, 300.0),
        );
        send_touch(
            &mut app,
            window,
            1,
            TouchPhase::Moved,
            Vec2::new(880.0, 300.0),
        );
        app.update();
        assert!(!intent(&app).active);

        app.world_mut().insert_resource(UiInputState::default());
        app.world_mut()
            .resource_mut::<UiInputState>()
            .pointer_blocked = true;
        send_touch(
            &mut app,
            window,
            2,
            TouchPhase::Started,
            Vec2::new(100.0, 300.0),
        );
        app.update();
        assert!(!intent(&app).active);
    }

    #[test]
    fn release_focus_background_and_ui_gates_emit_one_stop_and_require_rearm() {
        let (mut app, window) = movement_app(active_entry(1, "main-world-1"));
        press_key(&mut app, KeyCode::KeyW);
        app.update();
        assert!(intent(&app).active);
        assert_eq!(intent(&app).stop_sequence, 0);

        release_key(&mut app, KeyCode::KeyW);
        app.update();
        assert!(!intent(&app).active);
        assert_eq!(intent(&app).stop_sequence, 1);
        app.update();
        assert_eq!(intent(&app).stop_sequence, 1);

        press_key(&mut app, KeyCode::KeyW);
        app.update();
        assert!(intent(&app).active);
        app.world_mut().write_message(WindowFocused {
            window,
            focused: false,
        });
        app.update();
        assert!(!intent(&app).active);
        assert_eq!(intent(&app).stop_sequence, 2);

        app.update();
        assert!(!intent(&app).active);
        release_key(&mut app, KeyCode::KeyW);
        app.update();
        press_key(&mut app, KeyCode::KeyW);
        app.update();
        assert!(intent(&app).active);

        app.world_mut().write_message(AppLifecycle::WillSuspend);
        app.update();
        assert!(!intent(&app).active);
        assert_eq!(intent(&app).stop_sequence, 3);

        release_key(&mut app, KeyCode::KeyW);
        app.update();
        press_key(&mut app, KeyCode::KeyW);
        app.update();
        assert!(intent(&app).active);
        app.world_mut().insert_resource(UiInputState::default());
        app.world_mut()
            .resource_mut::<UiInputState>()
            .pointer_blocked = true;
        app.update();
        assert!(!intent(&app).active);
        assert_eq!(intent(&app).stop_sequence, 4);
    }
}
