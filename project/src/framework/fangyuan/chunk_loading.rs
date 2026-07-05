use bevy::prelude::*;
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt,
};

use super::{
    FangyuanChunkBounds, FangyuanChunkBudgetSummary, FangyuanChunkManifest,
    FangyuanChunkManifestEntry, FangyuanChunkSource, FangyuanChunkValidationError,
};

#[derive(Clone, Debug, Default, Resource)]
pub struct FangyuanChunkSourceLibrary {
    sources: HashMap<String, FangyuanChunkSource>,
}

impl FangyuanChunkSourceLibrary {
    pub fn from_sources(sources: impl IntoIterator<Item = FangyuanChunkSource>) -> Self {
        let mut library = Self::default();
        for source in sources {
            library.insert(source);
        }
        library
    }

    pub fn insert(&mut self, source: FangyuanChunkSource) {
        self.sources.insert(source.id.clone(), source);
    }

    pub fn get(&self, chunk_id: &str) -> Option<&FangyuanChunkSource> {
        self.sources.get(chunk_id)
    }

    pub fn contains(&self, chunk_id: &str) -> bool {
        self.sources.contains_key(chunk_id)
    }

    pub fn len(&self) -> usize {
        self.sources.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }

    pub fn clear(&mut self) {
        self.sources.clear();
    }
}

#[derive(Clone, Debug, Default, Resource)]
pub struct FangyuanChunkManifestRuntime {
    manifest: Option<FangyuanChunkManifest>,
}

impl FangyuanChunkManifestRuntime {
    pub fn from_manifest(manifest: FangyuanChunkManifest) -> Self {
        Self {
            manifest: Some(manifest),
        }
    }

    pub fn set(&mut self, manifest: FangyuanChunkManifest) {
        self.manifest = Some(manifest);
    }

    pub fn get(&self) -> Option<&FangyuanChunkManifest> {
        self.manifest.as_ref()
    }

    pub fn clear(&mut self) {
        self.manifest = None;
    }
}

#[derive(Clone, Debug, Default, Resource)]
pub struct FangyuanChunkRuntime {
    loaded: HashMap<String, FangyuanLoadedChunk>,
    last_failure: Option<FangyuanChunkFailure>,
    last_event: Option<FangyuanChunkRuntimeEvent>,
    sequence: u64,
}

