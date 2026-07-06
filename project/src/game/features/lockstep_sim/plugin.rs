use bevy::prelude::*;
use sim_core::SimCommand;

use crate::{
    framework::scene::prelude::SceneEvent,
    game::{
        authority::{AuthorityCommand, AuthoritySession},
        myserver::MyServerCommand,
    },
};

use super::{
    combat_events::{
        LockstepSimCombatEventState, LockstepSimCombatEventVisual,
        despawn_lockstep_sim_combat_event_visuals, sync_lockstep_sim_combat_event_visuals,
        update_lockstep_sim_combat_events,
    },
    config::LockstepSimConfig,
    input::{
        LockstepSimInputError, LockstepSimInputIntent, LockstepSimInputSeq, quantize_keyboard_axis,
    },
    payload::{
        LockstepSimInputGateError, LockstepSimPayloadError, build_sim_input_envelope,
        gate_lockstep_sim_input, log_sim_input_send,
    },
    replay::{
        LockstepSimReplayState, apply_lockstep_sim_authority_events, reset_lockstep_sim_replay,
    },
    state::LockstepSimSceneState,
    sync::{
        LockstepSimMyServerJoinState, cleanup_lockstep_sim_authority,
        follow_lockstep_sim_myserver_events, start_lockstep_sim_authority,
    },
    visual::{
        LockstepSimEntityVisual, LockstepSimVisualState, clear_lockstep_sim_visuals,
        despawn_lockstep_sim_visual_entities, sync_lockstep_sim_entity_visuals,
    },
};

pub(in crate::game) struct LockstepSimPlugin;

#[derive(Clone, Copy, Debug, Default, Resource, PartialEq, Eq)]
pub(in crate::game::features::lockstep_sim) struct LockstepSimInputSendState {
    last_sent_target_frame: Option<u32>,
    last_sent_command: Option<SimCommand>,
}

impl LockstepSimInputSendState {
    fn should_send(self, target_frame: u32, command: SimCommand) -> bool {
        if command == SimCommand::Stop && self.last_sent_command == Some(SimCommand::Stop) {
            return false;
        }

        self.last_sent_target_frame != Some(target_frame) || self.last_sent_command != Some(command)
    }

    fn mark_sent(&mut self, target_frame: u32, command: SimCommand) {
        self.last_sent_target_frame = Some(target_frame);
        self.last_sent_command = Some(command);
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

impl Plugin for LockstepSimPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LockstepSimConfig>()
            .init_resource::<LockstepSimSceneState>()
            .init_resource::<LockstepSimInputSeq>()
            .init_resource::<LockstepSimInputSendState>()
            .init_resource::<LockstepSimReplayState>()
            .init_resource::<LockstepSimVisualState>()
            .init_resource::<LockstepSimCombatEventState>()
            .init_resource::<LockstepSimMyServerJoinState>()
            .add_systems(
                Update,
                (
                    follow_lockstep_sim_myserver_events,
                    apply_lockstep_sim_authority_events,
                    sync_lockstep_sim_entity_visuals,
                    update_lockstep_sim_combat_events,
                    sync_lockstep_sim_combat_event_visuals,
                    send_local_lockstep_sim_input,
                )
                    .chain(),
            )
            .add_systems(PostUpdate, update_lockstep_sim_scene_state);
    }
}

fn send_local_lockstep_sim_input(
    scene_state: Res<LockstepSimSceneState>,
    authority_session: Res<AuthoritySession>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut seq: ResMut<LockstepSimInputSeq>,
    mut send_state: ResMut<LockstepSimInputSendState>,
    mut authority_commands: MessageWriter<AuthorityCommand>,
) {
    if !scene_state.active {
        return;
    }

    let command = match manual_lockstep_sim_command(&keyboard_input) {
        Ok(command) => command,
        Err(error) => {
            warn!(?error, "lockstep sim manual input ignored");
            return;
        }
    };
    let target_frame = authority_session.frame_id.saturating_add(1);
    if !send_state.should_send(target_frame, command) {
        return;
    }

    let context = match gate_lockstep_sim_input(
        &scene_state,
        authority_session.local_player_id.as_deref(),
        None,
        None,
        Some(sim_core::SIM_CORE_SCHEMA_VERSION),
    ) {
        Ok(context) => context,
        Err(
            LockstepSimInputGateError::InactiveScene
            | LockstepSimInputGateError::MissingInitialSnapshot,
        ) => {
            return;
        }
        Err(error) => {
            warn!(reason = %error, "lockstep sim input blocked");
            return;
        }
    };
    let seq = seq.next();
    let envelope = match build_sim_input_envelope(target_frame, seq, &[command]) {
        Ok(envelope) => envelope,
        Err(LockstepSimPayloadError::EmptyCommands) => return,
        Err(error) => {
            warn!(reason = %error, "lockstep sim input payload rejected");
            return;
        }
    };

    log_sim_input_send(
        &context.character_id,
        envelope.frame_id,
        envelope.seq,
        &envelope.command_summaries,
    );
    authority_commands.write(envelope.into_authority_command());
    send_state.mark_sent(target_frame, command);
}

