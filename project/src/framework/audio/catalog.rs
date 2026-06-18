use std::{collections::HashMap, error::Error, fmt};

use bevy::prelude::*;

use super::{
    id::{AudioClipId, AudioCueId, AudioGroupId},
    scope::{AudioBus, AudioScope},
};

#[derive(Debug, Default, Resource)]
pub struct AudioCatalog {
    clips: HashMap<AudioClipId, AudioClipEntry>,
    cues: HashMap<AudioCueId, AudioCueEntry>,
    groups: HashMap<AudioGroupId, AudioGroupEntry>,
}

impl AudioCatalog {
    pub fn register_clip(
        &mut self,
        clip_id: AudioClipId,
        path: impl Into<String>,
    ) -> Option<AudioClipEntry> {
        self.clips.insert(clip_id, AudioClipEntry::new(path))
    }

    pub fn register_cue(
        &mut self,
        cue_id: AudioCueId,
        cue: AudioCueEntry,
    ) -> Option<AudioCueEntry> {
        self.cues.insert(cue_id, cue)
    }

    pub fn register_group(
        &mut self,
        group_id: AudioGroupId,
        group: AudioGroupEntry,
    ) -> Option<AudioGroupEntry> {
        self.groups.insert(group_id, group)
    }

    pub fn clip(&self, clip_id: &AudioClipId) -> Result<&AudioClipEntry, AudioCatalogError> {
        self.clips
            .get(clip_id)
            .ok_or_else(|| AudioCatalogError::MissingClip(clip_id.clone()))
    }

    pub fn cue(&self, cue_id: &AudioCueId) -> Result<&AudioCueEntry, AudioCatalogError> {
        self.cues
            .get(cue_id)
            .ok_or_else(|| AudioCatalogError::MissingCue(cue_id.clone()))
    }

    pub fn group(&self, group_id: &AudioGroupId) -> Result<&AudioGroupEntry, AudioCatalogError> {
        self.groups
            .get(group_id)
            .ok_or_else(|| AudioCatalogError::MissingGroup(group_id.clone()))
    }

    pub fn resolve_cue(&self, cue_id: &AudioCueId) -> Result<AudioResolvedCue, AudioCatalogError> {
        let cue = self.cue(cue_id)?;
        if cue.clips.is_empty() {
            return Err(AudioCatalogError::EmptyCue(cue_id.clone()));
        }

        let mut clips = Vec::with_capacity(cue.clips.len());

        for cue_clip in &cue.clips {
            let clip = self.clip(&cue_clip.clip_id)?;
            clips.push(AudioResolvedCueClip {
                clip_id: cue_clip.clip_id.clone(),
                path: clip.path.clone(),
                weight: cue_clip.weight,
            });
        }

        Ok(AudioResolvedCue {
            cue_id: cue_id.clone(),
            clips,
            playback: cue.playback.clone(),
            rules: cue.rules,
        })
    }

