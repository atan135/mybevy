//! Read-only, process-local live preview contracts.
//!
//! This module deliberately contains no game or MyServer types. Game code writes
//! through the adapter boundary in `game::devtools::live_preview`, while views
//! only receive an immutable snapshot reference.

mod collect_ui;
mod model;
mod plugin;
mod schedule;
mod source_health;
mod timeline;

pub use model::{
    LIVE_PREVIEW_SCHEMA_VERSION, LivePreviewSnapshot, LivePreviewSnapshotHub,
    LivePreviewSnapshotRead, NetworkPreviewSection, NetworkPreviewState, PerformancePreviewSection,
    PerformancePreviewState, PlayerPreviewSection, PlayerPreviewState, PreviewDataStatus,
    PreviewFailure, PreviewSection, PreviewSourceHealth, PreviewSourceHealthSection,
    PreviewSourceHealthState, ScenePreviewSection, ScenePreviewState, StablePreviewId,
    StablePreviewValue, UiPanelKindPreviewCounts, UiPanelPreview, UiPreviewSection, UiPreviewState,
};

pub use model::{LivePreviewPolicy, LivePreviewPolicy as LivePreviewConfig};
pub use plugin::LivePreviewPlugin;
pub use schedule::{
    LIVE_PREVIEW_HEARTBEAT_INTERVAL_MS, LIVE_PREVIEW_PLAYER_SAMPLE_INTERVAL_MS,
    LIVE_PREVIEW_STATISTICS_SAMPLE_INTERVAL_MS, LivePreviewClock, LivePreviewCollectionBuffer,
    LivePreviewDirtySections, LivePreviewScheduler, LivePreviewSet,
};
pub use source_health::{
    LIVE_PREVIEW_SOURCE_STALE_AFTER_MS, LivePreviewSourceHealthRegistry, LivePreviewSourceRecord,
    LivePreviewSourceStatus,
};
pub use timeline::{
    LIVE_PREVIEW_TIMELINE_MAX_DETAIL_CHARS, LIVE_PREVIEW_TIMELINE_MAX_SUMMARY_CHARS,
    LivePreviewTimeline, LivePreviewTimelineEvent, LivePreviewTimelineSeverity,
    LivePreviewTimelineType,
};
