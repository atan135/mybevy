use bevy::{
    ecs::message::{MessageCursor, Messages},
    prelude::*,
};

use crate::framework::{
    audio::prelude::UiAudioCueOverride,
    scene::prelude::{SceneCommand, SceneEvent, SceneExitRequest},
    ui::{
        core::{UiLayer, UiLayerRoot, UiMetrics, UiPanelKind, UiViewport, UiWidthClass},
        i18n::UiI18n,
        style::{
            UiFontAssets, UiTheme,
            theme::{
                UiThemeBackgroundRole, UiThemeBorderRole, UiThemePanelNodeRole,
                UiThemeRootNodeRole, UiThemeTextColorRole, UiThemeTextStyleRole,
            },
        },
        widgets::{
            UiButtonEvent, UiButtonEventKind, screen_label, screen_label_key, screen_title_key,
            secondary_action_button_key,
        },
    },
};
use crate::game::{
    audio::UI_CONFIRM_CUE_ID,
    navigation::{AppUiMode, GameRouteCommand, game_panel_root},
    scenes::{
        FANGYUAN_HOME_DEFAULT_BLUEPRINT_PATH, FANGYUAN_HOME_SCENE_ID, FangyuanHomeBlueprintCommand,
        FangyuanHomeBlueprintStats,
    },
    ui_ids::{OWNER_FANGYUAN_HOME, PANEL_FANGYUAN_HOME_HUD},
};

const FANGYUAN_HOME_PRIMITIVE_LIMIT: usize = 1000;

#[derive(Component)]
pub(super) struct FangyuanHomeReloadButton;

#[derive(Component)]
pub(super) struct FangyuanHomeClearButton;

#[derive(Component)]
pub(super) struct FangyuanHomeLobbyButton;

#[derive(Component)]
pub(super) struct FangyuanHomeHudStatusText;

pub(super) fn setup_fangyuan_home_hud(
    mut commands: Commands,
    theme: Res<UiTheme>,
    metrics: Res<UiMetrics>,
    viewport: Res<UiViewport>,
    fonts: Res<UiFontAssets>,
    i18n: Res<UiI18n>,
) {
    let theme = theme.into_inner();
    let metrics = metrics.into_inner();
    let viewport = *viewport;
    let fonts = fonts.into_inner();
    let i18n = i18n.into_inner();

    commands.spawn((
        DespawnOnExit(AppUiMode::FangyuanHome),
        game_panel_root(
            PANEL_FANGYUAN_HOME_HUD,
            UiPanelKind::Hud,
            OWNER_FANGYUAN_HOME,
        ),
        UiLayerRoot {
            layer: UiLayer::Page,
        },
        fangyuan_home_hud_root_node(&viewport, metrics, theme),
        UiThemeRootNodeRole::Overlay,
        children![
            (
                UiThemePanelNodeRole::Content,
                fangyuan_home_status_panel_node(&viewport, theme),
                BackgroundColor(theme.colors.panel_background),
                BorderColor::all(theme.colors.panel_border),
                UiThemeBackgroundRole::Panel,
                UiThemeBorderRole::Panel,
                children![
                    screen_title_key(
                        theme,
                        fonts,
                        i18n,
                        "fangyuan_home.hud.title",
                        "方圆灵构家园",
                        UiThemeTextStyleRole::Title,
                    ),
                    screen_label_key(
                        theme,
                        fonts,
                        i18n,
                        "fangyuan_home.hud.scene",
                        "原型预览",
                        UiThemeTextStyleRole::Caption,
                        UiThemeTextColorRole::Muted,
                    ),
                    (
                        screen_label(
                            theme,
                            fonts,
                            fangyuan_home_hud_status_text(None),
                            UiThemeTextStyleRole::Caption,
                            UiThemeTextColorRole::Muted,
                        ),
                        FangyuanHomeHudStatusText,
                    ),
                ],
            ),
            (
                fangyuan_home_button_column_node(&viewport, theme),
                children![
                    (
                        secondary_action_button_key(
                            theme,
                            metrics,
                            fonts,
                            i18n,
                            "fangyuan_home.hud.reload",
                            "重新加载",
                        ),
                        FangyuanHomeReloadButton,
                    ),
                    (
                        secondary_action_button_key(
                            theme,
                            metrics,
                            fonts,
                            i18n,
                            "fangyuan_home.hud.clear",
                            "清空",
                        ),
                        FangyuanHomeClearButton,
                    ),
                    (
                        secondary_action_button_key(
                            theme,
                            metrics,
                            fonts,
                            i18n,
                            "nav.lobby",
                            "大厅",
                        ),
                        fangyuan_home_lobby_button_audio_override(),
                        FangyuanHomeLobbyButton,
                    ),
                ],
            ),
        ],
    ));
}

