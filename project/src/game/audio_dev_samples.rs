use std::time::Duration;

use bevy::prelude::*;

use crate::framework::audio::prelude::{
    AudioBus, AudioCatalog, AudioClipId, AudioCueClip, AudioCueEntry, AudioCueId, AudioCuePlayback,
    AudioCueRules, AudioGroupClip, AudioGroupEntry, AudioGroupId, AudioScope,
};

pub(in crate::game) const AUDIO_GALLERY_BANK_GROUP_ID: &str = "bank.audio_gallery";
pub(in crate::game) const AUDIO_GALLERY_RESIDENT_BANK_GROUP_ID: &str =
    "bank.audio_gallery.resident";

pub(in crate::game) const AUDIO_GALLERY_UI_NOTIFY_CLIP_ID: &str = "dev.audio.ui.notify_horn_01";
const AUDIO_GALLERY_UI_NOTIFY_CLIP_PATH: &str = "audio/ui/notify_horn_01.wav";
pub(in crate::game) const AUDIO_GALLERY_FOOTSTEP_CLIP_ID: &str =
    "dev.audio.common.footstep_concrete_01";
const AUDIO_GALLERY_FOOTSTEP_CLIP_PATH: &str = "audio/common/footstep_concrete_01.wav";
pub(in crate::game) const AUDIO_GALLERY_SWORD_HIT_CLIP_ID: &str = "dev.audio.battle.sword_hit_01";
const AUDIO_GALLERY_SWORD_HIT_CLIP_PATH: &str = "audio/battle/sword_hit_01.wav";
pub(in crate::game) const AUDIO_GALLERY_RAIN_LOOP_CLIP_ID: &str =
    "dev.audio.ambience.light_rain_loop";
const AUDIO_GALLERY_RAIN_LOOP_CLIP_PATH: &str = "audio/ambience/light_rain_loop.wav";
pub(in crate::game) const AUDIO_GALLERY_MENU_MUSIC_CLIP_ID: &str = "dev.audio.music.menu_loop";
const AUDIO_GALLERY_MENU_MUSIC_CLIP_PATH: &str = "audio/music/menu_loop.wav";
pub(in crate::game) const AUDIO_GALLERY_STEALTH_MUSIC_CLIP_ID: &str =
    "dev.audio.music.stealth_bass_loop";
const AUDIO_GALLERY_STEALTH_MUSIC_CLIP_PATH: &str = "audio/music/stealth_bass_loop.wav";
pub(in crate::game) const AUDIO_GALLERY_CAR_HORN_CLIP_ID: &str = "dev.audio.spatial.car_horn_taps";
const AUDIO_GALLERY_CAR_HORN_CLIP_PATH: &str = "audio/spatial/car_horn_taps.wav";
pub(in crate::game) const AUDIO_GALLERY_DOG_BARK_CLIP_ID: &str =
    "dev.audio.spatial.dog_bark_city_03";
const AUDIO_GALLERY_DOG_BARK_CLIP_PATH: &str = "audio/spatial/dog_bark_city_03.wav";
pub(in crate::game) const AUDIO_GALLERY_VOICE_CLIP_ID: &str = "dev.audio.voice.en_us_una_hs_lo_01";
const AUDIO_GALLERY_VOICE_CLIP_PATH: &str = "audio/voice/en_us_una_hs_lo_01.wav";
pub(in crate::game) const AUDIO_GALLERY_MISSING_CLIP_ID: &str = "dev.audio.failure.missing_asset";
const AUDIO_GALLERY_MISSING_CLIP_PATH: &str = "audio/dev_gallery/missing_asset.wav";
pub(in crate::game) const AUDIO_GALLERY_RESIDENT_CLICK_CLIP_ID: &str =
    "dev.audio.resident.ui.click_wood_01";
const AUDIO_GALLERY_RESIDENT_CLICK_CLIP_PATH: &str = "audio/ui/click_wood_01.wav";

