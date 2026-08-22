use bevy::prelude::{
    App, IntoScheduleConfigs, Plugin, PostUpdate, Res, ResMut, Resource, SystemSet, Time,
};
use bevy::time::Real;

use super::collect_scene::{ScenePreviewCollectorState, collect_scene_preview};
use super::collect_ui::{UiPreviewCollectorState, collect_ui_preview};
use super::model::{
    LivePreviewSnapshot, LivePreviewSnapshotHub, LivePreviewTimelineEventPreview,
    LivePreviewTimelinePreview, NetworkPreviewSection, PerformancePreviewSection,
    PlayerPreviewSection, PreviewSection, PreviewSourceHealthSection, ScenePreviewSection,
    UiPreviewSection,
};
use super::monitor::LivePreviewMonitorPlugin;
use super::source_health::LivePreviewSourceHealthRegistry;
use super::timeline::LivePreviewTimeline;

pub const LIVE_PREVIEW_PLAYER_SAMPLE_INTERVAL_MS: u64 = 100;
pub const LIVE_PREVIEW_STATISTICS_SAMPLE_INTERVAL_MS: u64 = 500;
pub const LIVE_PREVIEW_HEARTBEAT_INTERVAL_MS: u64 = 2_000;

#[derive(Clone, Debug, Default, Resource)]
pub struct LivePreviewClock {
    monotonic_ms: u64,
    frame: u64,
}

impl LivePreviewClock {
    pub fn monotonic_ms(&self) -> u64 {
        self.monotonic_ms
    }

    pub fn frame(&self) -> u64 {
        self.frame
    }

    /// Advances the deterministic preview clock. A caller cannot move it
    /// backwards, which keeps captured timestamps stable across frame drops.
    pub fn advance_millis(&mut self, delta_ms: u64) {
        self.monotonic_ms = self.monotonic_ms.saturating_add(delta_ms);
        self.frame = self.frame.saturating_add(1);
    }
}

#[derive(Clone, Debug, Default, Resource)]
pub struct LivePreviewScheduler {
    last_player_sample_ms: Option<u64>,
    last_statistics_sample_ms: Option<u64>,
    last_publish_ms: Option<u64>,
}

impl LivePreviewScheduler {
    pub fn player_sample_due(&self, now_ms: u64) -> bool {
        elapsed_at_least(
            self.last_player_sample_ms,
            now_ms,
            LIVE_PREVIEW_PLAYER_SAMPLE_INTERVAL_MS,
        )
    }

    pub fn statistics_sample_due(&self, now_ms: u64) -> bool {
        elapsed_at_least(
            self.last_statistics_sample_ms,
            now_ms,
            LIVE_PREVIEW_STATISTICS_SAMPLE_INTERVAL_MS,
        )
    }

    pub fn heartbeat_due(&self, now_ms: u64) -> bool {
        self.last_publish_ms
            .is_some_and(|last| now_ms.saturating_sub(last) >= LIVE_PREVIEW_HEARTBEAT_INTERVAL_MS)
    }

    pub fn record_player_sample(&mut self, now_ms: u64) {
        self.last_player_sample_ms = Some(now_ms);
    }

    pub fn record_statistics_sample(&mut self, now_ms: u64) {
        self.last_statistics_sample_ms = Some(now_ms);
    }

    pub fn record_publish(&mut self, now_ms: u64) {
        self.last_publish_ms = Some(now_ms);
    }

    pub fn last_publish_ms(&self) -> Option<u64> {
        self.last_publish_ms
    }
}

fn elapsed_at_least(last: Option<u64>, now: u64, interval: u64) -> bool {
    last.is_none_or(|previous| now.saturating_sub(previous) >= interval)
}

fn timeline_preview(timeline: &LivePreviewTimeline) -> LivePreviewTimelinePreview {
    LivePreviewTimelinePreview {
        capacity: timeline.capacity() as u64,
        events: timeline
            .iter()
            .map(|event| LivePreviewTimelineEventPreview {
                event_type: timeline_type_label(&event.event_type),
                severity: format!("{:?}", event.severity).to_ascii_lowercase(),
                timestamp_ms: event.timestamp_ms,
                snapshot_sequence: event.snapshot_sequence,
                summary: event.summary.clone(),
                detail: event.detail.clone(),
                repeat_count: event.repeat_count,
            })
            .collect(),
    }
}

