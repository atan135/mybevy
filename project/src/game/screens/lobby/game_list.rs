use bevy::prelude::*;

use crate::game::{
    navigation::AppUiMode,
    plugin::TouchLaunchMode,
    ui::{
        core::{
            UiLayer, UiLayerRoot, UiPanelCommand, UiPanelId, UiPanelKind, UiPanelRequest,
            UiPanelRoot,
        },
        i18n::UiI18n,
        overlays::{
            UiConfirmModal, UiI18nTextSpec, UiModalAction, UiModalActionSpec, UiModalActionStyle,
            UiModalId, UiModalResult, UiRouteCommand, UiToast,
        },
        style::{
            UiFontAssets, UiTheme,
            theme::{UiThemeBackgroundRole, UiThemeBorderRole, UiThemeTextColorRole},
        },
        widgets::{
            DisabledButton, LoadingButton, primary_action_button_key, screen_label_key,
            screen_title_key, secondary_route_button_key,
        },
    },
};

#[derive(Component)]
pub(super) struct TouchRipplePlayButton;

pub(super) fn setup_game_list_screen(
    mut commands: Commands,
    theme: Res<UiTheme>,
    fonts: Res<UiFontAssets>,
    i18n: Res<UiI18n>,
    mut clear_color: ResMut<ClearColor>,
) {
    let theme = theme.into_inner();
    let fonts = fonts.into_inner();
    let i18n = i18n.into_inner();
    clear_color.0 = theme.colors.screen_background;

    commands.spawn((
        DespawnOnExit(AppUiMode::Lobby),
        UiPanelRoot {
            id: UiPanelId::GameListPage,
            kind: UiPanelKind::Page,
            owner_mode: Some(AppUiMode::Lobby),
        },
        UiLayerRoot {
            layer: UiLayer::Page,
        },
        Node {
            width: percent(100),
            height: percent(100),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(px(theme.layout.screen_padding)),
            row_gap: px(theme.layout.page_gap),
            ..default()
        },
        BackgroundColor(theme.colors.screen_background),
        UiThemeBackgroundRole::Screen,
        children![
            (
                Node {
                    width: percent(100),
                    max_width: px(theme.layout.content_width),
                    align_self: AlignSelf::Center,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    column_gap: px(theme.layout.header_gap),
                    ..default()
                },
                children![
                    screen_title_key(
                        theme,
                        fonts,
                        i18n,
                        "lobby.title",
                        "Game List",
                        theme.text.title,
                    ),
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: px(theme.layout.row_column_gap),
                            ..default()
                        },
                        children![
                            secondary_route_button_key(
                                theme,
                                fonts,
                                i18n,
                                "nav.ui_gallery",
                                "UI Gallery",
                                AppUiMode::UiGallery,
                            ),
                            secondary_route_button_key(
                                theme,
                                fonts,
                                i18n,
                                "nav.logout",
                                "Logout",
                                AppUiMode::Login,
                            ),
                        ],
                    ),
                ],
            ),
            (
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
                        theme.text.section_label,
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
                                        theme.text.body,
                                        UiThemeTextColorRole::Primary,
                                    ),
                                    screen_label_key(
                                        theme,
                                        fonts,
                                        i18n,
                                        "lobby.touch_ripple.description",
                                        "Current prototype",
                                        theme.text.caption,
                                        UiThemeTextColorRole::Muted,
                                    ),
                                ],
                            ),
                            (
                                primary_action_button_key(
                                    theme,
                                    fonts,
                                    i18n,
                                    "lobby.play",
                                    "Play",
                                ),
                                TouchRipplePlayButton,
                            ),
                        ],
                    ),
                ],
            ),
        ],
    ));
}

pub(super) fn handle_game_list_touch_buttons(
    mut launch_mode: ResMut<TouchLaunchMode>,
    i18n: Res<UiI18n>,
    mut panel_commands: MessageWriter<UiPanelCommand>,
    mut route_commands: MessageWriter<UiRouteCommand>,
    mut modal_results: MessageReader<UiModalResult>,
    play_buttons: Query<
        &Interaction,
        (
            Changed<Interaction>,
            With<TouchRipplePlayButton>,
            Without<DisabledButton>,
            Without<LoadingButton>,
        ),
    >,
) {
    if play_buttons
        .iter()
        .any(|interaction| *interaction == Interaction::Pressed)
    {
        panel_commands.write(UiPanelCommand::Open(UiPanelRequest::Confirm(
            touch_ripple_confirm_modal(&i18n),
        )));
    }

    for result in modal_results.read() {
        if result.id != UiModalId::TouchRippleLaunch {
            continue;
        }

        match result.action {
            UiModalAction::Cancel | UiModalAction::Confirm => {}
            UiModalAction::TouchRippleSinglePlayer => {
                *launch_mode = TouchLaunchMode::SinglePlayer;
                route_commands.write(UiRouteCommand::ShowToast(UiToast::new_key(
                    &i18n,
                    "lobby.touch_ripple.toast.local",
                    "Starting local Touch Ripple",
                )));
                route_commands.write(UiRouteCommand::ChangeMode(AppUiMode::WanfaTouchRipple));
            }
            UiModalAction::TouchRippleNetworked => {
                *launch_mode = TouchLaunchMode::Auto;
                route_commands.write(UiRouteCommand::ShowToast(UiToast::new_key(
                    &i18n,
                    "lobby.touch_ripple.toast.networked",
                    "Starting networked Touch Ripple",
                )));
                route_commands.write(UiRouteCommand::ChangeMode(AppUiMode::WanfaTouchRipple));
            }
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
        id: UiModalId::TouchRippleLaunch,
        title: title.text,
        body: body.text,
        detail: Some(detail.text),
        title_i18n_text: Some(title.i18n_text),
        body_i18n_text: Some(body.i18n_text),
        detail_i18n_text: Some(detail.i18n_text),
        actions: vec![
            UiModalActionSpec {
                label: cancel.text,
                action: UiModalAction::Cancel,
                style: UiModalActionStyle::Secondary,
                i18n_text: Some(cancel.i18n_text),
            },
            UiModalActionSpec {
                label: networked.text,
                action: UiModalAction::TouchRippleNetworked,
                style: UiModalActionStyle::Secondary,
                i18n_text: Some(networked.i18n_text),
            },
            UiModalActionSpec {
                label: single_player.text,
                action: UiModalAction::TouchRippleSinglePlayer,
                style: UiModalActionStyle::Primary,
                i18n_text: Some(single_player.i18n_text),
            },
        ],
    }
}