pub(in crate::game) const AUDIO_GALLERY_UI_NOTIFY_CUE_ID: &str = "dev.audio.ui.notify";
pub(in crate::game) const AUDIO_GALLERY_FOOTSTEP_CUE_ID: &str = "dev.audio.sfx.footstep";
pub(in crate::game) const AUDIO_GALLERY_SWORD_HIT_CUE_ID: &str = "dev.audio.sfx.sword_hit";
pub(in crate::game) const AUDIO_GALLERY_RAIN_LOOP_CUE_ID: &str = "dev.audio.loop.light_rain";
pub(in crate::game) const AUDIO_GALLERY_MENU_MUSIC_CUE_ID: &str = "dev.audio.music.menu_loop";
pub(in crate::game) const AUDIO_GALLERY_STEALTH_MUSIC_CUE_ID: &str =
    "dev.audio.music.stealth_bass_loop";
pub(in crate::game) const AUDIO_GALLERY_CAR_HORN_CUE_ID: &str = "dev.audio.spatial.car_horn";
pub(in crate::game) const AUDIO_GALLERY_DOG_BARK_CUE_ID: &str = "dev.audio.spatial.dog_bark";
pub(in crate::game) const AUDIO_GALLERY_VOICE_CUE_ID: &str = "dev.audio.voice.line";
pub(in crate::game) const AUDIO_GALLERY_COOLDOWN_CUE_ID: &str = "dev.audio.rules.cooldown";
pub(in crate::game) const AUDIO_GALLERY_MAX_CONCURRENT_CUE_ID: &str =
    "dev.audio.rules.max_concurrent";
pub(in crate::game) const AUDIO_GALLERY_MISSING_CUE_ID: &str = "dev.audio.failure.missing_asset";
pub(in crate::game) const AUDIO_GALLERY_RESIDENT_CLICK_CUE_ID: &str = "dev.audio.resident.ui.click";

const AUDIO_GALLERY_LAZY_UNLOAD_SECONDS: f32 = 12.0;

#[derive(Clone, Debug, PartialEq)]
pub(in crate::game) struct AudioGalleryDevBankSetting {
    pub(in crate::game) group_id: AudioGroupId,
    pub(in crate::game) lazy_unload: Duration,
}

#[derive(Clone, Debug, PartialEq, Resource)]
pub(in crate::game) struct AudioGalleryDevBankConfig {
    pub(in crate::game) banks: Vec<AudioGalleryDevBankSetting>,
}

impl Default for AudioGalleryDevBankConfig {
    fn default() -> Self {
        Self {
            banks: vec![
                AudioGalleryDevBankSetting {
                    group_id: group_id(AUDIO_GALLERY_BANK_GROUP_ID),
                    lazy_unload: Duration::from_secs_f32(AUDIO_GALLERY_LAZY_UNLOAD_SECONDS),
                },
                AudioGalleryDevBankSetting {
                    group_id: group_id(AUDIO_GALLERY_RESIDENT_BANK_GROUP_ID),
                    lazy_unload: Duration::ZERO,
                },
            ],
        }
    }
}

pub(in crate::game) fn register_audio_gallery_dev_samples(catalog: Option<ResMut<AudioCatalog>>) {
    let Some(mut catalog) = catalog else {
        return;
    };

    register_audio_gallery_dev_samples_in_catalog(&mut catalog);
}

