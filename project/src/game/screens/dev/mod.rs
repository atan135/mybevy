mod audio_gallery;
mod audio_monitor;
mod ui_document_gallery;
mod ui_gallery;
mod ui_generated_acceptance;

use bevy::prelude::*;

use crate::framework::{
    audio::prelude::AudioSystemSet,
    ui::core::{UiAnimationSystems, UiFocusSystems, UiPanelSystems},
};
use crate::game::navigation::AppUiMode;

pub(super) struct DevScreensPlugin;

impl Plugin for DevScreensPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppUiMode::UiGallery), ui_gallery::setup_ui_gallery)
            .add_systems(
                OnEnter(AppUiMode::UiDocumentGallery),
                ui_document_gallery::setup_ui_document_gallery,
            )
            .add_systems(
                OnEnter(AppUiMode::UiGeneratedAcceptance),
                ui_generated_acceptance::setup_ui_generated_acceptance,
            )
            .add_systems(
                OnEnter(AppUiMode::AudioGallery),
                (
                    audio_gallery::enable_audio_gallery_debug,
                    audio_gallery::setup_audio_gallery,
                )
                    .chain(),
            )
            .add_systems(
                OnEnter(AppUiMode::AudioMonitor),
                (
                    audio_monitor::enable_audio_monitor_debug,
                    audio_monitor::setup_audio_monitor,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    ui_gallery::apply_gallery_icon_state_previews,
                    ui_gallery::apply_gallery_control_state_previews,
                )
                    .after(UiFocusSystems::SyncFocusedMarkers)
                    .before(UiFocusSystems::Visuals)
                    .run_if(in_state(AppUiMode::UiGallery)),
            )
            .add_systems(
                Update,
                ui_gallery::apply_gallery_component_audit_state
                    .before(UiPanelSystems::Commands)
                    .run_if(in_state(AppUiMode::UiGallery)),
            )
            .add_systems(
                Update,
                ui_gallery::freeze_gallery_animation_audit_state
                    .before(UiAnimationSystems::Tick)
                    .run_if(in_state(AppUiMode::UiGallery)),
            )
            .add_systems(
                Update,
                audio_monitor::refresh_audio_monitor_text
                    .after(AudioSystemSet::Debug)
                    .run_if(in_state(AppUiMode::AudioMonitor)),
            )
            .add_systems(
                OnExit(AppUiMode::UiGallery),
                ui_gallery::clear_ui_gallery_loading_preview,
            )
            .add_systems(
                OnExit(AppUiMode::UiDocumentGallery),
                ui_document_gallery::cleanup_ui_document_gallery,
            )
            .add_systems(
                OnExit(AppUiMode::UiGeneratedAcceptance),
                ui_generated_acceptance::cleanup_ui_generated_acceptance,
            )
            .add_systems(
                OnExit(AppUiMode::AudioGallery),
                audio_gallery::cleanup_audio_gallery,
            )
            .add_systems(
                Update,
                audio_gallery::handle_audio_gallery_buttons
                    .before(AudioSystemSet::Commands)
                    .run_if(in_state(AppUiMode::AudioGallery)),
            )
            .add_systems(
                Update,
                (
                    audio_gallery::handle_audio_gallery_events,
                    audio_gallery::update_audio_gallery_status,
                )
                    .chain()
                    .after(AudioSystemSet::Debug)
                    .run_if(in_state(AppUiMode::AudioGallery)),
            )
            .add_systems(
                Update,
                (
                    ui_gallery::handle_ui_gallery_buttons,
                    ui_gallery::log_ui_gallery_text_input_submissions,
                    ui_gallery::tick_ui_gallery_loading_preview,
                )
                    .before(UiPanelSystems::Commands)
                    .run_if(in_state(AppUiMode::UiGallery)),
            )
            .add_systems(
                Update,
                ui_gallery::tag_gallery_floating_i18n_texts
                    .after(UiPanelSystems::Commands)
                    .run_if(in_state(AppUiMode::UiGallery)),
            );
    }
}
