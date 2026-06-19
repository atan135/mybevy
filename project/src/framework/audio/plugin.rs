use bevy::prelude::*;

use crate::framework::scene::prelude::SceneEvent;
use crate::framework::ui::widgets::UiButtonEvent;

use super::{
    catalog::AudioCatalog,
    command::AudioCommand,
    debug::{AudioDebugConfig, AudioDebugSnapshot, AudioDebugState},
    event::AudioEvent,
    lifecycle::{AudioLifecyclePausePolicy, AudioLifecyclePauseState},
    loading::AudioLoadingState,
    mixer::AudioMixer,
    music::MusicController,
    playback::AudioPlaybackState,
    ui::{UiAudioAdapterConfig, UiAudioCooldowns},
};

pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<AudioCommand>()
            .add_message::<AudioEvent>()
            .add_message::<SceneEvent>()
            .add_message::<UiButtonEvent>()
            .add_message::<bevy::window::AppLifecycle>()
            .init_resource::<AudioCatalog>()
            .init_resource::<AudioMixer>()
            .init_resource::<AudioLifecyclePausePolicy>()
            .init_resource::<AudioLifecyclePauseState>()
            .init_resource::<AudioPlaybackState>()
            .init_resource::<AudioLoadingState>()
            .init_resource::<MusicController>()
            .insert_resource(AudioDebugConfig::from_env())
            .init_resource::<AudioDebugState>()
            .init_resource::<AudioDebugSnapshot>()
            .init_resource::<UiAudioAdapterConfig>()
            .init_resource::<UiAudioCooldowns>()
            .init_resource::<super::scene::SceneAudioAdapterConfig>()
            .configure_sets(
                Update,
                (
                    AudioSystemSet::Commands,
                    AudioSystemSet::Playback,
                    AudioSystemSet::Mixer,
                    AudioSystemSet::Cleanup,
                    AudioSystemSet::Debug,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    (
                        super::lifecycle::handle_audio_lifecycle_pause_policy,
                        super::mixer::handle_audio_mixer_commands,
                        super::scene::play_scene_audio_on_lifecycle,
                        super::ui::play_ui_button_audio,
                        super::music::handle_music_commands,
                        super::playback::handle_audio_playback_commands,
                        super::loading::handle_audio_loading_commands,
                    )
                        .chain()
                        .in_set(AudioSystemSet::Commands),
                    super::loading::poll_audio_group_load_progress.in_set(AudioSystemSet::Playback),
                    super::playback::report_audio_load_failures.in_set(AudioSystemSet::Playback),
                    super::playback::sync_audio_playback_positions.in_set(AudioSystemSet::Playback),
                    super::spatial::sync_spatial_listener_binding.in_set(AudioSystemSet::Playback),
                    super::spatial::sync_spatial_emitters.in_set(AudioSystemSet::Playback),
                    super::music::advance_music_fades.in_set(AudioSystemSet::Playback),
                    super::mixer::sync_audio_sinks_with_mixer.in_set(AudioSystemSet::Mixer),
                    super::spatial::sync_spatial_audio_sinks_with_mixer
                        .in_set(AudioSystemSet::Mixer),
                    super::playback::cleanup_finished_audio_instances
                        .in_set(AudioSystemSet::Cleanup),
                    super::music::cleanup_music_controller.in_set(AudioSystemSet::Cleanup),
                    super::debug::update_audio_debug_snapshot.in_set(AudioSystemSet::Debug),
                ),
            );
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
pub enum AudioSystemSet {
    Commands,
    Playback,
    Mixer,
    Cleanup,
    Debug,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::audio::{
        AudioBus, AudioBusVolumeCommand, AudioCatalog, AudioClipId, AudioClipRequest, AudioCommand,
        AudioCueClip, AudioCueEntry, AudioCueId, AudioCueStarted, AudioEvent, AudioInstanceId,
        AudioInstanceState, AudioLifecyclePausePolicy, AudioLifecyclePauseState, AudioLoadingState,
        AudioMixer, AudioPlaybackInstance, AudioPlaybackState, AudioScope,
        AudioSpatialListenerEntity, DEFAULT_BACKGROUND_PAUSED_BUSES, DEFAULT_UI_CLICK_CUE_ID,
        SceneAudioAdapterConfig, SceneAudioCue, SceneAudioEntry, SceneAudioPlayback,
    };
    use crate::framework::scene::prelude::{
        SceneEntered, SceneEvent, SceneExitStarted, SceneExited, SceneId, SceneSessionId,
    };
    use crate::framework::ui::widgets::UiButtonEventKind;
    use bevy::audio::{AudioSource, PlaybackSettings, Volume};
    use bevy::ecs::message::MessageCursor;