fn register_audio_gallery_dev_samples_in_catalog(catalog: &mut AudioCatalog) {
    for clip in audio_gallery_clip_entries() {
        catalog.register_clip(clip.id(), clip.path);
    }

    register_gallery_cue(
        catalog,
        AUDIO_GALLERY_UI_NOTIFY_CUE_ID,
        AUDIO_GALLERY_UI_NOTIFY_CLIP_ID,
        AudioBus::Ui,
        AudioScope::Ui,
        false,
        AudioCueRules {
            max_concurrent: Some(4),
            ..AudioCueRules::default()
        },
    );
    register_gallery_cue(
        catalog,
        AUDIO_GALLERY_FOOTSTEP_CUE_ID,
        AUDIO_GALLERY_FOOTSTEP_CLIP_ID,
        AudioBus::Sfx,
        AudioScope::Global,
        false,
        AudioCueRules {
            max_concurrent: Some(6),
            ..AudioCueRules::default()
        },
    );
    register_gallery_cue(
        catalog,
        AUDIO_GALLERY_SWORD_HIT_CUE_ID,
        AUDIO_GALLERY_SWORD_HIT_CLIP_ID,
        AudioBus::Battle,
        AudioScope::Global,
        false,
        AudioCueRules {
            max_concurrent: Some(4),
            priority: 5,
            ..AudioCueRules::default()
        },
    );
    register_gallery_cue(
        catalog,
        AUDIO_GALLERY_RAIN_LOOP_CUE_ID,
        AUDIO_GALLERY_RAIN_LOOP_CLIP_ID,
        AudioBus::Sfx,
        AudioScope::Global,
        true,
        AudioCueRules {
            max_concurrent: Some(1),
            priority: 2,
            ..AudioCueRules::default()
        },
    );
    register_gallery_cue(
        catalog,
        AUDIO_GALLERY_MENU_MUSIC_CUE_ID,
        AUDIO_GALLERY_MENU_MUSIC_CLIP_ID,
        AudioBus::Music,
        AudioScope::Global,
        true,
        AudioCueRules {
            max_concurrent: Some(1),
            priority: 10,
            ..AudioCueRules::default()
        },
    );
    register_gallery_cue(
        catalog,
        AUDIO_GALLERY_STEALTH_MUSIC_CUE_ID,
        AUDIO_GALLERY_STEALTH_MUSIC_CLIP_ID,
        AudioBus::Music,
        AudioScope::Global,
        true,
        AudioCueRules {
            max_concurrent: Some(1),
            priority: 10,
            ..AudioCueRules::default()
        },
    );
    register_gallery_cue(
        catalog,
        AUDIO_GALLERY_CAR_HORN_CUE_ID,
        AUDIO_GALLERY_CAR_HORN_CLIP_ID,
        AudioBus::Sfx,
        AudioScope::Global,
        false,
        AudioCueRules {
            max_concurrent: Some(4),
            priority: 3,
            ..AudioCueRules::default()
        },
    );
    register_gallery_cue(
        catalog,
        AUDIO_GALLERY_DOG_BARK_CUE_ID,
        AUDIO_GALLERY_DOG_BARK_CLIP_ID,
        AudioBus::Sfx,
        AudioScope::Global,
        false,
        AudioCueRules {
            max_concurrent: Some(4),
            priority: 3,
            ..AudioCueRules::default()
        },
    );
    register_gallery_cue(
        catalog,
        AUDIO_GALLERY_VOICE_CUE_ID,
        AUDIO_GALLERY_VOICE_CLIP_ID,
        AudioBus::Sfx,
        AudioScope::Global,
        false,
        AudioCueRules {
            max_concurrent: Some(2),
            priority: 4,
            ..AudioCueRules::default()
        },
    );
    register_gallery_cue(
        catalog,
        AUDIO_GALLERY_COOLDOWN_CUE_ID,
        AUDIO_GALLERY_UI_NOTIFY_CLIP_ID,
        AudioBus::Ui,
        AudioScope::Ui,
        false,
        AudioCueRules {
            cooldown_seconds: Some(0.75),
            max_concurrent: Some(4),
            priority: 1,
            ..AudioCueRules::default()
        },
    );
    register_gallery_cue(
        catalog,
        AUDIO_GALLERY_MAX_CONCURRENT_CUE_ID,
        AUDIO_GALLERY_FOOTSTEP_CLIP_ID,
        AudioBus::Sfx,
        AudioScope::Global,
        false,
        AudioCueRules {
            max_concurrent: Some(1),
            priority: 1,
            ..AudioCueRules::default()
        },
    );
    register_gallery_cue(
        catalog,
        AUDIO_GALLERY_MISSING_CUE_ID,
        AUDIO_GALLERY_MISSING_CLIP_ID,
        AudioBus::Sfx,
        AudioScope::Global,
        false,
        AudioCueRules {
            max_concurrent: Some(1),
            priority: -10,
            ..AudioCueRules::default()
        },
    );
    register_gallery_cue(
        catalog,
        AUDIO_GALLERY_RESIDENT_CLICK_CUE_ID,
        AUDIO_GALLERY_RESIDENT_CLICK_CLIP_ID,
        AudioBus::Ui,
        AudioScope::Ui,
        false,
        AudioCueRules {
            max_concurrent: Some(4),
            ..AudioCueRules::default()
        },
    );

    catalog.register_group(
        group_id(AUDIO_GALLERY_BANK_GROUP_ID),
        AudioGroupEntry::from_clips([
            AudioGroupClip::required(clip_id(AUDIO_GALLERY_UI_NOTIFY_CLIP_ID)),
            AudioGroupClip::required(clip_id(AUDIO_GALLERY_FOOTSTEP_CLIP_ID)),
            AudioGroupClip::required(clip_id(AUDIO_GALLERY_SWORD_HIT_CLIP_ID)),
            AudioGroupClip::required(clip_id(AUDIO_GALLERY_RAIN_LOOP_CLIP_ID)),
            AudioGroupClip::required(clip_id(AUDIO_GALLERY_MENU_MUSIC_CLIP_ID)),
            AudioGroupClip::required(clip_id(AUDIO_GALLERY_STEALTH_MUSIC_CLIP_ID)),
            AudioGroupClip::required(clip_id(AUDIO_GALLERY_CAR_HORN_CLIP_ID)),
            AudioGroupClip::required(clip_id(AUDIO_GALLERY_DOG_BARK_CLIP_ID)),
            AudioGroupClip::required(clip_id(AUDIO_GALLERY_VOICE_CLIP_ID)),
            AudioGroupClip::optional(clip_id(AUDIO_GALLERY_MISSING_CLIP_ID)),
        ]),
    );
    catalog.register_group(
        group_id(AUDIO_GALLERY_RESIDENT_BANK_GROUP_ID),
        AudioGroupEntry::from_required([clip_id(AUDIO_GALLERY_RESIDENT_CLICK_CLIP_ID)]),
    );
}

