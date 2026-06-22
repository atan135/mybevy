use bevy::{
    ecs::message::{MessageCursor, Messages},
    prelude::*,
};

use crate::framework::{
    audio::prelude::UiAudioCueOverride,
    scene::prelude::{SceneCommand, SceneEvent, SceneExitRequest},
    ui::{
        core::{UiLayer, UiLayerRoot, UiMetrics, UiPanelKind, UiViewport},
        i18n::UiI18n,
        style::{
            UiFontAssets, UiTheme,
            theme::{
                UiThemeBackgroundRole, UiThemeBorderRole, UiThemePanelNodeRole,
                UiThemeRootNodeRole, UiThemeTextColorRole, UiThemeTextStyleRole,
            },
        },
        widgets::{
            UiButtonEvent, UiButtonEventKind, screen_label, screen_title_key,
            secondary_action_button_key,
        },
    },
};
use crate::game::{
    audio::UI_CONFIRM_CUE_ID,
    authority::AuthoritySession,
    features::robot_sync::{format_robot_sync_hud_status, robot_sync_hud_snapshot},
    navigation::{AppUiMode, GameRouteCommand, game_panel_root},
    scenes::ROBOT_SYNC_ARENA_SCENE_ID,
    ui_ids::{OWNER_ROBOT_SYNC_SCENE, PANEL_ROBOT_SYNC_SCENE_HUD},
};

#[derive(Component)]
pub(super) struct RobotSyncSceneLobbyButton;

#[derive(Component)]
pub(super) struct RobotSyncHudStatusText;

pub(super) fn setup_robot_sync_scene_hud(
    mut commands: Commands,
    theme: Res<UiTheme>,
    metrics: Res<UiMetrics>,
    viewport: Res<UiViewport>,
    fonts: Res<UiFontAssets>,
    i18n: Res<UiI18n>,
) {
    let theme = theme.into_inner();
    let metrics = metrics.into_inner();
    let fonts = fonts.into_inner();
    let i18n = i18n.into_inner();

    commands.spawn((
        DespawnOnExit(AppUiMode::RobotSyncScene),
        game_panel_root(
            PANEL_ROBOT_SYNC_SCENE_HUD,
            UiPanelKind::Hud,
            OWNER_ROBOT_SYNC_SCENE,
        ),
        UiLayerRoot {
            layer: UiLayer::Page,
        },
        Node {
            width: percent(100),
            height: percent(100),
            padding: viewport.safe_area_padding(metrics.page_padding),
            align_items: AlignItems::FlexStart,
            justify_content: JustifyContent::SpaceBetween,
            column_gap: px(theme.layout.header_gap),
            ..default()
        },
        UiThemeRootNodeRole::Overlay,
        children![
            (
                UiThemePanelNodeRole::Content,
                Node {
                    max_width: px(440),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(theme.layout.row_gap),
                    padding: UiRect::all(px(theme.layout.panel_gap)),
                    border: UiRect::all(px(theme.panel.border)),
                    border_radius: BorderRadius::all(px(theme.panel.radius)),
                    ..default()
                },
                BackgroundColor(theme.colors.panel_background),
                BorderColor::all(theme.colors.panel_border),
                UiThemeBackgroundRole::Panel,
                UiThemeBorderRole::Panel,
                children![
                    screen_title_key(
                        theme,
                        fonts,
                        i18n,
                        "robot_sync_scene.hud.title",
                        "Robot Sync",
                        UiThemeTextStyleRole::Title,
                    ),
                    (
                        screen_label(
                            theme,
                            fonts,
                            "room=pending player=pending authority=pending frame=pending robots=0 local: pending",
                            UiThemeTextStyleRole::Caption,
                            UiThemeTextColorRole::Muted,
                        ),
                        RobotSyncHudStatusText,
                    ),
                ],
            ),
            (
                secondary_action_button_key(theme, metrics, fonts, i18n, "nav.lobby", "Lobby",),
                robot_sync_scene_lobby_button_audio_override(),
                RobotSyncSceneLobbyButton,
            ),
        ],
    ));
}

fn robot_sync_scene_lobby_button_audio_override() -> UiAudioCueOverride {
    UiAudioCueOverride::try_from(UI_CONFIRM_CUE_ID)
        .expect("robot sync scene lobby button UI audio cue id must be valid")
}

