use bevy::prelude::*;

use crate::framework::{
    audio::prelude::UiAudioCueOverride,
    scene::prelude::SCENE_CAMERA_3D_ORDER,
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
            UiButtonEvent, UiButtonEventKind, screen_label_key, screen_title_key,
            secondary_action_button_key,
        },
    },
};
use crate::game::{
    audio::UI_CONFIRM_CUE_ID,
    navigation::{AppUiMode, GameRouteCommand, game_panel_root},
    ui_ids::{OWNER_FANGYUAN_PLAYER_PREVIEW, PANEL_FANGYUAN_PLAYER_PREVIEW_HUD},
};

const FANGYUAN_PLAYER_PREVIEW_CAMERA_TRANSLATION: Vec3 = Vec3::new(0.0, 2.2, 5.0);
const FANGYUAN_PLAYER_PREVIEW_CAMERA_TARGET: Vec3 = Vec3::new(0.0, 1.0, 0.0);

#[derive(Component)]
pub(super) struct FangyuanPlayerPreviewLobbyButton;

#[derive(Component)]
pub(super) struct FangyuanPlayerPreviewCamera;

#[derive(Component)]
pub(super) struct FangyuanPlayerPreviewLight;

pub(super) fn setup_fangyuan_player_preview(
    mut commands: Commands,
    theme: Res<UiTheme>,
    metrics: Res<UiMetrics>,
    viewport: Res<UiViewport>,
    fonts: Res<UiFontAssets>,
    i18n: Res<UiI18n>,
    mut clear_color: ResMut<ClearColor>,
) {
    let theme = theme.into_inner();
    let metrics = metrics.into_inner();
    let viewport = *viewport;
    let fonts = fonts.into_inner();
    let i18n = i18n.into_inner();
    clear_color.0 = theme.colors.screen_background;

    spawn_fangyuan_player_preview_camera_and_light(&mut commands);
    spawn_fangyuan_player_preview_hud(&mut commands, theme, metrics, &viewport, fonts, i18n);
}

#[cfg(test)]
fn setup_fangyuan_player_preview_camera_and_light(mut commands: Commands) {
    spawn_fangyuan_player_preview_camera_and_light(&mut commands);
}

