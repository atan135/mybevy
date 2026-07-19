use serde::{Deserialize, Serialize};
use std::{
    fmt,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskFailureKind {
    InvalidInput,
    ManifestCorrupt,
    ProtocolIncompatible,
    InvalidStateTransition,
    ArtifactMissing,
    ArtifactInvalid,
    CacheIncompatible,
    RunnerLaunchFailed,
    RunnerTimeout,
    ValidationFailed,
    PreviewFailed,
    AuditFailed,
    FixPlanRejected,
    FixApplicationFailed,
    VerificationFailed,
    ApprovalRejected,
    SafetyPolicyRejected,
    ImageUnreadable,
    ImageHashMismatch,
    ImageUnsupportedFormat,
    ImageCorrupt,
    ImageTooSmall,
    ImageDimensionsUnsafe,
    ImageAspectRatioUnsupported,
    ImageBlank,
    ImageMetadataMismatch,
    PreprocessCacheCorrupt,
    TargetViewportMissing,
    OutputDirectoryConflict,
    UnsafeOutputPath,
    WorkspaceSnapshotFailed,
    WorkspaceIsolationFailed,
    WorkspaceLockTimeout,
    Cancelled,
    DependencyBoundaryViolation,
    ProviderNotFound,
    ProviderCapabilityUnsupported,
    ProviderTimeout,
    ProviderRateLimited,
    ProviderAuthentication,
    ProviderServiceUnavailable,
    ProviderResponseMalformed,
    CredentialUnavailable,
}

impl TaskFailureKind {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidInput => "UI_GENERATION_INPUT_INVALID",
            Self::ManifestCorrupt => "UI_GENERATION_MANIFEST_CORRUPT",
            Self::ProtocolIncompatible => "UI_GENERATION_PROTOCOL_INCOMPATIBLE",
            Self::InvalidStateTransition => "UI_GENERATION_STATE_TRANSITION_INVALID",
            Self::ArtifactMissing => "UI_GENERATION_ARTIFACT_MISSING",
            Self::ArtifactInvalid => "UI_GENERATION_ARTIFACT_INVALID",
            Self::CacheIncompatible => "UI_GENERATION_CACHE_INCOMPATIBLE",
            Self::RunnerLaunchFailed => "UI_GENERATION_RUNNER_LAUNCH_FAILED",
            Self::RunnerTimeout => "UI_GENERATION_RUNNER_TIMEOUT",
            Self::ValidationFailed => "UI_GENERATION_VALIDATION_FAILED",
            Self::PreviewFailed => "UI_GENERATION_PREVIEW_FAILED",
            Self::AuditFailed => "UI_GENERATION_AUDIT_FAILED",
            Self::FixPlanRejected => "UI_GENERATION_FIX_PLAN_REJECTED",
            Self::FixApplicationFailed => "UI_GENERATION_FIX_APPLICATION_FAILED",
            Self::VerificationFailed => "UI_GENERATION_VERIFICATION_FAILED",
            Self::ApprovalRejected => "UI_GENERATION_APPROVAL_REJECTED",
            Self::SafetyPolicyRejected => "UI_GENERATION_SAFETY_POLICY_REJECTED",
            Self::ImageUnreadable => "UI_GENERATION_IMAGE_UNREADABLE",
            Self::ImageHashMismatch => "UI_GENERATION_IMAGE_HASH_MISMATCH",
            Self::ImageUnsupportedFormat => "UI_GENERATION_IMAGE_FORMAT_UNSUPPORTED",
            Self::ImageCorrupt => "UI_GENERATION_IMAGE_CORRUPT",
            Self::ImageTooSmall => "UI_GENERATION_IMAGE_TOO_SMALL",
            Self::ImageDimensionsUnsafe => "UI_GENERATION_IMAGE_DIMENSIONS_UNSAFE",
            Self::ImageAspectRatioUnsupported => "UI_GENERATION_IMAGE_ASPECT_RATIO_UNSUPPORTED",
            Self::ImageBlank => "UI_GENERATION_IMAGE_BLANK",
            Self::ImageMetadataMismatch => "UI_GENERATION_IMAGE_METADATA_MISMATCH",
            Self::PreprocessCacheCorrupt => "UI_GENERATION_PREPROCESS_CACHE_CORRUPT",
            Self::TargetViewportMissing => "UI_GENERATION_TARGET_VIEWPORT_MISSING",
            Self::OutputDirectoryConflict => "UI_GENERATION_OUTPUT_DIRECTORY_CONFLICT",
            Self::UnsafeOutputPath => "UI_GENERATION_OUTPUT_PATH_UNSAFE",
            Self::WorkspaceSnapshotFailed => "UI_GENERATION_WORKSPACE_SNAPSHOT_FAILED",
            Self::WorkspaceIsolationFailed => "UI_GENERATION_WORKSPACE_ISOLATION_FAILED",
            Self::WorkspaceLockTimeout => "UI_GENERATION_WORKSPACE_LOCK_TIMEOUT",
            Self::Cancelled => "UI_GENERATION_CANCELLED",
            Self::DependencyBoundaryViolation => "UI_GENERATION_DEPENDENCY_BOUNDARY_VIOLATION",
            Self::ProviderNotFound => "UI_GENERATION_PROVIDER_NOT_FOUND",
            Self::ProviderCapabilityUnsupported => "UI_GENERATION_PROVIDER_CAPABILITY_UNSUPPORTED",
            Self::ProviderTimeout => "UI_GENERATION_PROVIDER_TIMEOUT",
            Self::ProviderRateLimited => "UI_GENERATION_PROVIDER_RATE_LIMITED",
            Self::ProviderAuthentication => "UI_GENERATION_PROVIDER_AUTHENTICATION_FAILED",
            Self::ProviderServiceUnavailable => "UI_GENERATION_PROVIDER_SERVICE_UNAVAILABLE",
            Self::ProviderResponseMalformed => "UI_GENERATION_PROVIDER_RESPONSE_MALFORMED",
            Self::CredentialUnavailable => "UI_GENERATION_CREDENTIAL_UNAVAILABLE",
        }
    }

    /// Converts the stable `failure_type` values emitted by the UI audit runner
    /// into the shared tool failure taxonomy. Unknown values remain explicit at
    /// their source instead of being silently relabelled.
    pub fn from_legacy_failure_type(value: &str) -> Option<Self> {
        match value {
            "manifest_missing" | "manifest_invalid" => Some(Self::ManifestCorrupt),
            "output_missing" | "artifact_upload_failed" => Some(Self::ArtifactMissing),
            "timeout" | "client_timeout" => Some(Self::RunnerTimeout),
            "launch_failed" | "process_failed" => Some(Self::RunnerLaunchFailed),
            "ai_blocking_issue"
            | "deterministic_hard_failure"
            | "ai_analysis_failed"
            | "ai_missing_capture"
            | "ai_missing_capture_metadata"
            | "ai_remote_artifact_read_failed"
            | "ai_result_invalid"
            | "audit_failed"
            | "nondeterministic_capture" => Some(Self::AuditFailed),
            "safety_policy_rejected" | "baseline_update_forbidden" => {
                Some(Self::SafetyPolicyRejected)
            }
            "fix_command_missing" | "fix_command_failed" => Some(Self::FixApplicationFailed),
            "fix_check_failed" | "max_iterations_reached" => Some(Self::VerificationFailed),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskFailure {
    kind: TaskFailureKind,
    code: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    subject: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    server_request_id: Option<String>,
}

impl TaskFailure {
    pub fn new(kind: TaskFailureKind, message: impl Into<String>, subject: Option<String>) -> Self {
        Self {
            kind,
            code: kind.code().to_owned(),
            message: message.into(),
            subject,
            server_request_id: None,
        }
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(TaskFailureKind::InvalidInput, message, None)
    }

    pub fn manifest_corrupt(message: impl Into<String>) -> Self {
        Self::new(TaskFailureKind::ManifestCorrupt, message, None)
    }

    pub fn protocol_incompatible(message: impl Into<String>) -> Self {
        Self::new(TaskFailureKind::ProtocolIncompatible, message, None)
    }

    pub fn invalid_state_transition(message: impl Into<String>) -> Self {
        Self::new(TaskFailureKind::InvalidStateTransition, message, None)
    }

    pub fn kind(&self) -> TaskFailureKind {
        self.kind
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn subject(&self) -> Option<&str> {
        self.subject.as_deref()
    }

    pub(crate) fn with_server_request_id(mut self, server_request_id: impl Into<String>) -> Self {
        self.server_request_id = Some(server_request_id.into());
        self
    }

    pub fn server_request_id(&self) -> Option<&str> {
        self.server_request_id.as_deref()
    }
}

impl fmt::Display for TaskFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for TaskFailure {}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    ValidatingInput,
    Ready,
    Running,
    Completed,
    Failed { failure: TaskFailure },
    Cancelled { reason: String },
}

impl TaskStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed { .. } | Self::Cancelled { .. }
        )
    }
}

