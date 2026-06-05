mod auth;
mod gameplay;
mod lobby;

use bevy::prelude::*;

use crate::game::navigation::NavigationPlugin;
use crate::game::ui::widgets::UiWidgetsPlugin;

pub(super) struct ScreensPlugin;

impl Plugin for ScreensPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((NavigationPlugin, UiWidgetsPlugin))
            .add_plugins((auth::AuthScreensPlugin, lobby::LobbyScreensPlugin))
            .add_plugins(gameplay::GameplayScreensPlugin);
    }
}
