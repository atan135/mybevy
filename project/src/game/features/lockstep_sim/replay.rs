use std::collections::VecDeque;
use std::fmt;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sim_core::{
    CastSkillCommand, EntityId, FaceCommand, Fp, FrameId, MoveCommand, QuantizedDir, SimCommand,
    SimConfig, SimEvent, SimHash, SimInput, SimInputSource, SimStepResult, SimWorld, SkillId,
    SkillTarget, StepError, hash_world, step,
};

use crate::game::authority::{AuthorityEvent, AuthorityFrame, PlayerInput};

use super::{
    diagnostics::{LockstepSimDiagnosticsState, LockstepSimHashMatchStatus},
    payload::{
        SIM_INPUT_ACTION, SIM_INPUT_MAX_COMMANDS, SIM_INPUT_PAYLOAD_MAX_BYTES, SIM_INPUT_VERSION,
    },
    snapshot::ParsedInitialSnapshot,
    state::LockstepSimSceneState,
};

const REPLAY_HASH_HISTORY_LIMIT: usize = 512;
const REPLAY_EVENT_HISTORY_LIMIT: usize = 512;
const REPLAY_INPUT_HISTORY_LIMIT: usize = 512;
const REPLAY_WORLD_SNAPSHOT_INTERVAL: u32 = 10;
const REPLAY_WORLD_SNAPSHOT_LIMIT: usize = 64;
const SIM_INPUT_MAX_SPEED_MILLI: i64 = 12_000;

#[derive(Clone, Debug, Default, Resource, PartialEq, Eq)]
pub(in crate::game) struct LockstepSimReplayState {
    pub(in crate::game::features::lockstep_sim) world: Option<SimWorld>,
    pub(in crate::game::features::lockstep_sim) config: Option<SimConfig>,
    pub(in crate::game::features::lockstep_sim) snapshot_start_frame: Option<u32>,
    pub(in crate::game::features::lockstep_sim) last_applied_frame: Option<u32>,
    pub(in crate::game::features::lockstep_sim) input_history: VecDeque<LockstepSimFrameInputs>,
    pub(in crate::game::features::lockstep_sim) hash_history: VecDeque<LockstepSimFrameHash>,
    pub(in crate::game::features::lockstep_sim) event_history: VecDeque<LockstepSimFrameEvents>,
    pub(in crate::game::features::lockstep_sim) world_snapshots: VecDeque<LockstepSimWorldSnapshot>,
    pub(in crate::game::features::lockstep_sim) last_error: Option<LockstepSimReplayError>,
    pub(in crate::game::features::lockstep_sim) ignored_duplicate_or_old_frames: u64,
    pub(in crate::game::features::lockstep_sim) diagnostics: LockstepSimDiagnosticsState,
    pub(in crate::game::features::lockstep_sim) debug_diagnostics: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::game::features::lockstep_sim) struct LockstepSimFrameInputs {
    pub(in crate::game::features::lockstep_sim) frame: u32,
    pub(in crate::game::features::lockstep_sim) sim_inputs: Vec<SimInput>,
    pub(in crate::game::features::lockstep_sim) raw_input_count: usize,
    pub(in crate::game::features::lockstep_sim) sim_action_count: usize,
    pub(in crate::game::features::lockstep_sim) sim_command_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::game::features::lockstep_sim) struct LockstepSimFrameHash {
    pub(in crate::game::features::lockstep_sim) frame: u32,
    pub(in crate::game::features::lockstep_sim) local_hash: SimHash,
    pub(in crate::game::features::lockstep_sim) server_hash: Option<SimHashEnvelope>,
    pub(in crate::game::features::lockstep_sim) event_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::game::features::lockstep_sim) struct LockstepSimFrameEvents {
    pub(in crate::game::features::lockstep_sim) frame: u32,
    pub(in crate::game::features::lockstep_sim) events: Vec<SimEvent>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::game::features::lockstep_sim) struct LockstepSimWorldSnapshot {
    pub(in crate::game::features::lockstep_sim) frame: u32,
    pub(in crate::game::features::lockstep_sim) world: SimWorld,
    pub(in crate::game::features::lockstep_sim) hash: SimHash,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub(in crate::game::features::lockstep_sim) struct LockstepSimReplaySummary {
    pub(in crate::game::features::lockstep_sim) snapshot_frame: u32,
    pub(in crate::game::features::lockstep_sim) target_frame: u32,
    pub(in crate::game::features::lockstep_sim) replayed_frames: u32,
    pub(in crate::game::features::lockstep_sim) final_hash: SimHash,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub(in crate::game::features::lockstep_sim) struct LockstepSimMismatchCoverage {
    pub(in crate::game::features::lockstep_sim) frame: u32,
    pub(in crate::game::features::lockstep_sim) has_hash: bool,
    pub(in crate::game::features::lockstep_sim) has_input: bool,
    pub(in crate::game::features::lockstep_sim) has_replay_inputs: bool,
    pub(in crate::game::features::lockstep_sim) missing_input_frame: Option<u32>,
    pub(in crate::game::features::lockstep_sim) has_snapshot_at_or_before: bool,
    pub(in crate::game::features::lockstep_sim) snapshot_frame: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(in crate::game::features::lockstep_sim) struct SimHashEnvelope {
    pub(in crate::game::features::lockstep_sim) frame: u32,
    pub(in crate::game::features::lockstep_sim) value: u64,
    pub(in crate::game::features::lockstep_sim) hex: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::game::features::lockstep_sim) enum LockstepSimReplayError {
    MissingInitialSnapshot,
    MissingReplayWorld,
    MissingReplayConfig,
    InputPayload(LockstepSimReplayInputError),
    MissingFrame {
        expected: u32,
        actual: u32,
    },
    FrameOverflow {
        last_applied_frame: u32,
    },
    WorldFrameDiscontinuous {
        expected_world_frame: u32,
        actual_world_frame: u32,
    },
    Step(StepError),
}

impl fmt::Display for LockstepSimReplayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingInitialSnapshot => {
                formatter.write_str("lockstep sim replay has no initial snapshot")
            }
            Self::MissingReplayWorld => formatter.write_str("lockstep sim replay world is missing"),
            Self::MissingReplayConfig => {
                formatter.write_str("lockstep sim replay config is missing")
            }
            Self::InputPayload(error) => write!(formatter, "{error}"),
            Self::MissingFrame { expected, actual } => write!(
                formatter,
                "lockstep sim replay missing frame: expected {expected}, got {actual}"
            ),
            Self::FrameOverflow { last_applied_frame } => write!(
                formatter,
                "lockstep sim replay frame overflow after {last_applied_frame}"
            ),
            Self::WorldFrameDiscontinuous {
                expected_world_frame,
                actual_world_frame,
            } => write!(
                formatter,
                "lockstep sim replay world frame discontinuous: expected {expected_world_frame}, got {actual_world_frame}"
            ),
            Self::Step(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for LockstepSimReplayError {}

#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub(in crate::game::features::lockstep_sim) enum LockstepSimReplayCacheError {
    MissingReplayConfig,
    MissingSnapshot { target_frame: u32 },
    MissingInput { frame: u32 },
    Step(StepError),
}

impl fmt::Display for LockstepSimReplayCacheError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingReplayConfig => {
                formatter.write_str("lockstep sim replay cache config is missing")
            }
            Self::MissingSnapshot { target_frame } => write!(
                formatter,
                "lockstep sim replay cache has no snapshot at or before frame {target_frame}"
            ),
            Self::MissingInput { frame } => write!(
                formatter,
                "lockstep sim replay cache is missing authoritative input for frame {frame}"
            ),
            Self::Step(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for LockstepSimReplayCacheError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::game::features::lockstep_sim) enum LockstepSimReplayInputError {
    PayloadTooLarge { bytes: usize, max: usize },
    InvalidJson,
    UnsupportedVersion { actual: u32 },
    TooManyCommands { count: usize, max: usize },
    DirectionOutOfRange,
    MoveDirectionZero,
    MoveSpeedOutOfRange { raw_milli: i64 },
    SkillIdOutOfRange,
    TargetEntityIdOutOfRange,
}

impl fmt::Display for LockstepSimReplayInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PayloadTooLarge { bytes, max } => write!(
                formatter,
                "lockstep sim replay payload size {bytes} exceeds max {max}"
            ),
            Self::InvalidJson => formatter.write_str("lockstep sim replay payload JSON is invalid"),
            Self::UnsupportedVersion { actual } => write!(
                formatter,
                "lockstep sim replay payload version {actual} is unsupported"
            ),
            Self::TooManyCommands { count, max } => write!(
                formatter,
                "lockstep sim replay command count {count} exceeds max {max}"
            ),
            Self::DirectionOutOfRange => {
                formatter.write_str("lockstep sim replay command direction is out of range")
            }
            Self::MoveDirectionZero => {
                formatter.write_str("lockstep sim replay move command has zero direction")
            }
            Self::MoveSpeedOutOfRange { raw_milli } => write!(
                formatter,
                "lockstep sim replay move speed {raw_milli} is outside server range"
            ),
            Self::SkillIdOutOfRange => {
                formatter.write_str("lockstep sim replay castSkill skillId must be positive")
            }
            Self::TargetEntityIdOutOfRange => formatter.write_str(
                "lockstep sim replay castSkill targetEntityId must be non-zero when present",
            ),
        }
    }
}

