use std::collections::HashMap;

use bevy::asset::LoadState;
use bevy::audio::{PlaybackMode, Volume};
use bevy::prelude::*;

use super::{
    catalog::{AudioCatalog, AudioCatalogError, AudioResolvedCueClip},
    command::{AudioClipRequest, AudioCommand, AudioCueRequest},
    event::{
        AudioClipStarted, AudioCueStarted, AudioEvent, AudioInstanceStopped, AudioLoadFailed,
        AudioStopReason,
    },
    id::{AudioClipId, AudioCueId, AudioInstanceId},
    mixer::AudioMixer,
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
    pub volume: f32,
    pub asset_path: String,
    pub source: Handle<AudioSource>,
    pub failed: bool,
}

#[derive(Clone, Debug, Component, PartialEq)]
pub struct AudioPlaybackInstance {
    pub instance_id: AudioInstanceId,
}

pub fn handle_audio_playback_commands(
    mut commands: Commands,
    mut audio_commands: MessageReader<AudioCommand>,
    mut audio_events: MessageWriter<AudioEvent>,
    asset_server: Res<AssetServer>,
    catalog: Res<AudioCatalog>,
    mixer: Res<AudioMixer>,
    mut playback: ResMut<AudioPlaybackState>,
) {
    for command in audio_commands.read() {
        match command {
            AudioCommand::PlayCue(request) => {
                play_cue(
                    request,
                    &mut commands,
                    &mut audio_events,
                    &asset_server,
                    &catalog,
                    &mixer,
                    &mut playback,
                );
            }
            AudioCommand::PlayClip(request) => {
                play_clip(
                    request,
                    None,
                    &mut commands,
                    &mut audio_events,
                    &asset_server,
                    &catalog,
                    &mixer,
                    &mut playback,
                );
            }
            _ => {}
        }
    }
}

pub fn cleanup_finished_audio_instances(
    mut audio_events: MessageWriter<AudioEvent>,
    mut playback: ResMut<AudioPlaybackState>,
    instance_entities: Query<(), With<AudioPlaybackInstance>>,
) {
    let stopped = playback
        .instances
        .iter()
        .filter_map(|(instance_id, instance)| {
            (!instance.failed && instance_entities.get(instance.entity).is_err())
                .then_some(*instance_id)
        })
        .collect::<Vec<_>>();

    for instance_id in stopped {
        if let Some(instance) = playback.instances.remove(&instance_id) {
            audio_events.write(AudioEvent::InstanceStopped(AudioInstanceStopped {
                instance_id,
                clip_id: Some(instance.clip_id),
                cue_id: instance.cue_id,
                scope: instance.scope,
                bus: instance.bus,
                reason: AudioStopReason::Completed,
            }));
        }
    }
}

pub fn report_audio_load_failures(
    mut commands: Commands,
    mut audio_events: MessageWriter<AudioEvent>,
    asset_server: Res<AssetServer>,
    mut playback: ResMut<AudioPlaybackState>,
) {
    let mut failed_instances = Vec::new();

    for (instance_id, instance) in &playback.instances {
        if instance.failed {
            continue;
        }

        let Some(LoadState::Failed(error)) = asset_server.get_load_state(instance.source.id())
        else {
            continue;
        };

        failed_instances.push(*instance_id);
        audio_events.write(AudioEvent::LoadFailed(AudioLoadFailed {
            clip_id: Some(instance.clip_id.clone()),
            cue_id: instance.cue_id.clone(),
            group_id: None,
            asset_path: Some(instance.asset_path.clone()),
            message: error.to_string(),
        }));
        audio_events.write(AudioEvent::InstanceStopped(AudioInstanceStopped {
            instance_id: *instance_id,
            clip_id: Some(instance.clip_id.clone()),
            cue_id: instance.cue_id.clone(),
            scope: instance.scope.clone(),
            bus: instance.bus,
            reason: AudioStopReason::LoadFailed,
        }));
        commands.entity(instance.entity).try_despawn();
    }

    for instance_id in failed_instances {
        playback.instances.remove(&instance_id);
    }
}

