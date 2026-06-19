use std::collections::HashMap;

use bevy::prelude::*;

use crate::framework::ui::widgets::{UiButtonEvent, UiButtonEventKind};

use super::{
    command::{AudioCommand, AudioCueRequest},
    event::{AudioCueSkipReason, AudioCueSkipped, AudioEvent},
    id::AudioCueId,
    mixer::AudioMixer,
    scope::{AudioBus, AudioScope},
};

pub const DEFAULT_UI_CLICK_CUE_ID: &str = "ui.click";
pub const DEFAULT_UI_CUE_COOLDOWN_SECONDS: f32 = 0.05;

#[derive(Clone, Debug, Component, PartialEq, Eq)]
pub struct UiAudioCueOverride {
    pub cue_id: AudioCueId,
}

impl UiAudioCueOverride {
    pub fn new(cue_id: AudioCueId) -> Self {
        Self { cue_id }
    }
}

impl TryFrom<&str> for UiAudioCueOverride {
    type Error = super::id::AudioIdError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(Self::new(AudioCueId::try_from(value)?))
    }
}

#[derive(Debug, Resource)]
pub struct UiAudioAdapterConfig {
    pub default_click_cue_id: AudioCueId,
    pub cooldown_seconds: f32,
}

impl Default for UiAudioAdapterConfig {
    fn default() -> Self {
        Self {
            default_click_cue_id: AudioCueId::try_from(DEFAULT_UI_CLICK_CUE_ID)
                .expect("default UI click cue id must be valid"),
            cooldown_seconds: DEFAULT_UI_CUE_COOLDOWN_SECONDS,
        }
    }
}

#[derive(Debug, Default, Resource)]
pub struct UiAudioCooldowns {
    last_trigger_seconds_by_cue: HashMap<AudioCueId, f64>,
}

impl UiAudioCooldowns {
    pub fn clear(&mut self) {
        self.last_trigger_seconds_by_cue.clear();
    }
}

