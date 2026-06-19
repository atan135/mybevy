use std::collections::{HashMap, VecDeque};

use bevy::prelude::*;

use super::{
    event::{AudioCueSkipped, AudioCueStarted, AudioEvent, AudioLoadFailed},
    id::{AudioClipId, AudioCueId, AudioGroupId, AudioInstanceId},
    loading::AudioLoadingState,
    playback::AudioPlaybackState,
    scope::{AudioBus, AudioScope},
};

const ENV_AUDIO_DEBUG: &str = "MYBEVY_AUDIO_DEBUG";
pub const DEFAULT_AUDIO_DEBUG_RECENT_LIMIT: usize = 32;

#[derive(Clone, Debug, Resource, PartialEq)]
pub struct AudioDebugConfig {
    pub enabled: bool,
}

impl Default for AudioDebugConfig {
    fn default() -> Self {
        Self { enabled: false }
    }
}

impl AudioDebugConfig {
    pub fn from_env() -> Self {
        Self::from_env_reader(|key| std::env::var(key).ok())
    }

    pub fn from_env_reader(mut read: impl FnMut(&str) -> Option<String>) -> Self {
        Self {
            enabled: read_bool(&mut read, ENV_AUDIO_DEBUG).unwrap_or(false),
        }
    }

    pub fn is_active(&self) -> bool {
        self.enabled
    }
}

#[derive(Clone, Debug, Resource, PartialEq, Eq)]
pub struct AudioDebugState {
    recent_limit: usize,
    recent_started_cues: VecDeque<AudioDebugCueStarted>,
    recent_skipped_cues: VecDeque<AudioDebugCueSkipped>,
    recent_load_failures: VecDeque<AudioDebugLoadFailure>,
}

impl Default for AudioDebugState {
    fn default() -> Self {
        Self::with_recent_limit(DEFAULT_AUDIO_DEBUG_RECENT_LIMIT)
    }
}

impl AudioDebugState {
    pub fn with_recent_limit(recent_limit: usize) -> Self {
        Self {
            recent_limit,
            recent_started_cues: VecDeque::with_capacity(recent_limit),
            recent_skipped_cues: VecDeque::with_capacity(recent_limit),
            recent_load_failures: VecDeque::with_capacity(recent_limit),
        }
    }

    pub fn recent_limit(&self) -> usize {
        self.recent_limit
    }

    pub fn clear(&mut self) {
        self.recent_started_cues.clear();
        self.recent_skipped_cues.clear();
        self.recent_load_failures.clear();
    }

    pub fn record_event(&mut self, event: &AudioEvent) {
        match event {
            AudioEvent::CueStarted(started) => self.record_cue_started(started),
            AudioEvent::CueSkipped(skipped) => self.record_cue_skipped(skipped),
            AudioEvent::LoadFailed(failure) => self.record_load_failed(failure),
            _ => {}
        }
    }

    pub fn record_cue_started(&mut self, started: &AudioCueStarted) {
        push_recent(
            &mut self.recent_started_cues,
            self.recent_limit,
            AudioDebugCueStarted::from(started),
        );
    }

    pub fn record_cue_skipped(&mut self, skipped: &AudioCueSkipped) {
        push_recent(
            &mut self.recent_skipped_cues,
            self.recent_limit,
            AudioDebugCueSkipped::from(skipped),
        );
    }

    pub fn record_load_failed(&mut self, failure: &AudioLoadFailed) {
        push_recent(
            &mut self.recent_load_failures,
            self.recent_limit,
            AudioDebugLoadFailure::from(failure),
        );
    }

    pub fn recent_started_cues(&self) -> Vec<AudioDebugCueStarted> {
        self.recent_started_cues.iter().cloned().collect()
    }

    pub fn recent_skipped_cues(&self) -> Vec<AudioDebugCueSkipped> {
        self.recent_skipped_cues.iter().cloned().collect()
    }

    pub fn recent_load_failures(&self) -> Vec<AudioDebugLoadFailure> {
        self.recent_load_failures.iter().cloned().collect()
    }
}

#[derive(Clone, Debug, Default, Resource, PartialEq)]
pub struct AudioDebugSnapshot {
    pub enabled: bool,
    pub active_instances: AudioDebugActiveInstanceCounts,
    pub instance_details: Vec<AudioDebugInstanceInfo>,
    pub loading_groups: Vec<AudioDebugLoadingGroupInfo>,
    pub recent_started_cues: Vec<AudioDebugCueStarted>,
    pub recent_skipped_cues: Vec<AudioDebugCueSkipped>,
    pub recent_load_failures: Vec<AudioDebugLoadFailure>,
}

