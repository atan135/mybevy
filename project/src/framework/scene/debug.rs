use bevy::prelude::*;

use super::{
    command::SceneEnterRequest,
    event::SceneFailure,
    id::{SceneId, SceneLayerId, SceneSessionId, SceneSpawnPointId},
    lifecycle::{SceneLifecycleState, SceneRuntime},
    root::{
        SceneEntityCounts, SceneLayerRoot, SceneLayerState, SceneOwned, SceneRoot,
        SceneRuntimeRoot, count_scene_entities, count_scene_entities_for_session,
    },
};

const ENV_SCENE_DEBUG: &str = "MYBEVY_SCENE_DEBUG";
const ENV_SCENE_LOG_LIFECYCLE: &str = "MYBEVY_SCENE_LOG_LIFECYCLE";
const ENV_SCENE_SLOW_LOADING_SECONDS: &str = "MYBEVY_SCENE_SLOW_LOADING_SECONDS";
const ENV_SCENE_SIMULATE_FAILURE: &str = "MYBEVY_SCENE_SIMULATE_FAILURE";
const ENV_START_SCENE: &str = "MYBEVY_START_SCENE";
const ENV_START_SPAWN: &str = "MYBEVY_START_SPAWN";

#[derive(Clone, Debug, Resource, PartialEq)]
pub struct SceneDebugConfig {
    pub enabled: bool,
    pub log_lifecycle: bool,
    pub simulate_slow_loading_seconds: Option<f32>,
    pub simulate_failure: Option<SceneDebugFailure>,
    pub startup: SceneDebugStartup,
}

impl Default for SceneDebugConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            log_lifecycle: false,
            simulate_slow_loading_seconds: None,
            simulate_failure: None,
            startup: SceneDebugStartup::default(),
        }
    }
}

impl SceneDebugConfig {
    pub fn from_env() -> Self {
        Self::from_env_reader(|key| std::env::var(key).ok())
    }

    pub fn from_env_reader(mut read: impl FnMut(&str) -> Option<String>) -> Self {
        let enabled = read_bool(&mut read, ENV_SCENE_DEBUG).unwrap_or(false);
        Self {
            enabled,
            log_lifecycle: read_bool(&mut read, ENV_SCENE_LOG_LIFECYCLE).unwrap_or(enabled),
            simulate_slow_loading_seconds: read_positive_f32(
                &mut read,
                ENV_SCENE_SLOW_LOADING_SECONDS,
            ),
            simulate_failure: read(&ENV_SCENE_SIMULATE_FAILURE)
                .and_then(|value| SceneDebugFailure::parse(value.as_str())),
            startup: SceneDebugStartup::from_env_reader(&mut read),
        }
    }

    pub fn is_active(&self) -> bool {
        self.enabled || self.startup.has_scene()
    }

    pub fn should_simulate_failure(&self, failure: SceneDebugFailure) -> bool {
        self.enabled && self.simulate_failure == Some(failure)
    }

