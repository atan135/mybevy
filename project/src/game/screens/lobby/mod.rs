mod game_list;

use bevy::prelude::*;

use crate::framework::ui::core::UiPanelSystems;
use crate::game::navigation::AppUiMode;

pub(super) struct LobbyScreensPlugin;

impl Plugin for LobbyScreensPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppUiMode::Lobby), game_list::setup_game_list_screen)
            .add_systems(
                Update,
                game_list::handle_game_list_touch_buttons
                    .before(UiPanelSystems::Commands)
                    .run_if(in_state(AppUiMode::Lobby)),
            );
    }
}
