use serde_json::{Map, Value};

use super::model::LivePreviewSnapshot;

pub(super) const LIVE_PREVIEW_MAX_EXPORT_BYTES: usize = 256 * 1024;

/// JSON keys that may leave the process. New snapshot fields are excluded until
/// deliberately added here, so a serde/Debug fallback cannot widen the export.
const ROOT_FIELDS: &[&str] = &[
    "schema_version",
    "sequence",
    "captured_frame",
    "captured_monotonic_ms",
    "base_sequence",
    "ui",
    "player",
    "scene",
    "network",
    "performance",
    "source_health",
    "timeline",
];
const SECTION_FIELDS: &[&str] = &["status", "revision", "content_hash", "value", "failure"];
const FAILURE_FIELDS: &[&str] = &["code", "summary"];
const UI_FIELDS: &[&str] = &[
    "canonical_screen",
    "screen_id",
    "owner",
    "panels",
    "pointer_blocked",
    "block_reason",
    "route_summary",
    "blocking_reason",
    "focus_panel_id",
    "focus_node_id",
    "ui_node_count",
    "visible_ui_node_count",
    "text_node_count",
    "panel_count",
    "panel_kind_counts",
    "document_id",
    "document_schema_version",
    "document_status",
    "document_source",
    "document_error",
    "viewport",
    "metrics",
];
const PANEL_FIELDS: &[&str] = &["id", "kind", "owner", "z_index", "active", "stack_index"];
const PANEL_COUNTS_FIELDS: &[&str] = &["page", "hud", "floating", "modal", "blocking_overlay"];
const VIEWPORT_FIELDS: &[&str] = &[
    "logical_width",
    "logical_height",
    "device_scale",
    "preview_scale",
    "width_class",
    "height_class",
    "orientation",
    "input_mode",
    "safe_area",
];
const METRICS_FIELDS: &[&str] = &[
    "page_padding",
    "panel_padding",
    "control_gap",
    "section_gap",
    "touch_target_min",
    "font_body",
    "content_max_width",
];
const PLAYER_FIELDS: &[&str] = &[
    "character_id",
    "display_name",
    "world_id",
    "selection_state",
    "attributes",
    "attributes_source",
    "attributes_snapshot_refreshed_at_ms",
    "attributes_push_sequence",
    "attributes_revision",
    "attributes_freshness",
    "position",
    "direction",
    "movement_state",
    "authority_frame",
];
const ATTRIBUTES_FIELDS: &[&str] = &["affinity", "mastery"];
const ELEMENT_FIELDS: &[&str] = &["earth", "fire", "water", "wind"];
const SCENE_FIELDS: &[&str] = &[
    "active_scene_id",
    "active_session_id",
    "pending_scene_id",
    "pending_session_id",
    "ready_scene_id",
    "ready_session_id",
    "scene_status",
    "lifecycle",
    "loading_phase",
    "loading_policy",
    "required_total",
    "required_loaded",
    "optional_total",
    "optional_loaded",
    "optional_failed",
    "loading_message_key",
    "authority_mode",
    "content_version",
    "seed",
    "scene_entity_count",
    "scene_root_count",
    "runtime_root_count",
    "layer_count",
    "layer_ids",
    "layers",
    "recent_error",
    "adapter_summary",
];
const LAYER_FIELDS: &[&str] = &["id", "session_id", "state", "required"];
const NETWORK_FIELDS: &[&str] = &[
    "session_status",
    "login_status",
    "registration_status",
    "character_selection_status",
    "connection_state",
    "transport",
    "connected",
    "authenticated",
    "room_id",
    "endpoint_kind",
    "endpoint_environment",
    "pending_request_count",
    "last_successful_receive_ms",
    "last_error_category",
    "reconnecting",
    "reconnect_phase",
    "authority_endpoint_kind",
    "authority_role",
    "authority_epoch",
    "authority_frame",
    "authority_last_activity_age_ms",
    "authority_sync_health",
];
const PERFORMANCE_FIELDS: &[&str] = &[
    "fps",
    "frame_time_ms",
    "collector_time_us",
    "ui_node_count",
    "scene_entity_count",
    "timeline_entry_count",
];
const SOURCE_FIELDS: &[&str] = &[
    "source_id",
    "status",
    "last_collected_frame",
    "revision",
    "detail",
];
const TIMELINE_FIELDS: &[&str] = &["capacity", "events"];
const TIMELINE_EVENT_FIELDS: &[&str] = &[
    "event_type",
    "severity",
    "timestamp_ms",
    "snapshot_sequence",
    "summary",
    "detail",
    "repeat_count",
];

