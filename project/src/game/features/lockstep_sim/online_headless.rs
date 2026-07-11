use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    net::SocketAddr,
    thread,
    time::{Duration, Instant},
};

use bevy::{
    ecs::message::{MessageCursor, Messages},
    prelude::*,
};
use sim_core::{
    CastSkillCommand, EntityId, Fp, MoveCommand, QuantizedDir, SimCommand, SimEvent, SimHash,
    SimInput, SimWorld, SkillId, SkillTarget,
};

use crate::{
    framework::{
        network::{NetworkCommand, NetworkPlugin, NetworkTransport},
        scene::prelude::{SceneId, SceneSessionId},
    },
    game::{
        authority::{AuthorityCommand, AuthorityEndpoint, AuthorityPlugin, AuthoritySession},
        myserver::{
            MyServerAutoClientConfig, MyServerCommand, MyServerConfig, MyServerEvent,
            MyServerPlugin, MyServerSession, ReconnectCause,
        },
        scenes::LOCKSTEP_SIM_ARENA_SCENE_ID,
    },
};

use super::{
    config::{LockstepSimAuthorityMode, LockstepSimConfig},
    hud::{format_lockstep_sim_hud_status, lockstep_sim_hud_snapshot},
    payload::{build_sim_input_envelope, gate_lockstep_sim_input},
    replay::{LockstepSimReplayState, apply_lockstep_sim_authority_events},
    snapshot::LockstepSimSnapshotError,
    state::LockstepSimSceneState,
    sync::{
        LockstepSimMyServerJoinState, follow_lockstep_sim_myserver_events,
        lockstep_snapshot_error_code,
    },
};

