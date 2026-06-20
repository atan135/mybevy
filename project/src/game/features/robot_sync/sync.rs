use bevy::prelude::*;

use crate::game::{
    authority::{AuthorityCommand, AuthorityEndpoint, AuthorityRole, AuthoritySession},
    myserver::{MyServerCommand, MyServerEvent},
};

use super::{
    config::{RobotSyncAuthorityMode, RobotSyncConfig},
    state::RobotSyncSceneState,
};

#[derive(Clone, Debug, Default, Resource, PartialEq, Eq)]
pub(in crate::game::features::robot_sync) struct RobotSyncReplayState {
    pub(in crate::game::features::robot_sync) buffered_frame_count: usize,
    pub(in crate::game::features::robot_sync) last_frame_id: Option<u32>,
}

#[derive(Clone, Debug, Default, Resource, PartialEq, Eq)]
pub(in crate::game::features::robot_sync) struct RobotSyncMyServerJoinState {
    pub(in crate::game::features::robot_sync) authority_started: bool,
    pub(in crate::game::features::robot_sync) login_sent: bool,
    pub(in crate::game::features::robot_sync) join_sent: bool,
    pub(in crate::game::features::robot_sync) ready_sent: bool,
    pub(in crate::game::features::robot_sync) start_sent: bool,
    pub(in crate::game::features::robot_sync) started: bool,
}

impl RobotSyncReplayState {
    pub(in crate::game::features::robot_sync) fn reset(&mut self) {
        *self = Self::default();
    }
}

impl RobotSyncMyServerJoinState {
    pub(in crate::game::features::robot_sync) fn reset(&mut self) {
        *self = Self::default();
    }
}

pub(in crate::game::features::robot_sync) fn reset_robot_sync_replay(
    state: &mut RobotSyncReplayState,
) {
    state.reset();
}

pub(in crate::game::features::robot_sync) fn start_robot_sync_authority(
    config: &RobotSyncConfig,
    session: &AuthoritySession,
    state: &mut RobotSyncMyServerJoinState,
    authority_commands: &mut MessageWriter<AuthorityCommand>,
    myserver_commands: &mut MessageWriter<MyServerCommand>,
) {
    if state.authority_started {
        debug!("robot sync authority startup already handled");
        return;
    }

    state.authority_started = true;

    match config.authority_mode {
        RobotSyncAuthorityMode::Off => {
            info!(
                player_id = %config.local_player_id,
                "robot sync authority startup disabled"
            );
        }
        RobotSyncAuthorityMode::Local => {
            leave_existing_authority_if_needed(session, authority_commands);
            info!(
                player_id = %config.local_player_id,
                "robot sync starting local authority"
            );
            authority_commands.write(AuthorityCommand::HostLocal {
                player_id: config.local_player_id.clone(),
            });
        }
        RobotSyncAuthorityMode::LanHost => {
            leave_existing_authority_if_needed(session, authority_commands);
            info!(
                player_id = %config.local_player_id,
                bind_addr = %config.lan_bind_addr,
                transport = ?config.transport,
                "robot sync starting LAN authority"
            );
            authority_commands.write(AuthorityCommand::HostLan {
                player_id: config.local_player_id.clone(),
                bind_addr: config.lan_bind_addr.clone(),
                transport: config.transport,
            });
        }
        RobotSyncAuthorityMode::LanClient => {
            leave_existing_authority_if_needed(session, authority_commands);
            info!(
                player_id = %config.local_player_id,
                host = %config.remote_host,
                port = config.remote_port,
                transport = ?config.transport,
                "robot sync joining LAN authority"
            );
            authority_commands.write(AuthorityCommand::Join {
                player_id: config.local_player_id.clone(),
                endpoint: AuthorityEndpoint::Remote {
                    host: config.remote_host.clone(),
                    port: config.remote_port,
                    transport: config.transport,
                },
            });
        }
        RobotSyncAuthorityMode::MyServer => {
            leave_existing_authority_if_needed(session, authority_commands);
            info!(
                player_id = %config.local_player_id,
                guest_id = config.myserver_guest_id.as_deref().unwrap_or_default(),
                room_id = %config.myserver_room_id,
                policy_id = %config.myserver_policy_id,
                transport = ?config.transport,
                "robot sync starting MyServer authority"
            );
            authority_commands.write(AuthorityCommand::Join {
                player_id: config.local_player_id.clone(),
                endpoint: AuthorityEndpoint::MyServer {
                    host: None,
                    port: None,
                    transport: config.transport,
                },
            });
            myserver_commands.write(MyServerCommand::GuestLogin {
                guest_id: config.myserver_guest_id.clone(),
                connect_game: true,
            });
            state.login_sent = true;
        }
    }
}