    pub fn resolve_group(
        &self,
        group_id: &AudioGroupId,
    ) -> Result<AudioResolvedGroup, AudioCatalogError> {
        let group = self.group(group_id)?;
        if group.clips.is_empty() {
            return Err(AudioCatalogError::EmptyGroup(group_id.clone()));
        }

        let mut clips = Vec::with_capacity(group.clips.len());

        for group_clip in &group.clips {
            let clip = self.clip(&group_clip.clip_id)?;
            clips.push(AudioResolvedGroupClip {
                clip_id: group_clip.clip_id.clone(),
                path: clip.path.clone(),
                required: group_clip.required,
            });
        }

        Ok(AudioResolvedGroup {
            group_id: group_id.clone(),
            clips,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioClipEntry {
    pub path: String,
}

impl AudioClipEntry {
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioCueEntry {
    pub clips: Vec<AudioCueClip>,
    pub playback: AudioCuePlayback,
    pub rules: AudioCueRules,
}

impl AudioCueEntry {
    pub fn new(clip_id: AudioClipId) -> Self {
        Self {
            clips: vec![AudioCueClip::new(clip_id)],
            playback: AudioCuePlayback::default(),
            rules: AudioCueRules::default(),
        }
    }

    pub fn from_clips(clips: impl IntoIterator<Item = AudioCueClip>) -> Self {
        Self {
            clips: clips.into_iter().collect(),
            playback: AudioCuePlayback::default(),
            rules: AudioCueRules::default(),
        }
    }

    pub fn with_playback(mut self, playback: AudioCuePlayback) -> Self {
        self.playback = playback;
        self
    }

    pub fn with_rules(mut self, rules: AudioCueRules) -> Self {
        self.rules = rules;
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioCueClip {
    pub clip_id: AudioClipId,
    pub weight: f32,
}

impl AudioCueClip {
    pub fn new(clip_id: AudioClipId) -> Self {
        Self {
            clip_id,
            weight: 1.0,
        }
    }

    pub fn weighted(clip_id: AudioClipId, weight: f32) -> Self {
        Self { clip_id, weight }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioGroupEntry {
    pub clips: Vec<AudioGroupClip>,
}

impl AudioGroupEntry {
    pub fn from_required(clips: impl IntoIterator<Item = AudioClipId>) -> Self {
        Self {
            clips: clips.into_iter().map(AudioGroupClip::required).collect(),
        }
    }

    pub fn from_clips(clips: impl IntoIterator<Item = AudioGroupClip>) -> Self {
        Self {
            clips: clips.into_iter().collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioGroupClip {
    pub clip_id: AudioClipId,
    pub required: bool,
}

impl AudioGroupClip {
    pub fn required(clip_id: AudioClipId) -> Self {
        Self {
            clip_id,
            required: true,
        }
    }

    pub fn optional(clip_id: AudioClipId) -> Self {
        Self {
            clip_id,
            required: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioCuePlayback {
    pub bus: AudioBus,
    pub scope: AudioScope,
    pub looped: bool,
}

impl Default for AudioCuePlayback {
    fn default() -> Self {
        Self {
            bus: AudioBus::Sfx,
            scope: AudioScope::Global,
            looped: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioCueRules {
    pub volume: f32,
    pub pitch: f32,
    pub cooldown_seconds: Option<f32>,
    pub max_concurrent: Option<usize>,
    pub priority: i32,
}

impl Default for AudioCueRules {
    fn default() -> Self {
        Self {
            volume: 1.0,
            pitch: 1.0,
            cooldown_seconds: None,
            max_concurrent: None,
            priority: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioResolvedCue {
    pub cue_id: AudioCueId,
    pub clips: Vec<AudioResolvedCueClip>,
    pub playback: AudioCuePlayback,
    pub rules: AudioCueRules,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioResolvedCueClip {
    pub clip_id: AudioClipId,
    pub path: String,
    pub weight: f32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioResolvedGroup {
    pub group_id: AudioGroupId,
    pub clips: Vec<AudioResolvedGroupClip>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioResolvedGroupClip {
    pub clip_id: AudioClipId,
    pub path: String,
    pub required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AudioCatalogError {
    MissingCue(AudioCueId),
    MissingClip(AudioClipId),
    MissingGroup(AudioGroupId),
    EmptyCue(AudioCueId),
    EmptyGroup(AudioGroupId),
}

impl fmt::Display for AudioCatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCue(cue_id) => write!(formatter, "audio cue not found: {cue_id}"),
            Self::MissingClip(clip_id) => write!(formatter, "audio clip not found: {clip_id}"),
            Self::MissingGroup(group_id) => write!(formatter, "audio group not found: {group_id}"),
            Self::EmptyCue(cue_id) => write!(formatter, "audio cue has no clips: {cue_id}"),
            Self::EmptyGroup(group_id) => {
                write!(formatter, "audio group has no clips: {group_id}")
            }
        }
    }
}

impl Error for AudioCatalogError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip_id(value: &str) -> AudioClipId {
        AudioClipId::try_from(value).unwrap()
    }

    fn cue_id(value: &str) -> AudioCueId {
        AudioCueId::try_from(value).unwrap()
    }

    fn group_id(value: &str) -> AudioGroupId {
        AudioGroupId::try_from(value).unwrap()
    }

    #[test]
    fn registers_clip_id_to_resource_path() {
        let mut catalog = AudioCatalog::default();
        let clip_id = clip_id("ui.click");

        catalog.register_clip(clip_id.clone(), "audio/ui/click.ogg");

        assert_eq!(
            catalog.clip(&clip_id).unwrap(),
            &AudioClipEntry::new("audio/ui/click.ogg")
        );
    }

    #[test]
    fn registers_cue_id_to_single_or_multiple_clips() {
        let mut catalog = AudioCatalog::default();
        let click = clip_id("ui.click");
        let click_alt = clip_id("ui.click_alt");
        let cue_id = cue_id("ui.click");

        catalog.register_cue(
            cue_id.clone(),
            AudioCueEntry::from_clips([
                AudioCueClip::new(click.clone()),
                AudioCueClip::weighted(click_alt.clone(), 2.0),
            ]),
        );

        let cue = catalog.cue(&cue_id).unwrap();
        assert_eq!(
            cue.clips,
            vec![
                AudioCueClip::new(click),
                AudioCueClip::weighted(click_alt, 2.0)
            ]
        );
    }

    #[test]
    fn registers_group_with_required_and_optional_clips() {
        let mut catalog = AudioCatalog::default();
        let required = clip_id("ui.click");
        let optional = clip_id("ui.confirm");
        let group_id = group_id("boot");

        catalog.register_group(
            group_id.clone(),
            AudioGroupEntry::from_clips([
                AudioGroupClip::required(required.clone()),
                AudioGroupClip::optional(optional.clone()),
            ]),
        );

        let group = catalog.group(&group_id).unwrap();
        assert_eq!(
            group.clips,
            vec![
                AudioGroupClip::required(required),
                AudioGroupClip::optional(optional)
            ]
        );
    }

    #[test]
    fn resolves_group_to_clip_paths_and_required_flags() {
        let mut catalog = AudioCatalog::default();
        let required = clip_id("ui.click");
        let optional = clip_id("ui.confirm");
        let group_id = group_id("boot");

        catalog.register_clip(required.clone(), "audio/ui/click_wood_01.wav");
        catalog.register_clip(optional.clone(), "audio/ui/confirm_brick_01.wav");
        catalog.register_group(
            group_id.clone(),
            AudioGroupEntry::from_clips([
                AudioGroupClip::required(required.clone()),
                AudioGroupClip::optional(optional.clone()),
            ]),
        );

        let resolved = catalog.resolve_group(&group_id).unwrap();

        assert_eq!(
            resolved,
            AudioResolvedGroup {
                group_id,
                clips: vec![
                    AudioResolvedGroupClip {
                        clip_id: required,
                        path: "audio/ui/click_wood_01.wav".to_string(),
                        required: true,
                    },
                    AudioResolvedGroupClip {
                        clip_id: optional,
                        path: "audio/ui/confirm_brick_01.wav".to_string(),
                        required: false,
                    },
                ],
            }
        );
    }

    #[test]
    fn reports_group_resolution_errors() {
        let missing_group = group_id("missing.group");
        let mut catalog = AudioCatalog::default();

        assert_eq!(
            catalog.resolve_group(&missing_group),
            Err(AudioCatalogError::MissingGroup(missing_group))
        );

        let empty_group = group_id("empty.group");
        catalog.register_group(empty_group.clone(), AudioGroupEntry::from_clips([]));
        assert_eq!(
            catalog.resolve_group(&empty_group),
            Err(AudioCatalogError::EmptyGroup(empty_group))
        );

        let missing_clip = clip_id("ui.missing");
        let group_id = group_id("boot");
        catalog.register_group(
            group_id.clone(),
            AudioGroupEntry::from_required([missing_clip.clone()]),
        );
        assert_eq!(
            catalog.resolve_group(&group_id),
            Err(AudioCatalogError::MissingClip(missing_clip))
        );
    }

    #[test]
    fn required_group_constructor_marks_all_clips_required() {
        let click = clip_id("ui.click");
        let confirm = clip_id("ui.confirm");

        let group = AudioGroupEntry::from_required([click.clone(), confirm.clone()]);

        assert_eq!(
            group.clips,
            vec![
                AudioGroupClip::required(click),
                AudioGroupClip::required(confirm)
            ]
        );
    }

    #[test]
    fn resolves_cue_to_registered_clip_paths() {
        let mut catalog = AudioCatalog::default();
        let click = clip_id("ui.click");
        let click_alt = clip_id("ui.click_alt");
        let cue_id = cue_id("ui.click");

        catalog.register_clip(click.clone(), "audio/ui/click.ogg");
        catalog.register_clip(click_alt.clone(), "audio/ui/click_alt.ogg");
        catalog.register_cue(
            cue_id.clone(),
            AudioCueEntry::from_clips([
                AudioCueClip::new(click.clone()),
                AudioCueClip::weighted(click_alt.clone(), 3.0),
            ]),
        );

        let resolved = catalog.resolve_cue(&cue_id).unwrap();

        assert_eq!(resolved.cue_id, cue_id);
        assert_eq!(
            resolved.clips,
            vec![
                AudioResolvedCueClip {
                    clip_id: click,
                    path: "audio/ui/click.ogg".to_string(),
                    weight: 1.0,
                },
                AudioResolvedCueClip {
                    clip_id: click_alt,
                    path: "audio/ui/click_alt.ogg".to_string(),
                    weight: 3.0,
                },
            ]
        );
    }

    #[test]
    fn reports_missing_cue() {
        let catalog = AudioCatalog::default();
        let cue_id = cue_id("ui.missing");

        assert_eq!(
            catalog.resolve_cue(&cue_id),
            Err(AudioCatalogError::MissingCue(cue_id))
        );
    }

    #[test]
    fn reports_missing_clip_when_resolving_cue() {
        let mut catalog = AudioCatalog::default();
        let clip_id = clip_id("ui.missing");
        let cue_id = cue_id("ui.click");

        catalog.register_cue(cue_id.clone(), AudioCueEntry::new(clip_id.clone()));

        assert_eq!(
            catalog.resolve_cue(&cue_id),
            Err(AudioCatalogError::MissingClip(clip_id))
        );
    }

    #[test]
    fn reports_empty_cue_when_resolving_cue() {
        let mut catalog = AudioCatalog::default();
        let cue_id = cue_id("ui.empty");

        catalog.register_cue(cue_id.clone(), AudioCueEntry::from_clips([]));

        assert_eq!(
            catalog.resolve_cue(&cue_id),
            Err(AudioCatalogError::EmptyCue(cue_id))
        );
    }

    #[test]
    fn cue_defaults_are_stable_for_playback() {
        let cue = AudioCueEntry::new(clip_id("ui.click"));

        assert_eq!(cue.playback.bus, AudioBus::Sfx);
        assert_eq!(cue.playback.scope, AudioScope::Global);
        assert_eq!(cue.rules.volume, 1.0);
        assert_eq!(cue.rules.pitch, 1.0);
        assert!(!cue.playback.looped);
    }

    #[test]
    fn stores_playback_defaults_and_rules_fields() {
        let cue = AudioCueEntry::new(clip_id("music.title"))
            .with_playback(AudioCuePlayback {
                bus: AudioBus::Music,
                scope: AudioScope::Ui,
                looped: true,
            })
            .with_rules(AudioCueRules {
                volume: 0.75,
                pitch: 0.95,
                cooldown_seconds: Some(0.2),
                max_concurrent: Some(3),
                priority: 10,
            });

        assert_eq!(cue.playback.bus, AudioBus::Music);
        assert_eq!(cue.playback.scope, AudioScope::Ui);
        assert!(cue.playback.looped);
        assert_eq!(cue.rules.volume, 0.75);
        assert_eq!(cue.rules.pitch, 0.95);
        assert_eq!(cue.rules.cooldown_seconds, Some(0.2));
        assert_eq!(cue.rules.max_concurrent, Some(3));
        assert_eq!(cue.rules.priority, 10);
    }
}
