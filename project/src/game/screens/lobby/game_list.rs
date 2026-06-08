use bevy::prelude::*;

use crate::game::{
    navigation::AppUiMode,
    plugin::TouchLaunchMode,
    ui::{
        core::{
            UiLayer, UiLayerRoot, UiPanelCommand, UiPanelId, UiPanelKind, UiPanelRequest,
            UiPanelRoot,
        },
        overlays::{
            UiConfirmModal, UiModalAction, UiModalActionSpec, UiModalActionStyle, UiModalId,
            UiModalResult, UiRouteCommand, UiToast,
        },
        style::UiTheme,
        widgets::{
            DisabledButton, LoadingButton, primary_action_button, screen_label, screen_title,
            secondary_route_button,
        },
    },
};

#[derive(Component)]
pub(super) struct TouchRipplePlayButton;

pub(super) fn setup_game_list_screen(
    mut commands: Commands,
    theme: Res<UiTheme>,
    mut clear_color: ResMut<ClearColor>,
) {
    let theme = theme.into_inner();
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
                    screen_title(theme, "Game List", theme.text.title),
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: px(theme.layout.row_column_gap),
                            ..default()
                        },
                        children![
                            secondary_route_button(theme, "UI Gallery", AppUiMode::UiGallery),
                            secondary_route_button(theme, "Logout", AppUiMode::Login),
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
                children![
                    screen_label(
                        "Available",
                        theme.text.section_label,
                        theme.colors.text_muted
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
                                    screen_label(
                                        "Touch Ripple",
                                        theme.text.body,
                                        theme.colors.text_primary,
                                    ),
                                    screen_label(
                                        "Current prototype",
                                        theme.text.caption,
                                        theme.colors.text_muted,
                                    ),
                                ],
                            ),
                            (primary_action_button(theme, "Play"), TouchRipplePlayButton,),
                        ],
                    ),
                ],
            ),
        ],
    ));
}

pub(super) fn handle_game_list_touch_buttons(
    mut launch_mode: ResMut<TouchLaunchMode>,
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
            touch_ripple_confirm_modal(),
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
                route_commands.write(UiRouteCommand::ShowToast(UiToast::new(
                    "Starting local Touch Ripple",
                )));
                route_commands.write(UiRouteCommand::ChangeMode(AppUiMode::WanfaTouchRipple));
            }
            UiModalAction::TouchRippleNetworked => {
                *launch_mode = TouchLaunchMode::Auto;
                route_commands.write(UiRouteCommand::ShowToast(UiToast::new(
                    "Starting networked Touch Ripple",
                )));
                route_commands.write(UiRouteCommand::ChangeMode(AppUiMode::WanfaTouchRipple));
            }
        };
    }
}

fn touch_ripple_confirm_modal() -> UiConfirmModal {
    UiConfirmModal {
        id: UiModalId::TouchRippleLaunch,
        title: "Touch Ripple".to_string(),
        body: "Start as a single-player session?".to_string(),
        detail: Some("Single player uses local authority only.".to_string()),
        actions: vec![
            UiModalActionSpec {
                label: "Cancel".to_string(),
                action: UiModalAction::Cancel,
                style: UiModalActionStyle::Secondary,
            },
            UiModalActionSpec {
                label: "Networked".to_string(),
                action: UiModalAction::TouchRippleNetworked,
                style: UiModalActionStyle::Secondary,
            },
            UiModalActionSpec {
                label: "Single Player".to_string(),
                action: UiModalAction::TouchRippleSinglePlayer,
                style: UiModalActionStyle::Primary,
            },
        ],
    }
}
