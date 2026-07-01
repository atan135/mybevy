use bevy::prelude::*;

use crate::framework::{
    scene::prelude::{SceneCommand, SceneSwitchRequest},
    ui::{
        core::{
            UiLayer, UiLayerRoot, UiMetrics, UiPanelCommand, UiPanelKind, UiPanelRequest,
            UiViewport,
        },
        i18n::UiI18n,
        overlays::{
            UiConfirmModal, UiI18nTextSpec, UiModalActionSpec, UiModalActionStyle, UiModalResult,
            UiOverlayCommand, UiToast,
        },
        style::{
            UiFontAssets, UiTheme,
            theme::{
                UiThemeBackgroundRole, UiThemeBorderRole, UiThemePanelNodeRole,
                UiThemeRootNodeRole, UiThemeTextColorRole, UiThemeTextStyleRole,
            },
        },
        widgets::{
            UiButtonEvent, UiButtonEventKind, primary_action_button_key, screen_label_key,
            screen_title_key, secondary_action_button_key,
        },
    },
};
use crate::game::{
    features::touch_ripple::TouchLaunchMode,
    myserver::MyServerCommand,
    navigation::{AppUiMode, GameRouteCommand, game_panel_root, secondary_route_button_key},
    scenes::{FANGYUAN_HOME_SCENE_ID, ROBOT_SYNC_ARENA_SCENE_ID, SAMPLE_DUNGEON_ROOM_SCENE_ID},
    ui_ids::{
        ACTION_CANCEL, ACTION_CONFIRM, ACTION_TOUCH_RIPPLE_NETWORKED,
        ACTION_TOUCH_RIPPLE_SINGLE_PLAYER, MODAL_TOUCH_RIPPLE_LAUNCH, OWNER_LOBBY, PANEL_GAME_LIST,
    },
};

#[derive(Component)]
pub(super) struct TouchRipplePlayButton;

#[derive(Component)]
pub(super) struct SampleDungeonRoomPlayButton;

#[derive(Component)]
pub(super) struct RobotSyncArenaPlayButton;

#[derive(Component)]
pub(super) struct FangyuanHomePlayButton;

#[derive(Component)]
pub(super) struct FangyuanPlayerPreviewPlayButton;

#[derive(Component)]
pub(super) struct LobbyChangeCharacterButton;

#[derive(Component)]
pub(super) struct LobbyLogoutButton;

#[derive(Resource, Default)]
pub(super) struct SampleDungeonRoomEntryState {
    pending: bool,
}

impl SampleDungeonRoomEntryState {
    pub(super) fn clear(&mut self) {
        self.pending = false;
    }
}

#[derive(Resource, Default)]
pub(super) struct RobotSyncArenaEntryState {
    pending: bool,
}

impl RobotSyncArenaEntryState {
    #[cfg(test)]
    pub(super) fn is_pending(&self) -> bool {
        self.pending
    }

    #[cfg(test)]
    pub(super) fn set_pending_for_test(&mut self, pending: bool) {
        self.pending = pending;
    }

    pub(super) fn clear(&mut self) {
        self.pending = false;
    }
}

#[derive(Resource, Default)]
pub(super) struct FangyuanHomeEntryState {
    pending: bool,
}

impl FangyuanHomeEntryState {
    #[cfg(test)]
    pub(super) fn is_pending(&self) -> bool {
        self.pending
    }

    #[cfg(test)]
    pub(super) fn set_pending_for_test(&mut self, pending: bool) {
        self.pending = pending;
    }

    pub(super) fn clear(&mut self) {
        self.pending = false;
    }
}

