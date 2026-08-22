use bevy::prelude::*;

use crate::framework::scene::{
    SceneDebugSnapshot, SceneEvent, SceneFailure, SceneLayerDebugInfo, SceneLayerState,
    SceneLifecycleState, SceneLoadPhase, SceneLoadProgress, SceneLoadingPolicy, SceneRuntime,
    scene_debug_snapshot,
};

use super::{
    LivePreviewClock, LivePreviewCollectionBuffer, LivePreviewSnapshotHub, LivePreviewTimeline,
    LivePreviewTimelineEvent, LivePreviewTimelineSeverity, LivePreviewTimelineType, PreviewSection,
    SceneLayerPreview, ScenePreviewState, StablePreviewId,
};

const SCENE_DEBUG_SAMPLE_INTERVAL_MS: u64 = 500;

/// A controlled adapter boundary for future game-specific scene summaries.
/// Concrete gameplay modules may provide a short, non-sensitive summary without
/// adding their fields to the framework-owned scene DTO.
pub trait ScenePreviewAdapter: Send + Sync + 'static {
    fn summary(&self, _snapshot: &SceneDebugSnapshot) -> Option<String> {
        None
    }
}

#[derive(Clone, Debug, Default, Resource)]
pub(crate) struct ScenePreviewCollectorState {
    last_state: Option<ScenePreviewState>,
    revision: u64,
    debug_snapshot: Option<SceneDebugSnapshot>,
    last_debug_sample_ms: Option<u64>,
    progress: Option<SceneProgressSummary>,
    event_error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SceneProgressSummary {
    scene_id: StablePreviewId,
    session_id: Option<StablePreviewId>,
    phase: String,
    loading_policy: String,
    required_total: u64,
    required_loaded: u64,
    optional_total: u64,
    optional_loaded: u64,
    optional_failed: u64,
    message_key: Option<String>,
}

impl From<&SceneLoadProgress> for SceneProgressSummary {
    fn from(progress: &SceneLoadProgress) -> Self {
        Self {
            scene_id: stable_id(&progress.scene_id),
            session_id: progress.session_id.as_ref().map(stable_id),
            phase: scene_load_phase_label(progress.phase).to_owned(),
            loading_policy: scene_loading_policy_label(progress.loading_policy).to_owned(),
            required_total: progress.required_total as u64,
            required_loaded: progress.required_loaded as u64,
            optional_total: progress.optional_total as u64,
            optional_loaded: progress.optional_loaded as u64,
            optional_failed: progress.optional_failed as u64,
            message_key: progress.message_key.clone(),
        }
    }
}

pub(crate) fn collect_scene_preview(
    clock: Res<LivePreviewClock>,
    runtime: Option<Res<SceneRuntime>>,
    owned_entities: Query<&crate::framework::scene::SceneOwned>,
    scene_roots: Query<&crate::framework::scene::SceneRoot>,
    layer_roots: Query<&crate::framework::scene::SceneLayerRoot>,
    runtime_roots: Query<&crate::framework::scene::SceneRuntimeRoot>,
    mut scene_events: Option<MessageReader<SceneEvent>>,
    mut collector_state: ResMut<ScenePreviewCollectorState>,
    mut buffer: ResMut<LivePreviewCollectionBuffer>,
    hub: Res<LivePreviewSnapshotHub>,
    mut timeline: ResMut<LivePreviewTimeline>,
) {
    if let Some(events) = scene_events.as_mut() {
        for event in events.read() {
            apply_scene_event(&mut collector_state, event);
            record_scene_event(
                event,
                collector_state.last_state.as_ref(),
                &mut timeline,
                clock.monotonic_ms(),
                next_publish_sequence(hub.read().sequence),
            );
        }
    }

    clear_scene_debug_cache_if_runtime_missing(&mut collector_state, runtime.as_deref());
    let now_ms = clock.monotonic_ms();
    if scene_debug_sample_due(collector_state.last_debug_sample_ms, now_ms)
        && let Some(runtime) = runtime.as_deref()
    {
        collector_state.debug_snapshot = Some(scene_debug_snapshot(
            runtime,
            &owned_entities,
            &scene_roots,
            &layer_roots,
            &runtime_roots,
        ));
        collector_state.last_debug_sample_ms = Some(now_ms);
    }
    let debug_snapshot = runtime.as_deref().and_then(|runtime| {
        collector_state.debug_snapshot.as_ref().map(|cached| {
            let metadata = SceneDebugSnapshot::from_runtime(runtime);
            let same_session = cached.session_id == metadata.session_id;
            let mut snapshot = cached.clone();
            snapshot.scene_id = metadata.scene_id;
            snapshot.session_id = metadata.session_id;
            snapshot.state = metadata.state;
            snapshot.last_error = metadata.last_error;
            if !same_session {
                snapshot.entity_counts = Default::default();
                snapshot.scene_owned_entities = 0;
                snapshot.layer_count = 0;
                snapshot.layers.clear();
            }
            snapshot
        })
    });
    let next_state = collect_scene_state(
        runtime.as_deref(),
        debug_snapshot.as_ref(),
        collector_state.progress.as_ref(),
        collector_state.event_error.as_deref(),
    );

    if collector_state.last_state.as_ref() == Some(&next_state) && collector_state.revision != 0 {
        return;
    }

    collector_state.last_state = Some(next_state.clone());
    collector_state.revision = collector_state.revision.saturating_add(1).max(1);
    buffer.set_scene(PreviewSection::available(
        collector_state.revision,
        next_state,
    ));
}

fn collect_scene_state(
    runtime: Option<&SceneRuntime>,
    debug_snapshot: Option<&SceneDebugSnapshot>,
    progress: Option<&SceneProgressSummary>,
    event_error: Option<&str>,
) -> ScenePreviewState {
    let active = runtime.and_then(|runtime| runtime.active.as_ref());
    let pending = runtime.and_then(|runtime| runtime.pending.as_ref());
    let ready = runtime.and_then(|runtime| runtime.ready.as_ref());
    let metadata = active.or(pending);
    let ready_matches_metadata = metadata
        .zip(ready)
        .filter(|(session, ready)| session.session_id == ready.session_id)
        .map(|(_, ready)| ready);

    let mut layers = debug_snapshot
        .map(|snapshot| {
            snapshot
                .layers
                .iter()
                .map(scene_layer_preview)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    layers.sort_by(|left, right| {
        left.id
            .cmp(&right.id)
            .then(left.session_id.cmp(&right.session_id))
    });
    let layer_ids = layers.iter().map(|layer| layer.id.clone()).collect();

    let scene_status = runtime.map(|runtime| scene_status(runtime).to_owned());
    let recent_error = debug_snapshot
        .and_then(|snapshot| snapshot.last_error.as_ref())
        .map(SceneFailure::message_key)
        .map(str::to_owned)
        .or_else(|| event_error.map(str::to_owned));

    ScenePreviewState {
        active_scene_id: active.map(|session| stable_id(&session.scene_id)),
        active_session_id: active.map(|session| stable_id(&session.session_id)),
        pending_scene_id: pending
            .map(|session| stable_id(&session.scene_id))
            .or_else(|| progress.map(|progress| progress.scene_id.clone())),
        pending_session_id: pending
            .map(|session| stable_id(&session.session_id))
            .or_else(|| progress.and_then(|progress| progress.session_id.clone())),
        ready_scene_id: ready.map(|ready| stable_id(&ready.scene_id)),
        ready_session_id: ready.map(|ready| stable_id(&ready.session_id)),
        scene_status,
        lifecycle: runtime.map(|runtime| scene_lifecycle_label(runtime.state).to_owned()),
        loading_phase: progress.map(|progress| progress.phase.clone()),
        loading_policy: progress.map(|progress| progress.loading_policy.clone()),
        required_total: progress.map(|progress| progress.required_total),
        required_loaded: progress.map(|progress| progress.required_loaded),
        optional_total: progress.map(|progress| progress.optional_total),
        optional_loaded: progress.map(|progress| progress.optional_loaded),
        optional_failed: progress.map(|progress| progress.optional_failed),
        loading_message_key: progress.and_then(|progress| progress.message_key.clone()),
        authority_mode: ready_matches_metadata
            .map(|ready| scene_authority_mode_label(ready.authority_mode).to_owned())
            .or_else(|| {
                metadata
                    .map(|session| scene_authority_mode_label(session.authority_mode).to_owned())
            })
            .or_else(|| {
                ready.map(|ready| scene_authority_mode_label(ready.authority_mode).to_owned())
            }),
        content_version: ready_matches_metadata
            .and_then(|ready| ready.content_version.clone())
            .or_else(|| metadata.and_then(|session| session.content_version.clone()))
            .or_else(|| ready.and_then(|ready| ready.content_version.clone())),
        seed: ready_matches_metadata
            .and_then(|ready| ready.seed)
            .or_else(|| metadata.and_then(|session| session.seed))
            .or_else(|| ready.and_then(|ready| ready.seed)),
        scene_entity_count: debug_snapshot
            .map(|snapshot| snapshot.entity_counts.total_scene_owned as u64),
        scene_root_count: debug_snapshot.map(|snapshot| snapshot.entity_counts.scene_roots as u64),
        runtime_root_count: debug_snapshot
            .map(|snapshot| snapshot.entity_counts.runtime_roots as u64),
        layer_count: debug_snapshot.map(|snapshot| snapshot.layer_count as u64),
        layer_ids,
        layers,
        recent_error,
        adapter_summary: None,
    }
}

fn apply_scene_event(state: &mut ScenePreviewCollectorState, event: &SceneEvent) {
    match event {
        SceneEvent::Resolving(_) => {
            state.progress = None;
            state.event_error = None;
        }
        SceneEvent::LoadProgress(progress) => {
            state.progress = Some(SceneProgressSummary::from(progress));
        }
        SceneEvent::Entered(_) | SceneEvent::Ready(_) => {
            state.progress = None;
            state.event_error = None;
        }
        SceneEvent::Failed(failure) => {
            state.event_error = Some(failure.message_key().to_owned());
        }
        SceneEvent::Exited(_) => {
            state.progress = None;
        }
        _ => {}
    }
}

fn record_scene_event(
    event: &SceneEvent,
    previous: Option<&ScenePreviewState>,
    timeline: &mut LivePreviewTimeline,
    timestamp_ms: u64,
    snapshot_sequence: u64,
) {
    let detail = |scene_id: &str, session_id: Option<&str>| {
        Some(match session_id {
            Some(session_id) => format!("scene={scene_id} session={session_id}"),
            None => format!("scene={scene_id}"),
        })
    };
    let push =
        |timeline: &mut LivePreviewTimeline, severity, summary: &str, detail: Option<String>| {
            timeline.push(LivePreviewTimelineEvent::new(
                LivePreviewTimelineType::Scene,
                severity,
                timestamp_ms,
                snapshot_sequence,
                summary,
                detail,
            ));
        };

    match event {
        SceneEvent::Resolving(resolving) => {
            let scene_id = resolving.scene_id.to_string();
            let is_reload = previous.is_some_and(|previous| {
                previous
                    .active_scene_id
                    .as_ref()
                    .is_some_and(|active| active.as_str() == scene_id)
            });
            push(
                timeline,
                LivePreviewTimelineSeverity::Info,
                if is_reload {
                    "scene reload"
                } else {
                    "scene resolving"
                },
                detail(
                    &scene_id,
                    resolving
                        .session_id
                        .as_ref()
                        .map(ToString::to_string)
                        .as_deref(),
                ),
            );
        }
        SceneEvent::LoadProgress(progress) => {
            let scene_id = progress.scene_id.to_string();
            let session = progress.session_id.as_ref().map(ToString::to_string);
            push(
                timeline,
                LivePreviewTimelineSeverity::Info,
                "scene loading progress",
                Some(match session {
                    Some(session) => format!(
                        "scene={scene_id} session={session} phase={}",
                        scene_load_phase_label(progress.phase)
                    ),
                    None => format!(
                        "scene={scene_id} phase={}",
                        scene_load_phase_label(progress.phase)
                    ),
                }),
            );
        }
        SceneEvent::Instantiating(instantiating) => {
            let scene_id = instantiating.scene_id.to_string();
            push(
                timeline,
                LivePreviewTimelineSeverity::Info,
                "scene instantiating",
                detail(
                    &scene_id,
                    Some(instantiating.session_id.to_string().as_str()),
                ),
            );
        }
        SceneEvent::Entered(entered) => {
            let scene_id = entered.scene_id.to_string();
            let session_id = entered.session_id.to_string();
            let scene_detail = detail(&scene_id, Some(session_id.as_str()));
            push(
                timeline,
                LivePreviewTimelineSeverity::Info,
                "scene entered",
                scene_detail.clone(),
            );
            push(
                timeline,
                LivePreviewTimelineSeverity::Info,
                "scene active",
                scene_detail,
            );
        }
        SceneEvent::Ready(ready) => {
            let scene_id = ready.scene_id.to_string();
            push(
                timeline,
                LivePreviewTimelineSeverity::Info,
                "scene ready",
                detail(&scene_id, Some(ready.session_id.to_string().as_str())),
            );
        }
        SceneEvent::ExitStarted(exit) => {
            let scene_id = exit.scene_id.to_string();
            push(
                timeline,
                LivePreviewTimelineSeverity::Info,
                "scene exit started",
                detail(&scene_id, Some(exit.session_id.to_string().as_str())),
            );
        }
        SceneEvent::Exited(exit) => {
            let scene_id = exit.scene_id.to_string();
            push(
                timeline,
                LivePreviewTimelineSeverity::Info,
                "scene exited",
                detail(&scene_id, Some(exit.session_id.to_string().as_str())),
            );
        }
        SceneEvent::LayerLoaded(layer) | SceneEvent::LayerUnloaded(layer) => {
            let scene_id = layer.scene_id.to_string();
            push(
                timeline,
                LivePreviewTimelineSeverity::Info,
                "scene layer changed",
                Some(format!(
                    "scene={scene_id} session={} layer={} state={}",
                    layer.session_id,
                    layer.layer_id,
                    scene_layer_state_label(layer.state)
                )),
            );
        }
        SceneEvent::Failed(failure) => {
            let scene_id = failure
                .scene_id
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "unknown".to_owned());
            let detail = match failure.session_id.as_ref().map(ToString::to_string) {
                Some(session) => Some(format!(
                    "scene={scene_id} session={session} key={}",
                    failure.message_key()
                )),
                None => Some(format!("scene={scene_id} key={}", failure.message_key())),
            };
            push(
                timeline,
                LivePreviewTimelineSeverity::Error,
                "scene failed",
                detail,
            );
        }
        _ => {}
    }
}

fn scene_layer_preview(layer: &SceneLayerDebugInfo) -> SceneLayerPreview {
    SceneLayerPreview {
        id: stable_id(&layer.layer_id),
        session_id: stable_id(&layer.session_id),
        state: Some(scene_layer_state_label(layer.state).to_owned()),
        required: Some(layer.required),
    }
}

fn scene_status(runtime: &SceneRuntime) -> &'static str {
    if runtime.state == SceneLifecycleState::Failed {
        "failed"
    } else if runtime.pending.is_some() && runtime.active.is_some() {
        "switching"
    } else if runtime.pending.is_some() {
        "loading"
    } else if runtime.active.is_some() && runtime.ready.is_some() {
        "active_ready"
    } else if runtime.active.is_some() {
        "active"
    } else if runtime.ready.is_some() {
        "ready"
    } else {
        scene_lifecycle_label(runtime.state)
    }
}

fn stable_id(value: &impl ToString) -> StablePreviewId {
    StablePreviewId::new(value.to_string())
}

fn next_publish_sequence(current_sequence: u64) -> u64 {
    current_sequence.saturating_add(1)
}

fn scene_debug_sample_due(last_sample_ms: Option<u64>, now_ms: u64) -> bool {
    last_sample_ms.is_none_or(|last| now_ms.saturating_sub(last) >= SCENE_DEBUG_SAMPLE_INTERVAL_MS)
}

fn clear_scene_debug_cache_if_runtime_missing(
    state: &mut ScenePreviewCollectorState,
    runtime: Option<&SceneRuntime>,
) {
    if runtime.is_none() {
        state.debug_snapshot = None;
        state.last_debug_sample_ms = None;
    }
}

fn scene_lifecycle_label(state: SceneLifecycleState) -> &'static str {
    match state {
        SceneLifecycleState::Idle => "idle",
        SceneLifecycleState::Resolving => "resolving",
        SceneLifecycleState::Downloading => "downloading",
        SceneLifecycleState::LoadingAssets => "loading_assets",
        SceneLifecycleState::Instantiating => "instantiating",
        SceneLifecycleState::Activating => "activating",
        SceneLifecycleState::Active => "active",
        SceneLifecycleState::Suspending => "suspending",
        SceneLifecycleState::Deactivating => "deactivating",
        SceneLifecycleState::Unloading => "unloading",
        SceneLifecycleState::Failed => "failed",
    }
}