fn fangyuan_home_hud_root_node(
    viewport: &UiViewport,
    metrics: &UiMetrics,
    theme: &UiTheme,
) -> Node {
    let compact = viewport.width_class == UiWidthClass::Compact;
    Node {
        width: percent(100),
        height: percent(100),
        padding: viewport.safe_area_padding(metrics.page_padding),
        align_items: AlignItems::FlexStart,
        justify_content: if compact {
            JustifyContent::FlexStart
        } else {
            JustifyContent::SpaceBetween
        },
        flex_direction: if compact {
            FlexDirection::Column
        } else {
            FlexDirection::Row
        },
        row_gap: px(theme.layout.row_gap),
        column_gap: px(theme.layout.header_gap),
        ..default()
    }
}

fn fangyuan_home_status_panel_node(viewport: &UiViewport, theme: &UiTheme) -> Node {
    let compact = viewport.width_class == UiWidthClass::Compact;
    Node {
        width: if compact { percent(100) } else { auto() },
        max_width: px(if compact { 360.0 } else { 420.0 }),
        flex_direction: FlexDirection::Column,
        overflow: Overflow::clip(),
        row_gap: px(theme.layout.row_gap),
        padding: UiRect::all(px(theme.layout.panel_gap)),
        border: UiRect::all(px(theme.panel.border)),
        border_radius: BorderRadius::all(px(theme.panel.radius)),
        ..default()
    }
}

fn fangyuan_home_button_column_node(viewport: &UiViewport, theme: &UiTheme) -> Node {
    let compact = viewport.width_class == UiWidthClass::Compact;
    Node {
        flex_direction: if compact {
            FlexDirection::Row
        } else {
            FlexDirection::Column
        },
        flex_wrap: FlexWrap::Wrap,
        row_gap: px(theme.layout.row_gap),
        column_gap: px(theme.layout.row_column_gap),
        align_items: AlignItems::Stretch,
        align_self: if compact {
            AlignSelf::FlexStart
        } else {
            AlignSelf::Auto
        },
        ..default()
    }
}

fn fangyuan_home_lobby_button_audio_override() -> UiAudioCueOverride {
    UiAudioCueOverride::try_from(UI_CONFIRM_CUE_ID)
        .expect("fangyuan home lobby button UI audio cue id must be valid")
}

pub(super) fn update_fangyuan_home_hud_status(
    stats: Res<FangyuanHomeBlueprintStats>,
    mut status_texts: Query<&mut Text, With<FangyuanHomeHudStatusText>>,
) {
    let status = fangyuan_home_hud_status_text(Some(&stats));
    for mut text in &mut status_texts {
        if text.0 != status {
            text.0 = status.clone();
        }
    }
}

