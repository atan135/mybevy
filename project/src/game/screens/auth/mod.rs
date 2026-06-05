mod login;

use bevy::prelude::*;

use crate::game::navigation::AppScreen;

pub(super) struct AuthScreensPlugin;

impl Plugin for AuthScreensPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppScreen::Login), login::setup_login_screen);
    }
}
