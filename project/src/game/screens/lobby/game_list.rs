use bevy::prelude::*;

use crate::game::{
    navigation::AppScreen,
    ui::{
        theme::UiTheme,
        widgets::{primary_route_button, screen_label, screen_title, secondary_route_button},
    },
};

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
                            primary_route_button(theme, "Play", AppScreen::TouchRipple),
                        ],
                    ),
                ],
            ),
        ],
    ));
}
