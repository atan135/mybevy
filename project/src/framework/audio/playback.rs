use std::collections::HashMap;

use bevy::asset::LoadState;
use bevy::audio::{PlaybackMode, Volume};
use bevy::prelude::*;

use super::{
    catalog::{AudioCatalog, AudioCatalogError, AudioResolvedCueClip},
    command::{
        AudioClipRequest, AudioCommand, AudioCueRequest, AudioScopeCommand, AudioScopeFadeCommand,
        AudioSpatialCueRequest,
    },
    event::{
        AudioClipStarted, AudioCueStarted, AudioEvent, AudioInstanceStopped, AudioLoadFailed,
        AudioStopReason,
    },
    id::{AudioClipId, AudioCueId, AudioInstanceId},
    mixer::AudioMixer,
    scope::{AudioBus, AudioScope},
    spatial::{AudioSpatialEmitter, AudioSpatialSource},
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
    pub paused: bool,
    pub stopping: bool,
    pub fade: Option<AudioFadeState>,
    pub spatial: bool,
}

#[derive(Clone, Debug, Component, PartialEq)]
pub struct AudioPlaybackInstance {
    pub instance_id: AudioInstanceId,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioFadeState {
    pub elapsed_seconds: f32,
    pub duration_seconds: f32,
    pub from_volume: f32,
    pub to_volume: f32,
    pub stop_when_finished: bool,
}

impl AudioFadeState {
    pub fn new(
        duration_seconds: f32,
        from_volume: f32,
        to_volume: f32,
        stop_when_finished: bool,
    ) -> Option<Self> {
        let duration_seconds = duration_seconds.max(0.0);
        (duration_seconds > 0.0).then_some(Self {
            elapsed_seconds: 0.0,
            duration_seconds,
            from_volume: from_volume.max(0.0),
            to_volume: to_volume.max(0.0),
            stop_when_finished,
        })
    }

    pub fn target_volume(&self) -> f32 {
        let progress = if self.duration_seconds <= 0.0 {
            1.0
        } else {
            (self.elapsed_seconds / self.duration_seconds).clamp(0.0, 1.0)
        };
        self.from_volume + (self.to_volume - self.from_volume) * progress
    }

