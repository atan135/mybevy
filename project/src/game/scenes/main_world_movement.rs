//! Client movement runtime boundary for the public main world.
//!
//! This module owns no scene entry policy. It provides the bounded,
//! generation-scoped input, prediction, authority-correction, and presentation
//! schedule required before the later remote-interpolation stage.

use std::{
    collections::{HashMap, VecDeque},
    time::{Duration, Instant},
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
            GameConnectionState, MovementClientState, MyServerCommand, MyServerEvent,
            MyServerSession, MyServerUpdateSet, protocol::pb,
        },
        scenes::{
            main_world_camera::{
                MainWorldCameraOrbitState, MainWorldTouchOwner, main_world_touch_owner,
            },
            main_world_contract::{
                MAIN_WORLD_AUTHORITY_CONTRACT, MAIN_WORLD_AUTHORITY_TICK_SECONDS,
                MAIN_WORLD_AUTHORITY_TICKS_PER_SECOND, MAIN_WORLD_MOVE_SPEED_METRES_PER_SECOND,
                MAIN_WORLD_PUBLIC_ROOM_ID, MAIN_WORLD_SERVER_COORDINATE_MAX_EXCLUSIVE_METRES,
                MAIN_WORLD_WORLD_CENTRE_OFFSET_METRES, MainWorldAuthorityFrame,
                MainWorldConfirmedFrame, MainWorldMoveInputKind, MainWorldPredictedFrame,
                MainWorldRenderFrame, main_world_bevy_position_from_authority,
                main_world_movement_snapshot_contains_complete_room_entities,
                main_world_movement_snapshot_from_event, main_world_normalized_direction,
                main_world_server_position,
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
/// Keep one normal movement correction interval buffered so remote rendering
/// can advance continuously between the server's three-frame samples.
pub(in crate::game) const MAIN_WORLD_REMOTE_INTERPOLATION_DELAY_FRAMES: u32 = 3;
pub(in crate::game) const MAIN_WORLD_REMOTE_MAX_SNAP_DISTANCE_METRES: f32 = 8.0;
/// The room accepts inputs up to this many frames ahead of its current clock.
/// Keep outbound input at the edge of that window so network latency cannot
/// make a frame expire before the server receives it.
pub(in crate::game) const MAIN_WORLD_INPUT_LEAD_FRAMES: u32 = 2;

/// Incremental authority differences no larger than this are eased only in
/// presentation. Larger differences replace the local baseline immediately.
pub(in crate::game) const MAIN_WORLD_SMALL_CORRECTION_DISTANCE_METRES: f32 = 0.5;

/// Small authority corrections decay over a short, deterministic presentation
/// interval while the fixed prediction baseline has already been corrected.
pub(in crate::game) const MAIN_WORLD_SMALL_CORRECTION_SECONDS: f32 = 0.10;

/// Lightweight development telemetry for the client movement pipeline. Values
/// are wall-clock observations, not frame-budget thresholds or authority data.
#[derive(Debug, Default, Resource)]
pub(in crate::game) struct MainWorldMovementDiagnostics {
    pub update_pipeline_last: Duration,
    pub fixed_prediction_last: Duration,
    pub presentation_pipeline_last: Duration,
    pub update_pipeline_samples: u64,
    pub fixed_prediction_samples: u64,
    pub presentation_pipeline_samples: u64,
    update_started_at: Option<Instant>,
    presentation_started_at: Option<Instant>,
}

impl MainWorldMovementDiagnostics {
    fn begin_update_pipeline(&mut self) {
        self.update_started_at = Some(Instant::now());
    }

    fn finish_update_pipeline(&mut self) {
        let Some(started_at) = self.update_started_at.take() else {
            return;
        };
        self.update_pipeline_last = started_at.elapsed();
        self.update_pipeline_samples = self.update_pipeline_samples.wrapping_add(1);
    }

    fn begin_presentation_pipeline(&mut self) {
        self.presentation_started_at = Some(Instant::now());
    }

    fn finish_presentation_pipeline(&mut self) {
        let Some(started_at) = self.presentation_started_at.take() else {
            return;
        };
        self.presentation_pipeline_last = started_at.elapsed();
        self.presentation_pipeline_samples = self.presentation_pipeline_samples.wrapping_add(1);
    }

    fn record_fixed_prediction(&mut self, started_at: Instant) {
        self.fixed_prediction_last = started_at.elapsed();
        self.fixed_prediction_samples = self.fixed_prediction_samples.wrapping_add(1);
    }
}

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
    room_clock_epoch: u64,
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
            room_clock_epoch: 0,
            last_dispatched_frame: MainWorldAuthorityFrame::default(),
            observed_stop_sequence: 0,
        }
    }
}

impl MainWorldMovementDispatchRuntime {
    fn bind(&mut self, movement: &MainWorldMovementRuntime, intent: &MainWorldMovementIntent) {
        if self.generation == movement.generation
            && self.session_id == movement.session_id
            && self.room_clock_epoch == movement.room_clock_epoch
        {
            return;
        }

        self.timer.reset();
        self.generation = movement.generation;
        self.session_id = movement.session_id.clone();
        self.room_clock_epoch = movement.room_clock_epoch;
        self.last_dispatched_frame = movement.latest_room_frame;
        self.observed_stop_sequence = intent.stop_sequence;
    }

    fn observe_closed_gate(&mut self, intent: &MainWorldMovementIntent) {
        self.timer.reset();
        self.observed_stop_sequence = intent.stop_sequence;
    }

