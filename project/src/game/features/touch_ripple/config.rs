use std::env;

use bevy::prelude::*;

use crate::{authority::AuthorityEndpoint, network::NetworkTransport};

const DEFAULT_TOUCH_PLAYER_ID: &str = "touch-local";
const DEFAULT_UI_TOUCH_ROOM_ID: &str = "ui-touch-room";
const DEFAULT_TOUCH_INPUT_DELAY_FRAMES: u32 = 2;

#[derive(Clone, Debug, Resource)]
pub(super) struct TouchSyncConfig {
    pub(super) auto_start_local_authority: bool,
    pub(super) local_player_id: String,
    pub(super) myserver_room_id: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Resource)]
pub(in crate::game) enum TouchLaunchMode {
    #[default]
    Auto,
    SinglePlayer,
}

impl Default for TouchSyncConfig {
    fn default() -> Self {
        Self {
            auto_start_local_authority: env_bool("TOUCH_AUTO_LOCAL_AUTHORITY", true),
            local_player_id: env_string("TOUCH_PLAYER_ID", DEFAULT_TOUCH_PLAYER_ID),
            myserver_room_id: env_string("TOUCH_ROOM_ID", DEFAULT_UI_TOUCH_ROOM_ID),
        }
    }
}

pub(super) fn authority_endpoint_from_env() -> Option<AuthorityEndpoint> {
    let mode = env::var("TOUCH_AUTHORITY_MODE")
        .ok()
        .unwrap_or_default()
        .to_ascii_lowercase();
    match mode.as_str() {
        "lan-client" | "client" | "remote" => Some(AuthorityEndpoint::Remote {
            host: env_string("AUTHORITY_REMOTE_HOST", "127.0.0.1"),
            port: env_u16("AUTHORITY_REMOTE_PORT", 15000),
            transport: env_transport("AUTHORITY_TRANSPORT").unwrap_or(NetworkTransport::Tcp),
        }),
        "myserver" | "server" => Some(AuthorityEndpoint::MyServer {
            host: None,
            port: None,
            transport: env_transport("MYSERVER_TRANSPORT").unwrap_or(NetworkTransport::Tcp),
        }),
        _ => None,
    }
}

fn env_string(name: &str, default: &str) -> String {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default.to_string())
}

pub(super) fn env_bool(name: &str, default: bool) -> bool {
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

fn env_u32(name: &str, default: u32) -> u32 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(default)
}

pub(super) fn touch_input_delay_frames() -> u32 {
    env_u32("TOUCH_INPUT_DELAY_FRAMES", DEFAULT_TOUCH_INPUT_DELAY_FRAMES).max(1)
}

pub(super) fn env_transport(name: &str) -> Option<NetworkTransport> {
    match env::var(name).ok()?.trim().to_ascii_lowercase().as_str() {
        "tcp" => Some(NetworkTransport::Tcp),
        "kcp" => Some(NetworkTransport::Kcp),
        _ => None,
    }
}
