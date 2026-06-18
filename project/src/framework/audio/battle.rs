use super::{
    command::{AudioBattleCueRequest, AudioCommand},
    id::{AudioCueId, AudioScopeId},
    scope::AudioBus,
};

pub const DEFAULT_BATTLE_AUDIO_BUS: AudioBus = AudioBus::Battle;

#[derive(Clone, Debug, PartialEq)]
pub struct BattleAudioCue {
    pub cue_id: AudioCueId,
    pub bus: Option<AudioBus>,
    pub volume: f32,
    pub pitch: f32,
    pub looped: bool,
    pub fade_in_seconds: Option<f32>,
}

impl BattleAudioCue {
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

    pub fn command(&self, battle_id: AudioScopeId) -> AudioCommand {
        AudioCommand::PlayBattleCue(AudioBattleCueRequest {
            battle_id,
            cue_id: self.cue_id.clone(),
            bus: self.bus,
            volume: self.volume,
            pitch: self.pitch,
            looped: self.looped,
            fade_in_seconds: self.fade_in_seconds,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::audio::AudioScope;

    #[test]
    fn battle_audio_cue_maps_to_battle_cue_command() {
        let battle_id = AudioScopeId::try_from("battle_01").unwrap();
        let cue_id = AudioCueId::try_from("battle.skill.cast").unwrap();
        let cue = BattleAudioCue::new(cue_id.clone())
            .with_volume(0.75)
            .with_pitch(1.1)
            .with_fade_in_seconds(0.2);

        let AudioCommand::PlayBattleCue(request) = cue.command(battle_id.clone()) else {
            panic!("battle cue should write PlayBattleCue");
        };

        assert_eq!(request.battle_id, battle_id.clone());
        assert_eq!(request.scope(), AudioScope::Battle(battle_id));
        assert_eq!(request.cue_id, cue_id);
        assert_eq!(request.bus, None);
        assert_eq!(request.volume, 0.75);
        assert_eq!(request.pitch, 1.1);
        assert_eq!(request.fade_in_seconds, Some(0.2));
        assert_eq!(DEFAULT_BATTLE_AUDIO_BUS, AudioBus::Battle);
    }
}