#[derive(Clone, Debug)]
pub struct TaskLifecycle {
    status: TaskStatus,
}

impl Default for TaskLifecycle {
    fn default() -> Self {
        Self {
            status: TaskStatus::Pending,
        }
    }
}

impl TaskLifecycle {
    pub fn status(&self) -> &TaskStatus {
        &self.status
    }

    pub fn transition(&mut self, next: TaskStatus) -> Result<(), TaskFailure> {
        if self.status.is_terminal() {
            return Err(TaskFailure::invalid(
                "a terminal generation task cannot transition to another state",
            ));
        }
        let allowed = matches!(
            (&self.status, &next),
            (TaskStatus::Pending, TaskStatus::ValidatingInput)
                | (TaskStatus::ValidatingInput, TaskStatus::Ready)
                | (TaskStatus::Ready, TaskStatus::Running)
                | (TaskStatus::Running, TaskStatus::Completed)
                | (_, TaskStatus::Failed { .. })
                | (_, TaskStatus::Cancelled { .. })
        );
        if !allowed {
            return Err(TaskFailure::invalid(format!(
                "invalid generation task transition from {:?} to {:?}",
                self.status, next
            )));
        }
        self.status = next;
        Ok(())
    }

    /// Cancellation is terminal and idempotent. It never overwrites a completed or failed result.
    pub fn cancel(&mut self, reason: impl Into<String>) -> bool {
        if self.status.is_terminal() {
            return false;
        }
        self.status = TaskStatus::Cancelled {
            reason: reason.into(),
        };
        true
    }
}

