use bevy::prelude::*;

use crate::game::{
    navigation::AppScreen,
    ui::{
        theme::{PANEL_BACKGROUND, PANEL_BORDER, SCREEN_BACKGROUND, TEXT_MUTED},
        widgets::{primary_route_button, screen_label, screen_title},
    },
};

pub(super) fn setup_login_screen(mut commands: Commands, mut clear_color: ResMut<ClearColor>) {
    clear_color.0 = SCREEN_BACKGROUND;

    commands.spawn((
        DespawnOnExit(AppScreen::Login),
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::all(px(24)),
            ..default()
        },
        BackgroundColor(SCREEN_BACKGROUND),
        children![(
            Node {
                width: percent(100),
                max_width: px(420),
                flex_direction: FlexDirection::Column,
                row_gap: px(20),
                padding: UiRect::all(px(28)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(8)),
                ..default()
            },
            BackgroundColor(PANEL_BACKGROUND),
            BorderColor::all(PANEL_BORDER),
            children![
                screen_title("MyBevy", 44.0),
                screen_label("Player Login", 18.0, TEXT_MUTED),
                primary_route_button("Guest Login", AppScreen::GameList),
            ],
        )],
    ));
}