pub(super) fn setup_game_list_screen(
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
    let fonts = fonts.into_inner();
    let i18n = i18n.into_inner();
    clear_color.0 = theme.colors.screen_background;

    commands.spawn((
        DespawnOnExit(AppUiMode::Lobby),
        game_panel_root(PANEL_GAME_LIST, UiPanelKind::Page, OWNER_LOBBY),
        UiLayerRoot {
            layer: UiLayer::Page,
        },
        Node {
            width: percent(100),
            height: percent(100),
            flex_direction: FlexDirection::Column,
            padding: viewport.safe_area_padding(metrics.page_padding),
            row_gap: px(theme.layout.page_gap),
            ..default()
        },
        BackgroundColor(theme.colors.screen_background),
        UiThemeBackgroundRole::Screen,
        UiThemeRootNodeRole::Screen,
        children![
            (
                Node {
                    width: percent(100),
                    max_width: px(theme.layout.content_width),
                    align_self: AlignSelf::Center,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    column_gap: px(theme.layout.header_gap),
                    row_gap: px(theme.layout.row_gap),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                },
                children![
                    screen_title_key(
                        theme,
                        fonts,
                        i18n,
                        "lobby.title",
                        "Game List",
                        UiThemeTextStyleRole::Title,
                    ),
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: px(theme.layout.row_column_gap),
                            row_gap: px(theme.layout.row_gap),
                            flex_wrap: FlexWrap::Wrap,
                            ..default()
                        },
                        children![
                            secondary_route_button_key(
                                theme,
                                metrics,
                                fonts,
                                i18n,
                                "nav.audio_settings",
                                "Audio Settings",
                                AppUiMode::AudioSettings,
                            ),
                            secondary_route_button_key(
                                theme,
                                metrics,
                                fonts,
                                i18n,
                                "nav.audio_monitor",
                                "Audio Monitor",
                                AppUiMode::AudioMonitor,
                            ),
                            secondary_route_button_key(
                                theme,
                                metrics,
                                fonts,
                                i18n,
                                "nav.audio_gallery",
                                "Audio Gallery",
                                AppUiMode::AudioGallery,
                            ),
                            secondary_route_button_key(
                                theme,
                                metrics,
                                fonts,
                                i18n,
                                "nav.ui_gallery",
                                "UI Gallery",
                                AppUiMode::UiGallery,
                            ),
                            (
                                secondary_action_button_key(
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    "nav.change_character",
                                    "Change Character",
                                ),
                                LobbyChangeCharacterButton,
                            ),
                            (
                                secondary_action_button_key(
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    "nav.logout",
                                    "Logout",
                                ),
                                LobbyLogoutButton,
                            ),
                        ],
                    ),
                ],
            ),
            (
                UiThemePanelNodeRole::Content,
                Node {
                    width: percent(100),
                    max_width: px(theme.layout.content_width),
                    align_self: AlignSelf::Center,
                    flex_direction: FlexDirection::Column,
                    row_gap: px(theme.layout.card_gap),
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
                    screen_label_key(
                        theme,
                        fonts,
                        i18n,
                        "lobby.available",
                        "Available",
                        UiThemeTextStyleRole::SectionLabel,
                        UiThemeTextColorRole::Muted,
                    ),
                    (
                        Node {
                            width: percent(100),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                            column_gap: px(theme.layout.row_column_gap),
                            padding: UiRect::axes(px(0), px(theme.layout.row_padding_y)),
                            ..default()
                        },
                        children![
                            (
                                Node {
                                    flex_direction: FlexDirection::Column,
                                    row_gap: px(theme.layout.row_gap),
                                    flex_grow: 1.0,
                                    ..default()
                                },
                                children![
                                    screen_label_key(
                                        theme,
                                        fonts,
                                        i18n,
                                        "lobby.touch_ripple.title",
                                        "Touch Ripple",
                                        UiThemeTextStyleRole::Body,
                                        UiThemeTextColorRole::Primary,
                                    ),
                                    screen_label_key(
                                        theme,
                                        fonts,
                                        i18n,
                                        "lobby.touch_ripple.description",
                                        "Current prototype",
                                        UiThemeTextStyleRole::Caption,
                                        UiThemeTextColorRole::Muted,
                                    ),
                                ],
                            ),
                            (
                                primary_action_button_key(
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    "lobby.play",
                                    "Play",
                                ),
                                TouchRipplePlayButton,
                            ),
                        ],
                    ),
                    (
                        Node {
                            width: percent(100),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                            column_gap: px(theme.layout.row_column_gap),
                            padding: UiRect::axes(px(0), px(theme.layout.row_padding_y)),
                            ..default()
                        },
                        children![
                            (
                                Node {
                                    flex_direction: FlexDirection::Column,
                                    row_gap: px(theme.layout.row_gap),
                                    flex_grow: 1.0,
                                    ..default()
                                },
                                children![
                                    screen_label_key(
                                        theme,
                                        fonts,
                                        i18n,
                                        "lobby.sample_scene.title",
                                        "Sample Scene",
                                        UiThemeTextStyleRole::Body,
                                        UiThemeTextColorRole::Primary,
                                    ),
                                    screen_label_key(
                                        theme,
                                        fonts,
                                        i18n,
                                        "lobby.sample_scene.description",
                                        "Dungeon room scene prototype",
                                        UiThemeTextStyleRole::Caption,
                                        UiThemeTextColorRole::Muted,
                                    ),
                                ],
                            ),
                            (
                                primary_action_button_key(
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    "lobby.enter",
                                    "Enter",
                                ),
                                SampleDungeonRoomPlayButton,
                            ),
                        ],
                    ),
                    (
                        Node {
                            width: percent(100),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                            column_gap: px(theme.layout.row_column_gap),
                            padding: UiRect::axes(px(0), px(theme.layout.row_padding_y)),
                            ..default()
                        },
                        children![
                            (
                                Node {
                                    flex_direction: FlexDirection::Column,
                                    row_gap: px(theme.layout.row_gap),
                                    flex_grow: 1.0,
                                    ..default()
                                },
                                children![
                                    screen_label_key(
                                        theme,
                                        fonts,
                                        i18n,
                                        "lobby.robot_sync_scene.title",
                                        "Robot Sync",
                                        UiThemeTextStyleRole::Body,
                                        UiThemeTextColorRole::Primary,
                                    ),
                                    screen_label_key(
                                        theme,
                                        fonts,
                                        i18n,
                                        "lobby.robot_sync_scene.description",
                                        "500x500 authority frame robot arena",
                                        UiThemeTextStyleRole::Caption,
                                        UiThemeTextColorRole::Muted,
                                    ),
                                ],
                            ),
                            (
                                primary_action_button_key(
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    "lobby.enter",
                                    "Enter",
                                ),
                                RobotSyncArenaPlayButton,
                            ),
                        ],
                    ),
                    (
                        Node {
                            width: percent(100),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                            column_gap: px(theme.layout.row_column_gap),
                            padding: UiRect::axes(px(0), px(theme.layout.row_padding_y)),
                            ..default()
                        },
                        children![
                            (
                                Node {
                                    flex_direction: FlexDirection::Column,
                                    row_gap: px(theme.layout.row_gap),
                                    flex_grow: 1.0,
                                    ..default()
                                },
                                children![
                                    screen_label_key(
                                        theme,
                                        fonts,
                                        i18n,
                                        "lobby.fangyuan_home.title",
                                        "方圆灵构家园原型",
                                        UiThemeTextStyleRole::Body,
                                        UiThemeTextColorRole::Primary,
                                    ),
                                    screen_label_key(
                                        theme,
                                        fonts,
                                        i18n,
                                        "lobby.fangyuan_home.description",
                                        "蓝图家园场景预览",
                                        UiThemeTextStyleRole::Caption,
                                        UiThemeTextColorRole::Muted,
                                    ),
                                ],
                            ),
                            (
                                primary_action_button_key(
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    "lobby.enter",
                                    "Enter",
                                ),
                                FangyuanHomePlayButton,
                            ),
                        ],
                    ),
                    (
                        Node {
                            width: percent(100),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                            column_gap: px(theme.layout.row_column_gap),
                            padding: UiRect::axes(px(0), px(theme.layout.row_padding_y)),
                            ..default()
                        },
                        children![
                            (
                                Node {
                                    flex_direction: FlexDirection::Column,
                                    row_gap: px(theme.layout.row_gap),
                                    flex_grow: 1.0,
                                    ..default()
                                },
                                children![
                                    screen_label_key(
                                        theme,
                                        fonts,
                                        i18n,
                                        "lobby.fangyuan_player_preview.title",
                                        "方圆玩家预览",
                                        UiThemeTextStyleRole::Body,
                                        UiThemeTextColorRole::Primary,
                                    ),
                                    screen_label_key(
                                        theme,
                                        fonts,
                                        i18n,
                                        "lobby.fangyuan_player_preview.description",
                                        "最小玩家 Entity 外观闭环",
                                        UiThemeTextStyleRole::Caption,
                                        UiThemeTextColorRole::Muted,
                                    ),
                                ],
                            ),
                            (
                                primary_action_button_key(
                                    theme,
                                    metrics,
                                    fonts,
                                    i18n,
                                    "lobby.enter",
                                    "Enter",
                                ),
                                FangyuanPlayerPreviewPlayButton,
                            ),
                        ],
                    ),
                ],
            ),
        ],
    ));
}