pub fn play_ui_button_audio(
    mut button_events: MessageReader<UiButtonEvent>,
    mut audio_commands: MessageWriter<AudioCommand>,
    mut audio_events: MessageWriter<AudioEvent>,
    time: Res<Time>,
    mixer: Res<AudioMixer>,
    config: Res<UiAudioAdapterConfig>,
    mut cooldowns: ResMut<UiAudioCooldowns>,
    cue_overrides: Query<&UiAudioCueOverride>,
) {
    let now_seconds = time.elapsed_secs_f64();

    for event in button_events.read() {
        if event.kind != UiButtonEventKind::Click {
            continue;
        }

        let cue_id = cue_overrides
            .get(event.entity)
            .map(|cue_override| cue_override.cue_id.clone())
            .unwrap_or_else(|_| config.default_click_cue_id.clone());

        if mixer.effective_bus_volume(AudioBus::Ui) <= 0.0
            || mixer.effective_bus_paused(AudioBus::Ui)
        {
            audio_events.write(AudioEvent::CueSkipped(AudioCueSkipped {
                cue_id,
                reason: AudioCueSkipReason::BusPaused,
                scope: AudioScope::Ui,
            }));
            continue;
        }

        let cooldown_seconds = config.cooldown_seconds.max(0.0) as f64;
        if let Some(last_trigger_seconds) =
            cooldowns.last_trigger_seconds_by_cue.get(&cue_id).copied()
        {
            if now_seconds - last_trigger_seconds < cooldown_seconds {
                audio_events.write(AudioEvent::CueSkipped(AudioCueSkipped {
                    cue_id,
                    reason: AudioCueSkipReason::Cooldown,
                    scope: AudioScope::Ui,
                }));
                continue;
            }
        }

        cooldowns
            .last_trigger_seconds_by_cue
            .insert(cue_id.clone(), now_seconds);

        audio_commands.write(AudioCommand::PlayCue(AudioCueRequest {
            cue_id,
            scope: AudioScope::Ui,
            bus: Some(AudioBus::Ui),
            volume: 1.0,
            pitch: 1.0,
            looped: false,
            fade_in_seconds: None,
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::message::MessageCursor;

    fn cue_id(value: &str) -> AudioCueId {
        AudioCueId::try_from(value).unwrap()
    }

    fn adapter_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<UiButtonEvent>()
            .add_message::<AudioCommand>()
            .add_message::<AudioEvent>()
            .init_resource::<AudioMixer>()
            .init_resource::<UiAudioAdapterConfig>()
            .init_resource::<UiAudioCooldowns>()
            .add_systems(Update, play_ui_button_audio);
        app
    }

    fn read_commands(app: &App) -> Vec<AudioCommand> {
        let messages = app.world().resource::<Messages<AudioCommand>>();
        let mut cursor = MessageCursor::default();
        cursor.read(messages).cloned().collect()
    }

    fn read_events(app: &App) -> Vec<AudioEvent> {
        let messages = app.world().resource::<Messages<AudioEvent>>();
        let mut cursor = MessageCursor::default();
        cursor.read(messages).cloned().collect()
    }

    fn click_button(app: &mut App, entity: Entity) {
        app.world_mut().write_message(UiButtonEvent {
            entity,
            kind: UiButtonEventKind::Click,
            button: None,
        });
    }

    #[test]
    fn ui_button_click_writes_default_ui_cue_command() {
        let mut app = adapter_app();
        let button = app.world_mut().spawn_empty().id();

        click_button(&mut app, button);
        app.update();

        assert_eq!(
            read_commands(&app),
            vec![AudioCommand::PlayCue(AudioCueRequest {
                cue_id: cue_id(DEFAULT_UI_CLICK_CUE_ID),
                scope: AudioScope::Ui,
                bus: Some(AudioBus::Ui),
                volume: 1.0,
                pitch: 1.0,
                looped: false,
                fade_in_seconds: None,
            })]
        );
    }

    #[test]
    fn ui_button_cue_override_replaces_default_cue() {
        let mut app = adapter_app();
        let button = app
            .world_mut()
            .spawn(UiAudioCueOverride::try_from("ui.confirm").unwrap())
            .id();

        click_button(&mut app, button);
        app.update();

        assert_eq!(
            read_commands(&app),
            vec![AudioCommand::PlayCue(AudioCueRequest {
                cue_id: cue_id("ui.confirm"),
                scope: AudioScope::Ui,
                bus: Some(AudioBus::Ui),
                volume: 1.0,
                pitch: 1.0,
                looped: false,
                fade_in_seconds: None,
            })]
        );
    }

    #[test]
    fn ui_button_audio_ignores_non_click_button_events() {
        let mut app = adapter_app();
        let button = app.world_mut().spawn_empty().id();

        app.world_mut().write_message(UiButtonEvent {
            entity: button,
            kind: UiButtonEventKind::Down,
            button: None,
        });
        app.update();

        assert!(read_commands(&app).is_empty());
    }

    #[test]
    fn ui_button_audio_cooldown_skips_repeated_cue() {
        let mut app = adapter_app();
        let button = app.world_mut().spawn_empty().id();

        click_button(&mut app, button);
        click_button(&mut app, button);
        app.update();

        assert_eq!(read_commands(&app).len(), 1);
        assert_eq!(
            read_events(&app),
            vec![AudioEvent::CueSkipped(AudioCueSkipped {
                cue_id: cue_id(DEFAULT_UI_CLICK_CUE_ID),
                reason: AudioCueSkipReason::Cooldown,
                scope: AudioScope::Ui,
            })]
        );
    }

    #[test]
    fn ui_button_audio_cooldown_is_per_cue() {
        let mut app = adapter_app();
        let default_button = app.world_mut().spawn_empty().id();
        let override_button = app
            .world_mut()
            .spawn(UiAudioCueOverride::try_from("ui.cancel").unwrap())
            .id();

        click_button(&mut app, default_button);
        click_button(&mut app, override_button);
        app.update();

        assert_eq!(read_commands(&app).len(), 2);
    }

    #[test]
    fn muted_ui_bus_skips_button_audio_without_play_command() {
        let mut app = adapter_app();
        let button = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<AudioMixer>()
            .set_bus_muted(AudioBus::Ui, true);

        click_button(&mut app, button);
        app.update();

        assert!(read_commands(&app).is_empty());
        assert_eq!(
            read_events(&app),
            vec![AudioEvent::CueSkipped(AudioCueSkipped {
                cue_id: cue_id(DEFAULT_UI_CLICK_CUE_ID),
                reason: AudioCueSkipReason::BusPaused,
                scope: AudioScope::Ui,
            })]
        );
    }

    #[test]
    fn muted_master_bus_skips_button_audio_without_play_command() {
        let mut app = adapter_app();
        let button = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<AudioMixer>()
            .set_bus_muted(AudioBus::Master, true);

        click_button(&mut app, button);
        app.update();

        assert!(read_commands(&app).is_empty());
        assert_eq!(
            read_events(&app),
            vec![AudioEvent::CueSkipped(AudioCueSkipped {
                cue_id: cue_id(DEFAULT_UI_CLICK_CUE_ID),
                reason: AudioCueSkipReason::BusPaused,
                scope: AudioScope::Ui,
            })]
        );

        app.world_mut()
            .resource_mut::<AudioMixer>()
            .set_bus_muted(AudioBus::Master, false);
        click_button(&mut app, button);
        app.update();

        assert_eq!(read_commands(&app).len(), 1);
    }
}
