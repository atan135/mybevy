use bevy::prelude::*;

use crate::game::{navigation::AppScreen, ui::widgets::secondary_route_button};

pub(super) fn setup_touch_ripple_overlay(mut commands: Commands) {
    commands.spawn((
        DespawnOnExit(AppScreen::TouchRipple),
        Node {
            width: percent(100),
            height: percent(100),
            padding: UiRect::all(px(16)),
            align_items: AlignItems::FlexStart,
            justify_content: JustifyContent::FlexEnd,
            ..default()
        },
        children![secondary_route_button("List", AppScreen::GameList)],
    ));
}
