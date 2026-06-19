use bevy::audio::{AudioSinkPlayback, SpatialAudioSink, SpatialListener};
use bevy::prelude::*;

use super::{
    event::{AudioEvent, AudioStopReason},
    mixer::{AudioMixer, calculate_sink_sync_target},
    playback::{AudioPlaybackInstance, AudioPlaybackState, stop_instance_immediately},
};

#[derive(Clone, Debug, PartialEq)]
pub enum AudioSpatialSource {
    Fixed(Transform),
    FollowEntity(Entity),
}

impl AudioSpatialSource {
    pub fn fixed(transform: Transform) -> Self {
        Self::Fixed(transform)
    }

    pub fn follow_entity(entity: Entity) -> Self {
        Self::FollowEntity(entity)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AudioSpatialAttenuation {
    pub max_distance: f32,
    pub rolloff_factor: f32,
}

impl AudioSpatialAttenuation {
    pub const fn new(max_distance: f32, rolloff_factor: f32) -> Self {
        Self {
            max_distance,
            rolloff_factor,
        }
    }

    pub fn normalized(self) -> Self {
        let max_distance = if self.max_distance.is_finite() {
            self.max_distance.max(0.0)
        } else {
            0.0
        };
        let rolloff_factor = if self.rolloff_factor.is_finite() {
            self.rolloff_factor.max(0.0)
        } else {
            0.0
        };

        Self {
            max_distance,
            rolloff_factor,
        }
    }
}

impl Default for AudioSpatialAttenuation {
    fn default() -> Self {
        Self {
            max_distance: 64.0,
            rolloff_factor: 1.0,
        }
    }
}

#[derive(Clone, Debug, Component, PartialEq)]
pub struct AudioSpatialEmitter {
    pub source: AudioSpatialSource,
    pub attenuation: AudioSpatialAttenuation,
}

#[derive(Clone, Copy, Debug, Resource, PartialEq)]
pub struct AudioSpatialListenerBinding {
    pub target: Entity,
    pub ear_gap: f32,
}

impl AudioSpatialListenerBinding {
    pub const fn new(target: Entity) -> Self {
        Self {
            target,
            ear_gap: 4.0,
        }
    }

    pub fn with_ear_gap(mut self, ear_gap: f32) -> Self {
        self.ear_gap = ear_gap.max(0.0);
        self
    }
}

#[derive(Clone, Copy, Debug, Resource, PartialEq, Eq)]
pub struct AudioSpatialListenerEntity(pub Entity);

#[derive(Clone, Copy, Debug, Component, Default, PartialEq, Eq)]
pub struct AudioSpatialListenerProxy;

/// Bevy 0.18.1 spatial audio uses simple stereo panning. This framework layer
/// stores max distance and rolloff so callers can express first-pass intent,
/// but it does not promise HRTF, reverb zones, occlusion, or advanced curves.
pub const BEVY_SPATIAL_AUDIO_LIMITS: &str =
    "Bevy 0.18.1 spatial audio is simple stereo panning; no HRTF, reverb, or occlusion.";

pub fn sync_spatial_listener_binding(
    mut commands: Commands,
    binding: Option<Res<AudioSpatialListenerBinding>>,
    listener_entity: Option<Res<AudioSpatialListenerEntity>>,
    targets: Query<(&Transform, Option<&GlobalTransform>)>,
) {
    let Some(binding) = binding else {
        return;
    };

    let Ok((target_transform, target_global_transform)) = targets.get(binding.target) else {
        if let Some(listener_entity) = listener_entity {
            commands.entity(listener_entity.0).try_despawn();
            commands.remove_resource::<AudioSpatialListenerEntity>();
        }
        return;
    };

    let listener_global_transform = target_global_transform
        .copied()
        .unwrap_or_else(|| GlobalTransform::from(*target_transform));
    let listener_transform = listener_global_transform.compute_transform();
    let listener = SpatialListener::new(binding.ear_gap.max(0.0));

    if let Some(listener_entity) = listener_entity {
        commands.entity(listener_entity.0).insert((
            listener_transform,
            listener_global_transform,
            listener,
            AudioSpatialListenerProxy,
        ));
    } else {
        let entity = commands
            .spawn((
                listener_transform,
                listener_global_transform,
                listener,
                AudioSpatialListenerProxy,
                Name::new("AudioSpatialListener"),
            ))
            .id();
        commands.insert_resource(AudioSpatialListenerEntity(entity));
    }
}

pub fn sync_spatial_emitters(
    mut commands: Commands,
    mut audio_events: MessageWriter<AudioEvent>,
    mut playback: ResMut<AudioPlaybackState>,
    mut queries: ParamSet<(
        Query<(&Transform, Option<&GlobalTransform>)>,
        Query<(
            Entity,
            &AudioPlaybackInstance,
            &AudioSpatialEmitter,
            &mut Transform,
            Option<&mut GlobalTransform>,
        )>,
    )>,
) {
    let source_updates = queries
        .p1()
        .iter()
        .map(|(entity, playback_instance, emitter, _, _)| {
            (
                entity,
                playback_instance.instance_id,
                emitter.source.clone(),
            )
        })
        .collect::<Vec<_>>();
    let mut transform_updates = Vec::new();
    let mut stale_instances = Vec::new();

    for (entity, instance_id, source) in source_updates {
        match source {
            AudioSpatialSource::Fixed(fixed_transform) => {
                transform_updates.push((
                    entity,
                    fixed_transform,
                    GlobalTransform::from(fixed_transform),
                ));
            }
            AudioSpatialSource::FollowEntity(target) => {
                let (target_transform, target_global_transform) = {
                    let targets = queries.p0();
                    let Ok((target_transform, target_global_transform)) = targets.get(target)
                    else {
                        stale_instances.push(instance_id);
                        continue;
                    };
                    let target_global_transform = target_global_transform
                        .copied()
                        .unwrap_or_else(|| GlobalTransform::from(*target_transform));
                    (
                        target_global_transform.compute_transform(),
                        target_global_transform,
                    )
                };
                transform_updates.push((entity, target_transform, target_global_transform));
            }
        }
    }

    for (entity, new_transform, new_global_transform) in transform_updates {
        if let Ok((_, _, _, mut transform, global_transform)) = queries.p1().get_mut(entity) {
            *transform = new_transform;
            if let Some(mut global_transform) = global_transform {
                *global_transform = new_global_transform;
            } else {
                commands.entity(entity).insert(new_global_transform);
            }
        }
    }

    for instance_id in stale_instances {
        stop_instance_immediately(
            instance_id,
            &mut commands,
            &mut audio_events,
            &mut playback,
            AudioStopReason::SourceEntityDespawned,
        );
    }
}

pub fn sync_spatial_audio_sinks_with_mixer(
    mixer: Res<AudioMixer>,
    playback: Res<AudioPlaybackState>,
    listeners: Query<&GlobalTransform, With<SpatialListener>>,
    mut sinks: Query<(
        &AudioPlaybackInstance,
        &GlobalTransform,
        &AudioSpatialEmitter,
        &mut SpatialAudioSink,
    )>,
) {
    let listener_position = listeners.iter().next().map(GlobalTransform::translation);

    for (playback_instance, emitter_transform, emitter, mut sink) in &mut sinks {
        let Some(instance) = playback.instances.get(&playback_instance.instance_id) else {
            continue;
        };

        let target = calculate_sink_sync_target(&mixer, instance);
        let attenuation = listener_position
            .map(|listener_position| {
                calculate_spatial_attenuation(
                    listener_position.distance(emitter_transform.translation()),
                    emitter.attenuation,
                )
            })
            .unwrap_or(1.0);

        sink.set_volume(bevy::audio::Volume::Linear(target.volume * attenuation));

        if target.paused {
            sink.pause();
        } else {
            sink.play();
        }
    }
}

pub fn calculate_spatial_attenuation(distance: f32, attenuation: AudioSpatialAttenuation) -> f32 {
    let attenuation = attenuation.normalized();
    if attenuation.max_distance <= 0.0 || attenuation.rolloff_factor <= 0.0 {
        return 1.0;
    }

    let distance = if distance.is_finite() {
        distance.max(0.0)
    } else {
        attenuation.max_distance
    };

    if distance >= attenuation.max_distance {
        return 0.0;
    }

    let normalized_distance = distance / attenuation.max_distance;
    (1.0 - normalized_distance).powf(attenuation.rolloff_factor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::audio::{
        AudioBus, AudioClipId, AudioCueId, AudioInstanceId, AudioInstanceState,
        AudioInstanceStopped, AudioPlaybackState, AudioScope,
    };
    use bevy::ecs::message::MessageCursor;

    fn read_events(app: &App) -> Vec<AudioEvent> {
        let messages = app.world().resource::<Messages<AudioEvent>>();
        let mut cursor = MessageCursor::default();
        cursor.read(messages).cloned().collect()
    }

    #[test]
    fn attenuation_is_clamped_to_supported_range() {
        assert_eq!(
            calculate_spatial_attenuation(0.0, AudioSpatialAttenuation::new(10.0, 1.0)),
            1.0
        );
        assert_eq!(
            calculate_spatial_attenuation(5.0, AudioSpatialAttenuation::new(10.0, 1.0)),
            0.5
        );
        assert_eq!(
            calculate_spatial_attenuation(10.0, AudioSpatialAttenuation::new(10.0, 1.0)),
            0.0
        );
        assert_eq!(
            calculate_spatial_attenuation(5.0, AudioSpatialAttenuation::new(0.0, 1.0)),
            1.0
        );
    }

    #[test]
    fn listener_binding_spawns_and_syncs_proxy_transform() {
        let mut app = App::new();
        app.add_message::<AudioEvent>()
            .add_systems(Update, sync_spatial_listener_binding);
        let target_world_position = Vec3::new(30.0, 40.0, 50.0);
        let target = app
            .world_mut()
            .spawn((
                Transform::from_xyz(3.0, 4.0, 5.0),
                GlobalTransform::from_translation(target_world_position),
            ))
            .id();
        app.insert_resource(AudioSpatialListenerBinding::new(target).with_ear_gap(8.0));

        app.update();

        let listener_entity = app.world().resource::<AudioSpatialListenerEntity>().0;
        let listener = app.world().entity(listener_entity);
        assert!(listener.get::<AudioSpatialListenerProxy>().is_some());
        assert_eq!(
            listener.get::<Transform>().unwrap().translation,
            target_world_position
        );
        assert_eq!(
            listener.get::<GlobalTransform>().unwrap().translation(),
            target_world_position
        );
        assert_eq!(
            listener.get::<SpatialListener>().unwrap().left_ear_offset,
            Vec3::X * -4.0
        );

        let updated_world_position = Vec3::new(-10.0, 20.0, 0.5);
        app.world_mut().entity_mut(target).insert((
            Transform::from_xyz(-1.0, 2.0, 0.0),
            GlobalTransform::from_translation(updated_world_position),
        ));
        app.update();

        assert_eq!(
            app.world()
                .entity(listener_entity)
                .get::<Transform>()
                .unwrap()
                .translation,
            updated_world_position
        );
        assert_eq!(
            app.world()
                .entity(listener_entity)
                .get::<GlobalTransform>()
                .unwrap()
                .translation(),
            updated_world_position
        );
    }

    #[test]
    fn emitter_sync_updates_follow_entity_transform_and_keeps_fixed_transform() {
        let mut app = App::new();
        app.add_message::<AudioEvent>()
            .init_resource::<AudioPlaybackState>()
            .add_systems(Update, sync_spatial_emitters);

        let target_world_position = Vec3::new(80.0, 90.0, 1.0);
        let target = app
            .world_mut()
            .spawn((
                Transform::from_xyz(8.0, 9.0, 0.0),
                GlobalTransform::from_translation(target_world_position),
            ))
            .id();
        let followed = app
            .world_mut()
            .spawn((
                AudioPlaybackInstance {
                    instance_id: AudioInstanceId::from_raw(1),
                },
                AudioSpatialEmitter {
                    source: AudioSpatialSource::follow_entity(target),
                    attenuation: AudioSpatialAttenuation::default(),
                },
                Transform::default(),
                GlobalTransform::default(),
            ))
            .id();
        let fixed_transform = Transform::from_xyz(1.0, 2.0, 3.0);
        let fixed = app
            .world_mut()
            .spawn((
                AudioPlaybackInstance {
                    instance_id: AudioInstanceId::from_raw(2),
                },
                AudioSpatialEmitter {
                    source: AudioSpatialSource::fixed(fixed_transform),
                    attenuation: AudioSpatialAttenuation::default(),
                },
                Transform::default(),
                GlobalTransform::default(),
            ))
            .id();

        app.update();

        assert_eq!(
            app.world()
                .entity(followed)
                .get::<Transform>()
                .unwrap()
                .translation,
            target_world_position
        );
        assert_eq!(
            app.world()
                .entity(followed)
                .get::<GlobalTransform>()
                .unwrap()
                .translation(),
            target_world_position
        );
        assert_eq!(
            *app.world().entity(fixed).get::<Transform>().unwrap(),
            fixed_transform
        );
        assert_eq!(
            app.world()
                .entity(fixed)
                .get::<GlobalTransform>()
                .unwrap()
                .translation(),
            fixed_transform.translation
        );
    }

    #[test]
    fn follow_entity_despawn_stops_tracked_instance() {
        let mut app = App::new();
        app.add_message::<AudioEvent>()
            .init_resource::<AudioPlaybackState>()
            .add_systems(Update, sync_spatial_emitters);

        let target = app.world_mut().spawn(Transform::default()).id();
        let instance_id = AudioInstanceId::from_raw(42);
        let emitter_entity = app
            .world_mut()
            .spawn((
                AudioPlaybackInstance { instance_id },
                AudioSpatialEmitter {
                    source: AudioSpatialSource::follow_entity(target),
                    attenuation: AudioSpatialAttenuation::default(),
                },
                Transform::default(),
            ))
            .id();
        app.world_mut()
            .resource_mut::<AudioPlaybackState>()
            .instances
            .insert(
                instance_id,
                AudioInstanceState {
                    entity: emitter_entity,
                    clip_id: AudioClipId::try_from("ambience.torch").unwrap(),
                    cue_id: Some(AudioCueId::try_from("scene.torch").unwrap()),
                    scope: AudioScope::Entity(target),
                    bus: AudioBus::Sfx,
                    volume: 1.0,
                    priority: 0,
                    looped: false,
                    asset_path: "audio/ambience/torch.ogg".to_string(),
                    source: Handle::<AudioSource>::default(),
                    failed: false,
                    paused: false,
                    stopping: false,
                    fade: None,
                    spatial: true,
                    start_seconds: 0.0,
                    position_seconds: 0.0,
                    pending_seek_seconds: None,
                },
            );

        app.world_mut().entity_mut(target).despawn();
        app.update();

        assert!(
            app.world()
                .resource::<AudioPlaybackState>()
                .instances
                .is_empty()
        );
        assert!(app.world().get_entity(emitter_entity).is_err());
        assert!(read_events(&app).iter().any(|event| matches!(
            event,
            AudioEvent::InstanceStopped(AudioInstanceStopped {
                instance_id: stopped,
                reason: AudioStopReason::SourceEntityDespawned,
                ..
            }) if stopped == &instance_id
        )));
    }

    #[test]
    fn limits_text_documents_bevy_spatial_audio_boundary() {
        assert!(BEVY_SPATIAL_AUDIO_LIMITS.contains("no HRTF"));
        assert!(BEVY_SPATIAL_AUDIO_LIMITS.contains("reverb"));
    }
}
