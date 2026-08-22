use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::ops::Deref;

/// Increment when the serialized shape or enum vocabulary changes incompatibly.
pub const LIVE_PREVIEW_SCHEMA_VERSION: u16 = 2;

/// A string identifier supplied by a domain adapter, never a Bevy `Entity`.
///
/// Adapters must derive this value from a domain key that remains stable across
/// frames (for example, a panel id or character id). Values are intentionally
/// opaque to the framework layer.
#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, Serialize, PartialOrd)]
#[serde(transparent)]
pub struct StablePreviewId(String);

impl StablePreviewId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for StablePreviewId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for StablePreviewId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

/// Why a value is not currently available. This is part of the wire contract;
/// an absent value must not be interpreted as a numeric default.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewDataStatus {
    Available,
    Unavailable,
    NotApplicable,
    NotCollected,
    Failed,
}

impl Default for PreviewDataStatus {
    fn default() -> Self {
        Self::NotCollected
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PreviewFailure {
    /// A stable, non-sensitive category such as `permission_denied`.
    pub code: String,
    /// A short diagnostic summary. Secrets, tickets and endpoint URLs are not
    /// valid contents for this field.
    pub summary: String,
}

impl PreviewFailure {
    pub fn new(code: impl Into<String>, summary: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            summary: summary.into(),
        }
    }
}

/// A section carries its own freshness metadata and an optional payload.
/// `revision` is assigned by the collector and `content_hash` is derived from
/// the canonical payload. Both are absent for non-available states.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PreviewSection<T> {
    pub status: PreviewDataStatus,
    pub revision: Option<u64>,
    pub content_hash: Option<u64>,
    pub value: Option<T>,
    pub failure: Option<PreviewFailure>,
}

impl<T> PreviewSection<T>
where
    T: Serialize,
{
    pub fn available(revision: u64, mut value: T) -> Self
    where
        T: StablePreviewValue,
    {
        value.stable_sort();
        match stable_hash(&value) {
            Ok(content_hash) => Self {
                status: PreviewDataStatus::Available,
                revision: Some(revision),
                content_hash: Some(content_hash),
                value: Some(value),
                failure: None,
            },
            Err(summary) => Self::failed(PreviewFailure::new("serialization_failed", summary)),
        }
    }

    pub fn unavailable() -> Self {
        Self::empty(PreviewDataStatus::Unavailable)
    }

    pub fn not_applicable() -> Self {
        Self::empty(PreviewDataStatus::NotApplicable)
    }

    pub fn not_collected() -> Self {
        Self::empty(PreviewDataStatus::NotCollected)
    }

    pub fn failed(failure: PreviewFailure) -> Self {
        Self {
            status: PreviewDataStatus::Failed,
            revision: None,
            content_hash: None,
            value: None,
            failure: Some(failure),
        }
    }

    fn empty(status: PreviewDataStatus) -> Self {
        Self {
            status,
            revision: None,
            content_hash: None,
            value: None,
            failure: None,
        }
    }

    pub fn canonicalize(&mut self)
    where
        T: StablePreviewValue,
    {
        if self.status == PreviewDataStatus::Available {
            if let Some(value) = self.value.as_mut() {
                value.stable_sort();
                match stable_hash(value) {
                    Ok(hash) => self.content_hash = Some(hash),
                    Err(summary) => {
                        self.status = PreviewDataStatus::Failed;
                        self.revision = None;
                        self.content_hash = None;
                        self.value = None;
                        self.failure = Some(PreviewFailure::new("serialization_failed", summary));
                    }
                }
            } else {
                // A malformed available section fails closed instead of
                // manufacturing a zero/default payload.
                self.status = PreviewDataStatus::Failed;
                self.revision = None;
                self.content_hash = None;
                self.failure = Some(PreviewFailure::new(
                    "missing_payload",
                    "available section did not contain a payload",
                ));
            }
        } else {
            self.revision = None;
            self.content_hash = None;
            self.value = None;
            if self.status != PreviewDataStatus::Failed {
                self.failure = None;
            }
        }
    }
}

impl<T: Serialize> Default for PreviewSection<T> {
    fn default() -> Self {
        Self::not_collected()
    }
}