fn timeline_type_label(event_type: &super::timeline::LivePreviewTimelineType) -> String {
    match event_type {
        super::timeline::LivePreviewTimelineType::Ui => "ui".to_owned(),
        super::timeline::LivePreviewTimelineType::Player => "player".to_owned(),
        super::timeline::LivePreviewTimelineType::Scene => "scene".to_owned(),
        super::timeline::LivePreviewTimelineType::Network => "network".to_owned(),
        super::timeline::LivePreviewTimelineType::Performance => "performance".to_owned(),
        super::timeline::LivePreviewTimelineType::SourceHealth => "source_health".to_owned(),
        super::timeline::LivePreviewTimelineType::Custom(id) => format!("custom:{}", id.as_str()),
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LivePreviewDirtySections {
    pub ui: bool,
    pub player: bool,
    pub scene: bool,
    pub network: bool,
    pub performance: bool,
    pub source_health: bool,
}

impl LivePreviewDirtySections {
    pub fn any(self) -> bool {
        self.ui
            || self.player
            || self.scene
            || self.network
            || self.performance
            || self.source_health
    }

    fn clear(&mut self) {
        *self = Self::default();
    }
}

/// Collector-owned staging area. It is intentionally separate from the hub:
/// views read the hub snapshot and never receive this mutable resource.
#[derive(Clone, Debug, Resource)]
pub struct LivePreviewCollectionBuffer {
    snapshot: LivePreviewSnapshot,
    dirty: LivePreviewDirtySections,
}

impl Default for LivePreviewCollectionBuffer {
    fn default() -> Self {
        Self {
            snapshot: LivePreviewSnapshot::default(),
            dirty: LivePreviewDirtySections::default(),
        }
    }
}

impl LivePreviewCollectionBuffer {
    pub fn dirty(&self) -> LivePreviewDirtySections {
        self.dirty
    }

    pub fn set_ui(&mut self, mut section: UiPreviewSection) {
        section.canonicalize();
        if section_metadata_changed(&self.snapshot.ui, &section) {
            self.snapshot.ui = section;
            self.dirty.ui = true;
        }
    }

    pub fn set_player(&mut self, mut section: PlayerPreviewSection) {
        section.canonicalize();
        if section_metadata_changed(&self.snapshot.player, &section) {
            self.snapshot.player = section;
            self.dirty.player = true;
        }
    }

    pub fn set_scene(&mut self, mut section: ScenePreviewSection) {
        section.canonicalize();
        if section_metadata_changed(&self.snapshot.scene, &section) {
            self.snapshot.scene = section;
            self.dirty.scene = true;
        }
    }

    pub fn set_network(&mut self, mut section: NetworkPreviewSection) {
        section.canonicalize();
        if section_metadata_changed(&self.snapshot.network, &section) {
            self.snapshot.network = section;
            self.dirty.network = true;
        }
    }

    pub fn set_performance(&mut self, mut section: PerformancePreviewSection) {
        section.canonicalize();
        if section_metadata_changed(&self.snapshot.performance, &section) {
            self.snapshot.performance = section;
            self.dirty.performance = true;
        }
    }

    pub fn set_source_health(&mut self, mut section: PreviewSourceHealthSection) {
        section.canonicalize();
        if section_metadata_changed(&self.snapshot.source_health, &section) {
            self.snapshot.source_health = section;
            self.dirty.source_health = true;
        }
    }

    fn snapshot_for_publish(
        &self,
        clock: &LivePreviewClock,
        timeline: &LivePreviewTimeline,
    ) -> LivePreviewSnapshot {
        let mut snapshot = self.snapshot.clone();
        snapshot.captured_frame = clock.frame();
        snapshot.captured_monotonic_ms = clock.monotonic_ms();
        snapshot.timeline = timeline_preview(timeline);
        snapshot
    }

    fn clear_dirty(&mut self) {
        self.dirty.clear();
    }
}

fn section_metadata_changed<T>(before: &PreviewSection<T>, after: &PreviewSection<T>) -> bool {
    before.status != after.status
        || before.revision != after.revision
        || before.content_hash != after.content_hash
        || before.failure != after.failure
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
pub enum LivePreviewSet {
    AdvanceClock,
    Collect,
    Publish,
}

pub struct LivePreviewPlugin;

impl Plugin for LivePreviewPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(LivePreviewMonitorPlugin)
            .init_resource::<LivePreviewClock>()
            .init_resource::<LivePreviewScheduler>()
            .init_resource::<UiPreviewCollectorState>()
            .init_resource::<ScenePreviewCollectorState>()
            .init_resource::<LivePreviewCollectionBuffer>()
            .init_resource::<LivePreviewSnapshotHub>()
            .init_resource::<LivePreviewTimeline>()
            .init_resource::<LivePreviewSourceHealthRegistry>()
            .configure_sets(
                PostUpdate,
                (
                    LivePreviewSet::AdvanceClock,
                    LivePreviewSet::Collect,
                    LivePreviewSet::Publish,
                )
                    .chain(),
            )
            .add_systems(
                PostUpdate,
                advance_live_preview_clock.in_set(LivePreviewSet::AdvanceClock),
            )
            .add_systems(
                PostUpdate,
                collect_ui_preview.in_set(LivePreviewSet::Collect),
            )
            .add_systems(
                PostUpdate,
                collect_scene_preview.in_set(LivePreviewSet::Collect),
            )
            .add_systems(
                PostUpdate,
                publish_live_preview_snapshot.in_set(LivePreviewSet::Publish),
            );
    }
}

fn advance_live_preview_clock(time: Option<Res<Time<Real>>>, mut clock: ResMut<LivePreviewClock>) {
    let Some(time) = time else {
        return;
    };
    let delta_ms = (time.delta_secs_f64() * 1_000.0).max(0.0).round() as u64;
    clock.advance_millis(delta_ms);
}

fn publish_live_preview_snapshot(
    clock: Res<LivePreviewClock>,
    mut scheduler: ResMut<LivePreviewScheduler>,
    mut buffer: ResMut<LivePreviewCollectionBuffer>,
    mut hub: ResMut<LivePreviewSnapshotHub>,
    timeline: Res<LivePreviewTimeline>,
) {
    if !buffer.dirty().any() && !scheduler.heartbeat_due(clock.monotonic_ms()) {
        return;
    }

    let snapshot = buffer.snapshot_for_publish(&clock, &timeline);
    hub.writer().publish(snapshot);
    buffer.clear_dirty();
    scheduler.record_publish(clock.monotonic_ms());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::devtools::live_preview::{
        PreviewDataStatus, StablePreviewId, UiPanelPreview, UiPreviewState,
    };

    #[test]
    fn cadence_is_deterministic_and_rate_limited() {
        let mut scheduler = LivePreviewScheduler::default();
        assert!(scheduler.player_sample_due(0));
        scheduler.record_player_sample(0);
        assert!(!scheduler.player_sample_due(99));
        assert!(scheduler.player_sample_due(100));
        scheduler.record_statistics_sample(0);
        assert!(!scheduler.statistics_sample_due(499));
        assert!(scheduler.statistics_sample_due(500));
    }

    #[test]
    fn dirty_detection_uses_section_revision_and_hash() {
        let mut buffer = LivePreviewCollectionBuffer::default();
        let first = PreviewSection::available(
            1,
            UiPreviewState {
                panels: vec![UiPanelPreview {
                    id: StablePreviewId::from("panel"),
                    ..Default::default()
                }],
                ..Default::default()
            },
        );
        buffer.set_ui(first.clone());
        assert!(buffer.dirty().ui);
        buffer.clear_dirty();
        buffer.set_ui(first);
        assert!(!buffer.dirty().ui);
        buffer.set_ui(PreviewSection::not_collected());
        assert!(buffer.dirty().ui);
    }

    #[test]
    fn publish_records_frame_and_monotonic_time_and_heartbeat() {
        let mut clock = LivePreviewClock::default();
        let mut scheduler = LivePreviewScheduler::default();
        let mut buffer = LivePreviewCollectionBuffer::default();
        let mut hub = LivePreviewSnapshotHub::default();
        buffer.set_ui(PreviewSection::not_collected());
        assert_eq!(buffer.dirty().ui, false);
        clock.advance_millis(16);
        let timeline = LivePreviewTimeline::default();
        let snapshot = buffer.snapshot_for_publish(&clock, &timeline);
        hub.writer().publish(snapshot);
        buffer.clear_dirty();
        scheduler.record_publish(clock.monotonic_ms());
        assert_eq!(hub.read().sequence, 1);
        assert_eq!(hub.read().captured_frame, 1);
        assert_eq!(hub.read().captured_monotonic_ms, 16);
        assert!(!scheduler.heartbeat_due(2_015));
        assert!(scheduler.heartbeat_due(2_016));
    }

    #[test]
    fn available_sections_have_revision_and_missing_sections_are_explicit() {
        let mut buffer = LivePreviewCollectionBuffer::default();
        buffer.set_ui(PreviewSection::available(4, UiPreviewState::default()));
        assert_eq!(buffer.snapshot.ui.status, PreviewDataStatus::Available);
        assert_eq!(buffer.snapshot.ui.revision, Some(4));
        buffer.set_ui(PreviewSection::unavailable());
        assert_eq!(buffer.snapshot.ui.status, PreviewDataStatus::Unavailable);
        assert_eq!(buffer.snapshot.ui.revision, None);
    }
}
