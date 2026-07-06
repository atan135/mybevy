#![allow(dead_code)]

use std::fmt;

use bevy::prelude::*;
use serde::Serialize;
use sim_core::{EntityId, Fp, QuantizedDir, SimCommand, SkillTarget};

use crate::game::authority::AuthorityCommand;

use super::state::LockstepSimSceneState;

pub(in crate::game::features::lockstep_sim) const SIM_INPUT_ACTION: &str = "sim_input";
pub(in crate::game::features::lockstep_sim) const SIM_INPUT_VERSION: u32 = 1;
pub(in crate::game::features::lockstep_sim) const SIM_INPUT_PAYLOAD_MAX_BYTES: usize = 2048;
pub(in crate::game::features::lockstep_sim) const SIM_INPUT_MAX_COMMANDS: usize = 8;
const SIM_INPUT_MAX_SPEED_MILLI: i64 = 12_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::game::features::lockstep_sim) enum LockstepSimPayloadError {
    EmptyCommands,
    TooManyCommands { count: usize, max: usize },
    PayloadTooLarge { bytes: usize, max: usize },
    UnsupportedCommand { command_type: &'static str },
    MoveDirectionZero,
    MoveSpeedOutOfRange { raw_milli: i64 },
    SkillIdOutOfRange,
    TargetEntityIdOutOfRange,
    UnsupportedSkillTarget { target_type: &'static str },
    Serialize(String),
}

impl fmt::Display for LockstepSimPayloadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCommands => formatter.write_str("lockstep sim input command list is empty"),
            Self::TooManyCommands { count, max } => write!(
                formatter,
                "lockstep sim input command count {count} exceeds max {max}"
            ),
            Self::PayloadTooLarge { bytes, max } => write!(
                formatter,
                "lockstep sim input payload size {bytes} exceeds max {max}"
            ),
            Self::UnsupportedCommand { command_type } => {
                write!(
                    formatter,
                    "lockstep sim command {command_type} is not supported on the online wire"
                )
            }
            Self::MoveDirectionZero => {
                formatter.write_str("lockstep sim move command has zero direction")
            }
            Self::MoveSpeedOutOfRange { raw_milli } => write!(
                formatter,
                "lockstep sim move speed {raw_milli} is outside server range"
            ),
            Self::SkillIdOutOfRange => {
                formatter.write_str("lockstep sim castSkill skillId must be positive")
            }
            Self::TargetEntityIdOutOfRange => formatter
                .write_str("lockstep sim castSkill targetEntityId must be non-zero when present"),
            Self::UnsupportedSkillTarget { target_type } => write!(
                formatter,
                "lockstep sim castSkill target {target_type} is not supported on the online wire"
            ),
            Self::Serialize(message) => {
                write!(
                    formatter,
                    "lockstep sim input payload serialization failed: {message}"
                )
            }
        }
    }
}

impl std::error::Error for LockstepSimPayloadError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::game::features::lockstep_sim) enum LockstepSimInputGateError {
    InactiveScene,
    SnapshotError,
    MissingInitialSnapshot,
    MissingLocalPlayerId,
    MissingControlBinding { character_id: String },
    ConfigVersionMismatch { expected: u64, actual: u64 },
    ConfigHashMismatch { expected: String, actual: String },
    SimSchemaVersionMismatch { expected: u16, actual: u16 },
}

impl fmt::Display for LockstepSimInputGateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InactiveScene => formatter.write_str("lockstep sim scene is inactive"),
            Self::SnapshotError => {
                formatter.write_str("lockstep sim initial snapshot is in an error state")
            }
            Self::MissingInitialSnapshot => {
                formatter.write_str("lockstep sim initial snapshot has not been received")
            }
            Self::MissingLocalPlayerId => {
                formatter.write_str("lockstep sim local player id is missing")
            }
            Self::MissingControlBinding { character_id } => write!(
                formatter,
                "lockstep sim local player {character_id} has no control binding"
            ),
            Self::ConfigVersionMismatch { expected, actual } => write!(
                formatter,
                "lockstep sim config version mismatch: expected {expected}, got {actual}"
            ),
            Self::ConfigHashMismatch { expected, actual } => write!(
                formatter,
                "lockstep sim config hash mismatch: expected {expected}, got {actual}"
            ),
            Self::SimSchemaVersionMismatch { expected, actual } => write!(
                formatter,
                "lockstep sim schema version mismatch: expected {expected}, got {actual}"
            ),
        }
    }
}