fn scene_load_phase_label(phase: SceneLoadPhase) -> &'static str {
    match phase {
        SceneLoadPhase::Resolving => "resolving",
        SceneLoadPhase::Downloading => "downloading",
        SceneLoadPhase::LoadingAssets => "loading_assets",
        SceneLoadPhase::Instantiating => "instantiating",
        SceneLoadPhase::Activating => "activating",
        SceneLoadPhase::Complete => "complete",
    }
}

fn scene_loading_policy_label(policy: SceneLoadingPolicy) -> &'static str {
    match policy {
        SceneLoadingPolicy::None => "none",
        SceneLoadingPolicy::Spinner => "spinner",
        SceneLoadingPolicy::Progress => "progress",
        SceneLoadingPolicy::Blocking => "blocking",
        SceneLoadingPolicy::NonBlocking => "non_blocking",
    }
}

fn scene_authority_mode_label(mode: crate::framework::scene::SceneAuthorityMode) -> &'static str {
    match mode {
        crate::framework::scene::SceneAuthorityMode::Local => "local",
        crate::framework::scene::SceneAuthorityMode::LocalHost => "local_host",
        crate::framework::scene::SceneAuthorityMode::Remote => "remote",
        crate::framework::scene::SceneAuthorityMode::External => "external",
    }
}

