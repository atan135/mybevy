use bevy::prelude::*;

use crate::{
    framework::scene::prelude::SceneEvent,
    game::{
        authority::{AuthorityCommand, AuthoritySession},
        myserver::MyServerCommand,
    },
};

use super::{
    bot::{
        ROBOT_MOVE_ACTION, RobotMoveDirection, RobotMovePayload, RobotSyncBotState,
        clear_robot_sync_bots,
    },
    config::{RobotSyncConfig, RobotSyncInputMode},
    state::RobotSyncSceneState,
    sync::{
        RobotSyncMyServerJoinState, RobotSyncReplayState, apply_robot_sync_authority_events,
        cleanup_robot_sync_authority, follow_robot_sync_myserver_events, reset_robot_sync_replay,
        start_robot_sync_authority,
    },
    visual::{RobotSyncVisualState, clear_robot_sync_visuals, sync_robot_sync_robot_visuals},
};

pub(in crate::game) struct RobotSyncPlugin;

impl Plugin for RobotSyncPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RobotSyncConfig>()
            .init_resource::<RobotSyncSceneState>()
            .init_resource::<RobotSyncBotState>()
            .init_resource::<RobotSyncReplayState>()
            .init_resource::<RobotSyncMyServerJoinState>()
            .init_resource::<RobotSyncVisualState>()
            .add_systems(
                Update,
                (
                    follow_robot_sync_myserver_events,
                    apply_robot_sync_authority_events,
                    send_local_robot_sync_input,
                ),
            )
            .add_systems(
                PostUpdate,
                (update_robot_sync_scene_state, sync_robot_sync_robot_visuals),
            );
    }
}

fn send_local_robot_sync_input(
    config: Res<RobotSyncConfig>,
    scene_state: Res<RobotSyncSceneState>,
    authority_session: Res<AuthoritySession>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut bot_state: ResMut<RobotSyncBotState>,
    mut authority_commands: MessageWriter<AuthorityCommand>,
) {
    if !scene_state.active {
        return;
    }

    let Some(local_player_id) = authority_session.local_player_id.as_deref() else {
        return;
    };

    match config.input_mode {
        RobotSyncInputMode::Bot => send_local_robot_sync_bot_input(
            &config,
            &authority_session,
            &mut bot_state,
            &mut authority_commands,
            local_player_id,
        ),
        RobotSyncInputMode::Manual => send_local_robot_sync_manual_input(
            &config,
            &authority_session,
            &keyboard_input,
            &mut bot_state,
            &mut authority_commands,
            local_player_id,
        ),
        RobotSyncInputMode::Off => {}
    }
}

fn send_local_robot_sync_bot_input(
    config: &RobotSyncConfig,
    authority_session: &AuthoritySession,
    bot_state: &mut RobotSyncBotState,
    authority_commands: &mut MessageWriter<AuthorityCommand>,
    local_player_id: &str,
) {
    let target_frame = authority_session
        .frame_id
        .saturating_add(config.input_delay_frames);
    if !bot_state.should_send_target_frame(target_frame, config.bot_input_interval_frames) {
        return;
    }

    let payload = bot_state.next_move_payload(local_player_id, config.bot_speed);
    let Ok(payload_json) = serde_json::to_string(&payload) else {
        return;
    };

    debug!(
        player_id = %local_player_id,
        target_frame,
        seq = payload.seq,
        botTick = payload.bot_tick,
        dirX = payload.dir_x,
        dirY = payload.dir_y,
        speed = payload.speed,
        "sending robot sync bot input"
    );
    authority_commands.write(AuthorityCommand::SendInput {
        frame_id: target_frame,
        action: ROBOT_MOVE_ACTION.to_string(),
        payload_json,
    });
    bot_state.mark_sent_target_frame(target_frame);
}

