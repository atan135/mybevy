#![allow(dead_code)]

//! Chat WebSocket transport. This is deliberately separate from the game TCP/KCP session:
//! chat has its own authenticated connection, pending-response map, and reconnect lifecycle.
//! The public API is intentionally not bound to UI in this migration stage.

use std::{
    collections::{HashMap, VecDeque},
    net::Ipv6Addr,
    sync::{
        Mutex,
        mpsc::{self as std_mpsc, TryRecvError},
    },
    thread,
    time::{Duration, Instant},
};

use bevy::{
    app::AppExit,
    ecs::message::{MessageReader, MessageWriter},
    prelude::{App, IntoScheduleConfigs, Plugin, Res, ResMut, Resource, Update},
};
use futures_util::{SinkExt, StreamExt};
use prost::Message;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message as WebSocketMessage};

use super::{
    protocol::{HEADER_LEN, Packet, PacketCodec, chat_pb, encode_raw_packet_type, parse_header},
    types::{
        ClientServiceEndpoint, ClientServices, MyServerCommand, MyServerEvent, MyServerSession,
        redact_secret_fingerprint,
    },
};

pub const CHAT_AUTH_REQ: u16 = 20_001;
pub const CHAT_AUTH_RES: u16 = 20_002;
pub const CHAT_PRIVATE_REQ: u16 = 20_101;
pub const CHAT_PRIVATE_RES: u16 = 20_102;
pub const CHAT_GROUP_REQ: u16 = 20_103;
pub const CHAT_GROUP_RES: u16 = 20_104;
pub const CHAT_PUSH: u16 = 20_105;
pub const GROUP_CREATE_REQ: u16 = 20_201;
pub const GROUP_CREATE_RES: u16 = 20_202;
pub const GROUP_JOIN_REQ: u16 = 20_203;
pub const GROUP_JOIN_RES: u16 = 20_204;
pub const GROUP_LEAVE_REQ: u16 = 20_205;
pub const GROUP_LEAVE_RES: u16 = 20_206;
pub const GROUP_DISMISS_REQ: u16 = 20_207;
pub const GROUP_DISMISS_RES: u16 = 20_208;
pub const GROUP_LIST_REQ: u16 = 20_209;
pub const GROUP_LIST_RES: u16 = 20_210;
pub const CHAT_HISTORY_REQ: u16 = 20_211;
pub const CHAT_HISTORY_RES: u16 = 20_212;
pub const MAIL_NOTIFY_PUSH: u16 = 20_301;
pub const ERROR_RES: u16 = 9_000;

/// Matches the server's default `MAX_BODY_LEN`. A deployment can negotiate a different
/// configured limit only when the application-level session owns that policy.
pub const DEFAULT_CHAT_MAX_BODY_LEN: usize = 4_096;
pub const DEFAULT_CHAT_MAX_FRAME_LEN: usize = HEADER_LEN + DEFAULT_CHAT_MAX_BODY_LEN;
pub const MAX_CHAT_RUNTIME_EVENTS_PER_UPDATE: usize = 32;
pub const MAX_PREAUTH_CHAT_REQUESTS: usize = 16;
pub const CHAT_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

