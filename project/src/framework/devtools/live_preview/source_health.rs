use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::model::StablePreviewId;
use super::timeline::{
    LivePreviewTimelineEvent, LivePreviewTimelineSeverity, LivePreviewTimelineType,
};

pub const LIVE_PREVIEW_SOURCE_STALE_AFTER_MS: u64 = 2_000;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LivePreviewSourceStatus {
    Healthy,
    Stale,
    Unavailable,
    Failed,
}

impl LivePreviewSourceStatus {
    fn severity(self) -> LivePreviewTimelineSeverity {
        match self {
            Self::Healthy => LivePreviewTimelineSeverity::Info,
            Self::Stale | Self::Unavailable => LivePreviewTimelineSeverity::Warning,
            Self::Failed => LivePreviewTimelineSeverity::Error,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LivePreviewSourceRecord {
    pub source_id: StablePreviewId,
    pub status: LivePreviewSourceStatus,
    pub last_seen_ms: Option<u64>,
    pub status_changed_ms: u64,
}

#[derive(Clone, Debug, Resource)]
pub struct LivePreviewSourceHealthRegistry {
    stale_after_ms: u64,
    sources: BTreeMap<StablePreviewId, LivePreviewSourceRecord>,
}

impl Default for LivePreviewSourceHealthRegistry {
    fn default() -> Self {
        Self::with_stale_after_ms(LIVE_PREVIEW_SOURCE_STALE_AFTER_MS)
    }
}

impl LivePreviewSourceHealthRegistry {
    pub fn with_stale_after_ms(stale_after_ms: u64) -> Self {
        Self {
            stale_after_ms,
            sources: BTreeMap::new(),
        }
    }

    pub fn stale_after_ms(&self) -> u64 {
        self.stale_after_ms
    }

    pub fn get(&self, source_id: &StablePreviewId) -> Option<&LivePreviewSourceRecord> {
        self.sources.get(source_id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &LivePreviewSourceRecord> {
        self.sources.values()
    }

    /// Records an explicit collector result and emits an event only when the
    /// source changes state. A transition back to healthy is always a recovery
    /// event, so it cannot be hidden by a stale heartbeat.
    pub fn observe(
        &mut self,
        source_id: impl Into<StablePreviewId>,
        status: LivePreviewSourceStatus,
        now_ms: u64,
        snapshot_sequence: u64,
    ) -> Option<LivePreviewTimelineEvent> {
        let source_id = source_id.into();
        let previous = self.sources.get(&source_id).map(|record| record.status);
        let record =
            self.sources
                .entry(source_id.clone())
                .or_insert_with(|| LivePreviewSourceRecord {
                    source_id: source_id.clone(),
                    status,
                    last_seen_ms: None,
                    status_changed_ms: now_ms,
                });

        if status == LivePreviewSourceStatus::Healthy {
            record.last_seen_ms = Some(now_ms);
        }
        if previous == Some(status) {
            return None;
        }
        record.status = status;
        record.status_changed_ms = now_ms;
        Some(source_health_event(
            &source_id,
            previous,
            status,
            now_ms,
            snapshot_sequence,
        ))
    }

    pub fn mark_stale(
        &mut self,
        now_ms: u64,
        snapshot_sequence: u64,
    ) -> Vec<LivePreviewTimelineEvent> {
        let stale_after_ms = self.stale_after_ms;
        let mut events = Vec::new();
        for record in self.sources.values_mut() {
            let Some(last_seen_ms) = record.last_seen_ms else {
                continue;
            };
            if record.status == LivePreviewSourceStatus::Healthy
                && now_ms.saturating_sub(last_seen_ms) >= stale_after_ms
            {
                let previous = record.status;
                record.status = LivePreviewSourceStatus::Stale;
                record.status_changed_ms = now_ms;
                events.push(source_health_event(
                    &record.source_id,
                    Some(previous),
                    LivePreviewSourceStatus::Stale,
                    now_ms,
                    snapshot_sequence,
                ));
            }
        }
        events
    }
}

fn source_health_event(
    source_id: &StablePreviewId,
    previous: Option<LivePreviewSourceStatus>,
    status: LivePreviewSourceStatus,
    now_ms: u64,
    snapshot_sequence: u64,
) -> LivePreviewTimelineEvent {
    let recovered = status == LivePreviewSourceStatus::Healthy
        && previous.is_some_and(|value| value != LivePreviewSourceStatus::Healthy);
    let summary = if recovered {
        "source recovered"
    } else {
        "source health changed"
    };
    LivePreviewTimelineEvent::new(
        LivePreviewTimelineType::SourceHealth,
        status.severity(),
        now_ms,
        snapshot_sequence,
        summary,
        Some(source_id.as_str().to_string()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_health_reports_stale_and_recovery() {
        let mut registry = LivePreviewSourceHealthRegistry::with_stale_after_ms(100);
        let initial = registry.observe("ui", LivePreviewSourceStatus::Healthy, 0, 1);
        assert!(initial.is_some());
        assert!(registry.mark_stale(99, 2).is_empty());
        let stale = registry.mark_stale(100, 3);
        assert_eq!(stale.len(), 1);
        assert_eq!(
            registry.get(&StablePreviewId::from("ui")).unwrap().status,
            LivePreviewSourceStatus::Stale
        );
        let recovered = registry
            .observe("ui", LivePreviewSourceStatus::Healthy, 101, 4)
            .unwrap();
        assert_eq!(recovered.summary, "source recovered");
        assert_eq!(
            registry.get(&StablePreviewId::from("ui")).unwrap().status,
            LivePreviewSourceStatus::Healthy
        );
    }

    #[test]
    fn repeated_status_does_not_emit_events() {
        let mut registry = LivePreviewSourceHealthRegistry::default();
        assert!(
            registry
                .observe("network", LivePreviewSourceStatus::Unavailable, 0, 1)
                .is_some()
        );
        assert!(
            registry
                .observe("network", LivePreviewSourceStatus::Unavailable, 1, 2)
                .is_none()
        );
    }
}
