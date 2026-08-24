use std::collections::HashMap;

use bevy::prelude::{
    App, IntoScheduleConfigs, Message, MessageReader, MessageWriter, Plugin, Res, ResMut, Resource,
    Update,
};
use serde_json::Value;

use crate::framework::network::ConnectionId;

use super::protocol::pb;
use super::types::redact_secret_fingerprint;
use super::{MyServerEvent, MyServerSession, MyServerUpdateSet};

pub const MAX_ACTIVITY_PROGRESS_JSON_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivityRequestKind {
    List,
    Detail,
    Progress,
    Claim,
    Action,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivitySummary {
    pub activity_id: String,
    pub version: u32,
    pub activity_type: String,
    pub status: String,
    pub start_at_ms: i64,
    pub end_at_ms: i64,
    pub claim_deadline_ms: i64,
    pub timezone: String,
}

impl ActivitySummary {
    fn from_proto(value: pb::ActivitySummary) -> Option<Self> {
        (!value.activity_id.trim().is_empty() && value.version != 0).then_some(Self {
            activity_id: value.activity_id,
            version: value.version,
            activity_type: value.activity_type,
            status: value.status,
            start_at_ms: value.start_at_ms,
            end_at_ms: value.end_at_ms,
            claim_deadline_ms: value.claim_deadline_ms,
            timezone: value.timezone,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ActivityProgressView {
    pub type_name: String,
    pub schema_version: u32,
    pub value: Value,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivityErrorKind {
    AuthenticationRequired,
    EngineUnavailable,
    QualificationNotMet,
    AlreadyClaimed,
    VersionConflict,
    Processing,
    RetryableFailure,
    ReconciliationPending,
    ManualReview,
    StorageUnavailable,
    InvalidProgress,
    ResponseMismatch,
    Transport,
    Unknown,
}

impl ActivityErrorKind {
    pub const fn message_key(&self) -> &'static str {
        match self {
            Self::AuthenticationRequired => "activity.error.authentication_required",
            Self::EngineUnavailable => "activity.error.engine_unavailable",
            Self::QualificationNotMet => "activity.error.qualification_not_met",
            Self::AlreadyClaimed => "activity.error.already_claimed",
            Self::VersionConflict => "activity.error.version_conflict",
            Self::Processing => "activity.error.processing",
            Self::RetryableFailure => "activity.error.retryable_failure",
            Self::ReconciliationPending => "activity.error.reconciliation_pending",
            Self::ManualReview => "activity.error.manual_review",
            Self::StorageUnavailable => "activity.error.storage_unavailable",
            Self::InvalidProgress => "activity.error.invalid_progress",
            Self::ResponseMismatch => "activity.error.response_mismatch",
            Self::Transport => "activity.error.transport",
            Self::Unknown => "activity.error.unknown",
        }
    }

    pub const fn retryable(&self) -> bool {
        matches!(
            self,
            Self::EngineUnavailable
                | Self::Processing
                | Self::RetryableFailure
                | Self::StorageUnavailable
                | Self::Transport
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivityError {
    pub kind: ActivityErrorKind,
    pub error_code: String,
    pub message_key: &'static str,
    pub retryable: bool,
}

impl ActivityError {
    fn from_code(code: impl AsRef<str>) -> Self {
        let error_code = code.as_ref().trim().to_ascii_uppercase();
        let kind = if error_code == "ACTIVITY_ENGINE_UNAVAILABLE" {
            ActivityErrorKind::EngineUnavailable
        } else if error_code == "ACTIVITY_AUTH_REQUIRED" {
            ActivityErrorKind::AuthenticationRequired
        } else if error_code == "ACTIVITY_QUALIFICATION_NOT_MET" {
            ActivityErrorKind::QualificationNotMet
        } else if error_code == "ACTIVITY_ALREADY_CLAIMED" {
            ActivityErrorKind::AlreadyClaimed
        } else if matches!(
            error_code.as_str(),
            "ACTIVITY_VERSION_CONFLICT" | "ACTIVITY_INVALID_VERSION"
        ) {
            ActivityErrorKind::VersionConflict
        } else if error_code == "ACTIVITY_PROCESSING" {
            ActivityErrorKind::Processing
        } else if error_code == "ACTIVITY_RETRYABLE_FAILURE" {
            ActivityErrorKind::RetryableFailure
        } else if error_code == "ACTIVITY_RECONCILIATION_PENDING" {
            ActivityErrorKind::ReconciliationPending
        } else if error_code == "ACTIVITY_MANUAL_REVIEW" {
            ActivityErrorKind::ManualReview
        } else if error_code == "ACTIVITY_STORAGE_UNAVAILABLE" {
            ActivityErrorKind::StorageUnavailable
        } else {
            ActivityErrorKind::Unknown
        };
        Self {
            message_key: kind.message_key(),
            retryable: kind.retryable(),
            kind,
            error_code,
        }
    }

    fn transport() -> Self {
        let kind = ActivityErrorKind::Transport;
        Self {
            kind,
            error_code: "ACTIVITY_TRANSPORT_FAILURE".to_owned(),
            message_key: kind.message_key(),
            retryable: kind.retryable(),
        }
    }

    fn invalid_progress() -> Self {
        let kind = ActivityErrorKind::InvalidProgress;
        Self {
            kind,
            error_code: "ACTIVITY_PROGRESS_INVALID".to_owned(),
            message_key: kind.message_key(),
            retryable: false,
        }
    }

    fn response_mismatch() -> Self {
        let kind = ActivityErrorKind::ResponseMismatch;
        Self {
            kind,
            error_code: "ACTIVITY_RESPONSE_MISMATCH".to_owned(),
            message_key: kind.message_key(),
            retryable: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ActivityKey {
    pub activity_id: String,
    pub version: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ActivityDetail {
    pub summary: ActivitySummary,
    pub progress: Option<ActivityProgressView>,
    pub state_revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ActivityScope {
    character_id: String,
    connection_id: ConnectionId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TrackedActivityRequest {
    kind: ActivityRequestKind,
    activity: Option<ActivityKey>,
    stage_id: Option<String>,
    action_type: Option<String>,
    request_id_fingerprint: Option<String>,
    scope: ActivityScope,
}

#[derive(Clone, Debug, Default)]
pub struct ActivityRequestTracker {
    pending: HashMap<u32, TrackedActivityRequest>,
}

impl ActivityRequestTracker {
    fn record(
        &mut self,
        seq: u32,
        kind: ActivityRequestKind,
        activity: Option<ActivityKey>,
        stage_id: Option<&str>,
        action_type: Option<&str>,
        client_request_id: Option<&str>,
        scope: ActivityScope,
    ) {
        self.pending.insert(
            seq,
            TrackedActivityRequest {
                kind,
                activity,
                stage_id: stage_id.map(str::to_owned),
                action_type: action_type.map(str::to_owned),
                request_id_fingerprint: client_request_id.map(redact_secret_fingerprint),
                scope,
            },
        );
    }

    fn complete(
        &mut self,
        seq: u32,
        connection_id: ConnectionId,
    ) -> Option<TrackedActivityRequest> {
        let tracked = self.pending.remove(&seq)?;
        (tracked.scope.connection_id == connection_id).then_some(tracked)
    }

    fn clear(&mut self) {
        self.pending.clear();
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.pending.len()
    }
}

#[derive(Clone, Debug, Message)]
pub enum ActivityEvent {
    ScopeChanged,
    ListUpdated,
    DetailUpdated {
        activity_id: String,
        version: u32,
    },
    ProgressUpdated {
        activity_id: String,
        version: u32,
    },
    ActionResolved {
        activity_id: String,
        version: u32,
        action_type: String,
        ok: bool,
        processing: bool,
        duplicate: bool,
    },
    RequestFailed {
        kind: ActivityRequestKind,
        error: ActivityError,
    },
}

#[derive(Clone, Debug, Default, Resource)]
pub struct ActivityClientState {
    scope: Option<ActivityScope>,
    pub generation: u64,
    pub activities: Vec<ActivitySummary>,
    pub selected: Option<ActivityKey>,
    pub details: HashMap<ActivityKey, ActivityDetail>,
    pub last_error: Option<ActivityError>,
    pub request_tracker: ActivityRequestTracker,
}

impl ActivityClientState {
    fn sync_scope(&mut self, session: &MyServerSession) -> bool {
        let scope = match (session.character_id.clone(), session.connection_id) {
            (Some(character_id), Some(connection_id)) if session.authenticated => {
                Some(ActivityScope {
                    character_id,
                    connection_id,
                })
            }
            _ => None,
        };
        if self.scope == scope {
            return false;
        }
        self.scope = scope;
        self.generation = self.generation.wrapping_add(1);
        self.activities.clear();
        self.selected = None;
        self.details.clear();
        self.last_error = None;
        self.request_tracker.clear();
        true
    }

    pub(crate) fn record_request(
        &mut self,
        session: &MyServerSession,
        seq: u32,
        kind: ActivityRequestKind,
        activity: Option<ActivityKey>,
        stage_id: Option<&str>,
        action_type: Option<&str>,
        client_request_id: Option<&str>,
    ) {
        self.sync_scope(session);
        let Some(scope) = self.scope.clone() else {
            return;
        };
        self.request_tracker.record(
            seq,
            kind,
            activity,
            stage_id,
            action_type,
            client_request_id,
            scope,
        );
    }

    fn complete_request(
        &mut self,
        seq: u32,
        connection_id: ConnectionId,
    ) -> Option<TrackedActivityRequest> {
        let tracked = self.request_tracker.complete(seq, connection_id)?;
        (self.scope.as_ref() == Some(&tracked.scope)).then_some(tracked)
    }

    fn fail(&mut self, error: ActivityError) {
        self.last_error = Some(error);
    }
}

pub(crate) struct ActivityPlugin;

impl Plugin for ActivityPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActivityClientState>()
            .add_message::<ActivityEvent>()
            .add_systems(
                Update,
                (
                    sync_activity_scope.in_set(MyServerUpdateSet::AutoClient),
                    apply_activity_responses
                        .after(sync_activity_scope)
                        .in_set(MyServerUpdateSet::AutoClient),
                ),
            );
    }
}

fn sync_activity_scope(
    session: Res<MyServerSession>,
    mut state: ResMut<ActivityClientState>,
    mut activity_events: MessageWriter<ActivityEvent>,
) {
    if state.sync_scope(&session) {
        activity_events.write(ActivityEvent::ScopeChanged);
    }
}

fn apply_activity_responses(
    mut myserver_events: MessageReader<MyServerEvent>,
    mut state: ResMut<ActivityClientState>,
    mut activity_events: MessageWriter<ActivityEvent>,
) {
    for event in myserver_events.read() {
        match event {
            MyServerEvent::ActivityListResponse {
                seq,
                connection_id,
                response,
            } => apply_list_response(
                &mut state,
                &mut activity_events,
                *seq,
                *connection_id,
                response,
            ),
            MyServerEvent::ActivityDetailResponse {
                seq,
                connection_id,
                response,
            } => apply_detail_response(
                &mut state,
                &mut activity_events,
                *seq,
                *connection_id,
                response,
            ),
            MyServerEvent::ActivityProgressResponse {
                seq,
                connection_id,
                response,
            } => apply_progress_response(
                &mut state,
                &mut activity_events,
                *seq,
                *connection_id,
                response,
            ),
            MyServerEvent::ActivityClaimResponse {
                seq,
                connection_id,
                response,
            } => apply_claim_response(
                &mut state,
                &mut activity_events,
                *seq,
                *connection_id,
                response,
            ),
            MyServerEvent::ActivityActionResponse {
                seq,
                connection_id,
                response,
            } => apply_action_response(
                &mut state,
                &mut activity_events,
                *seq,
                *connection_id,
                response,
            ),
            MyServerEvent::ActivityProtocolFailed {
                seq, connection_id, ..
            } => {
                if let Some(tracked) = state.complete_request(*seq, *connection_id) {
                    let error = ActivityError::transport();
                    state.fail(error.clone());
                    activity_events.write(ActivityEvent::RequestFailed {
                        kind: tracked.kind,
                        error,
                    });
                }
            }
            MyServerEvent::ActivityRequestRejected { kind, error_code } => {
                let error = ActivityError::from_code(error_code);
                state.fail(error.clone());
                activity_events.write(ActivityEvent::RequestFailed { kind: *kind, error });
            }
            MyServerEvent::RequestFailed {
                seq: Some(seq),
                message_type: Some(message_type),
                ..
            } if is_activity_request(*message_type) => {
                if let Some(tracked) = state.request_tracker.pending.remove(seq) {
                    let error = ActivityError::transport();
                    state.fail(error.clone());
                    activity_events.write(ActivityEvent::RequestFailed {
                        kind: tracked.kind,
                        error,
                    });
                }
            }
            MyServerEvent::Error {
                seq, error_code, ..
            } => {
                if let Some(tracked) = state.request_tracker.pending.remove(seq) {
                    let error = ActivityError::from_code(error_code);
                    state.fail(error.clone());
                    activity_events.write(ActivityEvent::RequestFailed {
                        kind: tracked.kind,
                        error,
                    });
                }
            }
            _ => {}
        }
    }
}

fn apply_list_response(
    state: &mut ActivityClientState,
    events: &mut MessageWriter<ActivityEvent>,
    seq: u32,
    connection_id: ConnectionId,
    response: &pb::ActivityListRes,
) {
    let Some(tracked) = state.complete_request(seq, connection_id) else {
        return;
    };
    if tracked.kind != ActivityRequestKind::List {
        return response_mismatch(state, events, tracked.kind);
    }
    if !response.ok {
        return request_error(state, events, tracked.kind, &response.error_code);
    }
    state.activities = response
        .activities
        .iter()
        .cloned()
        .filter_map(ActivitySummary::from_proto)
        .collect();
    state.last_error = None;
    events.write(ActivityEvent::ListUpdated);
}

fn apply_detail_response(
    state: &mut ActivityClientState,
    events: &mut MessageWriter<ActivityEvent>,
    seq: u32,
    connection_id: ConnectionId,
    response: &pb::ActivityDetailRes,
) {
    let Some(tracked) = state.complete_request(seq, connection_id) else {
        return;
    };
    let Some(key) = tracked.activity else {
        return response_mismatch(state, events, tracked.kind);
    };
    if tracked.kind != ActivityRequestKind::Detail || !response.ok {
        return if response.ok {
            response_mismatch(state, events, tracked.kind)
        } else {
            request_error(state, events, tracked.kind, &response.error_code)
        };
    }
    let Some(summary) = response
        .activity
        .clone()
        .and_then(ActivitySummary::from_proto)
    else {
        return response_mismatch(state, events, tracked.kind);
    };
    if summary.activity_id != key.activity_id || summary.version != key.version {
        return response_mismatch(state, events, tracked.kind);
    }
    let progress = match parse_progress_json(&response.progress_json) {
        Ok(progress) => progress,
        Err(()) => return invalid_progress(state, events, tracked.kind),
    };
    state.details.insert(
        key.clone(),
        ActivityDetail {
            summary,
            progress,
            state_revision: response.state_revision,
        },
    );
    state.selected = Some(key.clone());
    state.last_error = None;
    events.write(ActivityEvent::DetailUpdated {
        activity_id: key.activity_id,
        version: key.version,
    });
}

fn apply_progress_response(
    state: &mut ActivityClientState,
    events: &mut MessageWriter<ActivityEvent>,
    seq: u32,
    connection_id: ConnectionId,
    response: &pb::ActivityProgressRes,
) {
    let Some(tracked) = state.complete_request(seq, connection_id) else {
        return;
    };
    let Some(key) = tracked.activity else {
        return response_mismatch(state, events, tracked.kind);
    };
    if tracked.kind != ActivityRequestKind::Progress
        || !response.ok
        || response.activity_id != key.activity_id
        || response.version != key.version
    {
        return if response.ok {
            response_mismatch(state, events, tracked.kind)
        } else {
            request_error(state, events, tracked.kind, &response.error_code)
        };
    }
    let progress = match parse_progress_json(&response.progress_json) {
        Ok(progress) => progress,
        Err(()) => return invalid_progress(state, events, tracked.kind),
    };
    if let Some(detail) = state.details.get_mut(&key) {
        // The server currently returns `{}` from the progress route. Preserve
        // the last trusted detail view until that route gains a typed payload.
        if progress.is_some() {
            detail.progress = progress;
        }
        detail.state_revision = detail.state_revision.max(response.state_revision);
    }
    state.last_error = None;
    events.write(ActivityEvent::ProgressUpdated {
        activity_id: key.activity_id,
        version: key.version,
    });
}

fn apply_claim_response(
    state: &mut ActivityClientState,
    events: &mut MessageWriter<ActivityEvent>,
    seq: u32,
    connection_id: ConnectionId,
    response: &pb::ActivityClaimRes,
) {
    let Some(tracked) = state.complete_request(seq, connection_id) else {
        return;
    };
    if tracked.kind != ActivityRequestKind::Claim
        || !write_response_stage_matches(&tracked, &response.stage_id)
        || tracked
            .request_id_fingerprint
            .as_deref()
            .is_some_and(|fingerprint| {
                redact_secret_fingerprint(&response.client_request_id) != fingerprint
            })
    {
        return response_mismatch(state, events, tracked.kind);
    }
    let Some(key) = tracked.activity else {
        return response_mismatch(state, events, tracked.kind);
    };
    if response.activity_id != key.activity_id || response.version != key.version {
        return response_mismatch(state, events, tracked.kind);
    }
    if !response.ok {
        return request_error(state, events, tracked.kind, &response.error_code);
    }
    state.last_error = None;
    events.write(ActivityEvent::ActionResolved {
        activity_id: key.activity_id,
        version: key.version,
        action_type: "claim".to_owned(),
        ok: response.ok,
        processing: response.processing,
        duplicate: response.duplicate,
    });
}

fn apply_action_response(
    state: &mut ActivityClientState,
    events: &mut MessageWriter<ActivityEvent>,
    seq: u32,
    connection_id: ConnectionId,
    response: &pb::ActivityActionRes,
) {
    let Some(tracked) = state.complete_request(seq, connection_id) else {
        return;
    };
    if tracked.kind != ActivityRequestKind::Action
        || !write_response_stage_matches(&tracked, &response.stage_id)
        || tracked
            .request_id_fingerprint
            .as_deref()
            .is_some_and(|fingerprint| {
                redact_secret_fingerprint(&response.client_request_id) != fingerprint
            })
    {
        return response_mismatch(state, events, tracked.kind);
    }
    let Some(key) = tracked.activity else {
        return response_mismatch(state, events, tracked.kind);
    };
    let Some(action_type) = tracked.action_type else {
        return response_mismatch(state, events, tracked.kind);
    };
    if response.activity_id != key.activity_id
        || response.version != key.version
        || response.action_type != action_type
    {
        return response_mismatch(state, events, tracked.kind);
    }
    if !response.ok {
        return request_error(state, events, tracked.kind, &response.error_code);
    }
    state.last_error = None;
    events.write(ActivityEvent::ActionResolved {
        activity_id: key.activity_id,
        version: key.version,
        action_type,
        ok: response.ok,
        processing: response.processing,
        duplicate: response.duplicate,
    });
}

fn request_error(
    state: &mut ActivityClientState,
    events: &mut MessageWriter<ActivityEvent>,
    kind: ActivityRequestKind,
    code: &str,
) {
    let error = ActivityError::from_code(code);
    state.fail(error.clone());
    events.write(ActivityEvent::RequestFailed { kind, error });
}

fn write_response_stage_matches(tracked: &TrackedActivityRequest, response_stage_id: &str) -> bool {
    tracked.stage_id.as_deref() == Some(response_stage_id)
}

fn invalid_progress(
    state: &mut ActivityClientState,
    events: &mut MessageWriter<ActivityEvent>,
    kind: ActivityRequestKind,
) {
    let error = ActivityError::invalid_progress();
    state.fail(error.clone());
    events.write(ActivityEvent::RequestFailed { kind, error });
}

fn response_mismatch(
    state: &mut ActivityClientState,
    events: &mut MessageWriter<ActivityEvent>,
    kind: ActivityRequestKind,
) {
    let error = ActivityError::response_mismatch();
    state.fail(error.clone());
    events.write(ActivityEvent::RequestFailed { kind, error });
}

fn is_activity_request(message_type: super::protocol::MessageType) -> bool {
    matches!(
        message_type,
        super::protocol::MessageType::ActivityListReq
            | super::protocol::MessageType::ActivityDetailReq
            | super::protocol::MessageType::ActivityProgressReq
            | super::protocol::MessageType::ActivityClaimReq
            | super::protocol::MessageType::ActivityActionReq
    )
}

fn parse_progress_json(value: &str) -> Result<Option<ActivityProgressView>, ()> {
    if value.trim().is_empty() || value.trim() == "{}" {
        return Ok(None);
    }
    if value.len() > MAX_ACTIVITY_PROGRESS_JSON_BYTES {
        return Err(());
    }
    let value: Value = serde_json::from_str(value).map_err(|_| ())?;
    let object = value.as_object().ok_or(())?;
    let type_name = object
        .get("type")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or(())?
        .to_owned();
    let schema_version = object
        .get("schema_version")
        .and_then(Value::as_u64)
        .filter(|version| *version > 0 && *version <= u64::from(u32::MAX))
        .ok_or(())? as u32;
    Ok(Some(ActivityProgressView {
        type_name,
        schema_version,
        value,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_view_accepts_unknown_fields() {
        let view = parse_progress_json(
            r#"{"type":"login_reward","schema_version":1,"future_field":{"ignored":true}}"#,
        )
        .unwrap()
        .unwrap();
        assert_eq!(view.type_name, "login_reward");
        assert_eq!(view.schema_version, 1);
    }

    #[test]
    fn progress_route_placeholder_does_not_clear_detail() {
        assert_eq!(parse_progress_json("{}").unwrap(), None);
    }

    #[test]
    fn tracker_stores_only_a_retry_key_fingerprint() {
        let mut tracker = ActivityRequestTracker::default();
        tracker.record(
            7,
            ActivityRequestKind::Claim,
            None,
            None,
            None,
            Some("request-id-should-not-be-exposed"),
            ActivityScope {
                character_id: "character-1".to_owned(),
                connection_id: ConnectionId::from_raw(12),
            },
        );
        assert_eq!(tracker.len(), 1);
        let request = tracker.complete(7, ConnectionId::from_raw(12)).unwrap();
        assert_ne!(
            request.request_id_fingerprint.as_deref(),
            Some("request-id-should-not-be-exposed")
        );
    }

    #[test]
    fn scope_change_rejects_a_response_from_the_previous_character() {
        let old_scope = ActivityScope {
            character_id: "character-old".to_owned(),
            connection_id: ConnectionId::from_raw(12),
        };
        let mut state = ActivityClientState {
            scope: Some(old_scope.clone()),
            ..Default::default()
        };
        state.request_tracker.record(
            9,
            ActivityRequestKind::Detail,
            None,
            None,
            None,
            None,
            old_scope,
        );
        state.scope = Some(ActivityScope {
            character_id: "character-new".to_owned(),
            connection_id: ConnectionId::from_raw(12),
        });
        assert!(
            state
                .complete_request(9, ConnectionId::from_raw(12))
                .is_none()
        );
    }

    #[test]
    fn activity_error_classification_preserves_retry_policy() {
        let retryable = ActivityError::from_code("ACTIVITY_STORAGE_UNAVAILABLE");
        assert_eq!(retryable.kind, ActivityErrorKind::StorageUnavailable);
        assert!(retryable.retryable);

        let terminal = ActivityError::from_code("ACTIVITY_ALREADY_CLAIMED");
        assert_eq!(terminal.kind, ActivityErrorKind::AlreadyClaimed);
        assert!(!terminal.retryable);
    }

    #[test]
    fn write_response_stage_id_must_match_exactly_including_empty_values() {
        for kind in [ActivityRequestKind::Claim, ActivityRequestKind::Action] {
            let tracked = TrackedActivityRequest {
                kind,
                activity: None,
                stage_id: Some(String::new()),
                action_type: None,
                request_id_fingerprint: None,
                scope: ActivityScope {
                    character_id: "character-1".to_owned(),
                    connection_id: ConnectionId::from_raw(12),
                },
            };
            assert!(write_response_stage_matches(&tracked, ""));
            assert!(!write_response_stage_matches(&tracked, "other-stage"));

            let tracked = TrackedActivityRequest {
                stage_id: Some("stage-3".to_owned()),
                ..tracked
            };
            assert!(write_response_stage_matches(&tracked, "stage-3"));
            assert!(!write_response_stage_matches(&tracked, "stage-4"));
        }
    }
}
