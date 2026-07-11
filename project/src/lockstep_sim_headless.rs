//! Explicit, windowless lockstep telemetry for local development and CI.
//!
//! This module does not participate in [`crate::run`] and does not implement an
//! alternate MyServer authentication path. The offline scenario labels its
//! comparison hash as fixture authority, while `connect-probe` only checks TCP
//! reachability and never sends application bytes.

use std::{
    collections::BTreeMap,
    io::{self, Write},
    net::{SocketAddr, TcpStream},
    time::Duration,
};

use serde::Serialize;
use sim_core::{
    BuffDefinition, BuffId, CastSkillCommand, CombatConfig, CombatEffect, CombatState,
    DamageFormula, EntityId, EntityKind, FaceCommand, Fp, FrameId, MoveCommand, MovementConfig,
    MovementMode, MovementState, QuantizedDir, SceneBounds, SimCommand, SimConfig, SimEntity,
    SimEvent, SimHash, SimInput, SimInputSource, SimRngState, SimTransform, SimWorld,
    SkillDefinition, SkillId, SkillSlot, SkillTarget, SkillTargetType, TeamId, Vec2Fp, hash_world,
    restore, snapshot, step,
};

pub const HEADLESS_TELEMETRY_SCHEMA: &str = "mybevy.lockstep.telemetry";
pub const HEADLESS_TELEMETRY_SCHEMA_VERSION: u16 = 1;
pub const HEADLESS_LOCKSTEP_SCENE: &str = "arena.lockstep_sim";
pub const EXIT_SUCCESS: u8 = 0;
pub const EXIT_HASH_MISMATCH: u8 = 2;
pub const EXIT_CONFIGURATION: u8 = 3;
pub const EXIT_SIMULATION: u8 = 4;
pub const EXIT_CONNECTION: u8 = 5;
pub const EXIT_RECOVERY: u8 = 6;

const FIXTURE_ENTITY_ID: EntityId = EntityId::new(100);
const FIXTURE_TARGET_ID: EntityId = EntityId::new(200);
const FIXTURE_SKILL_ID: SkillId = SkillId::new(10);
const FIXTURE_DOT_BUFF_ID: BuffId = BuffId::new(100);
const FIXTURE_CHECKPOINT_FRAME: u32 = 2;
const FIXTURE_FINAL_FRAME: u32 = 6;

pub const HEADLESS_HELP: &str = "\
mybevy lockstep simulation headless telemetry\n\
\n\
Usage:\n\
  lockstep-sim-headless [options]\n\
\n\
Options:\n\
  --scenario <offline-fixture|connect-probe>\n\
  --run-id <id>\n\
  --room <room>\n\
  --policy <policy>\n\
  --player <player>\n\
  --inject-mismatch-frame <frame>  Explicit local test fault for diagnostics\n\
  --endpoint <ip:port>             Required by connect-probe\n\
  --connect-timeout-ms <ms>        Defaults to 500\n\
  --help\n\
\n\
Output is telemetry schema mybevy.lockstep.telemetry v1, one JSON object per line.\n";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HeadlessScenario {
    #[default]
    OfflineFixture,
    ConnectProbe,
}

