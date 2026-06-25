mod game_list;

use bevy::prelude::*;

use crate::framework::{
    scene::prelude::SceneEvent,
    ui::{
        core::UiPanelSystems,
        i18n::UiI18n,
        overlays::{UiOverlayCommand, UiToast},
    },
};
use crate::game::{
    navigation::{AppUiMode, GameRouteCommand},
    scenes::{FANGYUAN_HOME_SCENE_ID, ROBOT_SYNC_ARENA_SCENE_ID, SAMPLE_DUNGEON_ROOM_SCENE_ID},
};

pub(super) struct LobbyScreensPlugin;

impl Plugin for LobbyScreensPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<game_list::SampleDungeonRoomEntryState>()
            .init_resource::<game_list::RobotSyncArenaEntryState>()
            .init_resource::<game_list::FangyuanHomeEntryState>()
            .add_systems(
                OnEnter(AppUiMode::Lobby),
                (
                    reset_sample_dungeon_room_entry_state,
                    reset_robot_sync_arena_entry_state,
                    reset_fangyuan_home_entry_state,
                    game_list::setup_game_list_screen,
                ),
            )
            .add_systems(
                Update,
                game_list::handle_game_list_buttons
                    .before(UiPanelSystems::Commands)
                    .run_if(in_state(AppUiMode::Lobby)),
            )
            .add_systems(
                Update,
                handle_lobby_scene_entry_events.before(UiPanelSystems::Commands),
            );
    }
}

fn handle_lobby_scene_entry_events(
    i18n: Res<UiI18n>,
    mut scene_events: MessageReader<SceneEvent>,
    mut sample_scene_entry: ResMut<game_list::SampleDungeonRoomEntryState>,
    mut robot_sync_entry: ResMut<game_list::RobotSyncArenaEntryState>,
    mut fangyuan_home_entry: ResMut<game_list::FangyuanHomeEntryState>,
    mut overlay_commands: MessageWriter<UiOverlayCommand>,
    mut route_commands: MessageWriter<GameRouteCommand>,
) {
    for event in scene_events.read() {
        match event {
            SceneEvent::Entered(entered)
                if entered.scene_id.as_str() == SAMPLE_DUNGEON_ROOM_SCENE_ID =>
            {
                sample_scene_entry.clear();
            }
            SceneEvent::Entered(entered)
                if should_route_robot_sync_scene_entered(entered.scene_id.as_str()) =>
            {
                robot_sync_entry.clear();
                route_commands.write(GameRouteCommand::ChangeMode(AppUiMode::RobotSyncScene));
            }
            SceneEvent::Entered(entered)
                if should_route_fangyuan_home_entered(entered.scene_id.as_str()) =>
            {
                fangyuan_home_entry.clear();
                route_commands.write(GameRouteCommand::ChangeMode(AppUiMode::FangyuanHome));
            }
            SceneEvent::Failed(failure)
                if failure
                    .scene_id
                    .as_ref()
                    .is_some_and(|scene_id| scene_id.as_str() == SAMPLE_DUNGEON_ROOM_SCENE_ID) =>
            {
                sample_scene_entry.clear();
                warn!(
                    "failed to enter sample dungeon room scene: {}",
                    failure.log_description()
                );
                overlay_commands.write(UiOverlayCommand::ShowToast(UiToast::new_key(
                    &i18n,
                    "lobby.sample_scene.toast.failed",
                    "Failed to enter sample scene",
                )));
            }
            SceneEvent::Failed(failure)
                if failure
                    .scene_id
                    .as_ref()
                    .is_some_and(|scene_id| scene_id.as_str() == ROBOT_SYNC_ARENA_SCENE_ID) =>
            {
                robot_sync_entry.clear();
                warn!(
                    "failed to enter robot sync arena scene: {}",
                    failure.log_description()
                );
                overlay_commands.write(UiOverlayCommand::ShowToast(UiToast::new_key(
                    &i18n,
                    "lobby.robot_sync_scene.toast.failed",
                    "Failed to enter Robot Sync",
                )));
            }
            SceneEvent::Failed(failure)
                if failure
                    .scene_id
                    .as_ref()
                    .is_some_and(|scene_id| scene_id.as_str() == FANGYUAN_HOME_SCENE_ID) =>
            {
                fangyuan_home_entry.clear();
                warn!(
                    "failed to enter fangyuan home scene: {}",
                    failure.log_description()
                );
                overlay_commands.write(UiOverlayCommand::ShowToast(UiToast::new_key(
                    &i18n,
                    "lobby.fangyuan_home.toast.failed",
                    "Failed to enter Fangyuan Home",
                )));
            }
            SceneEvent::Exited(exited) if exited.scene_id.as_str() == ROBOT_SYNC_ARENA_SCENE_ID => {
                robot_sync_entry.clear();
            }
            SceneEvent::Exited(exited) if exited.scene_id.as_str() == FANGYUAN_HOME_SCENE_ID => {
                fangyuan_home_entry.clear();
            }
            _ => {}
        }
    }
}

