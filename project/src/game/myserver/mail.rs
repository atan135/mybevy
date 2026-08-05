use std::{
    collections::{HashMap, HashSet},
    net::Ipv6Addr,
    str::FromStr,
    time::Duration,
};

use bevy::prelude::*;
use serde::Deserialize;

use crate::framework::network::{HttpMethod, HttpRequest, NetworkCommand, NetworkEvent, RequestId};

use super::{
    chat::{ChatClientState, ChatEvent, ChatInbound},
    types::{ClientServiceEndpoint, ClientServices, MyServerSession},
};

pub const MAIL_API_PATH: &str = "/api/v1/mails";
pub const MAIL_GAME_TICKET_HEADER: &str = "X-Game-Ticket";
pub const PRODUCTION_MAIL_HOST: &str = "api.game.zergzerg.cn";
pub const PRODUCTION_MAIL_PORT: u16 = 443;
pub const MAIL_CLAIM_STATUSES: [&str; 7] = [
    "claimed",
    "processing",
    "retryable_failure",
    "blocked_capacity",
    "permanent_failure",
    "reconciliation_pending",
    "manual_review",
];
const MAIL_READ_TIMEOUT: Duration = Duration::from_secs(8);
const MAIL_CLAIM_TIMEOUT: Duration = Duration::from_secs(12);
const MAIL_MAX_RESPONSE_BYTES: usize = 256 * 1024;
const MAX_RECONCILIATION_POLLS: u8 = 3;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MailHttpEndpoint {
    base_url: String,
}

impl MailHttpEndpoint {
    pub fn from_services(
        services: Option<&ClientServices>,
        allow_insecure_http: bool,
        enforce_production_descriptor: bool,
    ) -> Result<Option<Self>, String> {
        let Some(service) = services.and_then(|services| services.mail.as_ref()) else {
            return Ok(None);
        };
        Self::from_service(service, allow_insecure_http, enforce_production_descriptor).map(Some)
    }

