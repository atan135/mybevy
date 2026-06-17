mod auth;
mod dev;
mod gameplay;
mod lobby;

use bevy::prelude::*;

use crate::framework::ui::core::UiFrameworkPlugin;
use crate::game::navigation::NavigationPlugin;

pub(super) struct ScreensPlugin;

impl Plugin for ScreensPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((NavigationPlugin, UiFrameworkPlugin))
            .add_plugins((auth::AuthScreensPlugin, lobby::LobbyScreensPlugin))
            .add_plugins(dev::DevScreensPlugin)
            .add_plugins(gameplay::GameplayScreensPlugin);
    }
}
