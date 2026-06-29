mod login;

use bevy::prelude::*;

use crate::framework::ui::core::binding::UiBindingSystems;
use crate::game::navigation::AppUiMode;

pub(super) struct AuthScreensPlugin;

impl Plugin for AuthScreensPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<login::LoginUiState>()
            .add_systems(OnEnter(AppUiMode::Login), login::setup_login_screen)
            .add_systems(
                OnEnter(AppUiMode::CharacterSelect),
                login::setup_character_select_screen,
            )
            .add_systems(OnExit(AppUiMode::Login), login::cleanup_login_screen_state)
            .add_systems(
                OnExit(AppUiMode::CharacterSelect),
                login::cleanup_login_screen_state,
            )
            .add_systems(Update, login::follow_myserver_login_events)
            .add_systems(
                Update,
                (
                    login::handle_login_buttons,
                    login::sync_login_screen_state,
                    login::sync_login_button_flags,
                    login::sync_login_binding_values.before(UiBindingSystems::Apply),
                )
                    .chain()
                    .run_if(in_state(AppUiMode::Login)),
            )
            .add_systems(
                Update,
                (
                    login::handle_login_buttons,
                    login::sync_character_select_screen_state,
                    login::sync_login_button_flags,
                )
                    .chain()
                    .run_if(in_state(AppUiMode::CharacterSelect)),
            );
    }
}
