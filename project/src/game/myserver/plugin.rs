use std::time::SystemTime;
use std::{env, time::Duration};

use bevy::prelude::*;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::framework::network::{
    ConnectionId, HttpMethod, HttpRequest, KcpConnectConfig, KcpSessionOptions, NetworkCommand,
    NetworkEvent, NetworkTransport, RequestId, TcpConnectConfig,
};

use super::protocol::{MessageType, Packet, encode_proto_packet, pb};
use super::types::{
    ApiErrorResponse, CharacterCreateResponse, CharacterLifecycleResponse, CharacterListResponse,
    CharacterProfileResponse, CharacterSelectResponse, ConnectPlan, DEFAULT_KEEPALIVE_INTERVAL,
    GameConnectionState, LoginResponse, MovementClientState, MyServerAutoClientConfig,
    MyServerAutoClientState, MyServerCommand, MyServerConfig, MyServerDiagnosticSnapshot,
    MyServerDisplayError, MyServerErrorSource, MyServerEvent, MyServerOperation, MyServerSession,
    PendingHttpOperation, PendingHttpRequest, PendingRequest, ReconnectCause, ReconnectPlan,
    RegisterPendingReviewResponse, RegisterResponse, SessionKickCategory, TicketResponse,
    character_select_endpoint, classify_game_auth_failure, parse_character_bound_ticket,
    redact_secret_fingerprint, ticket_endpoint,
};

pub struct MyServerPlugin;

impl Plugin for MyServerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MyServerConfig>()
            .init_resource::<MyServerSession>()
            .init_resource::<MyServerAutoClientConfig>()
            .init_resource::<MyServerAutoClientState>()
            .init_resource::<MyServerKeepaliveState>()
            .add_message::<MyServerCommand>()
            .add_message::<MyServerEvent>()
            .add_systems(Startup, auto_client_startup)
            .add_systems(
                Update,
                (
                    handle_myserver_commands,
                    handle_network_events,
                    keepalive_myserver_connection,
                    auto_client_follow_events,
                )
                    .chain(),
            );
    }
}

#[derive(Resource, Debug)]
struct MyServerKeepaliveState {
    timer: Timer,
    interval: Duration,
}

impl Default for MyServerKeepaliveState {
    fn default() -> Self {
        Self {
            timer: Timer::new(DEFAULT_KEEPALIVE_INTERVAL, TimerMode::Repeating),
            interval: DEFAULT_KEEPALIVE_INTERVAL,
        }
    }
}

fn myserver_debug_trace_enabled() -> bool {
    // Online defaults keep detailed transition diagnostics at trace level; local
    // debugging can promote the same redacted fields to debug with this switch.
    env::var("MYSERVER_DIAGNOSTIC_TRACE")
        .ok()
        .map(|value| {
            matches!(
                value.as_str(),
                "1" | "true" | "TRUE" | "True" | "yes" | "YES" | "debug" | "trace"
            )
        })
        .unwrap_or(false)
}

fn diagnostic_snapshot(session: &MyServerSession) -> MyServerDiagnosticSnapshot {
    MyServerDiagnosticSnapshot::from_session(session, SystemTime::now())
}

fn trace_http_transition(
    phase: &'static str,
    request_id: RequestId,
    operation: &PendingHttpOperation,
    status: Option<u16>,
    error_code: Option<&str>,
    session: &MyServerSession,
) {
    let snapshot = diagnostic_snapshot(session);
    let connection_id = snapshot.connection_id.map(ConnectionId::raw);
    if myserver_debug_trace_enabled() {
        debug!(
            phase,
            request_id = request_id.raw(),
            operation = operation.label(),
            endpoint = operation.endpoint_path(),
            status,
            error_code = error_code.unwrap_or_default(),
            account_state = ?snapshot.account_login_state,
            character_state = ?snapshot.character_selection_state,
            game_state = ?snapshot.game_connection_state,
            connection_id,
            transport = ?snapshot.transport,
            player_id = snapshot.player_id.as_deref().unwrap_or_default(),
            character_id = snapshot.character_id.as_deref().unwrap_or_default(),
            world_id = snapshot.world_id,
            access_token_fp = snapshot.access_token_fingerprint.as_deref().unwrap_or_default(),
            ticket_fp = snapshot.ticket_fingerprint.as_deref().unwrap_or_default(),
            ticket_expires_at = snapshot.ticket_expires_at.as_deref().unwrap_or_default(),
            ticket_remaining_seconds = snapshot.ticket_remaining_seconds,
            "MyServer HTTP diagnostic transition"
        );
    } else {
        trace!(
            phase,
            request_id = request_id.raw(),
            operation = operation.label(),
            endpoint = operation.endpoint_path(),
            status,
            error_code = error_code.unwrap_or_default(),
            account_state = ?snapshot.account_login_state,
            character_state = ?snapshot.character_selection_state,
            game_state = ?snapshot.game_connection_state,
            connection_id,
            ticket_fp = snapshot.ticket_fingerprint.as_deref().unwrap_or_default(),
            ticket_remaining_seconds = snapshot.ticket_remaining_seconds,
            "MyServer HTTP diagnostic transition"
        );
    }
}

fn trace_game_transition(
    phase: &'static str,
    session: &MyServerSession,
    connection_id: Option<ConnectionId>,
    endpoint: Option<&str>,
    seq: Option<u32>,
    message_type: Option<MessageType>,
    error_code: Option<&str>,
) {
    let snapshot = diagnostic_snapshot(session);
    let connection_id = connection_id
        .or(snapshot.connection_id)
        .map(ConnectionId::raw);
    if myserver_debug_trace_enabled() {
        debug!(
            phase,
            connection_id,
            endpoint = endpoint.unwrap_or_default(),
            seq,
            message_type = ?message_type,
            error_code = error_code.unwrap_or_default(),
            account_state = ?snapshot.account_login_state,
            character_state = ?snapshot.character_selection_state,
            game_state = ?snapshot.game_connection_state,
            transport = ?snapshot.transport,
            player_id = snapshot.player_id.as_deref().unwrap_or_default(),
            character_id = snapshot.character_id.as_deref().unwrap_or_default(),
            world_id = snapshot.world_id,
            ticket_fp = snapshot.ticket_fingerprint.as_deref().unwrap_or_default(),
            ticket_expires_at = snapshot.ticket_expires_at.as_deref().unwrap_or_default(),
            ticket_remaining_seconds = snapshot.ticket_remaining_seconds,
            "MyServer game diagnostic transition"
        );
    } else {
        trace!(
            phase,
            connection_id,
            endpoint = endpoint.unwrap_or_default(),
            seq,
            message_type = ?message_type,
            error_code = error_code.unwrap_or_default(),
            game_state = ?snapshot.game_connection_state,
            ticket_fp = snapshot.ticket_fingerprint.as_deref().unwrap_or_default(),
            ticket_remaining_seconds = snapshot.ticket_remaining_seconds,
            "MyServer game diagnostic transition"
        );
    }
}

fn auto_client_startup(
    config: Res<MyServerAutoClientConfig>,
    mut commands: MessageWriter<MyServerCommand>,
) {
    if !config.enabled {
        return;
    }

    info!(
        guest_id = config.guest_id.as_deref().unwrap_or_default(),
        "MyServer auto client starting guest login"
    );
    commands.write(MyServerCommand::GuestLogin {
        guest_id: config.guest_id.clone(),
        connect_game: true,
    });
}

fn auto_client_follow_events(
    config: Res<MyServerAutoClientConfig>,
    myserver_session: Res<MyServerSession>,
    mut state: ResMut<MyServerAutoClientState>,
    mut events: MessageReader<MyServerEvent>,
    mut commands: MessageWriter<MyServerCommand>,
) {
    let should_prepare_character = myserver_session.connect_after_login.is_some();
    if !config.enabled && !should_prepare_character {
        return;
    }

    for event in events.read() {
        match event {
            MyServerEvent::LoginSucceeded(session) => {
                info!(
                    player_id = %session.player_id,
                    game_host = session.game_host.as_deref().unwrap_or_default(),
                    game_port = session.game_port.unwrap_or_default(),
                    "MyServer login succeeded"
                );
                if state.character_flow_player_id.as_deref() != Some(session.player_id.as_str()) {
                    state.reset_character_flow();
                    state.character_flow_player_id = Some(session.player_id.clone());
                }
                if should_prepare_character
                    && session.ticket.is_none()
                    && !state.character_list_sent
                {
                    state.character_list_sent = true;
                    commands.write(MyServerCommand::LoadCharacterList);
                }
            }
            MyServerEvent::LoginFailed { error } => {
                state.reset_character_flow();
                error!(%error, "MyServer login failed");
            }
            MyServerEvent::CharacterListLoaded { characters, .. } if should_prepare_character => {
                if let Some(character) = characters.first() {
                    auto_client_select_character(
                        &mut state,
                        &mut commands,
                        character.character_id.clone(),
                    );
                } else if !state.character_create_sent {
                    state.character_create_sent = true;
                    let name = auto_client_character_name(
                        myserver_session
                            .guest_id
                            .as_deref()
                            .or(config.guest_id.as_deref()),
                    );
                    commands.write(MyServerCommand::CreateCharacter {
                        name,
                        appearance_json: None,
                    });
                }
            }
            MyServerEvent::CharacterCreationRequired { .. } if should_prepare_character => {
                if !state.character_create_sent {
                    state.character_create_sent = true;
                    let name = auto_client_character_name(
                        myserver_session
                            .guest_id
                            .as_deref()
                            .or(config.guest_id.as_deref()),
                    );
                    commands.write(MyServerCommand::CreateCharacter {
                        name,
                        appearance_json: None,
                    });
                }
            }
            MyServerEvent::CharacterCreated { character } if should_prepare_character => {
                state.pending_created_character_id = Some(character.character_id.clone());
                auto_client_select_character(
                    &mut state,
                    &mut commands,
                    character.character_id.clone(),
                );
            }
            MyServerEvent::Connecting {
                transport,
                remote_addr,
                ..
            } => {
                info!(?transport, %remote_addr, "MyServer connecting");
            }
            MyServerEvent::Connected {
                transport,
                remote_addr,
                ..
            } => {
                info!(?transport, %remote_addr, "MyServer transport connected");
            }
            MyServerEvent::ConnectionFailed {
                transport,
                remote_addr,
                error,
            } => {
                error!(?transport, %remote_addr, %error, "MyServer connection failed");
            }
            MyServerEvent::Authenticated { player_id } => {
                info!(%player_id, "MyServer game auth succeeded");

                if config.enabled && config.ping_after_auth && !state.ping_sent {
                    state.ping_sent = true;
                    commands.write(MyServerCommand::Ping {
                        client_time_ms: current_unix_ms(),
                    });
                }

                if config.enabled && config.join_after_auth && !state.join_sent {
                    state.join_sent = true;
                    commands.write(MyServerCommand::JoinRoom {
                        room_id: config.room_id.clone(),
                        policy_id: config.policy_id.clone(),
                    });
                }
            }
            MyServerEvent::AuthFailed { error_code } => {
                error!(%error_code, "MyServer game auth failed");
            }
            MyServerEvent::GameAuthRejected { error_code, reason } => {
                error!(%error_code, ?reason, "MyServer game auth rejected");
            }
            MyServerEvent::Pong(response) => {
                info!(server_time = response.server_time, "MyServer ping response");
            }
            MyServerEvent::RoomJoined(response) => {
                info!(
                    ok = response.ok,
                    room_id = %response.room_id,
                    error_code = %response.error_code,
                    "MyServer room join response"
                );
            }
            MyServerEvent::RoomStatePush(push) => {
                let snapshot = push.snapshot.as_ref();
                info!(
                    event = %push.event,
                    room_id = snapshot.map(|value| value.room_id.as_str()).unwrap_or_default(),
                    state = snapshot.map(|value| value.state.as_str()).unwrap_or_default(),
                    "MyServer room state push"
                );
            }
            MyServerEvent::MovementSnapshotPush(push) => {
                info!(
                    room_id = %push.room_id,
                    frame_id = push.frame_id,
                    entity_count = push.entities.len(),
                    "MyServer movement snapshot"
                );
            }
            MyServerEvent::CharacterElementsChanged(push) => {
                let meta = push.meta.as_ref();
                info!(
                    character_id = meta
                        .map(|value| value.character_id.as_str())
                        .unwrap_or_default(),
                    sequence = meta.map(|value| value.sequence).unwrap_or_default(),
                    revision = meta.map(|value| value.revision).unwrap_or_default(),
                    "MyServer character elements changed"
                );
            }
            MyServerEvent::Error {
                seq,
                error_code,
                message,
            } => {
                error!(seq = *seq, %error_code, %message, "MyServer protocol error response");
            }
            MyServerEvent::ProtocolError { error } => {
                error!(%error, "MyServer protocol decode error");
            }
            MyServerEvent::RequestFailed {
                seq,
                message_type,
                error,
            } => {
                error!(?seq, ?message_type, %error, "MyServer request failed");
            }
            MyServerEvent::Disconnected { reason } => {
                warn!(?reason, "MyServer disconnected");
            }
            MyServerEvent::LogoutSucceeded => {
                state.reset_character_flow();
            }
            _ => {}
        }
    }
}

fn auto_client_select_character(
    state: &mut MyServerAutoClientState,
    commands: &mut MessageWriter<MyServerCommand>,
    character_id: String,
) {
    if state.character_select_sent {
        return;
    }
    state.character_select_sent = true;
    commands.write(MyServerCommand::SelectCharacter {
        character_id,
        connect_game: true,
    });
}

fn auto_client_character_name(guest_id: Option<&str>) -> String {
    let suffix = guest_id
        .and_then(|guest_id| {
            let filtered = guest_id
                .chars()
                .filter(|ch| ch.is_ascii_alphanumeric())
                .collect::<String>();
            if filtered.is_empty() {
                None
            } else {
                Some(filtered)
            }
        })
        .unwrap_or_else(|| "Dev".to_string());
    let suffix = suffix
        .chars()
        .rev()
        .take(8)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    let mut name = format!("Bevy{suffix}");
    if name.len() > 16 {
        name.truncate(16);
    }
    if name.len() < 2 {
        "BevyDev".to_string()
    } else {
        name
    }
}

fn handle_myserver_commands(
    config: Res<MyServerConfig>,
    mut session: ResMut<MyServerSession>,
    mut commands: MessageReader<MyServerCommand>,
    mut network_commands: MessageWriter<NetworkCommand>,
    mut events: MessageWriter<MyServerEvent>,
) {
    for command in commands.read() {
        match command {
            MyServerCommand::Login {
                login_name,
                password,
                connect_game,
            } => send_login(
                &config,
                &mut session,
                &mut network_commands,
                &mut events,
                login_name,
                password,
                *connect_game,
            ),
            MyServerCommand::Register {
                login_name,
                password,
                connect_game,
            } => send_register(
                &config,
                &mut session,
                &mut network_commands,
                &mut events,
                login_name,
                password,
                *connect_game,
            ),
            MyServerCommand::GuestLogin {
                guest_id,
                connect_game,
            } => send_guest_login(
                &config,
                &mut session,
                &mut network_commands,
                &mut events,
                guest_id.as_deref(),
                *connect_game,
            ),
            MyServerCommand::LoadCharacterList => {
                send_character_list(&config, &mut session, &mut network_commands, &mut events)
            }
            MyServerCommand::CreateCharacter {
                name,
                appearance_json,
            } => send_character_create(
                &config,
                &mut session,
                &mut network_commands,
                &mut events,
                name,
                appearance_json.clone(),
            ),
            MyServerCommand::LoadCharacterProfile { character_id } => send_character_profile(
                &config,
                &mut session,
                &mut network_commands,
                &mut events,
                character_id,
            ),
            MyServerCommand::SelectCharacter {
                character_id,
                connect_game,
            } => send_character_select(
                &config,
                &mut session,
                &mut network_commands,
                &mut events,
                character_id,
                *connect_game,
            ),
            MyServerCommand::DeleteCharacter { character_id } => send_character_lifecycle(
                &config,
                &mut session,
                &mut network_commands,
                &mut events,
                PendingHttpOperation::CharacterDelete {
                    character_id: character_id.clone(),
                },
            ),
            MyServerCommand::RestoreCharacter { character_id } => send_character_lifecycle(
                &config,
                &mut session,
                &mut network_commands,
                &mut events,
                PendingHttpOperation::CharacterRestore {
                    character_id: character_id.clone(),
                },
            ),
            MyServerCommand::IssueTicket { reconnect_game }
            | MyServerCommand::RefreshTicket { reconnect_game } => send_refresh_ticket(
                &config,
                &mut session,
                &mut network_commands,
                &mut events,
                *reconnect_game,
            ),
            MyServerCommand::ConnectWithTicket {
                ticket,
                transport,
                host,
                port,
            } => {
                session.clear_reconnect_plan();
                connect_with_ticket(
                    &config,
                    &mut session,
                    &mut network_commands,
                    &mut events,
                    ticket.clone(),
                    ConnectPlan {
                        transport: *transport,
                        host: host.clone(),
                        port: *port,
                    },
                )
            }
            MyServerCommand::Disconnect => disconnect(&mut session, &mut network_commands),
            MyServerCommand::Logout => {
                send_logout(&config, &mut session, &mut network_commands, &mut events);
            }
            MyServerCommand::Ping { client_time_ms } => send_request(
                &mut session,
                &mut network_commands,
                &mut events,
                MessageType::PingReq,
                MessageType::PingRes,
                &pb::PingReq {
                    client_time: *client_time_ms,
                },
            ),
            MyServerCommand::JoinRoom { room_id, policy_id } => send_request(
                &mut session,
                &mut network_commands,
                &mut events,
                MessageType::RoomJoinReq,
                MessageType::RoomJoinRes,
                &pb::RoomJoinReq {
                    room_id: room_id.clone(),
                    policy_id: policy_id.clone(),
                },
            ),
            MyServerCommand::LeaveRoom => send_request(
                &mut session,
                &mut network_commands,
                &mut events,
                MessageType::RoomLeaveReq,
                MessageType::RoomLeaveRes,
                &pb::RoomLeaveReq {},
            ),
            MyServerCommand::SetReady { ready } => send_request(
                &mut session,
                &mut network_commands,
                &mut events,
                MessageType::RoomReadyReq,
                MessageType::RoomReadyRes,
                &pb::RoomReadyReq { ready: *ready },
            ),
            MyServerCommand::StartRoom => send_request(
                &mut session,
                &mut network_commands,
                &mut events,
                MessageType::RoomStartReq,
                MessageType::RoomStartRes,
                &pb::RoomStartReq {},
            ),
            MyServerCommand::SendPlayerInput {
                frame_id,
                action,
                payload_json,
            } => send_request(
                &mut session,
                &mut network_commands,
                &mut events,
                MessageType::PlayerInputReq,
                MessageType::PlayerInputRes,
                &pb::PlayerInputReq {
                    frame_id: *frame_id,
                    action: action.clone(),
                    payload_json: payload_json.clone(),
                    client_timestamp_ms: current_unix_ms(),
                },
            ),
            MyServerCommand::SendMoveInput {
                frame_id,
                input_type,
                dir_x,
                dir_y,
                client_state,
            } => send_move_input(
                &mut session,
                &mut network_commands,
                &mut events,
                *frame_id,
                *input_type,
                *dir_x,
                *dir_y,
                *client_state,
            ),
        }
    }
}

