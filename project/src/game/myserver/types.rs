#![allow(dead_code)]

use std::{collections::HashMap, env, time::Duration};

use bevy::prelude::{Message, Resource};
use serde::Deserialize;
use serde_json::Value;

use crate::framework::network::{ConnectionId, NetworkTransport, RequestId};

use super::protocol::{MessageType, PacketCodec, pb};

pub const DEFAULT_AUTH_HTTP_BASE_URL: &str = "http://127.0.0.1:3000";
pub const DEFAULT_GAME_PROXY_HOST: &str = "127.0.0.1";
pub const DEFAULT_GAME_PROXY_KCP_PORT: u16 = 4000;
pub const DEFAULT_GAME_PROXY_TCP_FALLBACK_PORT: u16 = 14000;
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
pub const DEFAULT_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, Resource)]
pub struct MyServerConfig {
    pub http_base_url: String,
    pub game_host: String,
    pub kcp_port: u16,
    pub tcp_fallback_port: u16,
    pub prefer_transport: NetworkTransport,
    pub forced_transport: Option<NetworkTransport>,
    pub request_timeout: Duration,
    pub auto_reconnect_with_fresh_ticket: bool,
    pub keepalive_enabled: bool,
    pub keepalive_interval: Duration,
}

impl Default for MyServerConfig {
    fn default() -> Self {
        let forced_transport = env_transport("MYSERVER_TRANSPORT");
        Self {
            http_base_url: env_string("MYSERVER_HTTP_BASE_URL", DEFAULT_AUTH_HTTP_BASE_URL),
            game_host: env_string("MYSERVER_GAME_HOST", DEFAULT_GAME_PROXY_HOST),
            kcp_port: env_u16("MYSERVER_KCP_PORT", DEFAULT_GAME_PROXY_KCP_PORT),
            tcp_fallback_port: env_u16(
                "MYSERVER_TCP_FALLBACK_PORT",
                DEFAULT_GAME_PROXY_TCP_FALLBACK_PORT,
            ),
            prefer_transport: forced_transport.unwrap_or(NetworkTransport::Tcp),
            forced_transport,
            request_timeout: Duration::from_millis(env_u64(
                "MYSERVER_REQUEST_TIMEOUT_MS",
                DEFAULT_REQUEST_TIMEOUT.as_millis() as u64,
            )),
            auto_reconnect_with_fresh_ticket: env_bool(
                "MYSERVER_AUTO_RECONNECT_WITH_FRESH_TICKET",
                false,
            ),
            keepalive_enabled: env_bool("MYSERVER_KEEPALIVE", true),
            keepalive_interval: Duration::from_millis(env_u64(
                "MYSERVER_KEEPALIVE_INTERVAL_MS",
                DEFAULT_KEEPALIVE_INTERVAL.as_millis() as u64,
            )),
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
    pub character_id: Option<String>,
    pub world_id: Option<i64>,
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
    CharacterElementsLoaded(pb::GetCharacterElementsRes),
    CharacterElementsChanged(pb::CharacterElementsChangePush),
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
    pub ticket: Option<String>,
    pub ticket_expires_at: Option<String>,
    pub game_host: Option<String>,
    pub game_port: Option<u16>,
    pub game_transport: Option<NetworkTransport>,
}

/// Account session response shared by password login/register and guest login.
///
/// New auth-http builds no longer issue an enter-game ticket here. A client must
/// list/create/select a character before connecting to game-proxy. `ticket` is
/// optional to keep older local servers parseable during transition.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginResponse {
    pub ok: bool,
    pub player_id: String,
    pub guest_id: Option<String>,
    pub login_name: Option<String>,
    pub access_token: String,
    pub ticket: Option<String>,
    pub ticket_expires_at: Option<String>,
    pub game_proxy_host: Option<String>,
    pub game_proxy_port: Option<u16>,
    pub services: Option<ClientServices>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterListResponse {
    pub ok: bool,
    pub player_id: String,
    #[serde(default)]
    pub characters: Vec<CharacterSummary>,
}

#[derive(Debug, Deserialize)]
pub struct CharacterCreateResponse {
    pub ok: bool,
    pub character: CharacterSummary,
}

#[derive(Debug, Deserialize)]
pub struct CharacterProfileResponse {
    pub ok: bool,
    pub profile: CharacterProfile,
}

#[derive(Debug, Deserialize)]
pub struct CharacterLifecycleResponse {
    pub ok: bool,
    pub character: CharacterSummary,
    pub lifecycle: CharacterLifecycle,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterSelectResponse {
    pub ok: bool,
    pub player_id: String,
    pub character: CharacterSummary,
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
    pub character_id: Option<String>,
    pub world_id: Option<i64>,
    pub ticket: String,
    pub ticket_expires_at: String,
    pub game_proxy_host: Option<String>,
    pub game_proxy_port: Option<u16>,
    pub services: Option<ClientServices>,
}

#[derive(Debug, Deserialize)]
pub struct CharacterSummary {
    pub character_id: String,
    #[serde(default)]
    pub character_id_short: Option<String>,
    #[serde(default)]
    pub display_discriminator: Option<String>,
    #[serde(default)]
    pub same_name_hint: Option<SameNameHint>,
    pub name: String,
    #[serde(default)]
    pub world_id: Option<i64>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub appearance_json: Option<Value>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub last_login_at: Option<String>,
    #[serde(default)]
    pub deleted_at: Option<String>,
    #[serde(default)]
    pub position: Option<CharacterPosition>,
    #[serde(default)]
    pub attributes: Option<CharacterAttributes>,
    #[serde(default)]
    pub lifecycle: Option<CharacterLifecycle>,
    #[serde(default, flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Deserialize)]
pub struct CharacterProfile {
    #[serde(flatten)]
    pub character: CharacterSummary,
    #[serde(default)]
    pub same_name: Option<SameNameInfo>,
    #[serde(default)]
    pub equipped_title: Option<Value>,
    #[serde(default)]
    pub discipline: Option<Value>,
    #[serde(default)]
    pub profile_sources: Option<HashMap<String, Value>>,
}

#[derive(Debug, Deserialize)]
pub struct SameNameHint {
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SameNameInfo {
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub world_id: Option<i64>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub count: Option<u32>,
    #[serde(default)]
    pub has_duplicates: Option<bool>,
    #[serde(default)]
    pub discriminator: Option<SameNameHint>,
}

#[derive(Debug, Deserialize)]
pub struct CharacterPosition {
    #[serde(default)]
    pub scene_id: Option<i64>,
    #[serde(default)]
    pub x: Option<f32>,
    #[serde(default)]
    pub y: Option<f32>,
    #[serde(default)]
    pub dir_x: Option<f32>,
    #[serde(default)]
    pub dir_y: Option<f32>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct ElementValues {
    #[serde(default)]
    pub earth: i32,
    #[serde(default)]
    pub fire: i32,
    #[serde(default)]
    pub water: i32,
    #[serde(default)]
    pub wind: i32,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct CharacterElements {
    #[serde(default)]
    pub affinity: ElementValues,
    #[serde(default)]
    pub mastery: ElementValues,
}

#[derive(Debug, Deserialize)]
pub struct CharacterAttributes {
    #[serde(default)]
    pub affinity: ElementValues,
    #[serde(default)]
    pub mastery: ElementValues,
}

#[derive(Debug, Deserialize)]
pub struct CharacterLifecycle {
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub deleted_at: Option<String>,
    #[serde(default)]
    pub restore_window_seconds: Option<i64>,
    #[serde(default)]
    pub restore_expires_at: Option<String>,
    #[serde(default)]
    pub delete_cooldown_seconds: Option<i64>,
    #[serde(default)]
    pub hard_delete_eligible_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CharacterPushMetaJson {
    pub character_id: String,
    #[serde(default)]
    pub sequence: u64,
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub source_type: Option<String>,
    #[serde(default)]
    pub source_id: Option<String>,
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub snapshot_compensation: bool,
}

#[derive(Debug, Deserialize)]
pub struct CharacterElementsChangePayload {
    pub meta: CharacterPushMetaJson,
    #[serde(default)]
    pub before: CharacterElements,
    #[serde(default)]
    pub change: Option<CharacterElements>,
    #[serde(default)]
    pub after: CharacterElements,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiErrorResponse {
    #[serde(default)]
    pub ok: Option<bool>,
    #[serde(default, alias = "errorCode")]
    pub error: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default, flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Deserialize)]
pub struct ClientServices {
    pub game: Option<ClientServiceEndpoint>,
    #[serde(default)]
    pub chat: Option<ClientServiceEndpoint>,
    #[serde(default)]
    pub mail: Option<ClientServiceEndpoint>,
    #[serde(default)]
    pub announce: Option<ClientServiceEndpoint>,
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

pub fn character_select_endpoint(
    response: &CharacterSelectResponse,
) -> (Option<String>, Option<u16>, Option<NetworkTransport>) {
    game_endpoint(
        response.game_proxy_host.clone(),
        response.game_proxy_port,
        response.services.as_ref(),
    )
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameTicketPayload {
    pub player_id: String,
    pub character_id: String,
    pub world_id: Option<i64>,
    pub exp: String,
    pub ver: Option<i64>,
    pub ticket_fingerprint: String,
    pub payload_fingerprint: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawGameTicketPayload {
    player_id: Option<String>,
    character_id: Option<String>,
    world_id: Option<i64>,
    exp: Option<String>,
    ver: Option<i64>,
}

pub fn parse_character_bound_ticket(ticket: &str) -> Result<GameTicketPayload, String> {
    let (payload_part, _) = ticket
        .split_once('.')
        .ok_or_else(|| "INVALID_TICKET_FORMAT".to_string())?;
    if payload_part.is_empty() {
        return Err("INVALID_TICKET_FORMAT".to_string());
    }

    let payload_bytes = decode_base64url(payload_part)?;
    let raw: RawGameTicketPayload = serde_json::from_slice(&payload_bytes)
        .map_err(|error| format!("INVALID_TICKET_PAYLOAD: {error}"))?;

    let player_id = raw
        .player_id
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "MISSING_PLAYER_ID".to_string())?;
    let character_id = raw
        .character_id
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "MISSING_CHARACTER_ID".to_string())?;
    let exp = raw
        .exp
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "MISSING_TICKET_EXP".to_string())?;

    Ok(GameTicketPayload {
        player_id,
        character_id,
        world_id: raw.world_id,
        exp,
        ver: raw.ver,
        ticket_fingerprint: short_fingerprint(ticket.as_bytes()),
        payload_fingerprint: short_fingerprint(&payload_bytes),
    })
}

fn decode_base64url(input: &str) -> Result<Vec<u8>, String> {
    let mut bits = 0u32;
    let mut bit_len = 0u8;
    let mut output = Vec::with_capacity(input.len() * 3 / 4);

    for byte in input.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            b'=' => break,
            _ => return Err("INVALID_TICKET_PAYLOAD_BASE64".to_string()),
        } as u32;

        bits = (bits << 6) | value;
        bit_len += 6;

        while bit_len >= 8 {
            bit_len -= 8;
            output.push(((bits >> bit_len) & 0xff) as u8);
        }
    }

    Ok(output)
}

fn short_fingerprint(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")[..12].to_string()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_account_login_without_game_ticket() {
        let response: LoginResponse = serde_json::from_str(
            r#"{
                "ok": true,
                "playerId": "plr_1",
                "guestId": "guest-a",
                "loginName": null,
                "accessToken": "access",
                "ticket": null,
                "ticketExpiresAt": null,
                "gameProxyHost": "127.0.0.1",
                "gameProxyPort": 14000,
                "services": { "game": { "host": "127.0.0.1", "port": 14000, "protocol": "tcp" } }
            }"#,
        )
        .unwrap();

        assert!(response.ok);
        assert_eq!(response.player_id, "plr_1");
        assert_eq!(response.access_token, "access");
        assert!(response.ticket.is_none());
        assert!(response.ticket_expires_at.is_none());
    }

    #[test]
    fn parses_character_list_and_empty_character_list() {
        let response: CharacterListResponse = serde_json::from_str(
            r#"{
                "ok": true,
                "playerId": "plr_1",
                "characters": [{
                    "character_id": "chr_0000000000001",
                    "character_id_short": "00000001",
                    "display_discriminator": "00000001",
                    "same_name_hint": {
                        "type": "character_id_short",
                        "value": "00000001",
                        "source": "characters.character_id"
                    },
                    "name": "WindRunner",
                    "world_id": 0,
                    "status": "active",
                    "appearance_json": { "body": "default" },
                    "created_at": "2026-06-25T12:00:00.000Z",
                    "last_login_at": null,
                    "deleted_at": null,
                    "position": { "scene_id": 100, "x": 0, "y": 0, "dir_x": 0, "dir_y": 1 },
                    "attributes": {
                        "affinity": { "earth": 2500, "fire": 2500, "water": 2500, "wind": 2500 },
                        "mastery": { "earth": 0, "fire": 0, "water": 0, "wind": 0 }
                    }
                }]
            }"#,
        )
        .unwrap();

        assert_eq!(response.characters.len(), 1);
        assert_eq!(response.characters[0].character_id, "chr_0000000000001");
        assert_eq!(
            response.characters[0]
                .attributes
                .as_ref()
                .unwrap()
                .affinity
                .fire,
            2500
        );

        let empty: CharacterListResponse =
            serde_json::from_str(r#"{ "ok": true, "playerId": "plr_1", "characters": [] }"#)
                .unwrap();
        assert!(empty.characters.is_empty());
    }

    #[test]
    fn parses_created_character_with_missing_optional_fields_and_unknown_extra() {
        let response: CharacterCreateResponse = serde_json::from_str(
            r#"{
                "ok": true,
                "character": {
                    "character_id": "chr_0000000000002",
                    "name": "NewRole",
                    "future_field": "ignored"
                }
            }"#,
        )
        .unwrap();

        assert_eq!(response.character.character_id, "chr_0000000000002");
        assert_eq!(
            response
                .character
                .extra
                .get("future_field")
                .and_then(Value::as_str),
            Some("ignored")
        );
        assert!(response.character.position.is_none());
    }

    #[test]
    fn parses_character_select_ticket_response() {
        let response: CharacterSelectResponse = serde_json::from_str(
            r#"{
                "ok": true,
                "playerId": "plr_1",
                "character": { "character_id": "chr_0000000000001", "name": "WindRunner" },
                "ticket": "payload.signature",
                "ticketExpiresAt": "2026-06-25T12:15:00.000Z",
                "gameProxyHost": "127.0.0.1",
                "gameProxyPort": 14000,
                "services": { "game": { "host": "127.0.0.1", "port": 14000, "protocol": "tcp" } }
            }"#,
        )
        .unwrap();

        assert_eq!(response.player_id, "plr_1");
        assert_eq!(response.character.character_id, "chr_0000000000001");
        assert_eq!(response.ticket_expires_at, "2026-06-25T12:15:00.000Z");
        assert_eq!(character_select_endpoint(&response).1, Some(14000));
    }

    #[test]
    fn parses_game_ticket_issue_response() {
        let response: TicketResponse = serde_json::from_str(
            r#"{
                "ok": true,
                "playerId": "plr_1",
                "characterId": "chr_0000000000001",
                "worldId": 0,
                "ticket": "payload.signature",
                "ticketExpiresAt": "2026-06-25T12:15:00.000Z",
                "services": { "game": { "host": "127.0.0.1", "port": 4000, "protocol": "kcp" } }
            }"#,
        )
        .unwrap();

        assert_eq!(response.character_id.as_deref(), Some("chr_0000000000001"));
        assert_eq!(response.world_id, Some(0));
        assert_eq!(ticket_endpoint(&response).2, Some(NetworkTransport::Kcp));
    }

    #[test]
    fn parses_lifecycle_profile_and_error_response() {
        let profile: CharacterProfileResponse = serde_json::from_str(
            r#"{
                "ok": true,
                "profile": {
                    "character_id": "chr_0000000000001",
                    "name": "WindRunner",
                    "lifecycle": {
                        "state": "active",
                        "deleted_at": null,
                        "restore_window_seconds": 2592000,
                        "restore_expires_at": null,
                        "delete_cooldown_seconds": 2592000,
                        "hard_delete_eligible_at": null
                    },
                    "profile_sources": { "equipped_title": "not_connected" }
                }
            }"#,
        )
        .unwrap();

        assert_eq!(
            profile
                .profile
                .character
                .lifecycle
                .as_ref()
                .unwrap()
                .state
                .as_deref(),
            Some("active")
        );

        let error: ApiErrorResponse = serde_json::from_str(
            r#"{ "ok": false, "error": "CHARACTER_NOT_FOUND", "message": "missing", "detail": 1 }"#,
        )
        .unwrap();
        assert_eq!(error.error.as_deref(), Some("CHARACTER_NOT_FOUND"));
        assert_eq!(error.extra.get("detail").and_then(Value::as_i64), Some(1));
    }

