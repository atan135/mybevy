use bevy::prelude::*;

use crate::framework::{
    scene::prelude::{SceneCommand, SceneExitRequest},
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
