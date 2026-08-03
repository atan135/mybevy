mod host;
mod model;
mod view;

#[cfg(test)]
mod tests;

use bevy::prelude::*;

use crate::framework::ui::{core::binding::UiBindingSystems, document::UiDocumentRuntimeSystems};
use crate::game::navigation::AppUiMode;

pub(super) struct AuthScreensPlugin;

impl Plugin for AuthScreensPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<host::LoginUiState>()
            .init_resource::<host::AuthHostContracts>()
            .add_systems(Startup, host::register_auth_contracts);

        #[cfg(all(debug_assertions, not(target_os = "android")))]
        app.add_systems(
            OnEnter(AppUiMode::CharacterSelect),
            (
                host::prepare_character_select_audit_fixture,
                view::setup_character_select_screen,
            )
                .chain(),
        );

        #[cfg(not(all(debug_assertions, not(target_os = "android"))))]
        app.add_systems(
            OnEnter(AppUiMode::CharacterSelect),
            view::setup_character_select_screen,
        );

        app.add_systems(OnExit(AppUiMode::Login), view::cleanup_login_screen_state)
            .add_systems(
                OnExit(AppUiMode::CharacterSelect),
                view::cleanup_login_screen_state,
            )
            .add_systems(Update, host::follow_myserver_login_events)
            .add_systems(
                Update,
                (
                    host::handle_login_document_actions.after(UiDocumentRuntimeSystems::Reconcile),
                    host::sync_login_document_bindings.before(UiBindingSystems::Apply),
                )
                    .chain()
                    .run_if(in_state(AppUiMode::Login)),
            )
            .add_systems(
                Update,
                (
                    host::handle_character_select_buttons,
                    view::sync_character_select_screen_state,
                    view::sync_character_select_button_flags,
                )
                    .chain()
                    .run_if(in_state(AppUiMode::CharacterSelect)),
            );
    }
}