pub(super) fn redacted_json(snapshot: &LivePreviewSnapshot) -> String {
    let mut canonical = snapshot.clone();
    canonical.canonicalize();
    let json = serde_json::to_string(&redacted_value(&canonical)).unwrap_or_else(|_| {
        format!(
            "{{\"schema_version\":{},\"sequence\":{},\"status\":\"export_failed\"}}",
            canonical.schema_version, canonical.sequence
        )
    });
    if json.len() <= LIVE_PREVIEW_MAX_EXPORT_BYTES {
        json
    } else {
        format!(
            "{{\"schema_version\":{},\"sequence\":{},\"status\":\"export_too_large\"}}",
            canonical.schema_version, canonical.sequence
        )
    }
}

pub(super) fn redacted_text(text: &str) -> String {
    let mut value = Value::String(text.to_owned());
    redact_strings(&mut value);
    value.as_str().unwrap_or("[redacted]").to_owned()
}

fn redacted_value(snapshot: &LivePreviewSnapshot) -> Value {
    let mut value = serde_json::to_value(snapshot).unwrap_or_else(|_| Value::Null);
    if let Value::Object(root) = &mut value {
        filter_object(root, ROOT_FIELDS, None);
        filter_section(root, "ui", UI_FIELDS);
        filter_section(root, "player", PLAYER_FIELDS);
        filter_section(root, "scene", SCENE_FIELDS);
        filter_section(root, "network", NETWORK_FIELDS);
        filter_section(root, "performance", PERFORMANCE_FIELDS);
        filter_section(root, "source_health", &["sources"]);
        filter_section(root, "timeline", TIMELINE_FIELDS);
        if let Some(Value::Object(timeline)) = root.get_mut("timeline") {
            if let Some(Value::Array(events)) = timeline.get_mut("events") {
                for event in events {
                    if let Value::Object(event) = event {
                        filter_object(event, TIMELINE_EVENT_FIELDS, None);
                    }
                }
            }
        }
    }
    value
}

fn filter_section(root: &mut Map<String, Value>, name: &str, value_fields: &[&str]) {
    let Some(Value::Object(section)) = root.get_mut(name) else {
        return;
    };
    filter_object(section, SECTION_FIELDS, None);
    if let Some(Value::Object(value)) = section.get_mut("value") {
        filter_object(value, value_fields, Some(name));
        filter_nested(value, name);
    }
    if let Some(Value::Object(failure)) = section.get_mut("failure") {
        filter_object(failure, FAILURE_FIELDS, None);
    }
}

fn filter_nested(value: &mut Map<String, Value>, section: &str) {
    match section {
        "ui" => {
            filter_array_objects(value, "panels", PANEL_FIELDS);
            filter_object_key(value, "panel_kind_counts", PANEL_COUNTS_FIELDS);
            filter_object_key(value, "viewport", VIEWPORT_FIELDS);
            filter_object_key(value, "metrics", METRICS_FIELDS);
        }
        "player" => {
            filter_object_key(value, "attributes", ATTRIBUTES_FIELDS);
            if let Some(Value::Object(attributes)) = value.get_mut("attributes") {
                filter_object_key(attributes, "affinity", ELEMENT_FIELDS);
                filter_object_key(attributes, "mastery", ELEMENT_FIELDS);
            }
        }
        "scene" => filter_array_objects(value, "layers", LAYER_FIELDS),
        "source_health" => filter_array_objects(value, "sources", SOURCE_FIELDS),
        _ => {}
    }
}

fn filter_array_objects(value: &mut Map<String, Value>, key: &str, fields: &[&str]) {
    if let Some(Value::Array(items)) = value.get_mut(key) {
        for item in items {
            if let Value::Object(item) = item {
                filter_object(item, fields, None);
            }
        }
    }
}