pub(in crate::game::features::robot_sync) fn cleanup_robot_sync_authority(
    config: &RobotSyncConfig,
    state: &mut RobotSyncMyServerJoinState,
    authority_commands: &mut MessageWriter<AuthorityCommand>,
    myserver_commands: &mut MessageWriter<MyServerCommand>,
) {
    let should_disconnect_myserver =
        matches!(config.authority_mode, RobotSyncAuthorityMode::MyServer)
            || state.login_sent
            || state.join_sent
            || state.ready_sent
            || state.start_sent;

    state.reset();
    info!(
        player_id = %config.local_player_id,
        guest_id = config.myserver_guest_id.as_deref().unwrap_or_default(),
        room_id = %config.myserver_room_id,
        policy_id = %config.myserver_policy_id,
        disconnect_myserver = should_disconnect_myserver,
        "robot sync authority cleanup"
    );
    authority_commands.write(AuthorityCommand::Leave);

    if should_disconnect_myserver {
        myserver_commands.write(MyServerCommand::Disconnect);
    }
}

pub(in crate::game::features::robot_sync) fn follow_robot_sync_myserver_events(
    config: Res<RobotSyncConfig>,
    scene_state: Res<RobotSyncSceneState>,
    mut state: ResMut<RobotSyncMyServerJoinState>,
    mut events: MessageReader<MyServerEvent>,
    mut commands: MessageWriter<MyServerCommand>,
) {
    if !scene_state.active || !matches!(config.authority_mode, RobotSyncAuthorityMode::MyServer) {
        return;
    }

    for event in events.read() {
        handle_robot_sync_myserver_event(&config, &mut state, event, &mut commands);
    }
}