fn fangyuan_home_hud_status_text(stats: Option<&FangyuanHomeBlueprintStats>) -> String {
    let default_stats = FangyuanHomeBlueprintStats::default();
    let stats = stats.unwrap_or(&default_stats);
    let primitive_stats = &stats.primitive_stats;
    let state = stats.state_label();
    let top_level = if stats.top_level_valid {
        "ok"
    } else {
        "invalid"
    };
    let path = compact_fangyuan_home_blueprint_path(stats.blueprint_path());

    format!(
        "primitive {}/{}  {state}\ncube {}  sphere {}  skipped {}\nmat {}  alpha {}  glow {}  top {top_level}\npath {path}",
        stats.primitive_total(),
        FANGYUAN_HOME_PRIMITIVE_LIMIT,
        primitive_stats.cube_count,
        primitive_stats.sphere_count,
        stats.skipped,
        stats.materials,
        primitive_stats.alpha_count,
        primitive_stats.emissive_count,
    )
}

fn compact_fangyuan_home_blueprint_path(path: &str) -> String {
    const MAX_PATH_CHARS: usize = 32;

    let path = if path.trim().is_empty() {
        FANGYUAN_HOME_DEFAULT_BLUEPRINT_PATH
    } else {
        path.trim()
    };
    let char_count = path.chars().count();
    if char_count <= MAX_PATH_CHARS {
        return path.to_string();
    }

    let tail = path
        .chars()
        .rev()
        .take(MAX_PATH_CHARS - 3)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("...{tail}")
}

pub(super) fn handle_fangyuan_home_hud_buttons(
    mut blueprint_commands: MessageWriter<FangyuanHomeBlueprintCommand>,
    mut scene_commands: MessageWriter<SceneCommand>,
    mut route_commands: MessageWriter<GameRouteCommand>,
    reload_buttons: Query<(), With<FangyuanHomeReloadButton>>,
    clear_buttons: Query<(), With<FangyuanHomeClearButton>>,
    lobby_buttons: Query<(), With<FangyuanHomeLobbyButton>>,
    mut button_events: MessageReader<UiButtonEvent>,
) {
    for event in button_events.read() {
        if event.kind != UiButtonEventKind::Click {
            continue;
        }

        if reload_buttons.contains(event.entity) {
            blueprint_commands.write(FangyuanHomeBlueprintCommand::Reload);
        } else if clear_buttons.contains(event.entity) {
            blueprint_commands.write(FangyuanHomeBlueprintCommand::Clear);
        } else if lobby_buttons.contains(event.entity) {
            scene_commands.write(SceneCommand::Exit(SceneExitRequest::default()));
            route_commands.write(GameRouteCommand::ChangeMode(AppUiMode::Lobby));
        }
    }
}

pub(super) fn route_to_lobby_on_fangyuan_home_exit(
    mut scene_events: MessageReader<SceneEvent>,
    current_mode: Res<State<AppUiMode>>,
    mut route_cursor: Local<MessageCursor<GameRouteCommand>>,
    mut route_messages: ResMut<Messages<GameRouteCommand>>,
) {
    let already_routing_to_lobby = route_cursor
        .read(&route_messages)
        .any(is_lobby_route_command);

    let mut fangyuan_home_exited = false;
    for event in scene_events.read() {
        let SceneEvent::Exited(exited) = event else {
            continue;
        };

        if exited.scene_id.as_str() != FANGYUAN_HOME_SCENE_ID {
            continue;
        }

        fangyuan_home_exited = true;
        break;
    }

    if should_route_fangyuan_home_exit_to_lobby(*current_mode.get(), already_routing_to_lobby)
        && fangyuan_home_exited
    {
        route_messages.write(GameRouteCommand::ChangeMode(AppUiMode::Lobby));
    }
}

fn should_route_fangyuan_home_exit_to_lobby(
    current_mode: AppUiMode,
    already_routing_to_lobby: bool,
) -> bool {
    current_mode == AppUiMode::FangyuanHome && !already_routing_to_lobby
}

