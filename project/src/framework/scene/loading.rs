use bevy::{asset::LoadedUntypedAsset, ecs::message::Messages, prelude::*};
use serde::{Deserialize, Deserializer};

use crate::framework::ui::{
    core::{UI_PANEL_GLOBAL_LOADING, UiPanelCommand, UiPanelRequest},
    overlays::UiLoading,
};

use super::{
    camera::SceneCameraConfig,
    event::SceneEvent,
    id::{SceneAssetId, SceneId, SceneLayerId, SceneSessionId},
    manifest::{
        SceneAssetRef, SceneManifest, asset_path_with_label, normalize_manifest_token,
        validate_asset_relative_path,
    },
    spawn::SceneSpawnSessionIndex,
    trigger::SceneTriggerManifest,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SceneLoadingPolicy {
    None,
    Spinner,
    Progress,
    #[default]
    Blocking,
    NonBlocking,
}

impl<'de> Deserialize<'de> for SceneLoadingPolicy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match normalize_manifest_token(&value).as_str() {
            "none" => Self::None,
            "spinner" => Self::Spinner,
            "progress" => Self::Progress,
            "blocking" | "blockingrequiredassets" => Self::Blocking,
            "nonblocking" | "background" | "preload" => Self::NonBlocking,
            _ => Self::Blocking,
        })
    }
}

impl SceneLoadingPolicy {
    pub fn opens_global_loading(self) -> bool {
        matches!(self, Self::Spinner | Self::Progress | Self::Blocking)
    }

