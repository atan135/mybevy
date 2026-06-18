use std::collections::HashMap;

use bevy::prelude::*;

use super::scope::AudioBus;

#[derive(Debug, Resource)]
pub struct AudioMixer {
    pub buses: HashMap<AudioBus, AudioBusState>,
}

impl Default for AudioMixer {
    fn default() -> Self {
        Self {
            buses: [
                (AudioBus::Master, AudioBusState::default()),
                (AudioBus::Music, AudioBusState::default()),
                (AudioBus::Sfx, AudioBusState::default()),
                (AudioBus::Ui, AudioBusState::default()),
            ]
            .into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioBusState {
    pub volume: f32,
    pub muted: bool,
    pub paused: bool,
}

impl Default for AudioBusState {
    fn default() -> Self {
        Self {
            volume: 1.0,
            muted: false,
            paused: false,
        }
    }
}
