use std::env;

use bevy::prelude::*;

use crate::game::{
    authority::{AuthorityCommand, AuthorityEndpoint, AuthorityRole, AuthoritySession},
    myserver::{MyServerCommand, MyServerEvent},
};

use super::{
    config::{LockstepSimAuthorityMode, LockstepSimConfig},
    snapshot::{
        LockstepSimSnapshotError, SIM_INITIAL_SNAPSHOT_SCHEMA,
        parse_initial_snapshot_from_game_state,
    },
    state::LockstepSimSceneState,
};

#[derive(Clone, Debug, Default, Resource, PartialEq, Eq)]
pub(in crate::game::features::lockstep_sim) struct LockstepSimMyServerJoinState {
    pub(in crate::game::features::lockstep_sim) authority_started: bool,
    pub(in crate::game::features::lockstep_sim) login_sent: bool,
    pub(in crate::game::features::lockstep_sim) join_sent: bool,
    pub(in crate::game::features::lockstep_sim) ready_sent: bool,
    pub(in crate::game::features::lockstep_sim) start_sent: bool,
    pub(in crate::game::features::lockstep_sim) started: bool,
    authenticated_player_id: Option<String>,
}

impl LockstepSimMyServerJoinState {
    pub(in crate::game::features::lockstep_sim) fn reset(&mut self) {
        *self = Self::default();
    }
}

pub(in crate::game::features::lockstep_sim) fn start_lockstep_sim_authority(
    config: &LockstepSimConfig,
    session: &AuthoritySession,
    state: &mut LockstepSimMyServerJoinState,
    authority_commands: &mut MessageWriter<AuthorityCommand>,
    myserver_commands: &mut MessageWriter<MyServerCommand>,
) {
    if state.authority_started {
        debug!("lockstep sim authority startup already handled");
        return;
    }

    state.authority_started = true;

    match config.authority_mode {
        LockstepSimAuthorityMode::Off => {
            info!(
                player_id = %config.local_player_id,
                "lockstep sim authority startup disabled"
            );
        }
        LockstepSimAuthorityMode::MyServer => {
            let connect_command = match myserver_start_command_from_env_reader(
                config.transport,
                config.myserver_guest_id.as_deref(),
                |name| env::var(name).ok(),
            ) {
                Ok(command) => command,
                Err(error) => {
                    error!(reason = %error, "lockstep sim direct ticket configuration rejected");
                    return;
                }
            };
            leave_existing_authority_if_needed(session, authority_commands);
            info!(
                player_id = %config.local_player_id,
                guest_id = config.myserver_guest_id.as_deref().unwrap_or_default(),
                room_id = %config.myserver_room_id,
                policy_id = %config.myserver_policy_id,
                transport = ?config.transport,
                "lockstep sim starting MyServer authority"
            );
            authority_commands.write(AuthorityCommand::Join {
                player_id: config.local_player_id.clone(),
                endpoint: AuthorityEndpoint::MyServer {
                    host: None,
                    port: None,
                    transport: config.transport,
                },
            });
            myserver_commands.write(connect_command);
            state.login_sent = true;
        }
    }
}

fn myserver_start_command_from_env_reader(
    transport: crate::framework::network::NetworkTransport,
    guest_id: Option<&str>,
    mut read: impl FnMut(&str) -> Option<String>,
) -> Result<MyServerCommand, String> {
    let ticket_env = read("LOCKSTEP_SIM_MYSERVER_TICKET_ENV")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let Some(ticket_env) = ticket_env else {
        return Ok(MyServerCommand::GuestLogin {
            guest_id: guest_id.map(str::to_string),
            connect_game: true,
        });
    };
    if !is_environment_variable_name(&ticket_env) {
        return Err(
            "LOCKSTEP_SIM_MYSERVER_TICKET_ENV is not a valid environment variable name".to_string(),
        );
    }
    let ticket = read(&ticket_env)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("ticket environment variable {ticket_env:?} is missing or empty"))?;
    Ok(MyServerCommand::ConnectWithTicket {
        ticket,
        transport,
        host: None,
        port: None,
    })
}