fn send_local_robot_sync_manual_input(
    config: &RobotSyncConfig,
    authority_session: &AuthoritySession,
    keyboard_input: &ButtonInput<KeyCode>,
    bot_state: &mut RobotSyncBotState,
    authority_commands: &mut MessageWriter<AuthorityCommand>,
    local_player_id: &str,
) {
    let direction = manual_robot_sync_direction(keyboard_input);
    let speed = if direction.is_zero() {
        0
    } else {
        config.manual_speed
    };
    let target_frame = authority_session
        .frame_id
        .saturating_add(config.input_delay_frames);

    if direction.is_zero() && bot_state.last_input_was_stop_or_none() {
        return;
    }
    if bot_state.last_input_matches(direction, speed)
        && !bot_state.should_send_target_frame(target_frame, config.bot_input_interval_frames)
    {
        return;
    }
    if !bot_state.last_input_matches(direction, speed)
        && matches!(bot_state.last_sent_target_frame, Some(last) if target_frame <= last)
    {
        return;
    }

    let payload = bot_state.next_move_payload_for_direction(direction, speed);
    send_robot_sync_input(
        authority_commands,
        local_player_id,
        target_frame,
        &payload,
        "sending robot sync manual input",
    );
    bot_state.mark_sent_target_frame(target_frame);
}

fn manual_robot_sync_direction(keyboard_input: &ButtonInput<KeyCode>) -> RobotMoveDirection {
    let x = pressed_axis(
        keyboard_input,
        [KeyCode::KeyA, KeyCode::ArrowLeft],
        [KeyCode::KeyD, KeyCode::ArrowRight],
    );
    let y = pressed_axis(
        keyboard_input,
        [KeyCode::KeyS, KeyCode::ArrowDown],
        [KeyCode::KeyW, KeyCode::ArrowUp],
    );

    match (x, y) {
        (0, 0) => RobotMoveDirection::ZERO,
        (x, 0) => RobotMoveDirection {
            dir_x: x * 1000,
            dir_y: 0,
        },
        (0, y) => RobotMoveDirection {
            dir_x: 0,
            dir_y: y * 1000,
        },
        (x, y) => RobotMoveDirection {
            dir_x: x * 707,
            dir_y: y * 707,
        },
    }
}

fn pressed_axis(
    keyboard_input: &ButtonInput<KeyCode>,
    negative_keys: [KeyCode; 2],
    positive_keys: [KeyCode; 2],
) -> i32 {
    let negative = keyboard_input.any_pressed(negative_keys);
    let positive = keyboard_input.any_pressed(positive_keys);
    match (negative, positive) {
        (true, false) => -1,
        (false, true) => 1,
        _ => 0,
    }
}

fn send_robot_sync_input(
    authority_commands: &mut MessageWriter<AuthorityCommand>,
    local_player_id: &str,
    target_frame: u32,
    payload: &RobotMovePayload,
    message: &str,
) {
    let Ok(payload_json) = serde_json::to_string(payload) else {
        return;
    };

    debug!(
        player_id = %local_player_id,
        target_frame,
        seq = payload.seq,
        botTick = payload.bot_tick,
        dirX = payload.dir_x,
        dirY = payload.dir_y,
        speed = payload.speed,
        "{message}"
    );
    authority_commands.write(AuthorityCommand::SendInput {
        frame_id: target_frame,
        action: ROBOT_MOVE_ACTION.to_string(),
        payload_json,
    });
}