fn manual_lockstep_sim_command(
    keyboard_input: &ButtonInput<KeyCode>,
) -> Result<SimCommand, LockstepSimInputError> {
    let x = pressed_axis(
        keyboard_input,
        [KeyCode::KeyA, KeyCode::ArrowLeft],
        [KeyCode::KeyD, KeyCode::ArrowRight],
    );
    let y = pressed_axis(
        keyboard_input,
        [KeyCode::KeyW, KeyCode::ArrowUp],
        [KeyCode::KeyS, KeyCode::ArrowDown],
    );

    if x == 0 && y == 0 {
        return Ok(SimCommand::Stop);
    }

    Ok(LockstepSimInputIntent::Move {
        dir: quantize_keyboard_axis(x, y)?,
        speed_per_second: None,
    }
    .into_sim_command())
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

fn update_lockstep_sim_scene_state(
    config: Res<LockstepSimConfig>,
    authority_session: Res<AuthoritySession>,
    mut scene_state: ResMut<LockstepSimSceneState>,
    mut input_seq: ResMut<LockstepSimInputSeq>,
    mut input_send_state: ResMut<LockstepSimInputSendState>,
    mut replay_state: ResMut<LockstepSimReplayState>,
    mut visual_state: ResMut<LockstepSimVisualState>,
    mut combat_event_state: ResMut<LockstepSimCombatEventState>,
    mut join_state: ResMut<LockstepSimMyServerJoinState>,
    visual_entities: Query<Entity, With<LockstepSimEntityVisual>>,
    combat_event_visual_entities: Query<Entity, With<LockstepSimCombatEventVisual>>,
    mut commands: Commands,
    mut scene_events: MessageReader<SceneEvent>,
    mut authority_commands: MessageWriter<AuthorityCommand>,
    mut myserver_commands: MessageWriter<MyServerCommand>,
) {
    for event in scene_events.read() {
        match event {
            SceneEvent::Entered(entered) if config.is_lockstep_sim_scene(&entered.scene_id) => {
                input_seq.reset();
                input_send_state.reset();
                reset_lockstep_sim_replay(&mut replay_state);
                clear_lockstep_sim_visuals(&mut visual_state);
                despawn_lockstep_sim_combat_event_visuals(
                    &mut commands,
                    &mut combat_event_state,
                    combat_event_visual_entities.iter(),
                );
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
                input_seq.reset();
                input_send_state.reset();
                reset_lockstep_sim_replay(&mut replay_state);
                despawn_lockstep_sim_combat_event_visuals(
                    &mut commands,
                    &mut combat_event_state,
                    combat_event_visual_entities.iter(),
                );
                despawn_lockstep_sim_visual_entities(
                    &mut commands,
                    &mut visual_state,
                    visual_entities.iter(),
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
            authority::{AuthorityEndpoint, AuthorityEvent, AuthorityFrame, AuthoritySnapshot},
            features::lockstep_sim::{
                config::{
                    DEFAULT_LOCKSTEP_SIM_PLAYER_ID, LOCKSTEP_SIM_MYSERVER_POLICY_ID,
                    LockstepSimAuthorityMode,
                },
                diagnostics::LockstepSimHashMatchStatus,
                sync::LockstepSimMyServerJoinState,
            },
            myserver::{MyServerEvent, protocol::pb},
            scenes::{LOCKSTEP_SIM_ARENA_SCENE_ID, ROBOT_SYNC_ARENA_SCENE_ID},
        },
    };
    use bevy::ecs::message::{MessageCursor, Messages};
    use serde_json::json;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_message::<SceneEvent>()
            .add_message::<AuthorityCommand>()
            .add_message::<AuthorityEvent>()
            .add_message::<MyServerCommand>()
            .add_message::<MyServerEvent>()
            .init_resource::<AuthoritySession>()
            .init_resource::<ButtonInput<KeyCode>>()
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
            debug_diagnostics: false,
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

    #[test]
    fn active_lockstep_with_snapshot_sends_sim_input_payload() {
        let mut app = active_lockstep_app();
        {
            let mut session = app.world_mut().resource_mut::<AuthoritySession>();
            session.local_player_id = Some("lockstep-player".to_string());
            session.frame_id = 12;
        }
        app.world_mut()
            .resource_mut::<LockstepSimSceneState>()
            .initial_snapshot = Some(parsed_snapshot_for_player("lockstep-player", 1000));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyD);

        app.update();

        let authority_commands = read_messages::<AuthorityCommand>(app.world());
        assert!(authority_commands.iter().any(|command| {
            matches!(
                command,
                AuthorityCommand::SendInput {
                    frame_id: 13,
                    action,
                    payload_json
                } if action == "sim_input"
                    && serde_json::from_str::<serde_json::Value>(payload_json).is_ok_and(|payload| {
                        payload["version"] == 1
                            && payload["seq"] == 0
                            && payload["commands"][0]["type"] == "move"
                            && payload["commands"][0]["dirX"] == 1000
                            && payload["commands"][0]["dirY"] == 0
                            && payload["commands"][0].get("entityId").is_none()
                    })
            )
        }));
    }

    #[test]
    fn active_lockstep_without_control_binding_does_not_send_sim_input() {
        let mut app = active_lockstep_app();
        app.world_mut()
            .resource_mut::<AuthoritySession>()
            .local_player_id = Some("missing-player".to_string());
        app.world_mut()
            .resource_mut::<LockstepSimSceneState>()
            .initial_snapshot = Some(parsed_snapshot_for_player("lockstep-player", 1000));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyD);

        app.update();

        assert!(
            read_messages::<AuthorityCommand>(app.world())
                .iter()
                .all(|command| !matches!(command, AuthorityCommand::SendInput { .. }))
        );
    }

    #[test]
    fn frame_bundle_snapshot_does_not_reset_live_replay_or_skip_authority_frame() {
        use sim_core::{FrameId, step};

        let mut app = active_lockstep_app();
        let snapshot = parsed_snapshot_for_player("lockstep-player", 1000);
        let mut offline_world = snapshot.world.clone();
        let frame1_result = step(&mut offline_world, FrameId::new(1), &[], &snapshot.config)
            .expect("frame 1 offline step should match replay config");
        let frame2_result = step(&mut offline_world, FrameId::new(2), &[], &snapshot.config)
            .expect("frame 2 offline step should match replay config");
        {
            let mut scene_state = app.world_mut().resource_mut::<LockstepSimSceneState>();
            scene_state.replace_initial_snapshot(snapshot);
        }

        app.world_mut()
            .write_message(MyServerEvent::FrameBundlePush(lockstep_frame_bundle_push(
                1,
                &game_state_with_initial_snapshot_marker(1, frame1_result.state_hash.value),
            )));
        app.world_mut().write_message(AuthorityEvent::FrameApplied {
            frame: lockstep_authority_frame(
                1,
                &game_state_with_initial_snapshot_marker(1, frame1_result.state_hash.value),
            ),
        });
        app.update();

        app.world_mut()
            .write_message(MyServerEvent::FrameBundlePush(lockstep_frame_bundle_push(
                2,
                &game_state_with_initial_snapshot_marker(2, frame2_result.state_hash.value),
            )));
        app.world_mut().write_message(AuthorityEvent::FrameApplied {
            frame: lockstep_authority_frame(
                2,
                &game_state_with_initial_snapshot_marker(2, frame2_result.state_hash.value),
            ),
        });
        app.update();

        let scene_state = app.world().resource::<LockstepSimSceneState>();
        assert_eq!(scene_state.snapshot_generation, 1);
        assert_eq!(
            scene_state
                .initial_snapshot
                .as_ref()
                .map(|snapshot| snapshot.start_frame),
            Some(0)
        );
        assert!(scene_state.initial_snapshot_error.is_none());

        let replay = app.world().resource::<LockstepSimReplayState>();
        assert_eq!(replay.snapshot_generation, 1);
        assert_eq!(replay.snapshot_start_frame, Some(0));
        assert_eq!(replay.last_applied_frame, Some(2));
        assert_eq!(replay.hash_history.len(), 2);
        assert_eq!(replay.hash_history.back().unwrap().frame, 2);
        assert_eq!(
            replay.hash_history.back().unwrap().local_hash,
            frame2_result.state_hash
        );
        assert_eq!(replay.event_history.len(), 2);
        assert_eq!(
            replay.diagnostics.last_match_status,
            LockstepSimHashMatchStatus::Matched
        );
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

    fn parsed_snapshot_for_player(
        player_id: &str,
        entity_id: u32,
    ) -> super::super::snapshot::ParsedInitialSnapshot {
        use std::collections::HashMap;

        use super::super::snapshot::SimHashEnvelope;
        use sim_core::{
            CombatConfig, CombatState, EntityId, EntityKind, Fp, FrameId, MovementConfig,
            MovementMode, MovementState, QuantizedDir, SceneBounds, SimConfig, SimEntity,
            SimRngState, SimTransform, SimWorld, TeamId, Vec2Fp,
        };

        let entity = SimEntity {
            id: EntityId::new(entity_id),
            kind: EntityKind::Player,
            owner_character_id: Some(player_id.to_string()),
            team_id: TeamId::new(1),
            transform: SimTransform {
                pos: Vec2Fp::zero(),
                facing: QuantizedDir::RIGHT,
                radius: Fp::from_milli(500),
            },
            movement: MovementState {
                mode: MovementMode::Idle,
                move_dir: QuantizedDir::ZERO,
                speed_per_second: Fp::ZERO,
            },
            combat: CombatState::default(),
            alive: true,
        };
        let world = SimWorld::with_rng(
            FrameId::new(0),
            SimRngState {
                seed: 1,
                counter: 0,
            },
            vec![entity.clone()],
        )
        .unwrap();
        let mut control_bindings = HashMap::new();
        control_bindings.insert(player_id.to_string(), EntityId::new(entity_id));

        super::super::snapshot::ParsedInitialSnapshot {
            room_id: "lockstep-room".to_string(),
            start_frame: 0,
            tick_rate: 20,
            config_version: 1,
            config_hash: "test-config-hash".to_string(),
            sim_schema_version: sim_core::SIM_CORE_SCHEMA_VERSION,
            rng_seed: 1,
            state_hash: SimHashEnvelope {
                frame: 0,
                value: 0,
                hex: "0000000000000000".to_string(),
            },
            world,
            config: SimConfig {
                movement: MovementConfig {
                    tick_rate: 20,
                    default_speed_per_second: Fp::from_i32(6),
                    max_speed_per_second: Fp::from_i32(12),
                    bounds: SceneBounds {
                        min: Vec2Fp::new(Fp::from_i32(-100), Fp::from_i32(-100)),
                        max: Vec2Fp::new(Fp::from_i32(100), Fp::from_i32(100)),
                    },
                    static_obstacles: Vec::new(),
                },
                combat: CombatConfig::default(),
            },
            control_bindings,
            entities: vec![entity],
        }
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

    fn lockstep_frame_bundle_push(frame_id: u32, game_state: &str) -> pb::FrameBundlePush {
        pb::FrameBundlePush {
            room_id: "lockstep-room".to_string(),
            frame_id,
            fps: 20,
            inputs: Vec::new(),
            is_silent_frame: false,
            snapshot: Some(pb::RoomSnapshot {
                room_id: "lockstep-room".to_string(),
                owner_character_id: "lockstep-player".to_string(),
                state: "in_game".to_string(),
                members: vec![pb::RoomMember {
                    character_id: "lockstep-player".to_string(),
                    ready: true,
                    is_owner: true,
                    offline: false,
                    role: pb::MemberRole::Player as i32,
                }],
                current_frame_id: frame_id,
                game_state: game_state.to_string(),
            }),
        }
    }

    fn lockstep_authority_frame(frame_id: u32, game_state_json: &str) -> AuthorityFrame {
        AuthorityFrame {
            authority_epoch: 1,
            frame_id,
            fps: 20,
            inputs: Vec::new(),
            snapshot: AuthoritySnapshot {
                authority_epoch: 1,
                frame_id,
                authority_player_id: "lockstep-player".to_string(),
                players: vec!["lockstep-player".to_string()],
                game_state_json: game_state_json.to_string(),
            },
        }
    }

    fn game_state_with_initial_snapshot_marker(frame_id: u32, state_hash: u64) -> String {
        json!({
            "initialSnapshot": {
                "schema": super::super::snapshot::SIM_INITIAL_SNAPSHOT_SCHEMA
            },
            "lastStateHash": {
                "frame": frame_id,
                "value": state_hash,
                "hex": format!("{state_hash:016x}")
            }
        })
        .to_string()
    }

    fn lockstep_room_state_push(room_id: &str, player_id: &str, ready: bool) -> pb::RoomStatePush {
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