pub trait StablePreviewValue {
    fn stable_sort(&mut self);
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct UiPanelPreview {
    pub id: StablePreviewId,
    pub kind: Option<String>,
    pub owner: Option<String>,
    pub z_index: Option<i32>,
    pub active: Option<bool>,
    pub stack_index: Option<u32>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct UiPreviewState {
    pub canonical_screen: Option<String>,
    pub screen_id: Option<StablePreviewId>,
    pub owner: Option<String>,
    pub panels: Vec<UiPanelPreview>,
    pub pointer_blocked: Option<bool>,
    pub block_reason: Option<String>,
    pub route_summary: Option<String>,
    pub blocking_reason: Option<String>,
    pub focus_panel_id: Option<StablePreviewId>,
    pub focus_node_id: Option<StablePreviewId>,
    pub ui_node_count: Option<u64>,
    pub visible_ui_node_count: Option<u64>,
    pub text_node_count: Option<u64>,
    pub panel_count: Option<u64>,
    pub panel_kind_counts: Option<UiPanelKindPreviewCounts>,
    pub document_id: Option<StablePreviewId>,
    pub document_schema_version: Option<u16>,
    pub document_status: Option<String>,
    pub document_source: Option<String>,
    pub document_error: Option<String>,
    pub viewport: Option<UiViewportPreview>,
    pub metrics: Option<UiMetricsPreview>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct UiViewportPreview {
    pub logical_width: f32,
    pub logical_height: f32,
    pub device_scale: f32,
    pub preview_scale: f32,
    pub width_class: String,
    pub height_class: String,
    pub orientation: String,
    pub input_mode: String,
    pub safe_area: [f32; 4],
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct UiMetricsPreview {
    pub page_padding: f32,
    pub panel_padding: f32,
    pub control_gap: f32,
    pub section_gap: f32,
    pub touch_target_min: f32,
    pub font_body: f32,
    pub content_max_width: f32,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct UiPanelKindPreviewCounts {
    pub page: u64,
    pub hud: u64,
    pub floating: u64,
    pub modal: u64,
    pub blocking_overlay: u64,
}

impl StablePreviewValue for UiPreviewState {
    fn stable_sort(&mut self) {
        self.panels.sort_by(|left, right| left.id.cmp(&right.id));
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct PlayerPreviewState {
    pub character_id: Option<StablePreviewId>,
    pub display_name: Option<String>,
    pub world_id: Option<StablePreviewId>,
    pub selection_state: Option<String>,
    pub attributes: Option<PlayerAttributesPreview>,
    pub attributes_source: Option<String>,
    pub attributes_snapshot_refreshed_at_ms: Option<u64>,
    pub attributes_push_sequence: Option<u64>,
    pub attributes_revision: Option<u64>,
    pub attributes_freshness: Option<String>,
    pub position: Option<[f32; 3]>,
    pub direction: Option<[f32; 3]>,
    pub movement_state: Option<String>,
    pub authority_frame: Option<u64>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct PlayerAttributesPreview {
    pub affinity: PlayerElementValuesPreview,
    pub mastery: PlayerElementValuesPreview,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct PlayerElementValuesPreview {
    pub earth: i32,
    pub fire: i32,
    pub water: i32,
    pub wind: i32,
}

impl StablePreviewValue for PlayerPreviewState {
    fn stable_sort(&mut self) {}
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct ScenePreviewState {
    pub active_scene_id: Option<StablePreviewId>,
    pub active_session_id: Option<StablePreviewId>,
    pub pending_scene_id: Option<StablePreviewId>,
    pub pending_session_id: Option<StablePreviewId>,
    pub ready_scene_id: Option<StablePreviewId>,
    pub ready_session_id: Option<StablePreviewId>,
    pub scene_status: Option<String>,
    pub lifecycle: Option<String>,
    pub loading_phase: Option<String>,
    pub loading_policy: Option<String>,
    pub required_total: Option<u64>,
    pub required_loaded: Option<u64>,
    pub optional_total: Option<u64>,
    pub optional_loaded: Option<u64>,
    pub optional_failed: Option<u64>,
    pub loading_message_key: Option<String>,
    pub authority_mode: Option<String>,
    pub content_version: Option<String>,
    pub seed: Option<u64>,
    pub scene_entity_count: Option<u64>,
    pub scene_root_count: Option<u64>,
    pub runtime_root_count: Option<u64>,
    pub layer_count: Option<u64>,
    pub layer_ids: Vec<StablePreviewId>,
    pub layers: Vec<SceneLayerPreview>,
    pub recent_error: Option<String>,
    pub adapter_summary: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct SceneLayerPreview {
    pub id: StablePreviewId,
    pub session_id: StablePreviewId,
    pub state: Option<String>,
    pub required: Option<bool>,
}

impl StablePreviewValue for ScenePreviewState {
    fn stable_sort(&mut self) {
        self.layer_ids.sort();
        self.layers.sort_by(|left, right| {
            left.id
                .cmp(&right.id)
                .then(left.session_id.cmp(&right.session_id))
        });
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct NetworkPreviewState {
    pub session_status: Option<String>,
    pub login_status: Option<String>,
    pub registration_status: Option<String>,
    pub character_selection_status: Option<String>,
    pub connection_state: Option<String>,
    pub transport: Option<String>,
    pub connected: Option<bool>,
    pub authenticated: Option<bool>,
    pub room_id: Option<StablePreviewId>,
    pub endpoint_kind: Option<String>,
    pub endpoint_environment: Option<String>,
    /// Deliberately empty unless a separately authorized local-debug view opts in.
    pub endpoint_detail: Option<String>,
    pub pending_request_count: Option<u32>,
    pub last_successful_receive_ms: Option<u64>,
    pub last_error_category: Option<String>,
    pub reconnecting: Option<bool>,
    pub reconnect_phase: Option<String>,
    pub authority_endpoint_kind: Option<String>,
    pub authority_role: Option<String>,
    pub authority_epoch: Option<u64>,
    pub authority_frame: Option<u64>,
    pub authority_last_activity_age_ms: Option<u64>,
    pub authority_sync_health: Option<String>,
}

impl StablePreviewValue for NetworkPreviewState {
    fn stable_sort(&mut self) {}
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct PerformancePreviewState {
    pub fps: Option<f32>,
    pub frame_time_ms: Option<f32>,
    pub collector_time_us: Option<u64>,
    pub ui_node_count: Option<u64>,
    pub scene_entity_count: Option<u64>,
    pub timeline_entry_count: Option<u64>,
}

impl StablePreviewValue for PerformancePreviewState {
    fn stable_sort(&mut self) {}
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PreviewSourceHealth {
    pub source_id: StablePreviewId,
    pub status: PreviewDataStatus,
    pub last_collected_frame: Option<u64>,
    pub revision: Option<u64>,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct PreviewSourceHealthState {
    pub sources: Vec<PreviewSourceHealth>,
}

impl StablePreviewValue for PreviewSourceHealthState {
    fn stable_sort(&mut self) {
        self.sources
            .sort_by(|left, right| left.source_id.cmp(&right.source_id));
    }
}

pub type UiPreviewSection = PreviewSection<UiPreviewState>;
pub type PlayerPreviewSection = PreviewSection<PlayerPreviewState>;
pub type ScenePreviewSection = PreviewSection<ScenePreviewState>;
pub type NetworkPreviewSection = PreviewSection<NetworkPreviewState>;
pub type PerformancePreviewSection = PreviewSection<PerformancePreviewState>;
pub type PreviewSourceHealthSection = PreviewSection<PreviewSourceHealthState>;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LivePreviewSnapshot {
    pub schema_version: u16,
    pub sequence: u64,
    pub captured_frame: u64,
    pub captured_monotonic_ms: u64,
    pub ui: UiPreviewSection,
    pub player: PlayerPreviewSection,
    pub scene: ScenePreviewSection,
    pub network: NetworkPreviewSection,
    pub performance: PerformancePreviewSection,
    pub source_health: PreviewSourceHealthSection,
    pub timeline: LivePreviewTimelinePreview,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct LivePreviewTimelinePreview {
    pub capacity: u64,
    pub events: Vec<LivePreviewTimelineEventPreview>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct LivePreviewTimelineEventPreview {
    pub event_type: String,
    pub severity: String,
    pub timestamp_ms: u64,
    pub snapshot_sequence: u64,
    pub summary: String,
    pub detail: Option<String>,
    pub repeat_count: u32,
}

impl Default for LivePreviewSnapshot {
    fn default() -> Self {
        Self {
            schema_version: LIVE_PREVIEW_SCHEMA_VERSION,
            sequence: 0,
            captured_frame: 0,
            captured_monotonic_ms: 0,
            ui: UiPreviewSection::default(),
            player: PlayerPreviewSection::default(),
            scene: ScenePreviewSection::default(),
            network: NetworkPreviewSection::default(),
            performance: PerformancePreviewSection::default(),
            source_health: PreviewSourceHealthSection::default(),
            timeline: LivePreviewTimelinePreview::default(),
        }
    }
}

impl LivePreviewSnapshot {
    pub fn canonicalize(&mut self) {
        self.schema_version = LIVE_PREVIEW_SCHEMA_VERSION;
        self.ui.canonicalize();
        self.player.canonicalize();
        self.scene.canonicalize();
        self.network.canonicalize();
        self.performance.canonicalize();
        self.source_health.canonicalize();
        self.timeline.events.sort_by(|left, right| {
            left.timestamp_ms
                .cmp(&right.timestamp_ms)
                .then(left.snapshot_sequence.cmp(&right.snapshot_sequence))
                .then(left.event_type.cmp(&right.event_type))
                .then(left.summary.cmp(&right.summary))
        });
    }
}

fn stable_hash<T: Serialize>(value: &T) -> Result<u64, String> {
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    let digest = Sha256::digest(bytes);
    Ok(u64::from_le_bytes(
        digest[..8]
            .try_into()
            .expect("sha256 digest is at least 8 bytes"),
    ))
}

/// Resource that owns the latest published snapshot. Consumers should use
/// [`LivePreviewSnapshotHub::read`] and never receive a mutable reference.
#[derive(Clone, Debug, Resource)]
pub struct LivePreviewSnapshotHub {
    snapshot: LivePreviewSnapshot,
    next_sequence: u64,
}

impl Default for LivePreviewSnapshotHub {
    fn default() -> Self {
        Self {
            snapshot: LivePreviewSnapshot::default(),
            next_sequence: 0,
        }
    }
}

impl LivePreviewSnapshotHub {
    pub fn read(&self) -> LivePreviewSnapshotRead<'_> {
        LivePreviewSnapshotRead(&self.snapshot)
    }

    pub(super) fn writer(&mut self) -> LivePreviewSnapshotWriter<'_> {
        LivePreviewSnapshotWriter { hub: self }
    }
}

pub struct LivePreviewSnapshotRead<'a>(&'a LivePreviewSnapshot);

impl<'a> Deref for LivePreviewSnapshotRead<'a> {
    type Target = LivePreviewSnapshot;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

pub(super) struct LivePreviewSnapshotWriter<'a> {
    hub: &'a mut LivePreviewSnapshotHub,
}

impl LivePreviewSnapshotWriter<'_> {
    pub(super) fn publish(&mut self, mut snapshot: LivePreviewSnapshot) {
        snapshot.canonicalize();
        self.hub.next_sequence = self.hub.next_sequence.saturating_add(1);
        snapshot.sequence = self.hub.next_sequence;
        self.hub.snapshot = snapshot;
    }
}

/// Explicit fail-closed policy for the process-local monitor.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct LivePreviewPolicy {
    requested: bool,
    authorized: bool,
}

impl LivePreviewPolicy {
    pub fn explicit_debug_authorization() -> Self {
        Self {
            requested: true,
            authorized: true,
        }
    }

    pub fn request(mut self, requested: bool) -> Self {
        self.requested = requested;
        self
    }

    pub fn authorize(mut self, authorized: bool) -> Self {
        self.authorized = authorized;
        self
    }

    pub fn is_enabled(self) -> bool {
        self.requested
            && self.authorized
            && cfg!(debug_assertions)
            && cfg!(not(target_os = "android"))
    }

    pub fn requested(self) -> bool {
        self.requested
    }

    pub fn authorized(self) -> bool {
        self.authorized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_snapshot_is_explicitly_not_collected() {
        let snapshot = LivePreviewSnapshot::default();
        assert_eq!(snapshot.schema_version, LIVE_PREVIEW_SCHEMA_VERSION);
        assert_eq!(snapshot.sequence, 0);
        assert_eq!(snapshot.ui.status, PreviewDataStatus::NotCollected);
        assert_eq!(snapshot.ui.revision, None);
        assert_eq!(snapshot.ui.content_hash, None);
        assert!(snapshot.ui.value.is_none());
    }

    #[test]
    fn unavailable_and_failed_states_never_have_payload() {
        let unavailable: UiPreviewSection = PreviewSection::unavailable();
        assert_eq!(unavailable.status, PreviewDataStatus::Unavailable);
        assert!(unavailable.value.is_none());
        let failed: UiPreviewSection =
            PreviewSection::failed(PreviewFailure::new("io", "read failed"));
        assert_eq!(failed.status, PreviewDataStatus::Failed);
        assert!(failed.failure.is_some());
        assert!(failed.content_hash.is_none());
    }

    #[test]
    fn available_sections_sort_stable_ids_and_hash_payload() {
        let value = UiPreviewState {
            panels: vec![
                UiPanelPreview {
                    id: StablePreviewId::from("z"),
                    ..Default::default()
                },
                UiPanelPreview {
                    id: StablePreviewId::from("a"),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let section = PreviewSection::available(7, value);
        let ids: Vec<_> = section
            .value
            .as_ref()
            .unwrap()
            .panels
            .iter()
            .map(|panel| panel.id.as_str())
            .collect();
        assert_eq!(ids, vec!["a", "z"]);
        assert!(section.content_hash.is_some());
    }

    #[test]
    fn hub_assigns_monotonic_sequence_and_read_is_immutable() {
        let mut hub = LivePreviewSnapshotHub::default();
        let mut writer = hub.writer();
        writer.publish(LivePreviewSnapshot::default());
        drop(writer);
        assert_eq!(hub.read().sequence, 1);
        let mut writer = hub.writer();
        writer.publish(LivePreviewSnapshot::default());
        drop(writer);
        assert_eq!(hub.read().sequence, 2);
    }

    #[test]
    fn policy_defaults_closed_and_requires_explicit_authorization() {
        assert!(!LivePreviewPolicy::default().is_enabled());
        assert!(!LivePreviewPolicy::default().request(true).is_enabled());
        #[cfg(all(debug_assertions, not(target_os = "android")))]
        assert!(LivePreviewPolicy::explicit_debug_authorization().is_enabled());
    }
}
