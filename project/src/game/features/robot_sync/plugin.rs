use bevy::prelude::*;

use crate::framework::scene::prelude::SceneEvent;

use super::{
    bot::{RobotSyncBotState, clear_robot_sync_bots},
    config::RobotSyncConfig,
    state::RobotSyncSceneState,
    sync::{RobotSyncReplayState, reset_robot_sync_replay},
    visual::{RobotSyncVisualState, clear_robot_sync_visuals},
};

pub(in crate::game) struct RobotSyncPlugin;

impl Plugin for RobotSyncPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RobotSyncConfig>()
            .init_resource::<RobotSyncSceneState>()
            .init_resource::<RobotSyncBotState>()
            .init_resource::<RobotSyncReplayState>()
            .init_resource::<RobotSyncVisualState>()
            .add_systems(PostUpdate, update_robot_sync_scene_state);
    }
}

fn update_robot_sync_scene_state(
    config: Res<RobotSyncConfig>,
    mut scene_state: ResMut<RobotSyncSceneState>,
    mut bot_state: ResMut<RobotSyncBotState>,
    mut replay_state: ResMut<RobotSyncReplayState>,
    mut visual_state: ResMut<RobotSyncVisualState>,
    mut scene_events: MessageReader<SceneEvent>,
) {
    for event in scene_events.read() {
        match event {
            SceneEvent::Entered(entered) if config.is_robot_sync_scene(&entered.scene_id) => {
                scene_state.activate(entered.scene_id.clone(), entered.session_id.clone());
            }
            SceneEvent::Exited(exited)
                if config.is_robot_sync_scene(&exited.scene_id)
                    && scene_state.is_active_session(&exited.session_id) =>
            {
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
        framework::scene::prelude::{SceneEntered, SceneExited, SceneId, SceneSessionId},
        game::scenes::ROBOT_SYNC_ARENA_SCENE_ID,
    };

    fn test_app() -> App {
        let mut app = App::new();
        app.add_message::<SceneEvent>().add_plugins(RobotSyncPlugin);
        app
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
            *app.world().resource::<RobotSyncVisualState>(),
            RobotSyncVisualState::default()
        );
    }

    #[test]
    fn robot_sync_scene_entered_activates_scene_state() {
        let mut app = test_app();
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
    }

    #[test]
    fn matching_robot_sync_scene_exited_clears_active_and_module_state() {
        let mut app = test_app();
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
}
