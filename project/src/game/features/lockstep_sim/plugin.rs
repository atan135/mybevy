use bevy::prelude::*;

use crate::{
    framework::scene::prelude::SceneEvent,
    game::{
        authority::{AuthorityCommand, AuthoritySession},
        myserver::MyServerCommand,
    },
};

use super::{
    config::LockstepSimConfig,
    input::LockstepSimInputSeq,
    state::LockstepSimSceneState,
    sync::{
        LockstepSimMyServerJoinState, cleanup_lockstep_sim_authority,
        follow_lockstep_sim_myserver_events, start_lockstep_sim_authority,
    },
};

pub(in crate::game) struct LockstepSimPlugin;

impl Plugin for LockstepSimPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LockstepSimConfig>()
            .init_resource::<LockstepSimSceneState>()
            .init_resource::<LockstepSimInputSeq>()
            .init_resource::<LockstepSimMyServerJoinState>()
            .add_systems(Update, follow_lockstep_sim_myserver_events)
            .add_systems(PostUpdate, update_lockstep_sim_scene_state);
    }
}

fn update_lockstep_sim_scene_state(
    config: Res<LockstepSimConfig>,
    authority_session: Res<AuthoritySession>,
    mut scene_state: ResMut<LockstepSimSceneState>,
    mut join_state: ResMut<LockstepSimMyServerJoinState>,
    mut scene_events: MessageReader<SceneEvent>,
    mut authority_commands: MessageWriter<AuthorityCommand>,
    mut myserver_commands: MessageWriter<MyServerCommand>,
) {
    for event in scene_events.read() {
        match event {
            SceneEvent::Entered(entered) if config.is_lockstep_sim_scene(&entered.scene_id) => {
                scene_state.activate(entered.scene_id.clone(), entered.session_id.clone());
                start_lockstep_sim_authority(
                    &config,
                    &authority_session,
                    &mut join_state,
                    &mut authority_commands,
                    &mut myserver_commands,
                );
            }
            SceneEvent::Exited(exited)
                if config.is_lockstep_sim_scene(&exited.scene_id)
                    && scene_state.is_active_session(&exited.session_id) =>
            {
                cleanup_lockstep_sim_authority(
                    &config,
                    &mut join_state,
                    &mut authority_commands,
                    &mut myserver_commands,
                );
                scene_state.reset();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        framework::{
            network::NetworkTransport,
            scene::prelude::{SceneEntered, SceneExited, SceneId, SceneSessionId},
        },
        game::{
            authority::{AuthorityEndpoint, AuthorityEvent},
            features::lockstep_sim::{
                config::{
                    DEFAULT_LOCKSTEP_SIM_PLAYER_ID, LOCKSTEP_SIM_MYSERVER_POLICY_ID,
                    LockstepSimAuthorityMode,
                },
                sync::LockstepSimMyServerJoinState,
            },
            myserver::{MyServerEvent, protocol::pb},
            scenes::{LOCKSTEP_SIM_ARENA_SCENE_ID, ROBOT_SYNC_ARENA_SCENE_ID},
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
            .add_plugins(LockstepSimPlugin);
        app
    }

    fn test_config(authority_mode: LockstepSimAuthorityMode) -> LockstepSimConfig {
        LockstepSimConfig {
            scene_id: SceneId::from(LOCKSTEP_SIM_ARENA_SCENE_ID),
            local_player_id: DEFAULT_LOCKSTEP_SIM_PLAYER_ID.to_string(),
            authority_mode,
            transport: NetworkTransport::Tcp,
            myserver_guest_id: Some("lockstep-guest".to_string()),
            myserver_room_id: "lockstep-room".to_string(),
            myserver_policy_id: LOCKSTEP_SIM_MYSERVER_POLICY_ID.to_string(),
        }
    }

    #[test]
    fn lockstep_sim_scene_entered_myserver_mode_joins_and_logs_in() {
        let mut app = test_app();
        app.insert_resource(test_config(LockstepSimAuthorityMode::MyServer));

        app.world_mut()
            .write_message(SceneEvent::Entered(SceneEntered {
                scene_id: SceneId::from(LOCKSTEP_SIM_ARENA_SCENE_ID),
                session_id: SceneSessionId::from("lockstep-session"),
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
            } if player_id == DEFAULT_LOCKSTEP_SIM_PLAYER_ID
        ));

        let myserver_commands = read_messages::<MyServerCommand>(app.world());
        assert_eq!(myserver_commands.len(), 1);
        assert!(matches!(
            &myserver_commands[0],
            MyServerCommand::GuestLogin {
                guest_id: Some(guest_id),
                connect_game: true
            } if guest_id == "lockstep-guest"
        ));
    }

    #[test]
    fn authenticated_joins_lockstep_policy_room() {
        let mut app = active_lockstep_app();

        app.world_mut().write_message(MyServerEvent::Authenticated {
            player_id: "lockstep-player".to_string(),
        });
        app.update();

        let myserver_commands = read_messages::<MyServerCommand>(app.world());
        assert!(matches!(
            myserver_commands.last(),
            Some(MyServerCommand::JoinRoom { room_id, policy_id })
                if room_id == "lockstep-room" && policy_id == LOCKSTEP_SIM_MYSERVER_POLICY_ID
        ));
    }

    #[test]
    fn room_joined_ok_sends_ready() {
        let mut app = active_lockstep_app();
        authenticate(&mut app);

        app.world_mut()
            .write_message(MyServerEvent::RoomJoined(pb::RoomJoinRes {
                ok: true,
                room_id: "lockstep-room".to_string(),
                error_code: String::new(),
            }));
        app.update();

        let myserver_commands = read_messages::<MyServerCommand>(app.world());
        assert!(matches!(
            myserver_commands.last(),
            Some(MyServerCommand::SetReady { ready: true })
        ));
    }

    #[test]
    fn ready_ok_starts_room_without_second_player() {
        let mut app = active_lockstep_app();
        authenticate(&mut app);
        join_room(&mut app);

        app.world_mut()
            .write_message(MyServerEvent::ReadyChanged(pb::RoomReadyRes {
                ok: true,
                room_id: "lockstep-room".to_string(),
                ready: true,
                error_code: String::new(),
            }));
        app.update();

        let myserver_commands = read_messages::<MyServerCommand>(app.world());
        assert!(matches!(
            myserver_commands.last(),
            Some(MyServerCommand::StartRoom)
        ));
        assert!(
            app.world()
                .resource::<LockstepSimMyServerJoinState>()
                .start_sent
        );
    }

    #[test]
    fn local_ready_room_state_push_can_start_room() {
        let mut app = active_lockstep_app();
        authenticate(&mut app);
        join_room(&mut app);

        app.world_mut()
            .write_message(MyServerEvent::RoomStatePush(lockstep_room_state_push(
                "lockstep-room",
                "lockstep-player",
                true,
            )));
        app.update();

        let myserver_commands = read_messages::<MyServerCommand>(app.world());
        assert!(matches!(
            myserver_commands.last(),
            Some(MyServerCommand::StartRoom)
        ));
    }

    #[test]
    fn lockstep_sim_exited_cleans_join_state_and_sends_leave_disconnect() {
        let mut app = active_lockstep_app();
        let session_id = SceneSessionId::from("lockstep-session");
        {
            let mut join_state = app
                .world_mut()
                .resource_mut::<LockstepSimMyServerJoinState>();
            join_state.join_sent = true;
            join_state.ready_sent = true;
        }

        app.world_mut()
            .write_message(SceneEvent::Exited(SceneExited {
                scene_id: SceneId::from(LOCKSTEP_SIM_ARENA_SCENE_ID),
                session_id,
            }));
        app.update();

        assert_eq!(
            *app.world().resource::<LockstepSimSceneState>(),
            LockstepSimSceneState::default()
        );
        assert_eq!(
            *app.world().resource::<LockstepSimMyServerJoinState>(),
            LockstepSimMyServerJoinState::default()
        );
        assert!(matches!(
            read_messages::<AuthorityCommand>(app.world()).last(),
            Some(AuthorityCommand::Leave)
        ));
        assert!(matches!(
            read_messages::<MyServerCommand>(app.world()).last(),
            Some(MyServerCommand::Disconnect)
        ));
    }

    #[test]
    fn robot_sync_scene_does_not_activate_lockstep_sim() {
        let mut app = test_app();
        app.insert_resource(test_config(LockstepSimAuthorityMode::MyServer));

        app.world_mut()
            .write_message(SceneEvent::Entered(SceneEntered {
                scene_id: SceneId::from(ROBOT_SYNC_ARENA_SCENE_ID),
                session_id: SceneSessionId::from("robot-session"),
                content_version: None,
            }));
        app.update();

        assert_eq!(
            *app.world().resource::<LockstepSimSceneState>(),
            LockstepSimSceneState::default()
        );
        assert!(read_messages::<AuthorityCommand>(app.world()).is_empty());
        assert!(read_messages::<MyServerCommand>(app.world()).is_empty());
    }

    fn active_lockstep_app() -> App {
        let mut app = test_app();
        app.insert_resource(test_config(LockstepSimAuthorityMode::MyServer));
        app.world_mut()
            .write_message(SceneEvent::Entered(SceneEntered {
                scene_id: SceneId::from(LOCKSTEP_SIM_ARENA_SCENE_ID),
                session_id: SceneSessionId::from("lockstep-session"),
                content_version: None,
            }));
        app.update();
        app
    }

    fn authenticate(app: &mut App) {
        app.world_mut().write_message(MyServerEvent::Authenticated {
            player_id: "lockstep-player".to_string(),
        });
        app.update();
    }

    fn join_room(app: &mut App) {
        app.world_mut()
            .write_message(MyServerEvent::RoomJoined(pb::RoomJoinRes {
                ok: true,
                room_id: "lockstep-room".to_string(),
                error_code: String::new(),
            }));
        app.update();
    }

    fn lockstep_room_state_push(
        room_id: &str,
        player_id: &str,
        ready: bool,
    ) -> pb::RoomStatePush {
        pb::RoomStatePush {
            event: "ready_changed".to_string(),
            snapshot: Some(pb::RoomSnapshot {
                room_id: room_id.to_string(),
                owner_character_id: player_id.to_string(),
                state: "ready".to_string(),
                members: vec![pb::RoomMember {
                    character_id: player_id.to_string(),
                    ready,
                    is_owner: true,
                    offline: false,
                    role: pb::MemberRole::Player as i32,
                }],
                current_frame_id: 0,
                game_state: "{}".to_string(),
            }),
        }
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