pub const CHAT_PROTOCOL_MESSAGE_TYPES: &[(u16, &str)] = &[
    (CHAT_AUTH_REQ, "ChatAuthReq"),
    (CHAT_AUTH_RES, "ChatAuthRes"),
    (CHAT_PRIVATE_REQ, "ChatPrivateReq"),
    (CHAT_PRIVATE_RES, "ChatPrivateRes"),
    (CHAT_GROUP_REQ, "ChatGroupReq"),
    (CHAT_GROUP_RES, "ChatGroupRes"),
    (CHAT_PUSH, "ChatPush"),
    (GROUP_CREATE_REQ, "GroupCreateReq"),
    (GROUP_CREATE_RES, "GroupCreateRes"),
    (GROUP_JOIN_REQ, "GroupJoinReq"),
    (GROUP_JOIN_RES, "GroupJoinRes"),
    (GROUP_LEAVE_REQ, "GroupLeaveReq"),
    (GROUP_LEAVE_RES, "GroupLeaveRes"),
    (GROUP_DISMISS_REQ, "GroupDismissReq"),
    (GROUP_DISMISS_RES, "GroupDismissRes"),
    (GROUP_LIST_REQ, "GroupListReq"),
    (GROUP_LIST_RES, "GroupListRes"),
    (CHAT_HISTORY_REQ, "ChatHistoryReq"),
    (CHAT_HISTORY_RES, "ChatHistoryRes"),
    (MAIL_NOTIFY_PUSH, "MailNotifyPush"),
    (ERROR_RES, "ErrorRes"),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatWebSocketEndpoint {
    url: String,
}

impl ChatWebSocketEndpoint {
    pub fn from_services(services: Option<&ClientServices>) -> Result<Option<Self>, String> {
        let Some(service) = services.and_then(|services| services.chat.as_ref()) else {
            return Ok(None);
        };
        Self::from_service(service).map(Some)
    }

    pub fn from_service(service: &ClientServiceEndpoint) -> Result<Self, String> {
        let protocol = service
            .protocol
            .as_deref()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        if protocol != "ws" && protocol != "wss" {
            return Err("services.chat protocol must be ws or wss".to_string());
        }

        let host = service.host.as_deref().unwrap_or_default().trim();
        if host.is_empty()
            || host.chars().any(char::is_whitespace)
            || host.contains(['/', '?', '#', '@'])
        {
            return Err("services.chat host must be a bare hostname or IP address".to_string());
        }
        let port = service
            .port
            .filter(|port| *port > 0)
            .ok_or_else(|| "services.chat port must be between 1 and 65535".to_string())?;
        let authority = chat_endpoint_authority(host)?;
        let default_port = match protocol.as_str() {
            "wss" => 443,
            _ => 80,
        };
        let port_suffix = if port == default_port {
            String::new()
        } else {
            format!(":{port}")
        };

        Ok(Self {
            url: format!("{protocol}://{authority}{port_suffix}/"),
        })
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn is_secure(&self) -> bool {
        self.url.starts_with("wss://")
    }
}

fn chat_endpoint_authority(host: &str) -> Result<String, String> {
    if let Some(bracketed) = host.strip_prefix('[') {
        let ipv6 = bracketed
            .strip_suffix(']')
            .ok_or_else(|| "services.chat IPv6 host must use matching brackets".to_string())?;
        ipv6.parse::<Ipv6Addr>()
            .map_err(|_| "services.chat host contains an invalid IPv6 address".to_string())?;
        return Ok(format!("[{ipv6}]"));
    }
    if host.contains(':') {
        host.parse::<Ipv6Addr>()
            .map_err(|_| "services.chat host contains an invalid IPv6 address".to_string())?;
        return Ok(format!("[{host}]"));
    }
    if !host
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
    {
        return Err("services.chat host must be a bare hostname or IP address".to_string());
    }
    Ok(host.to_string())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChatRequestKey {
    pub message_type: u16,
    pub seq: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatInbound {
    Response(Packet),
    ChatPush(Packet),
    MailNotifyPush(Packet),
    Unknown(Packet),
}

/// Correlates responses by their pair of response message type and sequence number.
/// Pushes are never inserted into the pending map and therefore cannot consume a response.
#[derive(Clone, Debug)]
pub struct ChatPacketRouter {
    next_seq: u32,
    pending: HashMap<ChatRequestKey, u16>,
}

impl Default for ChatPacketRouter {
    fn default() -> Self {
        Self {
            next_seq: 1,
            pending: HashMap::new(),
        }
    }
}

impl ChatPacketRouter {
    pub fn encode_request(
        &mut self,
        request_message_type: u16,
        response_message_type: u16,
        body: &[u8],
    ) -> (u32, Vec<u8>) {
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1).max(1);
        let packet = encode_chat_packet(request_message_type, seq, body)
            .expect("chat request message type must be declared in the shared protocol");
        self.pending.insert(
            ChatRequestKey {
                message_type: response_message_type,
                seq,
            },
            request_message_type,
        );
        (seq, packet)
    }

    pub fn route(&mut self, packet: Packet) -> ChatInbound {
        match packet.header.msg_type {
            CHAT_PUSH => ChatInbound::ChatPush(packet),
            MAIL_NOTIFY_PUSH => ChatInbound::MailNotifyPush(packet),
            message_type
                if self
                    .pending
                    .remove(&ChatRequestKey {
                        message_type,
                        seq: packet.header.seq,
                    })
                    .is_some() =>
            {
                ChatInbound::Response(packet)
            }
            _ => ChatInbound::Unknown(packet),
        }
    }

    pub fn cancel(&mut self, key: ChatRequestKey) -> Option<u16> {
        self.pending.remove(&key)
    }
}

/// A WebSocket logical binary message must contain exactly one existing packet.
pub fn decode_chat_binary_frame(frame: &[u8], max_body_len: usize) -> Result<Packet, String> {
    let max_frame_len = HEADER_LEN
        .checked_add(max_body_len)
        .ok_or_else(|| "chat maximum frame length overflow".to_string())?;
    if frame.len() > max_frame_len {
        return Err(format!(
            "chat WebSocket binary message exceeds frame limit: {} > {max_frame_len}",
            frame.len()
        ));
    }
    let header = parse_header(frame)?;
    let body_len = usize::try_from(header.body_len)
        .map_err(|_| "chat packet body length does not fit this platform".to_string())?;
    if body_len > max_body_len {
        return Err(format!(
            "chat packet body exceeds limit: {body_len} > {max_body_len}"
        ));
    }
    let expected_len = HEADER_LEN
        .checked_add(body_len)
        .ok_or_else(|| "chat packet frame length overflow".to_string())?;
    if frame.len() != expected_len {
        return Err(format!(
            "chat WebSocket binary message must contain exactly one packet: {} != {expected_len}",
            frame.len()
        ));
    }

    let mut codec = PacketCodec::new(max_body_len);
    let mut packets = codec.push_bytes(frame)?;
    if packets.len() != 1 {
        return Err(
            "chat WebSocket binary message did not decode to exactly one packet".to_string(),
        );
    }
    Ok(packets.remove(0))
}

#[derive(Clone, Debug)]
pub struct ChatReconnectPolicy {
    pub initial_delay: Duration,
    pub max_delay: Duration,
}

impl Default for ChatReconnectPolicy {
    fn default() -> Self {
        Self {
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(20),
        }
    }
}

impl ChatReconnectPolicy {
    pub fn delay_for(&self, attempt: u32, salt: u64) -> Duration {
        let multiplier = 1_u128 << attempt.min(16);
        let base_ms = self.initial_delay.as_millis().saturating_mul(multiplier);
        let capped_ms = base_ms.min(self.max_delay.as_millis());
        // Deterministic per-client jitter keeps tests stable while avoiding reconnect herds.
        let jitter_percent = (salt
            .wrapping_mul(1_103_515_245)
            .wrapping_add(u64::from(attempt))
            % 41) as i128
            - 20;
        let jittered_ms = (capped_ms as i128 * (100 + jitter_percent) / 100).max(1) as u128;
        Duration::from_millis(jittered_ms.min(u128::from(u64::MAX)) as u64)
    }
}

#[derive(Clone, Debug)]
pub enum ChatRuntimeEvent {
    Connected { url: String },
    Disconnected { reason: String },
    ReconnectScheduled { attempt: u32, delay: Duration },
    Packet(Packet),
    ProtocolError { error: String },
    TicketRefreshRequired,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatClientStatus {
    Unavailable,
    Disconnected,
    Connecting,
    Authenticating,
    Ready,
    Backoff { attempt: u32 },
    Paused,
    Failed,
}

#[derive(Clone, Debug, Resource)]
pub struct ChatClientState {
    pub status: ChatClientStatus,
    pub endpoint_generation: u64,
    pub endpoint_fingerprint: Option<String>,
    pub last_error_code: Option<String>,
    pub foreground: bool,
    pub reconnect_attempt: Option<u32>,
    pub pending_request_count: usize,
    pub preauth_queue_depth: usize,
    pub ticket_refresh_in_flight: bool,
}

impl Default for ChatClientState {
    fn default() -> Self {
        Self {
            status: ChatClientStatus::Unavailable,
            endpoint_generation: 0,
            endpoint_fingerprint: None,
            last_error_code: None,
            foreground: true,
            reconnect_attempt: None,
            pending_request_count: 0,
            preauth_queue_depth: 0,
            ticket_refresh_in_flight: false,
        }
    }
}

#[derive(Clone, bevy::prelude::Message)]
pub enum ChatCommand {
    SetForeground {
        foreground: bool,
    },
    SendRequest {
        request_message_type: u16,
        response_message_type: u16,
        body: Vec<u8>,
    },
}

#[derive(Clone, Debug, bevy::prelude::Message)]
pub enum ChatEvent {
    StateChanged {
        status: ChatClientStatus,
        generation: u64,
        error_code: Option<String>,
    },
    RuntimeDisconnected {
        generation: u64,
        error_code: String,
    },
    ProtocolError {
        generation: u64,
        error_code: String,
    },
    TicketRefreshRequired {
        generation: u64,
    },
    AuthenticationFailed {
        generation: u64,
        error_code: String,
    },
    Response {
        generation: u64,
        message_type: u16,
        seq: u32,
    },
    ChatPush {
        generation: u64,
        seq: u32,
    },
    MailNotifyPush {
        generation: u64,
    },
    RequestFailed {
        generation: u64,
        request_message_type: u16,
        seq: Option<u32>,
        error_code: String,
    },
}

#[derive(Resource, Default)]
pub struct ChatRuntimeOwner {
    active: Option<ActiveChatRuntime>,
}

struct ActiveChatRuntime {
    runtime: ChatRuntime,
    generation: u64,
    authenticated: bool,
    router: ChatPacketRouter,
    pending_deadlines: HashMap<ChatRequestKey, Instant>,
    preauth_queue: VecDeque<ChatOutboundRequest>,
    refresh_requested: bool,
}

struct ChatOutboundRequest {
    request_message_type: u16,
    response_message_type: u16,
    body: Vec<u8>,
}

impl ChatRuntimeOwner {
    pub fn is_active(&self) -> bool {
        self.active.is_some()
    }

    pub fn active_generation(&self) -> Option<u64> {
        self.active.as_ref().map(|active| active.generation)
    }

    fn shutdown(&mut self) {
        if let Some(active) = self.active.take() {
            let _ = active.runtime.disconnect();
        }
    }
}

impl Drop for ChatRuntimeOwner {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub struct ChatPlugin;

impl Plugin for ChatPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChatClientState>()
            .init_resource::<ChatRuntimeOwner>()
            .add_message::<ChatCommand>()
            .add_message::<ChatEvent>()
            .add_message::<MyServerCommand>()
            .add_message::<MyServerEvent>()
            .add_message::<AppExit>()
            .add_systems(
                Update,
                (
                    apply_chat_commands,
                    reconcile_chat_runtime,
                    drain_chat_runtime_events,
                    handle_chat_ticket_refresh_result,
                    sync_chat_diagnostics,
                    shutdown_chat_runtime_on_exit,
                )
                    .chain(),
            );
    }
}

enum ChatRuntimeCommand {
    Send(Vec<u8>),
    ReplaceTicket(String),
    SetForeground(bool),
    Disconnect,
}

/// Handle owned by the game layer. It starts an independent native WebSocket worker;
/// UI/background systems call `set_foreground`, and ticket refresh completion calls
/// `replace_ticket` before the next reconnect authentication attempt.
pub struct ChatRuntime {
    command_tx: mpsc::Sender<ChatRuntimeCommand>,
    event_rx: Mutex<std_mpsc::Receiver<ChatRuntimeEvent>>,
}

#[derive(Debug)]
pub struct ChatRuntimeDrain {
    pub events: Vec<ChatRuntimeEvent>,
    pub channel_closed: bool,
}

impl ChatRuntime {
    pub fn start(
        endpoint: ChatWebSocketEndpoint,
        ticket: String,
        max_body_len: usize,
        reconnect_policy: ChatReconnectPolicy,
    ) -> Result<Self, String> {
        let (command_tx, command_rx) = mpsc::channel(64);
        let (event_tx, event_rx) = std_mpsc::channel();
        thread::Builder::new()
            .name("myserver-chat-ws".to_string())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build();
                match runtime {
                    Ok(runtime) => runtime.block_on(run_chat_worker(
                        endpoint,
                        ticket,
                        max_body_len.max(1),
                        reconnect_policy,
                        command_rx,
                        event_tx,
                    )),
                    Err(error) => {
                        // The receiver can already have been dropped during application shutdown.
                        let _ = event_tx.send(ChatRuntimeEvent::Disconnected {
                            reason: format!("failed to start chat runtime: {error}"),
                        });
                    }
                }
            })
            .map_err(|error| format!("failed to start chat worker thread: {error}"))?;

        Ok(Self {
            command_tx,
            event_rx: Mutex::new(event_rx),
        })
    }

    pub fn send_packet(&self, packet: Vec<u8>) -> Result<(), String> {
        self.command_tx
            .try_send(ChatRuntimeCommand::Send(packet))
            .map_err(|error| format!("chat send queue unavailable: {error}"))
    }

    pub fn replace_ticket(&self, ticket: String) -> Result<(), String> {
        self.command_tx
            .try_send(ChatRuntimeCommand::ReplaceTicket(ticket))
            .map_err(|error| format!("chat ticket update queue unavailable: {error}"))
    }

    pub fn set_foreground(&self, foreground: bool) -> Result<(), String> {
        self.command_tx
            .try_send(ChatRuntimeCommand::SetForeground(foreground))
            .map_err(|error| format!("chat foreground queue unavailable: {error}"))
    }

    pub fn disconnect(&self) -> Result<(), String> {
        self.command_tx
            .try_send(ChatRuntimeCommand::Disconnect)
            .map_err(|error| format!("chat disconnect queue unavailable: {error}"))
    }

    pub fn drain_events(&self) -> Vec<ChatRuntimeEvent> {
        let Ok(event_rx) = self.event_rx.lock() else {
            return Vec::new();
        };
        event_rx.try_iter().collect()
    }

    pub fn drain_events_up_to(&self, limit: usize) -> ChatRuntimeDrain {
        let Ok(event_rx) = self.event_rx.lock() else {
            return ChatRuntimeDrain {
                events: Vec::new(),
                channel_closed: true,
            };
        };
        let mut events = Vec::with_capacity(limit);
        let mut channel_closed = false;
        for _ in 0..limit {
            match event_rx.try_recv() {
                Ok(event) => events.push(event),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    channel_closed = true;
                    break;
                }
            }
        }
        ChatRuntimeDrain {
            events,
            channel_closed,
        }
    }
}