impl std::error::Error for LockstepSimInputGateError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::game::features::lockstep_sim) struct LockstepSimInputContext {
    pub(in crate::game::features::lockstep_sim) character_id: String,
    pub(in crate::game::features::lockstep_sim) entity_id: EntityId,
    pub(in crate::game::features::lockstep_sim) config_version: u64,
    pub(in crate::game::features::lockstep_sim) config_hash: String,
    pub(in crate::game::features::lockstep_sim) sim_schema_version: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::game::features::lockstep_sim) struct LockstepSimInputEnvelope {
    pub(in crate::game::features::lockstep_sim) frame_id: u32,
    pub(in crate::game::features::lockstep_sim) seq: u32,
    pub(in crate::game::features::lockstep_sim) command_summaries: Vec<SimInputCommandSummary>,
    pub(in crate::game::features::lockstep_sim) payload_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::game::features::lockstep_sim) struct SimInputCommandSummary {
    pub(in crate::game::features::lockstep_sim) command_type: &'static str,
    pub(in crate::game::features::lockstep_sim) dir: Option<QuantizedDir>,
    pub(in crate::game::features::lockstep_sim) target_entity_id: Option<u32>,
    pub(in crate::game::features::lockstep_sim) skill_id: Option<u32>,
}

impl LockstepSimInputEnvelope {
    pub(in crate::game::features::lockstep_sim) fn into_authority_command(
        self,
    ) -> AuthorityCommand {
        AuthorityCommand::SendInput {
            frame_id: self.frame_id,
            action: SIM_INPUT_ACTION.to_string(),
            payload_json: self.payload_json,
        }
    }
}

pub(in crate::game::features::lockstep_sim) fn gate_lockstep_sim_input(
    scene_state: &LockstepSimSceneState,
    local_player_id: Option<&str>,
    expected_config_version: Option<u64>,
    expected_config_hash: Option<&str>,
    expected_sim_schema_version: Option<u16>,
) -> Result<LockstepSimInputContext, LockstepSimInputGateError> {
    if !scene_state.active {
        return Err(LockstepSimInputGateError::InactiveScene);
    }
    if scene_state.initial_snapshot_error.is_some() {
        return Err(LockstepSimInputGateError::SnapshotError);
    }

    let snapshot = scene_state
        .initial_snapshot
        .as_ref()
        .ok_or(LockstepSimInputGateError::MissingInitialSnapshot)?;
    let character_id = local_player_id
        .filter(|player_id| !player_id.trim().is_empty())
        .ok_or(LockstepSimInputGateError::MissingLocalPlayerId)?;
    let entity_id = snapshot
        .control_bindings
        .get(character_id)
        .copied()
        .ok_or_else(|| LockstepSimInputGateError::MissingControlBinding {
            character_id: character_id.to_string(),
        })?;

    if let Some(expected) = expected_config_version {
        if snapshot.config_version != expected {
            return Err(LockstepSimInputGateError::ConfigVersionMismatch {
                expected,
                actual: snapshot.config_version,
            });
        }
    }
    if let Some(expected) = expected_config_hash {
        if snapshot.config_hash != expected {
            return Err(LockstepSimInputGateError::ConfigHashMismatch {
                expected: expected.to_string(),
                actual: snapshot.config_hash.clone(),
            });
        }
    }
    if let Some(expected) = expected_sim_schema_version {
        if snapshot.sim_schema_version != expected {
            return Err(LockstepSimInputGateError::SimSchemaVersionMismatch {
                expected,
                actual: snapshot.sim_schema_version,
            });
        }
    }

    Ok(LockstepSimInputContext {
        character_id: character_id.to_string(),
        entity_id,
        config_version: snapshot.config_version,
        config_hash: snapshot.config_hash.clone(),
        sim_schema_version: snapshot.sim_schema_version,
    })
}

pub(in crate::game::features::lockstep_sim) fn build_sim_input_envelope(
    frame_id: u32,
    seq: u32,
    commands: &[SimCommand],
) -> Result<LockstepSimInputEnvelope, LockstepSimPayloadError> {
    let (payload_json, command_summaries) = serialize_sim_input_payload(seq, commands)?;
    Ok(LockstepSimInputEnvelope {
        frame_id,
        seq,
        command_summaries,
        payload_json,
    })
}