const ONLINE_SKILL_ID: SkillId = SkillId::new(1);
const ONLINE_TRAINING_TARGET_ID: EntityId = EntityId::new(9000);
// Mirrors the local MyServer `lockstep_sim_demo` policy. This is not runtime negotiation;
// replace it with negotiated room config if the protocol starts exposing input lead frames.
const ONLINE_DEMO_POLICY_INPUT_LEAD_FRAMES: u32 = 2;
const ONLINE_OBSERVATION_FRAMES: u32 = 2;
const UPDATE_SLEEP: Duration = Duration::from_millis(2);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OnlineHeadlessOptions {
    pub endpoint: SocketAddr,
    pub ticket_env: String,
    pub room: String,
    pub policy: String,
    pub player: String,
    pub timeout: Duration,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OnlineDualHeadlessOptions {
    pub endpoint: SocketAddr,
    pub primary_ticket_env: String,
    pub passive_ticket_env: String,
    pub room: String,
    pub policy: String,
    pub primary_player: String,
    pub passive_player: String,
    pub timeout: Duration,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OnlineReconnectObserverOptions {
    pub endpoint: SocketAddr,
    pub primary_ticket_env: String,
    pub observer_ticket_env: String,
    pub room: String,
    pub policy: String,
    pub primary_player: String,
    pub observer_player: String,
    pub timeout: Duration,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OnlineHeadlessFrame {
    pub frame: u32,
    pub server_hash: Option<SimHash>,
    pub local_hash: SimHash,
    pub world: SimWorld,
    pub inputs: Vec<SimInput>,
    pub events: Vec<SimEvent>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OnlineHeadlessReport {
    pub player_id: String,
    pub player_entity_id: EntityId,
    pub input_frame: u32,
    pub stop_frame: u32,
    pub initial_world: SimWorld,
    pub initial_hash: SimHash,
    pub frames: Vec<OnlineHeadlessFrame>,
    pub hud_status: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OnlineDualHeadlessReport {
    pub active: OnlineHeadlessReport,
    pub passive: OnlineHeadlessReport,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OnlineRecoveryCheckpoint {
    pub frame: u32,
    pub hash: SimHash,
    pub world: SimWorld,
    pub snapshot_generation: u64,
    pub inputs: Vec<SimInput>,
    pub events: Vec<SimEvent>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OnlineRecoveryStreamReport {
    pub player_id: String,
    pub snapshot_frame: u32,
    pub snapshot_hash: SimHash,
    pub snapshot_world: SimWorld,
    pub recovery_generation: u64,
    pub response_current_frame: u32,
    pub response_waiting_frame: u32,
    pub response_recent_input_frames: Vec<u32>,
    pub response_waiting_input_frames: Vec<u32>,
    pub frames: Vec<OnlineHeadlessFrame>,
    pub ignored_duplicate_or_old_frames: u64,
    pub local_input_acknowledgements: usize,
    pub has_control_binding: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OnlineReconnectObserverReport {
    pub pre_disconnect: OnlineRecoveryCheckpoint,
    pub disconnect_generation: u64,
    pub primary: OnlineRecoveryStreamReport,
    pub observer: OnlineRecoveryStreamReport,
    pub first_input_frame: u32,
    pub post_reconnect_input_frame: u32,
    pub common_frames: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OnlineHeadlessError {
    pub error_code: &'static str,
    pub failure_stage: &'static str,
    pub message: String,
}

impl OnlineHeadlessError {
    fn new(
        error_code: &'static str,
        failure_stage: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            error_code,
            failure_stage,
            message: message.into(),
        }
    }
}

pub(crate) fn run_online_headless(
    options: &OnlineHeadlessOptions,
) -> Result<OnlineHeadlessReport, OnlineHeadlessError> {
    validate_options(options)?;
    let ticket = read_ticket(&options.ticket_env)?;

    let mut app = build_online_app(options);
    app.update();
    activate_online_scene(&mut app, options, ticket);

    let mut event_cursor = MessageCursor::<MyServerEvent>::default();
    let deadline = Instant::now() + options.timeout;
    let mut connected = false;
    let mut authenticated_player_id = None;
    let mut input_frame = None;
    let mut stop_frame = None;
    let mut accepted_inputs = 0_usize;

    loop {
        app.update();
        let events = read_myserver_events(&app, &mut event_cursor);
        for event in &events {
            match event {
                MyServerEvent::Connected { .. } => connected = true,
                MyServerEvent::Authenticated { player_id } => {
                    authenticated_player_id = Some(player_id.clone());
                }
                MyServerEvent::PlayerInputAccepted(response) if response.ok => {
                    accepted_inputs = accepted_inputs.saturating_add(1);
                }
                MyServerEvent::Disconnected { reason } if connected => {
                    return Err(OnlineHeadlessError::new(
                        "HEADLESS_SERVER_DISCONNECTED",
                        "frame_wait",
                        format!(
                            "MyServer disconnected before online telemetry completed: {}",
                            reason.as_deref().unwrap_or("no reason")
                        ),
                    ));
                }
                _ => {}
            }
            if let Some(error) = failure_from_event(event) {
                return Err(error);
            }
        }

        if let Some(error) = replay_error(&app) {
            return Err(error);
        }

        if input_frame.is_none() && online_room_started(&app) {
            input_frame = try_send_scripted_input(&mut app)?;
        }

        if let Some(first_input_frame) = input_frame
            && stop_frame.is_none()
            && accepted_inputs >= 1
            && app
                .world()
                .resource::<LockstepSimReplayState>()
                .last_applied_frame
                .is_some_and(|frame| frame >= first_input_frame)
        {
            stop_frame = Some(send_scripted_stop(&mut app, first_input_frame)?);
        }

        if let (Some(input_frame), Some(stop_frame)) = (input_frame, stop_frame) {
            let observation_frame = stop_frame.saturating_add(ONLINE_OBSERVATION_FRAMES);
            let last_frame = app
                .world()
                .resource::<LockstepSimReplayState>()
                .last_applied_frame
                .unwrap_or_default();
            if accepted_inputs >= 2 && last_frame >= observation_frame {
                let player_id = authenticated_player_id.ok_or_else(|| {
                    OnlineHeadlessError::new(
                        "HEADLESS_AUTH_PLAYER_MISSING",
                        "authentication",
                        "MyServer authenticated without a gameplay character id",
                    )
                })?;
                let report = collect_report(&app, player_id, input_frame, stop_frame)?;
                cleanup_online_session(&mut app, &mut event_cursor, options.timeout)?;
                return Ok(report);
            }
        }

        if Instant::now() >= deadline {
            let (code, stage, detail) = if !connected {
                (
                    "HEADLESS_CONNECT_TIMEOUT",
                    "connect",
                    "timed out waiting for the MyServer TCP connection",
                )
            } else if authenticated_player_id.is_none() {
                (
                    "HEADLESS_AUTH_TIMEOUT",
                    "authentication",
                    "timed out waiting for ticket authentication",
                )
            } else if !online_room_started(&app) {
                (
                    "HEADLESS_ROOM_START_TIMEOUT",
                    "room_start",
                    "timed out waiting for the lockstep room to start",
                )
            } else if input_frame.is_none() {
                (
                    "HEADLESS_INPUT_PREPARE_TIMEOUT",
                    "input_prepare",
                    "timed out waiting for the initial lockstep snapshot and control binding",
                )
            } else {
                (
                    "HEADLESS_FRAME_TIMEOUT",
                    "frame_wait",
                    "timed out waiting for authoritative frames and input acknowledgements",
                )
            };
            return Err(OnlineHeadlessError::new(code, stage, detail));
        }

        thread::sleep(UPDATE_SLEEP);
    }
}

fn read_ticket(environment_name: &str) -> Result<String, OnlineHeadlessError> {
    let ticket = env::var(environment_name).map_err(|_| {
        OnlineHeadlessError::new(
            "HEADLESS_TICKET_ENV_MISSING",
            "configuration",
            format!(
                "ticket environment variable {:?} is missing or not valid Unicode",
                environment_name
            ),
        )
    })?;
    if ticket.trim().is_empty() {
        return Err(OnlineHeadlessError::new(
            "HEADLESS_TICKET_ENV_EMPTY",
            "configuration",
            format!(
                "ticket environment variable {:?} is empty",
                environment_name
            ),
        ));
    }
    Ok(ticket)
}

#[derive(Default)]
struct OnlineDualProgress {
    connected: bool,
    authenticated_player_id: Option<String>,
    ready_confirmed: bool,
    accepted_inputs: usize,
}

pub(crate) fn run_online_dual_headless(
    options: &OnlineDualHeadlessOptions,
) -> Result<OnlineDualHeadlessReport, OnlineHeadlessError> {
    let active_options = OnlineHeadlessOptions {
        endpoint: options.endpoint,
        ticket_env: options.primary_ticket_env.clone(),
        room: options.room.clone(),
        policy: options.policy.clone(),
        player: options.primary_player.clone(),
        timeout: options.timeout,
    };
    let passive_options = OnlineHeadlessOptions {
        endpoint: options.endpoint,
        ticket_env: options.passive_ticket_env.clone(),
        room: options.room.clone(),
        policy: options.policy.clone(),
        player: options.passive_player.clone(),
        timeout: options.timeout,
    };
    validate_options(&active_options)?;
    validate_options(&passive_options)?;
    if options.primary_ticket_env == options.passive_ticket_env {
        return Err(OnlineHeadlessError::new(
            "HEADLESS_DUAL_TICKET_ENV_NOT_DISTINCT",
            "configuration",
            "online-dual-client requires two distinct ticket environment variables",
        ));
    }

    let active_ticket = read_ticket(&options.primary_ticket_env)?;
    let mut passive_ticket = Some(read_ticket(&options.passive_ticket_env)?);
    if active_ticket == passive_ticket.as_deref().unwrap_or_default() {
        return Err(OnlineHeadlessError::new(
            "HEADLESS_DUAL_TICKET_NOT_DISTINCT",
            "configuration",
            "online-dual-client requires two distinct ticket values",
        ));
    }

    let mut active_app = build_online_app(&active_options);
    let mut passive_app = build_online_app(&passive_options);
    active_app.update();
    passive_app.update();
    activate_online_scene(&mut active_app, &active_options, active_ticket);
    defer_automatic_room_start(&mut active_app);

    let mut active_cursor = MessageCursor::<MyServerEvent>::default();
    let mut passive_cursor = MessageCursor::<MyServerEvent>::default();
    let mut active = OnlineDualProgress::default();
    let mut passive = OnlineDualProgress::default();
    let mut passive_activated = false;
    let mut room_start_sent = false;
    let mut input_frame = None;
    let mut stop_frame = None;
    let deadline = Instant::now() + options.timeout;

    loop {
        active_app.update();
        let active_events = read_myserver_events(&active_app, &mut active_cursor);
        observe_dual_client_events(&active_events, &mut active, true)?;
        if let Some(error) = replay_error(&active_app) {
            return Err(error);
        }

        if active.ready_confirmed && !passive_activated {
            activate_online_scene(
                &mut passive_app,
                &passive_options,
                passive_ticket
                    .take()
                    .expect("passive ticket is activated once"),
            );
            defer_automatic_room_start(&mut passive_app);
            passive_activated = true;
        }

        if passive_activated {
            passive_app.update();
            let passive_events = read_myserver_events(&passive_app, &mut passive_cursor);
            observe_dual_client_events(&passive_events, &mut passive, false)?;
            if let Some(error) = replay_error(&passive_app) {
                return Err(error);
            }
        }

        if active.ready_confirmed && passive.ready_confirmed && !room_start_sent {
            let active_player = active.authenticated_player_id.as_deref().ok_or_else(|| {
                OnlineHeadlessError::new(
                    "HEADLESS_AUTH_PLAYER_MISSING",
                    "authentication",
                    "active client authenticated without a gameplay character id",
                )
            })?;
            let passive_player = passive.authenticated_player_id.as_deref().ok_or_else(|| {
                OnlineHeadlessError::new(
                    "HEADLESS_AUTH_PLAYER_MISSING",
                    "authentication",
                    "passive client authenticated without a gameplay character id",
                )
            })?;
            if active_player == passive_player {
                return Err(OnlineHeadlessError::new(
                    "HEADLESS_DUAL_PLAYER_NOT_DISTINCT",
                    "authentication",
                    "online-dual-client tickets resolved to the same gameplay character",
                ));
            }
            active_app
                .world_mut()
                .resource_mut::<LockstepSimMyServerJoinState>()
                .start_sent = true;
            active_app
                .world_mut()
                .write_message(MyServerCommand::StartRoom);
            room_start_sent = true;
        }

        let both_started = room_start_sent
            && online_room_started(&active_app)
            && online_room_started(&passive_app);
        let both_snapshots_ready = both_started
            && initial_snapshot_ready(&active_app)
            && initial_snapshot_ready(&passive_app);
        if input_frame.is_none() && both_snapshots_ready {
            input_frame = try_send_scripted_input(&mut active_app)?;
        }

        if let Some(first_input_frame) = input_frame
            && stop_frame.is_none()
            && active.accepted_inputs >= 1
            && last_applied_frame(&active_app).is_some_and(|frame| frame >= first_input_frame)
        {
            stop_frame = Some(send_scripted_stop(&mut active_app, first_input_frame)?);
        }

        if let (Some(input_frame), Some(stop_frame)) = (input_frame, stop_frame) {
            let observation_frame = stop_frame.saturating_add(ONLINE_OBSERVATION_FRAMES);
            if active.accepted_inputs >= 2
                && last_applied_frame(&active_app).is_some_and(|frame| frame >= observation_frame)
                && last_applied_frame(&passive_app).is_some_and(|frame| frame >= observation_frame)
            {
                let active_player = active.authenticated_player_id.take().ok_or_else(|| {
                    OnlineHeadlessError::new(
                        "HEADLESS_AUTH_PLAYER_MISSING",
                        "authentication",
                        "active client authenticated without a gameplay character id",
                    )
                })?;
                let passive_player = passive.authenticated_player_id.take().ok_or_else(|| {
                    OnlineHeadlessError::new(
                        "HEADLESS_AUTH_PLAYER_MISSING",
                        "authentication",
                        "passive client authenticated without a gameplay character id",
                    )
                })?;
                let active_report =
                    collect_report(&active_app, active_player, input_frame, stop_frame)?;
                let passive_report =
                    collect_report(&passive_app, passive_player, input_frame, stop_frame)?;
                cleanup_dual_online_session(
                    &mut active_app,
                    &mut active_cursor,
                    &mut passive_app,
                    &mut passive_cursor,
                    options.timeout,
                )?;
                return Ok(OnlineDualHeadlessReport {
                    active: active_report,
                    passive: passive_report,
                });
            }
        }

        if Instant::now() >= deadline {
            let (code, stage, detail) = if !active.connected {
                (
                    "HEADLESS_DUAL_ACTIVE_CONNECT_TIMEOUT",
                    "connect",
                    "timed out waiting for the active MyServer TCP connection",
                )
            } else if !active.ready_confirmed {
                (
                    "HEADLESS_DUAL_ACTIVE_READY_TIMEOUT",
                    "room_ready",
                    "timed out waiting for the active client to join and become ready",
                )
            } else if !passive.connected {
                (
                    "HEADLESS_DUAL_PASSIVE_CONNECT_TIMEOUT",
                    "connect",
                    "timed out waiting for the passive MyServer TCP connection",
                )
            } else if !passive.ready_confirmed {
                (
                    "HEADLESS_DUAL_PASSIVE_READY_TIMEOUT",
                    "room_ready",
                    "timed out waiting for the passive client to join and become ready",
                )
            } else if !both_started {
                (
                    "HEADLESS_DUAL_ROOM_START_TIMEOUT",
                    "room_start",
                    "timed out waiting for both clients to observe the started room",
                )
            } else if input_frame.is_none() {
                (
                    "HEADLESS_DUAL_INPUT_PREPARE_TIMEOUT",
                    "input_prepare",
                    "timed out waiting for both initial snapshots",
                )
            } else {
                (
                    "HEADLESS_DUAL_FRAME_TIMEOUT",
                    "frame_wait",
                    "timed out waiting for both clients to replay the observation frame",
                )
            };
            return Err(OnlineHeadlessError::new(code, stage, detail));
        }

        thread::sleep(UPDATE_SLEEP);
    }
}

#[derive(Default)]
struct OnlineRecoveryProgress {
    connection_count: u32,
    authenticated_player_id: Option<String>,
    ready_confirmed: bool,
    reauthenticated: bool,
    local_input_acknowledgements: usize,
}

#[derive(Clone, Debug)]
struct OnlineRecoveryResponse {
    snapshot_frame: u32,
    current_frame: u32,
    waiting_frame: u32,
    recent_input_frames: Vec<u32>,
    waiting_input_frames: Vec<u32>,
}

pub(crate) fn run_online_reconnect_observer_headless(
    options: &OnlineReconnectObserverOptions,
) -> Result<OnlineReconnectObserverReport, OnlineHeadlessError> {
    let primary_options = OnlineHeadlessOptions {
        endpoint: options.endpoint,
        ticket_env: options.primary_ticket_env.clone(),
        room: options.room.clone(),
        policy: options.policy.clone(),
        player: options.primary_player.clone(),
        timeout: options.timeout,
    };
    let observer_options = OnlineHeadlessOptions {
        endpoint: options.endpoint,
        ticket_env: options.observer_ticket_env.clone(),
        room: options.room.clone(),
        policy: options.policy.clone(),
        player: options.observer_player.clone(),
        timeout: options.timeout,
    };
    validate_options(&primary_options)?;
    validate_options(&observer_options)?;
    if options.primary_ticket_env == options.observer_ticket_env {
        return Err(OnlineHeadlessError::new(
            "HEADLESS_RECOVERY_TICKET_ENV_NOT_DISTINCT",
            "configuration",
            "reconnect-observer recovery requires distinct ticket environment variables",
        ));
    }

    let primary_ticket = read_ticket(&options.primary_ticket_env)?;
    let observer_ticket = read_ticket(&options.observer_ticket_env)?;
    if primary_ticket == observer_ticket {
        return Err(OnlineHeadlessError::new(
            "HEADLESS_RECOVERY_TICKET_NOT_DISTINCT",
            "configuration",
            "reconnect-observer recovery requires distinct ticket values",
        ));
    }

    let mut primary_app = build_online_app(&primary_options);
    let mut observer_app = build_online_app(&observer_options);
    primary_app.update();
    observer_app.update();
    observer_app
        .world_mut()
        .resource_mut::<LockstepSimMyServerJoinState>()
        .configure_observer();
    activate_online_scene(&mut primary_app, &primary_options, primary_ticket.clone());

    let mut primary_cursor = MessageCursor::<MyServerEvent>::default();
    let mut observer_cursor = MessageCursor::<MyServerEvent>::default();
    let mut primary = OnlineRecoveryProgress::default();
    let mut observer = OnlineRecoveryProgress::default();
    let mut first_input_frame = None;
    let mut pre_disconnect = None;
    let mut disconnect_requested = false;
    let mut disconnect_observed = false;
    let mut disconnect_generation = None;
    let mut reconnect_sent = false;
    let mut primary_recovery = None;
    let mut observer_activated = false;
    let mut observer_recovery = None;
    let mut post_reconnect_input_frame = None;
    let deadline = Instant::now() + options.timeout;

    loop {
        primary_app.update();
        let primary_events = read_myserver_events(&primary_app, &mut primary_cursor);
        for event in &primary_events {
            match event {
                MyServerEvent::Connected { .. } => {
                    primary.connection_count = primary.connection_count.saturating_add(1);
                }
                MyServerEvent::Authenticated { player_id } => {
                    primary.authenticated_player_id = Some(player_id.clone());
                }
                MyServerEvent::ReadyChanged(response) if response.ok && response.ready => {
                    primary.ready_confirmed = true;
                }
                MyServerEvent::PlayerInputAccepted(response) if response.ok => {
                    primary.local_input_acknowledgements =
                        primary.local_input_acknowledgements.saturating_add(1);
                }
                MyServerEvent::Disconnected { .. } if disconnect_requested => {
                    disconnect_observed = true;
                }
                MyServerEvent::Disconnected { reason } => {
                    return Err(OnlineHeadlessError::new(
                        "HEADLESS_RECOVERY_UNEXPECTED_DISCONNECT",
                        "pre_reconnect",
                        format!(
                            "primary disconnected before the controlled recovery point: {}",
                            reason.as_deref().unwrap_or("no reason")
                        ),
                    ));
                }
                MyServerEvent::ReauthenticatedForReconnect { player_id, cause } => {
                    if !matches!(cause, ReconnectCause::TransportRecovery) {
                        return Err(OnlineHeadlessError::new(
                            "HEADLESS_RECOVERY_CAUSE_MISMATCH",
                            "reconnect_authentication",
                            "primary reconnect did not use the transport recovery cause",
                        ));
                    }
                    if primary.authenticated_player_id.as_deref() != Some(player_id.as_str()) {
                        return Err(OnlineHeadlessError::new(
                            "HEADLESS_RECOVERY_PLAYER_CHANGED",
                            "reconnect_authentication",
                            "primary gameplay character changed during reconnect",
                        ));
                    }
                    primary.reauthenticated = true;
                }
                MyServerEvent::RoomReconnected(response) => {
                    if !response.ok {
                        return Err(OnlineHeadlessError::new(
                            "HEADLESS_ROOM_RECONNECT_REJECTED",
                            "room_reconnect",
                            format!("room reconnect rejected: {}", response.error_code),
                        ));
                    }
                    let snapshot = response.snapshot.as_ref().ok_or_else(|| {
                        OnlineHeadlessError::new(
                            "HEADLESS_RECOVERY_SNAPSHOT_MISSING",
                            "room_reconnect",
                            "RoomReconnectRes did not include a recovery snapshot",
                        )
                    })?;
                    primary_recovery = Some(OnlineRecoveryResponse {
                        snapshot_frame: snapshot.current_frame_id,
                        current_frame: response.current_frame_id.max(snapshot.current_frame_id),
                        waiting_frame: response.waiting_frame_id,
                        recent_input_frames: response
                            .recent_inputs
                            .iter()
                            .map(|input| input.frame_id)
                            .collect(),
                        waiting_input_frames: response
                            .waiting_inputs
                            .iter()
                            .map(|input| input.frame_id)
                            .collect(),
                    });
                }
                _ => {}
            }
            if let Some(error) = failure_from_event(event) {
                return Err(recovery_stage_error(error, disconnect_requested));
            }
        }

        if let Some(error) = replay_error(&primary_app) {
            return Err(error);
        }

        if first_input_frame.is_none() && online_room_started(&primary_app) {
            first_input_frame = try_send_scripted_input(&mut primary_app)?;
        }

        if let Some(input_frame) = first_input_frame
            && pre_disconnect.is_none()
            && primary.local_input_acknowledgements >= 1
            && last_applied_frame(&primary_app)
                .is_some_and(|frame| frame >= input_frame.saturating_add(ONLINE_OBSERVATION_FRAMES))
        {
            pre_disconnect = Some(capture_recovery_checkpoint(&primary_app)?);
        }

        if pre_disconnect.is_some() && !disconnect_requested {
            let connection_id = primary_app
                .world()
                .resource::<MyServerSession>()
                .connection_id
                .ok_or_else(|| {
                    OnlineHeadlessError::new(
                        "HEADLESS_RECOVERY_CONNECTION_MISSING",
                        "disconnect",
                        "primary connection disappeared before controlled disconnect",
                    )
                })?;
            primary_app
                .world_mut()
                .write_message(NetworkCommand::Disconnect { connection_id });
            disconnect_requested = true;
        }

        if disconnect_observed && disconnect_generation.is_none() {
            let scene = primary_app.world().resource::<LockstepSimSceneState>();
            let checkpoint_generation = pre_disconnect
                .as_ref()
                .map(|checkpoint| checkpoint.snapshot_generation)
                .unwrap_or_default();
            if scene.initial_snapshot.is_none()
                && scene.snapshot_generation > checkpoint_generation
                && myserver_connection_closed(&primary_app)
            {
                disconnect_generation = Some(scene.snapshot_generation);
            }
        }

        if disconnect_generation.is_some() && !reconnect_sent {
            if !myserver_connection_closed(&primary_app) {
                return Err(OnlineHeadlessError::new(
                    "HEADLESS_RECOVERY_CONNECTION_STILL_OPEN",
                    "reconnect_prepare",
                    "primary old connection remained open after disconnect confirmation",
                ));
            }
            primary_app
                .world_mut()
                .write_message(MyServerCommand::ReconnectWithTicket {
                    ticket: primary_ticket.clone(),
                    transport: NetworkTransport::Tcp,
                    host: Some(options.endpoint.ip().to_string()),
                    port: Some(options.endpoint.port()),
                });
            reconnect_sent = true;
        }

        let primary_replay_ready = primary_recovery.as_ref().is_some_and(|recovery| {
            let scene = primary_app.world().resource::<LockstepSimSceneState>();
            let replay = primary_app.world().resource::<LockstepSimReplayState>();
            primary.reauthenticated
                && scene
                    .initial_snapshot
                    .as_ref()
                    .is_some_and(|snapshot| snapshot.start_frame == recovery.snapshot_frame)
                && replay.snapshot_generation == scene.snapshot_generation
                && replay
                    .last_applied_frame
                    .is_some_and(|frame| frame >= recovery.current_frame)
                && replay.last_error.is_none()
        });

        if primary_replay_ready && !observer_activated {
            activate_online_scene(
                &mut observer_app,
                &observer_options,
                observer_ticket.clone(),
            );
            observer_activated = true;
        }

        if observer_activated {
            observer_app.update();
            let observer_events = read_myserver_events(&observer_app, &mut observer_cursor);
            for event in &observer_events {
                match event {
                    MyServerEvent::Connected { .. } => {
                        observer.connection_count = observer.connection_count.saturating_add(1);
                    }
                    MyServerEvent::Authenticated { player_id } => {
                        observer.authenticated_player_id = Some(player_id.clone());
                    }
                    MyServerEvent::PlayerInputAccepted(response) if response.ok => {
                        observer.local_input_acknowledgements =
                            observer.local_input_acknowledgements.saturating_add(1);
                    }
                    MyServerEvent::RoomJoinedAsObserver(response) => {
                        if !response.ok {
                            return Err(OnlineHeadlessError::new(
                                "HEADLESS_OBSERVER_JOIN_REJECTED",
                                "observer_recovery",
                                format!("observer join rejected: {}", response.error_code),
                            ));
                        }
                        let snapshot = response.snapshot.as_ref().ok_or_else(|| {
                            OnlineHeadlessError::new(
                                "HEADLESS_OBSERVER_SNAPSHOT_MISSING",
                                "observer_recovery",
                                "RoomJoinAsObserverRes did not include a recovery snapshot",
                            )
                        })?;
                        observer_recovery = Some(OnlineRecoveryResponse {
                            snapshot_frame: snapshot.current_frame_id,
                            current_frame: response.current_frame_id.max(snapshot.current_frame_id),
                            waiting_frame: response.waiting_frame_id,
                            recent_input_frames: response
                                .recent_inputs
                                .iter()
                                .map(|input| input.frame_id)
                                .collect(),
                            waiting_input_frames: response
                                .waiting_inputs
                                .iter()
                                .map(|input| input.frame_id)
                                .collect(),
                        });
                    }
                    MyServerEvent::RoomJoined(_) => {
                        return Err(OnlineHeadlessError::new(
                            "HEADLESS_OBSERVER_JOIN_PATH_MISMATCH",
                            "observer_recovery",
                            "observer used the normal player room join path",
                        ));
                    }
                    MyServerEvent::Disconnected { reason } => {
                        return Err(OnlineHeadlessError::new(
                            "HEADLESS_OBSERVER_DISCONNECTED",
                            "observer_recovery",
                            format!(
                                "observer disconnected before recovery completed: {}",
                                reason.as_deref().unwrap_or("no reason")
                            ),
                        ));
                    }
                    _ => {}
                }
                if let Some(error) = failure_from_event(event) {
                    return Err(OnlineHeadlessError::new(
                        error.error_code,
                        "observer_recovery",
                        error.message,
                    ));
                }
            }
            if let Some(error) = replay_error(&observer_app) {
                return Err(error);
            }
        }

        let observer_replay_ready = observer_recovery.as_ref().is_some_and(|recovery| {
            let scene = observer_app.world().resource::<LockstepSimSceneState>();
            let replay = observer_app.world().resource::<LockstepSimReplayState>();
            scene
                .initial_snapshot
                .as_ref()
                .is_some_and(|snapshot| snapshot.start_frame == recovery.snapshot_frame)
                && replay.snapshot_generation == scene.snapshot_generation
                && replay
                    .last_applied_frame
                    .is_some_and(|frame| frame >= recovery.current_frame)
                && replay.last_error.is_none()
        });

        if observer_replay_ready && post_reconnect_input_frame.is_none() {
            post_reconnect_input_frame = Some(send_scripted_stop(
                &mut primary_app,
                first_input_frame.expect("pre-disconnect input exists before observer recovery"),
            )?);
        }

        if let Some(stop_frame) = post_reconnect_input_frame {
            let target_frame = stop_frame.saturating_add(ONLINE_OBSERVATION_FRAMES);
            if primary.local_input_acknowledgements >= 2
                && last_applied_frame(&primary_app).is_some_and(|frame| frame >= target_frame)
                && last_applied_frame(&observer_app).is_some_and(|frame| frame >= target_frame)
            {
                let primary_player = primary.authenticated_player_id.take().ok_or_else(|| {
                    OnlineHeadlessError::new(
                        "HEADLESS_AUTH_PLAYER_MISSING",
                        "report",
                        "primary authenticated without a gameplay character id",
                    )
                })?;
                let observer_player = observer.authenticated_player_id.take().ok_or_else(|| {
                    OnlineHeadlessError::new(
                        "HEADLESS_AUTH_PLAYER_MISSING",
                        "report",
                        "observer authenticated without a gameplay character id",
                    )
                })?;
                if primary_player == observer_player {
                    return Err(OnlineHeadlessError::new(
                        "HEADLESS_RECOVERY_PLAYER_NOT_DISTINCT",
                        "report",
                        "primary and observer tickets resolved to the same gameplay character",
                    ));
                }

                let primary_response = primary_recovery.as_ref().expect("primary recovery ready");
                let observer_response =
                    observer_recovery.as_ref().expect("observer recovery ready");
                let primary_report = collect_recovery_stream(
                    &primary_app,
                    primary_player,
                    primary.local_input_acknowledgements,
                    primary_response,
                )?;
                let observer_report = collect_recovery_stream(
                    &observer_app,
                    observer_player,
                    observer.local_input_acknowledgements,
                    observer_response,
                )?;
                let common_frames = reconcile_recovery_streams(
                    &primary_report,
                    &observer_report,
                    stop_frame,
                    target_frame,
                )?;

                cleanup_dual_online_session(
                    &mut primary_app,
                    &mut primary_cursor,
                    &mut observer_app,
                    &mut observer_cursor,
                    options.timeout,
                )?;
                return Ok(OnlineReconnectObserverReport {
                    pre_disconnect: pre_disconnect.expect("controlled disconnect captured"),
                    disconnect_generation: disconnect_generation
                        .expect("disconnect generation captured"),
                    primary: primary_report,
                    observer: observer_report,
                    first_input_frame: first_input_frame.expect("first input sent"),
                    post_reconnect_input_frame: stop_frame,
                    common_frames,
                });
            }
        }

        if Instant::now() >= deadline {
            let (code, stage, detail) = if primary.connection_count == 0 {
                (
                    "HEADLESS_RECOVERY_CONNECT_TIMEOUT",
                    "connect",
                    "timed out waiting for the primary connection",
                )
            } else if !primary.ready_confirmed {
                (
                    "HEADLESS_RECOVERY_ROOM_READY_TIMEOUT",
                    "room_ready",
                    "timed out waiting for the primary room to become ready",
                )
            } else if pre_disconnect.is_none() {
                (
                    "HEADLESS_RECOVERY_PRE_DISCONNECT_TIMEOUT",
                    "pre_reconnect",
                    "timed out waiting for the pre-disconnect authority checkpoint",
                )
            } else if !disconnect_observed || disconnect_generation.is_none() {
                (
                    "HEADLESS_RECOVERY_DISCONNECT_TIMEOUT",
                    "disconnect",
                    "timed out waiting for the old primary connection to close",
                )
            } else if primary_recovery.is_none() || !primary_replay_ready {
                (
                    "HEADLESS_RECOVERY_RECONNECT_TIMEOUT",
                    "room_reconnect",
                    "timed out waiting for RoomReconnectRes snapshot replay",
                )
            } else if observer.connection_count == 0 {
                (
                    "HEADLESS_OBSERVER_CONNECT_TIMEOUT",
                    "observer_connect",
                    "timed out waiting for the observer connection",
                )
            } else if observer_recovery.is_none() || !observer_replay_ready {
                (
                    "HEADLESS_OBSERVER_RECOVERY_TIMEOUT",
                    "observer_recovery",
                    "timed out waiting for RoomJoinAsObserverRes snapshot replay",
                )
            } else {
                (
                    "HEADLESS_RECOVERY_FRAME_TIMEOUT",
                    "post_reconnect_frames",
                    "timed out waiting for primary and observer post-recovery frames",
                )
            };
            return Err(OnlineHeadlessError::new(code, stage, detail));
        }

        thread::sleep(UPDATE_SLEEP);
    }
}

fn recovery_stage_error(
    error: OnlineHeadlessError,
    reconnect_started: bool,
) -> OnlineHeadlessError {
    if reconnect_started {
        OnlineHeadlessError::new(error.error_code, "room_reconnect", error.message)
    } else {
        error
    }
}

fn capture_recovery_checkpoint(app: &App) -> Result<OnlineRecoveryCheckpoint, OnlineHeadlessError> {
    let scene = app.world().resource::<LockstepSimSceneState>();
    let replay = app.world().resource::<LockstepSimReplayState>();
    let hash = replay.hash_history.back().ok_or_else(|| {
        OnlineHeadlessError::new(
            "HEADLESS_RECOVERY_CHECKPOINT_MISSING",
            "pre_reconnect",
            "pre-disconnect replay has no authoritative hash checkpoint",
        )
    })?;
    let (world, _) = replay
        .replay_from_cached_snapshot_to_frame(hash.frame)
        .map_err(|error| {
            OnlineHeadlessError::new(
                "HEADLESS_RECOVERY_CHECKPOINT_REPLAY_FAILED",
                "pre_reconnect",
                error.to_string(),
            )
        })?;
    Ok(OnlineRecoveryCheckpoint {
        frame: hash.frame,
        hash: hash.local_hash,
        world,
        snapshot_generation: scene.snapshot_generation,
        inputs: replay
            .input_history
            .iter()
            .flat_map(|entry| entry.sim_inputs.iter().cloned())
            .collect(),
        events: replay
            .event_history
            .iter()
            .flat_map(|entry| entry.events.iter().cloned())
            .collect(),
    })
}

fn collect_recovery_stream(
    app: &App,
    player_id: String,
    local_input_acknowledgements: usize,
    response: &OnlineRecoveryResponse,
) -> Result<OnlineRecoveryStreamReport, OnlineHeadlessError> {
    let scene = app.world().resource::<LockstepSimSceneState>();
    let snapshot = scene.initial_snapshot.as_ref().ok_or_else(|| {
        OnlineHeadlessError::new(
            "HEADLESS_RECOVERY_SNAPSHOT_MISSING",
            "report",
            "recovery report has no parsed snapshot",
        )
    })?;
    if snapshot.start_frame != response.snapshot_frame {
        return Err(OnlineHeadlessError::new(
            "HEADLESS_RECOVERY_SNAPSHOT_FRAME_MISMATCH",
            "report",
            format!(
                "parsed snapshot frame {} differs from response snapshot frame {}",
                snapshot.start_frame, response.snapshot_frame
            ),
        ));
    }

    let replay = app.world().resource::<LockstepSimReplayState>();
    let frames = collect_replay_frames(replay)?;
    validate_recovery_frame_continuity(snapshot.start_frame, &frames)?;
    Ok(OnlineRecoveryStreamReport {
        has_control_binding: snapshot.control_bindings.contains_key(&player_id),
        player_id,
        snapshot_frame: snapshot.start_frame,
        snapshot_hash: snapshot.initial_hash(),
        snapshot_world: snapshot.world.clone(),
        recovery_generation: scene.snapshot_generation,
        response_current_frame: response.current_frame,
        response_waiting_frame: response.waiting_frame,
        response_recent_input_frames: response.recent_input_frames.clone(),
        response_waiting_input_frames: response.waiting_input_frames.clone(),
        frames,
        ignored_duplicate_or_old_frames: replay.ignored_duplicate_or_old_frames,
        local_input_acknowledgements,
    })
}

fn collect_replay_frames(
    replay: &LockstepSimReplayState,
) -> Result<Vec<OnlineHeadlessFrame>, OnlineHeadlessError> {
    let mut frames = Vec::with_capacity(replay.hash_history.len());
    for hash in &replay.hash_history {
        let (world, _) = replay
            .replay_from_cached_snapshot_to_frame(hash.frame)
            .map_err(|error| {
                OnlineHeadlessError::new(
                    "HEADLESS_REPORT_REPLAY_FAILED",
                    "report",
                    error.to_string(),
                )
            })?;
        let inputs = replay
            .input_history
            .iter()
            .find(|input| input.frame == hash.frame)
            .map(|input| input.sim_inputs.clone())
            .unwrap_or_default();
        let events = replay
            .event_history
            .iter()
            .find(|events| events.frame == hash.frame)
            .map(|events| events.events.clone())
            .unwrap_or_default();
        frames.push(OnlineHeadlessFrame {
            frame: hash.frame,
            server_hash: hash.server_hash.as_ref().map(|server| SimHash {
                frame: sim_core::FrameId::new(server.frame),
                value: server.value,
            }),
            local_hash: hash.local_hash,
            world,
            inputs,
            events,
        });
    }
    Ok(frames)
}

fn validate_recovery_frame_continuity(
    snapshot_frame: u32,
    frames: &[OnlineHeadlessFrame],
) -> Result<(), OnlineHeadlessError> {
    let mut expected = snapshot_frame.saturating_add(1);
    let mut seen = BTreeSet::new();
    for frame in frames {
        if !seen.insert(frame.frame) {
            return Err(OnlineHeadlessError::new(
                "HEADLESS_RECOVERY_DUPLICATE_APPLIED_FRAME",
                "frame_continuity",
                format!("authority frame {} was applied more than once", frame.frame),
            ));
        }
        if frame.frame != expected {
            return Err(OnlineHeadlessError::new(
                "HEADLESS_RECOVERY_FRAME_GAP",
                "frame_continuity",
                format!(
                    "post-snapshot replay expected frame {expected}, got {}",
                    frame.frame
                ),
            ));
        }
        let server_hash = frame.server_hash.ok_or_else(|| {
            OnlineHeadlessError::new(
                "HEADLESS_RECOVERY_SERVER_HASH_MISSING",
                "frame_compare",
                format!("recovery frame {} has no server hash", frame.frame),
            )
        })?;
        if server_hash != frame.local_hash {
            return Err(OnlineHeadlessError::new(
                "HEADLESS_RECOVERY_HASH_MISMATCH",
                "frame_compare",
                format!("recovery frame {} hash did not realign", frame.frame),
            ));
        }
        expected = expected.saturating_add(1);
    }
    Ok(())
}

fn reconcile_recovery_streams(
    primary: &OnlineRecoveryStreamReport,
    observer: &OnlineRecoveryStreamReport,
    input_frame: u32,
    target_frame: u32,
) -> Result<Vec<u32>, OnlineHeadlessError> {
    if observer.local_input_acknowledgements != 0 || observer.has_control_binding {
        return Err(OnlineHeadlessError::new(
            "HEADLESS_OBSERVER_INPUT_ROLE_VIOLATION",
            "observer_input_role",
            "observer received a local input acknowledgement or a simulation control binding",
        ));
    }

    let observer_by_frame = observer
        .frames
        .iter()
        .map(|frame| (frame.frame, frame))
        .collect::<BTreeMap<_, _>>();
    let mut common = Vec::new();
    let mut post_reconnect_input_count = 0_usize;
    for primary_frame in &primary.frames {
        let Some(observer_frame) = observer_by_frame.get(&primary_frame.frame) else {
            continue;
        };
        if primary_frame.server_hash != observer_frame.server_hash
            || primary_frame.local_hash != observer_frame.local_hash
            || primary_frame.world != observer_frame.world
            || primary_frame.inputs != observer_frame.inputs
            || primary_frame.events != observer_frame.events
        {
            return Err(OnlineHeadlessError::new(
                "HEADLESS_OBSERVER_RECONCILIATION_MISMATCH",
                "observer_frame_compare",
                format!(
                    "primary and observer differ at authority frame {}",
                    primary_frame.frame
                ),
            ));
        }
        for input in &primary_frame.inputs {
            if input.character_id != primary.player_id {
                return Err(OnlineHeadlessError::new(
                    "HEADLESS_OBSERVER_INPUT_SOURCE_MISMATCH",
                    "observer_input_role",
                    format!(
                        "authority input at frame {} did not originate from the primary player",
                        primary_frame.frame
                    ),
                ));
            }
            if primary_frame.frame == input_frame && input.seq == 2 {
                post_reconnect_input_count = post_reconnect_input_count.saturating_add(1);
            }
        }
        common.push(primary_frame.frame);
    }

    let required_start = primary
        .snapshot_frame
        .max(observer.snapshot_frame)
        .saturating_add(1);
    if common.first().copied() != Some(required_start)
        || common
            .last()
            .copied()
            .is_none_or(|frame| frame < target_frame)
        || common
            .windows(2)
            .any(|frames| frames[1] != frames[0].saturating_add(1))
    {
        return Err(OnlineHeadlessError::new(
            "HEADLESS_OBSERVER_COMMON_FRAME_RANGE_INCOMPLETE",
            "observer_frame_compare",
            format!(
                "common recovery frames do not continuously cover {required_start} through {target_frame}"
            ),
        ));
    }
    if post_reconnect_input_count != 1 {
        return Err(OnlineHeadlessError::new(
            "HEADLESS_RECOVERY_INPUT_APPLICATION_COUNT_MISMATCH",
            "frame_continuity",
            format!(
                "post-reconnect input sequence 2 was applied {post_reconnect_input_count} times"
            ),
        ));
    }
    Ok(common)
}

fn defer_automatic_room_start(app: &mut App) {
    app.world_mut()
        .resource_mut::<LockstepSimMyServerJoinState>()
        .defer_start_room = true;
}

fn initial_snapshot_ready(app: &App) -> bool {
    app.world()
        .resource::<LockstepSimSceneState>()
        .initial_snapshot
        .is_some()
}

fn last_applied_frame(app: &App) -> Option<u32> {
    app.world()
        .resource::<LockstepSimReplayState>()
        .last_applied_frame
}

fn observe_dual_client_events(
    events: &[MyServerEvent],
    progress: &mut OnlineDualProgress,
    active_input_client: bool,
) -> Result<(), OnlineHeadlessError> {
    for event in events {
        match event {
            MyServerEvent::Connected { .. } => progress.connected = true,
            MyServerEvent::Authenticated { player_id } => {
                progress.authenticated_player_id = Some(player_id.clone());
            }
            MyServerEvent::ReadyChanged(response) if response.ok && response.ready => {
                progress.ready_confirmed = true;
            }
            MyServerEvent::PlayerInputAccepted(response) if active_input_client && response.ok => {
                progress.accepted_inputs = progress.accepted_inputs.saturating_add(1);
            }
            MyServerEvent::PlayerInputAccepted(_) if !active_input_client => {
                return Err(OnlineHeadlessError::new(
                    "HEADLESS_DUAL_PASSIVE_INPUT_SENT",
                    "input_role",
                    "passive replay client received an input acknowledgement",
                ));
            }
            MyServerEvent::Disconnected { reason } if progress.connected => {
                return Err(OnlineHeadlessError::new(
                    "HEADLESS_SERVER_DISCONNECTED",
                    "frame_wait",
                    format!(
                        "MyServer disconnected before dual telemetry completed: {}",
                        reason.as_deref().unwrap_or("no reason")
                    ),
                ));
            }
            _ => {}
        }
        if let Some(error) = failure_from_event(event) {
            return Err(error);
        }
    }
    Ok(())
}

fn validate_options(options: &OnlineHeadlessOptions) -> Result<(), OnlineHeadlessError> {
    if !options.endpoint.ip().is_loopback() {
        return Err(OnlineHeadlessError::new(
            "HEADLESS_ENDPOINT_NOT_LOOPBACK",
            "configuration",
            "online-single-client only accepts an explicit loopback endpoint",
        ));
    }
    if options.timeout.is_zero() {
        return Err(OnlineHeadlessError::new(
            "HEADLESS_TIMEOUT_INVALID",
            "configuration",
            "online timeout must be greater than zero",
        ));
    }
    if !is_environment_variable_name(&options.ticket_env) {
        return Err(OnlineHeadlessError::new(
            "HEADLESS_TICKET_ENV_INVALID",
            "configuration",
            "ticket environment variable name is invalid",
        ));
    }
    Ok(())
}

fn is_environment_variable_name(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|first| first == '_' || first.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn build_online_app(options: &OnlineHeadlessOptions) -> App {
    let mut server_config = MyServerConfig::default();
    server_config.game_host = options.endpoint.ip().to_string();
    server_config.tcp_fallback_port = options.endpoint.port();
    server_config.prefer_transport = NetworkTransport::Tcp;
    server_config.forced_transport = Some(NetworkTransport::Tcp);
    server_config.request_timeout = options.timeout;
    server_config.auto_reconnect_with_fresh_ticket = false;
    server_config.keepalive_enabled = false;

    let auto_config = MyServerAutoClientConfig {
        enabled: false,
        guest_id: None,
        ping_after_auth: false,
        join_after_auth: false,
        room_id: options.room.clone(),
        policy_id: options.policy.clone(),
    };
    let lockstep_config = LockstepSimConfig {
        scene_id: SceneId::from(LOCKSTEP_SIM_ARENA_SCENE_ID),
        local_player_id: options.player.clone(),
        authority_mode: LockstepSimAuthorityMode::MyServer,
        transport: NetworkTransport::Tcp,
        myserver_guest_id: None,
        myserver_room_id: options.room.clone(),
        myserver_policy_id: options.policy.clone(),
        debug_diagnostics: false,
    };

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(server_config)
        .insert_resource(auto_config)
        .insert_resource(lockstep_config)
        .add_plugins(NetworkPlugin)
        .add_plugins(MyServerPlugin)
        .add_plugins(AuthorityPlugin)
        .init_resource::<LockstepSimSceneState>()
        .init_resource::<LockstepSimReplayState>()
        .init_resource::<LockstepSimMyServerJoinState>()
        .add_systems(
            Update,
            (
                follow_lockstep_sim_myserver_events,
                apply_lockstep_sim_authority_events,
            )
                .chain(),
        );
    app
}

fn activate_online_scene(app: &mut App, options: &OnlineHeadlessOptions, ticket: String) {
    app.world_mut()
        .resource_mut::<LockstepSimSceneState>()
        .activate(
            SceneId::from(LOCKSTEP_SIM_ARENA_SCENE_ID),
            SceneSessionId::from(format!("headless-{}", options.room)),
        );
    {
        let mut state = app
            .world_mut()
            .resource_mut::<LockstepSimMyServerJoinState>();
        state.authority_started = true;
        state.login_sent = true;
    }
    app.world_mut().write_message(AuthorityCommand::Join {
        player_id: options.player.clone(),
        endpoint: AuthorityEndpoint::MyServer {
            host: Some(options.endpoint.ip().to_string()),
            port: Some(options.endpoint.port()),
            transport: NetworkTransport::Tcp,
        },
    });
    app.world_mut()
        .write_message(MyServerCommand::ConnectWithTicket {
            ticket,
            transport: NetworkTransport::Tcp,
            host: Some(options.endpoint.ip().to_string()),
            port: Some(options.endpoint.port()),
        });
}

fn read_myserver_events(
    app: &App,
    cursor: &mut MessageCursor<MyServerEvent>,
) -> Vec<MyServerEvent> {
    cursor
        .read(app.world().resource::<Messages<MyServerEvent>>())
        .cloned()
        .collect()
}

fn online_room_started(app: &App) -> bool {
    app.world()
        .resource::<LockstepSimMyServerJoinState>()
        .started
}

fn try_send_scripted_input(app: &mut App) -> Result<Option<u32>, OnlineHeadlessError> {
    let (player_id, authority_frame, snapshot_ready) = {
        let authority = app.world().resource::<AuthoritySession>();
        let scene = app.world().resource::<LockstepSimSceneState>();
        (
            authority.local_player_id.clone(),
            authority.frame_id,
            scene.initial_snapshot.is_some(),
        )
    };
    if !snapshot_ready {
        return Ok(None);
    }

    let player_id = player_id.ok_or_else(|| {
        OnlineHeadlessError::new(
            "HEADLESS_AUTH_PLAYER_MISSING",
            "input_prepare",
            "authority session has no authenticated gameplay character id",
        )
    })?;
    let context = {
        let scene = app.world().resource::<LockstepSimSceneState>();
        gate_lockstep_sim_input(
            &scene,
            Some(&player_id),
            None,
            None,
            Some(sim_core::SIM_CORE_SCHEMA_VERSION),
        )
        .map_err(|error| {
            OnlineHeadlessError::new(
                "HEADLESS_INPUT_GATE_FAILED",
                "input_prepare",
                error.to_string(),
            )
        })?
    };
    let snapshot_frame = app
        .world()
        .resource::<LockstepSimSceneState>()
        .initial_snapshot
        .as_ref()
        .map(|snapshot| snapshot.start_frame)
        .unwrap_or_default();
    let input_frame = first_scripted_input_frame(authority_frame, snapshot_frame);
    let input = build_sim_input_envelope(
        input_frame,
        1,
        &[
            SimCommand::Move(MoveCommand {
                dir: QuantizedDir::RIGHT,
                speed_per_second: Some(Fp::from_i32(6)),
            }),
            SimCommand::CastSkill(CastSkillCommand {
                skill_id: ONLINE_SKILL_ID,
                target: SkillTarget::Entity(ONLINE_TRAINING_TARGET_ID),
            }),
        ],
    )
    .map_err(|error| {
        OnlineHeadlessError::new(
            "HEADLESS_INPUT_SERIALIZE_FAILED",
            "input_prepare",
            error.to_string(),
        )
    })?;
    app.world_mut()
        .write_message(input.into_authority_command());
    let _ = context;
    Ok(Some(input_frame))
}

fn send_scripted_stop(app: &mut App, first_input_frame: u32) -> Result<u32, OnlineHeadlessError> {
    let authority_frame = app.world().resource::<AuthoritySession>().frame_id;
    let stop_frame = scripted_stop_frame(authority_frame, first_input_frame);
    let stop = build_sim_input_envelope(stop_frame, 2, &[SimCommand::Stop]).map_err(|error| {
        OnlineHeadlessError::new(
            "HEADLESS_INPUT_SERIALIZE_FAILED",
            "input_prepare",
            error.to_string(),
        )
    })?;
    app.world_mut().write_message(stop.into_authority_command());
    Ok(stop_frame)
}

fn first_scripted_input_frame(authority_frame: u32, snapshot_frame: u32) -> u32 {
    authority_frame
        .max(snapshot_frame)
        .saturating_add(ONLINE_DEMO_POLICY_INPUT_LEAD_FRAMES)
}

fn scripted_stop_frame(authority_frame: u32, first_input_frame: u32) -> u32 {
    authority_frame
        .max(first_input_frame)
        .saturating_add(ONLINE_DEMO_POLICY_INPUT_LEAD_FRAMES)
}

fn replay_error(app: &App) -> Option<OnlineHeadlessError> {
    let scene = app.world().resource::<LockstepSimSceneState>();
    let replay = app.world().resource::<LockstepSimReplayState>();
    if let Some(error) = scene.initial_snapshot_error.as_ref() {
        return Some(snapshot_rejection_error(error));
    }
    if scene.initial_snapshot.is_some()
        && let Some(error) = replay.last_error.as_ref()
    {
        return Some(OnlineHeadlessError::new(
            "HEADLESS_REPLAY_FAILED",
            "local_replay",
            error.to_string(),
        ));
    }
    None
}

fn snapshot_rejection_error(error: &LockstepSimSnapshotError) -> OnlineHeadlessError {
    let (error_code, failure_stage) = match error {
        LockstepSimSnapshotError::UnsupportedSchemaVersion { .. } => (
            "HEADLESS_SNAPSHOT_SCHEMA_VERSION_MISMATCH",
            "snapshot_schema_validation",
        ),
        LockstepSimSnapshotError::ConfigHashMismatch { .. } => (
            "HEADLESS_SNAPSHOT_CONFIG_HASH_MISMATCH",
            "snapshot_config_validation",
        ),
        LockstepSimSnapshotError::UnsupportedSimSchemaVersion { .. } => (
            "HEADLESS_SIM_SCHEMA_VERSION_MISMATCH",
            "sim_schema_validation",
        ),
        _ => ("HEADLESS_INITIAL_SNAPSHOT_REJECTED", "snapshot_restore"),
    };
    OnlineHeadlessError::new(
        error_code,
        failure_stage,
        format!("{}: {error}", lockstep_snapshot_error_code(error)),
    )
}

fn collect_report(
    app: &App,
    player_id: String,
    input_frame: u32,
    stop_frame: u32,
) -> Result<OnlineHeadlessReport, OnlineHeadlessError> {
    let scene = app.world().resource::<LockstepSimSceneState>();
    let snapshot = scene.initial_snapshot.as_ref().ok_or_else(|| {
        OnlineHeadlessError::new(
            "HEADLESS_INITIAL_SNAPSHOT_MISSING",
            "report",
            "online run completed without an initial snapshot",
        )
    })?;
    let player_entity_id = snapshot
        .control_bindings
        .get(&player_id)
        .copied()
        .ok_or_else(|| {
            OnlineHeadlessError::new(
                "HEADLESS_CONTROL_BINDING_MISSING",
                "report",
                format!("initial snapshot has no control binding for {player_id:?}"),
            )
        })?;
    let initial_world = snapshot.world.clone();
    let initial_hash = snapshot.initial_hash();

    let replay = app.world().resource::<LockstepSimReplayState>();
    let mut frames = Vec::with_capacity(replay.hash_history.len());
    for hash in &replay.hash_history {
        let (world, _) = replay
            .replay_from_cached_snapshot_to_frame(hash.frame)
            .map_err(|error| {
                OnlineHeadlessError::new(
                    "HEADLESS_REPORT_REPLAY_FAILED",
                    "report",
                    error.to_string(),
                )
            })?;
        let inputs = replay
            .input_history
            .iter()
            .find(|input| input.frame == hash.frame)
            .map(|input| input.sim_inputs.clone())
            .unwrap_or_default();
        let events = replay
            .event_history
            .iter()
            .find(|events| events.frame == hash.frame)
            .map(|events| events.events.clone())
            .unwrap_or_default();
        frames.push(OnlineHeadlessFrame {
            frame: hash.frame,
            server_hash: hash.server_hash.as_ref().map(|server| SimHash {
                frame: sim_core::FrameId::new(server.frame),
                value: server.value,
            }),
            local_hash: hash.local_hash,
            world,
            inputs,
            events,
        });
    }
    if frames.is_empty() {
        return Err(OnlineHeadlessError::new(
            "HEADLESS_FRAME_HISTORY_EMPTY",
            "report",
            "online replay produced no authoritative frame history",
        ));
    }

    let config = app.world().resource::<LockstepSimConfig>();
    let authority = app.world().resource::<AuthoritySession>();
    let hud_status = format_lockstep_sim_hud_status(&lockstep_sim_hud_snapshot(
        &config, &scene, &authority, &replay,
    ));

    Ok(OnlineHeadlessReport {
        player_id,
        player_entity_id,
        input_frame,
        stop_frame,
        initial_world,
        initial_hash,
        frames,
        hud_status,
    })
}

fn cleanup_online_session(
    app: &mut App,
    cursor: &mut MessageCursor<MyServerEvent>,
    timeout: Duration,
) -> Result<(), OnlineHeadlessError> {
    app.world_mut().write_message(MyServerCommand::EndRoom {
        reason: "mybevy-online-single-client-complete".to_string(),
    });
    let deadline = Instant::now() + timeout;
    let mut end_confirmed = false;
    let mut leave_sent = false;
    let mut leave_confirmed = false;
    let mut disconnect_sent = false;

    loop {
        app.update();
        for event in read_myserver_events(app, cursor) {
            match event {
                MyServerEvent::RoomEnded(response) if response.ok => end_confirmed = true,
                MyServerEvent::RoomEnded(response) => {
                    return Err(OnlineHeadlessError::new(
                        "HEADLESS_ROOM_END_REJECTED",
                        "cleanup_room_end",
                        format!("room end rejected: {}", response.error_code),
                    ));
                }
                MyServerEvent::RoomLeft(response) if response.ok => leave_confirmed = true,
                MyServerEvent::RoomLeft(response) => {
                    return Err(OnlineHeadlessError::new(
                        "HEADLESS_ROOM_LEAVE_REJECTED",
                        "cleanup_room_leave",
                        format!("room leave rejected: {}", response.error_code),
                    ));
                }
                MyServerEvent::Disconnected { .. } if disconnect_sent => return Ok(()),
                _ => {
                    if let Some(error) = failure_from_event(&event) {
                        return Err(OnlineHeadlessError::new(
                            error.error_code,
                            "cleanup",
                            error.message,
                        ));
                    }
                }
            }
        }

        if end_confirmed && !leave_sent {
            app.world_mut().write_message(MyServerCommand::LeaveRoom);
            leave_sent = true;
        }
        if leave_confirmed && !disconnect_sent {
            app.world_mut().write_message(MyServerCommand::Disconnect);
            disconnect_sent = true;
        }
        if disconnect_sent
            && app
                .world()
                .resource::<crate::game::myserver::MyServerSession>()
                .connection_id
                .is_none()
        {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(OnlineHeadlessError::new(
                "HEADLESS_CLEANUP_TIMEOUT",
                "cleanup",
                format!(
                    "online cleanup timed out (room_end={end_confirmed}, room_leave={leave_confirmed}, disconnect={disconnect_sent})"
                ),
            ));
        }
        thread::sleep(UPDATE_SLEEP);
    }
}

fn cleanup_dual_online_session(
    active_app: &mut App,
    active_cursor: &mut MessageCursor<MyServerEvent>,
    passive_app: &mut App,
    passive_cursor: &mut MessageCursor<MyServerEvent>,
    timeout: Duration,
) -> Result<(), OnlineHeadlessError> {
    active_app
        .world_mut()
        .write_message(MyServerCommand::EndRoom {
            reason: "mybevy-online-dual-client-complete".to_string(),
        });
    let deadline = Instant::now() + timeout;
    let mut end_confirmed = false;
    let mut leave_sent = false;
    let mut active_leave_confirmed = false;
    let mut passive_leave_confirmed = false;
    let mut active_disconnect_sent = false;
    let mut passive_disconnect_sent = false;

    loop {
        active_app.update();
        passive_app.update();
        for event in read_myserver_events(active_app, active_cursor) {
            match event {
                MyServerEvent::RoomEnded(response) if response.ok => end_confirmed = true,
                MyServerEvent::RoomEnded(response) => {
                    return Err(OnlineHeadlessError::new(
                        "HEADLESS_ROOM_END_REJECTED",
                        "cleanup_room_end",
                        format!("room end rejected: {}", response.error_code),
                    ));
                }
                MyServerEvent::RoomLeft(response) if response.ok => {
                    active_leave_confirmed = true;
                }
                MyServerEvent::RoomLeft(response) => {
                    return Err(OnlineHeadlessError::new(
                        "HEADLESS_ROOM_LEAVE_REJECTED",
                        "cleanup_room_leave",
                        format!("active room leave rejected: {}", response.error_code),
                    ));
                }
                _ => {}
            }
        }
        for event in read_myserver_events(passive_app, passive_cursor) {
            match event {
                MyServerEvent::RoomLeft(response) if response.ok => {
                    passive_leave_confirmed = true;
                }
                MyServerEvent::RoomLeft(response) => {
                    return Err(OnlineHeadlessError::new(
                        "HEADLESS_ROOM_LEAVE_REJECTED",
                        "cleanup_room_leave",
                        format!("passive room leave rejected: {}", response.error_code),
                    ));
                }
                _ => {}
            }
        }

        if end_confirmed && !leave_sent {
            active_app
                .world_mut()
                .write_message(MyServerCommand::LeaveRoom);
            passive_app
                .world_mut()
                .write_message(MyServerCommand::LeaveRoom);
            leave_sent = true;
        }
        if active_leave_confirmed && !active_disconnect_sent {
            active_app
                .world_mut()
                .write_message(MyServerCommand::Disconnect);
            active_disconnect_sent = true;
        }
        if passive_leave_confirmed && !passive_disconnect_sent {
            passive_app
                .world_mut()
                .write_message(MyServerCommand::Disconnect);
            passive_disconnect_sent = true;
        }
        if active_disconnect_sent
            && passive_disconnect_sent
            && myserver_connection_closed(active_app)
            && myserver_connection_closed(passive_app)
        {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(OnlineHeadlessError::new(
                "HEADLESS_DUAL_CLEANUP_TIMEOUT",
                "cleanup",
                format!(
                    "dual cleanup timed out (room_end={end_confirmed}, active_leave={active_leave_confirmed}, passive_leave={passive_leave_confirmed}, active_disconnect={active_disconnect_sent}, passive_disconnect={passive_disconnect_sent})"
                ),
            ));
        }
        thread::sleep(UPDATE_SLEEP);
    }
}

fn myserver_connection_closed(app: &App) -> bool {
    app.world()
        .resource::<crate::game::myserver::MyServerSession>()
        .connection_id
        .is_none()
}

fn failure_from_event(event: &MyServerEvent) -> Option<OnlineHeadlessError> {
    match event {
        MyServerEvent::ConnectionFailed { error, .. } => Some(OnlineHeadlessError::new(
            "HEADLESS_CONNECT_FAILED",
            "connect",
            error.clone(),
        )),
        MyServerEvent::AuthFailed { error_code }
        | MyServerEvent::GameAuthRejected { error_code, .. } => Some(OnlineHeadlessError::new(
            "HEADLESS_AUTH_REJECTED",
            "authentication",
            format!("ticket authentication rejected: {error_code}"),
        )),
        MyServerEvent::RoomJoined(response) if !response.ok => Some(OnlineHeadlessError::new(
            "HEADLESS_ROOM_JOIN_REJECTED",
            "room_join",
            format!("room join rejected: {}", response.error_code),
        )),
        MyServerEvent::ReadyChanged(response) if !response.ok => Some(OnlineHeadlessError::new(
            "HEADLESS_ROOM_READY_REJECTED",
            "room_ready",
            format!("room ready rejected: {}", response.error_code),
        )),
        MyServerEvent::RoomStarted(response) if !response.ok => Some(OnlineHeadlessError::new(
            "HEADLESS_ROOM_START_REJECTED",
            "room_start",
            format!("room start rejected: {}", response.error_code),
        )),
        MyServerEvent::PlayerInputAccepted(response) if !response.ok => {
            Some(OnlineHeadlessError::new(
                "HEADLESS_INPUT_REJECTED",
                "player_input",
                format!("player input rejected: {}", response.error_code),
            ))
        }
        MyServerEvent::Error {
            error_code,
            message,
            ..
        } => Some(OnlineHeadlessError::new(
            "HEADLESS_SERVER_ERROR",
            "protocol",
            format!("MyServer error {error_code}: {message}"),
        )),
        MyServerEvent::ProtocolError { error } => Some(OnlineHeadlessError::new(
            "HEADLESS_PROTOCOL_ERROR",
            "protocol",
            error.clone(),
        )),
        MyServerEvent::RequestFailed {
            message_type,
            error,
            ..
        } => Some(OnlineHeadlessError::new(
            "HEADLESS_REQUEST_FAILED",
            "protocol",
            format!("MyServer request {message_type:?} failed: {error}"),
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_world(frame: u32) -> SimWorld {
        SimWorld::new(sim_core::FrameId::new(frame), Vec::new()).unwrap()
    }

    fn matching_frame(frame: u32, with_stop: bool) -> OnlineHeadlessFrame {
        let world = empty_world(frame);
        let hash = sim_core::hash_world(&world);
        let inputs = with_stop
            .then(|| SimInput {
                frame: sim_core::FrameId::new(frame),
                character_id: "primary-player".to_string(),
                entity_id: EntityId::new(1),
                seq: 2,
                source: sim_core::SimInputSource::Real,
                command: SimCommand::Stop,
            })
            .into_iter()
            .collect();
        OnlineHeadlessFrame {
            frame,
            server_hash: Some(hash),
            local_hash: hash,
            world,
            inputs,
            events: Vec::new(),
        }
    }

    fn recovery_stream(player_id: &str, observer: bool) -> OnlineRecoveryStreamReport {
        let snapshot_world = empty_world(3);
        OnlineRecoveryStreamReport {
            player_id: player_id.to_string(),
            snapshot_frame: 3,
            snapshot_hash: sim_core::hash_world(&snapshot_world),
            snapshot_world,
            recovery_generation: 3,
            response_current_frame: 3,
            response_waiting_frame: 4,
            response_recent_input_frames: vec![3],
            response_waiting_input_frames: vec![4],
            frames: vec![matching_frame(4, true), matching_frame(5, false)],
            ignored_duplicate_or_old_frames: 0,
            local_input_acknowledgements: if observer { 0 } else { 2 },
            has_control_binding: !observer,
        }
    }

    fn assert_snapshot_rejection_stops_replay(
        error: LockstepSimSnapshotError,
        expected_code: &str,
        expected_stage: &str,
    ) {
        let mut app = App::new();
        app.add_message::<crate::game::authority::AuthorityEvent>()
            .init_resource::<LockstepSimConfig>()
            .init_resource::<LockstepSimSceneState>()
            .init_resource::<LockstepSimReplayState>()
            .add_systems(Update, apply_lockstep_sim_authority_events);
        {
            let mut scene = app.world_mut().resource_mut::<LockstepSimSceneState>();
            scene.active = true;
            scene.reject_initial_snapshot(error);
        }
        {
            let mut replay = app.world_mut().resource_mut::<LockstepSimReplayState>();
            replay.world = Some(empty_world(7));
            replay.last_applied_frame = Some(7);
        }

        app.update();

        let replay = app.world().resource::<LockstepSimReplayState>();
        assert!(replay.world.is_none());
        assert_eq!(replay.last_applied_frame, None);
        assert!(replay.hash_history.is_empty());
        let failure = replay_error(&app).unwrap();
        assert_eq!(failure.error_code, expected_code);
        assert_eq!(failure.failure_stage, expected_stage);
    }

    fn options() -> OnlineHeadlessOptions {
        OnlineHeadlessOptions {
            endpoint: "127.0.0.1:7000".parse().unwrap(),
            ticket_env: "MYSERVER_LOCKSTEP_TICKET".to_string(),
            room: "lockstep-test-room".to_string(),
            policy: "lockstep_sim_demo".to_string(),
            player: "lockstep-test-player".to_string(),
            timeout: Duration::from_secs(5),
        }
    }

    #[test]
    fn online_headless_options_require_loopback_and_safe_ticket_env_name() {
        assert_eq!(validate_options(&options()), Ok(()));

        let mut non_loopback = options();
        non_loopback.endpoint = "192.0.2.1:7000".parse().unwrap();
        assert_eq!(
            validate_options(&non_loopback).unwrap_err().error_code,
            "HEADLESS_ENDPOINT_NOT_LOOPBACK"
        );

        let mut invalid_env = options();
        invalid_env.ticket_env = "BAD-NAME".to_string();
        assert_eq!(
            validate_options(&invalid_env).unwrap_err().error_code,
            "HEADLESS_TICKET_ENV_INVALID"
        );
    }

    #[test]
    fn online_headless_app_disables_http_auto_login_and_uses_direct_tcp_endpoint() {
        let options = options();
        let app = build_online_app(&options);
        let server = app.world().resource::<MyServerConfig>();
        let auto = app.world().resource::<MyServerAutoClientConfig>();

        assert_eq!(server.game_host, "127.0.0.1");
        assert_eq!(server.tcp_fallback_port, 7000);
        assert_eq!(server.forced_transport, Some(NetworkTransport::Tcp));
        assert!(!server.auto_reconnect_with_fresh_ticket);
        assert!(!server.keepalive_enabled);
        assert!(!auto.enabled);
    }

    #[test]
    fn online_headless_first_input_uses_demo_policy_lead_after_latest_known_frame() {
        assert_eq!(first_scripted_input_frame(10, 7), 12);
        assert_eq!(first_scripted_input_frame(7, 10), 12);
        assert_eq!(first_scripted_input_frame(u32::MAX, 10), u32::MAX);
    }

    #[test]
    fn online_headless_stop_uses_demo_policy_lead_after_authority_and_first_input() {
        assert_eq!(scripted_stop_frame(11, 12), 14);
        assert_eq!(scripted_stop_frame(13, 12), 15);
        assert_eq!(scripted_stop_frame(10, u32::MAX), u32::MAX);
    }

    #[test]
    fn online_dual_headless_requires_distinct_ticket_environment_names() {
        let options = OnlineDualHeadlessOptions {
            endpoint: "127.0.0.1:7000".parse().unwrap(),
            primary_ticket_env: "MYSERVER_LOCKSTEP_TICKET".to_string(),
            passive_ticket_env: "MYSERVER_LOCKSTEP_TICKET".to_string(),
            room: "lockstep-test-room".to_string(),
            policy: "lockstep_sim_demo".to_string(),
            primary_player: "active-placeholder".to_string(),
            passive_player: "passive-placeholder".to_string(),
            timeout: Duration::from_secs(5),
        };

        assert_eq!(
            run_online_dual_headless(&options).unwrap_err().error_code,
            "HEADLESS_DUAL_TICKET_ENV_NOT_DISTINCT"
        );
    }

    #[test]
    fn online_dual_passive_client_rejects_any_input_acknowledgement() {
        let mut progress = OnlineDualProgress::default();
        let events = vec![MyServerEvent::PlayerInputAccepted(Default::default())];

        assert_eq!(
            observe_dual_client_events(&events, &mut progress, false)
                .unwrap_err()
                .error_code,
            "HEADLESS_DUAL_PASSIVE_INPUT_SENT"
        );
        assert_eq!(progress.accepted_inputs, 0);
    }

    #[test]
    fn online_dual_deferred_start_is_explicit_per_client() {
        let mut app = build_online_app(&options());
        defer_automatic_room_start(&mut app);

        let state = app.world().resource::<LockstepSimMyServerJoinState>();
        assert!(state.defer_start_room);
        assert!(!state.start_sent);
    }

    #[test]
    fn reconnect_snapshot_mismatches_have_stable_codes_and_stop_replay() {
        assert_snapshot_rejection_stops_replay(
            LockstepSimSnapshotError::UnsupportedSchemaVersion {
                actual: 2,
                expected: 1,
            },
            "HEADLESS_SNAPSHOT_SCHEMA_VERSION_MISMATCH",
            "snapshot_schema_validation",
        );
        assert_snapshot_rejection_stops_replay(
            LockstepSimSnapshotError::ConfigHashMismatch {
                actual: "server-config".to_string(),
                expected: "client-config".to_string(),
            },
            "HEADLESS_SNAPSHOT_CONFIG_HASH_MISMATCH",
            "snapshot_config_validation",
        );
        assert_snapshot_rejection_stops_replay(
            LockstepSimSnapshotError::UnsupportedSimSchemaVersion {
                actual: sim_core::SIM_CORE_SCHEMA_VERSION.saturating_add(1),
                expected: sim_core::SIM_CORE_SCHEMA_VERSION,
            },
            "HEADLESS_SIM_SCHEMA_VERSION_MISMATCH",
            "sim_schema_validation",
        );
    }

    #[test]
    fn recovery_frame_continuity_rejects_gap_without_advancing_assertion() {
        let frames = vec![matching_frame(5, false)];

        let error = validate_recovery_frame_continuity(3, &frames).unwrap_err();

        assert_eq!(error.error_code, "HEADLESS_RECOVERY_FRAME_GAP");
        assert_eq!(error.failure_stage, "frame_continuity");
    }

    #[test]
    fn observer_recovery_has_no_local_input_and_matches_primary_frames() {
        let primary = recovery_stream("primary-player", false);
        let observer = recovery_stream("observer-player", true);

        let common = reconcile_recovery_streams(&primary, &observer, 4, 5).unwrap();

        assert_eq!(common, vec![4, 5]);
        assert_eq!(observer.local_input_acknowledgements, 0);
        assert!(!observer.has_control_binding);
    }

    #[test]
    fn observer_recovery_rejects_local_input_acknowledgement() {
        let primary = recovery_stream("primary-player", false);
        let mut observer = recovery_stream("observer-player", true);
        observer.local_input_acknowledgements = 1;

        let error = reconcile_recovery_streams(&primary, &observer, 4, 5).unwrap_err();

        assert_eq!(error.error_code, "HEADLESS_OBSERVER_INPUT_ROLE_VIOLATION");
        assert_eq!(error.failure_stage, "observer_input_role");
    }
}
