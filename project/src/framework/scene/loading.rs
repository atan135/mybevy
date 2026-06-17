use bevy::{asset::LoadedUntypedAsset, prelude::*};
use serde::{Deserialize, Deserializer};

use super::{
    id::{SceneAssetId, SceneId, SceneLayerId, SceneSessionId},
    manifest::{
        SceneAssetRef, SceneManifest, asset_path_with_label, normalize_manifest_token,
        validate_asset_relative_path,
    },
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

#[derive(Clone, Debug, PartialEq)]
pub struct SceneLoadProgress {
    pub scene_id: SceneId,
    pub session_id: Option<SceneSessionId>,
    pub phase: SceneLoadPhase,
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
    pub(crate) has_world_root: bool,
    pub(crate) assets: Vec<SceneTrackedAsset>,
    required_gate_opened: bool,
    last_progress: Option<SceneLoadProgress>,
}

impl SceneAssetLoadSession {
    pub(crate) fn new(
        scene_id: SceneId,
        session_id: SceneSessionId,
        content_version: Option<String>,
        manifest: SceneManifest,
        has_world_root: bool,
        asset_server: &AssetServer,
    ) -> Self {
        let assets = scene_assets_from_manifest(&manifest, asset_server);
        Self {
            scene_id,
            session_id,
            content_version,
            has_world_root,
            assets,
            required_gate_opened: false,
            last_progress: None,
        }
    }

    pub(crate) fn progress(&self, asset_server: &AssetServer) -> SceneLoadProgress {
        let mut progress =
            SceneLoadProgress::new(self.scene_id.clone(), SceneLoadPhase::LoadingAssets);
        progress.session_id = Some(self.session_id.clone());
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
