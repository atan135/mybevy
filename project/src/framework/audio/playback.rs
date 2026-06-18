use std::collections::HashMap;

use bevy::prelude::*;

use super::{
    id::{AudioClipId, AudioCueId, AudioInstanceId},
    scope::{AudioBus, AudioScope},
};

#[derive(Debug, Default, Resource)]
pub struct AudioPlaybackState {
    pub instances: HashMap<AudioInstanceId, AudioInstanceState>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioInstanceState {
    pub entity: Entity,
    pub clip_id: AudioClipId,
    pub cue_id: Option<AudioCueId>,
    pub scope: AudioScope,
    pub bus: AudioBus,
}
