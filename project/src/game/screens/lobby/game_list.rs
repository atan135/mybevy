use bevy::prelude::*;

use crate::game::{
    navigation::AppScreen,
    ui::{
        theme::{PANEL_BACKGROUND, PANEL_BORDER, SCREEN_BACKGROUND, TEXT_MUTED, TEXT_PRIMARY},
        widgets::{primary_route_button, screen_label, screen_title, secondary_route_button},
    },
};

pub(super) fn setup_game_list_screen(mut commands: Commands, mut clear_color: ResMut<ClearColor>) {
    clear_color.0 = SCREEN_BACKGROUND;

    commands.spawn((
        DespawnOnExit(AppScreen::GameList),
        Node {
            width: percent(100),
            height: percent(100),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(px(24)),
            row_gap: px(18),
            ..default()
        },
        BackgroundColor(SCREEN_BACKGROUND),
        children![
            (
                Node {
                    width: percent(100),
                    max_width: px(760),
                    align_self: AlignSelf::Center,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    column_gap: px(12),
                    ..default()
                },
                children![
                    screen_title("Game List", 34.0),
                    secondary_route_button("Logout", AppScreen::Login),
                ],
            ),
            (
                Node {
                    width: percent(100),
                    max_width: px(760),
                    align_self: AlignSelf::Center,
                    flex_direction: FlexDirection::Column,
                    row_gap: px(12),
                    padding: UiRect::all(px(20)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(8)),
                    ..default()
                },
                BackgroundColor(PANEL_BACKGROUND),
                BorderColor::all(PANEL_BORDER),
                children![
                    screen_label("Available", 16.0, TEXT_MUTED),
                    (
                        Node {
                            width: percent(100),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                            column_gap: px(16),
                            padding: UiRect::axes(px(0), px(8)),
                            ..default()
                        },
                        children![
                            (
                                Node {
                                    flex_direction: FlexDirection::Column,
                                    row_gap: px(6),
                                    flex_grow: 1.0,
                                    ..default()
                                },
                                children![
                                    screen_label("Touch Ripple", 24.0, TEXT_PRIMARY),
                                    screen_label("Current prototype", 15.0, TEXT_MUTED),
                                ],
                            ),
                            primary_route_button("Play", AppScreen::TouchRipple),
                        ],
                    ),
                ],
            ),
        ],
    ));
}