    #[test]
    fn audio_plugin_registers_messages_and_resources() {
        let mut app = App::new();
        app.add_plugins(AudioPlugin);

        assert!(app.world().contains_resource::<Messages<AudioCommand>>());
        assert!(app.world().contains_resource::<Messages<AudioEvent>>());
        assert!(app.world().contains_resource::<Messages<SceneEvent>>());
        assert!(app.world().contains_resource::<AudioCatalog>());
        assert!(app.world().contains_resource::<AudioMixer>());
        assert!(app.world().contains_resource::<AudioLifecyclePausePolicy>());
        assert!(app.world().contains_resource::<AudioLifecyclePauseState>());
        assert!(app.world().contains_resource::<AudioPlaybackState>());
        assert!(app.world().contains_resource::<AudioLoadingState>());
        assert!(app.world().contains_resource::<MusicController>());
        assert!(app.world().contains_resource::<AudioDebugConfig>());
        assert!(app.world().contains_resource::<AudioDebugState>());
        assert!(app.world().contains_resource::<AudioDebugSnapshot>());
        assert!(
            !app.world()
                .contains_resource::<AudioSpatialListenerEntity>()
        );
        assert!(
            app.world()
                .contains_resource::<Messages<bevy::window::AppLifecycle>>()
        );

        let mixer = app.world().resource::<AudioMixer>();
        assert!(mixer.buses.contains_key(&AudioBus::Master));
        assert!(mixer.buses.contains_key(&AudioBus::Music));
        assert!(mixer.buses.contains_key(&AudioBus::Sfx));
        assert!(mixer.buses.contains_key(&AudioBus::Ui));
        assert!(mixer.buses.contains_key(&AudioBus::Battle));

        let lifecycle_policy = app.world().resource::<AudioLifecyclePausePolicy>();
        assert_eq!(
            lifecycle_policy.paused_buses,
            DEFAULT_BACKGROUND_PAUSED_BUSES
        );
        assert!(!lifecycle_policy.paused_buses.contains(&AudioBus::Ui));
    }

    #[derive(Debug, Default, Resource)]
    struct MixerOrderProbe {
        target_volume_seen_after_mixer: Option<f32>,
    }

    fn capture_music_target_after_mixer(
        mixer: Res<AudioMixer>,
        playback: Res<AudioPlaybackState>,
        mut probe: ResMut<MixerOrderProbe>,
    ) {
        let Some(instance) = playback.instances.values().next() else {
            return;
        };

        probe.target_volume_seen_after_mixer =
            Some(mixer.target_instance_volume(instance.volume, instance.bus));
    }

