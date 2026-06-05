use std::collections::HashMap;

use bevy::prelude::{Message, Resource};
use serde::{Deserialize, Serialize};

use crate::network::{ConnectionId, ListenerId, NetworkTransport};

pub const DEFAULT_AUTHORITY_HOST: &str = "127.0.0.1";
pub const DEFAULT_AUTHORITY_PORT: u16 = 15000;
pub const DEFAULT_AUTHORITY_FPS: u16 = 20;
pub const AUTHORITY_PROTOCOL_VERSION: u16 = 1;
pub const AUTHORITY_PACKET_MAX_BODY_LEN: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthorityRole {
    None,
    Host,
    Client,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthorityEndpoint {
    LocalLoopback,
    Remote {
        host: String,
        port: u16,
        transport: NetworkTransport,
    },
    MyServer {
        host: Option<String>,
        port: Option<u16>,
        transport: NetworkTransport,
    },
}

impl AuthorityEndpoint {
    pub fn remote_addr(&self) -> Option<String> {
        match self {
            Self::Remote { host, port, .. } => Some(format!("{host}:{port}")),
            Self::MyServer {
                host: Some(host),
                port: Some(port),
                ..
            } => Some(format!("{host}:{port}")),
            _ => None,
        }
    }

    pub fn transport(&self) -> Option<NetworkTransport> {
        match self {
            Self::Remote { transport, .. } | Self::MyServer { transport, .. } => Some(*transport),
            Self::LocalLoopback => None,
        }
    }
}

#[derive(Clone, Debug, Message)]
pub enum AuthorityCommand {
    HostLocal {
        player_id: String,
    },
    HostLan {
        player_id: String,
        bind_addr: String,
        transport: NetworkTransport,
    },
    Join {
        player_id: String,
        endpoint: AuthorityEndpoint,
    },
    SwitchAuthority {
        endpoint: AuthorityEndpoint,
        migration: AuthorityMigration,
    },
    Leave,
    SendInput {
        frame_id: u32,
        action: String,
        payload_json: String,
    },
    Tick,
}

#[derive(Clone, Debug, Message)]
pub enum AuthorityEvent {
    Hosting {
        listener_id: Option<ListenerId>,
        endpoint: AuthorityEndpoint,
    },
    HostFailed {
        error: String,
    },
    Connecting {
        endpoint: AuthorityEndpoint,
    },
    Connected {
        endpoint: AuthorityEndpoint,
        player_id: String,
    },
    ConnectionFailed {
        endpoint: AuthorityEndpoint,
        error: String,
    },
    PeerJoined {
        player_id: String,
        connection_id: Option<ConnectionId>,
    },
    PeerLeft {
        player_id: String,
    },
    InputAccepted {
        frame_id: u32,
    },
    FrameApplied {
        frame: AuthorityFrame,
    },
    Snapshot {
        snapshot: AuthoritySnapshot,
    },
    MigrationStarted {
        migration: AuthorityMigration,
    },
    MigrationCompleted {
        authority_epoch: u64,
    },
    Disconnected {
        reason: Option<String>,
    },
    ProtocolError {
        error: String,
    },
}

#[derive(Clone, Debug, Default, Resource)]
pub struct AuthoritySession {
    pub role: Option<AuthorityRole>,
    pub local_player_id: Option<String>,
    pub authority_player_id: Option<String>,
    pub authority_epoch: u64,
    pub endpoint: Option<AuthorityEndpoint>,
    pub listener_id: Option<ListenerId>,
    pub server_connection_id: Option<ConnectionId>,
    pub local_loopback: bool,
    pub peers: HashMap<String, AuthorityPeer>,
    pub connection_players: HashMap<ConnectionId, String>,
    pub frame_id: u32,
    pub fps: u16,
    pub pending_inputs: HashMap<u32, Vec<PlayerInput>>,
    pub packet_codecs: HashMap<ConnectionId, AuthorityPacketCodec>,
    pub local_client_codec: AuthorityPacketCodec,
    pub host_codec: AuthorityPacketCodec,
}

impl AuthoritySession {
    pub fn reset(&mut self) {
        self.role = Some(AuthorityRole::None);
        self.local_player_id = None;
        self.authority_player_id = None;
        self.endpoint = None;
        self.listener_id = None;
        self.server_connection_id = None;
        self.local_loopback = false;
        self.peers.clear();
        self.connection_players.clear();
        self.pending_inputs.clear();
        self.packet_codecs.clear();
        self.local_client_codec.clear();
        self.host_codec.clear();
    }

    pub fn next_epoch(&mut self) -> u64 {
        self.authority_epoch = self.authority_epoch.saturating_add(1).max(1);
        self.authority_epoch
    }
}

#[derive(Clone, Debug)]
pub struct AuthorityPeer {
    pub player_id: String,
    pub connection_id: Option<ConnectionId>,
    pub connected: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayerInput {
    pub player_id: String,
    pub frame_id: u32,
    pub action: String,
    pub payload_json: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthorityFrame {
    pub authority_epoch: u64,
    pub frame_id: u32,
    pub fps: u16,
    pub inputs: Vec<PlayerInput>,
    pub snapshot: AuthoritySnapshot,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthoritySnapshot {
    pub authority_epoch: u64,
    pub frame_id: u32,
    pub authority_player_id: String,
    pub players: Vec<String>,
    pub game_state_json: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthorityMigration {
    pub authority_epoch: u64,
    pub frozen_frame_id: u32,
    pub new_authority_player_id: String,
    pub snapshot: AuthoritySnapshot,
    pub pending_inputs: Vec<PlayerInput>,
    pub checksum: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AuthorityWireMessage {
    Hello {
        protocol_version: u16,
        player_id: String,
        authority_epoch: u64,
    },
    Welcome {
        protocol_version: u16,
        player_id: String,
        authority_epoch: u64,
        snapshot: AuthoritySnapshot,
    },
    PlayerJoined {
        player_id: String,
        snapshot: AuthoritySnapshot,
    },
    PlayerLeft {
        player_id: String,
        snapshot: AuthoritySnapshot,
    },
    Input(PlayerInput),
    InputAccepted {
        frame_id: u32,
    },
    Frame(AuthorityFrame),
    Snapshot(AuthoritySnapshot),
    MigrationStart(AuthorityMigration),
    MigrationComplete {
        authority_epoch: u64,
    },
    Error {
        message: String,
    },
}

#[derive(Clone, Debug)]
pub struct AuthorityPacketCodec {
    buffer: Vec<u8>,
    max_body_len: usize,
}

impl Default for AuthorityPacketCodec {
    fn default() -> Self {
        Self::new(AUTHORITY_PACKET_MAX_BODY_LEN)
    }
}

impl AuthorityPacketCodec {
    pub fn new(max_body_len: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(8 * 1024),
            max_body_len: max_body_len.max(1),
        }
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    pub fn push_bytes(&mut self, bytes: &[u8]) -> Result<Vec<AuthorityWireMessage>, String> {
        self.buffer.extend_from_slice(bytes);
        let mut messages = Vec::new();

        loop {
            if self.buffer.len() < 4 {
                break;
            }

            let body_len = u32::from_be_bytes([
                self.buffer[0],
                self.buffer[1],
                self.buffer[2],
                self.buffer[3],
            ]) as usize;
            if body_len > self.max_body_len {
                return Err(format!(
                    "authority packet body too large: {body_len} > {}",
                    self.max_body_len
                ));
            }

            let packet_len = 4 + body_len;
            if self.buffer.len() < packet_len {
                break;
            }

            let body = self.buffer[4..packet_len].to_vec();
            self.buffer.drain(..packet_len);
            let message = serde_json::from_slice::<AuthorityWireMessage>(&body)
                .map_err(|err| format!("failed to decode authority packet: {err}"))?;
            messages.push(message);
        }

        Ok(messages)
    }
}

pub fn encode_authority_message(message: &AuthorityWireMessage) -> Result<Vec<u8>, String> {
    let body = serde_json::to_vec(message)
        .map_err(|err| format!("failed to encode authority message: {err}"))?;
    let body_len = u32::try_from(body.len())
        .map_err(|_| format!("authority message too large: {}", body.len()))?;
    let mut packet = Vec::with_capacity(4 + body.len());
    packet.extend_from_slice(&body_len.to_be_bytes());
    packet.extend_from_slice(&body);
    Ok(packet)
}