pub(super) fn handle_game_list_buttons(
    mut launch_mode: ResMut<TouchLaunchMode>,
    mut sample_scene_entry: ResMut<SampleDungeonRoomEntryState>,
    mut robot_sync_entry: ResMut<RobotSyncArenaEntryState>,
    mut fangyuan_home_entry: ResMut<FangyuanHomeEntryState>,
    i18n: Res<UiI18n>,
    mut scene_commands: MessageWriter<SceneCommand>,
    mut panel_commands: MessageWriter<UiPanelCommand>,
    mut overlay_commands: MessageWriter<UiOverlayCommand>,
    mut game_route_commands: MessageWriter<GameRouteCommand>,
    mut myserver_commands: MessageWriter<MyServerCommand>,
    mut modal_results: MessageReader<UiModalResult>,
    mut button_queries: ParamSet<(
        Query<(), With<TouchRipplePlayButton>>,
        Query<(), With<SampleDungeonRoomPlayButton>>,
        Query<(), With<RobotSyncArenaPlayButton>>,
        Query<(), With<FangyuanHomePlayButton>>,
        Query<(), With<FangyuanPlayerPreviewPlayButton>>,
        Query<(), With<LobbyChangeCharacterButton>>,
        Query<(), With<LobbyLogoutButton>>,
    )>,
    mut button_events: MessageReader<UiButtonEvent>,
) {
    for event in button_events.read() {
        if event.kind != UiButtonEventKind::Click {
            continue;
        }

        let entity = event.entity;
        let is_play = button_queries.p0().contains(entity);
        let is_sample_scene = button_queries.p1().contains(entity);
        let is_robot_sync = button_queries.p2().contains(entity);
        let is_fangyuan_home = button_queries.p3().contains(entity);
        let is_fangyuan_player_preview = button_queries.p4().contains(entity);
        let is_change_character = button_queries.p5().contains(entity);
        let is_logout = button_queries.p6().contains(entity);

        if is_play {
            panel_commands.write(UiPanelCommand::Open(UiPanelRequest::Confirm(
                touch_ripple_confirm_modal(&i18n),
            )));
        } else if is_sample_scene {
            if sample_scene_entry.pending {
                continue;
            }

            sample_scene_entry.pending = true;
            scene_commands.write(SceneCommand::Switch(SceneSwitchRequest::new(
                SAMPLE_DUNGEON_ROOM_SCENE_ID,
            )));
        } else if is_robot_sync {
            if robot_sync_entry.pending {
                continue;
            }

            robot_sync_entry.pending = true;
            scene_commands.write(SceneCommand::Switch(SceneSwitchRequest::new(
                ROBOT_SYNC_ARENA_SCENE_ID,
            )));
        } else if is_fangyuan_home {
            if fangyuan_home_entry.pending {
                continue;
            }

            fangyuan_home_entry.pending = true;
            scene_commands.write(SceneCommand::Switch(SceneSwitchRequest::new(
                FANGYUAN_HOME_SCENE_ID,
            )));
        } else if is_fangyuan_player_preview {
            game_route_commands.write(GameRouteCommand::ChangeMode(
                AppUiMode::FangyuanPlayerPreview,
            ));
        } else if is_change_character {
            myserver_commands.write(MyServerCommand::SwitchCharacter);
            game_route_commands.write(GameRouteCommand::ChangeMode(AppUiMode::CharacterSelect));
        } else if is_logout {
            myserver_commands.write(MyServerCommand::Logout);
            game_route_commands.write(GameRouteCommand::ChangeMode(AppUiMode::Login));
        }
    }

    for result in modal_results.read() {
        if result.id != MODAL_TOUCH_RIPPLE_LAUNCH {
            continue;
        }

        match result.action {
            ACTION_CANCEL | ACTION_CONFIRM => {}
            ACTION_TOUCH_RIPPLE_SINGLE_PLAYER => {
                *launch_mode = TouchLaunchMode::SinglePlayer;
                overlay_commands.write(UiOverlayCommand::ShowToast(UiToast::new_key(
                    &i18n,
                    "lobby.touch_ripple.toast.local",
                    "Starting local Touch Ripple",
                )));
                game_route_commands
                    .write(GameRouteCommand::ChangeMode(AppUiMode::WanfaTouchRipple));
            }
            ACTION_TOUCH_RIPPLE_NETWORKED => {
                *launch_mode = TouchLaunchMode::Auto;
                overlay_commands.write(UiOverlayCommand::ShowToast(UiToast::new_key(
                    &i18n,
                    "lobby.touch_ripple.toast.networked",
                    "Starting networked Touch Ripple",
                )));
                game_route_commands
                    .write(GameRouteCommand::ChangeMode(AppUiMode::WanfaTouchRipple));
            }
            _ => {}
        };
    }
}

