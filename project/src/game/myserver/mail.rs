use std::{
    collections::{HashMap, HashSet},
    hash::{DefaultHasher, Hash, Hasher},
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
const MAIL_MAX_DETAIL_CONTENT_BYTES: usize = 32 * 1024;
const MAIL_MAX_DETAIL_CONTENT_CHARS: usize = 8 * 1024;
pub const MAIL_MAX_DETAIL_ATTACHMENTS: usize = 32;
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

#[derive(Clone, Deserialize)]
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

impl std::fmt::Debug for MailDetail {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MailDetail")
            .field("mail_id", &self.summary.mail_id)
            .field("status", &self.summary.status)
            .field("content", &"[REDACTED]")
            .field("attachment_count", &self.attachments.len())
            .finish_non_exhaustive()
    }
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MailDetailLoadState {
    #[default]
    Idle,
    Loading,
    Ready,
    NotFound,
    Forbidden,
    Expired,
    Failed,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MailMarkReadState {
    #[default]
    Idle,
    Submitting,
    Succeeded,
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MailClaimWorkflowState {
    #[default]
    Idle,
    Available,
    Submitting,
    Processing,
    ReconciliationPending,
    Claimed,
    AlreadyClaimed,
    Expired,
    RetryableFailure,
    BlockedCapacity,
    PermanentFailure,
    ManualReview,
    Unavailable,
}

impl MailClaimWorkflowState {
    pub const fn binding_value(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Available => "available",
            Self::Submitting => "submitting",
            Self::Processing => "processing",
            Self::ReconciliationPending => "reconciliation_pending",
            Self::Claimed => "claimed",
            Self::AlreadyClaimed => "already_claimed",
            Self::Expired => "expired",
            Self::RetryableFailure => "retryable_failure",
            Self::BlockedCapacity => "blocked_capacity",
            Self::PermanentFailure => "permanent_failure",
            Self::ManualReview => "manual_review",
            Self::Unavailable => "unavailable",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MailClaimWorkflow {
    pub mail_id: String,
    pub state: MailClaimWorkflowState,
    pub player_retryable: bool,
    pub exhausted: bool,
    pub result_state: Option<String>,
    pub error_code: Option<String>,
    post_attempted: bool,
}

#[derive(Clone, Debug, Resource)]
pub struct MailClientState {
    pub availability: MailAvailability,
    pub endpoint: Option<MailHttpEndpoint>,
    pub mails: Vec<MailSummary>,
    pub unread_count: Option<u32>,
    pub pagination: Option<MailPagination>,
    pub selected_mail_id: Option<String>,
    pub selected_mail: Option<MailDetail>,
    pub detail_load_state: MailDetailLoadState,
    pub mark_read_state: MailMarkReadState,
    pub detail_error: Option<MailClientError>,
    pub list_stale: bool,
    pub list_load_state: MailListLoadState,
    pub claim_reconciliation: Option<MailClaimReconciliation>,
    pub claim_workflow: Option<MailClaimWorkflow>,
    pub last_error: Option<MailClientError>,
    pending: HashMap<RequestId, PendingMailRequest>,
    identity: Option<MailIdentity>,
    desired_list_generation: u64,
    active_list_query: Option<MailListQuery>,
    list_refresh_queued: bool,
    desired_detail_generation: u64,
}

impl Default for MailClientState {
    fn default() -> Self {
        Self {
            availability: MailAvailability::default(),
            endpoint: None,
            mails: Vec::new(),
            unread_count: None,
            pagination: None,
            selected_mail_id: None,
            selected_mail: None,
            detail_load_state: MailDetailLoadState::Idle,
            mark_read_state: MailMarkReadState::Idle,
            detail_error: None,
            list_stale: false,
            list_load_state: MailListLoadState::Idle,
            claim_reconciliation: None,
            claim_workflow: None,
            last_error: None,
            pending: HashMap::new(),
            identity: None,
            desired_list_generation: 0,
            active_list_query: None,
            list_refresh_queued: false,
            desired_detail_generation: 0,
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

    pub fn detail_is_open(&self) -> bool {
        self.selected_mail_id.is_some()
    }

    pub fn selected_mail_id(&self) -> Option<&str> {
        self.selected_mail_id.as_deref()
    }

    pub fn selected_claim_workflow(&self) -> Option<&MailClaimWorkflow> {
        let selected = self.selected_mail_id()?;
        self.claim_workflow
            .as_ref()
            .filter(|workflow| workflow.mail_id == selected)
    }

    pub fn can_submit_claim(&self, mail_id: &str) -> bool {
        self.is_available()
            && self.selected_mail_id() == Some(mail_id)
            && self.contains_authoritative_mail(mail_id)
            && self.selected_mail.as_ref().is_some_and(|detail| {
                detail.summary.mail_id == mail_id
                    && !detail.attachments.is_empty()
                    && !detail.summary.status.eq_ignore_ascii_case("expired")
                    && !detail.summary.status.eq_ignore_ascii_case("claimed")
                    && !detail.claim.already_claimed
            })
            && self.claim_workflow.as_ref().is_some_and(|workflow| {
                workflow.mail_id == mail_id
                    && workflow.state == MailClaimWorkflowState::Available
                    && !workflow.post_attempted
            })
            && self.claim_reconciliation.is_none()
            && !self.pending.values().any(|pending| {
                matches!(pending, PendingMailRequest::Claim { mail_id: pending_id, .. } if pending_id == mail_id)
            })
    }

    pub fn dismiss_detail(&mut self) {
        self.desired_detail_generation = self.desired_detail_generation.wrapping_add(1);
        self.selected_mail_id = None;
        self.selected_mail = None;
        self.detail_load_state = MailDetailLoadState::Idle;
        self.mark_read_state = MailMarkReadState::Idle;
        self.detail_error = None;
        if self.last_error.as_ref().is_some_and(|error| {
            matches!(
                error.operation,
                MailOperation::Detail | MailOperation::MarkRead
            )
        }) {
            self.last_error = None;
        }
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

    #[cfg(test)]
    pub(crate) fn ready_with_detail_for_test(
        mails: Vec<MailSummary>,
        unread_count: u32,
        pagination: MailPagination,
        detail: MailDetail,
    ) -> Self {
        let mut state = Self::ready_with_list_for_test(mails, unread_count, pagination);
        state.claim_workflow = Some(claim_workflow_from_detail(&detail, false));
        state.selected_mail_id = Some(detail.summary.mail_id.clone());
        state.selected_mail = Some(detail);
        state.detail_load_state = MailDetailLoadState::Ready;
        state
    }

    #[cfg(test)]
    pub(crate) fn show_detail_for_test(&mut self, detail: MailDetail) {
        self.claim_workflow = Some(claim_workflow_from_detail(&detail, false));
        self.selected_mail_id = Some(detail.summary.mail_id.clone());
        self.selected_mail = Some(detail);
        self.detail_load_state = MailDetailLoadState::Ready;
        self.mark_read_state = MailMarkReadState::Idle;
        self.detail_error = None;
    }

    fn reset_for_identity(&mut self, identity: MailIdentity, availability: MailAvailability) {
        self.availability = availability;
        self.endpoint = identity.endpoint.clone();
        self.mails.clear();
        self.unread_count = None;
        self.pagination = None;
        self.selected_mail_id = None;
        self.selected_mail = None;
        self.detail_load_state = MailDetailLoadState::Idle;
        self.mark_read_state = MailMarkReadState::Idle;
        self.detail_error = None;
        self.list_stale = false;
        self.list_load_state = MailListLoadState::Idle;
        self.claim_reconciliation = None;
        self.claim_workflow = None;
        self.last_error = None;
        self.pending.clear();
        self.identity = Some(identity);
        self.desired_list_generation = 0;
        self.active_list_query = None;
        self.list_refresh_queued = false;
        self.desired_detail_generation = 0;
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MailIdentity {
    player_id: Option<String>,
    character_id: Option<String>,
    endpoint: Option<MailHttpEndpoint>,
    endpoint_error: Option<String>,
    ticket_fingerprint: Option<u64>,
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
        generation: u64,
        identity: Option<MailIdentity>,
    },
    MarkRead {
        mail_id: String,
        generation: u64,
        identity: Option<MailIdentity>,
    },
    Claim {
        mail_id: String,
        identity: Option<MailIdentity>,
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
    DismissDetail,
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
                    drive_queued_list_refresh,
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
    let mut refresh_required = false;
    for event in chat_events.read() {
        refresh_required |= matches!(
            event,
            ChatEvent::MailNotifyPush { generation }
                if current_generation.is_none_or(|current| *generation == current)
        );
    }
    if refresh_required {
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
        ticket_fingerprint: session.ticket.as_deref().map(ticket_fingerprint),
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

fn ticket_fingerprint(ticket: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    ticket.hash(&mut hasher);
    hasher.finish()
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
                let selected_is_authoritative = state.selected_mail_id.as_deref() == Some(mail_id)
                    && state.contains_authoritative_mail(mail_id)
                    && state
                        .selected_mail
                        .as_ref()
                        .is_some_and(|detail| detail.summary.mail_id == *mail_id)
                    && matches!(state.detail_load_state, MailDetailLoadState::Ready)
                    && state
                        .selected_mail
                        .as_ref()
                        .is_some_and(|detail| detail.summary.status.eq_ignore_ascii_case("unread"));
                if !valid_mail_id(mail_id) || !selected_is_authoritative {
                    reject_request(
                        &mut state,
                        &mut events,
                        MailOperation::MarkRead,
                        "INVALID_MAIL_ID",
                    );
                    continue;
                }
                if state.pending.values().any(|pending| {
                    matches!(pending, PendingMailRequest::MarkRead { mail_id: pending_id, .. } if pending_id == mail_id)
                }) {
                    continue;
                }
                let Some((endpoint, ticket)) =
                    request_context(&state, &session, MailOperation::MarkRead, &mut events)
                else {
                    state.mark_read_state = MailMarkReadState::Failed;
                    continue;
                };
                state.mark_read_state = MailMarkReadState::Submitting;
                state.detail_error = None;
                let request = build_mutation_request(
                    &endpoint,
                    &ticket,
                    mail_id,
                    "/read",
                    HttpMethod::Put,
                    MAIL_READ_TIMEOUT,
                );
                let generation = state.desired_detail_generation;
                let identity = state.identity.clone();
                queue_request(
                    &mut state,
                    PendingMailRequest::MarkRead {
                        mail_id: mail_id.clone(),
                        generation,
                        identity,
                    },
                    request,
                    &mut network_commands,
                );
            }
            MailClientCommand::DismissDetail => state.dismiss_detail(),
            MailClientCommand::Claim { mail_id } => {
                if state
                    .claim_workflow
                    .as_ref()
                    .is_some_and(|workflow| {
                        workflow.mail_id == *mail_id
                            && (workflow.post_attempted
                                || matches!(
                                    workflow.state,
                                    MailClaimWorkflowState::Submitting
                                        | MailClaimWorkflowState::Processing
                                        | MailClaimWorkflowState::ReconciliationPending
                                ))
                    })
                    || state.pending.values().any(|pending| {
                        matches!(pending, PendingMailRequest::Claim { mail_id: pending_id, .. } if pending_id == mail_id)
                    })
                    || state
                        .claim_reconciliation
                        .as_ref()
                        .is_some_and(|reconciliation| reconciliation.mail_id == *mail_id)
                {
                    continue;
                }
                if !valid_mail_id(mail_id) || !state.can_submit_claim(mail_id) {
                    reject_request(
                        &mut state,
                        &mut events,
                        MailOperation::Claim,
                        "MAIL_CLAIM_NOT_AVAILABLE",
                    );
                    continue;
                }
                let Some((endpoint, ticket)) =
                    request_context(&state, &session, MailOperation::Claim, &mut events)
                else {
                    set_claim_workflow_state(
                        &mut state,
                        mail_id,
                        MailClaimWorkflowState::Unavailable,
                    );
                    continue;
                };
                if let Some(workflow) = state
                    .claim_workflow
                    .as_mut()
                    .filter(|workflow| workflow.mail_id == *mail_id)
                {
                    workflow.state = MailClaimWorkflowState::Submitting;
                    workflow.post_attempted = true;
                    workflow.error_code = None;
                }
                let request = build_mutation_request(
                    &endpoint,
                    &ticket,
                    mail_id,
                    "/claim",
                    HttpMethod::Post,
                    MAIL_CLAIM_TIMEOUT,
                );
                let identity = state.identity.clone();
                queue_request(
                    &mut state,
                    PendingMailRequest::Claim {
                        mail_id: mail_id.clone(),
                        identity,
                    },
                    request,
                    &mut network_commands,
                );
            }
            MailClientCommand::MailNotifyPush => {
                events.write(MailClientEvent::RefreshRequired);
                state.list_stale = true;
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
    mut state: ResMut<MailClientState>,
    mut network_events: MessageReader<NetworkEvent>,
    mut events: MessageWriter<MailClientEvent>,
) {
    for event in network_events.read() {
        match event {
            NetworkEvent::HttpResponse(response) => {
                let Some(pending) = state.pending.remove(&response.request_id) else {
                    continue;
                };
                handle_mail_response(
                    &mut state,
                    pending,
                    response.status,
                    &response.body,
                    &mut events,
                );
            }
            NetworkEvent::HttpError { request_id, .. } => {
                let Some(pending) = state.pending.remove(request_id) else {
                    continue;
                };
                match pending {
                    PendingMailRequest::Claim { mail_id, identity } => {
                        if identity == state.identity {
                            begin_claim_reconciliation(
                                &mut state,
                                mail_id,
                                MailClaimWorkflowState::ReconciliationPending,
                                &mut events,
                            );
                        }
                    }
                    PendingMailRequest::Detail {
                        mail_id,
                        reconciliation_poll: true,
                        identity,
                        ..
                    } => {
                        if identity == state.identity {
                            continue_claim_reconciliation(&mut state, &mail_id, &mut events);
                        }
                    }
                    PendingMailRequest::List {
                        generation,
                        identity,
                        ..
                    } if generation != state.desired_list_generation
                        || identity != state.identity => {}
                    PendingMailRequest::Detail {
                        generation,
                        identity,
                        reconciliation_poll: false,
                        ..
                    } if generation != state.desired_detail_generation
                        || identity != state.identity => {}
                    PendingMailRequest::MarkRead {
                        generation,
                        identity,
                        ..
                    } if generation != state.desired_detail_generation
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
                        match pending {
                            PendingMailRequest::Detail {
                                reconciliation_poll: false,
                                ..
                            } => {
                                state.detail_load_state = MailDetailLoadState::Failed;
                                state.detail_error = Some(error.clone());
                            }
                            PendingMailRequest::MarkRead { .. } => {
                                state.mark_read_state = MailMarkReadState::Failed;
                                state.detail_error = Some(error.clone());
                            }
                            _ => {}
                        }
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
    state: &mut MailClientState,
    pending: PendingMailRequest,
    status: u16,
    body: &[u8],
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
            generation,
            identity,
        } => {
            if identity != state.identity
                || (!reconciliation_poll && generation != state.desired_detail_generation)
            {
                return;
            }
            let Some(response) = parse_success::<MailDetailResponse>(
                state,
                MailOperation::Detail,
                status,
                body,
                events,
            ) else {
                if reconciliation_poll {
                    continue_claim_reconciliation(state, &mail_id, events);
                } else {
                    set_detail_failure_from_last_error(state);
                }
                return;
            };
            if !response.ok || response.mail.summary.mail_id != mail_id {
                reject_request(state, events, MailOperation::Detail, "MAIL_DETAIL_REJECTED");
                if reconciliation_poll {
                    continue_claim_reconciliation(state, &mail_id, events);
                } else {
                    set_detail_failure_from_last_error(state);
                }
                return;
            }
            if !detail_response_is_within_budget(&response.mail) {
                reject_request(
                    state,
                    events,
                    MailOperation::Detail,
                    "MAIL_DETAIL_LIMIT_EXCEEDED",
                );
                if reconciliation_poll {
                    continue_claim_reconciliation(state, &mail_id, events);
                } else {
                    set_detail_failure_from_last_error(state);
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
                if reconciliation_poll {
                    continue_claim_reconciliation(state, &mail_id, events);
                } else {
                    set_detail_failure_from_last_error(state);
                }
                return;
            }
            if reconciliation_poll {
                apply_reconciled_claim_detail(state, &mail_id, &response.mail);
            } else {
                let post_attempted = state
                    .claim_workflow
                    .as_ref()
                    .is_some_and(|workflow| workflow.mail_id == mail_id && workflow.post_attempted);
                state.claim_workflow =
                    Some(claim_workflow_from_detail(&response.mail, post_attempted));
            }
            if !reconciliation_poll || state.selected_mail_id.as_deref() == Some(&mail_id) {
                let expired = response.mail.summary.status.eq_ignore_ascii_case("expired");
                state.selected_mail = Some(response.mail);
                state.selected_mail_id = Some(mail_id.clone());
                state.detail_load_state = if expired {
                    MailDetailLoadState::Expired
                } else {
                    MailDetailLoadState::Ready
                };
                state.mark_read_state = MailMarkReadState::Idle;
                state.detail_error = None;
            }
            state.last_error = None;
            events.write(MailClientEvent::MailLoaded {
                mail_id: mail_id.clone(),
            });
            if reconciliation_poll {
                apply_reconciled_claim_status(state, mail_id, claim_status, events);
            } else if matches!(
                claim_status.as_deref(),
                Some("processing" | "reconciliation_pending")
            ) {
                let workflow_state = if claim_status.as_deref() == Some("processing") {
                    MailClaimWorkflowState::Processing
                } else {
                    MailClaimWorkflowState::ReconciliationPending
                };
                begin_claim_reconciliation(state, mail_id, workflow_state, events);
            }
        }
        PendingMailRequest::MarkRead {
            mail_id,
            generation,
            identity,
        } => {
            if generation != state.desired_detail_generation
                || identity != state.identity
                || state.selected_mail_id.as_deref() != Some(&mail_id)
            {
                return;
            }
            let Some(response) = parse_success::<MailReadResponse>(
                state,
                MailOperation::MarkRead,
                status,
                body,
                events,
            ) else {
                state.mark_read_state = MailMarkReadState::Failed;
                state.detail_error = state.last_error.clone();
                return;
            };
            if !response.ok || response.mail_id != mail_id || response.status != "read" {
                reject_request(state, events, MailOperation::MarkRead, "MAIL_READ_REJECTED");
                state.mark_read_state = MailMarkReadState::Failed;
                state.detail_error = state.last_error.clone();
                return;
            }
            apply_read_to_cache(state, &mail_id, response.read_at);
            state.mark_read_state = MailMarkReadState::Succeeded;
            state.detail_error = None;
            state.last_error = None;
            events.write(MailClientEvent::MailRead {
                mail_id: mail_id.clone(),
                already_read: response.already_read,
            });
        }
        PendingMailRequest::Claim { mail_id, identity } => {
            if identity != state.identity {
                return;
            }
            if status == 202 {
                begin_claim_reconciliation(
                    state,
                    mail_id,
                    MailClaimWorkflowState::ReconciliationPending,
                    events,
                );
                return;
            }
            let Some(response) = parse_success::<MailClaimResponse>(
                state,
                MailOperation::Claim,
                status,
                body,
                events,
            ) else {
                let error = state.last_error.clone();
                if error.as_ref().is_some_and(claim_entry_is_paused) {
                    update_claim_workflow_error(
                        state,
                        &mail_id,
                        MailClaimWorkflowState::Unavailable,
                        error.as_ref(),
                    );
                } else if status >= 500 {
                    begin_claim_reconciliation(
                        state,
                        mail_id,
                        MailClaimWorkflowState::ReconciliationPending,
                        events,
                    );
                } else {
                    update_claim_workflow_error(
                        state,
                        &mail_id,
                        MailClaimWorkflowState::PermanentFailure,
                        error.as_ref(),
                    );
                }
                return;
            };
            if !response.ok || (!response.mail_id.is_empty() && response.mail_id != mail_id) {
                reject_request(state, events, MailOperation::Claim, "MAIL_CLAIM_REJECTED");
                let error = state.last_error.clone();
                update_claim_workflow_error(
                    state,
                    &mail_id,
                    MailClaimWorkflowState::PermanentFailure,
                    error.as_ref(),
                );
                return;
            }
            let claim_status = response.claim_status.clone();
            if !claim_status.as_deref().is_some_and(valid_claim_status) {
                reject_request(state, events, MailOperation::Claim, "MAIL_RESPONSE_INVALID");
                let error = state.last_error.clone();
                update_claim_workflow_error(
                    state,
                    &mail_id,
                    MailClaimWorkflowState::ManualReview,
                    error.as_ref(),
                );
                return;
            }
            if response.processing
                || matches!(
                    claim_status.as_deref(),
                    Some("processing" | "reconciliation_pending")
                )
            {
                let workflow_state = if claim_status.as_deref() == Some("processing") {
                    MailClaimWorkflowState::Processing
                } else {
                    MailClaimWorkflowState::ReconciliationPending
                };
                apply_claim_response_to_detail(state, &mail_id, &response);
                begin_claim_reconciliation(state, mail_id, workflow_state, events);
                return;
            }
            apply_claim_to_cache(state, &mail_id, &response);
            apply_claim_response_to_detail(state, &mail_id, &response);
            set_claim_workflow_from_response(state, &mail_id, &response);
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

fn drive_queued_list_refresh(
    session: Res<MyServerSession>,
    mut state: ResMut<MailClientState>,
    mut network_commands: MessageWriter<NetworkCommand>,
    mut events: MessageWriter<MailClientEvent>,
) {
    if !state.list_refresh_queued
        || state
            .pending
            .values()
            .any(|pending| matches!(pending, PendingMailRequest::List { .. }))
    {
        return;
    }
    state.list_refresh_queued = false;
    let query = state.refresh_query();
    start_list_request(
        &session,
        &mut state,
        query,
        &mut network_commands,
        &mut events,
    );
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
    if offset == 0
        && state
            .pending
            .values()
            .any(|pending| matches!(pending, PendingMailRequest::List { .. }))
    {
        state.list_stale = true;
        state.list_refresh_queued = true;
        return;
    }
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
    let generation = if reconciliation_poll {
        state.desired_detail_generation
    } else {
        if !state.contains_authoritative_mail(mail_id) {
            reject_request(
                state,
                events,
                MailOperation::Detail,
                "MAIL_DETAIL_NOT_IN_LIST",
            );
            return;
        }
        state.desired_detail_generation = state.desired_detail_generation.wrapping_add(1);
        state.selected_mail_id = Some(mail_id.to_owned());
        state.selected_mail = None;
        state.detail_load_state = MailDetailLoadState::Loading;
        state.mark_read_state = MailMarkReadState::Idle;
        state.detail_error = None;
        state.last_error = None;
        state.desired_detail_generation
    };
    if reconciliation_poll
        && state.pending.values().any(|pending| {
            matches!(pending, PendingMailRequest::Detail { mail_id: pending_id, .. } if pending_id == mail_id)
        })
    {
        return;
    }
    let Some((endpoint, ticket)) = request_context(state, session, MailOperation::Detail, events)
    else {
        if reconciliation_poll {
            continue_claim_reconciliation(state, mail_id, events);
        } else {
            state.detail_load_state = MailDetailLoadState::Failed;
            let error = MailClientError {
                operation: MailOperation::Detail,
                status: None,
                code: "MAIL_DETAIL_UNAVAILABLE".to_owned(),
            };
            state.detail_error = Some(error.clone());
            state.last_error = Some(error);
        }
        return;
    };
    if reconciliation_poll {
        if let Some(reconciliation) = state.claim_reconciliation.as_mut() {
            reconciliation.polls_completed = reconciliation.polls_completed.saturating_add(1);
        }
    }
    let request = build_read_request(&endpoint, &ticket, &format!("/{mail_id}"));
    let identity = state.identity.clone();
    queue_request(
        state,
        PendingMailRequest::Detail {
            mail_id: mail_id.to_string(),
            reconciliation_poll,
            generation,
            identity,
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
        .filter(|code| valid_public_error_code(code))
        .unwrap_or_else(|| format!("MAIL_HTTP_{status}"));
    MailClientError {
        operation,
        status: Some(status),
        code,
    }
}

fn valid_public_error_code(code: &str) -> bool {
    (1..=64).contains(&code.len())
        && code
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
}

fn detail_response_is_within_budget(detail: &MailDetail) -> bool {
    detail.content.len() <= MAIL_MAX_DETAIL_CONTENT_BYTES
        && detail.content.chars().count() <= MAIL_MAX_DETAIL_CONTENT_CHARS
        && detail.attachments.len() <= MAIL_MAX_DETAIL_ATTACHMENTS
}

fn set_detail_failure_from_last_error(state: &mut MailClientState) {
    let error = state.last_error.clone().unwrap_or(MailClientError {
        operation: MailOperation::Detail,
        status: None,
        code: "MAIL_DETAIL_FAILED".to_owned(),
    });
    state.detail_load_state = match error.status {
        Some(403) => MailDetailLoadState::Forbidden,
        Some(404) => MailDetailLoadState::NotFound,
        Some(410) => MailDetailLoadState::Expired,
        _ if error.code == "MAIL_EXPIRED" => MailDetailLoadState::Expired,
        _ => MailDetailLoadState::Failed,
    };
    state.detail_error = Some(error);
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

fn claim_workflow_from_detail(detail: &MailDetail, post_attempted: bool) -> MailClaimWorkflow {
    let state = if detail.summary.status.eq_ignore_ascii_case("expired") {
        MailClaimWorkflowState::Expired
    } else if detail.claim.already_claimed {
        MailClaimWorkflowState::AlreadyClaimed
    } else {
        match detail.claim.claim_status.as_deref() {
            Some("claimed") => MailClaimWorkflowState::Claimed,
            Some("processing") => MailClaimWorkflowState::Processing,
            Some("reconciliation_pending") => MailClaimWorkflowState::ReconciliationPending,
            Some("retryable_failure") => MailClaimWorkflowState::RetryableFailure,
            Some("blocked_capacity") => MailClaimWorkflowState::BlockedCapacity,
            Some("permanent_failure") => MailClaimWorkflowState::PermanentFailure,
            Some("manual_review") => MailClaimWorkflowState::ManualReview,
            _ if detail.attachments.is_empty() => MailClaimWorkflowState::Idle,
            _ => MailClaimWorkflowState::Available,
        }
    };
    MailClaimWorkflow {
        mail_id: detail.summary.mail_id.clone(),
        state,
        player_retryable: detail.claim.player_retryable,
        exhausted: false,
        result_state: public_result_state(detail.claim.result_state.as_deref()),
        error_code: public_claim_error(detail.claim.error.as_deref()),
        post_attempted,
    }
}

fn set_claim_workflow_state(
    state: &mut MailClientState,
    mail_id: &str,
    workflow_state: MailClaimWorkflowState,
) {
    if let Some(workflow) = state
        .claim_workflow
        .as_mut()
        .filter(|workflow| workflow.mail_id == mail_id)
    {
        workflow.state = workflow_state;
        return;
    }
    state.claim_workflow = Some(MailClaimWorkflow {
        mail_id: mail_id.to_owned(),
        state: workflow_state,
        player_retryable: false,
        exhausted: false,
        result_state: None,
        error_code: None,
        post_attempted: true,
    });
}

fn update_claim_workflow_error(
    state: &mut MailClientState,
    mail_id: &str,
    workflow_state: MailClaimWorkflowState,
    error: Option<&MailClientError>,
) {
    set_claim_workflow_state(state, mail_id, workflow_state);
    if let Some(workflow) = state.claim_workflow.as_mut() {
        workflow.error_code = error.map(|error| error.code.clone());
        workflow.player_retryable = false;
    }
}

fn claim_entry_is_paused(error: &MailClientError) -> bool {
    matches!(
        error.code.as_str(),
        "MAIL_CLAIM_DISABLED"
            | "MAIL_CLAIM_PAUSED"
            | "MAIL_CLAIM_ENTRY_PAUSED"
            | "MAIL_CLAIM_UNAVAILABLE"
    )
}

fn public_result_state(value: Option<&str>) -> Option<String> {
    value
        .filter(|value| matches!(*value, "success" | "failure" | "unknown" | "pending"))
        .map(str::to_owned)
}

fn public_claim_error(value: Option<&str>) -> Option<String> {
    value
        .filter(|value| valid_public_error_code(value))
        .map(str::to_owned)
}

fn set_claim_workflow_from_response(
    state: &mut MailClientState,
    mail_id: &str,
    response: &MailClaimResponse,
) {
    let workflow_state = if response.already_claimed {
        MailClaimWorkflowState::AlreadyClaimed
    } else {
        match response.claim_status.as_deref() {
            Some("claimed") => MailClaimWorkflowState::Claimed,
            Some("processing") => MailClaimWorkflowState::Processing,
            Some("reconciliation_pending") => MailClaimWorkflowState::ReconciliationPending,
            Some("retryable_failure") => MailClaimWorkflowState::RetryableFailure,
            Some("blocked_capacity") => MailClaimWorkflowState::BlockedCapacity,
            Some("permanent_failure") => MailClaimWorkflowState::PermanentFailure,
            Some("manual_review") => MailClaimWorkflowState::ManualReview,
            _ => MailClaimWorkflowState::ManualReview,
        }
    };
    set_claim_workflow_state(state, mail_id, workflow_state);
    if let Some(workflow) = state.claim_workflow.as_mut() {
        workflow.player_retryable = response.player_retryable;
        workflow.result_state = public_result_state(response.result_state.as_deref());
        workflow.error_code = public_claim_error(response.error.as_deref());
        workflow.exhausted = false;
    }
}

fn apply_claim_response_to_detail(
    state: &mut MailClientState,
    mail_id: &str,
    response: &MailClaimResponse,
) {
    let Some(detail) = state
        .selected_mail
        .as_mut()
        .filter(|detail| detail.summary.mail_id == mail_id)
    else {
        return;
    };
    if let Some(status) = response.status.as_ref() {
        detail.summary.status = status.clone();
    }
    detail.summary.read_at = response.read_at.clone().or(detail.summary.read_at.take());
    detail.summary.claimed_at = response
        .claimed_at
        .clone()
        .or(detail.summary.claimed_at.take());
    if !response.attachments.is_empty() {
        detail.attachments = response.attachments.clone();
    }
    detail.claim.claim_status = response.claim_status.clone();
    detail.claim.already_claimed = response.already_claimed;
    detail.claim.processing = response.processing;
    detail.claim.retryable = response.retryable;
    detail.claim.player_retryable = response.player_retryable;
    detail.claim.attempts = response.attempts;
    detail.claim.result_state = public_result_state(response.result_state.as_deref());
    detail.claim.error = public_claim_error(response.error.as_deref());
}

fn apply_reconciled_claim_detail(state: &mut MailClientState, mail_id: &str, detail: &MailDetail) {
    let was_unread = state
        .mails
        .iter()
        .find(|summary| summary.mail_id == mail_id)
        .is_some_and(|summary| summary.status.eq_ignore_ascii_case("unread"));
    if let Some(summary) = state
        .mails
        .iter_mut()
        .find(|summary| summary.mail_id == mail_id)
    {
        *summary = detail.summary.clone();
    }
    if was_unread && !detail.summary.status.eq_ignore_ascii_case("unread") {
        state.unread_count = state.unread_count.map(|count| count.saturating_sub(1));
    }
    state.claim_workflow = Some(claim_workflow_from_detail(detail, true));
}

fn begin_claim_reconciliation(
    state: &mut MailClientState,
    mail_id: String,
    workflow_state: MailClaimWorkflowState,
    events: &mut MessageWriter<MailClientEvent>,
) {
    if state
        .claim_reconciliation
        .as_ref()
        .is_some_and(|reconciliation| reconciliation.mail_id == mail_id)
    {
        set_claim_workflow_state(state, &mail_id, workflow_state);
        return;
    }
    state.claim_reconciliation = Some(MailClaimReconciliation {
        mail_id: mail_id.clone(),
        polls_completed: 0,
        next_poll: Some(Timer::new(Duration::from_secs(1), TimerMode::Once)),
    });
    set_claim_workflow_state(state, &mail_id, workflow_state);
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
        state.claim_reconciliation = None;
        if let Some(workflow) = state
            .claim_workflow
            .as_mut()
            .filter(|workflow| workflow.mail_id == mail_id)
        {
            workflow.state = MailClaimWorkflowState::ManualReview;
            workflow.exhausted = true;
            workflow.player_retryable = false;
            workflow.result_state = Some("unknown".to_owned());
            workflow.error_code = Some("MAIL_CLAIM_RECONCILIATION_EXHAUSTED".to_owned());
        }
        events.write(MailClientEvent::ClaimReconciliationSettled {
            mail_id: mail_id.to_string(),
            claim_status: Some("manual_review".to_owned()),
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
    let was_unread = state
        .mails
        .iter()
        .find(|mail| mail.mail_id == mail_id)
        .is_some_and(|mail| mail.status.eq_ignore_ascii_case("unread"));
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
    if was_unread {
        state.unread_count = state.unread_count.map(|count| count.saturating_sub(1));
    }
}

fn apply_claim_to_cache(state: &mut MailClientState, mail_id: &str, response: &MailClaimResponse) {
    let claimed = response.claimed
        || response.already_claimed
        || response.claim_status.as_deref() == Some("claimed");
    if !claimed {
        return;
    }
    let was_unread = state
        .mails
        .iter()
        .find(|mail| mail.mail_id == mail_id)
        .is_some_and(|mail| mail.status.eq_ignore_ascii_case("unread"));
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
    if was_unread {
        state.unread_count = state.unread_count.map(|count| count.saturating_sub(1));
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
            chat::{ChatClientState, ChatClientStatus, ChatEvent, MAIL_NOTIFY_PUSH},
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

    fn summary(mail_id: &str, status: &str) -> MailSummary {
        MailSummary {
            mail_id: mail_id.to_owned(),
            sender: MailSender::default(),
            title: format!("Mail {mail_id}"),
            mail_type: "system".to_owned(),
            status: status.to_owned(),
            has_attachments: false,
            created_at: None,
            read_at: None,
            claimed_at: None,
            expires_at: None,
        }
    }

    fn seed_claimable_mail(app: &mut App, mail_id: &str) {
        app.update();
        let mut mail = summary(mail_id, "unread");
        mail.has_attachments = true;
        let detail = MailDetail {
            summary: mail.clone(),
            content: "Reward".to_owned(),
            attachments: vec![MailAttachment {
                r#type: "item".to_owned(),
                id: Some(1001),
                count: 2,
                binded: true,
            }],
            claim: MailClaimSummary::default(),
        };
        let mut state = app.world_mut().resource_mut::<MailClientState>();
        state.mails = vec![mail];
        state.unread_count = Some(1);
        state.list_load_state = MailListLoadState::Ready;
        state.selected_mail_id = Some(mail_id.to_owned());
        state.selected_mail = Some(detail.clone());
        state.detail_load_state = MailDetailLoadState::Ready;
        state.claim_workflow = Some(claim_workflow_from_detail(&detail, false));
    }

    fn submit_claim(app: &mut App, mail_id: &str) -> HttpRequest {
        app.world_mut().write_message(MailClientCommand::Claim {
            mail_id: mail_id.to_owned(),
        });
        app.update();
        http_requests(app).pop().unwrap()
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
    fn overlapping_list_refresh_is_coalesced_and_followed_by_one_latest_request() {
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
        assert_eq!(http_requests(&app).len(), 1);
        assert_eq!(
            app.world().resource::<MailClientState>().list_generation(),
            1
        );
        assert!(
            app.world()
                .resource::<MailClientState>()
                .list_refresh_queued
        );

        respond(
            &mut app,
            &first,
            200,
            r#"{"ok":true,"mails":[{"mail_id":"mail_before_latest","title":"Before"}],
                "unread_count":1,"pagination":{"limit":50,"offset":0,"next_offset":null}}"#,
        );
        let requests = http_requests(&app);
        assert_eq!(requests.len(), 2);
        let second = requests.last().unwrap();
        assert_ne!(first.request_id, second.request_id);
        assert_eq!(
            app.world().resource::<MailClientState>().list_generation(),
            2
        );
        respond(
            &mut app,
            second,
            200,
            r#"{"ok":true,"mails":[{"mail_id":"mail_latest","title":"Latest"}],
                "unread_count":1,"pagination":{"limit":50,"offset":0,"next_offset":null}}"#,
        );
        let state = app.world().resource::<MailClientState>();
        assert_eq!(state.mails.len(), 1);
        assert_eq!(state.mails[0].mail_id, "mail_latest");
        assert_eq!(state.authoritative_unread_count(), Some(1));
        assert_eq!(http_requests(&app).len(), 2);
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
            state.mails.push(summary("mail_1", "unread"));
            state.unread_count = Some(1);
        }

        app.world_mut().write_message(MailClientCommand::LoadMail {
            mail_id: "mail_1".to_owned(),
        });
        app.update();
        let detail = http_requests(&app).pop().unwrap();
        respond(
            &mut app,
            &detail,
            200,
            r#"{"ok":true,"mail":{"mail_id":"mail_1","status":"unread","content":"Welcome"}}"#,
        );

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
            r#"{"ok":true,"mail_id":"mail_1","status":"read","read_at":"2026-08-01T00:00:00Z","already_read":true}"#,
        );
        let state = app.world().resource::<MailClientState>();
        assert_eq!(state.mails[0].status, "read");
        assert_eq!(state.selected_mail.as_ref().unwrap().summary.status, "read");
        assert_eq!(state.unread_count, Some(0));
        assert_eq!(state.mark_read_state, MailMarkReadState::Succeeded);
        assert!(!state.list_stale);
        assert!(
            messages::<MailClientEvent>(&app)
                .iter()
                .any(|event| matches!(
                    event,
                    MailClientEvent::MailRead {
                        mail_id,
                        already_read: true
                    } if mail_id == "mail_1"
                ))
        );
    }

    #[test]
    fn detail_generation_discards_late_selection_and_old_identity() {
        let mut app = test_app(session_with_mail());
        app.update();
        app.world_mut().resource_mut::<MailClientState>().mails =
            vec![summary("mail_a", "unread"), summary("mail_b", "unread")];

        for mail_id in ["mail_a", "mail_b"] {
            app.world_mut().write_message(MailClientCommand::LoadMail {
                mail_id: mail_id.to_owned(),
            });
            app.update();
        }
        let requests = http_requests(&app);
        let request_a = &requests[0];
        let request_b = &requests[1];
        respond(
            &mut app,
            request_b,
            200,
            r#"{"ok":true,"mail":{"mail_id":"mail_b","status":"unread","content":"new"}}"#,
        );
        respond(
            &mut app,
            request_a,
            200,
            r#"{"ok":true,"mail":{"mail_id":"mail_a","status":"unread","content":"old"}}"#,
        );
        let state = app.world().resource::<MailClientState>();
        assert_eq!(state.selected_mail_id(), Some("mail_b"));
        assert_eq!(state.selected_mail.as_ref().unwrap().content, "new");

        app.world_mut().write_message(MailClientCommand::LoadMail {
            mail_id: "mail_b".to_owned(),
        });
        app.update();
        let old_identity = http_requests(&app).pop().unwrap();
        app.world_mut()
            .resource_mut::<MyServerSession>()
            .character_id = Some("character_2".to_owned());
        app.update();
        respond(
            &mut app,
            &old_identity,
            200,
            r#"{"ok":true,"mail":{"mail_id":"mail_b","content":"wrong identity"}}"#,
        );
        let state = app.world().resource::<MailClientState>();
        assert!(state.selected_mail_id().is_none());
        assert!(state.selected_mail.is_none());
    }

    #[test]
    fn detail_errors_are_classified_and_sensitive_error_text_is_not_exposed() {
        for (status, expected) in [
            (403, MailDetailLoadState::Forbidden),
            (404, MailDetailLoadState::NotFound),
            (410, MailDetailLoadState::Expired),
        ] {
            let mut app = test_app(session_with_mail());
            app.update();
            app.world_mut().resource_mut::<MailClientState>().mails =
                vec![summary("mail_1", "unread")];
            app.world_mut().write_message(MailClientCommand::LoadMail {
                mail_id: "mail_1".to_owned(),
            });
            app.update();
            let request = http_requests(&app).pop().unwrap();
            respond(
                &mut app,
                &request,
                status,
                r#"{"error":"private body sentinel must not become a public code"}"#,
            );
            let state = app.world().resource::<MailClientState>();
            assert_eq!(state.detail_load_state, expected);
            let debug = format!("{state:?}");
            assert!(!debug.contains("private body sentinel"));
            assert_eq!(
                state.detail_error.as_ref().unwrap().code,
                format!("MAIL_HTTP_{status}")
            );
        }
    }

    #[test]
    fn detail_rejects_oversized_content_and_attachment_lists_without_retaining_them() {
        let oversized_content = "SENSITIVE_CONTENT".repeat(600);
        let attachments = (0..=MAIL_MAX_DETAIL_ATTACHMENTS)
            .map(|index| format!(r#"{{"type":"item","id":{index},"count":1}}"#))
            .collect::<Vec<_>>()
            .join(",");
        for body in [
            format!(
                r#"{{"ok":true,"mail":{{"mail_id":"mail_1","content":"{oversized_content}"}}}}"#
            ),
            format!(
                r#"{{"ok":true,"mail":{{"mail_id":"mail_1","content":"safe","attachments":[{attachments}]}}}}"#
            ),
        ] {
            let mut app = test_app(session_with_mail());
            app.update();
            app.world_mut().resource_mut::<MailClientState>().mails =
                vec![summary("mail_1", "unread")];
            app.world_mut().write_message(MailClientCommand::LoadMail {
                mail_id: "mail_1".to_owned(),
            });
            app.update();
            let request = http_requests(&app).pop().unwrap();
            respond(&mut app, &request, 200, &body);
            let state = app.world().resource::<MailClientState>();
            assert_eq!(state.detail_load_state, MailDetailLoadState::Failed);
            assert!(state.selected_mail.is_none());
            assert_eq!(
                state.detail_error.as_ref().unwrap().code,
                "MAIL_DETAIL_LIMIT_EXCEEDED"
            );
            assert!(!format!("{state:?}").contains("SENSITIVE_CONTENT"));
        }
    }

    #[test]
    fn mark_read_failure_preserves_authoritative_unread_state_and_can_retry() {
        let mut app = test_app(session_with_mail());
        app.update();
        {
            let mut state = app.world_mut().resource_mut::<MailClientState>();
            let detail = MailDetail {
                summary: summary("mail_1", "unread"),
                content: "secret body".to_owned(),
                attachments: Vec::new(),
                claim: MailClaimSummary::default(),
            };
            state.mails = vec![summary("mail_1", "unread")];
            state.unread_count = Some(1);
            state.selected_mail_id = Some("mail_1".to_owned());
            state.selected_mail = Some(detail);
            state.detail_load_state = MailDetailLoadState::Ready;
        }
        app.world_mut().write_message(MailClientCommand::MarkRead {
            mail_id: "mail_1".to_owned(),
        });
        app.update();
        let request = http_requests(&app).pop().unwrap();
        app.world_mut().write_message(NetworkEvent::HttpError {
            request_id: request.request_id,
            error: "timeout".to_owned(),
        });
        app.update();
        let state = app.world().resource::<MailClientState>();
        assert_eq!(state.mark_read_state, MailMarkReadState::Failed);
        assert_eq!(state.mails[0].status, "unread");
        assert_eq!(
            state.selected_mail.as_ref().unwrap().summary.status,
            "unread"
        );
        assert_eq!(state.unread_count, Some(1));
        assert!(!format!("{state:?}").contains("secret body"));
    }

    #[test]
    fn unknown_claim_converges_through_detail_without_another_claim_post() {
        let mut app = test_app(session_with_mail());
        seed_claimable_mail(&mut app, "mail_1");
        let claim = submit_claim(&mut app, "mail_1");
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
        let state = app.world().resource::<MailClientState>();
        assert_eq!(
            state.claim_workflow.as_ref().map(|workflow| workflow.state),
            Some(MailClaimWorkflowState::AlreadyClaimed)
        );
        assert_eq!(state.mails[0].status, "claimed");
        assert_eq!(state.unread_count, Some(0));
    }

    #[test]
    fn claim_requires_ready_authoritative_attachment_and_posts_only_once() {
        let mut app = test_app(session_with_mail());
        app.update();
        {
            let detail = MailDetail {
                summary: summary("mail_1", "unread"),
                content: String::new(),
                attachments: Vec::new(),
                claim: MailClaimSummary::default(),
            };
            let mut state = app.world_mut().resource_mut::<MailClientState>();
            state.mails = vec![detail.summary.clone()];
            state.selected_mail_id = Some("mail_1".to_owned());
            state.selected_mail = Some(detail.clone());
            state.detail_load_state = MailDetailLoadState::Ready;
            state.claim_workflow = Some(claim_workflow_from_detail(&detail, false));
        }
        assert!(
            !app.world()
                .resource::<MailClientState>()
                .can_submit_claim("mail_1")
        );
        app.world_mut().write_message(MailClientCommand::Claim {
            mail_id: "mail_1".to_owned(),
        });
        app.update();
        assert!(http_requests(&app).is_empty());

        seed_claimable_mail(&mut app, "mail_1");
        assert!(
            app.world()
                .resource::<MailClientState>()
                .can_submit_claim("mail_1")
        );
        let claim = submit_claim(&mut app, "mail_1");
        assert!(matches!(claim.method, HttpMethod::Post));
        assert_eq!(
            app.world()
                .resource::<MailClientState>()
                .claim_workflow
                .as_ref()
                .map(|workflow| workflow.state),
            Some(MailClaimWorkflowState::Submitting)
        );
        app.world_mut().write_message(MailClientCommand::Claim {
            mail_id: "mail_1".to_owned(),
        });
        app.update();
        assert_eq!(
            http_requests(&app)
                .iter()
                .filter(|request| matches!(request.method, HttpMethod::Post))
                .count(),
            1
        );

        app.world_mut().write_message(NetworkEvent::HttpError {
            request_id: claim.request_id,
            error: "connection reset".to_owned(),
        });
        app.update();
        let state = app.world().resource::<MailClientState>();
        assert_eq!(
            state.claim_workflow.as_ref().map(|workflow| workflow.state),
            Some(MailClaimWorkflowState::ReconciliationPending)
        );
        assert!(state.claim_reconciliation.is_some());
    }

    #[test]
    fn claim_terminal_states_preserve_server_retryability_and_distinguish_already_claimed() {
        for (status, extra, expected) in [
            (
                "claimed",
                r#""claimed":true"#,
                MailClaimWorkflowState::Claimed,
            ),
            (
                "claimed",
                r#""already_claimed":true"#,
                MailClaimWorkflowState::AlreadyClaimed,
            ),
            (
                "retryable_failure",
                r#""player_retryable":true,"result_state":"failure","error":"MAIL_INVENTORY_BUSY""#,
                MailClaimWorkflowState::RetryableFailure,
            ),
            (
                "blocked_capacity",
                r#""player_retryable":true,"result_state":"failure","error":"MAIL_INVENTORY_CAPACITY""#,
                MailClaimWorkflowState::BlockedCapacity,
            ),
            (
                "permanent_failure",
                r#""result_state":"failure","error":"MAIL_REWARD_INVALID""#,
                MailClaimWorkflowState::PermanentFailure,
            ),
            (
                "manual_review",
                r#""result_state":"unknown","error":"MAIL_CLAIM_REVIEW_REQUIRED""#,
                MailClaimWorkflowState::ManualReview,
            ),
        ] {
            let mut app = test_app(session_with_mail());
            seed_claimable_mail(&mut app, "mail_1");
            let claim = submit_claim(&mut app, "mail_1");
            let body =
                format!(r#"{{"ok":true,"mail_id":"mail_1","claim_status":"{status}",{extra}}}"#);
            respond(&mut app, &claim, 200, &body);
            let workflow = app
                .world()
                .resource::<MailClientState>()
                .claim_workflow
                .as_ref()
                .unwrap();
            assert_eq!(workflow.state, expected, "claim status {status}");
            if matches!(
                expected,
                MailClaimWorkflowState::RetryableFailure | MailClaimWorkflowState::BlockedCapacity
            ) {
                assert!(workflow.player_retryable);
            }
        }
    }

    #[test]
    fn claim_entry_pause_is_unavailable_without_starting_reconciliation() {
        let mut app = test_app(session_with_mail());
        seed_claimable_mail(&mut app, "mail_1");
        let claim = submit_claim(&mut app, "mail_1");
        respond(
            &mut app,
            &claim,
            503,
            r#"{"ok":false,"error":"MAIL_CLAIM_PAUSED","message":"paused"}"#,
        );
        let state = app.world().resource::<MailClientState>();
        assert_eq!(
            state.claim_workflow.as_ref().map(|workflow| workflow.state),
            Some(MailClaimWorkflowState::Unavailable)
        );
        assert!(state.claim_reconciliation.is_none());
    }

    #[test]
    fn reconciliation_survives_detail_dismiss_and_exhausts_after_three_gets() {
        let mut app = test_app(session_with_mail());
        seed_claimable_mail(&mut app, "mail_1");
        let claim = submit_claim(&mut app, "mail_1");
        respond(&mut app, &claim, 202, "");
        app.world_mut()
            .write_message(MailClientCommand::DismissDetail);
        app.update();
        assert!(
            app.world()
                .resource::<MailClientState>()
                .claim_reconciliation
                .is_some()
        );

        for poll in 1..=3 {
            app.world_mut()
                .write_message(MailClientCommand::PollClaimReconciliation {
                    mail_id: "mail_1".to_owned(),
                });
            app.update();
            let detail = http_requests(&app).pop().unwrap();
            assert!(matches!(detail.method, HttpMethod::Get));
            respond(
                &mut app,
                &detail,
                200,
                r#"{"ok":true,"mail":{"mail_id":"mail_1","status":"claiming","claim":{"claim_status":"reconciliation_pending"}}}"#,
            );
            assert_eq!(
                app.world()
                    .resource::<MailClientState>()
                    .claim_reconciliation
                    .is_some(),
                poll < 3
            );
        }
        let state = app.world().resource::<MailClientState>();
        let workflow = state.claim_workflow.as_ref().unwrap();
        assert_eq!(workflow.state, MailClaimWorkflowState::ManualReview);
        assert!(workflow.exhausted);
        assert_eq!(workflow.result_state.as_deref(), Some("unknown"));
        assert_eq!(
            workflow.error_code.as_deref(),
            Some("MAIL_CLAIM_RECONCILIATION_EXHAUSTED")
        );
    }

    #[test]
    fn reconciliation_get_network_error_schedules_the_next_bounded_poll() {
        let mut app = test_app(session_with_mail());
        seed_claimable_mail(&mut app, "mail_1");
        let claim = submit_claim(&mut app, "mail_1");
        respond(&mut app, &claim, 202, "");
        app.world_mut()
            .write_message(MailClientCommand::PollClaimReconciliation {
                mail_id: "mail_1".to_owned(),
            });
        app.update();
        let detail = http_requests(&app).pop().unwrap();
        app.world_mut().write_message(NetworkEvent::HttpError {
            request_id: detail.request_id,
            error: "connection reset".to_owned(),
        });
        app.update();
        let reconciliation = app
            .world()
            .resource::<MailClientState>()
            .claim_reconciliation
            .as_ref()
            .unwrap();
        assert_eq!(reconciliation.polls_completed, 1);
        assert!(reconciliation.next_poll.is_some());
        assert_eq!(
            app.world()
                .resource::<MailClientState>()
                .claim_workflow
                .as_ref()
                .map(|workflow| workflow.state),
            Some(MailClaimWorkflowState::ReconciliationPending)
        );
    }

    #[test]
    fn ticket_change_clears_claim_workflow_and_discards_late_claim_response() {
        let mut app = test_app(session_with_mail());
        seed_claimable_mail(&mut app, "mail_1");
        let claim = submit_claim(&mut app, "mail_1");
        app.world_mut().resource_mut::<MyServerSession>().ticket =
            Some("rotated-character-ticket".to_owned());
        app.update();
        {
            let state = app.world().resource::<MailClientState>();
            assert!(state.claim_workflow.is_none());
            assert!(state.claim_reconciliation.is_none());
            assert!(state.mails.is_empty());
        }
        respond(
            &mut app,
            &claim,
            200,
            r#"{"ok":true,"mail_id":"mail_1","claim_status":"claimed","claimed":true}"#,
        );
        let state = app.world().resource::<MailClientState>();
        assert!(state.claim_workflow.is_none());
        assert!(state.mails.is_empty());
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
            status: ChatClientStatus::Ready,
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

        app.world_mut()
            .write_message(ChatEvent::MailNotifyPush { generation: 8 });
        app.world_mut()
            .write_message(ChatEvent::MailNotifyPush { generation: 8 });
        app.update();
        assert_eq!(http_requests(&app).len(), 1);
        assert!(
            app.world()
                .resource::<MailClientState>()
                .list_refresh_queued
        );

        respond(
            &mut app,
            &requests[0],
            200,
            r#"{"ok":true,"mails":[{"mail_id":"mail_before_latest"}],"unread_count":1,
                "pagination":{"limit":50,"offset":0,"next_offset":null}}"#,
        );
        let requests = http_requests(&app);
        assert_eq!(requests.len(), 2);
        assert_ne!(requests[0].request_id, requests[1].request_id);
        assert!(app.world().resource::<MailClientState>().list_stale);
        assert!(
            !app.world()
                .resource::<MailClientState>()
                .list_refresh_queued
        );

        respond(
            &mut app,
            &requests[1],
            200,
            r#"{"ok":true,"mails":[{"mail_id":"mail_latest"}],"unread_count":1,
                "pagination":{"limit":50,"offset":0,"next_offset":null}}"#,
        );
        let state = app.world().resource::<MailClientState>();
        assert_eq!(state.mails[0].mail_id, "mail_latest");
        assert!(!state.list_stale);
        assert_eq!(http_requests(&app).len(), 2);
    }

    #[test]
    fn ticket_rotation_cancels_old_list_and_uses_the_latest_ticket() {
        let mut app = test_app(session_with_mail());
        app.world_mut().write_message(MailClientCommand::LoadList {
            query: MailListQuery::default(),
        });
        app.update();
        let old = http_requests(&app).pop().unwrap();
        assert_eq!(
            header(&old, "X-Game-Ticket"),
            Some("character-bound-ticket")
        );

        app.world_mut().resource_mut::<MyServerSession>().ticket =
            Some("rotated-character-ticket".to_owned());
        app.update();
        assert!(messages::<NetworkCommand>(&app).iter().any(|command| {
            matches!(command, NetworkCommand::CancelHttp { request_id } if *request_id == old.request_id)
        }));
        app.world_mut().write_message(MailClientCommand::LoadList {
            query: MailListQuery::default(),
        });
        app.update();
        let latest = http_requests(&app).pop().unwrap();
        assert_ne!(latest.request_id, old.request_id);
        assert_eq!(
            header(&latest, "X-Game-Ticket"),
            Some("rotated-character-ticket")
        );

        respond(
            &mut app,
            &old,
            200,
            r#"{"ok":true,"mails":[{"mail_id":"mail_old_ticket"}],"unread_count":9,
                "pagination":{"limit":50,"offset":0,"next_offset":null}}"#,
        );
        assert!(app.world().resource::<MailClientState>().mails.is_empty());
        respond(
            &mut app,
            &latest,
            200,
            r#"{"ok":true,"mails":[{"mail_id":"mail_latest_ticket"}],"unread_count":1,
                "pagination":{"limit":50,"offset":0,"next_offset":null}}"#,
        );
        assert_eq!(
            app.world().resource::<MailClientState>().mails[0].mail_id,
            "mail_latest_ticket"
        );
    }

    #[test]
    fn account_character_and_environment_resets_cancel_and_clear_mail_state() {
        for cause in ["logout", "account", "character", "environment"] {
            let mut app = test_app(session_with_mail());
            app.world_mut().write_message(MailClientCommand::LoadList {
                query: MailListQuery::default(),
            });
            app.update();
            let pending = http_requests(&app).pop().unwrap();
            {
                let mut state = app.world_mut().resource_mut::<MailClientState>();
                let detail = MailDetail {
                    summary: summary("mail_cached", "unread"),
                    content: "cached body".to_owned(),
                    attachments: Vec::new(),
                    claim: MailClaimSummary::default(),
                };
                state.mails = vec![detail.summary.clone()];
                state.unread_count = Some(1);
                state.selected_mail_id = Some("mail_cached".to_owned());
                state.selected_mail = Some(detail);
                state.detail_load_state = MailDetailLoadState::Failed;
                state.detail_error = Some(MailClientError {
                    operation: MailOperation::Detail,
                    status: Some(503),
                    code: "MAIL_HTTP_503".to_owned(),
                });
                state.last_error = state.detail_error.clone();
                state.list_refresh_queued = true;
            }
            {
                let mut session = app.world_mut().resource_mut::<MyServerSession>();
                match cause {
                    "logout" => session.logout(),
                    "account" => session.switch_account(),
                    "character" => session.switch_character(),
                    "environment" => session.reset(),
                    _ => unreachable!(),
                }
                assert!(session.mail_endpoint.is_none(), "cause {cause}");
                assert!(session.mail_endpoint_error.is_none(), "cause {cause}");
            }
            app.update();
            assert!(messages::<NetworkCommand>(&app).iter().any(|command| {
                matches!(command, NetworkCommand::CancelHttp { request_id } if *request_id == pending.request_id)
            }));
            let state = app.world().resource::<MailClientState>();
            assert!(state.mails.is_empty(), "cause {cause}");
            assert!(state.unread_count.is_none(), "cause {cause}");
            assert!(state.selected_mail.is_none(), "cause {cause}");
            assert!(state.detail_error.is_none(), "cause {cause}");
            assert!(state.last_error.is_none(), "cause {cause}");
            assert!(!state.list_refresh_queued, "cause {cause}");
            assert!(matches!(
                state.availability,
                MailAvailability::Unavailable { .. }
            ));
        }
    }

    #[test]
    fn unavailable_or_unreachable_mail_does_not_mutate_chat_state() {
        for session in [
            MyServerSession {
                player_id: Some("player_1".to_owned()),
                character_id: Some("character_1".to_owned()),
                ticket: Some("ticket".to_owned()),
                ..Default::default()
            },
            MyServerSession {
                player_id: Some("player_1".to_owned()),
                character_id: Some("character_1".to_owned()),
                ticket: Some("ticket".to_owned()),
                mail_endpoint_error: Some("invalid mail descriptor".to_owned()),
                ..Default::default()
            },
        ] {
            let mut app = test_app(session);
            app.insert_resource(ChatClientState {
                status: ChatClientStatus::Ready,
                endpoint_generation: 11,
                ..Default::default()
            });
            app.world_mut()
                .write_message(ChatEvent::MailNotifyPush { generation: 11 });
            app.update();
            assert!(http_requests(&app).is_empty());
            assert!(matches!(
                app.world().resource::<MailClientState>().availability,
                MailAvailability::Unavailable { .. }
            ));
            let chat = app.world().resource::<ChatClientState>();
            assert_eq!(chat.status, ChatClientStatus::Ready);
            assert_eq!(chat.endpoint_generation, 11);
        }

        let mut app = test_app(session_with_mail());
        app.insert_resource(ChatClientState {
            status: ChatClientStatus::Ready,
            endpoint_generation: 12,
            ..Default::default()
        });
        app.world_mut().write_message(MailClientCommand::LoadList {
            query: MailListQuery::default(),
        });
        app.update();
        let request = http_requests(&app).pop().unwrap();
        app.world_mut().write_message(NetworkEvent::HttpError {
            request_id: request.request_id,
            error: "connection refused".to_owned(),
        });
        app.update();
        assert_eq!(
            app.world().resource::<MailClientState>().list_load_state,
            MailListLoadState::Failed
        );
        assert_eq!(
            app.world().resource::<ChatClientState>().status,
            ChatClientStatus::Ready
        );
    }

    #[test]
    fn reconciliation_uses_bounded_one_two_four_second_backoff() {
        assert_eq!(reconciliation_delay(0), Duration::from_secs(1));
        assert_eq!(reconciliation_delay(1), Duration::from_secs(2));
        assert_eq!(reconciliation_delay(2), Duration::from_secs(4));
    }
}
