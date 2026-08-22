//! Read-only, process-local live preview contracts.
//!
//! This module deliberately contains no game or MyServer types. Game code writes
//! through the adapter boundary in `game::devtools::live_preview`, while views
//! only receive an immutable snapshot reference.

mod model;

pub use model::{
    LIVE_PREVIEW_SCHEMA_VERSION, LivePreviewSnapshot, LivePreviewSnapshotHub,
    LivePreviewSnapshotRead, NetworkPreviewSection, NetworkPreviewState, PerformancePreviewSection,
    PerformancePreviewState, PlayerPreviewSection, PlayerPreviewState, PreviewDataStatus,
    PreviewFailure, PreviewSection, PreviewSourceHealth, PreviewSourceHealthSection,
    PreviewSourceHealthState, ScenePreviewSection, ScenePreviewState, StablePreviewId,
    StablePreviewValue, UiPanelPreview, UiPreviewSection, UiPreviewState,
};

pub use model::{LivePreviewPolicy, LivePreviewPolicy as LivePreviewConfig};