#[derive(Clone, Copy)]
struct AudioGalleryClipEntry {
    id: &'static str,
    path: &'static str,
}

impl AudioGalleryClipEntry {
    fn id(self) -> AudioClipId {
        clip_id(self.id)
    }
}

fn audio_gallery_clip_entries() -> [AudioGalleryClipEntry; 11] {
    [
        AudioGalleryClipEntry {
            id: AUDIO_GALLERY_UI_NOTIFY_CLIP_ID,
            path: AUDIO_GALLERY_UI_NOTIFY_CLIP_PATH,
        },
        AudioGalleryClipEntry {
            id: AUDIO_GALLERY_FOOTSTEP_CLIP_ID,
            path: AUDIO_GALLERY_FOOTSTEP_CLIP_PATH,
        },
        AudioGalleryClipEntry {
            id: AUDIO_GALLERY_SWORD_HIT_CLIP_ID,
            path: AUDIO_GALLERY_SWORD_HIT_CLIP_PATH,
        },
        AudioGalleryClipEntry {
            id: AUDIO_GALLERY_RAIN_LOOP_CLIP_ID,
            path: AUDIO_GALLERY_RAIN_LOOP_CLIP_PATH,
        },
        AudioGalleryClipEntry {
            id: AUDIO_GALLERY_MENU_MUSIC_CLIP_ID,
            path: AUDIO_GALLERY_MENU_MUSIC_CLIP_PATH,
        },
        AudioGalleryClipEntry {
            id: AUDIO_GALLERY_STEALTH_MUSIC_CLIP_ID,
            path: AUDIO_GALLERY_STEALTH_MUSIC_CLIP_PATH,
        },
        AudioGalleryClipEntry {
            id: AUDIO_GALLERY_CAR_HORN_CLIP_ID,
            path: AUDIO_GALLERY_CAR_HORN_CLIP_PATH,
        },
        AudioGalleryClipEntry {
            id: AUDIO_GALLERY_DOG_BARK_CLIP_ID,
            path: AUDIO_GALLERY_DOG_BARK_CLIP_PATH,
        },
        AudioGalleryClipEntry {
            id: AUDIO_GALLERY_VOICE_CLIP_ID,
            path: AUDIO_GALLERY_VOICE_CLIP_PATH,
        },
        AudioGalleryClipEntry {
            id: AUDIO_GALLERY_MISSING_CLIP_ID,
            path: AUDIO_GALLERY_MISSING_CLIP_PATH,
        },
        AudioGalleryClipEntry {
            id: AUDIO_GALLERY_RESIDENT_CLICK_CLIP_ID,
            path: AUDIO_GALLERY_RESIDENT_CLICK_CLIP_PATH,
        },
    ]
}

