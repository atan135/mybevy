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
    ImageUnreadable,
    ImageHashMismatch,
    TargetViewportMissing,
    OutputDirectoryConflict,
    UnsafeOutputPath,
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
            Self::ImageUnreadable => "UI_GENERATION_IMAGE_UNREADABLE",
            Self::ImageHashMismatch => "UI_GENERATION_IMAGE_HASH_MISMATCH",
            Self::TargetViewportMissing => "UI_GENERATION_TARGET_VIEWPORT_MISSING",
            Self::OutputDirectoryConflict => "UI_GENERATION_OUTPUT_DIRECTORY_CONFLICT",
            Self::UnsafeOutputPath => "UI_GENERATION_OUTPUT_PATH_UNSAFE",
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
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
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
                TaskFailureKind::TargetViewportMissing,
                "UI_GENERATION_TARGET_VIEWPORT_MISSING",
            ),
            (
                TaskFailureKind::OutputDirectoryConflict,
                "UI_GENERATION_OUTPUT_DIRECTORY_CONFLICT",
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
}
