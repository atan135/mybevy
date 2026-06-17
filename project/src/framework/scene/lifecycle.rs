use bevy::prelude::*;
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use super::{
    camera::{
        SceneCameraConfig, SceneCameraRig, default_scene_camera_config_for_world,
        ensure_scene_camera,
    },
    command::{SceneCommand, SceneEnterRequest, SceneExitRequest, SceneSwitchRequest},
    event::{
        SceneEntered, SceneEvent, SceneExitStarted, SceneExited, SceneFailure, SceneFailureKind,
        SceneInstantiating, SceneResolving,
    },
    id::{SceneId, SceneSessionId, SceneSpawnPointId},
    loading::{
        SceneAssetLoadQueue, SceneAssetLoadSession, SceneLoadPhase, SceneLoadProgress,
        SceneLoadingPolicy,
    },
    manifest::{SceneManifest, SceneManifestLoadError},
    registry::{SceneDefinition, SceneRegistry},
    root::{SceneOwned, SceneRoot, despawn_scene_session_entities, spawn_scene_world_roots},
    spawn::{SceneSpawnLookupError, SceneSpawnRegistry, SceneSpawnSessionIndex},
    trigger::{SceneTriggerManifest, spawn_scene_triggers_from_manifest},
};

static NEXT_SCENE_SESSION_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, Resource, PartialEq)]
pub struct SceneRuntime {
    pub active: Option<SceneSessionInfo>,
    pub pending: Option<SceneSessionInfo>,
    pub state: SceneLifecycleState,
    pub last_error: Option<SceneFailure>,
}

impl Default for SceneRuntime {
    fn default() -> Self {
        Self {
            active: None,
            pending: None,
            state: SceneLifecycleState::Idle,
            last_error: None,
        }
    }
}

impl SceneRuntime {
    pub fn active(&self) -> Option<&SceneSessionInfo> {
        self.active.as_ref()
    }

    pub fn pending(&self) -> Option<&SceneSessionInfo> {
        self.pending.as_ref()
    }

    pub fn state(&self) -> SceneLifecycleState {
        self.state
    }

    pub fn last_error(&self) -> Option<&SceneFailure> {
        self.last_error.as_ref()
    }

    pub fn active_scene_id(&self) -> Option<&SceneId> {
        self.active.as_ref().map(|session| &session.scene_id)
    }

    pub fn active_session_id(&self) -> Option<&SceneSessionId> {
        self.active.as_ref().map(|session| &session.session_id)
    }

    pub fn pending_scene_id(&self) -> Option<&SceneId> {
        self.pending.as_ref().map(|session| &session.scene_id)
    }

    pub fn pending_session_id(&self) -> Option<&SceneSessionId> {
        self.pending.as_ref().map(|session| &session.session_id)
    }

    pub fn has_active(&self) -> bool {
        self.active.is_some()
    }

    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    pub fn is_active_scene(&self, scene_id: &SceneId) -> bool {
        self.active
            .as_ref()
            .is_some_and(|session| &session.scene_id == scene_id)
    }

