use bevy::prelude::*;

use crate::game::{
    navigation::AppScreen,
    plugin::TouchLaunchMode,
    ui::{
        theme::UiTheme,
        widgets::{
            primary_action_button, screen_label, screen_title, secondary_action_button,
            secondary_route_button,
        },
    },
};

#[derive(Component)]
pub(super) struct TouchRipplePlayButton;

#[derive(Clone, Copy, Component)]
pub(super) enum TouchRippleModeButton {
    SinglePlayer,
    Networked,
}

#[derive(Component)]
pub(super) struct TouchRippleCancelButton;

#[derive(Component)]
pub(super) struct TouchRippleConfirmDialog;

pub(super) fn setup_game_list_screen(
    mut commands: Commands,
    theme: Res<UiTheme>,
    mut clear_color: ResMut<ClearColor>,
) {
    let theme = theme.into_inner();
    clear_color.0 = theme.colors.screen_background;

    commands.spawn((
        DespawnOnExit(AppScreen::GameList),
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
                    secondary_route_button(theme, "Logout", AppScreen::Login),
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
            (
                TouchRippleConfirmDialog,
                Node {
                    display: Display::None,
                    position_type: PositionType::Absolute,
                    left: px(0),
                    right: px(0),
                    top: px(0),
                    bottom: px(0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    padding: UiRect::all(px(theme.layout.screen_padding)),
                    ..default()
                },
                ZIndex(10),
                BackgroundColor(Color::srgba(0.01, 0.02, 0.03, 0.72)),
                children![(
                    Node {
                        width: percent(100),
                        max_width: px(420),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(theme.layout.card_gap),
                        padding: UiRect::all(px(theme.panel.padding)),
                        border: UiRect::all(px(theme.panel.border)),
                        border_radius: BorderRadius::all(px(theme.panel.radius)),
                        ..default()
                    },
                    BackgroundColor(theme.colors.panel_background),
                    BorderColor::all(theme.colors.panel_border),
                    children![
                        screen_title(theme, "Touch Ripple", theme.text.subtitle),
                        screen_label(
                            "Start as a single-player session?",
                            theme.text.body,
                            theme.colors.text_primary,
                        ),
                        screen_label(
                            "Single player uses local authority only.",
                            theme.text.caption,
                            theme.colors.text_muted,
                        ),
                        (
                            Node {
                                width: percent(100),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::FlexEnd,
                                column_gap: px(theme.layout.row_column_gap),
                                margin: UiRect::top(px(theme.layout.row_gap)),
                                ..default()
                            },
                            children![
                                (
                                    secondary_action_button(theme, "Cancel"),
                                    TouchRippleCancelButton,
                                ),
                                (
                                    secondary_action_button(theme, "Networked"),
                                    TouchRippleModeButton::Networked,
                                ),
                                (
                                    primary_action_button(theme, "Single Player"),
                                    TouchRippleModeButton::SinglePlayer,
                                ),
                            ],
                        ),
                    ],
                )],
            ),
        ],
    ));
}

pub(super) fn handle_game_list_touch_buttons(
    mut next_screen: ResMut<NextState<AppScreen>>,
    mut launch_mode: ResMut<TouchLaunchMode>,
    play_buttons: Query<&Interaction, (Changed<Interaction>, With<TouchRipplePlayButton>)>,
    mode_buttons: Query<
        (&Interaction, &TouchRippleModeButton),
        (Changed<Interaction>, With<Button>),
    >,
    cancel_buttons: Query<&Interaction, (Changed<Interaction>, With<TouchRippleCancelButton>)>,
    mut dialogs: Query<&mut Node, With<TouchRippleConfirmDialog>>,
) {
    if play_buttons
        .iter()
        .any(|interaction| *interaction == Interaction::Pressed)
    {
        set_confirm_dialog_visible(&mut dialogs, true);
    }

    if cancel_buttons
        .iter()
        .any(|interaction| *interaction == Interaction::Pressed)
    {
        set_confirm_dialog_visible(&mut dialogs, false);
    }

    for (interaction, mode_button) in &mode_buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }

        *launch_mode = match mode_button {
            TouchRippleModeButton::SinglePlayer => TouchLaunchMode::SinglePlayer,
            TouchRippleModeButton::Networked => TouchLaunchMode::Auto,
        };
        set_confirm_dialog_visible(&mut dialogs, false);
        next_screen.set(AppScreen::TouchRipple);
    }
}

fn set_confirm_dialog_visible(
    dialogs: &mut Query<&mut Node, With<TouchRippleConfirmDialog>>,
    visible: bool,
) {
    for mut node in dialogs {
        node.display = if visible {
            Display::Flex
        } else {
            Display::None
        };
    }
}
