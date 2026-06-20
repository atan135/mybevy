use std::env;

use bevy::prelude::*;

use crate::{
    framework::{network::NetworkTransport, scene::prelude::SceneId},
    game::scenes::ROBOT_SYNC_ARENA_SCENE_ID,
};

pub(in crate::game::features::robot_sync) const DEFAULT_ROBOT_SYNC_PLAYER_ID: &str = "robot-local";
pub(in crate::game::features::robot_sync) const ROBOT_SYNC_MYSERVER_POLICY_ID: &str =
    "robot_sync_room";
const DEFAULT_ROBOT_SYNC_MYSERVER_ROOM_ID: &str = "robot-sync-room";

#[derive(Clone, Debug, Resource, PartialEq, Eq)]
pub(in crate::game::features::robot_sync) struct RobotSyncConfig {
    pub(in crate::game::features::robot_sync) scene_id: SceneId,
    pub(in crate::game::features::robot_sync) local_player_id: String,
    pub(in crate::game::features::robot_sync) authority_mode: RobotSyncAuthorityMode,
    pub(in crate::game::features::robot_sync) lan_bind_addr: String,
    pub(in crate::game::features::robot_sync) remote_host: String,
    pub(in crate::game::features::robot_sync) remote_port: u16,
    pub(in crate::game::features::robot_sync) transport: NetworkTransport,
    pub(in crate::game::features::robot_sync) myserver_guest_id: Option<String>,
    pub(in crate::game::features::robot_sync) myserver_room_id: String,
    pub(in crate::game::features::robot_sync) myserver_policy_id: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::game::features::robot_sync) enum RobotSyncAuthorityMode {
    #[default]
    Local,
    LanHost,
    LanClient,
    MyServer,
    Off,
}

impl Default for RobotSyncConfig {
    fn default() -> Self {
        let myserver_policy_id = env_string(
            &["AUTHORITY_MYSERVER_POLICY", "ROBOT_SYNC_MYSERVER_POLICY"],
            ROBOT_SYNC_MYSERVER_POLICY_ID,
        );
        if myserver_policy_id != ROBOT_SYNC_MYSERVER_POLICY_ID {
            warn!(
                policy_id = %myserver_policy_id,
                default_policy_id = ROBOT_SYNC_MYSERVER_POLICY_ID,
                "robot sync MyServer policy overridden"
            );
        }

        Self {
            scene_id: SceneId::from(ROBOT_SYNC_ARENA_SCENE_ID),
            local_player_id: env_string(
                &["AUTHORITY_PLAYER_ID", "ROBOT_SYNC_PLAYER_ID"],
                DEFAULT_ROBOT_SYNC_PLAYER_ID,
            ),
            authority_mode: env_authority_mode(&[
                "ROBOT_SYNC_AUTHORITY_MODE",
                "AUTHORITY_DEV_MODE",
                "AUTHORITY_MODE",
            ]),
            lan_bind_addr: env_string(
                &["ROBOT_SYNC_LAN_BIND_ADDR", "AUTHORITY_BIND_ADDR"],
                "127.0.0.1:15000",
            ),
            remote_host: env_string(
                &["ROBOT_SYNC_REMOTE_HOST", "AUTHORITY_REMOTE_HOST"],
                "127.0.0.1",
            ),
            remote_port: env_u16(&["ROBOT_SYNC_REMOTE_PORT", "AUTHORITY_REMOTE_PORT"], 15000),
            transport: env_transport(&[
                "ROBOT_SYNC_TRANSPORT",
                "AUTHORITY_TRANSPORT",
                "MYSERVER_TRANSPORT",
            ])
            .unwrap_or(NetworkTransport::Tcp),
            myserver_guest_id: env_optional_string(&[
                "AUTHORITY_MYSERVER_GUEST_ID",
                "ROBOT_SYNC_MYSERVER_GUEST_ID",
                "MYSERVER_GUEST_ID",
            ]),
            myserver_room_id: env_string(
                &["AUTHORITY_MYSERVER_ROOM", "ROBOT_SYNC_MYSERVER_ROOM"],
                DEFAULT_ROBOT_SYNC_MYSERVER_ROOM_ID,
            ),
            myserver_policy_id,
        }
    }
}

impl RobotSyncConfig {
    pub(in crate::game::features::robot_sync) fn is_robot_sync_scene(
        &self,
        scene_id: &SceneId,
    ) -> bool {
        self.scene_id.as_str() == scene_id.as_str()
    }
}

fn env_authority_mode(names: &[&str]) -> RobotSyncAuthorityMode {
    let value = env_first(names).unwrap_or_default();
    match value.trim().to_ascii_lowercase().as_str() {
        "off" | "none" | "disabled" => RobotSyncAuthorityMode::Off,
        "lan-host" | "host" => RobotSyncAuthorityMode::LanHost,
        "lan-client" | "client" | "remote" | "join" => RobotSyncAuthorityMode::LanClient,
        "myserver" | "server" => RobotSyncAuthorityMode::MyServer,
        "local" | "local-host" | "localhost" | "" => RobotSyncAuthorityMode::Local,
        other => {
            warn!(mode = %other, "unknown robot sync authority mode; using local");
            RobotSyncAuthorityMode::Local
        }
    }
}

fn env_string(names: &[&str], default: &str) -> String {
    env_first(names).unwrap_or_else(|| default.to_string())
}

fn env_optional_string(names: &[&str]) -> Option<String> {
    env_first(names)
}

fn env_u16(names: &[&str], default: u16) -> u16 {
    env_first(names)
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(default)
}

fn env_transport(names: &[&str]) -> Option<NetworkTransport> {
    match env_first(names)?.trim().to_ascii_lowercase().as_str() {
        "tcp" => Some(NetworkTransport::Tcp),
        "kcp" => Some(NetworkTransport::Kcp),
        _ => None,
    }
}

fn env_first(names: &[&str]) -> Option<String> {
    names
        .iter()
        .find_map(|name| env::var(name).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn robot_sync_config_defaults_to_robot_sync_room_policy() {
        let config = RobotSyncConfig::default();

        assert_eq!(config.myserver_policy_id, ROBOT_SYNC_MYSERVER_POLICY_ID);
    }
}