fn apply_chat_commands(
    mut commands: MessageReader<ChatCommand>,
    mut state: ResMut<ChatClientState>,
    mut owner: ResMut<ChatRuntimeOwner>,
    mut events: MessageWriter<ChatEvent>,
) {
    for command in commands.read() {
        match command {
            ChatCommand::SetForeground { foreground } => {
                if state.foreground == *foreground {
                    continue;
                }
                state.foreground = *foreground;
                if let Some(active) = owner.active.as_ref() {
                    let _ = active.runtime.set_foreground(*foreground);
                }
                if !foreground {
                    if let Some(active) = owner.active.as_mut() {
                        cancel_chat_requests(
                            active,
                            &mut events,
                            "CHAT_REQUEST_CANCELLED_BACKGROUND",
                        );
                    }
                }
            }
            ChatCommand::SendRequest {
                request_message_type,
                response_message_type,
                body,
            } => {
                let Some(active) = owner.active.as_mut() else {
                    events.write(ChatEvent::RequestFailed {
                        generation: state.endpoint_generation,
                        request_message_type: *request_message_type,
                        seq: None,
                        error_code: "CHAT_RUNTIME_UNAVAILABLE".to_string(),
                    });
                    continue;
                };
                if !active.authenticated {
                    if active.preauth_queue.len() >= MAX_PREAUTH_CHAT_REQUESTS {
                        events.write(ChatEvent::RequestFailed {
                            generation: active.generation,
                            request_message_type: *request_message_type,
                            seq: None,
                            error_code: "CHAT_AUTH_PENDING_QUEUE_FULL".to_string(),
                        });
                    } else {
                        active.preauth_queue.push_back(ChatOutboundRequest {
                            request_message_type: *request_message_type,
                            response_message_type: *response_message_type,
                            body: body.clone(),
                        });
                    }
                    continue;
                }
                queue_chat_request(
                    active,
                    *request_message_type,
                    *response_message_type,
                    body,
                    &mut events,
                );
            }
        }
    }
}

fn reconcile_chat_runtime(
    session: Res<MyServerSession>,
    mut state: ResMut<ChatClientState>,
    mut owner: ResMut<ChatRuntimeOwner>,
    mut events: MessageWriter<ChatEvent>,
) {
    let endpoint_fingerprint = session
        .chat_endpoint
        .as_ref()
        .map(|endpoint| redact_chat_endpoint(endpoint));
    state.endpoint_generation = session.chat_endpoint_generation;
    state.endpoint_fingerprint = endpoint_fingerprint;

    let active_generation = owner.active_generation();
    if active_generation.is_some_and(|generation| generation != session.chat_endpoint_generation) {
        owner.shutdown();
    }

    if !state.foreground {
        set_chat_state(
            &mut state,
            &mut events,
            ChatClientStatus::Paused,
            session.chat_endpoint_generation,
            None,
            None,
        );
        return;
    }

    let Some(endpoint) = session.chat_endpoint.clone() else {
        owner.shutdown();
        set_chat_state(
            &mut state,
            &mut events,
            ChatClientStatus::Unavailable,
            session.chat_endpoint_generation,
            session
                .chat_endpoint_error
                .as_ref()
                .map(|_| "CHAT_ENDPOINT_UNAVAILABLE".to_string()),
            None,
        );
        return;
    };
    let Some(ticket) = session
        .ticket
        .as_deref()
        .map(str::trim)
        .filter(|ticket| !ticket.is_empty())
    else {
        owner.shutdown();
        set_chat_state(
            &mut state,
            &mut events,
            ChatClientStatus::Disconnected,
            session.chat_endpoint_generation,
            Some("CHAT_CHARACTER_TICKET_REQUIRED".to_string()),
            None,
        );
        return;
    };

    if owner.active.is_none() {
        match ChatRuntime::start(
            endpoint,
            ticket.to_string(),
            DEFAULT_CHAT_MAX_BODY_LEN,
            ChatReconnectPolicy::default(),
        ) {
            Ok(runtime) => {
                owner.active = Some(ActiveChatRuntime {
                    runtime,
                    generation: session.chat_endpoint_generation,
                    authenticated: false,
                    router: ChatPacketRouter::default(),
                    pending_deadlines: HashMap::new(),
                    preauth_queue: VecDeque::new(),
                    refresh_requested: false,
                });
                set_chat_state(
                    &mut state,
                    &mut events,
                    ChatClientStatus::Connecting,
                    session.chat_endpoint_generation,
                    None,
                    None,
                );
            }
            Err(_) => {
                set_chat_state(
                    &mut state,
                    &mut events,
                    ChatClientStatus::Failed,
                    session.chat_endpoint_generation,
                    Some("CHAT_RUNTIME_START_FAILED".to_string()),
                    None,
                );
            }
        }
    } else if state.status == ChatClientStatus::Paused {
        set_chat_state(
            &mut state,
            &mut events,
            ChatClientStatus::Connecting,
            session.chat_endpoint_generation,
            None,
            None,
        );
    }
}

fn drain_chat_runtime_events(
    session: Res<MyServerSession>,
    mut state: ResMut<ChatClientState>,
    mut owner: ResMut<ChatRuntimeOwner>,
    mut events: MessageWriter<ChatEvent>,
    mut myserver_commands: MessageWriter<MyServerCommand>,
) {
    let Some(active) = owner.active.as_ref() else {
        return;
    };
    if active.generation != session.chat_endpoint_generation {
        return;
    }
    let generation = active.generation;
    let drained = active
        .runtime
        .drain_events_up_to(MAX_CHAT_RUNTIME_EVENTS_PER_UPDATE);

    for event in drained.events {
        match event {
            ChatRuntimeEvent::Connected { .. } => set_chat_state(
                &mut state,
                &mut events,
                ChatClientStatus::Authenticating,
                generation,
                None,
                None,
            ),
            ChatRuntimeEvent::Disconnected { reason } => {
                let error_code = classify_chat_transport_failure(&reason);
                events.write(ChatEvent::RuntimeDisconnected {
                    generation,
                    error_code: error_code.to_string(),
                });
                set_chat_state(
                    &mut state,
                    &mut events,
                    ChatClientStatus::Disconnected,
                    generation,
                    Some(error_code.to_string()),
                    None,
                );
            }
            ChatRuntimeEvent::ReconnectScheduled { attempt, .. } => set_chat_state(
                &mut state,
                &mut events,
                ChatClientStatus::Backoff { attempt },
                generation,
                None,
                Some(attempt),
            ),
            ChatRuntimeEvent::ProtocolError { .. } => {
                events.write(ChatEvent::ProtocolError {
                    generation,
                    error_code: "CHAT_PROTOCOL_ERROR".to_string(),
                });
                set_chat_state(
                    &mut state,
                    &mut events,
                    ChatClientStatus::Failed,
                    generation,
                    Some("CHAT_PROTOCOL_ERROR".to_string()),
                    None,
                );
            }
            ChatRuntimeEvent::TicketRefreshRequired => {
                let Some(active) = owner.active.as_mut() else {
                    continue;
                };
                if !active.refresh_requested {
                    active.refresh_requested = true;
                    events.write(ChatEvent::TicketRefreshRequired { generation });
                    myserver_commands.write(ticket_refresh_command());
                }
            }
            ChatRuntimeEvent::Packet(packet) => {
                let Some(active) = owner.active.as_mut() else {
                    continue;
                };
                handle_chat_packet(active, packet, &mut state, &mut events);
            }
        }
    }

    if let Some(active) = owner.active.as_mut() {
        expire_chat_requests(active, &mut events);
    }

    if drained.channel_closed {
        owner.shutdown();
        set_chat_state(
            &mut state,
            &mut events,
            ChatClientStatus::Failed,
            generation,
            Some("CHAT_RUNTIME_EVENT_CHANNEL_CLOSED".to_string()),
            None,
        );
    }
}