    pub fn is_pending_scene(&self, scene_id: &SceneId) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|session| &session.scene_id == scene_id)
    }

    pub fn is_idle(&self) -> bool {
        self.state.is_idle()
    }

    pub fn is_loading(&self) -> bool {
        self.state.is_loading()
    }

    pub fn is_transitioning(&self) -> bool {
        self.state.is_transitioning()
    }

    pub fn is_failed(&self) -> bool {
        self.state.is_failed()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneSessionInfo {
    pub scene_id: SceneId,
    pub session_id: SceneSessionId,
    pub authority_mode: SceneAuthorityMode,
    pub content_version: Option<String>,
    pub spawn_point: Option<SceneSpawnPointId>,
    pub seed: Option<u64>,
    pub entered_at: Option<Duration>,
}

impl SceneSessionInfo {
    pub fn new(scene_id: impl Into<SceneId>, session_id: impl Into<SceneSessionId>) -> Self {
        Self {
            scene_id: scene_id.into(),
            session_id: session_id.into(),
            authority_mode: SceneAuthorityMode::default(),
            content_version: None,
            spawn_point: None,
            seed: None,
            entered_at: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SceneAuthorityMode {
    #[default]
    Local,
    LocalHost,
    Remote,
    External,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SceneLifecycleState {
    #[default]
    Idle,
    Resolving,
    Downloading,
    LoadingAssets,
    Instantiating,
    Activating,
    Active,
    Suspending,
    Deactivating,
    Unloading,
    Failed,
}

impl SceneLifecycleState {
    pub fn is_idle(self) -> bool {
        matches!(self, Self::Idle)
    }

    pub fn is_loading(self) -> bool {
        matches!(
            self,
            Self::Resolving | Self::Downloading | Self::LoadingAssets | Self::Instantiating
        )
    }

    pub fn is_transitioning(self) -> bool {
        matches!(
            self,
            Self::Resolving
                | Self::Downloading
                | Self::LoadingAssets
                | Self::Instantiating
                | Self::Activating
                | Self::Deactivating
                | Self::Unloading
                | Self::Suspending
        )
    }

    pub fn is_active(self) -> bool {
        matches!(self, Self::Active)
    }

    pub fn is_failed(self) -> bool {
        matches!(self, Self::Failed)
    }
}

#[derive(Clone, Debug)]
enum SceneLifecycleRequest {
    Enter(SceneEnterRequest),
    Exit(SceneExitRequest),
    Switch(SceneSwitchRequest),
}

pub(crate) fn process_scene_lifecycle_commands(
    mut commands: Commands,
    mut command_reader: MessageReader<SceneCommand>,
    registry: Res<SceneRegistry>,
    asset_server: Res<AssetServer>,
    time: Option<Res<Time>>,
    mut runtime: ResMut<SceneRuntime>,
    mut load_queue: ResMut<SceneAssetLoadQueue>,
    mut spawn_registry: ResMut<SceneSpawnRegistry>,
    scene_cameras: Query<&SceneCameraRig>,
    scene_roots: Query<(Entity, &SceneRoot)>,
    owned_entities: Query<(Entity, &SceneOwned)>,
    mut events: MessageWriter<SceneEvent>,
) {
    let request = command_reader
        .read()
        .filter_map(|command| match command {
            SceneCommand::Enter(request) => Some(SceneLifecycleRequest::Enter(request.clone())),
            SceneCommand::Exit(request) => Some(SceneLifecycleRequest::Exit(request.clone())),
            SceneCommand::Switch(request) => Some(SceneLifecycleRequest::Switch(request.clone())),
            SceneCommand::Preload(_)
            | SceneCommand::Unload(_)
            | SceneCommand::ReloadCurrent(_)
            | SceneCommand::SetLayerEnabled(_) => None,
        })
        .last();

    let Some(request) = request else {
        return;
    };

    let entered_at = time.as_ref().map(|time| time.elapsed());

    match request {
        SceneLifecycleRequest::Enter(request) => {
            enter_scene(
                &mut commands,
                &registry,
                &asset_server,
                &mut runtime,
                &mut load_queue,
                &mut spawn_registry,
                &scene_cameras,
                &scene_roots,
                &owned_entities,
                &mut events,
                request,
                entered_at,
                true,
            );
        }
        SceneLifecycleRequest::Exit(request) => {
            exit_scene(
                &mut commands,
                &mut runtime,
                &mut load_queue,
                &mut spawn_registry,
                &scene_roots,
                &owned_entities,
                &mut events,
                &request,
            );
        }
        SceneLifecycleRequest::Switch(request) => {
            // Coalescing to the last transition command avoids building an
            // intermediate session in the same frame that cannot be queried yet.
            // Switch owns the whole active scene in this minimal version, so it
            // clears the current session even if the embedded exit request has
            // stale filters.
            exit_scene(
                &mut commands,
                &mut runtime,
                &mut load_queue,
                &mut spawn_registry,
                &scene_roots,
                &owned_entities,
                &mut events,
                &SceneExitRequest::default(),
            );
            enter_scene(
                &mut commands,
                &registry,
                &asset_server,
                &mut runtime,
                &mut load_queue,
                &mut spawn_registry,
                &scene_cameras,
                &scene_roots,
                &owned_entities,
                &mut events,
                request.enter,
                entered_at,
                false,
            );
        }
    }
}

fn enter_scene(
    commands: &mut Commands,
    registry: &SceneRegistry,
    asset_server: &AssetServer,
    runtime: &mut SceneRuntime,
    load_queue: &mut SceneAssetLoadQueue,
    spawn_registry: &mut SceneSpawnRegistry,
    scene_cameras: &Query<&SceneCameraRig>,
    scene_roots: &Query<(Entity, &SceneRoot)>,
    owned_entities: &Query<(Entity, &SceneOwned)>,
    events: &mut MessageWriter<SceneEvent>,
    request: SceneEnterRequest,
    entered_at: Option<Duration>,
    replace_existing: bool,
) {
    runtime.state = SceneLifecycleState::Resolving;

    let Some(definition) = registry.get(&request.scene_id) else {
        fail_scene_transition(
            runtime,
            events,
            SceneFailure {
                kind: SceneFailureKind::SceneNotFound,
                scene_id: Some(request.scene_id),
                session_id: request.session_id,
                content_version: request.content_version,
                state: SceneLifecycleState::Resolving,
                asset_id: None,
                asset_path: None,
                message: Some("scene id is not registered".to_string()),
            },
        );
        return;
    };

    let definition = definition.clone();

    if replace_existing && (runtime.active.is_some() || runtime.pending.is_some()) {
        exit_scene(
            commands,
            runtime,
            load_queue,
            spawn_registry,
            scene_roots,
            owned_entities,
            events,
            &SceneExitRequest::default(),
        );
    }

    let mut session = session_info_from_request(&request, &definition);
    runtime.pending = Some(session.clone());
    runtime.last_error = None;

    events.write(SceneEvent::Resolving(SceneResolving {
        scene_id: session.scene_id.clone(),
        session_id: Some(session.session_id.clone()),
    }));

    let Some(manifest_path) = definition.manifest_path.clone() else {
        let progress = resolving_progress(
            &session,
            SceneLoadPhase::LoadingAssets,
            definition.loading_policy,
        );
        events.write(SceneEvent::LoadProgress(progress));

        finish_scene_enter(
            commands,
            runtime,
            events,
            definition.has_world_root,
            default_scene_camera_config_for_world(definition.has_world_root),
            SceneSpawnSessionIndex::empty(session.scene_id.clone(), session.session_id.clone()),
            Vec::new(),
            session,
            scene_cameras,
            entered_at,
            spawn_registry,
            false,
        );
        return;
    };

    match SceneManifest::load_first_package_ron(&manifest_path) {
        Ok(manifest) => {
            let manifest_scene_id = manifest.scene_id.clone();
            if manifest_scene_id != session.scene_id {
                fail_scene_transition(
                    runtime,
                    events,
                    SceneFailure {
                        kind: SceneFailureKind::ManifestParseFailed,
                        scene_id: Some(session.scene_id),
                        session_id: Some(session.session_id),
                        content_version: session.content_version,
                        state: SceneLifecycleState::Resolving,
                        asset_id: None,
                        asset_path: Some(manifest_path),
                        message: Some(format!(
                            "scene manifest scene_id {manifest_scene_id} does not match registered scene id"
                        )),
                    },
                );
                return;
            }

            if session.spawn_point.is_none() {
                session.spawn_point = manifest.entry.default_spawn.clone();
            }

            runtime.state = SceneLifecycleState::LoadingAssets;
            let loading_policy = manifest_loading_policy(&definition, &manifest);
            let camera_config = manifest_camera_config(&definition, &manifest);
            let progress =
                resolving_progress(&session, SceneLoadPhase::LoadingAssets, loading_policy);
            events.write(SceneEvent::LoadProgress(progress));

            load_queue.start(SceneAssetLoadSession::new(
                session.scene_id.clone(),
                session.session_id.clone(),
                session.content_version.clone(),
                loading_policy,
                manifest,
                definition.has_world_root,
                camera_config,
                asset_server,
            ));
        }
        Err(error) => {
            let failure = manifest_failure_from_error(&session, manifest_path, error);
            fail_scene_transition(runtime, events, failure);
        }
    }
}

fn finish_scene_enter(
    commands: &mut Commands,
    runtime: &mut SceneRuntime,
    events: &mut MessageWriter<SceneEvent>,
    has_world_root: bool,
    camera_config: Option<SceneCameraConfig>,
    spawn_index: SceneSpawnSessionIndex,
    triggers: Vec<SceneTriggerManifest>,
    mut session: SceneSessionInfo,
    scene_cameras: &Query<&SceneCameraRig>,
    entered_at: Option<Duration>,
    spawn_registry: &mut SceneSpawnRegistry,
    validate_spawn_point: bool,
) {
    runtime.state = SceneLifecycleState::LoadingAssets;
    runtime.state = SceneLifecycleState::Instantiating;
    events.write(SceneEvent::Instantiating(SceneInstantiating {
        scene_id: session.scene_id.clone(),
        session_id: session.session_id.clone(),
    }));

    runtime.state = SceneLifecycleState::Activating;

    if validate_spawn_point {
        if let Err(error) = spawn_index.validate_default_spawn() {
            fail_scene_transition(runtime, events, spawn_lookup_failure(&session, error));
            return;
        }

        if let Some(spawn_point_id) = &session.spawn_point
            && let Err(error) = spawn_index.spawn_point(spawn_point_id)
        {
            fail_scene_transition(runtime, events, spawn_lookup_failure(&session, error));
            return;
        }
    }

    if has_world_root {
        spawn_scene_world_roots(commands, &session.scene_id, &session.session_id);
    }

    if let Some(camera_config) = camera_config {
        ensure_scene_camera(commands, &session.session_id, &camera_config, scene_cameras);
    }

    spawn_scene_triggers_from_manifest(commands, &session.session_id, &triggers);

    session.entered_at = entered_at;
    spawn_registry.set_session_index(spawn_index);
    runtime.active = Some(session.clone());
    runtime.pending = None;
    runtime.state = SceneLifecycleState::Active;

    events.write(SceneEvent::Entered(SceneEntered {
        scene_id: session.scene_id,
        session_id: session.session_id,
        content_version: session.content_version,
    }));
}

fn exit_scene(
    commands: &mut Commands,
    runtime: &mut SceneRuntime,
    load_queue: &mut SceneAssetLoadQueue,
    spawn_registry: &mut SceneSpawnRegistry,
    scene_roots: &Query<(Entity, &SceneRoot)>,
    owned_entities: &Query<(Entity, &SceneOwned)>,
    events: &mut MessageWriter<SceneEvent>,
    request: &SceneExitRequest,
) -> bool {
    let Some(session) = session_for_exit(runtime, request) else {
        return false;
    };

    runtime.state = SceneLifecycleState::Deactivating;
    events.write(SceneEvent::ExitStarted(SceneExitStarted {
        scene_id: session.scene_id.clone(),
        session_id: session.session_id.clone(),
    }));

    runtime.state = SceneLifecycleState::Unloading;
    despawn_scene_session_entities(commands, &session.session_id, scene_roots, owned_entities);
    load_queue.clear_session(&session.session_id);
    spawn_registry.clear_session(&session.session_id);
    clear_runtime_session(runtime, &session.session_id);

    events.write(SceneEvent::Exited(SceneExited {
        scene_id: session.scene_id,
        session_id: session.session_id,
    }));

    runtime.state = SceneLifecycleState::Idle;
    true
}

fn session_for_exit(
    runtime: &SceneRuntime,
    request: &SceneExitRequest,
) -> Option<SceneSessionInfo> {
    let session = runtime.active.as_ref().or(runtime.pending.as_ref())?;

    if request
        .scene_id
        .as_ref()
        .is_some_and(|scene_id| scene_id != &session.scene_id)
    {
        return None;
    }

    if request
        .session_id
        .as_ref()
        .is_some_and(|session_id| session_id != &session.session_id)
    {
        return None;
    }

    Some(session.clone())
}

fn clear_runtime_session(runtime: &mut SceneRuntime, session_id: &SceneSessionId) {
    if runtime
        .active
        .as_ref()
        .is_some_and(|session| &session.session_id == session_id)
    {
        runtime.active = None;
    }

    if runtime
        .pending
        .as_ref()
        .is_some_and(|session| &session.session_id == session_id)
    {
        runtime.pending = None;
    }
}

fn session_info_from_request(
    request: &SceneEnterRequest,
    definition: &SceneDefinition,
) -> SceneSessionInfo {
    let session_id = request
        .session_id
        .clone()
        .unwrap_or_else(|| next_session_id(&request.scene_id));

    SceneSessionInfo {
        scene_id: request.scene_id.clone(),
        session_id,
        authority_mode: request.authority_mode,
        content_version: request
            .content_version
            .clone()
            .or_else(|| definition.content_version.clone()),
        spawn_point: request
            .spawn_point
            .clone()
            .or_else(|| definition.default_spawn.clone()),
        seed: request.seed,
        entered_at: None,
    }
}

fn next_session_id(scene_id: &SceneId) -> SceneSessionId {
    let next_id = NEXT_SCENE_SESSION_ID.fetch_add(1, Ordering::Relaxed);
    SceneSessionId::from(format!("{scene_id}-{next_id}"))
}

fn fail_scene_transition(
    runtime: &mut SceneRuntime,
    events: &mut MessageWriter<SceneEvent>,
    failure: SceneFailure,
) {
    runtime.pending = None;
    runtime.state = SceneLifecycleState::Failed;
    runtime.last_error = Some(failure.clone());
    events.write(SceneEvent::Failed(failure));
}

pub(crate) fn poll_scene_asset_loads(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    time: Option<Res<Time>>,
    mut runtime: ResMut<SceneRuntime>,
    mut load_queue: ResMut<SceneAssetLoadQueue>,
    mut spawn_registry: ResMut<SceneSpawnRegistry>,
    scene_cameras: Query<&SceneCameraRig>,
    mut events: MessageWriter<SceneEvent>,
) {
    let Some(session) = load_queue.current_mut() else {
        return;
    };

    let session_is_current = if session.required_gate_opened() {
        runtime
            .active
            .as_ref()
            .is_some_and(|active| active.session_id == session.session_id)
    } else {
        runtime
            .pending
            .as_ref()
            .is_some_and(|pending| pending.session_id == session.session_id)
    };

    if !session_is_current {
        load_queue.take_current();
        return;
    }

    let progress = session.progress(&asset_server);
    if let Some(progress_event) = session.take_progress_if_changed(&asset_server) {
        events.write(SceneEvent::LoadProgress(progress_event));
    }

    if !session.required_gate_opened()
        && let Some(failure) = session.required_failure(&progress)
    {
        let session_id = session.session_id.clone();
        let scene_id = session.scene_id.clone();
        let content_version = session.content_version.clone();
        load_queue.take_current();
        fail_scene_transition(
            &mut runtime,
            &mut events,
            SceneFailure {
                kind: SceneFailureKind::AssetLoadFailed,
                scene_id: Some(scene_id),
                session_id: Some(session_id),
                content_version,
                state: SceneLifecycleState::LoadingAssets,
                asset_id: failure.asset_id,
                asset_path: failure.path,
                message: Some(failure.message),
            },
        );
        return;
    }

    if !session.required_gate_opened() && !session.required_assets_loaded(&progress) {
        runtime.state = SceneLifecycleState::LoadingAssets;
        return;
    }

    if session.required_gate_opened() {
        if session.optional_assets_finished(&progress) {
            load_queue.take_current();
        }
        return;
    }

    session.mark_required_gate_opened();
    let session_load = session.clone();
    let Some(session_info) = runtime.pending.clone() else {
        load_queue.take_current();
        return;
    };

    let mut complete_progress =
        SceneLoadProgress::new(session_info.scene_id.clone(), SceneLoadPhase::Instantiating);
    complete_progress.session_id = Some(session_info.session_id.clone());
    complete_progress.loading_policy = session_load.loading_policy;
    complete_progress.required_total = progress.required_total;
    complete_progress.required_loaded = progress.required_loaded;
    complete_progress.optional_total = progress.optional_total;
    complete_progress.optional_loaded = progress.optional_loaded;
    complete_progress.optional_failed = progress.optional_failed;
    complete_progress.failed = progress.failed;
    complete_progress.message_key = Some("scene.loading.instantiating".to_string());
    events.write(SceneEvent::LoadProgress(complete_progress));

    let entered_at = time.as_ref().map(|time| time.elapsed());
    finish_scene_enter(
        &mut commands,
        &mut runtime,
        &mut events,
        session_load.has_world_root,
        session_load.camera_config,
        session_load.spawn_index,
        session_load.triggers,
        session_info,
        &scene_cameras,
        entered_at,
        &mut spawn_registry,
        true,
    );
}

fn resolving_progress(
    session: &SceneSessionInfo,
    phase: SceneLoadPhase,
    loading_policy: SceneLoadingPolicy,
) -> SceneLoadProgress {
    let mut progress = SceneLoadProgress::new(session.scene_id.clone(), phase);
    progress.session_id = Some(session.session_id.clone());
    progress.loading_policy = loading_policy;
    progress.message_key = Some(
        match phase {
            SceneLoadPhase::Resolving => "scene.loading.resolving",
            SceneLoadPhase::Downloading => "scene.loading.downloading",
            SceneLoadPhase::LoadingAssets => "scene.loading.assets",
            SceneLoadPhase::Instantiating => "scene.loading.instantiating",
            SceneLoadPhase::Activating => "scene.loading.activating",
            SceneLoadPhase::Complete => "scene.loading.complete",
        }
        .to_string(),
    );
    progress
}

fn manifest_loading_policy(
    definition: &SceneDefinition,
    manifest: &SceneManifest,
) -> SceneLoadingPolicy {
    if manifest.entry.loading_policy != SceneLoadingPolicy::default() {
        manifest.entry.loading_policy
    } else {
        definition.loading_policy
    }
}

fn manifest_camera_config(
    definition: &SceneDefinition,
    manifest: &SceneManifest,
) -> Option<SceneCameraConfig> {
    manifest
        .entry
        .camera
        .as_ref()
        .map(|camera| camera.config().clone())
        .or_else(|| default_scene_camera_config_for_world(definition.has_world_root))
}

fn manifest_failure_from_error(
    session: &SceneSessionInfo,
    manifest_path: String,
    error: SceneManifestLoadError,
) -> SceneFailure {
    let kind = match &error {
        SceneManifestLoadError::UnsafeManifestPath(_)
        | SceneManifestLoadError::ManifestNotFound(_)
        | SceneManifestLoadError::ReadFailed { .. } => SceneFailureKind::ManifestLoadFailed,
        SceneManifestLoadError::ParseFailed { .. } => SceneFailureKind::ManifestParseFailed,
        SceneManifestLoadError::ValidationFailed(error)
            if matches!(
                error,
                super::manifest::SceneManifestError::UnsupportedVersion { .. }
            ) =>
        {
            SceneFailureKind::ManifestVersionUnsupported
        }
        SceneManifestLoadError::ValidationFailed(error)
            if matches!(
                error,
                super::manifest::SceneManifestError::DefaultSpawnMissing(_)
            ) =>
        {
            SceneFailureKind::SpawnPointMissing
        }
        SceneManifestLoadError::ValidationFailed(_) => SceneFailureKind::ManifestParseFailed,
    };

    SceneFailure {
        kind,
        scene_id: Some(session.scene_id.clone()),
        session_id: Some(session.session_id.clone()),
        content_version: session.content_version.clone(),
        state: SceneLifecycleState::Resolving,
        asset_id: None,
        asset_path: Some(manifest_path),
        message: Some(error.to_string()),
    }
}

fn spawn_lookup_failure(session: &SceneSessionInfo, error: SceneSpawnLookupError) -> SceneFailure {
    let kind = match error {
        SceneSpawnLookupError::SessionMissing { .. }
        | SceneSpawnLookupError::DefaultSpawnMissing { .. }
        | SceneSpawnLookupError::SpawnPointMissing { .. } => SceneFailureKind::SpawnPointMissing,
        SceneSpawnLookupError::AnchorMissing { .. } => SceneFailureKind::SceneInstanceFailed,
    };

    SceneFailure {
        kind,
        scene_id: Some(session.scene_id.clone()),
        session_id: Some(session.session_id.clone()),
        content_version: session.content_version.clone(),
        state: SceneLifecycleState::Activating,
        asset_id: None,
        asset_path: None,
        message: Some(error.to_string()),
    }
}