fn handle_network_events(
    config: Res<MyServerConfig>,
    mut session: ResMut<MyServerSession>,
    mut network_events: MessageReader<NetworkEvent>,
    mut network_commands: MessageWriter<NetworkCommand>,
    mut events: MessageWriter<MyServerEvent>,
) {
    for event in network_events.read() {
        match event {
            NetworkEvent::HttpResponse(response) => {
                let Some(pending) = session.pending_http.remove(&response.request_id) else {
                    continue;
                };
                clear_legacy_http_slots(&mut session, response.request_id);
                handle_http_response(
                    &config,
                    &mut session,
                    &mut network_commands,
                    &mut events,
                    response.request_id,
                    pending.operation,
                    response.status,
                    &response.body,
                );
            }
            NetworkEvent::HttpError { request_id, error } => {
                let Some(pending) = session.pending_http.remove(request_id) else {
                    continue;
                };
                clear_legacy_http_slots(&mut session, *request_id);
                trace_http_transition(
                    "http_error",
                    *request_id,
                    &pending.operation,
                    None,
                    None,
                    &session,
                );
                apply_http_failure_state(&mut session, &pending.operation, error);
                if matches!(
                    pending.operation,
                    PendingHttpOperation::TicketIssue {
                        reconnect_game: true
                    }
                ) {
                    session.clear_reconnect_plan();
                }
                let operation = pending.operation.event_operation();
                trace_http_transition(
                    "http_error_applied",
                    *request_id,
                    &pending.operation,
                    None,
                    None,
                    &session,
                );
                write_display_error(
                    &mut events,
                    MyServerDisplayError::transport(operation, Some(error.clone())),
                );
                write_legacy_http_failure(&mut events, &pending.operation, error.clone());
                events.write(MyServerEvent::NetworkFailed {
                    operation,
                    error: error.clone(),
                });
            }
            NetworkEvent::Connected {
                connection_id,
                transport,
                remote_addr,
            } if Some(*connection_id) == session.connection_id => {
                session.game_connected(*transport);
                trace_game_transition(
                    "game_connected",
                    &session,
                    Some(*connection_id),
                    Some(remote_addr),
                    None,
                    None,
                    None,
                );
                events.write(MyServerEvent::Connected {
                    connection_id: *connection_id,
                    transport: *transport,
                    remote_addr: remote_addr.clone(),
                });

                let Some(ticket) = session.ticket.clone() else {
                    session.game_auth_failed();
                    trace_game_transition(
                        "game_auth_missing_ticket",
                        &session,
                        Some(*connection_id),
                        Some(remote_addr),
                        None,
                        Some(MessageType::AuthReq),
                        Some("MISSING_TICKET"),
                    );
                    write_display_error(
                        &mut events,
                        display_error_from_game_code(
                            Some(MessageType::AuthReq),
                            None,
                            "MISSING_TICKET",
                            Some("connected without a ticket".to_string()),
                        ),
                    );
                    events.write(MyServerEvent::RequestFailed {
                        seq: None,
                        message_type: Some(MessageType::AuthReq),
                        error: "connected without a ticket".to_string(),
                    });
                    continue;
                };

                if let Err(error) = validate_auth_ticket(&ticket, &session) {
                    session.game_auth_failed();
                    trace_game_transition(
                        "game_auth_ticket_rejected",
                        &session,
                        Some(*connection_id),
                        Some(remote_addr),
                        None,
                        Some(MessageType::AuthReq),
                        Some(&error),
                    );
                    events.write(MyServerEvent::AuthFailed {
                        error_code: error.clone(),
                    });
                    events.write(MyServerEvent::GameAuthRejected {
                        error_code: error.clone(),
                        reason: classify_game_auth_failure(&error),
                    });
                    write_display_error(
                        &mut events,
                        display_error_from_game_code(
                            Some(MessageType::AuthReq),
                            None,
                            &error,
                            Some(error.clone()),
                        ),
                    );
                    events.write(MyServerEvent::RequestFailed {
                        seq: None,
                        message_type: Some(MessageType::AuthReq),
                        error,
                    });
                    continue;
                }

                send_auth_request(&mut session, &mut network_commands, &mut events, ticket);
            }
            NetworkEvent::ConnectionFailed {
                connection_id,
                transport,
                remote_addr,
                error,
            } if Some(*connection_id) == session.connection_id => {
                session.game_connection_failed();
                trace_game_transition(
                    "game_connection_failed",
                    &session,
                    Some(*connection_id),
                    Some(remote_addr),
                    None,
                    None,
                    None,
                );
                events.write(MyServerEvent::ConnectionFailed {
                    transport: *transport,
                    remote_addr: remote_addr.clone(),
                    error: error.clone(),
                });
                write_display_error(
                    &mut events,
                    MyServerDisplayError::transport(
                        MyServerOperation::GameConnect,
                        Some(error.clone()),
                    ),
                );
                events.write(MyServerEvent::NetworkFailed {
                    operation: MyServerOperation::GameConnect,
                    error: error.clone(),
                });
            }
            NetworkEvent::Packet {
                connection_id,
                payload,
                ..
            } if Some(*connection_id) == session.connection_id => {
                let packets = match session.codec.push_bytes(payload) {
                    Ok(packets) => packets,
                    Err(error) => {
                        write_display_error(
                            &mut events,
                            MyServerDisplayError::protocol(None, None, Some(error.clone())),
                        );
                        events.write(MyServerEvent::ProtocolError { error });
                        continue;
                    }
                };

                for packet in packets {
                    handle_game_packet(
                        &config,
                        &mut session,
                        &mut network_commands,
                        &mut events,
                        packet,
                    );
                }
            }
            NetworkEvent::SendFailed {
                connection_id,
                error,
                ..
            } if Some(*connection_id) == session.connection_id => {
                write_display_error(
                    &mut events,
                    MyServerDisplayError::transport(
                        MyServerOperation::GameRequest,
                        Some(error.clone()),
                    ),
                );
                events.write(MyServerEvent::RequestFailed {
                    seq: None,
                    message_type: None,
                    error: error.clone(),
                });
                trace_game_transition(
                    "game_send_failed",
                    &session,
                    Some(*connection_id),
                    None,
                    None,
                    None,
                    None,
                );
            }
            NetworkEvent::Disconnected {
                connection_id,
                reason,
                ..
            } if Some(*connection_id) == session.connection_id => {
                session.disconnect_cleanup();
                trace_game_transition(
                    "game_disconnected",
                    &session,
                    Some(*connection_id),
                    None,
                    None,
                    None,
                    reason.as_deref(),
                );
                events.write(MyServerEvent::Disconnected {
                    reason: reason.clone(),
                });
            }
            _ => {}
        }
    }
}

fn keepalive_myserver_connection(
    config: Res<MyServerConfig>,
    time: Res<Time>,
    mut state: ResMut<MyServerKeepaliveState>,
    mut session: ResMut<MyServerSession>,
    mut network_commands: MessageWriter<NetworkCommand>,
    mut events: MessageWriter<MyServerEvent>,
) {
    if state.interval != config.keepalive_interval {
        state.interval = config.keepalive_interval;
        state.timer = Timer::new(config.keepalive_interval, TimerMode::Repeating);
    }

    if !session.connected || !session.authenticated {
        state.timer.reset();
        return;
    }

    state.timer.tick(time.delta());
    if !state.timer.just_finished() {
        return;
    }

    let ticket_refresh_operation = PendingHttpOperation::TicketIssue {
        reconnect_game: false,
    };
    if session.needs_ticket_refresh(SystemTime::now(), config.ticket_refresh_margin)
        && !has_duplicate_pending_http(&session, &ticket_refresh_operation)
    {
        send_refresh_ticket(
            &config,
            &mut session,
            &mut network_commands,
            &mut events,
            false,
        );
        return;
    }

    if !config.keepalive_enabled {
        return;
    }

    send_request(
        &mut session,
        &mut network_commands,
        &mut events,
        MessageType::PingReq,
        MessageType::PingRes,
        &pb::PingReq {
            client_time: current_unix_ms(),
        },
    );
}

fn send_login(
    config: &MyServerConfig,
    session: &mut MyServerSession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<MyServerEvent>,
    login_name: &str,
    password: &str,
    connect_game: bool,
) {
    let operation = PendingHttpOperation::Login { connect_game };
    let body = json!({
        "loginName": login_name,
        "password": password,
    });
    let request = build_json_request(
        config,
        HttpMethod::Post,
        "/api/v1/auth/login",
        Some(body),
        None,
    );
    send_http_request(
        config,
        session,
        network_commands,
        events,
        operation,
        request,
    );
}

fn send_register(
    config: &MyServerConfig,
    session: &mut MyServerSession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<MyServerEvent>,
    login_name: &str,
    password: &str,
    connect_game: bool,
) {
    let operation = PendingHttpOperation::Register { connect_game };
    let body = json!({
        "loginName": login_name,
        "password": password,
    });
    let request = build_json_request(
        config,
        HttpMethod::Post,
        "/api/v1/auth/register",
        Some(body),
        None,
    );
    send_http_request(
        config,
        session,
        network_commands,
        events,
        operation,
        request,
    );
}

fn send_guest_login(
    config: &MyServerConfig,
    session: &mut MyServerSession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<MyServerEvent>,
    guest_id: Option<&str>,
    connect_game: bool,
) {
    let operation = PendingHttpOperation::GuestLogin { connect_game };
    let body = match guest_id {
        Some(guest_id) if !guest_id.trim().is_empty() => json!({ "guestId": guest_id }),
        _ => json!({}),
    };
    let request = build_json_request(
        config,
        HttpMethod::Post,
        "/api/v1/auth/guest-login",
        Some(body),
        None,
    );
    send_http_request(
        config,
        session,
        network_commands,
        events,
        operation,
        request,
    );
}

fn send_character_list(
    config: &MyServerConfig,
    session: &mut MyServerSession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<MyServerEvent>,
) {
    let Some(access_token) = session.access_token.as_deref() else {
        session.http_operation_failed(&PendingHttpOperation::CharacterList);
        write_http_failure(
            events,
            &PendingHttpOperation::CharacterList,
            "cannot load characters before login".to_string(),
        );
        return;
    };
    let request = build_json_request(
        config,
        HttpMethod::Get,
        "/api/v1/characters",
        None,
        Some(access_token),
    );
    send_http_request(
        config,
        session,
        network_commands,
        events,
        PendingHttpOperation::CharacterList,
        request,
    );
}

fn send_character_create(
    config: &MyServerConfig,
    session: &mut MyServerSession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<MyServerEvent>,
    name: &str,
    appearance_json: Option<Value>,
) {
    let Some(access_token) = session.access_token.as_deref() else {
        session.http_operation_failed(&PendingHttpOperation::CharacterCreate);
        write_http_failure(
            events,
            &PendingHttpOperation::CharacterCreate,
            "cannot create character before login".to_string(),
        );
        return;
    };
    let mut body = serde_json::Map::new();
    body.insert("name".to_string(), Value::String(name.to_string()));
    if let Some(appearance_json) = appearance_json {
        body.insert("appearance_json".to_string(), appearance_json);
    }
    let request = build_json_request(
        config,
        HttpMethod::Post,
        "/api/v1/characters",
        Some(Value::Object(body)),
        Some(access_token),
    );
    send_http_request(
        config,
        session,
        network_commands,
        events,
        PendingHttpOperation::CharacterCreate,
        request,
    );
}

fn send_character_profile(
    config: &MyServerConfig,
    session: &mut MyServerSession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<MyServerEvent>,
    character_id: &str,
) {
    let Some(access_token) = session.access_token.as_deref() else {
        session.http_operation_failed(&PendingHttpOperation::CharacterProfile {
            character_id: character_id.to_string(),
        });
        write_http_failure(
            events,
            &PendingHttpOperation::CharacterProfile {
                character_id: character_id.to_string(),
            },
            "cannot load character profile before login".to_string(),
        );
        return;
    };
    let path = format!(
        "/api/v1/characters/{}/profile",
        url_path_segment(character_id)
    );
    let request = build_json_request(config, HttpMethod::Get, &path, None, Some(access_token));
    send_http_request(
        config,
        session,
        network_commands,
        events,
        PendingHttpOperation::CharacterProfile {
            character_id: character_id.to_string(),
        },
        request,
    );
}

fn send_character_select(
    config: &MyServerConfig,
    session: &mut MyServerSession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<MyServerEvent>,
    character_id: &str,
    connect_game: bool,
) {
    let Some(access_token) = session.access_token.as_deref() else {
        session.http_operation_failed(&PendingHttpOperation::CharacterSelect {
            character_id: character_id.to_string(),
            connect_game,
        });
        write_http_failure(
            events,
            &PendingHttpOperation::CharacterSelect {
                character_id: character_id.to_string(),
                connect_game,
            },
            "cannot select character before login".to_string(),
        );
        return;
    };
    let request = build_json_request(
        config,
        HttpMethod::Post,
        "/api/v1/characters/select",
        Some(json!({ "character_id": character_id })),
        Some(access_token),
    );
    send_http_request(
        config,
        session,
        network_commands,
        events,
        PendingHttpOperation::CharacterSelect {
            character_id: character_id.to_string(),
            connect_game,
        },
        request,
    );
}

fn send_character_lifecycle(
    config: &MyServerConfig,
    session: &mut MyServerSession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<MyServerEvent>,
    operation: PendingHttpOperation,
) {
    let Some(access_token) = session.access_token.as_deref() else {
        session.http_operation_failed(&operation);
        write_http_failure(
            events,
            &operation,
            "cannot change character lifecycle before login".to_string(),
        );
        return;
    };
    let (path, character_id) = match &operation {
        PendingHttpOperation::CharacterDelete { character_id } => {
            ("/api/v1/characters/delete", character_id.as_str())
        }
        PendingHttpOperation::CharacterRestore { character_id } => {
            ("/api/v1/characters/restore", character_id.as_str())
        }
        _ => return,
    };
    let request = build_json_request(
        config,
        HttpMethod::Post,
        path,
        Some(json!({ "character_id": character_id })),
        Some(access_token),
    );
    send_http_request(
        config,
        session,
        network_commands,
        events,
        operation,
        request,
    );
}

fn send_refresh_ticket(
    config: &MyServerConfig,
    session: &mut MyServerSession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<MyServerEvent>,
    reconnect_game: bool,
) {
    if reconnect_game && session.reconnect_blocked {
        let error = "reconnect is blocked by session kick".to_string();
        session.ticket_issue_failed(true);
        session.clear_reconnect_plan();
        write_display_error(
            events,
            MyServerDisplayError::from_error_code(
                MyServerErrorSource::Client,
                Some(MyServerOperation::TicketRefresh),
                None,
                None,
                None,
                "SESSION_KICKED",
                Some(error.clone()),
            ),
        );
        events.write(MyServerEvent::TicketRefreshFailed {
            error: error.clone(),
        });
        events.write(MyServerEvent::NetworkFailed {
            operation: MyServerOperation::TicketRefresh,
            error,
        });
        return;
    }

    let Some(access_token) = session.access_token.clone() else {
        session.http_operation_failed(&PendingHttpOperation::TicketIssue { reconnect_game });
        if reconnect_game {
            session.clear_reconnect_plan();
        }
        write_display_error(
            events,
            MyServerDisplayError::from_error_code(
                MyServerErrorSource::Client,
                Some(MyServerOperation::TicketRefresh),
                None,
                None,
                None,
                "UNAUTHORIZED",
                Some("cannot issue ticket before login".to_string()),
            ),
        );
        events.write(MyServerEvent::TicketRefreshFailed {
            error: "cannot issue ticket before login".to_string(),
        });
        events.write(MyServerEvent::NetworkFailed {
            operation: MyServerOperation::TicketRefresh,
            error: "cannot issue ticket before login".to_string(),
        });
        return;
    };
    let Some(character_id) = session.character_id.clone() else {
        session.http_operation_failed(&PendingHttpOperation::TicketIssue { reconnect_game });
        if reconnect_game {
            session.clear_reconnect_plan();
        }
        write_display_error(
            events,
            MyServerDisplayError::from_error_code(
                MyServerErrorSource::Client,
                Some(MyServerOperation::TicketRefresh),
                None,
                None,
                None,
                "MISSING_CHARACTER_ID",
                Some("cannot issue ticket before selecting a character".to_string()),
            ),
        );
        events.write(MyServerEvent::TicketRefreshFailed {
            error: "cannot issue ticket before selecting a character".to_string(),
        });
        events.write(MyServerEvent::NetworkFailed {
            operation: MyServerOperation::TicketRefresh,
            error: "cannot issue ticket before selecting a character".to_string(),
        });
        return;
    };

    let request = build_json_request(
        config,
        HttpMethod::Post,
        "/api/v1/game-ticket/issue",
        Some(json!({ "character_id": character_id })),
        Some(&access_token),
    );
    send_http_request(
        config,
        session,
        network_commands,
        events,
        PendingHttpOperation::TicketIssue { reconnect_game },
        request,
    );
}

fn send_logout(
    config: &MyServerConfig,
    session: &mut MyServerSession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<MyServerEvent>,
) {
    disconnect(session, network_commands);
    let access_token = session.access_token.clone();
    if let Some(access_token) = access_token.as_deref() {
        let request = build_json_request(
            config,
            HttpMethod::Post,
            "/api/v1/auth/logout",
            Some(json!({})),
            Some(access_token),
        );
        send_http_request(
            config,
            session,
            network_commands,
            events,
            PendingHttpOperation::Logout,
            request,
        );
    } else {
        session.logout();
        events.write(MyServerEvent::LogoutSucceeded);
    }
}

fn send_http_request(
    config: &MyServerConfig,
    session: &mut MyServerSession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<MyServerEvent>,
    operation: PendingHttpOperation,
    request: HttpRequest,
) {
    if has_duplicate_pending_http(session, &operation) {
        let error = format!(
            "{:?} request is already pending; refusing duplicate request",
            operation.duplicate_group()
        );
        write_http_failure(events, &operation, error.clone());
        events.write(MyServerEvent::NetworkFailed {
            operation: operation.event_operation(),
            error,
        });
        return;
    }

    let request_id = request.request_id;
    match &operation {
        PendingHttpOperation::Login { connect_game }
        | PendingHttpOperation::Register { connect_game }
        | PendingHttpOperation::GuestLogin { connect_game } => {
            session.login_request = Some(request_id);
            session.connect_after_login = (*connect_game).then_some(ConnectPlan {
                transport: config.prefer_transport,
                host: None,
                port: None,
            });
        }
        PendingHttpOperation::CharacterSelect { connect_game, .. } => {
            session.connect_after_login = (*connect_game).then_some(ConnectPlan {
                transport: config.prefer_transport,
                host: None,
                port: None,
            });
        }
        PendingHttpOperation::TicketIssue { reconnect_game } => {
            session.ticket_request = Some(request_id);
            if *reconnect_game && session.connect_after_login.is_none() {
                session.connect_after_login = Some(ConnectPlan {
                    transport: config.prefer_transport,
                    host: None,
                    port: None,
                });
            } else if !*reconnect_game {
                session.connect_after_login = None;
            }
        }
        _ => {}
    }
    session.pending_http.insert(
        request_id,
        PendingHttpRequest {
            operation: operation.clone(),
        },
    );
    session.begin_http_operation(&operation);
    trace_http_transition(
        "http_request_sent",
        request_id,
        &operation,
        None,
        None,
        session,
    );
    network_commands.write(NetworkCommand::Http(request));
}

fn has_duplicate_pending_http(session: &MyServerSession, operation: &PendingHttpOperation) -> bool {
    let group = operation.duplicate_group();
    session
        .pending_http
        .values()
        .any(|pending| pending.operation.duplicate_group() == group)
}

fn clear_legacy_http_slots(session: &mut MyServerSession, request_id: RequestId) {
    if session.login_request == Some(request_id) {
        session.login_request = None;
    }
    if session.ticket_request == Some(request_id) {
        session.ticket_request = None;
    }
}

fn build_json_request(
    config: &MyServerConfig,
    method: HttpMethod,
    path: &str,
    body: Option<Value>,
    access_token: Option<&str>,
) -> HttpRequest {
    let url = format!(
        "{}/{}",
        config.http_base_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    );
    let mut request = HttpRequest::new(method, url)
        .with_header("Accept", "application/json")
        .with_timeout(config.request_timeout);
    if let Some(access_token) = access_token {
        request = request.with_header("Authorization", format!("Bearer {access_token}"));
    }
    if let Some(body) = body {
        request = request
            .with_header("Content-Type", "application/json")
            .with_body(serde_json::to_vec(&body).unwrap_or_else(|_| b"{}".to_vec()));
    }
    request
}

fn handle_http_response(
    config: &MyServerConfig,
    session: &mut MyServerSession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<MyServerEvent>,
    request_id: RequestId,
    operation: PendingHttpOperation,
    status: u16,
    body: &[u8],
) {
    let response_error_code = parse_api_error_code(body);
    trace_http_transition(
        "http_response_received",
        request_id,
        &operation,
        Some(status),
        response_error_code.as_deref(),
        session,
    );
    let diagnostic_operation = operation.clone();
    match operation {
        PendingHttpOperation::Login { .. } | PendingHttpOperation::GuestLogin { .. } => {
            handle_login_response(
                config,
                session,
                network_commands,
                events,
                operation,
                status,
                body,
            )
        }
        PendingHttpOperation::Register { .. } => handle_register_response(
            config,
            session,
            network_commands,
            events,
            operation,
            status,
            body,
        ),
        PendingHttpOperation::CharacterList => {
            handle_character_list_response(session, events, operation, status, body)
        }
        PendingHttpOperation::CharacterCreate => {
            handle_character_create_response(session, events, operation, status, body)
        }
        PendingHttpOperation::CharacterProfile { .. } => {
            handle_character_profile_response(session, events, operation, status, body)
        }
        PendingHttpOperation::CharacterSelect { .. } => handle_character_select_response(
            config,
            session,
            network_commands,
            events,
            operation,
            status,
            body,
        ),
        PendingHttpOperation::CharacterDelete { .. }
        | PendingHttpOperation::CharacterRestore { .. } => {
            handle_character_lifecycle_response(session, events, operation, status, body)
        }
        PendingHttpOperation::TicketIssue { .. } => handle_ticket_response(
            config,
            session,
            network_commands,
            events,
            operation,
            status,
            body,
        ),
        PendingHttpOperation::Logout => {
            handle_logout_response(session, events, operation, status, body)
        }
    }
    trace_http_transition(
        "http_response_applied",
        request_id,
        &diagnostic_operation,
        Some(status),
        response_error_code.as_deref(),
        session,
    );
}

