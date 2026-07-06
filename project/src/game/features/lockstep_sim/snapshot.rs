#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::fmt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sim_core::{
    BuffDefinition, BuffId, CombatConfig, CombatEffect, DamageFormula, EntityId, Fp,
    MovementConfig, SceneBounds, SimConfig, SimEntity, SimHash, SimSnapshot, SimWorld,
    SkillDefinition, SkillId, SkillTargetType, SnapshotError, Vec2Fp, hash_world,
    restore as restore_sim_snapshot,
};

pub(in crate::game::features::lockstep_sim) const SIM_INITIAL_SNAPSHOT_SCHEMA: &str =
    "myserver.lockstep-sim.initial-snapshot.v1";
const SIM_DOWNLINK_SCHEMA_VERSION: u32 = 1;
const LOCKSTEP_SIM_DEMO_FIXED_CONFIG_VERSION: u64 = 1;
const DEFAULT_PLAYER_SKILL_ID: u32 = 1;
const DEFAULT_DEMO_BUFF_ID: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::game::features::lockstep_sim) struct ParsedInitialSnapshot {
    pub(in crate::game::features::lockstep_sim) room_id: String,
    pub(in crate::game::features::lockstep_sim) start_frame: u32,
    pub(in crate::game::features::lockstep_sim) tick_rate: u16,
    pub(in crate::game::features::lockstep_sim) config_version: u64,
    pub(in crate::game::features::lockstep_sim) config_hash: String,
    pub(in crate::game::features::lockstep_sim) sim_schema_version: u16,
    pub(in crate::game::features::lockstep_sim) rng_seed: u64,
    pub(in crate::game::features::lockstep_sim) state_hash: SimHashEnvelope,
    pub(in crate::game::features::lockstep_sim) world: SimWorld,
    pub(in crate::game::features::lockstep_sim) config: SimConfig,
    pub(in crate::game::features::lockstep_sim) control_bindings: HashMap<String, EntityId>,
    pub(in crate::game::features::lockstep_sim) entities: Vec<SimEntity>,
}

impl ParsedInitialSnapshot {
    pub(in crate::game::features::lockstep_sim) fn initial_hash(&self) -> SimHash {
        hash_world(&self.world)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::game::features::lockstep_sim) enum LockstepSimSnapshotError {
    JsonDecode(String),
    MissingInitialSnapshot,
    UnsupportedSchema {
        actual: String,
    },
    UnsupportedSchemaVersion {
        actual: u32,
        expected: u32,
    },
    InvalidRoomId,
    InvalidTickRate,
    InvalidConfigVersion,
    UnsupportedSimSchemaVersion {
        actual: u16,
        expected: u16,
    },
    ConfigHashMismatch {
        actual: String,
        expected: String,
    },
    SnapshotRestore(String),
    FrameMismatch {
        start_frame: u32,
        world_frame: u32,
    },
    RngSeedMismatch {
        expected: u64,
        actual: u64,
    },
    HashEnvelopeMismatch {
        actual: SimHashEnvelope,
        expected: SimHashEnvelope,
    },
    EntitiesMismatch,
    InvalidControlBinding {
        code: &'static str,
    },
}

impl fmt::Display for LockstepSimSnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::JsonDecode(message) => {
                write!(f, "lockstep snapshot JSON decode failed: {message}")
            }
            Self::MissingInitialSnapshot => write!(f, "lockstep initialSnapshot is missing"),
            Self::UnsupportedSchema { actual } => {
                write!(f, "unsupported lockstep initial snapshot schema: {actual}")
            }
            Self::UnsupportedSchemaVersion { actual, expected } => write!(
                f,
                "unsupported lockstep initial snapshot schemaVersion: got {actual}, expected {expected}"
            ),
            Self::InvalidRoomId => write!(f, "lockstep initial snapshot roomId is empty"),
            Self::InvalidTickRate => write!(f, "lockstep initial snapshot tickRate is zero"),
            Self::InvalidConfigVersion => {
                write!(f, "lockstep initial snapshot configVersion is zero")
            }
            Self::UnsupportedSimSchemaVersion { actual, expected } => write!(
                f,
                "unsupported sim-core schema version: got {actual}, expected {expected}"
            ),
            Self::ConfigHashMismatch { actual, expected } => write!(
                f,
                "lockstep initial snapshot configHash mismatch: got {actual}, expected {expected}"
            ),
            Self::SnapshotRestore(message) => {
                write!(f, "lockstep sim snapshot restore failed: {message}")
            }
            Self::FrameMismatch {
                start_frame,
                world_frame,
            } => write!(
                f,
                "lockstep initial snapshot frame mismatch: startFrame {start_frame}, world frame {world_frame}"
            ),
            Self::RngSeedMismatch { expected, actual } => write!(
                f,
                "lockstep initial snapshot rngSeed mismatch: got {actual}, expected {expected}"
            ),
            Self::HashEnvelopeMismatch { actual, expected } => write!(
                f,
                "lockstep initial snapshot stateHash mismatch: got frame {} value {} hex {}, expected frame {} value {} hex {}",
                actual.frame,
                actual.value,
                actual.hex,
                expected.frame,
                expected.value,
                expected.hex
            ),
            Self::EntitiesMismatch => {
                write!(
                    f,
                    "lockstep initial snapshot entities do not match restored world"
                )
            }
            Self::InvalidControlBinding { code } => {
                write!(f, "invalid lockstep control binding: {code}")
            }
        }
    }
}