fn spawn_fangyuan_player_preview_camera_and_light(commands: &mut Commands) {
    commands.spawn((
        DespawnOnExit(AppUiMode::FangyuanPlayerPreview),
        Camera3d::default(),
        Camera {
            order: SCENE_CAMERA_3D_ORDER,
            clear_color: ClearColorConfig::Default,
            ..default()
        },
        Transform::from_translation(FANGYUAN_PLAYER_PREVIEW_CAMERA_TRANSLATION)
            .looking_at(FANGYUAN_PLAYER_PREVIEW_CAMERA_TARGET, Vec3::Y),
        GlobalTransform::default(),
        FangyuanPlayerPreviewCamera,
        Name::new("FangyuanPlayerPreviewCamera"),
    ));

    commands.spawn((
        DespawnOnExit(AppUiMode::FangyuanPlayerPreview),
        DirectionalLight {
            illuminance: 7_500.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(-2.0, 4.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
        GlobalTransform::default(),
        FangyuanPlayerPreviewLight,
        Name::new("FangyuanPlayerPreviewDirectionalLight"),
    ));
}

fn spawn_fangyuan_player_preview_hud(
    commands: &mut Commands,
    theme: &UiTheme,
    metrics: &UiMetrics,
    viewport: &UiViewport,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
) {
    commands.spawn((
        DespawnOnExit(AppUiMode::FangyuanPlayerPreview),
        game_panel_root(
            PANEL_FANGYUAN_PLAYER_PREVIEW_HUD,
            UiPanelKind::Hud,
            OWNER_FANGYUAN_PLAYER_PREVIEW,
        ),
        UiLayerRoot {
            layer: UiLayer::Page,
        },
        fangyuan_player_preview_hud_root_node(viewport, metrics, theme),
        UiThemeRootNodeRole::Overlay,
        children![
            (
                UiThemePanelNodeRole::Content,
                fangyuan_player_preview_status_panel_node(viewport, theme),
                BackgroundColor(theme.colors.panel_background),
                BorderColor::all(theme.colors.panel_border),
                UiThemeBackgroundRole::Panel,
                UiThemeBorderRole::Panel,
                children![
                    screen_title_key(
                        theme,
                        fonts,
                        i18n,
                        "fangyuan_player_preview.hud.title",
                        "方圆玩家预览",
                        UiThemeTextStyleRole::Title,
                    ),
                    screen_label_key(
                        theme,
                        fonts,
                        i18n,
                        "fangyuan_player_preview.hud.status",
                        "最小玩家 Entity",
                        UiThemeTextStyleRole::Caption,
                        UiThemeTextColorRole::Muted,
                    ),
                ],
            ),
            (
                secondary_action_button_key(theme, metrics, fonts, i18n, "nav.lobby", "大厅",),
                fangyuan_player_preview_lobby_button_audio_override(),
                FangyuanPlayerPreviewLobbyButton,
            ),
        ],
    ));
}

fn fangyuan_player_preview_hud_root_node(
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

fn fangyuan_player_preview_status_panel_node(viewport: &UiViewport, theme: &UiTheme) -> Node {
    let compact = viewport.width_class == UiWidthClass::Compact;
    Node {
        width: if compact { percent(100) } else { auto() },
        max_width: px(if compact { 360.0 } else { 420.0 }),
        flex_direction: FlexDirection::Column,
        row_gap: px(theme.layout.row_gap),
        padding: UiRect::all(px(theme.layout.panel_gap)),
        border: UiRect::all(px(theme.panel.border)),
        border_radius: BorderRadius::all(px(theme.panel.radius)),
        ..default()
    }
}

fn fangyuan_player_preview_lobby_button_audio_override() -> UiAudioCueOverride {
    UiAudioCueOverride::try_from(UI_CONFIRM_CUE_ID)
        .expect("fangyuan player preview lobby button UI audio cue id must be valid")
}

pub(super) fn handle_fangyuan_player_preview_buttons(
    mut route_commands: MessageWriter<GameRouteCommand>,
    lobby_buttons: Query<(), With<FangyuanPlayerPreviewLobbyButton>>,
    mut button_events: MessageReader<UiButtonEvent>,
) {
    for event in button_events.read() {
        if event.kind != UiButtonEventKind::Click || !lobby_buttons.contains(event.entity) {
            continue;
        }

        route_commands.write(GameRouteCommand::ChangeMode(AppUiMode::Lobby));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::ui::widgets::UiButtonEvent;
    use bevy::ecs::message::{MessageCursor, Messages};

    #[test]
    fn lobby_button_writes_lobby_route() {
        let mut app = App::new();
        app.add_message::<GameRouteCommand>()
            .add_message::<UiButtonEvent>()
            .add_systems(Update, handle_fangyuan_player_preview_buttons);

        let lobby_button = app.world_mut().spawn(FangyuanPlayerPreviewLobbyButton).id();
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

        let route_commands = read_messages::<GameRouteCommand>(app.world());
        assert_eq!(route_commands.len(), 1);
        assert!(matches!(
            route_commands[0],
            GameRouteCommand::ChangeMode(AppUiMode::Lobby)
        ));
    }

    #[test]
    fn preview_camera_uses_scene_3d_order_and_identity_roll() {
        let mut app = App::new();
        app.add_systems(Startup, setup_fangyuan_player_preview_camera_and_light);
        app.update();

        let mut cameras = app
            .world_mut()
            .query_filtered::<(&Camera, &Transform), With<FangyuanPlayerPreviewCamera>>();
        let (camera, transform) = cameras.single(app.world()).unwrap();

        assert_eq!(camera.order, SCENE_CAMERA_3D_ORDER);
        assert_eq!(
            transform.translation,
            FANGYUAN_PLAYER_PREVIEW_CAMERA_TRANSLATION
        );
    }

    #[test]
    fn preview_lighting_spawns_directional_light() {
        let mut app = App::new();
        app.add_systems(Startup, setup_fangyuan_player_preview_camera_and_light);
        app.update();

        let mut lights = app
            .world_mut()
            .query_filtered::<&DirectionalLight, With<FangyuanPlayerPreviewLight>>();
        let light = lights.single(app.world()).unwrap();

        assert_eq!(light.illuminance, 7_500.0);
        assert!(!light.shadows_enabled);
    }

    #[test]
    fn lobby_button_uses_confirm_audio_override() {
        assert_eq!(
            fangyuan_player_preview_lobby_button_audio_override()
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