fn is_environment_variable_name(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|first| first == '_' || first.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

pub(in crate::game::features::lockstep_sim) fn cleanup_lockstep_sim_authority(
    config: &LockstepSimConfig,
    state: &mut LockstepSimMyServerJoinState,
    authority_commands: &mut MessageWriter<AuthorityCommand>,
    myserver_commands: &mut MessageWriter<MyServerCommand>,
) {
    let should_disconnect_myserver =
        matches!(config.authority_mode, LockstepSimAuthorityMode::MyServer)
            || state.login_sent
            || state.join_sent
            || state.ready_sent
            || state.start_sent;

    state.reset();
    info!(
        player_id = %config.local_player_id,
        room_id = %config.myserver_room_id,
        policy_id = %config.myserver_policy_id,
        disconnect_myserver = should_disconnect_myserver,
        "lockstep sim authority cleanup"
    );
    authority_commands.write(AuthorityCommand::Leave);

    if should_disconnect_myserver {
        myserver_commands.write(MyServerCommand::Disconnect);
    }
}

pub(in crate::game::features::lockstep_sim) fn follow_lockstep_sim_myserver_events(
    config: Res<LockstepSimConfig>,
    mut scene_state: ResMut<LockstepSimSceneState>,
    mut state: ResMut<LockstepSimMyServerJoinState>,
    mut events: MessageReader<MyServerEvent>,
    mut commands: MessageWriter<MyServerCommand>,
) {
    if !scene_state.active || !matches!(config.authority_mode, LockstepSimAuthorityMode::MyServer) {
        return;
    }

    for event in events.read() {
        handle_lockstep_sim_myserver_event(
            &config,
            &mut scene_state,
            &mut state,
            event,
            &mut commands,
        );
    }
}

fn handle_lockstep_sim_myserver_event(
    config: &LockstepSimConfig,
    scene_state: &mut LockstepSimSceneState,
    state: &mut LockstepSimMyServerJoinState,
    event: &MyServerEvent,
    commands: &mut MessageWriter<MyServerCommand>,
) {
    match event {
        MyServerEvent::Authenticated { player_id } if !state.join_sent => {
            state.authenticated_player_id = Some(player_id.clone());
            state.join_sent = true;
            info!(
                player_id = %player_id,
                room_id = %config.myserver_room_id,
                policy_id = %config.myserver_policy_id,
                "lockstep sim joining MyServer room"
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
                "lockstep sim MyServer room joined"
            );
            commands.write(MyServerCommand::SetReady { ready: true });
        }
        MyServerEvent::RoomJoined(response) if !response.ok => {
            warn!(
                room_id = %response.room_id,
                policy_id = %config.myserver_policy_id,
                player_id = %config.local_player_id,
                error_code = %response.error_code,
                "lockstep sim MyServer room join rejected"
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
                "lockstep sim MyServer starting room after local ready"
            );
            commands.write(MyServerCommand::StartRoom);
        }
        MyServerEvent::ReadyChanged(response) if !response.ok => {
            warn!(
                room_id = %response.room_id,
                policy_id = %config.myserver_policy_id,
                player_id = %config.local_player_id,
                error_code = %response.error_code,
                "lockstep sim MyServer ready rejected"
            );
        }
        MyServerEvent::RoomStatePush(push)
            if state.ready_sent
                && !state.start_sent
                && room_state_says_local_ready(state, push) =>
        {
            if let Some(snapshot) = push.snapshot.as_ref() {
                try_parse_initial_snapshot(
                    scene_state,
                    &snapshot.room_id,
                    &snapshot.game_state,
                    "room_state",
                );
            }
            state.start_sent = true;
            info!(
                room_id = push.snapshot.as_ref().map(|snapshot| snapshot.room_id.as_str()).unwrap_or_default(),
                policy_id = %config.myserver_policy_id,
                "lockstep sim MyServer starting room after ready state push"
            );
            commands.write(MyServerCommand::StartRoom);
        }
        MyServerEvent::RoomStatePush(push) => {
            if let Some(snapshot) = push.snapshot.as_ref() {
                try_parse_initial_snapshot(
                    scene_state,
                    &snapshot.room_id,
                    &snapshot.game_state,
                    "room_state",
                );
            }
        }
        MyServerEvent::RoomReconnected(response) => {
            if response.ok {
                if let Some(snapshot) = response.snapshot.as_ref() {
                    try_parse_initial_snapshot(
                        scene_state,
                        &snapshot.room_id,
                        &snapshot.game_state,
                        "room_reconnect",
                    );
                } else {
                    scene_state.clear_initial_snapshot();
                    warn!(
                        room_id = %response.room_id,
                        policy_id = %config.myserver_policy_id,
                        player_id = %config.local_player_id,
                        "lockstep sim MyServer reconnect response had no recovery snapshot"
                    );
                }
            } else {
                scene_state.clear_initial_snapshot();
                warn!(
                    room_id = %response.room_id,
                    policy_id = %config.myserver_policy_id,
                    player_id = %config.local_player_id,
                    error_code = %response.error_code,
                    "lockstep sim MyServer reconnect rejected"
                );
            }
        }
        MyServerEvent::RoomStarted(response) if response.ok => {
            state.started = true;
            info!(
                room_id = %response.room_id,
                policy_id = %config.myserver_policy_id,
                "lockstep sim MyServer room started"
            );
        }
        MyServerEvent::RoomStarted(response) => {
            warn!(
                room_id = %response.room_id,
                policy_id = %config.myserver_policy_id,
                player_id = %config.local_player_id,
                error_code = %response.error_code,
                "lockstep sim MyServer room start rejected"
            );
        }
        MyServerEvent::ConnectionFailed {
            transport,
            remote_addr,
            error,
        } => {
            scene_state.clear_initial_snapshot();
            error!(
                room_id = %config.myserver_room_id,
                policy_id = %config.myserver_policy_id,
                player_id = %config.local_player_id,
                ?transport,
                remote_addr = %remote_addr,
                reason = %error,
                "lockstep sim MyServer connection failed"
            );
        }
        MyServerEvent::Disconnected { reason } => {
            scene_state.clear_initial_snapshot();
            warn!(
                room_id = %config.myserver_room_id,
                policy_id = %config.myserver_policy_id,
                player_id = %config.local_player_id,
                reason = reason.as_deref().unwrap_or_default(),
                "lockstep sim MyServer disconnected"
            );
        }
        MyServerEvent::AuthFailed { error_code } => {
            error!(
                room_id = %config.myserver_room_id,
                policy_id = %config.myserver_policy_id,
                player_id = %config.local_player_id,
                error_code = %error_code,
                "lockstep sim MyServer auth failed"
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
                seq = *seq,
                error_code = %error_code,
                reason = %message,
                "lockstep sim MyServer error"
            );
        }
        MyServerEvent::ProtocolError { error } => {
            error!(
                room_id = %config.myserver_room_id,
                policy_id = %config.myserver_policy_id,
                player_id = %config.local_player_id,
                reason = %error,
                "lockstep sim MyServer protocol error"
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
                ?seq,
                ?message_type,
                reason = %error,
                "lockstep sim MyServer request failed"
            );
        }
        _ => {}
    }
}

fn try_parse_initial_snapshot(
    scene_state: &mut LockstepSimSceneState,
    room_id: &str,
    game_state_json: &str,
    source: &'static str,
) {
    if !game_state_json.contains(SIM_INITIAL_SNAPSHOT_SCHEMA) {
        return;
    }

    match parse_initial_snapshot_from_game_state(game_state_json) {
        Ok(snapshot) => {
            let generation_changed = scene_state.replace_initial_snapshot(snapshot);
            let Some(snapshot) = scene_state.initial_snapshot.as_ref() else {
                return;
            };
            info!(
                room_id = %snapshot.room_id,
                start_frame = snapshot.start_frame,
                tick_rate = snapshot.tick_rate,
                config_version = snapshot.config_version,
                config_hash = %snapshot.config_hash,
                entity_count = snapshot.entities.len(),
                binding_count = snapshot.control_bindings.len(),
                state_hash = %snapshot.state_hash.hex,
                source,
                snapshot_generation = scene_state.snapshot_generation,
                generation_changed,
                "lockstep sim initial snapshot restored"
            );
        }
        Err(error) => {
            warn!(
                room_id = %room_id,
                error_code = %lockstep_snapshot_error_code(&error),
                reason = %error,
                source,
                "lockstep sim initial snapshot rejected"
            );
            scene_state.reject_initial_snapshot(error);
        }
    }
}

fn lockstep_snapshot_error_code(error: &LockstepSimSnapshotError) -> &'static str {
    match error {
        LockstepSimSnapshotError::JsonDecode(_) => "json_decode",
        LockstepSimSnapshotError::MissingInitialSnapshot => "missing_initial_snapshot",
        LockstepSimSnapshotError::UnsupportedSchema { .. } => "unsupported_schema",
        LockstepSimSnapshotError::UnsupportedSchemaVersion { .. } => "unsupported_schema_version",
        LockstepSimSnapshotError::InvalidRoomId => "invalid_room_id",
        LockstepSimSnapshotError::InvalidTickRate => "invalid_tick_rate",
        LockstepSimSnapshotError::InvalidConfigVersion => "invalid_config_version",
        LockstepSimSnapshotError::UnsupportedSimSchemaVersion { .. } => {
            "unsupported_sim_schema_version"
        }
        LockstepSimSnapshotError::ConfigHashMismatch { .. } => "config_hash_mismatch",
        LockstepSimSnapshotError::SnapshotRestore(_) => "snapshot_restore",
        LockstepSimSnapshotError::FrameMismatch { .. } => "frame_mismatch",
        LockstepSimSnapshotError::RngSeedMismatch { .. } => "rng_seed_mismatch",
        LockstepSimSnapshotError::HashEnvelopeMismatch { .. } => "hash_envelope_mismatch",
        LockstepSimSnapshotError::EntitiesMismatch => "entities_mismatch",
        LockstepSimSnapshotError::InvalidControlBinding { code } => code,
    }
}

