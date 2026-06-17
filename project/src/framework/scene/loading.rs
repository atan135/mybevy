use super::id::{SceneAssetId, SceneId, SceneLayerId, SceneSessionId};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SceneLoadingPolicy {
    None,
    Spinner,
    Progress,
    #[default]
    Blocking,
    NonBlocking,
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
            failed: Vec::new(),
            message_key: None,
        }
    }

    pub fn required_fraction(&self) -> Option<f32> {
        (self.required_total > 0).then(|| self.required_loaded as f32 / self.required_total as f32)
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