impl std::error::Error for LockstepSimReplayInputError {}

impl LockstepSimReplayState {
    pub(in crate::game::features::lockstep_sim) fn reset(&mut self) {
        *self = Self::default();
    }

    fn initialize_from_snapshot_if_needed(&mut self, snapshot: &ParsedInitialSnapshot) {
        if self.snapshot_start_frame == Some(snapshot.start_frame) && self.world.is_some() {
            return;
        }

        self.world = Some(snapshot.world.clone());
        self.config = Some(snapshot.config.clone());
        self.snapshot_start_frame = Some(snapshot.start_frame);
        self.last_applied_frame = Some(snapshot.start_frame);
        self.input_history.clear();
        self.hash_history.clear();
        self.event_history.clear();
        self.world_snapshots.clear();
        self.record_world_snapshot(snapshot.start_frame, &snapshot.world);
        self.diagnostics.clear();
        self.last_error = None;
        self.ignored_duplicate_or_old_frames = 0;
    }

    fn record_inputs(&mut self, frame: &AuthorityFrame, sim_inputs: Vec<SimInput>) {
        self.input_history.push_back(LockstepSimFrameInputs {
            frame: frame.frame_id,
            raw_input_count: frame.inputs.len(),
            sim_action_count: frame
                .inputs
                .iter()
                .filter(|input| input.action == SIM_INPUT_ACTION)
                .count(),
            sim_command_count: sim_inputs.len(),
            sim_inputs,
        });
        while self.input_history.len() > REPLAY_INPUT_HISTORY_LIMIT {
            self.input_history.pop_front();
        }
    }

    fn record_hash(
        &mut self,
        frame: u32,
        result: &SimStepResult,
        server_hash: Option<SimHashEnvelope>,
    ) {
        self.hash_history.push_back(LockstepSimFrameHash {
            frame,
            local_hash: result.state_hash,
            server_hash,
            event_count: result.events.len(),
        });
        while self.hash_history.len() > REPLAY_HASH_HISTORY_LIMIT {
            self.hash_history.pop_front();
        }
    }

    fn record_events(&mut self, frame: u32, events: &[SimEvent]) {
        self.event_history.push_back(LockstepSimFrameEvents {
            frame,
            events: events.to_vec(),
        });
        while self.event_history.len() > REPLAY_EVENT_HISTORY_LIMIT {
            self.event_history.pop_front();
        }
    }

    fn record_periodic_world_snapshot(&mut self, frame: u32) -> Result<(), LockstepSimReplayError> {
        if frame % REPLAY_WORLD_SNAPSHOT_INTERVAL != 0 {
            return Ok(());
        }

        let world = self
            .world
            .as_ref()
            .ok_or(LockstepSimReplayError::MissingReplayWorld)?
            .clone();
        self.record_world_snapshot(frame, &world);
        Ok(())
    }

