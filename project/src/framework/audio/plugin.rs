use bevy::prelude::*;

use super::{
    catalog::AudioCatalog, command::AudioCommand, debug::AudioDebugConfig, event::AudioEvent,
    mixer::AudioMixer, music::MusicController, playback::AudioPlaybackState,
};

pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<AudioCommand>()
            .add_message::<AudioEvent>()
            .init_resource::<AudioCatalog>()
            .init_resource::<AudioMixer>()
            .init_resource::<AudioPlaybackState>()
            .init_resource::<MusicController>()
            .init_resource::<AudioDebugConfig>()
            .configure_sets(
                Update,
                (
                    AudioSystemSet::Commands,
                    AudioSystemSet::Playback,
                    AudioSystemSet::Mixer,
                    AudioSystemSet::Cleanup,
                    AudioSystemSet::Debug,
                )
                    .chain(),
            );
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
pub enum AudioSystemSet {
    Commands,
    Playback,
    Mixer,
    Cleanup,
    Debug,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::audio::{AudioBus, AudioCommand, AudioEvent};

    #[test]
    fn audio_plugin_registers_messages_and_resources() {
        let mut app = App::new();
        app.add_plugins(AudioPlugin);

        assert!(app.world().contains_resource::<Messages<AudioCommand>>());
        assert!(app.world().contains_resource::<Messages<AudioEvent>>());
        assert!(app.world().contains_resource::<AudioCatalog>());
        assert!(app.world().contains_resource::<AudioMixer>());
        assert!(app.world().contains_resource::<AudioPlaybackState>());
        assert!(app.world().contains_resource::<MusicController>());
        assert!(app.world().contains_resource::<AudioDebugConfig>());

        let mixer = app.world().resource::<AudioMixer>();
        assert!(mixer.buses.contains_key(&AudioBus::Master));
        assert!(mixer.buses.contains_key(&AudioBus::Music));
        assert!(mixer.buses.contains_key(&AudioBus::Sfx));
        assert!(mixer.buses.contains_key(&AudioBus::Ui));
    }
}
