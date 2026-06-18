use bevy::prelude::*;

use super::{
    id::{AudioClipId, AudioCueId, AudioGroupId, AudioInstanceId},
    scope::{AudioBus, AudioScope},
};

#[derive(Clone, Debug, Message, PartialEq)]
pub enum AudioCommand {
    PlayCue(AudioCueRequest),
    PlayClip(AudioClipRequest),
    PlayMusic(AudioMusicRequest),
    CrossfadeMusic(AudioCrossfadeMusicRequest),
    StopInstance(AudioStopInstanceCommand),
    StopByScope(AudioScopeFadeCommand),
    PauseByScope(AudioScopeCommand),
    ResumeByScope(AudioScopeCommand),
    SetBusVolume(AudioBusVolumeCommand),
    SetBusMuted(AudioBusMutedCommand),
    SetBusPaused(AudioBusPausedCommand),
    PreloadGroup(AudioGroupCommand),
    UnloadGroup(AudioGroupCommand),
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioCueRequest {
    pub cue_id: AudioCueId,
    pub scope: AudioScope,
    pub bus: Option<AudioBus>,
    pub volume: f32,
    pub pitch: f32,
    pub looped: bool,
    pub fade_in_seconds: Option<f32>,
}

impl AudioCueRequest {
    pub fn new(cue_id: AudioCueId) -> Self {
        Self {
            cue_id,
            scope: AudioScope::Global,
            bus: None,
            volume: 1.0,
            pitch: 1.0,
            looped: false,
            fade_in_seconds: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioClipRequest {
    pub clip_id: AudioClipId,
    pub scope: AudioScope,
    pub bus: AudioBus,
    pub volume: f32,
    pub pitch: f32,
    pub looped: bool,
    pub fade_in_seconds: Option<f32>,
}

impl AudioClipRequest {
    pub fn new(clip_id: AudioClipId) -> Self {
        Self {
            clip_id,
            scope: AudioScope::Global,
            bus: AudioBus::Sfx,
            volume: 1.0,
            pitch: 1.0,
            looped: false,
            fade_in_seconds: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioMusicRequest {
    pub clip_id: AudioClipId,
    pub scope: AudioScope,
    pub volume: f32,
    pub looped: bool,
    pub fade_in_seconds: Option<f32>,
}

impl AudioMusicRequest {
    pub fn new(clip_id: AudioClipId) -> Self {
        Self {
            clip_id,
            scope: AudioScope::Global,
            volume: 1.0,
            looped: true,
            fade_in_seconds: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioCrossfadeMusicRequest {
    pub clip_id: AudioClipId,
    pub scope: AudioScope,
    pub volume: f32,
    pub looped: bool,
    pub fade_seconds: f32,
}

impl AudioCrossfadeMusicRequest {
    pub fn new(clip_id: AudioClipId, fade_seconds: f32) -> Self {
        Self {
            clip_id,
            scope: AudioScope::Global,
            volume: 1.0,
            looped: true,
            fade_seconds,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioStopInstanceCommand {
    pub instance_id: AudioInstanceId,
    pub fade_out_seconds: Option<f32>,
}

impl AudioStopInstanceCommand {
    pub const fn new(instance_id: AudioInstanceId) -> Self {
        Self {
            instance_id,
            fade_out_seconds: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioScopeCommand {
    pub scope: AudioScope,
}

impl AudioScopeCommand {
    pub fn new(scope: AudioScope) -> Self {
        Self { scope }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioScopeFadeCommand {
    pub scope: AudioScope,
    pub fade_out_seconds: Option<f32>,
}

impl AudioScopeFadeCommand {
    pub fn new(scope: AudioScope) -> Self {
        Self {
            scope,
            fade_out_seconds: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioBusVolumeCommand {
    pub bus: AudioBus,
    pub volume: f32,
}

impl AudioBusVolumeCommand {
    pub const fn new(bus: AudioBus, volume: f32) -> Self {
        Self { bus, volume }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioBusMutedCommand {
    pub bus: AudioBus,
    pub muted: bool,
}

impl AudioBusMutedCommand {
    pub const fn new(bus: AudioBus, muted: bool) -> Self {
        Self { bus, muted }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioBusPausedCommand {
    pub bus: AudioBus,
    pub paused: bool,
}

impl AudioBusPausedCommand {
    pub const fn new(bus: AudioBus, paused: bool) -> Self {
        Self { bus, paused }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioGroupCommand {
    pub group_id: AudioGroupId,
}

impl AudioGroupCommand {
    pub fn new(group_id: AudioGroupId) -> Self {
        Self { group_id }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cue_request_defaults_keep_catalog_overrides_open() {
        let cue_id = AudioCueId::try_from("ui.click").unwrap();
        let request = AudioCueRequest::new(cue_id.clone());

        assert_eq!(request.cue_id, cue_id);
        assert_eq!(request.scope, AudioScope::Global);
        assert_eq!(request.bus, None);
        assert_eq!(request.volume, 1.0);
        assert_eq!(request.pitch, 1.0);
        assert!(!request.looped);
        assert_eq!(request.fade_in_seconds, None);
    }

    #[test]
    fn clip_and_music_requests_have_distinct_defaults() {
        let clip_id = AudioClipId::try_from("music.title").unwrap();
        let clip_request = AudioClipRequest::new(clip_id.clone());
        let music_request = AudioMusicRequest::new(clip_id);

        assert_eq!(clip_request.bus, AudioBus::Sfx);
        assert!(!clip_request.looped);
        assert!(music_request.looped);
        assert_eq!(music_request.scope, AudioScope::Global);
    }

    #[test]
    fn command_variants_carry_control_payloads() {
        let instance_id = AudioInstanceId::from_raw(7);
        let command = AudioCommand::StopInstance(AudioStopInstanceCommand {
            instance_id,
            fade_out_seconds: Some(0.25),
        });

        assert_eq!(
            command,
            AudioCommand::StopInstance(AudioStopInstanceCommand {
                instance_id,
                fade_out_seconds: Some(0.25),
            })
        );
    }
}
