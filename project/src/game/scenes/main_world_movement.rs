//! Client movement runtime boundary for the public main world.
//!
//! This module owns no scene entry policy and does not yet simulate movement.
//! It establishes the bounded, generation-scoped state and schedule required
//! by later input, send, prediction, correction, and interpolation stages.

use std::collections::{HashMap, VecDeque};

use bevy::{prelude::*, time::Fixed, transform::TransformSystems};

use crate::{
    framework::scene::prelude::SceneSessionId,
    game::{
        myserver::MyServerUpdateSet,
        scenes::{
            main_world_contract::{
                MAIN_WORLD_AUTHORITY_TICKS_PER_SECOND, MainWorldAuthorityFrame,
                MainWorldConfirmedFrame, MainWorldPredictedFrame, MainWorldRenderFrame,
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
}

impl MainWorldMovementIntent {
    pub fn clear(&mut self) {
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
            .insert_resource(Time::<Fixed>::from_hz(f64::from(
                MAIN_WORLD_AUTHORITY_TICKS_PER_SECOND,
            )))
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
                sync_main_world_movement_lifecycle
                    .in_set(MainWorldMovementUpdateSet::ConsumeAuthority),
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
) {
    let Some(entry) = entry else {
        intent.clear();
        runtime.clear();
        return;
    };
    let Some(session_id) = entry.scene_session_id.clone() else {
        intent.clear();
        runtime.clear();
        runtime.generation = entry.generation;
        return;
    };

    if entry.phase == MainWorldEntryPhase::Active && !entry.input_frozen {
        runtime.bind_active_session(entry.generation, session_id);
        return;
    }

    intent.clear();
    runtime.clear();
    runtime.generation = entry.generation;
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

    fn movement_app(entry: MainWorldEntryState) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(entry)
            .add_plugins(MainWorldMovementPlugin);
        app
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
        let mut app = movement_app(active_entry(7, "main-world-7"));
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
        assert_eq!(
            *app.world().resource::<MainWorldMovementIntent>(),
            MainWorldMovementIntent::default()
        );
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
        let mut app = movement_app(active_entry(1, "main-world-1"));
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
        let mut app = movement_app(MainWorldEntryState::default());
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
        let mut app = movement_app(active_entry(1, "main-world-1"));
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
}