    fn record_world_snapshot(&mut self, frame: u32, world: &SimWorld) {
        if self
            .world_snapshots
            .back()
            .is_some_and(|last| last.frame == frame)
        {
            return;
        }

        self.world_snapshots.push_back(LockstepSimWorldSnapshot {
            frame,
            world: world.clone(),
            hash: hash_world(world),
        });
        while self.world_snapshots.len() > REPLAY_WORLD_SNAPSHOT_LIMIT {
            self.world_snapshots.pop_front();
        }
    }

    #[allow(dead_code)]
    pub(in crate::game::features::lockstep_sim) fn replay_from_cached_snapshot_to_frame(
        &self,
        target_frame: u32,
    ) -> Result<(SimWorld, LockstepSimReplaySummary), LockstepSimReplayCacheError> {
        let config = self
            .config
            .as_ref()
            .ok_or(LockstepSimReplayCacheError::MissingReplayConfig)?;
        let snapshot = self
            .world_snapshots
            .iter()
            .rev()
            .find(|snapshot| snapshot.frame <= target_frame)
            .ok_or(LockstepSimReplayCacheError::MissingSnapshot { target_frame })?;
        let mut world = snapshot.world.clone();
        let mut replayed_frames = 0_u32;

        for frame in snapshot.frame.saturating_add(1)..=target_frame {
            let inputs = self
                .cached_input_for_frame(frame)
                .ok_or(LockstepSimReplayCacheError::MissingInput { frame })?;
            step(&mut world, FrameId::new(frame), &inputs.sim_inputs, config)
                .map_err(LockstepSimReplayCacheError::Step)?;
            replayed_frames = replayed_frames.saturating_add(1);
        }

        let final_hash = hash_world(&world);
        Ok((
            world,
            LockstepSimReplaySummary {
                snapshot_frame: snapshot.frame,
                target_frame,
                replayed_frames,
                final_hash,
            },
        ))
    }

    #[allow(dead_code)]
    pub(in crate::game::features::lockstep_sim) fn mismatch_coverage(
        &self,
        frame: u32,
    ) -> LockstepSimMismatchCoverage {
        let snapshot_frame = self
            .world_snapshots
            .iter()
            .rev()
            .find(|snapshot| snapshot.frame <= frame)
            .map(|snapshot| snapshot.frame);
        let missing_input_frame = snapshot_frame.and_then(|snapshot_frame| {
            (snapshot_frame.saturating_add(1)..=frame)
                .find(|frame| self.cached_input_for_frame(*frame).is_none())
        });
        LockstepSimMismatchCoverage {
            frame,
            has_hash: self.hash_history.iter().any(|hash| hash.frame == frame),
            has_input: self.cached_input_for_frame(frame).is_some(),
            has_replay_inputs: snapshot_frame.is_some() && missing_input_frame.is_none(),
            missing_input_frame,
            has_snapshot_at_or_before: snapshot_frame.is_some(),
            snapshot_frame,
        }
    }

    fn cached_input_for_frame(&self, frame: u32) -> Option<&LockstepSimFrameInputs> {
        self.input_history.iter().find(|input| input.frame == frame)
    }
}

pub(in crate::game::features::lockstep_sim) fn reset_lockstep_sim_replay(
    state: &mut LockstepSimReplayState,
) {
    state.reset();
}

pub(in crate::game::features::lockstep_sim) fn apply_lockstep_sim_authority_events(
    config: Res<super::config::LockstepSimConfig>,
    scene_state: Res<LockstepSimSceneState>,
    mut events: MessageReader<AuthorityEvent>,
    mut replay_state: ResMut<LockstepSimReplayState>,
) {
    replay_state.debug_diagnostics = config.debug_diagnostics;

    if !scene_state.active {
        for _ in events.read() {}
        return;
    }

    let Some(snapshot) = scene_state.initial_snapshot.as_ref() else {
        for event in events.read() {
            if matches!(event, AuthorityEvent::FrameApplied { .. }) {
                replay_state.last_error = Some(LockstepSimReplayError::MissingInitialSnapshot);
            }
        }
        return;
    };

    replay_state.initialize_from_snapshot_if_needed(snapshot);

    for event in events.read() {
        let AuthorityEvent::FrameApplied { frame } = event else {
            continue;
        };

        if let Err(error) = apply_authority_frame(&mut replay_state, snapshot, frame) {
            warn!(
                frame = frame.frame_id,
                reason = %error,
                "lockstep sim replay frame rejected"
            );
            replay_state.last_error = Some(error);
        }
    }
}

