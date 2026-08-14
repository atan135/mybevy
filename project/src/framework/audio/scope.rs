use bevy::prelude::Entity;
use std::fmt;

use super::id::{AudioIdError, AudioScopeId};

pub use audio_core::AudioBus;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum AudioScope {
    Global,
    Ui,
    Scene(AudioScopeId),
    Entity(Entity),
    Battle(AudioScopeId),
}

impl AudioScope {
    pub fn scene(value: impl Into<String>) -> Result<Self, AudioIdError> {
        Ok(Self::Scene(AudioScopeId::new(value)?))
    }

    pub fn battle(value: impl Into<String>) -> Result<Self, AudioIdError> {
        Ok(Self::Battle(AudioScopeId::new(value)?))
    }
}

impl fmt::Display for AudioScope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Global => formatter.write_str("global"),
            Self::Ui => formatter.write_str("ui"),
            Self::Scene(id) => write!(formatter, "scene:{id}"),
            Self::Entity(entity) => write!(formatter, "entity:{entity}"),
            Self::Battle(id) => write!(formatter, "battle:{id}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_displays_basic_variants() {
        let scene_scope = AudioScope::scene("scene.demo").unwrap();
        let battle_scope = AudioScope::battle("battle_01").unwrap();
        let entity = Entity::from_raw_u32(7).unwrap();

        assert_eq!(AudioScope::Global.to_string(), "global");
        assert_eq!(AudioScope::Ui.to_string(), "ui");
        assert_eq!(scene_scope.to_string(), "scene:scene.demo");
        assert_eq!(
            AudioScope::Entity(entity).to_string(),
            format!("entity:{entity}")
        );
        assert_eq!(battle_scope.to_string(), "battle:battle_01");
    }

    #[test]
    fn scope_reuses_audio_id_validation() {
        assert!(AudioScope::scene("scene..demo").is_err());
        assert!(AudioScope::battle("Battle01").is_err());
    }
}
