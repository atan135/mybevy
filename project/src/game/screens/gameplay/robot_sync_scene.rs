use bevy::{
    ecs::message::{MessageCursor, Messages},
    prelude::*,
};

use crate::framework::{
    scene::prelude::SceneEvent,
    ui::{
        core::binding::UiBindingValues,
        document::{UiBindingValue, UiBindingVisibility},
        i18n::UiI18n,
    },
};
use crate::game::{
    authority::AuthoritySession,
    features::{
        lockstep_sim::{format_lockstep_sim_hud_status, lockstep_sim_hud_snapshot},
        robot_sync::{format_robot_sync_hud_status, robot_sync_hud_snapshot},
    },
    navigation::{AppUiMode, GameRouteCommand},
    scenes::ROBOT_SYNC_ARENA_SCENE_ID,
    ui_ids::OWNER_ROBOT_SYNC_SCENE,
};

use super::host::{GameplayHudHostContract, ROBOT_SYNC_DOCUMENT_ID, set_binding};

#[derive(Clone, Copy, Debug, Default, Resource, PartialEq, Eq)]
pub(super) struct RobotSyncHudVisibility {
    pub(super) show_details: bool,
}

pub(super) fn update_robot_sync_scene_hud_status(
    config: Res<crate::game::features::robot_sync::RobotSyncConfig>,
    scene_state: Res<crate::game::features::robot_sync::RobotSyncSceneState>,
    lockstep_config: Res<crate::game::features::lockstep_sim::LockstepSimConfig>,
    lockstep_scene_state: Res<crate::game::features::lockstep_sim::LockstepSimSceneState>,
    authority_session: Res<AuthoritySession>,
    replay_state: Res<crate::game::features::robot_sync::RobotSyncReplayState>,
    lockstep_replay_state: Res<crate::game::features::lockstep_sim::LockstepSimReplayState>,
    i18n: Res<UiI18n>,
    contract: Res<GameplayHudHostContract>,
    mut values: ResMut<UiBindingValues>,
) {
    let status = robot_sync_scene_hud_status(
        &config,
        &scene_state,
        &lockstep_config,
        &lockstep_scene_state,
        &authority_session,
        &replay_state,
        &lockstep_replay_state,
    );
    let title = if lockstep_scene_state.is_active() {
        "Lockstep Sim".to_owned()
    } else {
        i18n.tr("robot_sync_scene.hud.title", "Robot Sync")
    };

    set_binding(
        &contract,
        &mut values,
        ROBOT_SYNC_DOCUMENT_ID,
        OWNER_ROBOT_SYNC_SCENE.as_str(),
        "robot_sync.title",
        UiBindingValue::String(title),
    );
    set_binding(
        &contract,
        &mut values,
        ROBOT_SYNC_DOCUMENT_ID,
        OWNER_ROBOT_SYNC_SCENE.as_str(),
        "robot_sync.status",
        UiBindingValue::String(status),
    );
}

pub(super) fn sync_robot_sync_hud_visibility_bindings(
    visibility: Res<RobotSyncHudVisibility>,
    contract: Res<GameplayHudHostContract>,
    mut values: ResMut<UiBindingValues>,
) {
    for (path, visible) in [
        ("robot_sync.details_visibility", visibility.show_details),
        ("robot_sync.hide_visibility", visibility.show_details),
        ("robot_sync.show_visibility", !visibility.show_details),
    ] {
        set_binding(
            &contract,
            &mut values,
            ROBOT_SYNC_DOCUMENT_ID,
            OWNER_ROBOT_SYNC_SCENE.as_str(),
            path,
            UiBindingValue::Visibility(if visible {
                UiBindingVisibility::Visible
            } else {
                UiBindingVisibility::Hidden
            }),
        );
    }
}

fn robot_sync_scene_hud_status(
    config: &crate::game::features::robot_sync::RobotSyncConfig,
    scene_state: &crate::game::features::robot_sync::RobotSyncSceneState,
    lockstep_config: &crate::game::features::lockstep_sim::LockstepSimConfig,
    lockstep_scene_state: &crate::game::features::lockstep_sim::LockstepSimSceneState,
    authority_session: &AuthoritySession,
    replay_state: &crate::game::features::robot_sync::RobotSyncReplayState,
    lockstep_replay_state: &crate::game::features::lockstep_sim::LockstepSimReplayState,
) -> String {
    if lockstep_scene_state.is_active() {
        format_lockstep_sim_hud_status(&lockstep_sim_hud_snapshot(
            lockstep_config,
            lockstep_scene_state,
            authority_session,
            lockstep_replay_state,
        ))
    } else {
        format_robot_sync_hud_status(&robot_sync_hud_snapshot(
            config,
            scene_state,
            authority_session,
            replay_state,
        ))
    }
}