fn handle_chat_ticket_refresh_result(
    mut myserver_events: MessageReader<MyServerEvent>,
    mut state: ResMut<ChatClientState>,
    mut owner: ResMut<ChatRuntimeOwner>,
    mut events: MessageWriter<ChatEvent>,
) {
    if !myserver_events
        .read()
        .any(|event| matches!(event, MyServerEvent::TicketRefreshFailed { .. }))
    {
        return;
    }
    let generation = state.endpoint_generation;
    owner.shutdown();
    set_chat_state(
        &mut state,
        &mut events,
        ChatClientStatus::Failed,
        generation,
        Some("CHAT_TICKET_REFRESH_FAILED".to_string()),
        None,
    );
}

fn sync_chat_diagnostics(mut state: ResMut<ChatClientState>, owner: Res<ChatRuntimeOwner>) {
    let Some(active) = owner.active.as_ref() else {
        state.pending_request_count = 0;
        state.preauth_queue_depth = 0;
        state.ticket_refresh_in_flight = false;
        return;
    };
    state.pending_request_count = active.pending_deadlines.len();
    state.preauth_queue_depth = active.preauth_queue.len();
    state.ticket_refresh_in_flight = active.refresh_requested;
}

fn handle_chat_packet(
    active: &mut ActiveChatRuntime,
    packet: Packet,
    state: &mut ChatClientState,
    events: &mut MessageWriter<ChatEvent>,
) {
    if packet.header.msg_type == CHAT_AUTH_RES {
        let auth = match chat_pb::ChatAuthRes::decode(packet.body.as_slice()) {
            Ok(auth) => auth,
            Err(_) => {
                fail_chat_protocol(
                    active.generation,
                    state,
                    events,
                    "CHAT_AUTH_RESPONSE_INVALID",
                );
                return;
            }
        };
        if !auth.ok {
            let error_code = classify_chat_auth_failure(&auth.error_code);
            active.preauth_queue.clear();
            events.write(ChatEvent::AuthenticationFailed {
                generation: active.generation,
                error_code: error_code.clone(),
            });
            set_chat_state(
                state,
                events,
                ChatClientStatus::Failed,
                active.generation,
                Some(error_code),
                None,
            );
            return;
        }
        active.authenticated = true;
        set_chat_state(
            state,
            events,
            ChatClientStatus::Ready,
            active.generation,
            None,
            None,
        );
        while let Some(request) = active.preauth_queue.pop_front() {
            queue_chat_request(
                active,
                request.request_message_type,
                request.response_message_type,
                &request.body,
                events,
            );
        }
        return;
    }

    if !active.authenticated {
        fail_chat_protocol(
            active.generation,
            state,
            events,
            "CHAT_PREAUTH_PACKET_NOT_ALLOWED",
        );
        return;
    }

    if packet.header.msg_type == ERROR_RES {
        let error_code = chat_pb::ErrorRes::decode(packet.body.as_slice())
            .ok()
            .map(|error| classify_chat_server_error(&error.error_code))
            .unwrap_or_else(|| "CHAT_ERROR_RESPONSE_INVALID".to_string());
        events.write(ChatEvent::ProtocolError {
            generation: active.generation,
            error_code: error_code.clone(),
        });
        set_chat_state(
            state,
            events,
            ChatClientStatus::Failed,
            active.generation,
            Some(error_code),
            None,
        );
        return;
    }

    match active.router.route(packet) {
        ChatInbound::Response(packet) => {
            let key = ChatRequestKey {
                message_type: packet.header.msg_type,
                seq: packet.header.seq,
            };
            active.pending_deadlines.remove(&key);
            events.write(ChatEvent::Response {
                generation: active.generation,
                message_type: packet.header.msg_type,
                seq: packet.header.seq,
            });
        }
        ChatInbound::ChatPush(packet) => {
            events.write(ChatEvent::ChatPush {
                generation: active.generation,
                seq: packet.header.seq,
            });
        }
        ChatInbound::MailNotifyPush(_) => {
            events.write(ChatEvent::MailNotifyPush {
                generation: active.generation,
            });
        }
        ChatInbound::Unknown(_) => {
            fail_chat_protocol(active.generation, state, events, "CHAT_PACKET_UNKNOWN");
        }
    }
}

fn queue_chat_request(
    active: &mut ActiveChatRuntime,
    request_message_type: u16,
    response_message_type: u16,
    body: &[u8],
    events: &mut MessageWriter<ChatEvent>,
) {
    let (seq, packet) =
        active
            .router
            .encode_request(request_message_type, response_message_type, body);
    let key = ChatRequestKey {
        message_type: response_message_type,
        seq,
    };
    if active.runtime.send_packet(packet).is_err() {
        active.router.cancel(key);
        events.write(ChatEvent::RequestFailed {
            generation: active.generation,
            request_message_type,
            seq: Some(seq),
            error_code: "CHAT_SEND_QUEUE_UNAVAILABLE".to_string(),
        });
        return;
    }
    active
        .pending_deadlines
        .insert(key, Instant::now() + CHAT_REQUEST_TIMEOUT);
}

fn expire_chat_requests(active: &mut ActiveChatRuntime, events: &mut MessageWriter<ChatEvent>) {
    let now = Instant::now();
    let expired: Vec<_> = active
        .pending_deadlines
        .iter()
        .filter_map(|(key, deadline)| (*deadline <= now).then_some(*key))
        .collect();
    for key in expired {
        active.pending_deadlines.remove(&key);
        let request_message_type = active.router.cancel(key).unwrap_or(0);
        events.write(ChatEvent::RequestFailed {
            generation: active.generation,
            request_message_type,
            seq: Some(key.seq),
            error_code: "CHAT_RESPONSE_TIMEOUT".to_string(),
        });
    }
}

fn cancel_chat_requests(
    active: &mut ActiveChatRuntime,
    events: &mut MessageWriter<ChatEvent>,
    error_code: &str,
) {
    while let Some(request) = active.preauth_queue.pop_front() {
        events.write(ChatEvent::RequestFailed {
            generation: active.generation,
            request_message_type: request.request_message_type,
            seq: None,
            error_code: error_code.to_string(),
        });
    }

    let pending: Vec<_> = active
        .pending_deadlines
        .drain()
        .map(|(key, _)| key)
        .collect();
    for key in pending {
        let request_message_type = active.router.cancel(key).unwrap_or(0);
        events.write(ChatEvent::RequestFailed {
            generation: active.generation,
            request_message_type,
            seq: Some(key.seq),
            error_code: error_code.to_string(),
        });
    }
}

fn fail_chat_protocol(
    generation: u64,
    state: &mut ChatClientState,
    events: &mut MessageWriter<ChatEvent>,
    error_code: &str,
) {
    events.write(ChatEvent::ProtocolError {
        generation,
        error_code: error_code.to_string(),
    });
    set_chat_state(
        state,
        events,
        ChatClientStatus::Failed,
        generation,
        Some(error_code.to_string()),
        None,
    );
}

fn classify_chat_auth_failure(error_code: &str) -> String {
    match error_code.trim().to_ascii_uppercase().as_str() {
        "TICKET_EXPIRED" => "CHAT_AUTH_TICKET_EXPIRED",
        "TICKET_REVOKED" | "INVALID_TICKET" | "AUTH_TICKET_INVALID" => "CHAT_AUTH_TICKET_REVOKED",
        "TICKET_OWNERSHIP_MISMATCH"
        | "TICKET_VERSION_MISMATCH"
        | "PLAYER_TICKET_VERSION_MISMATCH" => "CHAT_AUTH_TICKET_OWNERSHIP",
        "PLAYER_BLOCKED" | "PLAYER_BANNED" => "CHAT_AUTH_PLAYER_BLOCKED",
        "MSG_RATE_EXCEEDED" => "CHAT_AUTH_RATE_LIMITED",
        "SERVICE_UNAVAILABLE" | "BLOCKLIST_UNAVAILABLE" => "CHAT_AUTH_SERVICE_UNAVAILABLE",
        _ => "CHAT_AUTH_REJECTED",
    }
    .to_string()
}

fn classify_chat_transport_failure(reason: &str) -> &'static str {
    let reason = reason.to_ascii_lowercase();
    if reason.contains("tls") || reason.contains("certificate") || reason.contains("handshake") {
        "CHAT_TLS_FAILED"
    } else {
        "CHAT_TRANSPORT_DISCONNECTED"
    }
}

fn classify_chat_server_error(error_code: &str) -> String {
    match error_code.trim().to_ascii_uppercase().as_str() {
        "MSG_RATE_EXCEEDED" => "CHAT_RATE_LIMITED",
        "PREAUTH_MESSAGE_NOT_ALLOWED" => "CHAT_PREAUTH_PACKET_NOT_ALLOWED",
        "OUTBOUND_QUEUE_FULL" => "CHAT_OUTBOUND_QUEUE_FULL",
        _ => "CHAT_SERVER_ERROR",
    }
    .to_string()
}