fn apply_authority_frame(
    replay_state: &mut LockstepSimReplayState,
    snapshot: &ParsedInitialSnapshot,
    frame: &AuthorityFrame,
) -> Result<(), LockstepSimReplayError> {
    if let Some(last_applied_frame) = replay_state.last_applied_frame {
        if frame.frame_id <= last_applied_frame {
            replay_state.ignored_duplicate_or_old_frames = replay_state
                .ignored_duplicate_or_old_frames
                .saturating_add(1);
            debug!(
                frame = frame.frame_id,
                last_applied_frame, "ignored duplicate or out-of-order lockstep sim frame"
            );
            return Ok(());
        }

        let expected = last_applied_frame
            .checked_add(1)
            .ok_or(LockstepSimReplayError::FrameOverflow { last_applied_frame })?;
        if frame.frame_id != expected {
            return Err(LockstepSimReplayError::MissingFrame {
                expected,
                actual: frame.frame_id,
            });
        }
    }

    let expected_world_frame = frame.frame_id.saturating_sub(1);
    let actual_world_frame = replay_state
        .world
        .as_ref()
        .ok_or(LockstepSimReplayError::MissingReplayWorld)?
        .frame
        .raw();
    if actual_world_frame != expected_world_frame {
        return Err(LockstepSimReplayError::WorldFrameDiscontinuous {
            expected_world_frame,
            actual_world_frame,
        });
    }

    let sim_inputs = sim_inputs_from_frame(frame, snapshot)?;
    let server_hash = server_hash_from_game_state(&frame.snapshot.game_state_json, frame.frame_id);
    let config = replay_state
        .config
        .as_ref()
        .ok_or(LockstepSimReplayError::MissingReplayConfig)?;
    let world = replay_state
        .world
        .as_mut()
        .ok_or(LockstepSimReplayError::MissingReplayWorld)?;
    let result = step(world, FrameId::new(frame.frame_id), &sim_inputs, config)
        .map_err(LockstepSimReplayError::Step)?;

    replay_state.record_inputs(frame, sim_inputs);
    replay_state.record_hash(frame.frame_id, &result, server_hash.clone());
    replay_state.record_events(frame.frame_id, &result.events);
    replay_state.record_periodic_world_snapshot(frame.frame_id)?;
    let hash_status = replay_state.diagnostics.record_frame(
        frame.frame_id,
        result.state_hash,
        server_hash.as_ref(),
        replay_state
            .world
            .as_ref()
            .ok_or(LockstepSimReplayError::MissingReplayWorld)?,
    );
    replay_state.last_applied_frame = Some(frame.frame_id);
    replay_state.last_error = None;

    let local_hash_hex = format!("{:016x}", result.state_hash.value);
    let server_hash_label = server_hash
        .as_ref()
        .map(|hash| hash.hex.as_str())
        .unwrap_or("none");
    let diff_summary = replay_state
        .diagnostics
        .first_mismatch
        .as_ref()
        .filter(|mismatch| mismatch.frame == frame.frame_id)
        .map(|mismatch| mismatch.summary())
        .unwrap_or_else(|| "none".to_string());

    if matches!(hash_status, LockstepSimHashMatchStatus::Mismatch) {
        warn!(
            frame = frame.frame_id,
            local_hash = %local_hash_hex,
            server_hash = %server_hash_label,
            hash_status = %hash_status.as_str(),
            event_count = result.events.len(),
            diff_summary = %diff_summary,
            "lockstep sim replay hash mismatch"
        );
    } else if replay_state.debug_diagnostics {
        debug!(
            frame = frame.frame_id,
            local_hash = %local_hash_hex,
            server_hash = %server_hash_label,
            hash_status = %hash_status.as_str(),
            event_count = result.events.len(),
            diff_summary = %diff_summary,
            "lockstep sim replay frame applied"
        );
    }

    Ok(())
}

fn sim_inputs_from_frame(
    frame: &AuthorityFrame,
    snapshot: &ParsedInitialSnapshot,
) -> Result<Vec<SimInput>, LockstepSimReplayError> {
    let mut sim_inputs = Vec::new();
    for input in &frame.inputs {
        sim_inputs.extend(sim_inputs_from_player_input(input, snapshot)?);
    }

    Ok(sim_inputs)
}

fn sim_inputs_from_player_input(
    input: &PlayerInput,
    snapshot: &ParsedInitialSnapshot,
) -> Result<Vec<SimInput>, LockstepSimReplayError> {
    if input.action != SIM_INPUT_ACTION {
        debug!(
            frame = input.frame_id,
            player_id = %input.player_id,
            action = %input.action,
            "ignored non sim_input lockstep authority input"
        );
        return Ok(Vec::new());
    }

    let Some(entity_id) = snapshot.control_bindings.get(&input.player_id).copied() else {
        debug!(
            frame = input.frame_id,
            player_id = %input.player_id,
            "ignored lockstep sim authority input without control binding"
        );
        return Ok(Vec::new());
    };
    let payload = parse_sim_input_payload(&input.payload_json)
        .map_err(LockstepSimReplayError::InputPayload)?;

    Ok(payload
        .commands
        .into_iter()
        .map(|command| SimInput {
            frame: FrameId::new(input.frame_id),
            character_id: input.player_id.clone(),
            entity_id,
            seq: payload.seq,
            source: SimInputSource::Real,
            command,
        })
        .collect())
}