fn filter_object_key(value: &mut Map<String, Value>, key: &str, fields: &[&str]) {
    if let Some(Value::Object(object)) = value.get_mut(key) {
        filter_object(object, fields, None);
    }
}

fn filter_object(object: &mut Map<String, Value>, fields: &[&str], _section: Option<&str>) {
    object.retain(|key, value| {
        if !fields.contains(&key.as_str()) || sensitive_key(key) {
            return false;
        }
        redact_strings(value);
        true
    });
}

fn sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    [
        "password",
        "token",
        "ticket",
        "authorization",
        "cookie",
        "binding",
        "chat_body",
        "mail_body",
        "access_secret",
        "refresh_secret",
        "credential",
    ]
    .iter()
    .any(|needle| key.contains(needle))
}

fn redact_strings(value: &mut Value) {
    match value {
        Value::String(text) => {
            let lower = text.to_ascii_lowercase();
            if looks_like_absolute_path(text)
                || [
                    "authorization:",
                    "bearer ",
                    "access_token",
                    "refresh_token",
                    "ticket=",
                    "password=",
                    "cookie=",
                    "chat_body",
                    "mail_body",
                    "binding_value",
                ]
                .iter()
                .any(|needle| lower.contains(needle))
            {
                *text = "[redacted]".to_owned();
            }
        }
        Value::Array(items) => items.iter_mut().for_each(redact_strings),
        Value::Object(object) => object.values_mut().for_each(redact_strings),
        _ => {}
    }
}

fn looks_like_absolute_path(text: &str) -> bool {
    text.starts_with('/')
        || text.starts_with('\\')
        || text.as_bytes().get(1).is_some_and(|byte| *byte == b':')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::devtools::live_preview::{
        LivePreviewTimelineEventPreview, NetworkPreviewState, PreviewSection, StablePreviewId,
    };

    #[test]
    fn redaction_removes_unknown_and_sensitive_network_fields() {
        let mut snapshot = LivePreviewSnapshot::default();
        snapshot.network = PreviewSection::available(
            1,
            NetworkPreviewState {
                endpoint_detail: Some("https://user:password@10.0.0.1:443".to_owned()),
                endpoint_kind: Some("game".to_owned()),
                ..Default::default()
            },
        );
        let json = redacted_json(&snapshot);
        assert!(json.contains("endpoint_kind"));
        assert!(!json.contains("endpoint_detail"));
        assert!(!json.contains("10.0.0.1"));
    }

    #[test]
    fn redaction_scrubs_timeline_text_and_absolute_paths() {
        let mut snapshot = LivePreviewSnapshot::default();
        snapshot
            .timeline
            .events
            .push(LivePreviewTimelineEventPreview {
            summary:
                "password=secret refresh_token=rt ticket=tk cookie=ck C:\\Users\\player\\mail.txt"
                    .to_owned(),
            detail: Some(
                "Authorization: Bearer abc chat_body=hello mail_body=world binding_value=x"
                    .to_owned(),
            ),
            ..Default::default()
        });
        let json = redacted_json(&snapshot);
        assert!(!json.contains("secret"));
        assert!(!json.contains("C:\\\\Users"));
        assert!(!json.contains("Bearer abc"));
        assert!(!json.contains("refresh_token=rt"));
        assert!(!json.contains("chat_body=hello"));
        assert!(!json.contains("binding_value=x"));
    }

    #[test]
    fn redaction_is_deterministic_and_preserves_sequence() {
        let mut snapshot = LivePreviewSnapshot::default();
        snapshot.sequence = 42;
        snapshot.ui = PreviewSection::available(
            2,
            crate::framework::devtools::live_preview::UiPreviewState {
                panels: vec![
                    crate::framework::devtools::live_preview::UiPanelPreview {
                        id: StablePreviewId::from("z"),
                        ..Default::default()
                    },
                    crate::framework::devtools::live_preview::UiPanelPreview {
                        id: StablePreviewId::from("a"),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            },
        );
        let first = redacted_json(&snapshot);
        let second = redacted_json(&snapshot);
        assert_eq!(first, second);
        assert!(first.contains("\"sequence\":42"));
        assert!(first.contains("\"schema_version\":2"));
        assert!(first.find("\"id\":\"a\"").unwrap() < first.find("\"id\":\"z\"").unwrap());
    }
}