fn scene_layer_state_label(state: SceneLayerState) -> &'static str {
    match state {
        SceneLayerState::Registered => "registered",
        SceneLayerState::Loading => "loading",
        SceneLayerState::Loaded => "loaded",
        SceneLayerState::Active => "active",
        SceneLayerState::Unloading => "unloading",
        SceneLayerState::Failed => "failed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::scene::{
        SceneAuthorityMode, SceneEntityCounts, SceneReadyInfo, SceneSessionInfo,
    };

    fn session(scene_id: &str, session_id: &str) -> SceneSessionInfo {
        SceneSessionInfo::new(scene_id, session_id)
    }

    fn debug_for(runtime: &SceneRuntime) -> SceneDebugSnapshot {
        SceneDebugSnapshot::from_runtime(runtime)
    }

    #[test]
    fn idle_scene_has_explicit_empty_state() {
        let runtime = SceneRuntime::default();
        let state = collect_scene_state(Some(&runtime), Some(&debug_for(&runtime)), None, None);

        assert_eq!(state.scene_status.as_deref(), Some("idle"));
        assert!(state.active_scene_id.is_none());
        assert!(state.pending_scene_id.is_none());
        assert!(state.ready_session_id.is_none());
    }

    #[test]
    fn pending_scene_is_distinct_from_active_and_ready_sessions() {
        let mut runtime = SceneRuntime {
            state: SceneLifecycleState::LoadingAssets,
            ..Default::default()
        };
        runtime.active = Some(session("world.old", "old-session"));
        runtime.pending = Some(session("world.new", "new-session"));
        let state = collect_scene_state(Some(&runtime), Some(&debug_for(&runtime)), None, None);

        assert_eq!(state.scene_status.as_deref(), Some("switching"));
        assert_eq!(
            state.active_scene_id.as_ref().map(StablePreviewId::as_str),
            Some("world.old")
        );
        assert_eq!(
            state.pending_scene_id.as_ref().map(StablePreviewId::as_str),
            Some("world.new")
        );
        assert_eq!(
            state
                .active_session_id
                .as_ref()
                .map(StablePreviewId::as_str),
            Some("old-session")
        );
        assert_eq!(
            state
                .pending_session_id
                .as_ref()
                .map(StablePreviewId::as_str),
            Some("new-session")
        );
        assert!(state.ready_session_id.is_none());
    }

    #[test]
    fn ready_active_scene_exposes_authority_content_and_seed() {
        let mut active = session("world.home", "session-1");
        active.authority_mode = SceneAuthorityMode::Remote;
        active.content_version = Some("v7".to_owned());
        active.seed = Some(42);
        let mut runtime = SceneRuntime {
            active: Some(active.clone()),
            state: SceneLifecycleState::Active,
            ..Default::default()
        };
        runtime.ready = Some(SceneReadyInfo::from_session(&active));
        let state = collect_scene_state(Some(&runtime), Some(&debug_for(&runtime)), None, None);

        assert_eq!(state.scene_status.as_deref(), Some("active_ready"));
        assert_eq!(state.authority_mode.as_deref(), Some("remote"));
        assert_eq!(state.content_version.as_deref(), Some("v7"));
        assert_eq!(state.seed, Some(42));
        assert_eq!(
            state.ready_scene_id.as_ref().map(StablePreviewId::as_str),
            Some("world.home")
        );
    }

    #[test]
    fn ready_event_clears_loading_progress_from_active_snapshot() {
        let mut collector = ScenePreviewCollectorState::default();
        let mut progress = SceneLoadProgress::new("world.home", SceneLoadPhase::Instantiating);
        progress.session_id = Some("session-1".into());
        progress.message_key = Some("scene.loading.instantiating".to_owned());
        apply_scene_event(&mut collector, &SceneEvent::LoadProgress(progress));
        assert!(collector.progress.is_some());

        let active = session("world.home", "session-1");
        let ready = SceneReadyInfo::from_session(&active);
        apply_scene_event(&mut collector, &SceneEvent::Ready(ready.event()));
        assert!(collector.progress.is_none());

        let runtime = SceneRuntime {
            active: Some(active),
            state: SceneLifecycleState::Active,
            ..Default::default()
        };
        let state = collect_scene_state(
            Some(&runtime),
            Some(&debug_for(&runtime)),
            collector.progress.as_ref(),
            None,
        );
        assert!(state.loading_phase.is_none());
        assert!(state.loading_message_key.is_none());
    }

    #[test]
    fn failed_scene_exposes_stable_error_key_only() {
        let failure = SceneFailure::new(
            crate::framework::scene::SceneFailureKind::AssetLoadFailed,
            SceneLifecycleState::Failed,
        )
        .with_scene("world.failed")
        .with_message("private path should not be collected");
        let runtime = SceneRuntime {
            state: SceneLifecycleState::Failed,
            last_error: Some(failure),
            ..Default::default()
        };
        let state = collect_scene_state(Some(&runtime), Some(&debug_for(&runtime)), None, None);

        assert_eq!(state.scene_status.as_deref(), Some("failed"));
        assert_eq!(
            state.recent_error.as_deref(),
            Some("scene.error.asset_load_failed")
        );
        assert!(
            !state
                .recent_error
                .as_deref()
                .unwrap_or_default()
                .contains("private")
        );
    }

    #[test]
    fn pure_ui_scene_keeps_session_metadata_without_world_entities() {
        let mut runtime = SceneRuntime {
            active: Some(session("ui.overlay", "ui-session")),
            state: SceneLifecycleState::Active,
            ..Default::default()
        };
        runtime.ready = Some(SceneReadyInfo::from_session(
            runtime.active.as_ref().unwrap(),
        ));
        let mut debug = debug_for(&runtime);
        debug.entity_counts = SceneEntityCounts::default();
        let state = collect_scene_state(Some(&runtime), Some(&debug), None, None);

        assert_eq!(state.scene_status.as_deref(), Some("active_ready"));
        assert_eq!(state.scene_entity_count, Some(0));
        assert_eq!(state.layer_count, Some(0));
    }

    #[test]
    fn loading_progress_and_layers_are_stable_and_separate() {
        let runtime = SceneRuntime {
            pending: Some(session("world.multi", "session-2")),
            state: SceneLifecycleState::LoadingAssets,
            ..Default::default()
        };
        let mut debug = debug_for(&runtime);
        debug.entity_counts.total_scene_owned = 12;
        debug.entity_counts.scene_roots = 1;
        debug.entity_counts.layer_roots = 2;
        debug.entity_counts.runtime_roots = 1;
        debug.layers = vec![
            SceneLayerDebugInfo {
                layer_id: "base".into(),
                session_id: "session-2".into(),
                state: SceneLayerState::Active,
                required: true,
            },
            SceneLayerDebugInfo {
                layer_id: "fx".into(),
                session_id: "session-2".into(),
                state: SceneLayerState::Loaded,
                required: false,
            },
        ];
        debug.layer_count = 2;
        let mut progress = SceneLoadProgress::new("world.multi", SceneLoadPhase::LoadingAssets);
        progress.session_id = Some("session-2".into());
        progress.loading_policy = SceneLoadingPolicy::Progress;
        progress.required_total = 4;
        progress.required_loaded = 2;
        progress.optional_total = 3;
        progress.optional_loaded = 1;
        progress.optional_failed = 1;
        progress.message_key = Some("scene.loading.assets".to_owned());
        let progress = SceneProgressSummary::from(&progress);
        let state = collect_scene_state(Some(&runtime), Some(&debug), Some(&progress), None);

        assert_eq!(state.loading_phase.as_deref(), Some("loading_assets"));
        assert_eq!(state.loading_policy.as_deref(), Some("progress"));
        assert_eq!(state.required_loaded, Some(2));
        assert_eq!(state.optional_failed, Some(1));
        assert_eq!(state.layer_ids.len(), 2);
        assert_eq!(state.layers.len(), 2);
        assert_eq!(state.scene_entity_count, Some(12));
        assert_eq!(state.layer_count, Some(2));
    }

    #[test]
    fn resolving_same_active_scene_is_classified_as_reload() {
        let previous = ScenePreviewState {
            active_scene_id: Some(StablePreviewId::from("world.home")),
            ..Default::default()
        };
        let event = SceneEvent::Resolving(crate::framework::scene::SceneResolving {
            scene_id: "world.home".into(),
            session_id: Some("session-reload".into()),
        });
        let mut timeline = LivePreviewTimeline::default();
        record_scene_event(&event, Some(&previous), &mut timeline, 1, 2);

        assert_eq!(timeline.iter().next().unwrap().summary, "scene reload");
    }

    #[test]
    fn scene_debug_sampling_uses_independent_500ms_cadence() {
        assert!(scene_debug_sample_due(None, 0));
        assert!(!scene_debug_sample_due(Some(0), 499));
        assert!(scene_debug_sample_due(Some(0), 500));
        assert!(scene_debug_sample_due(Some(1_000), 1_500));
    }

    #[test]
    fn active_scene_cache_is_cleared_before_idle_no_scene_snapshot() {
        let active = session("world.home", "session-1");
        let mut collector = ScenePreviewCollectorState {
            debug_snapshot: Some(debug_for(&SceneRuntime {
                active: Some(active),
                state: SceneLifecycleState::Active,
                ..Default::default()
            })),
            last_debug_sample_ms: Some(500),
            ..Default::default()
        };
        clear_scene_debug_cache_if_runtime_missing(&mut collector, None);
        assert!(collector.debug_snapshot.is_none());
        assert!(collector.last_debug_sample_ms.is_none());

        let runtime = SceneRuntime::default();
        let state = collect_scene_state(Some(&runtime), None, None, None);
        assert_eq!(state.scene_status.as_deref(), Some("idle"));
        assert!(state.scene_entity_count.is_none());
        assert!(state.layer_ids.is_empty());
    }

    #[test]
    fn loading_and_failure_timeline_details_identify_scene_sessions() {
        let mut progress = SceneLoadProgress::new("world.home", SceneLoadPhase::LoadingAssets);
        progress.session_id = Some("session-1".into());
        let failure = SceneFailure::new(
            crate::framework::scene::SceneFailureKind::AssetLoadFailed,
            SceneLifecycleState::Failed,
        )
        .with_scene("world.home")
        .with_session("session-2")
        .with_message("private failure text");
        let mut timeline = LivePreviewTimeline::default();

        record_scene_event(
            &SceneEvent::LoadProgress(progress),
            None,
            &mut timeline,
            1,
            2,
        );
        record_scene_event(&SceneEvent::Failed(failure), None, &mut timeline, 2, 3);

        let details: Vec<_> = timeline
            .iter()
            .map(|event| event.detail.as_deref().unwrap_or_default())
            .collect();
        assert_eq!(
            details[0],
            "scene=world.home session=session-1 phase=loading_assets"
        );
        assert_eq!(
            details[1],
            "scene=world.home session=session-2 key=scene.error.asset_load_failed"
        );
        assert!(!details[1].contains("private"));
    }
}