    pub fn simulated_loading_delay(&self) -> Option<std::time::Duration> {
        self.enabled
            .then_some(self.simulate_slow_loading_seconds)
            .flatten()
            .map(std::time::Duration::from_secs_f32)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SceneDebugStartup {
    pub scene_id: Option<SceneId>,
    pub spawn_point: Option<SceneSpawnPointId>,
}

impl SceneDebugStartup {
    pub fn from_env_reader(mut read: impl FnMut(&str) -> Option<String>) -> Self {
        let scene_id = read_non_empty(&mut read, ENV_START_SCENE).map(SceneId::from);
        let spawn_point = read_non_empty(&mut read, ENV_START_SPAWN).map(SceneSpawnPointId::from);

        Self {
            scene_id,
            spawn_point,
        }
    }

    pub fn has_scene(&self) -> bool {
        self.scene_id.is_some()
    }

    pub fn enter_request(&self) -> Option<SceneEnterRequest> {
        let mut request = SceneEnterRequest::new(self.scene_id.clone()?);
        request.spawn_point = self.spawn_point.clone();
        Some(request)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneDebugFailure {
    ManifestLoad,
    AssetLoad,
    CameraSetup,
}

impl SceneDebugFailure {
    pub fn parse(value: &str) -> Option<Self> {
        match normalized_env_value(value).as_str() {
            "manifest_load" | "manifest-load" | "manifest" => Some(Self::ManifestLoad),
            "asset_load" | "asset-load" | "asset" => Some(Self::AssetLoad),
            "camera_setup" | "camera-setup" | "camera" => Some(Self::CameraSetup),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SceneDebugSnapshot {
    pub scene_id: Option<SceneId>,
    pub session_id: Option<SceneSessionId>,
    pub state: SceneLifecycleState,
    pub entity_counts: SceneEntityCounts,
    pub scene_owned_entities: usize,
    pub layer_count: usize,
    pub layers: Vec<SceneLayerDebugInfo>,
    pub last_error: Option<SceneFailure>,
}

impl SceneDebugSnapshot {
    pub fn from_runtime(runtime: &SceneRuntime) -> Self {
        let session = runtime.active().or(runtime.pending());

        Self {
            scene_id: session.map(|session| session.scene_id.clone()),
            session_id: session.map(|session| session.session_id.clone()),
            state: runtime.state(),
            last_error: runtime.last_error().cloned(),
            ..Default::default()
        }
    }

    pub fn with_entity_counts(mut self, entity_counts: SceneEntityCounts) -> Self {
        self.scene_owned_entities = entity_counts.total_scene_owned;
        self.layer_count = entity_counts.layer_roots;
        self.entity_counts = entity_counts;
        self
    }

    pub fn with_layers(mut self, layers: Vec<SceneLayerDebugInfo>) -> Self {
        self.layer_count = layers.len();
        self.layers = layers;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneLayerDebugInfo {
    pub layer_id: SceneLayerId,
    pub session_id: SceneSessionId,
    pub state: SceneLayerState,
    pub required: bool,
}

impl From<&SceneLayerRoot> for SceneLayerDebugInfo {
    fn from(root: &SceneLayerRoot) -> Self {
        Self {
            layer_id: root.layer_id.clone(),
            session_id: root.session_id.clone(),
            state: root.state,
            required: root.required,
        }
    }
}

pub type SceneDebugDiagnostics = SceneDebugSnapshot;

pub fn scene_debug_snapshot(
    runtime: &SceneRuntime,
    owned_entities: &Query<&SceneOwned>,
    scene_roots: &Query<&SceneRoot>,
    layer_roots: &Query<&SceneLayerRoot>,
    runtime_roots: &Query<&SceneRuntimeRoot>,
) -> SceneDebugSnapshot {
    let session_id = runtime
        .active_session_id()
        .or(runtime.pending_session_id())
        .cloned();
    let entity_counts = session_id
        .as_ref()
        .map(|session_id| {
            count_scene_entities_for_session(
                session_id,
                owned_entities,
                scene_roots,
                layer_roots,
                runtime_roots,
            )
        })
        .unwrap_or_else(|| {
            count_scene_entities(owned_entities, scene_roots, layer_roots, runtime_roots)
        });
    let layers = scene_layer_debug_info(session_id.as_ref(), layer_roots);

    SceneDebugSnapshot::from_runtime(runtime)
        .with_entity_counts(entity_counts)
        .with_layers(layers)
}

pub fn scene_layer_debug_info(
    session_id: Option<&SceneSessionId>,
    layer_roots: &Query<&SceneLayerRoot>,
) -> Vec<SceneLayerDebugInfo> {
    layer_roots
        .iter()
        .filter(|root| session_id.is_none_or(|session_id| root.is_session(session_id)))
        .map(SceneLayerDebugInfo::from)
        .collect()
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

fn read_positive_f32(read: &mut impl FnMut(&str) -> Option<String>, key: &str) -> Option<f32> {
    read(key)
        .and_then(|value| value.trim().parse::<f32>().ok())
        .filter(|value| value.is_finite() && *value > 0.0)
}

fn read_non_empty(read: &mut impl FnMut(&str) -> Option<String>, key: &str) -> Option<String> {
    read(key)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn normalized_env_value(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_reader<'a>(values: &'a [(&'a str, &'a str)]) -> impl FnMut(&str) -> Option<String> + 'a {
        |key| {
            values
                .iter()
                .find_map(|(name, value)| (*name == key).then_some((*value).to_string()))
        }
    }

    #[test]
    fn debug_config_defaults_to_disabled_without_env() {
        let config = SceneDebugConfig::from_env_reader(env_reader(&[]));

        assert_eq!(config, SceneDebugConfig::default());
        assert!(!config.is_active());
    }

    #[test]
    fn debug_config_reads_env_flags_and_simulation_bounds() {
        let config = SceneDebugConfig::from_env_reader(env_reader(&[
            (ENV_SCENE_DEBUG, "on"),
            (ENV_SCENE_LOG_LIFECYCLE, "false"),
            (ENV_SCENE_SLOW_LOADING_SECONDS, "1.5"),
            (ENV_SCENE_SIMULATE_FAILURE, "asset_load"),
            (ENV_START_SCENE, "arena"),
            (ENV_START_SPAWN, "entry"),
        ]));

        assert!(config.enabled);
        assert!(!config.log_lifecycle);
        assert_eq!(config.simulate_slow_loading_seconds, Some(1.5));
        assert_eq!(config.simulate_failure, Some(SceneDebugFailure::AssetLoad));
        assert_eq!(config.startup.scene_id, Some(SceneId::from("arena")));
        assert_eq!(
            config.startup.spawn_point,
            Some(SceneSpawnPointId::from("entry"))
        );
        assert!(config.should_simulate_failure(SceneDebugFailure::AssetLoad));
        assert_eq!(
            config.simulated_loading_delay(),
            Some(std::time::Duration::from_millis(1500))
        );
    }

    #[test]
    fn startup_env_ignores_empty_scene_id() {
        let startup = SceneDebugStartup::from_env_reader(env_reader(&[
            (ENV_START_SCENE, "  "),
            (ENV_START_SPAWN, "entry"),
        ]));

        assert!(!startup.has_scene());
        assert!(startup.enter_request().is_none());
    }
}
