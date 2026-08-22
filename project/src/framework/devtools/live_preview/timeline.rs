use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

use super::model::StablePreviewId;

pub const LIVE_PREVIEW_TIMELINE_MAX_SUMMARY_CHARS: usize = 256;
pub const LIVE_PREVIEW_TIMELINE_MAX_DETAIL_CHARS: usize = 1_024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LivePreviewTimelineType {
    Ui,
    Player,
    Scene,
    Network,
    Performance,
    SourceHealth,
    Custom(StablePreviewId),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LivePreviewTimelineSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LivePreviewTimelineEvent {
    pub event_type: LivePreviewTimelineType,
    pub severity: LivePreviewTimelineSeverity,
    pub timestamp_ms: u64,
    pub snapshot_sequence: u64,
    pub summary: String,
    pub detail: Option<String>,
    pub repeat_count: u32,
}

impl LivePreviewTimelineEvent {
    pub fn new(
        event_type: LivePreviewTimelineType,
        severity: LivePreviewTimelineSeverity,
        timestamp_ms: u64,
        snapshot_sequence: u64,
        summary: impl AsRef<str>,
        detail: Option<String>,
    ) -> Self {
        Self {
            event_type,
            severity,
            timestamp_ms,
            snapshot_sequence,
            summary: sanitize_text(summary.as_ref(), LIVE_PREVIEW_TIMELINE_MAX_SUMMARY_CHARS),
            detail: detail
                .map(|value| sanitize_text(&value, LIVE_PREVIEW_TIMELINE_MAX_DETAIL_CHARS)),
            repeat_count: 1,
        }
    }

    fn same_continuous_event(&self, other: &Self) -> bool {
        self.event_type == other.event_type
            && self.severity == other.severity
            && self.summary == other.summary
            && self.detail == other.detail
    }
}

fn sanitize_text(text: &str, max_chars: usize) -> String {
    text.chars()
        .filter(|character| !character.is_control() || *character == '\t')
        .take(max_chars)
        .collect()
}

#[derive(Clone, Debug, Resource)]
pub struct LivePreviewTimeline {
    capacity: usize,
    events: VecDeque<LivePreviewTimelineEvent>,
}

impl Default for LivePreviewTimeline {
    fn default() -> Self {
        Self::with_capacity(128)
    }
}

impl LivePreviewTimeline {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity,
            events: VecDeque::with_capacity(capacity),
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn set_capacity(&mut self, capacity: usize) {
        self.capacity = capacity;
        while self.events.len() > capacity {
            self.events.pop_front();
        }
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &LivePreviewTimelineEvent> {
        self.events.iter()
    }

    pub fn push(&mut self, mut event: LivePreviewTimelineEvent) {
        if self.capacity == 0 {
            return;
        }
        if let Some(previous) = self.events.back_mut()
            && previous.same_continuous_event(&event)
        {
            previous.timestamp_ms = event.timestamp_ms;
            previous.snapshot_sequence = event.snapshot_sequence;
            previous.repeat_count = previous.repeat_count.saturating_add(event.repeat_count);
            return;
        }
        event.repeat_count = event.repeat_count.max(1);
        while self.events.len() >= self.capacity {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeline_is_ordered_and_evicts_oldest_at_capacity() {
        let mut timeline = LivePreviewTimeline::with_capacity(2);
        for index in 0..3 {
            timeline.push(LivePreviewTimelineEvent::new(
                LivePreviewTimelineType::Scene,
                LivePreviewTimelineSeverity::Info,
                index,
                index,
                format!("scene {index}"),
                None,
            ));
        }
        let timestamps: Vec<_> = timeline.iter().map(|event| event.timestamp_ms).collect();
        assert_eq!(timestamps, vec![1, 2]);
    }

    #[test]
    fn identical_continuous_events_merge_without_growing() {
        let mut timeline = LivePreviewTimeline::with_capacity(4);
        timeline.push(LivePreviewTimelineEvent::new(
            LivePreviewTimelineType::Player,
            LivePreviewTimelineSeverity::Info,
            1,
            1,
            "position sampled",
            None,
        ));
        timeline.push(LivePreviewTimelineEvent::new(
            LivePreviewTimelineType::Player,
            LivePreviewTimelineSeverity::Info,
            2,
            2,
            "position sampled",
            None,
        ));
        assert_eq!(timeline.len(), 1);
        let event = timeline.iter().next().unwrap();
        assert_eq!(event.repeat_count, 2);
        assert_eq!(event.timestamp_ms, 2);
        assert_eq!(event.snapshot_sequence, 2);
    }

    #[test]
    fn event_text_is_bounded_and_controlled() {
        let event = LivePreviewTimelineEvent::new(
            LivePreviewTimelineType::Network,
            LivePreviewTimelineSeverity::Warning,
            1,
            1,
            "line\nbreak",
            Some("detail\r\nvalue".to_string()),
        );
        assert_eq!(event.summary, "linebreak");
        assert_eq!(event.detail.as_deref(), Some("detailvalue"));
    }
}
