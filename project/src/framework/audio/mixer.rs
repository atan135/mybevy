use std::collections::HashMap;

use bevy::audio::{AudioSink, AudioSinkPlayback, Volume};
use bevy::prelude::*;

use super::{
    command::AudioCommand,
    event::{AudioBusChange, AudioBusChanged, AudioEvent},
    playback::{AudioInstanceState, AudioPlaybackInstance, AudioPlaybackState},
    scope::AudioBus,
    spatial::AudioSpatialEmitter,
};

pub const MIN_BUS_VOLUME: f32 = 0.0;
pub const MAX_BUS_VOLUME: f32 = 1.0;

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
                (AudioBus::Battle, AudioBusState::default()),
            ]
            .into(),
        }
    }
}

impl AudioMixer {
    pub fn bus_state(&self, bus: AudioBus) -> AudioBusState {
        self.buses.get(&bus).copied().unwrap_or_default()
    }

    pub fn set_bus_volume(&mut self, bus: AudioBus, volume: f32) -> (f32, f32) {
        let state = self.buses.entry(bus).or_default();
        let previous = state.volume;
        state.volume = clamp_bus_volume(volume);
        (previous, state.volume)
    }

    pub fn set_bus_muted(&mut self, bus: AudioBus, muted: bool) -> (bool, bool) {
        let state = self.buses.entry(bus).or_default();
        let previous = state.muted;
        state.muted = muted;
        (previous, state.muted)
    }

    pub fn set_bus_paused(&mut self, bus: AudioBus, paused: bool) -> (bool, bool) {
        let state = self.buses.entry(bus).or_default();
        let previous = state.paused;
        state.paused = paused;
        (previous, state.paused)
    }

    pub fn effective_bus_volume(&self, bus: AudioBus) -> f32 {
        if bus == AudioBus::Master {
            calculate_master_bus_volume(self.bus_state(AudioBus::Master))
        } else {
            calculate_effective_bus_volume(self.bus_state(AudioBus::Master), self.bus_state(bus))
        }
    }

    pub fn effective_bus_paused(&self, bus: AudioBus) -> bool {
        if bus == AudioBus::Master {
            self.bus_state(AudioBus::Master).paused
        } else {
            calculate_effective_bus_paused(self.bus_state(AudioBus::Master), self.bus_state(bus))
        }
    }

