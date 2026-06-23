use bevy::prelude::*;

use crate::game::authority::{AuthorityRole, AuthoritySession};

use std::collections::BTreeMap;

use super::{
    config::{RobotSyncAuthorityMode, RobotSyncConfig},
    coordinates::{
        ROBOT_SYNC_ROBOT_FOOT_WORLD_Y, robot_sync_axis_sync_units_from_fixed,
        robot_sync_axis_world_units_from_fixed,
    },
    state::RobotSyncSceneState,
    sync::{FixedPosition, RobotState, RobotSyncReplayState},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::game) struct RobotSyncHudSnapshot {
    pub(in crate::game) room: String,
    pub(in crate::game) player: String,
    pub(in crate::game) authority_status: String,
    pub(in crate::game) frame: String,
    pub(in crate::game) robot_count: usize,
    pub(in crate::game) local_position: String,
    pub(in crate::game) robot_positions: String,
}

pub(in crate::game) fn robot_sync_hud_snapshot(
    config: &RobotSyncConfig,
    scene_state: &RobotSyncSceneState,
    authority_session: &AuthoritySession,
    replay_state: &RobotSyncReplayState,
) -> RobotSyncHudSnapshot {
    let player = authority_session
        .local_player_id
        .as_deref()
        .unwrap_or(config.local_player_id.as_str())
        .to_string();
    let local_position = replay_state
        .robots
        .get(&player)
        .map(|robot| format_fixed_position(robot.position))
        .unwrap_or_else(|| "local: pending".to_string());

    RobotSyncHudSnapshot {
        room: robot_sync_room_label(config),
        player,
        authority_status: robot_sync_authority_status(
            config.authority_mode,
            scene_state.active,
            authority_session.role,
        ),
        frame: robot_sync_frame_label(replay_state.last_applied_frame, authority_session.frame_id),
        robot_count: replay_state.robots.len(),
        local_position,
        robot_positions: format_robot_positions(&replay_state.robots),
    }
}

pub(in crate::game) fn format_robot_sync_hud_status(snapshot: &RobotSyncHudSnapshot) -> String {
    format!(
        "room={} player={} authority={} frame={} robots={} {}\n{}",
        snapshot.room,
        snapshot.player,
        snapshot.authority_status,
        snapshot.frame,
        snapshot.robot_count,
        snapshot.local_position,
        snapshot.robot_positions
    )
}

fn robot_sync_room_label(config: &RobotSyncConfig) -> String {
    match config.authority_mode {
        RobotSyncAuthorityMode::MyServer => config.myserver_room_id.clone(),
        RobotSyncAuthorityMode::LanHost => format!("lan-host {}", config.lan_bind_addr),
        RobotSyncAuthorityMode::LanClient => {
            format!("lan-client {}:{}", config.remote_host, config.remote_port)
        }
        RobotSyncAuthorityMode::Local => "local".to_string(),
        RobotSyncAuthorityMode::Off => "off".to_string(),
    }
}

fn robot_sync_authority_status(
    mode: RobotSyncAuthorityMode,
    scene_active: bool,
    role: Option<AuthorityRole>,
) -> String {
    let scene = if scene_active { "active" } else { "inactive" };
    let role = match role {
        Some(AuthorityRole::Host) => "host",
        Some(AuthorityRole::Client) => "client",
        Some(AuthorityRole::None) => "none",
        None => "pending",
    };
    format!("{mode:?}/{scene}/{role}")
}

fn robot_sync_frame_label(last_applied_frame: Option<u32>, authority_frame: u32) -> String {
    match last_applied_frame {
        Some(frame) => frame.to_string(),
        None => format!("pending(authority={authority_frame})"),
    }
}

fn format_fixed_position(position: FixedPosition) -> String {
    format!(
        "local: fixed=({},{}) sync=({:.3},{:.3}) world3d=({:.3},{:.3},{:.3})",
        position.x,
        position.y,
        robot_sync_axis_sync_units_from_fixed(position.x),
        robot_sync_axis_sync_units_from_fixed(position.y),
        robot_sync_axis_world_units_from_fixed(position.x),
        f64::from(ROBOT_SYNC_ROBOT_FOOT_WORLD_Y),
        robot_sync_axis_world_units_from_fixed(position.y)
    )
}