impl HeadlessScenario {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "offline-fixture" => Some(Self::OfflineFixture),
            "connect-probe" => Some(Self::ConnectProbe),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeadlessOptions {
    pub run_id: String,
    pub room: String,
    pub policy: String,
    pub player: String,
    pub scenario: HeadlessScenario,
    pub inject_mismatch_frame: Option<u32>,
    pub endpoint: Option<String>,
    pub connect_timeout: Duration,
}

impl Default for HeadlessOptions {
    fn default() -> Self {
        Self {
            run_id: "offline-fixture-v1".to_string(),
            room: "lockstep-headless-room".to_string(),
            policy: "lockstep_sim_demo".to_string(),
            player: "lockstep-headless-player".to_string(),
            scenario: HeadlessScenario::OfflineFixture,
            inject_mismatch_frame: None,
            endpoint: None,
            connect_timeout: Duration::from_millis(500),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HeadlessCommand {
    Run(HeadlessOptions),
    Help,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeadlessCliError {
    pub error_code: &'static str,
    pub failure_stage: &'static str,
    pub message: String,
    pub exit_code: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeadlessRun {
    pub records: Vec<TelemetryRecord>,
    pub exit_code: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryRecord {
    pub schema: &'static str,
    pub schema_version: u16,
    pub event: TelemetryEvent,
    pub run_id: String,
    pub scenario: HeadlessScenario,
    pub scene: &'static str,
    pub room: String,
    pub policy: String,
    pub player: String,
    pub server_connected: bool,
    pub frame: Option<u32>,
    pub server_hash: Option<HashTelemetry>,
    pub local_hash: Option<HashTelemetry>,
    pub mismatch: Option<bool>,
    pub inputs: Vec<InputTelemetry>,
    pub entities: Vec<EntityTelemetry>,
    pub events: EventSummaryTelemetry,
    pub replay_recovery: ReplayRecoveryTelemetry,
    pub error_code: Option<String>,
    pub failure_stage: Option<String>,
    pub message: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryEvent {
    RunStarted,
    Frame,
    ReplayRecovery,
    RunCompleted,
    RunFailed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HashTelemetry {
    pub frame: u32,
    /// Decimal string avoids precision loss in JSON consumers using IEEE-754 numbers.
    pub value: String,
    pub hex: String,
    pub source: HashSource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HashSource {
    OfflineFixtureAuthority,
    LocalReplay,
    SnapshotRecovery,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputTelemetry {
    pub character_id: String,
    pub entity_id: u32,
    pub sequence: u32,
    pub command: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityTelemetry {
    pub entity_id: u32,
    pub owner_character_id: Option<String>,
    pub kind: &'static str,
    pub team_id: u16,
    pub fixed_position_milli: FixedPointPositionTelemetry,
    pub facing_quantized: DirectionTelemetry,
    pub movement_mode: &'static str,
    pub move_direction_quantized: DirectionTelemetry,
    pub speed_per_second_milli: i64,
    pub hp: i32,
    pub max_hp: i32,
    pub alive: bool,
    pub buffs: Vec<BuffTelemetry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct FixedPointPositionTelemetry {
    pub x: i64,
    pub y: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct DirectionTelemetry {
    pub x: i16,
    pub y: i16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuffTelemetry {
    pub buff_id: u32,
    pub source_entity_id: u32,
    pub duration_remaining: u32,
    pub interval_remaining: u32,
    pub stacks: u16,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventSummaryTelemetry {
    pub total: usize,
    pub by_kind: BTreeMap<&'static str, usize>,
    pub items: Vec<EventTelemetry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventTelemetry {
    pub kind: &'static str,
    pub source_entity_id: u32,
    pub target_entity_id: Option<u32>,
    pub skill_id: Option<u32>,
    pub buff_id: Option<u32>,
    pub value: i32,
    pub sequence: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayRecoveryTelemetry {
    pub status: ReplayRecoveryStatus,
    pub checkpoint_frame: Option<u32>,
    pub target_frame: Option<u32>,
    pub replayed_frames: u32,
}

impl Default for ReplayRecoveryTelemetry {
    fn default() -> Self {
        Self {
            status: ReplayRecoveryStatus::NotStarted,
            checkpoint_frame: None,
            target_frame: None,
            replayed_frames: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayRecoveryStatus {
    #[default]
    NotStarted,
    CheckpointCaptured,
    Pending,
    Verified,
    Failed,
}

pub fn parse_headless_args(
    args: impl IntoIterator<Item = String>,
) -> Result<HeadlessCommand, HeadlessCliError> {
    let mut options = HeadlessOptions::default();
    let mut args = args.into_iter();

    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--help" | "-h" => return Ok(HeadlessCommand::Help),
            "--scenario" => {
                let value = next_value(&mut args, "--scenario")?;
                options.scenario = HeadlessScenario::parse(&value).ok_or_else(|| {
                    cli_error(format!(
                        "unsupported scenario {value:?}; expected offline-fixture or connect-probe"
                    ))
                })?;
            }
            "--run-id" => {
                options.run_id = non_empty(next_value(&mut args, "--run-id")?, "--run-id")?
            }
            "--room" => options.room = non_empty(next_value(&mut args, "--room")?, "--room")?,
            "--policy" => {
                options.policy = non_empty(next_value(&mut args, "--policy")?, "--policy")?
            }
            "--player" => {
                options.player = non_empty(next_value(&mut args, "--player")?, "--player")?
            }
            "--inject-mismatch-frame" => {
                let value = next_value(&mut args, "--inject-mismatch-frame")?;
                let frame = value.parse::<u32>().map_err(|_| {
                    cli_error("--inject-mismatch-frame must be an integer".to_string())
                })?;
                if !(1..=FIXTURE_FINAL_FRAME).contains(&frame) {
                    return Err(cli_error(format!(
                        "--inject-mismatch-frame must be between 1 and {FIXTURE_FINAL_FRAME}"
                    )));
                }
                options.inject_mismatch_frame = Some(frame);
            }
            "--endpoint" => {
                options.endpoint = Some(non_empty(
                    next_value(&mut args, "--endpoint")?,
                    "--endpoint",
                )?);
            }
            "--connect-timeout-ms" => {
                let value = next_value(&mut args, "--connect-timeout-ms")?;
                let timeout_ms = value.parse::<u64>().map_err(|_| {
                    cli_error("--connect-timeout-ms must be a positive integer".to_string())
                })?;
                if timeout_ms == 0 {
                    return Err(cli_error(
                        "--connect-timeout-ms must be greater than zero".to_string(),
                    ));
                }
                options.connect_timeout = Duration::from_millis(timeout_ms);
            }
            _ => return Err(cli_error(format!("unknown argument {argument:?}"))),
        }
    }

    Ok(HeadlessCommand::Run(options))
}

fn next_value(
    args: &mut impl Iterator<Item = String>,
    option: &str,
) -> Result<String, HeadlessCliError> {
    args.next()
        .ok_or_else(|| cli_error(format!("{option} requires a value")))
}

fn non_empty(value: String, option: &str) -> Result<String, HeadlessCliError> {
    let value = value.trim().to_string();
    if value.is_empty() {
        Err(cli_error(format!("{option} must not be empty")))
    } else {
        Ok(value)
    }
}

fn cli_error(message: String) -> HeadlessCliError {
    HeadlessCliError {
        error_code: "HEADLESS_ARGUMENT_INVALID",
        failure_stage: "configuration",
        message,
        exit_code: EXIT_CONFIGURATION,
    }
}

pub fn run_headless(options: &HeadlessOptions) -> HeadlessRun {
    match options.scenario {
        HeadlessScenario::OfflineFixture => run_offline_fixture(options),
        HeadlessScenario::ConnectProbe => run_connect_probe_with(options, |address, timeout| {
            TcpStream::connect_timeout(&address, timeout).map(drop)
        }),
    }
}

pub fn write_jsonl(writer: &mut impl Write, records: &[TelemetryRecord]) -> io::Result<()> {
    for record in records {
        serde_json::to_writer(&mut *writer, record).map_err(io::Error::other)?;
        writer.write_all(b"\n")?;
    }
    Ok(())
}

pub fn failure_run(
    options: &HeadlessOptions,
    error_code: impl Into<String>,
    failure_stage: impl Into<String>,
    message: impl Into<String>,
    exit_code: u8,
) -> HeadlessRun {
    HeadlessRun {
        records: vec![failure_record(
            options,
            None,
            error_code,
            failure_stage,
            message,
            ReplayRecoveryTelemetry::default(),
            Vec::new(),
        )],
        exit_code,
    }
}

fn run_offline_fixture(options: &HeadlessOptions) -> HeadlessRun {
    let (initial_world, config) = match fixture_world_and_config(options) {
        Ok(fixture) => fixture,
        Err(message) => {
            return failure_run(
                options,
                "HEADLESS_FIXTURE_INVALID",
                "fixture_setup",
                message,
                EXIT_CONFIGURATION,
            );
        }
    };
    let mut authority_world = initial_world.clone();
    let mut local_world = initial_world;
    let initial_hash = hash_world(&authority_world);
    let mut records = vec![world_record(
        options,
        TelemetryEvent::RunStarted,
        Some(0),
        Some(HashTelemetry::new(
            initial_hash,
            HashSource::OfflineFixtureAuthority,
        )),
        Some(HashTelemetry::new(initial_hash, HashSource::LocalReplay)),
        Some(false),
        &local_world,
        &[],
        &[],
        ReplayRecoveryTelemetry::default(),
    )];
    let mut checkpoint = None;
    let mut input_history = Vec::new();

    for frame in 1..=FIXTURE_FINAL_FRAME {
        let inputs = fixture_inputs(frame, options);
        input_history.push((frame, inputs.clone()));
        let authority_result =
            match step(&mut authority_world, FrameId::new(frame), &inputs, &config) {
                Ok(result) => result,
                Err(error) => {
                    records.push(failure_record(
                        options,
                        Some(frame),
                        "HEADLESS_SIM_STEP_FAILED",
                        "fixture_authority_step",
                        error.to_string(),
                        ReplayRecoveryTelemetry::default(),
                        entities_from_world(&authority_world),
                    ));
                    return HeadlessRun {
                        records,
                        exit_code: EXIT_SIMULATION,
                    };
                }
            };

        if options.inject_mismatch_frame == Some(frame) {
            inject_local_mismatch(&mut local_world);
        }
        let local_result = match step(&mut local_world, FrameId::new(frame), &inputs, &config) {
            Ok(result) => result,
            Err(error) => {
                records.push(failure_record(
                    options,
                    Some(frame),
                    "HEADLESS_SIM_STEP_FAILED",
                    "local_replay_step",
                    error.to_string(),
                    ReplayRecoveryTelemetry::default(),
                    entities_from_world(&local_world),
                ));
                return HeadlessRun {
                    records,
                    exit_code: EXIT_SIMULATION,
                };
            }
        };

        if frame == FIXTURE_CHECKPOINT_FRAME {
            checkpoint = Some(snapshot(&authority_world, &config));
        }
        let mismatch = authority_result.state_hash != local_result.state_hash;
        let recovery = if frame < FIXTURE_CHECKPOINT_FRAME {
            ReplayRecoveryTelemetry::default()
        } else if frame == FIXTURE_CHECKPOINT_FRAME {
            ReplayRecoveryTelemetry {
                status: ReplayRecoveryStatus::CheckpointCaptured,
                checkpoint_frame: Some(frame),
                target_frame: Some(FIXTURE_FINAL_FRAME),
                replayed_frames: 0,
            }
        } else {
            ReplayRecoveryTelemetry {
                status: ReplayRecoveryStatus::Pending,
                checkpoint_frame: Some(FIXTURE_CHECKPOINT_FRAME),
                target_frame: Some(FIXTURE_FINAL_FRAME),
                replayed_frames: 0,
            }
        };
        records.push(world_record(
            options,
            TelemetryEvent::Frame,
            Some(frame),
            Some(HashTelemetry::new(
                authority_result.state_hash,
                HashSource::OfflineFixtureAuthority,
            )),
            Some(HashTelemetry::new(
                local_result.state_hash,
                HashSource::LocalReplay,
            )),
            Some(mismatch),
            &local_world,
            &inputs,
            &local_result.events,
            recovery,
        ));

        if mismatch {
            records.push(failure_record(
                options,
                Some(frame),
                "HEADLESS_HASH_MISMATCH",
                "frame_compare",
                format!("fixture authority and local replay hashes differ at frame {frame}"),
                ReplayRecoveryTelemetry {
                    status: ReplayRecoveryStatus::Failed,
                    checkpoint_frame: checkpoint.as_ref().map(|value| value.world.frame.raw()),
                    target_frame: Some(frame),
                    replayed_frames: 0,
                },
                entities_from_world(&local_world),
            ));
            return HeadlessRun {
                records,
                exit_code: EXIT_HASH_MISMATCH,
            };
        }
    }

    let checkpoint = match checkpoint {
        Some(checkpoint) => checkpoint,
        None => {
            records.push(failure_record(
                options,
                None,
                "HEADLESS_RECOVERY_CHECKPOINT_MISSING",
                "snapshot_recovery",
                "offline fixture did not capture its recovery checkpoint",
                ReplayRecoveryTelemetry {
                    status: ReplayRecoveryStatus::Failed,
                    ..Default::default()
                },
                entities_from_world(&local_world),
            ));
            return HeadlessRun {
                records,
                exit_code: EXIT_RECOVERY,
            };
        }
    };
    let mut recovered_world = match restore(&checkpoint) {
        Ok(world) => world,
        Err(error) => {
            records.push(failure_record(
                options,
                Some(FIXTURE_CHECKPOINT_FRAME),
                "HEADLESS_RECOVERY_RESTORE_FAILED",
                "snapshot_restore",
                error.to_string(),
                ReplayRecoveryTelemetry {
                    status: ReplayRecoveryStatus::Failed,
                    checkpoint_frame: Some(FIXTURE_CHECKPOINT_FRAME),
                    target_frame: Some(FIXTURE_FINAL_FRAME),
                    replayed_frames: 0,
                },
                Vec::new(),
            ));
            return HeadlessRun {
                records,
                exit_code: EXIT_RECOVERY,
            };
        }
    };
    let mut replayed_frames = 0;
    for (frame, inputs) in input_history
        .iter()
        .filter(|(frame, _)| *frame > FIXTURE_CHECKPOINT_FRAME)
    {
        if let Err(error) = step(&mut recovered_world, FrameId::new(*frame), inputs, &config) {
            records.push(failure_record(
                options,
                Some(*frame),
                "HEADLESS_RECOVERY_REPLAY_FAILED",
                "snapshot_replay",
                error.to_string(),
                ReplayRecoveryTelemetry {
                    status: ReplayRecoveryStatus::Failed,
                    checkpoint_frame: Some(FIXTURE_CHECKPOINT_FRAME),
                    target_frame: Some(FIXTURE_FINAL_FRAME),
                    replayed_frames,
                },
                entities_from_world(&recovered_world),
            ));
            return HeadlessRun {
                records,
                exit_code: EXIT_RECOVERY,
            };
        }
        replayed_frames += 1;
    }

    let authority_hash = hash_world(&authority_world);
    let recovered_hash = hash_world(&recovered_world);
    let recovery_mismatch = authority_hash != recovered_hash;
    let recovery = ReplayRecoveryTelemetry {
        status: if recovery_mismatch {
            ReplayRecoveryStatus::Failed
        } else {
            ReplayRecoveryStatus::Verified
        },
        checkpoint_frame: Some(FIXTURE_CHECKPOINT_FRAME),
        target_frame: Some(FIXTURE_FINAL_FRAME),
        replayed_frames,
    };
    records.push(world_record(
        options,
        TelemetryEvent::ReplayRecovery,
        Some(FIXTURE_FINAL_FRAME),
        Some(HashTelemetry::new(
            authority_hash,
            HashSource::OfflineFixtureAuthority,
        )),
        Some(HashTelemetry::new(
            recovered_hash,
            HashSource::SnapshotRecovery,
        )),
        Some(recovery_mismatch),
        &recovered_world,
        &[],
        &[],
        recovery.clone(),
    ));
    if recovery_mismatch {
        records.push(failure_record(
            options,
            Some(FIXTURE_FINAL_FRAME),
            "HEADLESS_RECOVERY_HASH_MISMATCH",
            "snapshot_compare",
            "fixture authority and snapshot recovery hashes differ",
            recovery,
            entities_from_world(&recovered_world),
        ));
        return HeadlessRun {
            records,
            exit_code: EXIT_RECOVERY,
        };
    }

    records.push(world_record(
        options,
        TelemetryEvent::RunCompleted,
        Some(FIXTURE_FINAL_FRAME),
        Some(HashTelemetry::new(
            authority_hash,
            HashSource::OfflineFixtureAuthority,
        )),
        Some(HashTelemetry::new(
            hash_world(&local_world),
            HashSource::LocalReplay,
        )),
        Some(false),
        &local_world,
        &[],
        &[],
        recovery,
    ));
    HeadlessRun {
        records,
        exit_code: EXIT_SUCCESS,
    }
}

fn run_connect_probe_with(
    options: &HeadlessOptions,
    connect: impl FnOnce(SocketAddr, Duration) -> io::Result<()>,
) -> HeadlessRun {
    let Some(endpoint) = options.endpoint.as_deref() else {
        return failure_run(
            options,
            "HEADLESS_ENDPOINT_REQUIRED",
            "configuration",
            "connect-probe requires --endpoint <ip:port>",
            EXIT_CONFIGURATION,
        );
    };
    let address = match endpoint.parse::<SocketAddr>() {
        Ok(address) => address,
        Err(_) => {
            return failure_run(
                options,
                "HEADLESS_ENDPOINT_INVALID",
                "configuration",
                "connect-probe endpoint must be a numeric ip:port",
                EXIT_CONFIGURATION,
            );
        }
    };

    match connect(address, options.connect_timeout) {
        Ok(()) => failure_run(
            options,
            "HEADLESS_ONLINE_PROTOCOL_UNAVAILABLE",
            "authentication",
            "TCP probe connected; secure MyServer headless authentication is not implemented",
            EXIT_CONNECTION,
        ),
        Err(_) => failure_run(
            options,
            "HEADLESS_CONNECT_FAILED",
            "connect",
            "TCP connection to the configured endpoint failed",
            EXIT_CONNECTION,
        ),
    }
}

fn fixture_world_and_config(options: &HeadlessOptions) -> Result<(SimWorld, SimConfig), String> {
    let combat = CombatConfig::from_definitions(
        vec![SkillDefinition {
            id: FIXTURE_SKILL_ID,
            cooldown_frames: 5,
            cast_range: Fp::from_i32(10),
            target_type: SkillTargetType::Enemy,
            effects: vec![CombatEffect::AddBuff {
                buff_id: FIXTURE_DOT_BUFF_ID,
            }],
        }],
        vec![BuffDefinition {
            id: FIXTURE_DOT_BUFF_ID,
            duration_frames: 4,
            interval_frames: 1,
            max_stacks: 1,
            effects: vec![CombatEffect::Damage {
                formula: DamageFormula::Fixed { amount: 5 },
            }],
        }],
    )
    .map_err(|error| error.to_string())?;
    let config = SimConfig {
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
        combat,
    };
    let player = SimEntity {
        id: FIXTURE_ENTITY_ID,
        kind: EntityKind::Player,
        owner_character_id: Some(options.player.clone()),
        team_id: TeamId::new(1),
        transform: SimTransform {
            pos: Vec2Fp::zero(),
            facing: QuantizedDir::RIGHT,
            radius: Fp::from_milli(500),
        },
        movement: MovementState::default(),
        combat: CombatState {
            hp: 100,
            max_hp: 100,
            attack: 10,
            defense: 2,
            speed: 6,
            crit_rate_bps: 0,
            crit_damage_bps: 10_000,
            skill_slots: vec![SkillSlot {
                skill_id: FIXTURE_SKILL_ID,
                cooldown_remaining: 0,
            }],
            buffs: Vec::new(),
        },
        alive: true,
    };
    let target = SimEntity {
        id: FIXTURE_TARGET_ID,
        kind: EntityKind::Npc,
        owner_character_id: Some("lockstep-headless-target".to_string()),
        team_id: TeamId::new(2),
        transform: SimTransform {
            pos: Vec2Fp::new(Fp::from_i32(2), Fp::ZERO),
            facing: QuantizedDir::LEFT,
            radius: Fp::from_milli(500),
        },
        movement: MovementState::default(),
        combat: CombatState {
            hp: 100,
            max_hp: 100,
            defense: 0,
            ..Default::default()
        },
        alive: true,
    };
    let world = SimWorld::with_rng(
        FrameId::new(0),
        SimRngState {
            seed: 0x4845_4144_4c45_5353,
            counter: 0,
        },
        vec![player, target],
    )
    .map_err(|error| error.to_string())?;
    Ok((world, config))
}

fn fixture_inputs(frame: u32, options: &HeadlessOptions) -> Vec<SimInput> {
    let command = match frame {
        1 => Some(SimCommand::Move(MoveCommand {
            dir: QuantizedDir::RIGHT,
            speed_per_second: Some(Fp::from_i32(6)),
        })),
        2 => Some(SimCommand::Face(FaceCommand {
            dir: QuantizedDir::UP,
        })),
        3 => Some(SimCommand::Stop),
        4 => Some(SimCommand::CastSkill(CastSkillCommand {
            skill_id: FIXTURE_SKILL_ID,
            target: SkillTarget::Entity(FIXTURE_TARGET_ID),
        })),
        _ => None,
    };
    command
        .map(|command| {
            vec![SimInput {
                frame: FrameId::new(frame),
                character_id: options.player.clone(),
                entity_id: FIXTURE_ENTITY_ID,
                seq: frame,
                source: SimInputSource::Real,
                command,
            }]
        })
        .unwrap_or_default()
}

fn inject_local_mismatch(world: &mut SimWorld) {
    let Some(entity) = world.entity_mut(FIXTURE_ENTITY_ID) else {
        return;
    };
    entity.transform.pos.x = Fp::from_raw(entity.transform.pos.x.raw().saturating_add(1));
}

impl HashTelemetry {
    fn new(hash: SimHash, source: HashSource) -> Self {
        Self {
            frame: hash.frame.raw(),
            value: hash.value.to_string(),
            hex: format!("{:016x}", hash.value),
            source,
        }
    }
}

fn world_record(
    options: &HeadlessOptions,
    event: TelemetryEvent,
    frame: Option<u32>,
    server_hash: Option<HashTelemetry>,
    local_hash: Option<HashTelemetry>,
    mismatch: Option<bool>,
    world: &SimWorld,
    inputs: &[SimInput],
    events: &[SimEvent],
    replay_recovery: ReplayRecoveryTelemetry,
) -> TelemetryRecord {
    TelemetryRecord {
        schema: HEADLESS_TELEMETRY_SCHEMA,
        schema_version: HEADLESS_TELEMETRY_SCHEMA_VERSION,
        event,
        run_id: options.run_id.clone(),
        scenario: options.scenario,
        scene: HEADLESS_LOCKSTEP_SCENE,
        room: options.room.clone(),
        policy: options.policy.clone(),
        player: options.player.clone(),
        server_connected: false,
        frame,
        server_hash,
        local_hash,
        mismatch,
        inputs: inputs.iter().map(InputTelemetry::from).collect(),
        entities: entities_from_world(world),
        events: EventSummaryTelemetry::from_events(events),
        replay_recovery,
        error_code: None,
        failure_stage: None,
        message: None,
    }
}

fn failure_record(
    options: &HeadlessOptions,
    frame: Option<u32>,
    error_code: impl Into<String>,
    failure_stage: impl Into<String>,
    message: impl Into<String>,
    replay_recovery: ReplayRecoveryTelemetry,
    entities: Vec<EntityTelemetry>,
) -> TelemetryRecord {
    TelemetryRecord {
        schema: HEADLESS_TELEMETRY_SCHEMA,
        schema_version: HEADLESS_TELEMETRY_SCHEMA_VERSION,
        event: TelemetryEvent::RunFailed,
        run_id: options.run_id.clone(),
        scenario: options.scenario,
        scene: HEADLESS_LOCKSTEP_SCENE,
        room: options.room.clone(),
        policy: options.policy.clone(),
        player: options.player.clone(),
        server_connected: false,
        frame,
        server_hash: None,
        local_hash: None,
        mismatch: None,
        inputs: Vec::new(),
        entities,
        events: EventSummaryTelemetry::default(),
        replay_recovery,
        error_code: Some(error_code.into()),
        failure_stage: Some(failure_stage.into()),
        message: Some(message.into()),
    }
}

impl From<&SimInput> for InputTelemetry {
    fn from(input: &SimInput) -> Self {
        Self {
            character_id: input.character_id.clone(),
            entity_id: input.entity_id.raw(),
            sequence: input.seq,
            command: command_name(&input.command),
        }
    }
}

fn command_name(command: &SimCommand) -> &'static str {
    match command {
        SimCommand::Move(_) => "move",
        SimCommand::Stop => "stop",
        SimCommand::Face(_) => "face",
        SimCommand::CastSkill(_) => "cast_skill",
        SimCommand::Noop => "noop",
    }
}

fn entities_from_world(world: &SimWorld) -> Vec<EntityTelemetry> {
    world
        .entities_sorted_by_id()
        .iter()
        .map(|entity| EntityTelemetry {
            entity_id: entity.id.raw(),
            owner_character_id: entity.owner_character_id.clone(),
            kind: entity_kind_name(entity.kind),
            team_id: entity.team_id.raw(),
            fixed_position_milli: FixedPointPositionTelemetry {
                x: entity.transform.pos.x.raw(),
                y: entity.transform.pos.y.raw(),
            },
            facing_quantized: direction_telemetry(entity.transform.facing),
            movement_mode: movement_mode_name(entity.movement.mode),
            move_direction_quantized: direction_telemetry(entity.movement.move_dir),
            speed_per_second_milli: entity.movement.speed_per_second.raw(),
            hp: entity.combat.hp,
            max_hp: entity.combat.max_hp,
            alive: entity.alive,
            buffs: entity
                .combat
                .buffs
                .iter()
                .map(|buff| BuffTelemetry {
                    buff_id: buff.buff_id.raw(),
                    source_entity_id: buff.source_entity.raw(),
                    duration_remaining: buff.duration_remaining,
                    interval_remaining: buff.interval_remaining,
                    stacks: buff.stacks,
                })
                .collect(),
        })
        .collect()
}

fn entity_kind_name(kind: EntityKind) -> &'static str {
    match kind {
        EntityKind::Player => "player",
        EntityKind::Npc => "npc",
        EntityKind::Monster => "monster",
        EntityKind::Projectile => "projectile",
        EntityKind::Summon => "summon",
    }
}

fn movement_mode_name(mode: MovementMode) -> &'static str {
    match mode {
        MovementMode::Idle => "idle",
        MovementMode::Controlled => "controlled",
    }
}

fn direction_telemetry(direction: QuantizedDir) -> DirectionTelemetry {
    DirectionTelemetry {
        x: direction.x(),
        y: direction.y(),
    }
}

impl EventSummaryTelemetry {
    fn from_events(events: &[SimEvent]) -> Self {
        let mut by_kind = BTreeMap::new();
        let items = events
            .iter()
            .map(|event| {
                let item = EventTelemetry::from(event);
                *by_kind.entry(item.kind).or_insert(0) += 1;
                item
            })
            .collect();
        Self {
            total: events.len(),
            by_kind,
            items,
        }
    }
}

impl From<&SimEvent> for EventTelemetry {
    fn from(event: &SimEvent) -> Self {
        match event {
            SimEvent::SkillCast {
                source_entity,
                target_entity,
                skill_id,
                value,
                sequence,
                ..
            } => event_telemetry(
                "skill_cast",
                *source_entity,
                *target_entity,
                Some(*skill_id),
                None,
                *value,
                *sequence,
            ),
            SimEvent::DamageApplied {
                source_entity,
                target_entity,
                skill_id,
                buff_id,
                value,
                sequence,
                ..
            } => event_telemetry(
                "damage_applied",
                *source_entity,
                Some(*target_entity),
                *skill_id,
                *buff_id,
                *value,
                *sequence,
            ),
            SimEvent::HealApplied {
                source_entity,
                target_entity,
                skill_id,
                buff_id,
                value,
                sequence,
                ..
            } => event_telemetry(
                "heal_applied",
                *source_entity,
                Some(*target_entity),
                *skill_id,
                *buff_id,
                *value,
                *sequence,
            ),
            SimEvent::BuffApplied {
                source_entity,
                target_entity,
                buff_id,
                value,
                sequence,
                ..
            } => event_telemetry(
                "buff_applied",
                *source_entity,
                Some(*target_entity),
                None,
                Some(*buff_id),
                *value,
                *sequence,
            ),
            SimEvent::BuffExpired {
                source_entity,
                target_entity,
                buff_id,
                value,
                sequence,
                ..
            } => event_telemetry(
                "buff_expired",
                *source_entity,
                Some(*target_entity),
                None,
                Some(*buff_id),
                *value,
                *sequence,
            ),
            SimEvent::EntityDied {
                source_entity,
                target_entity,
                skill_id,
                buff_id,
                value,
                sequence,
                ..
            } => event_telemetry(
                "entity_died",
                *source_entity,
                Some(*target_entity),
                *skill_id,
                *buff_id,
                *value,
                *sequence,
            ),
            SimEvent::BuffTick {
                source_entity,
                target_entity,
                buff_id,
                value,
                sequence,
                ..
            } => event_telemetry(
                "buff_tick",
                *source_entity,
                Some(*target_entity),
                None,
                Some(*buff_id),
                *value,
                *sequence,
            ),
        }
    }
}

fn event_telemetry(
    kind: &'static str,
    source_entity: EntityId,
    target_entity: Option<EntityId>,
    skill_id: Option<SkillId>,
    buff_id: Option<BuffId>,
    value: i32,
    sequence: u32,
) -> EventTelemetry {
    EventTelemetry {
        kind,
        source_entity_id: source_entity.raw(),
        target_entity_id: target_entity.map(EntityId::raw),
        skill_id: skill_id.map(SkillId::raw),
        buff_id: buff_id.map(BuffId::raw),
        value,
        sequence,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lockstep_sim_headless_offline_fixture_is_stable_and_hash_matched() {
        let options = HeadlessOptions::default();
        let first = run_headless(&options);
        let second = run_headless(&options);

        assert_eq!(first, second);
        assert_eq!(first.exit_code, EXIT_SUCCESS);
        assert_eq!(
            first.records.first().unwrap().event,
            TelemetryEvent::RunStarted
        );
        assert_eq!(
            first.records.last().unwrap().event,
            TelemetryEvent::RunCompleted
        );
        assert!(first.records.iter().all(|record| {
            record.event != TelemetryEvent::Frame || record.mismatch == Some(false)
        }));
        assert!(first.records.iter().all(|record| {
            record.server_hash.as_ref().is_none_or(|hash| {
                hash.source == HashSource::OfflineFixtureAuthority && !record.server_connected
            })
        }));

        let mut first_jsonl = Vec::new();
        let mut second_jsonl = Vec::new();
        write_jsonl(&mut first_jsonl, &first.records).unwrap();
        write_jsonl(&mut second_jsonl, &second.records).unwrap();
        assert_eq!(first_jsonl, second_jsonl);
        assert!(first_jsonl.ends_with(b"\n"));
    }

    #[test]
    fn lockstep_sim_headless_fixture_covers_fixed_input_and_dot_events() {
        let run = run_headless(&HeadlessOptions::default());
        let commands = run
            .records
            .iter()
            .flat_map(|record| record.inputs.iter().map(|input| input.command))
            .collect::<Vec<_>>();
        assert_eq!(commands, vec!["move", "face", "stop", "cast_skill"]);

        let event_kinds = run
            .records
            .iter()
            .flat_map(|record| record.events.items.iter().map(|event| event.kind))
            .collect::<Vec<_>>();
        assert!(event_kinds.contains(&"skill_cast"));
        assert!(event_kinds.contains(&"buff_applied"));
        assert!(event_kinds.contains(&"buff_tick"));
        assert!(event_kinds.contains(&"damage_applied"));

        let face_frame = run
            .records
            .iter()
            .find(|record| record.frame == Some(2) && record.event == TelemetryEvent::Frame)
            .unwrap();
        let player = face_frame
            .entities
            .iter()
            .find(|entity| entity.entity_id == FIXTURE_ENTITY_ID.raw())
            .unwrap();
        assert_eq!(player.fixed_position_milli.x, 600);
        assert_eq!(
            player.facing_quantized,
            DirectionTelemetry { x: 0, y: -1000 }
        );
    }

    #[test]
    fn lockstep_sim_headless_recovery_replays_to_authority_hash() {
        let run = run_headless(&HeadlessOptions::default());
        let recovery = run
            .records
            .iter()
            .find(|record| record.event == TelemetryEvent::ReplayRecovery)
            .unwrap();

        assert_eq!(
            recovery.replay_recovery.status,
            ReplayRecoveryStatus::Verified
        );
        assert_eq!(recovery.replay_recovery.checkpoint_frame, Some(2));
        assert_eq!(recovery.replay_recovery.target_frame, Some(6));
        assert_eq!(recovery.replay_recovery.replayed_frames, 4);
        assert_eq!(recovery.mismatch, Some(false));
        assert_eq!(
            recovery.server_hash.as_ref().unwrap().value,
            recovery.local_hash.as_ref().unwrap().value
        );
    }

    #[test]
    fn lockstep_sim_headless_detects_injected_hash_mismatch() {
        let options = HeadlessOptions {
            inject_mismatch_frame: Some(3),
            ..Default::default()
        };
        let run = run_headless(&options);

        assert_eq!(run.exit_code, EXIT_HASH_MISMATCH);
        assert!(run.records.iter().any(|record| {
            record.event == TelemetryEvent::Frame
                && record.frame == Some(3)
                && record.mismatch == Some(true)
        }));
        let failure = run.records.last().unwrap();
        assert_eq!(failure.event, TelemetryEvent::RunFailed);
        assert_eq!(
            failure.error_code.as_deref(),
            Some("HEADLESS_HASH_MISMATCH")
        );
        assert_eq!(failure.failure_stage.as_deref(), Some("frame_compare"));
    }

    #[test]
    fn lockstep_sim_headless_connection_failure_has_stable_code_and_stage() {
        let options = HeadlessOptions {
            scenario: HeadlessScenario::ConnectProbe,
            endpoint: Some("127.0.0.1:1".to_string()),
            ..Default::default()
        };
        let run = run_connect_probe_with(&options, |_, _| {
            Err(io::Error::new(io::ErrorKind::ConnectionRefused, "test"))
        });

        assert_eq!(run.exit_code, EXIT_CONNECTION);
        let failure = run.records.last().unwrap();
        assert_eq!(
            failure.error_code.as_deref(),
            Some("HEADLESS_CONNECT_FAILED")
        );
        assert_eq!(failure.failure_stage.as_deref(), Some("connect"));
        assert!(!failure.server_connected);
    }

    #[test]
    fn lockstep_sim_headless_json_schema_uses_expected_machine_fields() {
        let run = run_headless(&HeadlessOptions::default());
        let frame = run
            .records
            .iter()
            .find(|record| record.event == TelemetryEvent::Frame)
            .unwrap();
        let value = serde_json::to_value(frame).unwrap();

        assert_eq!(value["schema"], HEADLESS_TELEMETRY_SCHEMA);
        assert_eq!(value["schemaVersion"], HEADLESS_TELEMETRY_SCHEMA_VERSION);
        assert_eq!(value["event"], "frame");
        assert_eq!(value["scene"], HEADLESS_LOCKSTEP_SCENE);
        assert_eq!(value["room"], "lockstep-headless-room");
        assert_eq!(value["policy"], "lockstep_sim_demo");
        assert_eq!(value["player"], "lockstep-headless-player");
        assert!(value["serverHash"]["value"].is_string());
        assert!(value["localHash"]["hex"].is_string());
        assert!(value["entities"][0]["fixedPositionMilli"]["x"].is_i64());
        assert!(value.get("replayRecovery").is_some());
        assert!(value.get("errorCode").is_some());
        assert!(value.get("failureStage").is_some());
    }

    #[test]
    fn lockstep_sim_headless_cli_rejects_invalid_scenario() {
        let error = parse_headless_args(["--scenario".to_string(), "not-a-scenario".to_string()])
            .unwrap_err();

        assert_eq!(error.error_code, "HEADLESS_ARGUMENT_INVALID");
        assert_eq!(error.failure_stage, "configuration");
        assert_eq!(error.exit_code, EXIT_CONFIGURATION);
    }
}
