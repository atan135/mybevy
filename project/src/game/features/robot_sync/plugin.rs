use bevy::prelude::*;

use crate::{
    framework::scene::prelude::SceneEvent,
    game::{
        authority::{AuthorityCommand, AuthoritySession},
        myserver::MyServerCommand,
    },
};

use super::{
    bot::{RobotSyncBotState, clear_robot_sync_bots},
    config::RobotSyncConfig,
    state::RobotSyncSceneState,
    sync::{
        RobotSyncMyServerJoinState, RobotSyncReplayState, cleanup_robot_sync_authority,
        follow_robot_sync_myserver_events, reset_robot_sync_replay, start_robot_sync_authority,
    },
    visual::{RobotSyncVisualState, clear_robot_sync_visuals},
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
            .add_systems(Update, follow_robot_sync_myserver_events)
            .add_systems(PostUpdate, update_robot_sync_scene_state);
    }
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
            authority::AuthorityEndpoint,
            features::robot_sync::{
                config::{
                    DEFAULT_ROBOT_SYNC_PLAYER_ID, ROBOT_SYNC_MYSERVER_POLICY_ID,
                    RobotSyncAuthorityMode,
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
            .add_message::<MyServerCommand>()
            .add_message::<MyServerEvent>()
            .init_resource::<AuthoritySession>()
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

    fn read_messages<M>(world: &World) -> Vec<M>
    where
        M: Message + Clone,
    {
        let messages = world.resource::<Messages<M>>();
        let mut cursor = MessageCursor::default();
        cursor.read(messages).cloned().collect()
    }
}