impl FangyuanChunkRuntime {
    pub fn load_chunk<'a>(
        &mut self,
        source: &FangyuanChunkSource,
        available_prefab_ids: impl IntoIterator<Item = &'a str>,
        root_entity: Option<Entity>,
    ) -> FangyuanChunkRuntimeEvent {
        self.sequence += 1;
        let previous = self.loaded.get(&source.id).cloned();
        let event = match self.try_load_chunk(source, available_prefab_ids, root_entity, false) {
            Ok(chunk) => {
                self.loaded.insert(source.id.clone(), chunk.clone());
                self.last_failure = None;
                FangyuanChunkRuntimeEvent::loaded(self.sequence, chunk, previous.is_some())
            }
            Err(error) => {
                if let Some(previous) = previous {
                    self.loaded.insert(source.id.clone(), previous);
                }
                self.record_failure(source.id.clone(), error)
            }
        };
        self.last_event = Some(event.clone());
        event
    }

    pub fn reload_chunk<'a>(
        &mut self,
        source: &FangyuanChunkSource,
        available_prefab_ids: impl IntoIterator<Item = &'a str>,
        root_entity: Option<Entity>,
    ) -> FangyuanChunkRuntimeEvent {
        self.sequence += 1;
        let previous = self.loaded.get(&source.id).cloned();
        match self.try_load_chunk(source, available_prefab_ids, root_entity, true) {
            Ok(chunk) => {
                self.loaded.insert(source.id.clone(), chunk.clone());
                self.last_failure = None;
                let event = FangyuanChunkRuntimeEvent::reloaded(
                    self.sequence,
                    chunk,
                    previous.map(|chunk| chunk.status),
                );
                self.last_event = Some(event.clone());
                event
            }
            Err(error) => {
                if let Some(previous) = previous {
                    self.loaded.insert(source.id.clone(), previous);
                }
                let event = self.record_failure(source.id.clone(), error);
                self.last_event = Some(event.clone());
                event
            }
        }
    }

    pub fn unload_chunk(&mut self, chunk_id: impl Into<String>) -> FangyuanChunkRuntimeEvent {
        self.sequence += 1;
        let chunk_id = chunk_id.into();
        let event = match self.loaded.remove(&chunk_id) {
            Some(chunk) => {
                self.last_failure = None;
                FangyuanChunkRuntimeEvent::unloaded(self.sequence, chunk)
            }
            None => self.record_failure(
                chunk_id,
                FangyuanChunkLoadError::ChunkNotLoaded {
                    operation: FangyuanChunkOperation::Unload,
                },
            ),
        };
        self.last_event = Some(event.clone());
        event
    }

    pub fn clear(&mut self, reason: FangyuanChunkClearReason) -> FangyuanChunkRuntimeEvent {
        self.sequence += 1;
        let cleared_chunk_ids = self.loaded.keys().cloned().collect::<Vec<_>>();
        self.loaded.clear();
        self.last_failure = None;
        let event = FangyuanChunkRuntimeEvent {
            sequence: self.sequence,
            kind: FangyuanChunkRuntimeEventKind::Cleared,
            chunk_id: None,
            status: FangyuanChunkLoadStatus::Cleared,
            visible_objects: 0,
            failure: None,
            duplicate: false,
            previous_status: None,
            cleared_chunk_ids,
            clear_reason: Some(reason),
        };
        self.last_event = Some(event.clone());
        event
    }

    pub fn select_nearby_chunks(
        &mut self,
        manifest: &FangyuanChunkManifest,
        position: [f32; 3],
        radius: f32,
    ) -> FangyuanChunkSelection {
        self.sequence += 1;
        let selection = select_fangyuan_chunks_near_position(
            position,
            radius,
            self.loaded_chunk_ids().collect::<Vec<_>>(),
            &manifest.chunks,
        );
        self.last_event = Some(FangyuanChunkRuntimeEvent {
            sequence: self.sequence,
            kind: FangyuanChunkRuntimeEventKind::SelectionUpdated,
            chunk_id: None,
            status: FangyuanChunkLoadStatus::Selecting,
            visible_objects: self.visible_object_count(),
            failure: None,
            duplicate: false,
            previous_status: None,
            cleared_chunk_ids: Vec::new(),
            clear_reason: None,
        });
        selection
    }

    pub fn loaded_chunk(&self, chunk_id: &str) -> Option<&FangyuanLoadedChunk> {
        self.loaded.get(chunk_id)
    }

    pub fn loaded_chunk_ids(&self) -> impl Iterator<Item = &str> {
        self.loaded.keys().map(String::as_str)
    }

    pub fn loaded_chunk_count(&self) -> usize {
        self.loaded.len()
    }

    pub fn visible_object_count(&self) -> usize {
        self.loaded
            .values()
            .map(FangyuanLoadedChunk::visible_objects)
            .sum()
    }

    pub fn last_failure(&self) -> Option<&FangyuanChunkFailure> {
        self.last_failure.as_ref()
    }

    pub fn last_event(&self) -> Option<&FangyuanChunkRuntimeEvent> {
        self.last_event.as_ref()
    }

    pub fn debug_summary(&self) -> FangyuanChunkDebugSummary {
        FangyuanChunkDebugSummary::from_runtime(self)
    }

    fn set_chunk_root_entity(&mut self, chunk_id: &str, root_entity: Entity) {
        if let Some(chunk) = self.loaded.get_mut(chunk_id) {
            chunk.root_entity = Some(root_entity);
        }
    }

    fn chunk_root_entity(&self, chunk_id: &str) -> Option<Entity> {
        self.loaded
            .get(chunk_id)
            .and_then(|chunk| chunk.root_entity)
    }

    fn chunk_root_entities(&self) -> Vec<Entity> {
        self.loaded
            .values()
            .filter_map(|chunk| chunk.root_entity)
            .collect()
    }

    fn try_load_chunk<'a>(
        &self,
        source: &FangyuanChunkSource,
        available_prefab_ids: impl IntoIterator<Item = &'a str>,
        root_entity: Option<Entity>,
        allow_existing: bool,
    ) -> Result<FangyuanLoadedChunk, FangyuanChunkLoadError> {
        if !allow_existing && self.loaded.contains_key(&source.id) {
            return Err(FangyuanChunkLoadError::DuplicateLoad);
        }

        source
            .validate_against_prefab_ids(available_prefab_ids)
            .map_err(FangyuanChunkLoadError::ValidationFailed)?;

        Ok(FangyuanLoadedChunk::from_source(source, root_entity))
    }

    fn record_failure(
        &mut self,
        chunk_id: String,
        error: FangyuanChunkLoadError,
    ) -> FangyuanChunkRuntimeEvent {
        let failure = FangyuanChunkFailure {
            chunk_id: chunk_id.clone(),
            code: error.code(),
            reason: error.reason(),
        };
        self.last_failure = Some(failure.clone());
        FangyuanChunkRuntimeEvent::failed(self.sequence, chunk_id, error, failure)
    }
}

#[derive(Clone, Debug, Component, PartialEq, Eq)]
pub struct FangyuanChunkRoot {
    pub chunk_id: String,
}

impl FangyuanChunkRoot {
    pub fn new(chunk_id: impl Into<String>) -> Self {
        Self {
            chunk_id: chunk_id.into(),
        }
    }
}

#[derive(Clone, Debug, Message, PartialEq)]
pub enum FangyuanChunkCommand {
    Load {
        chunk_id: String,
        root_entity: Option<Entity>,
    },
    Unload {
        chunk_id: String,
    },
    Reload {
        chunk_id: String,
        root_entity: Option<Entity>,
    },
    Clear {
        reason: FangyuanChunkClearReason,
    },
    SelectNearPosition {
        position: [f32; 3],
        radius: f32,
    },
}

impl FangyuanChunkCommand {
    pub fn load(chunk_id: impl Into<String>, root_entity: Option<Entity>) -> Self {
        Self::Load {
            chunk_id: chunk_id.into(),
            root_entity,
        }
    }

    pub fn unload(chunk_id: impl Into<String>) -> Self {
        Self::Unload {
            chunk_id: chunk_id.into(),
        }
    }

    pub fn reload(chunk_id: impl Into<String>, root_entity: Option<Entity>) -> Self {
        Self::Reload {
            chunk_id: chunk_id.into(),
            root_entity,
        }
    }

    pub fn clear(reason: FangyuanChunkClearReason) -> Self {
        Self::Clear { reason }
    }

