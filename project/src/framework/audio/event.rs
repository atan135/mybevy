use bevy::prelude::*;

use super::{
    id::{AudioClipId, AudioCueId, AudioGroupId, AudioInstanceId},
    scope::{AudioBus, AudioScope},
};

#[derive(Clone, Debug, Message, PartialEq)]
pub enum AudioEvent {
    CueStarted(AudioCueStarted),
    ClipStarted(AudioClipStarted),
    CueSkipped(AudioCueSkipped),
    InstanceStopped(AudioInstanceStopped),
    InstanceProgress(AudioInstanceProgress),
    InstanceControlFailed(AudioInstanceControlFailed),
    LoadProgress(AudioLoadProgress),
    LoadFailed(AudioLoadFailed),
    MusicChanged(AudioMusicChanged),
    BusChanged(AudioBusChanged),
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioCueStarted {
    pub cue_id: AudioCueId,
    pub clip_id: AudioClipId,
    pub instance_id: AudioInstanceId,
    pub scope: AudioScope,
    pub bus: AudioBus,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioClipStarted {
    pub clip_id: AudioClipId,
    pub instance_id: AudioInstanceId,
    pub scope: AudioScope,
    pub bus: AudioBus,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioCueSkipped {
    pub cue_id: AudioCueId,
    pub reason: AudioCueSkipReason,
    pub scope: AudioScope,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AudioCueSkipReason {
    Cooldown,
    MaxConcurrency,
    LowerPriority,
    MissingCue,
    MissingClip,
    BusPaused,
    ScopePaused,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioInstanceStopped {
    pub instance_id: AudioInstanceId,
    pub clip_id: Option<AudioClipId>,
    pub cue_id: Option<AudioCueId>,
    pub scope: AudioScope,
    pub bus: AudioBus,
    pub reason: AudioStopReason,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioInstanceProgress {
    pub instance_id: AudioInstanceId,
    pub clip_id: AudioClipId,
    pub cue_id: Option<AudioCueId>,
    pub scope: AudioScope,
    pub bus: AudioBus,
    pub position_seconds: f32,
    pub paused: bool,
    pub spatial: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioInstanceControlFailed {
    pub instance_id: AudioInstanceId,
    pub action: AudioInstanceControlAction,
    pub reason: AudioInstanceControlFailureReason,
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AudioInstanceControlAction {
    Pause,
    Resume,
    Seek,
    QueryProgress,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AudioInstanceControlFailureReason {
    MissingInstance,
    StoppedInstance,
    SinkNotReady,
    SeekUnsupported,
    InvalidPosition,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioLoadProgress {
    pub group_id: AudioGroupId,
    pub loaded: usize,
    pub total: usize,
    pub failed: usize,
    pub required_loaded: usize,
    pub required_total: usize,
    pub required_failed: usize,
    pub clip_id: Option<AudioClipId>,
    pub asset_path: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AudioStopReason {
    Completed,
    Stopped,
    StoppedByScope,
    ReplacedByMusic,
    LoadFailed,
    SourceEntityDespawned,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioLoadFailed {
    pub clip_id: Option<AudioClipId>,
    pub cue_id: Option<AudioCueId>,
    pub group_id: Option<AudioGroupId>,
    pub asset_path: Option<String>,
    pub message: String,
}

impl AudioLoadFailed {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            clip_id: None,
            cue_id: None,
            group_id: None,
            asset_path: None,
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioMusicChanged {
    pub previous_instance_id: Option<AudioInstanceId>,
    pub previous_clip_id: Option<AudioClipId>,
    pub new_instance_id: Option<AudioInstanceId>,
    pub new_clip_id: AudioClipId,
    pub scope: AudioScope,
    pub crossfade_seconds: Option<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioBusChanged {
    pub bus: AudioBus,
    pub change: AudioBusChange,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AudioBusChange {
    Volume { previous: f32, current: f32 },
    Muted { previous: bool, current: bool },
    Paused { previous: bool, current: bool },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn started_events_keep_instance_scope_and_bus() {
        let event = AudioEvent::CueStarted(AudioCueStarted {
            cue_id: AudioCueId::try_from("ui.click").unwrap(),
            clip_id: AudioClipId::try_from("ui.click_01").unwrap(),
            instance_id: AudioInstanceId::from_raw(12),
            scope: AudioScope::Ui,
            bus: AudioBus::Ui,
        });

        assert_eq!(
            event,
            AudioEvent::CueStarted(AudioCueStarted {
                cue_id: AudioCueId::try_from("ui.click").unwrap(),
                clip_id: AudioClipId::try_from("ui.click_01").unwrap(),
                instance_id: AudioInstanceId::from_raw(12),
                scope: AudioScope::Ui,
                bus: AudioBus::Ui,
            })
        );
    }

    #[test]
    fn load_failed_builder_preserves_message_and_optional_fields() {
        let failure = AudioLoadFailed::new("missing asset");

        assert_eq!(failure.message, "missing asset");
        assert_eq!(failure.clip_id, None);
        assert_eq!(failure.cue_id, None);
        assert_eq!(failure.group_id, None);
        assert_eq!(failure.asset_path, None);
    }

    #[test]
    fn bus_change_event_carries_previous_and_current_values() {
        let event = AudioEvent::BusChanged(AudioBusChanged {
            bus: AudioBus::Music,
            change: AudioBusChange::Volume {
                previous: 1.0,
                current: 0.5,
            },
        });

        assert_eq!(
            event,
            AudioEvent::BusChanged(AudioBusChanged {
                bus: AudioBus::Music,
                change: AudioBusChange::Volume {
                    previous: 1.0,
                    current: 0.5,
                },
            })
        );
    }

    #[test]
    fn instance_control_events_are_explicit_about_progress_and_failures() {
        let instance_id = AudioInstanceId::from_raw(42);
        let progress = AudioEvent::InstanceProgress(AudioInstanceProgress {
            instance_id,
            clip_id: AudioClipId::try_from("voice.line").unwrap(),
            cue_id: Some(AudioCueId::try_from("voice.line").unwrap()),
            scope: AudioScope::Global,
            bus: AudioBus::Sfx,
            position_seconds: 12.25,
            paused: true,
            spatial: false,
        });
        let failure = AudioEvent::InstanceControlFailed(AudioInstanceControlFailed {
            instance_id,
            action: AudioInstanceControlAction::Seek,
            reason: AudioInstanceControlFailureReason::SinkNotReady,
            message: "audio sink is not ready yet".to_string(),
        });

        assert!(matches!(
            progress,
            AudioEvent::InstanceProgress(AudioInstanceProgress {
                position_seconds: 12.25,
                paused: true,
                ..
            })
        ));
        assert!(matches!(
            failure,
            AudioEvent::InstanceControlFailed(AudioInstanceControlFailed {
                action: AudioInstanceControlAction::Seek,
                reason: AudioInstanceControlFailureReason::SinkNotReady,
                ..
            })
        ));
    }
}