fn play_cue(
    request: &AudioCueRequest,
    commands: &mut Commands,
    audio_events: &mut MessageWriter<AudioEvent>,
    asset_server: &AssetServer,
    catalog: &AudioCatalog,
    mixer: &AudioMixer,
    playback: &mut AudioPlaybackState,
) {
    let resolved = match catalog.resolve_cue(&request.cue_id) {
        Ok(resolved) => resolved,
        Err(error) => {
            send_catalog_failure(audio_events, &error, Some(request.cue_id.clone()));
            return;
        }
    };

    let Some(clip) = choose_cue_clip(&resolved.clips) else {
        send_catalog_failure(
            audio_events,
            &AudioCatalogError::EmptyCue(request.cue_id.clone()),
            Some(request.cue_id.clone()),
        );
        return;
    };

    let bus = request.bus.unwrap_or(resolved.playback.bus);
    let scope = if request.scope == AudioScope::Global {
        resolved.playback.scope
    } else {
        request.scope.clone()
    };
    let event_scope = scope.clone();
    let volume = request.volume * resolved.rules.volume;
    let pitch = request.pitch * resolved.rules.pitch;
    let looped = request.looped || resolved.playback.looped;

    let Some(instance_id) = spawn_audio_instance(
        commands,
        asset_server,
        mixer,
        playback,
        SpawnAudioInstance {
            clip_id: clip.clip_id.clone(),
            cue_id: Some(request.cue_id.clone()),
            asset_path: clip.path.clone(),
            scope,
            bus,
            volume,
            pitch,
            looped,
        },
    ) else {
        return;
    };

    audio_events.write(AudioEvent::CueStarted(AudioCueStarted {
        cue_id: request.cue_id.clone(),
        clip_id: clip.clip_id.clone(),
        instance_id,
        scope: event_scope,
        bus,
    }));
}

fn play_clip(
    request: &AudioClipRequest,
    cue_id: Option<AudioCueId>,
    commands: &mut Commands,
    audio_events: &mut MessageWriter<AudioEvent>,
    asset_server: &AssetServer,
    catalog: &AudioCatalog,
    mixer: &AudioMixer,
    playback: &mut AudioPlaybackState,
) {
    let clip = match catalog.clip(&request.clip_id) {
        Ok(clip) => clip,
        Err(error) => {
            send_catalog_failure(audio_events, &error, cue_id);
            return;
        }
    };

    let Some(instance_id) = spawn_audio_instance(
        commands,
        asset_server,
        mixer,
        playback,
        SpawnAudioInstance {
            clip_id: request.clip_id.clone(),
            cue_id,
            asset_path: clip.path.clone(),
            scope: request.scope.clone(),
            bus: request.bus,
            volume: request.volume,
            pitch: request.pitch,
            looped: request.looped,
        },
    ) else {
        return;
    };

    audio_events.write(AudioEvent::ClipStarted(AudioClipStarted {
        clip_id: request.clip_id.clone(),
        instance_id,
        scope: request.scope.clone(),
        bus: request.bus,
    }));
}

#[derive(Clone, Debug, PartialEq)]
struct SpawnAudioInstance {
    clip_id: AudioClipId,
    cue_id: Option<AudioCueId>,
    asset_path: String,
    scope: AudioScope,
    bus: AudioBus,
    volume: f32,
    pitch: f32,
    looped: bool,
}

fn spawn_audio_instance(
    commands: &mut Commands,
    asset_server: &AssetServer,
    mixer: &AudioMixer,
    playback: &mut AudioPlaybackState,
    request: SpawnAudioInstance,
) -> Option<AudioInstanceId> {
    let instance_id = AudioInstanceId::new();
    let source = asset_server.load::<AudioSource>(request.asset_path.clone());
    let volume = request.volume.max(0.0);
    let settings = PlaybackSettings {
        mode: if request.looped {
            PlaybackMode::Loop
        } else {
            PlaybackMode::Despawn
        },
        volume: Volume::Linear(mixer.target_instance_volume(volume, request.bus)),
        speed: request.pitch.max(0.01),
        paused: mixer.effective_bus_paused(request.bus),
        ..PlaybackSettings::default()
    };

    let entity = commands
        .spawn((
            AudioPlayer::new(source.clone()),
            settings,
            AudioPlaybackInstance { instance_id },
        ))
        .id();

    playback.instances.insert(
        instance_id,
        AudioInstanceState {
            entity,
            clip_id: request.clip_id,
            cue_id: request.cue_id,
            scope: request.scope,
            bus: request.bus,
            volume,
            asset_path: request.asset_path,
            source,
            failed: false,
        },
    );

    Some(instance_id)
}

