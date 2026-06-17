mod login;

use bevy::prelude::*;

use crate::framework::ui::core::binding::UiBindingSystems;
use crate::game::navigation::AppUiMode;

pub(super) struct AuthScreensPlugin;

impl Plugin for AuthScreensPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppUiMode::Login), login::setup_login_screen)
            .add_systems(
                Update,
                login::sync_login_binding_values
                    .before(UiBindingSystems::Apply)
                    .run_if(in_state(AppUiMode::Login)),
            );
    }
}
