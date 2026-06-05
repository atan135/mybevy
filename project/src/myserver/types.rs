use std::{collections::HashMap, env, time::Duration};

use bevy::prelude::{Message, Resource};
use serde::Deserialize;

use crate::network::{ConnectionId, NetworkTransport, RequestId};

use super::protocol::{MessageType, PacketCodec, pb};

pub const DEFAULT_AUTH_HTTP_BASE_URL: &str = "http://127.0.0.1:3000";
pub const DEFAULT_GAME_PROXY_HOST: &str = "127.0.0.1";
pub const DEFAULT_GAME_PROXY_KCP_PORT: u16 = 4000;
pub const DEFAULT_GAME_PROXY_TCP_FALLBACK_PORT: u16 = 14000;
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, Resource)]
pub struct MyServerConfig {
    pub http_base_url: String,
    pub game_host: String,
    pub kcp_port: u16,
    pub tcp_fallback_port: u16,
    pub prefer_transport: NetworkTransport,
    pub request_timeout: Duration,
    pub auto_reconnect_with_fresh_ticket: bool,
}

impl Default for MyServerConfig {
    fn default() -> Self {
        Self {
            http_base_url: env_string("MYSERVER_HTTP_BASE_URL", DEFAULT_AUTH_HTTP_BASE_URL),
            game_host: env_string("MYSERVER_GAME_HOST", DEFAULT_GAME_PROXY_HOST),
            kcp_port: env_u16("MYSERVER_KCP_PORT", DEFAULT_GAME_PROXY_KCP_PORT),
            tcp_fallback_port: env_u16(
                "MYSERVER_TCP_FALLBACK_PORT",
                DEFAULT_GAME_PROXY_TCP_FALLBACK_PORT,
            ),
            prefer_transport: env_transport("MYSERVER_TRANSPORT").unwrap_or(NetworkTransport::Tcp),
            request_timeout: Duration::from_millis(env_u64(
                "MYSERVER_REQUEST_TIMEOUT_MS",
                DEFAULT_REQUEST_TIMEOUT.as_millis() as u64,
            )),
            auto_reconnect_with_fresh_ticket: env_bool(
                "MYSERVER_AUTO_RECONNECT_WITH_FRESH_TICKET",
                false,
            ),
        }
    }
}

impl MyServerConfig {
    pub fn game_addr(&self, transport: NetworkTransport) -> String {
        match transport {
            NetworkTransport::Tcp => format!("{}:{}", self.game_host, self.tcp_fallback_port),
            NetworkTransport::Kcp => format!("{}:{}", self.game_host, self.kcp_port),
        }
    }
}

#[derive(Clone, Debug, Default, Resource)]
pub struct MyServerSession {
    pub access_token: Option<String>,
    pub ticket: Option<String>,
    pub ticket_expires_at: Option<String>,
    pub player_id: Option<String>,
    pub guest_id: Option<String>,
    pub login_name: Option<String>,
    pub connection_id: Option<ConnectionId>,
    pub transport: Option<NetworkTransport>,
    pub connected: bool,
    pub authenticated: bool,
    pub room_id: Option<String>,
    pub next_seq: u32,
    pub codec: PacketCodec,
    pub pending: HashMap<u32, PendingRequest>,
    pub login_request: Option<RequestId>,
    pub ticket_request: Option<RequestId>,
    pub connect_after_login: Option<ConnectPlan>,
}

impl MyServerSession {
    pub fn reserve_seq(&mut self) -> u32 {
        self.next_seq = self.next_seq.wrapping_add(1);
        if self.next_seq == 0 {
            self.next_seq = 1;
        }
        self.next_seq
    }

    pub fn reset_connection_state(&mut self) {
        self.connection_id = None;
        self.transport = None;
        self.connected = false;
        self.authenticated = false;
        self.room_id = None;
        self.codec.clear();
        self.pending.clear();
    }
}

#[derive(Clone, Debug)]
pub struct PendingRequest {
    pub response_type: MessageType,
}