fn shutdown_chat_runtime_on_exit(
    mut app_exit: MessageReader<AppExit>,
    mut owner: ResMut<ChatRuntimeOwner>,
) {
    if app_exit.read().next().is_some() {
        owner.shutdown();
    }
}

fn set_chat_state(
    state: &mut ChatClientState,
    events: &mut MessageWriter<ChatEvent>,
    status: ChatClientStatus,
    generation: u64,
    error_code: Option<String>,
    reconnect_attempt: Option<u32>,
) {
    let changed = state.status != status
        || state.endpoint_generation != generation
        || state.last_error_code != error_code
        || state.reconnect_attempt != reconnect_attempt;
    state.status = status.clone();
    state.endpoint_generation = generation;
    state.last_error_code = error_code.clone();
    state.reconnect_attempt = reconnect_attempt;
    if changed {
        events.write(ChatEvent::StateChanged {
            status,
            generation,
            error_code,
        });
    }
}

fn redact_chat_endpoint(endpoint: &ChatWebSocketEndpoint) -> String {
    format!("endpoint_fp={}", redact_secret_fingerprint(endpoint.url()))
}

/// Reuse the established HTTP ticket issue flow when chat authentication reports a stale ticket.
pub fn ticket_refresh_command() -> MyServerCommand {
    MyServerCommand::RefreshTicket {
        reconnect_game: false,
    }
}

fn encode_chat_auth_packet(ticket: &str) -> Vec<u8> {
    let request = chat_pb::ChatAuthReq {
        player_id: String::new(),
        token: ticket.to_string(),
    };
    let mut body = Vec::new();
    request
        .encode(&mut body)
        .expect("protobuf encoding to Vec cannot fail");
    encode_raw_packet_type(CHAT_AUTH_REQ, 1, &body)
}

