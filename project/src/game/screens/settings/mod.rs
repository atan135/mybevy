mod audio;

use bevy::prelude::*;

use crate::framework::ui::core::UiPanelSystems;
use crate::game::navigation::AppUiMode;

pub(super) struct SettingsScreensPlugin;

impl Plugin for SettingsScreensPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(AppUiMode::AudioSettings),
            audio::setup_audio_settings,
        )
        .add_systems(
            Update,
            (
                audio::handle_audio_settings_sliders,
                audio::handle_audio_settings_master_mute_toggle,
            )
                .before(UiPanelSystems::Commands)
                .run_if(in_state(AppUiMode::AudioSettings)),
        );
    }
}
