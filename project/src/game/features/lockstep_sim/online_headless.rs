use std::{
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
        network::{NetworkPlugin, NetworkTransport},
        scene::prelude::{SceneId, SceneSessionId},
    },
    game::{
        authority::{AuthorityCommand, AuthorityEndpoint, AuthorityPlugin, AuthoritySession},
        myserver::{
            MyServerAutoClientConfig, MyServerCommand, MyServerConfig, MyServerEvent,
            MyServerPlugin,
        },
        scenes::LOCKSTEP_SIM_ARENA_SCENE_ID,
    },
};

use super::{
    config::{LockstepSimAuthorityMode, LockstepSimConfig},
    hud::{format_lockstep_sim_hud_status, lockstep_sim_hud_snapshot},
    payload::{build_sim_input_envelope, gate_lockstep_sim_input},
    replay::{LockstepSimReplayState, apply_lockstep_sim_authority_events},
    state::LockstepSimSceneState,
    sync::{LockstepSimMyServerJoinState, follow_lockstep_sim_myserver_events},
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
        return Some(OnlineHeadlessError::new(
            "HEADLESS_INITIAL_SNAPSHOT_REJECTED",
            "snapshot_restore",
            error.to_string(),
        ));
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
}