fn parse_sim_input_payload(
    payload_json: &str,
) -> Result<ParsedSimInputPayload, LockstepSimReplayInputError> {
    if payload_json.len() > SIM_INPUT_PAYLOAD_MAX_BYTES {
        return Err(LockstepSimReplayInputError::PayloadTooLarge {
            bytes: payload_json.len(),
            max: SIM_INPUT_PAYLOAD_MAX_BYTES,
        });
    }

    let payload = serde_json::from_str::<RawSimInputPayload>(payload_json)
        .map_err(|_| LockstepSimReplayInputError::InvalidJson)?;
    if payload.version != SIM_INPUT_VERSION {
        return Err(LockstepSimReplayInputError::UnsupportedVersion {
            actual: payload.version,
        });
    }
    if payload.commands.len() > SIM_INPUT_MAX_COMMANDS {
        return Err(LockstepSimReplayInputError::TooManyCommands {
            count: payload.commands.len(),
            max: SIM_INPUT_MAX_COMMANDS,
        });
    }

    let commands = payload
        .commands
        .into_iter()
        .map(RawSimCommand::into_sim_command)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ParsedSimInputPayload {
        seq: payload.seq,
        commands,
    })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSimInputPayload {
    version: u32,
    seq: u32,
    commands: Vec<RawSimCommand>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
enum RawSimCommand {
    Move {
        #[serde(rename = "dirX")]
        dir_x: i16,
        #[serde(rename = "dirY")]
        dir_y: i16,
        #[serde(default)]
        speed: Option<i64>,
    },
    Stop {},
    Face {
        #[serde(rename = "dirX")]
        dir_x: i16,
        #[serde(rename = "dirY")]
        dir_y: i16,
    },
    CastSkill {
        #[serde(rename = "skillId")]
        skill_id: u32,
        #[serde(rename = "targetEntityId", default)]
        target_entity_id: Option<u32>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedSimInputPayload {
    seq: u32,
    commands: Vec<SimCommand>,
}

impl RawSimCommand {
    fn into_sim_command(self) -> Result<SimCommand, LockstepSimReplayInputError> {
        match self {
            Self::Move {
                dir_x,
                dir_y,
                speed,
            } => {
                let dir = QuantizedDir::new(dir_x, dir_y)
                    .map_err(|_| LockstepSimReplayInputError::DirectionOutOfRange)?;
                if dir == QuantizedDir::ZERO {
                    return Err(LockstepSimReplayInputError::MoveDirectionZero);
                }
                let speed_per_second = speed
                    .map(|raw_milli| {
                        if !(1..=SIM_INPUT_MAX_SPEED_MILLI).contains(&raw_milli) {
                            return Err(LockstepSimReplayInputError::MoveSpeedOutOfRange {
                                raw_milli,
                            });
                        }
                        Ok(Fp::from_milli(raw_milli))
                    })
                    .transpose()?;
                Ok(SimCommand::Move(MoveCommand {
                    dir,
                    speed_per_second,
                }))
            }
            Self::Stop {} => Ok(SimCommand::Stop),
            Self::Face { dir_x, dir_y } => {
                let dir = QuantizedDir::new(dir_x, dir_y)
                    .map_err(|_| LockstepSimReplayInputError::DirectionOutOfRange)?;
                Ok(SimCommand::Face(FaceCommand { dir }))
            }
            Self::CastSkill {
                skill_id,
                target_entity_id,
            } => {
                if skill_id == 0 {
                    return Err(LockstepSimReplayInputError::SkillIdOutOfRange);
                }
                let target = match target_entity_id {
                    Some(0) => return Err(LockstepSimReplayInputError::TargetEntityIdOutOfRange),
                    Some(target_entity_id) => SkillTarget::Entity(EntityId::new(target_entity_id)),
                    None => SkillTarget::None,
                };
                Ok(SimCommand::CastSkill(CastSkillCommand {
                    skill_id: SkillId::new(skill_id),
                    target,
                }))
            }
        }
    }
}

fn server_hash_from_game_state(game_state_json: &str, frame_id: u32) -> Option<SimHashEnvelope> {
    let value = serde_json::from_str::<Value>(game_state_json).ok()?;

    hash_from_frame_envelope(value.pointer("/observerFrame/lastFrame"), Some(frame_id))
        .or_else(|| hash_from_frame_envelope(value.get("lastFrame"), Some(frame_id)))
        .or_else(|| {
            hash_from_envelope_value(value.pointer("/observerFrame/stateHash"), Some(frame_id))
        })
        .or_else(|| hash_from_envelope_value(value.get("lastStateHash"), Some(frame_id)))
}

fn hash_from_frame_envelope(
    value: Option<&Value>,
    expected_frame: Option<u32>,
) -> Option<SimHashEnvelope> {
    let envelope = value?;
    if envelope.is_null() {
        return None;
    }
    if expected_frame.is_none_or(|frame| {
        envelope
            .get("frame")
            .and_then(Value::as_u64)
            .is_some_and(|actual| actual == u64::from(frame))
    }) {
        return hash_from_envelope_value(envelope.get("stateHash"), expected_frame);
    }

    None
}

fn hash_from_envelope_value(
    value: Option<&Value>,
    expected_frame: Option<u32>,
) -> Option<SimHashEnvelope> {
    let value = value?;
    if value.is_null() {
        return None;
    }
    let frame = value
        .get("frame")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())?;
    if expected_frame.is_some_and(|expected| frame != expected) {
        return None;
    }
    let hash_value = value.get("value").and_then(Value::as_u64)?;
    let hex = value
        .get("hex")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("{hash_value:016x}"));

    Some(SimHashEnvelope {
        frame,
        value: hash_value,
        hex,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::authority::AuthoritySnapshot;
    use serde_json::json;
    use sim_core::{
        CombatConfig, CombatEffect, CombatState, DamageFormula, EntityKind, MovementConfig,
        MovementMode, MovementState, SceneBounds, SimConfig, SimEntity, SimRngState, SimTransform,
        SkillDefinition, SkillSlot, SkillTargetType, TeamId, Vec2Fp, hash_world,
    };
    use std::collections::HashMap;

    const PLAYER_ID: &str = "player-a";
    const PLAYER_ENTITY_ID: u32 = 1000;
    const TARGET_ENTITY_ID: u32 = 9000;

    #[test]
    fn replay_move_frame_matches_offline_step_hash() {
        let snapshot = parsed_snapshot_fixture();
        let frame = authority_frame(
            1,
            vec![player_input(
                1,
                PLAYER_ID,
                r#"{"version":1,"seq":7,"commands":[{"type":"move","dirX":1000,"dirY":0}]}"#,
            )],
            "{}",
        );
        let mut replay = replay_state_from_snapshot(&snapshot);

        apply_authority_frame(&mut replay, &snapshot, &frame).unwrap();

        let mut offline_world = snapshot.world.clone();
        let offline_result = step(
            &mut offline_world,
            FrameId::new(1),
            &[SimInput {
                frame: FrameId::new(1),
                character_id: PLAYER_ID.to_string(),
                entity_id: EntityId::new(PLAYER_ENTITY_ID),
                seq: 7,
                source: SimInputSource::Real,
                command: SimCommand::Move(MoveCommand {
                    dir: QuantizedDir::RIGHT,
                    speed_per_second: None,
                }),
            }],
            &snapshot.config,
        )
        .unwrap();

        assert_eq!(replay.last_applied_frame, Some(1));
        assert_eq!(replay.world.as_ref().unwrap(), &offline_world);
        assert_eq!(
            replay.hash_history.back().unwrap().local_hash,
            offline_result.state_hash
        );
    }

    #[test]
    fn replay_cast_skill_payload_steps_without_result_fields() {
        let snapshot = parsed_snapshot_fixture();
        let frame = authority_frame(
            1,
            vec![player_input(
                1,
                PLAYER_ID,
                r#"{"version":1,"seq":8,"commands":[{"type":"castSkill","skillId":1,"targetEntityId":9000}]}"#,
            )],
            "{}",
        );
        let mut replay = replay_state_from_snapshot(&snapshot);

        apply_authority_frame(&mut replay, &snapshot, &frame).unwrap();

        let hash = replay.hash_history.back().unwrap();
        assert_eq!(hash.frame, 1);
        assert_eq!(hash.event_count, 2);
        assert_eq!(replay.event_history.back().unwrap().frame, 1);
        assert_eq!(replay.event_history.back().unwrap().events.len(), 2);
        assert_ne!(hash.local_hash.value, 0);

        let target = replay
            .world
            .as_ref()
            .unwrap()
            .entity(EntityId::new(TARGET_ENTITY_ID))
            .unwrap();
        assert_eq!(target.combat.hp, 136);
    }

    #[test]
    fn replay_rejects_payload_with_result_fields() {
        let error = parse_sim_input_payload(
            r#"{"version":1,"seq":1,"commands":[{"type":"stop","stateHash":"0000000000000000"}]}"#,
        )
        .unwrap_err();

        assert_eq!(error, LockstepSimReplayInputError::InvalidJson);
    }

    #[test]
    fn duplicate_and_out_of_order_frames_are_ignored_missing_frame_errors() {
        let snapshot = parsed_snapshot_fixture();
        let frame1 = authority_frame(1, Vec::new(), "{}");
        let duplicate = authority_frame(1, Vec::new(), "{}");
        let skipped = authority_frame(3, Vec::new(), "{}");
        let mut replay = replay_state_from_snapshot(&snapshot);

        apply_authority_frame(&mut replay, &snapshot, &frame1).unwrap();
        apply_authority_frame(&mut replay, &snapshot, &duplicate).unwrap();
        let error = apply_authority_frame(&mut replay, &snapshot, &skipped).unwrap_err();

        assert_eq!(replay.last_applied_frame, Some(1));
        assert_eq!(replay.ignored_duplicate_or_old_frames, 1);
        assert_eq!(
            error,
            LockstepSimReplayError::MissingFrame {
                expected: 2,
                actual: 3
            }
        );
    }

    #[test]
    fn world_frame_discontinuity_errors_before_step() {
        let snapshot = parsed_snapshot_fixture();
        let frame = authority_frame(1, Vec::new(), "{}");
        let mut replay = replay_state_from_snapshot(&snapshot);
        replay.world.as_mut().unwrap().frame = FrameId::new(5);

        let error = apply_authority_frame(&mut replay, &snapshot, &frame).unwrap_err();

        assert_eq!(
            error,
            LockstepSimReplayError::WorldFrameDiscontinuous {
                expected_world_frame: 0,
                actual_world_frame: 5
            }
        );
    }

    #[test]
    fn records_local_and_server_hash_from_game_state_json() {
        let snapshot = parsed_snapshot_fixture();
        let mut offline_world = snapshot.world.clone();
        let result = step(&mut offline_world, FrameId::new(1), &[], &snapshot.config).unwrap();
        let server_hash = SimHashEnvelope {
            frame: 1,
            value: result.state_hash.value,
            hex: format!("{:016x}", result.state_hash.value),
        };
        let game_state_json = json!({
            "lastFrame": {
                "frame": 1,
                "stateHash": {
                    "frame": 1,
                    "value": 111,
                    "hex": "000000000000006f"
                }
            },
            "observerFrame": {
                "worldFrame": 1,
                "stateHash": server_hash,
                "lastFrame": {
                    "frame": 1,
                    "stateHash": server_hash
                }
            },
            "lastStateHash": {
                "frame": 1,
                "value": 222,
                "hex": "00000000000000de"
            }
        })
        .to_string();
        let frame = authority_frame(1, Vec::new(), &game_state_json);
        let mut replay = replay_state_from_snapshot(&snapshot);

        apply_authority_frame(&mut replay, &snapshot, &frame).unwrap();

        let recorded = replay.hash_history.back().unwrap();
        assert_eq!(recorded.local_hash, result.state_hash);
        assert_eq!(recorded.server_hash.as_ref(), Some(&server_hash));
    }

    #[test]
    fn server_hash_parser_uses_last_frame_match_then_fallbacks() {
        let matched = server_hash_from_game_state(
            &json!({
                "observerFrame": {
                    "lastFrame": {
                        "frame": 2,
                        "stateHash": {
                            "frame": 2,
                            "value": 20,
                            "hex": "0000000000000014"
                        }
                    }
                },
                "lastFrame": {
                    "frame": 1,
                    "stateHash": {
                        "frame": 1,
                        "value": 10,
                        "hex": "000000000000000a"
                    }
                },
                "lastStateHash": {
                    "frame": 2,
                    "value": 30,
                    "hex": "000000000000001e"
                }
            })
            .to_string(),
            2,
        );
        assert_eq!(
            matched,
            Some(SimHashEnvelope {
                frame: 2,
                value: 20,
                hex: "0000000000000014".to_string()
            })
        );

        let fallback = server_hash_from_game_state(
            &json!({
                "observerFrame": {
                    "lastFrame": {
                        "frame": 99,
                        "stateHash": {
                            "frame": 99,
                            "value": 99,
                            "hex": "0000000000000063"
                        }
                    }
                },
                "lastStateHash": {
                    "frame": 2,
                    "value": 30,
                    "hex": "000000000000001e"
                }
            })
            .to_string(),
            2,
        );
        assert_eq!(
            fallback,
            Some(SimHashEnvelope {
                frame: 2,
                value: 30,
                hex: "000000000000001e".to_string()
            })
        );
    }

    #[test]
    fn system_consumes_authority_frame_after_snapshot() {
        let mut app = App::new();
        app.add_message::<AuthorityEvent>()
            .init_resource::<super::super::config::LockstepSimConfig>()
            .init_resource::<LockstepSimSceneState>()
            .init_resource::<LockstepSimReplayState>()
            .add_systems(Update, apply_lockstep_sim_authority_events);
        let snapshot = parsed_snapshot_fixture();
        {
            let mut scene_state = app.world_mut().resource_mut::<LockstepSimSceneState>();
            scene_state.active = true;
            scene_state.initial_snapshot = Some(snapshot.clone());
        }

        app.world_mut().write_message(AuthorityEvent::FrameApplied {
            frame: authority_frame(1, Vec::new(), "{}"),
        });
        app.update();

        let replay = app.world().resource::<LockstepSimReplayState>();
        assert_eq!(replay.last_applied_frame, Some(1));
        assert!(replay.last_error.is_none());
        assert_eq!(replay.hash_history.len(), 1);
    }

    #[test]
    fn replay_records_first_hash_mismatch_for_hud_and_diagnostics() {
        let snapshot = parsed_snapshot_fixture();
        let frame = authority_frame(
            1,
            Vec::new(),
            &json!({
                "lastStateHash": {
                    "frame": 1,
                    "value": 1,
                    "hex": "0000000000000001"
                }
            })
            .to_string(),
        );
        let mut replay = replay_state_from_snapshot(&snapshot);

        apply_authority_frame(&mut replay, &snapshot, &frame).unwrap();

        assert_eq!(
            replay.diagnostics.last_match_status,
            LockstepSimHashMatchStatus::Mismatch
        );
        let mismatch = replay.diagnostics.first_mismatch.as_ref().unwrap();
        assert_eq!(mismatch.frame, 1);
        assert_eq!(mismatch.server_hash, "1:0000000000000001");
        assert!(mismatch.local_hash.starts_with("1:"));
        assert!(mismatch.entity_summary.contains("id=1000"));
        assert!(mismatch.entity_summary.contains("owner=player-a"));
        assert!(mismatch.entity_summary.contains("hp=100/100"));
    }

    #[test]
    fn replay_records_authoritative_inputs_hashes_and_periodic_snapshots() {
        let snapshot = parsed_snapshot_fixture();
        let mut replay = replay_state_from_snapshot(&snapshot);

        for frame_id in 1..=REPLAY_WORLD_SNAPSHOT_INTERVAL {
            apply_authority_frame(
                &mut replay,
                &snapshot,
                &authority_frame(frame_id, Vec::new(), "{}"),
            )
            .unwrap();
        }

        assert_eq!(
            replay.input_history.len(),
            REPLAY_WORLD_SNAPSHOT_INTERVAL as usize
        );
        let first_input = replay.input_history.front().unwrap();
        assert_eq!(first_input.frame, 1);
        assert_eq!(first_input.raw_input_count, 0);
        assert_eq!(first_input.sim_action_count, 0);
        assert_eq!(first_input.sim_command_count, 0);
        assert!(first_input.sim_inputs.is_empty());
        assert_eq!(
            replay.hash_history.len(),
            REPLAY_WORLD_SNAPSHOT_INTERVAL as usize
        );
        assert_eq!(
            replay
                .hash_history
                .back()
                .map(|hash| (hash.frame, hash.local_hash)),
            Some((
                REPLAY_WORLD_SNAPSHOT_INTERVAL,
                hash_world(replay.world.as_ref().unwrap())
            ))
        );
        assert_eq!(
            replay
                .world_snapshots
                .iter()
                .map(|snapshot| snapshot.frame)
                .collect::<Vec<_>>(),
            vec![0, REPLAY_WORLD_SNAPSHOT_INTERVAL]
        );
        let latest_snapshot = replay.world_snapshots.back().unwrap();
        assert_eq!(latest_snapshot.world, *replay.world.as_ref().unwrap());
        assert_eq!(latest_snapshot.hash, hash_world(&latest_snapshot.world));
    }

    #[test]
    fn replay_from_cached_snapshot_matches_continuous_world_and_keeps_live_world() {
        let snapshot = parsed_snapshot_fixture();
        let mut replay = replay_state_from_snapshot(&snapshot);
        for frame_id in 1..=25 {
            let inputs = if frame_id == 21 {
                vec![player_input(
                    frame_id,
                    PLAYER_ID,
                    r#"{"version":1,"seq":21,"commands":[{"type":"move","dirX":1000,"dirY":0}]}"#,
                )]
            } else if frame_id == 23 {
                vec![player_input(
                    frame_id,
                    PLAYER_ID,
                    r#"{"version":1,"seq":23,"commands":[{"type":"stop"}]}"#,
                )]
            } else {
                Vec::new()
            };
            apply_authority_frame(
                &mut replay,
                &snapshot,
                &authority_frame(frame_id, inputs, "{}"),
            )
            .unwrap();
        }
        let live_before = replay.world.clone();

        let (replayed_world, summary) = replay.replay_from_cached_snapshot_to_frame(25).unwrap();

        assert_eq!(summary.snapshot_frame, 20);
        assert_eq!(summary.target_frame, 25);
        assert_eq!(summary.replayed_frames, 5);
        assert_eq!(replayed_world, *live_before.as_ref().unwrap());
        assert_eq!(
            summary.final_hash,
            replay.hash_history.back().unwrap().local_hash
        );
        assert_eq!(replay.world, live_before);
    }

    #[test]
    fn replay_cache_coverage_reports_mismatch_supporting_data() {
        let snapshot = parsed_snapshot_fixture();
        let mut replay = replay_state_from_snapshot(&snapshot);
        for frame_id in 1..=12 {
            apply_authority_frame(
                &mut replay,
                &snapshot,
                &authority_frame(frame_id, Vec::new(), "{}"),
            )
            .unwrap();
        }

        let coverage = replay.mismatch_coverage(12);

        assert_eq!(
            coverage,
            LockstepSimMismatchCoverage {
                frame: 12,
                has_hash: true,
                has_input: true,
                has_replay_inputs: true,
                missing_input_frame: None,
                has_snapshot_at_or_before: true,
                snapshot_frame: Some(10),
            }
        );
        assert!(replay.replay_from_cached_snapshot_to_frame(12).is_ok());
    }

    #[test]
    fn replay_cache_coverage_reports_missing_intermediate_replay_input() {
        let snapshot = parsed_snapshot_fixture();
        let mut replay = replay_state_from_snapshot(&snapshot);
        for frame_id in 1..=12 {
            apply_authority_frame(
                &mut replay,
                &snapshot,
                &authority_frame(frame_id, Vec::new(), "{}"),
            )
            .unwrap();
        }
        let missing_index = replay
            .input_history
            .iter()
            .position(|entry| entry.frame == 11)
            .unwrap();
        replay.input_history.remove(missing_index);

        let coverage = replay.mismatch_coverage(12);

        assert_eq!(
            coverage,
            LockstepSimMismatchCoverage {
                frame: 12,
                has_hash: true,
                has_input: true,
                has_replay_inputs: false,
                missing_input_frame: Some(11),
                has_snapshot_at_or_before: true,
                snapshot_frame: Some(10),
            }
        );
        assert_eq!(
            replay.replay_from_cached_snapshot_to_frame(12).unwrap_err(),
            LockstepSimReplayCacheError::MissingInput { frame: 11 }
        );
    }

    #[test]
    fn replay_cache_enforces_history_limits_and_snapshot_interval() {
        let snapshot = parsed_snapshot_fixture();
        let mut replay = replay_state_from_snapshot(&snapshot);
        for frame_id in 1..=650 {
            apply_authority_frame(
                &mut replay,
                &snapshot,
                &authority_frame(frame_id, Vec::new(), "{}"),
            )
            .unwrap();
        }

        assert_eq!(replay.input_history.len(), REPLAY_INPUT_HISTORY_LIMIT);
        assert_eq!(replay.input_history.front().unwrap().frame, 139);
        assert_eq!(replay.input_history.back().unwrap().frame, 650);
        assert_eq!(replay.hash_history.len(), REPLAY_HASH_HISTORY_LIMIT);
        assert_eq!(replay.hash_history.front().unwrap().frame, 139);
        assert_eq!(replay.hash_history.back().unwrap().frame, 650);
        assert_eq!(replay.world_snapshots.len(), REPLAY_WORLD_SNAPSHOT_LIMIT);
        assert_eq!(replay.world_snapshots.front().unwrap().frame, 20);
        assert_eq!(replay.world_snapshots.back().unwrap().frame, 650);
        assert!(
            replay
                .world_snapshots
                .iter()
                .all(|snapshot| snapshot.frame % REPLAY_WORLD_SNAPSHOT_INTERVAL == 0)
        );
    }

    fn replay_state_from_snapshot(snapshot: &ParsedInitialSnapshot) -> LockstepSimReplayState {
        let mut state = LockstepSimReplayState::default();
        state.initialize_from_snapshot_if_needed(snapshot);
        state
    }

    fn authority_frame(
        frame_id: u32,
        inputs: Vec<PlayerInput>,
        game_state_json: &str,
    ) -> AuthorityFrame {
        AuthorityFrame {
            authority_epoch: 1,
            frame_id,
            fps: 20,
            inputs,
            snapshot: AuthoritySnapshot {
                authority_epoch: 1,
                frame_id,
                authority_player_id: PLAYER_ID.to_string(),
                players: vec![PLAYER_ID.to_string()],
                game_state_json: game_state_json.to_string(),
            },
        }
    }

    fn player_input(frame_id: u32, player_id: &str, payload_json: &str) -> PlayerInput {
        PlayerInput {
            player_id: player_id.to_string(),
            frame_id,
            action: SIM_INPUT_ACTION.to_string(),
            payload_json: payload_json.to_string(),
        }
    }

    fn parsed_snapshot_fixture() -> ParsedInitialSnapshot {
        let config = sim_config_fixture();
        let world = world_fixture();
        let mut control_bindings = HashMap::new();
        control_bindings.insert(PLAYER_ID.to_string(), EntityId::new(PLAYER_ENTITY_ID));
        let state_hash = hash_world(&world);

        ParsedInitialSnapshot {
            room_id: "lockstep-room".to_string(),
            start_frame: 0,
            tick_rate: 20,
            config_version: 1,
            config_hash: "fixture-config-hash".to_string(),
            sim_schema_version: sim_core::SIM_CORE_SCHEMA_VERSION,
            rng_seed: world.rng.seed,
            state_hash: super::super::snapshot::SimHashEnvelope {
                frame: state_hash.frame.raw(),
                value: state_hash.value,
                hex: format!("{:016x}", state_hash.value),
            },
            entities: world.entities_sorted_by_id().to_vec(),
            world,
            config,
            control_bindings,
        }
    }

    fn world_fixture() -> SimWorld {
        SimWorld::with_rng(
            FrameId::new(0),
            SimRngState {
                seed: 11,
                counter: 22,
            },
            vec![player_entity(), target_entity()],
        )
        .unwrap()
    }

    fn sim_config_fixture() -> SimConfig {
        SimConfig {
            movement: MovementConfig {
                tick_rate: 20,
                default_speed_per_second: Fp::from_i32(6),
                max_speed_per_second: Fp::from_i32(12),
                bounds: SceneBounds {
                    min: Vec2Fp::new(Fp::from_i32(-100), Fp::from_i32(-100)),
                    max: Vec2Fp::new(Fp::from_i32(100), Fp::from_i32(100)),
                },
                static_obstacles: Vec::new(),
            },
            combat: CombatConfig::from_definitions(
                vec![SkillDefinition {
                    id: SkillId::new(1),
                    cooldown_frames: 20,
                    cast_range: Fp::from_i32(12),
                    target_type: SkillTargetType::Enemy,
                    effects: vec![CombatEffect::Damage {
                        formula: DamageFormula::Fixed { amount: 15 },
                    }],
                }],
                Vec::new(),
            )
            .unwrap(),
        }
    }

    fn player_entity() -> SimEntity {
        SimEntity {
            id: EntityId::new(PLAYER_ENTITY_ID),
            kind: EntityKind::Player,
            owner_character_id: Some(PLAYER_ID.to_string()),
            team_id: TeamId::new(1),
            transform: SimTransform {
                pos: Vec2Fp::zero(),
                facing: QuantizedDir::RIGHT,
                radius: Fp::from_milli(500),
            },
            movement: MovementState {
                mode: MovementMode::Idle,
                move_dir: QuantizedDir::ZERO,
                speed_per_second: Fp::ZERO,
            },
            combat: CombatState {
                hp: 100,
                max_hp: 100,
                attack: 10,
                defense: 1,
                speed: 6,
                crit_rate_bps: 0,
                crit_damage_bps: 10_000,
                skill_slots: vec![SkillSlot {
                    skill_id: SkillId::new(1),
                    cooldown_remaining: 0,
                }],
                buffs: Vec::new(),
            },
            alive: true,
        }
    }

    fn target_entity() -> SimEntity {
        SimEntity {
            id: EntityId::new(TARGET_ENTITY_ID),
            kind: EntityKind::Monster,
            owner_character_id: None,
            team_id: TeamId::new(90),
            transform: SimTransform {
                pos: Vec2Fp::new(Fp::from_i32(8), Fp::ZERO),
                facing: QuantizedDir::LEFT,
                radius: Fp::from_milli(500),
            },
            movement: MovementState::default(),
            combat: CombatState {
                hp: 150,
                max_hp: 150,
                attack: 0,
                defense: 1,
                speed: 0,
                crit_rate_bps: 0,
                crit_damage_bps: 10_000,
                skill_slots: Vec::new(),
                buffs: Vec::new(),
            },
            alive: true,
        }
    }
}