pub(super) fn update_robot_sync_scene_hud_status(
    config: Res<crate::game::features::robot_sync::RobotSyncConfig>,
    scene_state: Res<crate::game::features::robot_sync::RobotSyncSceneState>,
    authority_session: Res<AuthoritySession>,
    replay_state: Res<crate::game::features::robot_sync::RobotSyncReplayState>,
    mut status_texts: Query<&mut Text, With<RobotSyncHudStatusText>>,
) {
    let status = format_robot_sync_hud_status(&robot_sync_hud_snapshot(
        &config,
        &scene_state,
        &authority_session,
        &replay_state,
    ));

    for mut text in &mut status_texts {
        if text.0 != status {
            text.0 = status.clone();
        }
    }
}

pub(super) fn handle_robot_sync_scene_hud_buttons(
    mut scene_commands: MessageWriter<SceneCommand>,
    mut route_commands: MessageWriter<GameRouteCommand>,
    lobby_buttons: Query<(), With<RobotSyncSceneLobbyButton>>,
    mut button_events: MessageReader<UiButtonEvent>,
) {
    for event in button_events.read() {
        if event.kind != UiButtonEventKind::Click || !lobby_buttons.contains(event.entity) {
            continue;
        }

        scene_commands.write(SceneCommand::Exit(SceneExitRequest::default()));
        route_commands.write(GameRouteCommand::ChangeMode(AppUiMode::Lobby));
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

    let mut robot_sync_scene_exited = false;
    for event in scene_events.read() {
        let SceneEvent::Exited(exited) = event else {
            continue;
        };

        if exited.scene_id.as_str() != ROBOT_SYNC_ARENA_SCENE_ID {
            continue;
        }

        robot_sync_scene_exited = true;
        break;
    }

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
    use crate::framework::scene::prelude::{SceneExited, SceneId, SceneSessionId};
    use crate::framework::ui::widgets::UiButtonEvent;

    #[test]
    fn lobby_button_writes_scene_exit_and_lobby_route() {
        let mut app = App::new();
        app.add_message::<SceneCommand>()
            .add_message::<GameRouteCommand>()
            .add_message::<UiButtonEvent>()
            .add_systems(Update, handle_robot_sync_scene_hud_buttons);

        let lobby_button = app.world_mut().spawn(RobotSyncSceneLobbyButton).id();
        let ignored_button = app.world_mut().spawn_empty().id();

        app.world_mut().write_message(UiButtonEvent {
            entity: ignored_button,
            kind: UiButtonEventKind::Click,
            button: None,
        });
        app.world_mut().write_message(UiButtonEvent {
            entity: lobby_button,
            kind: UiButtonEventKind::Down,
            button: None,
        });
        app.world_mut().write_message(UiButtonEvent {
            entity: lobby_button,
            kind: UiButtonEventKind::Click,
            button: None,
        });

        app.update();

        let scene_commands = read_messages::<SceneCommand>(app.world());
        assert_eq!(
            scene_commands,
            vec![SceneCommand::Exit(SceneExitRequest::default())]
        );

        let route_commands = read_messages::<GameRouteCommand>(app.world());
        assert_eq!(route_commands.len(), 1);
        assert!(matches!(
            route_commands[0],
            GameRouteCommand::ChangeMode(AppUiMode::Lobby)
        ));
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
        assert!(!should_route_robot_sync_scene_exit_to_lobby(
            AppUiMode::SampleScene,
            false
        ));
        assert!(is_lobby_route_command(&GameRouteCommand::ChangeMode(
            AppUiMode::Lobby
        )));
        assert!(!is_lobby_route_command(&GameRouteCommand::ChangeMode(
            AppUiMode::RobotSyncScene
        )));
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
        let route_commands = read_messages::<GameRouteCommand>(app.world());
        assert!(matches!(
            route_commands.last(),
            Some(GameRouteCommand::ChangeMode(AppUiMode::Lobby))
        ));
    }

    #[test]
    fn lobby_button_uses_confirm_audio_override() {
        assert_eq!(
            robot_sync_scene_lobby_button_audio_override()
                .cue_id
                .as_str(),
            UI_CONFIRM_CUE_ID
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
}