    fn next_frame(
        &mut self,
        movement: &MainWorldMovementRuntime,
    ) -> Option<MainWorldPredictedFrame> {
        let room_frame = movement.latest_room_frame.0;
        if room_frame == 0 {
            return None;
        }
        let baseline = self.last_dispatched_frame.0.max(room_frame);
        let next_sequential = baseline.wrapping_add(1).max(1);
        let window_end = room_frame.saturating_add(MAIN_WORLD_INPUT_LEAD_FRAMES);
        let frame = next_sequential.max(window_end);
        if frame > window_end {
            return None;
        }
        self.last_dispatched_frame = MainWorldAuthorityFrame(frame);
        Some(MainWorldPredictedFrame(frame))
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MainWorldAuthorityCorrection {
    Smoothed,
    Rebased,
}

#[derive(Clone, Copy, Debug)]
struct MainWorldLocalAuthoritySnapshot {
    frame: MainWorldAuthorityFrame,
    confirmed_frame: MainWorldConfirmedFrame,
    server_entity_id: u64,
    scene_id: i32,
    position: Vec3,
    direction: Vec2,
    moving: bool,
    force_rebase: bool,
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
    entity_id: Option<u64>,
    scene_id: Option<i32>,
    presentation_frame: Option<f32>,
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

    fn reset_baseline(&mut self) {
        self.snapshots.clear();
        self.presentation_frame = None;
    }

    fn set_identity(&mut self, entity_id: u64, scene_id: i32) -> bool {
        let changed = self.entity_id.is_some_and(|id| id != entity_id)
            || self.scene_id.is_some_and(|id| id != scene_id);
        if changed {
            self.reset_baseline();
        }
        self.entity_id = Some(entity_id);
        self.scene_id = Some(scene_id);
        changed
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

        let previous_latest = self.snapshots.back().copied();
        if let Some(previous_latest) = previous_latest
            && snapshot.frame > previous_latest.frame
            && !previous_latest.moving
            && snapshot.moving
        {
            let delayed_frame = snapshot
                .frame
                .0
                .saturating_sub(MAIN_WORLD_REMOTE_INTERPOLATION_DELAY_FRAMES);
            if delayed_frame > previous_latest.frame.0 {
                self.insert_snapshot(MainWorldRemoteSnapshot {
                    frame: MainWorldAuthorityFrame(delayed_frame),
                    ..previous_latest
                });
            }
            let current = self
                .presentation_frame
                .unwrap_or(previous_latest.frame.0 as f32);
            self.presentation_frame = Some(current.max(delayed_frame as f32));
        }

        self.insert_snapshot(snapshot);
        if self.presentation_frame.is_none() {
            self.presentation_frame = Some(snapshot.frame.0 as f32);
        }
        true
    }

    fn insert_snapshot(&mut self, snapshot: MainWorldRemoteSnapshot) {
        let insert_at = self
            .snapshots
            .iter()
            .position(|existing| existing.frame > snapshot.frame)
            .unwrap_or(self.snapshots.len());
        self.snapshots.insert(insert_at, snapshot);
        if self.snapshots.len() > MAIN_WORLD_REMOTE_INTERPOLATION_CAPACITY {
            self.snapshots.pop_front();
        }
    }

    fn advance_presentation_frame(&mut self, delta_seconds: f32) -> Option<f32> {
        let oldest = self.snapshots.front()?.frame.0 as f32;
        let latest = self.snapshots.back()?;
        let target = if latest.moving {
            latest
                .frame
                .0
                .saturating_sub(MAIN_WORLD_REMOTE_INTERPOLATION_DELAY_FRAMES)
        } else {
            latest.frame.0
        } as f32;
        let current = self
            .presentation_frame
            .unwrap_or(latest.frame.0 as f32)
            .max(oldest);
        let advance = delta_seconds.max(0.0) * MAIN_WORLD_AUTHORITY_TICKS_PER_SECOND as f32;
        let next = if target > current {
            (current + advance).min(target)
        } else {
            current
        };
        self.presentation_frame = Some(next);
        Some(next)
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
    last_applied_authority_frame: Option<MainWorldAuthorityFrame>,
    latest_room_frame: MainWorldAuthorityFrame,
    room_clock_epoch: u64,
    local_authority_entity_id: Option<u64>,
    local_authority_scene_id: Option<i32>,
    visual_correction_offset: Vec3,
    visual_correction_remaining_seconds: f32,
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
            last_applied_authority_frame: None,
            latest_room_frame: MainWorldAuthorityFrame::default(),
            room_clock_epoch: 0,
            local_authority_entity_id: None,
            local_authority_scene_id: None,
            visual_correction_offset: Vec3::ZERO,
            visual_correction_remaining_seconds: 0.0,
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
            self.authority_baseline = Some(MainWorldAuthorityBaseline {
                frame: authority_frame,
                position: self.predicted.position,
                ..default()
            });
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
        self.last_applied_authority_frame = None;
        self.latest_room_frame = MainWorldAuthorityFrame::default();
        self.room_clock_epoch = 0;
        self.local_authority_entity_id = None;
        self.local_authority_scene_id = None;
        self.visual_correction_offset = Vec3::ZERO;
        self.visual_correction_remaining_seconds = 0.0;
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

    fn visual_position(&self, alpha: f32) -> Vec3 {
        main_world_predicted_visual_position(
            self.predicted_previous.position,
            self.predicted.position,
            alpha,
        ) + self.visual_correction_offset
    }

    fn set_small_visual_correction(&mut self, previous_visual: Vec3, alpha: f32) {
        let corrected_visual = main_world_predicted_visual_position(
            self.predicted_previous.position,
            self.predicted.position,
            alpha,
        );
        self.visual_correction_offset = previous_visual - corrected_visual;
        self.visual_correction_remaining_seconds = MAIN_WORLD_SMALL_CORRECTION_SECONDS;
    }

    fn clear_visual_correction(&mut self) {
        self.visual_correction_offset = Vec3::ZERO;
        self.visual_correction_remaining_seconds = 0.0;
    }

    fn advance_visual_correction(&mut self, delta_seconds: f32) {
        if self.visual_correction_remaining_seconds <= 0.0 {
            self.clear_visual_correction();
            return;
        }
        let remaining = self.visual_correction_remaining_seconds;
        self.visual_correction_remaining_seconds = (remaining - delta_seconds.max(0.0)).max(0.0);
        self.visual_correction_offset *= self.visual_correction_remaining_seconds / remaining;
        if self.visual_correction_remaining_seconds == 0.0 {
            self.visual_correction_offset = Vec3::ZERO;
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
            .init_resource::<MainWorldMovementDispatchRuntime>()
            .init_resource::<MainWorldMovementDiagnostics>()
            .insert_resource(Time::<Fixed>::from_hz(f64::from(
                MAIN_WORLD_AUTHORITY_TICKS_PER_SECOND,
            )))
            .init_resource::<ButtonInput<KeyCode>>()
            .add_message::<TouchInput>()
            .add_message::<WindowFocused>()
            .add_message::<AppLifecycle>()
            .add_message::<MyServerCommand>()
            .add_message::<MyServerEvent>()
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
                    (
                        sync_main_world_movement_lifecycle,
                        consume_main_world_movement_rejects,
                        consume_main_world_local_authority_snapshots,
                        consume_main_world_remote_authority_snapshots,
                    )
                        .chain()
                        .in_set(MainWorldMovementUpdateSet::ConsumeAuthority),
                    collect_main_world_movement_intent
                        .after(UiInputSystems::Update)
                        .in_set(MainWorldMovementUpdateSet::CollectIntent),
                    dispatch_main_world_move_input
                        .in_set(MainWorldMovementUpdateSet::DispatchInput),
                ),
            )
            .add_systems(
                Update,
                (
                    begin_main_world_movement_update_diagnostics
                        .before(MainWorldMovementUpdateSet::ConsumeAuthority),
                    finish_main_world_movement_update_diagnostics
                        .after(MainWorldMovementUpdateSet::DispatchInput),
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
                    write_main_world_remote_visual,
                    advance_main_world_render_frame,
                )
                    .chain()
                    .in_set(MainWorldMovementPostUpdateSet::WriteTransforms),
            );
        app.add_systems(
            PostUpdate,
            (
                begin_main_world_movement_presentation_diagnostics
                    .before(MainWorldMovementPostUpdateSet::WriteTransforms),
                finish_main_world_movement_presentation_diagnostics
                    .after(MainWorldMovementPostUpdateSet::WriteTransforms),
            ),
        );
    }
}

fn begin_main_world_movement_update_diagnostics(
    mut diagnostics: ResMut<MainWorldMovementDiagnostics>,
) {
    diagnostics.begin_update_pipeline();
}

fn finish_main_world_movement_update_diagnostics(
    mut diagnostics: ResMut<MainWorldMovementDiagnostics>,
) {
    diagnostics.finish_update_pipeline();
}

fn begin_main_world_movement_presentation_diagnostics(
    mut diagnostics: ResMut<MainWorldMovementDiagnostics>,
) {
    diagnostics.begin_presentation_pipeline();
}

fn finish_main_world_movement_presentation_diagnostics(
    mut diagnostics: ResMut<MainWorldMovementDiagnostics>,
) {
    diagnostics.finish_presentation_pipeline();
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

    if entry.phase == MainWorldEntryPhase::Recovering && entry.reconnect_requested {
        if runtime.generation != entry.generation
            || runtime.session_id.as_ref() != Some(&session_id)
        {
            runtime.clear();
            runtime.generation = entry.generation;
            runtime.session_id = Some(session_id);
            runtime.predicted.position = entry.position.unwrap_or_default();
            runtime.predicted.frame = MainWorldPredictedFrame(entry.snapshot_generation);
            runtime.predicted_previous = runtime.predicted;
        }
        intent.request_stop();
        runtime.freeze();
        input_runtime.reset_all();
        return;
    }

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

/// Consumes only the local character from the shared snapshot stream. The
/// player registry has its own reader for presentation; this reader performs
/// the prediction baseline/correction work and never touches remote players.
fn consume_main_world_local_authority_snapshots(
    mut events: MessageReader<MyServerEvent>,
    fixed_time: Res<Time<Fixed>>,
    entry: Option<Res<MainWorldEntryState>>,
    mut runtime: ResMut<MainWorldMovementRuntime>,
    mut dispatch: ResMut<MainWorldMovementDispatchRuntime>,
) {
    let Some(entry) = entry else {
        return;
    };
    let Some(character_id) = entry.character_id.as_deref() else {
        return;
    };
    let Some(session_id) = entry.scene_session_id.as_ref() else {
        return;
    };
    if !(entry.allows_gameplay_input()
        || (entry.reconnect_requested
            && matches!(
                entry.phase,
                MainWorldEntryPhase::Recovering
                    | MainWorldEntryPhase::WaitingSceneReady
                    | MainWorldEntryPhase::Active
            )))
        || runtime.generation != entry.generation
        || runtime.session_id.as_ref() != Some(session_id)
    {
        return;
    }

    let alpha = fixed_time.overstep_fraction();
    for event in events.read() {
        if let MyServerEvent::FrameBundlePush(push) = event {
            if push.frame_id < runtime.latest_room_frame.0 {
                if push.frame_id <= 1 {
                    runtime.room_clock_epoch = runtime.room_clock_epoch.wrapping_add(1);
                    runtime.pending_prediction.clear();
                    runtime.unconfirmed_inputs.clear();
                    runtime.last_applied_authority_frame = None;
                    runtime.authority_baseline = None;
                    runtime.remote_interpolation.clear();
                    dispatch.last_dispatched_frame = MainWorldAuthorityFrame::default();
                    runtime.latest_room_frame = MainWorldAuthorityFrame(push.frame_id);
                } else {
                    continue;
                }
            } else {
                runtime.latest_room_frame = MainWorldAuthorityFrame(push.frame_id);
            }
        }
        let Some(push) = main_world_movement_snapshot_from_event(event) else {
            continue;
        };
        if push.room_id != MAIN_WORLD_PUBLIC_ROOM_ID
            || (!push.target_character_ids.is_empty()
                && !push
                    .target_character_ids
                    .iter()
                    .any(|id| id == character_id))
        {
            continue;
        }
        let Some(entity) = push
            .entities
            .iter()
            .find(|entity| entity.character_id == character_id)
        else {
            continue;
        };
        let Ok(position) =
            main_world_bevy_position_from_authority(entity.scene_id, entity.x, entity.y)
        else {
            continue;
        };
        if runtime
            .last_applied_authority_frame
            .is_some_and(|frame| push.frame_id <= frame.0)
        {
            continue;
        }
        let direction = Vec2::new(entity.dir_x, entity.dir_y);
        let direction = if direction == Vec2::ZERO {
            Vec2::ZERO
        } else {
            let Ok(direction) = main_world_normalized_direction(direction) else {
                continue;
            };
            direction
        };
        let authority = MainWorldLocalAuthoritySnapshot {
            frame: MainWorldAuthorityFrame(push.frame_id),
            confirmed_frame: MainWorldConfirmedFrame(entity.last_input_frame),
            server_entity_id: entity.entity_id,
            scene_id: entity.scene_id,
            position,
            direction,
            moving: entity.moving,
            force_rebase: push.full_sync
                || matches!(
                    pb_correction_kind(push.correction_kind),
                    Some(pb::MovementCorrectionKind::FullSync)
                        | Some(pb::MovementCorrectionKind::Strong)
                        | Some(pb::MovementCorrectionKind::Recovery)
                )
                || entry.reconnect_requested,
        };
        reconcile_main_world_local_authority(&mut runtime, authority, alpha);
    }
}

/// Applies a server rejection only to the currently bound local character.
/// Rejects are intentionally stricter than broadcast snapshots because a
/// stale reject must never rewind a newer prediction generation.
fn consume_main_world_movement_rejects(
    mut events: MessageReader<MyServerEvent>,
    fixed_time: Res<Time<Fixed>>,
    entry: Option<Res<MainWorldEntryState>>,
    mut intent: ResMut<MainWorldMovementIntent>,
    mut runtime: ResMut<MainWorldMovementRuntime>,
    mut dispatch: ResMut<MainWorldMovementDispatchRuntime>,
) {
    let Some(entry) = entry else {
        return;
    };
    let Some(character_id) = entry.character_id.as_deref() else {
        return;
    };
    let Some(session_id) = entry.scene_session_id.as_ref() else {
        return;
    };
    if !(entry.allows_gameplay_input()
        || (entry.reconnect_requested
            && matches!(
                entry.phase,
                MainWorldEntryPhase::WaitingSceneReady | MainWorldEntryPhase::Active
            )))
        || runtime.generation != entry.generation
        || runtime.session_id.as_ref() != Some(session_id)
    {
        return;
    }

    for event in events.read() {
        let MyServerEvent::MovementRejectPush(reject) = event else {
            continue;
        };
        if reject.room_id != MAIN_WORLD_PUBLIC_ROOM_ID || reject.character_id != character_id {
            continue;
        }
        if reject.frame_id != 0
            && runtime
                .last_applied_authority_frame
                .is_some_and(|frame| reject.frame_id < frame.0)
        {
            continue;
        }
        let reference_frame = reject.reference_frame_id;
        let known_frame = runtime.predicted.frame.0.max(
            runtime
                .unconfirmed_inputs
                .back()
                .map_or(0, |input| input.frame.0),
        );
        if reference_frame != 0
            && reference_frame > known_frame
            && runtime
                .last_applied_authority_frame
                .is_none_or(|frame| reference_frame > frame.0)
        {
            continue;
        }
        let Some(corrected) = reject.corrected.as_ref() else {
            continue;
        };
        if corrected.character_id != character_id
            || !MAIN_WORLD_AUTHORITY_CONTRACT.is_authoritative_entity_scene(corrected.scene_id)
        {
            continue;
        }
        let Ok(position) =
            main_world_bevy_position_from_authority(corrected.scene_id, corrected.x, corrected.y)
        else {
            continue;
        };
        let raw_direction = Vec2::new(corrected.dir_x, corrected.dir_y);
        let direction_valid =
            raw_direction.is_finite() && raw_direction.length_squared() > f32::EPSILON;
        let direction = if direction_valid {
            raw_direction.normalize()
        } else {
            Vec2::ZERO
        };
        let error_code = reject.error_code.to_ascii_uppercase();
        let timing_reject = matches!(
            error_code.as_str(),
            "INPUT_FRAME_EXPIRED" | "INPUT_FRAME_TOO_FAR"
        );
        // A rejected frame was never simulated by the server. A client-state
        // reference is diagnostic only and must not advance confirmation.
        let confirmed_frame = MainWorldConfirmedFrame(corrected.last_input_frame);
        if timing_reject {
            runtime.unconfirmed_inputs.retain(|input| {
                Some(input.frame.0) != (reference_frame != 0).then_some(reference_frame)
            });
            runtime.pending_prediction.retain(|pending| {
                Some(pending.input.frame.0) != (reference_frame != 0).then_some(reference_frame)
            });
            dispatch.last_dispatched_frame = MainWorldAuthorityFrame(
                runtime.latest_room_frame.0.max(corrected.last_input_frame),
            );
        }
        reconcile_main_world_local_authority(
            &mut runtime,
            MainWorldLocalAuthoritySnapshot {
                frame: MainWorldAuthorityFrame(reject.frame_id),
                confirmed_frame,
                server_entity_id: corrected.entity_id,
                scene_id: corrected.scene_id,
                position,
                direction,
                moving: corrected.moving && direction_valid,
                force_rebase: true,
            },
            fixed_time.overstep_fraction(),
        );

        let reason = pb::MovementCorrectionReason::try_from(reject.reason_code).ok();
        let stop_prediction = (!timing_reject && !direction_valid)
            || matches!(
                reason,
                Some(
                    pb::MovementCorrectionReason::CollisionBlocked
                        | pb::MovementCorrectionReason::ControlTimeout
                        | pb::MovementCorrectionReason::MovementRejected
                )
            )
            || [
                "INVALID_DIRECTION",
                "OUT_OF_BOUNDS",
                "BOUNDARY",
                "COLLISION",
                "TIMEOUT",
            ]
            .iter()
            .any(|marker| error_code.contains(marker));
        if stop_prediction {
            runtime.unconfirmed_inputs.clear();
            runtime.pending_prediction.clear();
            runtime.predicted = MainWorldPredictedState {
                frame: MainWorldPredictedFrame(confirmed_frame.0),
                position,
                direction,
                moving: false,
            };
            runtime.predicted_previous = runtime.predicted;
            intent.request_stop();
        }
        debug!(
            room_id = %reject.room_id,
            character_id = %reject.character_id,
            frame_id = reject.frame_id,
            reference_frame_id = reject.reference_frame_id,
            reason_code = reject.reason_code,
            "main world movement reject applied"
        );
    }
}

fn consume_main_world_remote_authority_snapshots(
    mut events: MessageReader<MyServerEvent>,
    entry: Option<Res<MainWorldEntryState>>,
    mut runtime: ResMut<MainWorldMovementRuntime>,
) {
    let Some(entry) = entry else {
        return;
    };
    let Some(session_id) = entry.scene_session_id.as_ref() else {
        return;
    };
    let Some(local_character_id) = entry.character_id.as_deref() else {
        return;
    };
    if !(entry.allows_gameplay_input()
        || (entry.reconnect_requested
            && matches!(
                entry.phase,
                MainWorldEntryPhase::Recovering
                    | MainWorldEntryPhase::WaitingSceneReady
                    | MainWorldEntryPhase::Active
            )))
        || runtime.generation != entry.generation
        || runtime.session_id.as_ref() != Some(session_id)
    {
        return;
    }
    for event in events.read() {
        let Some(push) = main_world_movement_snapshot_from_event(event) else {
            continue;
        };
        if push.room_id != MAIN_WORLD_PUBLIC_ROOM_ID {
            continue;
        }
        // target_character_ids identifies snapshot recipients, not the entities
        // visible inside the snapshot. A recovery push for a newly joined
        // client can contain every room entity while targeting only that client.
        if !push.target_character_ids.is_empty()
            && !push
                .target_character_ids
                .iter()
                .any(|id| id == local_character_id)
        {
            continue;
        }
        let recovery_sync = push.full_sync
            || matches!(
                pb_correction_kind(push.correction_kind),
                Some(pb::MovementCorrectionKind::Recovery)
            );
        let complete_room_entities =
            main_world_movement_snapshot_contains_complete_room_entities(&push);
        if recovery_sync {
            runtime.remote_interpolation.clear();
        }
        for entity in &push.entities {
            if entity.character_id == local_character_id {
                continue;
            }
            let Ok(position) =
                main_world_bevy_position_from_authority(entity.scene_id, entity.x, entity.y)
            else {
                continue;
            };
            let buffer = runtime.remote_buffer_mut(entity.character_id.clone());
            let identity_changed = buffer.set_identity(entity.entity_id, entity.scene_id);
            let previous = buffer.snapshots().back().copied();
            if push.full_sync
                || matches!(
                    pb_correction_kind(push.correction_kind),
                    Some(
                        pb::MovementCorrectionKind::FullSync
                            | pb::MovementCorrectionKind::Strong
                            | pb::MovementCorrectionKind::Recovery
                    )
                )
                || identity_changed
                || previous.is_some_and(|sample| {
                    sample.position.distance(position) > MAIN_WORLD_REMOTE_MAX_SNAP_DISTANCE_METRES
                })
            {
                buffer.reset_baseline();
            }
            let direction = Vec2::new(entity.dir_x, entity.dir_y);
            let direction = if direction.is_finite() && direction.length_squared() > f32::EPSILON {
                direction.normalize()
            } else {
                previous.map(|sample| sample.direction).unwrap_or(Vec2::X)
            };
            buffer.push(MainWorldRemoteSnapshot {
                frame: MainWorldAuthorityFrame(push.frame_id),
                position,
                direction,
                moving: entity.moving,
            });
        }
        if push.full_sync || complete_room_entities {
            let visible: std::collections::HashSet<_> = push
                .entities
                .iter()
                .map(|e| e.character_id.as_str())
                .collect();
            runtime
                .remote_interpolation
                .retain(|id, _| visible.contains(id.as_str()) && id != local_character_id);
        }
    }
}

fn write_main_world_remote_visual(
    time: Res<Time>,
    mut runtime: ResMut<MainWorldMovementRuntime>,
    mut players: Query<(
        &MainWorldPlayer,
        &mut FangyuanPlayerPosition,
        &mut FangyuanObjectState,
        &mut Transform,
    )>,
) {
    if !runtime.allows_local_movement() {
        return;
    }
    for (player, mut position, mut object_state, mut transform) in &mut players {
        if player.ownership != MainWorldPlayerOwnership::Remote
            || Some(&player.scene_session_id) != runtime.session_id.as_ref()
        {
            continue;
        }
        let Some(buffer) = runtime.remote_interpolation.get_mut(&player.character_id) else {
            continue;
        };
        let Some(presentation_frame) = buffer.advance_presentation_frame(time.delta_secs()) else {
            continue;
        };
        let (a, b, factor) = remote_interpolation_sample(buffer.snapshots(), presentation_frame);
        let visual = a.position.lerp(b.position, factor);
        position.translation = visual;
        object_state.root_translation = visual;
        transform.translation = visual;
        let direction = a.direction.lerp(b.direction, factor).normalize_or_zero();
        if direction.length_squared() > f32::EPSILON {
            transform.rotation = Quat::from_rotation_y(direction.x.atan2(direction.y));
        }
    }
}

fn remote_interpolation_sample(
    snapshots: &VecDeque<MainWorldRemoteSnapshot>,
    target_frame: f32,
) -> (MainWorldRemoteSnapshot, MainWorldRemoteSnapshot, f32) {
    let Some(after_index) = snapshots
        .iter()
        .position(|sample| sample.frame.0 as f32 >= target_frame)
    else {
        let latest = *snapshots.back().unwrap();
        return (latest, latest, 0.0);
    };
    if after_index == 0 {
        let first = snapshots[0];
        return (first, first, 0.0);
    }
    let before = snapshots[after_index - 1];
    let after = snapshots[after_index];
    let span = (after.frame.0 - before.frame.0).max(1) as f32;
    let factor = ((target_frame - before.frame.0 as f32) / span).clamp(0.0, 1.0);
    (before, after, factor)
}

fn pb_correction_kind(value: i32) -> Option<pb::MovementCorrectionKind> {
    pb::MovementCorrectionKind::try_from(value).ok()
}

fn reconcile_main_world_local_authority(
    runtime: &mut MainWorldMovementRuntime,
    authority: MainWorldLocalAuthoritySnapshot,
    visual_alpha: f32,
) -> MainWorldAuthorityCorrection {
    let previous_visual = runtime.visual_position(visual_alpha);
    let identity_changed = runtime
        .local_authority_entity_id
        .is_some_and(|entity_id| entity_id != authority.server_entity_id)
        || runtime
            .local_authority_scene_id
            .is_some_and(|scene_id| scene_id != authority.scene_id);
    let anchor = runtime
        .unconfirmed_inputs
        .iter()
        .find(|input| input.frame.0 == authority.confirmed_frame.0)
        .map(|input| input.predicted_after.position)
        .or_else(|| {
            (runtime.predicted.frame.0 == authority.confirmed_frame.0)
                .then_some(runtime.predicted.position)
        });
    let error_distance = anchor.map_or(f32::INFINITY, |position| {
        position.distance(authority.position)
    });
    let force_rebase = authority.force_rebase
        || identity_changed
        || anchor.is_none()
        || !error_distance.is_finite()
        || error_distance > MAIN_WORLD_SMALL_CORRECTION_DISTANCE_METRES;

    runtime.authority_baseline = Some(MainWorldAuthorityBaseline {
        frame: authority.frame,
        confirmed_frame: authority.confirmed_frame,
        position: authority.position,
        direction: authority.direction,
        moving: authority.moving,
    });
    runtime.last_applied_authority_frame = Some(authority.frame);
    runtime.local_authority_entity_id = Some(authority.server_entity_id);
    runtime.local_authority_scene_id = Some(authority.scene_id);

    while runtime
        .unconfirmed_inputs
        .front()
        .is_some_and(|input| input.frame.0 <= authority.confirmed_frame.0)
    {
        runtime.unconfirmed_inputs.pop_front();
    }
    while runtime
        .pending_prediction
        .front()
        .is_some_and(|pending| pending.input.frame.0 <= authority.confirmed_frame.0)
    {
        runtime.pending_prediction.pop_front();
    }

    // FixedUpdate consumes the pending queue independently from the render
    // update that receives authority. Rebuild every input's replay state, but
    // keep the predicted baseline at the last input already consumed by
    // FixedUpdate. Otherwise the next tick would apply an older pending input
    // and visibly move the local player backwards.
    let pending_frames: std::collections::HashSet<u32> = runtime
        .pending_prediction
        .iter()
        .map(|pending| pending.input.frame.0)
        .collect();
    let mut replayed = MainWorldPredictedState {
        frame: MainWorldPredictedFrame(authority.confirmed_frame.0),
        position: authority.position,
        direction: authority.direction,
        moving: authority.moving,
    };
    let mut replayed_applied = replayed;
    let mut replayed_previous_applied = replayed;
    for input in &mut runtime.unconfirmed_inputs {
        input.predicted_before = replayed;
        input.predicted_after =
            main_world_predicted_after_input(replayed, input.frame, input.direction);
        input.confirmed = false;
        replayed = input.predicted_after;
        if !pending_frames.contains(&input.frame.0) {
            replayed_previous_applied = replayed_applied;
            replayed_applied = replayed;
        }
    }
    runtime.pending_prediction = runtime
        .unconfirmed_inputs
        .iter()
        .filter(|input| pending_frames.contains(&input.frame.0))
        .copied()
        .map(|input| MainWorldPendingPrediction { input })
        .collect();
    runtime.predicted_previous = replayed_previous_applied;
    runtime.predicted = replayed_applied;
    if force_rebase {
        runtime.predicted_previous = runtime.predicted;
        runtime.clear_visual_correction();
        MainWorldAuthorityCorrection::Rebased
    } else {
        runtime.set_small_visual_correction(previous_visual, visual_alpha);
        MainWorldAuthorityCorrection::Smoothed
    }
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
        if intent.active {
            warn!(
                entry_phase = ?entry.phase,
                entry_input_frozen = entry.input_frozen,
                movement_input_frozen = movement.input_frozen,
                generation_matches = movement.generation == entry.generation,
                scene_session_matches = movement.session_id == entry.scene_session_id,
                room_membership = ?entry.room_membership,
                entry_room_id = ?entry.room_id,
                session_room_id = ?session.room_id,
                character_matches = entry.character_id == session.character_id,
                has_connection = session.connection_id.is_some(),
                connected = session.connected,
                authenticated = session.authenticated,
                connection_state = ?session.game_connection_state,
                "main world move input blocked by send gate"
            );
        }
        dispatch.observe_closed_gate(&intent);
        return;
    }

    dispatch.bind(&movement, &intent);
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
    let Some(frame) = dispatch.next_frame(&movement) else {
        warn!(
            latest_room_frame = movement.latest_room_frame.0,
            predicted_frame = movement.predicted.frame.0,
            snapshot_generation = entry.snapshot_generation,
            last_dispatched_frame = dispatch.last_dispatched_frame.0,
            "main world move input waiting for legal frame window"
        );
        return;
    };
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
    warn!(
        frame_id = frame.0,
        latest_room_frame = movement.latest_room_frame.0,
        dir_x = direction.x,
        dir_y = direction.y,
        "main world move input dispatched"
    );
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
/// using the current follow-camera yaw. The camera orbit stores the offset
/// from the actor to the camera, so its inverse is the view-forward vector.
/// Pitch does not alter planar movement.
pub(in crate::game) fn main_world_camera_relative_direction(local_axis: Vec2, yaw: f32) -> Vec2 {
    if !local_axis.is_finite() || !yaw.is_finite() {
        return Vec2::ZERO;
    }
    let forward = -Vec2::new(yaw.sin(), yaw.cos());
    let right = Vec2::new(-forward.y, forward.x);
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

/// Converts a virtual-stick displacement into continuous local movement. The
/// screen Y axis grows downward, while local forward grows upward, so Y is
/// inverted here before keyboard and touch axes are combined. A fixed dead
/// zone prevents tiny touch jitter from producing `MOVE_DIR` later.
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
    Vec2::new(displacement.x, -displacement.y).normalize_or_zero() * magnitude
}

/// Advances exactly one sent authority input per fixed tick. Render frames can
/// never create prediction frames, and un-sent local intent never mutates the
/// predicted baseline.
fn predict_main_world_movement_fixed(
    entry: Option<Res<MainWorldEntryState>>,
    mut runtime: ResMut<MainWorldMovementRuntime>,
    mut diagnostics: ResMut<MainWorldMovementDiagnostics>,
) {
    let started_at = Instant::now();
    let Some(entry) = entry else {
        diagnostics.record_fixed_prediction(started_at);
        return;
    };
    if !entry.allows_gameplay_input()
        || runtime.generation != entry.generation
        || runtime.session_id != entry.scene_session_id
        || !runtime.allows_local_movement()
    {
        diagnostics.record_fixed_prediction(started_at);
        return;
    }
    let Some(pending) = runtime.pending_prediction.pop_front() else {
        // Fixed interpolation restarts from alpha zero after every fixed
        // boundary. When no new authority input is ready, collapse the
        // endpoints to the current state so presentation holds its position
        // instead of replaying the previous movement step backwards.
        runtime.predicted_previous = runtime.predicted;
        diagnostics.record_fixed_prediction(started_at);
        return;
    };

    runtime.predicted_previous = runtime.predicted;
    runtime.predicted = pending.input.predicted_after;
    diagnostics.record_fixed_prediction(started_at);
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
    time: Res<Time>,
    fixed_time: Res<Time<Fixed>>,
    mut runtime: ResMut<MainWorldMovementRuntime>,
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
    runtime.advance_visual_correction(time.delta_secs());
    let visual_position = runtime.visual_position(fixed_time.overstep_fraction());
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
    use crate::game::scenes::main_world_contract::MAIN_WORLD_SERVER_SCENE_ID;
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
        app.world_mut()
            .write_message(MyServerEvent::FrameBundlePush(pb::FrameBundlePush {
                room_id: MAIN_WORLD_PUBLIC_ROOM_ID.to_owned(),
                frame_id: 40,
                fps: MAIN_WORLD_AUTHORITY_TICKS_PER_SECOND,
                inputs: Vec::new(),
                is_silent_frame: true,
                snapshot: None,
            }));
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

    fn remote_snapshot_state(frame: u32, x: f32, moving: bool) -> MainWorldRemoteSnapshot {
        MainWorldRemoteSnapshot {
            frame: MainWorldAuthorityFrame(frame),
            position: Vec3::new(x, 0.0, 0.0),
            direction: Vec2::X,
            moving,
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

    fn local_snapshot(
        room_id: &str,
        frame: u32,
        character_id: &str,
        entity_id: u64,
        scene_id: i32,
        x: f32,
        y: f32,
        last_input_frame: u32,
    ) -> MyServerEvent {
        MyServerEvent::MovementSnapshotPush(pb::MovementSnapshotPush {
            room_id: room_id.to_owned(),
            frame_id: frame,
            entities: vec![pb::EntityTransform {
                entity_id,
                character_id: character_id.to_owned(),
                scene_id,
                x,
                y,
                dir_x: 1.0,
                dir_y: 0.0,
                moving: true,
                last_input_frame,
            }],
            ..default()
        })
    }

    fn remote_snapshot_event(
        frame: u32,
        x: f32,
        y: f32,
        moving: bool,
        dir_x: f32,
        dir_y: f32,
    ) -> MyServerEvent {
        MyServerEvent::MovementSnapshotPush(pb::MovementSnapshotPush {
            room_id: MAIN_WORLD_PUBLIC_ROOM_ID.to_owned(),
            frame_id: frame,
            entities: vec![pb::EntityTransform {
                entity_id: 99,
                character_id: "chr-remote".to_owned(),
                scene_id: MAIN_WORLD_SERVER_SCENE_ID,
                x,
                y,
                dir_x,
                dir_y,
                moving,
                ..default()
            }],
            ..default()
        })
    }

    fn remote_snapshot_push(
        frame: u32,
        entities: Vec<(&str, u64, i32, f32, f32, bool, f32, f32)>,
        full_sync: bool,
        correction_kind: pb::MovementCorrectionKind,
    ) -> MyServerEvent {
        MyServerEvent::MovementSnapshotPush(pb::MovementSnapshotPush {
            room_id: MAIN_WORLD_PUBLIC_ROOM_ID.to_owned(),
            frame_id: frame,
            full_sync,
            correction_kind: correction_kind as i32,
            entities: entities
                .into_iter()
                .map(
                    |(character_id, entity_id, scene_id, x, y, moving, dir_x, dir_y)| {
                        pb::EntityTransform {
                            character_id: character_id.to_owned(),
                            entity_id,
                            scene_id,
                            x,
                            y,
                            moving,
                            dir_x,
                            dir_y,
                            ..default()
                        }
                    },
                )
                .collect(),
            ..default()
        })
    }

    fn movement_reject_event(
        room_id: &str,
        character_id: &str,
        frame: u32,
        reference_frame: u32,
        scene_id: i32,
        reason: pb::MovementCorrectionReason,
        dir_x: f32,
        dir_y: f32,
    ) -> MyServerEvent {
        MyServerEvent::MovementRejectPush(pb::MovementRejectPush {
            room_id: room_id.to_owned(),
            frame_id: frame,
            character_id: character_id.to_owned(),
            reference_frame_id: reference_frame,
            reason_code: reason as i32,
            error_code: "MOVEMENT_REJECTED".to_owned(),
            corrected: Some(pb::EntityTransform {
                entity_id: 7,
                character_id: character_id.to_owned(),
                scene_id,
                x: 2002.0,
                y: 2003.0,
                dir_x,
                dir_y,
                moving: true,
                last_input_frame: reference_frame,
                ..default()
            }),
            correction_kind: pb::MovementCorrectionKind::Strong as i32,
            ..default()
        })
    }

    #[test]
    fn remote_snapshot_events_interpolate_with_delay_preserve_heading_and_clear_on_full_sync() {
        let (mut app, _) = networked_movement_app();
        app.update();
        let session_id = SceneSessionId::from("main-world-7");
        let remote = app
            .world_mut()
            .spawn((
                MainWorldPlayer {
                    character_id: "chr-remote".to_owned(),
                    server_entity_id: 99,
                    ownership: MainWorldPlayerOwnership::Remote,
                    scene_session_id: session_id,
                    last_authoritative_frame: 0,
                },
                FangyuanPlayerPosition::default(),
                FangyuanObjectState::default(),
                Transform::default(),
            ))
            .id();

        app.world_mut()
            .write_message(remote_snapshot_event(10, 2001.0, 2001.0, true, 1.0, 0.0));
        app.update();
        app.world_mut()
            .write_message(remote_snapshot_event(12, 2003.0, 2001.0, true, 1.0, 0.0));
        app.update();
        let runtime = app.world().resource::<MainWorldMovementRuntime>();
        let buffer = runtime.remote_interpolation.get("chr-remote").unwrap();
        assert_eq!(
            buffer
                .snapshots()
                .iter()
                .map(|s| s.frame.0)
                .collect::<Vec<_>>(),
            vec![10, 12]
        );
        assert_eq!(
            app.world().get::<Transform>(remote).unwrap().translation,
            Vec3::new(1.0, 0.0, 1.0)
        );

        app.world_mut()
            .write_message(remote_snapshot_event(11, 2002.0, 2001.0, true, 0.0, 0.0));
        app.update();
        let runtime = app.world().resource::<MainWorldMovementRuntime>();
        let buffer = runtime.remote_interpolation.get("chr-remote").unwrap();
        assert_eq!(
            buffer
                .snapshots()
                .iter()
                .map(|s| s.frame.0)
                .collect::<Vec<_>>(),
            vec![10, 11, 12]
        );
        assert_eq!(buffer.snapshots()[1].direction, Vec2::X);

        app.world_mut()
            .write_message(remote_snapshot_event(13, 2004.0, 2001.0, false, 0.0, 0.0));
        app.update();
        assert_eq!(
            app.world().get::<Transform>(remote).unwrap().translation,
            Vec3::new(1.0, 0.0, 1.0)
        );

        app.world_mut()
            .write_message(remote_snapshot_event(16, 2007.0, 2001.0, false, 0.0, 0.0));
        advance_update(&mut app, Duration::from_millis(40));
        let before_fixed_boundary = app.world().get::<Transform>(remote).unwrap().translation;
        advance_update(&mut app, Duration::from_millis(20));
        let after_fixed_boundary = app.world().get::<Transform>(remote).unwrap().translation;
        assert!(before_fixed_boundary.x > 1.0);
        assert!(
            after_fixed_boundary.x > before_fixed_boundary.x,
            "remote presentation must not rewind when fixed overstep resets: before={before_fixed_boundary:?}, after={after_fixed_boundary:?}"
        );

        app.world_mut()
            .write_message(MyServerEvent::MovementSnapshotPush(
                pb::MovementSnapshotPush {
                    room_id: MAIN_WORLD_PUBLIC_ROOM_ID.to_owned(),
                    frame_id: 14,
                    full_sync: true,
                    entities: Vec::new(),
                    ..default()
                },
            ));
        app.update();
        assert!(
            !app.world()
                .resource::<MainWorldMovementRuntime>()
                .remote_interpolation
                .contains_key("chr-remote")
        );
    }

    #[test]
    fn shared_snapshot_fixture_drives_local_correction_and_remote_visual_path() {
        let (mut app, _) = networked_movement_app();
        app.update();
        let session_id = SceneSessionId::from("main-world-7");
        let local = app
            .world_mut()
            .spawn((
                MainWorldPlayer {
                    character_id: "chr-local".to_owned(),
                    server_entity_id: 7,
                    ownership: MainWorldPlayerOwnership::Local,
                    scene_session_id: session_id.clone(),
                    last_authoritative_frame: 0,
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
                    server_entity_id: 99,
                    ownership: MainWorldPlayerOwnership::Remote,
                    scene_session_id: session_id,
                    last_authoritative_frame: 0,
                },
                FangyuanPlayerPosition::default(),
                FangyuanObjectState::default(),
                Transform::default(),
            ))
            .id();
        app.world_mut().write_message(remote_snapshot_push(
            50,
            vec![
                (
                    "chr-local",
                    7,
                    MAIN_WORLD_SERVER_SCENE_ID,
                    2001.0,
                    2002.0,
                    true,
                    0.0,
                    1.0,
                ),
                (
                    "chr-remote",
                    99,
                    MAIN_WORLD_SERVER_SCENE_ID,
                    2003.0,
                    2004.0,
                    true,
                    1.0,
                    0.0,
                ),
            ],
            true,
            pb::MovementCorrectionKind::FullSync,
        ));
        app.update();

        let runtime = app.world().resource::<MainWorldMovementRuntime>();
        assert_eq!(
            runtime.authority_baseline.unwrap().position,
            Vec3::new(1.0, 0.0, 2.0)
        );
        assert_eq!(runtime.remote_interpolation["chr-remote"].len(), 1);
        assert_eq!(
            app.world().get::<Transform>(local).unwrap().translation,
            Vec3::new(1.0, 0.0, 2.0)
        );
        assert_eq!(
            app.world().get::<Transform>(remote).unwrap().translation,
            Vec3::new(3.0, 0.0, 4.0)
        );
    }

    #[test]
    fn remote_snapshot_push_is_idempotent_and_resets_on_identity_or_large_jump() {
        let (mut app, _) = networked_movement_app();
        app.update();

        app.world_mut().write_message(remote_snapshot_push(
            20,
            vec![(
                "chr-remote",
                99,
                MAIN_WORLD_SERVER_SCENE_ID,
                2001.0,
                2001.0,
                true,
                1.0,
                0.0,
            )],
            false,
            pb::MovementCorrectionKind::Incremental,
        ));
        app.update();
        app.world_mut().write_message(remote_snapshot_push(
            21,
            vec![(
                "chr-remote",
                99,
                MAIN_WORLD_SERVER_SCENE_ID,
                2002.0,
                2001.0,
                true,
                1.0,
                0.0,
            )],
            false,
            pb::MovementCorrectionKind::Incremental,
        ));
        app.update();

        // Replaying frame 21 replaces the sample rather than growing the queue.
        app.world_mut().write_message(remote_snapshot_push(
            21,
            vec![(
                "chr-remote",
                99,
                MAIN_WORLD_SERVER_SCENE_ID,
                2002.5,
                2001.0,
                false,
                0.0,
                0.0,
            )],
            false,
            pb::MovementCorrectionKind::Incremental,
        ));
        app.update();
        let runtime = app.world().resource::<MainWorldMovementRuntime>();
        let buffer = runtime.remote_interpolation.get("chr-remote").unwrap();
        assert_eq!(buffer.len(), 2);
        assert_eq!(
            buffer.snapshots().back().unwrap().position,
            Vec3::new(2.5, 0.0, 1.0)
        );
        assert!(!buffer.snapshots().back().unwrap().moving);

        // A large discontinuity starts a new interpolation baseline.
        app.world_mut().write_message(remote_snapshot_push(
            22,
            vec![(
                "chr-remote",
                99,
                MAIN_WORLD_SERVER_SCENE_ID,
                2015.0,
                2001.0,
                true,
                0.0,
                1.0,
            )],
            false,
            pb::MovementCorrectionKind::Incremental,
        ));
        app.update();
        let runtime = app.world().resource::<MainWorldMovementRuntime>();
        let buffer = runtime.remote_interpolation.get("chr-remote").unwrap();
        assert_eq!(buffer.len(), 1);
        assert_eq!(
            buffer.snapshots().front().unwrap().frame,
            MainWorldAuthorityFrame(22)
        );
        assert!(buffer.snapshots().front().unwrap().direction.is_finite());

        // Changing the server identity also drops the prior baseline.
        app.world_mut().write_message(remote_snapshot_push(
            23,
            vec![(
                "chr-remote",
                100,
                MAIN_WORLD_SERVER_SCENE_ID,
                2015.2,
                2001.0,
                true,
                1.0,
                0.0,
            )],
            false,
            pb::MovementCorrectionKind::Incremental,
        ));
        app.update();
        let runtime = app.world().resource::<MainWorldMovementRuntime>();
        let buffer = runtime.remote_interpolation.get("chr-remote").unwrap();
        assert_eq!(buffer.len(), 1);
        assert_eq!(
            buffer.snapshots().front().unwrap().frame,
            MainWorldAuthorityFrame(23)
        );

        // A snapshot from an unrelated authority scene is rejected without
        // changing the established remote baseline.
        app.world_mut().write_message(remote_snapshot_push(
            24,
            vec![(
                "chr-remote",
                100,
                MAIN_WORLD_SERVER_SCENE_ID + 1,
                2015.4,
                2001.0,
                true,
                1.0,
                0.0,
            )],
            false,
            pb::MovementCorrectionKind::Incremental,
        ));
        app.update();
        let runtime = app.world().resource::<MainWorldMovementRuntime>();
        let buffer = runtime.remote_interpolation.get("chr-remote").unwrap();
        assert_eq!(buffer.len(), 1);
        assert_eq!(
            buffer.snapshots().front().unwrap().frame,
            MainWorldAuthorityFrame(23)
        );
    }

    #[test]
    fn remote_snapshot_push_strong_and_recovery_start_new_baselines_with_valid_rotation() {
        let (mut app, _) = networked_movement_app();
        app.update();
        let remote = app
            .world_mut()
            .spawn((
                MainWorldPlayer {
                    character_id: "chr-remote".to_owned(),
                    server_entity_id: 99,
                    ownership: MainWorldPlayerOwnership::Remote,
                    scene_session_id: SceneSessionId::from("main-world-7"),
                    last_authoritative_frame: 0,
                },
                FangyuanPlayerPosition::default(),
                FangyuanObjectState::default(),
                Transform::default(),
            ))
            .id();

        for (frame, correction_kind) in [
            (30, pb::MovementCorrectionKind::Incremental),
            (31, pb::MovementCorrectionKind::Strong),
            (32, pb::MovementCorrectionKind::Recovery),
        ] {
            app.world_mut().write_message(remote_snapshot_push(
                frame,
                vec![(
                    "chr-remote",
                    99,
                    MAIN_WORLD_SERVER_SCENE_ID,
                    2000.5 + frame as f32,
                    2001.0,
                    true,
                    0.0,
                    1.0,
                )],
                false,
                correction_kind,
            ));
            app.update();
            let runtime = app.world().resource::<MainWorldMovementRuntime>();
            let buffer = runtime.remote_interpolation.get("chr-remote").unwrap();
            assert_eq!(buffer.len(), 1, "{correction_kind:?} must reset baseline");
            assert!(buffer.snapshots().front().unwrap().direction.is_finite());
            let transform = app.world().get::<Transform>(remote).unwrap();
            assert!(transform.translation.is_finite());
            assert!(transform.rotation.is_finite());
            assert!((transform.rotation.length_squared() - 1.0).abs() < 0.0001);
        }
    }

    #[test]
    fn targeted_recovery_snapshot_registers_all_visible_remote_entities() {
        let (mut app, _) = networked_movement_app();
        app.update();
        let remote = app
            .world_mut()
            .spawn((
                MainWorldPlayer {
                    character_id: "chr-remote".to_owned(),
                    server_entity_id: 99,
                    ownership: MainWorldPlayerOwnership::Remote,
                    scene_session_id: SceneSessionId::from("main-world-7"),
                    last_authoritative_frame: 0,
                },
                FangyuanPlayerPosition::default(),
                FangyuanObjectState::default(),
                Transform::default(),
            ))
            .id();

        let MyServerEvent::MovementSnapshotPush(mut push) = remote_snapshot_push(
            60,
            vec![
                (
                    "chr-local",
                    7,
                    MAIN_WORLD_SERVER_SCENE_ID,
                    2002.0,
                    2002.0,
                    false,
                    1.0,
                    0.0,
                ),
                (
                    "chr-remote",
                    99,
                    MAIN_WORLD_SERVER_SCENE_ID,
                    2004.0,
                    2002.0,
                    true,
                    1.0,
                    0.0,
                ),
            ],
            true,
            pb::MovementCorrectionKind::Recovery,
        ) else {
            unreachable!();
        };
        push.target_character_ids = vec!["chr-local".to_owned()];
        app.world_mut()
            .write_message(MyServerEvent::MovementSnapshotPush(push));
        app.update();

        let runtime = app.world().resource::<MainWorldMovementRuntime>();
        assert!(runtime.remote_interpolation.contains_key("chr-remote"));
        assert_eq!(runtime.remote_interpolation["chr-remote"].len(), 1);
        assert!(
            app.world()
                .get::<Transform>(remote)
                .unwrap()
                .translation
                .is_finite()
        );
    }

    #[test]
    fn remote_full_sync_retains_visible_players_and_removes_missing_players() {
        let (mut app, _) = networked_movement_app();
        app.update();
        for (index, character_id) in ["chr-remote", "chr-other"].iter().enumerate() {
            app.world_mut().write_message(remote_snapshot_push(
                40 + index as u32,
                vec![(
                    character_id,
                    100 + index as u64,
                    MAIN_WORLD_SERVER_SCENE_ID,
                    2001.0 + index as f32,
                    2001.0,
                    true,
                    1.0,
                    0.0,
                )],
                false,
                pb::MovementCorrectionKind::Incremental,
            ));
            app.update();
        }
        assert_eq!(
            app.world()
                .resource::<MainWorldMovementRuntime>()
                .remote_interpolation
                .len(),
            2
        );

        app.world_mut().write_message(remote_snapshot_push(
            42,
            vec![(
                "chr-remote",
                100,
                MAIN_WORLD_SERVER_SCENE_ID,
                2003.0,
                2001.0,
                true,
                1.0,
                0.0,
            )],
            true,
            pb::MovementCorrectionKind::FullSync,
        ));
        app.update();
        let runtime = app.world().resource::<MainWorldMovementRuntime>();
        assert!(runtime.remote_interpolation.contains_key("chr-remote"));
        assert!(!runtime.remote_interpolation.contains_key("chr-other"));
        assert_eq!(runtime.remote_interpolation["chr-remote"].len(), 1);
    }

    #[test]
    fn complete_room_snapshot_removes_missing_remote_interpolation_without_rebase() {
        let (mut app, _) = networked_movement_app();
        app.update();
        for (index, character_id) in ["chr-remote", "chr-other"].iter().enumerate() {
            app.world_mut().write_message(remote_snapshot_push(
                50 + index as u32,
                vec![(
                    character_id,
                    100 + index as u64,
                    MAIN_WORLD_SERVER_SCENE_ID,
                    2001.0 + index as f32,
                    2001.0,
                    true,
                    1.0,
                    0.0,
                )],
                false,
                pb::MovementCorrectionKind::Incremental,
            ));
            app.update();
        }
        assert_eq!(
            app.world()
                .resource::<MainWorldMovementRuntime>()
                .remote_interpolation
                .len(),
            2
        );

        let MyServerEvent::MovementSnapshotPush(mut room_snapshot) = remote_snapshot_push(
            52,
            vec![(
                "chr-remote",
                100,
                MAIN_WORLD_SERVER_SCENE_ID,
                2003.0,
                2001.0,
                true,
                1.0,
                0.0,
            )],
            false,
            pb::MovementCorrectionKind::Incremental,
        ) else {
            panic!();
        };
        room_snapshot.reason =
            super::super::main_world_contract::MAIN_WORLD_ROOM_SNAPSHOT_REASON.to_owned();
        app.world_mut()
            .write_message(MyServerEvent::MovementSnapshotPush(room_snapshot));
        app.update();

        let runtime = app.world().resource::<MainWorldMovementRuntime>();
        assert!(runtime.remote_interpolation.contains_key("chr-remote"));
        assert!(!runtime.remote_interpolation.contains_key("chr-other"));
        assert_eq!(runtime.remote_interpolation["chr-remote"].len(), 2);
    }

    #[test]
    fn remote_snapshot_push_is_cleared_when_scene_session_exits() {
        let (mut app, _) = networked_movement_app();
        app.update();
        app.world_mut()
            .write_message(remote_snapshot_event(50, 2001.0, 2001.0, true, 1.0, 0.0));
        app.update();
        assert!(
            !app.world()
                .resource::<MainWorldMovementRuntime>()
                .remote_interpolation
                .is_empty()
        );

        {
            let mut entry = app.world_mut().resource_mut::<MainWorldEntryState>();
            entry.phase = MainWorldEntryPhase::Exiting;
            entry.input_frozen = true;
        }
        app.update();
        let runtime = app.world().resource::<MainWorldMovementRuntime>();
        assert!(runtime.remote_interpolation.is_empty());
        assert!(runtime.session_id.is_none());
        assert!(runtime.input_frozen);
    }

    fn seed_authority_history(app: &mut App) {
        let mut runtime = app.world_mut().resource_mut::<MainWorldMovementRuntime>();
        let base = predicted_state(41, Vec3::new(1.0, 0.0, 1.0), Vec2::X, true);
        let first = MainWorldUnconfirmedInput {
            frame: MainWorldPredictedFrame(42),
            direction: Vec2::X,
            predicted_before: base,
            predicted_after: predicted_state(42, Vec3::new(1.2, 0.0, 1.0), Vec2::X, true),
            confirmed: false,
        };
        let second = MainWorldUnconfirmedInput {
            frame: MainWorldPredictedFrame(43),
            direction: Vec2::Y,
            predicted_before: first.predicted_after,
            predicted_after: predicted_state(43, Vec3::new(1.2, 0.0, 1.2), Vec2::Y, true),
            confirmed: false,
        };
        runtime.predicted = second.predicted_after;
        runtime.predicted_previous = first.predicted_after;
        runtime.push_unconfirmed_input(first);
        runtime.push_unconfirmed_input(second);
        runtime.queue_prediction(first);
        runtime.queue_prediction(second);
    }

    #[test]
    fn local_snapshot_event_confirms_history_and_replays_remaining_inputs_idempotently() {
        let (mut app, _) = networked_movement_app();
        app.update();
        seed_authority_history(&mut app);

        app.world_mut().write_message(local_snapshot(
            MAIN_WORLD_PUBLIC_ROOM_ID,
            50,
            "chr-local",
            7,
            MAIN_WORLD_SERVER_SCENE_ID,
            2001.2,
            2001.0,
            42,
        ));
        app.update();
        let runtime = app.world().resource::<MainWorldMovementRuntime>();
        assert_eq!(
            runtime.authority_baseline.unwrap().confirmed_frame,
            MainWorldConfirmedFrame(42)
        );
        assert_eq!(runtime.unconfirmed_inputs.len(), 1);
        assert_eq!(
            runtime.unconfirmed_inputs.front().unwrap().frame,
            MainWorldPredictedFrame(43)
        );
        assert_eq!(runtime.pending_prediction.len(), 1);
        assert_eq!(runtime.predicted.frame, MainWorldPredictedFrame(42));
        let baseline = runtime.authority_baseline.unwrap();

        app.world_mut().write_message(local_snapshot(
            MAIN_WORLD_PUBLIC_ROOM_ID,
            50,
            "chr-local",
            7,
            MAIN_WORLD_SERVER_SCENE_ID,
            2001.2,
            2001.0,
            42,
        ));
        app.update();
        let runtime = app.world().resource::<MainWorldMovementRuntime>();
        assert_eq!(runtime.authority_baseline.unwrap(), baseline);
        assert_eq!(runtime.unconfirmed_inputs.len(), 1);

        advance_update(&mut app, Duration::from_millis(50));
        let runtime = app.world().resource::<MainWorldMovementRuntime>();
        assert_eq!(runtime.predicted.frame, MainWorldPredictedFrame(43));
        assert_eq!(runtime.pending_prediction.len(), 0);
    }

    #[test]
    fn authority_replay_waits_for_fixed_ticks_before_applying_pending_inputs() {
        let (mut app, _) = networked_movement_app();
        app.update();
        let base = predicted_state(41, Vec3::ZERO, Vec2::X, true);
        let first_after =
            main_world_predicted_after_input(base, MainWorldPredictedFrame(42), Vec2::X);
        let first = MainWorldUnconfirmedInput {
            frame: MainWorldPredictedFrame(42),
            direction: Vec2::X,
            predicted_before: base,
            predicted_after: first_after,
            confirmed: false,
        };
        let second_after =
            main_world_predicted_after_input(first_after, MainWorldPredictedFrame(43), Vec2::X);
        let second = MainWorldUnconfirmedInput {
            frame: MainWorldPredictedFrame(43),
            direction: Vec2::X,
            predicted_before: first_after,
            predicted_after: second_after,
            confirmed: false,
        };
        let third_after =
            main_world_predicted_after_input(second_after, MainWorldPredictedFrame(44), Vec2::X);
        let third = MainWorldUnconfirmedInput {
            frame: MainWorldPredictedFrame(44),
            direction: Vec2::X,
            predicted_before: second_after,
            predicted_after: third_after,
            confirmed: false,
        };
        {
            let mut runtime = app.world_mut().resource_mut::<MainWorldMovementRuntime>();
            runtime.predicted_previous = base;
            runtime.predicted = first_after;
            runtime.push_unconfirmed_input(first);
            runtime.push_unconfirmed_input(second);
            runtime.push_unconfirmed_input(third);
            runtime.queue_prediction(second);
            runtime.queue_prediction(third);

            let correction = reconcile_main_world_local_authority(
                &mut runtime,
                MainWorldLocalAuthoritySnapshot {
                    frame: MainWorldAuthorityFrame(50),
                    confirmed_frame: MainWorldConfirmedFrame(42),
                    server_entity_id: 7,
                    scene_id: MAIN_WORLD_SERVER_SCENE_ID,
                    position: Vec3::new(0.21, 0.0, 0.0),
                    direction: Vec2::X,
                    moving: true,
                    force_rebase: false,
                },
                0.5,
            );
            assert_eq!(correction, MainWorldAuthorityCorrection::Smoothed);
            assert_eq!(runtime.predicted.frame, MainWorldPredictedFrame(42));
            assert!((runtime.predicted.position.x - 0.21).abs() < 0.000_01);
            assert_eq!(runtime.pending_prediction.len(), 2);
            assert!(
                (runtime.pending_prediction[0]
                    .input
                    .predicted_after
                    .position
                    .x
                    - 0.41)
                    .abs()
                    < 0.000_01
            );
            assert!(
                (runtime.pending_prediction[1]
                    .input
                    .predicted_after
                    .position
                    .x
                    - 0.61)
                    .abs()
                    < 0.000_01
            );
        }

        advance_update(&mut app, Duration::from_millis(50));
        let runtime = app.world().resource::<MainWorldMovementRuntime>();
        assert_eq!(runtime.predicted.frame, MainWorldPredictedFrame(43));
        assert!((runtime.predicted.position.x - 0.41).abs() < 0.000_01);

        advance_update(&mut app, Duration::from_millis(50));
        let runtime = app.world().resource::<MainWorldMovementRuntime>();
        assert_eq!(runtime.predicted.frame, MainWorldPredictedFrame(44));
        assert!((runtime.predicted.position.x - 0.61).abs() < 0.000_01);
    }

    #[test]
    fn authority_replay_preserves_applied_fixed_step_interpolation_endpoints() {
        let (mut app, _) = networked_movement_app();
        app.update();
        let base = predicted_state(40, Vec3::ZERO, Vec2::X, true);
        let confirmed_after =
            main_world_predicted_after_input(base, MainWorldPredictedFrame(41), Vec2::X);
        let confirmed = MainWorldUnconfirmedInput {
            frame: MainWorldPredictedFrame(41),
            direction: Vec2::X,
            predicted_before: base,
            predicted_after: confirmed_after,
            confirmed: false,
        };
        let applied_after =
            main_world_predicted_after_input(confirmed_after, MainWorldPredictedFrame(42), Vec2::X);
        let applied = MainWorldUnconfirmedInput {
            frame: MainWorldPredictedFrame(42),
            direction: Vec2::X,
            predicted_before: confirmed_after,
            predicted_after: applied_after,
            confirmed: false,
        };

        let mut runtime = app.world_mut().resource_mut::<MainWorldMovementRuntime>();
        runtime.predicted_previous = confirmed_after;
        runtime.predicted = applied_after;
        runtime.push_unconfirmed_input(confirmed);
        runtime.push_unconfirmed_input(applied);

        let correction = reconcile_main_world_local_authority(
            &mut runtime,
            MainWorldLocalAuthoritySnapshot {
                frame: MainWorldAuthorityFrame(50),
                confirmed_frame: MainWorldConfirmedFrame(41),
                server_entity_id: 7,
                scene_id: MAIN_WORLD_SERVER_SCENE_ID,
                position: confirmed_after.position + Vec3::new(0.000_1, 0.0, 0.0),
                direction: Vec2::X,
                moving: true,
                force_rebase: false,
            },
            0.1,
        );

        assert_eq!(correction, MainWorldAuthorityCorrection::Smoothed);
        assert_eq!(
            runtime.predicted_previous.frame,
            MainWorldPredictedFrame(41)
        );
        assert_eq!(runtime.predicted.frame, MainWorldPredictedFrame(42));
        assert!(runtime.visual_correction_offset.length() < 0.001);
        assert!(
            (runtime.visual_correction_offset.x + 0.000_1).abs() < 0.000_01,
            "visual correction must reflect only the authority error: {:?}",
            runtime.visual_correction_offset
        );
    }

    #[test]
    fn local_visual_correction_decays_by_render_delta() {
        let (mut app, _) = movement_app(active_entry(7, "main-world-7"));
        app.update();
        {
            let mut runtime = app.world_mut().resource_mut::<MainWorldMovementRuntime>();
            runtime.visual_correction_offset = Vec3::X;
            runtime.visual_correction_remaining_seconds = MAIN_WORLD_SMALL_CORRECTION_SECONDS;
        }

        advance_update(&mut app, Duration::from_millis(10));
        let runtime = app.world().resource::<MainWorldMovementRuntime>();
        assert!((runtime.visual_correction_remaining_seconds - 0.09).abs() < 0.000_01);
        assert!((runtime.visual_correction_offset.x - 0.9).abs() < 0.000_01);
    }

    #[test]
    fn local_snapshot_event_gates_identity_and_rebases_small_strong_and_missing_anchor_cases() {
        let (mut app, _) = networked_movement_app();
        app.update();
        seed_authority_history(&mut app);
        let baseline_before = app
            .world()
            .resource::<MainWorldMovementRuntime>()
            .authority_baseline;

        for event in [
            local_snapshot(
                "wrong-room",
                50,
                "chr-local",
                7,
                MAIN_WORLD_SERVER_SCENE_ID,
                2001.2,
                2001.0,
                42,
            ),
            local_snapshot(
                MAIN_WORLD_PUBLIC_ROOM_ID,
                50,
                "other-character",
                7,
                MAIN_WORLD_SERVER_SCENE_ID,
                2001.2,
                2001.0,
                42,
            ),
            local_snapshot(
                MAIN_WORLD_PUBLIC_ROOM_ID,
                50,
                "chr-local",
                7,
                999,
                2001.2,
                2001.0,
                42,
            ),
        ] {
            app.world_mut().write_message(event);
            app.update();
            assert_eq!(
                app.world()
                    .resource::<MainWorldMovementRuntime>()
                    .authority_baseline,
                baseline_before
            );
        }

        let mut small = local_snapshot(
            MAIN_WORLD_PUBLIC_ROOM_ID,
            50,
            "chr-local",
            7,
            MAIN_WORLD_SERVER_SCENE_ID,
            2001.3,
            2001.0,
            42,
        );
        if let MyServerEvent::MovementSnapshotPush(push) = &mut small {
            push.correction_kind = pb::MovementCorrectionKind::Incremental as i32;
        }
        app.world_mut().write_message(small);
        app.update();
        assert!(
            app.world()
                .resource::<MainWorldMovementRuntime>()
                .visual_correction_remaining_seconds
                > 0.0
        );

        let mut strong = local_snapshot(
            MAIN_WORLD_PUBLIC_ROOM_ID,
            51,
            "chr-local",
            8,
            MAIN_WORLD_SERVER_SCENE_ID,
            2010.0,
            2010.0,
            43,
        );
        if let MyServerEvent::MovementSnapshotPush(push) = &mut strong {
            push.full_sync = true;
            push.correction_kind = pb::MovementCorrectionKind::Strong as i32;
        }
        app.world_mut().write_message(strong);
        app.update();
        let runtime = app.world().resource::<MainWorldMovementRuntime>();
        assert_eq!(runtime.visual_correction_remaining_seconds, 0.0);
        assert_eq!(runtime.predicted_previous, runtime.predicted);

        app.world_mut().write_message(local_snapshot(
            MAIN_WORLD_PUBLIC_ROOM_ID,
            52,
            "chr-local",
            8,
            MAIN_WORLD_SERVER_SCENE_ID,
            2011.0,
            2011.0,
            999,
        ));
        app.update();
        assert!(
            app.world()
                .resource::<MainWorldMovementRuntime>()
                .unconfirmed_inputs
                .is_empty()
        );
    }

    #[test]
    fn movement_reject_applies_corrected_baseline_and_stops_invalid_prediction() {
        let (mut app, _) = networked_movement_app();
        app.update();
        seed_authority_history(&mut app);
        let baseline = app
            .world()
            .resource::<MainWorldMovementRuntime>()
            .authority_baseline;

        // Wrong room, character, scene and future reference frame are all
        // ignored and leave the prediction baseline untouched.
        for event in [
            movement_reject_event(
                "other-room",
                "chr-local",
                60,
                42,
                MAIN_WORLD_SERVER_SCENE_ID,
                pb::MovementCorrectionReason::CollisionBlocked,
                1.0,
                0.0,
            ),
            movement_reject_event(
                MAIN_WORLD_PUBLIC_ROOM_ID,
                "other-character",
                60,
                42,
                MAIN_WORLD_SERVER_SCENE_ID,
                pb::MovementCorrectionReason::CollisionBlocked,
                1.0,
                0.0,
            ),
            movement_reject_event(
                MAIN_WORLD_PUBLIC_ROOM_ID,
                "chr-local",
                60,
                42,
                MAIN_WORLD_SERVER_SCENE_ID + 1,
                pb::MovementCorrectionReason::CollisionBlocked,
                1.0,
                0.0,
            ),
            movement_reject_event(
                MAIN_WORLD_PUBLIC_ROOM_ID,
                "chr-local",
                60,
                999,
                MAIN_WORLD_SERVER_SCENE_ID,
                pb::MovementCorrectionReason::CollisionBlocked,
                1.0,
                0.0,
            ),
        ] {
            app.world_mut().write_message(event);
            app.update();
            assert_eq!(
                app.world()
                    .resource::<MainWorldMovementRuntime>()
                    .authority_baseline,
                baseline
            );
        }

        app.world_mut().write_message(movement_reject_event(
            MAIN_WORLD_PUBLIC_ROOM_ID,
            "chr-local",
            60,
            42,
            MAIN_WORLD_SERVER_SCENE_ID,
            pb::MovementCorrectionReason::CollisionBlocked,
            f32::NAN,
            0.0,
        ));
        app.update();
        let runtime = app.world().resource::<MainWorldMovementRuntime>();
        assert_eq!(runtime.predicted.position, Vec3::new(2.0, 0.0, 3.0));
        assert!(runtime.unconfirmed_inputs.is_empty());
        assert!(runtime.pending_prediction.is_empty());
        assert!(!app.world().resource::<MainWorldMovementIntent>().active);
    }

    #[test]
    fn late_movement_reject_cannot_rewind_a_newer_authority_snapshot() {
        let (mut app, _) = networked_movement_app();
        app.update();
        seed_authority_history(&mut app);
        app.world_mut().write_message(local_snapshot(
            MAIN_WORLD_PUBLIC_ROOM_ID,
            100,
            "chr-local",
            7,
            MAIN_WORLD_SERVER_SCENE_ID,
            2008.0,
            2009.0,
            41,
        ));
        app.update();
        {
            let mut intent = app.world_mut().resource_mut::<MainWorldMovementIntent>();
            intent.active = true;
            intent.direction = Vec2::X;
            intent.stop_sequence = 7;
        }
        // Let input collection settle this synthetic active intent before the
        // reject is injected, so the assertion observes only reject effects.
        app.update();
        let runtime_before = app.world().resource::<MainWorldMovementRuntime>();
        let baseline_before = runtime_before.authority_baseline;
        let predicted_before = runtime_before.predicted;
        let history_before = runtime_before.unconfirmed_inputs.clone();
        let pending_len_before = runtime_before.pending_prediction.len();
        let stop_sequence_before = app
            .world()
            .resource::<MainWorldMovementIntent>()
            .stop_sequence;

        app.world_mut().write_message(movement_reject_event(
            MAIN_WORLD_PUBLIC_ROOM_ID,
            "chr-local",
            50,
            42,
            MAIN_WORLD_SERVER_SCENE_ID,
            pb::MovementCorrectionReason::CollisionBlocked,
            f32::NAN,
            0.0,
        ));
        app.update();

        let runtime_after = app.world().resource::<MainWorldMovementRuntime>();
        assert_eq!(runtime_after.authority_baseline, baseline_before);
        assert_eq!(runtime_after.predicted, predicted_before);
        assert_eq!(runtime_after.unconfirmed_inputs, history_before);
        assert_eq!(runtime_after.pending_prediction.len(), pending_len_before);
        assert_eq!(
            app.world()
                .resource::<MainWorldMovementIntent>()
                .stop_sequence,
            stop_sequence_before
        );
    }

    #[test]
    fn recovery_snapshot_rebuilds_local_baseline_and_clears_remote_history() {
        let (mut app, _) = networked_movement_app();
        app.update();
        {
            let mut runtime = app.world_mut().resource_mut::<MainWorldMovementRuntime>();
            runtime
                .remote_buffer_mut("chr-remote")
                .push(remote_snapshot(9));
            runtime.push_unconfirmed_input(input(42));
        }
        {
            let mut entry = app.world_mut().resource_mut::<MainWorldEntryState>();
            entry.phase = MainWorldEntryPhase::Recovering;
            entry.input_frozen = true;
            entry.reconnect_requested = true;
        }
        app.world_mut().write_message({
            let mut event = local_snapshot(
                MAIN_WORLD_PUBLIC_ROOM_ID,
                70,
                "chr-local",
                7,
                MAIN_WORLD_SERVER_SCENE_ID,
                2005.0,
                2006.0,
                70,
            );
            if let MyServerEvent::MovementSnapshotPush(push) = &mut event {
                push.full_sync = true;
                push.correction_kind = pb::MovementCorrectionKind::Recovery as i32;
                push.entities.push(pb::EntityTransform {
                    entity_id: 99,
                    character_id: "chr-remote".to_owned(),
                    scene_id: MAIN_WORLD_SERVER_SCENE_ID,
                    x: 2004.0,
                    y: 2005.0,
                    dir_x: 0.0,
                    dir_y: 1.0,
                    moving: true,
                    ..default()
                });
            }
            event
        });
        app.update();
        let runtime = app.world().resource::<MainWorldMovementRuntime>();
        assert_eq!(
            runtime.authority_baseline.unwrap().position,
            Vec3::new(5.0, 0.0, 6.0)
        );
        assert!(runtime.remote_interpolation.contains_key("chr-remote"));
        assert_eq!(runtime.remote_interpolation["chr-remote"].len(), 1);
        assert!(runtime.predicted_previous == runtime.predicted);
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
        assert_eq!(input.predicted_after.position, Vec3::new(1.25, 0.0, -2.7));
        assert!(!input.confirmed);

        advance_update(&mut app, Duration::from_millis(50));
        let runtime = app.world().resource::<MainWorldMovementRuntime>();
        assert_eq!(runtime.predicted.frame, MainWorldPredictedFrame(42));
        assert_eq!(runtime.predicted.position, Vec3::new(1.25, 0.0, -2.7));
        assert_eq!(runtime.unconfirmed_inputs.len(), 1);
    }

    #[test]
    fn empty_fixed_tick_holds_current_prediction_without_visual_rewind() {
        let (mut app, _) = networked_movement_app();
        app.update();
        let previous = predicted_state(41, Vec3::ZERO, Vec2::X, true);
        let current = predicted_state(42, Vec3::new(0.2, 0.0, 0.0), Vec2::X, true);
        {
            let mut runtime = app.world_mut().resource_mut::<MainWorldMovementRuntime>();
            runtime.predicted_previous = previous;
            runtime.predicted = current;
            runtime.pending_prediction.clear();
        }

        advance_update(&mut app, Duration::from_millis(50));
        let runtime = app.world().resource::<MainWorldMovementRuntime>();
        assert_eq!(runtime.predicted_previous, current);
        assert_eq!(runtime.predicted, current);
        assert_eq!(runtime.visual_position(0.0), current.position);
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
    fn movement_diagnostics_record_update_fixed_and_presentation_samples() {
        let (mut app, _) = networked_movement_app();
        app.update();
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            50,
        )));
        app.update();
        let diagnostics = app.world().resource::<MainWorldMovementDiagnostics>();
        assert!(diagnostics.update_pipeline_samples >= 2);
        assert!(diagnostics.fixed_prediction_samples >= 1);
        assert!(diagnostics.presentation_pipeline_samples >= 2);
        assert!(diagnostics.update_pipeline_last < Duration::from_secs(1));
        assert!(diagnostics.fixed_prediction_last < Duration::from_secs(1));
        assert!(diagnostics.presentation_pipeline_last < Duration::from_secs(1));
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
    fn remote_stop_snapshot_releases_terminal_frame_without_an_idle_followup() {
        let mut buffer = MainWorldRemoteInterpolationBuffer::default();
        assert!(buffer.push(remote_snapshot_state(100, 0.0, true)));
        assert!(buffer.push(remote_snapshot_state(103, 3.0, true)));
        assert_eq!(buffer.advance_presentation_frame(1.0), Some(100.0));
        assert!(buffer.push(remote_snapshot_state(106, 6.0, true)));
        assert_eq!(buffer.advance_presentation_frame(0.15), Some(103.0));

        assert!(buffer.push(remote_snapshot_state(109, 9.0, false)));
        let middle_frame = buffer.advance_presentation_frame(0.15).unwrap();
        let (middle_before, middle_after, middle_factor) =
            remote_interpolation_sample(buffer.snapshots(), middle_frame);
        assert_eq!(middle_frame, 106.0);
        assert_eq!(
            middle_before
                .position
                .lerp(middle_after.position, middle_factor),
            Vec3::new(6.0, 0.0, 0.0)
        );

        let terminal_frame = buffer.advance_presentation_frame(0.15).unwrap();
        let (terminal_before, terminal_after, terminal_factor) =
            remote_interpolation_sample(buffer.snapshots(), terminal_frame);
        assert_eq!(terminal_frame, 109.0);
        assert_eq!(
            terminal_before
                .position
                .lerp(terminal_after.position, terminal_factor),
            Vec3::new(9.0, 0.0, 0.0)
        );
        assert_eq!(buffer.advance_presentation_frame(1.0), Some(109.0));
    }

    #[test]
    fn remote_idle_to_moving_rebases_to_delay_window_without_visual_jump() {
        let mut buffer = MainWorldRemoteInterpolationBuffer::default();
        assert!(buffer.push(remote_snapshot_state(100, 5.0, false)));
        assert!(buffer.push(remote_snapshot_state(115, 5.0, false)));

        assert!(buffer.push(remote_snapshot_state(130, 8.0, true)));
        assert_eq!(buffer.presentation_frame, Some(127.0));
        let anchor = buffer
            .snapshots()
            .iter()
            .find(|sample| sample.frame == MainWorldAuthorityFrame(127))
            .unwrap();
        assert_eq!(anchor.position, Vec3::new(5.0, 0.0, 0.0));
        assert!(!anchor.moving);
        let (at_anchor, _, _) = remote_interpolation_sample(buffer.snapshots(), 127.0);
        assert_eq!(at_anchor.position, Vec3::new(5.0, 0.0, 0.0));

        let next_frame = buffer.advance_presentation_frame(0.05).unwrap();
        let (before, after, factor) = remote_interpolation_sample(buffer.snapshots(), next_frame);
        assert_eq!(next_frame, 127.0);
        assert_eq!(before.position.lerp(after.position, factor).x, 5.0);

        assert!(buffer.push(remote_snapshot_state(133, 11.0, true)));
        assert_eq!(buffer.advance_presentation_frame(0.15), Some(130.0));
        assert_eq!(133.0 - buffer.presentation_frame.unwrap(), 3.0);
    }

    #[test]
    fn remote_late_snapshot_does_not_rewind_or_rebase_presentation() {
        let mut buffer = MainWorldRemoteInterpolationBuffer::default();
        assert!(buffer.push(remote_snapshot_state(100, 0.0, false)));
        assert!(buffer.push(remote_snapshot_state(130, 3.0, true)));
        assert_eq!(buffer.presentation_frame, Some(127.0));

        assert!(buffer.push(remote_snapshot_state(120, 1.0, true)));
        assert_eq!(buffer.presentation_frame, Some(127.0));
        assert_eq!(buffer.advance_presentation_frame(0.0), Some(127.0));
        assert_eq!(buffer.snapshots().back().unwrap().frame.0, 130);
    }

    #[test]
    fn remote_continuous_movement_keeps_the_normal_delay_and_monotonic_time() {
        let mut buffer = MainWorldRemoteInterpolationBuffer::default();
        assert!(buffer.push(remote_snapshot_state(200, 0.0, true)));
        assert!(buffer.push(remote_snapshot_state(203, 3.0, true)));
        assert_eq!(buffer.advance_presentation_frame(1.0), Some(200.0));
        assert!(buffer.push(remote_snapshot_state(206, 6.0, true)));
        assert_eq!(buffer.advance_presentation_frame(0.15), Some(203.0));
        assert!(buffer.push(remote_snapshot_state(209, 9.0, true)));
        assert_eq!(buffer.advance_presentation_frame(0.15), Some(206.0));
        assert_eq!(209.0 - buffer.presentation_frame.unwrap(), 3.0);
        assert_eq!(buffer.advance_presentation_frame(-1.0), Some(206.0));
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
        assert_vec2_approx_eq(intent(&app).direction, Vec2::new(1.0, -1.0).normalize());

        release_key(&mut app, KeyCode::KeyW);
        release_key(&mut app, KeyCode::KeyD);
        press_key(&mut app, KeyCode::ArrowLeft);
        press_key(&mut app, KeyCode::ArrowDown);
        app.update();
        assert_vec2_approx_eq(intent(&app).direction, Vec2::new(-1.0, 1.0).normalize());
    }

    #[test]
    fn camera_relative_mapping_matches_view_forward_and_right_at_cardinal_yaws() {
        // At yaw 0 the camera sits on +Z and looks toward -Z: W moves into
        // the view and D moves to the view's right.
        assert_vec2_approx_eq(main_world_camera_relative_direction(Vec2::Y, 0.0), -Vec2::Y);
        assert_eq!(main_world_camera_relative_direction(Vec2::X, 0.0), Vec2::X);

        // At +90 degrees the camera sits on +X and looks toward -X. The
        // screen-right direction rotates with the camera to -Z.
        assert_vec2_approx_eq(
            main_world_camera_relative_direction(Vec2::Y, std::f32::consts::FRAC_PI_2),
            -Vec2::X,
        );
        assert_vec2_approx_eq(
            main_world_camera_relative_direction(Vec2::X, std::f32::consts::FRAC_PI_2),
            -Vec2::Y,
        );
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
        assert_eq!(
            main_world_virtual_joystick_axis(Vec2::new(0.0, -MAIN_WORLD_TOUCH_JOYSTICK_RADIUS)),
            Vec2::Y
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

        app.insert_resource(MainWorldCameraOrbitState {
            yaw_radians: std::f32::consts::FRAC_PI_2,
            ..Default::default()
        });
        send_touch(
            &mut app,
            window,
            1,
            TouchPhase::Moved,
            Vec2::new(100.0, 220.0),
        );
        app.update();
        assert_vec2_approx_eq(intent(&app).direction, -Vec2::X);

        send_touch(
            &mut app,
            window,
            1,
            TouchPhase::Moved,
            Vec2::new(900.0, 300.0),
        );
        app.update();
        assert!(intent(&app).active);
        assert_vec2_approx_eq(intent(&app).direction, -Vec2::Y);

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
        assert_eq!(Vec2::new(dir_x, dir_y), -Vec2::Y);
        assert_eq!(client_state.frame_id, frame);
        assert_eq!(
            Vec2::new(client_state.x, client_state.y),
            Vec2::new(2001.25, 1997.3)
        );

        press_key(&mut app, KeyCode::KeyD);
        app.world_mut()
            .write_message(MyServerEvent::FrameBundlePush(pb::FrameBundlePush {
                room_id: MAIN_WORLD_PUBLIC_ROOM_ID.to_owned(),
                frame_id: 41,
                fps: MAIN_WORLD_AUTHORITY_TICKS_PER_SECOND,
                inputs: Vec::new(),
                is_silent_frame: true,
                snapshot: None,
            }));
        app.update();
        advance_update(&mut app, Duration::from_millis(50));
        let commands = read_new_move_commands(&app, &mut cursor);
        assert_eq!(commands.len(), 1);
        let (frame, input_type, dir_x, dir_y, client_state) = move_command_details(&commands[0]);
        assert_eq!(frame, 43);
        assert_eq!(input_type, pb::MoveInputType::MoveDir);
        assert_vec2_approx_eq(Vec2::new(dir_x, dir_y), Vec2::new(1.0, -1.0).normalize());
        assert_eq!(client_state.frame_id, frame);
    }

    #[test]
    fn move_dispatch_uses_room_lead_window_and_waits_for_clock_progress() {
        let (mut app, _) = networked_movement_app();
        let mut cursor = MessageCursor::<MyServerCommand>::default();
        app.update();
        app.world_mut()
            .write_message(MyServerEvent::FrameBundlePush(pb::FrameBundlePush {
                room_id: MAIN_WORLD_PUBLIC_ROOM_ID.to_owned(),
                frame_id: 1_200,
                fps: MAIN_WORLD_AUTHORITY_TICKS_PER_SECOND,
                inputs: Vec::new(),
                is_silent_frame: true,
                snapshot: None,
            }));
        press_key(&mut app, KeyCode::KeyW);
        app.update();

        advance_update(&mut app, Duration::from_millis(50));
        let commands = read_new_move_commands(&app, &mut cursor);
        assert_eq!(commands.len(), 1);
        assert_eq!(move_command_details(&commands[0]).0, 1_202);

        advance_update(&mut app, Duration::from_millis(50));
        assert!(read_new_move_commands(&app, &mut cursor).is_empty());

        app.world_mut()
            .write_message(MyServerEvent::FrameBundlePush(pb::FrameBundlePush {
                room_id: MAIN_WORLD_PUBLIC_ROOM_ID.to_owned(),
                frame_id: 1_201,
                fps: MAIN_WORLD_AUTHORITY_TICKS_PER_SECOND,
                inputs: Vec::new(),
                is_silent_frame: true,
                snapshot: None,
            }));
        app.update();
        advance_update(&mut app, Duration::from_millis(50));
        let commands = read_new_move_commands(&app, &mut cursor);
        assert_eq!(commands.len(), 1);
        assert_eq!(move_command_details(&commands[0]).0, 1_203);
    }

    #[test]
    fn move_dispatch_ignores_snapshot_sequence_ahead_of_room_clock() {
        let (mut app, _) = networked_movement_app();
        app.world_mut()
            .resource_mut::<MainWorldEntryState>()
            .snapshot_generation = 41_792;
        let mut cursor = MessageCursor::<MyServerCommand>::default();
        app.update();
        press_key(&mut app, KeyCode::KeyW);
        app.update();

        advance_update(&mut app, Duration::from_millis(50));
        let commands = read_new_move_commands(&app, &mut cursor);
        assert_eq!(commands.len(), 1);
        assert_eq!(move_command_details(&commands[0]).0, 42);
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
        app.world_mut()
            .write_message(MyServerEvent::FrameBundlePush(pb::FrameBundlePush {
                room_id: MAIN_WORLD_PUBLIC_ROOM_ID.to_owned(),
                frame_id: 41,
                fps: MAIN_WORLD_AUTHORITY_TICKS_PER_SECOND,
                inputs: Vec::new(),
                is_silent_frame: true,
                snapshot: None,
            }));
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
