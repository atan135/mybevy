pub(in crate::game) mod audio;
pub(in crate::game) use audio::MAIN_WORLD_SETTINGS_ROUTE;

use bevy::prelude::*;

use crate::framework::{
    audio::prelude::AudioSystemSet,
    ui::{core::binding::UiBindingSystems, document::UiDocumentRuntimeSystems},
};
use crate::game::navigation::AppUiMode;

pub(super) struct SettingsScreensPlugin;

impl Plugin for SettingsScreensPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<audio::AudioSettingsHostContract>()
            .add_systems(Startup, audio::register_audio_settings_contract)
            .add_systems(
                OnExit(AppUiMode::AudioSettings),
                audio::cleanup_audio_settings_screen,
            )
            .add_systems(
                Update,
                audio::handle_audio_settings_document_actions
                    .after(UiDocumentRuntimeSystems::Reconcile)
                    .before(AudioSystemSet::Commands),
            )
            .add_systems(
                Update,
                audio::sync_audio_settings_document_bindings
                    .after(AudioSystemSet::Commands)
                    .before(UiBindingSystems::Apply)
                    .run_if(in_state(AppUiMode::AudioSettings)),
            );
        app.add_systems(
            Update,
            audio::sync_main_world_audio_settings_document_bindings
                .after(AudioSystemSet::Commands)
                .before(UiBindingSystems::Apply)
                .run_if(in_state(AppUiMode::MainWorld)),
        );

        #[cfg(all(debug_assertions, not(target_os = "android")))]
        app.init_resource::<audio::AudioSettingsAuditFixture>()
            .add_systems(
                Update,
                audio::mark_audio_settings_audit_scroll
                    .after(UiDocumentRuntimeSystems::Reconcile)
                    .run_if(in_state(AppUiMode::AudioSettings)),
            )
            .add_systems(
                OnEnter(AppUiMode::AudioSettings),
                audio::prepare_audio_settings_audit_fixture,
            )
            .add_systems(
                OnExit(AppUiMode::AudioSettings),
                audio::clear_audio_settings_audit_fixture,
            );
    }
}