#[derive(Clone, Debug, Default)]
pub struct CancellationToken {
    requested: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn request(&self) {
        self.requested.store(true, Ordering::Release);
    }

    pub fn is_requested(&self) -> bool {
        self.requested.load(Ordering::Acquire)
    }

    pub fn checkpoint(&self) -> Result<(), TaskFailure> {
        if self.is_requested() {
            Err(TaskFailure::new(
                TaskFailureKind::Cancelled,
                "generation task cancellation was requested",
                None,
            ))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failure_kinds_have_stable_codes() {
        let cases = [
            (TaskFailureKind::InvalidInput, "UI_GENERATION_INPUT_INVALID"),
            (
                TaskFailureKind::ImageUnreadable,
                "UI_GENERATION_IMAGE_UNREADABLE",
            ),
            (
                TaskFailureKind::ImageUnsupportedFormat,
                "UI_GENERATION_IMAGE_FORMAT_UNSUPPORTED",
            ),
            (TaskFailureKind::ImageCorrupt, "UI_GENERATION_IMAGE_CORRUPT"),
            (
                TaskFailureKind::ImageTooSmall,
                "UI_GENERATION_IMAGE_TOO_SMALL",
            ),
            (
                TaskFailureKind::ImageDimensionsUnsafe,
                "UI_GENERATION_IMAGE_DIMENSIONS_UNSAFE",
            ),
            (
                TaskFailureKind::ImageAspectRatioUnsupported,
                "UI_GENERATION_IMAGE_ASPECT_RATIO_UNSUPPORTED",
            ),
            (TaskFailureKind::ImageBlank, "UI_GENERATION_IMAGE_BLANK"),
            (
                TaskFailureKind::ImageMetadataMismatch,
                "UI_GENERATION_IMAGE_METADATA_MISMATCH",
            ),
            (
                TaskFailureKind::PreprocessCacheCorrupt,
                "UI_GENERATION_PREPROCESS_CACHE_CORRUPT",
            ),
            (
                TaskFailureKind::TargetViewportMissing,
                "UI_GENERATION_TARGET_VIEWPORT_MISSING",
            ),
            (
                TaskFailureKind::OutputDirectoryConflict,
                "UI_GENERATION_OUTPUT_DIRECTORY_CONFLICT",
            ),
            (
                TaskFailureKind::WorkspaceSnapshotFailed,
                "UI_GENERATION_WORKSPACE_SNAPSHOT_FAILED",
            ),
            (
                TaskFailureKind::WorkspaceIsolationFailed,
                "UI_GENERATION_WORKSPACE_ISOLATION_FAILED",
            ),
            (
                TaskFailureKind::WorkspaceLockTimeout,
                "UI_GENERATION_WORKSPACE_LOCK_TIMEOUT",
            ),
            (TaskFailureKind::Cancelled, "UI_GENERATION_CANCELLED"),
        ];
        for (kind, expected) in cases {
            assert_eq!(kind.code(), expected);
        }
    }

    #[test]
    fn cancellation_is_terminal_and_does_not_replace_completion() {
        let mut lifecycle = TaskLifecycle::default();
        lifecycle.transition(TaskStatus::ValidatingInput).unwrap();
        assert!(lifecycle.cancel("user requested"));
        assert!(!lifecycle.cancel("again"));
        assert!(matches!(lifecycle.status(), TaskStatus::Cancelled { .. }));

        let mut completed = TaskLifecycle::default();
        completed.transition(TaskStatus::ValidatingInput).unwrap();
        completed.transition(TaskStatus::Ready).unwrap();
        completed.transition(TaskStatus::Running).unwrap();
        completed.transition(TaskStatus::Completed).unwrap();
        assert!(!completed.cancel("too late"));
        assert_eq!(completed.status(), &TaskStatus::Completed);
    }

    #[test]
    fn cancellation_token_is_shared_and_checked_at_boundaries() {
        let token = CancellationToken::default();
        let observer = token.clone();
        token.request();
        let failure = observer.checkpoint().unwrap_err();
        assert_eq!(failure.kind(), TaskFailureKind::Cancelled);
    }

    #[test]
    fn legacy_audit_failure_types_use_the_shared_taxonomy() {
        assert_eq!(
            TaskFailureKind::from_legacy_failure_type("manifest_invalid"),
            Some(TaskFailureKind::ManifestCorrupt)
        );
        assert_eq!(
            TaskFailureKind::from_legacy_failure_type("ai_blocking_issue"),
            Some(TaskFailureKind::AuditFailed)
        );
        assert_eq!(
            TaskFailureKind::from_legacy_failure_type("safety_policy_rejected"),
            Some(TaskFailureKind::SafetyPolicyRejected)
        );
        assert_eq!(
            TaskFailureKind::from_legacy_failure_type("unknown_runner_failure"),
            None
        );
    }
}