    pub fn is_finished(&self) -> bool {
        self.elapsed_seconds >= self.duration_seconds
    }
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
            AudioCommand::PlaySpatialCue(request) => {
                play_spatial_cue(
                    request,
                    &mut commands,
                    &mut audio_events,
                    &asset_server,
                    &catalog,
                    &mixer,
                    &mut playback,
                );
            }
            AudioCommand::StopInstance(command) => {
                stop_instance_now(
                    command.instance_id,
                    command.fade_out_seconds,
                    &mut commands,
                    &mut audio_events,
                    &mut playback,
                    AudioStopReason::Stopped,
                );
            }
            AudioCommand::StopByScope(command) => {
                stop_by_scope(
                    command,
                    &mut commands,
                    &mut audio_events,
                    &mut playback,
                    AudioStopReason::StoppedByScope,
                );
            }
            AudioCommand::PauseByScope(command) => {
                set_scope_paused(command, true, &mut playback);
            }
            AudioCommand::ResumeByScope(command) => {
                set_scope_paused(command, false, &mut playback);
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
            fade_in_seconds: request.fade_in_seconds,
            paused: false,
            spatial: None,
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
            fade_in_seconds: request.fade_in_seconds,
            paused: false,
            spatial: None,
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

fn play_spatial_cue(
    request: &AudioSpatialCueRequest,
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
    let scope = match (&scope, &request.source) {
        (AudioScope::Global, AudioSpatialSource::FollowEntity(target)) => {
            AudioScope::Entity(*target)
        }
        _ => scope,
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
            fade_in_seconds: request.fade_in_seconds,
            paused: false,
            spatial: Some(SpawnSpatialAudioInstance {
                source: request.source.clone(),
                attenuation: request.attenuation.normalized(),
            }),
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

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SpawnAudioInstance {
    pub clip_id: AudioClipId,
    pub cue_id: Option<AudioCueId>,
    pub asset_path: String,
    pub scope: AudioScope,
    pub bus: AudioBus,
    pub volume: f32,
    pub pitch: f32,
    pub looped: bool,
    pub fade_in_seconds: Option<f32>,
    pub paused: bool,
    pub spatial: Option<SpawnSpatialAudioInstance>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SpawnSpatialAudioInstance {
    pub source: AudioSpatialSource,
    pub attenuation: super::spatial::AudioSpatialAttenuation,
}

pub(crate) fn spawn_audio_instance(
    commands: &mut Commands,
    asset_server: &AssetServer,
    mixer: &AudioMixer,
    playback: &mut AudioPlaybackState,
    request: SpawnAudioInstance,
) -> Option<AudioInstanceId> {
    let instance_id = AudioInstanceId::new();
    let source = asset_server.load::<AudioSource>(request.asset_path.clone());
    let volume = request.volume.max(0.0);
    let fade = request
        .fade_in_seconds
        .and_then(|seconds| AudioFadeState::new(seconds, 0.0, volume, false));
    let startup_volume = fade.as_ref().map_or(volume, AudioFadeState::target_volume);
    let settings = PlaybackSettings {
        mode: if request.looped {
            PlaybackMode::Loop
        } else {
            PlaybackMode::Despawn
        },
        volume: Volume::Linear(mixer.target_instance_volume(startup_volume, request.bus)),
        speed: request.pitch.max(0.01),
        paused: request.paused || mixer.effective_bus_paused(request.bus),
        ..PlaybackSettings::default()
    }
    .with_spatial(request.spatial.is_some());

    let spatial = request.spatial.clone();
    let transform = spatial
        .as_ref()
        .map(|spatial| match spatial.source {
            AudioSpatialSource::Fixed(transform) => transform,
            AudioSpatialSource::FollowEntity(_) => Transform::default(),
        })
        .unwrap_or_default();
    let global_transform = GlobalTransform::from(transform);
    let is_spatial = spatial.is_some();

    let mut entity_commands = commands.spawn((
        AudioPlayer::new(source.clone()),
        settings,
        AudioPlaybackInstance { instance_id },
        transform,
        global_transform,
    ));
    if let Some(spatial) = spatial {
        entity_commands.insert(AudioSpatialEmitter {
            source: spatial.source,
            attenuation: spatial.attenuation,
        });
    }
    let entity = entity_commands.id();

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
            paused: request.paused,
            stopping: false,
            fade,
            spatial: is_spatial,
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

pub(crate) fn stop_by_scope(
    command: &AudioScopeFadeCommand,
    commands: &mut Commands,
    audio_events: &mut MessageWriter<AudioEvent>,
    playback: &mut AudioPlaybackState,
    reason: AudioStopReason,
) -> Vec<AudioInstanceId> {
    let instance_ids = playback
        .instances
        .iter()
        .filter_map(|(instance_id, instance)| {
            (instance.scope == command.scope
                && (!instance.stopping || command.fade_out_seconds.is_none()))
            .then_some(*instance_id)
        })
        .collect::<Vec<_>>();

    for instance_id in &instance_ids {
        stop_instance_now(
            *instance_id,
            command.fade_out_seconds,
            commands,
            audio_events,
            playback,
            reason,
        );
    }

    instance_ids
}

pub(crate) fn stop_instance_now(
    instance_id: AudioInstanceId,
    fade_out_seconds: Option<f32>,
    commands: &mut Commands,
    audio_events: &mut MessageWriter<AudioEvent>,
    playback: &mut AudioPlaybackState,
    reason: AudioStopReason,
) -> bool {
    if let Some(seconds) = fade_out_seconds.filter(|seconds| *seconds > 0.0) {
        fade_out_instance(instance_id, seconds, playback);
        return playback.instances.contains_key(&instance_id);
    }

    stop_instance_immediately(instance_id, commands, audio_events, playback, reason)
}

pub(crate) fn stop_instance_immediately(
    instance_id: AudioInstanceId,
    commands: &mut Commands,
    audio_events: &mut MessageWriter<AudioEvent>,
    playback: &mut AudioPlaybackState,
    reason: AudioStopReason,
) -> bool {
    let Some(instance) = playback.instances.remove(&instance_id) else {
        return false;
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
    true
}

pub(crate) fn fade_out_instance(
    instance_id: AudioInstanceId,
    fade_out_seconds: f32,
    playback: &mut AudioPlaybackState,
) -> bool {
    let Some(instance) = playback.instances.get_mut(&instance_id) else {
        return false;
    };

    instance.fade = AudioFadeState::new(fade_out_seconds, instance.volume, 0.0, true);
    instance.stopping = true;
    true
}

fn set_scope_paused(command: &AudioScopeCommand, paused: bool, playback: &mut AudioPlaybackState) {
    for instance in playback.instances.values_mut() {
        if instance.scope == command.scope {
            instance.paused = paused;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::audio::{
        AudioCueClip, AudioCueEntry, AudioCuePlayback, AudioCueRules, AudioSpatialAttenuation,
        AudioSpatialCueRequest, AudioSpatialEmitter, AudioSpatialSource,
    };
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
    fn play_spatial_cue_spawns_spatial_audio_entity_with_fixed_transform() {
        let mut app = playback_app();
        let clip_id = clip_id("ambience.torch");
        let cue_id = cue_id("scene.torch");
        let transform = Transform::from_xyz(10.0, 20.0, 0.0);
        let attenuation = AudioSpatialAttenuation::new(30.0, 2.0);
        app.world_mut()
            .resource_mut::<AudioCatalog>()
            .register_clip(clip_id.clone(), "audio/ambience/torch.ogg");
        app.world_mut().resource_mut::<AudioCatalog>().register_cue(
            cue_id.clone(),
            AudioCueEntry::from_clips([AudioCueClip::new(clip_id.clone())]).with_playback(
                AudioCuePlayback {
                    bus: AudioBus::Sfx,
                    scope: AudioScope::scene("scene-1").unwrap(),
                    looped: true,
                },
            ),
        );

        app.world_mut()
            .write_message(AudioCommand::PlaySpatialCue(AudioSpatialCueRequest {
                cue_id: cue_id.clone(),
                scope: AudioScope::Global,
                bus: None,
                volume: 0.5,
                pitch: 1.25,
                looped: false,
                fade_in_seconds: None,
                source: AudioSpatialSource::fixed(transform),
                attenuation,
            }));
        app.update();

        let playback = app.world().resource::<AudioPlaybackState>();
        assert_eq!(playback.instances.len(), 1);
        let (instance_id, instance) = playback.instances.iter().next().unwrap();
        assert_eq!(instance.clip_id, clip_id);
        assert_eq!(instance.cue_id, Some(cue_id.clone()));
        assert_eq!(instance.scope, AudioScope::scene("scene-1").unwrap());
        assert_eq!(instance.bus, AudioBus::Sfx);
        assert_eq!(instance.volume, 0.5);
        assert!(instance.spatial);

        let entity = app.world().entity(instance.entity);
        let settings = entity.get::<PlaybackSettings>().unwrap();
        assert!(settings.spatial);
        assert!(matches!(settings.mode, PlaybackMode::Loop));
        assert_eq!(settings.volume, Volume::Linear(0.5));
        assert_eq!(settings.speed, 1.25);
        assert_eq!(*entity.get::<Transform>().unwrap(), transform);
        assert!(entity.get::<GlobalTransform>().is_some());
        assert_eq!(
            entity.get::<AudioSpatialEmitter>().unwrap(),
            &AudioSpatialEmitter {
                source: AudioSpatialSource::fixed(transform),
                attenuation,
            }
        );

        assert_eq!(
            read_events(&app),
            vec![AudioEvent::CueStarted(AudioCueStarted {
                cue_id,
                clip_id,
                instance_id: *instance_id,
                scope: AudioScope::scene("scene-1").unwrap(),
                bus: AudioBus::Sfx,
            })]
        );
    }

    #[test]
    fn play_spatial_cue_can_follow_entity_and_default_scope_to_entity() {
        let mut app = playback_app();
        let clip_id = clip_id("ambience.crystal");
        let cue_id = cue_id("scene.crystal");
        let target = app
            .world_mut()
            .spawn(Transform::from_xyz(2.0, 0.0, 0.0))
            .id();
        app.world_mut()
            .resource_mut::<AudioCatalog>()
            .register_clip(clip_id.clone(), "audio/ambience/crystal.ogg");
        app.world_mut().resource_mut::<AudioCatalog>().register_cue(
            cue_id.clone(),
            AudioCueEntry::from_clips([AudioCueClip::new(clip_id.clone())]),
        );

        app.world_mut()
            .write_message(AudioCommand::PlaySpatialCue(AudioSpatialCueRequest::new(
                cue_id.clone(),
                AudioSpatialSource::follow_entity(target),
            )));
        app.update();

        let playback = app.world().resource::<AudioPlaybackState>();
        let instance = playback.instances.values().next().unwrap();
        assert_eq!(instance.scope, AudioScope::Entity(target));
        assert!(instance.spatial);

        let entity = app.world().entity(instance.entity);
        assert_eq!(
            entity.get::<AudioSpatialEmitter>().unwrap().source,
            AudioSpatialSource::follow_entity(target)
        );
        assert_eq!(*entity.get::<Transform>().unwrap(), Transform::default());
        assert!(entity.get::<PlaybackSettings>().unwrap().spatial);
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
                    paused: false,
                    stopping: false,
                    fade: None,
                    spatial: false,
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

    #[test]
    fn stop_by_scope_removes_matching_instances_and_reports_stopped_by_scope() {
        let mut app = playback_app();
        let scene_clip = clip_id("ambience.room");
        let ui_clip = clip_id("ui.click");
        app.world_mut()
            .resource_mut::<AudioCatalog>()
            .register_clip(scene_clip.clone(), "audio/ambience/room.ogg");
        app.world_mut()
            .resource_mut::<AudioCatalog>()
            .register_clip(ui_clip.clone(), "audio/ui/click.ogg");

        app.world_mut()
            .write_message(AudioCommand::PlayClip(AudioClipRequest {
                clip_id: scene_clip.clone(),
                scope: AudioScope::scene("scene-1").unwrap(),
                bus: AudioBus::Sfx,
                volume: 0.5,
                pitch: 1.0,
                looped: true,
                fade_in_seconds: None,
            }));
        app.world_mut()
            .write_message(AudioCommand::PlayClip(AudioClipRequest {
                clip_id: ui_clip,
                scope: AudioScope::Ui,
                bus: AudioBus::Ui,
                volume: 1.0,
                pitch: 1.0,
                looped: false,
                fade_in_seconds: None,
            }));
        app.update();

        app.world_mut()
            .write_message(AudioCommand::StopByScope(AudioScopeFadeCommand {
                scope: AudioScope::scene("scene-1").unwrap(),
                fade_out_seconds: None,
            }));
        app.update();

        let playback = app.world().resource::<AudioPlaybackState>();
        assert_eq!(playback.instances.len(), 1);
        assert!(
            playback
                .instances
                .values()
                .all(|instance| instance.scope == AudioScope::Ui)
        );
        assert!(read_events(&app).iter().any(|event| matches!(
            event,
            AudioEvent::InstanceStopped(AudioInstanceStopped {
                clip_id: Some(clip_id),
                scope,
                reason: AudioStopReason::StoppedByScope,
                ..
            }) if clip_id == &scene_clip && scope == &AudioScope::scene("scene-1").unwrap()
        )));
    }

    #[test]
    fn stop_by_scope_removes_matching_spatial_instances() {
        let mut app = playback_app();
        let clip_id = clip_id("ambience.waterfall");
        let cue_id = cue_id("scene.waterfall");
        let scene_scope = AudioScope::scene("scene-1").unwrap();
        app.world_mut()
            .resource_mut::<AudioCatalog>()
            .register_clip(clip_id.clone(), "audio/ambience/waterfall.ogg");
        app.world_mut().resource_mut::<AudioCatalog>().register_cue(
            cue_id.clone(),
            AudioCueEntry::from_clips([AudioCueClip::new(clip_id.clone())]),
        );

        app.world_mut()
            .write_message(AudioCommand::PlaySpatialCue(AudioSpatialCueRequest {
                cue_id: cue_id.clone(),
                scope: scene_scope.clone(),
                bus: Some(AudioBus::Sfx),
                volume: 1.0,
                pitch: 1.0,
                looped: true,
                fade_in_seconds: None,
                source: AudioSpatialSource::fixed(Transform::from_xyz(4.0, 0.0, 0.0)),
                attenuation: AudioSpatialAttenuation::new(20.0, 1.0),
            }));
        app.update();

        let instance_id = *app
            .world()
            .resource::<AudioPlaybackState>()
            .instances
            .keys()
            .next()
            .unwrap();

        app.world_mut()
            .write_message(AudioCommand::StopByScope(AudioScopeFadeCommand {
                scope: scene_scope.clone(),
                fade_out_seconds: None,
            }));
        app.update();

        assert!(
            app.world()
                .resource::<AudioPlaybackState>()
                .instances
                .is_empty()
        );
        assert!(read_events(&app).iter().any(|event| matches!(
            event,
            AudioEvent::InstanceStopped(AudioInstanceStopped {
                instance_id: stopped,
                clip_id: Some(stopped_clip),
                cue_id: Some(stopped_cue),
                scope,
                bus: AudioBus::Sfx,
                reason: AudioStopReason::StoppedByScope,
            }) if stopped == &instance_id
                && stopped_clip == &clip_id
                && stopped_cue == &cue_id
                && scope == &scene_scope
        )));
    }

    #[test]
    fn stop_by_scope_can_force_clear_instance_already_fading_out() {
        let mut app = playback_app();
        let clip_id = clip_id("ambience.room");
        app.world_mut()
            .resource_mut::<AudioCatalog>()
            .register_clip(clip_id.clone(), "audio/ambience/room.ogg");
        app.world_mut()
            .write_message(AudioCommand::PlayClip(AudioClipRequest {
                clip_id,
                scope: AudioScope::scene("scene-1").unwrap(),
                bus: AudioBus::Sfx,
                volume: 0.5,
                pitch: 1.0,
                looped: true,
                fade_in_seconds: None,
            }));
        app.update();

        app.world_mut()
            .write_message(AudioCommand::StopByScope(AudioScopeFadeCommand {
                scope: AudioScope::scene("scene-1").unwrap(),
                fade_out_seconds: Some(0.5),
            }));
        app.update();
        assert_eq!(
            app.world()
                .resource::<AudioPlaybackState>()
                .instances
                .values()
                .filter(|instance| instance.stopping)
                .count(),
            1
        );

        app.world_mut()
            .write_message(AudioCommand::StopByScope(AudioScopeFadeCommand {
                scope: AudioScope::scene("scene-1").unwrap(),
                fade_out_seconds: None,
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