#[derive(Clone, Debug)]
pub struct ConnectPlan {
    pub transport: NetworkTransport,
    pub host: Option<String>,
    pub port: Option<u16>,
}

#[derive(Clone, Debug, Message)]
pub enum MyServerCommand {
    GuestLogin {
        guest_id: Option<String>,
        connect_game: bool,
    },
    RefreshTicket {
        reconnect_game: bool,
    },
    ConnectWithTicket {
        ticket: String,
        transport: NetworkTransport,
        host: Option<String>,
        port: Option<u16>,
    },
    Disconnect,
    Ping {
        client_time_ms: i64,
    },
    JoinRoom {
        room_id: String,
        policy_id: String,
    },
    LeaveRoom,
    SetReady {
        ready: bool,
    },
    StartRoom,
    SendPlayerInput {
        frame_id: u32,
        action: String,
        payload_json: String,
    },
    SendMoveInput {
        frame_id: u32,
        input_type: pb::MoveInputType,
        dir_x: f32,
        dir_y: f32,
        client_state: Option<MovementClientState>,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct MovementClientState {
    pub x: f32,
    pub y: f32,
    pub frame_id: u32,
}

#[derive(Clone, Debug, Resource)]
pub struct MyServerAutoClientConfig {
    pub enabled: bool,
    pub guest_id: Option<String>,
    pub ping_after_auth: bool,
    pub join_after_auth: bool,
    pub room_id: String,
    pub policy_id: String,
}

impl Default for MyServerAutoClientConfig {
    fn default() -> Self {
        Self {
            enabled: env_bool("MYSERVER_AUTO_CONNECT", false),
            guest_id: env::var("MYSERVER_GUEST_ID")
                .ok()
                .filter(|value| !value.trim().is_empty()),
            ping_after_auth: env_bool("MYSERVER_AUTO_PING", true),
            join_after_auth: env_bool("MYSERVER_AUTO_JOIN", true),
            room_id: env_string("MYSERVER_AUTO_JOIN_ROOM", "room-default"),
            policy_id: env_string("MYSERVER_AUTO_JOIN_POLICY", "movement_demo"),
        }
    }
}

#[derive(Clone, Debug, Default, Resource)]
pub struct MyServerAutoClientState {
    pub ping_sent: bool,
    pub join_sent: bool,
}

#[derive(Clone, Debug, Message)]
pub enum MyServerEvent {
    LoginSucceeded(LoginSession),
    LoginFailed {
        error: String,
    },
    TicketRefreshed {
        ticket_expires_at: String,
    },
    TicketRefreshFailed {
        error: String,
    },
    Connecting {
        connection_id: ConnectionId,
        transport: NetworkTransport,
        remote_addr: String,
    },
    Connected {
        connection_id: ConnectionId,
        transport: NetworkTransport,
        remote_addr: String,
    },
    ConnectionFailed {
        transport: NetworkTransport,
        remote_addr: String,
        error: String,
    },
    Disconnected {
        reason: Option<String>,
    },
    Authenticated {
        player_id: String,
    },
    AuthFailed {
        error_code: String,
    },
    Pong(pb::PingRes),
    RoomJoined(pb::RoomJoinRes),
    RoomLeft(pb::RoomLeaveRes),
    ReadyChanged(pb::RoomReadyRes),
    RoomStarted(pb::RoomStartRes),
    PlayerInputAccepted(pb::PlayerInputRes),
    MoveInputAccepted(pb::MoveInputRes),
    RoomStatePush(pb::RoomStatePush),
    GameMessagePush(pb::GameMessagePush),
    FrameBundlePush(pb::FrameBundlePush),
    RoomFrameRatePush(pb::RoomFrameRatePush),
    RoomMemberOfflinePush(pb::RoomMemberOfflinePush),
    MovementSnapshotPush(pb::MovementSnapshotPush),
    MovementRejectPush(pb::MovementRejectPush),
    ServerRedirectPush(pb::ServerRedirectPush),
    SessionKickPush(pb::SessionKickPush),
    AuthorityMigrationStartPush(pb::AuthorityMigrationStartPush),
    AuthorityMigrationCompletePush(pb::AuthorityMigrationCompletePush),
    Error {
        seq: u32,
        error_code: String,
        message: String,
    },
    ProtocolError {
        error: String,
    },
    RequestFailed {
        seq: Option<u32>,
        message_type: Option<MessageType>,
        error: String,
    },
}

#[derive(Clone, Debug)]
pub struct LoginSession {
    pub player_id: String,
    pub access_token: String,
    pub ticket: String,
    pub ticket_expires_at: String,
    pub game_host: Option<String>,
    pub game_port: Option<u16>,
    pub game_transport: Option<NetworkTransport>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginResponse {
    pub ok: bool,
    pub player_id: String,
    pub guest_id: Option<String>,
    pub login_name: Option<String>,
    pub access_token: String,
    pub ticket: String,
    pub ticket_expires_at: String,
    pub game_proxy_host: Option<String>,
    pub game_proxy_port: Option<u16>,
    pub services: Option<ClientServices>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TicketResponse {
    pub ok: bool,
    pub player_id: String,
    pub ticket: String,
    pub ticket_expires_at: String,
    pub game_proxy_host: Option<String>,
    pub game_proxy_port: Option<u16>,
    pub services: Option<ClientServices>,
}

#[derive(Debug, Deserialize)]
pub struct ClientServices {
    pub game: Option<ClientServiceEndpoint>,
}

#[derive(Debug, Deserialize)]
pub struct ClientServiceEndpoint {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub protocol: Option<String>,
}

pub fn login_session_from_response(response: &LoginResponse) -> LoginSession {
    let (host, port, transport) = game_endpoint(
        response.game_proxy_host.clone(),
        response.game_proxy_port,
        response.services.as_ref(),
    );
    LoginSession {
        player_id: response.player_id.clone(),
        access_token: response.access_token.clone(),
        ticket: response.ticket.clone(),
        ticket_expires_at: response.ticket_expires_at.clone(),
        game_host: host,
        game_port: port,
        game_transport: transport,
    }
}

pub fn ticket_endpoint(
    response: &TicketResponse,
) -> (Option<String>, Option<u16>, Option<NetworkTransport>) {
    game_endpoint(
        response.game_proxy_host.clone(),
        response.game_proxy_port,
        response.services.as_ref(),
    )
}

fn game_endpoint(
    fallback_host: Option<String>,
    fallback_port: Option<u16>,
    services: Option<&ClientServices>,
) -> (Option<String>, Option<u16>, Option<NetworkTransport>) {
    let service = services.and_then(|services| services.game.as_ref());
    let host = service
        .and_then(|service| service.host.clone())
        .or(fallback_host);
    let port = service.and_then(|service| service.port).or(fallback_port);
    let transport = service
        .and_then(|service| service.protocol.as_deref())
        .and_then(parse_transport);
    (host, port, transport)
}

fn parse_transport(protocol: &str) -> Option<NetworkTransport> {
    match protocol.to_ascii_lowercase().as_str() {
        "tcp" => Some(NetworkTransport::Tcp),
        "kcp" => Some(NetworkTransport::Kcp),
        _ => None,
    }
}

fn env_string(name: &str, default: &str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_string())
}

fn env_bool(name: &str, default: bool) -> bool {
    env::var(name)
        .ok()
        .map(|value| {
            matches!(
                value.as_str(),
                "1" | "true" | "TRUE" | "True" | "yes" | "YES"
            )
        })
        .unwrap_or(default)
}

fn env_u16(name: &str, default: u16) -> u16 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(default)
}

fn env_u64(name: &str, default: u64) -> u64 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default)
}

fn env_transport(name: &str) -> Option<NetworkTransport> {
    env::var(name)
        .ok()
        .and_then(|value| parse_transport(&value))
}