fn format_robot_positions(robots: &BTreeMap<String, RobotState>) -> String {
    if robots.is_empty() {
        return "all: pending".to_string();
    }

    let positions = robots
        .iter()
        .map(|(player_id, robot)| {
            format!(
                "{}=fixed=({},{}) sync=({:.1},{:.1}) world3d=({:.2},{:.2},{:.2})",
                short_robot_player_label(player_id),
                robot.position.x,
                robot.position.y,
                robot_sync_axis_sync_units_from_fixed(robot.position.x),
                robot_sync_axis_sync_units_from_fixed(robot.position.y),
                robot_sync_axis_world_units_from_fixed(robot.position.x),
                f64::from(ROBOT_SYNC_ROBOT_FOOT_WORLD_Y),
                robot_sync_axis_world_units_from_fixed(robot.position.y)
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!("all: {positions}")
}

fn short_robot_player_label(player_id: &str) -> &str {
    player_id.strip_prefix("robot-player-").unwrap_or(player_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::{network::NetworkTransport, scene::prelude::SceneId};
    use crate::game::{
        authority::AuthoritySession,
        features::robot_sync::config::{ROBOT_SYNC_MYSERVER_POLICY_ID, RobotSyncInputMode},
        scenes::ROBOT_SYNC_ARENA_SCENE_ID,
    };

    #[test]
    fn hud_status_formats_local_position() {
        let snapshot = RobotSyncHudSnapshot {
            room: "robot-room".to_string(),
            player: "player-a".to_string(),
            authority_status: "MyServer/active/client".to_string(),
            frame: "42".to_string(),
            robot_count: 2,
            local_position:
                "local: fixed=(10240,-5000) sync=(10.240,-5.000) world3d=(1.024,0.050,-0.500)"
                    .to_string(),
            robot_positions:
                "all: a=fixed=(10240,-5000) sync=(10.2,-5.0) world3d=(1.02,0.05,-0.50) b=fixed=(0,0) sync=(0.0,0.0) world3d=(0.00,0.05,0.00)"
                    .to_string(),
        };

        assert_eq!(
            format_robot_sync_hud_status(&snapshot),
            "room=robot-room player=player-a authority=MyServer/active/client frame=42 robots=2 local: fixed=(10240,-5000) sync=(10.240,-5.000) world3d=(1.024,0.050,-0.500)\nall: a=fixed=(10240,-5000) sync=(10.2,-5.0) world3d=(1.02,0.05,-0.50) b=fixed=(0,0) sync=(0.0,0.0) world3d=(0.00,0.05,0.00)"
        );
    }

    #[test]
    fn hud_snapshot_formats_fixed_sync_and_world3d_coordinates() {
        let config = test_config();
        let mut scene_state = RobotSyncSceneState::default();
        scene_state.active = true;
        let mut authority_session = AuthoritySession::default();
        authority_session.role = Some(AuthorityRole::Client);
        authority_session.local_player_id = Some("robot-player-a".to_string());
        authority_session.frame_id = 41;
        let mut replay_state = RobotSyncReplayState::default();
        replay_state.last_applied_frame = Some(42);
        replay_state.robots.insert(
            "robot-player-a".to_string(),
            RobotState {
                player_id: "robot-player-a".to_string(),
                position: FixedPosition {
                    x: 10_240,
                    y: -5_000,
                },
                dir_x: 1000,
                dir_y: 0,
                speed: 10_000,
                last_input_seq: Some(1),
                last_frame: Some(42),
                spawn_index: 0,
                color_index: 0,
            },
        );
        replay_state.robots.insert(
            "robot-player-b".to_string(),
            RobotState {
                player_id: "robot-player-b".to_string(),
                position: FixedPosition { x: 0, y: 0 },
                dir_x: 0,
                dir_y: 0,
                speed: 0,
                last_input_seq: None,
                last_frame: Some(42),
                spawn_index: 1,
                color_index: 1,
            },
        );

        let snapshot =
            robot_sync_hud_snapshot(&config, &scene_state, &authority_session, &replay_state);

        assert_eq!(
            snapshot.local_position,
            "local: fixed=(10240,-5000) sync=(10.240,-5.000) world3d=(1.024,0.050,-0.500)"
        );
        assert_eq!(
            snapshot.robot_positions,
            "all: a=fixed=(10240,-5000) sync=(10.2,-5.0) world3d=(1.02,0.05,-0.50) b=fixed=(0,0) sync=(0.0,0.0) world3d=(0.00,0.05,0.00)"
        );
    }

    #[test]
    fn hud_snapshot_uses_pending_local_position_fallback() {
        let config = test_config();
        let mut scene_state = RobotSyncSceneState::default();
        scene_state.active = true;
        let mut authority_session = AuthoritySession::default();
        authority_session.role = Some(AuthorityRole::Client);
        authority_session.local_player_id = Some("player-a".to_string());
        authority_session.frame_id = 8;
        let replay_state = RobotSyncReplayState::default();

        let snapshot =
            robot_sync_hud_snapshot(&config, &scene_state, &authority_session, &replay_state);

        assert_eq!(snapshot.room, "robot-room");
        assert_eq!(snapshot.player, "player-a");
        assert_eq!(snapshot.authority_status, "MyServer/active/client");
        assert_eq!(snapshot.frame, "pending(authority=8)");
        assert_eq!(snapshot.robot_count, 0);
        assert_eq!(snapshot.local_position, "local: pending");
        assert_eq!(snapshot.robot_positions, "all: pending");
    }

    fn test_config() -> RobotSyncConfig {
        RobotSyncConfig {
            scene_id: SceneId::from(ROBOT_SYNC_ARENA_SCENE_ID),
            local_player_id: "player-a".to_string(),
            authority_mode: RobotSyncAuthorityMode::MyServer,
            lan_bind_addr: "127.0.0.1:15000".to_string(),
            remote_host: "127.0.0.1".to_string(),
            remote_port: 15000,
            transport: NetworkTransport::Tcp,
            myserver_guest_id: Some("guest-a".to_string()),
            myserver_room_id: "robot-room".to_string(),
            myserver_policy_id: ROBOT_SYNC_MYSERVER_POLICY_ID.to_string(),
            input_mode: RobotSyncInputMode::Bot,
            input_delay_frames: 2,
            bot_input_interval_frames: 1,
            bot_speed: 10_000,
            manual_speed: 10_000,
        }
    }
}