    #[test]
    fn plugin_updates_mixer_before_later_mixer_systems_in_same_update() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), AudioPlugin))
            .init_resource::<MixerOrderProbe>()
            .add_systems(
                Update,
                capture_music_target_after_mixer.after(AudioSystemSet::Mixer),
            );

        let instance_id = AudioInstanceId::from_raw(7);
        let entity = app
            .world_mut()
            .spawn(AudioPlaybackInstance { instance_id })
            .id();
        app.world_mut()
            .resource_mut::<AudioPlaybackState>()
            .instances
            .insert(
                instance_id,
                AudioInstanceState {
                    entity,
                    clip_id: AudioClipId::try_from("music.title").unwrap(),
                    cue_id: None,
                    scope: AudioScope::Global,
                    bus: AudioBus::Music,
                    volume: 0.8,
                    priority: 0,
                    looped: false,
                    asset_path: "audio/music/title.ogg".to_string(),
                    source: Handle::<AudioSource>::default(),
                    failed: false,
                    paused: false,
                    stopping: false,
                    fade: None,
                    spatial: false,
                    start_seconds: 0.0,
                    position_seconds: 0.0,
                    pending_seek_seconds: None,
                },
            );
        app.world_mut()
            .write_message(AudioCommand::SetBusVolume(AudioBusVolumeCommand::new(
                AudioBus::Music,
                0.25,
            )));

        app.update();

        assert_eq!(
            app.world()
                .resource::<AudioMixer>()
                .bus_state(AudioBus::Music)
                .volume,
            0.25
        );
        assert_eq!(
            app.world()
                .resource::<MixerOrderProbe>()
                .target_volume_seen_after_mixer,
            Some(0.2)
        );
    }

    #[test]
    fn plugin_applies_same_frame_bus_command_to_new_playback_settings() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), AudioPlugin))
            .init_asset::<AudioSource>();

        let clip_id = AudioClipId::try_from("ui.click").unwrap();
        app.world_mut()
            .resource_mut::<AudioCatalog>()
            .register_clip(clip_id.clone(), "audio/ui/click.ogg");
        app.world_mut()
            .write_message(AudioCommand::SetBusVolume(AudioBusVolumeCommand::new(
                AudioBus::Ui,
                0.25,
            )));
        app.world_mut()
            .write_message(AudioCommand::PlayClip(AudioClipRequest {
                clip_id,
                scope: AudioScope::Ui,
                bus: AudioBus::Ui,
                volume: 0.8,
                pitch: 1.0,
                looped: false,
                fade_in_seconds: None,
                start_seconds: None,
            }));

        app.update();

        let playback = app.world().resource::<AudioPlaybackState>();
        let instance = playback.instances.values().next().unwrap();
        assert_eq!(instance.volume, 0.8);

        let settings = app
            .world()
            .entity(instance.entity)
            .get::<PlaybackSettings>()
            .unwrap();
        assert_eq!(settings.volume, Volume::Linear(0.2));
    }

    #[test]
    fn plugin_consumes_ui_button_audio_command_in_same_update() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), AudioPlugin))
            .init_asset::<AudioSource>();

        let cue_id = AudioCueId::try_from(DEFAULT_UI_CLICK_CUE_ID).unwrap();
        let clip_id = AudioClipId::try_from("ui.click").unwrap();
        app.world_mut()
            .resource_mut::<AudioCatalog>()
            .register_clip(clip_id.clone(), "audio/ui/click.ogg");
        app.world_mut().resource_mut::<AudioCatalog>().register_cue(
            cue_id.clone(),
            AudioCueEntry::from_clips([AudioCueClip::new(clip_id.clone())]),
        );

        let button = app.world_mut().spawn_empty().id();
        app.world_mut().write_message(UiButtonEvent {
            entity: button,
            kind: UiButtonEventKind::Click,
            button: None,
        });

        app.update();

        let playback = app.world().resource::<AudioPlaybackState>();
        assert_eq!(playback.instances.len(), 1);
        let (instance_id, instance) = playback.instances.iter().next().unwrap();
        assert_eq!(instance.bus, AudioBus::Ui);
        assert_eq!(instance.scope, AudioScope::Ui);
        assert_eq!(instance.cue_id, Some(cue_id.clone()));

        let messages = app.world().resource::<Messages<AudioEvent>>();
        let mut cursor = MessageCursor::default();
        let events = cursor.read(messages).cloned().collect::<Vec<_>>();
        assert!(events.contains(&AudioEvent::CueStarted(AudioCueStarted {
            cue_id,
            clip_id,
            instance_id: *instance_id,
            scope: AudioScope::Ui,
            bus: AudioBus::Ui,
        })));
    }

    #[test]
    fn plugin_pauses_background_audio_buses_for_mobile_lifecycle() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), AudioPlugin));

        app.world_mut()
            .resource_mut::<AudioMixer>()
            .set_bus_paused(AudioBus::Battle, true);
        app.world_mut()
            .write_message(bevy::window::AppLifecycle::WillSuspend);
        app.update();

        let mixer = app.world().resource::<AudioMixer>();
        assert!(mixer.bus_state(AudioBus::Music).paused);
        assert!(mixer.bus_state(AudioBus::Sfx).paused);
        assert!(mixer.bus_state(AudioBus::Battle).paused);
        assert!(!mixer.bus_state(AudioBus::Ui).paused);

        app.world_mut()
            .write_message(bevy::window::AppLifecycle::WillResume);
        app.update();

        let mixer = app.world().resource::<AudioMixer>();
        assert!(!mixer.bus_state(AudioBus::Music).paused);
        assert!(!mixer.bus_state(AudioBus::Sfx).paused);
        assert!(mixer.bus_state(AudioBus::Battle).paused);
        assert!(!mixer.bus_state(AudioBus::Ui).paused);
    }

    #[test]
    fn ui_button_audio_still_works_under_phone_profile_dimensions() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), AudioPlugin))
            .init_asset::<AudioSource>();

        let cue_id = AudioCueId::try_from(DEFAULT_UI_CLICK_CUE_ID).unwrap();
        let clip_id = AudioClipId::try_from("ui.click").unwrap();
        app.world_mut()
            .resource_mut::<AudioCatalog>()
            .register_clip(clip_id.clone(), "audio/ui/click_wood_01.wav");
        app.world_mut().resource_mut::<AudioCatalog>().register_cue(
            cue_id.clone(),
            AudioCueEntry::from_clips([AudioCueClip::new(clip_id.clone())]),
        );
        app.world_mut().spawn(bevy::window::Window {
            resolution: bevy::window::WindowResolution::new(720, 1600)
                .with_scale_factor_override(2.0),
            ..default()
        });

        let button = app.world_mut().spawn_empty().id();
        app.world_mut().write_message(UiButtonEvent {
            entity: button,
            kind: UiButtonEventKind::Click,
            button: None,
        });
        app.update();

        let playback = app.world().resource::<AudioPlaybackState>();
        assert_eq!(playback.instances.len(), 1);
        let instance = playback.instances.values().next().unwrap();
        assert_eq!(instance.bus, AudioBus::Ui);
        assert_eq!(instance.scope, AudioScope::Ui);
        assert_eq!(instance.asset_path, "audio/ui/click_wood_01.wav");
    }

    #[test]
    fn plugin_consumes_scene_audio_lifecycle_without_residual_instances() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), AudioPlugin))
            .init_asset::<AudioSource>();

        let scene_id = SceneId::from("sample.scene");
        let cue_id = AudioCueId::try_from("scene.sample.ambience").unwrap();
        let clip_id = AudioClipId::try_from("ambience.sample").unwrap();
        app.world_mut()
            .resource_mut::<AudioCatalog>()
            .register_clip(clip_id, "audio/ambience/sample.ogg");
        app.world_mut().resource_mut::<AudioCatalog>().register_cue(
            cue_id.clone(),
            AudioCueEntry::from_clips([AudioCueClip::new(
                AudioClipId::try_from("ambience.sample").unwrap(),
            )]),
        );
        app.world_mut()
            .resource_mut::<SceneAudioAdapterConfig>()
            .register(
                scene_id.clone(),
                SceneAudioEntry::from_playback(SceneAudioPlayback::Cue(
                    SceneAudioCue::ambience(cue_id).with_fade_in_seconds(0.1),
                ))
                .with_exit_fade_out_seconds(0.5),
            );

        for index in 1..=2 {
            let session_id = SceneSessionId::from(format!("sample.scene-{index}"));
            app.world_mut()
                .write_message(SceneEvent::Entered(SceneEntered {
                    scene_id: scene_id.clone(),
                    session_id: session_id.clone(),
                    content_version: None,
                }));
            app.update();

            let scene_scope = AudioScope::scene(session_id.as_str()).unwrap();
            assert_eq!(
                app.world()
                    .resource::<AudioPlaybackState>()
                    .instances
                    .values()
                    .filter(|instance| instance.scope == scene_scope)
                    .count(),
                1
            );

            app.world_mut()
                .write_message(SceneEvent::ExitStarted(SceneExitStarted {
                    scene_id: scene_id.clone(),
                    session_id: session_id.clone(),
                }));
            app.world_mut()
                .write_message(SceneEvent::Exited(SceneExited {
                    scene_id: scene_id.clone(),
                    session_id,
                }));
            app.update();

            assert!(
                app.world()
                    .resource::<AudioPlaybackState>()
                    .instances
                    .is_empty()
            );
        }
    }
}
