use bevy::{
    ecs::message::{MessageCursor, Messages},
    prelude::*,
};

use crate::framework::{
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
            UiButtonEvent, UiButtonEventKind, screen_label_key, screen_title_key,
            secondary_action_button_key,
        },
    },
};
use crate::game::{
    navigation::{AppUiMode, GameRouteCommand, game_panel_root},
    ui_ids::{OWNER_SAMPLE_SCENE, PANEL_SAMPLE_SCENE_HUD},
};

#[derive(Component)]
pub(super) struct SampleSceneLobbyButton;

pub(super) fn setup_sample_scene_hud(
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
        DespawnOnExit(AppUiMode::SampleScene),
        game_panel_root(PANEL_SAMPLE_SCENE_HUD, UiPanelKind::Hud, OWNER_SAMPLE_SCENE),
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
                    max_width: px(360),
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
                        "sample_scene.hud.title",
                        "Sample Scene",
                        UiThemeTextStyleRole::Title,
                    ),
                    screen_label_key(
                        theme,
                        fonts,
                        i18n,
                        "sample_scene.hud.status",
                        "Scene running",
                        UiThemeTextStyleRole::Caption,
                        UiThemeTextColorRole::Muted,
                    ),
                ],
            ),
            (
                secondary_action_button_key(theme, metrics, fonts, i18n, "nav.lobby", "Lobby",),
                SampleSceneLobbyButton,
            ),
        ],
    ));
}

pub(super) fn handle_sample_scene_hud_buttons(
    mut scene_commands: MessageWriter<SceneCommand>,
    mut route_commands: MessageWriter<GameRouteCommand>,
    lobby_buttons: Query<(), With<SampleSceneLobbyButton>>,
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

pub(super) fn route_to_lobby_on_sample_scene_exit(
    mut scene_events: MessageReader<SceneEvent>,
    current_mode: Res<State<AppUiMode>>,
    mut route_cursor: Local<MessageCursor<GameRouteCommand>>,
    mut route_messages: ResMut<Messages<GameRouteCommand>>,
) {
    let already_routing_to_lobby = route_cursor
        .read(&route_messages)
        .any(is_lobby_route_command);

    let mut sample_scene_exited = false;
    for event in scene_events.read() {
        let SceneEvent::Exited(exited) = event else {
            continue;
        };

        if exited.scene_id.as_str() != crate::game::scenes::SAMPLE_DUNGEON_ROOM_SCENE_ID {
            continue;
        }

        sample_scene_exited = true;
        break;
    }

    if should_route_sample_scene_exit_to_lobby(*current_mode.get(), already_routing_to_lobby)
        && sample_scene_exited
    {
        route_messages.write(GameRouteCommand::ChangeMode(AppUiMode::Lobby));
    }
}

fn should_route_sample_scene_exit_to_lobby(
    current_mode: AppUiMode,
    already_routing_to_lobby: bool,
) -> bool {
    current_mode == AppUiMode::SampleScene && !already_routing_to_lobby
}

fn is_lobby_route_command(command: &GameRouteCommand) -> bool {
    matches!(command, GameRouteCommand::ChangeMode(AppUiMode::Lobby))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::ui::widgets::UiButtonEvent;

    #[test]
    fn lobby_button_writes_scene_exit_and_lobby_route() {
        let mut app = App::new();
        app.add_message::<SceneCommand>()
            .add_message::<GameRouteCommand>()
            .add_message::<UiButtonEvent>()
            .add_systems(Update, handle_sample_scene_hud_buttons);

        let lobby_button = app.world_mut().spawn(SampleSceneLobbyButton).id();
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
    fn sample_scene_exit_fallback_only_routes_while_hud_is_active() {
        assert!(should_route_sample_scene_exit_to_lobby(
            AppUiMode::SampleScene,
            false
        ));
        assert!(!should_route_sample_scene_exit_to_lobby(
            AppUiMode::SampleScene,
            true
        ));
        assert!(!should_route_sample_scene_exit_to_lobby(
            AppUiMode::Lobby,
            false
        ));
        assert!(!should_route_sample_scene_exit_to_lobby(
            AppUiMode::Login,
            false
        ));
        assert!(is_lobby_route_command(&GameRouteCommand::ChangeMode(
            AppUiMode::Lobby
        )));
        assert!(!is_lobby_route_command(&GameRouteCommand::ChangeMode(
            AppUiMode::SampleScene
        )));
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