impl std::error::Error for LockstepSimSnapshotError {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::game::features::lockstep_sim) struct SimHashEnvelope {
    pub(in crate::game::features::lockstep_sim) frame: u32,
    pub(in crate::game::features::lockstep_sim) value: u64,
    pub(in crate::game::features::lockstep_sim) hex: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SimControlBinding {
    character_id: String,
    entity_id: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SimInitialSnapshot {
    schema: String,
    schema_version: u32,
    room_id: String,
    start_frame: u32,
    tick_rate: u16,
    #[serde(default = "default_config_version")]
    config_version: u64,
    config_hash: String,
    #[serde(default = "default_sim_schema_version")]
    sim_schema_version: u16,
    rng_seed: u64,
    state_hash: SimHashEnvelope,
    snapshot: SimSnapshot,
    entities: Vec<SimEntity>,
    control_bindings: Vec<SimControlBinding>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RoomGameState {
    initial_snapshot: Option<SimInitialSnapshot>,
}

pub(in crate::game::features::lockstep_sim) fn parse_initial_snapshot_from_game_state(
    game_state_json: &str,
) -> Result<ParsedInitialSnapshot, LockstepSimSnapshotError> {
    let game_state: RoomGameState = serde_json::from_str(game_state_json)
        .map_err(|error| LockstepSimSnapshotError::JsonDecode(error.to_string()))?;
    let snapshot = game_state
        .initial_snapshot
        .ok_or(LockstepSimSnapshotError::MissingInitialSnapshot)?;
    restore_initial_snapshot(snapshot)
}

fn restore_initial_snapshot(
    snapshot: SimInitialSnapshot,
) -> Result<ParsedInitialSnapshot, LockstepSimSnapshotError> {
    validate_snapshot_metadata(&snapshot)?;

    let config = client_demo_sim_config_for_snapshot(snapshot.tick_rate);
    if snapshot.config_hash != config_hash_hex(&config) {
        return Err(LockstepSimSnapshotError::ConfigHashMismatch {
            actual: snapshot.config_hash,
            expected: config_hash_hex(&config),
        });
    }

    let world = restore_sim_snapshot(&snapshot.snapshot).map_err(snapshot_restore_error)?;
    if snapshot.start_frame != world.frame.raw() {
        return Err(LockstepSimSnapshotError::FrameMismatch {
            start_frame: snapshot.start_frame,
            world_frame: world.frame.raw(),
        });
    }
    if snapshot.rng_seed != world.rng.seed {
        return Err(LockstepSimSnapshotError::RngSeedMismatch {
            expected: world.rng.seed,
            actual: snapshot.rng_seed,
        });
    }

    let expected_state_hash = sim_hash_envelope(snapshot.snapshot.hash);
    if snapshot.state_hash != expected_state_hash {
        return Err(LockstepSimSnapshotError::HashEnvelopeMismatch {
            actual: snapshot.state_hash,
            expected: expected_state_hash,
        });
    }

    let mut entities = snapshot.entities;
    entities.sort_by_key(|entity| entity.id);
    if entities != world.entities_sorted_by_id() {
        return Err(LockstepSimSnapshotError::EntitiesMismatch);
    }

    let control_bindings = restore_control_bindings(&snapshot.control_bindings, &world)?;

    Ok(ParsedInitialSnapshot {
        room_id: snapshot.room_id,
        start_frame: snapshot.start_frame,
        tick_rate: snapshot.tick_rate,
        config_version: snapshot.config_version,
        config_hash: snapshot.config_hash,
        sim_schema_version: snapshot.sim_schema_version,
        rng_seed: snapshot.rng_seed,
        state_hash: expected_state_hash,
        world,
        config,
        control_bindings,
        entities,
    })
}

fn validate_snapshot_metadata(
    snapshot: &SimInitialSnapshot,
) -> Result<(), LockstepSimSnapshotError> {
    if snapshot.schema != SIM_INITIAL_SNAPSHOT_SCHEMA {
        return Err(LockstepSimSnapshotError::UnsupportedSchema {
            actual: snapshot.schema.clone(),
        });
    }
    if snapshot.schema_version != SIM_DOWNLINK_SCHEMA_VERSION {
        return Err(LockstepSimSnapshotError::UnsupportedSchemaVersion {
            actual: snapshot.schema_version,
            expected: SIM_DOWNLINK_SCHEMA_VERSION,
        });
    }
    if snapshot.room_id.trim().is_empty() {
        return Err(LockstepSimSnapshotError::InvalidRoomId);
    }
    if snapshot.tick_rate == 0 {
        return Err(LockstepSimSnapshotError::InvalidTickRate);
    }
    if snapshot.config_version == 0 {
        return Err(LockstepSimSnapshotError::InvalidConfigVersion);
    }
    if snapshot.sim_schema_version != sim_core::SIM_CORE_SCHEMA_VERSION {
        return Err(LockstepSimSnapshotError::UnsupportedSimSchemaVersion {
            actual: snapshot.sim_schema_version,
            expected: sim_core::SIM_CORE_SCHEMA_VERSION,
        });
    }

    Ok(())
}

fn restore_control_bindings(
    bindings: &[SimControlBinding],
    world: &SimWorld,
) -> Result<HashMap<String, EntityId>, LockstepSimSnapshotError> {
    let mut restored = HashMap::new();
    let mut character_ids = HashSet::new();
    let mut entity_ids = HashSet::new();

    for binding in bindings {
        if binding.character_id.trim().is_empty() {
            return Err(control_binding_error("empty_character_id"));
        }
        if !character_ids.insert(binding.character_id.clone()) {
            return Err(control_binding_error("duplicate_character_id"));
        }

        let entity_id = EntityId::new(binding.entity_id);
        if !entity_ids.insert(entity_id) {
            return Err(control_binding_error("duplicate_entity_id"));
        }

        let Some(entity) = world.entity(entity_id) else {
            return Err(control_binding_error("missing_entity"));
        };
        if entity.owner_character_id.as_deref() != Some(binding.character_id.as_str()) {
            return Err(control_binding_error("owner_character_mismatch"));
        }

        restored.insert(binding.character_id.clone(), entity_id);
    }

    Ok(restored)
}

fn control_binding_error(code: &'static str) -> LockstepSimSnapshotError {
    LockstepSimSnapshotError::InvalidControlBinding { code }
}

fn snapshot_restore_error(error: SnapshotError) -> LockstepSimSnapshotError {
    LockstepSimSnapshotError::SnapshotRestore(error.to_string())
}

fn sim_hash_envelope(hash: SimHash) -> SimHashEnvelope {
    SimHashEnvelope {
        frame: hash.frame.raw(),
        value: hash.value,
        hex: format!("{:016x}", hash.value),
    }
}

fn default_config_version() -> u64 {
    LOCKSTEP_SIM_DEMO_FIXED_CONFIG_VERSION
}

fn default_sim_schema_version() -> u16 {
    sim_core::SIM_CORE_SCHEMA_VERSION
}

fn client_demo_sim_config_for_snapshot(tick_rate: u16) -> SimConfig {
    client_demo_sim_config(tick_rate)
}

fn client_demo_sim_config(tick_rate: u16) -> SimConfig {
    let tick_rate = tick_rate.max(1);
    // Client mirror of MyServer lockstep_sim_demo.fixed_v1 until sim-core CSV
    // config mapping becomes the source of truth on both sides.
    SimConfig {
        movement: MovementConfig {
            tick_rate,
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
                id: SkillId::new(DEFAULT_PLAYER_SKILL_ID),
                cooldown_frames: tick_rate as u32,
                cast_range: Fp::from_i32(12),
                target_type: SkillTargetType::Enemy,
                effects: vec![CombatEffect::Damage {
                    formula: DamageFormula::Fixed { amount: 15 },
                }],
            }],
            vec![BuffDefinition {
                id: BuffId::new(DEFAULT_DEMO_BUFF_ID),
                duration_frames: tick_rate as u32 * 3,
                interval_frames: tick_rate as u32,
                max_stacks: 1,
                effects: vec![CombatEffect::Heal {
                    formula: DamageFormula::Fixed { amount: 1 },
                }],
            }],
        )
        .expect("client demo sim config mirrors server-validated fixed_v1"),
    }
}

fn config_hash_hex(config: &SimConfig) -> String {
    let encoded = serde_json::to_vec(config).expect("sim config should serialize to JSON");
    let digest = Sha256::digest(encoded);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut hex, "{byte:02x}").expect("writing sha256 hex to String should not fail");
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use sim_core::{
        CombatState, EntityKind, FrameId, MovementMode, MovementState, QuantizedDir, SimRngState,
        SimTransform, TeamId, Vec2Fp, hash_world, snapshot as capture_sim_snapshot,
    };

