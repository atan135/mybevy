use bevy::prelude::*;

use crate::framework::audio::prelude::{
    AudioBankGroupConfig, AudioBankRuntime, AudioBus, AudioCatalog, AudioClipId, AudioCueClip,
    AudioCueEntry, AudioCueId, AudioCuePlayback, AudioCueRules, AudioScope,
    DEFAULT_UI_CLICK_CUE_ID,
};

#[path = "audio_dev_samples.rs"]
pub(in crate::game) mod dev_samples;

const UI_CLICK_CLIP_ID: &str = "ui.click_wood_01";
const UI_CLICK_CLIP_PATH: &str = "audio/ui/click_wood_01.wav";
const UI_CONFIRM_CLIP_ID: &str = "ui.confirm_brick_01";
const UI_CONFIRM_CLIP_PATH: &str = "audio/ui/confirm_brick_01.wav";
pub(in crate::game) const UI_CONFIRM_CUE_ID: &str = "ui.confirm";

pub(in crate::game) struct GameAudioPlugin;

impl Plugin for GameAudioPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<dev_samples::AudioGalleryDevBankConfig>()
            .add_systems(
                Startup,
                (
                    register_game_ui_audio,
                    dev_samples::register_audio_gallery_dev_samples,
                    register_audio_gallery_dev_banks,
                ),
            );
    }
}

fn register_audio_gallery_dev_banks(
    config: Option<Res<dev_samples::AudioGalleryDevBankConfig>>,
    bank: Option<ResMut<AudioBankRuntime>>,
) {
    let (Some(config), Some(mut bank)) = (config, bank) else {
        return;
    };

    for setting in &config.banks {
        bank.register_group_config(AudioBankGroupConfig::new(
            setting.group_id.clone(),
            setting.lazy_unload,
        ));
    }
}

fn register_game_ui_audio(catalog: Option<ResMut<AudioCatalog>>) {
    let Some(mut catalog) = catalog else {
        return;
    };

    register_ui_cue(
        &mut catalog,
        DEFAULT_UI_CLICK_CUE_ID,
        UI_CLICK_CLIP_ID,
        UI_CLICK_CLIP_PATH,
    );
    register_ui_cue(
        &mut catalog,
        UI_CONFIRM_CUE_ID,
        UI_CONFIRM_CLIP_ID,
        UI_CONFIRM_CLIP_PATH,
    );
}

