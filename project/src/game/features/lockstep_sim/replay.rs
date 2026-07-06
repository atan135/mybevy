use std::collections::VecDeque;
use std::fmt;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sim_core::{
    CastSkillCommand, EntityId, FaceCommand, Fp, FrameId, MoveCommand, QuantizedDir, SimCommand,
    SimConfig, SimHash, SimInput, SimInputSource, SimStepResult, SimWorld, SkillId, SkillTarget,
    StepError, step,
};

use crate::game::authority::{AuthorityEvent, AuthorityFrame, PlayerInput};

use super::{
    payload::{
        SIM_INPUT_ACTION, SIM_INPUT_MAX_COMMANDS, SIM_INPUT_PAYLOAD_MAX_BYTES, SIM_INPUT_VERSION,
    },
    snapshot::ParsedInitialSnapshot,
    state::LockstepSimSceneState,
};

const REPLAY_HASH_HISTORY_LIMIT: usize = 512;
const SIM_INPUT_MAX_SPEED_MILLI: i64 = 12_000;

#[derive(Clone, Debug, Default, Resource, PartialEq, Eq)]
pub(in crate::game) struct LockstepSimReplayState {
    pub(in crate::game::features::lockstep_sim) world: Option<SimWorld>,
    pub(in crate::game::features::lockstep_sim) config: Option<SimConfig>,
    pub(in crate::game::features::lockstep_sim) snapshot_start_frame: Option<u32>,
    pub(in crate::game::features::lockstep_sim) last_applied_frame: Option<u32>,
    pub(in crate::game::features::lockstep_sim) hash_history: VecDeque<LockstepSimFrameHash>,
    pub(in crate::game::features::lockstep_sim) last_error: Option<LockstepSimReplayError>,
    pub(in crate::game::features::lockstep_sim) ignored_duplicate_or_old_frames: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::game::features::lockstep_sim) struct LockstepSimFrameHash {
    pub(in crate::game::features::lockstep_sim) frame: u32,
    pub(in crate::game::features::lockstep_sim) local_hash: SimHash,
    pub(in crate::game::features::lockstep_sim) server_hash: Option<SimHashEnvelope>,
    pub(in crate::game::features::lockstep_sim) event_count: usize,
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
        self.hash_history.clear();
        self.last_error = None;
        self.ignored_duplicate_or_old_frames = 0;
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
}

pub(in crate::game::features::lockstep_sim) fn reset_lockstep_sim_replay(
    state: &mut LockstepSimReplayState,
) {
    state.reset();
}

pub(in crate::game::features::lockstep_sim) fn apply_lockstep_sim_authority_events(
    scene_state: Res<LockstepSimSceneState>,
    mut events: MessageReader<AuthorityEvent>,
    mut replay_state: ResMut<LockstepSimReplayState>,
) {
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

    replay_state.record_hash(frame.frame_id, &result, server_hash.clone());
    replay_state.last_applied_frame = Some(frame.frame_id);
    replay_state.last_error = None;

    if let Some(server_hash) = server_hash.as_ref() {
        let matched = server_hash.frame == result.state_hash.frame.raw()
            && server_hash.value == result.state_hash.value;
        debug!(
            frame = frame.frame_id,
            local_hash = %format!("{:016x}", result.state_hash.value),
            server_hash = %server_hash.hex,
            matched,
            event_count = result.events.len(),
            "lockstep sim replay frame applied"
        );
    } else {
        debug!(
            frame = frame.frame_id,
            local_hash = %format!("{:016x}", result.state_hash.value),
            event_count = result.events.len(),
            "lockstep sim replay frame applied without server hash"
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