fn handle_character_list_response(
    session: &mut MyServerSession,
    events: &mut MessageWriter<MyServerEvent>,
    operation: PendingHttpOperation,
    status: u16,
    body: &[u8],
) {
    let Some(response) =
        parse_http_json::<CharacterListResponse>(session, events, &operation, status, body)
    else {
        return;
    };
    if !response.ok {
        let error = "character list returned ok=false".to_string();
        apply_http_failure_state(session, &operation, &error);
        write_http_failure(events, &operation, error);
        return;
    }
    let needs_character = session.apply_character_list_response(&response);
    events.write(MyServerEvent::CharacterListLoaded {
        player_id: response.player_id.clone(),
        characters: response.characters.clone(),
    });
    if needs_character {
        events.write(MyServerEvent::CharacterCreationRequired {
            player_id: response.player_id,
        });
    }
}

fn handle_character_create_response(
    session: &mut MyServerSession,
    events: &mut MessageWriter<MyServerEvent>,
    operation: PendingHttpOperation,
    status: u16,
    body: &[u8],
) {
    let Some(response) =
        parse_http_json::<CharacterCreateResponse>(session, events, &operation, status, body)
    else {
        return;
    };
    if !response.ok {
        let error = "character create returned ok=false".to_string();
        apply_http_failure_state(session, &operation, &error);
        write_http_failure(events, &operation, error);
        return;
    }
    session.apply_character_create_response(&response);
    events.write(MyServerEvent::CharacterCreated {
        character: response.character,
    });
}

fn handle_character_profile_response(
    session: &mut MyServerSession,
    events: &mut MessageWriter<MyServerEvent>,
    operation: PendingHttpOperation,
    status: u16,
    body: &[u8],
) {
    let Some(response) =
        parse_http_json::<CharacterProfileResponse>(session, events, &operation, status, body)
    else {
        return;
    };
    if !response.ok {
        let error = "character profile returned ok=false".to_string();
        apply_http_failure_state(session, &operation, &error);
        write_http_failure(events, &operation, error);
        return;
    }
    session.apply_character_profile_response(&response);
    events.write(MyServerEvent::CharacterProfileLoaded {
        profile: response.profile,
    });
}

fn handle_character_select_response(
    config: &MyServerConfig,
    session: &mut MyServerSession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<MyServerEvent>,
    operation: PendingHttpOperation,
    status: u16,
    body: &[u8],
) {
    let Some(response) =
        parse_http_json::<CharacterSelectResponse>(session, events, &operation, status, body)
    else {
        return;
    };
    if !response.ok {
        let error = "character select returned ok=false".to_string();
        apply_http_failure_state(session, &operation, &error);
        write_http_failure(events, &operation, error);
        return;
    }
    let (host, port, transport) = character_select_endpoint(&response);
    if session.connection_id.is_some() {
        disconnect(session, network_commands);
    }
    session.apply_character_select_response(&response);
    events.write(MyServerEvent::CharacterSelected {
        player_id: response.player_id.clone(),
        character_id: response.character.character_id.clone(),
        world_id: response.character.world_id,
    });

    if let Some(mut plan) = session.connect_after_login.take() {
        apply_discovered_endpoint(&mut plan, host, port, transport, config);
        connect_with_ticket(
            config,
            session,
            network_commands,
            events,
            response.ticket,
            plan,
        );
    }
}

fn handle_character_lifecycle_response(
    session: &mut MyServerSession,
    events: &mut MessageWriter<MyServerEvent>,
    operation: PendingHttpOperation,
    status: u16,
    body: &[u8],
) {
    let Some(response) =
        parse_http_json::<CharacterLifecycleResponse>(session, events, &operation, status, body)
    else {
        return;
    };
    if !response.ok {
        let error = "character lifecycle returned ok=false".to_string();
        apply_http_failure_state(session, &operation, &error);
        write_http_failure(events, &operation, error);
        return;
    }
    let character_id = response.character.character_id.clone();
    let restored_character = response.character.clone();
    session.apply_character_lifecycle_response(&response);
    match operation {
        PendingHttpOperation::CharacterDelete { .. } => {
            events.write(MyServerEvent::CharacterDeleted { character_id });
        }
        PendingHttpOperation::CharacterRestore { .. } => {
            events.write(MyServerEvent::CharacterRestored {
                character: restored_character,
            });
        }
        _ => {}
    }
}

fn handle_logout_response(
    session: &mut MyServerSession,
    events: &mut MessageWriter<MyServerEvent>,
    operation: PendingHttpOperation,
    status: u16,
    body: &[u8],
) {
    if !(200..300).contains(&status) {
        let error = http_error_message(&operation, status, body);
        write_http_failure(events, &operation, error);
        return;
    }
    session.logout();
    events.write(MyServerEvent::LogoutSucceeded);
}

fn parse_http_json<T>(
    session: &mut MyServerSession,
    events: &mut MessageWriter<MyServerEvent>,
    operation: &PendingHttpOperation,
    status: u16,
    body: &[u8],
) -> Option<T>
where
    T: DeserializeOwned,
{
    if !(200..300).contains(&status) {
        let error = http_error_message(operation, status, body);
        apply_http_failure_state(session, operation, &error);
        write_http_failure_with_context(
            events,
            operation,
            error.clone(),
            Some(status),
            parse_api_error_code(body),
        );
        events.write(MyServerEvent::NetworkFailed {
            operation: operation.event_operation(),
            error,
        });
        return None;
    }

    if let Some(api_error) = parse_api_error_detail(body) {
        if api_error.ok == Some(false) {
            let error = api_error
                .display
                .clone()
                .unwrap_or_else(|| format!("{:?} returned ok=false", operation.event_operation()));
            apply_http_failure_state(session, operation, &error);
            write_http_failure_with_context(events, operation, error.clone(), None, api_error.code);
            events.write(MyServerEvent::NetworkFailed {
                operation: operation.event_operation(),
                error,
            });
            return None;
        }
    }

    match serde_json::from_slice::<T>(body) {
        Ok(response) => Some(response),
        Err(error) => {
            let error = format!("failed to parse HTTP response JSON: {error}");
            apply_http_failure_state(session, operation, &error);
            write_display_error(
                events,
                MyServerDisplayError::json_parse(operation.event_operation(), Some(error.clone())),
            );
            write_legacy_http_failure(events, operation, error.clone());
            events.write(MyServerEvent::NetworkFailed {
                operation: operation.event_operation(),
                error,
            });
            None
        }
    }
}

fn parse_http_json_with<T, F>(
    session: &mut MyServerSession,
    events: &mut MessageWriter<MyServerEvent>,
    operation: &PendingHttpOperation,
    status: u16,
    body: &[u8],
    parser: F,
) -> Option<T>
where
    F: FnOnce(&[u8]) -> Result<T, serde_json::Error>,
{
    if !(200..300).contains(&status) {
        let error = http_error_message(operation, status, body);
        apply_http_failure_state(session, operation, &error);
        write_http_failure_with_context(
            events,
            operation,
            error.clone(),
            Some(status),
            parse_api_error_code(body),
        );
        events.write(MyServerEvent::NetworkFailed {
            operation: operation.event_operation(),
            error,
        });
        return None;
    }

    if let Some(api_error) = parse_api_error_detail(body) {
        if api_error.ok == Some(false) {
            let error = api_error
                .display
                .clone()
                .unwrap_or_else(|| format!("{:?} returned ok=false", operation.event_operation()));
            apply_http_failure_state(session, operation, &error);
            write_http_failure_with_context(events, operation, error.clone(), None, api_error.code);
            events.write(MyServerEvent::NetworkFailed {
                operation: operation.event_operation(),
                error,
            });
            return None;
        }
    }

    match parser(body) {
        Ok(response) => Some(response),
        Err(error) => {
            let error = format!("failed to parse HTTP response JSON: {error}");
            apply_http_failure_state(session, operation, &error);
            write_display_error(
                events,
                MyServerDisplayError::json_parse(operation.event_operation(), Some(error.clone())),
            );
            write_legacy_http_failure(events, operation, error.clone());
            events.write(MyServerEvent::NetworkFailed {
                operation: operation.event_operation(),
                error,
            });
            None
        }
    }
}

fn http_error_message(operation: &PendingHttpOperation, status: u16, body: &[u8]) -> String {
    if let Some(error) = parse_api_error(body) {
        return format!(
            "{:?} returned HTTP {status}: {error}",
            operation.event_operation()
        );
    }
    let body = body_text(body);
    if body.trim().is_empty() {
        format!("{:?} returned HTTP {status}", operation.event_operation())
    } else {
        format!(
            "{:?} returned HTTP {status}: {}",
            operation.event_operation(),
            body
        )
    }
}

fn parse_api_error(body: &[u8]) -> Option<String> {
    parse_api_error_detail(body).and_then(|detail| detail.display)
}

#[derive(Clone, Debug)]
struct ParsedApiError {
    ok: Option<bool>,
    code: Option<String>,
    display: Option<String>,
}

fn parse_api_error_detail(body: &[u8]) -> Option<ParsedApiError> {
    let response = serde_json::from_slice::<ApiErrorResponse>(body).ok()?;
    let ok = response.ok;
    let code = api_error_code(&response);
    let message = api_error_message(response);
    let display = match (code.clone(), message) {
        (Some(code), Some(message)) if !message.trim().is_empty() => {
            Some(format!("{code}: {message}"))
        }
        (Some(code), _) => Some(code),
        (None, Some(message)) if !message.trim().is_empty() => Some(message),
        _ => None,
    };
    Some(ParsedApiError { ok, code, display })
}

fn api_error_message(response: ApiErrorResponse) -> Option<String> {
    response.message.or_else(|| {
        response
            .extra
            .get("message")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
    })
}

fn parse_api_error_code(body: &[u8]) -> Option<String> {
    serde_json::from_slice::<ApiErrorResponse>(body)
        .ok()
        .and_then(|response| api_error_code(&response))
}

