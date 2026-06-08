mod game_list;

use bevy::prelude::*;

use crate::game::navigation::AppScreen;

pub(super) struct LobbyScreensPlugin;

impl Plugin for LobbyScreensPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(AppScreen::GameList),
            game_list::setup_game_list_screen,
        )
        .add_systems(
            Update,
            game_list::handle_game_list_touch_buttons.run_if(in_state(AppScreen::GameList)),
        );
    }
}