fn register_gallery_cue(
    catalog: &mut AudioCatalog,
    cue_id: &'static str,
    clip_id: &'static str,
    bus: AudioBus,
    scope: AudioScope,
    looped: bool,
    rules: AudioCueRules,
) {
    catalog.register_cue(
        AudioCueId::try_from(cue_id).expect("audio gallery cue id must be valid"),
        AudioCueEntry::from_clips([AudioCueClip::new(
            AudioClipId::try_from(clip_id).expect("audio gallery cue clip id must be valid"),
        )])
        .with_playback(AudioCuePlayback { bus, scope, looped })
        .with_rules(rules),
    );
}

fn clip_id(value: &str) -> AudioClipId {
    AudioClipId::try_from(value).expect("audio gallery clip id must be valid")
}

fn group_id(value: &str) -> AudioGroupId {
    AudioGroupId::try_from(value).expect("audio gallery group id must be valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cue_id(value: &str) -> AudioCueId {
        AudioCueId::try_from(value).unwrap()
    }

    fn expected_clip_paths() -> [(&'static str, &'static str); 11] {
        [
            (
                AUDIO_GALLERY_UI_NOTIFY_CLIP_ID,
                AUDIO_GALLERY_UI_NOTIFY_CLIP_PATH,
            ),
            (
                AUDIO_GALLERY_FOOTSTEP_CLIP_ID,
                AUDIO_GALLERY_FOOTSTEP_CLIP_PATH,
            ),
            (
                AUDIO_GALLERY_SWORD_HIT_CLIP_ID,
                AUDIO_GALLERY_SWORD_HIT_CLIP_PATH,
            ),
            (
                AUDIO_GALLERY_RAIN_LOOP_CLIP_ID,
                AUDIO_GALLERY_RAIN_LOOP_CLIP_PATH,
            ),
            (
                AUDIO_GALLERY_MENU_MUSIC_CLIP_ID,
                AUDIO_GALLERY_MENU_MUSIC_CLIP_PATH,
            ),
            (
                AUDIO_GALLERY_STEALTH_MUSIC_CLIP_ID,
                AUDIO_GALLERY_STEALTH_MUSIC_CLIP_PATH,
            ),
            (
                AUDIO_GALLERY_CAR_HORN_CLIP_ID,
                AUDIO_GALLERY_CAR_HORN_CLIP_PATH,
            ),
            (
                AUDIO_GALLERY_DOG_BARK_CLIP_ID,
                AUDIO_GALLERY_DOG_BARK_CLIP_PATH,
            ),
            (AUDIO_GALLERY_VOICE_CLIP_ID, AUDIO_GALLERY_VOICE_CLIP_PATH),
            (
                AUDIO_GALLERY_MISSING_CLIP_ID,
                AUDIO_GALLERY_MISSING_CLIP_PATH,
            ),
            (
                AUDIO_GALLERY_RESIDENT_CLICK_CLIP_ID,
                AUDIO_GALLERY_RESIDENT_CLICK_CLIP_PATH,
            ),
        ]
    }

    #[test]
    fn audio_gallery_dev_samples_register_clip_paths() {
        let mut catalog = AudioCatalog::default();
        register_audio_gallery_dev_samples_in_catalog(&mut catalog);

        for (id, path) in expected_clip_paths() {
            assert_eq!(catalog.clip(&clip_id(id)).unwrap().path, path);
            assert!(id.starts_with("dev.audio."));
        }
    }

    #[test]
    fn audio_gallery_dev_samples_register_cue_playback_and_rules() {
        let mut catalog = AudioCatalog::default();
        register_audio_gallery_dev_samples_in_catalog(&mut catalog);

        let cooldown = catalog
            .resolve_cue(&cue_id(AUDIO_GALLERY_COOLDOWN_CUE_ID))
            .unwrap();
        assert_eq!(
            cooldown.clips[0].clip_id,
            clip_id(AUDIO_GALLERY_UI_NOTIFY_CLIP_ID)
        );
        assert_eq!(cooldown.clips[0].path, AUDIO_GALLERY_UI_NOTIFY_CLIP_PATH);
        assert_eq!(cooldown.playback.bus, AudioBus::Ui);
        assert_eq!(cooldown.playback.scope, AudioScope::Ui);
        assert_eq!(cooldown.rules.cooldown_seconds, Some(0.75));

        let max_concurrent = catalog
            .resolve_cue(&cue_id(AUDIO_GALLERY_MAX_CONCURRENT_CUE_ID))
            .unwrap();
        assert_eq!(
            max_concurrent.clips[0].clip_id,
            clip_id(AUDIO_GALLERY_FOOTSTEP_CLIP_ID)
        );
        assert_eq!(max_concurrent.rules.max_concurrent, Some(1));

        let music = catalog
            .resolve_cue(&cue_id(AUDIO_GALLERY_MENU_MUSIC_CUE_ID))
            .unwrap();
        assert_eq!(music.clips[0].path, AUDIO_GALLERY_MENU_MUSIC_CLIP_PATH);
        assert_eq!(music.playback.bus, AudioBus::Music);
        assert!(music.playback.looped);

        let missing = catalog
            .resolve_cue(&cue_id(AUDIO_GALLERY_MISSING_CUE_ID))
            .unwrap();
        assert_eq!(missing.clips[0].path, AUDIO_GALLERY_MISSING_CLIP_PATH);
    }

    #[test]
    fn audio_gallery_dev_bank_group_covers_required_and_optional_clips() {
        let mut catalog = AudioCatalog::default();
        register_audio_gallery_dev_samples_in_catalog(&mut catalog);

        let group = catalog
            .resolve_group(&group_id(AUDIO_GALLERY_BANK_GROUP_ID))
            .unwrap();
        assert_eq!(group.clips.len(), 10);

        for required_clip in [
            AUDIO_GALLERY_UI_NOTIFY_CLIP_ID,
            AUDIO_GALLERY_FOOTSTEP_CLIP_ID,
            AUDIO_GALLERY_SWORD_HIT_CLIP_ID,
            AUDIO_GALLERY_RAIN_LOOP_CLIP_ID,
            AUDIO_GALLERY_MENU_MUSIC_CLIP_ID,
            AUDIO_GALLERY_STEALTH_MUSIC_CLIP_ID,
            AUDIO_GALLERY_CAR_HORN_CLIP_ID,
            AUDIO_GALLERY_DOG_BARK_CLIP_ID,
            AUDIO_GALLERY_VOICE_CLIP_ID,
        ] {
            assert!(group.clips.iter().any(|clip| {
                clip.clip_id == clip_id(required_clip)
                    && clip.required
                    && clip.path.starts_with("audio/")
            }));
        }

        assert!(group.clips.iter().any(|clip| {
            clip.clip_id == clip_id(AUDIO_GALLERY_MISSING_CLIP_ID)
                && !clip.required
                && clip.path == AUDIO_GALLERY_MISSING_CLIP_PATH
        }));
    }

    #[test]
    fn audio_gallery_resident_bank_is_separate_from_lazy_bank() {
        let mut catalog = AudioCatalog::default();
        register_audio_gallery_dev_samples_in_catalog(&mut catalog);

        let lazy_group = catalog
            .resolve_group(&group_id(AUDIO_GALLERY_BANK_GROUP_ID))
            .unwrap();
        let resident_group = catalog
            .resolve_group(&group_id(AUDIO_GALLERY_RESIDENT_BANK_GROUP_ID))
            .unwrap();

        assert_eq!(resident_group.clips.len(), 1);
        assert_eq!(
            resident_group.clips[0].clip_id,
            clip_id(AUDIO_GALLERY_RESIDENT_CLICK_CLIP_ID)
        );
        assert!(resident_group.clips[0].required);
        assert!(
            !lazy_group
                .clips
                .iter()
                .any(|clip| clip.clip_id == resident_group.clips[0].clip_id)
        );
    }

    #[test]
    fn audio_gallery_lazy_unload_config_marks_dev_and_resident_groups() {
        let config = AudioGalleryDevBankConfig::default();
        let dev_group_id = group_id(AUDIO_GALLERY_BANK_GROUP_ID);
        let resident_group_id = group_id(AUDIO_GALLERY_RESIDENT_BANK_GROUP_ID);

        let dev_unload = config
            .banks
            .iter()
            .find(|bank| bank.group_id == dev_group_id)
            .unwrap()
            .lazy_unload;
        let resident_unload = config
            .banks
            .iter()
            .find(|bank| bank.group_id == resident_group_id)
            .unwrap()
            .lazy_unload;

        assert!(dev_unload > Duration::ZERO);
        assert_eq!(dev_unload, Duration::from_secs_f32(12.0));
        assert_eq!(resident_unload, Duration::ZERO);
    }
}