fn api_error_code(response: &ApiErrorResponse) -> Option<String> {
    response
        .error
        .clone()
        .or_else(|| {
            response
                .extra
                .get("errorCode")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
        .or_else(|| {
            response
                .extra
                .get("code")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
}

fn write_http_failure(
    events: &mut MessageWriter<MyServerEvent>,
    operation: &PendingHttpOperation,
    error: String,
) {
    write_http_failure_with_context(events, operation, error, None, None);
}

fn write_http_failure_with_context(
    events: &mut MessageWriter<MyServerEvent>,
    operation: &PendingHttpOperation,
    error: String,
    status: Option<u16>,
    error_code: Option<String>,
) {
    events.write(MyServerEvent::DisplayError {
        error: display_error_from_http_operation(
            operation,
            status,
            error_code,
            Some(error.clone()),
        ),
    });
    write_legacy_http_failure(events, operation, error);
}

fn write_legacy_http_failure(
    events: &mut MessageWriter<MyServerEvent>,
    operation: &PendingHttpOperation,
    error: String,
) {
    match operation {
        PendingHttpOperation::Login { .. }
        | PendingHttpOperation::Register { .. }
        | PendingHttpOperation::GuestLogin { .. } => {
            events.write(MyServerEvent::LoginFailed { error });
        }
        PendingHttpOperation::CharacterList => {
            events.write(MyServerEvent::CharacterListFailed { error });
        }
        PendingHttpOperation::CharacterCreate => {
            events.write(MyServerEvent::CharacterCreateFailed { error });
        }
        PendingHttpOperation::CharacterProfile { .. } => {
            events.write(MyServerEvent::CharacterProfileFailed { error });
        }
        PendingHttpOperation::CharacterSelect { .. } => {
            events.write(MyServerEvent::CharacterSelectFailed { error });
        }
        PendingHttpOperation::CharacterDelete { .. } => {
            events.write(MyServerEvent::CharacterDeleteFailed { error });
        }
        PendingHttpOperation::CharacterRestore { .. } => {
            events.write(MyServerEvent::CharacterRestoreFailed { error });
        }
        PendingHttpOperation::TicketIssue { .. } => {
            events.write(MyServerEvent::TicketRefreshFailed { error });
        }
        PendingHttpOperation::Logout => {
            events.write(MyServerEvent::LogoutFailed { error });
        }
    }
}

fn write_display_error(events: &mut MessageWriter<MyServerEvent>, error: MyServerDisplayError) {
    events.write(MyServerEvent::DisplayError { error });
}

fn display_error_from_http_operation(
    operation: &PendingHttpOperation,
    status: Option<u16>,
    error_code: Option<String>,
    detail: Option<String>,
) -> MyServerDisplayError {
    let operation_kind = operation.event_operation();
    if let Some(status) = status {
        return MyServerDisplayError::http_status(operation_kind, status, error_code, detail);
    }
    if let Some(error_code) = error_code {
        return MyServerDisplayError::from_error_code(
            MyServerErrorSource::Http,
            Some(operation_kind),
            None,
            None,
            None,
            error_code,
            detail,
        );
    }
    MyServerDisplayError::from_error_code(
        MyServerErrorSource::Client,
        Some(operation_kind),
        None,
        None,
        None,
        detail.clone().unwrap_or_default(),
        detail,
    )
}

fn display_error_from_game_code(
    message_type: Option<MessageType>,
    seq: Option<u32>,
    error_code: &str,
    detail: Option<String>,
) -> MyServerDisplayError {
    MyServerDisplayError::from_error_code(
        MyServerErrorSource::Game,
        Some(MyServerOperation::GameRequest),
        message_type,
        seq,
        None,
        error_code,
        detail,
    )
}

fn classify_session_kick(reason: &str) -> SessionKickCategory {
    let code = reason.trim().to_ascii_uppercase();
    if code.contains("CONCURRENT")
        || code.contains("LOGIN_ELSEWHERE")
        || code.contains("OTHER_DEVICE")
        || code.contains("DUPLICATE_LOGIN")
    {
        SessionKickCategory::ConcurrentLogin
    } else if code.contains("BAN")
        || code.contains("BLOCK")
        || code.contains("SUSPEND")
        || code.contains("FORBIDDEN")
    {
        SessionKickCategory::Banned
    } else if code.contains("MAINTENANCE") {
        SessionKickCategory::Maintenance
    } else if code.contains("SERVER")
        || code.contains("OFFLINE")
        || code.contains("SHUTDOWN")
        || code.contains("ADMIN")
    {
        SessionKickCategory::ServerOffline
    } else {
        SessionKickCategory::Unknown
    }
}

fn display_error_from_kick(
    category: SessionKickCategory,
    reason: &str,
    seq: Option<u32>,
) -> MyServerDisplayError {
    let error_code = match category {
        SessionKickCategory::ConcurrentLogin => "SESSION_KICK_CONCURRENT_LOGIN",
        SessionKickCategory::Banned => "SESSION_KICK_BANNED",
        SessionKickCategory::Maintenance => "MAINTENANCE",
        SessionKickCategory::ServerOffline => "SESSION_KICK_SERVER_OFFLINE",
        SessionKickCategory::Unknown => "SESSION_KICK_UNKNOWN",
    };
    MyServerDisplayError::from_error_code(
        MyServerErrorSource::Game,
        Some(MyServerOperation::GameRequest),
        Some(MessageType::SessionKickPush),
        seq,
        None,
        error_code,
        Some(reason.to_string()),
    )
}

fn apply_http_failure_state(
    session: &mut MyServerSession,
    operation: &PendingHttpOperation,
    error: &str,
) {
    match classify_failure_state(error) {
        FailureState::AccountBlocked => session.account_blocked(),
        FailureState::AccountExpired
            if !matches!(
                operation,
                PendingHttpOperation::Login { .. }
                    | PendingHttpOperation::Register { .. }
                    | PendingHttpOperation::GuestLogin { .. }
            ) =>
        {
            session.account_expired()
        }
        FailureState::AccountExpired => session.http_operation_failed(operation),
        FailureState::CharacterBlocked => session.character_blocked(),
        FailureState::Default => session.http_operation_failed(operation),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FailureState {
    Default,
    AccountBlocked,
    AccountExpired,
    CharacterBlocked,
}

fn classify_failure_state(error: &str) -> FailureState {
    let upper = error.to_ascii_uppercase();
    if upper.contains("EXPIRED")
        || upper.contains("TOKEN_INVALID")
        || upper.contains("UNAUTHORIZED")
        || upper.contains("HTTP 401")
    {
        return FailureState::AccountExpired;
    }
    if upper.contains("CHARACTER_")
        && (upper.contains("BLOCKED") || upper.contains("BANNED") || upper.contains("DELETED"))
    {
        return FailureState::CharacterBlocked;
    }
    if upper.contains("BLOCKED")
        || upper.contains("BANNED")
        || upper.contains("PENDING_REVIEW")
        || upper.contains("SUSPENDED")
        || upper.contains("MAINTENANCE")
        || upper.contains("VERSION_INCOMPATIBLE")
    {
        return FailureState::AccountBlocked;
    }
    FailureState::Default
}

fn url_path_segment(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}

fn handle_register_response(
    config: &MyServerConfig,
    session: &mut MyServerSession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<MyServerEvent>,
    operation: PendingHttpOperation,
    status: u16,
    body: &[u8],
) {
    let Some(response) = parse_http_json_with(
        session,
        events,
        &operation,
        status,
        body,
        parse_register_response,
    ) else {
        return;
    };

    match response {
        RegisterResponse::Login(response) => {
            handle_login_success(config, session, network_commands, events, response);
        }
        RegisterResponse::PendingReview(response) => {
            session.account_blocked();
            session.connect_after_login = None;
            let code = register_pending_review_code(&response);
            let message = response
                .message
                .filter(|message| !message.trim().is_empty())
                .unwrap_or_else(|| "Registration submitted for review".to_string());
            write_display_error(
                events,
                display_error_from_http_operation(
                    &operation,
                    None,
                    Some(code.clone()),
                    Some(message.clone()),
                ),
            );
            events.write(MyServerEvent::AccountStatusBlocked { code, message });
        }
    }
}

fn parse_register_response(body: &[u8]) -> Result<RegisterResponse, serde_json::Error> {
    let value: Value = serde_json::from_slice(body)?;
    if value
        .get("pendingReview")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        let response = serde_json::from_value::<RegisterPendingReviewResponse>(value)?;
        Ok(RegisterResponse::PendingReview(response))
    } else {
        let response = serde_json::from_value::<LoginResponse>(value)?;
        Ok(RegisterResponse::Login(response))
    }
}

fn register_pending_review_code(response: &RegisterPendingReviewResponse) -> String {
    response
        .status
        .as_deref()
        .map(str::trim)
        .filter(|status| !status.is_empty())
        .map(|status| format!("REGISTER_{}", stable_error_code(status)))
        .unwrap_or_else(|| "REGISTER_PENDING_REVIEW".to_string())
}

fn stable_error_code(value: &str) -> String {
    let mut code = String::new();
    let mut last_was_underscore = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            code.push(ch.to_ascii_uppercase());
            last_was_underscore = false;
        } else if !last_was_underscore && !code.is_empty() {
            code.push('_');
            last_was_underscore = true;
        }
    }
    while code.ends_with('_') {
        code.pop();
    }
    if code.is_empty() {
        "PENDING_REVIEW".to_string()
    } else {
        code
    }
}

fn handle_login_response(
    config: &MyServerConfig,
    session: &mut MyServerSession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<MyServerEvent>,
    operation: PendingHttpOperation,
    status: u16,
    body: &[u8],
) {
    let Some(response) =
        parse_http_json::<LoginResponse>(session, events, &operation, status, body)
    else {
        return;
    };

    if !response.ok {
        let error = "login returned ok=false".to_string();
        apply_http_failure_state(session, &operation, &error);
        write_http_failure(events, &operation, error);
        return;
    }

    handle_login_success(config, session, network_commands, events, response);
}

fn handle_login_success(
    config: &MyServerConfig,
    session: &mut MyServerSession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<MyServerEvent>,
    response: LoginResponse,
) {
    let login_session = session.apply_login_response(&response);
    info!(
        player_id = %login_session.player_id,
        access_token_fp = %redact_secret_fingerprint(&login_session.access_token),
        ticket_fp = login_session.ticket.as_deref().map(redact_secret_fingerprint).unwrap_or_default(),
        ticket_expires_at = login_session.ticket_expires_at.as_deref().unwrap_or_default(),
        endpoint_host = login_session.game_host.as_deref().unwrap_or_default(),
        endpoint_port = login_session.game_port,
        endpoint_transport = ?login_session.game_transport,
        "MyServer login session accepted"
    );

    events.write(MyServerEvent::LoginSucceeded(login_session.clone()));

    let Some(ticket) = login_session.ticket else {
        return;
    };

    if let Some(mut plan) = session.connect_after_login.take() {
        apply_discovered_endpoint(
            &mut plan,
            login_session.game_host,
            login_session.game_port,
            login_session.game_transport,
            config,
        );
        connect_with_ticket(config, session, network_commands, events, ticket, plan);
    }
}

fn handle_ticket_response(
    config: &MyServerConfig,
    session: &mut MyServerSession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<MyServerEvent>,
    operation: PendingHttpOperation,
    status: u16,
    body: &[u8],
) {
    let PendingHttpOperation::TicketIssue { reconnect_game } = operation else {
        return;
    };
    let operation = PendingHttpOperation::TicketIssue { reconnect_game };

    let Some(response) =
        parse_http_json::<TicketResponse>(session, events, &operation, status, body)
    else {
        cleanup_reconnect_ticket_failure(session, reconnect_game);
        return;
    };

    if !response.ok {
        let error = "ticket issue returned ok=false".to_string();
        apply_http_failure_state(session, &operation, &error);
        cleanup_reconnect_ticket_failure(session, reconnect_game);
        write_http_failure(events, &operation, error);
        return;
    }

    let ticket_payload = match parse_character_bound_ticket(&response.ticket) {
        Ok(payload) => payload,
        Err(error) => {
            apply_http_failure_state(session, &operation, &error);
            cleanup_reconnect_ticket_failure(session, reconnect_game);
            write_http_failure(
                events,
                &operation,
                format!("ticket issue returned invalid character-bound ticket: {error}"),
            );
            return;
        }
    };
    if let Some(character_id) = session.character_id.as_deref() {
        if ticket_payload.character_id != character_id {
            let error = format!(
                "ticket issue returned ticket for {}, expected {}",
                ticket_payload.character_id, character_id
            );
            apply_http_failure_state(session, &operation, &error);
            cleanup_reconnect_ticket_failure(session, reconnect_game);
            write_http_failure(events, &operation, error);
            return;
        }
    }

    let (host, port, transport) = ticket_endpoint(&response);
    session.apply_ticket_response(&response);
    info!(
        player_id = session.player_id.as_deref().unwrap_or_default(),
        character_id = session.character_id.as_deref().unwrap_or_default(),
        world_id = session.world_id,
        ticket_fp = %ticket_payload.ticket_fingerprint,
        payload_fp = %ticket_payload.payload_fingerprint,
        ticket_expires_at = %response.ticket_expires_at,
        reconnect_game,
        endpoint_host = host.as_deref().unwrap_or_default(),
        endpoint_port = port,
        endpoint_transport = ?transport,
        "MyServer game ticket issued"
    );
    events.write(MyServerEvent::TicketRefreshed {
        ticket_expires_at: response.ticket_expires_at.clone(),
    });

    if let Some(mut plan) = session.connect_after_login.take() {
        apply_discovered_endpoint(&mut plan, host, port, transport, config);
        disconnect_transport_preserving_reconnect_plan(session, network_commands);
        connect_with_ticket(
            config,
            session,
            network_commands,
            events,
            response.ticket,
            plan,
        );
    }
}

fn cleanup_reconnect_ticket_failure(session: &mut MyServerSession, reconnect_game: bool) {
    if reconnect_game {
        session.clear_reconnect_plan();
    }
}

fn apply_discovered_endpoint(
    plan: &mut ConnectPlan,
    host: Option<String>,
    port: Option<u16>,
    transport: Option<NetworkTransport>,
    config: &MyServerConfig,
) {
    plan.host = plan.host.take().or(host);

    if let Some(forced_transport) = config.forced_transport {
        plan.transport = forced_transport;
        if plan.port.is_none() {
            plan.port = Some(match forced_transport {
                NetworkTransport::Tcp => config.tcp_fallback_port,
                NetworkTransport::Kcp => config.kcp_port,
            });
        }
        return;
    }

    plan.port = plan.port.or(port);
    plan.transport = transport.unwrap_or(plan.transport);
}

fn redirect_transport(value: &str) -> Option<NetworkTransport> {
    match value.trim().to_ascii_lowercase().as_str() {
        "tcp" => Some(NetworkTransport::Tcp),
        "kcp" => Some(NetworkTransport::Kcp),
        _ => None,
    }
}

fn non_empty(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

fn connect_with_ticket(
    config: &MyServerConfig,
    session: &mut MyServerSession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<MyServerEvent>,
    ticket: String,
    plan: ConnectPlan,
) {
    if session.reconnect_blocked {
        let error = "reconnect is blocked by session kick".to_string();
        fail_game_connection_attempt(session, network_commands);
        session.clear_reconnect_plan();
        write_display_error(
            events,
            MyServerDisplayError::from_error_code(
                MyServerErrorSource::Client,
                Some(MyServerOperation::GameConnect),
                Some(MessageType::AuthReq),
                None,
                None,
                "SESSION_KICKED",
                Some(error.clone()),
            ),
        );
        events.write(MyServerEvent::RequestFailed {
            seq: None,
            message_type: Some(MessageType::AuthReq),
            error,
        });
        return;
    }

    let ticket_payload = match parse_character_bound_ticket(&ticket) {
        Ok(payload) => payload,
        Err(error) => {
            fail_game_connection_attempt(session, network_commands);
            session.clear_reconnect_plan();
            warn!(
                error_code = %error,
                ticket_fp = %redact_secret_fingerprint(&ticket),
                "MyServer refused game connection with invalid ticket"
            );
            write_display_error(
                events,
                display_error_from_game_code(
                    Some(MessageType::AuthReq),
                    None,
                    &error,
                    Some(error.clone()),
                ),
            );
            events.write(MyServerEvent::RequestFailed {
                seq: None,
                message_type: Some(MessageType::AuthReq),
                error: format!(
                    "refusing game connection with invalid character-bound ticket: {error}"
                ),
            });
            events.write(MyServerEvent::AuthFailed {
                error_code: error.clone(),
            });
            events.write(MyServerEvent::GameAuthRejected {
                error_code: error.clone(),
                reason: classify_game_auth_failure(&error),
            });
            return;
        }
    };

    if let Some(current_character_id) = session.character_id.clone() {
        if current_character_id != ticket_payload.character_id {
            let error = format!(
                "TICKET_CHARACTER_MISMATCH: refusing ticket for {} while selected character is {}",
                ticket_payload.character_id, current_character_id
            );
            fail_game_connection_attempt(session, network_commands);
            session.clear_reconnect_plan();
            warn!(
                error_code = "TICKET_CHARACTER_MISMATCH",
                ticket_fp = %ticket_payload.ticket_fingerprint,
                ticket_character_id = %ticket_payload.character_id,
                selected_character_id = %current_character_id,
                "MyServer refused game connection with mismatched ticket"
            );
            write_display_error(
                events,
                display_error_from_game_code(
                    Some(MessageType::AuthReq),
                    None,
                    "TICKET_CHARACTER_MISMATCH",
                    Some("TICKET_CHARACTER_MISMATCH".to_string()),
                ),
            );
            events.write(MyServerEvent::RequestFailed {
                seq: None,
                message_type: Some(MessageType::AuthReq),
                error: error.clone(),
            });
            events.write(MyServerEvent::AuthFailed {
                error_code: "TICKET_CHARACTER_MISMATCH".to_string(),
            });
            events.write(MyServerEvent::GameAuthRejected {
                error_code: "TICKET_CHARACTER_MISMATCH".to_string(),
                reason: classify_game_auth_failure("TICKET_CHARACTER_MISMATCH"),
            });
            return;
        }
    }

    disconnect_transport_preserving_reconnect_plan(session, network_commands);

    let connection_id = ConnectionId::new();
    let host = plan.host.unwrap_or_else(|| config.game_host.clone());
    let port = plan.port.unwrap_or(match plan.transport {
        NetworkTransport::Tcp => config.tcp_fallback_port,
        NetworkTransport::Kcp => config.kcp_port,
    });
    let remote_addr = format!("{host}:{port}");

    session.ticket = Some(ticket);
    session.player_id = Some(ticket_payload.player_id);
    session.character_id = Some(ticket_payload.character_id);
    session.world_id = ticket_payload.world_id;
    session.begin_connect_game(connection_id, plan.transport);
    trace_game_transition(
        "game_connect_begin",
        session,
        Some(connection_id),
        Some(&remote_addr),
        None,
        None,
        None,
    );
    info!(
        connection_id = connection_id.raw(),
        ?plan.transport,
        endpoint = %remote_addr,
        player_id = session.player_id.as_deref().unwrap_or_default(),
        character_id = session.character_id.as_deref().unwrap_or_default(),
        world_id = session.world_id,
        ticket_fp = %ticket_payload.ticket_fingerprint,
        ticket_expires_at = %ticket_payload.exp,
        "MyServer game connection starting"
    );

    events.write(MyServerEvent::Connecting {
        connection_id,
        transport: plan.transport,
        remote_addr: remote_addr.clone(),
    });

    match plan.transport {
        NetworkTransport::Tcp => {
            network_commands.write(NetworkCommand::ConnectTcp(
                TcpConnectConfig::new(remote_addr).with_connection_id(connection_id),
            ));
        }
        NetworkTransport::Kcp => {
            network_commands.write(NetworkCommand::ConnectKcp(
                KcpConnectConfig::new(remote_addr)
                    .with_connection_id(connection_id)
                    .with_session(KcpSessionOptions {
                        stream: true,
                        ..KcpSessionOptions::fastest()
                    }),
            ));
        }
    }
}

fn validate_auth_ticket(ticket: &str, session: &MyServerSession) -> Result<(), String> {
    let payload = parse_character_bound_ticket(ticket)?;
    let Some(character_id) = session.character_id.as_deref() else {
        return Err("MISSING_CHARACTER_ID".to_string());
    };
    if payload.character_id != character_id {
        return Err("TICKET_CHARACTER_MISMATCH".to_string());
    }
    Ok(())
}

fn fail_game_connection_attempt(
    session: &mut MyServerSession,
    network_commands: &mut MessageWriter<NetworkCommand>,
) {
    if let Some(connection_id) = session.connection_id {
        network_commands.write(NetworkCommand::Disconnect { connection_id });
    }
    session.game_connection_failed();
}

fn disconnect(session: &mut MyServerSession, network_commands: &mut MessageWriter<NetworkCommand>) {
    if let Some(connection_id) = session.connection_id {
        network_commands.write(NetworkCommand::Disconnect { connection_id });
    }
    session.disconnect_cleanup();
}

fn disconnect_transport_preserving_reconnect_plan(
    session: &mut MyServerSession,
    network_commands: &mut MessageWriter<NetworkCommand>,
) {
    let reconnect_after_auth = session.reconnect_after_auth.clone();
    if let Some(connection_id) = session.connection_id {
        network_commands.write(NetworkCommand::Disconnect { connection_id });
    }
    session.disconnect_cleanup();
    session.reconnect_after_auth = reconnect_after_auth;
}

fn send_auth_request(
    session: &mut MyServerSession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<MyServerEvent>,
    ticket: String,
) {
    session.begin_game_auth();
    let ticket_fp = redact_secret_fingerprint(&ticket);
    let Some(auth_seq) = send_request_with_seq(
        session,
        network_commands,
        events,
        MessageType::AuthReq,
        MessageType::AuthRes,
        &pb::AuthReq { ticket },
    ) else {
        return;
    };
    trace_game_transition(
        "game_auth_request_sent",
        session,
        session.connection_id,
        None,
        Some(auth_seq),
        Some(MessageType::AuthReq),
        None,
    );
    info!(
        connection_id = session.connection_id.map(ConnectionId::raw),
        seq = auth_seq,
        ticket_fp = %ticket_fp,
        ticket_remaining_seconds = diagnostic_snapshot(session).ticket_remaining_seconds,
        "MyServer game auth request sent"
    );
}

fn send_move_input(
    session: &mut MyServerSession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<MyServerEvent>,
    frame_id: u32,
    input_type: pb::MoveInputType,
    dir_x: f32,
    dir_y: f32,
    client_state: Option<MovementClientState>,
) {
    let (has_client_state, client_x, client_y, client_frame_id) = match client_state {
        Some(state) => (true, state.x, state.y, state.frame_id),
        None => (false, 0.0, 0.0, 0),
    };

    send_request(
        session,
        network_commands,
        events,
        MessageType::MoveInputReq,
        MessageType::MoveInputRes,
        &pb::MoveInputReq {
            frame_id,
            input_type: input_type as i32,
            dir_x,
            dir_y,
            has_client_state,
            client_x,
            client_y,
            client_frame_id,
            client_timestamp_ms: current_unix_ms(),
        },
    );
}

fn send_request<M>(
    session: &mut MyServerSession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<MyServerEvent>,
    request_type: MessageType,
    response_type: MessageType,
    message: &M,
) where
    M: prost::Message,
{
    let _ = send_request_with_seq(
        session,
        network_commands,
        events,
        request_type,
        response_type,
        message,
    );
}

fn send_room_reconnect_request(
    session: &mut MyServerSession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<MyServerEvent>,
) {
    let last_character_push_sequence = session
        .character_elements
        .last_push_sequence
        .unwrap_or_default();
    let _ = send_request_with_seq(
        session,
        network_commands,
        events,
        MessageType::RoomReconnectReq,
        MessageType::RoomReconnectRes,
        &pb::RoomReconnectReq {
            last_character_push_sequence,
        },
    );
}

fn send_character_elements_snapshot_request(
    session: &mut MyServerSession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<MyServerEvent>,
) {
    let _ = send_request_with_seq(
        session,
        network_commands,
        events,
        MessageType::GetCharacterElementsReq,
        MessageType::GetCharacterElementsRes,
        &pb::GetCharacterElementsReq {},
    );
}

fn send_request_with_seq<M>(
    session: &mut MyServerSession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<MyServerEvent>,
    request_type: MessageType,
    response_type: MessageType,
    message: &M,
) -> Option<u32>
where
    M: prost::Message,
{
    let Some(connection_id) = session.connection_id else {
        write_display_error(
            events,
            MyServerDisplayError::transport(
                MyServerOperation::GameRequest,
                Some("game connection is not open".to_string()),
            ),
        );
        events.write(MyServerEvent::RequestFailed {
            seq: None,
            message_type: Some(request_type),
            error: "game connection is not open".to_string(),
        });
        return None;
    };

    let seq = session.reserve_seq();
    session
        .pending
        .insert(seq, PendingRequest { response_type });
    let payload = encode_proto_packet(request_type, seq, message);
    trace_game_transition(
        "game_request_sent",
        session,
        Some(connection_id),
        None,
        Some(seq),
        Some(request_type),
        None,
    );
    network_commands.write(NetworkCommand::Send {
        connection_id,
        payload,
    });
    Some(seq)
}

fn handle_game_packet(
    config: &MyServerConfig,
    session: &mut MyServerSession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<MyServerEvent>,
    packet: Packet,
) {
    let Some(message_type) = packet.message_type() else {
        write_display_error(
            events,
            MyServerDisplayError::protocol(
                None,
                Some(packet.header.seq),
                Some(format!("unknown msgType {}", packet.header.msg_type)),
            ),
        );
        events.write(MyServerEvent::ProtocolError {
            error: format!("unknown msgType {}", packet.header.msg_type),
        });
        return;
    };

    if message_type == MessageType::ErrorRes {
        match packet.decode::<pb::ErrorRes>() {
            Ok(error) => {
                session.pending.remove(&packet.header.seq);
                write_display_error(
                    events,
                    display_error_from_game_code(
                        Some(MessageType::ErrorRes),
                        Some(packet.header.seq),
                        &error.error_code,
                        Some(error.message.clone()),
                    ),
                );
                events.write(MyServerEvent::Error {
                    seq: packet.header.seq,
                    error_code: error.error_code,
                    message: error.message,
                });
            }
            Err(error) => {
                write_display_error(
                    events,
                    MyServerDisplayError::protobuf_decode(
                        Some(MessageType::ErrorRes),
                        Some(packet.header.seq),
                        Some(error.clone()),
                    ),
                );
                events.write(MyServerEvent::ProtocolError { error });
            }
        }
        return;
    }

    match message_type {
        MessageType::RoomStatePush => {
            decode_push::<pb::RoomStatePush, _>(events, &packet, MyServerEvent::RoomStatePush)
        }
        MessageType::GameMessagePush => {
            decode_push::<pb::GameMessagePush, _>(events, &packet, MyServerEvent::GameMessagePush)
        }
        MessageType::FrameBundlePush => {
            decode_push::<pb::FrameBundlePush, _>(events, &packet, MyServerEvent::FrameBundlePush)
        }
        MessageType::RoomFrameRatePush => decode_push::<pb::RoomFrameRatePush, _>(
            events,
            &packet,
            MyServerEvent::RoomFrameRatePush,
        ),
        MessageType::RoomMemberOfflinePush => decode_push::<pb::RoomMemberOfflinePush, _>(
            events,
            &packet,
            MyServerEvent::RoomMemberOfflinePush,
        ),
        MessageType::MovementSnapshotPush => decode_push::<pb::MovementSnapshotPush, _>(
            events,
            &packet,
            MyServerEvent::MovementSnapshotPush,
        ),
        MessageType::MovementRejectPush => decode_push::<pb::MovementRejectPush, _>(
            events,
            &packet,
            MyServerEvent::MovementRejectPush,
        ),
        MessageType::ServerRedirectPush => {
            handle_server_redirect_push(config, session, network_commands, events, packet)
        }
        MessageType::SessionKickPush => {
            handle_session_kick_push(session, network_commands, events, packet)
        }
        MessageType::AuthorityMigrationStartPush => {
            decode_push::<pb::AuthorityMigrationStartPush, _>(
                events,
                &packet,
                MyServerEvent::AuthorityMigrationStartPush,
            )
        }
        MessageType::AuthorityMigrationCompletePush => {
            decode_push::<pb::AuthorityMigrationCompletePush, _>(
                events,
                &packet,
                MyServerEvent::AuthorityMigrationCompletePush,
            )
        }
        MessageType::CharacterElementsChangePush => {
            handle_character_elements_push(session, events, packet)
        }
        _ => handle_response_packet(session, network_commands, events, message_type, packet),
    }
}

fn handle_response_packet(
    session: &mut MyServerSession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<MyServerEvent>,
    message_type: MessageType,
    packet: Packet,
) {
    let Some(pending) = session.pending.remove(&packet.header.seq) else {
        write_display_error(
            events,
            MyServerDisplayError::protocol(
                Some(message_type),
                Some(packet.header.seq),
                Some(format!(
                    "received response {:?} for unknown seq {}",
                    message_type, packet.header.seq
                )),
            ),
        );
        events.write(MyServerEvent::ProtocolError {
            error: format!(
                "received response {:?} for unknown seq {}",
                message_type, packet.header.seq
            ),
        });
        return;
    };

    if pending.response_type != message_type {
        write_display_error(
            events,
            MyServerDisplayError::protocol(
                Some(message_type),
                Some(packet.header.seq),
                Some(format!(
                    "unexpected response type for seq {}: expected {:?}, got {:?}",
                    packet.header.seq, pending.response_type, message_type
                )),
            ),
        );
        events.write(MyServerEvent::ProtocolError {
            error: format!(
                "unexpected response type for seq {}: expected {:?}, got {:?}",
                packet.header.seq, pending.response_type, message_type
            ),
        });
        return;
    }

    match message_type {
        MessageType::AuthRes => match packet.decode::<pb::AuthRes>() {
            Ok(response) if response.ok => {
                let Some(gameplay_character_id) = session.character_id.clone() else {
                    session.game_auth_failed();
                    session.clear_reconnect_plan();
                    trace_game_transition(
                        "game_auth_missing_character_id",
                        session,
                        session.connection_id,
                        None,
                        Some(packet.header.seq),
                        Some(MessageType::AuthRes),
                        Some("MISSING_CHARACTER_ID"),
                    );
                    warn!(
                        connection_id = session.connection_id.map(ConnectionId::raw),
                        seq = packet.header.seq,
                        account_player_id = %response.player_id,
                        "MyServer game auth succeeded without selected character"
                    );
                    events.write(MyServerEvent::AuthFailed {
                        error_code: "MISSING_CHARACTER_ID".to_string(),
                    });
                    events.write(MyServerEvent::GameAuthRejected {
                        error_code: "MISSING_CHARACTER_ID".to_string(),
                        reason: classify_game_auth_failure("MISSING_CHARACTER_ID"),
                    });
                    write_display_error(
                        events,
                        display_error_from_game_code(
                            Some(MessageType::AuthRes),
                            Some(packet.header.seq),
                            "MISSING_CHARACTER_ID",
                            Some("MISSING_CHARACTER_ID".to_string()),
                        ),
                    );
                    return;
                };
                session.game_authenticated(response.player_id.clone());
                let reconnect_plan = session.reconnect_after_auth.clone();
                trace_game_transition(
                    "game_auth_succeeded",
                    session,
                    session.connection_id,
                    None,
                    Some(packet.header.seq),
                    Some(MessageType::AuthRes),
                    None,
                );
                info!(
                    connection_id = session.connection_id.map(ConnectionId::raw),
                    seq = packet.header.seq,
                    account_player_id = %response.player_id,
                    character_id = %gameplay_character_id,
                    ticket_fp = diagnostic_snapshot(session).ticket_fingerprint.as_deref().unwrap_or_default(),
                    "MyServer game auth succeeded"
                );
                if let Some(plan) = reconnect_plan {
                    // Reconnect auth is intentionally not the normal Authenticated event:
                    // room/authority recovery comes from RoomReconnectRes, not a fresh join.
                    events.write(MyServerEvent::ReauthenticatedForReconnect {
                        player_id: gameplay_character_id,
                        cause: plan.cause.clone(),
                    });
                    send_room_reconnect_request(session, network_commands, events);
                } else {
                    events.write(MyServerEvent::Authenticated {
                        player_id: gameplay_character_id,
                    });
                }
            }
            Ok(response) => {
                session.game_auth_failed();
                session.clear_reconnect_plan();
                let reason = classify_game_auth_failure(&response.error_code);
                trace_game_transition(
                    "game_auth_failed",
                    session,
                    session.connection_id,
                    None,
                    Some(packet.header.seq),
                    Some(MessageType::AuthRes),
                    Some(&response.error_code),
                );
                warn!(
                    connection_id = session.connection_id.map(ConnectionId::raw),
                    seq = packet.header.seq,
                    error_code = %response.error_code,
                    ?reason,
                    ticket_fp = diagnostic_snapshot(session).ticket_fingerprint.as_deref().unwrap_or_default(),
                    "MyServer game auth rejected"
                );
                events.write(MyServerEvent::AuthFailed {
                    error_code: response.error_code.clone(),
                });
                events.write(MyServerEvent::GameAuthRejected {
                    error_code: response.error_code.clone(),
                    reason,
                });
                write_display_error(
                    events,
                    display_error_from_game_code(
                        Some(MessageType::AuthRes),
                        Some(packet.header.seq),
                        &response.error_code,
                        Some(response.error_code.clone()),
                    ),
                );
            }
            Err(error) => {
                session.game_auth_failed();
                session.clear_reconnect_plan();
                trace_game_transition(
                    "game_auth_decode_failed",
                    session,
                    session.connection_id,
                    None,
                    Some(packet.header.seq),
                    Some(MessageType::AuthRes),
                    Some("PROTOCOL_ERROR"),
                );
                events.write(MyServerEvent::GameAuthRejected {
                    error_code: "PROTOCOL_ERROR".to_string(),
                    reason: classify_game_auth_failure("PROTOCOL_ERROR"),
                });
                write_display_error(
                    events,
                    MyServerDisplayError::protobuf_decode(
                        Some(MessageType::AuthRes),
                        Some(packet.header.seq),
                        Some(error.clone()),
                    ),
                );
                events.write(MyServerEvent::ProtocolError { error });
            }
        },
        MessageType::PingRes => decode_push::<pb::PingRes, _>(events, &packet, MyServerEvent::Pong),
        MessageType::RoomJoinRes => match packet.decode::<pb::RoomJoinRes>() {
            Ok(response) => {
                if response.ok {
                    session.room_id = Some(response.room_id.clone());
                } else {
                    write_display_error(
                        events,
                        display_error_from_game_code(
                            Some(MessageType::RoomJoinRes),
                            Some(packet.header.seq),
                            &response.error_code,
                            Some(response.error_code.clone()),
                        ),
                    );
                }
                events.write(MyServerEvent::RoomJoined(response));
            }
            Err(error) => {
                write_display_error(
                    events,
                    MyServerDisplayError::protobuf_decode(
                        Some(MessageType::RoomJoinRes),
                        Some(packet.header.seq),
                        Some(error.clone()),
                    ),
                );
                events.write(MyServerEvent::ProtocolError { error });
            }
        },
        MessageType::RoomLeaveRes => match packet.decode::<pb::RoomLeaveRes>() {
            Ok(response) => {
                if response.ok {
                    session.room_id = None;
                }
                events.write(MyServerEvent::RoomLeft(response));
            }
            Err(error) => {
                write_display_error(
                    events,
                    MyServerDisplayError::protobuf_decode(
                        Some(MessageType::RoomLeaveRes),
                        Some(packet.header.seq),
                        Some(error.clone()),
                    ),
                );
                events.write(MyServerEvent::ProtocolError { error });
            }
        },
        MessageType::RoomReconnectRes => match packet.decode::<pb::RoomReconnectRes>() {
            Ok(response) => {
                if response.ok {
                    session.room_id =
                        (!response.room_id.is_empty()).then_some(response.room_id.clone());
                    session.reconnect_after_auth = None;
                    // Room reconnect restores connection and room membership only. The
                    // character elements snapshot is always refreshed; title/class snapshots
                    // stay on their existing HTTP/profile refresh boundary.
                    send_character_elements_snapshot_request(session, network_commands, events);
                } else {
                    session.room_id = None;
                    session.reconnect_failed_cleanup();
                    write_display_error(
                        events,
                        display_error_from_game_code(
                            Some(MessageType::RoomReconnectRes),
                            Some(packet.header.seq),
                            &response.error_code,
                            Some(response.error_code.clone()),
                        ),
                    );
                }
                events.write(MyServerEvent::RoomReconnected(response));
            }
            Err(error) => {
                session.reconnect_failed_cleanup();
                write_display_error(
                    events,
                    MyServerDisplayError::protobuf_decode(
                        Some(MessageType::RoomReconnectRes),
                        Some(packet.header.seq),
                        Some(error.clone()),
                    ),
                );
                events.write(MyServerEvent::ProtocolError { error });
            }
        },
        MessageType::RoomReadyRes => {
            decode_push::<pb::RoomReadyRes, _>(events, &packet, MyServerEvent::ReadyChanged)
        }
        MessageType::RoomStartRes => {
            decode_push::<pb::RoomStartRes, _>(events, &packet, MyServerEvent::RoomStarted)
        }
        MessageType::PlayerInputRes => decode_push::<pb::PlayerInputRes, _>(
            events,
            &packet,
            MyServerEvent::PlayerInputAccepted,
        ),
        MessageType::MoveInputRes => {
            decode_push::<pb::MoveInputRes, _>(events, &packet, MyServerEvent::MoveInputAccepted)
        }
        MessageType::GetCharacterElementsRes => {
            handle_character_elements_response(session, events, packet)
        }
        _ => {
            write_display_error(
                events,
                MyServerDisplayError::protocol(
                    Some(message_type),
                    Some(packet.header.seq),
                    Some(format!("unhandled response type {:?}", message_type)),
                ),
            );
            events.write(MyServerEvent::ProtocolError {
                error: format!("unhandled response type {:?}", message_type),
            });
        }
    }
}

fn decode_push<M, F>(events: &mut MessageWriter<MyServerEvent>, packet: &Packet, event_factory: F)
where
    M: prost::Message + Default,
    F: FnOnce(M) -> MyServerEvent,
{
    let message_type = packet.message_type();
    match packet.decode::<M>() {
        Ok(message) => {
            events.write(event_factory(message));
        }
        Err(error) => {
            write_display_error(
                events,
                MyServerDisplayError::protobuf_decode(
                    message_type,
                    Some(packet.header.seq),
                    Some(error.clone()),
                ),
            );
            events.write(MyServerEvent::ProtocolError { error });
        }
    }
}

fn handle_server_redirect_push(
    config: &MyServerConfig,
    session: &mut MyServerSession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<MyServerEvent>,
    packet: Packet,
) {
    match packet.decode::<pb::ServerRedirectPush>() {
        Ok(push) => {
            let endpoint = if push.target_host.is_empty() || push.target_port == 0 {
                String::new()
            } else {
                format!("{}:{}", push.target_host, push.target_port)
            };
            trace_game_transition(
                "server_redirect_push",
                session,
                session.connection_id,
                (!endpoint.is_empty()).then_some(endpoint.as_str()),
                Some(packet.header.seq),
                Some(MessageType::ServerRedirectPush),
                Some(push.reason.as_str()),
            );
            warn!(
                connection_id = session.connection_id.map(ConnectionId::raw),
                seq = packet.header.seq,
                reason = %push.reason,
                room_id = %push.room_id,
                rollout_epoch = %push.rollout_epoch,
                reconnect_required = push.reconnect_required,
                retry_after_ms = push.retry_after_ms,
                endpoint = %endpoint,
                target_server_id = %push.target_server_id,
                transport = %push.transport,
                ticket_fp = diagnostic_snapshot(session).ticket_fingerprint.as_deref().unwrap_or_default(),
                "MyServer server redirect push"
            );
            let reconnect_required = push.reconnect_required;
            let reason = push.reason.clone();
            let room_id = non_empty(push.room_id.as_str()).map(ToOwned::to_owned);
            let target_server_id = non_empty(push.target_server_id.as_str()).map(ToOwned::to_owned);
            let rollout_epoch = non_empty(push.rollout_epoch.as_str()).map(ToOwned::to_owned);
            let target_host = non_empty(push.target_host.as_str()).map(ToOwned::to_owned);
            let target_port = u16::try_from(push.target_port)
                .ok()
                .filter(|port| *port > 0);
            let target_transport =
                redirect_transport(&push.transport).unwrap_or(config.prefer_transport);

            events.write(MyServerEvent::ServerRedirectPush(push));

            if !reconnect_required {
                events.write(MyServerEvent::ServerRedirectIgnored {
                    reason,
                    detail: "redirect does not require reconnect".to_string(),
                });
                return;
            }

            let (Some(target_host), Some(target_port)) = (target_host, target_port) else {
                write_display_error(
                    events,
                    MyServerDisplayError::from_error_code(
                        MyServerErrorSource::Game,
                        Some(MyServerOperation::GameConnect),
                        Some(MessageType::ServerRedirectPush),
                        Some(packet.header.seq),
                        None,
                        "REDIRECT_MISSING_ENDPOINT",
                        Some(
                            "server redirect requires reconnect but did not include endpoint"
                                .to_string(),
                        ),
                    ),
                );
                events.write(MyServerEvent::ServerRedirectIgnored {
                    reason,
                    detail: "redirect requires reconnect but endpoint is missing".to_string(),
                });
                return;
            };

            if session.reconnect_blocked {
                write_display_error(
                    events,
                    MyServerDisplayError::from_error_code(
                        MyServerErrorSource::Client,
                        Some(MyServerOperation::GameConnect),
                        Some(MessageType::ServerRedirectPush),
                        Some(packet.header.seq),
                        None,
                        "SESSION_KICKED",
                        Some("redirect reconnect blocked by session kick".to_string()),
                    ),
                );
                events.write(MyServerEvent::ServerRedirectIgnored {
                    reason,
                    detail: "redirect reconnect blocked by session kick".to_string(),
                });
                return;
            }

            let ticket_operation = PendingHttpOperation::TicketIssue {
                reconnect_game: true,
            };
            if has_duplicate_pending_http(session, &ticket_operation) {
                write_display_error(
                    events,
                    MyServerDisplayError::from_error_code(
                        MyServerErrorSource::Client,
                        Some(MyServerOperation::TicketRefresh),
                        Some(MessageType::ServerRedirectPush),
                        Some(packet.header.seq),
                        None,
                        "REDIRECT_TICKET_PENDING",
                        Some(
                            "redirect reconnect refused because ticket issue is already pending"
                                .to_string(),
                        ),
                    ),
                );
                events.write(MyServerEvent::ServerRedirectIgnored {
                    reason,
                    detail: "redirect reconnect refused because ticket issue is already pending"
                        .to_string(),
                });
                return;
            }

            session.room_id = None;
            session.reconnect_after_auth = Some(ReconnectPlan {
                cause: ReconnectCause::ServerRedirect {
                    reason: reason.clone(),
                    room_id,
                    target_server_id,
                    rollout_epoch,
                },
            });
            session.connect_after_login = Some(ConnectPlan {
                transport: target_transport,
                host: Some(target_host.clone()),
                port: Some(target_port),
            });

            if let Some(connection_id) = session.connection_id {
                network_commands.write(NetworkCommand::Disconnect { connection_id });
            }
            session.connection_id = None;
            session.connected = false;
            session.authenticated = false;
            session.codec.clear();
            session.pending.clear();
            session.game_connection_state = GameConnectionState::Reconnecting;

            events.write(MyServerEvent::ServerRedirectReconnectStarted {
                reason: reason.clone(),
                target_host,
                target_port,
                transport: target_transport,
            });
            send_refresh_ticket(config, session, network_commands, events, true);
        }
        Err(error) => {
            write_display_error(
                events,
                MyServerDisplayError::protobuf_decode(
                    Some(MessageType::ServerRedirectPush),
                    Some(packet.header.seq),
                    Some(error.clone()),
                ),
            );
            events.write(MyServerEvent::ProtocolError { error });
        }
    }
}

fn handle_session_kick_push(
    session: &mut MyServerSession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<MyServerEvent>,
    packet: Packet,
) {
    match packet.decode::<pb::SessionKickPush>() {
        Ok(push) => {
            trace_game_transition(
                "session_kick_push",
                session,
                session.connection_id,
                None,
                Some(packet.header.seq),
                Some(MessageType::SessionKickPush),
                Some(push.reason.as_str()),
            );
            warn!(
                connection_id = session.connection_id.map(ConnectionId::raw),
                seq = packet.header.seq,
                reason = %push.reason,
                timestamp = push.timestamp,
                ticket_fp = diagnostic_snapshot(session).ticket_fingerprint.as_deref().unwrap_or_default(),
                "MyServer session kick push"
            );
            let reason = push.reason.clone();
            let category = classify_session_kick(reason.as_str());
            let timestamp = push.timestamp;
            events.write(MyServerEvent::SessionKickPush(push));
            let old_connection_id = session.connection_id;
            session.block_reconnect_after_kick();
            if let Some(connection_id) = old_connection_id {
                network_commands.write(NetworkCommand::Disconnect { connection_id });
            }
            apply_session_kick_state_and_events(session, events, category, reason.as_str());
            write_display_error(
                events,
                display_error_from_kick(category, reason.as_str(), Some(packet.header.seq)),
            );
            events.write(MyServerEvent::SessionKicked {
                reason,
                category,
                timestamp,
            });
        }
        Err(error) => {
            write_display_error(
                events,
                MyServerDisplayError::protobuf_decode(
                    Some(MessageType::SessionKickPush),
                    Some(packet.header.seq),
                    Some(error.clone()),
                ),
            );
            events.write(MyServerEvent::ProtocolError { error });
        }
    }
}

fn apply_session_kick_state_and_events(
    session: &mut MyServerSession,
    events: &mut MessageWriter<MyServerEvent>,
    category: SessionKickCategory,
    reason: &str,
) {
    match category {
        SessionKickCategory::ConcurrentLogin => {
            session.account_expired();
            events.write(MyServerEvent::AccountStatusBlocked {
                code: "SESSION_KICK_CONCURRENT_LOGIN".to_string(),
                message: reason.to_string(),
            });
        }
        SessionKickCategory::Banned => {
            session.account_blocked();
            events.write(MyServerEvent::AccountBanned {
                message: reason.to_string(),
                banned_until: None,
            });
        }
        SessionKickCategory::Maintenance => {
            session.account_blocked();
            events.write(MyServerEvent::MaintenanceBlocked {
                message: reason.to_string(),
                retry_after_seconds: None,
            });
        }
        SessionKickCategory::ServerOffline | SessionKickCategory::Unknown => {}
    }
    session.reconnect_blocked = true;
    session.game_connection_state = GameConnectionState::Disconnected;
}

fn handle_character_elements_response(
    session: &mut MyServerSession,
    events: &mut MessageWriter<MyServerEvent>,
    packet: Packet,
) {
    match packet.decode::<pb::GetCharacterElementsRes>() {
        Ok(response) => {
            if !response.ok {
                write_display_error(
                    events,
                    display_error_from_game_code(
                        Some(MessageType::GetCharacterElementsRes),
                        Some(packet.header.seq),
                        &response.error_code,
                        Some(response.error_code.clone()),
                    ),
                );
            }
            if let Some(cache) =
                session.apply_character_elements_response(&response, SystemTime::now())
            {
                events.write(MyServerEvent::CharacterElementsCacheUpdated(cache));
            }
            events.write(MyServerEvent::CharacterElementsLoaded(response));
        }
        Err(error) => {
            write_display_error(
                events,
                MyServerDisplayError::protobuf_decode(
                    Some(MessageType::GetCharacterElementsRes),
                    Some(packet.header.seq),
                    Some(error.clone()),
                ),
            );
            events.write(MyServerEvent::ProtocolError { error });
        }
    }
}

fn handle_character_elements_push(
    session: &mut MyServerSession,
    events: &mut MessageWriter<MyServerEvent>,
    packet: Packet,
) {
    match packet.decode::<pb::CharacterElementsChangePush>() {
        Ok(push) => {
            if let Some(cache) = session.apply_character_elements_push(&push, SystemTime::now()) {
                events.write(MyServerEvent::CharacterElementsCacheUpdated(cache));
            }
            events.write(MyServerEvent::CharacterElementsChanged(push));
        }
        Err(error) => {
            write_display_error(
                events,
                MyServerDisplayError::protobuf_decode(
                    Some(MessageType::CharacterElementsChangePush),
                    Some(packet.header.seq),
                    Some(error.clone()),
                ),
            );
            events.write(MyServerEvent::ProtocolError { error });
        }
    }
}

fn body_text(body: &[u8]) -> String {
    String::from_utf8_lossy(body).into_owned()
}

fn current_unix_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy::ecs::message::MessageCursor;
    use bevy::prelude::{App, Messages, MinimalPlugins};
    use serde_json::Value;

    use super::super::protocol::encode_raw_packet;
    use super::*;
    use crate::framework::network::{HttpMethod, HttpResponse};
    use crate::game::myserver::types::{
        AccountLoginState, CharacterSummary, GameAuthFailureReason, GameConnectionState,
        MyServerErrorKind, MyServerErrorSource,
    };

    fn test_config() -> MyServerConfig {
        MyServerConfig {
            http_base_url: "http://auth.test/root/".to_string(),
            request_timeout: Duration::from_millis(1234),
            ..Default::default()
        }
    }

    fn header<'a>(request: &'a HttpRequest, name: &str) -> Option<&'a str> {
        request
            .headers
            .iter()
            .find(|(header, _)| header == name)
            .map(|(_, value)| value.as_str())
    }

    fn body_json(request: &HttpRequest) -> Value {
        serde_json::from_slice(request.body.as_deref().unwrap_or_default()).unwrap()
    }

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<NetworkCommand>()
            .add_message::<NetworkEvent>()
            .add_plugins(MyServerPlugin)
            .insert_resource(test_config());
        app
    }

    fn read_messages<M>(app: &App) -> Vec<M>
    where
        M: Message + Clone,
    {
        let messages = app.world().resource::<Messages<M>>();
        let mut cursor = MessageCursor::default();
        cursor.read(messages).cloned().collect()
    }

    fn latest_http_request(app: &App) -> Option<HttpRequest> {
        read_messages::<NetworkCommand>(app)
            .into_iter()
            .filter_map(|command| match command {
                NetworkCommand::Http(request) => Some(request),
                _ => None,
            })
            .last()
    }

    fn respond_http_ok(app: &mut App, request: &HttpRequest, body: &str) {
        app.world_mut()
            .write_message(NetworkEvent::HttpResponse(HttpResponse {
                request_id: request.request_id,
                status: 200,
                headers: Vec::new(),
                body: body.as_bytes().to_vec(),
            }));
        app.update();
    }

    fn login_without_ticket_response(player_id: &str, guest_id: &str) -> String {
        format!(
            r#"{{
                "ok": true,
                "playerId": "{player_id}",
                "guestId": "{guest_id}",
                "accessToken": "access-token",
                "ticket": null,
                "ticketExpiresAt": null,
                "gameProxyHost": "game.test",
                "gameProxyPort": 14400
            }}"#
        )
    }

    fn start_guest_auto_login(app: &mut App, guest_id: &str) -> HttpRequest {
        app.world_mut().write_message(MyServerCommand::GuestLogin {
            guest_id: Some(guest_id.to_string()),
            connect_game: true,
        });
        app.update();
        latest_http_request(app).unwrap()
    }

    fn complete_guest_auto_account_login(app: &mut App, guest_id: &str) {
        let request = start_guest_auto_login(app, guest_id);
        respond_http_ok(
            app,
            &request,
            &login_without_ticket_response("plr_1", guest_id),
        );
    }

    fn drive_latest_http_request(app: &mut App) -> HttpRequest {
        app.update();
        latest_http_request(app).unwrap()
    }

    fn count_load_character_list_commands(app: &App) -> usize {
        read_messages::<MyServerCommand>(app)
            .iter()
            .filter(|command| matches!(command, MyServerCommand::LoadCharacterList))
            .count()
    }

    fn create_character_commands(app: &App) -> Vec<(String, Option<Value>)> {
        read_messages::<MyServerCommand>(app)
            .into_iter()
            .filter_map(|command| match command {
                MyServerCommand::CreateCharacter {
                    name,
                    appearance_json,
                } => Some((name, appearance_json)),
                _ => None,
            })
            .collect()
    }

    fn select_character_commands(app: &App) -> Vec<(String, bool)> {
        read_messages::<MyServerCommand>(app)
            .into_iter()
            .filter_map(|command| match command {
                MyServerCommand::SelectCharacter {
                    character_id,
                    connect_game,
                } => Some((character_id, connect_game)),
                _ => None,
            })
            .collect()
    }

    fn character_summary(character_id: &str) -> CharacterSummary {
        serde_json::from_value(json!({
            "character_id": character_id,
            "name": "BevyDev",
            "world_id": 0
        }))
        .unwrap()
    }

    fn latest_connect_command(app: &App) -> Option<(ConnectionId, NetworkTransport, String)> {
        read_messages::<NetworkCommand>(app)
            .into_iter()
            .filter_map(|command| match command {
                NetworkCommand::ConnectTcp(config) => {
                    Some((config.connection_id, NetworkTransport::Tcp, config.addr))
                }
                NetworkCommand::ConnectKcp(config) => {
                    Some((config.connection_id, NetworkTransport::Kcp, config.addr))
                }
                _ => None,
            })
            .last()
    }

    fn sent_packets(app: &App) -> Vec<(ConnectionId, Vec<u8>)> {
        read_messages::<NetworkCommand>(app)
            .into_iter()
            .filter_map(|command| match command {
                NetworkCommand::Send {
                    connection_id,
                    payload,
                } => Some((connection_id, payload)),
                _ => None,
            })
            .collect()
    }

    fn decoded_sent_packets(app: &App) -> Vec<(ConnectionId, Packet)> {
        sent_packets(app)
            .into_iter()
            .flat_map(|(connection_id, payload)| {
                let mut codec = super::super::protocol::PacketCodec::default();
                codec
                    .push_bytes(&payload)
                    .unwrap()
                    .into_iter()
                    .map(move |packet| (connection_id, packet))
            })
            .collect()
    }

    fn latest_sent_packet(app: &App) -> Option<(ConnectionId, Packet)> {
        decoded_sent_packets(app).into_iter().last()
    }

    fn disconnect_commands(app: &App) -> Vec<ConnectionId> {
        read_messages::<NetworkCommand>(app)
            .into_iter()
            .filter_map(|command| match command {
                NetworkCommand::Disconnect { connection_id } => Some(connection_id),
                _ => None,
            })
            .collect()
    }

    fn ticket_for_test(player_id: &str, character_id: &str, exp: &str) -> String {
        let payload = format!(
            r#"{{"playerId":"{player_id}","characterId":"{character_id}","worldId":0,"exp":"{exp}","ver":1}}"#
        );
        format!(
            "{}.signature",
            encode_base64url_for_test(payload.as_bytes())
        )
    }

    fn encode_base64url_for_test(input: &[u8]) -> String {
        const TABLE: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut output = String::new();
        let mut index = 0;
        while index < input.len() {
            let b0 = input[index];
            let b1 = input.get(index + 1).copied();
            let b2 = input.get(index + 2).copied();

            output.push(TABLE[(b0 >> 2) as usize] as char);
            output.push(TABLE[(((b0 & 0b0000_0011) << 4) | b1.unwrap_or(0) >> 4) as usize] as char);
            if let Some(b1) = b1 {
                output.push(
                    TABLE[(((b1 & 0b0000_1111) << 2) | b2.unwrap_or(0) >> 6) as usize] as char,
                );
            }
            if let Some(b2) = b2 {
                output.push(TABLE[(b2 & 0b0011_1111) as usize] as char);
            }

            index += 3;
        }
        output
    }

    fn auth_response_packet(seq: u32, ok: bool, player_id: &str, error_code: &str) -> Vec<u8> {
        encode_proto_packet(
            MessageType::AuthRes,
            seq,
            &pb::AuthRes {
                ok,
                player_id: player_id.to_string(),
                error_code: error_code.to_string(),
            },
        )
    }

    fn room_join_response_packet(seq: u32, ok: bool, error_code: &str) -> Vec<u8> {
        encode_proto_packet(
            MessageType::RoomJoinRes,
            seq,
            &pb::RoomJoinRes {
                ok,
                room_id: "room-1".to_string(),
                error_code: error_code.to_string(),
            },
        )
    }

    fn room_reconnect_response_packet(
        seq: u32,
        ok: bool,
        room_id: &str,
        error_code: &str,
    ) -> Vec<u8> {
        encode_proto_packet(
            MessageType::RoomReconnectRes,
            seq,
            &pb::RoomReconnectRes {
                ok,
                room_id: room_id.to_string(),
                error_code: error_code.to_string(),
                snapshot: None,
                current_frame_id: 0,
                recent_inputs: Vec::new(),
                waiting_frame_id: 0,
                waiting_inputs: Vec::new(),
                input_delay_frames: 0,
                movement_recovery: None,
            },
        )
    }

    fn server_redirect_packet(
        reason: &str,
        host: &str,
        port: u32,
        transport: &str,
        reconnect_required: bool,
    ) -> Vec<u8> {
        encode_proto_packet(
            MessageType::ServerRedirectPush,
            0,
            &pb::ServerRedirectPush {
                reason: reason.to_string(),
                room_id: "room-1".to_string(),
                rollout_epoch: "epoch-1".to_string(),
                reconnect_required,
                retry_after_ms: 0,
                target_host: host.to_string(),
                target_port: port,
                target_server_id: "server-b".to_string(),
                transport: transport.to_string(),
            },
        )
    }

    fn session_kick_packet(reason: &str) -> Vec<u8> {
        encode_proto_packet(
            MessageType::SessionKickPush,
            0,
            &pb::SessionKickPush {
                reason: reason.to_string(),
                timestamp: 1_782_399_300,
            },
        )
    }

    fn connect_and_authenticate(app: &mut App, ticket: String) -> ConnectionId {
        app.world_mut()
            .write_message(MyServerCommand::ConnectWithTicket {
                ticket,
                transport: NetworkTransport::Tcp,
                host: Some("game.test".to_string()),
                port: Some(14400),
            });
        app.update();
        let (connection_id, _, _) = latest_connect_command(app).unwrap();
        app.world_mut().write_message(NetworkEvent::Connected {
            connection_id,
            transport: NetworkTransport::Tcp,
            remote_addr: "game.test:14400".to_string(),
        });
        app.update();
        let auth_seq = latest_sent_packet(app).unwrap().1.header.seq;
        app.world_mut().write_message(NetworkEvent::Packet {
            connection_id,
            transport: NetworkTransport::Tcp,
            payload: auth_response_packet(auth_seq, true, "plr_1", ""),
        });
        app.update();
        connection_id
    }

    fn character_elements_response_packet(seq: u32, ok: bool, error_code: &str) -> Vec<u8> {
        encode_proto_packet(
            MessageType::GetCharacterElementsRes,
            seq,
            &pb::GetCharacterElementsRes {
                ok,
                error_code: error_code.to_string(),
                character_id: "chr_1".to_string(),
                elements: None,
            },
        )
    }

    fn display_errors(app: &App) -> Vec<MyServerDisplayError> {
        read_messages::<MyServerEvent>(app)
            .into_iter()
            .filter_map(|event| match event {
                MyServerEvent::DisplayError { error } => Some(error),
                _ => None,
            })
            .collect()
    }

    fn prime_keepalive_timer(app: &mut App, interval: Duration) {
        let mut state = app.world_mut().resource_mut::<MyServerKeepaliveState>();
        state.interval = interval;
        state.timer = Timer::new(interval, TimerMode::Repeating);
        state.timer.set_elapsed(interval);
    }

    #[test]
    fn auto_guest_login_loads_character_list_after_account_session() {
        let mut app = test_app();
        let login_request = start_guest_auto_login(&mut app, "guest-auto-1");

        respond_http_ok(
            &mut app,
            &login_request,
            &login_without_ticket_response("plr_1", "guest-auto-1"),
        );

        assert_eq!(count_load_character_list_commands(&app), 1);
        assert!(
            app.world()
                .resource::<MyServerSession>()
                .connect_after_login
                .is_some()
        );
        assert!(!read_messages::<MyServerEvent>(&app).iter().any(|event| {
            matches!(
                event,
                MyServerEvent::RequestFailed { error, .. }
                    if error.contains("select a character before connecting")
            )
        }));
    }

    #[test]
    fn auto_guest_login_creates_character_after_empty_list_once() {
        let mut app = test_app();
        complete_guest_auto_account_login(&mut app, "guest-auto-empty-123456789");
        let list_request = drive_latest_http_request(&mut app);
        assert_eq!(list_request.url, "http://auth.test/root/api/v1/characters");

        respond_http_ok(
            &mut app,
            &list_request,
            r#"{
                "ok": true,
                "playerId": "plr_1",
                "characters": []
            }"#,
        );

        let create_commands = create_character_commands(&app);
        assert_eq!(create_commands.len(), 1);
        let (name, appearance_json) = &create_commands[0];
        assert!(name.starts_with("Bevy"));
        assert!((2..=16).contains(&name.len()));
        assert!(name.chars().all(|ch| ch.is_ascii_alphanumeric()));
        assert!(appearance_json.is_none());

        app.world_mut()
            .write_message(MyServerEvent::CharacterListLoaded {
                player_id: "plr_1".to_string(),
                characters: Vec::new(),
            });
        app.world_mut()
            .write_message(MyServerEvent::CharacterCreationRequired {
                player_id: "plr_1".to_string(),
            });
        app.update();

        assert_eq!(create_character_commands(&app).len(), 1);
    }

    #[test]
    fn auto_guest_login_selects_existing_character_once() {
        let mut app = test_app();
        complete_guest_auto_account_login(&mut app, "guest-auto-existing");
        let list_request = drive_latest_http_request(&mut app);

        respond_http_ok(
            &mut app,
            &list_request,
            r#"{
                "ok": true,
                "playerId": "plr_1",
                "characters": [
                    {
                        "character_id": "chr_auto_existing",
                        "name": "BevyDev",
                        "world_id": 0
                    }
                ]
            }"#,
        );

        assert!(create_character_commands(&app).is_empty());
        let select_commands = select_character_commands(&app);
        assert_eq!(
            select_commands,
            vec![("chr_auto_existing".to_string(), true)]
        );

        app.world_mut()
            .write_message(MyServerEvent::CharacterListLoaded {
                player_id: "plr_1".to_string(),
                characters: vec![character_summary("chr_auto_existing")],
            });
        app.update();

        assert_eq!(
            select_character_commands(&app),
            vec![("chr_auto_existing".to_string(), true)]
        );
    }

    #[test]
    fn auto_guest_login_selects_created_character_once() {
        let mut app = test_app();
        complete_guest_auto_account_login(&mut app, "guest-auto-create");
        let list_request = drive_latest_http_request(&mut app);
        respond_http_ok(
            &mut app,
            &list_request,
            r#"{
                "ok": true,
                "playerId": "plr_1",
                "characters": []
            }"#,
        );

        let create_request = drive_latest_http_request(&mut app);
        assert_eq!(
            create_request.url,
            "http://auth.test/root/api/v1/characters"
        );
        respond_http_ok(
            &mut app,
            &create_request,
            r#"{
                "ok": true,
                "character": {
                    "character_id": "chr_auto_created",
                    "name": "BevyDev",
                    "world_id": 0
                }
            }"#,
        );

        let select_commands = select_character_commands(&app);
        assert_eq!(
            select_commands,
            vec![("chr_auto_created".to_string(), true)]
        );

        app.world_mut()
            .write_message(MyServerEvent::CharacterCreated {
                character: character_summary("chr_auto_created"),
            });
        app.update();

        assert_eq!(
            select_character_commands(&app),
            vec![("chr_auto_created".to_string(), true)]
        );
    }

    #[test]
    fn builds_account_login_request() {
        let request = build_json_request(
            &test_config(),
            HttpMethod::Post,
            "/api/v1/auth/login",
            Some(json!({ "loginName": "alice", "password": "secret" })),
            None,
        );

        assert!(matches!(request.method, HttpMethod::Post));
        assert_eq!(request.url, "http://auth.test/root/api/v1/auth/login");
        assert_eq!(request.timeout, Duration::from_millis(1234));
        assert!(request.request_id.raw() > 0);
        assert_eq!(header(&request, "Content-Type"), Some("application/json"));
        assert_eq!(header(&request, "Accept"), Some("application/json"));
        assert_eq!(body_json(&request)["loginName"], "alice");
        assert_eq!(body_json(&request)["password"], "secret");
    }

    #[test]
    fn builds_guest_login_request() {
        let request = build_json_request(
            &test_config(),
            HttpMethod::Post,
            "/api/v1/auth/guest-login",
            Some(json!({ "guestId": "guest-a" })),
            None,
        );

        assert_eq!(request.url, "http://auth.test/root/api/v1/auth/guest-login");
        assert_eq!(body_json(&request)["guestId"], "guest-a");
        assert_eq!(header(&request, "Content-Type"), Some("application/json"));
    }

    #[test]
    fn builds_bearer_get_character_list_request() {
        let request = build_json_request(
            &test_config(),
            HttpMethod::Get,
            "/api/v1/characters",
            None,
            Some("access-token"),
        );

        assert!(matches!(request.method, HttpMethod::Get));
        assert_eq!(request.body, None);
        assert_eq!(
            header(&request, "Authorization"),
            Some("Bearer access-token")
        );
        assert_eq!(header(&request, "Accept"), Some("application/json"));
        assert_eq!(header(&request, "Content-Type"), None);
    }

    #[test]
    fn builds_character_create_request() {
        let request = build_json_request(
            &test_config(),
            HttpMethod::Post,
            "/api/v1/characters",
            Some(json!({
                "name": "WindRunner",
                "appearance_json": { "hair": "black" }
            })),
            Some("access-token"),
        );

        assert_eq!(request.url, "http://auth.test/root/api/v1/characters");
        assert_eq!(
            header(&request, "Authorization"),
            Some("Bearer access-token")
        );
        assert_eq!(body_json(&request)["name"], "WindRunner");
        assert_eq!(body_json(&request)["appearance_json"]["hair"], "black");
    }

    #[test]
    fn builds_character_select_and_ticket_issue_requests() {
        let select = build_json_request(
            &test_config(),
            HttpMethod::Post,
            "/api/v1/characters/select",
            Some(json!({ "character_id": "chr_1" })),
            Some("access-token"),
        );
        let ticket = build_json_request(
            &test_config(),
            HttpMethod::Post,
            "/api/v1/game-ticket/issue",
            Some(json!({ "character_id": "chr_1" })),
            Some("access-token"),
        );

        assert_eq!(select.url, "http://auth.test/root/api/v1/characters/select");
        assert_eq!(ticket.url, "http://auth.test/root/api/v1/game-ticket/issue");
        assert_eq!(body_json(&select)["character_id"], "chr_1");
        assert_eq!(body_json(&ticket)["character_id"], "chr_1");
        assert_eq!(
            header(&select, "Authorization"),
            Some("Bearer access-token")
        );
        assert_eq!(header(&ticket, "Content-Type"), Some("application/json"));
    }

    #[test]
    fn issue_ticket_requires_login_and_selected_character() {
        let mut app = test_app();
        app.world_mut().write_message(MyServerCommand::IssueTicket {
            reconnect_game: false,
        });
        app.update();

        let events = read_messages::<MyServerEvent>(&app);
        assert!(events.iter().any(|event| matches!(
            event,
            MyServerEvent::TicketRefreshFailed { error }
                if error == "cannot issue ticket before login"
        )));
        let errors = display_errors(&app);
        assert!(errors.iter().any(|error| {
            error.kind == MyServerErrorKind::Unauthorized
                && error.source == MyServerErrorSource::Client
                && error.operation == Some(MyServerOperation::TicketRefresh)
                && error.error_code.as_deref() == Some("UNAUTHORIZED")
        }));
        assert!(latest_http_request(&app).is_none());

        app.world_mut()
            .resource_mut::<MyServerSession>()
            .access_token = Some("access-token".to_string());
        app.world_mut().write_message(MyServerCommand::IssueTicket {
            reconnect_game: false,
        });
        app.update();

        let events = read_messages::<MyServerEvent>(&app);
        assert!(events.iter().any(|event| matches!(
            event,
            MyServerEvent::TicketRefreshFailed { error }
                if error == "cannot issue ticket before selecting a character"
        )));
        let errors = display_errors(&app);
        assert!(errors.iter().any(|error| {
            error.kind == MyServerErrorKind::MissingCharacterId
                && error.source == MyServerErrorSource::Client
                && error.operation == Some(MyServerOperation::TicketRefresh)
                && error.error_code.as_deref() == Some("MISSING_CHARACTER_ID")
        }));
        assert!(latest_http_request(&app).is_none());
    }

    #[test]
    fn auth_request_packet_seq_matches_pending_request_seq() {
        let mut app = test_app();
        let ticket = ticket_for_test("plr_1", "chr_1", "2026-06-25T12:20:00Z");
        app.world_mut()
            .write_message(MyServerCommand::ConnectWithTicket {
                ticket,
                transport: NetworkTransport::Tcp,
                host: Some("game.test".to_string()),
                port: Some(14400),
            });
        app.update();

        let (connection_id, _, _) = latest_connect_command(&app).unwrap();
        app.world_mut().write_message(NetworkEvent::Connected {
            connection_id,
            transport: NetworkTransport::Tcp,
            remote_addr: "game.test:14400".to_string(),
        });
        app.update();

        let packets = sent_packets(&app);
        let (_, payload) = packets.last().unwrap();
        let mut codec = super::super::protocol::PacketCodec::default();
        let packet = codec.push_bytes(payload).unwrap().remove(0);
        assert_eq!(packet.message_type(), Some(MessageType::AuthReq));

        let session = app.world().resource::<MyServerSession>();
        assert_eq!(session.next_seq, packet.header.seq);
        assert_eq!(
            session
                .pending
                .get(&packet.header.seq)
                .map(|pending| pending.response_type),
            Some(MessageType::AuthRes)
        );
    }

    #[test]
    fn issue_ticket_sends_selected_character_and_preserves_connection_without_reconnect() {
        let mut app = test_app();
        let connection_id = ConnectionId::from_raw(77);
        {
            let mut session = app.world_mut().resource_mut::<MyServerSession>();
            session.access_token = Some("access-token".to_string());
            session.character_id = Some("chr_1".to_string());
            session.ticket = Some(ticket_for_test("plr_1", "chr_1", "2026-06-25T12:15:00Z"));
            session.connected = true;
            session.authenticated = true;
            session.connection_id = Some(connection_id);
            session.game_connection_state = GameConnectionState::Authenticated;
        }

        app.world_mut().write_message(MyServerCommand::IssueTicket {
            reconnect_game: false,
        });
        app.update();

        let request = latest_http_request(&app).unwrap();
        assert_eq!(
            request.url,
            "http://auth.test/root/api/v1/game-ticket/issue"
        );
        assert_eq!(body_json(&request)["character_id"], "chr_1");
        app.world_mut()
            .write_message(NetworkEvent::HttpResponse(HttpResponse {
                request_id: request.request_id,
                status: 200,
                headers: Vec::new(),
                body: format!(
                    r#"{{
                        "ok": true,
                        "playerId": "plr_1",
                        "characterId": "chr_1",
                        "worldId": 0,
                        "ticket": "{}",
                        "ticketExpiresAt": "2026-06-25T12:20:00Z",
                        "gameProxyHost": "game.test",
                        "gameProxyPort": 14400
                    }}"#,
                    ticket_for_test("plr_1", "chr_1", "2026-06-25T12:20:00Z")
                )
                .into_bytes(),
            }));
        app.update();

        let session = app.world().resource::<MyServerSession>();
        assert_eq!(
            session.ticket_expires_at.as_deref(),
            Some("2026-06-25T12:20:00Z")
        );
        assert_eq!(session.connection_id, Some(connection_id));
        assert_eq!(
            session.game_connection_state,
            GameConnectionState::Authenticated
        );
        assert!(disconnect_commands(&app).is_empty());
        assert!(
            read_messages::<MyServerEvent>(&app)
                .iter()
                .any(|event| matches!(
                    event,
                    MyServerEvent::TicketRefreshed { ticket_expires_at }
                        if ticket_expires_at == "2026-06-25T12:20:00Z"
                ))
        );
    }

    #[test]
    fn disabled_keepalive_still_refreshes_expiring_ticket_without_sending_ping() {
        let mut app = test_app();
        {
            let mut config = app.world_mut().resource_mut::<MyServerConfig>();
            config.keepalive_enabled = false;
            config.keepalive_interval = Duration::from_secs(1);
            config.ticket_refresh_margin = Duration::from_secs(60);
        }
        {
            let mut session = app.world_mut().resource_mut::<MyServerSession>();
            session.access_token = Some("access-token".to_string());
            session.character_id = Some("chr_1".to_string());
            session.ticket = Some(ticket_for_test("plr_1", "chr_1", "1"));
            session.ticket_expires_at = Some("1".to_string());
            session.connected = true;
            session.authenticated = true;
            session.connection_id = Some(ConnectionId::from_raw(91));
            session.game_connection_state = GameConnectionState::Authenticated;
        }
        prime_keepalive_timer(&mut app, Duration::from_secs(1));
        app.update();

        let request = latest_http_request(&app).unwrap();
        assert_eq!(
            request.url,
            "http://auth.test/root/api/v1/game-ticket/issue"
        );
        assert_eq!(body_json(&request)["character_id"], "chr_1");
        assert!(sent_packets(&app).is_empty());

        let mut app = test_app();
        {
            let mut config = app.world_mut().resource_mut::<MyServerConfig>();
            config.keepalive_enabled = false;
            config.keepalive_interval = Duration::from_secs(1);
            config.ticket_refresh_margin = Duration::from_secs(1);
        }
        {
            let mut session = app.world_mut().resource_mut::<MyServerSession>();
            session.access_token = Some("access-token".to_string());
            session.character_id = Some("chr_1".to_string());
            session.ticket = Some(ticket_for_test("plr_1", "chr_1", "4102444800"));
            session.ticket_expires_at = Some("4102444800".to_string());
            session.connected = true;
            session.authenticated = true;
            session.connection_id = Some(ConnectionId::from_raw(92));
            session.game_connection_state = GameConnectionState::Authenticated;
        }
        prime_keepalive_timer(&mut app, Duration::from_secs(1));
        app.update();

        assert!(latest_http_request(&app).is_none());
        assert!(sent_packets(&app).is_empty());
    }

    #[test]
    fn issue_ticket_reconnect_uses_response_ticket_and_disconnects_old_connection() {
        let mut app = test_app();
        let old_connection_id = ConnectionId::from_raw(88);
        {
            let mut session = app.world_mut().resource_mut::<MyServerSession>();
            session.access_token = Some("access-token".to_string());
            session.character_id = Some("chr_1".to_string());
            session.connection_id = Some(old_connection_id);
            session.connected = true;
            session.authenticated = true;
            session.game_connection_state = GameConnectionState::Authenticated;
        }

        app.world_mut().write_message(MyServerCommand::IssueTicket {
            reconnect_game: true,
        });
        app.update();
        let request = latest_http_request(&app).unwrap();

        let new_ticket = ticket_for_test("plr_1", "chr_1", "2026-06-25T12:20:00Z");
        app.world_mut()
            .write_message(NetworkEvent::HttpResponse(HttpResponse {
                request_id: request.request_id,
                status: 200,
                headers: Vec::new(),
                body: format!(
                    r#"{{
                        "ok": true,
                        "playerId": "plr_1",
                        "characterId": "chr_1",
                        "worldId": 0,
                        "ticket": "{new_ticket}",
                        "ticketExpiresAt": "2026-06-25T12:20:00Z",
                        "gameProxyHost": "game.test",
                        "gameProxyPort": 14400
                    }}"#
                )
                .into_bytes(),
            }));
        app.update();

        assert!(disconnect_commands(&app).contains(&old_connection_id));
        let (_, transport, addr) = latest_connect_command(&app).unwrap();
        assert_eq!(transport, NetworkTransport::Tcp);
        assert_eq!(addr, "game.test:14400");
        assert_eq!(
            app.world().resource::<MyServerSession>().ticket.as_deref(),
            Some(new_ticket.as_str())
        );
    }

    #[test]
    fn redirect_push_refreshes_ticket_and_reconnects_to_target_endpoint() {
        let mut app = test_app();
        let old_connection_id = ConnectionId::from_raw(188);
        {
            let mut session = app.world_mut().resource_mut::<MyServerSession>();
            session.access_token = Some("access-token".to_string());
            session.character_id = Some("chr_1".to_string());
            session.ticket = Some(ticket_for_test("plr_1", "chr_1", "2026-06-25T12:15:00Z"));
            session.connection_id = Some(old_connection_id);
            session.connected = true;
            session.authenticated = true;
            session.room_id = Some("room-1".to_string());
            session.game_connection_state = GameConnectionState::Authenticated;
        }

        app.world_mut().write_message(NetworkEvent::Packet {
            connection_id: old_connection_id,
            transport: NetworkTransport::Tcp,
            payload: server_redirect_packet("rollout", "redirect.test", 15500, "tcp", true),
        });
        app.update();

        assert!(disconnect_commands(&app).contains(&old_connection_id));
        let request = latest_http_request(&app).unwrap();
        assert_eq!(body_json(&request)["character_id"], "chr_1");
        {
            let session = app.world().resource::<MyServerSession>();
            assert_eq!(session.room_id, None);
            assert!(session.reconnect_after_auth.is_some());
            assert_eq!(
                session.game_connection_state,
                GameConnectionState::Reconnecting
            );
        }

        let new_ticket = ticket_for_test("plr_1", "chr_1", "2026-06-25T12:20:00Z");
        app.world_mut()
            .write_message(NetworkEvent::HttpResponse(HttpResponse {
                request_id: request.request_id,
                status: 200,
                headers: Vec::new(),
                body: format!(
                    r#"{{
                        "ok": true,
                        "playerId": "plr_1",
                        "characterId": "chr_1",
                        "worldId": 0,
                        "ticket": "{new_ticket}",
                        "ticketExpiresAt": "2026-06-25T12:20:00Z",
                        "gameProxyHost": "ignored.test",
                        "gameProxyPort": 14400
                    }}"#
                )
                .into_bytes(),
            }));
        app.update();

        let (connection_id, transport, addr) = latest_connect_command(&app).unwrap();
        assert_ne!(connection_id, old_connection_id);
        assert_eq!(transport, NetworkTransport::Tcp);
        assert_eq!(addr, "redirect.test:15500");

        app.world_mut().write_message(NetworkEvent::Connected {
            connection_id,
            transport,
            remote_addr: addr,
        });
        app.update();
        let auth_seq = latest_sent_packet(&app).unwrap().1.header.seq;
        app.world_mut().write_message(NetworkEvent::Packet {
            connection_id,
            transport,
            payload: auth_response_packet(auth_seq, true, "plr_1", ""),
        });
        app.update();

        let events = read_messages::<MyServerEvent>(&app);
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, MyServerEvent::Authenticated { .. }))
        );
        assert!(events.iter().any(|event| matches!(
            event,
            MyServerEvent::ReauthenticatedForReconnect { player_id, .. }
                if player_id == "chr_1"
        )));
        assert!(
            decoded_sent_packets(&app).iter().any(|(_, packet)| {
                packet.message_type() == Some(MessageType::RoomReconnectReq)
            })
        );
    }

    #[test]
    fn redirect_does_not_disconnect_when_ticket_issue_is_already_pending() {
        let mut app = test_app();
        let connection_id = ConnectionId::from_raw(191);
        {
            let mut session = app.world_mut().resource_mut::<MyServerSession>();
            session.access_token = Some("access-token".to_string());
            session.character_id = Some("chr_1".to_string());
            session.connection_id = Some(connection_id);
            session.connected = true;
            session.authenticated = true;
            session.game_connection_state = GameConnectionState::Authenticated;
        }

        app.world_mut().write_message(MyServerCommand::IssueTicket {
            reconnect_game: true,
        });
        app.update();
        assert!(latest_http_request(&app).is_some());

        app.world_mut().write_message(NetworkEvent::Packet {
            connection_id,
            transport: NetworkTransport::Tcp,
            payload: server_redirect_packet("rollout", "redirect.test", 15500, "tcp", true),
        });
        app.update();

        let session = app.world().resource::<MyServerSession>();
        assert_eq!(session.connection_id, Some(connection_id));
        assert_eq!(session.room_id, None);
        assert!(session.reconnect_after_auth.is_none());
        assert!(!disconnect_commands(&app).contains(&connection_id));
        assert!(
            read_messages::<MyServerEvent>(&app)
                .iter()
                .any(|event| matches!(event, MyServerEvent::ServerRedirectIgnored { .. }))
        );
    }

    #[test]
    fn redirect_ticket_failure_clears_reconnect_plan_before_next_normal_auth() {
        let mut app = test_app();
        let old_connection_id = ConnectionId::from_raw(192);
        {
            let mut session = app.world_mut().resource_mut::<MyServerSession>();
            session.access_token = Some("access-token".to_string());
            session.character_id = Some("chr_1".to_string());
            session.connection_id = Some(old_connection_id);
            session.connected = true;
            session.authenticated = true;
            session.game_connection_state = GameConnectionState::Authenticated;
        }

        app.world_mut().write_message(NetworkEvent::Packet {
            connection_id: old_connection_id,
            transport: NetworkTransport::Tcp,
            payload: server_redirect_packet("rollout", "redirect.test", 15500, "tcp", true),
        });
        app.update();
        let request = latest_http_request(&app).unwrap();

        app.world_mut()
            .write_message(NetworkEvent::HttpResponse(HttpResponse {
                request_id: request.request_id,
                status: 403,
                headers: Vec::new(),
                body: br#"{ "ok": false, "error": "SERVER_BUSY", "message": "try later" }"#
                    .to_vec(),
            }));
        app.update();

        {
            let session = app.world().resource::<MyServerSession>();
            assert!(session.reconnect_after_auth.is_none());
            assert!(session.connect_after_login.is_none());
        }

        let ticket = ticket_for_test("plr_1", "chr_1", "2026-06-25T12:20:00Z");
        connect_and_authenticate(&mut app, ticket);

        let events = read_messages::<MyServerEvent>(&app);
        assert!(
            events
                .iter()
                .any(|event| matches!(event, MyServerEvent::Authenticated { .. }))
        );
        assert!(
            !decoded_sent_packets(&app).iter().any(|(_, packet)| {
                packet.message_type() == Some(MessageType::RoomReconnectReq)
            })
        );
    }

    #[test]
    fn redirect_auth_rejection_clears_reconnect_plan_before_next_normal_auth() {
        let mut app = test_app();
        let connection_id = ConnectionId::from_raw(193);
        let ticket = ticket_for_test("plr_1", "chr_1", "2026-06-25T12:20:00Z");
        {
            let mut session = app.world_mut().resource_mut::<MyServerSession>();
            session.ticket = Some(ticket.clone());
            session.player_id = Some("plr_1".to_string());
            session.character_id = Some("chr_1".to_string());
            session.reconnect_after_auth = Some(ReconnectPlan {
                cause: ReconnectCause::ServerRedirect {
                    reason: "rollout".to_string(),
                    room_id: Some("room-1".to_string()),
                    target_server_id: Some("server-b".to_string()),
                    rollout_epoch: Some("epoch-1".to_string()),
                },
            });
            session.begin_connect_game(connection_id, NetworkTransport::Tcp);
        }

        app.world_mut().write_message(NetworkEvent::Connected {
            connection_id,
            transport: NetworkTransport::Tcp,
            remote_addr: "redirect.test:15500".to_string(),
        });
        app.update();
        let auth_seq = latest_sent_packet(&app).unwrap().1.header.seq;
        app.world_mut().write_message(NetworkEvent::Packet {
            connection_id,
            transport: NetworkTransport::Tcp,
            payload: auth_response_packet(auth_seq, false, "", "TICKET_EXPIRED"),
        });
        app.update();

        assert!(
            app.world()
                .resource::<MyServerSession>()
                .reconnect_after_auth
                .is_none()
        );

        let ticket = ticket_for_test("plr_1", "chr_1", "2026-06-25T12:30:00Z");
        connect_and_authenticate(&mut app, ticket);

        let reconnect_count = decoded_sent_packets(&app)
            .iter()
            .filter(|(_, packet)| packet.message_type() == Some(MessageType::RoomReconnectReq))
            .count();
        assert_eq!(reconnect_count, 0);
        assert!(
            read_messages::<MyServerEvent>(&app)
                .iter()
                .any(|event| matches!(event, MyServerEvent::Authenticated { .. }))
        );
    }

    #[test]
    fn reconnect_after_auth_sends_character_push_cursor_and_refreshes_elements() {
        let mut app = test_app();
        let connection_id = ConnectionId::from_raw(189);
        let ticket = ticket_for_test("plr_1", "chr_1", "2026-06-25T12:20:00Z");
        {
            let mut session = app.world_mut().resource_mut::<MyServerSession>();
            session.ticket = Some(ticket.clone());
            session.player_id = Some("plr_1".to_string());
            session.character_id = Some("chr_1".to_string());
            session.connection_id = Some(connection_id);
            session.character_elements.last_push_sequence = Some(42);
            session.reconnect_after_auth = Some(ReconnectPlan {
                cause: ReconnectCause::ServerRedirect {
                    reason: "rollout".to_string(),
                    room_id: Some("room-1".to_string()),
                    target_server_id: Some("server-b".to_string()),
                    rollout_epoch: Some("epoch-1".to_string()),
                },
            });
            session.begin_connect_game(connection_id, NetworkTransport::Tcp);
        }

        app.world_mut().write_message(NetworkEvent::Connected {
            connection_id,
            transport: NetworkTransport::Tcp,
            remote_addr: "redirect.test:15500".to_string(),
        });
        app.update();
        let auth_seq = latest_sent_packet(&app).unwrap().1.header.seq;

        app.world_mut().write_message(NetworkEvent::Packet {
            connection_id,
            transport: NetworkTransport::Tcp,
            payload: auth_response_packet(auth_seq, true, "plr_1", ""),
        });
        app.update();

        let (_, reconnect_packet) = latest_sent_packet(&app).unwrap();
        assert_eq!(
            reconnect_packet.message_type(),
            Some(MessageType::RoomReconnectReq)
        );
        assert_eq!(
            reconnect_packet
                .decode::<pb::RoomReconnectReq>()
                .unwrap()
                .last_character_push_sequence,
            42
        );
        assert!(
            read_messages::<MyServerEvent>(&app)
                .iter()
                .any(|event| matches!(
                    event,
                    MyServerEvent::ReauthenticatedForReconnect { player_id, .. }
                        if player_id == "chr_1"
                ))
        );
        assert!(
            !read_messages::<MyServerEvent>(&app)
                .iter()
                .any(|event| matches!(event, MyServerEvent::Authenticated { .. }))
        );

        let reconnect_seq = reconnect_packet.header.seq;
        app.world_mut().write_message(NetworkEvent::Packet {
            connection_id,
            transport: NetworkTransport::Tcp,
            payload: room_reconnect_response_packet(reconnect_seq, true, "room-1", ""),
        });
        app.update();

        let (_, elements_packet) = latest_sent_packet(&app).unwrap();
        assert_eq!(
            elements_packet.message_type(),
            Some(MessageType::GetCharacterElementsReq)
        );
        let session = app.world().resource::<MyServerSession>();
        assert_eq!(session.room_id.as_deref(), Some("room-1"));
        assert!(session.reconnect_after_auth.is_none());
    }

    #[test]
    fn kick_push_blocks_reconnect_and_classifies_unknown_reason() {
        let mut app = test_app();
        let connection_id = ConnectionId::from_raw(190);
        {
            let mut session = app.world_mut().resource_mut::<MyServerSession>();
            session.access_token = Some("access-token".to_string());
            session.character_id = Some("chr_1".to_string());
            session.ticket = Some(ticket_for_test("plr_1", "chr_1", "2026-06-25T12:15:00Z"));
            session.connection_id = Some(connection_id);
            session.connected = true;
            session.authenticated = true;
            session.game_connection_state = GameConnectionState::Authenticated;
        }

        app.world_mut().write_message(NetworkEvent::Packet {
            connection_id,
            transport: NetworkTransport::Tcp,
            payload: session_kick_packet("new_reason"),
        });
        app.update();

        assert!(disconnect_commands(&app).contains(&connection_id));
        {
            let session = app.world().resource::<MyServerSession>();
            assert!(session.reconnect_blocked);
            assert_eq!(
                session.game_connection_state,
                GameConnectionState::Disconnected
            );
        }
        assert!(
            read_messages::<MyServerEvent>(&app)
                .iter()
                .any(|event| matches!(
                    event,
                    MyServerEvent::SessionKicked {
                        category: SessionKickCategory::Unknown,
                        reason,
                        ..
                    } if reason == "new_reason"
                ))
        );

        app.world_mut().write_message(MyServerCommand::IssueTicket {
            reconnect_game: true,
        });
        app.update();

        assert_eq!(
            app.world()
                .resource::<MyServerSession>()
                .game_connection_state,
            GameConnectionState::ReconnectFailed
        );
        assert!(latest_connect_command(&app).is_none());
        assert!(display_errors(&app).iter().any(|error| {
            error.kind == MyServerErrorKind::SessionKicked
                && error.error_code.as_deref() == Some("SESSION_KICKED")
        }));
    }

    #[test]
    fn kick_push_updates_login_state_and_legacy_ui_events_by_category() {
        for (reason, expected_state, expected_event) in [
            (
                "duplicate_login",
                AccountLoginState::Expired,
                "account_status_blocked",
            ),
            (
                "account_banned",
                AccountLoginState::Blocked,
                "account_banned",
            ),
            (
                "maintenance_window",
                AccountLoginState::Blocked,
                "maintenance_blocked",
            ),
        ] {
            let mut app = test_app();
            let connection_id = ConnectionId::from_raw(194);
            {
                let mut session = app.world_mut().resource_mut::<MyServerSession>();
                session.account_login_state = AccountLoginState::LoggedIn;
                session.connection_id = Some(connection_id);
                session.connected = true;
                session.authenticated = true;
                session.game_connection_state = GameConnectionState::Authenticated;
            }

            app.world_mut().write_message(NetworkEvent::Packet {
                connection_id,
                transport: NetworkTransport::Tcp,
                payload: session_kick_packet(reason),
            });
            app.update();

            assert_eq!(
                app.world()
                    .resource::<MyServerSession>()
                    .account_login_state,
                expected_state,
                "{reason}"
            );
            let events = read_messages::<MyServerEvent>(&app);
            assert!(
                events.iter().any(|event| match (expected_event, event) {
                    ("account_status_blocked", MyServerEvent::AccountStatusBlocked { .. }) => true,
                    ("account_banned", MyServerEvent::AccountBanned { .. }) => true,
                    ("maintenance_blocked", MyServerEvent::MaintenanceBlocked { .. }) => true,
                    _ => false,
                }),
                "{reason}"
            );
            assert!(
                events
                    .iter()
                    .any(|event| matches!(event, MyServerEvent::SessionKickPush(_))),
                "{reason}"
            );
            assert!(
                events
                    .iter()
                    .any(|event| matches!(event, MyServerEvent::SessionKicked { .. })),
                "{reason}"
            );
        }
    }

    #[test]
    fn login_success_after_kick_unblocks_future_ticket_connection() {
        let mut app = test_app();
        {
            let mut session = app.world_mut().resource_mut::<MyServerSession>();
            session.reconnect_blocked = true;
            session.apply_login_response(&LoginResponse {
                ok: true,
                player_id: "plr_1".to_string(),
                guest_id: Some("guest-1".to_string()),
                login_name: None,
                access_token: "access-token".to_string(),
                refresh_token: None,
                access_token_expires_at: None,
                refresh_token_expires_at: None,
                ticket: None,
                ticket_expires_at: None,
                game_proxy_host: None,
                game_proxy_port: None,
                services: None,
            });
        }

        assert!(!app.world().resource::<MyServerSession>().reconnect_blocked);

        app.world_mut()
            .write_message(MyServerCommand::ConnectWithTicket {
                ticket: ticket_for_test("plr_1", "chr_1", "2026-06-25T12:20:00Z"),
                transport: NetworkTransport::Tcp,
                host: Some("game.test".to_string()),
                port: Some(14400),
            });
        app.update();

        assert!(latest_connect_command(&app).is_some());
        assert!(!display_errors(&app).iter().any(|error| {
            error.kind == MyServerErrorKind::SessionKicked
                && error.error_code.as_deref() == Some("SESSION_KICKED")
        }));
    }

    #[test]
    fn connect_rejects_legacy_ticket_without_character_id() {
        let mut app = test_app();
        let legacy_ticket = format!(
            "{}.signature",
            encode_base64url_for_test(
                br#"{"playerId":"plr_1","worldId":0,"exp":"2026-06-25T12:15:00Z","ver":1}"#
            )
        );
        app.world_mut()
            .write_message(MyServerCommand::ConnectWithTicket {
                ticket: legacy_ticket,
                transport: NetworkTransport::Tcp,
                host: Some("game.test".to_string()),
                port: Some(14400),
            });
        app.update();

        let session = app.world().resource::<MyServerSession>();
        assert_eq!(
            session.game_connection_state,
            GameConnectionState::ReconnectFailed
        );
        assert!(latest_connect_command(&app).is_none());
        assert!(
            read_messages::<MyServerEvent>(&app)
                .iter()
                .any(|event| matches!(
                    event,
                    MyServerEvent::GameAuthRejected {
                        error_code,
                        reason: GameAuthFailureReason::MissingCharacterId,
                    } if error_code == "MISSING_CHARACTER_ID"
                ))
        );
    }

    #[test]
    fn connect_then_auth_uses_current_ticket_and_accepts_success() {
        let mut app = test_app();
        let ticket = ticket_for_test("plr_1", "chr_1", "2026-06-25T12:20:00Z");
        app.world_mut()
            .write_message(MyServerCommand::ConnectWithTicket {
                ticket: ticket.clone(),
                transport: NetworkTransport::Tcp,
                host: Some("game.test".to_string()),
                port: Some(14400),
            });
        app.update();

        let (connection_id, _, _) = latest_connect_command(&app).unwrap();
        app.world_mut().write_message(NetworkEvent::Connected {
            connection_id,
            transport: NetworkTransport::Tcp,
            remote_addr: "game.test:14400".to_string(),
        });
        app.update();

        let packets = sent_packets(&app);
        let (_, payload) = packets.last().unwrap();
        let mut codec = super::super::protocol::PacketCodec::default();
        let packet = codec.push_bytes(payload).unwrap().remove(0);
        assert_eq!(packet.message_type(), Some(MessageType::AuthReq));
        assert_eq!(packet.decode::<pb::AuthReq>().unwrap().ticket, ticket);
        let seq = packet.header.seq;

        app.world_mut().write_message(NetworkEvent::Packet {
            connection_id,
            transport: NetworkTransport::Tcp,
            payload: auth_response_packet(seq, true, "plr_1", ""),
        });
        app.update();

        let session = app.world().resource::<MyServerSession>();
        assert!(session.authenticated);
        assert_eq!(session.player_id.as_deref(), Some("plr_1"));
        assert_eq!(session.character_id.as_deref(), Some("chr_1"));
        assert_eq!(
            session.game_connection_state,
            GameConnectionState::Authenticated
        );
        assert!(
            read_messages::<MyServerEvent>(&app)
                .iter()
                .any(|event| matches!(
                    event,
                    MyServerEvent::Authenticated { player_id } if player_id == "chr_1"
                ))
        );
    }

    #[test]
    fn auth_success_without_selected_character_is_rejected() {
        let mut app = test_app();
        let connection_id = ConnectionId::from_raw(241);
        {
            let mut session = app.world_mut().resource_mut::<MyServerSession>();
            session.connection_id = Some(connection_id);
            session.ticket = Some(ticket_for_test("plr_1", "chr_1", "2026-06-25T12:20:00Z"));
            session.begin_connect_game(connection_id, NetworkTransport::Tcp);
            session.game_connected(NetworkTransport::Tcp);
            session.begin_game_auth();
            session.pending.insert(
                7,
                PendingRequest {
                    response_type: MessageType::AuthRes,
                },
            );
        }

        app.world_mut().write_message(NetworkEvent::Packet {
            connection_id,
            transport: NetworkTransport::Tcp,
            payload: auth_response_packet(7, true, "plr_1", ""),
        });
        app.update();

        let session = app.world().resource::<MyServerSession>();
        assert!(!session.authenticated);
        assert_eq!(
            session.game_connection_state,
            GameConnectionState::Disconnected
        );
        let events = read_messages::<MyServerEvent>(&app);
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, MyServerEvent::Authenticated { .. }))
        );
        assert!(events.iter().any(|event| matches!(
            event,
            MyServerEvent::GameAuthRejected {
                error_code,
                reason: GameAuthFailureReason::MissingCharacterId,
            } if error_code == "MISSING_CHARACTER_ID"
        )));
    }

    #[test]
    fn auth_failure_codes_are_classified() {
        for (error_code, expected_reason) in [
            ("TICKET_EXPIRED", GameAuthFailureReason::TicketExpired),
            (
                "MISSING_CHARACTER_ID",
                GameAuthFailureReason::MissingCharacterId,
            ),
            ("ACCOUNT_BLOCKED", GameAuthFailureReason::AccountBlocked),
            ("CHARACTER_BLOCKED", GameAuthFailureReason::CharacterBlocked),
            ("SOMETHING_NEW", GameAuthFailureReason::Unknown),
        ] {
            let mut app = test_app();
            let ticket = ticket_for_test("plr_1", "chr_1", "2026-06-25T12:20:00Z");
            app.world_mut()
                .write_message(MyServerCommand::ConnectWithTicket {
                    ticket,
                    transport: NetworkTransport::Tcp,
                    host: Some("game.test".to_string()),
                    port: Some(14400),
                });
            app.update();

            let (connection_id, _, _) = latest_connect_command(&app).unwrap();
            app.world_mut().write_message(NetworkEvent::Connected {
                connection_id,
                transport: NetworkTransport::Tcp,
                remote_addr: "game.test:14400".to_string(),
            });
            app.update();
            let seq = {
                let packets = sent_packets(&app);
                let (_, payload) = packets.last().unwrap();
                let mut codec = super::super::protocol::PacketCodec::default();
                codec.push_bytes(payload).unwrap().remove(0).header.seq
            };

            app.world_mut().write_message(NetworkEvent::Packet {
                connection_id,
                transport: NetworkTransport::Tcp,
                payload: auth_response_packet(seq, false, "", error_code),
            });
            app.update();

            assert_eq!(
                app.world()
                    .resource::<MyServerSession>()
                    .game_connection_state,
                GameConnectionState::Disconnected
            );
            assert!(
                read_messages::<MyServerEvent>(&app)
                    .iter()
                    .any(|event| matches!(
                        event,
                        MyServerEvent::GameAuthRejected {
                            error_code: code,
                            reason,
                        } if code == error_code && *reason == expected_reason
                    ))
            );
        }
    }

    #[test]
    fn http_failures_emit_stable_display_error() {
        let mut app = test_app();
        app.world_mut().write_message(MyServerCommand::GuestLogin {
            guest_id: Some("guest-1".to_string()),
            connect_game: false,
        });
        app.update();

        let request = latest_http_request(&app).unwrap();
        app.world_mut()
            .write_message(NetworkEvent::HttpResponse(HttpResponse {
                request_id: request.request_id,
                status: 403,
                headers: Vec::new(),
                body: br#"{ "ok": false, "error": "IP_BLOCKED", "message": "blocked" }"#.to_vec(),
            }));
        app.update();

        let errors = display_errors(&app);
        assert!(errors.iter().any(|error| {
            error.kind == MyServerErrorKind::IpBlocked
                && error.source == MyServerErrorSource::Http
                && error.operation == Some(MyServerOperation::GuestLogin)
                && error.http_status == Some(403)
                && error.error_code.as_deref() == Some("IP_BLOCKED")
                && error.message_key == "myserver.error.ip_blocked"
                && error.blocking
        }));
    }

    #[test]
    fn http_ok_false_preserves_server_error_code_for_display_error() {
        let mut app = test_app();
        {
            let mut session = app.world_mut().resource_mut::<MyServerSession>();
            session.access_token = Some("access-token".to_string());
            session.character_id = Some("chr_1".to_string());
        }

        app.world_mut()
            .write_message(MyServerCommand::SelectCharacter {
                character_id: "chr_1".to_string(),
                connect_game: false,
            });
        app.update();

        let request = latest_http_request(&app).unwrap();
        app.world_mut()
            .write_message(NetworkEvent::HttpResponse(HttpResponse {
                request_id: request.request_id,
                status: 200,
                headers: Vec::new(),
                body: br#"{ "ok": false, "errorCode": "CHARACTER_BANNED", "message": "banned" }"#
                    .to_vec(),
            }));
        app.update();

        let errors = display_errors(&app);
        assert!(errors.iter().any(|error| {
            error.kind == MyServerErrorKind::CharacterUnavailable
                && error.source == MyServerErrorSource::Http
                && error.operation == Some(MyServerOperation::CharacterSelect)
                && error.error_code.as_deref() == Some("CHARACTER_BANNED")
                && error.message_key == "myserver.error.character_unavailable"
                && !error.blocking
        }));
    }

    #[test]
    fn json_parse_failures_emit_stable_display_error() {
        let mut app = test_app();
        app.world_mut().write_message(MyServerCommand::GuestLogin {
            guest_id: Some("guest-1".to_string()),
            connect_game: false,
        });
        app.update();

        let request = latest_http_request(&app).unwrap();
        app.world_mut()
            .write_message(NetworkEvent::HttpResponse(HttpResponse {
                request_id: request.request_id,
                status: 200,
                headers: Vec::new(),
                body: b"{".to_vec(),
            }));
        app.update();

        let errors = display_errors(&app);
        assert!(errors.iter().any(|error| {
            error.kind == MyServerErrorKind::JsonParseFailed
                && error.source == MyServerErrorSource::Protocol
                && error.operation == Some(MyServerOperation::GuestLogin)
                && error.message_key == "myserver.error.json_parse_failed"
        }));
    }

    #[test]
    fn transport_failures_emit_stable_display_error() {
        let mut app = test_app();
        let ticket = ticket_for_test("plr_1", "chr_1", "2026-06-25T12:20:00Z");
        app.world_mut()
            .write_message(MyServerCommand::ConnectWithTicket {
                ticket,
                transport: NetworkTransport::Tcp,
                host: Some("game.test".to_string()),
                port: Some(14400),
            });
        app.update();

        let (connection_id, _, _) = latest_connect_command(&app).unwrap();
        app.world_mut()
            .write_message(NetworkEvent::ConnectionFailed {
                connection_id,
                transport: NetworkTransport::Tcp,
                remote_addr: "game.test:14400".to_string(),
                error: "connect timeout after 10s".to_string(),
            });
        app.update();

        let errors = display_errors(&app);
        assert!(errors.iter().any(|error| {
            error.kind == MyServerErrorKind::ConnectionTimeout
                && error.source == MyServerErrorSource::Transport
                && error.operation == Some(MyServerOperation::GameConnect)
                && error.retryable
        }));
    }

    #[test]
    fn game_auth_and_domain_failures_emit_stable_display_errors() {
        let mut app = test_app();
        let ticket = ticket_for_test("plr_1", "chr_1", "2026-06-25T12:20:00Z");
        app.world_mut()
            .write_message(MyServerCommand::ConnectWithTicket {
                ticket,
                transport: NetworkTransport::Tcp,
                host: Some("game.test".to_string()),
                port: Some(14400),
            });
        app.update();

        let (connection_id, _, _) = latest_connect_command(&app).unwrap();
        app.world_mut().write_message(NetworkEvent::Connected {
            connection_id,
            transport: NetworkTransport::Tcp,
            remote_addr: "game.test:14400".to_string(),
        });
        app.update();
        let auth_seq = {
            let packets = sent_packets(&app);
            let (_, payload) = packets.last().unwrap();
            let mut codec = super::super::protocol::PacketCodec::default();
            codec.push_bytes(payload).unwrap().remove(0).header.seq
        };

        app.world_mut().write_message(NetworkEvent::Packet {
            connection_id,
            transport: NetworkTransport::Tcp,
            payload: auth_response_packet(auth_seq, false, "", "MISSING_CHARACTER_ID"),
        });
        app.update();

        let errors = display_errors(&app);
        assert!(errors.iter().any(|error| {
            error.kind == MyServerErrorKind::MissingCharacterId
                && error.message_type == Some(MessageType::AuthRes)
                && error.seq == Some(auth_seq)
                && error.error_code.as_deref() == Some("MISSING_CHARACTER_ID")
        }));

        let mut app = test_app();
        {
            let mut session = app.world_mut().resource_mut::<MyServerSession>();
            session.connection_id = Some(ConnectionId::from_raw(55));
            session.connected = true;
            session.authenticated = true;
            session.game_connection_state = GameConnectionState::Authenticated;
        }
        app.world_mut().write_message(MyServerCommand::JoinRoom {
            room_id: "room-1".to_string(),
            policy_id: "movement_demo".to_string(),
        });
        app.update();
        let join_seq = {
            let packets = sent_packets(&app);
            let (_, payload) = packets.last().unwrap();
            let mut codec = super::super::protocol::PacketCodec::default();
            codec.push_bytes(payload).unwrap().remove(0).header.seq
        };
        app.world_mut().write_message(NetworkEvent::Packet {
            connection_id: ConnectionId::from_raw(55),
            transport: NetworkTransport::Tcp,
            payload: room_join_response_packet(join_seq, false, "ROOM_FULL"),
        });
        app.update();

        let errors = display_errors(&app);
        assert!(errors.iter().any(|error| {
            error.kind == MyServerErrorKind::RoomJoinFailed
                && error.message_type == Some(MessageType::RoomJoinRes)
                && error.seq == Some(join_seq)
                && error.error_code.as_deref() == Some("ROOM_FULL")
        }));
    }

    #[test]
    fn protocol_decode_and_character_elements_failures_emit_display_errors() {
        let mut app = test_app();
        let connection_id = ConnectionId::from_raw(66);
        {
            let mut session = app.world_mut().resource_mut::<MyServerSession>();
            session.connection_id = Some(connection_id);
            session.connected = true;
            session.authenticated = true;
            session.game_connection_state = GameConnectionState::Authenticated;
        }

        app.world_mut()
            .resource_mut::<MyServerSession>()
            .pending
            .insert(
                9,
                PendingRequest {
                    response_type: MessageType::AuthRes,
                },
            );
        app.world_mut().write_message(NetworkEvent::Packet {
            connection_id,
            transport: NetworkTransport::Tcp,
            payload: encode_raw_packet(MessageType::AuthRes, 9, &[0x0a, 0xff]),
        });
        app.update();

        let errors = display_errors(&app);
        assert!(errors.iter().any(|error| {
            error.kind == MyServerErrorKind::ProtobufDecodeFailed
                && error.message_type == Some(MessageType::AuthRes)
                && error.seq == Some(9)
        }));

        app.world_mut()
            .resource_mut::<MyServerSession>()
            .pending
            .insert(
                10,
                PendingRequest {
                    response_type: MessageType::GetCharacterElementsRes,
                },
            );
        app.world_mut().write_message(NetworkEvent::Packet {
            connection_id,
            transport: NetworkTransport::Tcp,
            payload: character_elements_response_packet(10, false, "ELEMENTS_UNAVAILABLE"),
        });
        app.update();

        let errors = display_errors(&app);
        assert!(errors.iter().any(|error| {
            error.kind == MyServerErrorKind::CharacterElementsFailed
                && error.message_type == Some(MessageType::GetCharacterElementsRes)
                && error.seq == Some(10)
                && error.error_code.as_deref() == Some("ELEMENTS_UNAVAILABLE")
        }));
    }

    #[test]
    fn extracts_json_error_code_before_raw_body_for_non_2xx() {
        let error = http_error_message(
            &PendingHttpOperation::CharacterSelect {
                character_id: "chr_1".to_string(),
                connect_game: false,
            },
            409,
            br#"{ "ok": false, "errorCode": "CHARACTER_DELETED", "message": "deleted" }"#,
        );

        assert!(error.contains("HTTP 409"));
        assert!(error.contains("CHARACTER_DELETED: deleted"));
    }

    #[test]
    fn parses_error_alias_and_falls_back_to_raw_body() {
        assert_eq!(
            parse_api_error(br#"{ "error": "PLAYER_BLOCKED", "message": "blocked" }"#).as_deref(),
            Some("PLAYER_BLOCKED: blocked")
        );

        let error = http_error_message(
            &PendingHttpOperation::TicketIssue {
                reconnect_game: false,
            },
            504,
            b"gateway timeout",
        );
        assert!(error.contains("HTTP 504"));
        assert!(error.contains("gateway timeout"));
    }

    #[test]
    fn parses_register_pending_review_without_access_token() {
        let response = parse_register_response(
            br#"{
                "ok": true,
                "playerId": "plr_pending",
                "loginName": "alice",
                "displayName": "Alice",
                "status": "pending_review",
                "pendingReview": true,
                "message": "Registration submitted for review"
            }"#,
        )
        .unwrap();

        match response {
            RegisterResponse::PendingReview(response) => {
                assert!(response.ok);
                assert_eq!(response.player_id, "plr_pending");
                assert_eq!(response.login_name.as_deref(), Some("alice"));
                assert_eq!(response.display_name.as_deref(), Some("Alice"));
                assert!(response.pending_review);
                assert_eq!(
                    register_pending_review_code(&response),
                    "REGISTER_PENDING_REVIEW"
                );
                assert_eq!(
                    response.message.as_deref(),
                    Some("Registration submitted for review")
                );
            }
            RegisterResponse::Login(_) => panic!("pending review must not parse as login"),
        }
    }

    #[test]
    fn parses_register_success_as_login_response() {
        let response = parse_register_response(
            br#"{
                "ok": true,
                "playerId": "plr_1",
                "guestId": null,
                "loginName": "alice",
                "accessToken": "access",
                "ticket": null,
                "ticketExpiresAt": null
            }"#,
        )
        .unwrap();

        match response {
            RegisterResponse::Login(response) => {
                assert_eq!(response.player_id, "plr_1");
                assert_eq!(response.access_token, "access");
            }
            RegisterResponse::PendingReview(_) => {
                panic!("register success with accessToken must parse as login")
            }
        }
    }

    #[test]
    fn duplicate_pending_policy_rejects_same_operation_group() {
        let mut session = MyServerSession::default();
        session.pending_http.insert(
            RequestId::from_raw(42),
            PendingHttpRequest {
                operation: PendingHttpOperation::CharacterList,
            },
        );

        assert!(has_duplicate_pending_http(
            &session,
            &PendingHttpOperation::CharacterList
        ));
        assert!(!has_duplicate_pending_http(
            &session,
            &PendingHttpOperation::CharacterCreate
        ));
    }

    #[test]
    fn url_path_segment_escapes_profile_character_id() {
        assert_eq!(url_path_segment("chr a/b"), "chr%20a%2Fb");
    }
}
