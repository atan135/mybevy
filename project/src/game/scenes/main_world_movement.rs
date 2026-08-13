//! Client movement runtime boundary for the public main world.
//!
//! This module owns no scene entry policy. It provides the bounded,
//! generation-scoped input, fixed local-prediction, and presentation schedule
//! required before later authority correction and remote interpolation stages.

use std::{
    collections::{HashMap, VecDeque},
    time::Duration,
};

use bevy::{
    input::touch::{TouchInput, TouchPhase},
    prelude::*,
    time::Fixed,
    transform::TransformSystems,
    window::{AppLifecycle, PrimaryWindow, WindowFocused},
};

use crate::{
    framework::{
        fangyuan::{FangyuanObjectState, FangyuanPlayerPosition},
        scene::prelude::SceneSessionId,
        ui::core::{UiInputState, UiInputSystems},
    },
    game::{
        myserver::{
            GameConnectionState, MovementClientState, MyServerCommand, MyServerSession,
            MyServerUpdateSet,
        },
        scenes::{
            main_world_camera::{
                MainWorldCameraOrbitState, MainWorldTouchOwner, main_world_touch_owner,
            },
            main_world_contract::{
                MAIN_WORLD_AUTHORITY_TICK_SECONDS, MAIN_WORLD_AUTHORITY_TICKS_PER_SECOND,
                MAIN_WORLD_MOVE_SPEED_METRES_PER_SECOND, MAIN_WORLD_PUBLIC_ROOM_ID,
                MAIN_WORLD_SERVER_COORDINATE_MAX_EXCLUSIVE_METRES,
                MAIN_WORLD_WORLD_CENTRE_OFFSET_METRES, MainWorldAuthorityFrame,
                MainWorldConfirmedFrame, MainWorldMoveInputKind, MainWorldPredictedFrame,
                MainWorldRenderFrame, main_world_normalized_direction, main_world_server_position,
            },
            main_world_entry::{
                MainWorldEntryPhase, MainWorldEntryState, MainWorldEntryUpdateSet,
                MainWorldRoomMembership,
            },
            main_world_players::{MainWorldPlayer, MainWorldPlayerOwnership},
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

/// Outbound movement cadence and deduplication state. This is separate from
/// prediction so stage 4 can submit legal client state without claiming that
/// the rendered transform is authoritative.
#[derive(Resource)]
struct MainWorldMovementDispatchRuntime {
    timer: Timer,
    generation: u64,
    session_id: Option<SceneSessionId>,
    last_dispatched_frame: MainWorldAuthorityFrame,
    observed_stop_sequence: u64,
}

#[derive(Clone, Copy, Debug)]
struct MainWorldPendingPrediction {
    input: MainWorldUnconfirmedInput,
}

impl Default for MainWorldMovementDispatchRuntime {
    fn default() -> Self {
        Self {
            timer: Timer::new(
                Duration::from_secs(1) / MAIN_WORLD_AUTHORITY_TICKS_PER_SECOND,
                TimerMode::Repeating,
            ),
            generation: 0,
            session_id: None,
            last_dispatched_frame: MainWorldAuthorityFrame::default(),
            observed_stop_sequence: 0,
        }
    }
}

impl MainWorldMovementDispatchRuntime {
    fn bind(
        &mut self,
        entry: &MainWorldEntryState,
        movement: &MainWorldMovementRuntime,
        intent: &MainWorldMovementIntent,
    ) {
        if self.generation == movement.generation && self.session_id == movement.session_id {
            return;
        }

        self.timer.reset();
        self.generation = movement.generation;
        self.session_id = movement.session_id.clone();
        self.last_dispatched_frame =
            MainWorldAuthorityFrame(movement.predicted.frame.0.max(entry.snapshot_generation));
        self.observed_stop_sequence = intent.stop_sequence;
    }

    fn observe_closed_gate(&mut self, intent: &MainWorldMovementIntent) {
        self.timer.reset();
        self.observed_stop_sequence = intent.stop_sequence;
    }

    fn next_frame(
        &mut self,
        entry: &MainWorldEntryState,
        movement: &MainWorldMovementRuntime,
    ) -> MainWorldPredictedFrame {
        let baseline = self
            .last_dispatched_frame
            .0
            .max(entry.snapshot_generation)
            .max(movement.predicted.frame.0);
        let frame = baseline.wrapping_add(1).max(1);
        self.last_dispatched_frame = MainWorldAuthorityFrame(frame);
        MainWorldPredictedFrame(frame)
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
    pub confirmed: bool,
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
    predicted_previous: MainWorldPredictedState,
    pending_prediction: VecDeque<MainWorldPendingPrediction>,
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
            predicted_previous: MainWorldPredictedState::default(),
            pending_prediction: VecDeque::new(),
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

    pub fn bind_active_session(
        &mut self,
        generation: u64,
        session_id: SceneSessionId,
        authoritative_position: Option<Vec3>,
        authority_frame: MainWorldAuthorityFrame,
    ) {
        if self.generation != generation || self.session_id.as_ref() != Some(&session_id) {
            self.clear();
            self.generation = generation;
            self.session_id = Some(session_id);
            self.predicted.position = authoritative_position.unwrap_or_default();
            self.predicted.frame = MainWorldPredictedFrame(authority_frame.0);
            self.predicted_previous = self.predicted;
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
        self.predicted_previous = MainWorldPredictedState::default();
        self.pending_prediction.clear();
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

    fn queue_prediction(&mut self, input: MainWorldUnconfirmedInput) {
        self.pending_prediction
            .push_back(MainWorldPendingPrediction { input });
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
            .init_resource::<MainWorldMovementDispatchRuntime>()
            .insert_resource(Time::<Fixed>::from_hz(f64::from(
                MAIN_WORLD_AUTHORITY_TICKS_PER_SECOND,
            )))
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<TouchInput>()
            .add_message::<WindowFocused>()
            .add_message::<AppLifecycle>()
            .add_message::<MyServerCommand>()
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
                    dispatch_main_world_move_input
                        .in_set(MainWorldMovementUpdateSet::DispatchInput),
                ),
            )
            .add_systems(
                FixedUpdate,
                predict_main_world_movement_fixed.in_set(MainWorldMovementFixedSet::Predict),
            )
            .add_systems(
                PostUpdate,
                (
                    write_main_world_predicted_visual,
                    advance_main_world_render_frame,
                )
                    .chain()
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
        runtime.bind_active_session(
            entry.generation,
            session_id,
            entry.position,
            MainWorldAuthorityFrame(entry.snapshot_generation),
        );
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

/// Emits the existing binary `MoveInputReq` command at the frozen 20 Hz
/// authority cadence. Its frame-bound predicted state is queued for the next
/// fixed step; this Update system never mutates the prediction baseline.
fn dispatch_main_world_move_input(
    time: Res<Time>,
    entry: Option<Res<MainWorldEntryState>>,
    session: Option<Res<MyServerSession>>,
    intent: Res<MainWorldMovementIntent>,
    mut movement: ResMut<MainWorldMovementRuntime>,
    mut dispatch: ResMut<MainWorldMovementDispatchRuntime>,
    mut commands: MessageWriter<MyServerCommand>,
) {
    let Some(entry) = entry else {
        dispatch.observe_closed_gate(&intent);
        return;
    };
    let Some(session) = session else {
        dispatch.observe_closed_gate(&intent);
        return;
    };
    if !main_world_movement_send_gate(&entry, &movement, &session) {
        dispatch.observe_closed_gate(&intent);
        return;
    }

    dispatch.bind(&entry, &movement, &intent);
    if !dispatch.timer.tick(time.delta()).just_finished() {
        return;
    }

    let kind = if intent.active {
        MainWorldMoveInputKind::MoveDirection
    } else if intent.stop_sequence != dispatch.observed_stop_sequence {
        MainWorldMoveInputKind::MoveStop
    } else {
        return;
    };
    let direction = match kind {
        MainWorldMoveInputKind::MoveDirection => {
            let Ok(direction) = main_world_normalized_direction(intent.direction) else {
                return;
            };
            direction
        }
        MainWorldMoveInputKind::MoveStop => Vec2::ZERO,
    };
    let frame = dispatch.next_frame(&entry, &movement);
    let predicted_before = movement
        .pending_prediction
        .back()
        .map(|pending| pending.input.predicted_after)
        .unwrap_or(movement.predicted);
    let predicted_after = main_world_predicted_after_input(predicted_before, frame, direction);
    let Ok(server_position) = main_world_server_position(predicted_after.position) else {
        return;
    };
    let input_type = match kind {
        MainWorldMoveInputKind::MoveDirection => {
            crate::game::myserver::protocol::pb::MoveInputType::MoveDir
        }
        MainWorldMoveInputKind::MoveStop => {
            crate::game::myserver::protocol::pb::MoveInputType::MoveStop
        }
    };
    commands.write(MyServerCommand::SendMoveInput {
        frame_id: frame.0,
        input_type,
        dir_x: direction.x,
        dir_y: direction.y,
        client_state: Some(MovementClientState {
            x: server_position.x,
            y: server_position.y,
            frame_id: frame.0,
        }),
    });
    let input = MainWorldUnconfirmedInput {
        frame,
        direction,
        predicted_before,
        predicted_after,
        confirmed: false,
    };
    movement.push_unconfirmed_input(input);
    movement.queue_prediction(input);
    if kind == MainWorldMoveInputKind::MoveStop {
        dispatch.observed_stop_sequence = intent.stop_sequence;
    }
}

/// Validates the exact entry/session ownership boundary before a move request
/// can leave the client. A valid keyboard or touch intent alone is never
/// sufficient to send to a stale room, character, generation, or connection.
fn main_world_movement_send_gate(
    entry: &MainWorldEntryState,
    movement: &MainWorldMovementRuntime,
    session: &MyServerSession,
) -> bool {
    entry.allows_gameplay_input()
        && movement.allows_local_movement()
        && movement.generation == entry.generation
        && movement.session_id == entry.scene_session_id
        && entry.room_membership == MainWorldRoomMembership::Joined
        && entry.room_id.as_deref() == Some(MAIN_WORLD_PUBLIC_ROOM_ID)
        && session.room_id.as_deref() == Some(MAIN_WORLD_PUBLIC_ROOM_ID)
        && entry.character_id.is_some()
        && entry.character_id == session.character_id
        && session.connection_id.is_some()
        && session.connected
        && session.authenticated
        && session.game_connection_state == GameConnectionState::Authenticated
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

/// Advances exactly one sent authority input per fixed tick. Render frames can
/// never create prediction frames, and un-sent local intent never mutates the
/// predicted baseline.
fn predict_main_world_movement_fixed(
    entry: Option<Res<MainWorldEntryState>>,
    mut runtime: ResMut<MainWorldMovementRuntime>,
) {
    let Some(entry) = entry else {
        return;
    };
    if !entry.allows_gameplay_input()
        || runtime.generation != entry.generation
        || runtime.session_id != entry.scene_session_id
        || !runtime.allows_local_movement()
    {
        return;
    }
    let Some(pending) = runtime.pending_prediction.pop_front() else {
        return;
    };

    runtime.predicted_previous = runtime.predicted;
    runtime.predicted = pending.input.predicted_after;
}

fn main_world_predicted_after_input(
    predicted_before: MainWorldPredictedState,
    frame: MainWorldPredictedFrame,
    direction: Vec2,
) -> MainWorldPredictedState {
    let direction = main_world_normalized_direction(direction).unwrap_or(Vec2::ZERO);
    let moving = direction != Vec2::ZERO;
    let mut predicted_after = predicted_before;
    predicted_after.frame = frame;
    predicted_after.direction = direction;
    predicted_after.moving = moving;
    if moving {
        let step = MAIN_WORLD_MOVE_SPEED_METRES_PER_SECOND * MAIN_WORLD_AUTHORITY_TICK_SECONDS;
        predicted_after.position = main_world_clamped_predicted_position(
            predicted_before.position + Vec3::new(direction.x * step, 0.0, direction.y * step),
        );
    }
    predicted_after
}

fn main_world_clamped_predicted_position(position: Vec3) -> Vec3 {
    // Clamp in server-coordinate precision, then map back. `next_down(2000)`
    // can round back to 4000 when the centre offset is restored.
    let upper = f32::next_down(MAIN_WORLD_SERVER_COORDINATE_MAX_EXCLUSIVE_METRES)
        - MAIN_WORLD_WORLD_CENTRE_OFFSET_METRES;
    Vec3::new(
        position
            .x
            .clamp(-MAIN_WORLD_WORLD_CENTRE_OFFSET_METRES, upper),
        position.y,
        position
            .z
            .clamp(-MAIN_WORLD_WORLD_CENTRE_OFFSET_METRES, upper),
    )
}

/// Interpolates only the visual representation. The fixed-step predicted
/// baseline remains untouched so later authority replay is deterministic.
fn write_main_world_predicted_visual(
    fixed_time: Res<Time<Fixed>>,
    runtime: Res<MainWorldMovementRuntime>,
    mut local_players: Query<(
        &MainWorldPlayer,
        &mut FangyuanPlayerPosition,
        &mut FangyuanObjectState,
        &mut Transform,
    )>,
) {
    if !runtime.allows_local_movement() {
        return;
    }
    let visual_position = main_world_predicted_visual_position(
        runtime.predicted_previous.position,
        runtime.predicted.position,
        fixed_time.overstep_fraction(),
    );
    for (player, mut position, mut object_state, mut transform) in &mut local_players {
        if player.ownership == MainWorldPlayerOwnership::Local
            && Some(&player.scene_session_id) == runtime.session_id.as_ref()
        {
            position.translation = visual_position;
            object_state.root_translation = visual_position;
            transform.translation = visual_position;
        }
    }
}

fn main_world_predicted_visual_position(previous: Vec3, current: Vec3, alpha: f32) -> Vec3 {
    previous.lerp(current, alpha.clamp(0.0, 1.0))
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
    use crate::{framework::network::ConnectionId, game::myserver::protocol::pb};
    use bevy::ecs::message::{MessageCursor, Messages};
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    fn active_entry(generation: u64, session_id: &str) -> MainWorldEntryState {
        MainWorldEntryState {
            generation,
            phase: MainWorldEntryPhase::Active,
            scene_session_id: Some(SceneSessionId::from(session_id)),
            input_frozen: false,
            ..Default::default()
        }
    }

    fn networked_entry(generation: u64, session_id: &str) -> MainWorldEntryState {
        MainWorldEntryState {
            character_id: Some("chr-local".to_owned()),
            room_id: Some(MAIN_WORLD_PUBLIC_ROOM_ID.to_owned()),
            room_membership: MainWorldRoomMembership::Joined,
            position: Some(Vec3::new(1.25, 0.0, -2.5)),
            snapshot_generation: 41,
            ..active_entry(generation, session_id)
        }
    }

    fn connected_main_world_session() -> MyServerSession {
        MyServerSession {
            character_id: Some("chr-local".to_owned()),
            room_id: Some(MAIN_WORLD_PUBLIC_ROOM_ID.to_owned()),
            connection_id: Some(ConnectionId::from_raw(41)),
            connected: true,
            authenticated: true,
            game_connection_state: GameConnectionState::Authenticated,
            ..Default::default()
        }
    }

    fn movement_app(entry: MainWorldEntryState) -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(entry)
            .add_plugins(MainWorldMovementPlugin)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
        let window = app
            .world_mut()
            .spawn((PrimaryWindow, Window::default()))
            .id();
        (app, window)
    }

    fn networked_movement_app() -> (App, Entity) {
        let (mut app, window) = movement_app(networked_entry(7, "main-world-7"));
        app.insert_resource(connected_main_world_session());
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

    fn advance_update(app: &mut App, duration: Duration) {
        app.insert_resource(TimeUpdateStrategy::ManualDuration(duration));
        app.update();
        assert_eq!(app.world().resource::<Time>().delta(), duration);
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
    }

    fn read_new_move_commands(
        app: &App,
        cursor: &mut MessageCursor<MyServerCommand>,
    ) -> Vec<MyServerCommand> {
        cursor
            .read(app.world().resource::<Messages<MyServerCommand>>())
            .filter(|command| matches!(command, MyServerCommand::SendMoveInput { .. }))
            .cloned()
            .collect()
    }

    fn move_command_details(
        command: &MyServerCommand,
    ) -> (u32, pb::MoveInputType, f32, f32, MovementClientState) {
        match command {
            MyServerCommand::SendMoveInput {
                frame_id,
                input_type,
                dir_x,
                dir_y,
                client_state: Some(client_state),
            } => (*frame_id, *input_type, *dir_x, *dir_y, *client_state),
            unexpected => panic!("expected SendMoveInput with client state, got {unexpected:?}"),
        }
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
            confirmed: false,
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

    fn predicted_state(
        frame: u32,
        position: Vec3,
        direction: Vec2,
        moving: bool,
    ) -> MainWorldPredictedState {
        MainWorldPredictedState {
            frame: MainWorldPredictedFrame(frame),
            position,
            direction,
            moving,
        }
    }

    #[test]
    fn fixed_prediction_advances_exactly_four_metres_per_second_and_normalizes_diagonals() {
        let start = predicted_state(41, Vec3::ZERO, Vec2::ZERO, false);
        let forward = main_world_predicted_after_input(start, MainWorldPredictedFrame(42), Vec2::Y);
        assert_eq!(forward.position, Vec3::new(0.0, 0.0, 0.2));
        assert_eq!(forward.direction, Vec2::Y);
        assert!(forward.moving);

        let diagonal = main_world_predicted_after_input(
            start,
            MainWorldPredictedFrame(42),
            Vec2::new(1.0, 1.0),
        );
        assert!((diagonal.position.length() - 0.2).abs() < 0.000_01);
        assert_vec2_approx_eq(diagonal.direction, Vec2::new(1.0, 1.0).normalize());
    }

    #[test]
    fn fixed_prediction_preserves_exclusive_server_upper_boundary_and_converges_on_stop() {
        let upper = f32::next_down(MAIN_WORLD_SERVER_COORDINATE_MAX_EXCLUSIVE_METRES)
            - MAIN_WORLD_WORLD_CENTRE_OFFSET_METRES;
        let start = predicted_state(
            41,
            Vec3::new(upper - 0.05, 3.0, upper - 0.05),
            Vec2::X,
            true,
        );
        let bounded = main_world_predicted_after_input(
            start,
            MainWorldPredictedFrame(42),
            Vec2::new(1.0, 1.0),
        );
        assert_eq!(bounded.position.x, upper);
        assert_eq!(bounded.position.z, upper);
        assert_eq!(bounded.position.y, 3.0);
        assert!(main_world_server_position(bounded.position).is_ok());

        let stopped =
            main_world_predicted_after_input(bounded, MainWorldPredictedFrame(43), Vec2::ZERO);
        assert_eq!(stopped.position, bounded.position);
        assert_eq!(stopped.direction, Vec2::ZERO);
        assert!(!stopped.moving);
    }

    #[test]
    fn fixed_prediction_replay_is_deterministic_across_render_delta_partitions() {
        let sequence = [Vec2::Y, Vec2::new(1.0, 1.0), Vec2::ZERO, Vec2::X];
        let replay = |render_deltas: &[Duration]| {
            let mut state = predicted_state(41, Vec3::new(1.25, 0.0, -2.5), Vec2::ZERO, false);
            for (index, direction) in sequence.iter().copied().enumerate() {
                for _ in render_deltas {
                    // Render partitions do not enter fixed prediction state.
                }
                state = main_world_predicted_after_input(
                    state,
                    MainWorldPredictedFrame(42 + index as u32),
                    direction,
                );
            }
            state
        };
        assert_eq!(
            replay(&[Duration::from_millis(50)]),
            replay(&[Duration::from_millis(17), Duration::from_millis(33)])
        );
    }

    #[test]
    fn dispatched_frames_bind_prediction_history_and_advance_once_on_the_next_fixed_tick() {
        let (mut app, _) = networked_movement_app();
        app.update();
        press_key(&mut app, KeyCode::KeyW);
        app.update();

        advance_update(&mut app, Duration::from_millis(50));
        let runtime = app.world().resource::<MainWorldMovementRuntime>();
        assert_eq!(runtime.predicted.frame, MainWorldPredictedFrame(41));
        assert_eq!(runtime.unconfirmed_inputs.len(), 1);
        let input = runtime.unconfirmed_inputs.back().unwrap();
        assert_eq!(input.frame, MainWorldPredictedFrame(42));
        assert_eq!(input.predicted_before.position, Vec3::new(1.25, 0.0, -2.5));
        assert_eq!(input.predicted_after.position, Vec3::new(1.25, 0.0, -2.3));
        assert!(!input.confirmed);

        advance_update(&mut app, Duration::from_millis(50));
        let runtime = app.world().resource::<MainWorldMovementRuntime>();
        assert_eq!(runtime.predicted.frame, MainWorldPredictedFrame(42));
        assert_eq!(runtime.predicted.position, Vec3::new(1.25, 0.0, -2.3));
        assert_eq!(runtime.unconfirmed_inputs.len(), 2);
    }

    #[test]
    fn predicted_visual_is_local_only_and_never_writes_back_to_the_fixed_baseline() {
        let (mut app, _) = networked_movement_app();
        app.update();
        let session_id = SceneSessionId::from("main-world-7");
        let local = app
            .world_mut()
            .spawn((
                MainWorldPlayer {
                    character_id: "chr-local".to_owned(),
                    server_entity_id: 1,
                    ownership: MainWorldPlayerOwnership::Local,
                    scene_session_id: session_id.clone(),
                    last_authoritative_frame: 41,
                },
                FangyuanPlayerPosition::default(),
                FangyuanObjectState::default(),
                Transform::default(),
            ))
            .id();
        let remote = app
            .world_mut()
            .spawn((
                MainWorldPlayer {
                    character_id: "chr-remote".to_owned(),
                    server_entity_id: 2,
                    ownership: MainWorldPlayerOwnership::Remote,
                    scene_session_id: session_id,
                    last_authoritative_frame: 41,
                },
                FangyuanPlayerPosition {
                    translation: Vec3::splat(9.0),
                },
                FangyuanObjectState::from_translation(Vec3::splat(9.0)),
                Transform::from_translation(Vec3::splat(9.0)),
            ))
            .id();
        {
            let mut runtime = app.world_mut().resource_mut::<MainWorldMovementRuntime>();
            runtime.predicted_previous =
                predicted_state(41, Vec3::new(1.0, 0.0, 2.0), Vec2::Y, true);
            runtime.predicted = predicted_state(42, Vec3::new(1.0, 0.0, 2.2), Vec2::Y, true);
        }
        app.update();

        assert_eq!(
            app.world().get::<Transform>(local).unwrap().translation,
            Vec3::new(1.0, 0.0, 2.0)
        );
        assert_eq!(
            app.world().get::<Transform>(remote).unwrap().translation,
            Vec3::splat(9.0)
        );
        let runtime = app.world().resource::<MainWorldMovementRuntime>();
        assert_eq!(runtime.predicted.position, Vec3::new(1.0, 0.0, 2.2));
        assert_eq!(
            main_world_predicted_visual_position(Vec3::ZERO, Vec3::X, 0.5),
            Vec3::new(0.5, 0.0, 0.0)
        );
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

    #[test]
    fn move_dispatch_uses_20_hz_continuous_frames_and_server_coordinates() {
        let (mut app, _) = networked_movement_app();
        let mut cursor = MessageCursor::<MyServerCommand>::default();
        app.update();
        press_key(&mut app, KeyCode::KeyW);
        app.update();
        assert!(read_new_move_commands(&app, &mut cursor).is_empty());

        advance_update(&mut app, Duration::from_millis(49));
        assert!(read_new_move_commands(&app, &mut cursor).is_empty());
        advance_update(&mut app, Duration::from_millis(1));
        let commands = read_new_move_commands(&app, &mut cursor);
        assert_eq!(commands.len(), 1);
        let (frame, input_type, dir_x, dir_y, client_state) = move_command_details(&commands[0]);
        assert_eq!(frame, 42);
        assert_eq!(input_type, pb::MoveInputType::MoveDir);
        assert_eq!(Vec2::new(dir_x, dir_y), Vec2::Y);
        assert_eq!(client_state.frame_id, frame);
        assert_eq!(
            Vec2::new(client_state.x, client_state.y),
            Vec2::new(2001.25, 1997.7)
        );

        press_key(&mut app, KeyCode::KeyD);
        app.update();
        advance_update(&mut app, Duration::from_millis(50));
        let commands = read_new_move_commands(&app, &mut cursor);
        assert_eq!(commands.len(), 1);
        let (frame, input_type, dir_x, dir_y, client_state) = move_command_details(&commands[0]);
        assert_eq!(frame, 43);
        assert_eq!(input_type, pb::MoveInputType::MoveDir);
        assert_vec2_approx_eq(Vec2::new(dir_x, dir_y), Vec2::new(1.0, 1.0).normalize());
        assert_eq!(client_state.frame_id, frame);
    }

    #[test]
    fn move_stop_is_deferred_to_one_authority_tick_and_not_repeated_while_idle() {
        let (mut app, _) = networked_movement_app();
        let mut cursor = MessageCursor::<MyServerCommand>::default();
        app.update();
        press_key(&mut app, KeyCode::KeyW);
        app.update();
        advance_update(&mut app, Duration::from_millis(50));
        let start = read_new_move_commands(&app, &mut cursor);
        assert_eq!(start.len(), 1);
        assert_eq!(
            move_command_details(&start[0]).1,
            pb::MoveInputType::MoveDir
        );

        release_key(&mut app, KeyCode::KeyW);
        app.update();
        assert!(read_new_move_commands(&app, &mut cursor).is_empty());
        advance_update(&mut app, Duration::from_millis(50));
        let stop = read_new_move_commands(&app, &mut cursor);
        assert_eq!(stop.len(), 1);
        let (frame, input_type, dir_x, dir_y, client_state) = move_command_details(&stop[0]);
        assert_eq!(frame, 43);
        assert_eq!(input_type, pb::MoveInputType::MoveStop);
        assert_eq!(Vec2::new(dir_x, dir_y), Vec2::ZERO);
        assert_eq!(client_state.frame_id, frame);

        advance_update(&mut app, Duration::from_millis(200));
        assert!(read_new_move_commands(&app, &mut cursor).is_empty());
    }

    #[test]
    fn move_dispatch_rejects_stale_entry_room_character_and_connection_states() {
        let entry = networked_entry(7, "main-world-7");
        let mut runtime = MainWorldMovementRuntime::default();
        runtime.bind_active_session(
            entry.generation,
            entry.scene_session_id.clone().unwrap(),
            entry.position,
            MainWorldAuthorityFrame(entry.snapshot_generation),
        );
        let session = connected_main_world_session();
        assert!(main_world_movement_send_gate(&entry, &runtime, &session));

        let mut frozen_entry = entry.clone();
        frozen_entry.input_frozen = true;
        assert!(!main_world_movement_send_gate(
            &frozen_entry,
            &runtime,
            &session
        ));

        let mut wrong_room = entry.clone();
        wrong_room.room_id = Some("another-room".to_owned());
        assert!(!main_world_movement_send_gate(
            &wrong_room,
            &runtime,
            &session
        ));

        let mut stale_generation = entry.clone();
        stale_generation.generation += 1;
        assert!(!main_world_movement_send_gate(
            &stale_generation,
            &runtime,
            &session
        ));

        let mut wrong_character = connected_main_world_session();
        wrong_character.character_id = Some("chr-other".to_owned());
        assert!(!main_world_movement_send_gate(
            &entry,
            &runtime,
            &wrong_character
        ));

        let mut unavailable = connected_main_world_session();
        unavailable.connected = false;
        assert!(!main_world_movement_send_gate(
            &entry,
            &runtime,
            &unavailable
        ));
    }

    #[test]
    fn connection_loss_sends_nothing_resets_the_timer_and_preserves_next_frame_order() {
        let (mut app, _) = networked_movement_app();
        let mut cursor = MessageCursor::<MyServerCommand>::default();
        app.update();
        press_key(&mut app, KeyCode::KeyW);
        app.update();
        app.world_mut().resource_mut::<MyServerSession>().connected = false;
        advance_update(&mut app, Duration::from_millis(80));
        assert!(read_new_move_commands(&app, &mut cursor).is_empty());

        app.world_mut().resource_mut::<MyServerSession>().connected = true;
        advance_update(&mut app, Duration::from_millis(49));
        assert!(read_new_move_commands(&app, &mut cursor).is_empty());
        advance_update(&mut app, Duration::from_millis(1));
        let commands = read_new_move_commands(&app, &mut cursor);
        assert_eq!(commands.len(), 1);
        let (frame, input_type, _, _, client_state) = move_command_details(&commands[0]);
        assert_eq!(frame, 42);
        assert_eq!(input_type, pb::MoveInputType::MoveDir);
        assert_eq!(client_state.frame_id, frame);
    }
}