fn touch_ripple_confirm_modal(i18n: &UiI18n) -> UiConfirmModal {
    let title = UiI18nTextSpec::new(i18n, "lobby.touch_ripple.confirm.title", "Touch Ripple");
    let body = UiI18nTextSpec::new(
        i18n,
        "lobby.touch_ripple.confirm.body",
        "Start as a single-player session?",
    );
    let detail = UiI18nTextSpec::new(
        i18n,
        "lobby.touch_ripple.confirm.detail",
        "Single player uses local authority only.",
    );
    let cancel = UiI18nTextSpec::new(i18n, "common.cancel", "Cancel");
    let networked = UiI18nTextSpec::new(i18n, "lobby.touch_ripple.confirm.networked", "Networked");
    let single_player = UiI18nTextSpec::new(
        i18n,
        "lobby.touch_ripple.confirm.single_player",
        "Single Player",
    );

    UiConfirmModal {
        id: MODAL_TOUCH_RIPPLE_LAUNCH,
        title: title.text,
        body: body.text,
        detail: Some(detail.text),
        title_i18n_text: Some(title.i18n_text),
        body_i18n_text: Some(body.i18n_text),
        detail_i18n_text: Some(detail.i18n_text),
        actions: vec![
            UiModalActionSpec {
                label: cancel.text,
                action: ACTION_CANCEL,
                style: UiModalActionStyle::Secondary,
                i18n_text: Some(cancel.i18n_text),
            },
            UiModalActionSpec {
                label: networked.text,
                action: ACTION_TOUCH_RIPPLE_NETWORKED,
                style: UiModalActionStyle::Secondary,
                i18n_text: Some(networked.i18n_text),
            },
            UiModalActionSpec {
                label: single_player.text,
                action: ACTION_TOUCH_RIPPLE_SINGLE_PLAYER,
                style: UiModalActionStyle::Primary,
                i18n_text: Some(single_player.i18n_text),
            },
        ],
    }
}
