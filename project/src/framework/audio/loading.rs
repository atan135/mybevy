use std::collections::HashMap;

use bevy::asset::LoadState;
use bevy::prelude::*;

use super::{
    catalog::{AudioCatalog, AudioCatalogError, AudioResolvedGroup, AudioResolvedGroupClip},
    command::AudioCommand,
    event::{AudioEvent, AudioLoadFailed, AudioLoadProgress},
    id::{AudioClipId, AudioGroupId},
};

#[derive(Debug, Default, Resource)]
pub struct AudioLoadingState {
    pub groups: HashMap<AudioGroupId, AudioGroupLoadState>,
}

impl AudioLoadingState {
    pub fn unload_group(&mut self, group_id: &AudioGroupId) -> Option<AudioGroupLoadState> {
        self.groups.remove(group_id)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioGroupLoadState {
    pub group_id: AudioGroupId,
    pub clips: Vec<AudioClipLoadState>,
    last_progress: Option<AudioLoadProgress>,
}

impl AudioGroupLoadState {
    fn new(group: AudioResolvedGroup, asset_server: &AssetServer) -> Self {
        Self {
            group_id: group.group_id,
            clips: group
                .clips
                .into_iter()
                .map(|clip| AudioClipLoadState::new(clip, asset_server))
                .collect(),
            last_progress: None,
        }
    }

    pub fn progress(&self) -> AudioGroupProgress {
        let mut progress = AudioGroupProgress {
            group_id: self.group_id.clone(),
            loaded: 0,
            total: self.clips.len(),
            failed: 0,
            required_loaded: 0,
            required_total: self.clips.iter().filter(|clip| clip.required).count(),
            required_failed: 0,
        };

        for clip in &self.clips {
            match clip.state {
                AudioClipLoadStatus::Loaded => {
                    progress.loaded += 1;
                    if clip.required {
                        progress.required_loaded += 1;
                    }
                }
                AudioClipLoadStatus::Failed => {
                    progress.failed += 1;
                    if clip.required {
                        progress.required_failed += 1;
                    }
                }
                AudioClipLoadStatus::Loading => {}
            }
        }

        progress
    }

    fn progress_event(&self, clip: Option<&AudioClipLoadState>) -> AudioLoadProgress {
        let progress = self.progress();
        AudioLoadProgress {
            group_id: self.group_id.clone(),
            loaded: progress.loaded,
            total: progress.total,
            failed: progress.failed,
            required_loaded: progress.required_loaded,
            required_total: progress.required_total,
            required_failed: progress.required_failed,
            clip_id: clip.map(|clip| clip.clip_id.clone()),
            asset_path: clip.map(|clip| clip.asset_path.clone()),
        }
    }

    fn take_initial_progress(&mut self) -> AudioLoadProgress {
        let progress = self.progress_event(None);
        self.last_progress = Some(progress.clone());
        progress
    }

    fn take_progress_if_changed(&mut self, clip_index: usize) -> Option<AudioLoadProgress> {
        let clip = self.clips.get(clip_index);
        let progress = self.progress_event(clip);
        if self.last_progress.as_ref() == Some(&progress) {
            return None;
        }

        self.last_progress = Some(progress.clone());
        Some(progress)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioClipLoadState {
    pub clip_id: AudioClipId,
    pub asset_path: String,
    pub required: bool,
    pub handle: Handle<AudioSource>,
    pub state: AudioClipLoadStatus,
    failure_reported: bool,
}

impl AudioClipLoadState {
    fn new(clip: AudioResolvedGroupClip, asset_server: &AssetServer) -> Self {
        let handle = asset_server.load::<AudioSource>(clip.path.clone());
        Self {
            clip_id: clip.clip_id,
            asset_path: clip.path,
            required: clip.required,
            handle,
            state: AudioClipLoadStatus::Loading,
            failure_reported: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AudioClipLoadStatus {
    #[default]
    Loading,
    Loaded,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioGroupProgress {
    pub group_id: AudioGroupId,
    pub loaded: usize,
    pub total: usize,
    pub failed: usize,
    pub required_loaded: usize,
    pub required_total: usize,
    pub required_failed: usize,
}

pub fn handle_audio_loading_commands(
    mut audio_commands: MessageReader<AudioCommand>,
    mut audio_events: MessageWriter<AudioEvent>,
    asset_server: Res<AssetServer>,
    catalog: Res<AudioCatalog>,
    mut loading: ResMut<AudioLoadingState>,
) {
    for command in audio_commands.read() {
        match command {
            AudioCommand::PreloadGroup(command) => {
                preload_audio_group(
                    command.group_id.clone(),
                    &asset_server,
                    &catalog,
                    &mut loading,
                    &mut audio_events,
                );
            }
            AudioCommand::UnloadGroup(command) => {
                loading.unload_group(&command.group_id);
            }
            _ => {}
        }
    }
}

pub fn poll_audio_group_load_progress(
    mut audio_events: MessageWriter<AudioEvent>,
    asset_server: Res<AssetServer>,
    mut loading: ResMut<AudioLoadingState>,
) {
    for group in loading.groups.values_mut() {
        let mut changed = Vec::new();

        for (index, clip) in group.clips.iter_mut().enumerate() {
            if clip.state != AudioClipLoadStatus::Loading {
                continue;
            }

            match asset_server.load_state(clip.handle.id()) {
                LoadState::Loaded => {
                    clip.state = AudioClipLoadStatus::Loaded;
                    changed.push(index);
                }
                LoadState::Failed(error) => {
                    clip.state = AudioClipLoadStatus::Failed;
                    if !clip.failure_reported {
                        clip.failure_reported = true;
                        audio_events.write(AudioEvent::LoadFailed(AudioLoadFailed {
                            clip_id: Some(clip.clip_id.clone()),
                            cue_id: None,
                            group_id: Some(group.group_id.clone()),
                            asset_path: Some(clip.asset_path.clone()),
                            message: error.to_string(),
                        }));
                    }
                    changed.push(index);
                }
                LoadState::NotLoaded | LoadState::Loading => {}
            }
        }

        for index in changed {
            if let Some(progress) = group.take_progress_if_changed(index) {
                audio_events.write(AudioEvent::LoadProgress(progress));
            }
        }
    }
}

pub(crate) fn preload_audio_group(
    group_id: AudioGroupId,
    asset_server: &AssetServer,
    catalog: &AudioCatalog,
    loading: &mut AudioLoadingState,
    audio_events: &mut MessageWriter<AudioEvent>,
) {
    let resolved = match catalog.resolve_group(&group_id) {
        Ok(resolved) => resolved,
        Err(error) => {
            send_group_catalog_failure(audio_events, &error, Some(group_id));
            return;
        }
    };

    let mut group = AudioGroupLoadState::new(resolved, asset_server);
    let progress = group.take_initial_progress();
    loading.groups.insert(group.group_id.clone(), group);
    audio_events.write(AudioEvent::LoadProgress(progress));
}

fn send_group_catalog_failure(
    audio_events: &mut MessageWriter<AudioEvent>,
    error: &AudioCatalogError,
    requested_group_id: Option<AudioGroupId>,
) {
    let (clip_id, cue_id, group_id, asset_path) = match error {
        AudioCatalogError::MissingCue(missing_cue) => {
            (None, Some(missing_cue.clone()), requested_group_id, None)
        }
        AudioCatalogError::MissingClip(missing_clip) => {
            (Some(missing_clip.clone()), None, requested_group_id, None)
        }
        AudioCatalogError::MissingGroup(missing_group) => {
            (None, None, Some(missing_group.clone()), None)
        }
        AudioCatalogError::EmptyCue(empty_cue) => {
            (None, Some(empty_cue.clone()), requested_group_id, None)
        }
        AudioCatalogError::EmptyGroup(empty_group) => (None, None, Some(empty_group.clone()), None),
    };

    audio_events.write(AudioEvent::LoadFailed(AudioLoadFailed {
        clip_id,
        cue_id,
        group_id,
        asset_path,
        message: error.to_string(),
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::audio::{
        AudioCatalog, AudioGroupClip, AudioGroupCommand, AudioGroupEntry,
    };
    use bevy::audio::AudioPlugin as BevyAudioPlugin;
    use bevy::ecs::message::MessageCursor;

    fn clip_id(value: &str) -> AudioClipId {
        AudioClipId::try_from(value).unwrap()
    }

    fn group_id(value: &str) -> AudioGroupId {
        AudioGroupId::try_from(value).unwrap()
    }

    fn loading_app() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin::default(),
            BevyAudioPlugin::default(),
        ))
        .add_message::<AudioCommand>()
        .add_message::<AudioEvent>()
        .init_asset::<AudioSource>()
        .init_resource::<AudioCatalog>()
        .init_resource::<AudioLoadingState>()
        .add_systems(
            Update,
            (
                handle_audio_loading_commands,
                poll_audio_group_load_progress,
            )
                .chain(),
        );
        app
    }

    fn read_events(app: &App) -> Vec<AudioEvent> {
        let messages = app.world().resource::<Messages<AudioEvent>>();
        let mut cursor = MessageCursor::default();
        cursor.read(messages).cloned().collect()
    }

    #[test]
    fn preload_group_records_handles_and_sends_initial_progress() {
        let mut app = loading_app();
        let group_id = group_id("boot");
        let click = clip_id("ui.click");
        let confirm = clip_id("ui.confirm");
        app.world_mut()
            .resource_mut::<AudioCatalog>()
            .register_clip(click.clone(), "audio/ui/click_wood_01.wav");
        app.world_mut()
            .resource_mut::<AudioCatalog>()
            .register_clip(confirm.clone(), "audio/ui/confirm_brick_01.wav");
        app.world_mut()
            .resource_mut::<AudioCatalog>()
            .register_group(
                group_id.clone(),
                AudioGroupEntry::from_clips([
                    AudioGroupClip::required(click.clone()),
                    AudioGroupClip::optional(confirm.clone()),
                ]),
            );

        app.world_mut()
            .write_message(AudioCommand::PreloadGroup(AudioGroupCommand::new(
                group_id.clone(),
            )));
        app.update();

        let loading = app.world().resource::<AudioLoadingState>();
        let group = loading.groups.get(&group_id).unwrap();
        assert_eq!(group.clips.len(), 2);
        assert!(group.clips.iter().any(|clip| {
            clip.clip_id == click
                && clip.required
                && clip.asset_path == "audio/ui/click_wood_01.wav"
        }));
        assert!(group.clips.iter().any(|clip| {
            clip.clip_id == confirm
                && !clip.required
                && clip.asset_path == "audio/ui/confirm_brick_01.wav"
        }));

        assert_eq!(
            read_events(&app),
            vec![AudioEvent::LoadProgress(AudioLoadProgress {
                group_id,
                loaded: 0,
                total: 2,
                failed: 0,
                required_loaded: 0,
                required_total: 1,
                required_failed: 0,
                clip_id: None,
                asset_path: None,
            })]
        );
    }

    #[test]
    fn poll_progress_marks_loaded_clips_and_reports_clip_context() {
        let mut app = loading_app();
        let group_id = group_id("boot");
        let clip_id = clip_id("ui.click");
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
            .write_message(AudioCommand::PreloadGroup(AudioGroupCommand::new(
                group_id.clone(),
            )));
        app.update();
        run_until_group_progress(&mut app, &group_id, |progress| progress.loaded == 1);

        let group = app
            .world()
            .resource::<AudioLoadingState>()
            .groups
            .get(&group_id)
            .unwrap();
        assert_eq!(group.clips[0].state, AudioClipLoadStatus::Loaded);
        assert_eq!(
            group.progress(),
            AudioGroupProgress {
                group_id: group_id.clone(),
                loaded: 1,
                total: 1,
                failed: 0,
                required_loaded: 1,
                required_total: 1,
                required_failed: 0,
            }
        );
        assert!(
            read_events(&app).contains(&AudioEvent::LoadProgress(AudioLoadProgress {
                group_id,
                loaded: 1,
                total: 1,
                failed: 0,
                required_loaded: 1,
                required_total: 1,
                required_failed: 0,
                clip_id: Some(clip_id),
                asset_path: Some("audio/ui/click_wood_01.wav".to_string()),
            }))
        );
    }

    #[test]
    fn missing_clip_id_reports_group_and_clip_without_starting_load() {
        let mut app = loading_app();
        let group_id = group_id("boot");
        let missing = clip_id("ui.missing");
        app.world_mut()
            .resource_mut::<AudioCatalog>()
            .register_group(
                group_id.clone(),
                AudioGroupEntry::from_required([missing.clone()]),
            );

        app.world_mut()
            .write_message(AudioCommand::PreloadGroup(AudioGroupCommand::new(
                group_id.clone(),
            )));
        app.update();

        assert!(
            app.world()
                .resource::<AudioLoadingState>()
                .groups
                .is_empty()
        );
        assert_eq!(
            read_events(&app),
            vec![AudioEvent::LoadFailed(AudioLoadFailed {
                clip_id: Some(missing.clone()),
                cue_id: None,
                group_id: Some(group_id.clone()),
                asset_path: None,
                message: format!("audio clip not found: {missing}"),
            })]
        );
    }

    #[test]
    fn unload_group_clears_cached_state_and_handles() {
        let mut app = loading_app();
        let group_id = group_id("boot");
        let clip_id = clip_id("ui.click");
        app.world_mut()
            .resource_mut::<AudioCatalog>()
            .register_clip(clip_id.clone(), "audio/ui/click_wood_01.wav");
        app.world_mut()
            .resource_mut::<AudioCatalog>()
            .register_group(group_id.clone(), AudioGroupEntry::from_required([clip_id]));

        app.world_mut()
            .write_message(AudioCommand::PreloadGroup(AudioGroupCommand::new(
                group_id.clone(),
            )));
        app.update();
        assert!(
            app.world()
                .resource::<AudioLoadingState>()
                .groups
                .contains_key(&group_id)
        );

        app.world_mut()
            .write_message(AudioCommand::UnloadGroup(AudioGroupCommand::new(
                group_id.clone(),
            )));
        app.update();

        assert!(
            !app.world()
                .resource::<AudioLoadingState>()
                .groups
                .contains_key(&group_id)
        );
    }

    #[test]
    fn optional_clip_failure_does_not_block_required_progress() {
        let mut app = loading_app();
        let group_id = group_id("boot");
        let required = clip_id("ui.click");
        let optional = clip_id("ui.optional");
        app.world_mut()
            .resource_mut::<AudioCatalog>()
            .register_clip(required.clone(), "audio/ui/click_wood_01.wav");
        app.world_mut()
            .resource_mut::<AudioCatalog>()
            .register_clip(optional.clone(), "audio/ui/does_not_exist.wav");
        app.world_mut()
            .resource_mut::<AudioCatalog>()
            .register_group(
                group_id.clone(),
                AudioGroupEntry::from_clips([
                    AudioGroupClip::required(required.clone()),
                    AudioGroupClip::optional(optional.clone()),
                ]),
            );

        app.world_mut()
            .write_message(AudioCommand::PreloadGroup(AudioGroupCommand::new(
                group_id.clone(),
            )));
        app.update();
        run_until_group_progress(&mut app, &group_id, |progress| {
            progress.required_loaded == 1 && progress.failed == 1
        });

        let group = app
            .world()
            .resource::<AudioLoadingState>()
            .groups
            .get(&group_id)
            .unwrap();
        assert_eq!(
            group.progress(),
            AudioGroupProgress {
                group_id: group_id.clone(),
                loaded: 1,
                total: 2,
                failed: 1,
                required_loaded: 1,
                required_total: 1,
                required_failed: 0,
            }
        );

        let events = read_events(&app);
        assert!(events.iter().any(|event| matches!(
            event,
            AudioEvent::LoadFailed(AudioLoadFailed {
                clip_id: Some(failed_clip),
                group_id: Some(failed_group),
                asset_path: Some(path),
                ..
            }) if failed_clip == &optional
                && failed_group == &group_id
                && path == "audio/ui/does_not_exist.wav"
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            AudioEvent::LoadProgress(AudioLoadProgress {
                group_id: progress_group,
                loaded: 1,
                total: 2,
                failed: 1,
                required_loaded: 1,
                required_total: 1,
                required_failed: 0,
                ..
            }) if progress_group == &group_id
        )));
    }

    #[test]
    fn progress_counts_optional_failure_as_finished_without_required_failure() {
        let group_id = group_id("boot");
        let required = clip_id("ui.click");
        let optional = clip_id("ui.optional");
        let mut state = AudioGroupLoadState {
            group_id: group_id.clone(),
            clips: vec![
                AudioClipLoadState {
                    clip_id: required,
                    asset_path: "audio/ui/click_wood_01.wav".to_string(),
                    required: true,
                    handle: Handle::default(),
                    state: AudioClipLoadStatus::Loaded,
                    failure_reported: false,
                },
                AudioClipLoadState {
                    clip_id: optional,
                    asset_path: "audio/ui/optional.wav".to_string(),
                    required: false,
                    handle: Handle::default(),
                    state: AudioClipLoadStatus::Failed,
                    failure_reported: true,
                },
            ],
            last_progress: None,
        };

        assert_eq!(
            state.progress(),
            AudioGroupProgress {
                group_id: group_id.clone(),
                loaded: 1,
                total: 2,
                failed: 1,
                required_loaded: 1,
                required_total: 1,
                required_failed: 0,
            }
        );
        assert_eq!(
            state.take_initial_progress(),
            AudioLoadProgress {
                group_id,
                loaded: 1,
                total: 2,
                failed: 1,
                required_loaded: 1,
                required_total: 1,
                required_failed: 0,
                clip_id: None,
                asset_path: None,
            }
        );
    }

    fn run_until_group_progress(
        app: &mut App,
        group_id: &AudioGroupId,
        predicate: impl Fn(&AudioGroupProgress) -> bool,
    ) {
        for _ in 0..500 {
            app.update();
            let progress = app
                .world()
                .resource::<AudioLoadingState>()
                .groups
                .get(group_id)
                .unwrap()
                .progress();
            if predicate(&progress) {
                return;
            }
        }

        panic!(
            "timed out waiting for audio group progress: {:?}",
            app.world()
                .resource::<AudioLoadingState>()
                .groups
                .get(group_id)
                .unwrap()
                .progress()
        );
    }
}