    pub fn target_instance_volume(&self, instance_volume: f32, bus: AudioBus) -> f32 {
        instance_volume.max(0.0) * self.effective_bus_volume(bus)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioBusState {
    pub volume: f32,
    pub muted: bool,
    pub paused: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioSinkSyncTarget {
    pub volume: f32,
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

pub fn clamp_bus_volume(volume: f32) -> f32 {
    if volume.is_finite() {
        volume.clamp(MIN_BUS_VOLUME, MAX_BUS_VOLUME)
    } else {
        MIN_BUS_VOLUME
    }
}

pub fn calculate_master_bus_volume(master: AudioBusState) -> f32 {
    if master.muted {
        MIN_BUS_VOLUME
    } else {
        clamp_bus_volume(master.volume)
    }
}

pub fn calculate_effective_bus_volume(master: AudioBusState, bus: AudioBusState) -> f32 {
    if master.muted || bus.muted {
        MIN_BUS_VOLUME
    } else {
        clamp_bus_volume(master.volume) * clamp_bus_volume(bus.volume)
    }
}

pub fn calculate_effective_bus_paused(master: AudioBusState, bus: AudioBusState) -> bool {
    master.paused || bus.paused
}

pub fn calculate_sink_sync_target(
    mixer: &AudioMixer,
    instance: &AudioInstanceState,
) -> AudioSinkSyncTarget {
    AudioSinkSyncTarget {
        volume: mixer.target_instance_volume(instance.volume, instance.bus),
        paused: instance.paused || mixer.effective_bus_paused(instance.bus),
    }
}

pub fn handle_audio_mixer_commands(
    mut audio_commands: MessageReader<AudioCommand>,
    mut audio_events: MessageWriter<AudioEvent>,
    mut mixer: ResMut<AudioMixer>,
) {
    for command in audio_commands.read() {
        match command {
            AudioCommand::SetBusVolume(command) => {
                let (previous, current) = mixer.set_bus_volume(command.bus, command.volume);
                if previous != current {
                    audio_events.write(AudioEvent::BusChanged(AudioBusChanged {
                        bus: command.bus,
                        change: AudioBusChange::Volume { previous, current },
                    }));
                }
            }
            AudioCommand::SetBusMuted(command) => {
                let (previous, current) = mixer.set_bus_muted(command.bus, command.muted);
                if previous != current {
                    audio_events.write(AudioEvent::BusChanged(AudioBusChanged {
                        bus: command.bus,
                        change: AudioBusChange::Muted { previous, current },
                    }));
                }
            }
            AudioCommand::SetBusPaused(command) => {
                let (previous, current) = mixer.set_bus_paused(command.bus, command.paused);
                if previous != current {
                    audio_events.write(AudioEvent::BusChanged(AudioBusChanged {
                        bus: command.bus,
                        change: AudioBusChange::Paused { previous, current },
                    }));
                }
            }
            _ => {}
        }
    }
}

pub fn sync_audio_sinks_with_mixer(
    mixer: Res<AudioMixer>,
    playback: Res<AudioPlaybackState>,
    mut sinks: Query<(&AudioPlaybackInstance, &mut AudioSink), Without<AudioSpatialEmitter>>,
) {
    for (playback_instance, mut sink) in &mut sinks {
        let Some(instance) = playback.instances.get(&playback_instance.instance_id) else {
            continue;
        };

        let target = calculate_sink_sync_target(&mixer, instance);
        sink.set_volume(Volume::Linear(target.volume));

        if target.paused {
            sink.pause();
        } else {
            sink.play();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::audio::{
        AudioBusMutedCommand, AudioBusPausedCommand, AudioBusVolumeCommand, AudioClipId,
        AudioCueId, AudioScope,
    };
    use bevy::ecs::message::MessageCursor;

    fn read_events(app: &App) -> Vec<AudioEvent> {
        let messages = app.world().resource::<Messages<AudioEvent>>();
        let mut cursor = MessageCursor::default();
        cursor.read(messages).cloned().collect()
    }

    #[test]
    fn bus_volume_is_clamped_to_supported_range() {
        assert_eq!(clamp_bus_volume(-0.5), 0.0);
        assert_eq!(clamp_bus_volume(0.4), 0.4);
        assert_eq!(clamp_bus_volume(2.0), 1.0);
        assert_eq!(clamp_bus_volume(f32::NAN), 0.0);
        assert_eq!(clamp_bus_volume(f32::INFINITY), 0.0);
    }

    #[test]
    fn effective_volume_combines_master_bus_and_muted_state() {
        let master = AudioBusState {
            volume: 0.5,
            muted: false,
            paused: false,
        };
        let music = AudioBusState {
            volume: 0.25,
            muted: false,
            paused: false,
        };

        assert_eq!(calculate_effective_bus_volume(master, music), 0.125);
        assert_eq!(
            calculate_effective_bus_volume(
                AudioBusState {
                    muted: true,
                    ..master
                },
                music,
            ),
            0.0
        );
        assert_eq!(
            calculate_effective_bus_volume(
                master,
                AudioBusState {
                    muted: true,
                    ..music
                },
            ),
            0.0
        );
    }

    #[test]
    fn master_bus_target_volume_applies_master_state_once() {
        let mut mixer = AudioMixer::default();
        mixer.set_bus_volume(AudioBus::Master, 0.5);

        assert_eq!(mixer.effective_bus_volume(AudioBus::Master), 0.5);
        assert_eq!(mixer.target_instance_volume(0.8, AudioBus::Master), 0.4);

        mixer.set_bus_muted(AudioBus::Master, true);
        assert_eq!(mixer.effective_bus_volume(AudioBus::Master), 0.0);
        assert_eq!(mixer.target_instance_volume(0.8, AudioBus::Master), 0.0);
    }

    #[test]
    fn effective_pause_combines_master_and_bus_state() {
        assert!(!calculate_effective_bus_paused(
            AudioBusState::default(),
            AudioBusState::default()
        ));
        assert!(calculate_effective_bus_paused(
            AudioBusState {
                paused: true,
                ..AudioBusState::default()
            },
            AudioBusState::default()
        ));
        assert!(calculate_effective_bus_paused(
            AudioBusState::default(),
            AudioBusState {
                paused: true,
                ..AudioBusState::default()
            },
        ));
    }

    #[test]
    fn mixer_commands_update_bus_state_and_emit_changes() {
        let mut app = App::new();
        app.add_message::<AudioCommand>()
            .add_message::<AudioEvent>()
            .init_resource::<AudioMixer>()
            .add_systems(Update, handle_audio_mixer_commands);

        app.world_mut()
            .write_message(AudioCommand::SetBusVolume(AudioBusVolumeCommand::new(
                AudioBus::Music,
                0.5,
            )));
        app.world_mut()
            .write_message(AudioCommand::SetBusMuted(AudioBusMutedCommand::new(
                AudioBus::Ui,
                true,
            )));
        app.world_mut()
            .write_message(AudioCommand::SetBusPaused(AudioBusPausedCommand::new(
                AudioBus::Sfx,
                true,
            )));

        app.update();

        let mixer = app.world().resource::<AudioMixer>();
        assert_eq!(mixer.bus_state(AudioBus::Music).volume, 0.5);
        assert!(mixer.bus_state(AudioBus::Ui).muted);
        assert!(mixer.bus_state(AudioBus::Sfx).paused);
        assert_eq!(
            read_events(&app),
            vec![
                AudioEvent::BusChanged(AudioBusChanged {
                    bus: AudioBus::Music,
                    change: AudioBusChange::Volume {
                        previous: 1.0,
                        current: 0.5,
                    },
                }),
                AudioEvent::BusChanged(AudioBusChanged {
                    bus: AudioBus::Ui,
                    change: AudioBusChange::Muted {
                        previous: false,
                        current: true,
                    },
                }),
                AudioEvent::BusChanged(AudioBusChanged {
                    bus: AudioBus::Sfx,
                    change: AudioBusChange::Paused {
                        previous: false,
                        current: true,
                    },
                }),
            ]
        );
    }

    #[test]
    fn running_music_and_ui_instances_follow_bus_volume_changes() {
        let mut mixer = AudioMixer::default();

        assert_eq!(mixer.target_instance_volume(0.8, AudioBus::Music), 0.8);
        assert_eq!(mixer.target_instance_volume(0.25, AudioBus::Ui), 0.25);

        mixer.set_bus_volume(AudioBus::Music, 0.5);
        mixer.set_bus_volume(AudioBus::Ui, 0.2);

        assert_eq!(mixer.target_instance_volume(0.8, AudioBus::Music), 0.4);
        assert_eq!(mixer.target_instance_volume(0.25, AudioBus::Ui), 0.05);

        mixer.set_bus_muted(AudioBus::Ui, true);
        assert_eq!(mixer.target_instance_volume(0.25, AudioBus::Ui), 0.0);
    }

    #[test]
    fn sink_sync_target_uses_base_instance_volume_without_mutating_it() {
        let mut mixer = AudioMixer::default();
        mixer.set_bus_volume(AudioBus::Music, 0.5);

        let instance = AudioInstanceState {
            entity: Entity::from_raw_u32(1).unwrap(),
            clip_id: AudioClipId::try_from("music.title").unwrap(),
            cue_id: Some(AudioCueId::try_from("music.title").unwrap()),
            scope: AudioScope::Global,
            bus: AudioBus::Music,
            volume: 0.8,
            priority: 0,
            asset_path: "audio/music/title.ogg".to_string(),
            source: Handle::<AudioSource>::default(),
            failed: false,
            paused: false,
            stopping: false,
            fade: None,
            spatial: false,
        };

        assert_eq!(
            calculate_sink_sync_target(&mixer, &instance),
            AudioSinkSyncTarget {
                volume: 0.4,
                paused: false,
            }
        );
        assert_eq!(instance.volume, 0.8);

        mixer.set_bus_muted(AudioBus::Music, true);
        assert_eq!(
            calculate_sink_sync_target(&mixer, &instance),
            AudioSinkSyncTarget {
                volume: 0.0,
                paused: false,
            }
        );

        mixer.set_bus_paused(AudioBus::Master, true);
        assert_eq!(
            calculate_sink_sync_target(&mixer, &instance),
            AudioSinkSyncTarget {
                volume: 0.0,
                paused: true,
            }
        );
    }

    #[test]
    fn sink_sync_target_is_available_when_late_sink_appears_without_resource_changes() {
        let mut mixer = AudioMixer::default();
        mixer.set_bus_volume(AudioBus::Ui, 0.25);
        mixer.set_bus_paused(AudioBus::Ui, true);
        let instance = AudioInstanceState {
            entity: Entity::from_raw_u32(2).unwrap(),
            clip_id: AudioClipId::try_from("ui.click").unwrap(),
            cue_id: None,
            scope: AudioScope::Ui,
            bus: AudioBus::Ui,
            volume: 0.8,
            priority: 0,
            asset_path: "audio/ui/click.ogg".to_string(),
            source: Handle::<AudioSource>::default(),
            failed: false,
            paused: false,
            stopping: false,
            fade: None,
            spatial: false,
        };

        assert_eq!(
            calculate_sink_sync_target(&mixer, &instance),
            AudioSinkSyncTarget {
                volume: 0.2,
                paused: true,
            }
        );
    }
}
