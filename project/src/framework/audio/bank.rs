use std::{
    collections::{BTreeSet, HashMap},
    time::Duration,
};

use bevy::prelude::*;

use super::{
    catalog::AudioCatalog,
    command::{AudioCommand, AudioGroupCommand},
    event::{AudioEvent, AudioMusicChanged},
    id::{AudioClipId, AudioCueId, AudioGroupId, AudioInstanceId},
    loading::AudioLoadingState,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioBankGroupConfig {
    pub group_id: AudioGroupId,
    pub lazy_unload: Duration,
}

impl AudioBankGroupConfig {
    pub fn new(group_id: AudioGroupId, lazy_unload: Duration) -> Self {
        Self {
            group_id,
            lazy_unload,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AudioBankLoadStatus {
    #[default]
    NotLoaded,
    Loading,
    Loaded,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioBankGroupState {
    pub group_id: AudioGroupId,
    pub lazy_unload: Duration,
    pub load_status: AudioBankLoadStatus,
    pub preload_requested: bool,
    pub active_instance_ids: BTreeSet<AudioInstanceId>,
    pub idle_countdown_seconds: Option<f32>,
}

impl AudioBankGroupState {
    fn new(config: AudioBankGroupConfig) -> Self {
        Self {
            group_id: config.group_id,
            lazy_unload: config.lazy_unload,
            load_status: AudioBankLoadStatus::NotLoaded,
            preload_requested: false,
            active_instance_ids: BTreeSet::new(),
            idle_countdown_seconds: None,
        }
    }

    pub fn resident(&self) -> bool {
        self.lazy_unload == Duration::ZERO
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioBankMappingConflict {
    pub item: AudioBankMappingConflictItem,
    pub kept_group_id: AudioGroupId,
    pub ignored_group_id: AudioGroupId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AudioBankMappingConflictItem {
    Clip(AudioClipId),
    Cue(AudioCueId),
}

#[derive(Debug, Default, Resource)]
pub struct AudioBankRuntime {
    pub groups: HashMap<AudioGroupId, AudioBankGroupState>,
    group_order: Vec<AudioGroupId>,
    clip_to_group: HashMap<AudioClipId, AudioGroupId>,
    cue_to_group: HashMap<AudioCueId, AudioGroupId>,
    instance_to_group: HashMap<AudioInstanceId, AudioGroupId>,
    mapping_conflicts: Vec<AudioBankMappingConflict>,
    mappings_dirty: bool,
}

impl AudioBankRuntime {
    pub fn register_group_config(&mut self, config: AudioBankGroupConfig) {
        let group_id = config.group_id.clone();
        if !self.groups.contains_key(&group_id) {
            self.group_order.push(group_id.clone());
        }

        self.groups
            .entry(group_id)
            .and_modify(|state| {
                state.lazy_unload = config.lazy_unload;
                if state.resident() {
                    state.idle_countdown_seconds = None;
                }
            })
            .or_insert_with(|| AudioBankGroupState::new(config));
        self.mappings_dirty = true;
    }

    pub fn rebuild_mappings(&mut self, catalog: &AudioCatalog) {
        self.clip_to_group.clear();
        self.cue_to_group.clear();
        self.mapping_conflicts.clear();

        for group_id in &self.group_order {
            let Ok(group) = catalog.group(group_id) else {
                continue;
            };

            for clip in &group.clips {
                if let Some(kept_group_id) = self.clip_to_group.get(&clip.clip_id) {
                    if kept_group_id != group_id {
                        self.mapping_conflicts.push(AudioBankMappingConflict {
                            item: AudioBankMappingConflictItem::Clip(clip.clip_id.clone()),
                            kept_group_id: kept_group_id.clone(),
                            ignored_group_id: group_id.clone(),
                        });
                    }
                    continue;
                }

                self.clip_to_group
                    .insert(clip.clip_id.clone(), group_id.clone());
            }
        }

        for (cue_id, cue) in catalog.cues() {
            let mut mapped_group_id = None::<AudioGroupId>;
            for cue_clip in &cue.clips {
                let Some(group_id) = self.clip_to_group.get(&cue_clip.clip_id) else {
                    continue;
                };

                match &mapped_group_id {
                    Some(kept_group_id) if kept_group_id != group_id => {
                        self.mapping_conflicts.push(AudioBankMappingConflict {
                            item: AudioBankMappingConflictItem::Cue(cue_id.clone()),
                            kept_group_id: kept_group_id.clone(),
                            ignored_group_id: group_id.clone(),
                        });
                        break;
                    }
                    Some(_) => {}
                    None => {
                        mapped_group_id = Some(group_id.clone());
                    }
                }
            }

            if let Some(group_id) = mapped_group_id {
                self.cue_to_group.insert(cue_id.clone(), group_id);
            }
        }

        self.mappings_dirty = false;
    }

    pub fn rebuild_mappings_if_needed(&mut self, catalog: &AudioCatalog, catalog_changed: bool) {
        if self.mappings_dirty || catalog_changed {
            self.rebuild_mappings(catalog);
        }
    }

    pub fn clip_group(&self, clip_id: &AudioClipId) -> Option<&AudioGroupId> {
        self.clip_to_group.get(clip_id)
    }

    pub fn cue_group(&self, cue_id: &AudioCueId) -> Option<&AudioGroupId> {
        self.cue_to_group.get(cue_id)
    }

    pub fn mapping_conflicts(&self) -> &[AudioBankMappingConflict] {
        &self.mapping_conflicts
    }

    pub fn group_for_command(&self, command: &AudioCommand) -> Option<AudioGroupId> {
        match command {
            AudioCommand::PlayCue(request) => self.cue_group(&request.cue_id).cloned(),
            AudioCommand::PlayBattleCue(request) => self.cue_group(&request.cue_id).cloned(),
            AudioCommand::PlaySpatialCue(request) => self.cue_group(&request.cue_id).cloned(),
            AudioCommand::PlayClip(request) => self.clip_group(&request.clip_id).cloned(),
            AudioCommand::PlayMusic(request) => self.clip_group(&request.clip_id).cloned(),
            AudioCommand::CrossfadeMusic(request) => self.clip_group(&request.clip_id).cloned(),
            _ => None,
        }
    }

    fn ensure_group_loading(
        &mut self,
        group_id: &AudioGroupId,
        audio_commands: &mut MessageWriter<AudioCommand>,
    ) {
        let Some(state) = self.groups.get_mut(group_id) else {
            return;
        };

        state.idle_countdown_seconds = None;
        let should_request =
            !state.preload_requested || matches!(state.load_status, AudioBankLoadStatus::NotLoaded);
        if !should_request {
            return;
        }

        state.preload_requested = true;
        state.load_status = AudioBankLoadStatus::Loading;
        audio_commands.write(AudioCommand::PreloadGroup(AudioGroupCommand::new(
            group_id.clone(),
        )));
    }

    fn mark_group_preload_requested(&mut self, group_id: &AudioGroupId) {
        let Some(state) = self.groups.get_mut(group_id) else {
            return;
        };

        state.preload_requested = true;
        state.load_status = AudioBankLoadStatus::Loading;
        state.idle_countdown_seconds = None;
    }

    fn mark_group_unloaded(&mut self, group_id: &AudioGroupId) {
        let Some(state) = self.groups.get_mut(group_id) else {
            return;
        };

        state.preload_requested = false;
        state.load_status = AudioBankLoadStatus::NotLoaded;
        state.idle_countdown_seconds = None;
    }

    fn sync_loading_state(&mut self, loading: &AudioLoadingState) {
        for (group_id, state) in &mut self.groups {
            let Some(group) = loading.groups.get(group_id) else {
                if state.preload_requested {
                    state.preload_requested = false;
                    state.load_status = AudioBankLoadStatus::NotLoaded;
                    state.idle_countdown_seconds = None;
                }
                continue;
            };

            let progress = group.progress();
            state.preload_requested = true;
            state.load_status =
                if progress.total > 0 && progress.loaded + progress.failed >= progress.total {
                    AudioBankLoadStatus::Loaded
                } else {
                    AudioBankLoadStatus::Loading
                };
        }
    }

    fn record_cue_started(
        &mut self,
        cue_id: &AudioCueId,
        clip_id: &AudioClipId,
        instance_id: AudioInstanceId,
    ) {
        let group_id = self
            .cue_group(cue_id)
            .or_else(|| self.clip_group(clip_id))
            .cloned();
        if let Some(group_id) = group_id {
            self.record_active_instance(&group_id, instance_id);
        }
    }

    fn record_clip_started(&mut self, clip_id: &AudioClipId, instance_id: AudioInstanceId) {
        let group_id = self.clip_group(clip_id).cloned();
        if let Some(group_id) = group_id {
            self.record_active_instance(&group_id, instance_id);
        }
    }

    fn record_music_changed(&mut self, changed: &AudioMusicChanged) {
        let Some(instance_id) = changed.new_instance_id else {
            return;
        };

        self.record_clip_started(&changed.new_clip_id, instance_id);
    }

    fn record_active_instance(&mut self, group_id: &AudioGroupId, instance_id: AudioInstanceId) {
        if let Some(previous_group_id) = self
            .instance_to_group
            .insert(instance_id, group_id.clone())
            .filter(|previous| previous != group_id)
        {
            if let Some(previous_state) = self.groups.get_mut(&previous_group_id) {
                previous_state.active_instance_ids.remove(&instance_id);
                start_idle_countdown_if_needed(previous_state);
            }
        }

        let Some(state) = self.groups.get_mut(group_id) else {
            return;
        };
        state.active_instance_ids.insert(instance_id);
        state.idle_countdown_seconds = None;
    }

    fn record_instance_stopped(&mut self, instance_id: AudioInstanceId) {
        let Some(group_id) = self.instance_to_group.remove(&instance_id) else {
            for state in self.groups.values_mut() {
                if state.active_instance_ids.remove(&instance_id) {
                    start_idle_countdown_if_needed(state);
                    break;
                }
            }
            return;
        };

        let Some(state) = self.groups.get_mut(&group_id) else {
            return;
        };
        state.active_instance_ids.remove(&instance_id);
        start_idle_countdown_if_needed(state);
    }

    fn ensure_idle_countdowns_started(&mut self) {
        for state in self.groups.values_mut() {
            start_idle_countdown_if_needed(state);
        }
    }

    fn advance_idle_countdowns(&mut self, delta_seconds: f32) -> Vec<AudioGroupId> {
        self.ensure_idle_countdowns_started();

        let delta_seconds = delta_seconds.max(0.0);
        let mut expired = Vec::new();
        for state in self.groups.values_mut() {
            let Some(countdown) = &mut state.idle_countdown_seconds else {
                continue;
            };

            *countdown -= delta_seconds;
            if *countdown > 0.0 {
                continue;
            }

            let group_id = state.group_id.clone();
            state.preload_requested = false;
            state.load_status = AudioBankLoadStatus::NotLoaded;
            state.idle_countdown_seconds = None;
            expired.push(group_id);
        }

        expired
    }
}

pub fn handle_audio_bank_commands(
    mut audio_commands: ParamSet<(MessageReader<AudioCommand>, MessageWriter<AudioCommand>)>,
    catalog: Res<AudioCatalog>,
    mut bank: ResMut<AudioBankRuntime>,
) {
    let catalog_changed = catalog.is_changed();
    bank.rebuild_mappings_if_needed(&catalog, catalog_changed);

    let mut groups_to_load = Vec::new();
    for command in audio_commands.p0().read() {
        match command {
            AudioCommand::PreloadGroup(command) => {
                bank.mark_group_preload_requested(&command.group_id);
            }
            AudioCommand::UnloadGroup(command) => {
                bank.mark_group_unloaded(&command.group_id);
            }
            _ => {
                if let Some(group_id) = bank.group_for_command(command) {
                    groups_to_load.push(group_id);
                }
            }
        }
    }

    let mut audio_command_writer = audio_commands.p1();
    for group_id in groups_to_load {
        bank.ensure_group_loading(&group_id, &mut audio_command_writer);
    }
}

pub fn update_audio_bank_runtime(
    mut audio_event_reader: MessageReader<AudioEvent>,
    mut audio_command_writer: MessageWriter<AudioCommand>,
    catalog: Res<AudioCatalog>,
    loading: Res<AudioLoadingState>,
    time: Res<Time>,
    mut bank: ResMut<AudioBankRuntime>,
) {
    let catalog_changed = catalog.is_changed();
    bank.rebuild_mappings_if_needed(&catalog, catalog_changed);
    bank.sync_loading_state(&loading);

    for event in audio_event_reader.read() {
        match event {
            AudioEvent::CueStarted(started) => {
                bank.record_cue_started(&started.cue_id, &started.clip_id, started.instance_id)
            }
            AudioEvent::ClipStarted(started) => {
                bank.record_clip_started(&started.clip_id, started.instance_id);
            }
            AudioEvent::MusicChanged(changed) => {
                bank.record_music_changed(changed);
            }
            AudioEvent::InstanceStopped(stopped) => {
                bank.record_instance_stopped(stopped.instance_id);
            }
            _ => {}
        }
    }

    for group_id in bank.advance_idle_countdowns(time.delta_secs()) {
        audio_command_writer.write(AudioCommand::UnloadGroup(AudioGroupCommand::new(group_id)));
    }
}

fn start_idle_countdown_if_needed(state: &mut AudioBankGroupState) {
    if !state.active_instance_ids.is_empty() || state.resident() {
        state.idle_countdown_seconds = None;
        return;
    }

    if !state.preload_requested && matches!(state.load_status, AudioBankLoadStatus::NotLoaded) {
        state.idle_countdown_seconds = None;
        return;
    }

    if state.idle_countdown_seconds.is_none() {
        state.idle_countdown_seconds = Some(state.lazy_unload.as_secs_f32());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::audio::{
        AudioBattleCueRequest, AudioClipRequest, AudioCrossfadeMusicRequest, AudioCueClip,
        AudioCueEntry, AudioCueRequest, AudioGroupClip, AudioGroupEntry, AudioLoadingState,
        AudioMixer, AudioMusicRequest, AudioPlaybackState, AudioScopeId, AudioSpatialCueRequest,
        AudioSpatialSource, loading::handle_audio_loading_commands,
        playback::handle_audio_playback_commands,
    };
    use bevy::time::TimeUpdateStrategy;

    fn clip_id(value: &str) -> AudioClipId {
        AudioClipId::try_from(value).unwrap()
    }

    fn cue_id(value: &str) -> AudioCueId {
        AudioCueId::try_from(value).unwrap()
    }

    fn group_id(value: &str) -> AudioGroupId {
        AudioGroupId::try_from(value).unwrap()
    }

    fn bank_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .add_message::<AudioCommand>()
            .add_message::<AudioEvent>()
            .init_asset::<AudioSource>()
            .init_resource::<AudioCatalog>()
            .init_resource::<AudioMixer>()
            .init_resource::<AudioPlaybackState>()
            .init_resource::<AudioLoadingState>()
            .init_resource::<AudioBankRuntime>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO))
            .add_systems(
                Update,
                (
                    handle_audio_bank_commands,
                    handle_audio_playback_commands,
                    handle_audio_loading_commands,
                    update_audio_bank_runtime,
                )
                    .chain(),
            );
        app
    }

    fn register_banked_clip(
        app: &mut App,
        group_id: &AudioGroupId,
        clip_id: &AudioClipId,
        lazy_unload: Duration,
    ) {
        app.world_mut()
            .resource_mut::<AudioCatalog>()
            .register_clip(clip_id.clone(), "audio/ui/click_wood_01.wav");
        app.world_mut()
            .resource_mut::<AudioCatalog>()
            .register_group(
                group_id.clone(),
                AudioGroupEntry::from_required([clip_id.clone()]),
            );
        app.world_mut()
            .resource_mut::<AudioBankRuntime>()
            .register_group_config(AudioBankGroupConfig::new(group_id.clone(), lazy_unload));
    }

    fn set_delta(app: &mut App, seconds: f32) {
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
            seconds,
        )));
    }

    fn first_instance_id(app: &App) -> AudioInstanceId {
        *app.world()
            .resource::<AudioPlaybackState>()
            .instances
            .keys()
            .next()
            .unwrap()
    }

    #[test]
    fn bank_loads_group_on_first_clip_use_without_blocking_playback() {
        let mut app = bank_app();
        let group_id = group_id("bank.ui");
        let clip_id = clip_id("ui.click");
        register_banked_clip(&mut app, &group_id, &clip_id, Duration::from_secs_f32(1.0));

        app.world_mut()
            .write_message(AudioCommand::PlayClip(AudioClipRequest::new(
                clip_id.clone(),
            )));
        app.update();

        assert!(
            app.world()
                .resource::<AudioLoadingState>()
                .groups
                .contains_key(&group_id)
        );
        assert_eq!(
            app.world()
                .resource::<AudioPlaybackState>()
                .instances
                .values()
                .next()
                .unwrap()
                .clip_id,
            clip_id
        );
        let bank = app.world().resource::<AudioBankRuntime>();
        let state = bank.groups.get(&group_id).unwrap();
        assert!(state.preload_requested);
        assert_eq!(state.load_status, AudioBankLoadStatus::Loading);
        assert_eq!(state.active_instance_ids.len(), 1);
        assert_eq!(state.idle_countdown_seconds, None);
    }

    #[test]
    fn bank_idle_timer_is_cancelled_by_replay_before_unload() {
        let mut app = bank_app();
        let group_id = group_id("bank.ui");
        let clip_id = clip_id("ui.click");
        register_banked_clip(&mut app, &group_id, &clip_id, Duration::from_secs_f32(1.0));

        app.world_mut()
            .write_message(AudioCommand::PlayClip(AudioClipRequest::new(
                clip_id.clone(),
            )));
        app.update();
        let first = first_instance_id(&app);
        app.world_mut().write_message(AudioCommand::StopInstance(
            super::super::command::AudioStopInstanceCommand::new(first),
        ));
        app.update();
        assert!(
            app.world()
                .resource::<AudioBankRuntime>()
                .groups
                .get(&group_id)
                .unwrap()
                .idle_countdown_seconds
                .is_some()
        );

        set_delta(&mut app, 0.5);
        app.update();
        app.world_mut()
            .write_message(AudioCommand::PlayClip(AudioClipRequest::new(clip_id)));
        app.update();

        let state = app
            .world()
            .resource::<AudioBankRuntime>()
            .groups
            .get(&group_id)
            .unwrap();
        assert_eq!(state.idle_countdown_seconds, None);
        assert_eq!(state.active_instance_ids.len(), 1);

        set_delta(&mut app, 1.0);
        app.update();
        assert!(
            app.world()
                .resource::<AudioLoadingState>()
                .groups
                .contains_key(&group_id)
        );
    }

    #[test]
    fn bank_unloads_after_idle_timer_expires() {
        let mut app = bank_app();
        let group_id = group_id("bank.ui");
        let clip_id = clip_id("ui.click");
        register_banked_clip(&mut app, &group_id, &clip_id, Duration::from_secs_f32(0.25));

        app.world_mut()
            .write_message(AudioCommand::PlayClip(AudioClipRequest::new(clip_id)));
        app.update();
        let instance_id = first_instance_id(&app);
        app.world_mut().write_message(AudioCommand::StopInstance(
            super::super::command::AudioStopInstanceCommand::new(instance_id),
        ));
        app.update();

        set_delta(&mut app, 0.3);
        app.update();
        assert_eq!(
            app.world()
                .resource::<AudioBankRuntime>()
                .groups
                .get(&group_id)
                .unwrap()
                .load_status,
            AudioBankLoadStatus::NotLoaded
        );

        set_delta(&mut app, 0.0);
        app.update();
        assert!(
            !app.world()
                .resource::<AudioLoadingState>()
                .groups
                .contains_key(&group_id)
        );
    }

    #[test]
    fn resident_bank_does_not_start_idle_countdown_or_auto_unload() {
        let mut app = bank_app();
        let group_id = group_id("bank.resident");
        let clip_id = clip_id("ui.click");
        register_banked_clip(&mut app, &group_id, &clip_id, Duration::ZERO);

        app.world_mut()
            .write_message(AudioCommand::PlayClip(AudioClipRequest::new(clip_id)));
        app.update();
        let instance_id = first_instance_id(&app);
        app.world_mut().write_message(AudioCommand::StopInstance(
            super::super::command::AudioStopInstanceCommand::new(instance_id),
        ));
        app.update();

        set_delta(&mut app, 10.0);
        app.update();
        app.update();

        let state = app
            .world()
            .resource::<AudioBankRuntime>()
            .groups
            .get(&group_id)
            .unwrap();
        assert!(state.resident());
        assert_eq!(state.idle_countdown_seconds, None);
        assert!(
            app.world()
                .resource::<AudioLoadingState>()
                .groups
                .contains_key(&group_id)
        );
    }

    #[test]
    fn manual_unload_releases_group_handles_without_stopping_active_instance() {
        let mut app = bank_app();
        let group_id = group_id("bank.ui");
        let clip_id = clip_id("ui.click");
        register_banked_clip(&mut app, &group_id, &clip_id, Duration::from_secs_f32(1.0));

        app.world_mut()
            .write_message(AudioCommand::PlayClip(AudioClipRequest::new(clip_id)));
        app.update();
        let instance_id = first_instance_id(&app);

        app.world_mut()
            .write_message(AudioCommand::UnloadGroup(AudioGroupCommand::new(
                group_id.clone(),
            )));
        app.update();

        assert!(
            app.world()
                .resource::<AudioPlaybackState>()
                .instances
                .contains_key(&instance_id)
        );
        assert!(
            !app.world()
                .resource::<AudioLoadingState>()
                .groups
                .contains_key(&group_id)
        );
        let state = app
            .world()
            .resource::<AudioBankRuntime>()
            .groups
            .get(&group_id)
            .unwrap();
        assert_eq!(state.load_status, AudioBankLoadStatus::NotLoaded);
        assert!(state.active_instance_ids.contains(&instance_id));
    }

    #[test]
    fn bank_command_mapping_covers_cue_clip_music_spatial_and_battle_requests() {
        let group_id = group_id("bank.shared");
        let clip_id = clip_id("shared.hit");
        let cue_id = cue_id("shared.hit");
        let mut catalog = AudioCatalog::default();
        catalog.register_clip(clip_id.clone(), "audio/battle/sword_hit_01.wav");
        catalog.register_cue(
            cue_id.clone(),
            AudioCueEntry::from_clips([AudioCueClip::new(clip_id.clone())]),
        );
        catalog.register_group(
            group_id.clone(),
            AudioGroupEntry::from_required([clip_id.clone()]),
        );

        let mut bank = AudioBankRuntime::default();
        bank.register_group_config(AudioBankGroupConfig::new(
            group_id.clone(),
            Duration::from_secs(1),
        ));
        bank.rebuild_mappings(&catalog);

        let commands = [
            AudioCommand::PlayCue(AudioCueRequest::new(cue_id.clone())),
            AudioCommand::PlayClip(AudioClipRequest::new(clip_id.clone())),
            AudioCommand::PlayMusic(AudioMusicRequest::new(clip_id.clone())),
            AudioCommand::CrossfadeMusic(AudioCrossfadeMusicRequest::new(clip_id.clone(), 0.5)),
            AudioCommand::PlaySpatialCue(AudioSpatialCueRequest::new(
                cue_id.clone(),
                AudioSpatialSource::fixed(Transform::default()),
            )),
            AudioCommand::PlayBattleCue(AudioBattleCueRequest::new(
                AudioScopeId::try_from("battle.test").unwrap(),
                cue_id,
            )),
        ];

        for command in commands {
            assert_eq!(bank.group_for_command(&command), Some(group_id.clone()));
        }
    }

    #[test]
    fn duplicate_clip_mapping_keeps_first_group_and_records_conflict() {
        let first_group = group_id("bank.first");
        let second_group = group_id("bank.second");
        let clip_id = clip_id("shared.hit");
        let mut catalog = AudioCatalog::default();
        catalog.register_clip(clip_id.clone(), "audio/battle/sword_hit_01.wav");
        catalog.register_group(
            first_group.clone(),
            AudioGroupEntry::from_clips([AudioGroupClip::required(clip_id.clone())]),
        );
        catalog.register_group(
            second_group.clone(),
            AudioGroupEntry::from_clips([AudioGroupClip::required(clip_id.clone())]),
        );

        let mut bank = AudioBankRuntime::default();
        bank.register_group_config(AudioBankGroupConfig::new(
            first_group.clone(),
            Duration::from_secs(1),
        ));
        bank.register_group_config(AudioBankGroupConfig::new(
            second_group.clone(),
            Duration::from_secs(1),
        ));
        bank.rebuild_mappings(&catalog);

        assert_eq!(bank.clip_group(&clip_id), Some(&first_group));
        assert_eq!(
            bank.mapping_conflicts(),
            &[AudioBankMappingConflict {
                item: AudioBankMappingConflictItem::Clip(clip_id),
                kept_group_id: first_group,
                ignored_group_id: second_group,
            }]
        );
    }
}
