use std::env;

use bevy::prelude::*;

use crate::{
    framework::{network::NetworkTransport, scene::prelude::SceneId},
    game::scenes::LOCKSTEP_SIM_ARENA_SCENE_ID,
};

pub(in crate::game::features::lockstep_sim) const DEFAULT_LOCKSTEP_SIM_PLAYER_ID: &str =
    "lockstep-local";
pub(in crate::game::features::lockstep_sim) const LOCKSTEP_SIM_MYSERVER_POLICY_ID: &str =
    "lockstep_sim_demo";
const DEFAULT_LOCKSTEP_SIM_MYSERVER_ROOM_ID: &str = "lockstep-sim-room";

#[derive(Clone, Debug, Resource, PartialEq, Eq)]
pub(in crate::game) struct LockstepSimConfig {
    pub(in crate::game::features::lockstep_sim) scene_id: SceneId,
    pub(in crate::game::features::lockstep_sim) local_player_id: String,
    pub(in crate::game::features::lockstep_sim) authority_mode: LockstepSimAuthorityMode,
    pub(in crate::game::features::lockstep_sim) transport: NetworkTransport,
    pub(in crate::game::features::lockstep_sim) myserver_guest_id: Option<String>,
    pub(in crate::game::features::lockstep_sim) myserver_room_id: String,
    pub(in crate::game::features::lockstep_sim) myserver_policy_id: String,
    pub(in crate::game::features::lockstep_sim) debug_diagnostics: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::game::features::lockstep_sim) enum LockstepSimAuthorityMode {
    #[default]
    MyServer,
    Off,
}

impl Default for LockstepSimConfig {
    fn default() -> Self {
        Self::from_env_reader(|key| env::var(key).ok())
    }
}

impl LockstepSimConfig {
    pub(in crate::game::features::lockstep_sim) fn from_env_reader(
        mut read: impl FnMut(&str) -> Option<String>,
    ) -> Self {
        Self {
            scene_id: SceneId::from(LOCKSTEP_SIM_ARENA_SCENE_ID),
            local_player_id: env_string(
                &mut read,
                &["LOCKSTEP_SIM_PLAYER_ID", "AUTHORITY_PLAYER_ID"],
                DEFAULT_LOCKSTEP_SIM_PLAYER_ID,
            ),
            authority_mode: env_authority_mode(&mut read, &["LOCKSTEP_SIM_AUTHORITY_MODE"]),
            transport: env_transport(&mut read, &["LOCKSTEP_SIM_TRANSPORT", "MYSERVER_TRANSPORT"])
                .unwrap_or(NetworkTransport::Tcp),
            myserver_guest_id: env_optional_string(
                &mut read,
                &["LOCKSTEP_SIM_MYSERVER_GUEST_ID", "MYSERVER_GUEST_ID"],
            ),
            myserver_room_id: env_string(
                &mut read,
                &["LOCKSTEP_SIM_MYSERVER_ROOM"],
                DEFAULT_LOCKSTEP_SIM_MYSERVER_ROOM_ID,
            ),
            myserver_policy_id: env_string(
                &mut read,
                &["LOCKSTEP_SIM_MYSERVER_POLICY"],
                LOCKSTEP_SIM_MYSERVER_POLICY_ID,
            ),
            debug_diagnostics: env_bool(&mut read, &["LOCKSTEP_SIM_DEBUG_DIAGNOSTICS"]),
        }
    }

    pub(in crate::game::features::lockstep_sim) fn is_lockstep_sim_scene(
        &self,
        scene_id: &SceneId,
    ) -> bool {
        self.scene_id.as_str() == scene_id.as_str()
    }
}

fn env_authority_mode(
    read: &mut impl FnMut(&str) -> Option<String>,
    names: &[&str],
) -> LockstepSimAuthorityMode {
    match env_first(read, names)
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "off" | "none" | "disabled" => LockstepSimAuthorityMode::Off,
        "myserver" | "server" | "" => LockstepSimAuthorityMode::MyServer,
        other => {
            warn!(mode = %other, "unknown lockstep sim authority mode; using myserver");
            LockstepSimAuthorityMode::MyServer
        }
    }
}

fn env_string(
    read: &mut impl FnMut(&str) -> Option<String>,
    names: &[&str],
    default: &str,
) -> String {
    env_first(read, names).unwrap_or_else(|| default.to_string())
}

fn env_optional_string(
    read: &mut impl FnMut(&str) -> Option<String>,
    names: &[&str],
) -> Option<String> {
    env_first(read, names)
}

fn env_transport(
    read: &mut impl FnMut(&str) -> Option<String>,
    names: &[&str],
) -> Option<NetworkTransport> {
    match env_first(read, names)?.trim().to_ascii_lowercase().as_str() {
        "tcp" => Some(NetworkTransport::Tcp),
        "kcp" => Some(NetworkTransport::Kcp),
        _ => None,
    }
}

fn env_bool(read: &mut impl FnMut(&str) -> Option<String>, names: &[&str]) -> bool {
    matches!(
        env_first(read, names)
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn env_first(read: &mut impl FnMut(&str) -> Option<String>, names: &[&str]) -> Option<String> {
    names
        .iter()
        .find_map(|name| read(name))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_reader<'a>(values: &'a [(&'a str, &'a str)]) -> impl FnMut(&str) -> Option<String> + 'a {
        |key| {
            values
                .iter()
                .find_map(|(name, value)| (*name == key).then_some((*value).to_string()))
        }
    }

    #[test]
    fn lockstep_sim_config_defaults_to_demo_policy() {
        let config = LockstepSimConfig::from_env_reader(env_reader(&[]));

        assert_eq!(config.myserver_policy_id, LOCKSTEP_SIM_MYSERVER_POLICY_ID);
        assert_eq!(
            config.myserver_room_id,
            DEFAULT_LOCKSTEP_SIM_MYSERVER_ROOM_ID
        );
        assert_eq!(config.authority_mode, LockstepSimAuthorityMode::MyServer);
        assert!(!config.debug_diagnostics);
    }

    #[test]
    fn lockstep_sim_config_reads_debug_diagnostics_switch() {
        let config = LockstepSimConfig::from_env_reader(env_reader(&[(
            "LOCKSTEP_SIM_DEBUG_DIAGNOSTICS",
            "true",
        )]));

        assert!(config.debug_diagnostics);
    }

    #[test]
    fn robot_sync_catalog_entry_stays_separate_from_lockstep_sim() {
        let catalog = include_str!("../../../../assets/game/scenes.csv");
        let robot_sync_row = catalog
            .lines()
            .find(|line| line.starts_with("arena.robot_sync,"))
            .unwrap();

        assert!(robot_sync_row.contains(",200,"));
        assert!(robot_sync_row.contains("scenes/robot_sync_arena/scene.ron"));
        assert!(!robot_sync_row.contains(LOCKSTEP_SIM_MYSERVER_POLICY_ID));
    }
}