    #[test]
    fn parses_initial_snapshot_and_restores_world() {
        let game_state = fixture_game_state();

        let parsed = parse_initial_snapshot_from_game_state(&game_state).unwrap();

        assert_eq!(parsed.room_id, "lockstep-room");
        assert_eq!(parsed.start_frame, 7);
        assert_eq!(parsed.tick_rate, 20);
        assert_eq!(
            parsed.config_version,
            LOCKSTEP_SIM_DEMO_FIXED_CONFIG_VERSION
        );
        assert_eq!(parsed.sim_schema_version, sim_core::SIM_CORE_SCHEMA_VERSION);
        assert_eq!(parsed.rng_seed, 11);
        assert_eq!(parsed.world.frame, FrameId::new(7));
        assert_eq!(parsed.world.rng.seed, 11);
        assert_eq!(
            parsed.control_bindings.get("chr_100").copied(),
            Some(EntityId::new(100))
        );
        assert_eq!(parsed.entities, parsed.world.entities_sorted_by_id());
    }

    #[test]
    fn restoring_same_snapshot_repeats_initial_hash() {
        let game_state = fixture_game_state();

        let first = parse_initial_snapshot_from_game_state(&game_state).unwrap();
        let second = parse_initial_snapshot_from_game_state(&game_state).unwrap();

        assert_eq!(first.initial_hash(), second.initial_hash());
        assert_eq!(first.initial_hash(), hash_world(&first.world));
    }

