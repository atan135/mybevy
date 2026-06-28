use std::time::Duration;
use std::time::SystemTime;

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
    LoginResponse, MovementClientState, MyServerAutoClientConfig, MyServerAutoClientState,
    MyServerCommand, MyServerConfig, MyServerEvent, MyServerOperation, MyServerSession,
    PendingHttpOperation, PendingHttpRequest, PendingRequest, RegisterPendingReviewResponse,
    RegisterResponse, TicketResponse, character_select_endpoint, parse_character_bound_ticket,
    ticket_endpoint,
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
    mut state: ResMut<MyServerAutoClientState>,
    mut events: MessageReader<MyServerEvent>,
    mut commands: MessageWriter<MyServerCommand>,
) {
    if !config.enabled {
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
            }
            MyServerEvent::LoginFailed { error } => {
                error!(%error, "MyServer login failed");
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

                if config.ping_after_auth && !state.ping_sent {
                    state.ping_sent = true;
                    commands.write(MyServerCommand::Ping {
                        client_time_ms: current_unix_ms(),
                    });
                }

                if config.join_after_auth && !state.join_sent {
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
            _ => {}
        }
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
            } => connect_with_ticket(
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
            ),
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
                let operation = pending.operation.event_operation();
                write_http_failure(&mut events, &pending.operation, error.clone());
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
                session.connected = true;
                session.transport = Some(*transport);
                events.write(MyServerEvent::Connected {
                    connection_id: *connection_id,
                    transport: *transport,
                    remote_addr: remote_addr.clone(),
                });

                let Some(ticket) = session.ticket.clone() else {
                    events.write(MyServerEvent::RequestFailed {
                        seq: None,
                        message_type: Some(MessageType::AuthReq),
                        error: "connected without a ticket".to_string(),
                    });
                    continue;
                };

                send_auth_request(&mut session, &mut network_commands, &mut events, ticket);
            }
            NetworkEvent::ConnectionFailed {
                connection_id,
                transport,
                remote_addr,
                error,
            } if Some(*connection_id) == session.connection_id => {
                session.disconnect_cleanup();
                events.write(MyServerEvent::ConnectionFailed {
                    transport: *transport,
                    remote_addr: remote_addr.clone(),
                    error: error.clone(),
                });
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
                        events.write(MyServerEvent::ProtocolError { error });
                        continue;
                    }
                };

                for packet in packets {
                    handle_game_packet(&mut session, &mut events, packet);
                }
            }
            NetworkEvent::SendFailed {
                connection_id,
                error,
                ..
            } if Some(*connection_id) == session.connection_id => {
                events.write(MyServerEvent::RequestFailed {
                    seq: None,
                    message_type: None,
                    error: error.clone(),
                });
            }
            NetworkEvent::Disconnected {
                connection_id,
                reason,
                ..
            } if Some(*connection_id) == session.connection_id => {
                session.disconnect_cleanup();
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

    if !config.keepalive_enabled || !session.connected || !session.authenticated {
        state.timer.reset();
        return;
    }

    state.timer.tick(time.delta());
    if !state.timer.just_finished() {
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
        write_http_failure(
            events,
            &PendingHttpOperation::CharacterSelect { connect_game },
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
        PendingHttpOperation::CharacterSelect { connect_game },
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
    let Some(access_token) = session.access_token.clone() else {
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
        PendingHttpOperation::CharacterSelect { connect_game } => {
            session.connect_after_login = (*connect_game).then_some(ConnectPlan {
                transport: config.prefer_transport,
                host: None,
                port: None,
            });
        }
        PendingHttpOperation::TicketIssue { reconnect_game } => {
            session.ticket_request = Some(request_id);
            session.connect_after_login = (*reconnect_game).then_some(ConnectPlan {
                transport: config.prefer_transport,
                host: None,
                port: None,
            });
        }
        _ => {}
    }
    session
        .pending_http
        .insert(request_id, PendingHttpRequest { operation });
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
    operation: PendingHttpOperation,
    status: u16,
    body: &[u8],
) {
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
}

fn handle_character_list_response(
    session: &mut MyServerSession,
    events: &mut MessageWriter<MyServerEvent>,
    operation: PendingHttpOperation,
    status: u16,
    body: &[u8],
) {
    let Some(response) = parse_http_json::<CharacterListResponse>(events, &operation, status, body)
    else {
        return;
    };
    if !response.ok {
        write_http_failure(
            events,
            &operation,
            "character list returned ok=false".to_string(),
        );
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
        parse_http_json::<CharacterCreateResponse>(events, &operation, status, body)
    else {
        return;
    };
    if !response.ok {
        write_http_failure(
            events,
            &operation,
            "character create returned ok=false".to_string(),
        );
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
        parse_http_json::<CharacterProfileResponse>(events, &operation, status, body)
    else {
        return;
    };
    if !response.ok {
        write_http_failure(
            events,
            &operation,
            "character profile returned ok=false".to_string(),
        );
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
        parse_http_json::<CharacterSelectResponse>(events, &operation, status, body)
    else {
        return;
    };
    if !response.ok {
        write_http_failure(
            events,
            &operation,
            "character select returned ok=false".to_string(),
        );
        return;
    }
    let (host, port, transport) = character_select_endpoint(&response);
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
        parse_http_json::<CharacterLifecycleResponse>(events, &operation, status, body)
    else {
        return;
    };
    if !response.ok {
        write_http_failure(
            events,
            &operation,
            "character lifecycle returned ok=false".to_string(),
        );
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
        write_http_failure(events, operation, error.clone());
        events.write(MyServerEvent::NetworkFailed {
            operation: operation.event_operation(),
            error,
        });
        return None;
    }

    match serde_json::from_slice::<T>(body) {
        Ok(response) => Some(response),
        Err(error) => {
            let error = format!("failed to parse HTTP response JSON: {error}");
            write_http_failure(events, operation, error.clone());
            events.write(MyServerEvent::NetworkFailed {
                operation: operation.event_operation(),
                error,
            });
            None
        }
    }
}

fn parse_http_json_with<T, F>(
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
        write_http_failure(events, operation, error.clone());
        events.write(MyServerEvent::NetworkFailed {
            operation: operation.event_operation(),
            error,
        });
        return None;
    }

    match parser(body) {
        Ok(response) => Some(response),
        Err(error) => {
            let error = format!("failed to parse HTTP response JSON: {error}");
            write_http_failure(events, operation, error.clone());
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
    let response = serde_json::from_slice::<ApiErrorResponse>(body).ok()?;
    let code = response
        .error
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
        });
    let message = response.message.or_else(|| {
        response
            .extra
            .get("message")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
    });
    match (code, message) {
        (Some(code), Some(message)) if !message.trim().is_empty() => {
            Some(format!("{code}: {message}"))
        }
        (Some(code), _) => Some(code),
        (None, Some(message)) if !message.trim().is_empty() => Some(message),
        _ => None,
    }
}

fn write_http_failure(
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
    let Some(response) =
        parse_http_json_with(events, &operation, status, body, parse_register_response)
    else {
        return;
    };

    match response {
        RegisterResponse::Login(response) => {
            handle_login_success(config, session, network_commands, events, response);
        }
        RegisterResponse::PendingReview(response) => {
            session.connect_after_login = None;
            let code = register_pending_review_code(&response);
            let message = response
                .message
                .filter(|message| !message.trim().is_empty())
                .unwrap_or_else(|| "Registration submitted for review".to_string());
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
    let Some(response) = parse_http_json::<LoginResponse>(events, &operation, status, body) else {
        return;
    };

    if !response.ok {
        write_http_failure(events, &operation, "login returned ok=false".to_string());
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

    events.write(MyServerEvent::LoginSucceeded(login_session.clone()));

    let Some(ticket) = login_session.ticket else {
        if session.connect_after_login.take().is_some() {
            events.write(MyServerEvent::RequestFailed {
                seq: None,
                message_type: Some(MessageType::AuthReq),
                error:
                    "login only returned an account session; select a character before connecting"
                        .to_string(),
            });
        }
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
    let Some(response) = parse_http_json::<TicketResponse>(events, &operation, status, body) else {
        return;
    };

    if !response.ok {
        write_http_failure(
            events,
            &operation,
            "ticket issue returned ok=false".to_string(),
        );
        return;
    }

    let (host, port, transport) = ticket_endpoint(&response);
    session.apply_ticket_response(&response);
    events.write(MyServerEvent::TicketRefreshed {
        ticket_expires_at: response.ticket_expires_at.clone(),
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

fn connect_with_ticket(
    config: &MyServerConfig,
    session: &mut MyServerSession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<MyServerEvent>,
    ticket: String,
    plan: ConnectPlan,
) {
    let ticket_payload = match parse_character_bound_ticket(&ticket) {
        Ok(payload) => payload,
        Err(error) => {
            events.write(MyServerEvent::RequestFailed {
                seq: None,
                message_type: Some(MessageType::AuthReq),
                error: format!(
                    "refusing game connection with invalid character-bound ticket: {error}"
                ),
            });
            return;
        }
    };

    disconnect(session, network_commands);

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
    session.connection_id = Some(connection_id);
    session.transport = Some(plan.transport);
    session.connected = false;
    session.authenticated = false;
    session.codec.clear();
    session.pending.clear();

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

fn disconnect(session: &mut MyServerSession, network_commands: &mut MessageWriter<NetworkCommand>) {
    if let Some(connection_id) = session.connection_id {
        network_commands.write(NetworkCommand::Disconnect { connection_id });
    }
    session.reset_connection_state();
}

fn send_auth_request(
    session: &mut MyServerSession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<MyServerEvent>,
    ticket: String,
) {
    send_request(
        session,
        network_commands,
        events,
        MessageType::AuthReq,
        MessageType::AuthRes,
        &pb::AuthReq { ticket },
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
    let Some(connection_id) = session.connection_id else {
        events.write(MyServerEvent::RequestFailed {
            seq: None,
            message_type: Some(request_type),
            error: "game connection is not open".to_string(),
        });
        return;
    };

    let seq = session.reserve_seq();
    session
        .pending
        .insert(seq, PendingRequest { response_type });
    let payload = encode_proto_packet(request_type, seq, message);
    network_commands.write(NetworkCommand::Send {
        connection_id,
        payload,
    });
}

fn handle_game_packet(
    session: &mut MyServerSession,
    events: &mut MessageWriter<MyServerEvent>,
    packet: Packet,
) {
    let Some(message_type) = packet.message_type() else {
        events.write(MyServerEvent::ProtocolError {
            error: format!("unknown msgType {}", packet.header.msg_type),
        });
        return;
    };

    if message_type == MessageType::ErrorRes {
        match packet.decode::<pb::ErrorRes>() {
            Ok(error) => {
                session.pending.remove(&packet.header.seq);
                events.write(MyServerEvent::Error {
                    seq: packet.header.seq,
                    error_code: error.error_code,
                    message: error.message,
                });
            }
            Err(error) => {
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
        MessageType::ServerRedirectPush => decode_push::<pb::ServerRedirectPush, _>(
            events,
            &packet,
            MyServerEvent::ServerRedirectPush,
        ),
        MessageType::SessionKickPush => {
            decode_push::<pb::SessionKickPush, _>(events, &packet, MyServerEvent::SessionKickPush)
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
        _ => handle_response_packet(session, events, message_type, packet),
    }
}

fn handle_response_packet(
    session: &mut MyServerSession,
    events: &mut MessageWriter<MyServerEvent>,
    message_type: MessageType,
    packet: Packet,
) {
    let Some(pending) = session.pending.remove(&packet.header.seq) else {
        events.write(MyServerEvent::ProtocolError {
            error: format!(
                "received response {:?} for unknown seq {}",
                message_type, packet.header.seq
            ),
        });
        return;
    };

    if pending.response_type != message_type {
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
                session.authenticated = true;
                session.player_id = Some(response.player_id.clone());
                events.write(MyServerEvent::Authenticated {
                    player_id: response.player_id,
                });
            }
            Ok(response) => {
                session.authenticated = false;
                events.write(MyServerEvent::AuthFailed {
                    error_code: response.error_code,
                });
            }
            Err(error) => {
                events.write(MyServerEvent::ProtocolError { error });
            }
        },
        MessageType::PingRes => decode_push::<pb::PingRes, _>(events, &packet, MyServerEvent::Pong),
        MessageType::RoomJoinRes => match packet.decode::<pb::RoomJoinRes>() {
            Ok(response) => {
                if response.ok {
                    session.room_id = Some(response.room_id.clone());
                }
                events.write(MyServerEvent::RoomJoined(response));
            }
            Err(error) => {
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
    match packet.decode::<M>() {
        Ok(message) => {
            events.write(event_factory(message));
        }
        Err(error) => {
            events.write(MyServerEvent::ProtocolError { error });
        }
    }
}

fn handle_character_elements_response(
    session: &mut MyServerSession,
    events: &mut MessageWriter<MyServerEvent>,
    packet: Packet,
) {
    match packet.decode::<pb::GetCharacterElementsRes>() {
        Ok(response) => {
            if let Some(cache) =
                session.apply_character_elements_response(&response, SystemTime::now())
            {
                events.write(MyServerEvent::CharacterElementsCacheUpdated(cache));
            }
            events.write(MyServerEvent::CharacterElementsLoaded(response));
        }
        Err(error) => {
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

    use serde_json::Value;

    use super::*;
    use crate::framework::network::HttpMethod;

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
    fn extracts_json_error_code_before_raw_body_for_non_2xx() {
        let error = http_error_message(
            &PendingHttpOperation::CharacterSelect {
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