    pub fn select_near_position(position: [f32; 3], radius: f32) -> Self {
        Self::SelectNearPosition { position, radius }
    }
}

#[derive(Clone, Debug, Message, PartialEq)]
pub struct FangyuanChunkEvent {
    pub event: FangyuanChunkRuntimeEvent,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanChunkRuntimeEvent {
    pub sequence: u64,
    pub kind: FangyuanChunkRuntimeEventKind,
    pub chunk_id: Option<String>,
    pub status: FangyuanChunkLoadStatus,
    pub visible_objects: usize,
    pub failure: Option<FangyuanChunkFailure>,
    pub duplicate: bool,
    pub previous_status: Option<FangyuanChunkLoadStatus>,
    pub cleared_chunk_ids: Vec<String>,
    pub clear_reason: Option<FangyuanChunkClearReason>,
}

impl FangyuanChunkRuntimeEvent {
    fn loaded(sequence: u64, chunk: FangyuanLoadedChunk, duplicate: bool) -> Self {
        let visible_objects = chunk.visible_objects();
        Self {
            sequence,
            kind: FangyuanChunkRuntimeEventKind::Loaded,
            chunk_id: Some(chunk.id),
            status: chunk.status,
            visible_objects,
            failure: None,
            duplicate,
            previous_status: None,
            cleared_chunk_ids: Vec::new(),
            clear_reason: None,
        }
    }

    fn reloaded(
        sequence: u64,
        chunk: FangyuanLoadedChunk,
        previous_status: Option<FangyuanChunkLoadStatus>,
    ) -> Self {
        let visible_objects = chunk.visible_objects();
        Self {
            sequence,
            kind: FangyuanChunkRuntimeEventKind::Reloaded,
            chunk_id: Some(chunk.id),
            status: chunk.status,
            visible_objects,
            failure: None,
            duplicate: false,
            previous_status,
            cleared_chunk_ids: Vec::new(),
            clear_reason: None,
        }
    }

    fn unloaded(sequence: u64, chunk: FangyuanLoadedChunk) -> Self {
        Self {
            sequence,
            kind: FangyuanChunkRuntimeEventKind::Unloaded,
            chunk_id: Some(chunk.id),
            status: FangyuanChunkLoadStatus::Unloaded,
            visible_objects: 0,
            failure: None,
            duplicate: false,
            previous_status: Some(chunk.status),
            cleared_chunk_ids: Vec::new(),
            clear_reason: None,
        }
    }

