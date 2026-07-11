//! Explicit, windowless lockstep telemetry for local development and CI.
//!
//! This module does not participate in [`crate::run`] and does not implement an
//! alternate MyServer authentication path. The online scenario reuses the
//! production MyServer and authority plugins with a ticket read from an
//! explicitly named environment variable.

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

use crate::game::{
    OnlineDualHeadlessOptions, OnlineDualHeadlessReport, OnlineHeadlessOptions,
    OnlineHeadlessReport, OnlineReconnectObserverOptions, OnlineReconnectObserverReport,
    OnlineRecoveryStreamReport, run_online_dual_headless, run_online_headless,
    run_online_reconnect_observer_headless,
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
const ONLINE_DUAL_OBSERVATION_FRAMES: u32 = 2;

pub const HEADLESS_HELP: &str = "\
mybevy lockstep simulation headless telemetry\n\
\n\
Usage:\n\
  lockstep-sim-headless [options]\n\
\n\
Options:\n\
  --scenario <offline-fixture|connect-probe|online-single-client|online-dual-client|online-reconnect-observer>\n\
  --run-id <id>\n\
  --room <room>\n\
  --policy <policy>\n\
  --player <player>\n\
  --inject-mismatch-frame <frame>  Explicit local test fault for diagnostics\n\
  --endpoint <ip:port>             Required by connect-probe\n\
  --ticket-env <name>              Required by online-single-client\n\
  --observer-ticket-env <name>     Second ticket for dual or reconnect-observer scenarios\n\
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
    OnlineSingleClient,
    OnlineDualClient,
    OnlineReconnectObserver,
}

