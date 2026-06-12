use std::time::Duration;

use bevy::prelude::*;

use crate::network::{
    ConnectionId, HttpRequest, KcpConnectConfig, KcpSessionOptions, NetworkCommand, NetworkEvent,
    NetworkTransport, TcpConnectConfig,
};

use super::protocol::{MessageType, Packet, encode_proto_packet, pb};
use super::types::{
    ConnectPlan, DEFAULT_KEEPALIVE_INTERVAL, LoginResponse, MovementClientState,
    MyServerAutoClientConfig, MyServerAutoClientState, MyServerCommand, MyServerConfig,
    MyServerEvent, MyServerSession, PendingRequest, TicketResponse, login_session_from_response,
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
            MyServerCommand::GuestLogin {
                guest_id,
                connect_game,
            } => send_guest_login(
                &config,
                &mut session,
                &mut network_commands,
                guest_id.as_deref(),
                *connect_game,
            ),
            MyServerCommand::RefreshTicket { reconnect_game } => send_refresh_ticket(
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
            NetworkEvent::HttpResponse(response)
                if Some(response.request_id) == session.login_request =>
            {
                session.login_request = None;
                handle_login_response(
                    &config,
                    &mut session,
                    &mut network_commands,
                    &mut events,
                    response.status,
                    &response.body,
                );
            }
            NetworkEvent::HttpError { request_id, error }
                if Some(*request_id) == session.login_request =>
            {
                session.login_request = None;
                events.write(MyServerEvent::LoginFailed {
                    error: error.clone(),
                });
            }
            NetworkEvent::HttpResponse(response)
                if Some(response.request_id) == session.ticket_request =>
            {
                session.ticket_request = None;
                handle_ticket_response(
                    &config,
                    &mut session,
                    &mut network_commands,
                    &mut events,
                    response.status,
                    &response.body,
                );
            }
            NetworkEvent::HttpError { request_id, error }
                if Some(*request_id) == session.ticket_request =>
            {
                session.ticket_request = None;
                events.write(MyServerEvent::TicketRefreshFailed {
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
                session.reset_connection_state();
                events.write(MyServerEvent::ConnectionFailed {
                    transport: *transport,
                    remote_addr: remote_addr.clone(),
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
                session.reset_connection_state();
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

fn send_guest_login(
    config: &MyServerConfig,
    session: &mut MyServerSession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    guest_id: Option<&str>,
    connect_game: bool,
) {
    let url = format!(
        "{}/api/v1/auth/guest-login",
        config.http_base_url.trim_end_matches('/')
    );
    let body = match guest_id {
        Some(guest_id) if !guest_id.trim().is_empty() => {
            serde_json::json!({ "guestId": guest_id }).to_string()
        }
        _ => "{}".to_string(),
    };
    let request = HttpRequest::post(url, body)
        .with_header("Content-Type", "application/json")
        .with_header("Accept", "application/json")
        .with_timeout(config.request_timeout);

    session.login_request = Some(request.request_id);
    session.connect_after_login = connect_game.then_some(ConnectPlan {
        transport: config.prefer_transport,
        host: None,
        port: None,
    });
    network_commands.write(NetworkCommand::Http(request));
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
        return;
    };

    let url = format!(
        "{}/api/v1/game-ticket/issue",
        config.http_base_url.trim_end_matches('/')
    );
    let request = HttpRequest::post(url, "{}")
        .with_header("Content-Type", "application/json")
        .with_header("Accept", "application/json")
        .with_header("Authorization", format!("Bearer {access_token}"))
        .with_timeout(config.request_timeout);

    session.ticket_request = Some(request.request_id);
    session.connect_after_login = reconnect_game.then_some(ConnectPlan {
        transport: config.prefer_transport,
        host: None,
        port: None,
    });
    network_commands.write(NetworkCommand::Http(request));
}

fn handle_login_response(
    config: &MyServerConfig,
    session: &mut MyServerSession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<MyServerEvent>,
    status: u16,
    body: &[u8],
) {
    if !(200..300).contains(&status) {
        events.write(MyServerEvent::LoginFailed {
            error: format!("guest login returned HTTP {status}: {}", body_text(body)),
        });
        return;
    }

    let response = match serde_json::from_slice::<LoginResponse>(body) {
        Ok(response) => response,
        Err(error) => {
            events.write(MyServerEvent::LoginFailed {
                error: format!("failed to parse guest login response: {error}"),
            });
            return;
        }
    };

    if !response.ok {
        events.write(MyServerEvent::LoginFailed {
            error: "guest login returned ok=false".to_string(),
        });
        return;
    }

    let login_session = login_session_from_response(&response);
    session.access_token = Some(response.access_token);
    session.ticket = Some(response.ticket);
    session.ticket_expires_at = Some(response.ticket_expires_at);
    session.player_id = Some(response.player_id);
    session.guest_id = response.guest_id;
    session.login_name = response.login_name;

    events.write(MyServerEvent::LoginSucceeded(login_session.clone()));

    if let Some(mut plan) = session.connect_after_login.take() {
        apply_discovered_endpoint(
            &mut plan,
            login_session.game_host,
            login_session.game_port,
            login_session.game_transport,
            config,
        );
        connect_with_ticket(
            config,
            session,
            network_commands,
            events,
            login_session.ticket,
            plan,
        );
    }
}

fn handle_ticket_response(
    config: &MyServerConfig,
    session: &mut MyServerSession,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<MyServerEvent>,
    status: u16,
    body: &[u8],
) {
    if !(200..300).contains(&status) {
        events.write(MyServerEvent::TicketRefreshFailed {
            error: format!("ticket issue returned HTTP {status}: {}", body_text(body)),
        });
        return;
    }

    let response = match serde_json::from_slice::<TicketResponse>(body) {
        Ok(response) => response,
        Err(error) => {
            events.write(MyServerEvent::TicketRefreshFailed {
                error: format!("failed to parse ticket response: {error}"),
            });
            return;
        }
    };

    if !response.ok {
        events.write(MyServerEvent::TicketRefreshFailed {
            error: "ticket issue returned ok=false".to_string(),
        });
        return;
    }

    let (host, port, transport) = ticket_endpoint(&response);
    session.player_id = Some(response.player_id);
    session.ticket = Some(response.ticket.clone());
    session.ticket_expires_at = Some(response.ticket_expires_at.clone());
    events.write(MyServerEvent::TicketRefreshed {
        ticket_expires_at: response.ticket_expires_at,
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
    disconnect(session, network_commands);

    let connection_id = ConnectionId::new();
    let host = plan.host.unwrap_or_else(|| config.game_host.clone());
    let port = plan.port.unwrap_or(match plan.transport {
        NetworkTransport::Tcp => config.tcp_fallback_port,
        NetworkTransport::Kcp => config.kcp_port,
    });
    let remote_addr = format!("{host}:{port}");

    session.ticket = Some(ticket);
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

fn body_text(body: &[u8]) -> String {
    String::from_utf8_lossy(body).into_owned()
}

fn current_unix_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}