async fn run_chat_worker(
    endpoint: ChatWebSocketEndpoint,
    mut ticket: String,
    max_body_len: usize,
    policy: ChatReconnectPolicy,
    mut command_rx: mpsc::Receiver<ChatRuntimeCommand>,
    event_tx: std_mpsc::Sender<ChatRuntimeEvent>,
) {
    let mut foreground = true;
    let mut attempt = 0_u32;

    loop {
        if !foreground {
            match command_rx.recv().await {
                Some(ChatRuntimeCommand::SetForeground(true)) => foreground = true,
                Some(ChatRuntimeCommand::ReplaceTicket(value)) => ticket = value,
                Some(ChatRuntimeCommand::Disconnect) | None => return,
                Some(ChatRuntimeCommand::Send(_))
                | Some(ChatRuntimeCommand::SetForeground(false)) => {}
            }
            continue;
        }

        let mut socket = match connect_async(endpoint.url()).await {
            Ok((socket, _)) => {
                attempt = 0;
                let _ = event_tx.send(ChatRuntimeEvent::Connected {
                    url: endpoint.url().to_string(),
                });
                socket
            }
            Err(error) => {
                let delay = policy.delay_for(attempt, endpoint.url().len() as u64);
                let _ = event_tx.send(ChatRuntimeEvent::Disconnected {
                    reason: format!("chat connect failed: {error}"),
                });
                let _ = event_tx.send(ChatRuntimeEvent::ReconnectScheduled { attempt, delay });
                attempt = attempt.saturating_add(1);
                if !wait_reconnect_delay(&mut command_rx, &mut ticket, &mut foreground, delay).await
                {
                    return;
                }
                continue;
            }
        };

        if let Err(error) = socket
            .send(WebSocketMessage::Binary(
                encode_chat_auth_packet(&ticket).into(),
            ))
            .await
        {
            let _ = event_tx.send(ChatRuntimeEvent::Disconnected {
                reason: format!("chat auth send failed: {error}"),
            });
            continue;
        }

        let mut reconnect = true;
        while foreground {
            tokio::select! {
                command = command_rx.recv() => match command {
                    Some(ChatRuntimeCommand::Send(packet)) => {
                        if let Err(error) = decode_chat_binary_frame(&packet, max_body_len) {
                            let _ = event_tx.send(ChatRuntimeEvent::ProtocolError { error });
                            continue;
                        }
                        if let Err(error) = socket.send(WebSocketMessage::Binary(packet.into())).await {
                            let _ = event_tx.send(ChatRuntimeEvent::Disconnected { reason: format!("chat send failed: {error}") });
                            break;
                        }
                    }
                    Some(ChatRuntimeCommand::ReplaceTicket(value)) => {
                        ticket = value;
                        let _ = socket.send(WebSocketMessage::Close(None)).await;
                        break;
                    }
                    Some(ChatRuntimeCommand::SetForeground(value)) => {
                        foreground = value;
                        if !foreground {
                            let _ = socket.send(WebSocketMessage::Close(None)).await;
                            let _ = event_tx.send(ChatRuntimeEvent::Disconnected { reason: "chat paused in background".to_string() });
                        }
                        break;
                    }
                    Some(ChatRuntimeCommand::Disconnect) | None => {
                        let _ = socket.send(WebSocketMessage::Close(None)).await;
                        reconnect = false;
                        break;
                    }
                },
                message = socket.next() => match message {
                    Some(Ok(WebSocketMessage::Binary(frame))) => match decode_chat_binary_frame(&frame, max_body_len) {
                        Ok(packet) => {
                            if packet.header.msg_type == CHAT_AUTH_RES {
                                if let Ok(auth) = chat_pb::ChatAuthRes::decode(packet.body.as_slice()) {
                                    if !auth.ok && ticket_error_requires_refresh(&auth.error_code) {
                                        let _ = event_tx.send(ChatRuntimeEvent::TicketRefreshRequired);
                                    }
                                }
                            }
                            let _ = event_tx.send(ChatRuntimeEvent::Packet(packet));
                        }
                        Err(error) => {
                            let _ = event_tx.send(ChatRuntimeEvent::ProtocolError { error });
                            let _ = socket.send(WebSocketMessage::Close(None)).await;
                            break;
                        }
                    },
                    Some(Ok(WebSocketMessage::Ping(payload))) => {
                        if socket.send(WebSocketMessage::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(WebSocketMessage::Text(_))) => {
                        let _ = event_tx.send(ChatRuntimeEvent::ProtocolError { error: "chat received text WebSocket message".to_string() });
                        let _ = socket.send(WebSocketMessage::Close(None)).await;
                        break;
                    }
                    Some(Ok(WebSocketMessage::Close(frame))) => {
                        let reason = frame.map(|frame| frame.reason.to_string()).unwrap_or_else(|| "peer closed chat connection".to_string());
                        let _ = event_tx.send(ChatRuntimeEvent::Disconnected { reason });
                        break;
                    }
                    Some(Ok(WebSocketMessage::Pong(_))) => {}
                    Some(Ok(_)) => {}
                    Some(Err(error)) => {
                        let _ = event_tx.send(ChatRuntimeEvent::Disconnected { reason: format!("chat WebSocket error: {error}") });
                        break;
                    }
                    None => {
                        let _ = event_tx.send(ChatRuntimeEvent::Disconnected { reason: "chat WebSocket stream ended".to_string() });
                        break;
                    }
                }
            }
        }

        if !reconnect {
            return;
        }
        let delay = policy.delay_for(attempt, endpoint.url().len() as u64);
        let _ = event_tx.send(ChatRuntimeEvent::ReconnectScheduled { attempt, delay });
        attempt = attempt.saturating_add(1);
        if !wait_reconnect_delay(&mut command_rx, &mut ticket, &mut foreground, delay).await {
            return;
        }
    }
}

async fn wait_reconnect_delay(
    command_rx: &mut mpsc::Receiver<ChatRuntimeCommand>,
    ticket: &mut String,
    foreground: &mut bool,
    delay: Duration,
) -> bool {
    tokio::select! {
        _ = tokio::time::sleep(delay) => true,
        command = command_rx.recv() => match command {
            Some(ChatRuntimeCommand::ReplaceTicket(value)) => {
                *ticket = value;
                true
            }
            Some(ChatRuntimeCommand::SetForeground(value)) => {
                *foreground = value;
                true
            }
            Some(ChatRuntimeCommand::Disconnect) | None => false,
            Some(ChatRuntimeCommand::Send(_)) => true,
        }
    }
}

fn ticket_error_requires_refresh(error_code: &str) -> bool {
    matches!(
        error_code.trim().to_ascii_uppercase().as_str(),
        "TICKET_EXPIRED" | "TICKET_REVOKED" | "INVALID_TICKET" | "AUTH_TICKET_INVALID"
    )
}

fn encode_chat_packet(message_type: u16, seq: u32, body: &[u8]) -> Result<Vec<u8>, String> {
    Ok(encode_raw_packet_type(message_type, seq, body))
}

#[cfg(test)]
mod tests {
    use bevy::app::AppExit;
    use bevy::ecs::message::{MessageCursor, Messages};
    use bevy::prelude::{App, MinimalPlugins};

    use super::*;

    fn chat_endpoint() -> ChatWebSocketEndpoint {
        ChatWebSocketEndpoint::from_service(&ClientServiceEndpoint {
            host: Some("chat.game.zergzerg.cn".to_string()),
            port: Some(443),
            protocol: Some("wss".to_string()),
        })
        .unwrap()
    }

    fn chat_test_app(session: MyServerSession) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(session)
            .add_plugins(ChatPlugin);
        app
    }

    fn read_myserver_commands(app: &App) -> Vec<MyServerCommand> {
        let messages = app.world().resource::<Messages<MyServerCommand>>();
        let mut cursor = MessageCursor::default();
        cursor.read(messages).cloned().collect()
    }

    fn read_chat_events(app: &App) -> Vec<ChatEvent> {
        let messages = app.world().resource::<Messages<ChatEvent>>();
        let mut cursor = MessageCursor::default();
        cursor.read(messages).cloned().collect()
    }

    fn install_fake_runtime(
        app: &mut App,
        generation: u64,
        authenticated: bool,
        command_capacity: usize,
    ) -> (
        std_mpsc::Sender<ChatRuntimeEvent>,
        mpsc::Receiver<ChatRuntimeCommand>,
    ) {
        let (command_tx, command_rx) = mpsc::channel(command_capacity);
        let (event_tx, event_rx) = std_mpsc::channel();
        app.world_mut().resource_mut::<ChatRuntimeOwner>().active = Some(ActiveChatRuntime {
            runtime: ChatRuntime {
                command_tx,
                event_rx: Mutex::new(event_rx),
            },
            generation,
            authenticated,
            router: ChatPacketRouter::default(),
            pending_deadlines: HashMap::new(),
            preauth_queue: VecDeque::new(),
            refresh_requested: false,
        });
        (event_tx, command_rx)
    }

    fn packet(message_type: u16, seq: u32, body: &[u8]) -> Packet {
        decode_chat_binary_frame(
            &encode_chat_packet(message_type, seq, body).unwrap(),
            DEFAULT_CHAT_MAX_BODY_LEN,
        )
        .unwrap()
    }

    #[test]
    fn builds_public_wss_endpoint_from_auth_descriptor() {
        let endpoint = ChatWebSocketEndpoint::from_service(&ClientServiceEndpoint {
            host: Some("chat.game.zergzerg.cn".to_string()),
            port: Some(443),
            protocol: Some("wss".to_string()),
        })
        .unwrap();
        assert_eq!(endpoint.url(), "wss://chat.game.zergzerg.cn/");
    }

    #[test]
    fn absent_chat_descriptor_is_unavailable_without_an_internal_fallback() {
        assert_eq!(ChatWebSocketEndpoint::from_services(None).unwrap(), None);
        assert_eq!(
            ChatWebSocketEndpoint::from_services(Some(&ClientServices {
                game: None,
                chat: None,
                mail: None,
                announce: None,
            }))
            .unwrap(),
            None
        );
    }

    #[test]
    fn local_ws_descriptor_remains_explicit() {
        let endpoint = ChatWebSocketEndpoint::from_service(&ClientServiceEndpoint {
            host: Some("127.0.0.1".to_string()),
            port: Some(9001),
            protocol: Some("ws".to_string()),
        })
        .unwrap();
        assert_eq!(endpoint.url(), "ws://127.0.0.1:9001/");
    }

    #[test]
    fn parses_ipv6_descriptor_and_rejects_malformed_hosts() {
        let endpoint = ChatWebSocketEndpoint::from_service(&ClientServiceEndpoint {
            host: Some("2001:db8::5".to_string()),
            port: Some(443),
            protocol: Some("wss".to_string()),
        })
        .unwrap();
        assert_eq!(endpoint.url(), "wss://[2001:db8::5]/");

        for host in ["chat.example.com/path", "[2001:db8::5", "host_name"] {
            assert!(
                ChatWebSocketEndpoint::from_service(&ClientServiceEndpoint {
                    host: Some(host.to_string()),
                    port: Some(443),
                    protocol: Some("wss".to_string()),
                })
                .is_err(),
                "host {host} should be rejected"
            );
        }
    }

    #[test]
    fn rejects_non_websocket_or_non_public_chat_descriptor() {
        assert!(
            ChatWebSocketEndpoint::from_service(&ClientServiceEndpoint {
                host: Some("chat-server:9011/path".to_string()),
                port: Some(9011),
                protocol: Some("ws".to_string()),
            })
            .is_err()
        );
        assert!(
            ChatWebSocketEndpoint::from_service(&ClientServiceEndpoint {
                host: Some("chat.example.com".to_string()),
                port: Some(443),
                protocol: Some("tcp".to_string()),
            })
            .is_err()
        );
    }

    #[test]
    fn binary_frame_requires_exactly_one_existing_packet() {
        let packet = encode_chat_packet(CHAT_AUTH_RES, 7, &[1]).unwrap();
        let decoded = decode_chat_binary_frame(&packet, 64).unwrap();
        assert_eq!(decoded.header.msg_type, CHAT_AUTH_RES);
        assert_eq!(decoded.header.seq, 7);

        let mut multiple = packet.clone();
        multiple.extend_from_slice(&encode_chat_packet(CHAT_PUSH, 0, &[]).unwrap());
        assert!(decode_chat_binary_frame(&multiple, 64).is_err());

        let oversized = encode_chat_packet(CHAT_PUSH, 0, &[0; 65]).unwrap();
        assert!(decode_chat_binary_frame(&oversized, 64).is_err());
    }

    #[test]
    fn chat_protocol_message_numbers_and_default_frame_limit_match_the_contract() {
        assert_eq!(DEFAULT_CHAT_MAX_FRAME_LEN, HEADER_LEN + 4_096);
        assert_eq!(
            CHAT_PROTOCOL_MESSAGE_TYPES,
            [
                (20_001, "ChatAuthReq"),
                (20_002, "ChatAuthRes"),
                (20_101, "ChatPrivateReq"),
                (20_102, "ChatPrivateRes"),
                (20_103, "ChatGroupReq"),
                (20_104, "ChatGroupRes"),
                (20_105, "ChatPush"),
                (20_201, "GroupCreateReq"),
                (20_202, "GroupCreateRes"),
                (20_203, "GroupJoinReq"),
                (20_204, "GroupJoinRes"),
                (20_205, "GroupLeaveReq"),
                (20_206, "GroupLeaveRes"),
                (20_207, "GroupDismissReq"),
                (20_208, "GroupDismissRes"),
                (20_209, "GroupListReq"),
                (20_210, "GroupListRes"),
                (20_211, "ChatHistoryReq"),
                (20_212, "ChatHistoryRes"),
                (20_301, "MailNotifyPush"),
                (9_000, "ErrorRes"),
            ]
        );
    }

    #[test]
    fn response_router_uses_message_type_and_sequence_without_consuming_pushes() {
        let mut router = ChatPacketRouter::default();
        let (seq, _) = router.encode_request(CHAT_PRIVATE_REQ, CHAT_PRIVATE_RES, &[]);
        let push = decode_chat_binary_frame(&encode_chat_packet(CHAT_PUSH, seq, &[]).unwrap(), 64)
            .unwrap();
        assert!(matches!(router.route(push), ChatInbound::ChatPush(_)));

        let response =
            decode_chat_binary_frame(&encode_chat_packet(CHAT_PRIVATE_RES, seq, &[]).unwrap(), 64)
                .unwrap();
        assert!(matches!(router.route(response), ChatInbound::Response(_)));
    }

    #[test]
    fn reconnect_delay_is_bounded_and_ticket_errors_request_refresh() {
        let policy = ChatReconnectPolicy::default();
        let delay = policy.delay_for(99, 7);
        assert!(delay <= policy.max_delay);
        assert!(delay > Duration::ZERO);
        assert!(ticket_error_requires_refresh("ticket_expired"));
        assert!(!ticket_error_requires_refresh("PLAYER_BLOCKED"));
        assert_eq!(
            classify_chat_transport_failure("TLS certificate verification failed"),
            "CHAT_TLS_FAILED"
        );
        assert_eq!(
            classify_chat_transport_failure("connection reset"),
            "CHAT_TRANSPORT_DISCONNECTED"
        );
    }

    #[test]
    fn chat_plugin_exposes_unavailable_and_ticket_required_states_without_runtime() {
        let mut app = chat_test_app(MyServerSession {
            chat_endpoint_error: Some("invalid descriptor detail".to_string()),
            chat_endpoint_generation: 4,
            ..Default::default()
        });
        app.update();
        let state = app.world().resource::<ChatClientState>();
        assert_eq!(state.status, ChatClientStatus::Unavailable);
        assert_eq!(
            state.last_error_code.as_deref(),
            Some("CHAT_ENDPOINT_UNAVAILABLE")
        );
        assert!(!app.world().resource::<ChatRuntimeOwner>().is_active());

        {
            let mut session = app.world_mut().resource_mut::<MyServerSession>();
            session.chat_endpoint = Some(chat_endpoint());
            session.chat_endpoint_error = None;
            session.chat_endpoint_generation = 5;
        }
        app.update();
        let state = app.world().resource::<ChatClientState>();
        assert_eq!(state.status, ChatClientStatus::Disconnected);
        assert_eq!(
            state.last_error_code.as_deref(),
            Some("CHAT_CHARACTER_TICKET_REQUIRED")
        );
        assert!(!app.world().resource::<ChatRuntimeOwner>().is_active());
    }

    #[test]
    fn chat_plugin_pauses_without_exposing_ticket_in_debug_state() {
        let mut app = chat_test_app(MyServerSession {
            chat_endpoint: Some(chat_endpoint()),
            chat_endpoint_generation: 7,
            ticket: Some("character-bound-ticket-secret".to_string()),
            ..Default::default()
        });
        app.world_mut()
            .write_message(ChatCommand::SetForeground { foreground: false });
        app.update();

        let state = app.world().resource::<ChatClientState>();
        assert_eq!(state.status, ChatClientStatus::Paused);
        assert!(!state.foreground);
        assert!(state.endpoint_fingerprint.is_some());
        assert!(!format!("{state:?}").contains("character-bound-ticket-secret"));
        assert!(!app.world().resource::<ChatRuntimeOwner>().is_active());
    }

    #[test]
    fn closed_worker_event_channel_becomes_a_stable_failed_state() {
        let mut app = chat_test_app(MyServerSession {
            chat_endpoint: Some(chat_endpoint()),
            chat_endpoint_generation: 9,
            ticket: Some("ticket".to_string()),
            ..Default::default()
        });
        let (command_tx, _command_rx) = mpsc::channel(1);
        let (event_tx, event_rx) = std_mpsc::channel();
        drop(event_tx);
        app.world_mut().resource_mut::<ChatRuntimeOwner>().active = Some(ActiveChatRuntime {
            runtime: ChatRuntime {
                command_tx,
                event_rx: Mutex::new(event_rx),
            },
            generation: 9,
            authenticated: false,
            router: ChatPacketRouter::default(),
            pending_deadlines: HashMap::new(),
            preauth_queue: VecDeque::new(),
            refresh_requested: false,
        });

        app.update();
        let state = app.world().resource::<ChatClientState>();
        assert_eq!(state.status, ChatClientStatus::Failed);
        assert_eq!(
            state.last_error_code.as_deref(),
            Some("CHAT_RUNTIME_EVENT_CHANNEL_CLOSED")
        );
        assert!(!app.world().resource::<ChatRuntimeOwner>().is_active());
    }

    #[test]
    fn expired_ticket_refresh_is_deduplicated_and_failure_stops_chat_runtime() {
        let mut app = chat_test_app(MyServerSession {
            chat_endpoint: Some(chat_endpoint()),
            chat_endpoint_generation: 10,
            ticket: Some("ticket".to_string()),
            ..Default::default()
        });
        let (command_tx, _command_rx) = mpsc::channel(1);
        let (event_tx, event_rx) = std_mpsc::channel();
        app.world_mut().resource_mut::<ChatRuntimeOwner>().active = Some(ActiveChatRuntime {
            runtime: ChatRuntime {
                command_tx,
                event_rx: Mutex::new(event_rx),
            },
            generation: 10,
            authenticated: false,
            router: ChatPacketRouter::default(),
            pending_deadlines: HashMap::new(),
            preauth_queue: VecDeque::new(),
            refresh_requested: false,
        });

        event_tx
            .send(ChatRuntimeEvent::TicketRefreshRequired)
            .unwrap();
        event_tx
            .send(ChatRuntimeEvent::TicketRefreshRequired)
            .unwrap();
        app.update();

        assert_eq!(
            read_myserver_commands(&app)
                .iter()
                .filter(|command| matches!(
                    command,
                    MyServerCommand::RefreshTicket {
                        reconnect_game: false
                    }
                ))
                .count(),
            1
        );
        assert!(
            app.world()
                .resource::<ChatClientState>()
                .ticket_refresh_in_flight
        );

        app.world_mut()
            .write_message(MyServerEvent::TicketRefreshFailed {
                error: "ticket refresh failed".to_string(),
            });
        app.update();

        let state = app.world().resource::<ChatClientState>();
        assert_eq!(state.status, ChatClientStatus::Failed);
        assert_eq!(
            state.last_error_code.as_deref(),
            Some("CHAT_TICKET_REFRESH_FAILED")
        );
        assert!(!state.ticket_refresh_in_flight);
        assert!(!app.world().resource::<ChatRuntimeOwner>().is_active());
    }

    #[test]
    fn background_cancels_queued_requests_before_the_next_authentication() {
        let mut app = chat_test_app(MyServerSession {
            chat_endpoint: Some(chat_endpoint()),
            chat_endpoint_generation: 12,
            ticket: Some("ticket".to_string()),
            ..Default::default()
        });
        let (command_tx, mut command_rx) = mpsc::channel(4);
        let (event_tx, event_rx) = std_mpsc::channel();
        app.world_mut().resource_mut::<ChatRuntimeOwner>().active = Some(ActiveChatRuntime {
            runtime: ChatRuntime {
                command_tx,
                event_rx: Mutex::new(event_rx),
            },
            generation: 12,
            authenticated: false,
            router: ChatPacketRouter::default(),
            pending_deadlines: HashMap::new(),
            preauth_queue: VecDeque::from([ChatOutboundRequest {
                request_message_type: CHAT_PRIVATE_REQ,
                response_message_type: CHAT_PRIVATE_RES,
                body: vec![1],
            }]),
            refresh_requested: false,
        });

        app.world_mut()
            .write_message(ChatCommand::SetForeground { foreground: false });
        app.update();
        assert_eq!(
            app.world()
                .resource::<ChatRuntimeOwner>()
                .active
                .as_ref()
                .unwrap()
                .preauth_queue
                .len(),
            0
        );
        assert!(matches!(
            command_rx.try_recv(),
            Ok(ChatRuntimeCommand::SetForeground(false))
        ));

        app.world_mut()
            .write_message(ChatCommand::SetForeground { foreground: true });
        app.update();
        let mut auth_body = Vec::new();
        chat_pb::ChatAuthRes {
            ok: true,
            error_code: String::new(),
        }
        .encode(&mut auth_body)
        .unwrap();
        event_tx
            .send(ChatRuntimeEvent::Packet(
                decode_chat_binary_frame(
                    &encode_chat_packet(CHAT_AUTH_RES, 1, &auth_body).unwrap(),
                    DEFAULT_CHAT_MAX_BODY_LEN,
                )
                .unwrap(),
            ))
            .unwrap();
        app.update();

        assert!(matches!(
            command_rx.try_recv(),
            Ok(ChatRuntimeCommand::SetForeground(true))
        ));
        assert!(command_rx.try_recv().is_err());
    }

    #[test]
    fn app_exit_releases_the_controlled_runtime_owner() {
        let mut app = chat_test_app(MyServerSession {
            chat_endpoint: Some(chat_endpoint()),
            chat_endpoint_generation: 11,
            ticket: Some("ticket".to_string()),
            ..Default::default()
        });
        let (command_tx, _command_rx) = mpsc::channel(1);
        let (_event_tx, event_rx) = std_mpsc::channel();
        app.world_mut().resource_mut::<ChatRuntimeOwner>().active = Some(ActiveChatRuntime {
            runtime: ChatRuntime {
                command_tx,
                event_rx: Mutex::new(event_rx),
            },
            generation: 11,
            authenticated: false,
            router: ChatPacketRouter::default(),
            pending_deadlines: HashMap::new(),
            preauth_queue: VecDeque::new(),
            refresh_requested: false,
        });

        app.world_mut().write_message(AppExit::Success);
        app.update();
        assert!(!app.world().resource::<ChatRuntimeOwner>().is_active());
    }

    #[test]
    fn chat_auth_response_gates_requests_until_ready() {
        let mut app = chat_test_app(MyServerSession {
            chat_endpoint: Some(chat_endpoint()),
            chat_endpoint_generation: 13,
            ticket: Some("ticket".to_string()),
            ..Default::default()
        });
        let (command_tx, mut command_rx) = mpsc::channel(4);
        let (event_tx, event_rx) = std_mpsc::channel();
        app.world_mut().resource_mut::<ChatRuntimeOwner>().active = Some(ActiveChatRuntime {
            runtime: ChatRuntime {
                command_tx,
                event_rx: Mutex::new(event_rx),
            },
            generation: 13,
            authenticated: false,
            router: ChatPacketRouter::default(),
            pending_deadlines: HashMap::new(),
            preauth_queue: VecDeque::new(),
            refresh_requested: false,
        });

        app.world_mut().write_message(ChatCommand::SendRequest {
            request_message_type: CHAT_PRIVATE_REQ,
            response_message_type: CHAT_PRIVATE_RES,
            body: vec![1],
        });
        app.update();
        assert!(command_rx.try_recv().is_err());
        assert_ne!(
            app.world().resource::<ChatClientState>().status,
            ChatClientStatus::Ready
        );

        let mut auth_body = Vec::new();
        chat_pb::ChatAuthRes {
            ok: true,
            error_code: String::new(),
        }
        .encode(&mut auth_body)
        .unwrap();
        event_tx
            .send(ChatRuntimeEvent::Packet(
                decode_chat_binary_frame(
                    &encode_chat_packet(CHAT_AUTH_RES, 1, &auth_body).unwrap(),
                    DEFAULT_CHAT_MAX_BODY_LEN,
                )
                .unwrap(),
            ))
            .unwrap();
        app.update();
        assert_eq!(
            app.world().resource::<ChatClientState>().status,
            ChatClientStatus::Ready
        );
        assert!(matches!(
            command_rx.try_recv(),
            Ok(ChatRuntimeCommand::Send(_))
        ));
    }

    #[test]
    fn rejected_chat_authentication_enters_a_typed_failed_state() {
        let mut app = chat_test_app(MyServerSession {
            chat_endpoint: Some(chat_endpoint()),
            chat_endpoint_generation: 14,
            ticket: Some("ticket".to_string()),
            ..Default::default()
        });
        let (event_tx, _command_rx) = install_fake_runtime(&mut app, 14, false, 1);
        let mut auth_body = Vec::new();
        chat_pb::ChatAuthRes {
            ok: false,
            error_code: "PLAYER_BLOCKED".to_string(),
        }
        .encode(&mut auth_body)
        .unwrap();
        event_tx
            .send(ChatRuntimeEvent::Packet(packet(
                CHAT_AUTH_RES,
                1,
                &auth_body,
            )))
            .unwrap();

        app.update();

        let state = app.world().resource::<ChatClientState>();
        assert_eq!(state.status, ChatClientStatus::Failed);
        assert_eq!(
            state.last_error_code.as_deref(),
            Some("CHAT_AUTH_PLAYER_BLOCKED")
        );
        assert!(read_chat_events(&app).iter().any(|event| matches!(
            event,
            ChatEvent::AuthenticationFailed {
                generation: 14,
                error_code,
            } if error_code == "CHAT_AUTH_PLAYER_BLOCKED"
        )));
    }

    #[test]
    fn preauth_queue_is_bounded_before_the_runtime_is_ready() {
        let mut app = chat_test_app(MyServerSession {
            chat_endpoint: Some(chat_endpoint()),
            chat_endpoint_generation: 15,
            ticket: Some("ticket".to_string()),
            ..Default::default()
        });
        let (_event_tx, _command_rx) = install_fake_runtime(&mut app, 15, false, 1);
        for _ in 0..=MAX_PREAUTH_CHAT_REQUESTS {
            app.world_mut().write_message(ChatCommand::SendRequest {
                request_message_type: CHAT_PRIVATE_REQ,
                response_message_type: CHAT_PRIVATE_RES,
                body: vec![1],
            });
        }

        app.update();

        assert_eq!(
            app.world()
                .resource::<ChatRuntimeOwner>()
                .active
                .as_ref()
                .unwrap()
                .preauth_queue
                .len(),
            MAX_PREAUTH_CHAT_REQUESTS
        );
        assert_eq!(
            app.world()
                .resource::<ChatClientState>()
                .preauth_queue_depth,
            MAX_PREAUTH_CHAT_REQUESTS
        );
        assert_eq!(
            app.world()
                .resource::<ChatClientState>()
                .pending_request_count,
            0
        );
        assert_eq!(
            read_chat_events(&app)
                .iter()
                .filter(|event| matches!(
                    event,
                    ChatEvent::RequestFailed { error_code, .. }
                        if error_code == "CHAT_AUTH_PENDING_QUEUE_FULL"
                ))
                .count(),
            1
        );
    }

    #[test]
    fn send_queue_backpressure_and_response_timeout_are_typed_failures() {
        let mut app = chat_test_app(MyServerSession {
            chat_endpoint: Some(chat_endpoint()),
            chat_endpoint_generation: 16,
            ticket: Some("ticket".to_string()),
            ..Default::default()
        });
        let (_event_tx, mut command_rx) = install_fake_runtime(&mut app, 16, true, 1);
        for _ in 0..2 {
            app.world_mut().write_message(ChatCommand::SendRequest {
                request_message_type: CHAT_PRIVATE_REQ,
                response_message_type: CHAT_PRIVATE_RES,
                body: vec![1],
            });
        }

        app.update();
        assert!(matches!(
            command_rx.try_recv(),
            Ok(ChatRuntimeCommand::Send(_))
        ));
        assert!(read_chat_events(&app).iter().any(|event| matches!(
            event,
            ChatEvent::RequestFailed { error_code, .. }
                if error_code == "CHAT_SEND_QUEUE_UNAVAILABLE"
        )));
        assert_eq!(
            app.world()
                .resource::<ChatClientState>()
                .pending_request_count,
            1
        );

        {
            let mut owner = app.world_mut().resource_mut::<ChatRuntimeOwner>();
            let active = owner.active.as_mut().unwrap();
            for deadline in active.pending_deadlines.values_mut() {
                *deadline = Instant::now() - Duration::from_secs(1);
            }
        }
        app.update();

        assert!(read_chat_events(&app).iter().any(|event| matches!(
            event,
            ChatEvent::RequestFailed { error_code, .. }
                if error_code == "CHAT_RESPONSE_TIMEOUT"
        )));
        assert_eq!(
            app.world()
                .resource::<ChatClientState>()
                .pending_request_count,
            0
        );
    }

    #[test]
    fn interleaved_response_and_pushes_remain_independent() {
        let mut app = chat_test_app(MyServerSession {
            chat_endpoint: Some(chat_endpoint()),
            chat_endpoint_generation: 17,
            ticket: Some("ticket".to_string()),
            ..Default::default()
        });
        let (event_tx, mut command_rx) = install_fake_runtime(&mut app, 17, true, 4);
        app.world_mut().write_message(ChatCommand::SendRequest {
            request_message_type: CHAT_PRIVATE_REQ,
            response_message_type: CHAT_PRIVATE_RES,
            body: vec![1],
        });
        app.update();
        let ChatRuntimeCommand::Send(request) = command_rx.try_recv().unwrap() else {
            panic!("expected a chat request packet");
        };
        let seq = packet(CHAT_PRIVATE_REQ, 1, &[]).header.seq;
        let request_seq = decode_chat_binary_frame(&request, DEFAULT_CHAT_MAX_BODY_LEN)
            .unwrap()
            .header
            .seq;
        assert_eq!(request_seq, seq);

        event_tx
            .send(ChatRuntimeEvent::Packet(packet(CHAT_PUSH, 0, &[])))
            .unwrap();
        event_tx
            .send(ChatRuntimeEvent::Packet(packet(MAIL_NOTIFY_PUSH, 0, &[])))
            .unwrap();
        event_tx
            .send(ChatRuntimeEvent::Packet(packet(
                CHAT_PRIVATE_RES,
                request_seq,
                &[],
            )))
            .unwrap();
        app.update();

        let events = read_chat_events(&app);
        assert!(events.iter().any(|event| matches!(
            event,
            ChatEvent::ChatPush {
                generation: 17,
                seq: 0
            }
        )));
        assert!(
            events
                .iter()
                .any(|event| matches!(event, ChatEvent::MailNotifyPush { generation: 17 }))
        );
        assert!(events.iter().any(|event| matches!(
            event,
            ChatEvent::Response {
                generation: 17,
                message_type: CHAT_PRIVATE_RES,
                seq,
            } if *seq == request_seq
        )));
    }

    #[test]
    fn logout_invalidates_the_runtime_before_old_generation_events_are_drained() {
        let mut app = chat_test_app(MyServerSession {
            chat_endpoint: Some(chat_endpoint()),
            chat_endpoint_generation: 18,
            ticket: Some("ticket".to_string()),
            ..Default::default()
        });
        let (event_tx, mut command_rx) = install_fake_runtime(&mut app, 18, true, 1);
        event_tx
            .send(ChatRuntimeEvent::Packet(packet(CHAT_PUSH, 0, &[])))
            .unwrap();
        app.world_mut().resource_mut::<MyServerSession>().logout();

        app.update();

        assert!(matches!(
            command_rx.try_recv(),
            Ok(ChatRuntimeCommand::Disconnect)
        ));
        assert!(!app.world().resource::<ChatRuntimeOwner>().is_active());
        assert_eq!(
            app.world().resource::<ChatClientState>().status,
            ChatClientStatus::Unavailable
        );
        assert!(
            !read_chat_events(&app)
                .iter()
                .any(|event| matches!(event, ChatEvent::ChatPush { .. }))
        );
    }
}
