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
    features::{
        lockstep_sim::{format_lockstep_sim_hud_status, lockstep_sim_hud_snapshot},
        robot_sync::{format_robot_sync_hud_status, robot_sync_hud_snapshot},
    },
    navigation::{AppUiMode, GameRouteCommand, game_panel_root},
    scenes::ROBOT_SYNC_ARENA_SCENE_ID,
    ui_ids::{OWNER_ROBOT_SYNC_SCENE, PANEL_ROBOT_SYNC_SCENE_HUD},
};

const ROBOT_SYNC_HUD_PANEL_WIDTH: f32 = 440.0;
const ROBOT_SYNC_HUD_PANEL_MIN_HEIGHT: f32 = 280.0;

#[derive(Component)]
pub(super) struct RobotSyncSceneLobbyButton;

#[derive(Clone, Copy, Debug, Default, Resource, PartialEq, Eq)]
pub(super) struct RobotSyncHudVisibility {
    show_details: bool,
}

#[derive(Component)]
pub(super) struct RobotSyncHudDetailsPanel;

#[derive(Component)]
pub(super) struct RobotSyncHudStatusText;

#[derive(Component)]
pub(super) struct RobotSyncHudTitleText;

#[derive(Component)]
pub(super) struct RobotSyncHudHideButton;

#[derive(Component)]
pub(super) struct RobotSyncHudShowButton;

