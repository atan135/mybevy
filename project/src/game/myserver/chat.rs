#![allow(dead_code)]

//! Chat WebSocket transport. This is deliberately separate from the game TCP/KCP session:
//! chat has its own authenticated connection, pending-response map, and reconnect lifecycle.
//! The public API is intentionally not bound to UI in this migration stage.

use std::{
    collections::HashMap,
    sync::{Mutex, mpsc as std_mpsc},
    thread,
    time::Duration,
};

use futures_util::{SinkExt, StreamExt};
use prost::Message;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message as WebSocketMessage};

use super::{
    protocol::{HEADER_LEN, Packet, PacketCodec, chat_pb, encode_raw_packet_type, parse_header},
    types::{ClientServiceEndpoint, ClientServices, MyServerCommand},
};

pub const CHAT_AUTH_REQ: u16 = 20_001;
pub const CHAT_AUTH_RES: u16 = 20_002;
pub const CHAT_PUSH: u16 = 20_105;
pub const MAIL_NOTIFY_PUSH: u16 = 20_301;
pub const DEFAULT_CHAT_MAX_BODY_LEN: usize = 64 * 1024;

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
        let authority = if host.contains(':') && !host.starts_with('[') {
            format!("[{host}]")
        } else {
            host.to_string()
        };
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
}

/// A WebSocket logical binary message must contain exactly one existing packet.
pub fn decode_chat_binary_frame(frame: &[u8], max_body_len: usize) -> Result<Packet, String> {
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
    use super::*;

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
    }

    #[test]
    fn response_router_uses_message_type_and_sequence_without_consuming_pushes() {
        let mut router = ChatPacketRouter::default();
        let (seq, _) = router.encode_request(20_101, 20_102, &[]);
        let push = decode_chat_binary_frame(&encode_chat_packet(CHAT_PUSH, seq, &[]).unwrap(), 64)
            .unwrap();
        assert!(matches!(router.route(push), ChatInbound::ChatPush(_)));

        let response =
            decode_chat_binary_frame(&encode_chat_packet(20_102, seq, &[]).unwrap(), 64).unwrap();
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
    }
}