pub(in crate::game::features::lockstep_sim) fn serialize_sim_input_payload(
    seq: u32,
    commands: &[SimCommand],
) -> Result<(String, Vec<SimInputCommandSummary>), LockstepSimPayloadError> {
    if commands.is_empty() {
        return Err(LockstepSimPayloadError::EmptyCommands);
    }
    if commands.len() > SIM_INPUT_MAX_COMMANDS {
        return Err(LockstepSimPayloadError::TooManyCommands {
            count: commands.len(),
            max: SIM_INPUT_MAX_COMMANDS,
        });
    }

    let mut wire_commands = Vec::with_capacity(commands.len());
    let mut summaries = Vec::with_capacity(commands.len());
    for command in commands {
        let (wire, summary) = wire_command(command)?;
        wire_commands.push(wire);
        summaries.push(summary);
    }

    let payload = SimInputPayload {
        version: SIM_INPUT_VERSION,
        seq,
        commands: wire_commands,
    };
    let payload_json = serde_json::to_string(&payload)
        .map_err(|error| LockstepSimPayloadError::Serialize(error.to_string()))?;
    if payload_json.len() > SIM_INPUT_PAYLOAD_MAX_BYTES {
        return Err(LockstepSimPayloadError::PayloadTooLarge {
            bytes: payload_json.len(),
            max: SIM_INPUT_PAYLOAD_MAX_BYTES,
        });
    }

    Ok((payload_json, summaries))
}

pub(in crate::game::features::lockstep_sim) fn log_sim_input_send(
    player_id: &str,
    frame_id: u32,
    seq: u32,
    summaries: &[SimInputCommandSummary],
) {
    for summary in summaries {
        match summary.command_type {
            "move" | "face" => {
                let dir = summary.dir.unwrap_or(QuantizedDir::ZERO);
                debug!(
                    player_id = %player_id,
                    frame = frame_id,
                    seq,
                    command_type = summary.command_type,
                    dir_x = dir.x(),
                    dir_y = dir.y(),
                    "sending lockstep sim input"
                );
            }
            "castSkill" => {
                debug!(
                    player_id = %player_id,
                    frame = frame_id,
                    seq,
                    command_type = summary.command_type,
                    skill_id = summary.skill_id.unwrap_or_default(),
                    target_entity_id = summary.target_entity_id.unwrap_or_default(),
                    "sending lockstep sim input"
                );
            }
            _ => {
                debug!(
                    player_id = %player_id,
                    frame = frame_id,
                    seq,
                    command_type = summary.command_type,
                    "sending lockstep sim input"
                );
            }
        }
    }
}