pub(super) fn route_to_lobby_on_robot_sync_scene_exit(
    mut scene_events: MessageReader<SceneEvent>,
    current_mode: Res<State<AppUiMode>>,
    mut route_cursor: Local<MessageCursor<GameRouteCommand>>,
    mut route_messages: ResMut<Messages<GameRouteCommand>>,
) {
    let already_routing_to_lobby = route_cursor
        .read(&route_messages)
        .any(is_lobby_route_command);
    let robot_sync_scene_exited = scene_events.read().any(|event| {
        matches!(
            event,
            SceneEvent::Exited(exited) if exited.scene_id.as_str() == ROBOT_SYNC_ARENA_SCENE_ID
        )
    });

    if should_route_robot_sync_scene_exit_to_lobby(*current_mode.get(), already_routing_to_lobby)
        && robot_sync_scene_exited
    {
        route_messages.write(GameRouteCommand::ChangeMode(AppUiMode::Lobby));
    }
}

fn should_route_robot_sync_scene_exit_to_lobby(
    current_mode: AppUiMode,
    already_routing_to_lobby: bool,
) -> bool {
    current_mode == AppUiMode::RobotSyncScene && !already_routing_to_lobby
}

fn is_lobby_route_command(command: &GameRouteCommand) -> bool {
    matches!(command, GameRouteCommand::ChangeMode(AppUiMode::Lobby))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::scene::prelude::{SceneEntered, SceneExited, SceneId, SceneSessionId};
    use crate::game::features::{
        lockstep_sim::LockstepSimPlugin,
        robot_sync::{RobotSyncConfig, RobotSyncReplayState, RobotSyncSceneState},
    };

    #[test]
    fn hud_status_uses_lockstep_snapshot_when_lockstep_scene_is_active() {
        let mut app = App::new();
        app.add_message::<SceneEvent>()
            .add_message::<crate::game::authority::AuthorityCommand>()
            .add_message::<crate::game::authority::AuthorityEvent>()
            .add_message::<crate::game::myserver::MyServerCommand>()
            .add_message::<crate::game::myserver::MyServerEvent>()
            .init_resource::<AuthoritySession>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_plugins(LockstepSimPlugin);
        app.world_mut()
            .write_message(SceneEvent::Entered(SceneEntered {
                scene_id: SceneId::from(crate::game::scenes::LOCKSTEP_SIM_ARENA_SCENE_ID),
                session_id: SceneSessionId::from("lockstep-session"),
                content_version: None,
            }));
        app.update();

        let status = robot_sync_scene_hud_status(
            &RobotSyncConfig::default(),
            &RobotSyncSceneState::default(),
            app.world()
                .resource::<crate::game::features::lockstep_sim::LockstepSimConfig>(),
            app.world()
                .resource::<crate::game::features::lockstep_sim::LockstepSimSceneState>(),
            app.world().resource::<AuthoritySession>(),
            &RobotSyncReplayState::default(),
            app.world()
                .resource::<crate::game::features::lockstep_sim::LockstepSimReplayState>(),
        );

        assert!(status.contains("policy=lockstep_sim_demo"));
        assert!(status.contains("local_hash="));
    }

    #[test]
    fn hud_status_keeps_robot_sync_snapshot_when_lockstep_scene_is_inactive() {
        let status = robot_sync_scene_hud_status(
            &RobotSyncConfig::default(),
            &RobotSyncSceneState::default(),
            &crate::game::features::lockstep_sim::LockstepSimConfig::default(),
            &crate::game::features::lockstep_sim::LockstepSimSceneState::default(),
            &AuthoritySession::default(),
            &RobotSyncReplayState::default(),
            &crate::game::features::lockstep_sim::LockstepSimReplayState::default(),
        );

        assert!(status.contains("robots="));
        assert!(!status.contains("policy="));
    }

    #[test]
    fn robot_sync_scene_exit_fallback_only_routes_while_hud_is_active() {
        assert!(should_route_robot_sync_scene_exit_to_lobby(
            AppUiMode::RobotSyncScene,
            false
        ));
        assert!(!should_route_robot_sync_scene_exit_to_lobby(
            AppUiMode::RobotSyncScene,
            true
        ));
        assert!(!should_route_robot_sync_scene_exit_to_lobby(
            AppUiMode::Lobby,
            false
        ));
    }

    #[test]
    fn robot_sync_scene_exit_fallback_ignores_other_scene_ids() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<AppUiMode>()
            .add_message::<SceneEvent>()
            .add_message::<GameRouteCommand>()
            .add_systems(Update, route_to_lobby_on_robot_sync_scene_exit);
        app.world_mut()
            .resource_mut::<NextState<AppUiMode>>()
            .set(AppUiMode::RobotSyncScene);
        app.update();

        app.world_mut()
            .write_message(SceneEvent::Exited(SceneExited {
                scene_id: SceneId::from("sample.dungeon_room"),
                session_id: SceneSessionId::from("sample-session"),
            }));
        app.update();
        assert!(read_messages::<GameRouteCommand>(app.world()).is_empty());

        app.world_mut()
            .write_message(SceneEvent::Exited(SceneExited {
                scene_id: SceneId::from(ROBOT_SYNC_ARENA_SCENE_ID),
                session_id: SceneSessionId::from("robot-sync-session"),
            }));
        app.update();
        assert!(matches!(
            read_messages::<GameRouteCommand>(app.world()).last(),
            Some(GameRouteCommand::ChangeMode(AppUiMode::Lobby))
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