fn handle_robot_sync_myserver_event(
    config: &RobotSyncConfig,
    state: &mut RobotSyncMyServerJoinState,
    event: &MyServerEvent,
    commands: &mut MessageWriter<MyServerCommand>,
) {
    match event {
        MyServerEvent::Authenticated { player_id } if !state.join_sent => {
            state.join_sent = true;
            info!(
                player_id = %player_id,
                guest_id = config.myserver_guest_id.as_deref().unwrap_or_default(),
                room_id = %config.myserver_room_id,
                policy_id = %config.myserver_policy_id,
                "robot sync joining MyServer room"
            );
            commands.write(MyServerCommand::JoinRoom {
                room_id: config.myserver_room_id.clone(),
                policy_id: config.myserver_policy_id.clone(),
            });
        }
        MyServerEvent::RoomJoined(response)
            if response.ok && state.join_sent && !state.ready_sent =>
        {
            state.ready_sent = true;
            info!(
                room_id = %response.room_id,
                policy_id = %config.myserver_policy_id,
                guest_id = config.myserver_guest_id.as_deref().unwrap_or_default(),
                "robot sync MyServer room joined"
            );
            commands.write(MyServerCommand::SetReady { ready: true });
        }
        MyServerEvent::RoomJoined(response) if !response.ok => {
            warn!(
                room_id = %response.room_id,
                policy_id = %config.myserver_policy_id,
                player_id = %config.local_player_id,
                guest_id = config.myserver_guest_id.as_deref().unwrap_or_default(),
                error_code = %response.error_code,
                "robot sync MyServer room join rejected"
            );
        }
        MyServerEvent::ReadyChanged(response)
            if response.ok && state.ready_sent && !state.start_sent =>
        {
            state.start_sent = true;
            info!(
                room_id = %response.room_id,
                policy_id = %config.myserver_policy_id,
                ready = response.ready,
                guest_id = config.myserver_guest_id.as_deref().unwrap_or_default(),
                "robot sync MyServer ready changed"
            );
            commands.write(MyServerCommand::StartRoom);
        }
        MyServerEvent::ReadyChanged(response) if !response.ok => {
            warn!(
                room_id = %response.room_id,
                policy_id = %config.myserver_policy_id,
                player_id = %config.local_player_id,
                guest_id = config.myserver_guest_id.as_deref().unwrap_or_default(),
                error_code = %response.error_code,
                "robot sync MyServer ready rejected"
            );
        }
        MyServerEvent::RoomStarted(response) if response.ok => {
            state.started = true;
            info!(
                room_id = %response.room_id,
                policy_id = %config.myserver_policy_id,
                guest_id = config.myserver_guest_id.as_deref().unwrap_or_default(),
                "robot sync MyServer room started"
            );
        }
        MyServerEvent::RoomStarted(response) => {
            warn!(
                room_id = %response.room_id,
                policy_id = %config.myserver_policy_id,
                player_id = %config.local_player_id,
                guest_id = config.myserver_guest_id.as_deref().unwrap_or_default(),
                error_code = %response.error_code,
                "robot sync MyServer room start rejected"
            );
        }
        MyServerEvent::ConnectionFailed {
            transport,
            remote_addr,
            error,
        } => {
            error!(
                room_id = %config.myserver_room_id,
                policy_id = %config.myserver_policy_id,
                player_id = %config.local_player_id,
                guest_id = config.myserver_guest_id.as_deref().unwrap_or_default(),
                ?transport,
                remote_addr = %remote_addr,
                reason = %error,
                "robot sync MyServer connection failed"
            );
        }
        MyServerEvent::Disconnected { reason } => {
            warn!(
                room_id = %config.myserver_room_id,
                policy_id = %config.myserver_policy_id,
                player_id = %config.local_player_id,
                guest_id = config.myserver_guest_id.as_deref().unwrap_or_default(),
                reason = reason.as_deref().unwrap_or_default(),
                "robot sync MyServer disconnected"
            );
        }
        MyServerEvent::AuthFailed { error_code } => {
            error!(
                room_id = %config.myserver_room_id,
                policy_id = %config.myserver_policy_id,
                player_id = %config.local_player_id,
                guest_id = config.myserver_guest_id.as_deref().unwrap_or_default(),
                error_code = %error_code,
                "robot sync MyServer auth failed"
            );
        }
        MyServerEvent::Error {
            seq,
            error_code,
            message,
        } => {
            warn!(
                room_id = %config.myserver_room_id,
                policy_id = %config.myserver_policy_id,
                player_id = %config.local_player_id,
                guest_id = config.myserver_guest_id.as_deref().unwrap_or_default(),
                seq = *seq,
                error_code = %error_code,
                reason = %message,
                "robot sync MyServer error"
            );
        }
        MyServerEvent::ProtocolError { error } => {
            error!(
                room_id = %config.myserver_room_id,
                policy_id = %config.myserver_policy_id,
                player_id = %config.local_player_id,
                guest_id = config.myserver_guest_id.as_deref().unwrap_or_default(),
                reason = %error,
                "robot sync MyServer protocol error"
            );
        }
        MyServerEvent::RequestFailed {
            seq,
            message_type,
            error,
        } => {
            warn!(
                room_id = %config.myserver_room_id,
                policy_id = %config.myserver_policy_id,
                player_id = %config.local_player_id,
                guest_id = config.myserver_guest_id.as_deref().unwrap_or_default(),
                ?seq,
                ?message_type,
                reason = %error,
                "robot sync MyServer request failed"
            );
        }
        _ => {}
    }
}

fn leave_existing_authority_if_needed(
    session: &AuthoritySession,
    authority_commands: &mut MessageWriter<AuthorityCommand>,
) {
    if session.role.is_some_and(|role| role != AuthorityRole::None) {
        authority_commands.write(AuthorityCommand::Leave);
    }
}