#[derive(Serialize)]
struct SimInputPayload {
    version: u32,
    seq: u32,
    commands: Vec<WireSimCommand>,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum WireSimCommand {
    Move {
        #[serde(rename = "dirX")]
        dir_x: i16,
        #[serde(rename = "dirY")]
        dir_y: i16,
        #[serde(skip_serializing_if = "Option::is_none")]
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
        #[serde(rename = "targetEntityId", skip_serializing_if = "Option::is_none")]
        target_entity_id: Option<u32>,
    },
}

fn wire_command(
    command: &SimCommand,
) -> Result<(WireSimCommand, SimInputCommandSummary), LockstepSimPayloadError> {
    match command {
        SimCommand::Move(command) => {
            if command.dir == QuantizedDir::ZERO {
                return Err(LockstepSimPayloadError::MoveDirectionZero);
            }
            let speed = command.speed_per_second.map(validate_speed).transpose()?;
            Ok((
                WireSimCommand::Move {
                    dir_x: command.dir.x(),
                    dir_y: command.dir.y(),
                    speed,
                },
                SimInputCommandSummary {
                    command_type: "move",
                    dir: Some(command.dir),
                    target_entity_id: None,
                    skill_id: None,
                },
            ))
        }
        SimCommand::Stop => Ok((
            WireSimCommand::Stop {},
            SimInputCommandSummary {
                command_type: "stop",
                dir: None,
                target_entity_id: None,
                skill_id: None,
            },
        )),
        SimCommand::Face(command) => Ok((
            WireSimCommand::Face {
                dir_x: command.dir.x(),
                dir_y: command.dir.y(),
            },
            SimInputCommandSummary {
                command_type: "face",
                dir: Some(command.dir),
                target_entity_id: None,
                skill_id: None,
            },
        )),
        SimCommand::CastSkill(command) => {
            let skill_id = command.skill_id.raw();
            if skill_id == 0 {
                return Err(LockstepSimPayloadError::SkillIdOutOfRange);
            }
            let target_entity_id = match command.target {
                SkillTarget::None => None,
                SkillTarget::Entity(entity_id) => {
                    let raw = entity_id.raw();
                    if raw == 0 {
                        return Err(LockstepSimPayloadError::TargetEntityIdOutOfRange);
                    }
                    Some(raw)
                }
                SkillTarget::Position(_) => {
                    return Err(LockstepSimPayloadError::UnsupportedSkillTarget {
                        target_type: "position",
                    });
                }
                SkillTarget::Direction(_) => {
                    return Err(LockstepSimPayloadError::UnsupportedSkillTarget {
                        target_type: "direction",
                    });
                }
            };

            Ok((
                WireSimCommand::CastSkill {
                    skill_id,
                    target_entity_id,
                },
                SimInputCommandSummary {
                    command_type: "castSkill",
                    dir: None,
                    target_entity_id,
                    skill_id: Some(skill_id),
                },
            ))
        }
        SimCommand::Noop => Err(LockstepSimPayloadError::UnsupportedCommand {
            command_type: "noop",
        }),
    }
}

fn validate_speed(speed: Fp) -> Result<i64, LockstepSimPayloadError> {
    let raw_milli = speed.raw();
    if !(1..=SIM_INPUT_MAX_SPEED_MILLI).contains(&raw_milli) {
        return Err(LockstepSimPayloadError::MoveSpeedOutOfRange { raw_milli });
    }
    Ok(raw_milli)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::features::lockstep_sim::snapshot::LockstepSimSnapshotError;
    use serde_json::{Value, json};
    use sim_core::{
        CastSkillCommand, FaceCommand, FrameId, MoveCommand, SimWorld, SkillId, Vec2Fp,
    };

    #[test]
    fn serializes_move_stop_face_and_cast_skill_with_server_field_names() {
        let commands = vec![
            SimCommand::Move(MoveCommand {
                dir: QuantizedDir::RIGHT,
                speed_per_second: Some(Fp::from_milli(6_000)),
            }),
            SimCommand::Stop,
            SimCommand::Face(FaceCommand {
                dir: QuantizedDir::UP,
            }),
            SimCommand::CastSkill(CastSkillCommand {
                skill_id: SkillId::new(1),
                target: SkillTarget::Entity(EntityId::new(9000)),
            }),
        ];

        let (payload_json, summaries) = serialize_sim_input_payload(7, &commands).unwrap();
        let payload: Value = serde_json::from_str(&payload_json).unwrap();

        assert_eq!(
            payload,
            json!({
                "version": 1,
                "seq": 7,
                "commands": [
                    { "type": "move", "dirX": 1000, "dirY": 0, "speed": 6000 },
                    { "type": "stop" },
                    { "type": "face", "dirX": 0, "dirY": -1000 },
                    { "type": "castSkill", "skillId": 1, "targetEntityId": 9000 }
                ]
            })
        );
        assert_eq!(
            payload
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["commands", "seq", "version"]
        );
        assert_eq!(
            summaries
                .iter()
                .map(|summary| summary.command_type)
                .collect::<Vec<_>>(),
            vec!["move", "stop", "face", "castSkill"]
        );
        assert_eq!(summaries[0].dir, Some(QuantizedDir::RIGHT));
        assert_eq!(summaries[3].target_entity_id, Some(9000));
    }

    #[test]
    fn omits_optional_speed_and_cast_skill_target_entity_id_when_absent() {
        let commands = vec![
            SimCommand::Move(MoveCommand {
                dir: QuantizedDir::LEFT,
                speed_per_second: None,
            }),
            SimCommand::CastSkill(CastSkillCommand {
                skill_id: SkillId::new(2),
                target: SkillTarget::None,
            }),
        ];

        let (payload_json, _) = serialize_sim_input_payload(8, &commands).unwrap();
        let payload: Value = serde_json::from_str(&payload_json).unwrap();

        assert!(payload["commands"][0].get("speed").is_none());
        assert!(payload["commands"][1].get("targetEntityId").is_none());
        assert_eq!(payload["commands"][1]["type"], "castSkill");
    }

    #[test]
    fn serialized_payload_contains_no_result_or_state_fields() {
        let (payload_json, _) =
            serialize_sim_input_payload(1, &[SimCommand::Stop]).expect("stop is serializable");
        let payload: Value = serde_json::from_str(&payload_json).unwrap();
        let forbidden = [
            "entityId",
            "hit",
            "damage",
            "buffs",
            "finalState",
            "stateHash",
        ];

        for field in forbidden {
            assert!(payload.get(field).is_none());
            assert!(payload["commands"][0].get(field).is_none());
        }
    }

    #[test]
    fn rejects_payloads_outside_server_limits() {
        let too_many = vec![SimCommand::Stop; SIM_INPUT_MAX_COMMANDS + 1];
        assert_eq!(
            serialize_sim_input_payload(1, &too_many),
            Err(LockstepSimPayloadError::TooManyCommands {
                count: SIM_INPUT_MAX_COMMANDS + 1,
                max: SIM_INPUT_MAX_COMMANDS
            })
        );
        assert_eq!(
            serialize_sim_input_payload(1, &[]),
            Err(LockstepSimPayloadError::EmptyCommands)
        );
        assert_eq!(
            serialize_sim_input_payload(
                1,
                &[SimCommand::Move(MoveCommand {
                    dir: QuantizedDir::ZERO,
                    speed_per_second: None,
                })],
            ),
            Err(LockstepSimPayloadError::MoveDirectionZero)
        );
        assert_eq!(
            serialize_sim_input_payload(
                1,
                &[SimCommand::Move(MoveCommand {
                    dir: QuantizedDir::RIGHT,
                    speed_per_second: Some(Fp::from_milli(12_001)),
                })],
            ),
            Err(LockstepSimPayloadError::MoveSpeedOutOfRange { raw_milli: 12_001 })
        );
    }

    #[test]
    fn rejects_online_unsupported_skill_targets_and_noop() {
        assert_eq!(
            serialize_sim_input_payload(
                1,
                &[SimCommand::CastSkill(CastSkillCommand {
                    skill_id: SkillId::new(1),
                    target: SkillTarget::Position(Vec2Fp::new(Fp::from_i32(1), Fp::ZERO)),
                })],
            ),
            Err(LockstepSimPayloadError::UnsupportedSkillTarget {
                target_type: "position"
            })
        );
        assert_eq!(
            serialize_sim_input_payload(
                1,
                &[SimCommand::CastSkill(CastSkillCommand {
                    skill_id: SkillId::new(1),
                    target: SkillTarget::Direction(QuantizedDir::RIGHT),
                })],
            ),
            Err(LockstepSimPayloadError::UnsupportedSkillTarget {
                target_type: "direction"
            })
        );
        assert_eq!(
            serialize_sim_input_payload(1, &[SimCommand::Noop]),
            Err(LockstepSimPayloadError::UnsupportedCommand {
                command_type: "noop"
            })
        );
    }

    #[test]
    fn generated_json_shape_matches_game_server_contract() {
        let commands = vec![SimCommand::Face(FaceCommand {
            dir: QuantizedDir::DOWN_LEFT,
        })];
        let (payload_json, _) = serialize_sim_input_payload(u32::MAX, &commands).unwrap();
        let payload: Value = serde_json::from_str(&payload_json).unwrap();

        assert!(payload_json.len() <= SIM_INPUT_PAYLOAD_MAX_BYTES);
        assert_eq!(payload["version"], SIM_INPUT_VERSION);
        assert_eq!(payload["seq"], u32::MAX);
        assert_eq!(payload["commands"].as_array().unwrap().len(), 1);
        assert_eq!(payload["commands"][0]["type"], "face");
        assert_eq!(payload["commands"][0]["dirX"], -707);
        assert_eq!(payload["commands"][0]["dirY"], 707);
        assert!(payload["commands"][0].get("dir_x").is_none());
        assert!(payload["commands"][0].get("dirY").is_some());
    }

    #[test]
    fn envelope_builds_authority_send_input_command() {
        let envelope = build_sim_input_envelope(42, 3, &[SimCommand::Stop]).unwrap();

        assert_eq!(envelope.frame_id, 42);
        assert_eq!(envelope.seq, 3);
        assert_eq!(envelope.command_summaries[0].command_type, "stop");
        assert!(matches!(
            envelope.into_authority_command(),
            AuthorityCommand::SendInput {
                frame_id: 42,
                action,
                payload_json
            } if action == SIM_INPUT_ACTION && payload_json.contains("\"type\":\"stop\"")
        ));
    }

    #[test]
    fn input_gate_requires_active_snapshot_and_local_control_binding() {
        let mut state = LockstepSimSceneState::default();
        assert_eq!(
            gate_lockstep_sim_input(&state, Some("chr_100"), None, None, None),
            Err(LockstepSimInputGateError::InactiveScene)
        );

        state.active = true;
        assert_eq!(
            gate_lockstep_sim_input(&state, Some("chr_100"), None, None, None),
            Err(LockstepSimInputGateError::MissingInitialSnapshot)
        );

        state.initial_snapshot_error = Some(LockstepSimSnapshotError::InvalidConfigVersion);
        assert_eq!(
            gate_lockstep_sim_input(&state, Some("chr_100"), None, None, None),
            Err(LockstepSimInputGateError::SnapshotError)
        );
    }

    #[test]
    fn input_gate_returns_bound_entity_and_checks_version_hash_schema() {
        let snapshot = parsed_snapshot_for_gate();
        let expected_hash = snapshot.config_hash.clone();
        let expected_schema = snapshot.sim_schema_version;
        let mut state = LockstepSimSceneState {
            active: true,
            initial_snapshot: Some(snapshot),
            ..Default::default()
        };

        let context = gate_lockstep_sim_input(
            &state,
            Some("chr_100"),
            Some(2),
            Some(&expected_hash),
            Some(expected_schema),
        )
        .unwrap();

        assert_eq!(context.character_id, "chr_100");
        assert_eq!(context.entity_id, EntityId::new(100));
        assert_eq!(context.config_version, 2);
        assert_eq!(context.config_hash, expected_hash);

        assert_eq!(
            gate_lockstep_sim_input(&state, Some("missing"), None, None, None),
            Err(LockstepSimInputGateError::MissingControlBinding {
                character_id: "missing".to_string()
            })
        );
        assert!(matches!(
            gate_lockstep_sim_input(&state, Some("chr_100"), Some(3), None, None),
            Err(LockstepSimInputGateError::ConfigVersionMismatch {
                expected: 3,
                actual: 2
            })
        ));
        assert!(matches!(
            gate_lockstep_sim_input(&state, Some("chr_100"), None, Some("bad-hash"), None),
            Err(LockstepSimInputGateError::ConfigHashMismatch { .. })
        ));

        state.initial_snapshot.as_mut().unwrap().sim_schema_version =
            expected_schema.saturating_add(1);
        assert!(matches!(
            gate_lockstep_sim_input(&state, Some("chr_100"), None, None, Some(expected_schema)),
            Err(LockstepSimInputGateError::SimSchemaVersionMismatch { .. })
        ));
    }

    fn parsed_snapshot_for_gate() -> super::super::snapshot::ParsedInitialSnapshot {
        use std::collections::HashMap;

        use super::super::snapshot::SimHashEnvelope;
        use sim_core::{
            CombatConfig, CombatState, EntityKind, MovementConfig, MovementMode, MovementState,
            SceneBounds, SimConfig, SimEntity, SimRngState, SimTransform, TeamId,
        };

        let entity = SimEntity {
            id: EntityId::new(100),
            kind: EntityKind::Player,
            owner_character_id: Some("chr_100".to_string()),
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
            combat: CombatState::default(),
            alive: true,
        };
        let world = SimWorld::with_rng(
            FrameId::new(0),
            SimRngState {
                seed: 1,
                counter: 0,
            },
            vec![entity.clone()],
        )
        .unwrap();
        let mut control_bindings = HashMap::new();
        control_bindings.insert("chr_100".to_string(), EntityId::new(100));

        super::super::snapshot::ParsedInitialSnapshot {
            room_id: "lockstep-room".to_string(),
            start_frame: 0,
            tick_rate: 20,
            config_version: 2,
            config_hash: "hash-2".to_string(),
            sim_schema_version: sim_core::SIM_CORE_SCHEMA_VERSION,
            rng_seed: 1,
            state_hash: SimHashEnvelope {
                frame: 0,
                value: 0,
                hex: "0000000000000000".to_string(),
            },
            world,
            config: SimConfig {
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
                combat: CombatConfig::default(),
            },
            control_bindings,
            entities: vec![entity],
        }
    }
}