    #[test]
    fn parses_character_elements_change_payload() {
        let payload: CharacterElementsChangePayload = serde_json::from_str(
            r#"{
                "meta": {
                    "character_id": "chr_0000000000001",
                    "sequence": 7,
                    "revision": 3,
                    "source_type": "item_use",
                    "source_id": "item_id:1",
                    "action": "element_change",
                    "summary": "debug",
                    "snapshot_compensation": false
                },
                "before": {
                    "affinity": { "earth": 2500, "fire": 2500, "water": 2500, "wind": 2500 },
                    "mastery": { "earth": 0, "fire": 0, "water": 0, "wind": 0 }
                },
                "change": {
                    "affinity": { "earth": -100, "fire": 100, "water": 0, "wind": 0 },
                    "mastery": { "earth": 0, "fire": 5, "water": 0, "wind": 0 }
                },
                "after": {
                    "affinity": { "earth": 2400, "fire": 2600, "water": 2500, "wind": 2500 },
                    "mastery": { "earth": 0, "fire": 5, "water": 0, "wind": 0 }
                }
            }"#,
        )
        .unwrap();

        assert_eq!(payload.meta.character_id, "chr_0000000000001");
        assert_eq!(payload.change.unwrap().mastery.fire, 5);
        assert_eq!(payload.after.affinity.fire, 2600);
    }

    #[test]
    fn rejects_legacy_ticket_without_character_id() {
        let ticket = format!(
            "{}.signature",
            encode_base64url_for_test(
                br#"{"playerId":"plr_1","worldId":0,"exp":"2026-06-25T12:15:00.000Z","ver":1}"#
            )
        );

        let error = parse_character_bound_ticket(&ticket).unwrap_err();
        assert_eq!(error, "MISSING_CHARACTER_ID");
    }

    #[test]
    fn parses_character_bound_ticket_payload_with_fingerprints() {
        let ticket = format!(
            "{}.signature",
            encode_base64url_for_test(
                br#"{"playerId":"plr_1","characterId":"chr_0000000000001","worldId":0,"exp":"2026-06-25T12:15:00.000Z","ver":1}"#
            )
        );

        let payload = parse_character_bound_ticket(&ticket).unwrap();

        assert_eq!(payload.player_id, "plr_1");
        assert_eq!(payload.character_id, "chr_0000000000001");
        assert_eq!(payload.world_id, Some(0));
        assert_eq!(payload.ver, Some(1));
        assert_eq!(payload.ticket_fingerprint.len(), 12);
        assert_eq!(payload.payload_fingerprint.len(), 12);
    }

    fn encode_base64url_for_test(input: &[u8]) -> String {
        const TABLE: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut output = String::new();
        let mut index = 0;
        while index < input.len() {
            let b0 = input[index];
            let b1 = input.get(index + 1).copied().unwrap_or(0);
            let b2 = input.get(index + 2).copied().unwrap_or(0);
            let triple = ((b0 as u32) << 16) | ((b1 as u32) << 8) | b2 as u32;

            output.push(TABLE[((triple >> 18) & 0x3f) as usize] as char);
            output.push(TABLE[((triple >> 12) & 0x3f) as usize] as char);
            if index + 1 < input.len() {
                output.push(TABLE[((triple >> 6) & 0x3f) as usize] as char);
            }
            if index + 2 < input.len() {
                output.push(TABLE[(triple & 0x3f) as usize] as char);
            }
            index += 3;
        }
        output
    }
}