    pub fn from_service(
        service: &ClientServiceEndpoint,
        allow_insecure_http: bool,
        enforce_production_descriptor: bool,
    ) -> Result<Self, String> {
        let protocol = service
            .protocol
            .as_deref()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        if protocol != "https" && !(protocol == "http" && allow_insecure_http) {
            return Err(
                "services.mail protocol must be https; http requires the explicit local policy"
                    .to_string(),
            );
        }

        let host = service.host.as_deref().unwrap_or_default().trim();
        if host.is_empty()
            || host.chars().any(char::is_whitespace)
            || host.contains(['/', '\\', '?', '#', '@', '[', ']'])
        {
            return Err("services.mail host must be a bare hostname or IP address".to_string());
        }

        let authority_host = if host.contains(':') {
            Ipv6Addr::from_str(host)
                .map(|_| format!("[{host}]"))
                .map_err(|_| "services.mail host must not include a port".to_string())?
        } else {
            host.to_string()
        };
        let port = service
            .port
            .filter(|port| *port > 0)
            .ok_or_else(|| "services.mail port must be between 1 and 65535".to_string())?;
        if enforce_production_descriptor
            && (protocol != "https"
                || !host.eq_ignore_ascii_case(PRODUCTION_MAIL_HOST)
                || port != PRODUCTION_MAIL_PORT)
        {
            return Err(
                "production services.mail must be api.game.zergzerg.cn:443 over https".to_string(),
            );
        }

        let port_suffix = match (protocol.as_str(), port) {
            ("https", 443) | ("http", 80) => String::new(),
            _ => format!(":{port}"),
        };
        Ok(Self {
            base_url: format!("{protocol}://{authority_host}{port_suffix}{MAIL_API_PATH}"),
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    fn url_for(&self, suffix: &str) -> String {
        format!("{}{}", self.base_url, suffix)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MailListQuery {
    pub status: Option<MailListStatus>,
    pub limit: Option<u8>,
    pub offset: Option<u32>,
}

impl MailListQuery {
    fn normalized(&self) -> Result<(Option<MailListStatus>, u8, u32), String> {
        let limit = self.limit.unwrap_or(50);
        if !(1..=50).contains(&limit) {
            return Err("mail list limit must be between 1 and 50".to_string());
        }
        let offset = self.offset.unwrap_or(0);
        if offset > 10_000 {
            return Err("mail list offset must be between 0 and 10000".to_string());
        }
        Ok((self.status, limit, offset))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MailListStatus {
    Unread,
    Read,
    Claiming,
    Claimed,
}

impl MailListStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Unread => "unread",
            Self::Read => "read",
            Self::Claiming => "claiming",
            Self::Claimed => "claimed",
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct MailSummary {
    pub mail_id: String,
    #[serde(default)]
    pub sender: MailSender,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub mail_type: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub has_attachments: bool,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub read_at: Option<String>,
    #[serde(default)]
    pub claimed_at: Option<String>,
    #[serde(default)]
    pub expires_at: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct MailSender {
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MailDetail {
    #[serde(flatten)]
    pub summary: MailSummary,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub attachments: Vec<MailAttachment>,
    #[serde(default)]
    pub claim: MailClaimSummary,
}

#[derive(Clone, Debug, Deserialize)]
pub struct MailAttachment {
    #[serde(default)]
    pub r#type: String,
    #[serde(default)]
    pub id: Option<i64>,
    #[serde(default)]
    pub count: i64,
    #[serde(default)]
    pub binded: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct MailClaimSummary {
    #[serde(default)]
    pub claim_status: Option<String>,
    #[serde(default)]
    pub already_claimed: bool,
    #[serde(default)]
    pub processing: bool,
    #[serde(default)]
    pub retryable: bool,
    #[serde(default)]
    pub player_retryable: bool,
    #[serde(default)]
    pub attempts: u32,
    #[serde(default)]
    pub result_state: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct MailListResponse {
    ok: bool,
    #[serde(default)]
    mails: Vec<MailSummary>,
    #[serde(default)]
    unread_count: u32,
    #[serde(default)]
    pagination: MailPagination,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct MailPagination {
    #[serde(default)]
    pub limit: u8,
    #[serde(default)]
    pub offset: u32,
    #[serde(default)]
    pub next_offset: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
struct MailDetailResponse {
    ok: bool,
    mail: MailDetail,
}

#[derive(Clone, Debug, Deserialize)]
struct MailReadResponse {
    ok: bool,
    mail_id: String,
    status: String,
    #[serde(default)]
    read_at: Option<String>,
    #[serde(default)]
    already_read: bool,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Deserialize)]
struct MailClaimResponse {
    #[serde(default)]
    ok: bool,
    #[serde(default)]
    mail_id: String,
    #[serde(default)]
    claim_status: Option<String>,
    #[serde(default)]
    claimed: bool,
    #[serde(default)]
    already_claimed: bool,
    #[serde(default)]
    processing: bool,
    #[serde(default)]
    retryable: bool,
    #[serde(default)]
    player_retryable: bool,
    #[serde(default)]
    request_id: Option<String>,
    #[serde(default)]
    attempts: u32,
    #[serde(default)]
    result_state: Option<String>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    attachments: Vec<MailAttachment>,
    #[serde(default)]
    claimed_at: Option<String>,
    #[serde(default)]
    completed_at: Option<String>,
    #[serde(default)]
    read_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MailAvailability {
    Unavailable { reason: String },
    AwaitingCharacterTicket,
    Ready,
}

impl Default for MailAvailability {
    fn default() -> Self {
        Self::Unavailable {
            reason: "mail service descriptor is unavailable".to_string(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MailOperation {
    List,
    Detail,
    MarkRead,
    Claim,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MailListLoadState {
    #[default]
    Idle,
    InitialLoading,
    Refreshing,
    LoadingMore,
    Ready,
    Empty,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MailClientError {
    pub operation: MailOperation,
    pub status: Option<u16>,
    pub code: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MailClaimReconciliation {
    pub mail_id: String,
    pub polls_completed: u8,
    next_poll: Option<Timer>,
}

#[derive(Clone, Debug, Resource)]
pub struct MailClientState {
    pub availability: MailAvailability,
    pub endpoint: Option<MailHttpEndpoint>,
    pub mails: Vec<MailSummary>,
    pub unread_count: Option<u32>,
    pub pagination: Option<MailPagination>,
    pub selected_mail: Option<MailDetail>,
    pub list_stale: bool,
    pub list_load_state: MailListLoadState,
    pub claim_reconciliation: Option<MailClaimReconciliation>,
    pub last_error: Option<MailClientError>,
    pending: HashMap<RequestId, PendingMailRequest>,
    identity: Option<MailIdentity>,
    desired_list_generation: u64,
    active_list_query: Option<MailListQuery>,
}

impl Default for MailClientState {
    fn default() -> Self {
        Self {
            availability: MailAvailability::default(),
            endpoint: None,
            mails: Vec::new(),
            unread_count: None,
            pagination: None,
            selected_mail: None,
            list_stale: false,
            list_load_state: MailListLoadState::Idle,
            claim_reconciliation: None,
            last_error: None,
            pending: HashMap::new(),
            identity: None,
            desired_list_generation: 0,
            active_list_query: None,
        }
    }
}

impl MailClientState {
    pub fn is_available(&self) -> bool {
        matches!(self.availability, MailAvailability::Ready)
    }

    pub fn authoritative_unread_count(&self) -> Option<u32> {
        (self.is_available()
            && !self.list_stale
            && matches!(
                self.list_load_state,
                MailListLoadState::Ready
                    | MailListLoadState::Empty
                    | MailListLoadState::LoadingMore
            ))
        .then_some(self.unread_count)
        .flatten()
    }

    pub fn contains_authoritative_mail(&self, mail_id: &str) -> bool {
        self.mails.iter().any(|mail| mail.mail_id == mail_id)
    }

    pub fn refresh_query(&self) -> MailListQuery {
        self.active_list_query
            .as_ref()
            .map(|query| MailListQuery {
                status: query.status,
                limit: query.limit,
                offset: Some(0),
            })
            .unwrap_or_default()
    }

    pub fn next_page_query(&self) -> Option<MailListQuery> {
        let active = self.active_list_query.as_ref()?;
        let next_offset = self.pagination.as_ref()?.next_offset?;
        (!self.list_stale
            && !matches!(
                self.list_load_state,
                MailListLoadState::InitialLoading
                    | MailListLoadState::Refreshing
                    | MailListLoadState::LoadingMore
            ))
        .then_some(MailListQuery {
            status: active.status,
            limit: active.limit,
            offset: Some(next_offset),
        })
    }

    #[cfg(test)]
    fn list_generation(&self) -> u64 {
        self.desired_list_generation
    }

    #[cfg(test)]
    pub(crate) fn ready_for_test() -> Self {
        Self::with_availability_for_test(MailAvailability::Ready)
    }

    #[cfg(test)]
    pub(crate) fn with_availability_for_test(availability: MailAvailability) -> Self {
        Self {
            availability,
            ..default()
        }
    }

    #[cfg(test)]
    pub(crate) fn ready_with_reconciliation_for_test(mail_id: impl Into<String>) -> Self {
        Self {
            availability: MailAvailability::Ready,
            claim_reconciliation: Some(MailClaimReconciliation {
                mail_id: mail_id.into(),
                polls_completed: 1,
                next_poll: None,
            }),
            ..default()
        }
    }

    #[cfg(test)]
    pub(crate) fn ready_with_list_for_test(
        mails: Vec<MailSummary>,
        unread_count: u32,
        pagination: MailPagination,
    ) -> Self {
        let limit = if pagination.limit == 0 {
            50
        } else {
            pagination.limit
        };
        let list_load_state = if mails.is_empty() {
            MailListLoadState::Empty
        } else {
            MailListLoadState::Ready
        };
        Self {
            availability: MailAvailability::Ready,
            mails,
            unread_count: Some(unread_count),
            pagination: Some(pagination),
            list_load_state,
            active_list_query: Some(MailListQuery {
                status: None,
                limit: Some(limit),
                offset: Some(0),
            }),
            ..default()
        }
    }

    fn reset_for_identity(&mut self, identity: MailIdentity, availability: MailAvailability) {
        self.availability = availability;
        self.endpoint = identity.endpoint.clone();
        self.mails.clear();
        self.unread_count = None;
        self.pagination = None;
        self.selected_mail = None;
        self.list_stale = false;
        self.list_load_state = MailListLoadState::Idle;
        self.claim_reconciliation = None;
        self.last_error = None;
        self.pending.clear();
        self.identity = Some(identity);
        self.desired_list_generation = 0;
        self.active_list_query = None;
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MailIdentity {
    player_id: Option<String>,
    character_id: Option<String>,
    endpoint: Option<MailHttpEndpoint>,
    endpoint_error: Option<String>,
}

#[derive(Clone, Debug)]
enum PendingMailRequest {
    List {
        query: MailListQuery,
        generation: u64,
        identity: Option<MailIdentity>,
    },
    Detail {
        mail_id: String,
        reconciliation_poll: bool,
    },
    MarkRead {
        mail_id: String,
    },
    Claim {
        mail_id: String,
    },
}

impl PendingMailRequest {
    fn operation(&self) -> MailOperation {
        match self {
            Self::List { .. } => MailOperation::List,
            Self::Detail { .. } => MailOperation::Detail,
            Self::MarkRead { .. } => MailOperation::MarkRead,
            Self::Claim { .. } => MailOperation::Claim,
        }
    }
}

#[derive(Clone, Debug, Message)]
pub enum MailClientCommand {
    LoadList {
        query: MailListQuery,
    },
    LoadMail {
        mail_id: String,
    },
    MarkRead {
        mail_id: String,
    },
    Claim {
        mail_id: String,
    },
    MailNotifyPush,
    #[doc(hidden)]
    PollClaimReconciliation {
        mail_id: String,
    },
}

#[derive(Clone, Debug, Message)]
pub enum MailClientEvent {
    AvailabilityChanged {
        availability: MailAvailability,
    },
    ListLoaded {
        unread_count: u32,
    },
    MailLoaded {
        mail_id: String,
    },
    MailRead {
        mail_id: String,
        already_read: bool,
    },
    ClaimUpdated {
        mail_id: String,
        claim_status: Option<String>,
    },
    ClaimReconciliationStarted {
        mail_id: String,
    },
    ClaimReconciliationSettled {
        mail_id: String,
        claim_status: Option<String>,
    },
    RefreshRequired,
    RequestFailed {
        error: MailClientError,
    },
}

pub(crate) struct MailPlugin;

impl Plugin for MailPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MailClientState>()
            .add_message::<NetworkCommand>()
            .add_message::<NetworkEvent>()
            .add_message::<ChatEvent>()
            .add_message::<MailClientCommand>()
            .add_message::<MailClientEvent>()
            .add_systems(
                PostUpdate,
                (
                    sync_mail_session,
                    forward_chat_mail_notifications,
                    handle_mail_commands,
                    handle_mail_network_events,
                    drive_claim_reconciliation,
                )
                    .chain(),
            );
    }
}

/// Chat payloads are intentionally not decoded into a mail. A push can only invalidate
/// cached list and unread state; detail data always comes from the mail HTTPS API.
pub fn forward_chat_inbound(
    inbound: &ChatInbound,
    commands: &mut MessageWriter<MailClientCommand>,
) {
    if matches!(inbound, ChatInbound::MailNotifyPush(_)) {
        commands.write(MailClientCommand::MailNotifyPush);
    }
}

/// Coalesces chat notifications for the current session into one authoritative HTTPS refresh.
/// The chat event deliberately carries no packet body, title, or preview data.
fn forward_chat_mail_notifications(
    mut chat_events: MessageReader<ChatEvent>,
    chat_state: Option<Res<ChatClientState>>,
    mut mail_commands: MessageWriter<MailClientCommand>,
) {
    let current_generation = chat_state.map(|state| state.endpoint_generation);
    if chat_events.read().any(|event| {
        matches!(
            event,
            ChatEvent::MailNotifyPush { generation }
                if current_generation.is_none_or(|current| *generation == current)
        )
    }) {
        mail_commands.write(MailClientCommand::MailNotifyPush);
    }
}

fn sync_mail_session(
    session: Res<MyServerSession>,
    mut state: ResMut<MailClientState>,
    mut network_commands: MessageWriter<NetworkCommand>,
    mut events: MessageWriter<MailClientEvent>,
) {
    let identity = MailIdentity {
        player_id: session.player_id.clone(),
        character_id: session.character_id.clone(),
        endpoint: session.mail_endpoint.clone(),
        endpoint_error: session.mail_endpoint_error.clone(),
    };
    let availability = mail_availability(&identity, &session);
    if state.identity.as_ref() != Some(&identity) {
        for request_id in state.pending.keys() {
            network_commands.write(NetworkCommand::CancelHttp {
                request_id: *request_id,
            });
        }
        state.reset_for_identity(identity, availability.clone());
        events.write(MailClientEvent::AvailabilityChanged { availability });
    } else if state.availability != availability {
        state.availability = availability.clone();
        events.write(MailClientEvent::AvailabilityChanged { availability });
    }
}

fn mail_availability(identity: &MailIdentity, session: &MyServerSession) -> MailAvailability {
    if let Some(reason) = identity.endpoint_error.clone() {
        return MailAvailability::Unavailable { reason };
    }
    if identity.endpoint.is_none() {
        return MailAvailability::Unavailable {
            reason: "mail service descriptor is unavailable".to_string(),
        };
    }
    if session
        .ticket
        .as_deref()
        .is_none_or(|ticket| ticket.trim().is_empty())
        || identity
            .character_id
            .as_deref()
            .is_none_or(|character_id| character_id.trim().is_empty())
    {
        return MailAvailability::AwaitingCharacterTicket;
    }
    MailAvailability::Ready
}

fn handle_mail_commands(
    session: Res<MyServerSession>,
    mut state: ResMut<MailClientState>,
    mut commands: MessageReader<MailClientCommand>,
    mut network_commands: MessageWriter<NetworkCommand>,
    mut events: MessageWriter<MailClientEvent>,
) {
    for command in commands.read() {
        match command {
            MailClientCommand::LoadList { query } => {
                start_list_request(
                    &session,
                    &mut state,
                    query.clone(),
                    &mut network_commands,
                    &mut events,
                );
            }
            MailClientCommand::LoadMail { mail_id } => {
                start_detail_request(
                    &session,
                    &mut state,
                    mail_id,
                    false,
                    &mut network_commands,
                    &mut events,
                );
            }
            MailClientCommand::MarkRead { mail_id } => {
                if !valid_mail_id(mail_id) {
                    reject_request(
                        &mut state,
                        &mut events,
                        MailOperation::MarkRead,
                        "INVALID_MAIL_ID",
                    );
                    continue;
                }
                if state.pending.values().any(|pending| {
                    matches!(pending, PendingMailRequest::MarkRead { mail_id: pending_id } if pending_id == mail_id)
                }) {
                    continue;
                }
                let Some((endpoint, ticket)) =
                    request_context(&state, &session, MailOperation::MarkRead, &mut events)
                else {
                    continue;
                };
                let request = build_mutation_request(
                    &endpoint,
                    &ticket,
                    mail_id,
                    "/read",
                    HttpMethod::Put,
                    MAIL_READ_TIMEOUT,
                );
                queue_request(
                    &mut state,
                    PendingMailRequest::MarkRead {
                        mail_id: mail_id.clone(),
                    },
                    request,
                    &mut network_commands,
                );
            }
            MailClientCommand::Claim { mail_id } => {
                if !valid_mail_id(mail_id) {
                    reject_request(
                        &mut state,
                        &mut events,
                        MailOperation::Claim,
                        "INVALID_MAIL_ID",
                    );
                    continue;
                }
                if state
                    .claim_reconciliation
                    .as_ref()
                    .is_some_and(|reconciliation| reconciliation.mail_id == *mail_id)
                {
                    events.write(MailClientEvent::ClaimReconciliationStarted {
                        mail_id: mail_id.clone(),
                    });
                    continue;
                }
                if state.pending.values().any(|pending| {
                    matches!(pending, PendingMailRequest::Claim { mail_id: pending_id } if pending_id == mail_id)
                }) {
                    continue;
                }
                let Some((endpoint, ticket)) =
                    request_context(&state, &session, MailOperation::Claim, &mut events)
                else {
                    continue;
                };
                let request = build_mutation_request(
                    &endpoint,
                    &ticket,
                    mail_id,
                    "/claim",
                    HttpMethod::Post,
                    MAIL_CLAIM_TIMEOUT,
                );
                queue_request(
                    &mut state,
                    PendingMailRequest::Claim {
                        mail_id: mail_id.clone(),
                    },
                    request,
                    &mut network_commands,
                );
            }
            MailClientCommand::MailNotifyPush => {
                events.write(MailClientEvent::RefreshRequired);
                let query = state.refresh_query();
                start_list_request(
                    &session,
                    &mut state,
                    query,
                    &mut network_commands,
                    &mut events,
                );
            }
            MailClientCommand::PollClaimReconciliation { mail_id } => {
                if state
                    .claim_reconciliation
                    .as_ref()
                    .is_some_and(|reconciliation| reconciliation.mail_id == *mail_id)
                {
                    if let Some(reconciliation) = state.claim_reconciliation.as_mut() {
                        reconciliation.next_poll = None;
                    }
                    start_detail_request(
                        &session,
                        &mut state,
                        mail_id,
                        true,
                        &mut network_commands,
                        &mut events,
                    );
                }
            }
        }
    }
}

fn handle_mail_network_events(
    session: Res<MyServerSession>,
    mut state: ResMut<MailClientState>,
    mut network_events: MessageReader<NetworkEvent>,
    mut network_commands: MessageWriter<NetworkCommand>,
    mut events: MessageWriter<MailClientEvent>,
) {
    for event in network_events.read() {
        match event {
            NetworkEvent::HttpResponse(response) => {
                let Some(pending) = state.pending.remove(&response.request_id) else {
                    continue;
                };
                handle_mail_response(
                    &session,
                    &mut state,
                    pending,
                    response.status,
                    &response.body,
                    &mut network_commands,
                    &mut events,
                );
            }
            NetworkEvent::HttpError { request_id, .. } => {
                let Some(pending) = state.pending.remove(request_id) else {
                    continue;
                };
                match pending {
                    PendingMailRequest::Claim { mail_id } => {
                        begin_claim_reconciliation(&mut state, mail_id, &mut events);
                    }
                    PendingMailRequest::List {
                        generation,
                        identity,
                        ..
                    } if generation != state.desired_list_generation
                        || identity != state.identity => {}
                    pending => {
                        if matches!(pending, PendingMailRequest::List { .. }) {
                            state.list_stale = true;
                            state.list_load_state = MailListLoadState::Failed;
                        }
                        let error = MailClientError {
                            operation: pending.operation(),
                            status: None,
                            code: "MAIL_NETWORK_UNAVAILABLE".to_string(),
                        };
                        state.last_error = Some(error.clone());
                        events.write(MailClientEvent::RequestFailed { error });
                    }
                }
            }
            _ => {}
        }
    }
}

fn handle_mail_response(
    session: &MyServerSession,
    state: &mut MailClientState,
    pending: PendingMailRequest,
    status: u16,
    body: &[u8],
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<MailClientEvent>,
) {
    match pending {
        PendingMailRequest::List {
            query,
            generation,
            identity,
        } => {
            if generation != state.desired_list_generation || identity != state.identity {
                return;
            }
            let Some(response) =
                parse_success::<MailListResponse>(state, MailOperation::List, status, body, events)
            else {
                state.list_stale = true;
                state.list_load_state = MailListLoadState::Failed;
                return;
            };
            if !response.ok || !list_response_matches_request(&query, &response) {
                state.list_stale = true;
                state.list_load_state = MailListLoadState::Failed;
                reject_request(state, events, MailOperation::List, "MAIL_LIST_REJECTED");
                return;
            }
            let offset = query.offset.unwrap_or(0);
            if offset == 0 {
                state.mails = deduplicate_mail_page(response.mails);
            } else {
                merge_mail_page(&mut state.mails, response.mails);
            }
            state.unread_count = Some(response.unread_count);
            state.pagination = Some(response.pagination);
            state.last_error = None;
            state.list_stale = false;
            state.list_load_state = if state.mails.is_empty() {
                MailListLoadState::Empty
            } else {
                MailListLoadState::Ready
            };
            events.write(MailClientEvent::ListLoaded {
                unread_count: response.unread_count,
            });
        }
        PendingMailRequest::Detail {
            mail_id,
            reconciliation_poll,
        } => {
            let Some(response) = parse_success::<MailDetailResponse>(
                state,
                MailOperation::Detail,
                status,
                body,
                events,
            ) else {
                if reconciliation_poll {
                    continue_claim_reconciliation(state, &mail_id, events);
                }
                return;
            };
            if !response.ok || response.mail.summary.mail_id != mail_id {
                reject_request(state, events, MailOperation::Detail, "MAIL_DETAIL_REJECTED");
                if reconciliation_poll {
                    continue_claim_reconciliation(state, &mail_id, events);
                }
                return;
            }
            let claim_status = response.mail.claim.claim_status.clone();
            if claim_status
                .as_deref()
                .is_some_and(|status| !valid_claim_status(status))
            {
                reject_request(
                    state,
                    events,
                    MailOperation::Detail,
                    "MAIL_RESPONSE_INVALID",
                );
                return;
            }
            state.selected_mail = Some(response.mail);
            state.last_error = None;
            events.write(MailClientEvent::MailLoaded {
                mail_id: mail_id.clone(),
            });
            if reconciliation_poll {
                apply_reconciled_claim_status(state, mail_id, claim_status, events);
            }
        }
        PendingMailRequest::MarkRead { mail_id } => {
            let Some(response) = parse_success::<MailReadResponse>(
                state,
                MailOperation::MarkRead,
                status,
                body,
                events,
            ) else {
                return;
            };
            if !response.ok || response.mail_id != mail_id || response.status != "read" {
                reject_request(state, events, MailOperation::MarkRead, "MAIL_READ_REJECTED");
                return;
            }
            apply_read_to_cache(state, &mail_id, response.read_at);
            state.last_error = None;
            let query = state.refresh_query();
            events.write(MailClientEvent::MailRead {
                mail_id: mail_id.clone(),
                already_read: response.already_read,
            });
            start_list_request(session, state, query, network_commands, events);
        }
        PendingMailRequest::Claim { mail_id } => {
            if status == 202 {
                begin_claim_reconciliation(state, mail_id, events);
                return;
            }
            let Some(response) = parse_success::<MailClaimResponse>(
                state,
                MailOperation::Claim,
                status,
                body,
                events,
            ) else {
                return;
            };
            if !response.ok || (!response.mail_id.is_empty() && response.mail_id != mail_id) {
                reject_request(state, events, MailOperation::Claim, "MAIL_CLAIM_REJECTED");
                return;
            }
            let claim_status = response.claim_status.clone();
            if !claim_status.as_deref().is_some_and(valid_claim_status) {
                reject_request(state, events, MailOperation::Claim, "MAIL_RESPONSE_INVALID");
                return;
            }
            if response.processing
                || matches!(
                    claim_status.as_deref(),
                    Some("processing" | "reconciliation_pending")
                )
            {
                begin_claim_reconciliation(state, mail_id, events);
                return;
            }
            apply_claim_to_cache(state, &mail_id, &response);
            state.last_error = None;
            events.write(MailClientEvent::ClaimUpdated {
                mail_id,
                claim_status,
            });
        }
    }
}

fn list_response_matches_request(query: &MailListQuery, response: &MailListResponse) -> bool {
    let Ok((_, limit, offset)) = query.normalized() else {
        return false;
    };
    response.pagination.limit == limit
        && response.pagination.offset == offset
        && response.mails.len() <= usize::from(limit)
        && response
            .mails
            .iter()
            .all(|mail| valid_mail_id(&mail.mail_id))
        && response.pagination.next_offset.is_none_or(|next_offset| {
            next_offset > offset && next_offset <= 10_000 && !response.mails.is_empty()
        })
}

fn deduplicate_mail_page(mails: Vec<MailSummary>) -> Vec<MailSummary> {
    let mut seen = HashSet::with_capacity(mails.len());
    mails
        .into_iter()
        .filter(|mail| seen.insert(mail.mail_id.clone()))
        .collect()
}

fn merge_mail_page(current: &mut Vec<MailSummary>, page: Vec<MailSummary>) {
    let mut indices = current
        .iter()
        .enumerate()
        .map(|(index, mail)| (mail.mail_id.clone(), index))
        .collect::<HashMap<_, _>>();
    for mail in deduplicate_mail_page(page) {
        if let Some(index) = indices.get(&mail.mail_id).copied() {
            current[index] = mail;
        } else {
            indices.insert(mail.mail_id.clone(), current.len());
            current.push(mail);
        }
    }
}

fn drive_claim_reconciliation(
    time: Res<Time>,
    session: Res<MyServerSession>,
    mut state: ResMut<MailClientState>,
    mut network_commands: MessageWriter<NetworkCommand>,
    mut events: MessageWriter<MailClientEvent>,
) {
    let due_mail_id = state
        .claim_reconciliation
        .as_mut()
        .and_then(|reconciliation| {
            let timer = reconciliation.next_poll.as_mut()?;
            timer.tick(time.delta());
            if timer.just_finished() {
                reconciliation.next_poll = None;
                Some(reconciliation.mail_id.clone())
            } else {
                None
            }
        });
    if let Some(mail_id) = due_mail_id {
        start_detail_request(
            &session,
            &mut state,
            &mail_id,
            true,
            &mut network_commands,
            &mut events,
        );
    }
}

fn start_list_request(
    session: &MyServerSession,
    state: &mut MailClientState,
    query: MailListQuery,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<MailClientEvent>,
) {
    let Ok((status, limit, offset)) = query.normalized() else {
        state.list_load_state = MailListLoadState::Failed;
        reject_request(
            state,
            events,
            MailOperation::List,
            "INVALID_MAIL_LIST_QUERY",
        );
        return;
    };
    let (generation, query) = if offset == 0 {
        state.desired_list_generation = state.desired_list_generation.wrapping_add(1);
        state.list_stale = true;
        state.list_load_state = if state.mails.is_empty() {
            MailListLoadState::InitialLoading
        } else {
            MailListLoadState::Refreshing
        };
        state.last_error = None;
        let query = MailListQuery {
            status,
            limit: Some(limit),
            offset: Some(0),
        };
        state.active_list_query = Some(query.clone());
        (state.desired_list_generation, query)
    } else {
        let Some(active) = state.active_list_query.as_ref() else {
            reject_request(
                state,
                events,
                MailOperation::List,
                "MAIL_LIST_PAGE_CONTEXT_REQUIRED",
            );
            return;
        };
        let active_status = active.status;
        let active_limit = active.limit.unwrap_or(50);
        let expected_offset = state
            .pagination
            .as_ref()
            .and_then(|pagination| pagination.next_offset);
        if state.list_stale
            || active_status != status
            || active_limit != limit
            || expected_offset != Some(offset)
        {
            reject_request(
                state,
                events,
                MailOperation::List,
                "MAIL_LIST_PAGE_OUT_OF_SEQUENCE",
            );
            return;
        }
        state.list_load_state = MailListLoadState::LoadingMore;
        state.last_error = None;
        (
            state.desired_list_generation,
            MailListQuery {
                status,
                limit: Some(limit),
                offset: Some(offset),
            },
        )
    };
    if state.pending.values().any(|pending| {
        matches!(
            pending,
            PendingMailRequest::List {
                query: pending_query,
                generation: pending_generation,
                ..
            } if *pending_generation == generation
                && pending_query.offset.unwrap_or(0) == offset
        )
    }) {
        return;
    }
    let Some((endpoint, ticket)) = request_context(state, session, MailOperation::List, events)
    else {
        state.list_load_state = MailListLoadState::Failed;
        return;
    };
    let mut query_parts = Vec::with_capacity(3);
    if let Some(status) = status {
        query_parts.push(format!("status={}", status.as_str()));
    }
    query_parts.push(format!("limit={limit}"));
    query_parts.push(format!("offset={offset}"));
    let request = build_read_request(&endpoint, &ticket, &format!("?{}", query_parts.join("&")));
    queue_request(
        state,
        PendingMailRequest::List {
            query,
            generation,
            identity: state.identity.clone(),
        },
        request,
        network_commands,
    );
}

fn start_detail_request(
    session: &MyServerSession,
    state: &mut MailClientState,
    mail_id: &str,
    reconciliation_poll: bool,
    network_commands: &mut MessageWriter<NetworkCommand>,
    events: &mut MessageWriter<MailClientEvent>,
) {
    if !valid_mail_id(mail_id) {
        reject_request(state, events, MailOperation::Detail, "INVALID_MAIL_ID");
        return;
    }
    if state.pending.values().any(|pending| {
        matches!(pending, PendingMailRequest::Detail { mail_id: pending_id, .. } if pending_id == mail_id)
    }) {
        return;
    }
    let Some((endpoint, ticket)) = request_context(state, session, MailOperation::Detail, events)
    else {
        if reconciliation_poll {
            continue_claim_reconciliation(state, mail_id, events);
        }
        return;
    };
    if reconciliation_poll {
        if let Some(reconciliation) = state.claim_reconciliation.as_mut() {
            reconciliation.polls_completed = reconciliation.polls_completed.saturating_add(1);
        }
    }
    let request = build_read_request(&endpoint, &ticket, &format!("/{mail_id}"));
    queue_request(
        state,
        PendingMailRequest::Detail {
            mail_id: mail_id.to_string(),
            reconciliation_poll,
        },
        request,
        network_commands,
    );
}

fn request_context(
    state: &MailClientState,
    session: &MyServerSession,
    operation: MailOperation,
    events: &mut MessageWriter<MailClientEvent>,
) -> Option<(MailHttpEndpoint, String)> {
    let Some(endpoint) = state.endpoint.clone() else {
        events.write(MailClientEvent::RequestFailed {
            error: MailClientError {
                operation,
                status: None,
                code: "MAIL_SERVICE_UNAVAILABLE".to_string(),
            },
        });
        return None;
    };
    let Some(ticket) = session
        .ticket
        .as_deref()
        .map(str::trim)
        .filter(|ticket| !ticket.is_empty())
    else {
        events.write(MailClientEvent::RequestFailed {
            error: MailClientError {
                operation,
                status: None,
                code: "MAIL_CHARACTER_TICKET_REQUIRED".to_string(),
            },
        });
        return None;
    };
    if session
        .character_id
        .as_deref()
        .is_none_or(|character_id| character_id.trim().is_empty())
    {
        events.write(MailClientEvent::RequestFailed {
            error: MailClientError {
                operation,
                status: None,
                code: "MAIL_CHARACTER_TICKET_REQUIRED".to_string(),
            },
        });
        return None;
    }
    Some((endpoint, ticket.to_string()))
}

fn build_read_request(endpoint: &MailHttpEndpoint, ticket: &str, suffix: &str) -> HttpRequest {
    HttpRequest::new(HttpMethod::Get, endpoint.url_for(suffix))
        .with_header("Accept", "application/json")
        .with_header(MAIL_GAME_TICKET_HEADER, ticket)
        .with_timeout(MAIL_READ_TIMEOUT)
        .with_max_response_bytes(MAIL_MAX_RESPONSE_BYTES)
}

fn build_mutation_request(
    endpoint: &MailHttpEndpoint,
    ticket: &str,
    mail_id: &str,
    suffix: &str,
    method: HttpMethod,
    timeout: Duration,
) -> HttpRequest {
    HttpRequest::new(method, endpoint.url_for(&format!("/{mail_id}{suffix}")))
        .with_header("Accept", "application/json")
        .with_header(MAIL_GAME_TICKET_HEADER, ticket)
        .with_timeout(timeout)
        .with_max_response_bytes(MAIL_MAX_RESPONSE_BYTES)
}

fn queue_request(
    state: &mut MailClientState,
    pending: PendingMailRequest,
    request: HttpRequest,
    network_commands: &mut MessageWriter<NetworkCommand>,
) {
    state.pending.insert(request.request_id, pending);
    network_commands.write(NetworkCommand::Http(request));
}

fn parse_success<T>(
    state: &mut MailClientState,
    operation: MailOperation,
    status: u16,
    body: &[u8],
    events: &mut MessageWriter<MailClientEvent>,
) -> Option<T>
where
    T: for<'a> Deserialize<'a>,
{
    if !(200..300).contains(&status) {
        let error = public_http_error(operation, status, body);
        state.last_error = Some(error.clone());
        events.write(MailClientEvent::RequestFailed { error });
        return None;
    }
    match serde_json::from_slice(body) {
        Ok(response) => Some(response),
        Err(_) => {
            let error = MailClientError {
                operation,
                status: Some(status),
                code: "MAIL_RESPONSE_INVALID".to_string(),
            };
            state.last_error = Some(error.clone());
            events.write(MailClientEvent::RequestFailed { error });
            None
        }
    }
}

fn public_http_error(operation: MailOperation, status: u16, body: &[u8]) -> MailClientError {
    #[derive(Deserialize)]
    struct PublicError {
        #[serde(default)]
        error: Option<String>,
    }
    let code = serde_json::from_slice::<PublicError>(body)
        .ok()
        .and_then(|error| error.error)
        .filter(|code| !code.trim().is_empty())
        .unwrap_or_else(|| format!("MAIL_HTTP_{status}"));
    MailClientError {
        operation,
        status: Some(status),
        code,
    }
}

fn reject_request(
    state: &mut MailClientState,
    events: &mut MessageWriter<MailClientEvent>,
    operation: MailOperation,
    code: &str,
) {
    let error = MailClientError {
        operation,
        status: None,
        code: code.to_string(),
    };
    state.last_error = Some(error.clone());
    events.write(MailClientEvent::RequestFailed { error });
}

fn begin_claim_reconciliation(
    state: &mut MailClientState,
    mail_id: String,
    events: &mut MessageWriter<MailClientEvent>,
) {
    if state
        .claim_reconciliation
        .as_ref()
        .is_some_and(|reconciliation| reconciliation.mail_id == mail_id)
    {
        return;
    }
    state.claim_reconciliation = Some(MailClaimReconciliation {
        mail_id: mail_id.clone(),
        polls_completed: 0,
        next_poll: Some(Timer::new(Duration::from_secs(1), TimerMode::Once)),
    });
    state.last_error = None;
    events.write(MailClientEvent::ClaimReconciliationStarted { mail_id });
}

fn apply_reconciled_claim_status(
    state: &mut MailClientState,
    mail_id: String,
    claim_status: Option<String>,
    events: &mut MessageWriter<MailClientEvent>,
) {
    if claim_is_settled(claim_status.as_deref()) {
        state.claim_reconciliation = None;
        events.write(MailClientEvent::ClaimReconciliationSettled {
            mail_id,
            claim_status,
        });
    } else {
        continue_claim_reconciliation(state, &mail_id, events);
    }
}

fn continue_claim_reconciliation(
    state: &mut MailClientState,
    mail_id: &str,
    events: &mut MessageWriter<MailClientEvent>,
) {
    let Some(reconciliation) = state.claim_reconciliation.as_mut() else {
        return;
    };
    if reconciliation.mail_id != mail_id {
        return;
    }
    if reconciliation.polls_completed >= MAX_RECONCILIATION_POLLS {
        let claim_status = state
            .selected_mail
            .as_ref()
            .and_then(|mail| mail.claim.claim_status.clone());
        state.claim_reconciliation = None;
        events.write(MailClientEvent::ClaimReconciliationSettled {
            mail_id: mail_id.to_string(),
            claim_status,
        });
        return;
    }
    reconciliation.next_poll = Some(Timer::new(
        reconciliation_delay(reconciliation.polls_completed),
        TimerMode::Once,
    ));
}

fn reconciliation_delay(polls_completed: u8) -> Duration {
    match polls_completed {
        0 => Duration::from_secs(1),
        1 => Duration::from_secs(2),
        _ => Duration::from_secs(4),
    }
}

fn claim_is_settled(status: Option<&str>) -> bool {
    matches!(
        status,
        Some(
            "claimed"
                | "retryable_failure"
                | "blocked_capacity"
                | "permanent_failure"
                | "manual_review"
        )
    )
}

fn apply_read_to_cache(state: &mut MailClientState, mail_id: &str, read_at: Option<String>) {
    if let Some(summary) = state.mails.iter_mut().find(|mail| mail.mail_id == mail_id) {
        summary.status = "read".to_string();
        summary.read_at = read_at.clone();
    }
    if let Some(detail) = state
        .selected_mail
        .as_mut()
        .filter(|mail| mail.summary.mail_id == mail_id)
    {
        detail.summary.status = "read".to_string();
        detail.summary.read_at = read_at;
    }
}

fn apply_claim_to_cache(state: &mut MailClientState, mail_id: &str, response: &MailClaimResponse) {
    let claimed = response.claimed
        || response.already_claimed
        || response.claim_status.as_deref() == Some("claimed");
    if !claimed {
        return;
    }
    if let Some(summary) = state.mails.iter_mut().find(|mail| mail.mail_id == mail_id) {
        summary.status = response
            .status
            .clone()
            .unwrap_or_else(|| "claimed".to_string());
        summary.claimed_at = response.claimed_at.clone();
        summary.read_at = response.read_at.clone();
    }
    if let Some(detail) = state
        .selected_mail
        .as_mut()
        .filter(|mail| mail.summary.mail_id == mail_id)
    {
        detail.summary.status = response
            .status
            .clone()
            .unwrap_or_else(|| "claimed".to_string());
        detail.summary.claimed_at = response.claimed_at.clone();
        detail.summary.read_at = response.read_at.clone();
        detail.claim.claim_status = response.claim_status.clone();
        detail.claim.already_claimed = true;
        detail.claim.processing = false;
        detail.claim.retryable = response.retryable;
        detail.claim.player_retryable = response.player_retryable;
    }
}

fn valid_mail_id(mail_id: &str) -> bool {
    (1..=64).contains(&mail_id.len())
        && mail_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'_' | b'-'))
}

fn valid_claim_status(status: &str) -> bool {
    MAIL_CLAIM_STATUSES.contains(&status)
}

#[cfg(test)]
mod tests {
    use bevy::{
        ecs::message::MessageCursor,
        prelude::{App, Message, Messages, MinimalPlugins, Time},
    };

    use super::*;
    use crate::{
        framework::network::{HttpResponse, NetworkCommand, NetworkEvent},
        game::myserver::{
            chat::{ChatClientState, ChatEvent, MAIL_NOTIFY_PUSH},
            protocol::{Packet, PacketHeader},
        },
    };

    fn endpoint() -> MailHttpEndpoint {
        MailHttpEndpoint::from_service(
            &ClientServiceEndpoint {
                host: Some("api.game.zergzerg.cn".to_string()),
                port: Some(443),
                protocol: Some("https".to_string()),
            },
            false,
            true,
        )
        .unwrap()
    }

    fn session_with_mail() -> MyServerSession {
        MyServerSession {
            player_id: Some("player_1".to_string()),
            character_id: Some("character_1".to_string()),
            ticket: Some("character-bound-ticket".to_string()),
            mail_endpoint: Some(endpoint()),
            ..Default::default()
        }
    }

    fn test_app(session: MyServerSession) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Time>()
            .add_message::<NetworkCommand>()
            .add_message::<NetworkEvent>()
            .insert_resource(session)
            .add_plugins(MailPlugin);
        app
    }

    fn messages<M>(app: &App) -> Vec<M>
    where
        M: Message + Clone,
    {
        let mut cursor = MessageCursor::default();
        cursor
            .read(app.world().resource::<Messages<M>>())
            .cloned()
            .collect()
    }

    fn http_requests(app: &App) -> Vec<HttpRequest> {
        messages::<NetworkCommand>(app)
            .into_iter()
            .filter_map(|command| match command {
                NetworkCommand::Http(request) => Some(request),
                _ => None,
            })
            .collect()
    }

    fn header<'a>(request: &'a HttpRequest, name: &str) -> Option<&'a str> {
        request
            .headers
            .iter()
            .find(|(header, _)| header == name)
            .map(|(_, value)| value.as_str())
    }

    fn respond(app: &mut App, request: &HttpRequest, status: u16, body: &str) {
        app.world_mut()
            .write_message(NetworkEvent::HttpResponse(HttpResponse {
                request_id: request.request_id,
                status,
                headers: Vec::new(),
                body: body.as_bytes().to_vec(),
            }));
        app.update();
    }

    #[test]
    fn mail_endpoint_uses_only_the_https_descriptor_and_fixed_api_path() {
        assert_eq!(
            endpoint().base_url(),
            "https://api.game.zergzerg.cn/api/v1/mails"
        );
        assert!(
            MailHttpEndpoint::from_service(
                &ClientServiceEndpoint {
                    host: Some("mail-service".to_string()),
                    port: Some(9003),
                    protocol: Some("http".to_string()),
                },
                false,
                true
            )
            .is_err()
        );
        assert_eq!(
            MailHttpEndpoint::from_service(
                &ClientServiceEndpoint {
                    host: Some("127.0.0.1".to_string()),
                    port: Some(9003),
                    protocol: Some("http".to_string()),
                },
                true,
                false
            )
            .unwrap()
            .base_url(),
            "http://127.0.0.1:9003/api/v1/mails"
        );
        assert!(
            MailHttpEndpoint::from_service(
                &ClientServiceEndpoint {
                    host: Some("api.game.zergzerg.cn/other".to_string()),
                    port: Some(443),
                    protocol: Some("https".to_string()),
                },
                false,
                true
            )
            .is_err()
        );
        assert!(
            MailHttpEndpoint::from_service(
                &ClientServiceEndpoint {
                    host: Some("mail-preview.example.com".to_string()),
                    port: Some(443),
                    protocol: Some("https".to_string()),
                },
                false,
                true
            )
            .is_err()
        );
    }

    #[test]
    fn public_mail_contract_deserializes_null_pagination_and_claim_states() {
        let list: MailListResponse = serde_json::from_str(
            r#"{
                "ok":true,
                "mails":[{
                    "mail_id":"rw_1","sender":{"type":"system","id":"system","name":"System"},
                    "title":"Reward","mail_type":"system_reward","status":"unread",
                    "has_attachments":true,"created_at":"2026-08-05T00:00:00Z",
                    "read_at":null,"claimed_at":null,"expires_at":null
                }],
                "unread_count":1,
                "pagination":{"limit":50,"offset":0,"next_offset":null}
            }"#,
        )
        .unwrap();
        assert_eq!(list.mails[0].mail_id, "rw_1");
        assert_eq!(list.unread_count, 1);
        assert_eq!(list.pagination.next_offset, None);

        let detail: MailDetailResponse = serde_json::from_str(
            r#"{
                "ok":true,
                "mail":{
                    "mail_id":"rw_1","status":"claiming","content":"body",
                    "attachments":[{"type":"item","id":1001,"count":2,"binded":true}],
                    "claim":{"claim_status":"reconciliation_pending","already_claimed":false,
                    "processing":false,"retryable":false,"player_retryable":false,"attempts":1,
                    "result_state":"unknown","error":"MAIL_CLAIM_RECONCILIATION_PENDING"}
                }
            }"#,
        )
        .unwrap();
        assert_eq!(detail.mail.summary.mail_id, "rw_1");
        assert_eq!(detail.mail.attachments.len(), 1);
        assert_eq!(
            detail.mail.claim.claim_status.as_deref(),
            Some("reconciliation_pending")
        );

        let error = public_http_error(
            MailOperation::Claim,
            429,
            br#"{"ok":false,"error":"MAIL_RATE_LIMITED","message":"Too many mail requests"}"#,
        );
        assert_eq!(error.status, Some(429));
        assert_eq!(error.code, "MAIL_RATE_LIMITED");

        let claim: MailClaimResponse = serde_json::from_str(
            r#"{
                "ok":true,"mail_id":"rw_1","claim_status":"reconciliation_pending",
                "claimed":false,"already_claimed":false,"processing":false,
                "retryable":false,"player_retryable":false,"request_id":"mail_claim:rw_1",
                "attempts":1,"result_state":"unknown",
                "error":"MAIL_CLAIM_RECONCILIATION_PENDING",
                "message":"Mail attachment claim result is being verified","status":"claiming",
                "attachments":[{"type":"item","id":1001,"count":2,"binded":true}],
                "read_at":null,"claimed_at":null,"completed_at":null
            }"#,
        )
        .unwrap();
        assert_eq!(claim.request_id.as_deref(), Some("mail_claim:rw_1"));
        assert_eq!(claim.attempts, 1);
        assert_eq!(claim.attachments.len(), 1);
        assert!(MAIL_CLAIM_STATUSES.contains(&claim.claim_status.as_deref().unwrap()));
    }

    #[test]
    fn list_request_uses_game_ticket_without_player_identity_or_other_credentials() {
        let mut app = test_app(session_with_mail());
        app.world_mut().write_message(MailClientCommand::LoadList {
            query: MailListQuery::default(),
        });
        app.update();

        let request = http_requests(&app).pop().unwrap();
        assert_eq!(
            request.url,
            "https://api.game.zergzerg.cn/api/v1/mails?limit=50&offset=0"
        );
        assert_eq!(
            header(&request, "X-Game-Ticket"),
            Some("character-bound-ticket")
        );
        assert_eq!(header(&request, "Authorization"), None);
        assert!(!request.url.contains("player_1"));
        assert!(request.body.is_none());
        let state = app.world().resource::<MailClientState>();
        assert_eq!(state.list_load_state, MailListLoadState::InitialLoading);
        assert!(state.authoritative_unread_count().is_none());
    }

    #[test]
    fn list_query_supports_status_limit_and_stable_paginated_merge() {
        let mut app = test_app(session_with_mail());
        let first_query = MailListQuery {
            status: Some(MailListStatus::Unread),
            limit: Some(2),
            offset: Some(0),
        };
        app.world_mut()
            .write_message(MailClientCommand::LoadList { query: first_query });
        app.update();
        let first = http_requests(&app).pop().unwrap();
        assert_eq!(
            first.url,
            "https://api.game.zergzerg.cn/api/v1/mails?status=unread&limit=2&offset=0"
        );
        respond(
            &mut app,
            &first,
            200,
            r#"{"ok":true,"mails":[
                {"mail_id":"mail_a","title":"A","status":"unread"},
                {"mail_id":"mail_b","title":"B","status":"unread"}],
                "unread_count":3,"pagination":{"limit":2,"offset":0,"next_offset":2}}"#,
        );
        {
            let state = app.world().resource::<MailClientState>();
            assert_eq!(state.list_load_state, MailListLoadState::Ready);
            assert_eq!(state.authoritative_unread_count(), Some(3));
            assert_eq!(
                state
                    .mails
                    .iter()
                    .map(|mail| mail.mail_id.as_str())
                    .collect::<Vec<_>>(),
                ["mail_a", "mail_b"]
            );
        }

        let next_query = app
            .world()
            .resource::<MailClientState>()
            .next_page_query()
            .unwrap();
        app.world_mut().write_message(MailClientCommand::LoadList {
            query: next_query.clone(),
        });
        app.update();
        let second = http_requests(&app).pop().unwrap();
        assert_eq!(
            second.url,
            "https://api.game.zergzerg.cn/api/v1/mails?status=unread&limit=2&offset=2"
        );
        assert_eq!(
            app.world().resource::<MailClientState>().list_load_state,
            MailListLoadState::LoadingMore
        );

        let request_count = http_requests(&app).len();
        app.world_mut()
            .write_message(MailClientCommand::LoadList { query: next_query });
        app.update();
        assert_eq!(http_requests(&app).len(), request_count);

        respond(
            &mut app,
            &second,
            200,
            r#"{"ok":true,"mails":[
                {"mail_id":"mail_b","title":"B updated","status":"read"},
                {"mail_id":"mail_c","title":"C","status":"unread"}],
                "unread_count":2,"pagination":{"limit":2,"offset":2,"next_offset":null}}"#,
        );
        let state = app.world().resource::<MailClientState>();
        assert_eq!(state.list_load_state, MailListLoadState::Ready);
        assert_eq!(state.authoritative_unread_count(), Some(2));
        assert_eq!(
            state
                .mails
                .iter()
                .map(|mail| mail.mail_id.as_str())
                .collect::<Vec<_>>(),
            ["mail_a", "mail_b", "mail_c"]
        );
        assert_eq!(state.mails[1].title, "B updated");
        assert!(state.next_page_query().is_none());
    }

    #[test]
    fn latest_list_generation_discards_late_refresh_without_overwriting_authority() {
        let mut app = test_app(session_with_mail());
        app.world_mut().write_message(MailClientCommand::LoadList {
            query: MailListQuery::default(),
        });
        app.update();
        let first = http_requests(&app).pop().unwrap();
        app.world_mut().write_message(MailClientCommand::LoadList {
            query: MailListQuery::default(),
        });
        app.update();
        let second = http_requests(&app).pop().unwrap();
        assert_ne!(first.request_id, second.request_id);
        assert_eq!(
            app.world().resource::<MailClientState>().list_generation(),
            2
        );

        respond(
            &mut app,
            &second,
            200,
            r#"{"ok":true,"mails":[{"mail_id":"mail_new","title":"New"}],
                "unread_count":1,"pagination":{"limit":50,"offset":0,"next_offset":null}}"#,
        );
        respond(
            &mut app,
            &first,
            200,
            r#"{"ok":true,"mails":[{"mail_id":"mail_old","title":"Old"}],
                "unread_count":9,"pagination":{"limit":50,"offset":0,"next_offset":null}}"#,
        );
        let state = app.world().resource::<MailClientState>();
        assert_eq!(state.mails.len(), 1);
        assert_eq!(state.mails[0].mail_id, "mail_new");
        assert_eq!(state.authoritative_unread_count(), Some(1));
    }

    #[test]
    fn identity_change_cancels_and_discards_the_old_list_response() {
        let mut app = test_app(session_with_mail());
        app.world_mut().write_message(MailClientCommand::LoadList {
            query: MailListQuery::default(),
        });
        app.update();
        let old_request = http_requests(&app).pop().unwrap();

        {
            let mut session = app.world_mut().resource_mut::<MyServerSession>();
            session.character_id = Some("character_2".to_owned());
            session.ticket = Some("character-2-ticket".to_owned());
        }
        app.update();
        assert!(messages::<NetworkCommand>(&app).iter().any(|command| {
            matches!(command, NetworkCommand::CancelHttp { request_id } if *request_id == old_request.request_id)
        }));
        respond(
            &mut app,
            &old_request,
            200,
            r#"{"ok":true,"mails":[{"mail_id":"mail_old_identity"}],
                "unread_count":1,"pagination":{"limit":50,"offset":0,"next_offset":null}}"#,
        );
        let state = app.world().resource::<MailClientState>();
        assert!(state.mails.is_empty());
        assert_eq!(state.unread_count, None);
        assert_eq!(state.list_load_state, MailListLoadState::Idle);
    }

    #[test]
    fn list_rejects_out_of_sequence_page_and_invalid_pagination() {
        let mut app = test_app(session_with_mail());
        app.world_mut().write_message(MailClientCommand::LoadList {
            query: MailListQuery {
                status: Some(MailListStatus::Read),
                limit: Some(2),
                offset: Some(2),
            },
        });
        app.update();
        assert!(http_requests(&app).is_empty());
        assert_eq!(
            app.world()
                .resource::<MailClientState>()
                .last_error
                .as_ref()
                .map(|error| error.code.as_str()),
            Some("MAIL_LIST_PAGE_CONTEXT_REQUIRED")
        );

        app.world_mut().write_message(MailClientCommand::LoadList {
            query: MailListQuery::default(),
        });
        app.update();
        let request = http_requests(&app).pop().unwrap();
        respond(
            &mut app,
            &request,
            200,
            r#"{"ok":true,"mails":[{"mail_id":"mail_a"}],"unread_count":1,
                "pagination":{"limit":50,"offset":0,"next_offset":0}}"#,
        );
        let state = app.world().resource::<MailClientState>();
        assert_eq!(state.list_load_state, MailListLoadState::Failed);
        assert!(state.authoritative_unread_count().is_none());
    }

    #[test]
    fn list_moves_through_empty_error_and_authoritative_retry_states() {
        let mut app = test_app(session_with_mail());
        app.world_mut().write_message(MailClientCommand::LoadList {
            query: MailListQuery::default(),
        });
        app.update();
        let initial = http_requests(&app).pop().unwrap();
        assert_eq!(
            app.world().resource::<MailClientState>().list_load_state,
            MailListLoadState::InitialLoading
        );
        respond(
            &mut app,
            &initial,
            200,
            r#"{"ok":true,"mails":[],"unread_count":0,
                "pagination":{"limit":50,"offset":0,"next_offset":null}}"#,
        );
        assert_eq!(
            app.world().resource::<MailClientState>().list_load_state,
            MailListLoadState::Empty
        );
        assert_eq!(
            app.world()
                .resource::<MailClientState>()
                .authoritative_unread_count(),
            Some(0)
        );

        app.world_mut().write_message(MailClientCommand::LoadList {
            query: MailListQuery::default(),
        });
        app.update();
        let failed = http_requests(&app).pop().unwrap();
        app.world_mut().write_message(NetworkEvent::HttpError {
            request_id: failed.request_id,
            error: "request timeout".to_owned(),
        });
        app.update();
        assert_eq!(
            app.world().resource::<MailClientState>().list_load_state,
            MailListLoadState::Failed
        );
        assert!(
            app.world()
                .resource::<MailClientState>()
                .authoritative_unread_count()
                .is_none()
        );

        app.world_mut().write_message(MailClientCommand::LoadList {
            query: MailListQuery::default(),
        });
        app.update();
        let retry = http_requests(&app).pop().unwrap();
        respond(
            &mut app,
            &retry,
            200,
            r#"{"ok":true,"mails":[{"mail_id":"mail_after_retry"}],"unread_count":1,
                "pagination":{"limit":50,"offset":0,"next_offset":null}}"#,
        );
        let state = app.world().resource::<MailClientState>();
        assert_eq!(state.list_load_state, MailListLoadState::Ready);
        assert_eq!(state.mails[0].mail_id, "mail_after_retry");
        assert_eq!(state.authoritative_unread_count(), Some(1));
    }

    #[test]
    fn absent_descriptor_is_explicitly_unavailable_and_never_uses_port_9003() {
        let mut app = test_app(MyServerSession {
            player_id: Some("player_1".to_string()),
            character_id: Some("character_1".to_string()),
            ticket: Some("ticket".to_string()),
            ..Default::default()
        });
        app.world_mut().write_message(MailClientCommand::LoadList {
            query: MailListQuery::default(),
        });
        app.update();

        assert!(matches!(
            app.world().resource::<MailClientState>().availability,
            MailAvailability::Unavailable { .. }
        ));
        assert!(http_requests(&app).is_empty());
    }

    #[test]
    fn marking_read_updates_cached_status_and_refreshes_authoritative_unread_count() {
        let mut app = test_app(session_with_mail());
        app.update();
        {
            let mut state = app.world_mut().resource_mut::<MailClientState>();
            state.mails.push(MailSummary {
                mail_id: "mail_1".to_string(),
                sender: MailSender::default(),
                title: "Welcome".to_string(),
                mail_type: "system".to_string(),
                status: "unread".to_string(),
                has_attachments: false,
                created_at: None,
                read_at: None,
                claimed_at: None,
                expires_at: None,
            });
            state.unread_count = Some(1);
        }

        app.world_mut().write_message(MailClientCommand::MarkRead {
            mail_id: "mail_1".to_string(),
        });
        app.update();
        let mark_read = http_requests(&app).pop().unwrap();
        assert!(matches!(mark_read.method, HttpMethod::Put));
        assert_eq!(
            mark_read.url,
            "https://api.game.zergzerg.cn/api/v1/mails/mail_1/read"
        );

        respond(
            &mut app,
            &mark_read,
            200,
            r#"{"ok":true,"mail_id":"mail_1","status":"read","read_at":"2026-08-01T00:00:00Z","already_read":false}"#,
        );
        let refresh = http_requests(&app).pop().unwrap();
        assert!(matches!(refresh.method, HttpMethod::Get));
        assert_eq!(
            app.world().resource::<MailClientState>().mails[0].status,
            "read"
        );
        assert!(app.world().resource::<MailClientState>().list_stale);
    }

    #[test]
    fn unknown_claim_converges_through_detail_without_another_claim_post() {
        let mut app = test_app(session_with_mail());
        app.world_mut().write_message(MailClientCommand::Claim {
            mail_id: "mail_1".to_string(),
        });
        app.update();
        let claim = http_requests(&app).pop().unwrap();
        assert!(matches!(claim.method, HttpMethod::Post));

        respond(
            &mut app,
            &claim,
            202,
            r#"{"ok":true,"mail_id":"mail_1","claim_status":"reconciliation_pending"}"#,
        );
        assert_eq!(
            app.world()
                .resource::<MailClientState>()
                .claim_reconciliation
                .as_ref()
                .map(|state| state.mail_id.as_str()),
            Some("mail_1")
        );

        app.world_mut().write_message(MailClientCommand::Claim {
            mail_id: "mail_1".to_string(),
        });
        app.update();
        assert_eq!(
            http_requests(&app)
                .iter()
                .filter(|request| matches!(request.method, HttpMethod::Post))
                .count(),
            1
        );

        app.world_mut()
            .write_message(MailClientCommand::PollClaimReconciliation {
                mail_id: "mail_1".to_string(),
            });
        app.update();
        let detail = http_requests(&app).pop().unwrap();
        assert!(matches!(detail.method, HttpMethod::Get));
        assert_eq!(
            detail.url,
            "https://api.game.zergzerg.cn/api/v1/mails/mail_1"
        );

        respond(
            &mut app,
            &detail,
            200,
            r#"{"ok":true,"mail":{"mail_id":"mail_1","status":"claimed","claim":{"claim_status":"claimed","already_claimed":true}}}"#,
        );
        assert!(
            app.world()
                .resource::<MailClientState>()
                .claim_reconciliation
                .is_none()
        );
    }

    #[test]
    fn mail_push_only_invalidates_and_refreshes_authoritative_list() {
        let inbound = ChatInbound::MailNotifyPush(Packet {
            header: PacketHeader {
                msg_type: MAIL_NOTIFY_PUSH,
                seq: 7,
                body_len: 3,
            },
            body: vec![1, 2, 3],
        });
        assert!(matches!(inbound, ChatInbound::MailNotifyPush(_)));

        let mut app = test_app(session_with_mail());
        app.world_mut()
            .write_message(MailClientCommand::MailNotifyPush);
        app.update();
        let request = http_requests(&app).pop().unwrap();
        assert!(matches!(request.method, HttpMethod::Get));
        assert_eq!(
            request.url,
            "https://api.game.zergzerg.cn/api/v1/mails?limit=50&offset=0"
        );
        assert!(
            app.world()
                .resource::<MailClientState>()
                .selected_mail
                .is_none()
        );
    }

    #[test]
    fn chat_mail_notifications_are_coalesced_and_ignore_stale_generations() {
        let mut app = test_app(session_with_mail());
        app.insert_resource(ChatClientState {
            endpoint_generation: 8,
            ..Default::default()
        });
        app.world_mut()
            .write_message(ChatEvent::MailNotifyPush { generation: 7 });
        app.world_mut()
            .write_message(ChatEvent::MailNotifyPush { generation: 8 });
        app.world_mut()
            .write_message(ChatEvent::MailNotifyPush { generation: 8 });
        app.update();

        let requests = http_requests(&app);
        assert_eq!(requests.len(), 1);
        assert!(matches!(requests[0].method, HttpMethod::Get));
        assert_eq!(
            requests[0].url,
            "https://api.game.zergzerg.cn/api/v1/mails?limit=50&offset=0"
        );
        assert!(app.world().resource::<MailClientState>().list_stale);
    }

    #[test]
    fn reconciliation_uses_bounded_one_two_four_second_backoff() {
        assert_eq!(reconciliation_delay(0), Duration::from_secs(1));
        assert_eq!(reconciliation_delay(1), Duration::from_secs(2));
        assert_eq!(reconciliation_delay(2), Duration::from_secs(4));
    }
}