fn register_ui_cue(
    catalog: &mut AudioCatalog,
    cue_id: &str,
    clip_id: &str,
    clip_path: &'static str,
) {
    let clip_id = AudioClipId::try_from(clip_id).expect("game UI audio clip id must be valid");
    let cue_id = AudioCueId::try_from(cue_id).expect("game UI audio cue id must be valid");

    catalog.register_clip(clip_id.clone(), clip_path);
    catalog.register_cue(
        cue_id,
        AudioCueEntry::from_clips([AudioCueClip::new(clip_id)])
            .with_playback(AudioCuePlayback {
                bus: AudioBus::Ui,
                scope: AudioScope::Ui,
                looped: false,
            })
            .with_rules(AudioCueRules {
                volume: 1.0,
                pitch: 1.0,
                cooldown_seconds: None,
                max_concurrent: Some(4),
                priority: 0,
            }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::{
        audio::prelude::{
            AudioBankRuntime, AudioPlaybackState, AudioPlugin, DEFAULT_UI_CLICK_CUE_ID,
            UiAudioCueOverride,
        },
        ui::widgets::{UiButtonEvent, UiButtonEventKind},
    };
    use bevy::asset::AssetPlugin;

    fn cue_id(value: &str) -> AudioCueId {
        AudioCueId::try_from(value).unwrap()
    }

    fn clip_id(value: &str) -> AudioClipId {
        AudioClipId::try_from(value).unwrap()
    }

    fn group_id(value: &str) -> crate::framework::audio::prelude::AudioGroupId {
        crate::framework::audio::prelude::AudioGroupId::try_from(value).unwrap()
    }

    #[test]
    fn game_audio_registers_default_and_special_ui_cues() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, GameAudioPlugin))
            .init_resource::<AudioCatalog>();

        app.update();

        let catalog = app.world().resource::<AudioCatalog>();
        assert_eq!(
            catalog
                .resolve_cue(&cue_id(DEFAULT_UI_CLICK_CUE_ID))
                .unwrap(),
            crate::framework::audio::prelude::AudioResolvedCue {
                cue_id: cue_id(DEFAULT_UI_CLICK_CUE_ID),
                clips: vec![crate::framework::audio::prelude::AudioResolvedCueClip {
                    clip_id: clip_id(UI_CLICK_CLIP_ID),
                    path: UI_CLICK_CLIP_PATH.to_string(),
                    weight: 1.0,
                }],
                playback: AudioCuePlayback {
                    bus: AudioBus::Ui,
                    scope: AudioScope::Ui,
                    looped: false,
                },
                rules: AudioCueRules {
                    volume: 1.0,
                    pitch: 1.0,
                    cooldown_seconds: None,
                    max_concurrent: Some(4),
                    priority: 0,
                },
            }
        );
        assert_eq!(
            catalog
                .resolve_cue(&cue_id(UI_CONFIRM_CUE_ID))
                .unwrap()
                .clips[0]
                .path,
            UI_CONFIRM_CLIP_PATH
        );
    }

    #[test]
    fn game_audio_registers_audio_gallery_dev_banks_with_framework_runtime() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            AudioPlugin,
            GameAudioPlugin,
        ))
        .init_asset::<bevy::audio::AudioSource>();

        app.update();

        let bank = app.world().resource::<AudioBankRuntime>();
        let lazy_group_id = group_id(dev_samples::AUDIO_GALLERY_BANK_GROUP_ID);
        let resident_group_id = group_id(dev_samples::AUDIO_GALLERY_RESIDENT_BANK_GROUP_ID);
        let lazy = bank.groups.get(&lazy_group_id).unwrap();
        let resident = bank.groups.get(&resident_group_id).unwrap();

        assert_eq!(lazy.lazy_unload, std::time::Duration::from_secs_f32(12.0));
        assert_eq!(resident.lazy_unload, std::time::Duration::ZERO);
        assert!(resident.resident());
    }

    #[test]
    fn ui_audio_override_plays_registered_special_cue() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            AudioPlugin,
            GameAudioPlugin,
        ))
        .init_asset::<bevy::audio::AudioSource>();
        app.update();

        let button = app
            .world_mut()
            .spawn(UiAudioCueOverride::try_from(UI_CONFIRM_CUE_ID).unwrap())
            .id();
        app.world_mut().write_message(UiButtonEvent {
            entity: button,
            kind: UiButtonEventKind::Click,
            button: None,
        });

        app.update();

        let playback = app.world().resource::<AudioPlaybackState>();
        let instance = playback.instances.values().next().unwrap();
        assert_eq!(instance.cue_id, Some(cue_id(UI_CONFIRM_CUE_ID)));
        assert_eq!(instance.bus, AudioBus::Ui);
        assert_eq!(instance.scope, AudioScope::Ui);
        assert_eq!(instance.asset_path, UI_CONFIRM_CLIP_PATH);
    }

    #[test]
    fn ui_button_without_override_plays_registered_default_click_cue() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            AudioPlugin,
            GameAudioPlugin,
        ))
        .init_asset::<bevy::audio::AudioSource>();
        app.update();

        let button = app.world_mut().spawn_empty().id();
        app.world_mut().write_message(UiButtonEvent {
            entity: button,
            kind: UiButtonEventKind::Click,
            button: None,
        });

        app.update();

        let playback = app.world().resource::<AudioPlaybackState>();
        let instance = playback.instances.values().next().unwrap();
        assert_eq!(instance.cue_id, Some(cue_id(DEFAULT_UI_CLICK_CUE_ID)));
        assert_eq!(instance.bus, AudioBus::Ui);
        assert_eq!(instance.scope, AudioScope::Ui);
        assert_eq!(instance.asset_path, UI_CLICK_CLIP_PATH);
    }
}
