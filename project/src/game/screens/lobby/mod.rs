mod game_list;

use bevy::prelude::*;

use crate::game::navigation::AppScreen;

pub(super) struct LobbyScreensPlugin;

impl Plugin for LobbyScreensPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(AppScreen::GameList),
            game_list::setup_game_list_screen,
        );
    }
}