impl HeadlessScenario {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "offline-fixture" => Some(Self::OfflineFixture),
            "connect-probe" => Some(Self::ConnectProbe),
            "online-single-client" => Some(Self::OnlineSingleClient),
            "online-dual-client" => Some(Self::OnlineDualClient),
            "online-reconnect-observer" => Some(Self::OnlineReconnectObserver),
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
    pub ticket_env: Option<String>,
    pub observer_ticket_env: Option<String>,
    pub connect_timeout: Duration,
    pub client_role: Option<HeadlessClientRole>,
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
            ticket_env: None,
            observer_ticket_env: None,
            connect_timeout: Duration::from_millis(500),
            client_role: None,
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
    pub client_role: Option<HeadlessClientRole>,
    pub server_connected: bool,
    pub frame: Option<u32>,
    pub server_hash: Option<HashTelemetry>,
    pub local_hash: Option<HashTelemetry>,
    pub mismatch: Option<bool>,
    pub inputs: Vec<InputTelemetry>,
    pub entities: Vec<EntityTelemetry>,
    pub events: EventSummaryTelemetry,
    pub replay_recovery: ReplayRecoveryTelemetry,
    pub recovery_acceptance: Option<RecoveryAcceptanceTelemetry>,
    pub error_code: Option<String>,
    pub failure_stage: Option<String>,
    pub message: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HeadlessClientRole {
    ActiveInput,
    PassiveReplay,
    ReconnectPrimary,
    Observer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryEvent {
    RunStarted,
    Frame,
    TransportDisconnected,
    SnapshotRecovered,
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
    MyServerAuthority,
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryAcceptanceTelemetry {
    pub pre_disconnect_frame: Option<u32>,
    pub pre_disconnect_hash: Option<String>,
    pub pre_disconnect_input_frame: Option<u32>,
    pub pre_disconnect_input_commands: Vec<&'static str>,
    pub pre_disconnect_event_kinds: Vec<&'static str>,
    pub disconnect_generation: Option<u64>,
    pub snapshot_frame: u32,
    pub snapshot_hash: String,
    pub response_current_frame: u32,
    pub response_waiting_frame: u32,
    pub response_recent_input_frames: Vec<u32>,
    pub response_waiting_input_frames: Vec<u32>,
    pub recovery_generation: u64,
    pub continuity_start_frame: Option<u32>,
    pub continuity_end_frame: Option<u32>,
    pub continuity_frame_count: usize,
    pub contiguous_without_duplicate_apply: bool,
    pub ignored_duplicate_or_old_frames: u64,
    pub post_reconnect_input_frame: u32,
    pub post_reconnect_input_application_count: usize,
    pub local_input_acknowledgements: usize,
    pub has_control_binding: bool,
    pub common_frame_start: u32,
    pub common_frame_end: u32,
    pub common_frame_count: usize,
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
                        "unsupported scenario {value:?}; expected offline-fixture, connect-probe, online-single-client, online-dual-client, or online-reconnect-observer"
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
            "--ticket-env" => {
                options.ticket_env = Some(non_empty(
                    next_value(&mut args, "--ticket-env")?,
                    "--ticket-env",
                )?);
            }
            "--observer-ticket-env" => {
                options.observer_ticket_env = Some(non_empty(
                    next_value(&mut args, "--observer-ticket-env")?,
                    "--observer-ticket-env",
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
        HeadlessScenario::OnlineSingleClient => run_online_single_client(options),
        HeadlessScenario::OnlineDualClient => run_online_dual_client(options),
        HeadlessScenario::OnlineReconnectObserver => run_online_reconnect_observer(options),
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

fn run_online_single_client(options: &HeadlessOptions) -> HeadlessRun {
    let Some(endpoint) = options.endpoint.as_deref() else {
        return failure_run(
            options,
            "HEADLESS_ENDPOINT_REQUIRED",
            "configuration",
            "online-single-client requires --endpoint <ip:port>",
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
                "online-single-client endpoint must be a numeric ip:port",
                EXIT_CONFIGURATION,
            );
        }
    };
    let Some(ticket_env) = options.ticket_env.as_deref() else {
        return failure_run(
            options,
            "HEADLESS_TICKET_ENV_REQUIRED",
            "configuration",
            "online-single-client requires --ticket-env <name>",
            EXIT_CONFIGURATION,
        );
    };

    let report = match run_online_headless(&OnlineHeadlessOptions {
        endpoint: address,
        ticket_env: ticket_env.to_string(),
        room: options.room.clone(),
        policy: options.policy.clone(),
        player: options.player.clone(),
        timeout: options.connect_timeout,
    }) {
        Ok(report) => report,
        Err(error) => {
            return failure_run(
                options,
                error.error_code,
                error.failure_stage,
                error.message,
                EXIT_CONNECTION,
            );
        }
    };

    online_report_to_run(options, report)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct OnlineDualMismatch {
    frame: u32,
    error_code: &'static str,
    message: String,
}

fn run_online_dual_client(options: &HeadlessOptions) -> HeadlessRun {
    let Some(endpoint) = options.endpoint.as_deref() else {
        return failure_run(
            options,
            "HEADLESS_ENDPOINT_REQUIRED",
            "configuration",
            "online-dual-client requires --endpoint <ip:port>",
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
                "online-dual-client endpoint must be a numeric ip:port",
                EXIT_CONFIGURATION,
            );
        }
    };
    let Some(primary_ticket_env) = options.ticket_env.as_deref() else {
        return failure_run(
            options,
            "HEADLESS_TICKET_ENV_REQUIRED",
            "configuration",
            "online-dual-client requires --ticket-env <name>",
            EXIT_CONFIGURATION,
        );
    };
    let Some(passive_ticket_env) = options.observer_ticket_env.as_deref() else {
        return failure_run(
            options,
            "HEADLESS_OBSERVER_TICKET_ENV_REQUIRED",
            "configuration",
            "online-dual-client requires --observer-ticket-env <name>",
            EXIT_CONFIGURATION,
        );
    };

    let report = match run_online_dual_headless(&OnlineDualHeadlessOptions {
        endpoint: address,
        primary_ticket_env: primary_ticket_env.to_string(),
        passive_ticket_env: passive_ticket_env.to_string(),
        room: options.room.clone(),
        policy: options.policy.clone(),
        primary_player: options.player.clone(),
        passive_player: format!("{}-passive", options.player),
        timeout: options.connect_timeout,
    }) {
        Ok(report) => report,
        Err(error) => {
            return failure_run(
                options,
                error.error_code,
                error.failure_stage,
                error.message,
                EXIT_CONNECTION,
            );
        }
    };

    online_dual_report_to_run(options, report)
}

fn run_online_reconnect_observer(options: &HeadlessOptions) -> HeadlessRun {
    let Some(endpoint) = options.endpoint.as_deref() else {
        return failure_run(
            options,
            "HEADLESS_ENDPOINT_REQUIRED",
            "configuration",
            "online-reconnect-observer requires --endpoint <ip:port>",
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
                "online-reconnect-observer endpoint must be a numeric ip:port",
                EXIT_CONFIGURATION,
            );
        }
    };
    let Some(primary_ticket_env) = options.ticket_env.as_deref() else {
        return failure_run(
            options,
            "HEADLESS_TICKET_ENV_REQUIRED",
            "configuration",
            "online-reconnect-observer requires --ticket-env <name>",
            EXIT_CONFIGURATION,
        );
    };
    let Some(observer_ticket_env) = options.observer_ticket_env.as_deref() else {
        return failure_run(
            options,
            "HEADLESS_OBSERVER_TICKET_ENV_REQUIRED",
            "configuration",
            "online-reconnect-observer requires --observer-ticket-env <name>",
            EXIT_CONFIGURATION,
        );
    };

    let report = match run_online_reconnect_observer_headless(&OnlineReconnectObserverOptions {
        endpoint: address,
        primary_ticket_env: primary_ticket_env.to_string(),
        observer_ticket_env: observer_ticket_env.to_string(),
        room: options.room.clone(),
        policy: options.policy.clone(),
        primary_player: options.player.clone(),
        observer_player: format!("{}-observer", options.player),
        timeout: options.connect_timeout,
    }) {
        Ok(report) => report,
        Err(error) => {
            let exit_code = if error.failure_stage == "configuration" {
                EXIT_CONFIGURATION
            } else {
                EXIT_RECOVERY
            };
            return failure_run(
                options,
                error.error_code,
                error.failure_stage,
                error.message,
                exit_code,
            );
        }
    };

    online_reconnect_observer_report_to_run(options, report)
}

fn online_reconnect_observer_report_to_run(
    options: &HeadlessOptions,
    report: OnlineReconnectObserverReport,
) -> HeadlessRun {
    let mut primary_options = options.clone();
    primary_options.player = report.primary.player_id.clone();
    primary_options.client_role = Some(HeadlessClientRole::ReconnectPrimary);
    let mut observer_options = options.clone();
    observer_options.player = report.observer.player_id.clone();
    observer_options.client_role = Some(HeadlessClientRole::Observer);

    let primary_acceptance = recovery_acceptance(&report, &report.primary, true);
    let observer_acceptance = recovery_acceptance(&report, &report.observer, false);
    let mut records = Vec::new();

    records.push(connected_world_record(
        &primary_options,
        TelemetryEvent::RunStarted,
        Some(report.pre_disconnect.frame),
        Some(HashTelemetry::new(
            report.pre_disconnect.hash,
            HashSource::MyServerAuthority,
        )),
        Some(HashTelemetry::new(
            report.pre_disconnect.hash,
            HashSource::LocalReplay,
        )),
        Some(false),
        &report.pre_disconnect.world,
        &report.pre_disconnect.inputs,
        &report.pre_disconnect.events,
        ReplayRecoveryTelemetry {
            status: ReplayRecoveryStatus::CheckpointCaptured,
            checkpoint_frame: Some(report.pre_disconnect.frame),
            target_frame: report.primary.frames.last().map(|frame| frame.frame),
            replayed_frames: 0,
        },
    ));
    let mut disconnected = connected_world_record(
        &primary_options,
        TelemetryEvent::TransportDisconnected,
        Some(report.pre_disconnect.frame),
        Some(HashTelemetry::new(
            report.pre_disconnect.hash,
            HashSource::MyServerAuthority,
        )),
        Some(HashTelemetry::new(
            report.pre_disconnect.hash,
            HashSource::LocalReplay,
        )),
        Some(false),
        &report.pre_disconnect.world,
        &report.pre_disconnect.inputs,
        &report.pre_disconnect.events,
        ReplayRecoveryTelemetry {
            status: ReplayRecoveryStatus::Pending,
            checkpoint_frame: Some(report.pre_disconnect.frame),
            target_frame: None,
            replayed_frames: 0,
        },
    );
    disconnected.server_connected = false;
    records.push(disconnected);
    records.extend(recovery_stream_records(
        &primary_options,
        &report.primary,
        &primary_acceptance,
        false,
    ));

    records.push(connected_world_record(
        &observer_options,
        TelemetryEvent::RunStarted,
        Some(report.observer.snapshot_frame),
        Some(HashTelemetry::new(
            report.observer.snapshot_hash,
            HashSource::MyServerAuthority,
        )),
        Some(HashTelemetry::new(
            report.observer.snapshot_hash,
            HashSource::SnapshotRecovery,
        )),
        Some(false),
        &report.observer.snapshot_world,
        &[],
        &[],
        ReplayRecoveryTelemetry {
            status: ReplayRecoveryStatus::CheckpointCaptured,
            checkpoint_frame: Some(report.observer.snapshot_frame),
            target_frame: report.observer.frames.last().map(|frame| frame.frame),
            replayed_frames: 0,
        },
    ));
    records.extend(recovery_stream_records(
        &observer_options,
        &report.observer,
        &observer_acceptance,
        true,
    ));

    HeadlessRun {
        records,
        exit_code: EXIT_SUCCESS,
    }
}

fn recovery_stream_records(
    options: &HeadlessOptions,
    stream: &OnlineRecoveryStreamReport,
    acceptance: &RecoveryAcceptanceTelemetry,
    observer: bool,
) -> Vec<TelemetryRecord> {
    let final_frame = stream
        .frames
        .last()
        .map(|frame| frame.frame)
        .unwrap_or(stream.snapshot_frame);
    let final_hash = stream
        .frames
        .last()
        .map(|frame| frame.local_hash)
        .unwrap_or(stream.snapshot_hash);
    let final_world = stream
        .frames
        .last()
        .map(|frame| &frame.world)
        .unwrap_or(&stream.snapshot_world);
    let recovery = ReplayRecoveryTelemetry {
        status: ReplayRecoveryStatus::Verified,
        checkpoint_frame: Some(stream.snapshot_frame),
        target_frame: Some(final_frame),
        replayed_frames: stream.frames.len() as u32,
    };
    let mut records = Vec::new();
    let mut snapshot_record = connected_world_record(
        options,
        TelemetryEvent::SnapshotRecovered,
        Some(stream.snapshot_frame),
        Some(HashTelemetry::new(
            stream.snapshot_hash,
            HashSource::MyServerAuthority,
        )),
        Some(HashTelemetry::new(
            stream.snapshot_hash,
            HashSource::SnapshotRecovery,
        )),
        Some(false),
        &stream.snapshot_world,
        &[],
        &[],
        ReplayRecoveryTelemetry {
            status: ReplayRecoveryStatus::CheckpointCaptured,
            checkpoint_frame: Some(stream.snapshot_frame),
            target_frame: Some(final_frame),
            replayed_frames: 0,
        },
    );
    snapshot_record.recovery_acceptance = Some(acceptance.clone());
    records.push(snapshot_record);

    for frame in &stream.frames {
        let mut record = connected_world_record(
            options,
            TelemetryEvent::Frame,
            Some(frame.frame),
            frame
                .server_hash
                .map(|hash| HashTelemetry::new(hash, HashSource::MyServerAuthority)),
            Some(HashTelemetry::new(
                frame.local_hash,
                HashSource::LocalReplay,
            )),
            Some(false),
            &frame.world,
            &frame.inputs,
            &frame.events,
            ReplayRecoveryTelemetry {
                status: ReplayRecoveryStatus::Pending,
                checkpoint_frame: Some(stream.snapshot_frame),
                target_frame: Some(final_frame),
                replayed_frames: frame.frame.saturating_sub(stream.snapshot_frame),
            },
        );
        if observer {
            record.recovery_acceptance = Some(acceptance.clone());
        }
        records.push(record);
    }

    for event in [TelemetryEvent::ReplayRecovery, TelemetryEvent::RunCompleted] {
        let mut record = connected_world_record(
            options,
            event,
            Some(final_frame),
            Some(HashTelemetry::new(
                final_hash,
                HashSource::MyServerAuthority,
            )),
            Some(HashTelemetry::new(final_hash, HashSource::LocalReplay)),
            Some(false),
            final_world,
            &[],
            &[],
            recovery.clone(),
        );
        record.recovery_acceptance = Some(acceptance.clone());
        records.push(record);
    }
    records
}

fn recovery_acceptance(
    report: &OnlineReconnectObserverReport,
    stream: &OnlineRecoveryStreamReport,
    primary: bool,
) -> RecoveryAcceptanceTelemetry {
    let post_reconnect_input_application_count = stream
        .frames
        .iter()
        .flat_map(|frame| frame.inputs.iter())
        .filter(|input| input.frame.raw() == report.post_reconnect_input_frame && input.seq == 2)
        .count();
    RecoveryAcceptanceTelemetry {
        pre_disconnect_frame: primary.then_some(report.pre_disconnect.frame),
        pre_disconnect_hash: primary.then(|| format!("{:016x}", report.pre_disconnect.hash.value)),
        pre_disconnect_input_frame: primary.then_some(report.first_input_frame),
        pre_disconnect_input_commands: primary
            .then(|| {
                report
                    .pre_disconnect
                    .inputs
                    .iter()
                    .map(|input| command_name(&input.command))
                    .collect()
            })
            .unwrap_or_default(),
        pre_disconnect_event_kinds: primary
            .then(|| {
                report
                    .pre_disconnect
                    .events
                    .iter()
                    .map(|event| EventTelemetry::from(event).kind)
                    .collect()
            })
            .unwrap_or_default(),
        disconnect_generation: primary.then_some(report.disconnect_generation),
        snapshot_frame: stream.snapshot_frame,
        snapshot_hash: format!("{:016x}", stream.snapshot_hash.value),
        response_current_frame: stream.response_current_frame,
        response_waiting_frame: stream.response_waiting_frame,
        response_recent_input_frames: stream.response_recent_input_frames.clone(),
        response_waiting_input_frames: stream.response_waiting_input_frames.clone(),
        recovery_generation: stream.recovery_generation,
        continuity_start_frame: stream.frames.first().map(|frame| frame.frame),
        continuity_end_frame: stream.frames.last().map(|frame| frame.frame),
        continuity_frame_count: stream.frames.len(),
        contiguous_without_duplicate_apply: stream
            .frames
            .windows(2)
            .all(|frames| frames[1].frame == frames[0].frame.saturating_add(1)),
        ignored_duplicate_or_old_frames: stream.ignored_duplicate_or_old_frames,
        post_reconnect_input_frame: report.post_reconnect_input_frame,
        post_reconnect_input_application_count,
        local_input_acknowledgements: stream.local_input_acknowledgements,
        has_control_binding: stream.has_control_binding,
        common_frame_start: report.common_frames[0],
        common_frame_end: *report.common_frames.last().expect("common recovery frames"),
        common_frame_count: report.common_frames.len(),
    }
}

fn online_dual_report_to_run(
    options: &HeadlessOptions,
    report: OnlineDualHeadlessReport,
) -> HeadlessRun {
    let reconciliation = reconcile_online_dual_reports(&report.active, &report.passive);
    let active_player = report.active.player_id.clone();
    let passive_player = report.passive.player_id.clone();

    let mut active_options = options.clone();
    active_options.player = active_player.clone();
    active_options.client_role = Some(HeadlessClientRole::ActiveInput);
    let mut passive_options = options.clone();
    passive_options.player = passive_player;
    passive_options.client_role = Some(HeadlessClientRole::PassiveReplay);

    let active_final_world = report
        .active
        .frames
        .last()
        .map(|frame| frame.world.clone())
        .unwrap_or_else(|| report.active.initial_world.clone());
    let active_run =
        online_report_to_run_for_role(&active_options, report.active, true, Some(&active_player));
    let passive_run = online_report_to_run_for_role(
        &passive_options,
        report.passive,
        false,
        Some(&active_player),
    );
    let mut records = active_run.records;
    records.extend(passive_run.records);
    if active_run.exit_code != EXIT_SUCCESS || passive_run.exit_code != EXIT_SUCCESS {
        return HeadlessRun {
            records,
            exit_code: active_run.exit_code.max(passive_run.exit_code),
        };
    }

    if let Err(mismatch) = reconciliation {
        records.push(connected_failure_record(
            &active_options,
            Some(mismatch.frame),
            mismatch.error_code,
            "dual_frame_compare",
            mismatch.message,
            ReplayRecoveryTelemetry {
                status: ReplayRecoveryStatus::Failed,
                checkpoint_frame: Some(0),
                target_frame: Some(mismatch.frame),
                replayed_frames: mismatch.frame,
            },
            entities_from_world(&active_final_world),
        ));
        return HeadlessRun {
            records,
            exit_code: if mismatch.error_code == "HEADLESS_DUAL_HASH_MISMATCH" {
                EXIT_HASH_MISMATCH
            } else {
                EXIT_SIMULATION
            },
        };
    }

    HeadlessRun {
        records,
        exit_code: EXIT_SUCCESS,
    }
}

fn reconcile_online_dual_reports(
    active: &OnlineHeadlessReport,
    passive: &OnlineHeadlessReport,
) -> Result<Vec<u32>, OnlineDualMismatch> {
    let initial_frame = active
        .initial_hash
        .frame
        .raw()
        .max(passive.initial_hash.frame.raw());
    if active.player_id == passive.player_id {
        return Err(OnlineDualMismatch {
            frame: initial_frame,
            error_code: "HEADLESS_DUAL_PLAYER_NOT_DISTINCT",
            message: "active and passive telemetry identify the same gameplay character"
                .to_string(),
        });
    }
    if active.initial_hash != passive.initial_hash || active.initial_world != passive.initial_world
    {
        return Err(OnlineDualMismatch {
            frame: initial_frame,
            error_code: "HEADLESS_DUAL_INITIAL_STATE_MISMATCH",
            message: "active and passive clients restored different initial authority state"
                .to_string(),
        });
    }

    let passive_frames = passive
        .frames
        .iter()
        .map(|frame| (frame.frame, frame))
        .collect::<BTreeMap<_, _>>();
    let mut common_frames = Vec::new();
    let mut observed_sequences = Vec::new();
    for active_frame in &active.frames {
        let Some(passive_frame) = passive_frames.get(&active_frame.frame) else {
            continue;
        };
        let frame = active_frame.frame;
        common_frames.push(frame);
        let active_server_hash = active_frame.server_hash.ok_or_else(|| OnlineDualMismatch {
            frame,
            error_code: "HEADLESS_DUAL_SERVER_HASH_MISSING",
            message: format!("active client has no server hash at frame {frame}"),
        })?;
        let passive_server_hash = passive_frame
            .server_hash
            .ok_or_else(|| OnlineDualMismatch {
                frame,
                error_code: "HEADLESS_DUAL_SERVER_HASH_MISSING",
                message: format!("passive client has no server hash at frame {frame}"),
            })?;
        if active_server_hash != active_frame.local_hash
            || passive_server_hash != passive_frame.local_hash
            || active_server_hash != passive_server_hash
        {
            return Err(OnlineDualMismatch {
                frame,
                error_code: "HEADLESS_DUAL_HASH_MISMATCH",
                message: format!(
                    "server, active local, and passive local hashes differ at frame {frame}"
                ),
            });
        }
        if active_frame.world != passive_frame.world {
            return Err(OnlineDualMismatch {
                frame,
                error_code: "HEADLESS_DUAL_ENTITY_MISMATCH",
                message: format!("active and passive fixed entity state differs at frame {frame}"),
            });
        }
        if active_frame.events != passive_frame.events {
            return Err(OnlineDualMismatch {
                frame,
                error_code: "HEADLESS_DUAL_EVENT_MISMATCH",
                message: format!(
                    "active and passive deterministic event sequence differs at frame {frame}"
                ),
            });
        }
        if active_frame.inputs != passive_frame.inputs {
            return Err(OnlineDualMismatch {
                frame,
                error_code: "HEADLESS_DUAL_INPUT_MISMATCH",
                message: format!(
                    "active and passive authority input sequence differs at frame {frame}"
                ),
            });
        }
        for input in &active_frame.inputs {
            if input.character_id != active.player_id {
                return Err(OnlineDualMismatch {
                    frame,
                    error_code: "HEADLESS_DUAL_INPUT_SOURCE_MISMATCH",
                    message: format!(
                        "authority input at frame {frame} did not originate from the active client"
                    ),
                });
            }
            observed_sequences.push(input.seq);
        }
    }

    let required_last_frame = active
        .stop_frame
        .max(passive.stop_frame)
        .saturating_add(ONLINE_DUAL_OBSERVATION_FRAMES);
    if common_frames
        .first()
        .is_none_or(|frame| *frame > active.input_frame)
        || common_frames
            .last()
            .is_none_or(|frame| *frame < required_last_frame)
    {
        return Err(OnlineDualMismatch {
            frame: common_frames.last().copied().unwrap_or(initial_frame),
            error_code: "HEADLESS_DUAL_COMMON_FRAME_RANGE_INCOMPLETE",
            message: "dual telemetry does not cover the active input through observation frame intersection"
                .to_string(),
        });
    }
    if !observed_sequences.contains(&1) || !observed_sequences.contains(&2) {
        return Err(OnlineDualMismatch {
            frame: common_frames.last().copied().unwrap_or(initial_frame),
            error_code: "HEADLESS_DUAL_INPUT_SEQUENCE_MISSING",
            message: "dual telemetry did not observe active input sequences 1 and 2".to_string(),
        });
    }
    Ok(common_frames)
}

fn online_report_to_run(options: &HeadlessOptions, report: OnlineHeadlessReport) -> HeadlessRun {
    online_report_to_run_for_role(options, report, true, None)
}

fn online_report_to_run_for_role(
    options: &HeadlessOptions,
    report: OnlineHeadlessReport,
    expect_controlled_movement: bool,
    expected_input_player: Option<&str>,
) -> HeadlessRun {
    let mut online_options = options.clone();
    online_options.player = report.player_id.clone();
    let final_frame = report
        .frames
        .last()
        .map(|frame| frame.frame)
        .unwrap_or(report.initial_hash.frame.raw());
    let recovery = ReplayRecoveryTelemetry {
        status: ReplayRecoveryStatus::Verified,
        checkpoint_frame: Some(report.initial_hash.frame.raw()),
        target_frame: Some(final_frame),
        replayed_frames: report.frames.len() as u32,
    };
    let mut records = vec![connected_world_record(
        &online_options,
        TelemetryEvent::RunStarted,
        Some(report.initial_hash.frame.raw()),
        Some(HashTelemetry::new(
            report.initial_hash,
            HashSource::MyServerAuthority,
        )),
        Some(HashTelemetry::new(
            report.initial_hash,
            HashSource::LocalReplay,
        )),
        Some(false),
        &report.initial_world,
        &[],
        &[],
        ReplayRecoveryTelemetry {
            status: ReplayRecoveryStatus::CheckpointCaptured,
            checkpoint_frame: Some(report.initial_hash.frame.raw()),
            target_frame: Some(final_frame),
            replayed_frames: 0,
        },
    )];

    let mut first_failure = None;
    let mut observed_move = false;
    let mut observed_cast = false;
    let mut observed_stop = false;
    let mut observed_skill = false;
    let mut observed_damage_or_buff = false;
    let mut unexpected_input_source = false;
    let initial_player_x = report
        .initial_world
        .entity(report.player_entity_id)
        .map(|entity| entity.transform.pos.x.raw());

    for frame in &report.frames {
        let mismatch = frame
            .server_hash
            .map(|server_hash| server_hash != frame.local_hash);
        observed_move |= frame
            .inputs
            .iter()
            .any(|input| matches!(input.command, SimCommand::Move(_)));
        observed_cast |= frame
            .inputs
            .iter()
            .any(|input| matches!(input.command, SimCommand::CastSkill(_)));
        observed_stop |= frame
            .inputs
            .iter()
            .any(|input| matches!(input.command, SimCommand::Stop));
        observed_skill |= frame
            .events
            .iter()
            .any(|event| matches!(event, SimEvent::SkillCast { .. }));
        observed_damage_or_buff |= frame.events.iter().any(|event| {
            matches!(
                event,
                SimEvent::DamageApplied { .. }
                    | SimEvent::BuffApplied { .. }
                    | SimEvent::BuffTick { .. }
            )
        });
        unexpected_input_source |= expected_input_player.is_some_and(|expected| {
            frame
                .inputs
                .iter()
                .any(|input| input.character_id != expected)
        });

        records.push(connected_world_record(
            &online_options,
            TelemetryEvent::Frame,
            Some(frame.frame),
            frame
                .server_hash
                .map(|hash| HashTelemetry::new(hash, HashSource::MyServerAuthority)),
            Some(HashTelemetry::new(
                frame.local_hash,
                HashSource::LocalReplay,
            )),
            mismatch,
            &frame.world,
            &frame.inputs,
            &frame.events,
            ReplayRecoveryTelemetry {
                status: ReplayRecoveryStatus::Pending,
                checkpoint_frame: Some(report.initial_hash.frame.raw()),
                target_frame: Some(final_frame),
                replayed_frames: frame.frame.saturating_sub(report.initial_hash.frame.raw()),
            },
        ));

        if first_failure.is_none() && frame.server_hash.is_none() {
            first_failure = Some((
                frame.frame,
                "HEADLESS_SERVER_HASH_MISSING",
                "frame_compare",
                format!(
                    "MyServer authority frame {} did not include a server hash",
                    frame.frame
                ),
            ));
        } else if first_failure.is_none() && mismatch == Some(true) {
            first_failure = Some((
                frame.frame,
                "HEADLESS_HASH_MISMATCH",
                "frame_compare",
                format!(
                    "MyServer authority and local replay hashes differ at frame {}",
                    frame.frame
                ),
            ));
        }
    }

    let final_world = report
        .frames
        .last()
        .map(|frame| &frame.world)
        .unwrap_or(&report.initial_world);
    let final_player_x = final_world
        .entity(report.player_entity_id)
        .map(|entity| entity.transform.pos.x.raw());
    let controlled_movement_matches_role = initial_player_x.is_some()
        && if expect_controlled_movement {
            final_player_x != initial_player_x
        } else {
            final_player_x == initial_player_x
        };
    if first_failure.is_none()
        && (!observed_move || !observed_cast || !observed_stop || !controlled_movement_matches_role)
    {
        first_failure = Some((
            final_frame,
            "HEADLESS_MOVEMENT_NOT_OBSERVED",
            "scenario_assertion",
            if expect_controlled_movement {
                "active input telemetry did not prove move/cast/stop and controlled fixed-position movement"
                    .to_string()
            } else {
                "passive replay telemetry did not prove move/cast/stop while keeping its controlled entity unchanged"
                    .to_string()
            },
        ));
    }
    if first_failure.is_none() && unexpected_input_source {
        first_failure = Some((
            final_frame,
            "HEADLESS_DUAL_INPUT_SOURCE_MISMATCH",
            "input_role",
            "dual-client authority frames contained input from a non-active character".to_string(),
        ));
    }
    if first_failure.is_none() && (!observed_skill || !observed_damage_or_buff) {
        first_failure = Some((
            final_frame,
            "HEADLESS_COMBAT_EVENTS_MISSING",
            "scenario_assertion",
            "online event telemetry did not include SkillCast and damage or Buff evidence"
                .to_string(),
        ));
    }
    let required_hud_fields = [
        "room=",
        "policy=",
        "frame=",
        "local_hash=",
        "server_hash=",
        "mismatch=",
        "events=",
    ];
    if first_failure.is_none()
        && required_hud_fields
            .iter()
            .any(|field| !report.hud_status.contains(field))
    {
        first_failure = Some((
            final_frame,
            "HEADLESS_HUD_INCOMPLETE",
            "scenario_assertion",
            "lockstep HUD status is missing required online diagnostic fields".to_string(),
        ));
    }

    if let Some((frame, code, stage, message)) = first_failure {
        records.push(connected_failure_record(
            &online_options,
            Some(frame),
            code,
            stage,
            message,
            ReplayRecoveryTelemetry {
                status: ReplayRecoveryStatus::Failed,
                checkpoint_frame: Some(report.initial_hash.frame.raw()),
                target_frame: Some(frame),
                replayed_frames: frame.saturating_sub(report.initial_hash.frame.raw()),
            },
            entities_from_world(final_world),
        ));
        return HeadlessRun {
            records,
            exit_code: if code == "HEADLESS_HASH_MISMATCH" {
                EXIT_HASH_MISMATCH
            } else {
                EXIT_SIMULATION
            },
        };
    }

    let final_hash = report
        .frames
        .last()
        .map(|frame| frame.local_hash)
        .unwrap_or(report.initial_hash);
    records.push(connected_world_record(
        &online_options,
        TelemetryEvent::ReplayRecovery,
        Some(final_frame),
        Some(HashTelemetry::new(
            final_hash,
            HashSource::MyServerAuthority,
        )),
        Some(HashTelemetry::new(final_hash, HashSource::LocalReplay)),
        Some(false),
        final_world,
        &[],
        &[],
        recovery.clone(),
    ));
    records.push(connected_world_record(
        &online_options,
        TelemetryEvent::RunCompleted,
        Some(final_frame),
        Some(HashTelemetry::new(
            final_hash,
            HashSource::MyServerAuthority,
        )),
        Some(HashTelemetry::new(final_hash, HashSource::LocalReplay)),
        Some(false),
        final_world,
        &[],
        &[],
        recovery,
    ));
    HeadlessRun {
        records,
        exit_code: EXIT_SUCCESS,
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
        client_role: options.client_role,
        server_connected: false,
        frame,
        server_hash,
        local_hash,
        mismatch,
        inputs: inputs.iter().map(InputTelemetry::from).collect(),
        entities: entities_from_world(world),
        events: EventSummaryTelemetry::from_events(events),
        replay_recovery,
        recovery_acceptance: None,
        error_code: None,
        failure_stage: None,
        message: None,
    }
}

#[allow(clippy::too_many_arguments)]
fn connected_world_record(
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
    let mut record = world_record(
        options,
        event,
        frame,
        server_hash,
        local_hash,
        mismatch,
        world,
        inputs,
        events,
        replay_recovery,
    );
    record.server_connected = true;
    record
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
        client_role: options.client_role,
        server_connected: false,
        frame,
        server_hash: None,
        local_hash: None,
        mismatch: None,
        inputs: Vec::new(),
        entities,
        events: EventSummaryTelemetry::default(),
        replay_recovery,
        recovery_acceptance: None,
        error_code: Some(error_code.into()),
        failure_stage: Some(failure_stage.into()),
        message: Some(message.into()),
    }
}

fn connected_failure_record(
    options: &HeadlessOptions,
    frame: Option<u32>,
    error_code: impl Into<String>,
    failure_stage: impl Into<String>,
    message: impl Into<String>,
    replay_recovery: ReplayRecoveryTelemetry,
    entities: Vec<EntityTelemetry>,
) -> TelemetryRecord {
    let mut record = failure_record(
        options,
        frame,
        error_code,
        failure_stage,
        message,
        replay_recovery,
        entities,
    );
    record.server_connected = true;
    record
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
    use crate::game::OnlineHeadlessFrame;

    fn dual_test_reports() -> (OnlineHeadlessReport, OnlineHeadlessReport) {
        let options = HeadlessOptions {
            player: "active-player".to_string(),
            ..Default::default()
        };
        let (mut initial_world, _) = fixture_world_and_config(&options).unwrap();
        let mut passive_entity = initial_world.entity(FIXTURE_ENTITY_ID).unwrap().clone();
        passive_entity.id = EntityId::new(FIXTURE_ENTITY_ID.raw() + 1);
        passive_entity.owner_character_id = Some("passive-player".to_string());
        initial_world.entities.push(passive_entity);
        initial_world.sort_entities_by_id();
        let initial_hash = hash_world(&initial_world);
        let frames = (4..=6)
            .map(|frame| {
                let mut world = initial_world.clone();
                world.frame = FrameId::new(frame);
                let local_hash = hash_world(&world);
                let inputs = match frame {
                    4 => vec![SimInput {
                        frame: FrameId::new(frame),
                        character_id: "active-player".to_string(),
                        entity_id: FIXTURE_ENTITY_ID,
                        seq: 1,
                        source: SimInputSource::Real,
                        command: SimCommand::Move(MoveCommand {
                            dir: QuantizedDir::RIGHT,
                            speed_per_second: Some(Fp::from_i32(6)),
                        }),
                    }],
                    5 => vec![SimInput {
                        frame: FrameId::new(frame),
                        character_id: "active-player".to_string(),
                        entity_id: FIXTURE_ENTITY_ID,
                        seq: 2,
                        source: SimInputSource::Real,
                        command: SimCommand::Stop,
                    }],
                    _ => Vec::new(),
                };
                OnlineHeadlessFrame {
                    frame,
                    server_hash: Some(local_hash),
                    local_hash,
                    world,
                    inputs,
                    events: Vec::new(),
                }
            })
            .collect::<Vec<_>>();
        let active = OnlineHeadlessReport {
            player_id: "active-player".to_string(),
            player_entity_id: FIXTURE_ENTITY_ID,
            input_frame: 4,
            stop_frame: 4,
            initial_world: initial_world.clone(),
            initial_hash,
            frames: frames.clone(),
            hud_status: String::new(),
        };
        let passive = OnlineHeadlessReport {
            player_id: "passive-player".to_string(),
            player_entity_id: EntityId::new(FIXTURE_ENTITY_ID.raw() + 1),
            input_frame: 4,
            stop_frame: 4,
            initial_world,
            initial_hash,
            frames,
            hud_status: String::new(),
        };
        (active, passive)
    }

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

    #[test]
    fn lockstep_sim_headless_cli_parses_dual_ticket_environment_names() {
        let command = parse_headless_args([
            "--scenario".to_string(),
            "online-dual-client".to_string(),
            "--ticket-env".to_string(),
            "ACTIVE_TICKET".to_string(),
            "--observer-ticket-env".to_string(),
            "PASSIVE_TICKET".to_string(),
        ])
        .unwrap();
        let HeadlessCommand::Run(options) = command else {
            panic!("expected run command");
        };

        assert_eq!(options.scenario, HeadlessScenario::OnlineDualClient);
        assert_eq!(options.ticket_env.as_deref(), Some("ACTIVE_TICKET"));
        assert_eq!(
            options.observer_ticket_env.as_deref(),
            Some("PASSIVE_TICKET")
        );
    }

    #[test]
    fn lockstep_sim_headless_cli_parses_reconnect_observer_scenario() {
        let command = parse_headless_args([
            "--scenario".to_string(),
            "online-reconnect-observer".to_string(),
            "--ticket-env".to_string(),
            "PRIMARY_TICKET".to_string(),
            "--observer-ticket-env".to_string(),
            "OBSERVER_TICKET".to_string(),
        ])
        .unwrap();
        let HeadlessCommand::Run(options) = command else {
            panic!("expected run command");
        };

        assert_eq!(options.scenario, HeadlessScenario::OnlineReconnectObserver);
        assert_eq!(options.ticket_env.as_deref(), Some("PRIMARY_TICKET"));
        assert_eq!(
            options.observer_ticket_env.as_deref(),
            Some("OBSERVER_TICKET")
        );
    }

    #[test]
    fn online_dual_reconciliation_matches_common_authority_frames() {
        let (active, passive) = dual_test_reports();

        assert_eq!(
            reconcile_online_dual_reports(&active, &passive).unwrap(),
            vec![4, 5, 6]
        );
    }

    #[test]
    fn online_dual_reconciliation_reports_first_entity_mismatch_frame() {
        let (active, mut passive) = dual_test_reports();
        passive.frames[1]
            .world
            .entity_mut(FIXTURE_TARGET_ID)
            .unwrap()
            .combat
            .hp -= 1;

        let mismatch = reconcile_online_dual_reports(&active, &passive).unwrap_err();
        assert_eq!(mismatch.frame, 5);
        assert_eq!(mismatch.error_code, "HEADLESS_DUAL_ENTITY_MISMATCH");
    }
}