    #[test]
    fn accepts_non_zero_config_version_as_room_metadata() {
        let game_state = fixture_game_state_with(|snapshot| {
            snapshot["configVersion"] = json!(2);
        });

        let parsed = parse_initial_snapshot_from_game_state(&game_state).unwrap();

        assert_eq!(parsed.config_version, 2);
        assert_eq!(parsed.tick_rate, 20);
    }

    #[test]
    fn rejects_zero_config_version() {
        let game_state = fixture_game_state_with(|snapshot| {
            snapshot["configVersion"] = json!(0);
        });

        let error = parse_initial_snapshot_from_game_state(&game_state).unwrap_err();

        assert_eq!(error, LockstepSimSnapshotError::InvalidConfigVersion);
    }

    #[test]
    fn rejects_schema_version_mismatch() {
        let game_state = fixture_game_state_with(|snapshot| {
            snapshot["schemaVersion"] = json!(SIM_DOWNLINK_SCHEMA_VERSION + 1);
        });

        let error = parse_initial_snapshot_from_game_state(&game_state).unwrap_err();

        assert_eq!(
            error,
            LockstepSimSnapshotError::UnsupportedSchemaVersion {
                actual: SIM_DOWNLINK_SCHEMA_VERSION + 1,
                expected: SIM_DOWNLINK_SCHEMA_VERSION
            }
        );
    }

    #[test]
    fn rejects_config_hash_mismatch() {
        let game_state = fixture_game_state_with(|snapshot| {
            snapshot["configHash"] = json!("not-the-server-config-hash");
        });

        let error = parse_initial_snapshot_from_game_state(&game_state).unwrap_err();

        assert!(matches!(
            error,
            LockstepSimSnapshotError::ConfigHashMismatch { .. }
        ));
    }

