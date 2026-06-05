mod auth;
mod gameplay;
mod lobby;

use bevy::prelude::*;

use crate::game::{
    navigation::NavigationPlugin,
    ui::{theme::UiThemePlugin, widgets::UiWidgetsPlugin},
};

pub(super) struct ScreensPlugin;

impl Plugin for ScreensPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((NavigationPlugin, UiThemePlugin, UiWidgetsPlugin))
            .add_plugins((auth::AuthScreensPlugin, lobby::LobbyScreensPlugin))
            .add_plugins(gameplay::GameplayScreensPlugin);
    }
}