pub type AudioDebugDiagnostics = AudioDebugSnapshot;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AudioDebugActiveInstanceCounts {
    pub total: usize,
    pub by_bus: Vec<AudioDebugBusInstanceCount>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioDebugBusInstanceCount {
    pub bus: AudioBus,
    pub count: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioDebugInstanceInfo {
    pub instance_id: AudioInstanceId,
    pub clip_id: AudioClipId,
    pub cue_id: Option<AudioCueId>,
    pub scope: AudioScope,
    pub bus: AudioBus,
    pub asset_path: String,
    pub paused: bool,
    pub stopping: bool,
    pub failed: bool,
    pub spatial: bool,
    pub looped: bool,
    pub start_seconds: f32,
    pub position_seconds: f32,
    pub pending_seek_seconds: Option<f32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioDebugLoadingGroupInfo {
    pub group_id: AudioGroupId,
    pub loaded: usize,
    pub total: usize,
    pub failed: usize,
    pub required_loaded: usize,
    pub required_total: usize,
    pub required_failed: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioDebugCueStarted {
    pub cue_id: AudioCueId,
    pub clip_id: AudioClipId,
    pub instance_id: AudioInstanceId,
    pub scope: AudioScope,
    pub bus: AudioBus,
}

impl From<&AudioCueStarted> for AudioDebugCueStarted {
    fn from(started: &AudioCueStarted) -> Self {
        Self {
            cue_id: started.cue_id.clone(),
            clip_id: started.clip_id.clone(),
            instance_id: started.instance_id,
            scope: started.scope.clone(),
            bus: started.bus,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioDebugCueSkipped {
    pub cue_id: AudioCueId,
    pub reason: super::event::AudioCueSkipReason,
    pub scope: AudioScope,
}

impl From<&AudioCueSkipped> for AudioDebugCueSkipped {
    fn from(skipped: &AudioCueSkipped) -> Self {
        Self {
            cue_id: skipped.cue_id.clone(),
            reason: skipped.reason,
            scope: skipped.scope.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioDebugLoadFailure {
    pub clip_id: Option<AudioClipId>,
    pub cue_id: Option<AudioCueId>,
    pub group_id: Option<AudioGroupId>,
    pub asset_path: Option<String>,
    pub message: String,
}

impl From<&AudioLoadFailed> for AudioDebugLoadFailure {
    fn from(failure: &AudioLoadFailed) -> Self {
        Self {
            clip_id: failure.clip_id.clone(),
            cue_id: failure.cue_id.clone(),
            group_id: failure.group_id.clone(),
            asset_path: failure.asset_path.clone(),
            message: failure.message.clone(),
        }
    }
}

pub fn update_audio_debug_snapshot(
    config: Res<AudioDebugConfig>,
    mut state: ResMut<AudioDebugState>,
    mut snapshot: ResMut<AudioDebugSnapshot>,
    mut audio_events: MessageReader<AudioEvent>,
    playback: Res<AudioPlaybackState>,
    loading: Res<AudioLoadingState>,
) {
    if !config.is_active() {
        for _ in audio_events.read() {}
        state.clear();
        *snapshot = AudioDebugSnapshot::default();
        return;
    }

    for event in audio_events.read() {
        state.record_event(event);
    }

    *snapshot = audio_debug_snapshot(&config, &state, &playback, &loading);
}

pub fn audio_debug_snapshot(
    config: &AudioDebugConfig,
    state: &AudioDebugState,
    playback: &AudioPlaybackState,
    loading: &AudioLoadingState,
) -> AudioDebugSnapshot {
    AudioDebugSnapshot {
        enabled: config.is_active(),
        active_instances: active_audio_instance_counts(playback),
        instance_details: audio_debug_instance_info(playback),
        loading_groups: audio_debug_loading_group_info(loading),
        recent_started_cues: state.recent_started_cues(),
        recent_skipped_cues: state.recent_skipped_cues(),
        recent_load_failures: state.recent_load_failures(),
    }
}

pub fn active_audio_instance_counts(
    playback: &AudioPlaybackState,
) -> AudioDebugActiveInstanceCounts {
    let mut counts = HashMap::<AudioBus, usize>::new();
    for instance in playback.instances.values() {
        *counts.entry(instance.bus).or_default() += 1;
    }

    let mut by_bus = counts
        .into_iter()
        .map(|(bus, count)| AudioDebugBusInstanceCount { bus, count })
        .collect::<Vec<_>>();
    by_bus.sort_by_key(|entry| audio_bus_sort_key(entry.bus));

    AudioDebugActiveInstanceCounts {
        total: playback.instances.len(),
        by_bus,
    }
}

pub fn audio_debug_instance_info(playback: &AudioPlaybackState) -> Vec<AudioDebugInstanceInfo> {
    let mut instances = playback
        .instances
        .iter()
        .map(|(instance_id, instance)| AudioDebugInstanceInfo {
            instance_id: *instance_id,
            clip_id: instance.clip_id.clone(),
            cue_id: instance.cue_id.clone(),
            scope: instance.scope.clone(),
            bus: instance.bus,
            asset_path: instance.asset_path.clone(),
            paused: instance.paused,
            stopping: instance.stopping,
            failed: instance.failed,
            spatial: instance.spatial,
            looped: instance.looped,
            start_seconds: instance.start_seconds,
            position_seconds: instance.position_seconds,
            pending_seek_seconds: instance.pending_seek_seconds,
        })
        .collect::<Vec<_>>();
    instances.sort_by_key(|instance| instance.instance_id.raw());
    instances
}

pub fn audio_debug_loading_group_info(
    loading: &AudioLoadingState,
) -> Vec<AudioDebugLoadingGroupInfo> {
    let mut groups = loading
        .groups
        .values()
        .map(|group| {
            let progress = group.progress();
            AudioDebugLoadingGroupInfo {
                group_id: progress.group_id,
                loaded: progress.loaded,
                total: progress.total,
                failed: progress.failed,
                required_loaded: progress.required_loaded,
                required_total: progress.required_total,
                required_failed: progress.required_failed,
            }
        })
        .collect::<Vec<_>>();
    groups.sort_by(|left, right| left.group_id.cmp(&right.group_id));
    groups
}

fn push_recent<T>(items: &mut VecDeque<T>, limit: usize, item: T) {
    if limit == 0 {
        items.clear();
        return;
    }

    while items.len() >= limit {
        items.pop_front();
    }
    items.push_back(item);
}

fn audio_bus_sort_key(bus: AudioBus) -> u8 {
    match bus {
        AudioBus::Master => 0,
        AudioBus::Music => 1,
        AudioBus::Sfx => 2,
        AudioBus::Ui => 3,
        AudioBus::Battle => 4,
    }
}

fn read_bool(read: &mut impl FnMut(&str) -> Option<String>, key: &str) -> Option<bool> {
    read(key).and_then(|value| parse_bool(value.as_str()))
}

fn parse_bool(value: &str) -> Option<bool> {
    match normalized_env_value(value).as_str() {
        "1" | "true" | "on" | "yes" | "enabled" => Some(true),
        "0" | "false" | "off" | "no" | "disabled" => Some(false),
        _ => None,
    }
}

fn normalized_env_value(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::audio::{
        AudioCatalog, AudioCommand, AudioGroupClip, AudioGroupCommand, AudioGroupEntry,
    };
    use bevy::audio::AudioSource;
    use bevy::ecs::message::MessageCursor;

    fn env_reader<'a>(values: &'a [(&'a str, &'a str)]) -> impl FnMut(&str) -> Option<String> + 'a {
        |key| {
            values
                .iter()
                .find_map(|(name, value)| (*name == key).then_some((*value).to_string()))
        }
    }

    fn clip_id(value: &str) -> AudioClipId {
        AudioClipId::try_from(value).unwrap()
    }

    fn cue_id(value: &str) -> AudioCueId {
        AudioCueId::try_from(value).unwrap()
    }

    fn group_id(value: &str) -> AudioGroupId {
        AudioGroupId::try_from(value).unwrap()
    }

    fn debug_loading_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .add_message::<AudioCommand>()
            .add_message::<AudioEvent>()
            .init_asset::<AudioSource>()
            .init_resource::<AudioCatalog>()
            .init_resource::<AudioLoadingState>()
            .add_systems(Update, super::super::loading::handle_audio_loading_commands);
        app
    }

    #[test]
    fn debug_config_defaults_to_disabled_without_env() {
        let config = AudioDebugConfig::from_env_reader(env_reader(&[]));

        assert_eq!(config, AudioDebugConfig::default());
        assert!(!config.is_active());
    }

    #[test]
    fn debug_config_reads_true_env_values() {
        for value in ["on", "true", "1", "yes", "enabled"] {
            let config = AudioDebugConfig::from_env_reader(env_reader(&[(ENV_AUDIO_DEBUG, value)]));

            assert!(config.enabled, "{value} should enable audio debug");
            assert!(config.is_active());
        }
    }

    #[test]
    fn debug_config_reads_false_env_values() {
        for value in ["off", "false", "0", "no", "disabled"] {
            let config = AudioDebugConfig::from_env_reader(env_reader(&[(ENV_AUDIO_DEBUG, value)]));

            assert!(!config.enabled, "{value} should disable audio debug");
            assert!(!config.is_active());
        }
    }

    #[test]
    fn debug_config_ignores_unknown_env_values() {
        let config = AudioDebugConfig::from_env_reader(env_reader(&[(ENV_AUDIO_DEBUG, "maybe")]));

        assert_eq!(config, AudioDebugConfig::default());
    }

    #[test]
    fn debug_state_records_recent_cue_and_load_failure_events() {
        let mut state = AudioDebugState::with_recent_limit(2);
        let first_cue = cue_id("ui.first");
        let second_cue = cue_id("ui.second");
        let third_cue = cue_id("ui.third");
        let clip = clip_id("ui.click");

        state.record_event(&AudioEvent::CueStarted(AudioCueStarted {
            cue_id: first_cue,
            clip_id: clip.clone(),
            instance_id: AudioInstanceId::from_raw(1),
            scope: AudioScope::Ui,
            bus: AudioBus::Ui,
        }));
        state.record_event(&AudioEvent::CueStarted(AudioCueStarted {
            cue_id: second_cue.clone(),
            clip_id: clip.clone(),
            instance_id: AudioInstanceId::from_raw(2),
            scope: AudioScope::Ui,
            bus: AudioBus::Ui,
        }));
        state.record_event(&AudioEvent::CueStarted(AudioCueStarted {
            cue_id: third_cue.clone(),
            clip_id: clip.clone(),
            instance_id: AudioInstanceId::from_raw(3),
            scope: AudioScope::Ui,
            bus: AudioBus::Ui,
        }));
        state.record_event(&AudioEvent::CueSkipped(AudioCueSkipped {
            cue_id: third_cue.clone(),
            reason: super::super::event::AudioCueSkipReason::Cooldown,
            scope: AudioScope::Ui,
        }));
        state.record_event(&AudioEvent::LoadFailed(AudioLoadFailed {
            clip_id: Some(clip.clone()),
            cue_id: Some(third_cue.clone()),
            group_id: Some(group_id("boot")),
            asset_path: Some("audio/ui/missing.ogg".to_string()),
            message: "missing asset".to_string(),
        }));

        assert_eq!(
            state.recent_started_cues(),
            vec![
                AudioDebugCueStarted {
                    cue_id: second_cue,
                    clip_id: clip.clone(),
                    instance_id: AudioInstanceId::from_raw(2),
                    scope: AudioScope::Ui,
                    bus: AudioBus::Ui,
                },
                AudioDebugCueStarted {
                    cue_id: third_cue.clone(),
                    clip_id: clip.clone(),
                    instance_id: AudioInstanceId::from_raw(3),
                    scope: AudioScope::Ui,
                    bus: AudioBus::Ui,
                },
            ]
        );
        assert_eq!(
            state.recent_skipped_cues(),
            vec![AudioDebugCueSkipped {
                cue_id: third_cue.clone(),
                reason: super::super::event::AudioCueSkipReason::Cooldown,
                scope: AudioScope::Ui,
            }]
        );
        assert_eq!(
            state.recent_load_failures(),
            vec![AudioDebugLoadFailure {
                clip_id: Some(clip),
                cue_id: Some(third_cue),
                group_id: Some(group_id("boot")),
                asset_path: Some("audio/ui/missing.ogg".to_string()),
                message: "missing asset".to_string(),
            }]
        );
    }

    #[test]
    fn debug_snapshot_counts_active_instances_by_bus() {
        let mut playback = AudioPlaybackState::default();
        insert_instance(
            &mut playback,
            AudioInstanceId::from_raw(3),
            clip_id("ui.click"),
            Some(cue_id("ui.click")),
            AudioBus::Ui,
        );
        insert_instance(
            &mut playback,
            AudioInstanceId::from_raw(1),
            clip_id("battle.hit"),
            Some(cue_id("battle.hit")),
            AudioBus::Battle,
        );
        insert_instance(
            &mut playback,
            AudioInstanceId::from_raw(2),
            clip_id("ui.confirm"),
            None,
            AudioBus::Ui,
        );

        let snapshot = audio_debug_snapshot(
            &AudioDebugConfig { enabled: true },
            &AudioDebugState::default(),
            &playback,
            &AudioLoadingState::default(),
        );

        assert!(snapshot.enabled);
        assert_eq!(
            snapshot.active_instances,
            AudioDebugActiveInstanceCounts {
                total: 3,
                by_bus: vec![
                    AudioDebugBusInstanceCount {
                        bus: AudioBus::Ui,
                        count: 2,
                    },
                    AudioDebugBusInstanceCount {
                        bus: AudioBus::Battle,
                        count: 1,
                    },
                ],
            }
        );
        assert_eq!(
            snapshot
                .instance_details
                .iter()
                .map(|instance| instance.instance_id.raw())
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
    }

    #[test]
    fn debug_snapshot_reports_loading_group_progress() {
        let mut app = debug_loading_app();
        let group_id = group_id("boot");
        let click = clip_id("ui.click");
        let optional = clip_id("ui.optional");
        app.world_mut()
            .resource_mut::<AudioCatalog>()
            .register_clip(click.clone(), "audio/ui/click.ogg");
        app.world_mut()
            .resource_mut::<AudioCatalog>()
            .register_clip(optional.clone(), "audio/ui/optional.ogg");
        app.world_mut()
            .resource_mut::<AudioCatalog>()
            .register_group(
                group_id.clone(),
                AudioGroupEntry::from_clips([
                    AudioGroupClip::required(click),
                    AudioGroupClip::optional(optional),
                ]),
            );
        app.world_mut()
            .write_message(AudioCommand::PreloadGroup(AudioGroupCommand::new(
                group_id.clone(),
            )));
        app.update();

        let snapshot = audio_debug_snapshot(
            &AudioDebugConfig { enabled: true },
            &AudioDebugState::default(),
            &AudioPlaybackState::default(),
            app.world().resource::<AudioLoadingState>(),
        );

        assert_eq!(
            snapshot.loading_groups,
            vec![AudioDebugLoadingGroupInfo {
                group_id,
                loaded: 0,
                total: 2,
                failed: 0,
                required_loaded: 0,
                required_total: 1,
                required_failed: 0,
            }]
        );
    }

    #[test]
    fn debug_system_records_events_into_snapshot_when_enabled() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<AudioEvent>()
            .insert_resource(AudioDebugConfig { enabled: true })
            .init_resource::<AudioDebugState>()
            .init_resource::<AudioDebugSnapshot>()
            .init_resource::<AudioPlaybackState>()
            .init_resource::<AudioLoadingState>()
            .add_systems(Update, update_audio_debug_snapshot);

        let cue = cue_id("ui.click");
        let clip = clip_id("ui.click");
        app.world_mut()
            .write_message(AudioEvent::CueStarted(AudioCueStarted {
                cue_id: cue.clone(),
                clip_id: clip.clone(),
                instance_id: AudioInstanceId::from_raw(9),
                scope: AudioScope::Ui,
                bus: AudioBus::Ui,
            }));

        app.update();

        assert_eq!(
            app.world()
                .resource::<AudioDebugSnapshot>()
                .recent_started_cues,
            vec![AudioDebugCueStarted {
                cue_id: cue,
                clip_id: clip,
                instance_id: AudioInstanceId::from_raw(9),
                scope: AudioScope::Ui,
                bus: AudioBus::Ui,
            }]
        );
    }

    #[test]
    fn debug_system_consumes_events_without_recording_when_disabled() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<AudioEvent>()
            .insert_resource(AudioDebugConfig { enabled: false })
            .init_resource::<AudioDebugState>()
            .init_resource::<AudioDebugSnapshot>()
            .init_resource::<AudioPlaybackState>()
            .init_resource::<AudioLoadingState>()
            .add_systems(Update, update_audio_debug_snapshot);

        app.world_mut()
            .write_message(AudioEvent::CueSkipped(AudioCueSkipped {
                cue_id: cue_id("ui.click"),
                reason: super::super::event::AudioCueSkipReason::Cooldown,
                scope: AudioScope::Ui,
            }));
        app.update();

        assert_eq!(
            *app.world().resource::<AudioDebugSnapshot>(),
            AudioDebugSnapshot::default()
        );
        let messages = app.world().resource::<Messages<AudioEvent>>();
        let mut cursor = MessageCursor::default();
        assert_eq!(cursor.read(messages).count(), 1);
    }

    fn insert_instance(
        playback: &mut AudioPlaybackState,
        instance_id: AudioInstanceId,
        clip_id: AudioClipId,
        cue_id: Option<AudioCueId>,
        bus: AudioBus,
    ) {
        playback.instances.insert(
            instance_id,
            super::super::playback::AudioInstanceState {
                entity: Entity::from_raw_u32(instance_id.raw() as u32).unwrap(),
                clip_id,
                cue_id,
                scope: AudioScope::Ui,
                bus,
                volume: 1.0,
                priority: 0,
                looped: false,
                asset_path: "audio/test.ogg".to_string(),
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
    }
}
