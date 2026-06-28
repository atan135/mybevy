#![allow(dead_code)]

use std::{
    collections::HashMap,
    env,
    time::{Duration, SystemTime},
};

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
    pub refresh_token: Option<String>,
    pub access_token_expires_at: Option<String>,
    pub refresh_token_expires_at: Option<String>,
    pub ticket: Option<String>,
    pub ticket_expires_at: Option<String>,
    pub player_id: Option<String>,
    pub character_id: Option<String>,
    pub world_id: Option<i64>,
    pub guest_id: Option<String>,
    pub login_name: Option<String>,
    pub characters: Vec<CharacterSummary>,
    pub current_character: Option<CharacterSummary>,
    pub character_profile: Option<CharacterProfile>,
    pub game_endpoint: Option<GameServiceEndpoint>,
    pub character_elements: CharacterElementsCache,
    pub connection_id: Option<ConnectionId>,
    pub transport: Option<NetworkTransport>,
    pub connected: bool,
    pub authenticated: bool,
    pub room_id: Option<String>,
    pub next_seq: u32,
    pub codec: PacketCodec,
    pub pending: HashMap<u32, PendingRequest>,
    pub pending_http: HashMap<RequestId, PendingHttpRequest>,
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
        self.pending_http.clear();
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn logout(&mut self) {
        self.reset_connection_state();
        self.clear_account_state();
        self.login_request = None;
        self.ticket_request = None;
        self.pending_http.clear();
        self.connect_after_login = None;
    }

    pub fn switch_account(&mut self) {
        self.logout();
    }

    pub fn switch_character(&mut self) {
        self.reset_connection_state();
        self.clear_selected_character_state();
        self.ticket_request = None;
        self.pending_http.retain(|_, pending| {
            !matches!(
                pending.operation,
                PendingHttpOperation::CharacterSelect { .. }
                    | PendingHttpOperation::TicketIssue { .. }
            )
        });
        self.connect_after_login = None;
    }

    pub fn disconnect_cleanup(&mut self) {
        self.reset_connection_state();
    }

    pub fn clear_character_after_lifecycle_change(&mut self, character_id: &str) {
        self.characters
            .retain(|character| character.character_id != character_id);
        if self.character_id.as_deref() == Some(character_id) {
            self.switch_character();
        }
    }

    pub fn apply_character_lifecycle_response(&mut self, response: &CharacterLifecycleResponse) {
        let character_id = response.character.character_id.clone();
        let is_deleted = response.lifecycle.state.as_deref() == Some("deleted")
            || response.character.deleted_at.is_some()
            || response.lifecycle.deleted_at.is_some();

        self.characters
            .retain(|character| character.character_id != character_id);

        if is_deleted {
            if self.character_id.as_deref() == Some(character_id.as_str()) {
                self.switch_character();
            }
            return;
        }

        self.characters.push(response.character.clone());
        if self.character_id.as_deref() == Some(character_id.as_str()) {
            self.current_character = Some(response.character.clone());
            self.world_id = response.character.world_id;
        }
    }

    pub fn apply_login_response(&mut self, response: &LoginResponse) -> LoginSession {
        self.reset_connection_state();
        self.clear_selected_character_state();
        self.characters.clear();
        self.access_token = Some(response.access_token.clone());
        self.refresh_token = response.refresh_token.clone();
        self.access_token_expires_at = response.access_token_expires_at.clone();
        self.refresh_token_expires_at = response.refresh_token_expires_at.clone();
        self.ticket = response.ticket.clone();
        self.ticket_expires_at = response.ticket_expires_at.clone();
        self.player_id = Some(response.player_id.clone());
        self.guest_id = response.guest_id.clone();
        self.login_name = response.login_name.clone();
        self.game_endpoint = GameServiceEndpoint::from_auth_parts(
            response.game_proxy_host.clone(),
            response.game_proxy_port,
            response.services.as_ref(),
        );
        login_session_from_response(response)
    }

    pub fn apply_character_list_response(&mut self, response: &CharacterListResponse) -> bool {
        self.player_id = Some(response.player_id.clone());
        self.characters = response.characters.clone();

        if let Some(character_id) = self.character_id.clone() {
            if let Some(character) = self
                .characters
                .iter()
                .find(|character| character.character_id == character_id)
                .cloned()
            {
                self.current_character = Some(character);
            } else {
                self.clear_selected_character_state();
            }
        }

        self.characters.is_empty()
    }

    pub fn apply_character_create_response(&mut self, response: &CharacterCreateResponse) {
        self.characters
            .retain(|character| character.character_id != response.character.character_id);
        self.characters.push(response.character.clone());
    }

    pub fn apply_character_select_response(&mut self, response: &CharacterSelectResponse) {
        self.reset_connection_state();
        self.player_id = Some(response.player_id.clone());
        self.character_id = Some(response.character.character_id.clone());
        self.world_id = response.character.world_id;
        self.current_character = Some(response.character.clone());
        self.character_profile = None;
        self.ticket = Some(response.ticket.clone());
        self.ticket_expires_at = Some(response.ticket_expires_at.clone());
        self.game_endpoint = GameServiceEndpoint::from_auth_parts(
            response.game_proxy_host.clone(),
            response.game_proxy_port,
            response.services.as_ref(),
        );
        self.character_elements
            .clear_for_character(response.character.character_id.clone());
    }

    pub fn apply_ticket_response(&mut self, response: &TicketResponse) {
        self.player_id = Some(response.player_id.clone());
        if let Some(character_id) = response.character_id.clone() {
            self.character_id = Some(character_id);
        }
        self.world_id = response.world_id;
        self.ticket = Some(response.ticket.clone());
        self.ticket_expires_at = Some(response.ticket_expires_at.clone());
        self.game_endpoint = GameServiceEndpoint::from_auth_parts(
            response.game_proxy_host.clone(),
            response.game_proxy_port,
            response.services.as_ref(),
        );
    }

    pub fn apply_character_profile_response(&mut self, response: &CharacterProfileResponse) {
        self.character_id = Some(response.profile.character.character_id.clone());
        self.world_id = response.profile.character.world_id;
        self.current_character = Some(response.profile.character.clone());
        self.character_profile = Some(response.profile.clone());
        if let Some(attributes) = response.profile.character.attributes.as_ref() {
            self.apply_character_elements_snapshot(
                response.profile.character.character_id.clone(),
                CharacterElements {
                    affinity: attributes.affinity,
                    mastery: attributes.mastery,
                },
                SystemTime::now(),
            );
        }
    }

    pub fn apply_character_elements_snapshot(
        &mut self,
        character_id: String,
        elements: CharacterElements,
        refreshed_at: SystemTime,
    ) {
        self.character_elements.character_id = Some(character_id);
        self.character_elements.affinity = elements.affinity;
        self.character_elements.mastery = elements.mastery;
        self.character_elements.snapshot_refreshed_at = Some(refreshed_at);
    }

    pub fn apply_character_elements_response(
        &mut self,
        response: &pb::GetCharacterElementsRes,
        refreshed_at: SystemTime,
    ) -> Option<CharacterElementsCache> {
        if !response.ok {
            return None;
        }
        let elements = response.elements.as_ref()?;
        let character_id = non_empty_string(&response.character_id)
            .map(ToOwned::to_owned)
            .or_else(|| self.character_id.clone())?;
        self.apply_character_elements_snapshot(
            character_id,
            CharacterElements::from_proto(elements),
            refreshed_at,
        );
        Some(self.character_elements.clone())
    }

    pub fn apply_character_elements_push(
        &mut self,
        push: &pb::CharacterElementsChangePush,
        refreshed_at: SystemTime,
    ) -> Option<CharacterElementsCache> {
        let meta = push.meta.as_ref()?;
        let after = push.after.as_ref()?;
        self.apply_character_elements_snapshot(
            meta.character_id.clone(),
            CharacterElements::from_proto(after),
            refreshed_at,
        );
        self.character_elements.last_push_sequence = Some(meta.sequence);
        self.character_elements.last_push_revision = Some(meta.revision);
        Some(self.character_elements.clone())
    }

    fn clear_account_state(&mut self) {
        self.access_token = None;
        self.refresh_token = None;
        self.access_token_expires_at = None;
        self.refresh_token_expires_at = None;
        self.player_id = None;
        self.guest_id = None;
        self.login_name = None;
        self.characters.clear();
        self.clear_selected_character_state();
    }

    fn clear_selected_character_state(&mut self) {
        self.ticket = None;
        self.ticket_expires_at = None;
        self.character_id = None;
        self.world_id = None;
        self.current_character = None;
        self.character_profile = None;
        self.game_endpoint = None;
        self.character_elements = CharacterElementsCache::default();
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameServiceEndpoint {
    pub host: String,
    pub port: u16,
    pub transport: Option<NetworkTransport>,
}

impl GameServiceEndpoint {
    fn from_auth_parts(
        fallback_host: Option<String>,
        fallback_port: Option<u16>,
        services: Option<&ClientServices>,
    ) -> Option<Self> {
        let (host, port, transport) = game_endpoint(fallback_host, fallback_port, services);
        Some(Self {
            host: host?,
            port: port?,
            transport,
        })
    }
}

#[derive(Clone, Debug)]
pub struct CharacterElementsCache {
    pub character_id: Option<String>,
    pub affinity: ElementValues,
    pub mastery: ElementValues,
    pub last_push_sequence: Option<u64>,
    pub last_push_revision: Option<u64>,
    pub snapshot_refreshed_at: Option<SystemTime>,
}

impl Default for CharacterElementsCache {
    fn default() -> Self {
        Self {
            character_id: None,
            affinity: ElementValues::default(),
            mastery: ElementValues::default(),
            last_push_sequence: None,
            last_push_revision: None,
            snapshot_refreshed_at: None,
        }
    }
}

impl CharacterElementsCache {
    fn clear_for_character(&mut self, character_id: String) {
        *self = Self {
            character_id: Some(character_id),
            ..Self::default()
        };
    }
}

#[derive(Clone, Debug)]
pub struct PendingRequest {
    pub response_type: MessageType,
}

#[derive(Clone, Debug)]
pub struct PendingHttpRequest {
    pub operation: PendingHttpOperation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PendingHttpOperation {
    Login { connect_game: bool },
    Register { connect_game: bool },
    GuestLogin { connect_game: bool },
    CharacterList,
    CharacterCreate,
    CharacterProfile { character_id: String },
    CharacterSelect { connect_game: bool },
    CharacterDelete { character_id: String },
    CharacterRestore { character_id: String },
    TicketIssue { reconnect_game: bool },
    Logout,
}

impl PendingHttpOperation {
    pub fn event_operation(&self) -> MyServerOperation {
        match self {
            Self::Login { .. } => MyServerOperation::Login,
            Self::Register { .. } => MyServerOperation::Register,
            Self::GuestLogin { .. } => MyServerOperation::GuestLogin,
            Self::CharacterList => MyServerOperation::CharacterList,
            Self::CharacterCreate => MyServerOperation::CharacterCreate,
            Self::CharacterProfile { .. } => MyServerOperation::CharacterProfile,
            Self::CharacterSelect { .. } => MyServerOperation::CharacterSelect,
            Self::CharacterDelete { .. } => MyServerOperation::CharacterDelete,
            Self::CharacterRestore { .. } => MyServerOperation::CharacterRestore,
            Self::TicketIssue { .. } => MyServerOperation::TicketRefresh,
            Self::Logout => MyServerOperation::Logout,
        }
    }

    pub fn duplicate_group(&self) -> PendingHttpGroup {
        match self {
            Self::Login { .. } | Self::Register { .. } | Self::GuestLogin { .. } => {
                PendingHttpGroup::Login
            }
            Self::CharacterList => PendingHttpGroup::CharacterList,
            Self::CharacterCreate => PendingHttpGroup::CharacterCreate,
            Self::CharacterProfile { .. } => PendingHttpGroup::CharacterProfile,
            Self::CharacterSelect { .. } => PendingHttpGroup::CharacterSelect,
            Self::CharacterDelete { .. } => PendingHttpGroup::CharacterDelete,
            Self::CharacterRestore { .. } => PendingHttpGroup::CharacterRestore,
            Self::TicketIssue { .. } => PendingHttpGroup::TicketIssue,
            Self::Logout => PendingHttpGroup::Logout,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingHttpGroup {
    Login,
    CharacterList,
    CharacterCreate,
    CharacterProfile,
    CharacterSelect,
    CharacterDelete,
    CharacterRestore,
    TicketIssue,
    Logout,
}

#[derive(Clone, Debug)]
pub struct ConnectPlan {
    pub transport: NetworkTransport,
    pub host: Option<String>,
    pub port: Option<u16>,
}

#[derive(Clone, Debug, Message)]
pub enum MyServerCommand {
    Login {
        login_name: String,
        password: String,
        connect_game: bool,
    },
    Register {
        login_name: String,
        password: String,
        connect_game: bool,
    },
    GuestLogin {
        guest_id: Option<String>,
        connect_game: bool,
    },
    LoadCharacterList,
    CreateCharacter {
        name: String,
        appearance_json: Option<Value>,
    },
    LoadCharacterProfile {
        character_id: String,
    },
    SelectCharacter {
        character_id: String,
        connect_game: bool,
    },
    DeleteCharacter {
        character_id: String,
    },
    RestoreCharacter {
        character_id: String,
    },
    IssueTicket {
        reconnect_game: bool,
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
    Logout,
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
    CharacterListLoaded {
        player_id: String,
        characters: Vec<CharacterSummary>,
    },
    CharacterListFailed {
        error: String,
    },
    CharacterCreationRequired {
        player_id: String,
    },
    CharacterCreated {
        character: CharacterSummary,
    },
    CharacterCreateFailed {
        error: String,
    },
    CharacterProfileLoaded {
        profile: CharacterProfile,
    },
    CharacterProfileFailed {
        error: String,
    },
    CharacterSelected {
        player_id: String,
        character_id: String,
        world_id: Option<i64>,
    },
    CharacterSelectFailed {
        error: String,
    },
    CharacterDeleted {
        character_id: String,
    },
    CharacterDeleteFailed {
        error: String,
    },
    CharacterRestored {
        character: CharacterSummary,
    },
    CharacterRestoreFailed {
        error: String,
    },
    LogoutSucceeded,
    LogoutFailed {
        error: String,
    },
    AccountStatusBlocked {
        code: String,
        message: String,
    },
    MaintenanceBlocked {
        message: String,
        retry_after_seconds: Option<u64>,
    },
    AccountBanned {
        message: String,
        banned_until: Option<String>,
    },
    VersionIncompatible {
        message: String,
        required_version: Option<String>,
        current_version: Option<String>,
    },
    NetworkFailed {
        operation: MyServerOperation,
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
    CharacterElementsCacheUpdated(CharacterElementsCache),
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MyServerOperation {
    Login,
    Register,
    GuestLogin,
    CharacterList,
    CharacterCreate,
    CharacterSelect,
    CharacterProfile,
    CharacterDelete,
    CharacterRestore,
    TicketRefresh,
    Logout,
    GameConnect,
    GameRequest,
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
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginResponse {
    pub ok: bool,
    pub player_id: String,
    pub guest_id: Option<String>,
    pub login_name: Option<String>,
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub access_token_expires_at: Option<String>,
    #[serde(default)]
    pub refresh_token_expires_at: Option<String>,
    pub ticket: Option<String>,
    pub ticket_expires_at: Option<String>,
    pub game_proxy_host: Option<String>,
    pub game_proxy_port: Option<u16>,
    pub services: Option<ClientServices>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterPendingReviewResponse {
    pub ok: bool,
    pub player_id: String,
    #[serde(default)]
    pub login_name: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub pending_review: bool,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Clone, Debug)]
pub enum RegisterResponse {
    Login(LoginResponse),
    PendingReview(RegisterPendingReviewResponse),
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterListResponse {
    pub ok: bool,
    pub player_id: String,
    #[serde(default)]
    pub characters: Vec<CharacterSummary>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CharacterCreateResponse {
    pub ok: bool,
    pub character: CharacterSummary,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CharacterProfileResponse {
    pub ok: bool,
    pub profile: CharacterProfile,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CharacterLifecycleResponse {
    pub ok: bool,
    pub character: CharacterSummary,
    pub lifecycle: CharacterLifecycle,
}

#[derive(Clone, Debug, Deserialize)]
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

#[derive(Clone, Debug, Deserialize)]
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

#[derive(Clone, Debug, Deserialize)]
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

#[derive(Clone, Debug, Deserialize)]
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

#[derive(Clone, Debug, Deserialize)]
pub struct SameNameHint {
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
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

#[derive(Clone, Debug, Deserialize)]
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

impl CharacterElements {
    pub fn from_proto(value: &pb::CharacterElements) -> Self {
        Self {
            affinity: value
                .affinity
                .as_ref()
                .map(ElementValues::from_proto)
                .unwrap_or_default(),
            mastery: value
                .mastery
                .as_ref()
                .map(ElementValues::from_proto)
                .unwrap_or_default(),
        }
    }
}

impl ElementValues {
    pub fn from_proto(value: &pb::ElementValues) -> Self {
        Self {
            earth: value.earth,
            fire: value.fire,
            water: value.water,
            wind: value.wind,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct CharacterAttributes {
    #[serde(default)]
    pub affinity: ElementValues,
    #[serde(default)]
    pub mastery: ElementValues,
}

#[derive(Clone, Debug, Deserialize)]
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

#[derive(Clone, Debug, Deserialize)]
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

#[derive(Clone, Debug, Deserialize)]
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

#[derive(Clone, Debug, Deserialize)]
pub struct ClientServices {
    pub game: Option<ClientServiceEndpoint>,
    #[serde(default)]
    pub chat: Option<ClientServiceEndpoint>,
    #[serde(default)]
    pub mail: Option<ClientServiceEndpoint>,
    #[serde(default)]
    pub announce: Option<ClientServiceEndpoint>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ClientServiceEndpoint {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub protocol: Option<String>,
}

fn non_empty_string(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
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
                "refreshToken": "refresh",
                "accessTokenExpiresAt": "2026-06-25T12:05:00.000Z",
                "refreshTokenExpiresAt": "2026-07-25T12:00:00.000Z",
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

        let mut session = MyServerSession::default();
        let login_session = session.apply_login_response(&response);

        assert_eq!(login_session.player_id, "plr_1");
        assert_eq!(session.access_token.as_deref(), Some("access"));
        assert_eq!(session.refresh_token.as_deref(), Some("refresh"));
        assert_eq!(
            session.access_token_expires_at.as_deref(),
            Some("2026-06-25T12:05:00.000Z")
        );
        assert_eq!(session.guest_id.as_deref(), Some("guest-a"));
        assert!(session.character_id.is_none());
        assert!(session.ticket.is_none());
        assert_eq!(session.game_endpoint.as_ref().unwrap().port, 14000);
    }

    #[test]
    fn writes_account_login_without_guest_or_ticket_to_session() {
        let response: LoginResponse = serde_json::from_str(
            r#"{
                "ok": true,
                "playerId": "plr_account",
                "guestId": null,
                "loginName": "alice",
                "accessToken": "access-new",
                "ticket": null,
                "ticketExpiresAt": null,
                "services": { "game": { "host": "game.local", "port": 4000, "protocol": "kcp" } }
            }"#,
        )
        .unwrap();

        let mut session = MyServerSession {
            character_id: Some("old-character".to_string()),
            ticket: Some("old-ticket".to_string()),
            ..Default::default()
        };
        session.apply_login_response(&response);

        assert_eq!(session.player_id.as_deref(), Some("plr_account"));
        assert_eq!(session.login_name.as_deref(), Some("alice"));
        assert!(session.guest_id.is_none());
        assert!(session.ticket.is_none());
        assert!(session.character_id.is_none());
        let endpoint = session.game_endpoint.as_ref().unwrap();
        assert_eq!(endpoint.host, "game.local");
        assert_eq!(endpoint.port, 4000);
        assert_eq!(endpoint.transport, Some(NetworkTransport::Kcp));
    }

    #[test]
    fn parses_register_pending_review_without_access_token() {
        let response: RegisterPendingReviewResponse = serde_json::from_str(
            r#"{
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

        assert!(response.ok);
        assert_eq!(response.player_id, "plr_pending");
        assert_eq!(response.login_name.as_deref(), Some("alice"));
        assert_eq!(response.display_name.as_deref(), Some("Alice"));
        assert_eq!(response.status.as_deref(), Some("pending_review"));
        assert!(response.pending_review);
        assert_eq!(
            response.message.as_deref(),
            Some("Registration submitted for review")
        );
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

        let mut session = MyServerSession::default();
        let needs_character = session.apply_character_list_response(&response);

        assert!(!needs_character);
        assert_eq!(session.player_id.as_deref(), Some("plr_1"));
        assert_eq!(session.characters.len(), 1);
        assert_eq!(session.characters[0].world_id, Some(0));
        assert_eq!(
            session.characters[0]
                .attributes
                .as_ref()
                .unwrap()
                .mastery
                .fire,
            0
        );

        let mut empty_session = MyServerSession::default();
        assert!(empty_session.apply_character_list_response(&empty));
        assert!(empty_session.characters.is_empty());
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

        let mut session = MyServerSession::default();
        session.apply_character_select_response(&response);

        assert_eq!(session.player_id.as_deref(), Some("plr_1"));
        assert_eq!(session.character_id.as_deref(), Some("chr_0000000000001"));
        assert_eq!(
            session.current_character.as_ref().unwrap().name,
            "WindRunner"
        );
        assert_eq!(session.ticket.as_deref(), Some("payload.signature"));
        assert_eq!(
            session.ticket_expires_at.as_deref(),
            Some("2026-06-25T12:15:00.000Z")
        );
        assert_eq!(
            session.character_elements.character_id.as_deref(),
            Some("chr_0000000000001")
        );
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

        let mut session = MyServerSession::default();
        session.apply_ticket_response(&response);

        assert_eq!(session.player_id.as_deref(), Some("plr_1"));
        assert_eq!(session.character_id.as_deref(), Some("chr_0000000000001"));
        assert_eq!(session.world_id, Some(0));
        assert_eq!(session.ticket.as_deref(), Some("payload.signature"));
        assert_eq!(
            session.game_endpoint.as_ref().unwrap().transport,
            Some(NetworkTransport::Kcp)
        );
    }

    #[test]
    fn writes_character_profile_attributes_to_session_cache() {
        let response: CharacterProfileResponse = serde_json::from_str(
            r#"{
                "ok": true,
                "profile": {
                    "character_id": "chr_profile",
                    "name": "Profiled",
                    "world_id": 9,
                    "attributes": {
                        "affinity": { "earth": 1, "fire": 2, "water": 3, "wind": 4 },
                        "mastery": { "earth": 5, "fire": 6, "water": 7, "wind": 8 }
                    }
                }
            }"#,
        )
        .unwrap();

        let mut session = MyServerSession::default();
        session.apply_character_profile_response(&response);

        assert_eq!(session.character_id.as_deref(), Some("chr_profile"));
        assert_eq!(session.world_id, Some(9));
        assert_eq!(
            session.character_profile.as_ref().unwrap().character.name,
            "Profiled"
        );
        assert_eq!(session.character_elements.affinity.wind, 4);
        assert_eq!(session.character_elements.mastery.fire, 6);
        assert!(session.character_elements.snapshot_refreshed_at.is_some());
    }

    #[test]
    fn writes_character_elements_response_and_push_to_session_cache() {
        let mut session = MyServerSession {
            character_id: Some("fallback-character".to_string()),
            ..Default::default()
        };
        let refreshed_at = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
        let response = pb::GetCharacterElementsRes {
            ok: true,
            error_code: String::new(),
            character_id: "chr_elements".to_string(),
            elements: Some(pb::CharacterElements {
                affinity: Some(pb::ElementValues {
                    earth: 10,
                    fire: 20,
                    water: 30,
                    wind: 40,
                }),
                mastery: Some(pb::ElementValues {
                    earth: 1,
                    fire: 2,
                    water: 3,
                    wind: 4,
                }),
            }),
        };

        let cache = session
            .apply_character_elements_response(&response, refreshed_at)
            .unwrap();

        assert_eq!(cache.character_id.as_deref(), Some("chr_elements"));
        assert_eq!(session.character_elements.affinity.water, 30);
        assert_eq!(session.character_elements.mastery.wind, 4);
        assert_eq!(
            session.character_elements.snapshot_refreshed_at,
            Some(refreshed_at)
        );

        let pushed_at = SystemTime::UNIX_EPOCH + Duration::from_secs(20);
        let push = pb::CharacterElementsChangePush {
            meta: Some(pb::CharacterPushMeta {
                character_id: "chr_elements".to_string(),
                sequence: 7,
                revision: 3,
                source_type: "item_use".to_string(),
                source_id: "item:1".to_string(),
                action: "element_change".to_string(),
                summary: "debug".to_string(),
                snapshot_compensation: false,
            }),
            before: None,
            after: Some(pb::CharacterElements {
                affinity: Some(pb::ElementValues {
                    earth: 11,
                    fire: 21,
                    water: 31,
                    wind: 41,
                }),
                mastery: Some(pb::ElementValues {
                    earth: 2,
                    fire: 3,
                    water: 4,
                    wind: 5,
                }),
            }),
        };

        let cache = session
            .apply_character_elements_push(&push, pushed_at)
            .unwrap();

        assert_eq!(cache.affinity.earth, 11);
        assert_eq!(cache.mastery.wind, 5);
        assert_eq!(cache.last_push_sequence, Some(7));
        assert_eq!(cache.last_push_revision, Some(3));
        assert_eq!(cache.snapshot_refreshed_at, Some(pushed_at));
    }

    #[test]
    fn session_cleanup_methods_keep_account_and_character_boundaries_clear() {
        let mut session = MyServerSession {
            access_token: Some("access".to_string()),
            refresh_token: Some("refresh".to_string()),
            player_id: Some("plr".to_string()),
            guest_id: Some("guest".to_string()),
            login_name: Some("name".to_string()),
            ticket: Some("ticket".to_string()),
            ticket_expires_at: Some("exp".to_string()),
            character_id: Some("chr".to_string()),
            world_id: Some(1),
            characters: vec![CharacterSummary {
                character_id: "chr".to_string(),
                character_id_short: None,
                display_discriminator: None,
                same_name_hint: None,
                name: "Role".to_string(),
                world_id: Some(1),
                status: None,
                appearance_json: None,
                created_at: None,
                last_login_at: None,
                deleted_at: None,
                position: None,
                attributes: None,
                lifecycle: None,
                extra: HashMap::new(),
            }],
            connected: true,
            authenticated: true,
            room_id: Some("room".to_string()),
            character_elements: CharacterElementsCache {
                character_id: Some("chr".to_string()),
                affinity: ElementValues {
                    fire: 1,
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };

        session.disconnect_cleanup();
        assert_eq!(session.player_id.as_deref(), Some("plr"));
        assert_eq!(session.character_id.as_deref(), Some("chr"));
        assert!(!session.connected);
        assert!(!session.authenticated);
        assert!(session.room_id.is_none());

        session.switch_character();
        assert_eq!(session.player_id.as_deref(), Some("plr"));
        assert_eq!(session.access_token.as_deref(), Some("access"));
        assert!(session.ticket.is_none());
        assert!(session.character_id.is_none());
        assert_eq!(session.characters.len(), 1);
        assert!(session.character_elements.character_id.is_none());

        session.switch_account();
        assert!(session.access_token.is_none());
        assert!(session.refresh_token.is_none());
        assert!(session.player_id.is_none());
        assert!(session.characters.is_empty());
    }

    #[test]
    fn character_lifecycle_delete_clears_selected_character_and_restore_updates_list() {
        let mut session = MyServerSession {
            access_token: Some("access".to_string()),
            player_id: Some("plr".to_string()),
            ticket: Some("ticket".to_string()),
            character_id: Some("chr".to_string()),
            current_character: Some(test_character("chr", "OldRole")),
            characters: vec![test_character("chr", "OldRole")],
            ..Default::default()
        };

        let deleted = CharacterLifecycleResponse {
            ok: true,
            character: CharacterSummary {
                deleted_at: Some("2026-06-25T12:00:00.000Z".to_string()),
                lifecycle: Some(CharacterLifecycle {
                    state: Some("deleted".to_string()),
                    ..empty_lifecycle()
                }),
                ..test_character("chr", "OldRole")
            },
            lifecycle: CharacterLifecycle {
                state: Some("deleted".to_string()),
                deleted_at: Some("2026-06-25T12:00:00.000Z".to_string()),
                ..empty_lifecycle()
            },
        };

        session.apply_character_lifecycle_response(&deleted);

        assert_eq!(session.player_id.as_deref(), Some("plr"));
        assert_eq!(session.access_token.as_deref(), Some("access"));
        assert!(session.ticket.is_none());
        assert!(session.character_id.is_none());
        assert!(session.characters.is_empty());

        let restored = CharacterLifecycleResponse {
            ok: true,
            character: test_character("chr", "RestoredRole"),
            lifecycle: CharacterLifecycle {
                state: Some("active".to_string()),
                ..empty_lifecycle()
            },
        };

        session.apply_character_lifecycle_response(&restored);

        assert_eq!(session.characters.len(), 1);
        assert_eq!(session.characters[0].name, "RestoredRole");
        assert!(session.character_id.is_none());
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

    fn test_character(character_id: &str, name: &str) -> CharacterSummary {
        CharacterSummary {
            character_id: character_id.to_string(),
            character_id_short: None,
            display_discriminator: None,
            same_name_hint: None,
            name: name.to_string(),
            world_id: Some(1),
            status: Some("active".to_string()),
            appearance_json: None,
            created_at: None,
            last_login_at: None,
            deleted_at: None,
            position: None,
            attributes: None,
            lifecycle: None,
            extra: HashMap::new(),
        }
    }

    fn empty_lifecycle() -> CharacterLifecycle {
        CharacterLifecycle {
            state: None,
            deleted_at: None,
            restore_window_seconds: None,
            restore_expires_at: None,
            delete_cooldown_seconds: None,
            hard_delete_eligible_at: None,
        }
    }
}