fn robot_sync_hud_details_panel_node(theme: &UiTheme) -> Node {
    Node {
        width: px(ROBOT_SYNC_HUD_PANEL_WIDTH),
        max_width: percent(100),
        min_height: px(ROBOT_SYNC_HUD_PANEL_MIN_HEIGHT),
        flex_direction: FlexDirection::Column,
        row_gap: px(theme.layout.row_gap),
        padding: UiRect::all(px(theme.layout.panel_gap)),
        border: UiRect::all(px(theme.panel.border)),
        border_radius: BorderRadius::all(px(theme.panel.radius)),
        ..default()
    }
}

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

    commands.insert_resource(RobotSyncHudVisibility { show_details: true });
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
                robot_sync_hud_details_panel_node(theme),
                BackgroundColor(theme.colors.panel_background),
                BorderColor::all(theme.colors.panel_border),
                UiThemeBackgroundRole::Panel,
                UiThemeBorderRole::Panel,
                RobotSyncHudDetailsPanel,
                children![
                    (
                        screen_title_key(
                            theme,
                            fonts,
                            i18n,
                            "robot_sync_scene.hud.title",
                            "Robot Sync",
                            UiThemeTextStyleRole::Title,
                        ),
                        RobotSyncHudTitleText,
                    ),
                    (
                        screen_label(
                            theme,
                            fonts,
                            "room=pending player=pending authority=pending frame=pending robots=0 local: pending",
                            UiThemeTextStyleRole::Caption,
                            UiThemeTextColorRole::Muted,
                        ),
                        Node {
                            width: percent(100),
                            ..default()
                        },
                        RobotSyncHudStatusText,
                    ),
                ],
            ),
            (
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(theme.layout.row_gap),
                    align_items: AlignItems::Stretch,
                    ..default()
                },
                children![
                    (
                        secondary_action_button_key(
                            theme,
                            metrics,
                            fonts,
                            i18n,
                            "robot_sync_scene.hud.hide",
                            "Hide HUD",
                        ),
                        RobotSyncHudHideButton,
                        Visibility::Visible,
                    ),
                    (
                        secondary_action_button_key(
                            theme,
                            metrics,
                            fonts,
                            i18n,
                            "robot_sync_scene.hud.show",
                            "Show HUD",
                        ),
                        RobotSyncHudShowButton,
                        Visibility::Hidden,
                    ),
                    (
                        secondary_action_button_key(theme, metrics, fonts, i18n, "nav.lobby", "Lobby",),
                        robot_sync_scene_lobby_button_audio_override(),
                        RobotSyncSceneLobbyButton,
                    ),
                ],
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
    lockstep_config: Res<crate::game::features::lockstep_sim::LockstepSimConfig>,
    lockstep_scene_state: Res<crate::game::features::lockstep_sim::LockstepSimSceneState>,
    authority_session: Res<AuthoritySession>,
    replay_state: Res<crate::game::features::robot_sync::RobotSyncReplayState>,
    lockstep_replay_state: Res<crate::game::features::lockstep_sim::LockstepSimReplayState>,
    i18n: Res<UiI18n>,
    mut title_texts: Query<
        &mut Text,
        (With<RobotSyncHudTitleText>, Without<RobotSyncHudStatusText>),
    >,
    mut status_texts: Query<
        &mut Text,
        (With<RobotSyncHudStatusText>, Without<RobotSyncHudTitleText>),
    >,
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
        "Lockstep Sim".to_string()
    } else {
        i18n.tr("robot_sync_scene.hud.title", "Robot Sync")
    };

    for mut text in &mut title_texts {
        if text.0 != title {
            text.0 = title.clone();
        }
    }

    for mut text in &mut status_texts {
        if text.0 != status {
            text.0 = status.clone();
        }
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

pub(super) fn handle_robot_sync_scene_hud_buttons(
    mut scene_commands: MessageWriter<SceneCommand>,
    mut route_commands: MessageWriter<GameRouteCommand>,
    mut hud_visibility: ResMut<RobotSyncHudVisibility>,
    lobby_buttons: Query<(), With<RobotSyncSceneLobbyButton>>,
    hide_buttons: Query<(), With<RobotSyncHudHideButton>>,
    show_buttons: Query<(), With<RobotSyncHudShowButton>>,
    mut button_events: MessageReader<UiButtonEvent>,
) {
    for event in button_events.read() {
        if event.kind != UiButtonEventKind::Click {
            continue;
        }

        if lobby_buttons.contains(event.entity) {
            scene_commands.write(SceneCommand::Exit(SceneExitRequest::default()));
            route_commands.write(GameRouteCommand::ChangeMode(AppUiMode::Lobby));
        } else if hide_buttons.contains(event.entity) {
            hud_visibility.show_details = false;
        } else if show_buttons.contains(event.entity) {
            hud_visibility.show_details = true;
        }
    }
}

pub(super) fn sync_robot_sync_hud_visibility(
    hud_visibility: Res<RobotSyncHudVisibility>,
    mut detail_panels: Query<&mut Node, With<RobotSyncHudDetailsPanel>>,
    mut hide_buttons: Query<
        &mut Visibility,
        (
            With<RobotSyncHudHideButton>,
            Without<RobotSyncHudShowButton>,
        ),
    >,
    mut show_buttons: Query<
        &mut Visibility,
        (
            With<RobotSyncHudShowButton>,
            Without<RobotSyncHudHideButton>,
        ),
    >,
) {
    if !hud_visibility.is_changed() {
        return;
    }

    for mut node in &mut detail_panels {
        set_node_display(&mut node, hud_visibility.show_details);
    }
    for mut visibility in &mut hide_buttons {
        set_visibility(&mut visibility, hud_visibility.show_details);
    }
    for mut visibility in &mut show_buttons {
        set_visibility(&mut visibility, !hud_visibility.show_details);
    }
}

fn set_node_display(node: &mut Node, visible: bool) {
    node.display = if visible {
        Display::Flex
    } else {
        Display::None
    };
}

fn set_visibility(visibility: &mut Visibility, visible: bool) {
    *visibility = if visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
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
    use crate::framework::scene::prelude::{SceneEntered, SceneExited, SceneId, SceneSessionId};
    use crate::framework::ui::widgets::UiButtonEvent;
    use crate::game::features::{
        lockstep_sim::LockstepSimPlugin,
        robot_sync::{RobotSyncConfig, RobotSyncReplayState, RobotSyncSceneState},
    };

    #[test]
    fn hud_details_panel_reserves_stable_wrapped_status_space() {
        let node = robot_sync_hud_details_panel_node(&UiTheme::default());

        assert_eq!(node.width, px(ROBOT_SYNC_HUD_PANEL_WIDTH));
        assert_eq!(node.max_width, percent(100));
        assert_eq!(node.min_height, px(ROBOT_SYNC_HUD_PANEL_MIN_HEIGHT));
        assert_eq!(node.flex_direction, FlexDirection::Column);
    }

    #[test]
    fn lobby_button_writes_scene_exit_and_lobby_route() {
        let mut app = App::new();
        app.add_message::<SceneCommand>()
            .add_message::<GameRouteCommand>()
            .add_message::<UiButtonEvent>()
            .insert_resource(RobotSyncHudVisibility { show_details: true })
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
    fn hud_toggle_buttons_switch_details_visibility() {
        let mut app = App::new();
        app.add_message::<SceneCommand>()
            .add_message::<GameRouteCommand>()
            .add_message::<UiButtonEvent>()
            .insert_resource(RobotSyncHudVisibility { show_details: true })
            .add_systems(
                Update,
                (
                    handle_robot_sync_scene_hud_buttons,
                    sync_robot_sync_hud_visibility,
                )
                    .chain(),
            );

        let detail_panel = app
            .world_mut()
            .spawn((RobotSyncHudDetailsPanel, Node::default()))
            .id();
        let hide_button = app
            .world_mut()
            .spawn((RobotSyncHudHideButton, Visibility::Visible))
            .id();
        let show_button = app
            .world_mut()
            .spawn((RobotSyncHudShowButton, Visibility::Hidden))
            .id();

        app.world_mut().write_message(UiButtonEvent {
            entity: hide_button,
            kind: UiButtonEventKind::Click,
            button: None,
        });
        app.update();

        assert!(
            !app.world()
                .resource::<RobotSyncHudVisibility>()
                .show_details
        );
        assert_eq!(
            app.world().get::<Node>(detail_panel).unwrap().display,
            Display::None
        );
        assert_eq!(
            *app.world().get::<Visibility>(hide_button).unwrap(),
            Visibility::Hidden
        );
        assert_eq!(
            *app.world().get::<Visibility>(show_button).unwrap(),
            Visibility::Visible
        );

        app.world_mut().write_message(UiButtonEvent {
            entity: show_button,
            kind: UiButtonEventKind::Click,
            button: None,
        });
        app.update();

        assert!(
            app.world()
                .resource::<RobotSyncHudVisibility>()
                .show_details
        );
        assert_eq!(
            app.world().get::<Node>(detail_panel).unwrap().display,
            Display::Flex
        );
        assert_eq!(
            *app.world().get::<Visibility>(hide_button).unwrap(),
            Visibility::Visible
        );
        assert_eq!(
            *app.world().get::<Visibility>(show_button).unwrap(),
            Visibility::Hidden
        );
    }

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
            &test_robot_sync_config(),
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
        assert!(status.contains("mismatch="));
    }

    #[test]
    fn hud_status_keeps_robot_sync_snapshot_when_lockstep_scene_is_inactive() {
        let status = robot_sync_scene_hud_status(
            &test_robot_sync_config(),
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

    fn test_robot_sync_config() -> RobotSyncConfig {
        RobotSyncConfig::default()
    }
}
