mod ui_gallery;

use bevy::prelude::*;

use crate::game::{navigation::AppUiMode, ui::core::UiPanelSystems};

pub(super) struct DevScreensPlugin;

impl Plugin for DevScreensPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppUiMode::UiGallery), ui_gallery::setup_ui_gallery)
            .add_systems(
                OnExit(AppUiMode::UiGallery),
                ui_gallery::clear_ui_gallery_loading_preview,
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
            );
    }
}