    pub fn shows_progress_counts(self) -> bool {
        matches!(self, Self::Progress | Self::Blocking)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneLoadProgress {
    pub scene_id: SceneId,
    pub session_id: Option<SceneSessionId>,
    pub phase: SceneLoadPhase,
    pub loading_policy: SceneLoadingPolicy,
    pub required_total: usize,
    pub required_loaded: usize,
    pub optional_total: usize,
    pub optional_loaded: usize,
    pub optional_failed: usize,
    pub failed: Vec<SceneAssetLoadFailure>,
    pub message_key: Option<String>,
}

impl SceneLoadProgress {
    pub fn new(scene_id: impl Into<SceneId>, phase: SceneLoadPhase) -> Self {
        Self {
            scene_id: scene_id.into(),
            session_id: None,
            phase,
            loading_policy: SceneLoadingPolicy::default(),
            required_total: 0,
            required_loaded: 0,
            optional_total: 0,
            optional_loaded: 0,
            optional_failed: 0,
            failed: Vec::new(),
            message_key: None,
        }
    }

    pub fn required_fraction(&self) -> Option<f32> {
        (self.required_total > 0).then(|| self.required_loaded as f32 / self.required_total as f32)
    }

    pub fn total_fraction(&self) -> Option<f32> {
        let total = self.required_total + self.optional_total;
        (total > 0).then(|| {
            (self.required_loaded + self.optional_loaded + self.optional_failed) as f32
                / total as f32
        })
    }

    pub fn required_complete(&self) -> bool {
        self.required_loaded == self.required_total
            && self.failed.iter().all(|failure| !failure.required)
    }

    pub fn ui_loading_text(&self) -> String {
        let base_text = match self.phase {
            SceneLoadPhase::Resolving | SceneLoadPhase::LoadingAssets => "Loading...",
            SceneLoadPhase::Downloading => "Loading... downloading",
            SceneLoadPhase::Instantiating => "Loading... preparing",
            SceneLoadPhase::Activating => "Loading... activating",
            SceneLoadPhase::Complete => "Loading... complete",
        };

        if self.loading_policy.shows_progress_counts()
            && let Some(progress_text) = self.ui_progress_text()
        {
            return format!("{base_text} {progress_text}");
        }

        base_text.to_string()
    }

    fn ui_progress_text(&self) -> Option<String> {
        let total = self.required_total + self.optional_total;
        if total == 0 {
            return None;
        }

        let completed =
            (self.required_loaded + self.optional_loaded + self.optional_failed).min(total);
        Some(format!("{completed}/{total}"))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SceneLoadPhase {
    #[default]
    Resolving,
    Downloading,
    LoadingAssets,
    Instantiating,
    Activating,
    Complete,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneAssetLoadFailure {
    pub asset_id: Option<SceneAssetId>,
    pub layer_id: Option<SceneLayerId>,
    pub path: Option<String>,
    pub required: bool,
    pub message: String,
}

#[derive(Clone, Debug, Resource, PartialEq, Eq)]
pub struct SceneLoadingUiConfig {
    pub enabled: bool,
}

impl Default for SceneLoadingUiConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Clone, Debug, Default, Resource, PartialEq, Eq)]
pub struct SceneLoadingUiState {
    current: Option<SceneLoadingUiSession>,
    global_loading_open: bool,
    finished_session: Option<SceneSessionId>,
}

impl SceneLoadingUiState {
    pub fn current(&self) -> Option<&SceneLoadingUiSession> {
        self.current.as_ref()
    }

    pub fn global_loading_open(&self) -> bool {
        self.global_loading_open
    }

    fn note_resolving(&mut self, scene_id: &SceneId, session_id: Option<&SceneSessionId>) {
        if self.finished_matches(scene_id, session_id) {
            self.finished_session = None;
        }
    }

    fn apply_progress(&mut self, progress: &SceneLoadProgress) -> SceneLoadingUiAction {
        if self.progress_is_finished(progress) {
            return SceneLoadingUiAction::None;
        }

        if !progress.loading_policy.opens_global_loading() {
            if self.current_matches_progress(progress) {
                return self.close_current();
            }

            return SceneLoadingUiAction::None;
        }

        let text = progress.ui_loading_text();
        let should_open = !self.global_loading_open
            || self
                .current
                .as_ref()
                .is_none_or(|current| !current.matches_progress(progress) || current.text != text);

        self.current = Some(SceneLoadingUiSession::from_progress(progress, text.clone()));
        self.global_loading_open = true;

        if should_open {
            SceneLoadingUiAction::Open(text)
        } else {
            SceneLoadingUiAction::None
        }
    }

    fn complete_scene(
        &mut self,
        scene_id: Option<&SceneId>,
        session_id: Option<&SceneSessionId>,
    ) -> SceneLoadingUiAction {
        if let Some(session_id) = session_id {
            self.finished_session = Some(session_id.clone());
        }

        let matches_current = self
            .current
            .as_ref()
            .map(|current| current.matches(scene_id, session_id))
            .unwrap_or(true);

        if matches_current {
            self.close_current()
        } else {
            SceneLoadingUiAction::None
        }
    }

    fn close_current(&mut self) -> SceneLoadingUiAction {
        self.current = None;

        if self.global_loading_open {
            self.global_loading_open = false;
            SceneLoadingUiAction::Close
        } else {
            SceneLoadingUiAction::None
        }
    }

    fn current_matches_progress(&self, progress: &SceneLoadProgress) -> bool {
        self.current
            .as_ref()
            .is_some_and(|current| current.matches_progress(progress))
    }

    fn progress_is_finished(&self, progress: &SceneLoadProgress) -> bool {
        progress
            .session_id
            .as_ref()
            .is_some_and(|session_id| self.finished_session.as_ref() == Some(session_id))
    }

    fn finished_matches(&self, scene_id: &SceneId, session_id: Option<&SceneSessionId>) -> bool {
        if let Some(session_id) = session_id {
            return self.finished_session.as_ref() == Some(session_id);
        }

        self.current
            .as_ref()
            .is_some_and(|current| &current.scene_id == scene_id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneLoadingUiSession {
    pub scene_id: SceneId,
    pub session_id: Option<SceneSessionId>,
    pub policy: SceneLoadingPolicy,
    pub text: String,
}

impl SceneLoadingUiSession {
    fn from_progress(progress: &SceneLoadProgress, text: String) -> Self {
        Self {
            scene_id: progress.scene_id.clone(),
            session_id: progress.session_id.clone(),
            policy: progress.loading_policy,
            text,
        }
    }

    fn matches_progress(&self, progress: &SceneLoadProgress) -> bool {
        self.matches(Some(&progress.scene_id), progress.session_id.as_ref())
    }

    fn matches(&self, scene_id: Option<&SceneId>, session_id: Option<&SceneSessionId>) -> bool {
        if let Some(session_id) = session_id {
            return self.session_id.as_ref() == Some(session_id);
        }

        scene_id.is_none_or(|scene_id| &self.scene_id == scene_id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum SceneLoadingUiAction {
    None,
    Open(String),
    Close,
}

pub(crate) fn sync_scene_loading_ui(
    config: Res<SceneLoadingUiConfig>,
    mut state: ResMut<SceneLoadingUiState>,
    mut scene_events: MessageReader<SceneEvent>,
    mut ui_panel_messages: Option<ResMut<Messages<UiPanelCommand>>>,
) {
    if !config.enabled {
        for _ in scene_events.read() {}
        write_scene_loading_ui_action(&mut ui_panel_messages, state.close_current());
        return;
    }

    for event in scene_events.read() {
        let action = match event {
            SceneEvent::Resolving(resolving) => {
                state.note_resolving(&resolving.scene_id, resolving.session_id.as_ref());
                SceneLoadingUiAction::None
            }
            SceneEvent::LoadProgress(progress) => state.apply_progress(progress),
            SceneEvent::Entered(entered) => {
                state.complete_scene(Some(&entered.scene_id), Some(&entered.session_id))
            }
            SceneEvent::Exited(exited) => {
                state.complete_scene(Some(&exited.scene_id), Some(&exited.session_id))
            }
            SceneEvent::Failed(failure) => {
                state.complete_scene(failure.scene_id.as_ref(), failure.session_id.as_ref())
            }
            _ => SceneLoadingUiAction::None,
        };

        write_scene_loading_ui_action(&mut ui_panel_messages, action);
    }
}

fn write_scene_loading_ui_action(
    ui_panel_messages: &mut Option<ResMut<Messages<UiPanelCommand>>>,
    action: SceneLoadingUiAction,
) {
    let Some(ui_panel_messages) = ui_panel_messages else {
        return;
    };

    match action {
        SceneLoadingUiAction::Open(text) => {
            ui_panel_messages.write(UiPanelCommand::Open(UiPanelRequest::Loading(
                UiLoading::new(text),
            )));
        }
        SceneLoadingUiAction::Close => {
            ui_panel_messages.write(UiPanelCommand::Close(UI_PANEL_GLOBAL_LOADING));
        }
        SceneLoadingUiAction::None => {}
    }
}

#[derive(Clone, Debug, Default, Resource)]
pub(crate) struct SceneAssetLoadQueue {
    pub(crate) current: Option<SceneAssetLoadSession>,
}

impl SceneAssetLoadQueue {
    pub(crate) fn start(&mut self, session: SceneAssetLoadSession) {
        self.current = Some(session);
    }

    pub(crate) fn take_current(&mut self) -> Option<SceneAssetLoadSession> {
        self.current.take()
    }

    pub(crate) fn current_mut(&mut self) -> Option<&mut SceneAssetLoadSession> {
        self.current.as_mut()
    }

    pub(crate) fn clear_session(&mut self, session_id: &SceneSessionId) {
        if self
            .current
            .as_ref()
            .is_some_and(|session| &session.session_id == session_id)
        {
            self.current = None;
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SceneAssetLoadSession {
    pub(crate) scene_id: SceneId,
    pub(crate) session_id: SceneSessionId,
    pub(crate) content_version: Option<String>,
    pub(crate) loading_policy: SceneLoadingPolicy,
    pub(crate) has_world_root: bool,
    pub(crate) camera_config: Option<SceneCameraConfig>,
    pub(crate) spawn_index: SceneSpawnSessionIndex,
    pub(crate) triggers: Vec<SceneTriggerManifest>,
    pub(crate) assets: Vec<SceneTrackedAsset>,
    required_gate_opened: bool,
    last_progress: Option<SceneLoadProgress>,
}

impl SceneAssetLoadSession {
    pub(crate) fn new(
        scene_id: SceneId,
        session_id: SceneSessionId,
        content_version: Option<String>,
        loading_policy: SceneLoadingPolicy,
        manifest: SceneManifest,
        has_world_root: bool,
        camera_config: Option<SceneCameraConfig>,
        asset_server: &AssetServer,
    ) -> Self {
        let assets = scene_assets_from_manifest(&manifest, asset_server);
        let spawn_index = SceneSpawnSessionIndex::from_manifest_parts(
            scene_id.clone(),
            session_id.clone(),
            manifest.entry.default_spawn.clone(),
            &manifest.spawn_points,
            &manifest.anchors,
        );
        let triggers = manifest.triggers.clone();

        Self {
            scene_id,
            session_id,
            content_version,
            loading_policy,
            has_world_root,
            camera_config,
            spawn_index,
            triggers,
            assets,
            required_gate_opened: false,
            last_progress: None,
        }
    }

    pub(crate) fn progress(&self, asset_server: &AssetServer) -> SceneLoadProgress {
        let mut progress =
            SceneLoadProgress::new(self.scene_id.clone(), SceneLoadPhase::LoadingAssets);
        progress.session_id = Some(self.session_id.clone());
        progress.loading_policy = self.loading_policy;
        progress.message_key = Some("scene.loading.assets".to_string());

        for asset in &self.assets {
            if asset.required {
                progress.required_total += 1;
            } else {
                progress.optional_total += 1;
            }

            match asset.load_state(asset_server) {
                SceneTrackedAssetState::Loaded => {
                    if asset.required {
                        progress.required_loaded += 1;
                    } else {
                        progress.optional_loaded += 1;
                    }
                }
                SceneTrackedAssetState::Failed(message) => {
                    if !asset.required {
                        progress.optional_failed += 1;
                    }

                    progress.failed.push(SceneAssetLoadFailure {
                        asset_id: Some(asset.asset_id.clone()),
                        layer_id: Some(asset.layer_id.clone()),
                        path: Some(asset.path.clone()),
                        required: asset.required,
                        message,
                    });
                }
                SceneTrackedAssetState::Loading => {}
            }
        }

        if progress.required_total == progress.required_loaded {
            progress.message_key = Some("scene.loading.optional_assets".to_string());
        }

        progress
    }

    pub(crate) fn take_progress_if_changed(
        &mut self,
        asset_server: &AssetServer,
    ) -> Option<SceneLoadProgress> {
        let progress = self.progress(asset_server);
        if self.last_progress.as_ref() == Some(&progress) {
            return None;
        }

        self.last_progress = Some(progress.clone());
        Some(progress)
    }

    pub(crate) fn required_failure(
        &self,
        progress: &SceneLoadProgress,
    ) -> Option<SceneAssetLoadFailure> {
        progress
            .failed
            .iter()
            .find(|failure| failure.required)
            .cloned()
    }

    pub(crate) fn required_assets_loaded(&self, progress: &SceneLoadProgress) -> bool {
        progress.required_loaded == progress.required_total
            && self.required_failure(progress).is_none()
    }

    pub(crate) fn optional_assets_finished(&self, progress: &SceneLoadProgress) -> bool {
        progress.optional_loaded + progress.optional_failed == progress.optional_total
    }

    pub(crate) fn required_gate_opened(&self) -> bool {
        self.required_gate_opened
    }

    pub(crate) fn mark_required_gate_opened(&mut self) {
        self.required_gate_opened = true;
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SceneTrackedAsset {
    pub(crate) asset_id: SceneAssetId,
    pub(crate) layer_id: SceneLayerId,
    pub(crate) path: String,
    pub(crate) required: bool,
    handle: Option<Handle<LoadedUntypedAsset>>,
    startup_error: Option<String>,
}

impl SceneTrackedAsset {
    fn new(
        asset: &SceneAssetRef,
        layer_id: SceneLayerId,
        required: bool,
        asset_server: &AssetServer,
    ) -> Self {
        let path = asset_path_with_label(asset);
        let (handle, startup_error) = match validate_asset_relative_path(&asset.path) {
            Ok(()) => (Some(asset_server.load_untyped(path.clone())), None),
            Err(error) => (None, Some(error.to_string())),
        };

        Self {
            asset_id: asset.id.clone(),
            layer_id,
            path,
            required,
            handle,
            startup_error,
        }
    }

    fn load_state(&self, asset_server: &AssetServer) -> SceneTrackedAssetState {
        if let Some(error) = &self.startup_error {
            return SceneTrackedAssetState::Failed(error.clone());
        }

        let Some(handle) = &self.handle else {
            return SceneTrackedAssetState::Failed("asset handle was not created".to_string());
        };

        match asset_server.load_state(handle.id()) {
            bevy::asset::LoadState::Loaded => SceneTrackedAssetState::Loaded,
            bevy::asset::LoadState::Failed(error) => {
                SceneTrackedAssetState::Failed(error.to_string())
            }
            bevy::asset::LoadState::NotLoaded | bevy::asset::LoadState::Loading => {
                SceneTrackedAssetState::Loading
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum SceneTrackedAssetState {
    Loading,
    Loaded,
    Failed(String),
}

fn scene_assets_from_manifest(
    manifest: &SceneManifest,
    asset_server: &AssetServer,
) -> Vec<SceneTrackedAsset> {
    manifest
        .layers
        .iter()
        .flat_map(|layer| {
            layer.assets.iter().map(|asset| {
                SceneTrackedAsset::new(asset, layer.id.clone(), layer.required, asset_server)
            })
        })
        .collect()
}