    #[test]
    fn rejects_control_binding_to_missing_entity() {
        let game_state = fixture_game_state_with(|snapshot| {
            snapshot["controlBindings"] = json!([
                {
                    "characterId": "chr_100",
                    "entityId": 999
                }
            ]);
        });

        let error = parse_initial_snapshot_from_game_state(&game_state).unwrap_err();

        assert_eq!(error, control_binding_error("missing_entity"));
    }

    #[test]
    fn rejects_start_frame_world_frame_mismatch() {
        let game_state = fixture_game_state_with(|snapshot| {
            snapshot["startFrame"] = json!(8);
        });

        let error = parse_initial_snapshot_from_game_state(&game_state).unwrap_err();

        assert_eq!(
            error,
            LockstepSimSnapshotError::FrameMismatch {
                start_frame: 8,
                world_frame: 7
            }
        );
    }

    fn fixture_game_state() -> String {
        fixture_game_state_with(|_| {})
    }

    fn fixture_game_state_with(mut mutate: impl FnMut(&mut serde_json::Value)) -> String {
        let world = fixture_world();
        let config = client_demo_sim_config(20);
        let snapshot = capture_sim_snapshot(&world, &config);
        let state_hash = sim_hash_envelope(snapshot.hash);
        let mut initial_snapshot = json!({
            "schema": SIM_INITIAL_SNAPSHOT_SCHEMA,
            "schemaVersion": SIM_DOWNLINK_SCHEMA_VERSION,
            "roomId": "lockstep-room",
            "startFrame": world.frame.raw(),
            "tickRate": 20,
            "configVersion": LOCKSTEP_SIM_DEMO_FIXED_CONFIG_VERSION,
            "configHash": config_hash_hex(&config),
            "simSchemaVersion": sim_core::SIM_CORE_SCHEMA_VERSION,
            "rngSeed": world.rng.seed,
            "stateHash": state_hash,
            "snapshot": snapshot,
            "entities": world.entities_sorted_by_id(),
            "controlBindings": [
                {
                    "characterId": "chr_100",
                    "entityId": 100
                }
            ]
        });
        mutate(&mut initial_snapshot);

        json!({
            "logicType": "lockstep_sim_demo",
            "roomId": "lockstep-room",
            "worldFrame": world.frame.raw(),
            "tickRate": 20,
            "configVersion": LOCKSTEP_SIM_DEMO_FIXED_CONFIG_VERSION,
            "configHash": config_hash_hex(&config),
            "simSchemaVersion": sim_core::SIM_CORE_SCHEMA_VERSION,
            "initialSnapshot": initial_snapshot,
            "lastFrame": world.frame.raw(),
            "observerFrame": world.frame.raw(),
            "lastStateHash": state_hash,
        })
        .to_string()
    }

    fn fixture_world() -> SimWorld {
        SimWorld::with_rng(
            FrameId::new(7),
            SimRngState {
                seed: 11,
                counter: 22,
            },
            vec![
                fixture_entity(200, "chr_200", Vec2Fp::new(Fp::from_i32(2), Fp::ZERO)),
                fixture_entity(100, "chr_100", Vec2Fp::new(Fp::from_i32(1), Fp::ZERO)),
            ],
        )
        .unwrap()
    }

    fn fixture_entity(id: u32, character_id: &str, position: Vec2Fp) -> SimEntity {
        SimEntity {
            id: EntityId::new(id),
            kind: EntityKind::Player,
            owner_character_id: Some(character_id.to_string()),
            team_id: TeamId::new(1),
            transform: SimTransform {
                pos: position,
                facing: QuantizedDir::RIGHT,
                radius: Fp::from_milli(500),
            },
            movement: MovementState {
                mode: MovementMode::Controlled,
                move_dir: QuantizedDir::RIGHT,
                speed_per_second: Fp::from_i32(6),
            },
            combat: CombatState {
                hp: 100,
                max_hp: 100,
                attack: 10,
                defense: 3,
                speed: 6,
                crit_rate_bps: 500,
                crit_damage_bps: 15_000,
                skill_slots: Vec::new(),
                buffs: Vec::new(),
            },
            alive: true,
        }
    }
}