fn is_lobby_route_command(command: &GameRouteCommand) -> bool {
    matches!(command, GameRouteCommand::ChangeMode(AppUiMode::Lobby))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        framework::{
            fangyuan::{
                FangyuanPrimitive, FangyuanPrimitiveKind, FangyuanPrimitiveRole,
                FangyuanPrimitiveSet,
            },
            scene::prelude::{SceneExited, SceneId, SceneSessionId},
            ui::widgets::UiButtonEvent,
        },
        game::scenes::FangyuanHomeBlueprintStats,
    };

    #[test]
    fn hud_buttons_write_reload_clear_and_lobby_exit_route() {
        let mut app = App::new();
        app.add_message::<FangyuanHomeBlueprintCommand>()
            .add_message::<SceneCommand>()
            .add_message::<GameRouteCommand>()
            .add_message::<UiButtonEvent>()
            .add_systems(Update, handle_fangyuan_home_hud_buttons);

        let reload_button = app.world_mut().spawn(FangyuanHomeReloadButton).id();
        let clear_button = app.world_mut().spawn(FangyuanHomeClearButton).id();
        let lobby_button = app.world_mut().spawn(FangyuanHomeLobbyButton).id();
        let ignored_button = app.world_mut().spawn_empty().id();

        app.world_mut().write_message(UiButtonEvent {
            entity: ignored_button,
            kind: UiButtonEventKind::Click,
            button: None,
        });
        app.world_mut().write_message(UiButtonEvent {
            entity: reload_button,
            kind: UiButtonEventKind::Click,
            button: None,
        });
        app.world_mut().write_message(UiButtonEvent {
            entity: clear_button,
            kind: UiButtonEventKind::Click,
            button: None,
        });
        app.world_mut().write_message(UiButtonEvent {
            entity: lobby_button,
            kind: UiButtonEventKind::Click,
            button: None,
        });
        app.update();

        assert_eq!(
            read_messages::<FangyuanHomeBlueprintCommand>(app.world()),
            vec![
                FangyuanHomeBlueprintCommand::Reload,
                FangyuanHomeBlueprintCommand::Clear
            ]
        );
        assert_eq!(
            read_messages::<SceneCommand>(app.world()),
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
    fn hud_status_text_updates_from_blueprint_stats() {
        let mut app = App::new();
        let session_id = SceneSessionId::from("fangyuan-session");
        let primitive_set = hud_test_primitive_set();
        let mut stats = FangyuanHomeBlueprintStats::default();
        stats.record_loaded(&session_id, "fangyuan/home_preview.ron", &primitive_set, 2);
        app.insert_resource(stats)
            .add_systems(Update, update_fangyuan_home_hud_status);
        let status_text = app
            .world_mut()
            .spawn((Text::new("pending"), FangyuanHomeHudStatusText))
            .id();

        app.update();

        let text = app.world().get::<Text>(status_text).unwrap();
        assert_eq!(
            text.0,
            "primitive 3/1000  loaded\ncube 1  sphere 2  skipped 2\nmat 3  alpha 2  glow 1  top ok\npath fangyuan/home_preview.ron"
        );
    }

    #[test]
    fn hud_status_text_reports_clear_reload_and_failure_states() {
        let session_id = SceneSessionId::from("fangyuan-session");
        let primitive_set = hud_test_primitive_set();
        let mut stats = FangyuanHomeBlueprintStats::default();

        stats.record_loaded(&session_id, "fangyuan/home_preview.ron", &primitive_set, 4);
        assert_eq!(
            fangyuan_home_hud_status_text(Some(&stats)),
            "primitive 3/1000  loaded\ncube 1  sphere 2  skipped 4\nmat 3  alpha 2  glow 1  top ok\npath fangyuan/home_preview.ron"
        );

        stats.record_cleared(&session_id);
        assert_eq!(
            fangyuan_home_hud_status_text(Some(&stats)),
            "primitive 0/1000  cleared\ncube 0  sphere 0  skipped 4\nmat 3  alpha 0  glow 0  top ok\npath fangyuan/home_preview.ron"
        );

        stats.record_loaded(&session_id, "fangyuan/home_preview.ron", &primitive_set, 4);
        assert_eq!(
            fangyuan_home_hud_status_text(Some(&stats)),
            "primitive 3/1000  loaded\ncube 1  sphere 2  skipped 4\nmat 3  alpha 2  glow 1  top ok\npath fangyuan/home_preview.ron"
        );

        stats.record_failed(
            &session_id,
            "fangyuan/very/deep/generated/debug/home_preview_failure_case.ron",
            9,
            3,
        );
        assert_eq!(
            fangyuan_home_hud_status_text(Some(&stats)),
            "primitive 0/1000  failed\ncube 0  sphere 0  skipped 9\nmat 3  alpha 0  glow 0  top invalid\npath ...home_preview_failure_case.ron"
        );
    }

    #[test]
    fn hud_status_text_defaults_to_non_successful_empty_state() {
        assert_eq!(
            fangyuan_home_hud_status_text(None),
            "primitive 0/1000  pending\ncube 0  sphere 0  skipped 0\nmat 0  alpha 0  glow 0  top invalid\npath fangyuan/home_preview.ron"
        );
    }

    #[test]
    fn fangyuan_home_exit_fallback_only_routes_while_hud_is_active() {
        assert!(should_route_fangyuan_home_exit_to_lobby(
            AppUiMode::FangyuanHome,
            false
        ));
        assert!(!should_route_fangyuan_home_exit_to_lobby(
            AppUiMode::FangyuanHome,
            true
        ));
        assert!(!should_route_fangyuan_home_exit_to_lobby(
            AppUiMode::Lobby,
            false
        ));
        assert!(is_lobby_route_command(&GameRouteCommand::ChangeMode(
            AppUiMode::Lobby
        )));
        assert!(!is_lobby_route_command(&GameRouteCommand::ChangeMode(
            AppUiMode::FangyuanHome
        )));
    }

    #[test]
    fn fangyuan_home_exit_fallback_ignores_other_scene_ids() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<AppUiMode>()
            .add_message::<SceneEvent>()
            .add_message::<GameRouteCommand>()
            .add_systems(Update, route_to_lobby_on_fangyuan_home_exit);
        app.world_mut()
            .resource_mut::<NextState<AppUiMode>>()
            .set(AppUiMode::FangyuanHome);
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
                scene_id: SceneId::from(FANGYUAN_HOME_SCENE_ID),
                session_id: SceneSessionId::from("fangyuan-session"),
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
            fangyuan_home_lobby_button_audio_override().cue_id.as_str(),
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

    fn hud_test_primitive_set() -> FangyuanPrimitiveSet {
        FangyuanPrimitiveSet::from_primitives(vec![
            FangyuanPrimitive::with_runtime_metadata(
                FangyuanPrimitiveKind::Cube,
                Vec3::ZERO,
                Vec3::ONE,
                Color::srgba(0.1, 0.2, 0.3, 1.0),
                FangyuanPrimitiveRole::Structure,
                1.0,
                0.0,
                None,
                Default::default(),
            ),
            FangyuanPrimitive::with_runtime_metadata(
                FangyuanPrimitiveKind::Sphere,
                Vec3::Y,
                Vec3::ONE,
                Color::srgba(0.4, 0.5, 0.6, 0.5),
                FangyuanPrimitiveRole::Core,
                0.5,
                0.0,
                None,
                Default::default(),
            ),
            FangyuanPrimitive::with_runtime_metadata(
                FangyuanPrimitiveKind::Sphere,
                Vec3::NEG_Y,
                Vec3::ONE,
                Color::srgba(0.7, 0.8, 0.9, 0.25),
                FangyuanPrimitiveRole::Decoration,
                0.25,
                2.0,
                Some("glow".to_string()),
                Default::default(),
            ),
        ])
    }
}
