use bevy::prelude::*;

use super::{
    catalog::AudioCatalog,
    command::{AudioCommand, AudioCrossfadeMusicRequest, AudioMusicRequest},
    event::{
        AudioEvent, AudioInstanceStopped, AudioLoadFailed, AudioMusicChanged, AudioStopReason,
    },
    id::{AudioClipId, AudioInstanceId},
    mixer::AudioMixer,
    playback::{AudioFadeState, AudioPlaybackState, SpawnAudioInstance, spawn_audio_instance},
    scope::{AudioBus, AudioScope},
};

#[derive(Debug, Default, Resource)]
pub struct MusicController {
    pub current: Option<MusicTrackState>,
    pub outgoing: Vec<MusicTrackState>,
    pub paused: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MusicTrackState {
    pub instance_id: AudioInstanceId,
    pub clip_id: AudioClipId,
    pub scope: AudioScope,
    pub volume: f32,
    pub fade: Option<MusicFadePlan>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MusicFadePlan {
    pub fade_in_seconds: Option<f32>,
    pub fade_out_seconds: Option<f32>,
}

pub fn handle_music_commands(
    mut commands: Commands,
    mut audio_commands: MessageReader<AudioCommand>,
    mut audio_events: MessageWriter<AudioEvent>,
    asset_server: Res<AssetServer>,
    catalog: Res<AudioCatalog>,
    mixer: Res<AudioMixer>,
    mut playback: ResMut<AudioPlaybackState>,
    mut music: ResMut<MusicController>,
) {
    for command in audio_commands.read() {
        match command {
            AudioCommand::PlayMusic(request) => play_music(
                request,
                None,
                &mut commands,
                &mut audio_events,
                &asset_server,
                &catalog,
                &mixer,
                &mut playback,
                &mut music,
            ),
            AudioCommand::CrossfadeMusic(request) => play_music(
                &AudioMusicRequest {
                    clip_id: request.clip_id.clone(),
                    scope: request.scope.clone(),
                    volume: request.volume,
                    looped: request.looped,
                    fade_in_seconds: Some(request.fade_seconds),
                },
                Some(request),
                &mut commands,
                &mut audio_events,
                &asset_server,
                &catalog,
                &mixer,
                &mut playback,
                &mut music,
            ),
            AudioCommand::StopMusic(command) => {
                stop_current_music(
                    command.fade_out_seconds,
                    &mut commands,
                    &mut audio_events,
                    &mut playback,
                    &mut music,
                );
            }
            AudioCommand::PauseMusic => {
                music.paused = true;
                if let Some(current) = &music.current {
                    if let Some(instance) = playback.instances.get_mut(&current.instance_id) {
                        instance.paused = true;
                    }
                }
            }
            AudioCommand::ResumeMusic => {
                music.paused = false;
                if let Some(current) = &music.current {
                    if let Some(instance) = playback.instances.get_mut(&current.instance_id) {
                        instance.paused = false;
                    }
                }
            }
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn play_music(
    request: &AudioMusicRequest,
    crossfade: Option<&AudioCrossfadeMusicRequest>,
    commands: &mut Commands,
    audio_events: &mut MessageWriter<AudioEvent>,
    asset_server: &AssetServer,
    catalog: &AudioCatalog,
    mixer: &AudioMixer,
    playback: &mut AudioPlaybackState,
    music: &mut MusicController,
) {
    let Ok(clip) = catalog.clip(&request.clip_id) else {
        audio_events.write(AudioEvent::LoadFailed(AudioLoadFailed {
            clip_id: Some(request.clip_id.clone()),
            cue_id: None,
            group_id: None,
            asset_path: None,
            message: format!("audio clip not found: {}", request.clip_id),
        }));
        return;
    };

    let previous = music.current.take();
    let previous_instance_id = previous.as_ref().map(|track| track.instance_id);
    let previous_clip_id = previous.as_ref().map(|track| track.clip_id.clone());
    let fade_out_seconds = crossfade
        .map(|request| Some(request.fade_seconds))
        .unwrap_or(None);

    if let Some(previous) = previous {
        if let Some(fade_seconds) = fade_out_seconds.filter(|seconds| *seconds > 0.0) {
            fade_out_instance(previous.instance_id, fade_seconds, playback);
            music.outgoing.push(MusicTrackState {
                fade: Some(MusicFadePlan {
                    fade_in_seconds: None,
                    fade_out_seconds: Some(fade_seconds),
                }),
                ..previous.clone()
            });
        } else {
            stop_instance_now(
                previous.instance_id,
                commands,
                audio_events,
                playback,
                AudioStopReason::ReplacedByMusic,
            );
        }
    }

    let Some(instance_id) = spawn_audio_instance(
        commands,
        asset_server,
        mixer,
        playback,
        SpawnAudioInstance {
            clip_id: request.clip_id.clone(),
            cue_id: None,
            asset_path: clip.path.clone(),
            scope: request.scope.clone(),
            bus: AudioBus::Music,
            volume: request.volume,
            pitch: 1.0,
            looped: request.looped,
            fade_in_seconds: request.fade_in_seconds,
            paused: music.paused,
        },
    ) else {
        return;
    };

    music.current = Some(MusicTrackState {
        instance_id,
        clip_id: request.clip_id.clone(),
        scope: request.scope.clone(),
        volume: request.volume.max(0.0),
        fade: request.fade_in_seconds.map(|seconds| MusicFadePlan {
            fade_in_seconds: Some(seconds),
            fade_out_seconds: None,
        }),
    });

    audio_events.write(AudioEvent::MusicChanged(AudioMusicChanged {
        previous_instance_id,
        previous_clip_id,
        new_instance_id: Some(instance_id),
        new_clip_id: request.clip_id.clone(),
        scope: request.scope.clone(),
        crossfade_seconds: crossfade.map(|request| request.fade_seconds),
    }));
}

fn stop_current_music(
    fade_out_seconds: Option<f32>,
    commands: &mut Commands,
    audio_events: &mut MessageWriter<AudioEvent>,
    playback: &mut AudioPlaybackState,
    music: &mut MusicController,
) {
    let Some(current) = music.current.take() else {
        return;
    };

    if let Some(seconds) = fade_out_seconds.filter(|seconds| *seconds > 0.0) {
        fade_out_instance(current.instance_id, seconds, playback);
        music.outgoing.push(MusicTrackState {
            fade: Some(MusicFadePlan {
                fade_in_seconds: None,
                fade_out_seconds: Some(seconds),
            }),
            ..current
        });
    } else {
        stop_instance_now(
            current.instance_id,
            commands,
            audio_events,
            playback,
            AudioStopReason::Stopped,
        );
    }
}

fn fade_out_instance(
    instance_id: AudioInstanceId,
    fade_out_seconds: f32,
    playback: &mut AudioPlaybackState,
) {
    if let Some(instance) = playback.instances.get_mut(&instance_id) {
        instance.fade = AudioFadeState::new(fade_out_seconds, instance.volume, 0.0, true);
        instance.stopping = true;
    }
}

fn stop_instance_now(
    instance_id: AudioInstanceId,
    commands: &mut Commands,
    audio_events: &mut MessageWriter<AudioEvent>,
    playback: &mut AudioPlaybackState,
    reason: AudioStopReason,
) {
    let Some(instance) = playback.instances.remove(&instance_id) else {
        return;
    };

    commands.entity(instance.entity).try_despawn();
    audio_events.write(AudioEvent::InstanceStopped(AudioInstanceStopped {
        instance_id,
        clip_id: Some(instance.clip_id),
        cue_id: instance.cue_id,
        scope: instance.scope,
        bus: instance.bus,
        reason,
    }));
}

pub fn advance_music_fades(
    mut commands: Commands,
    time: Res<Time>,
    mut audio_events: MessageWriter<AudioEvent>,
    mut playback: ResMut<AudioPlaybackState>,
    mut music: ResMut<MusicController>,
) {
    let fading_instances = advance_fade_state(&mut playback, time.delta_secs());
    stop_faded_music_instances(
        fading_instances,
        &mut commands,
        &mut audio_events,
        &mut playback,
        &mut music,
    );
}

pub(crate) fn stop_faded_music_instances(
    fading_instances: Vec<AudioInstanceId>,
    commands: &mut Commands,
    audio_events: &mut MessageWriter<AudioEvent>,
    playback: &mut AudioPlaybackState,
    music: &mut MusicController,
) {
    for instance_id in fading_instances {
        stop_instance_now(
            instance_id,
            commands,
            audio_events,
            playback,
            AudioStopReason::Stopped,
        );
        music
            .outgoing
            .retain(|track| track.instance_id != instance_id);
    }
}

pub fn advance_fade_state(
    playback: &mut AudioPlaybackState,
    delta_seconds: f32,
) -> Vec<AudioInstanceId> {
    playback
        .instances
        .iter_mut()
        .filter_map(|(instance_id, instance)| {
            let fade = instance.fade.as_mut()?;
            fade.elapsed_seconds += delta_seconds.max(0.0);
            instance.volume = fade.target_volume();

            if fade.is_finished() {
                let stop = fade.stop_when_finished;
                instance.volume = fade.to_volume;
                instance.fade = None;
                stop.then_some(*instance_id)
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
}

pub fn cleanup_music_controller(
    playback: Res<AudioPlaybackState>,
    mut music: ResMut<MusicController>,
) {
    if music
        .current
        .as_ref()
        .is_some_and(|current| !playback.instances.contains_key(&current.instance_id))
    {
        music.current = None;
    }
    music
        .outgoing
        .retain(|track| playback.instances.contains_key(&track.instance_id));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::audio::AudioMusicFadeCommand;
    use bevy::audio::PlaybackSettings;
    use bevy::ecs::message::MessageCursor;

    fn clip_id(value: &str) -> AudioClipId {
        AudioClipId::try_from(value).unwrap()
    }

    fn music_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .add_message::<AudioCommand>()
            .add_message::<AudioEvent>()
            .init_asset::<AudioSource>()
            .init_resource::<AudioCatalog>()
            .init_resource::<AudioMixer>()
            .init_resource::<AudioPlaybackState>()
            .init_resource::<MusicController>()
            .add_systems(Update, (handle_music_commands, advance_music_fades).chain());
        app
    }

    fn register_music(app: &mut App, id: &AudioClipId, path: &str) {
        app.world_mut()
            .resource_mut::<AudioCatalog>()
            .register_clip(id.clone(), path);
    }

    fn read_events(app: &App) -> Vec<AudioEvent> {
        let messages = app.world().resource::<Messages<AudioEvent>>();
        let mut cursor = MessageCursor::default();
        cursor.read(messages).cloned().collect()
    }

    #[test]
    fn play_music_replaces_current_music_and_removes_old_instance() {
        let mut app = music_app();
        let title = clip_id("music.title");
        let battle = clip_id("music.battle");
        register_music(&mut app, &title, "audio/music/title.ogg");
        register_music(&mut app, &battle, "audio/music/battle.ogg");

        app.world_mut()
            .write_message(AudioCommand::PlayMusic(AudioMusicRequest::new(
                title.clone(),
            )));
        app.update();
        let first = app
            .world()
            .resource::<MusicController>()
            .current
            .clone()
            .unwrap();
        let first_entity = app
            .world()
            .resource::<AudioPlaybackState>()
            .instances
            .get(&first.instance_id)
            .unwrap()
            .entity;

        app.world_mut()
            .write_message(AudioCommand::PlayMusic(AudioMusicRequest::new(
                battle.clone(),
            )));
        app.update();

        let music = app.world().resource::<MusicController>();
        let current = music.current.clone().unwrap();
        assert_eq!(current.clip_id, battle);
        assert_ne!(current.instance_id, first.instance_id);
        assert!(music.outgoing.is_empty());
        assert!(
            !app.world()
                .resource::<AudioPlaybackState>()
                .instances
                .contains_key(&first.instance_id)
        );
        assert!(app.world().get_entity(first_entity).is_err());
    }

    #[test]
    fn crossfade_music_records_outgoing_current_and_fade_seconds() {
        let mut app = music_app();
        let title = clip_id("music.title");
        let battle = clip_id("music.battle");
        register_music(&mut app, &title, "audio/music/title.ogg");
        register_music(&mut app, &battle, "audio/music/battle.ogg");

        app.world_mut()
            .write_message(AudioCommand::PlayMusic(AudioMusicRequest::new(
                title.clone(),
            )));
        app.update();
        let first = app
            .world()
            .resource::<MusicController>()
            .current
            .clone()
            .unwrap();

        app.world_mut().write_message(AudioCommand::CrossfadeMusic(
            AudioCrossfadeMusicRequest::new(battle.clone(), 1.5),
        ));
        app.update();

        let music = app.world().resource::<MusicController>();
        let current = music.current.clone().unwrap();
        assert_eq!(current.clip_id, battle);
        assert_eq!(
            current.fade,
            Some(MusicFadePlan {
                fade_in_seconds: Some(1.5),
                fade_out_seconds: None,
            })
        );
        assert_eq!(music.outgoing.len(), 1);
        assert_eq!(music.outgoing[0].instance_id, first.instance_id);
        assert_eq!(
            music.outgoing[0].fade,
            Some(MusicFadePlan {
                fade_in_seconds: None,
                fade_out_seconds: Some(1.5),
            })
        );

        let playback = app.world().resource::<AudioPlaybackState>();
        let old = playback.instances.get(&first.instance_id).unwrap();
        assert!(old.stopping);
        let fade = old.fade.unwrap();
        assert!(fade.elapsed_seconds >= 0.0);
        assert_eq!(fade.duration_seconds, 1.5);
        assert_eq!(fade.from_volume, 1.0);
        assert_eq!(fade.to_volume, 0.0);
        assert!(fade.stop_when_finished);
        assert_eq!(
            read_events(&app).last(),
            Some(&AudioEvent::MusicChanged(AudioMusicChanged {
                previous_instance_id: Some(first.instance_id),
                previous_clip_id: Some(title),
                new_instance_id: Some(current.instance_id),
                new_clip_id: battle,
                scope: AudioScope::Global,
                crossfade_seconds: Some(1.5),
            }))
        );
    }

    #[test]
    fn missing_music_clip_reports_load_failed_without_clearing_current() {
        let mut app = music_app();
        let title = clip_id("music.title");
        let missing = clip_id("music.missing");
        register_music(&mut app, &title, "audio/music/title.ogg");

        app.world_mut()
            .write_message(AudioCommand::PlayMusic(AudioMusicRequest::new(
                title.clone(),
            )));
        app.update();
        let current = app
            .world()
            .resource::<MusicController>()
            .current
            .clone()
            .unwrap();
        let instance_count = app.world().resource::<AudioPlaybackState>().instances.len();

        app.world_mut()
            .write_message(AudioCommand::PlayMusic(AudioMusicRequest::new(
                missing.clone(),
            )));
        app.update();

        assert_eq!(
            app.world().resource::<MusicController>().current,
            Some(current)
        );
        assert_eq!(
            app.world().resource::<AudioPlaybackState>().instances.len(),
            instance_count
        );
        assert_eq!(
            read_events(&app).last(),
            Some(&AudioEvent::LoadFailed(AudioLoadFailed {
                clip_id: Some(missing.clone()),
                cue_id: None,
                group_id: None,
                asset_path: None,
                message: format!("audio clip not found: {missing}"),
            }))
        );
    }

    #[test]
    fn play_music_while_controller_paused_sets_initial_playback_settings_paused() {
        let mut app = music_app();
        let title = clip_id("music.title");
        register_music(&mut app, &title, "audio/music/title.ogg");
        app.world_mut().resource_mut::<MusicController>().paused = true;

        app.world_mut()
            .write_message(AudioCommand::PlayMusic(AudioMusicRequest::new(title)));
        app.update();

        let current = app
            .world()
            .resource::<MusicController>()
            .current
            .clone()
            .unwrap();
        let playback = app.world().resource::<AudioPlaybackState>();
        let instance = playback.instances.get(&current.instance_id).unwrap();
        assert!(instance.paused);
        assert!(
            app.world()
                .entity(instance.entity)
                .get::<PlaybackSettings>()
                .unwrap()
                .paused
        );
    }

    #[test]
    fn stop_pause_and_resume_music_update_controller_and_instance_state() {
        let mut app = music_app();
        let title = clip_id("music.title");
        register_music(&mut app, &title, "audio/music/title.ogg");

        app.world_mut()
            .write_message(AudioCommand::PlayMusic(AudioMusicRequest::new(
                title.clone(),
            )));
        app.update();
        let instance_id = app
            .world()
            .resource::<MusicController>()
            .current
            .as_ref()
            .unwrap()
            .instance_id;

        app.world_mut().write_message(AudioCommand::PauseMusic);
        app.update();
        assert!(app.world().resource::<MusicController>().paused);
        assert!(
            app.world()
                .resource::<AudioPlaybackState>()
                .instances
                .get(&instance_id)
                .unwrap()
                .paused
        );

        app.world_mut().write_message(AudioCommand::ResumeMusic);
        app.update();
        assert!(!app.world().resource::<MusicController>().paused);
        assert!(
            !app.world()
                .resource::<AudioPlaybackState>()
                .instances
                .get(&instance_id)
                .unwrap()
                .paused
        );

        app.world_mut()
            .write_message(AudioCommand::StopMusic(AudioMusicFadeCommand::new()));
        app.update();

        assert!(app.world().resource::<MusicController>().current.is_none());
        assert!(
            !app.world()
                .resource::<AudioPlaybackState>()
                .instances
                .contains_key(&instance_id)
        );
    }

    #[test]
    fn fade_state_reaches_target_volume_and_marks_stopped_instance() {
        let mut playback = AudioPlaybackState::default();
        let instance_id = AudioInstanceId::from_raw(300);
        playback.instances.insert(
            instance_id,
            super::super::playback::AudioInstanceState {
                entity: Entity::from_raw_u32(300).unwrap(),
                clip_id: clip_id("music.title"),
                cue_id: None,
                scope: AudioScope::Global,
                bus: AudioBus::Music,
                volume: 1.0,
                asset_path: "audio/music/title.ogg".to_string(),
                source: Handle::<AudioSource>::default(),
                failed: false,
                paused: false,
                stopping: true,
                fade: AudioFadeState::new(0.5, 1.0, 0.0, true),
            },
        );

        assert!(advance_fade_state(&mut playback, 0.25).is_empty());
        assert_eq!(playback.instances.get(&instance_id).unwrap().volume, 0.5);

        assert_eq!(advance_fade_state(&mut playback, 0.25), vec![instance_id]);
        let instance = playback.instances.get(&instance_id).unwrap();
        assert_eq!(instance.volume, 0.0);
        assert_eq!(instance.fade, None);
    }

    #[test]
    fn faded_out_old_music_instance_is_cleaned_after_crossfade() {
        let mut app = music_app();
        let title = clip_id("music.title");
        let battle = clip_id("music.battle");
        register_music(&mut app, &title, "audio/music/title.ogg");
        register_music(&mut app, &battle, "audio/music/battle.ogg");

        app.world_mut()
            .write_message(AudioCommand::PlayMusic(AudioMusicRequest::new(title)));
        app.update();
        let old = app
            .world()
            .resource::<MusicController>()
            .current
            .clone()
            .unwrap();
        let old_entity = app
            .world()
            .resource::<AudioPlaybackState>()
            .instances
            .get(&old.instance_id)
            .unwrap()
            .entity;

        app.world_mut().write_message(AudioCommand::CrossfadeMusic(
            AudioCrossfadeMusicRequest::new(battle, 0.0),
        ));
        app.update();

        assert!(
            !app.world()
                .resource::<AudioPlaybackState>()
                .instances
                .contains_key(&old.instance_id)
        );
        assert!(app.world().get_entity(old_entity).is_err());
        assert!(
            app.world()
                .resource::<MusicController>()
                .outgoing
                .is_empty()
        );
    }

    #[test]
    fn positive_crossfade_cleans_old_music_after_fade_finishes() {
        let mut app = music_app();
        let title = clip_id("music.title");
        let battle = clip_id("music.battle");
        register_music(&mut app, &title, "audio/music/title.ogg");
        register_music(&mut app, &battle, "audio/music/battle.ogg");

        app.world_mut()
            .write_message(AudioCommand::PlayMusic(AudioMusicRequest::new(title)));
        app.update();
        let old = app
            .world()
            .resource::<MusicController>()
            .current
            .clone()
            .unwrap();
        let old_entity = app
            .world()
            .resource::<AudioPlaybackState>()
            .instances
            .get(&old.instance_id)
            .unwrap()
            .entity;

        app.world_mut().write_message(AudioCommand::CrossfadeMusic(
            AudioCrossfadeMusicRequest::new(battle, 0.25),
        ));
        app.update();
        assert!(
            app.world()
                .resource::<AudioPlaybackState>()
                .instances
                .contains_key(&old.instance_id)
        );
        assert_eq!(app.world().resource::<MusicController>().outgoing.len(), 1);

        fn finish_music_fades(
            mut commands: Commands,
            mut audio_events: MessageWriter<AudioEvent>,
            mut playback: ResMut<AudioPlaybackState>,
            mut music: ResMut<MusicController>,
        ) {
            let fading_instances = advance_fade_state(&mut playback, 0.3);
            stop_faded_music_instances(
                fading_instances,
                &mut commands,
                &mut audio_events,
                &mut playback,
                &mut music,
            );
        }

        app.add_systems(Update, finish_music_fades);
        app.update();

        assert!(
            !app.world()
                .resource::<AudioPlaybackState>()
                .instances
                .contains_key(&old.instance_id)
        );
        assert!(app.world().get_entity(old_entity).is_err());
        assert!(
            app.world()
                .resource::<MusicController>()
                .outgoing
                .is_empty()
        );
    }
}