fn choose_cue_clip(clips: &[AudioResolvedCueClip]) -> Option<&AudioResolvedCueClip> {
    clips
        .iter()
        .filter(|clip| clip.weight > 0.0)
        .max_by(|left, right| left.weight.total_cmp(&right.weight))
        .or_else(|| clips.first())
}

fn send_catalog_failure(
    audio_events: &mut MessageWriter<AudioEvent>,
    error: &AudioCatalogError,
    cue_id: Option<AudioCueId>,
) {
    let (clip_id, cue_id, asset_path) = match error {
        AudioCatalogError::MissingCue(missing_cue) => (None, Some(missing_cue.clone()), None),
        AudioCatalogError::MissingClip(missing_clip) => (Some(missing_clip.clone()), cue_id, None),
        AudioCatalogError::EmptyCue(empty_cue) => (None, Some(empty_cue.clone()), None),
    };

    audio_events.write(AudioEvent::LoadFailed(AudioLoadFailed {
        clip_id,
        cue_id,
        group_id: None,
        asset_path,
        message: error.to_string(),
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::audio::{AudioCueClip, AudioCueEntry, AudioCuePlayback, AudioCueRules};
    use bevy::ecs::message::MessageCursor;

    fn clip_id(value: &str) -> AudioClipId {
        AudioClipId::try_from(value).unwrap()
    }

    fn cue_id(value: &str) -> AudioCueId {
        AudioCueId::try_from(value).unwrap()
    }

    fn playback_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .add_message::<AudioCommand>()
            .add_message::<AudioEvent>()
            .init_asset::<AudioSource>()
            .init_resource::<AudioCatalog>()
            .init_resource::<AudioMixer>()
            .init_resource::<AudioPlaybackState>()
            .add_systems(Update, handle_audio_playback_commands);
        app
    }

    fn read_events(app: &App) -> Vec<AudioEvent> {
        let messages = app.world().resource::<Messages<AudioEvent>>();
        let mut cursor = MessageCursor::default();
        cursor.read(messages).cloned().collect()
    }

    #[test]
    fn play_clip_uses_catalog_path_and_records_started_instance() {
        let mut app = playback_app();
        let clip_id = clip_id("ui.click");
        app.world_mut()
            .resource_mut::<AudioCatalog>()
            .register_clip(clip_id.clone(), "audio/ui/click.ogg");
        app.world_mut()
            .write_message(AudioCommand::PlayClip(AudioClipRequest {
                clip_id: clip_id.clone(),
                scope: AudioScope::Ui,
                bus: AudioBus::Ui,
                volume: 0.75,
                pitch: 1.25,
                looped: false,
                fade_in_seconds: None,
            }));

        app.update();

        let playback = app.world().resource::<AudioPlaybackState>();
        assert_eq!(playback.instances.len(), 1);
        let (instance_id, instance) = playback.instances.iter().next().unwrap();
        assert_eq!(instance.clip_id, clip_id);
        assert_eq!(instance.cue_id, None);
        assert_eq!(instance.scope, AudioScope::Ui);
        assert_eq!(instance.bus, AudioBus::Ui);
        assert_eq!(instance.volume, 0.75);
        assert_eq!(instance.asset_path, "audio/ui/click.ogg");

        let entity = app.world().entity(instance.entity);
        assert_eq!(
            entity.get::<AudioPlaybackInstance>().unwrap().instance_id,
            *instance_id
        );
        assert!(entity.get::<AudioPlayer>().is_some());
        let settings = entity.get::<PlaybackSettings>().unwrap();
        assert!(matches!(settings.mode, PlaybackMode::Despawn));
        assert_eq!(settings.volume, Volume::Linear(0.75));
        assert_eq!(settings.speed, 1.25);

        assert_eq!(
            read_events(&app),
            vec![AudioEvent::ClipStarted(AudioClipStarted {
                clip_id,
                instance_id: *instance_id,
                scope: AudioScope::Ui,
                bus: AudioBus::Ui,
            })]
        );
    }

    #[test]
    fn play_cue_uses_catalog_defaults_rules_and_reports_cue_started() {
        let mut app = playback_app();
        let clip_id = clip_id("ui.click");
        let cue_id = cue_id("button.click");
        app.world_mut()
            .resource_mut::<AudioCatalog>()
            .register_clip(clip_id.clone(), "audio/ui/click.ogg");
        app.world_mut().resource_mut::<AudioCatalog>().register_cue(
            cue_id.clone(),
            AudioCueEntry::from_clips([AudioCueClip::new(clip_id.clone())])
                .with_playback(AudioCuePlayback {
                    bus: AudioBus::Ui,
                    scope: AudioScope::Ui,
                    looped: true,
                })
                .with_rules(AudioCueRules {
                    volume: 0.5,
                    pitch: 1.5,
                    cooldown_seconds: Some(0.2),
                    max_concurrent: Some(2),
                    priority: 10,
                }),
        );
        app.world_mut()
            .write_message(AudioCommand::PlayCue(AudioCueRequest {
                cue_id: cue_id.clone(),
                scope: AudioScope::Global,
                bus: None,
                volume: 0.8,
                pitch: 0.5,
                looped: false,
                fade_in_seconds: None,
            }));

        app.update();

        let playback = app.world().resource::<AudioPlaybackState>();
        assert_eq!(playback.instances.len(), 1);
        let (instance_id, instance) = playback.instances.iter().next().unwrap();
        assert_eq!(instance.clip_id, clip_id);
        assert_eq!(instance.cue_id, Some(cue_id.clone()));
        assert_eq!(instance.scope, AudioScope::Ui);
        assert_eq!(instance.bus, AudioBus::Ui);
        assert_eq!(instance.volume, 0.4);
        assert_eq!(instance.asset_path, "audio/ui/click.ogg");

        let settings = app
            .world()
            .entity(instance.entity)
            .get::<PlaybackSettings>()
            .unwrap();
        assert!(matches!(settings.mode, PlaybackMode::Loop));
        assert_eq!(settings.volume, Volume::Linear(0.4));
        assert_eq!(settings.speed, 0.75);

        assert_eq!(
            read_events(&app),
            vec![AudioEvent::CueStarted(AudioCueStarted {
                cue_id,
                clip_id,
                instance_id: *instance_id,
                scope: AudioScope::Ui,
                bus: AudioBus::Ui,
            })]
        );
    }

    #[test]
    fn play_clip_keeps_base_volume_but_uses_mixer_for_initial_settings() {
        let mut app = playback_app();
        let clip_id = clip_id("music.title");
        app.world_mut()
            .resource_mut::<AudioCatalog>()
            .register_clip(clip_id.clone(), "audio/music/title.ogg");
        app.world_mut()
            .resource_mut::<AudioMixer>()
            .set_bus_volume(AudioBus::Music, 0.25);
        app.world_mut()
            .write_message(AudioCommand::PlayClip(AudioClipRequest {
                clip_id: clip_id.clone(),
                scope: AudioScope::Global,
                bus: AudioBus::Music,
                volume: 0.8,
                pitch: 1.0,
                looped: true,
                fade_in_seconds: None,
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
    fn play_clip_uses_mixer_paused_state_for_initial_settings() {
        let mut app = playback_app();
        let clip_id = clip_id("ui.notice");
        app.world_mut()
            .resource_mut::<AudioCatalog>()
            .register_clip(clip_id.clone(), "audio/ui/notice.ogg");
        app.world_mut()
            .resource_mut::<AudioMixer>()
            .set_bus_paused(AudioBus::Master, true);
        app.world_mut()
            .write_message(AudioCommand::PlayClip(AudioClipRequest {
                clip_id: clip_id.clone(),
                scope: AudioScope::Ui,
                bus: AudioBus::Ui,
                volume: 0.6,
                pitch: 1.0,
                looped: false,
                fade_in_seconds: None,
            }));

        app.update();

        let playback = app.world().resource::<AudioPlaybackState>();
        let instance = playback.instances.values().next().unwrap();
        assert_eq!(instance.volume, 0.6);

        let settings = app
            .world()
            .entity(instance.entity)
            .get::<PlaybackSettings>()
            .unwrap();
        assert!(settings.paused);
        assert_eq!(settings.volume, Volume::Linear(0.6));
    }

    #[test]
    fn missing_cue_or_clip_sends_load_failed_without_instance() {
        let mut missing_cue_app = playback_app();
        let missing_cue = cue_id("ui.missing");
        missing_cue_app
            .world_mut()
            .write_message(AudioCommand::PlayCue(AudioCueRequest::new(
                missing_cue.clone(),
            )));

        missing_cue_app.update();

        assert!(
            missing_cue_app
                .world()
                .resource::<AudioPlaybackState>()
                .instances
                .is_empty()
        );
        assert_eq!(
            read_events(&missing_cue_app),
            vec![AudioEvent::LoadFailed(AudioLoadFailed {
                clip_id: None,
                cue_id: Some(missing_cue.clone()),
                group_id: None,
                asset_path: None,
                message: format!("audio cue not found: {missing_cue}"),
            })]
        );

        let mut missing_clip_app = playback_app();
        let missing_clip = clip_id("ui.missing");
        missing_clip_app
            .world_mut()
            .write_message(AudioCommand::PlayClip(AudioClipRequest::new(
                missing_clip.clone(),
            )));

        missing_clip_app.update();

        assert!(
            missing_clip_app
                .world()
                .resource::<AudioPlaybackState>()
                .instances
                .is_empty()
        );
        assert_eq!(
            read_events(&missing_clip_app),
            vec![AudioEvent::LoadFailed(AudioLoadFailed {
                clip_id: Some(missing_clip.clone()),
                cue_id: None,
                group_id: None,
                asset_path: None,
                message: format!("audio clip not found: {missing_clip}"),
            })]
        );
    }

    #[test]
    fn cleanup_removes_despawned_short_instance_and_reports_completed() {
        let mut app = App::new();
        app.add_message::<AudioEvent>()
            .init_resource::<AudioPlaybackState>()
            .add_systems(Update, cleanup_finished_audio_instances);

        let clip_id = clip_id("ui.click");
        let cue_id = cue_id("button.click");
        let entity = app
            .world_mut()
            .spawn(AudioPlaybackInstance {
                instance_id: AudioInstanceId::from_raw(99),
            })
            .id();
        let source = Handle::<AudioSource>::default();
        let instance_id = AudioInstanceId::from_raw(99);
        app.world_mut()
            .resource_mut::<AudioPlaybackState>()
            .instances
            .insert(
                instance_id,
                AudioInstanceState {
                    entity,
                    clip_id: clip_id.clone(),
                    cue_id: Some(cue_id.clone()),
                    scope: AudioScope::Ui,
                    bus: AudioBus::Ui,
                    volume: 1.0,
                    asset_path: "audio/ui/click.ogg".to_string(),
                    source,
                    failed: false,
                },
            );
        app.world_mut().entity_mut(entity).despawn();

        app.update();

        assert!(
            app.world()
                .resource::<AudioPlaybackState>()
                .instances
                .is_empty()
        );
        assert_eq!(
            read_events(&app),
            vec![AudioEvent::InstanceStopped(AudioInstanceStopped {
                instance_id,
                clip_id: Some(clip_id),
                cue_id: Some(cue_id),
                scope: AudioScope::Ui,
                bus: AudioBus::Ui,
                reason: AudioStopReason::Completed,
            })]
        );
    }
}
