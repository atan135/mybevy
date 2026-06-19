use std::collections::HashMap;

use bevy::prelude::*;

use crate::framework::scene::prelude::{SceneEvent, SceneId, SceneSessionId};

use super::{
    command::{AudioCommand, AudioCueRequest, AudioMusicRequest, AudioScopeFadeCommand},
    id::{AudioClipId, AudioCueId},
    scope::{AudioBus, AudioScope},
};

#[derive(Clone, Debug, Resource, Default)]
pub struct SceneAudioAdapterConfig {
    entries: HashMap<SceneId, SceneAudioEntry>,
}

impl SceneAudioAdapterConfig {
    pub fn register(&mut self, scene_id: impl Into<SceneId>, entry: SceneAudioEntry) {
        self.entries.insert(scene_id.into(), entry);
    }

    pub fn get(&self, scene_id: &SceneId) -> Option<&SceneAudioEntry> {
        self.entries.get(scene_id)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneAudioEntry {
    pub on_enter: Vec<SceneAudioPlayback>,
    pub exit_fade_out_seconds: Option<f32>,
}

impl SceneAudioEntry {
    pub fn from_playback(playback: SceneAudioPlayback) -> Self {
        Self {
            on_enter: vec![playback],
            exit_fade_out_seconds: None,
        }
    }

    pub fn with_exit_fade_out_seconds(mut self, seconds: f32) -> Self {
        self.exit_fade_out_seconds = Some(seconds.max(0.0));
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SceneAudioPlayback {
    Cue(SceneAudioCue),
    Music(SceneAudioMusic),
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneAudioCue {
    pub cue_id: AudioCueId,
    pub bus: Option<AudioBus>,
    pub volume: f32,
    pub pitch: f32,
    pub looped: bool,
    pub fade_in_seconds: Option<f32>,
}

impl SceneAudioCue {
    pub fn new(cue_id: AudioCueId) -> Self {
        Self {
            cue_id,
            bus: None,
            volume: 1.0,
            pitch: 1.0,
            looped: false,
            fade_in_seconds: None,
        }
    }

    pub fn ambience(cue_id: AudioCueId) -> Self {
        Self {
            bus: Some(AudioBus::Sfx),
            looped: true,
            ..Self::new(cue_id)
        }
    }

    pub fn with_bus(mut self, bus: AudioBus) -> Self {
        self.bus = Some(bus);
        self
    }

    pub fn with_volume(mut self, volume: f32) -> Self {
        self.volume = volume.max(0.0);
        self
    }

    pub fn with_pitch(mut self, pitch: f32) -> Self {
        self.pitch = pitch.max(0.01);
        self
    }

    pub fn looped(mut self, looped: bool) -> Self {
        self.looped = looped;
        self
    }

    pub fn with_fade_in_seconds(mut self, seconds: f32) -> Self {
        self.fade_in_seconds = Some(seconds.max(0.0));
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneAudioMusic {
    pub clip_id: AudioClipId,
    pub volume: f32,
    pub looped: bool,
    pub fade_in_seconds: Option<f32>,
}

impl SceneAudioMusic {
    pub fn new(clip_id: AudioClipId) -> Self {
        Self {
            clip_id,
            volume: 1.0,
            looped: true,
            fade_in_seconds: None,
        }
    }

    pub fn with_volume(mut self, volume: f32) -> Self {
        self.volume = volume.max(0.0);
        self
    }

    pub fn looped(mut self, looped: bool) -> Self {
        self.looped = looped;
        self
    }

    pub fn with_fade_in_seconds(mut self, seconds: f32) -> Self {
        self.fade_in_seconds = Some(seconds.max(0.0));
        self
    }
}

pub fn play_scene_audio_on_lifecycle(
    mut scene_events: MessageReader<SceneEvent>,
    mut audio_commands: MessageWriter<AudioCommand>,
    config: Res<SceneAudioAdapterConfig>,
) {
    for event in scene_events.read() {
        match event {
            SceneEvent::Entered(entered) => {
                let Some(entry) = config.get(&entered.scene_id) else {
                    continue;
                };
                let Some(scope) = scene_scope(&entered.session_id) else {
                    warn!(
                        "skipping scene audio for session `{}` because it is not a valid audio scope id",
                        entered.session_id
                    );
                    continue;
                };

                for playback in &entry.on_enter {
                    audio_commands.write(playback.command(scope.clone()));
                }
            }
            SceneEvent::ExitStarted(exited) => {
                let fade_out_seconds = config
                    .get(&exited.scene_id)
                    .and_then(|entry| entry.exit_fade_out_seconds);
                if let Some(command) =
                    stop_scene_scope_command(&exited.session_id, fade_out_seconds)
                {
                    audio_commands.write(command);
                }
            }
            SceneEvent::Exited(exited) => {
                if let Some(command) = stop_scene_scope_command(&exited.session_id, None) {
                    audio_commands.write(command);
                }
            }
            _ => {}
        }
    }
}

impl SceneAudioPlayback {
    fn command(&self, scope: AudioScope) -> AudioCommand {
        match self {
            Self::Cue(cue) => AudioCommand::PlayCue(AudioCueRequest {
                cue_id: cue.cue_id.clone(),
                scope,
                bus: cue.bus,
                volume: cue.volume,
                pitch: cue.pitch,
                looped: cue.looped,
                fade_in_seconds: cue.fade_in_seconds,
                start_seconds: None,
            }),
            Self::Music(music) => AudioCommand::PlayMusic(AudioMusicRequest {
                clip_id: music.clip_id.clone(),
                scope,
                volume: music.volume,
                looped: music.looped,
                fade_in_seconds: music.fade_in_seconds,
                start_seconds: None,
            }),
        }
    }
}

fn stop_scene_scope_command(
    session_id: &SceneSessionId,
    fade_out_seconds: Option<f32>,
) -> Option<AudioCommand> {
    Some(AudioCommand::StopByScope(AudioScopeFadeCommand {
        scope: scene_scope(session_id)?,
        fade_out_seconds,
    }))
}

fn scene_scope(session_id: &SceneSessionId) -> Option<AudioScope> {
    AudioScope::scene(session_id.as_str()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::message::MessageCursor;

    fn cue_id(value: &str) -> AudioCueId {
        AudioCueId::try_from(value).unwrap()
    }

    fn clip_id(value: &str) -> AudioClipId {
        AudioClipId::try_from(value).unwrap()
    }

    fn adapter_app() -> App {
        let mut app = App::new();
        app.add_message::<SceneEvent>()
            .add_message::<AudioCommand>()
            .init_resource::<SceneAudioAdapterConfig>()
            .add_systems(Update, play_scene_audio_on_lifecycle);
        app
    }

    fn read_commands(app: &App) -> Vec<AudioCommand> {
        let messages = app.world().resource::<Messages<AudioCommand>>();
        let mut cursor = MessageCursor::default();
        cursor.read(messages).cloned().collect()
    }

    #[test]
    fn entered_scene_writes_configured_scene_scope_playback_commands() {
        let mut app = adapter_app();
        app.world_mut()
            .resource_mut::<SceneAudioAdapterConfig>()
            .register(
                "sample.scene",
                SceneAudioEntry {
                    on_enter: vec![
                        SceneAudioPlayback::Cue(
                            SceneAudioCue::ambience(cue_id("ambience.room"))
                                .with_volume(0.35)
                                .with_fade_in_seconds(0.2),
                        ),
                        SceneAudioPlayback::Music(
                            SceneAudioMusic::new(clip_id("music.room")).with_volume(0.5),
                        ),
                    ],
                    exit_fade_out_seconds: Some(0.15),
                },
            );

        app.world_mut().write_message(SceneEvent::Entered(
            crate::framework::scene::prelude::SceneEntered {
                scene_id: SceneId::from("sample.scene"),
                session_id: SceneSessionId::from("sample.scene-1"),
                content_version: None,
            },
        ));
        app.update();

        assert_eq!(
            read_commands(&app),
            vec![
                AudioCommand::PlayCue(AudioCueRequest {
                    cue_id: cue_id("ambience.room"),
                    scope: AudioScope::scene("sample.scene-1").unwrap(),
                    bus: Some(AudioBus::Sfx),
                    volume: 0.35,
                    pitch: 1.0,
                    looped: true,
                    fade_in_seconds: Some(0.2),
                    start_seconds: None,
                }),
                AudioCommand::PlayMusic(AudioMusicRequest {
                    clip_id: clip_id("music.room"),
                    scope: AudioScope::scene("sample.scene-1").unwrap(),
                    volume: 0.5,
                    looped: true,
                    fade_in_seconds: None,
                    start_seconds: None,
                }),
            ]
        );
    }

    #[test]
    fn exit_started_and_exited_write_scene_scope_stop_commands() {
        let mut app = adapter_app();
        app.world_mut()
            .resource_mut::<SceneAudioAdapterConfig>()
            .register(
                "sample.scene",
                SceneAudioEntry::from_playback(SceneAudioPlayback::Cue(SceneAudioCue::ambience(
                    cue_id("ambience.room"),
                )))
                .with_exit_fade_out_seconds(0.25),
            );

        app.world_mut().write_message(SceneEvent::ExitStarted(
            crate::framework::scene::prelude::SceneExitStarted {
                scene_id: SceneId::from("sample.scene"),
                session_id: SceneSessionId::from("sample.scene-1"),
            },
        ));
        app.world_mut().write_message(SceneEvent::Exited(
            crate::framework::scene::prelude::SceneExited {
                scene_id: SceneId::from("sample.scene"),
                session_id: SceneSessionId::from("sample.scene-1"),
            },
        ));
        app.update();

        assert_eq!(
            read_commands(&app),
            vec![
                AudioCommand::StopByScope(AudioScopeFadeCommand {
                    scope: AudioScope::scene("sample.scene-1").unwrap(),
                    fade_out_seconds: Some(0.25),
                }),
                AudioCommand::StopByScope(AudioScopeFadeCommand {
                    scope: AudioScope::scene("sample.scene-1").unwrap(),
                    fade_out_seconds: None,
                }),
            ]
        );
    }

    #[test]
    fn unconfigured_scene_does_not_play_but_still_stops_session_scope_on_exit() {
        let mut app = adapter_app();

        app.world_mut().write_message(SceneEvent::Entered(
            crate::framework::scene::prelude::SceneEntered {
                scene_id: SceneId::from("unconfigured.scene"),
                session_id: SceneSessionId::from("unconfigured.scene-1"),
                content_version: None,
            },
        ));
        app.world_mut().write_message(SceneEvent::Exited(
            crate::framework::scene::prelude::SceneExited {
                scene_id: SceneId::from("unconfigured.scene"),
                session_id: SceneSessionId::from("unconfigured.scene-1"),
            },
        ));
        app.update();

        assert_eq!(
            read_commands(&app),
            vec![AudioCommand::StopByScope(AudioScopeFadeCommand {
                scope: AudioScope::scene("unconfigured.scene-1").unwrap(),
                fade_out_seconds: None,
            })]
        );
    }
}
