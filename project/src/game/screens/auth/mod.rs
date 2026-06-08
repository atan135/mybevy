mod login;

use bevy::prelude::*;

use crate::game::navigation::AppUiMode;

pub(super) struct AuthScreensPlugin;

impl Plugin for AuthScreensPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppUiMode::Login), login::setup_login_screen);
    }
}
