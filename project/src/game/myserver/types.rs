#![allow(dead_code)]

use std::{
    collections::HashMap,
    env,
    time::{Duration, SystemTime, UNIX_EPOCH},
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
pub const DEFAULT_TICKET_REFRESH_MARGIN: Duration = Duration::from_secs(30);
pub const DIAGNOSTIC_FINGERPRINT_LEN: usize = 12;

#[derive(Clone, Debug, Resource)]
pub struct MyServerConfig {
    pub http_base_url: String,
    pub game_host: String,
    pub kcp_port: u16,
    pub tcp_fallback_port: u16,
    pub prefer_transport: NetworkTransport,
    pub forced_transport: Option<NetworkTransport>,
    pub request_timeout: Duration,
    pub ticket_refresh_margin: Duration,
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
            ticket_refresh_margin: Duration::from_millis(env_u64(
                "MYSERVER_TICKET_REFRESH_MARGIN_MS",
                DEFAULT_TICKET_REFRESH_MARGIN.as_millis() as u64,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountLoginState {
    NotLoggedIn,
    LoggingIn,
    LoggedIn,
    LoginFailed,
    Blocked,
    Expired,
    LoggedOut,
}

impl Default for AccountLoginState {
    fn default() -> Self {
        Self::NotLoggedIn
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharacterSelectionState {
    NotLoaded,
    Loading,
    NoCharacters,
    Creating,
    AwaitingSelection,
    LoadingProfile,
    Selecting,
    Selected,
    Blocked,
    SelectionFailed,
}

impl Default for CharacterSelectionState {
    fn default() -> Self {
        Self::NotLoaded
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameConnectionState {
    NotConnected,
    Connecting,
    Connected,
    Authenticating,
    Authenticated,
    Disconnected,
    Reconnecting,
    ReconnectFailed,
}

impl Default for GameConnectionState {
    fn default() -> Self {
        Self::NotConnected
    }
}

#[derive(Clone, Debug, Default, Resource)]
pub struct MyServerSession {
    pub account_login_state: AccountLoginState,
    pub character_selection_state: CharacterSelectionState,
    pub game_connection_state: GameConnectionState,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub access_token_expires_at: Option<String>,
    pub refresh_token_expires_at: Option<String>,
    pub ticket: Option<String>,
    pub ticket_expires_at: Option<String>,
    pub player_id: Option<String>,
    pub character_id: Option<String>,
    pub pending_character_id: Option<String>,
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
    pub reconnect_after_auth: Option<ReconnectPlan>,
    pub reconnect_blocked: bool,
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
        self.reset_transport_state();
        self.pending_http.clear();
    }

    pub fn reset_transport_state(&mut self) {
        self.game_connection_state = GameConnectionState::NotConnected;
        self.connection_id = None;
        self.transport = None;
        self.connected = false;
        self.authenticated = false;
        self.room_id = None;
        self.codec.clear();
        self.pending.clear();
        self.reconnect_after_auth = None;
    }

    pub fn clear_reconnect_plan(&mut self) {
        self.connect_after_login = None;
        self.reconnect_after_auth = None;
    }

    pub fn reconnect_failed_cleanup(&mut self) {
        self.clear_reconnect_plan();
        self.game_connection_state = GameConnectionState::ReconnectFailed;
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn ticket_expiration_time(&self) -> Option<SystemTime> {
        self.ticket_expires_at
            .as_deref()
            .and_then(parse_ticket_expiration)
            .or_else(|| {
                let payload = parse_character_bound_ticket(self.ticket.as_deref()?).ok()?;
                parse_ticket_expiration(&payload.exp)
            })
    }

    pub fn needs_ticket_refresh(&self, now: SystemTime, refresh_margin: Duration) -> bool {
        if self.ticket.as_deref().and_then(non_empty_string).is_none()
            || self
                .character_id
                .as_deref()
                .and_then(non_empty_string)
                .is_none()
        {
            return false;
        }

        let Some(expires_at) = self.ticket_expiration_time() else {
            return false;
        };

        match expires_at.duration_since(now) {
            Ok(remaining) => remaining <= refresh_margin,
            Err(_) => true,
        }
    }

    pub fn logout(&mut self) {
        self.reset_connection_state();
        self.clear_account_state();
        self.account_login_state = AccountLoginState::LoggedOut;
        self.character_selection_state = CharacterSelectionState::NotLoaded;
        self.login_request = None;
        self.ticket_request = None;
        self.pending_character_id = None;
        self.pending_http.clear();
        self.connect_after_login = None;
        self.reconnect_after_auth = None;
        self.reconnect_blocked = false;
    }

    pub fn switch_account(&mut self) {
        self.logout();
    }

    pub fn switch_character(&mut self) {
        self.reset_connection_state();
        self.clear_selected_character_state();
        self.character_selection_state = if self.characters.is_empty() {
            CharacterSelectionState::NoCharacters
        } else {
            CharacterSelectionState::AwaitingSelection
        };
        self.pending_character_id = None;
        self.ticket_request = None;
        self.pending_http.retain(|_, pending| {
            !matches!(
                pending.operation,
                PendingHttpOperation::CharacterSelect { .. }
                    | PendingHttpOperation::TicketIssue { .. }
                    | PendingHttpOperation::CharacterList
                    | PendingHttpOperation::CharacterCreate
                    | PendingHttpOperation::CharacterProfile { .. }
            )
        });
        self.connect_after_login = None;
        self.reconnect_after_auth = None;
    }

    pub fn disconnect_cleanup(&mut self) {
        self.reset_transport_state();
        self.ticket_request = None;
        self.connect_after_login = None;
        self.reconnect_after_auth = None;
        self.game_connection_state = GameConnectionState::Disconnected;
    }

    pub fn block_reconnect_after_kick(&mut self) {
        self.reset_transport_state();
        self.ticket_request = None;
        self.connect_after_login = None;
        self.reconnect_after_auth = None;
        self.reconnect_blocked = true;
        self.pending_http.clear();
        self.game_connection_state = GameConnectionState::Disconnected;
    }

    pub fn begin_login(&mut self) {
        self.account_login_state = AccountLoginState::LoggingIn;
    }

    pub fn login_failed(&mut self) {
        self.account_login_state = AccountLoginState::LoginFailed;
    }

    pub fn account_blocked(&mut self) {
        self.reset_connection_state();
        self.clear_account_state();
        self.account_login_state = AccountLoginState::Blocked;
        self.character_selection_state = CharacterSelectionState::NotLoaded;
        self.login_request = None;
        self.ticket_request = None;
        self.connect_after_login = None;
        self.reconnect_after_auth = None;
        self.reconnect_blocked = true;
        self.pending_character_id = None;
        self.pending_http.clear();
    }

    pub fn account_expired(&mut self) {
        self.reset_connection_state();
        self.clear_account_state();
        self.account_login_state = AccountLoginState::Expired;
        self.character_selection_state = CharacterSelectionState::NotLoaded;
        self.login_request = None;
        self.ticket_request = None;
        self.connect_after_login = None;
        self.reconnect_after_auth = None;
        self.reconnect_blocked = true;
        self.pending_character_id = None;
        self.pending_http.clear();
    }

    pub fn begin_character_list(&mut self) {
        self.character_selection_state = CharacterSelectionState::Loading;
    }

    pub fn character_list_failed(&mut self) {
        self.character_selection_state = CharacterSelectionState::SelectionFailed;
    }

    pub fn begin_character_create(&mut self) {
        self.character_selection_state = CharacterSelectionState::Creating;
    }

    pub fn character_create_failed(&mut self) {
        self.character_selection_state = CharacterSelectionState::SelectionFailed;
    }

    pub fn begin_character_profile(&mut self) {
        self.character_selection_state = CharacterSelectionState::LoadingProfile;
    }

    pub fn character_profile_failed(&mut self) {
        self.character_selection_state = CharacterSelectionState::SelectionFailed;
    }

    pub fn begin_character_select(&mut self, character_id: String) {
        self.pending_character_id = Some(character_id);
        self.character_selection_state = CharacterSelectionState::Selecting;
    }

    pub fn character_select_failed(&mut self) {
        self.pending_character_id = None;
        self.character_selection_state = CharacterSelectionState::SelectionFailed;
    }

    pub fn character_blocked(&mut self) {
        self.pending_character_id = None;
        self.character_selection_state = CharacterSelectionState::Blocked;
    }

    pub fn begin_ticket_issue(&mut self, reconnect_game: bool) {
        if reconnect_game {
            self.game_connection_state = GameConnectionState::Reconnecting;
        }
    }

    pub fn ticket_issue_failed(&mut self, reconnect_game: bool) {
        if reconnect_game {
            self.game_connection_state = GameConnectionState::ReconnectFailed;
        }
    }

    pub fn begin_connect_game(&mut self, connection_id: ConnectionId, transport: NetworkTransport) {
        self.connection_id = Some(connection_id);
        self.transport = Some(transport);
        self.connected = false;
        self.authenticated = false;
        self.codec.clear();
        self.pending.clear();
        self.game_connection_state = GameConnectionState::Connecting;
    }

    pub fn game_connected(&mut self, transport: NetworkTransport) {
        self.connected = true;
        self.transport = Some(transport);
        self.game_connection_state = GameConnectionState::Connected;
    }

    pub fn begin_game_auth(&mut self) {
        self.game_connection_state = GameConnectionState::Authenticating;
    }

    pub fn game_authenticated(&mut self, player_id: String) {
        self.authenticated = true;
        self.player_id = Some(player_id);
        self.game_connection_state = GameConnectionState::Authenticated;
    }

    pub fn game_auth_failed(&mut self) {
        self.authenticated = false;
        self.game_connection_state = GameConnectionState::Disconnected;
    }

    pub fn game_connection_failed(&mut self) {
        self.reset_connection_state();
        self.ticket_request = None;
        self.connect_after_login = None;
        self.game_connection_state = GameConnectionState::ReconnectFailed;
    }

    pub fn begin_http_operation(&mut self, operation: &PendingHttpOperation) {
        match operation {
            PendingHttpOperation::Login { .. }
            | PendingHttpOperation::Register { .. }
            | PendingHttpOperation::GuestLogin { .. } => self.begin_login(),
            PendingHttpOperation::CharacterList => self.begin_character_list(),
            PendingHttpOperation::CharacterCreate => self.begin_character_create(),
            PendingHttpOperation::CharacterProfile { .. } => self.begin_character_profile(),
            PendingHttpOperation::CharacterSelect { character_id, .. } => {
                self.begin_character_select(character_id.clone());
            }
            PendingHttpOperation::TicketIssue { reconnect_game } => {
                self.begin_ticket_issue(*reconnect_game);
            }
            PendingHttpOperation::Logout
            | PendingHttpOperation::CharacterDelete { .. }
            | PendingHttpOperation::CharacterRestore { .. } => {}
        }
    }

    pub fn http_operation_failed(&mut self, operation: &PendingHttpOperation) {
        match operation {
            PendingHttpOperation::Login { .. }
            | PendingHttpOperation::Register { .. }
            | PendingHttpOperation::GuestLogin { .. } => self.login_failed(),
            PendingHttpOperation::CharacterList => self.character_list_failed(),
            PendingHttpOperation::CharacterCreate => self.character_create_failed(),
            PendingHttpOperation::CharacterProfile { .. } => self.character_profile_failed(),
            PendingHttpOperation::CharacterSelect { .. } => self.character_select_failed(),
            PendingHttpOperation::TicketIssue { reconnect_game } => {
                self.ticket_issue_failed(*reconnect_game);
            }
            PendingHttpOperation::Logout
            | PendingHttpOperation::CharacterDelete { .. }
            | PendingHttpOperation::CharacterRestore { .. } => {}
        }
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
        self.account_login_state = AccountLoginState::LoggedIn;
        self.character_selection_state = CharacterSelectionState::NotLoaded;
        self.reconnect_blocked = false;
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
        let mut has_selected_character = false;

        if let Some(character_id) = self.character_id.clone() {
            if let Some(character) = self
                .characters
                .iter()
                .find(|character| character.character_id == character_id)
                .cloned()
            {
                self.current_character = Some(character);
                has_selected_character = true;
            } else {
                self.clear_selected_character_state();
            }
        }

        let needs_character = self.characters.is_empty();
        self.character_selection_state = if needs_character {
            CharacterSelectionState::NoCharacters
        } else if has_selected_character {
            CharacterSelectionState::Selected
        } else {
            CharacterSelectionState::AwaitingSelection
        };
        needs_character
    }

    pub fn apply_character_create_response(&mut self, response: &CharacterCreateResponse) {
        self.characters
            .retain(|character| character.character_id != response.character.character_id);
        self.characters.push(response.character.clone());
        self.character_selection_state = CharacterSelectionState::AwaitingSelection;
    }

    pub fn apply_character_select_response(&mut self, response: &CharacterSelectResponse) {
        self.reset_connection_state();
        self.player_id = Some(response.player_id.clone());
        self.character_id = Some(response.character.character_id.clone());
        self.pending_character_id = None;
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
        self.character_selection_state = CharacterSelectionState::Selected;
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
        let profile_character_id = response.profile.character.character_id.clone();
        let profile_is_selected = self.character_id.as_deref()
            == Some(profile_character_id.as_str())
            && self.ticket.is_some();
        if profile_is_selected {
            self.world_id = response.profile.character.world_id;
            self.current_character = Some(response.profile.character.clone());
        }
        self.character_profile = Some(response.profile.clone());
        self.character_selection_state = if profile_is_selected {
            CharacterSelectionState::Selected
        } else if self.characters.is_empty() {
            CharacterSelectionState::NoCharacters
        } else {
            CharacterSelectionState::AwaitingSelection
        };
        if let Some(attributes) = response.profile.character.attributes.as_ref() {
            self.apply_character_elements_snapshot(
                profile_character_id,
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
        self.pending_character_id = None;
        self.world_id = None;
        self.current_character = None;
        self.character_profile = None;
        self.game_endpoint = None;
        self.character_elements = CharacterElementsCache::default();
        self.reconnect_after_auth = None;
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
    Login {
        connect_game: bool,
    },
    Register {
        connect_game: bool,
    },
    GuestLogin {
        connect_game: bool,
    },
    CharacterList,
    CharacterCreate,
    CharacterProfile {
        character_id: String,
    },
    CharacterSelect {
        character_id: String,
        connect_game: bool,
    },
    CharacterDelete {
        character_id: String,
    },
    CharacterRestore {
        character_id: String,
    },
    TicketIssue {
        reconnect_game: bool,
    },
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

    pub fn endpoint_path(&self) -> &'static str {
        match self {
            Self::Login { .. } => "/api/v1/auth/login",
            Self::Register { .. } => "/api/v1/auth/register",
            Self::GuestLogin { .. } => "/api/v1/auth/guest-login",
            Self::CharacterList => "/api/v1/characters",
            Self::CharacterCreate => "/api/v1/characters",
            Self::CharacterProfile { .. } => "/api/v1/characters/{character_id}/profile",
            Self::CharacterSelect { .. } => "/api/v1/characters/select",
            Self::CharacterDelete { .. } => "/api/v1/characters/delete",
            Self::CharacterRestore { .. } => "/api/v1/characters/restore",
            Self::TicketIssue { .. } => "/api/v1/game-ticket/issue",
            Self::Logout => "/api/v1/auth/logout",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Login { .. } => "login",
            Self::Register { .. } => "register",
            Self::GuestLogin { .. } => "guest_login",
            Self::CharacterList => "character_list",
            Self::CharacterCreate => "character_create",
            Self::CharacterProfile { .. } => "character_profile",
            Self::CharacterSelect { .. } => "character_select",
            Self::CharacterDelete { .. } => "character_delete",
            Self::CharacterRestore { .. } => "character_restore",
            Self::TicketIssue { .. } => "ticket_issue",
            Self::Logout => "logout",
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReconnectCause {
    ServerRedirect {
        reason: String,
        room_id: Option<String>,
        target_server_id: Option<String>,
        rollout_epoch: Option<String>,
    },
    TransportRecovery,
}

#[derive(Clone, Debug)]
pub struct ReconnectPlan {
    pub cause: ReconnectCause,
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
    DisplayError {
        error: MyServerDisplayError,
    },
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
    ReauthenticatedForReconnect {
        player_id: String,
        cause: ReconnectCause,
    },
    AuthFailed {
        error_code: String,
    },
    GameAuthRejected {
        error_code: String,
        reason: GameAuthFailureReason,
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
    RoomReconnected(pb::RoomReconnectRes),
    ServerRedirectPush(pb::ServerRedirectPush),
    ServerRedirectReconnectStarted {
        reason: String,
        target_host: String,
        target_port: u16,
        transport: NetworkTransport,
    },
    ServerRedirectIgnored {
        reason: String,
        detail: String,
    },
    SessionKickPush(pb::SessionKickPush),
    SessionKicked {
        reason: String,
        category: SessionKickCategory,
        timestamp: i64,
    },
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MyServerErrorSource {
    Client,
    Http,
    Transport,
    Protocol,
    Game,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MyServerErrorKind {
    AccountBlocked,
    IpBlocked,
    PlayerBlocked,
    BlocklistUnavailable,
    Maintenance,
    AccountBanned,
    PendingReview,
    VersionIncompatible,
    CharacterUnavailable,
    CharacterLimitReached,
    CharacterLifecycleFailed,
    TicketExpired,
    MissingCharacterId,
    PreauthMessageNotAllowed,
    MessageRateExceeded,
    GameAuthRejected,
    RoomJoinFailed,
    CharacterElementsFailed,
    ServerRedirectFailed,
    SessionKicked,
    Unauthorized,
    HttpStatus,
    JsonParseFailed,
    ProtobufDecodeFailed,
    ConnectionTimeout,
    TransportFailed,
    ProtocolError,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MyServerDisplayError {
    pub kind: MyServerErrorKind,
    pub source: MyServerErrorSource,
    pub operation: Option<MyServerOperation>,
    pub message_type: Option<MessageType>,
    pub seq: Option<u32>,
    pub http_status: Option<u16>,
    pub error_code: Option<String>,
    pub message_key: &'static str,
    pub retryable: bool,
    pub blocking: bool,
    pub detail: Option<String>,
}

impl MyServerDisplayError {
    pub fn from_error_code(
        source: MyServerErrorSource,
        operation: Option<MyServerOperation>,
        message_type: Option<MessageType>,
        seq: Option<u32>,
        http_status: Option<u16>,
        error_code: impl AsRef<str>,
        detail: Option<String>,
    ) -> Self {
        let code = normalize_error_code(error_code.as_ref());
        let kind = classify_display_error_code(&code, operation, message_type);
        Self::new(
            kind,
            source,
            operation,
            message_type,
            seq,
            http_status,
            (!code.is_empty()).then_some(code),
            detail,
        )
    }

    pub fn http_status(
        operation: MyServerOperation,
        http_status: u16,
        error_code: Option<String>,
        detail: Option<String>,
    ) -> Self {
        if let Some(error_code) = error_code {
            return Self::from_error_code(
                MyServerErrorSource::Http,
                Some(operation),
                None,
                None,
                Some(http_status),
                error_code,
                detail,
            );
        }

        Self::new(
            if http_status == 401 {
                MyServerErrorKind::Unauthorized
            } else {
                MyServerErrorKind::HttpStatus
            },
            MyServerErrorSource::Http,
            Some(operation),
            None,
            None,
            Some(http_status),
            None,
            detail,
        )
    }

    pub fn json_parse(operation: MyServerOperation, detail: Option<String>) -> Self {
        Self::new(
            MyServerErrorKind::JsonParseFailed,
            MyServerErrorSource::Protocol,
            Some(operation),
            None,
            None,
            None,
            None,
            detail,
        )
    }

    pub fn protobuf_decode(
        message_type: Option<MessageType>,
        seq: Option<u32>,
        detail: Option<String>,
    ) -> Self {
        Self::new(
            MyServerErrorKind::ProtobufDecodeFailed,
            MyServerErrorSource::Protocol,
            Some(MyServerOperation::GameRequest),
            message_type,
            seq,
            None,
            None,
            detail,
        )
    }

    pub fn protocol(
        message_type: Option<MessageType>,
        seq: Option<u32>,
        detail: Option<String>,
    ) -> Self {
        Self::new(
            MyServerErrorKind::ProtocolError,
            MyServerErrorSource::Protocol,
            Some(MyServerOperation::GameRequest),
            message_type,
            seq,
            None,
            None,
            detail,
        )
    }

    pub fn transport(operation: MyServerOperation, detail: Option<String>) -> Self {
        let is_timeout = detail
            .as_deref()
            .map(|value| normalize_error_code(value).contains("TIMEOUT"))
            .unwrap_or(false);
        Self::new(
            if is_timeout {
                MyServerErrorKind::ConnectionTimeout
            } else {
                MyServerErrorKind::TransportFailed
            },
            MyServerErrorSource::Transport,
            Some(operation),
            None,
            None,
            None,
            None,
            detail,
        )
    }

    fn new(
        kind: MyServerErrorKind,
        source: MyServerErrorSource,
        operation: Option<MyServerOperation>,
        message_type: Option<MessageType>,
        seq: Option<u32>,
        http_status: Option<u16>,
        error_code: Option<String>,
        detail: Option<String>,
    ) -> Self {
        Self {
            kind,
            source,
            operation,
            message_type,
            seq,
            http_status,
            error_code,
            message_key: kind.message_key(),
            retryable: kind.retryable(),
            blocking: kind.blocking(),
            detail: sanitize_error_detail(detail),
        }
    }
}

impl MyServerErrorKind {
    pub const fn message_key(self) -> &'static str {
        match self {
            Self::AccountBlocked => "myserver.error.account_blocked",
            Self::IpBlocked => "myserver.error.ip_blocked",
            Self::PlayerBlocked => "myserver.error.player_blocked",
            Self::BlocklistUnavailable => "myserver.error.blocklist_unavailable",
            Self::Maintenance => "myserver.error.maintenance",
            Self::AccountBanned => "myserver.error.account_banned",
            Self::PendingReview => "myserver.error.pending_review",
            Self::VersionIncompatible => "myserver.error.version_incompatible",
            Self::CharacterUnavailable => "myserver.error.character_unavailable",
            Self::CharacterLimitReached => "myserver.error.character_limit_reached",
            Self::CharacterLifecycleFailed => "myserver.error.character_lifecycle_failed",
            Self::TicketExpired => "myserver.error.ticket_expired",
            Self::MissingCharacterId => "myserver.error.missing_character_id",
            Self::PreauthMessageNotAllowed => "myserver.error.preauth_message_not_allowed",
            Self::MessageRateExceeded => "myserver.error.message_rate_exceeded",
            Self::GameAuthRejected => "myserver.error.game_auth_rejected",
            Self::RoomJoinFailed => "myserver.error.room_join_failed",
            Self::CharacterElementsFailed => "myserver.error.character_elements_failed",
            Self::ServerRedirectFailed => "myserver.error.server_redirect_failed",
            Self::SessionKicked => "myserver.error.session_kicked",
            Self::Unauthorized => "myserver.error.unauthorized",
            Self::HttpStatus => "myserver.error.http_status",
            Self::JsonParseFailed => "myserver.error.json_parse_failed",
            Self::ProtobufDecodeFailed => "myserver.error.protobuf_decode_failed",
            Self::ConnectionTimeout => "myserver.error.connection_timeout",
            Self::TransportFailed => "myserver.error.transport_failed",
            Self::ProtocolError => "myserver.error.protocol_error",
            Self::Unknown => "myserver.error.unknown",
        }
    }

    pub const fn retryable(self) -> bool {
        matches!(
            self,
            Self::BlocklistUnavailable
                | Self::Maintenance
                | Self::HttpStatus
                | Self::ConnectionTimeout
                | Self::TransportFailed
                | Self::MessageRateExceeded
                | Self::TicketExpired
                | Self::ServerRedirectFailed
        )
    }

    pub const fn blocking(self) -> bool {
        matches!(
            self,
            Self::AccountBlocked
                | Self::IpBlocked
                | Self::PlayerBlocked
                | Self::Maintenance
                | Self::AccountBanned
                | Self::PendingReview
                | Self::VersionIncompatible
                | Self::Unauthorized
                | Self::SessionKicked
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionKickCategory {
    ConcurrentLogin,
    Banned,
    Maintenance,
    ServerOffline,
    Unknown,
}

pub fn classify_display_error_code(
    normalized_code: &str,
    operation: Option<MyServerOperation>,
    message_type: Option<MessageType>,
) -> MyServerErrorKind {
    let code = normalized_code;
    if code.is_empty() {
        return fallback_display_error_kind(operation, message_type);
    }
    if code.contains("IP_BLOCKED") || code.contains("IP_BANNED") {
        return MyServerErrorKind::IpBlocked;
    }
    if code.contains("ACCOUNT_BLOCKED") {
        return MyServerErrorKind::AccountBlocked;
    }
    if code.contains("PLAYER_BLOCKED") {
        return MyServerErrorKind::PlayerBlocked;
    }
    if code.contains("BLOCKLIST_UNAVAILABLE") || code.contains("DYNAMIC_BLACKLIST_UNAVAILABLE") {
        return MyServerErrorKind::BlocklistUnavailable;
    }
    if code.contains("MAINTENANCE") {
        return MyServerErrorKind::Maintenance;
    }
    if code.contains("REDIRECT") {
        return MyServerErrorKind::ServerRedirectFailed;
    }
    if code.contains("KICK")
        || code.contains("CONCURRENT_LOGIN")
        || code.contains("LOGIN_ELSEWHERE")
    {
        return MyServerErrorKind::SessionKicked;
    }
    if code.contains("VERSION_INCOMPATIBLE") || code.contains("CLIENT_VERSION") {
        return MyServerErrorKind::VersionIncompatible;
    }
    if code.contains("PENDING_REVIEW")
        || code.contains("UNDER_REVIEW")
        || code.contains("REVIEWING")
    {
        return MyServerErrorKind::PendingReview;
    }
    if code.contains("CHARACTER_LIMIT")
        || code.contains("CHARACTER_COUNT_LIMIT")
        || code.contains("CHARACTER_QUOTA")
        || code.contains("TOO_MANY_CHARACTERS")
    {
        return MyServerErrorKind::CharacterLimitReached;
    }
    if code.contains("CHARACTER_NOT_FOUND")
        || code.contains("CHARACTER_NOT_SELECTABLE")
        || code.contains("CHARACTER_UNAVAILABLE")
        || code.contains("CHARACTER_DELETED")
        || code.contains("CHARACTER_BLOCKED")
        || code.contains("CHARACTER_BANNED")
    {
        return MyServerErrorKind::CharacterUnavailable;
    }
    if code.contains("LIFECYCLE") || code.contains("RESTORE") || code.contains("DELETE_COOLDOWN") {
        return MyServerErrorKind::CharacterLifecycleFailed;
    }
    if code.contains("BANNED") || code.contains("SUSPENDED") || code.contains("FORBIDDEN") {
        return MyServerErrorKind::AccountBanned;
    }
    if code.contains("UNAUTHORIZED")
        || code.contains("TOKEN_INVALID")
        || code.contains("TOKEN_EXPIRED")
    {
        return MyServerErrorKind::Unauthorized;
    }
    if code.contains("MISSING_CHARACTER_ID")
        || code.contains("CHARACTER_ID_REQUIRED")
        || code.contains("NO_CHARACTER_ID")
    {
        return MyServerErrorKind::MissingCharacterId;
    }
    if code.contains("TICKET") && code.contains("EXPIRED") {
        return MyServerErrorKind::TicketExpired;
    }
    if code.contains("PREAUTH_MESSAGE_NOT_ALLOWED") {
        return MyServerErrorKind::PreauthMessageNotAllowed;
    }
    if code.contains("MSG_RATE_EXCEEDED") || code.contains("RATE_LIMIT") {
        return MyServerErrorKind::MessageRateExceeded;
    }
    if code.contains("AUTH") {
        return MyServerErrorKind::GameAuthRejected;
    }
    fallback_display_error_kind(operation, message_type)
}

fn fallback_display_error_kind(
    operation: Option<MyServerOperation>,
    message_type: Option<MessageType>,
) -> MyServerErrorKind {
    match (operation, message_type) {
        (_, Some(MessageType::RoomJoinReq | MessageType::RoomJoinRes)) => {
            MyServerErrorKind::RoomJoinFailed
        }
        (_, Some(MessageType::RoomReconnectReq | MessageType::RoomReconnectRes)) => {
            MyServerErrorKind::ServerRedirectFailed
        }
        (_, Some(MessageType::GetCharacterElementsReq | MessageType::GetCharacterElementsRes)) => {
            MyServerErrorKind::CharacterElementsFailed
        }
        (Some(MyServerOperation::CharacterDelete | MyServerOperation::CharacterRestore), _) => {
            MyServerErrorKind::CharacterLifecycleFailed
        }
        (Some(MyServerOperation::CharacterSelect), _) => MyServerErrorKind::CharacterUnavailable,
        (Some(MyServerOperation::TicketRefresh), _) => MyServerErrorKind::TicketExpired,
        (Some(MyServerOperation::GameConnect), _) => MyServerErrorKind::TransportFailed,
        _ => MyServerErrorKind::Unknown,
    }
}

fn sanitize_error_detail(detail: Option<String>) -> Option<String> {
    detail.and_then(|value| {
        let value = value.trim();
        if value.is_empty() {
            None
        } else if detail_contains_secret_marker(value) {
            Some("[redacted sensitive detail]".to_string())
        } else {
            Some(value.chars().take(512).collect())
        }
    })
}

fn detail_contains_secret_marker(value: &str) -> bool {
    let code = normalize_error_code(value);
    code.contains("ACCESS_TOKEN")
        || code.contains("REFRESH_TOKEN")
        || code.contains("PASSWORD")
        || code.contains("TICKET")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameAuthFailureReason {
    TicketExpired,
    MissingCharacterId,
    AccountBlocked,
    CharacterBlocked,
    ProtocolError,
    Unknown,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticFingerprint(String);

impl DiagnosticFingerprint {
    pub fn new(secret: &str) -> Self {
        Self(short_fingerprint(secret.as_bytes()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DiagnosticFingerprint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MyServerDiagnosticSnapshot {
    pub account_login_state: AccountLoginState,
    pub character_selection_state: CharacterSelectionState,
    pub game_connection_state: GameConnectionState,
    pub connection_id: Option<ConnectionId>,
    pub transport: Option<NetworkTransport>,
    pub access_token_fingerprint: Option<String>,
    pub ticket_fingerprint: Option<String>,
    pub ticket_expires_at: Option<String>,
    pub ticket_remaining_seconds: Option<u64>,
    pub player_id: Option<String>,
    pub character_id: Option<String>,
    pub world_id: Option<i64>,
}

impl MyServerDiagnosticSnapshot {
    pub fn from_session(session: &MyServerSession, now: SystemTime) -> Self {
        Self {
            account_login_state: session.account_login_state,
            character_selection_state: session.character_selection_state,
            game_connection_state: session.game_connection_state,
            connection_id: session.connection_id,
            transport: session.transport,
            access_token_fingerprint: session
                .access_token
                .as_deref()
                .and_then(non_empty_string)
                .map(redact_secret_fingerprint),
            ticket_fingerprint: session
                .ticket
                .as_deref()
                .and_then(non_empty_string)
                .map(redact_secret_fingerprint),
            ticket_expires_at: session.ticket_expires_at.clone(),
            ticket_remaining_seconds: session
                .ticket_expiration_time()
                .and_then(|expires_at| expires_at.duration_since(now).ok())
                .map(|duration| duration.as_secs()),
            player_id: session.player_id.clone(),
            character_id: session.character_id.clone(),
            world_id: session.world_id,
        }
    }
}

pub fn redact_secret_fingerprint(secret: &str) -> String {
    DiagnosticFingerprint::new(secret).to_string()
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

pub fn classify_game_auth_failure(error_code: &str) -> GameAuthFailureReason {
    let code = normalize_error_code(error_code);
    if code.is_empty() {
        return GameAuthFailureReason::Unknown;
    }
    if code.contains("TICKET") && code.contains("EXPIRED") {
        return GameAuthFailureReason::TicketExpired;
    }
    if code.contains("MISSING_CHARACTER_ID")
        || code.contains("CHARACTER_ID_REQUIRED")
        || code.contains("NO_CHARACTER_ID")
    {
        return GameAuthFailureReason::MissingCharacterId;
    }
    if (code.contains("ACCOUNT") || code.contains("PLAYER"))
        && (code.contains("BLOCKED")
            || code.contains("BANNED")
            || code.contains("SUSPENDED")
            || code.contains("DISABLED"))
    {
        return GameAuthFailureReason::AccountBlocked;
    }
    if code.contains("CHARACTER")
        && (code.contains("BLOCKED")
            || code.contains("BANNED")
            || code.contains("SUSPENDED")
            || code.contains("DELETED")
            || code.contains("DISABLED"))
    {
        return GameAuthFailureReason::CharacterBlocked;
    }
    if code.contains("PROTOCOL")
        || code.contains("MALFORMED")
        || code.contains("DECODE")
        || code.contains("INVALID_AUTH")
        || code.contains("INVALID_TICKET")
        || code.contains("INVALID_TICKET_FORMAT")
        || code.contains("INVALID_TICKET_PAYLOAD")
        || code.contains("TICKET_CHARACTER_MISMATCH")
        || code.contains("CHARACTER_MISMATCH")
    {
        return GameAuthFailureReason::ProtocolError;
    }
    GameAuthFailureReason::Unknown
}

fn normalize_error_code(value: &str) -> String {
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
    code
}

pub fn parse_ticket_expiration(value: &str) -> Option<SystemTime> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    if let Ok(seconds) = value.parse::<u64>() {
        return Some(UNIX_EPOCH + Duration::from_secs(seconds));
    }

    parse_rfc3339_utc(value)
}

fn parse_rfc3339_utc(value: &str) -> Option<SystemTime> {
    let trimmed = value.trim();
    let date_time = trimmed.strip_suffix('Z').unwrap_or(trimmed);
    let (date, time) = date_time.split_once('T')?;
    let mut date_parts = date.split('-');
    let year = date_parts.next()?.parse::<i32>().ok()?;
    let month = date_parts.next()?.parse::<u32>().ok()?;
    let day = date_parts.next()?.parse::<u32>().ok()?;
    if date_parts.next().is_some() {
        return None;
    }

    let mut time_parts = time.split(':');
    let hour = time_parts.next()?.parse::<u32>().ok()?;
    let minute = time_parts.next()?.parse::<u32>().ok()?;
    let second_part = time_parts.next()?;
    if time_parts.next().is_some() {
        return None;
    }
    let second_text = second_part
        .split_once('.')
        .map_or(second_part, |(second, _)| second);
    let second = second_text.parse::<u32>().ok()?;

    let unix_seconds = unix_seconds_from_ymdhms(year, month, day, hour, minute, second)?;
    Some(UNIX_EPOCH + Duration::from_secs(unix_seconds))
}

fn unix_seconds_from_ymdhms(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
) -> Option<u64> {
    if !(1970..=9999).contains(&year)
        || !(1..=12).contains(&month)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }

    let max_day = days_in_month(year, month)?;
    if day == 0 || day > max_day {
        return None;
    }

    let mut days = 0u64;
    for current_year in 1970..year {
        days += if is_leap_year(current_year) { 366 } else { 365 };
    }
    for current_month in 1..month {
        days += u64::from(days_in_month(year, current_month)?);
    }
    days += u64::from(day - 1);

    Some(days * 86_400 + u64::from(hour) * 3_600 + u64::from(minute) * 60 + u64::from(second))
}

fn days_in_month(year: i32, month: u32) -> Option<u32> {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => Some(31),
        4 | 6 | 9 | 11 => Some(30),
        2 if is_leap_year(year) => Some(29),
        2 => Some(28),
        _ => None,
    }
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
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
    format!("{hash:016x}")[..DIAGNOSTIC_FINGERPRINT_LEN].to_string()
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
    fn state_machine_tracks_successful_login_and_empty_character_create_flow() {
        let mut session = MyServerSession::default();

        session.begin_http_operation(&PendingHttpOperation::GuestLogin {
            connect_game: false,
        });
        assert_eq!(session.account_login_state, AccountLoginState::LoggingIn);

        session.apply_login_response(&LoginResponse {
            ok: true,
            player_id: "plr_1".to_string(),
            guest_id: Some("guest-a".to_string()),
            login_name: None,
            access_token: "access".to_string(),
            refresh_token: Some("refresh".to_string()),
            access_token_expires_at: None,
            refresh_token_expires_at: None,
            ticket: None,
            ticket_expires_at: None,
            game_proxy_host: None,
            game_proxy_port: None,
            services: None,
        });
        assert_eq!(session.account_login_state, AccountLoginState::LoggedIn);
        assert_eq!(
            session.character_selection_state,
            CharacterSelectionState::NotLoaded
        );

        session.begin_http_operation(&PendingHttpOperation::CharacterList);
        assert_eq!(
            session.character_selection_state,
            CharacterSelectionState::Loading
        );
        let needs_character = session.apply_character_list_response(&CharacterListResponse {
            ok: true,
            player_id: "plr_1".to_string(),
            characters: vec![],
        });
        assert!(needs_character);
        assert_eq!(
            session.character_selection_state,
            CharacterSelectionState::NoCharacters
        );

        session.begin_http_operation(&PendingHttpOperation::CharacterCreate);
        assert_eq!(
            session.character_selection_state,
            CharacterSelectionState::Creating
        );
        session.apply_character_create_response(&CharacterCreateResponse {
            ok: true,
            character: test_character("chr_new", "NewRole"),
        });
        assert_eq!(
            session.character_selection_state,
            CharacterSelectionState::AwaitingSelection
        );
    }

    #[test]
    fn state_machine_tracks_character_select_success_and_switch_character_cleanup() {
        let mut session = MyServerSession {
            access_token: Some("access".to_string()),
            player_id: Some("plr_1".to_string()),
            characters: vec![
                test_character("chr_old", "OldRole"),
                test_character("chr_new", "NewRole"),
            ],
            account_login_state: AccountLoginState::LoggedIn,
            character_id: Some("chr_old".to_string()),
            ticket: Some("old-ticket".to_string()),
            current_character: Some(test_character("chr_old", "OldRole")),
            character_selection_state: CharacterSelectionState::Selected,
            game_connection_state: GameConnectionState::Authenticated,
            connected: true,
            authenticated: true,
            pending_http: HashMap::from([
                (
                    RequestId::from_raw(10),
                    PendingHttpRequest {
                        operation: PendingHttpOperation::CharacterSelect {
                            character_id: "chr_new".to_string(),
                            connect_game: false,
                        },
                    },
                ),
                (
                    RequestId::from_raw(11),
                    PendingHttpRequest {
                        operation: PendingHttpOperation::TicketIssue {
                            reconnect_game: true,
                        },
                    },
                ),
                (
                    RequestId::from_raw(12),
                    PendingHttpRequest {
                        operation: PendingHttpOperation::CharacterList,
                    },
                ),
            ]),
            ..Default::default()
        };

        session.begin_http_operation(&PendingHttpOperation::CharacterSelect {
            character_id: "chr_new".to_string(),
            connect_game: false,
        });
        assert_eq!(
            session.character_selection_state,
            CharacterSelectionState::Selecting
        );
        assert_eq!(session.character_id.as_deref(), Some("chr_old"));
        assert_eq!(session.pending_character_id.as_deref(), Some("chr_new"));
        assert_eq!(session.ticket.as_deref(), Some("old-ticket"));

        session.apply_character_select_response(&CharacterSelectResponse {
            ok: true,
            player_id: "plr_1".to_string(),
            character: test_character("chr_new", "NewRole"),
            ticket: "ticket-new".to_string(),
            ticket_expires_at: "2026-06-25T12:15:00.000Z".to_string(),
            game_proxy_host: None,
            game_proxy_port: None,
            services: None,
        });
        assert_eq!(
            session.character_selection_state,
            CharacterSelectionState::Selected
        );
        assert_eq!(session.ticket.as_deref(), Some("ticket-new"));
        assert!(session.pending_character_id.is_none());
        assert_eq!(
            session.game_connection_state,
            GameConnectionState::NotConnected
        );

        session.switch_character();
        assert_eq!(
            session.character_selection_state,
            CharacterSelectionState::AwaitingSelection
        );
        assert_eq!(session.account_login_state, AccountLoginState::LoggedIn);
        assert!(session.ticket.is_none());
        assert!(session.character_id.is_none());
        assert!(session.pending_character_id.is_none());
        assert!(session.pending_http.is_empty());
    }

    #[test]
    fn state_machine_tracks_failures_logout_and_switch_account_cleanup() {
        let mut session = MyServerSession {
            access_token: Some("access".to_string()),
            refresh_token: Some("refresh".to_string()),
            player_id: Some("plr_1".to_string()),
            character_id: Some("chr_1".to_string()),
            ticket: Some("ticket".to_string()),
            characters: vec![test_character("chr_1", "Role")],
            account_login_state: AccountLoginState::LoggedIn,
            character_selection_state: CharacterSelectionState::Selected,
            game_connection_state: GameConnectionState::Authenticated,
            connected: true,
            authenticated: true,
            pending_http: HashMap::from([(
                RequestId::from_raw(20),
                PendingHttpRequest {
                    operation: PendingHttpOperation::CharacterSelect {
                        character_id: "chr_1".to_string(),
                        connect_game: true,
                    },
                },
            )]),
            ..Default::default()
        };

        session.http_operation_failed(&PendingHttpOperation::Login {
            connect_game: false,
        });
        assert_eq!(session.account_login_state, AccountLoginState::LoginFailed);

        session.account_blocked();
        assert_eq!(session.account_login_state, AccountLoginState::Blocked);
        assert!(session.access_token.is_none());
        assert!(session.character_id.is_none());

        session.account_expired();
        assert_eq!(session.account_login_state, AccountLoginState::Expired);

        session.logout();
        assert_eq!(session.account_login_state, AccountLoginState::LoggedOut);
        assert_eq!(
            session.character_selection_state,
            CharacterSelectionState::NotLoaded
        );
        assert!(session.pending_http.is_empty());

        session.access_token = Some("access".to_string());
        session.player_id = Some("plr_1".to_string());
        session.character_id = Some("chr_1".to_string());
        session.ticket = Some("ticket".to_string());
        session.pending_http.insert(
            RequestId::from_raw(21),
            PendingHttpRequest {
                operation: PendingHttpOperation::CharacterList,
            },
        );
        session.switch_account();
        assert_eq!(session.account_login_state, AccountLoginState::LoggedOut);
        assert!(session.access_token.is_none());
        assert!(session.player_id.is_none());
        assert!(session.pending_http.is_empty());
    }

    #[test]
    fn reset_connection_state_clears_connection_and_http_pending() {
        let mut session = MyServerSession {
            connection_id: Some(ConnectionId::from_raw(30)),
            connected: true,
            authenticated: true,
            room_id: Some("room-1".to_string()),
            game_connection_state: GameConnectionState::Authenticated,
            connect_after_login: Some(ConnectPlan {
                transport: NetworkTransport::Tcp,
                host: Some("game.test".to_string()),
                port: Some(14400),
            }),
            reconnect_after_auth: Some(ReconnectPlan {
                cause: ReconnectCause::TransportRecovery,
            }),
            pending_http: HashMap::from([(
                RequestId::from_raw(31),
                PendingHttpRequest {
                    operation: PendingHttpOperation::TicketIssue {
                        reconnect_game: true,
                    },
                },
            )]),
            ..Default::default()
        };

        session.reset_connection_state();

        assert!(session.connection_id.is_none());
        assert!(!session.connected);
        assert!(!session.authenticated);
        assert!(session.room_id.is_none());
        assert!(session.pending_http.is_empty());
        assert!(session.reconnect_after_auth.is_none());
        assert_eq!(
            session.game_connection_state,
            GameConnectionState::NotConnected
        );
        assert!(session.connect_after_login.is_some());

        session.clear_reconnect_plan();
        assert!(session.connect_after_login.is_none());
    }

    #[test]
    fn state_machine_tracks_local_precondition_failures() {
        let mut session = MyServerSession {
            game_connection_state: GameConnectionState::Authenticated,
            ..Default::default()
        };

        session.http_operation_failed(&PendingHttpOperation::CharacterList);
        assert_eq!(
            session.character_selection_state,
            CharacterSelectionState::SelectionFailed
        );

        session.character_selection_state = CharacterSelectionState::Selecting;
        session.pending_character_id = Some("chr_pending".to_string());
        session.http_operation_failed(&PendingHttpOperation::CharacterSelect {
            character_id: "chr_pending".to_string(),
            connect_game: false,
        });
        assert!(session.character_id.is_none());
        assert!(session.pending_character_id.is_none());
        assert_eq!(
            session.character_selection_state,
            CharacterSelectionState::SelectionFailed
        );

        session.http_operation_failed(&PendingHttpOperation::TicketIssue {
            reconnect_game: false,
        });
        assert_eq!(
            session.game_connection_state,
            GameConnectionState::Authenticated
        );

        session.http_operation_failed(&PendingHttpOperation::TicketIssue {
            reconnect_game: true,
        });
        assert_eq!(
            session.game_connection_state,
            GameConnectionState::ReconnectFailed
        );
    }

    #[test]
    fn state_machine_keeps_duplicate_command_state_and_tracks_connection_edges() {
        let mut session = MyServerSession::default();
        session.begin_http_operation(&PendingHttpOperation::Login {
            connect_game: false,
        });
        session.http_operation_failed(&PendingHttpOperation::CharacterList);
        assert_eq!(session.account_login_state, AccountLoginState::LoggingIn);
        assert_eq!(
            session.character_selection_state,
            CharacterSelectionState::SelectionFailed
        );

        session.begin_http_operation(&PendingHttpOperation::CharacterSelect {
            character_id: "chr_a".to_string(),
            connect_game: false,
        });
        session.begin_http_operation(&PendingHttpOperation::CharacterSelect {
            character_id: "chr_b".to_string(),
            connect_game: false,
        });
        assert!(session.character_id.is_none());
        assert_eq!(session.pending_character_id.as_deref(), Some("chr_b"));
        assert!(session.ticket.is_none());
        session.http_operation_failed(&PendingHttpOperation::CharacterSelect {
            character_id: "chr_b".to_string(),
            connect_game: false,
        });
        assert!(session.character_id.is_none());
        assert!(session.pending_character_id.is_none());
        assert_eq!(
            session.character_selection_state,
            CharacterSelectionState::SelectionFailed
        );

        session.begin_ticket_issue(true);
        assert_eq!(
            session.game_connection_state,
            GameConnectionState::Reconnecting
        );
        session.disconnect_cleanup();
        assert_eq!(
            session.game_connection_state,
            GameConnectionState::Disconnected
        );
        session.ticket_issue_failed(true);
        assert_eq!(
            session.game_connection_state,
            GameConnectionState::ReconnectFailed
        );
        session.game_authenticated("plr_1".to_string());
        session.begin_ticket_issue(false);
        assert_eq!(
            session.game_connection_state,
            GameConnectionState::Authenticated
        );
        session.ticket_issue_failed(false);
        assert_eq!(
            session.game_connection_state,
            GameConnectionState::Authenticated
        );

        let connection_id = ConnectionId::new();
        session.begin_connect_game(connection_id, NetworkTransport::Tcp);
        assert_eq!(session.connection_id, Some(connection_id));
        assert_eq!(
            session.game_connection_state,
            GameConnectionState::Connecting
        );
        session.game_connected(NetworkTransport::Tcp);
        assert_eq!(
            session.game_connection_state,
            GameConnectionState::Connected
        );
        session.begin_game_auth();
        assert_eq!(
            session.game_connection_state,
            GameConnectionState::Authenticating
        );
        session.game_authenticated("plr_1".to_string());
        assert_eq!(
            session.game_connection_state,
            GameConnectionState::Authenticated
        );
        session.disconnect_cleanup();
        assert_eq!(
            session.game_connection_state,
            GameConnectionState::Disconnected
        );
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

        let mut session = MyServerSession {
            characters: vec![test_character("chr_profile", "Profiled")],
            character_selection_state: CharacterSelectionState::LoadingProfile,
            ..Default::default()
        };
        session.apply_character_profile_response(&response);

        assert!(session.character_id.is_none());
        assert!(session.current_character.is_none());
        assert_eq!(
            session.character_selection_state,
            CharacterSelectionState::AwaitingSelection
        );
        assert_eq!(
            session.character_profile.as_ref().unwrap().character.name,
            "Profiled"
        );
        assert_eq!(session.character_elements.affinity.wind, 4);
        assert_eq!(session.character_elements.mastery.fire, 6);
        assert!(session.character_elements.snapshot_refreshed_at.is_some());

        session.character_id = Some("chr_profile".to_string());
        session.ticket = Some("ticket".to_string());
        session.character_selection_state = CharacterSelectionState::LoadingProfile;
        session.apply_character_profile_response(&response);

        assert_eq!(session.character_id.as_deref(), Some("chr_profile"));
        assert_eq!(session.world_id, Some(9));
        assert_eq!(session.current_character.as_ref().unwrap().name, "Profiled");
        assert_eq!(
            session.character_selection_state,
            CharacterSelectionState::Selected
        );
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

    #[test]
    fn diagnostic_fingerprint_redacts_secret_values_with_stable_length() {
        let access_token = "access-token-super-secret";
        let ticket = ticket_for_test("plr_1", "chr_1", "2026-06-25T12:15:00.000Z");
        let password = "correct-horse-password";

        for secret in [access_token, ticket.as_str(), password] {
            let fingerprint = redact_secret_fingerprint(secret);
            assert_eq!(fingerprint.len(), DIAGNOSTIC_FINGERPRINT_LEN);
            assert_eq!(fingerprint, redact_secret_fingerprint(secret));
            assert!(!fingerprint.contains(secret));

            let wrapper = DiagnosticFingerprint::new(secret);
            assert_eq!(wrapper.as_str(), fingerprint);
            assert!(!format!("{wrapper:?}").contains(secret));
            assert!(!format!("{wrapper}").contains(secret));
        }
    }

    #[test]
    fn diagnostic_snapshot_uses_fingerprints_without_secret_plaintext() {
        let access_token = "access-token-super-secret";
        let ticket = ticket_for_test("plr_1", "chr_1", "2026-06-25T12:15:00.000Z");
        let mut session = MyServerSession {
            access_token: Some(access_token.to_string()),
            ticket: Some(ticket.clone()),
            ticket_expires_at: Some("2026-06-25T12:15:00.000Z".to_string()),
            character_id: Some("chr_1".to_string()),
            ..Default::default()
        };
        session.begin_connect_game(ConnectionId::from_raw(7), NetworkTransport::Tcp);

        let now = parse_ticket_expiration("2026-06-25T12:14:00.000Z").unwrap();
        let snapshot = MyServerDiagnosticSnapshot::from_session(&session, now);
        let debug_text = format!("{snapshot:?}");

        assert_eq!(
            snapshot.access_token_fingerprint.as_deref().unwrap().len(),
            DIAGNOSTIC_FINGERPRINT_LEN
        );
        assert_eq!(
            snapshot.ticket_fingerprint.as_deref().unwrap().len(),
            DIAGNOSTIC_FINGERPRINT_LEN
        );
        assert_eq!(snapshot.ticket_remaining_seconds, Some(60));
        assert!(!debug_text.contains(access_token));
        assert!(!debug_text.contains(ticket.as_str()));
    }

    #[test]
    fn ticket_refresh_expiration_helper_uses_server_time_or_payload_exp() {
        let now = parse_ticket_expiration("2026-06-25T12:14:31.000Z").unwrap();
        let soon = parse_ticket_expiration("2026-06-25T12:15:00.000Z").unwrap();
        assert_eq!(
            parse_ticket_expiration("2026-06-25T12:15:00.000Z"),
            Some(soon)
        );
        assert_eq!(
            parse_ticket_expiration("1782399300"),
            Some(UNIX_EPOCH + Duration::from_secs(1_782_399_300))
        );

        let mut session = MyServerSession {
            ticket: Some(ticket_for_test(
                "plr_1",
                "chr_1",
                "2026-06-25T12:20:00.000Z",
            )),
            ticket_expires_at: Some("2026-06-25T12:15:00.000Z".to_string()),
            character_id: Some("chr_1".to_string()),
            ..Default::default()
        };
        assert!(session.needs_ticket_refresh(now, Duration::from_secs(30)));

        session.ticket_expires_at = Some("2026-06-25T12:15:02.000Z".to_string());
        assert!(!session.needs_ticket_refresh(now, Duration::from_secs(30)));

        session.ticket_expires_at = None;
        session.ticket = Some(ticket_for_test(
            "plr_1",
            "chr_1",
            "2026-06-25T12:15:00.000Z",
        ));
        assert!(session.needs_ticket_refresh(now, Duration::from_secs(30)));

        session.ticket_expires_at = Some("not-a-date".to_string());
        session.ticket = Some("invalid-ticket".to_string());
        assert!(!session.needs_ticket_refresh(now, Duration::from_secs(30)));

        session.character_id = None;
        session.ticket_expires_at = Some("2026-06-25T12:15:00.000Z".to_string());
        assert!(!session.needs_ticket_refresh(now, Duration::from_secs(30)));
    }

    #[test]
    fn classifies_game_auth_failure_codes() {
        assert_eq!(
            classify_game_auth_failure("TICKET_EXPIRED"),
            GameAuthFailureReason::TicketExpired
        );
        assert_eq!(
            classify_game_auth_failure("MISSING_CHARACTER_ID"),
            GameAuthFailureReason::MissingCharacterId
        );
        assert_eq!(
            classify_game_auth_failure("ACCOUNT_BLOCKED"),
            GameAuthFailureReason::AccountBlocked
        );
        assert_eq!(
            classify_game_auth_failure("CHARACTER_BANNED"),
            GameAuthFailureReason::CharacterBlocked
        );
        assert_eq!(
            classify_game_auth_failure("INVALID_TICKET_PAYLOAD"),
            GameAuthFailureReason::ProtocolError
        );
        assert_eq!(
            classify_game_auth_failure("SOMETHING_ELSE"),
            GameAuthFailureReason::Unknown
        );
    }

    #[test]
    fn maps_display_error_codes_to_stable_kinds_and_keys() {
        for (code, expected_kind, expected_key, retryable, blocking) in [
            (
                "ACCOUNT_BLOCKED",
                MyServerErrorKind::AccountBlocked,
                "myserver.error.account_blocked",
                false,
                true,
            ),
            (
                "IP_BLOCKED",
                MyServerErrorKind::IpBlocked,
                "myserver.error.ip_blocked",
                false,
                true,
            ),
            (
                "PLAYER_BLOCKED",
                MyServerErrorKind::PlayerBlocked,
                "myserver.error.player_blocked",
                false,
                true,
            ),
            (
                "BLOCKLIST_UNAVAILABLE",
                MyServerErrorKind::BlocklistUnavailable,
                "myserver.error.blocklist_unavailable",
                true,
                false,
            ),
            (
                "MISSING_CHARACTER_ID",
                MyServerErrorKind::MissingCharacterId,
                "myserver.error.missing_character_id",
                false,
                false,
            ),
            (
                "PREAUTH_MESSAGE_NOT_ALLOWED",
                MyServerErrorKind::PreauthMessageNotAllowed,
                "myserver.error.preauth_message_not_allowed",
                false,
                false,
            ),
            (
                "MSG_RATE_EXCEEDED",
                MyServerErrorKind::MessageRateExceeded,
                "myserver.error.message_rate_exceeded",
                true,
                false,
            ),
            (
                "CHARACTER_DELETE_COOLDOWN",
                MyServerErrorKind::CharacterLifecycleFailed,
                "myserver.error.character_lifecycle_failed",
                false,
                false,
            ),
            (
                "CHARACTER_LIMIT_REACHED",
                MyServerErrorKind::CharacterLimitReached,
                "myserver.error.character_limit_reached",
                false,
                false,
            ),
            (
                "CHARACTER_BANNED",
                MyServerErrorKind::CharacterUnavailable,
                "myserver.error.character_unavailable",
                false,
                false,
            ),
            (
                "CHARACTER_BLOCKED",
                MyServerErrorKind::CharacterUnavailable,
                "myserver.error.character_unavailable",
                false,
                false,
            ),
            (
                "CHARACTER_DELETED",
                MyServerErrorKind::CharacterUnavailable,
                "myserver.error.character_unavailable",
                false,
                false,
            ),
            (
                "TICKET_EXPIRED",
                MyServerErrorKind::TicketExpired,
                "myserver.error.ticket_expired",
                true,
                false,
            ),
            (
                "SOMETHING_NEW",
                MyServerErrorKind::Unknown,
                "myserver.error.unknown",
                false,
                false,
            ),
        ] {
            let error = MyServerDisplayError::from_error_code(
                MyServerErrorSource::Game,
                Some(MyServerOperation::GameRequest),
                None,
                None,
                None,
                code,
                Some("diagnostic".to_string()),
            );

            assert_eq!(error.kind, expected_kind, "{code}");
            assert_eq!(error.message_key, expected_key, "{code}");
            assert_eq!(error.retryable, retryable, "{code}");
            assert_eq!(error.blocking, blocking, "{code}");
            assert_eq!(error.error_code.as_deref(), Some(code));
        }
    }

    #[test]
    fn character_banned_display_error_does_not_block_account() {
        let error = MyServerDisplayError::from_error_code(
            MyServerErrorSource::Http,
            Some(MyServerOperation::CharacterSelect),
            None,
            None,
            None,
            "CHARACTER_BANNED",
            Some("character is banned".to_string()),
        );

        assert_eq!(error.kind, MyServerErrorKind::CharacterUnavailable);
        assert_eq!(error.message_key, "myserver.error.character_unavailable");
        assert!(!error.blocking);
    }

    #[test]
    fn display_error_detail_redacts_secret_markers() {
        let error = MyServerDisplayError::from_error_code(
            MyServerErrorSource::Http,
            Some(MyServerOperation::Login),
            None,
            None,
            None,
            "PLAYER_BLOCKED",
            Some("ticket=secret password=secret access_token=secret".to_string()),
        );

        assert_eq!(error.detail.as_deref(), Some("[redacted sensitive detail]"));
    }

    #[test]
    fn maps_network_and_protocol_display_errors() {
        let http = MyServerDisplayError::http_status(
            MyServerOperation::Login,
            503,
            None,
            Some("service unavailable".to_string()),
        );
        assert_eq!(http.kind, MyServerErrorKind::HttpStatus);
        assert_eq!(http.source, MyServerErrorSource::Http);
        assert_eq!(http.http_status, Some(503));
        assert!(http.retryable);

        let json = MyServerDisplayError::json_parse(
            MyServerOperation::Login,
            Some("expected value".to_string()),
        );
        assert_eq!(json.kind, MyServerErrorKind::JsonParseFailed);
        assert_eq!(json.message_key, "myserver.error.json_parse_failed");

        let proto = MyServerDisplayError::protobuf_decode(
            Some(MessageType::AuthRes),
            Some(7),
            Some("decode failed".to_string()),
        );
        assert_eq!(proto.kind, MyServerErrorKind::ProtobufDecodeFailed);
        assert_eq!(proto.message_type, Some(MessageType::AuthRes));
        assert_eq!(proto.seq, Some(7));

        let timeout = MyServerDisplayError::transport(
            MyServerOperation::GameConnect,
            Some("connect timeout after 10s".to_string()),
        );
        assert_eq!(timeout.kind, MyServerErrorKind::ConnectionTimeout);
        assert!(timeout.retryable);

        let transport = MyServerDisplayError::transport(
            MyServerOperation::GameRequest,
            Some("connection reset".to_string()),
        );
        assert_eq!(transport.kind, MyServerErrorKind::TransportFailed);
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