    fn failed(
        sequence: u64,
        chunk_id: String,
        error: FangyuanChunkLoadError,
        failure: FangyuanChunkFailure,
    ) -> Self {
        Self {
            sequence,
            kind: FangyuanChunkRuntimeEventKind::Failed,
            chunk_id: Some(chunk_id),
            status: FangyuanChunkLoadStatus::Fallback,
            visible_objects: 0,
            duplicate: matches!(error, FangyuanChunkLoadError::DuplicateLoad),
            previous_status: None,
            cleared_chunk_ids: Vec::new(),
            clear_reason: None,
            failure: Some(failure),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanChunkRuntimeEventKind {
    Loaded,
    Unloaded,
    Reloaded,
    Cleared,
    Failed,
    SelectionUpdated,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FangyuanChunkLoadStatus {
    #[default]
    Pending,
    Selecting,
    Loaded,
    Unloaded,
    Cleared,
    Fallback,
}

impl FangyuanChunkLoadStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Selecting => "selecting",
            Self::Loaded => "loaded",
            Self::Unloaded => "unloaded",
            Self::Cleared => "cleared",
            Self::Fallback => "fallback",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanChunkFailure {
    pub chunk_id: String,
    pub code: &'static str,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanLoadedChunk {
    pub id: String,
    pub status: FangyuanChunkLoadStatus,
    pub bounds: FangyuanChunkBounds,
    pub budget: FangyuanChunkBudgetSummary,
    pub root_entity: Option<Entity>,
    pub prefab_instance_count: usize,
    pub tiandao_ref_count: usize,
    pub static_decoration_count: usize,
}

impl FangyuanLoadedChunk {
    pub fn from_source(source: &FangyuanChunkSource, root_entity: Option<Entity>) -> Self {
        Self {
            id: source.id.clone(),
            status: FangyuanChunkLoadStatus::Loaded,
            bounds: source.bounds,
            budget: source.budget,
            root_entity,
            prefab_instance_count: source.prefab_instances.len(),
            tiandao_ref_count: source.tiandao_refs.len(),
            static_decoration_count: source.static_decorations.len(),
        }
    }

    pub fn visible_objects(&self) -> usize {
        self.prefab_instance_count + self.tiandao_ref_count + self.static_decoration_count
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanChunkOperation {
    Load,
    Unload,
    Reload,
    Clear,
    SelectNearPosition,
}

impl FangyuanChunkOperation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Load => "load",
            Self::Unload => "unload",
            Self::Reload => "reload",
            Self::Clear => "clear",
            Self::SelectNearPosition => "select_near_position",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanChunkClearReason {
    Manual,
    SceneExit,
    Reload,
    Fallback,
}

impl Default for FangyuanChunkClearReason {
    fn default() -> Self {
        Self::Manual
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum FangyuanChunkLoadError {
    ChunkNotFound,
    ChunkNotLoaded { operation: FangyuanChunkOperation },
    DuplicateLoad,
    ValidationFailed(FangyuanChunkValidationError),
    InvalidSelectionRadius { radius: f32 },
}

impl FangyuanChunkLoadError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::ChunkNotFound => "chunk_not_found",
            Self::ChunkNotLoaded { .. } => "chunk_not_loaded",
            Self::DuplicateLoad => "duplicate_load",
            Self::ValidationFailed(error) => error.code(),
            Self::InvalidSelectionRadius { .. } => "invalid_selection_radius",
        }
    }

    pub fn reason(&self) -> String {
        match self {
            Self::ChunkNotFound => "chunk source is not present in the local library".to_string(),
            Self::ChunkNotLoaded { operation } => {
                format!("cannot {} a chunk that is not loaded", operation.as_str())
            }
            Self::DuplicateLoad => "chunk is already loaded; use reload to replace it".to_string(),
            Self::ValidationFailed(error) => error.to_string(),
            Self::InvalidSelectionRadius { radius } => {
                format!("selection radius {radius} must be finite and non-negative")
            }
        }
    }
}

impl fmt::Display for FangyuanChunkLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "fangyuan chunk load error [{}]: {}",
            self.code(),
            self.reason()
        )
    }
}

impl Error for FangyuanChunkLoadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ValidationFailed(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FangyuanChunkSelection {
    pub load: Vec<String>,
    pub unload: Vec<String>,
    pub keep: Vec<String>,
}

impl FangyuanChunkSelection {
    pub fn is_empty(&self) -> bool {
        self.load.is_empty() && self.unload.is_empty() && self.keep.is_empty()
    }

    pub fn desired_chunk_ids(&self) -> Vec<String> {
        let mut desired = self.load.clone();
        desired.extend(self.keep.clone());
        desired.sort();
        desired
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FangyuanChunkDebugSummary {
    pub loaded_chunks: usize,
    pub loaded_chunk_ids: Vec<String>,
    pub visible_objects: usize,
    pub load_state: String,
    pub failure_reason: String,
}

impl FangyuanChunkDebugSummary {
    pub fn from_runtime(runtime: &FangyuanChunkRuntime) -> Self {
        let mut loaded_chunk_ids = runtime
            .loaded_chunk_ids()
            .map(str::to_string)
            .collect::<Vec<_>>();
        loaded_chunk_ids.sort();
        let load_state = runtime
            .last_event()
            .map(|event| event.status.as_str())
            .unwrap_or(FangyuanChunkLoadStatus::Pending.as_str())
            .to_string();
        let failure_reason = runtime
            .last_failure()
            .map(|failure| format!("{}:{}", failure.chunk_id, failure.code))
            .unwrap_or_else(|| "-".to_string());

        Self {
            loaded_chunks: loaded_chunk_ids.len(),
            loaded_chunk_ids,
            visible_objects: runtime.visible_object_count(),
            load_state,
            failure_reason,
        }
    }

    pub fn loaded_ids_label(&self, max_chars: usize) -> String {
        compact_fangyuan_chunk_text(&self.loaded_chunk_ids.join(","), "-", max_chars)
    }

    pub fn failure_label(&self, max_chars: usize) -> String {
        compact_fangyuan_chunk_text(&self.failure_reason, "-", max_chars)
    }
}

pub fn select_fangyuan_chunks_near_position<'a>(
    position: [f32; 3],
    radius: f32,
    loaded_chunk_ids: impl IntoIterator<Item = &'a str>,
    entries: &[FangyuanChunkManifestEntry],
) -> FangyuanChunkSelection {
    if !radius.is_finite() || radius < 0.0 {
        let mut loaded = loaded_chunk_ids
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        loaded.sort();
        return FangyuanChunkSelection {
            load: Vec::new(),
            unload: loaded,
            keep: Vec::new(),
        };
    }

    let desired = entries
        .iter()
        .filter(|entry| chunk_bounds_intersects_sphere(&entry.bounds, position, radius))
        .map(|entry| entry.id.as_str())
        .collect::<HashSet<_>>();
    let loaded = loaded_chunk_ids.into_iter().collect::<HashSet<_>>();

    let mut load = desired
        .difference(&loaded)
        .map(|chunk_id| (*chunk_id).to_string())
        .collect::<Vec<_>>();
    let mut unload = loaded
        .difference(&desired)
        .map(|chunk_id| (*chunk_id).to_string())
        .collect::<Vec<_>>();
    let mut keep = desired
        .intersection(&loaded)
        .map(|chunk_id| (*chunk_id).to_string())
        .collect::<Vec<_>>();

    load.sort();
    unload.sort();
    keep.sort();

    FangyuanChunkSelection { load, unload, keep }
}

pub fn process_fangyuan_chunk_commands(
    mut entity_commands: Commands,
    mut commands: MessageReader<FangyuanChunkCommand>,
    manifest: Option<Res<FangyuanChunkManifestRuntime>>,
    source_library: Res<FangyuanChunkSourceLibrary>,
    available_prefabs: Res<FangyuanChunkAvailablePrefabs>,
    mut runtime: ResMut<FangyuanChunkRuntime>,
    mut events: MessageWriter<FangyuanChunkEvent>,
) {
    for command in commands.read() {
        match command {
            FangyuanChunkCommand::Load {
                chunk_id,
                root_entity,
            } => {
                let event = if let Some(source) = source_library.get(chunk_id) {
                    let event = runtime.load_chunk(
                        source,
                        available_prefabs.ids.iter().map(String::as_str),
                        None,
                    );
                    if matches!(event.kind, FangyuanChunkRuntimeEventKind::Loaded)
                        && let Some(parent) = *root_entity
                    {
                        let chunk_root =
                            spawn_fangyuan_chunk_root(&mut entity_commands, parent, chunk_id);
                        runtime.set_chunk_root_entity(chunk_id, chunk_root);
                    }
                    event
                } else {
                    runtime.sequence += 1;
                    let event = runtime
                        .record_failure(chunk_id.clone(), FangyuanChunkLoadError::ChunkNotFound);
                    runtime.last_event = Some(event.clone());
                    event
                };
                events.write(FangyuanChunkEvent { event });
            }
            FangyuanChunkCommand::Unload { chunk_id } => {
                let chunk_root = runtime.chunk_root_entity(chunk_id);
                let event = runtime.unload_chunk(chunk_id.clone());
                if matches!(event.kind, FangyuanChunkRuntimeEventKind::Unloaded)
                    && let Some(chunk_root) = chunk_root
                {
                    entity_commands.entity(chunk_root).try_despawn();
                }
                events.write(FangyuanChunkEvent { event });
            }
            FangyuanChunkCommand::Reload {
                chunk_id,
                root_entity,
            } => {
                let event = if let Some(source) = source_library.get(chunk_id) {
                    let previous_root = runtime.chunk_root_entity(chunk_id);
                    let event = runtime.reload_chunk(
                        source,
                        available_prefabs.ids.iter().map(String::as_str),
                        previous_root,
                    );
                    if matches!(event.kind, FangyuanChunkRuntimeEventKind::Reloaded)
                        && previous_root.is_none()
                        && let Some(parent) = *root_entity
                    {
                        let chunk_root =
                            spawn_fangyuan_chunk_root(&mut entity_commands, parent, chunk_id);
                        runtime.set_chunk_root_entity(chunk_id, chunk_root);
                    }
                    event
                } else {
                    runtime.sequence += 1;
                    let event = runtime
                        .record_failure(chunk_id.clone(), FangyuanChunkLoadError::ChunkNotFound);
                    runtime.last_event = Some(event.clone());
                    event
                };
                events.write(FangyuanChunkEvent { event });
            }
            FangyuanChunkCommand::Clear { reason } => {
                let chunk_roots = runtime.chunk_root_entities();
                let event = runtime.clear(*reason);
                for chunk_root in chunk_roots {
                    entity_commands.entity(chunk_root).try_despawn();
                }
                events.write(FangyuanChunkEvent { event });
            }
            FangyuanChunkCommand::SelectNearPosition { position, radius } => {
                if !radius.is_finite() || *radius < 0.0 {
                    runtime.sequence += 1;
                    let event = runtime.record_failure(
                        "selection".to_string(),
                        FangyuanChunkLoadError::InvalidSelectionRadius { radius: *radius },
                    );
                    runtime.last_event = Some(event.clone());
                    events.write(FangyuanChunkEvent { event });
                    continue;
                }

                let Some(manifest) = manifest
                    .as_deref()
                    .and_then(FangyuanChunkManifestRuntime::get)
                else {
                    runtime.sequence += 1;
                    let event = runtime.record_failure(
                        "manifest".to_string(),
                        FangyuanChunkLoadError::ChunkNotFound,
                    );
                    runtime.last_event = Some(event.clone());
                    events.write(FangyuanChunkEvent { event });
                    continue;
                };

                let selection = runtime.select_nearby_chunks(manifest, *position, *radius);
                if let Some(event) = runtime.last_event().cloned() {
                    events.write(FangyuanChunkEvent { event });
                }
                for chunk_id in selection.unload {
                    let chunk_root = runtime.chunk_root_entity(&chunk_id);
                    let event = runtime.unload_chunk(chunk_id);
                    if matches!(event.kind, FangyuanChunkRuntimeEventKind::Unloaded)
                        && let Some(chunk_root) = chunk_root
                    {
                        entity_commands.entity(chunk_root).try_despawn();
                    }
                    events.write(FangyuanChunkEvent { event });
                }
                for chunk_id in selection.load {
                    let event = if let Some(source) = source_library.get(&chunk_id) {
                        runtime.load_chunk(
                            source,
                            available_prefabs.ids.iter().map(String::as_str),
                            None,
                        )
                    } else {
                        runtime.sequence += 1;
                        let event =
                            runtime.record_failure(chunk_id, FangyuanChunkLoadError::ChunkNotFound);
                        runtime.last_event = Some(event.clone());
                        event
                    };
                    events.write(FangyuanChunkEvent { event });
                }
            }
        }
    }
}

fn spawn_fangyuan_chunk_root(commands: &mut Commands, parent: Entity, chunk_id: &str) -> Entity {
    let chunk_root = commands
        .spawn((
            FangyuanChunkRoot::new(chunk_id),
            Transform::default(),
            Name::new(format!("FangyuanChunkRoot({chunk_id})")),
        ))
        .id();
    commands.entity(parent).add_child(chunk_root);
    chunk_root
}

#[derive(Clone, Debug, Default, Resource, PartialEq, Eq)]
pub struct FangyuanChunkAvailablePrefabs {
    ids: HashSet<String>,
}

impl FangyuanChunkAvailablePrefabs {
    pub fn from_ids<'a>(ids: impl IntoIterator<Item = &'a str>) -> Self {
        Self {
            ids: ids.into_iter().map(str::to_string).collect(),
        }
    }

    pub fn insert(&mut self, id: impl Into<String>) {
        self.ids.insert(id.into());
    }

    pub fn contains(&self, id: &str) -> bool {
        self.ids.contains(id)
    }

    pub fn clear(&mut self) {
        self.ids.clear();
    }
}

fn chunk_bounds_intersects_sphere(
    bounds: &FangyuanChunkBounds,
    position: [f32; 3],
    radius: f32,
) -> bool {
    let radius_squared = radius * radius;
    let distance_squared = position
        .into_iter()
        .enumerate()
        .map(|(axis, value)| {
            if value < bounds.min[axis] {
                let delta = bounds.min[axis] - value;
                delta * delta
            } else if value > bounds.max[axis] {
                let delta = value - bounds.max[axis];
                delta * delta
            } else {
                0.0
            }
        })
        .sum::<f32>();
    distance_squared <= radius_squared
}

fn compact_fangyuan_chunk_text(value: &str, fallback: &str, max_chars: usize) -> String {
    let value = value.trim();
    if value.is_empty() {
        return fallback.to_string();
    }
    let char_count = value.chars().count();
    if char_count <= max_chars {
        return value.to_string();
    }

    let keep = max_chars.saturating_sub(3);
    let tail = value
        .chars()
        .rev()
        .take(keep)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("...{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::message::MessageCursor;

    #[test]
    fn fangyuan_chunk_loading_loads_and_unloads_chunks() {
        let mut runtime = FangyuanChunkRuntime::default();
        let source = valid_source("home_chunk_a", -8.0, 8.0, "stone_block");

        let event = runtime.load_chunk(&source, ["stone_block"], None);

        assert_eq!(event.kind, FangyuanChunkRuntimeEventKind::Loaded);
        assert_eq!(event.status, FangyuanChunkLoadStatus::Loaded);
        assert_eq!(runtime.loaded_chunk_count(), 1);
        assert_eq!(runtime.visible_object_count(), 1);
        assert_eq!(runtime.last_failure(), None);

        let event = runtime.unload_chunk("home_chunk_a");

        assert_eq!(event.kind, FangyuanChunkRuntimeEventKind::Unloaded);
        assert_eq!(event.status, FangyuanChunkLoadStatus::Unloaded);
        assert_eq!(runtime.loaded_chunk_count(), 0);
    }

    #[test]
    fn fangyuan_chunk_loading_rejects_duplicate_load_without_dropping_existing_chunk() {
        let mut runtime = FangyuanChunkRuntime::default();
        let source = valid_source("home_chunk_a", -8.0, 8.0, "stone_block");
        runtime.load_chunk(&source, ["stone_block"], None);

        let event = runtime.load_chunk(&source, ["stone_block"], None);

        assert_eq!(event.kind, FangyuanChunkRuntimeEventKind::Failed);
        assert_eq!(event.status, FangyuanChunkLoadStatus::Fallback);
        assert!(event.duplicate);
        assert_eq!(
            event.failure.as_ref().map(|failure| failure.code),
            Some("duplicate_load")
        );
        assert_eq!(runtime.loaded_chunk_count(), 1);
        assert!(runtime.loaded_chunk("home_chunk_a").is_some());
    }

    #[test]
    fn fangyuan_chunk_loading_rolls_back_failed_reload() {
        let mut runtime = FangyuanChunkRuntime::default();
        let mut source = valid_source("home_chunk_a", -8.0, 8.0, "stone_block");
        runtime.load_chunk(&source, ["stone_block"], None);

        source.prefab_instances[0].prefab = "missing_prefab".to_string();
        let event = runtime.reload_chunk(&source, ["stone_block"], None);

        assert_eq!(event.kind, FangyuanChunkRuntimeEventKind::Failed);
        assert_eq!(
            event.failure.as_ref().map(|failure| failure.code),
            Some("missing_prefab_ref")
        );
        assert_eq!(runtime.loaded_chunk_count(), 1);
        assert_eq!(
            runtime
                .loaded_chunk("home_chunk_a")
                .unwrap()
                .prefab_instance_count,
            1
        );
    }

    #[test]
    fn fangyuan_chunk_loading_reports_missing_prefab_and_keeps_runtime_empty() {
        let mut runtime = FangyuanChunkRuntime::default();
        let mut source = valid_source("home_chunk_a", -8.0, 8.0, "stone_block");
        source.prefab_instances[0].prefab = "missing_prefab".to_string();

        let event = runtime.load_chunk(&source, ["stone_block"], None);

        assert_eq!(event.kind, FangyuanChunkRuntimeEventKind::Failed);
        assert_eq!(
            runtime.last_failure().map(|failure| failure.code),
            Some("missing_prefab_ref")
        );
        assert_eq!(runtime.loaded_chunk_count(), 0);
    }

    #[test]
    fn fangyuan_chunk_loading_reports_validation_failure_and_fallback_status() {
        let mut runtime = FangyuanChunkRuntime::default();
        let mut source = valid_source("home_chunk_a", -8.0, 8.0, "stone_block");
        source.bounds.max[0] = source.bounds.min[0];

        let event = runtime.load_chunk(&source, ["stone_block"], None);

        assert_eq!(event.status, FangyuanChunkLoadStatus::Fallback);
        assert_eq!(
            event.failure.as_ref().map(|failure| failure.code),
            Some("invalid_chunk_bounds")
        );
        assert_eq!(runtime.debug_summary().load_state, "fallback");
    }

    #[test]
    fn fangyuan_chunk_loading_selects_nearby_chunks_with_jitter_protection() {
        let manifest = valid_manifest();
        let loaded = ["home_chunk_a", "home_chunk_far"];

        let selection =
            select_fangyuan_chunks_near_position([1.0, 1.0, 0.0], 7.0, loaded, &manifest.chunks);

        assert_eq!(selection.load, vec!["home_chunk_b"]);
        assert_eq!(selection.keep, vec!["home_chunk_a"]);
        assert_eq!(selection.unload, vec!["home_chunk_far"]);

        let selection = select_fangyuan_chunks_near_position(
            [1.1, 1.0, 0.0],
            7.0,
            ["home_chunk_a", "home_chunk_b"],
            &manifest.chunks,
        );

        assert!(selection.load.is_empty());
        assert_eq!(selection.keep, vec!["home_chunk_a", "home_chunk_b"]);
        assert!(selection.unload.is_empty());
    }

    #[test]
    fn fangyuan_chunk_loading_invalid_selection_radius_unloads_as_fallback() {
        let manifest = valid_manifest();
        let selection = select_fangyuan_chunks_near_position(
            [0.0, 0.0, 0.0],
            -1.0,
            ["home_chunk_b", "home_chunk_a"],
            &manifest.chunks,
        );

        assert!(selection.load.is_empty());
        assert!(selection.keep.is_empty());
        assert_eq!(selection.unload, vec!["home_chunk_a", "home_chunk_b"]);
    }

    #[test]
    fn fangyuan_chunk_loading_clear_handles_scene_exit() {
        let mut runtime = FangyuanChunkRuntime::default();
        runtime.load_chunk(
            &valid_source("home_chunk_a", -8.0, 8.0, "stone_block"),
            ["stone_block"],
            None,
        );

        let event = runtime.clear(FangyuanChunkClearReason::SceneExit);

        assert_eq!(event.kind, FangyuanChunkRuntimeEventKind::Cleared);
        assert_eq!(event.status, FangyuanChunkLoadStatus::Cleared);
        assert_eq!(
            event.clear_reason,
            Some(FangyuanChunkClearReason::SceneExit)
        );
        assert_eq!(event.cleared_chunk_ids, vec!["home_chunk_a"]);
        assert_eq!(runtime.loaded_chunk_count(), 0);
    }

    #[test]
    fn fangyuan_chunk_loading_processes_messages_and_missing_sources() {
        let mut app = App::new();
        app.add_message::<FangyuanChunkCommand>()
            .add_message::<FangyuanChunkEvent>()
            .insert_resource(FangyuanChunkSourceLibrary::from_sources([valid_source(
                "home_chunk_a",
                -8.0,
                8.0,
                "stone_block",
            )]))
            .insert_resource(FangyuanChunkAvailablePrefabs::from_ids(["stone_block"]))
            .init_resource::<FangyuanChunkRuntime>()
            .add_systems(Update, process_fangyuan_chunk_commands);

        app.world_mut()
            .write_message(FangyuanChunkCommand::load("home_chunk_a", None));
        app.world_mut()
            .write_message(FangyuanChunkCommand::load("missing_chunk", None));
        app.update();

        let events = read_events(app.world());
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event.kind, FangyuanChunkRuntimeEventKind::Loaded);
        assert_eq!(events[1].event.kind, FangyuanChunkRuntimeEventKind::Failed);
        assert_eq!(
            events[1].event.failure.as_ref().map(|failure| failure.code),
            Some("chunk_not_found")
        );
    }

    #[test]
    fn fangyuan_chunk_loading_attaches_and_cleans_chunk_roots() {
        let mut app = App::new();
        app.add_message::<FangyuanChunkCommand>()
            .add_message::<FangyuanChunkEvent>()
            .insert_resource(FangyuanChunkSourceLibrary::from_sources([valid_source(
                "home_chunk_a",
                -8.0,
                8.0,
                "stone_block",
            )]))
            .insert_resource(FangyuanChunkAvailablePrefabs::from_ids(["stone_block"]))
            .init_resource::<FangyuanChunkRuntime>()
            .add_systems(Update, process_fangyuan_chunk_commands);
        let parent = app.world_mut().spawn_empty().id();

        app.world_mut()
            .write_message(FangyuanChunkCommand::load("home_chunk_a", Some(parent)));
        app.update();

        let root = app
            .world()
            .resource::<FangyuanChunkRuntime>()
            .loaded_chunk("home_chunk_a")
            .and_then(|chunk| chunk.root_entity)
            .expect("loaded chunk should record spawned root");
        assert!(app.world().get::<FangyuanChunkRoot>(root).is_some());

        app.world_mut()
            .write_message(FangyuanChunkCommand::unload("home_chunk_a"));
        app.update();

        assert!(app.world().get_entity(root).is_err());
        assert_eq!(
            app.world()
                .resource::<FangyuanChunkRuntime>()
                .loaded_chunk_count(),
            0
        );
    }

    #[test]
    fn fangyuan_chunk_loading_select_near_position_messages_load_and_unload() {
        let mut app = App::new();
        app.add_message::<FangyuanChunkCommand>()
            .add_message::<FangyuanChunkEvent>()
            .insert_resource(FangyuanChunkSourceLibrary::from_sources([
                valid_source("home_chunk_a", -8.0, 8.0, "stone_block"),
                valid_source("home_chunk_b", 8.0, 24.0, "stone_block"),
                valid_source("home_chunk_far", 80.0, 96.0, "stone_block"),
            ]))
            .insert_resource(FangyuanChunkManifestRuntime::from_manifest(valid_manifest()))
            .insert_resource(FangyuanChunkAvailablePrefabs::from_ids(["stone_block"]))
            .init_resource::<FangyuanChunkRuntime>()
            .add_systems(Update, process_fangyuan_chunk_commands);
        app.world_mut()
            .write_message(FangyuanChunkCommand::load("home_chunk_far", None));
        app.update();

        app.world_mut()
            .write_message(FangyuanChunkCommand::select_near_position(
                [1.0, 1.0, 0.0],
                7.0,
            ));
        app.update();

        let events = read_events(app.world());
        assert!(
            events
                .iter()
                .any(|event| event.event.kind == FangyuanChunkRuntimeEventKind::SelectionUpdated)
        );

        let mut ids = app
            .world()
            .resource::<FangyuanChunkRuntime>()
            .loaded_chunk_ids()
            .map(str::to_string)
            .collect::<Vec<_>>();
        ids.sort();
        assert_eq!(ids, vec!["home_chunk_a", "home_chunk_b"]);
    }

    #[test]
    fn fangyuan_chunk_loading_debug_summary_reports_loaded_visible_and_failure() {
        let mut runtime = FangyuanChunkRuntime::default();
        runtime.load_chunk(
            &valid_source("home_chunk_a", -8.0, 8.0, "stone_block"),
            ["stone_block"],
            None,
        );
        runtime.load_chunk(
            &valid_source("home_chunk_a", -8.0, 8.0, "stone_block"),
            ["stone_block"],
            None,
        );

        let summary = runtime.debug_summary();

        assert_eq!(summary.loaded_chunks, 1);
        assert_eq!(summary.visible_objects, 1);
        assert_eq!(summary.load_state, "fallback");
        assert_eq!(summary.failure_reason, "home_chunk_a:duplicate_load");
        assert_eq!(summary.loaded_ids_label(64), "home_chunk_a");
    }

    fn read_events(world: &World) -> Vec<FangyuanChunkEvent> {
        let messages = world.resource::<Messages<FangyuanChunkEvent>>();
        let mut cursor = MessageCursor::default();
        cursor.read(messages).cloned().collect()
    }

    fn valid_manifest() -> FangyuanChunkManifest {
        FangyuanChunkManifest {
            version: super::super::FANGYUAN_CHUNK_VERSION.to_string(),
            name: "home_chunks".to_string(),
            description: String::new(),
            world_id: Some("home.world".to_string()),
            chunks: vec![
                manifest_entry("home_chunk_a", -8.0, 8.0),
                manifest_entry("home_chunk_b", 8.0, 24.0),
                manifest_entry("home_chunk_far", 80.0, 96.0),
            ],
        }
    }

    fn manifest_entry(id: &str, min_x: f32, max_x: f32) -> FangyuanChunkManifestEntry {
        FangyuanChunkManifestEntry {
            id: id.to_string(),
            bounds: FangyuanChunkBounds::new([min_x, 0.0, -8.0], [max_x, 6.0, 8.0]),
            region: valid_region(),
            dev_ron: None,
            bin: None,
            hash: None,
            data_version: None,
            budget: FangyuanChunkBudgetSummary {
                prefab_instance_count: 1,
                tiandao_ref_count: 0,
                static_decoration_count: 0,
                total_ref_count: 1,
                prefab_cost: 5,
                tiandao_cost: 0,
                static_decoration_cost: 0,
                total_cost: 5,
            },
        }
    }

    fn valid_source(id: &str, min_x: f32, max_x: f32, prefab: &str) -> FangyuanChunkSource {
        FangyuanChunkSource {
            version: super::super::FANGYUAN_CHUNK_VERSION.to_string(),
            id: id.to_string(),
            name: id.to_string(),
            description: String::new(),
            bounds: FangyuanChunkBounds::new([min_x, 0.0, -8.0], [max_x, 6.0, 8.0]),
            region: valid_region(),
            prefab_instances: vec![super::super::FangyuanChunkPrefabInstanceRef {
                id: "stone_a".to_string(),
                prefab: prefab.to_string(),
                transform: super::super::FangyuanChunkTransform::new(
                    [min_x + 1.0, 0.0, 0.0],
                    [1.0, 1.0, 1.0],
                ),
                budget_cost: 5,
            }],
            tiandao_refs: Vec::new(),
            static_decorations: Vec::new(),
            bin: None,
            hash: None,
            data_version: None,
            budget: FangyuanChunkBudgetSummary {
                prefab_instance_count: 1,
                tiandao_ref_count: 0,
                static_decoration_count: 0,
                total_ref_count: 1,
                prefab_cost: 5,
                tiandao_cost: 0,
                static_decoration_cost: 0,
                total_cost: 5,
            },
        }
    }

    fn valid_region() -> super::super::FangyuanChunkRegionMetadata {
        super::super::FangyuanChunkRegionMetadata {
            region_id: "home.default".to_string(),
            layer: "ground".to_string(),
            tags: Vec::new(),
        }
    }
}