fn room_state_says_local_ready(
    state: &LockstepSimMyServerJoinState,
    push: &crate::game::myserver::protocol::pb::RoomStatePush,
) -> bool {
    let Some(snapshot) = push.snapshot.as_ref() else {
        return false;
    };
    if snapshot.state == "in_game" {
        return false;
    }
    let Some(player_id) = state.authenticated_player_id.as_deref() else {
        return false;
    };

    snapshot
        .members
        .iter()
        .any(|member| member.character_id == player_id && member.ready)
}

fn leave_existing_authority_if_needed(
    session: &AuthoritySession,
    authority_commands: &mut MessageWriter<AuthorityCommand>,
) {
    if session.role.is_some_and(|role| role != AuthorityRole::None) {
        authority_commands.write(AuthorityCommand::Leave);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::network::NetworkTransport;

    #[test]
    fn direct_ticket_start_is_explicit_and_keeps_default_guest_login() {
        let guest =
            myserver_start_command_from_env_reader(NetworkTransport::Tcp, Some("guest-a"), |_| {
                None
            })
            .unwrap();
        assert!(matches!(
            guest,
            MyServerCommand::GuestLogin {
                guest_id: Some(ref guest_id),
                connect_game: true,
            } if guest_id == "guest-a"
        ));

        let direct =
            myserver_start_command_from_env_reader(
                NetworkTransport::Tcp,
                None,
                |name| match name {
                    "LOCKSTEP_SIM_MYSERVER_TICKET_ENV" => Some("LOCAL_TEST_TICKET".to_string()),
                    "LOCAL_TEST_TICKET" => Some("ticket-value".to_string()),
                    _ => None,
                },
            )
            .unwrap();
        assert!(matches!(
            direct,
            MyServerCommand::ConnectWithTicket {
                ref ticket,
                transport: NetworkTransport::Tcp,
                host: None,
                port: None,
            } if ticket == "ticket-value"
        ));
    }

    #[test]
    fn direct_ticket_start_rejects_missing_or_invalid_environment_reference() {
        let missing = myserver_start_command_from_env_reader(NetworkTransport::Tcp, None, |name| {
            (name == "LOCKSTEP_SIM_MYSERVER_TICKET_ENV").then(|| "LOCAL_TEST_TICKET".to_string())
        })
        .unwrap_err();
        assert!(missing.contains("missing or empty"));

        let invalid = myserver_start_command_from_env_reader(NetworkTransport::Tcp, None, |name| {
            (name == "LOCKSTEP_SIM_MYSERVER_TICKET_ENV").then(|| "INVALID-NAME".to_string())
        })
        .unwrap_err();
        assert!(invalid.contains("valid environment variable name"));
    }
}