fn should_route_robot_sync_scene_entered(scene_id: &str) -> bool {
    scene_id == ROBOT_SYNC_ARENA_SCENE_ID
}

fn should_route_fangyuan_home_entered(scene_id: &str) -> bool {
    scene_id == FANGYUAN_HOME_SCENE_ID
}

fn reset_sample_dungeon_room_entry_state(
    mut sample_scene_entry: ResMut<game_list::SampleDungeonRoomEntryState>,
) {
    sample_scene_entry.clear();
}

fn reset_robot_sync_arena_entry_state(
    mut robot_sync_entry: ResMut<game_list::RobotSyncArenaEntryState>,
) {
    robot_sync_entry.clear();
}

fn reset_fangyuan_home_entry_state(
    mut fangyuan_home_entry: ResMut<game_list::FangyuanHomeEntryState>,
) {
    fangyuan_home_entry.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::message::{MessageCursor, Messages};

    use crate::{
        framework::{
            scene::prelude::{
                SceneCommand, SceneEntered, SceneExited, SceneFailure, SceneFailureKind,
                SceneLifecycleState, SceneSwitchRequest,
            },
            ui::{
                core::UiPanelCommand,
                i18n::UiI18nPlugin,
                overlays::{UiModalResult, UiOverlayCommand},
                widgets::{UiButtonEvent, UiButtonEventKind},
            },
        },
        game::features::touch_ripple::TouchLaunchMode,
    };

    #[test]
    fn robot_sync_lobby_button_writes_switch_once_while_pending() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, UiI18nPlugin))
            .init_resource::<TouchLaunchMode>()
            .init_resource::<game_list::SampleDungeonRoomEntryState>()
            .init_resource::<game_list::RobotSyncArenaEntryState>()
            .init_resource::<game_list::FangyuanHomeEntryState>()
            .add_message::<SceneCommand>()
            .add_message::<UiPanelCommand>()
            .add_message::<UiOverlayCommand>()
            .add_message::<GameRouteCommand>()
            .add_message::<UiModalResult>()
            .add_message::<UiButtonEvent>()
            .add_systems(Update, game_list::handle_game_list_buttons);

        let robot_sync_button = app
            .world_mut()
            .spawn(game_list::RobotSyncArenaPlayButton)
            .id();

        app.world_mut().write_message(UiButtonEvent {
            entity: robot_sync_button,
            kind: UiButtonEventKind::Click,
            button: None,
        });
        app.world_mut().write_message(UiButtonEvent {
            entity: robot_sync_button,
            kind: UiButtonEventKind::Click,
            button: None,
        });
        app.update();

        let scene_commands = read_messages::<SceneCommand>(app.world());
        assert_eq!(scene_commands.len(), 1);
        assert_eq!(
            scene_commands[0],
            SceneCommand::Switch(SceneSwitchRequest::new(ROBOT_SYNC_ARENA_SCENE_ID))
        );
        assert!(
            app.world()
                .resource::<game_list::RobotSyncArenaEntryState>()
                .is_pending()
        );
    }

    #[test]
    fn fangyuan_home_lobby_button_writes_switch_once_while_pending() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, UiI18nPlugin))
            .init_resource::<TouchLaunchMode>()
            .init_resource::<game_list::SampleDungeonRoomEntryState>()
            .init_resource::<game_list::RobotSyncArenaEntryState>()
            .init_resource::<game_list::FangyuanHomeEntryState>()
            .add_message::<SceneCommand>()
            .add_message::<UiPanelCommand>()
            .add_message::<UiOverlayCommand>()
            .add_message::<GameRouteCommand>()
            .add_message::<UiModalResult>()
            .add_message::<UiButtonEvent>()
            .add_systems(Update, game_list::handle_game_list_buttons);

        let fangyuan_button = app
            .world_mut()
            .spawn(game_list::FangyuanHomePlayButton)
            .id();

        app.world_mut().write_message(UiButtonEvent {
            entity: fangyuan_button,
            kind: UiButtonEventKind::Click,
            button: None,
        });
        app.world_mut().write_message(UiButtonEvent {
            entity: fangyuan_button,
            kind: UiButtonEventKind::Click,
            button: None,
        });
        app.update();

        let scene_commands = read_messages::<SceneCommand>(app.world());
        assert_eq!(scene_commands.len(), 1);
        assert_eq!(
            scene_commands[0],
            SceneCommand::Switch(SceneSwitchRequest::new(FANGYUAN_HOME_SCENE_ID))
        );
        assert!(
            app.world()
                .resource::<game_list::FangyuanHomeEntryState>()
                .is_pending()
        );
    }

    #[test]
    fn robot_sync_entered_routes_to_robot_sync_scene_hud() {
        let mut app = lobby_scene_event_test_app();
        app.world_mut()
            .resource_mut::<game_list::RobotSyncArenaEntryState>()
            .set_pending_for_test(true);

        app.world_mut()
            .write_message(SceneEvent::Entered(SceneEntered {
                scene_id: ROBOT_SYNC_ARENA_SCENE_ID.into(),
                session_id: "robot-sync-session".into(),
                content_version: None,
            }));
        app.update();

        assert!(
            !app.world()
                .resource::<game_list::RobotSyncArenaEntryState>()
                .is_pending()
        );
        let route_commands = read_messages::<GameRouteCommand>(app.world());
        assert_eq!(route_commands.len(), 1);
        assert!(matches!(
            route_commands[0],
            GameRouteCommand::ChangeMode(AppUiMode::RobotSyncScene)
        ));
    }

    #[test]
    fn fangyuan_home_entered_routes_to_fangyuan_home_hud() {
        let mut app = lobby_scene_event_test_app();
        app.world_mut()
            .resource_mut::<game_list::FangyuanHomeEntryState>()
            .set_pending_for_test(true);

        app.world_mut()
            .write_message(SceneEvent::Entered(SceneEntered {
                scene_id: FANGYUAN_HOME_SCENE_ID.into(),
                session_id: "fangyuan-session".into(),
                content_version: None,
            }));
        app.update();

        assert!(
            !app.world()
                .resource::<game_list::FangyuanHomeEntryState>()
                .is_pending()
        );
        let route_commands = read_messages::<GameRouteCommand>(app.world());
        assert_eq!(route_commands.len(), 1);
        assert!(matches!(
            route_commands[0],
            GameRouteCommand::ChangeMode(AppUiMode::FangyuanHome)
        ));
    }

    #[test]
    fn non_robot_sync_entered_does_not_route_to_robot_sync_scene_hud() {
        assert!(should_route_robot_sync_scene_entered(
            ROBOT_SYNC_ARENA_SCENE_ID
        ));
        assert!(!should_route_robot_sync_scene_entered(
            SAMPLE_DUNGEON_ROOM_SCENE_ID
        ));
        assert!(should_route_fangyuan_home_entered(FANGYUAN_HOME_SCENE_ID));
        assert!(!should_route_fangyuan_home_entered(
            SAMPLE_DUNGEON_ROOM_SCENE_ID
        ));

        let mut app = lobby_scene_event_test_app();
        app.world_mut()
            .write_message(SceneEvent::Entered(SceneEntered {
                scene_id: SAMPLE_DUNGEON_ROOM_SCENE_ID.into(),
                session_id: "sample-session".into(),
                content_version: None,
            }));
        app.update();

        assert!(read_messages::<GameRouteCommand>(app.world()).is_empty());
    }

    #[test]
    fn robot_sync_failed_or_exited_clears_pending_without_routing() {
        let mut app = lobby_scene_event_test_app();
        app.world_mut()
            .resource_mut::<game_list::RobotSyncArenaEntryState>()
            .set_pending_for_test(true);

        app.world_mut().write_message(SceneEvent::Failed(
            SceneFailure::new(
                SceneFailureKind::SceneNotFound,
                SceneLifecycleState::Resolving,
            )
            .with_scene(ROBOT_SYNC_ARENA_SCENE_ID)
            .with_message("missing robot sync manifest"),
        ));
        app.update();

        assert!(
            !app.world()
                .resource::<game_list::RobotSyncArenaEntryState>()
                .is_pending()
        );
        assert!(read_messages::<GameRouteCommand>(app.world()).is_empty());
        assert_eq!(count_messages::<UiOverlayCommand>(app.world()), 1);

        app.world_mut()
            .resource_mut::<game_list::RobotSyncArenaEntryState>()
            .set_pending_for_test(true);
        app.world_mut()
            .write_message(SceneEvent::Exited(SceneExited {
                scene_id: ROBOT_SYNC_ARENA_SCENE_ID.into(),
                session_id: "robot-sync-session".into(),
            }));
        app.update();

        assert!(
            !app.world()
                .resource::<game_list::RobotSyncArenaEntryState>()
                .is_pending()
        );
        assert!(read_messages::<GameRouteCommand>(app.world()).is_empty());
    }

    #[test]
    fn fangyuan_home_failed_or_exited_clears_pending_without_routing() {
        let mut app = lobby_scene_event_test_app();
        app.world_mut()
            .resource_mut::<game_list::FangyuanHomeEntryState>()
            .set_pending_for_test(true);

        app.world_mut().write_message(SceneEvent::Failed(
            SceneFailure::new(
                SceneFailureKind::SceneNotFound,
                SceneLifecycleState::Resolving,
            )
            .with_scene(FANGYUAN_HOME_SCENE_ID)
            .with_message("missing fangyuan home manifest"),
        ));
        app.update();

        assert!(
            !app.world()
                .resource::<game_list::FangyuanHomeEntryState>()
                .is_pending()
        );
        assert!(read_messages::<GameRouteCommand>(app.world()).is_empty());
        assert_eq!(count_messages::<UiOverlayCommand>(app.world()), 1);

        app.world_mut()
            .resource_mut::<game_list::FangyuanHomeEntryState>()
            .set_pending_for_test(true);
        app.world_mut()
            .write_message(SceneEvent::Exited(SceneExited {
                scene_id: FANGYUAN_HOME_SCENE_ID.into(),
                session_id: "fangyuan-session".into(),
            }));
        app.update();

        assert!(
            !app.world()
                .resource::<game_list::FangyuanHomeEntryState>()
                .is_pending()
        );
        assert!(read_messages::<GameRouteCommand>(app.world()).is_empty());
    }

    fn lobby_scene_event_test_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, UiI18nPlugin))
            .init_resource::<game_list::SampleDungeonRoomEntryState>()
            .init_resource::<game_list::RobotSyncArenaEntryState>()
            .init_resource::<game_list::FangyuanHomeEntryState>()
            .add_message::<SceneEvent>()
            .add_message::<UiOverlayCommand>()
            .add_message::<GameRouteCommand>()
            .add_systems(Update, handle_lobby_scene_entry_events);
        app
    }

    fn read_messages<M>(world: &World) -> Vec<M>
    where
        M: Message + Clone,
    {
        let messages = world.resource::<Messages<M>>();
        let mut cursor = MessageCursor::default();
        cursor.read(messages).cloned().collect()
    }

    fn count_messages<M>(world: &World) -> usize
    where
        M: Message,
    {
        let messages = world.resource::<Messages<M>>();
        let mut cursor = MessageCursor::default();
        cursor.read(messages).count()
    }
}