fn update_robot_sync_scene_state(
    config: Res<RobotSyncConfig>,
    authority_session: Res<AuthoritySession>,
    mut scene_state: ResMut<RobotSyncSceneState>,
    mut bot_state: ResMut<RobotSyncBotState>,
    mut replay_state: ResMut<RobotSyncReplayState>,
    mut join_state: ResMut<RobotSyncMyServerJoinState>,
    mut visual_state: ResMut<RobotSyncVisualState>,
    mut scene_events: MessageReader<SceneEvent>,
    mut authority_commands: MessageWriter<AuthorityCommand>,
    mut myserver_commands: MessageWriter<MyServerCommand>,
) {
    for event in scene_events.read() {
        match event {
            SceneEvent::Entered(entered) if config.is_robot_sync_scene(&entered.scene_id) => {
                scene_state.activate(entered.scene_id.clone(), entered.session_id.clone());
                start_robot_sync_authority(
                    &config,
                    &authority_session,
                    &mut join_state,
                    &mut authority_commands,
                    &mut myserver_commands,
                );
            }
            SceneEvent::Exited(exited)
                if config.is_robot_sync_scene(&exited.scene_id)
                    && scene_state.is_active_session(&exited.session_id) =>
            {
                cleanup_robot_sync_authority(
                    &config,
                    &mut join_state,
                    &mut authority_commands,
                    &mut myserver_commands,
                );
                scene_state.reset();
                clear_robot_sync_bots(&mut bot_state);
                reset_robot_sync_replay(&mut replay_state);
                clear_robot_sync_visuals(&mut visual_state);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        framework::network::NetworkTransport,
        framework::scene::prelude::{SceneEntered, SceneExited, SceneId, SceneSessionId},
        game::{
            authority::{AuthorityEndpoint, AuthorityEvent},
            features::robot_sync::{
                config::{
                    DEFAULT_ROBOT_SYNC_PLAYER_ID, ROBOT_SYNC_MYSERVER_POLICY_ID,
                    RobotSyncAuthorityMode, RobotSyncInputMode,
                },
                sync::RobotSyncMyServerJoinState,
            },
            myserver::{MyServerEvent, protocol::pb},
            scenes::ROBOT_SYNC_ARENA_SCENE_ID,
        },
    };
    use bevy::ecs::message::{MessageCursor, Messages};

    fn test_app() -> App {
        let mut app = App::new();
        app.add_message::<SceneEvent>()
            .add_message::<AuthorityCommand>()
            .add_message::<AuthorityEvent>()
            .add_message::<MyServerCommand>()
            .add_message::<MyServerEvent>()
            .init_resource::<AuthoritySession>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_plugins(RobotSyncPlugin);
        app
    }

    fn test_config(authority_mode: RobotSyncAuthorityMode) -> RobotSyncConfig {
        RobotSyncConfig {
            scene_id: SceneId::from(ROBOT_SYNC_ARENA_SCENE_ID),
            local_player_id: DEFAULT_ROBOT_SYNC_PLAYER_ID.to_string(),
            authority_mode,
            lan_bind_addr: "127.0.0.1:15000".to_string(),
            remote_host: "127.0.0.1".to_string(),
            remote_port: 15000,
            transport: NetworkTransport::Tcp,
            myserver_guest_id: Some("robot-guest".to_string()),
            myserver_room_id: "robot-room".to_string(),
            myserver_policy_id: ROBOT_SYNC_MYSERVER_POLICY_ID.to_string(),
            input_mode: RobotSyncInputMode::Bot,
            input_delay_frames: 2,
            bot_input_interval_frames: 1,
            bot_speed: 10000,
            manual_speed: 10000,
        }
    }

    #[test]
    fn robot_sync_plugin_initializes_resources() {
        let app = test_app();

        assert!(app.world().contains_resource::<RobotSyncConfig>());
        assert_eq!(
            app.world().resource::<RobotSyncConfig>().scene_id.as_str(),
            ROBOT_SYNC_ARENA_SCENE_ID
        );
        assert_eq!(
            *app.world().resource::<RobotSyncSceneState>(),
            RobotSyncSceneState::default()
        );
        assert_eq!(
            *app.world().resource::<RobotSyncBotState>(),
            RobotSyncBotState::default()
        );
        assert_eq!(
            *app.world().resource::<RobotSyncReplayState>(),
            RobotSyncReplayState::default()
        );
        assert_eq!(
            *app.world().resource::<RobotSyncMyServerJoinState>(),
            RobotSyncMyServerJoinState::default()
        );
        assert_eq!(
            *app.world().resource::<RobotSyncVisualState>(),
            RobotSyncVisualState::default()
        );
    }

    #[test]
    fn robot_sync_scene_entered_activates_scene_state() {
        let mut app = test_app();
        app.insert_resource(test_config(RobotSyncAuthorityMode::Off));
        let session_id = SceneSessionId::from("robot-sync-session");

        app.world_mut()
            .write_message(SceneEvent::Entered(SceneEntered {
                scene_id: SceneId::from(ROBOT_SYNC_ARENA_SCENE_ID),
                session_id: session_id.clone(),
                content_version: None,
            }));
        app.update();

        let state = app.world().resource::<RobotSyncSceneState>();
        assert!(state.active);
        assert_eq!(state.session_id.as_ref(), Some(&session_id));
        assert_eq!(
            state.scene_id.as_ref().map(SceneId::as_str),
            Some(ROBOT_SYNC_ARENA_SCENE_ID)
        );
    }

    #[test]
    fn non_robot_sync_scene_entered_does_not_activate_scene_state() {
        let mut app = test_app();
        app.insert_resource(test_config(RobotSyncAuthorityMode::Local));

        app.world_mut()
            .write_message(SceneEvent::Entered(SceneEntered {
                scene_id: SceneId::from("arena.other"),
                session_id: SceneSessionId::from("other-session"),
                content_version: None,
            }));
        app.update();

        assert_eq!(
            *app.world().resource::<RobotSyncSceneState>(),
            RobotSyncSceneState::default()
        );
        assert!(read_messages::<AuthorityCommand>(app.world()).is_empty());
        assert!(read_messages::<MyServerCommand>(app.world()).is_empty());
    }

    #[test]
    fn matching_robot_sync_scene_exited_clears_active_and_module_state() {
        let mut app = test_app();
        app.insert_resource(test_config(RobotSyncAuthorityMode::Off));
        let session_id = SceneSessionId::from("robot-sync-session");

        app.world_mut()
            .write_message(SceneEvent::Entered(SceneEntered {
                scene_id: SceneId::from(ROBOT_SYNC_ARENA_SCENE_ID),
                session_id: session_id.clone(),
                content_version: None,
            }));
        app.update();

        app.world_mut()
            .resource_mut::<RobotSyncBotState>()
            .local_bot_slots = 2;
        {
            let mut replay_state = app.world_mut().resource_mut::<RobotSyncReplayState>();
            replay_state.buffered_frame_count = 3;
            replay_state.last_frame_id = Some(12);
        }
        app.world_mut()
            .resource_mut::<RobotSyncVisualState>()
            .tracked_robot_entities = 4;

        app.world_mut()
            .write_message(SceneEvent::Exited(SceneExited {
                scene_id: SceneId::from(ROBOT_SYNC_ARENA_SCENE_ID),
                session_id,
            }));
        app.update();

        assert_eq!(
            *app.world().resource::<RobotSyncSceneState>(),
            RobotSyncSceneState::default()
        );
        assert_eq!(
            *app.world().resource::<RobotSyncBotState>(),
            RobotSyncBotState::default()
        );
        assert_eq!(
            *app.world().resource::<RobotSyncReplayState>(),
            RobotSyncReplayState::default()
        );
        assert_eq!(
            *app.world().resource::<RobotSyncVisualState>(),
            RobotSyncVisualState::default()
        );
    }

    #[test]
    fn robot_sync_scene_entered_local_mode_starts_local_authority() {
        let mut app = test_app();
        app.insert_resource(test_config(RobotSyncAuthorityMode::Local));

        app.world_mut()
            .write_message(SceneEvent::Entered(SceneEntered {
                scene_id: SceneId::from(ROBOT_SYNC_ARENA_SCENE_ID),
                session_id: SceneSessionId::from("robot-sync-session"),
                content_version: None,
            }));
        app.update();

        let authority_commands = read_messages::<AuthorityCommand>(app.world());
        assert_eq!(authority_commands.len(), 1);
        assert!(matches!(
            &authority_commands[0],
            AuthorityCommand::HostLocal { player_id }
                if player_id == DEFAULT_ROBOT_SYNC_PLAYER_ID
        ));
        assert!(read_messages::<MyServerCommand>(app.world()).is_empty());
    }

    #[test]
    fn robot_sync_scene_entered_myserver_mode_joins_and_logs_in() {
        let mut app = test_app();
        app.insert_resource(test_config(RobotSyncAuthorityMode::MyServer));

        app.world_mut()
            .write_message(SceneEvent::Entered(SceneEntered {
                scene_id: SceneId::from(ROBOT_SYNC_ARENA_SCENE_ID),
                session_id: SceneSessionId::from("robot-sync-session"),
                content_version: None,
            }));
        app.update();

        let authority_commands = read_messages::<AuthorityCommand>(app.world());
        assert_eq!(authority_commands.len(), 1);
        assert!(matches!(
            &authority_commands[0],
            AuthorityCommand::Join {
                player_id,
                endpoint:
                    AuthorityEndpoint::MyServer {
                        host: None,
                        port: None,
                        transport: NetworkTransport::Tcp
                    }
            } if player_id == DEFAULT_ROBOT_SYNC_PLAYER_ID
        ));

        let myserver_commands = read_messages::<MyServerCommand>(app.world());
        assert_eq!(myserver_commands.len(), 1);
        assert!(matches!(
            &myserver_commands[0],
            MyServerCommand::GuestLogin {
                guest_id: Some(guest_id),
                connect_game: true
            } if guest_id == "robot-guest"
        ));

        let join_state = app.world().resource::<RobotSyncMyServerJoinState>();
        assert!(join_state.authority_started);
        assert!(join_state.login_sent);
    }

    #[test]
    fn robot_sync_myserver_events_send_join_ready_start_in_order() {
        let mut app = test_app();
        app.insert_resource(test_config(RobotSyncAuthorityMode::MyServer));
        app.world_mut()
            .write_message(SceneEvent::Entered(SceneEntered {
                scene_id: SceneId::from(ROBOT_SYNC_ARENA_SCENE_ID),
                session_id: SceneSessionId::from("robot-sync-session"),
                content_version: None,
            }));
        app.update();

        app.world_mut().write_message(MyServerEvent::Authenticated {
            player_id: "robot-player".to_string(),
        });
        app.update();
        let myserver_commands = read_messages::<MyServerCommand>(app.world());
        assert!(matches!(
            myserver_commands.last(),
            Some(MyServerCommand::JoinRoom { room_id, policy_id })
                if room_id == "robot-room" && policy_id == ROBOT_SYNC_MYSERVER_POLICY_ID
        ));

        app.world_mut()
            .write_message(MyServerEvent::RoomJoined(pb::RoomJoinRes {
                ok: true,
                room_id: "robot-room".to_string(),
                error_code: String::new(),
            }));
        app.update();
        let myserver_commands = read_messages::<MyServerCommand>(app.world());
        assert!(matches!(
            myserver_commands.last(),
            Some(MyServerCommand::SetReady { ready: true })
        ));

        app.world_mut()
            .write_message(MyServerEvent::ReadyChanged(pb::RoomReadyRes {
                ok: true,
                room_id: "robot-room".to_string(),
                ready: true,
                error_code: String::new(),
            }));
        app.update();
        let myserver_commands = read_messages::<MyServerCommand>(app.world());
        assert!(matches!(
            myserver_commands.last(),
            Some(MyServerCommand::StartRoom)
        ));

        let join_state = app.world().resource::<RobotSyncMyServerJoinState>();
        assert!(join_state.join_sent);
        assert!(join_state.ready_sent);
        assert!(join_state.start_sent);
    }

    #[test]
    fn robot_sync_exited_cleans_join_state_and_sends_leave_disconnect() {
        let mut app = test_app();
        app.insert_resource(test_config(RobotSyncAuthorityMode::MyServer));
        let session_id = SceneSessionId::from("robot-sync-session");

        app.world_mut()
            .write_message(SceneEvent::Entered(SceneEntered {
                scene_id: SceneId::from(ROBOT_SYNC_ARENA_SCENE_ID),
                session_id: session_id.clone(),
                content_version: None,
            }));
        app.update();
        {
            let mut join_state = app.world_mut().resource_mut::<RobotSyncMyServerJoinState>();
            join_state.join_sent = true;
            join_state.ready_sent = true;
        }

        app.world_mut()
            .write_message(SceneEvent::Exited(SceneExited {
                scene_id: SceneId::from(ROBOT_SYNC_ARENA_SCENE_ID),
                session_id,
            }));
        app.update();

        assert_eq!(
            *app.world().resource::<RobotSyncMyServerJoinState>(),
            RobotSyncMyServerJoinState::default()
        );
        let authority_commands = read_messages::<AuthorityCommand>(app.world());
        assert!(matches!(
            authority_commands.last(),
            Some(AuthorityCommand::Leave)
        ));
        let myserver_commands = read_messages::<MyServerCommand>(app.world());
        assert!(matches!(
            myserver_commands.last(),
            Some(MyServerCommand::Disconnect)
        ));
    }

    #[test]
    fn active_robot_sync_with_local_player_sends_robot_move_input_for_delayed_frame() {
        let mut app = test_app();
        app.insert_resource(test_config(RobotSyncAuthorityMode::Off));
        activate_robot_sync_scene(&mut app);
        {
            let mut session = app.world_mut().resource_mut::<AuthoritySession>();
            session.local_player_id = Some("robot-player-a".to_string());
            session.frame_id = 40;
        }

        app.update();

        let authority_commands = read_messages::<AuthorityCommand>(app.world());
        assert_eq!(authority_commands.len(), 1);
        let AuthorityCommand::SendInput {
            frame_id,
            action,
            payload_json,
        } = &authority_commands[0]
        else {
            panic!("expected robot move input command");
        };
        assert_eq!(*frame_id, 42);
        assert_eq!(action, ROBOT_MOVE_ACTION);

        let payload = serde_json::from_str::<serde_json::Value>(payload_json).unwrap();
        assert_eq!(
            payload.get("version").and_then(|value| value.as_i64()),
            Some(1)
        );
        assert_eq!(payload.get("seq").and_then(|value| value.as_i64()), Some(1));
        assert_eq!(
            payload.get("botTick").and_then(|value| value.as_i64()),
            Some(0)
        );
        assert_eq!(
            payload.get("speed").and_then(|value| value.as_i64()),
            Some(10000)
        );
        assert!(
            payload
                .get("dirX")
                .and_then(|value| value.as_i64())
                .is_some()
        );
        assert!(
            payload
                .get("dirY")
                .and_then(|value| value.as_i64())
                .is_some()
        );
    }

    #[test]
    fn robot_sync_bot_input_does_not_repeat_same_target_frame() {
        let mut app = test_app();
        app.insert_resource(test_config(RobotSyncAuthorityMode::Off));
        activate_robot_sync_scene(&mut app);
        {
            let mut session = app.world_mut().resource_mut::<AuthoritySession>();
            session.local_player_id = Some("robot-player-a".to_string());
            session.frame_id = 40;
        }

        app.update();
        app.update();

        let authority_commands = read_messages::<AuthorityCommand>(app.world());
        let robot_move_count = authority_commands
            .iter()
            .filter(|command| {
                matches!(
                    command,
                    AuthorityCommand::SendInput { action, .. } if action == ROBOT_MOVE_ACTION
                )
            })
            .count();
        assert_eq!(robot_move_count, 1);
    }

    #[test]
    fn robot_sync_bot_input_skips_inactive_scene() {
        let mut app = test_app();
        app.insert_resource(test_config(RobotSyncAuthorityMode::Off));
        {
            let mut session = app.world_mut().resource_mut::<AuthoritySession>();
            session.local_player_id = Some("robot-player-a".to_string());
            session.frame_id = 40;
        }

        app.update();

        assert!(read_messages::<AuthorityCommand>(app.world()).is_empty());
    }

    #[test]
    fn robot_sync_bot_input_skips_missing_local_player() {
        let mut app = test_app();
        app.insert_resource(test_config(RobotSyncAuthorityMode::Off));
        activate_robot_sync_scene(&mut app);
        app.world_mut().resource_mut::<AuthoritySession>().frame_id = 40;

        app.update();

        assert!(read_messages::<AuthorityCommand>(app.world()).is_empty());
    }

    #[test]
    fn robot_sync_manual_input_uses_keyboard_direction() {
        let mut app = test_app();
        let mut config = test_config(RobotSyncAuthorityMode::Off);
        config.input_mode = RobotSyncInputMode::Manual;
        app.insert_resource(config);
        activate_robot_sync_scene(&mut app);
        {
            let mut session = app.world_mut().resource_mut::<AuthoritySession>();
            session.local_player_id = Some("robot-player-a".to_string());
            session.frame_id = 40;
        }
        {
            let mut keyboard = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keyboard.press(KeyCode::KeyW);
            keyboard.press(KeyCode::KeyD);
        }

        app.update();

        let authority_commands = read_messages::<AuthorityCommand>(app.world());
        assert_eq!(authority_commands.len(), 1);
        let payload = robot_move_payload(&authority_commands[0]);
        assert_eq!(
            payload.get("dirX").and_then(|value| value.as_i64()),
            Some(707)
        );
        assert_eq!(
            payload.get("dirY").and_then(|value| value.as_i64()),
            Some(707)
        );
        assert_eq!(
            payload.get("speed").and_then(|value| value.as_i64()),
            Some(10000)
        );
    }

    #[test]
    fn robot_sync_manual_input_sends_stop_after_key_release() {
        let mut app = test_app();
        let mut config = test_config(RobotSyncAuthorityMode::Off);
        config.input_mode = RobotSyncInputMode::Manual;
        app.insert_resource(config);
        activate_robot_sync_scene(&mut app);
        {
            let mut session = app.world_mut().resource_mut::<AuthoritySession>();
            session.local_player_id = Some("robot-player-a".to_string());
            session.frame_id = 40;
        }
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);
        app.update();

        app.world_mut().resource_mut::<AuthoritySession>().frame_id = 41;
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::KeyW);
        app.update();

        let authority_commands = read_messages::<AuthorityCommand>(app.world());
        assert_eq!(authority_commands.len(), 2);
        let payload = robot_move_payload(&authority_commands[1]);
        assert_eq!(
            payload.get("dirX").and_then(|value| value.as_i64()),
            Some(0)
        );
        assert_eq!(
            payload.get("dirY").and_then(|value| value.as_i64()),
            Some(0)
        );
        assert_eq!(
            payload.get("speed").and_then(|value| value.as_i64()),
            Some(0)
        );
    }

    #[test]
    fn robot_sync_manual_input_skips_idle_until_key_is_pressed() {
        let mut app = test_app();
        let mut config = test_config(RobotSyncAuthorityMode::Off);
        config.input_mode = RobotSyncInputMode::Manual;
        app.insert_resource(config);
        activate_robot_sync_scene(&mut app);
        {
            let mut session = app.world_mut().resource_mut::<AuthoritySession>();
            session.local_player_id = Some("robot-player-a".to_string());
            session.frame_id = 40;
        }

        app.update();

        assert!(read_messages::<AuthorityCommand>(app.world()).is_empty());
    }

    #[test]
    fn robot_sync_input_mode_off_does_not_send_local_input() {
        let mut app = test_app();
        let mut config = test_config(RobotSyncAuthorityMode::Off);
        config.input_mode = RobotSyncInputMode::Off;
        app.insert_resource(config);
        activate_robot_sync_scene(&mut app);
        {
            let mut session = app.world_mut().resource_mut::<AuthoritySession>();
            session.local_player_id = Some("robot-player-a".to_string());
            session.frame_id = 40;
        }
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);

        app.update();

        assert!(read_messages::<AuthorityCommand>(app.world()).is_empty());
    }

    fn activate_robot_sync_scene(app: &mut App) {
        app.world_mut()
            .resource_mut::<RobotSyncSceneState>()
            .activate(
                SceneId::from(ROBOT_SYNC_ARENA_SCENE_ID),
                SceneSessionId::from("robot-sync-session"),
            );
    }

    fn read_messages<M>(world: &World) -> Vec<M>
    where
        M: Message + Clone,
    {
        let messages = world.resource::<Messages<M>>();
        let mut cursor = MessageCursor::default();
        cursor.read(messages).cloned().collect()
    }

    fn robot_move_payload(command: &AuthorityCommand) -> serde_json::Value {
        let AuthorityCommand::SendInput {
            action,
            payload_json,
            ..
        } = command
        else {
            panic!("expected robot move input command");
        };
        assert_eq!(action, ROBOT_MOVE_ACTION);
        serde_json::from_str(payload_json).unwrap()
    }
}
