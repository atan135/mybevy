use bevy::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;

use super::id::{
    SceneAssetId, SceneChunkId, SceneLayerId, SceneRegionId, SceneSessionId, SceneZoneId,
};

/// Chunk metadata is static partition data only.
///
/// Dynamic/runtime entities are not owned by chunks and are not auto-despawned
/// when a chunk becomes cold. Gameplay code must move or despawn dynamic
/// entities through explicit game-layer systems.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SceneChunkManifest {
    pub zone_id: SceneZoneId,
    pub region_id: SceneRegionId,
    pub chunk_id: SceneChunkId,
    pub bounds: SceneChunkBounds,
    #[serde(default)]
    pub neighbors: Vec<SceneChunkId>,
    #[serde(default)]
    pub layers: SceneChunkLayerRefs,
    #[serde(default)]
    pub assets: Vec<SceneChunkAssetRef>,
    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub memory_budget_bytes: Option<u64>,
}

impl SceneChunkManifest {
    pub fn new(
        zone_id: impl Into<SceneZoneId>,
        region_id: impl Into<SceneRegionId>,
        chunk_id: impl Into<SceneChunkId>,
        bounds: SceneChunkBounds,
    ) -> Self {
        Self {
            zone_id: zone_id.into(),
            region_id: region_id.into(),
            chunk_id: chunk_id.into(),
            bounds,
            neighbors: Vec::new(),
            layers: SceneChunkLayerRefs::default(),
            assets: Vec::new(),
            priority: 0,
            memory_budget_bytes: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct SceneChunkBounds {
    pub min: Vec3,
    pub max: Vec3,
}

impl SceneChunkBounds {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    pub fn contains_point(&self, point: Vec3) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
            && point.z >= self.min.z
            && point.z <= self.max.z
    }

    pub fn is_valid(&self) -> bool {
        self.min.x <= self.max.x && self.min.y <= self.max.y && self.min.z <= self.max.z
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct SceneChunkLayerRefs {
    #[serde(default)]
    pub required: Vec<SceneLayerId>,
    #[serde(default)]
    pub optional: Vec<SceneLayerId>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct SceneChunkAssetRef {
    pub id: SceneAssetId,
    pub path: String,
    #[serde(default)]
    pub required: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SceneChunkLoadState {
    #[default]
    Cold,
    Warm,
    Active,
    Loading,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneStreamingCommand {
    pub enabled: bool,
}

impl SceneStreamingCommand {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

#[derive(Clone, Debug, Resource, PartialEq)]
pub struct SceneStreamingState {
    enabled: bool,
    chunks: HashMap<SceneChunkId, SceneRegisteredChunk>,
    chunk_states: HashMap<SceneChunkId, SceneChunkLoadState>,
}

impl Default for SceneStreamingState {
    fn default() -> Self {
        Self {
            enabled: false,
            chunks: HashMap::new(),
            chunk_states: HashMap::new(),
        }
    }
}

impl SceneStreamingState {
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn register_chunks(
        &mut self,
        session_id: SceneSessionId,
        chunks: impl IntoIterator<Item = SceneChunkManifest>,
    ) {
        for chunk in chunks {
            let chunk_id = chunk.chunk_id.clone();
            self.chunks.insert(
                chunk_id.clone(),
                SceneRegisteredChunk {
                    session_id: session_id.clone(),
                    manifest: chunk,
                },
            );
            self.chunk_states
                .entry(chunk_id)
                .or_insert(SceneChunkLoadState::Cold);
        }
    }

    pub fn clear_session(&mut self, session_id: &SceneSessionId) {
        self.chunks.retain(|chunk_id, chunk| {
            let keep = &chunk.session_id != session_id;
            if !keep {
                self.chunk_states.remove(chunk_id);
            }
            keep
        });
    }

    pub fn clear(&mut self) {
        self.chunks.clear();
        self.chunk_states.clear();
    }

    pub fn chunk_at_position(&self, position: Vec3) -> Option<&SceneChunkManifest> {
        self.chunks
            .values()
            .find(|chunk| chunk.manifest.bounds.contains_point(position))
            .map(|chunk| &chunk.manifest)
    }

    pub fn chunk_bounds(&self, chunk_id: &SceneChunkId) -> Option<SceneChunkBounds> {
        self.chunks.get(chunk_id).map(|chunk| chunk.manifest.bounds)
    }

    pub fn chunk_state(&self, chunk_id: &SceneChunkId) -> Option<SceneChunkLoadState> {
        self.chunk_states.get(chunk_id).copied()
    }

    pub fn set_chunk_state(&mut self, chunk_id: &SceneChunkId, state: SceneChunkLoadState) -> bool {
        let Some(current) = self.chunk_states.get_mut(chunk_id) else {
            return false;
        };

        *current = state;
        true
    }

    pub fn chunks_by_state(&self, state: SceneChunkLoadState) -> Vec<&SceneChunkManifest> {
        self.chunks
            .iter()
            .filter_map(|(chunk_id, chunk)| {
                (self.chunk_states.get(chunk_id).copied() == Some(state)).then_some(&chunk.manifest)
            })
            .collect()
    }

    pub fn active_chunks(&self) -> Vec<&SceneChunkManifest> {
        self.chunks_by_state(SceneChunkLoadState::Active)
    }

    pub fn warm_chunks(&self) -> Vec<&SceneChunkManifest> {
        self.chunks_by_state(SceneChunkLoadState::Warm)
    }

    pub fn cold_chunks(&self) -> Vec<&SceneChunkManifest> {
        self.chunks_by_state(SceneChunkLoadState::Cold)
    }
}

#[derive(Clone, Debug, PartialEq)]
struct SceneRegisteredChunk {
    session_id: SceneSessionId,
    manifest: SceneChunkManifest,
}

#[derive(Clone, Debug, Resource, PartialEq)]
pub struct SceneStreamingDriverConfig {
    pub enabled: bool,
    pub active_radius: f32,
    pub warm_radius: f32,
}

impl Default for SceneStreamingDriverConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            active_radius: 0.0,
            warm_radius: 0.0,
        }
    }
}

/// Reserved system entry for future camera/player-position driven chunk radius updates.
pub(crate) fn update_scene_streaming_driver(
    _config: Res<SceneStreamingDriverConfig>,
    _state: ResMut<SceneStreamingState>,
) {
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(id: &str, min: Vec3, max: Vec3) -> SceneChunkManifest {
        SceneChunkManifest::new("zone", "region", id, SceneChunkBounds::new(min, max))
    }

    #[test]
    fn bounds_contains_point() {
        let bounds = SceneChunkBounds::new(Vec3::ZERO, Vec3::new(10.0, 10.0, 10.0));

        assert!(bounds.contains_point(Vec3::new(5.0, 5.0, 5.0)));
        assert!(bounds.contains_point(Vec3::new(10.0, 10.0, 10.0)));
        assert!(!bounds.contains_point(Vec3::new(11.0, 5.0, 5.0)));
    }

    #[test]
    fn state_defaults_disabled_and_can_be_enabled() {
        let mut state = SceneStreamingState::default();
        assert!(!state.enabled());

        let command = SceneStreamingCommand::new(true);
        state.set_enabled(command.enabled);

        assert!(state.enabled());
    }

    #[test]
    fn state_queries_chunk_by_position_and_state() {
        let mut state = SceneStreamingState::default();
        let active_id = SceneChunkId::from("active");
        let warm_id = SceneChunkId::from("warm");
        state.register_chunks(
            SceneSessionId::from("session"),
            vec![
                chunk("active", Vec3::ZERO, Vec3::new(10.0, 10.0, 10.0)),
                chunk(
                    "warm",
                    Vec3::new(10.1, 0.0, 0.0),
                    Vec3::new(20.0, 10.0, 10.0),
                ),
            ],
        );

        assert_eq!(
            state
                .chunk_at_position(Vec3::new(2.0, 2.0, 2.0))
                .map(|chunk| chunk.chunk_id.as_str()),
            Some("active")
        );
        assert_eq!(state.chunk_bounds(&active_id).unwrap().max.x, 10.0);

        assert!(state.set_chunk_state(&active_id, SceneChunkLoadState::Active));
        assert!(state.set_chunk_state(&warm_id, SceneChunkLoadState::Warm));

        assert_eq!(state.active_chunks().len(), 1);
        assert_eq!(state.warm_chunks().len(), 1);
        assert_eq!(state.cold_chunks().len(), 0);
    }
}
