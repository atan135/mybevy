mod fixture;
mod mock;
mod runner;

pub use fixture::{FixtureCase, FixtureProvider};
pub use mock::{MockProvider, MockScenario};
pub use runner::{
    ProviderAttemptOutcome, ProviderAttemptTrace, ProviderExecution, ProviderExecutionFailure,
    ProviderExecutionPolicy, ProviderExecutionTrace, ProviderRegistry, ProviderRunner, RetryPolicy,
};

use crate::lifecycle::{CancellationToken, TaskFailure, TaskFailureKind};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    fmt,
    sync::Arc,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderOperation {
    VisualAnalysis,
    StructuredGeneration,
}

#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ProviderId(String);

impl ProviderId {
    pub fn new(value: impl Into<String>) -> Result<Self, TaskFailure> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        {
            return Err(TaskFailure::invalid(
                "provider ID must contain only ASCII letters, digits, '.', '-' or '_'",
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ProviderId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_tuple("ProviderId").field(&self.0).finish()
    }
}

impl<'de> Deserialize<'de> for ProviderId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderCapabilities {
    pub image_input: bool,
    pub structured_output: bool,
    pub max_image_count: usize,
    pub operations: BTreeSet<ProviderOperation>,
}

impl ProviderCapabilities {
    pub fn validate_request(&self, request: &ProviderRequest) -> Result<(), TaskFailure> {
        let operation = request.operation();
        if !self.operations.contains(&operation) {
            return Err(capability_failure(format!(
                "provider does not support operation {operation:?}"
            )));
        }
        if !self.structured_output {
            return Err(capability_failure(
                "provider does not support required structured output",
            ));
        }
        let image_count = request.image_count();
        if image_count > 0 && !self.image_input {
            return Err(capability_failure(
                "provider does not support required image input",
            ));
        }
        if image_count > self.max_image_count {
            return Err(capability_failure(format!(
                "request contains {image_count} images but provider limit is {}",
                self.max_image_count
            )));
        }
        Ok(())
    }
}

fn capability_failure(message: impl Into<String>) -> TaskFailure {
    TaskFailure::new(
        TaskFailureKind::ProviderCapabilityUnsupported,
        message,
        None,
    )
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderDescriptor {
    pub id: ProviderId,
    pub capabilities: ProviderCapabilities,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StructuredOutputContract {
    pub schema_id: String,
    pub schema_version: u32,
}

impl StructuredOutputContract {
    pub fn new(schema_id: impl Into<String>, schema_version: u32) -> Result<Self, TaskFailure> {
        let schema_id = schema_id.into();
        if !is_safe_metadata_label(&schema_id, 128) || schema_version == 0 {
            return Err(TaskFailure::invalid(
                "structured output contract requires a bounded schema ID and nonzero version",
            ));
        }
        Ok(Self {
            schema_id,
            schema_version,
        })
    }
}

#[derive(Clone)]
pub struct ProviderImage {
    id: String,
    media_type: String,
    bytes: Arc<[u8]>,
}

impl ProviderImage {
    pub fn new(
        id: impl Into<String>,
        media_type: impl Into<String>,
        bytes: impl Into<Arc<[u8]>>,
    ) -> Result<Self, TaskFailure> {
        let id = id.into();
        let media_type = media_type.into();
        let bytes = bytes.into();
        if !is_safe_metadata_label(&id, 128)
            || media_type.is_empty()
            || media_type.len() > 128
            || !media_type.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'+' | b'-' | b'.')
            })
            || bytes.is_empty()
        {
            return Err(TaskFailure::invalid(
                "provider image ID, media type, and bytes must be nonempty",
            ));
        }
        Ok(Self {
            id,
            media_type,
            bytes,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn media_type(&self) -> &str {
        &self.media_type
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl fmt::Debug for ProviderImage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderImage")
            .field("id", &self.id)
            .field("media_type", &self.media_type)
            .field("byte_length", &self.bytes.len())
            .field("bytes", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone)]
pub struct VisualAnalysisRequest {
    run_id: String,
    prompt_version: String,
    instruction: Arc<str>,
    structured_inputs: Option<Arc<Value>>,
    images: Vec<ProviderImage>,
    output: StructuredOutputContract,
}

#[derive(Clone)]
pub struct StructuredGenerationRequest {
    run_id: String,
    prompt_version: String,
    instruction: Arc<str>,
    structured_inputs: Arc<Value>,
    images: Vec<ProviderImage>,
    output: StructuredOutputContract,
}

#[derive(Clone)]
pub enum ProviderRequest {
    VisualAnalysis(VisualAnalysisRequest),
    StructuredGeneration(StructuredGenerationRequest),
}

impl ProviderRequest {
    pub fn visual_analysis(
        run_id: impl Into<String>,
        prompt_version: impl Into<String>,
        instruction: impl Into<Arc<str>>,
        images: Vec<ProviderImage>,
        output: StructuredOutputContract,
    ) -> Result<Self, TaskFailure> {
        Self::visual_analysis_with_context(
            run_id,
            prompt_version,
            instruction,
            None,
            images,
            output,
        )
    }

    pub fn visual_analysis_with_context(
        run_id: impl Into<String>,
        prompt_version: impl Into<String>,
        instruction: impl Into<Arc<str>>,
        structured_inputs: Option<Value>,
        images: Vec<ProviderImage>,
        output: StructuredOutputContract,
    ) -> Result<Self, TaskFailure> {
        let run_id = run_id.into();
        let prompt_version = prompt_version.into();
        validate_request_labels(&run_id, &prompt_version)?;
        if images.is_empty() {
            return Err(TaskFailure::invalid(
                "visual analysis requests require at least one image",
            ));
        }
        Ok(Self::VisualAnalysis(VisualAnalysisRequest {
            run_id,
            prompt_version,
            instruction: instruction.into(),
            structured_inputs: structured_inputs.map(Arc::new),
            images,
            output,
        }))
    }

    pub fn structured_generation(
        run_id: impl Into<String>,
        prompt_version: impl Into<String>,
        instruction: impl Into<Arc<str>>,
        structured_inputs: Value,
        images: Vec<ProviderImage>,
        output: StructuredOutputContract,
    ) -> Result<Self, TaskFailure> {
        let run_id = run_id.into();
        let prompt_version = prompt_version.into();
        validate_request_labels(&run_id, &prompt_version)?;
        Ok(Self::StructuredGeneration(StructuredGenerationRequest {
            run_id,
            prompt_version,
            instruction: instruction.into(),
            structured_inputs: Arc::new(structured_inputs),
            images,
            output,
        }))
    }

    pub fn operation(&self) -> ProviderOperation {
        match self {
            Self::VisualAnalysis(_) => ProviderOperation::VisualAnalysis,
            Self::StructuredGeneration(_) => ProviderOperation::StructuredGeneration,
        }
    }

    pub fn images(&self) -> &[ProviderImage] {
        match self {
            Self::VisualAnalysis(request) => &request.images,
            Self::StructuredGeneration(request) => &request.images,
        }
    }

    pub fn image_count(&self) -> usize {
        self.images().len()
    }

    pub fn instruction(&self) -> &str {
        match self {
            Self::VisualAnalysis(request) => &request.instruction,
            Self::StructuredGeneration(request) => &request.instruction,
        }
    }

    pub fn structured_inputs(&self) -> Option<&Value> {
        match self {
            Self::VisualAnalysis(request) => request.structured_inputs.as_deref(),
            Self::StructuredGeneration(request) => Some(&request.structured_inputs),
        }
    }

    pub fn output_contract(&self) -> &StructuredOutputContract {
        match self {
            Self::VisualAnalysis(request) => &request.output,
            Self::StructuredGeneration(request) => &request.output,
        }
    }

    pub fn log_metadata(&self) -> RequestLogMetadata {
        let (run_id, prompt_version) = match self {
            Self::VisualAnalysis(request) => (&request.run_id, &request.prompt_version),
            Self::StructuredGeneration(request) => (&request.run_id, &request.prompt_version),
        };
        RequestLogMetadata {
            run_id: run_id.clone(),
            operation: self.operation(),
            prompt_version: prompt_version.clone(),
            output_schema: self.output_contract().clone(),
            image_count: self.image_count(),
            image_bytes: self.images().iter().map(|image| image.bytes.len()).sum(),
            has_structured_inputs: self.structured_inputs().is_some(),
        }
    }
}

impl fmt::Debug for ProviderRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderRequest")
            .field("metadata", &self.log_metadata())
            .field("instruction", &"[REDACTED]")
            .field("images", &"[REDACTED]")
            .field("structured_inputs", &"[REDACTED]")
            .finish()
    }
}

fn validate_request_labels(run_id: &str, prompt_version: &str) -> Result<(), TaskFailure> {
    if !is_safe_metadata_label(run_id, 128) || !is_safe_metadata_label(prompt_version, 128) {
        return Err(TaskFailure::invalid(
            "provider request run ID and prompt version must be nonempty and bounded",
        ));
    }
    Ok(())
}

pub(crate) fn is_safe_metadata_label(value: &str, maximum_length: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum_length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RequestLogMetadata {
    pub run_id: String,
    pub operation: ProviderOperation,
    pub prompt_version: String,
    pub output_schema: StructuredOutputContract,
    pub image_count: usize,
    pub image_bytes: usize,
    pub has_structured_inputs: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StructuredProviderOutput {
    pub operation: ProviderOperation,
    pub schema: StructuredOutputContract,
    pub value: Value,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderUsage {
    pub input_units: Option<u64>,
    pub output_units: Option<u64>,
}

#[derive(Clone, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ServerRequestId(String);

impl ServerRequestId {
    pub fn new(value: impl Into<String>) -> Result<Self, TaskFailure> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 128
            || !value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':')
            })
        {
            return Err(TaskFailure::new(
                TaskFailureKind::ProviderResponseMalformed,
                "provider returned an invalid server request ID",
                None,
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ServerRequestId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("ServerRequestId")
            .field(&self.0)
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProviderResponse {
    pub output: StructuredProviderOutput,
    pub server_request_id: Option<ServerRequestId>,
    pub usage: ProviderUsage,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderErrorKind {
    Timeout,
    Cancelled,
    RateLimited,
    Authentication,
    ServiceUnavailable,
    MalformedResponse,
}

impl ProviderErrorKind {
    pub fn retryable(self) -> bool {
        matches!(
            self,
            Self::Timeout | Self::RateLimited | Self::ServiceUnavailable
        )
    }

    fn task_failure_kind(self) -> TaskFailureKind {
        match self {
            Self::Timeout => TaskFailureKind::ProviderTimeout,
            Self::Cancelled => TaskFailureKind::Cancelled,
            Self::RateLimited => TaskFailureKind::ProviderRateLimited,
            Self::Authentication => TaskFailureKind::ProviderAuthentication,
            Self::ServiceUnavailable => TaskFailureKind::ProviderServiceUnavailable,
            Self::MalformedResponse => TaskFailureKind::ProviderResponseMalformed,
        }
    }

    fn safe_message(self) -> &'static str {
        match self {
            Self::Timeout => "provider request timed out",
            Self::Cancelled => "provider request was cancelled",
            Self::RateLimited => "provider rate limit was reached",
            Self::Authentication => "provider authentication failed",
            Self::ServiceUnavailable => "provider service is unavailable",
            Self::MalformedResponse => "provider returned a malformed structured response",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderError {
    pub kind: ProviderErrorKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_request_id: Option<ServerRequestId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_ms: Option<u64>,
}

impl ProviderError {
    pub fn new(kind: ProviderErrorKind) -> Self {
        Self {
            kind,
            server_request_id: None,
            retry_after_ms: None,
        }
    }

    pub fn with_request_id(mut self, request_id: ServerRequestId) -> Self {
        self.server_request_id = Some(request_id);
        self
    }

    pub fn with_retry_after(mut self, retry_after: Duration) -> Self {
        self.retry_after_ms = Some(duration_millis(retry_after));
        self
    }

    pub fn to_task_failure(&self) -> TaskFailure {
        let mut failure = TaskFailure::new(
            self.kind.task_failure_kind(),
            self.kind.safe_message(),
            None,
        );
        if let Some(request_id) = &self.server_request_id {
            failure = failure.with_server_request_id(request_id.as_str());
        }
        failure
    }

    pub fn retry_after(&self) -> Option<Duration> {
        self.retry_after_ms.map(Duration::from_millis)
    }
}

#[derive(Clone, Debug)]
pub struct ProviderCallContext {
    attempt: u32,
    deadline: Instant,
    cancellation: CancellationToken,
}

impl ProviderCallContext {
    pub(crate) fn new(attempt: u32, timeout: Duration, cancellation: CancellationToken) -> Self {
        Self {
            attempt,
            deadline: Instant::now() + timeout,
            cancellation,
        }
    }

    pub fn attempt(&self) -> u32 {
        self.attempt
    }

    pub fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }

    pub fn checkpoint(&self) -> Result<(), ProviderError> {
        if self.cancellation.is_requested() {
            Err(ProviderError::new(ProviderErrorKind::Cancelled))
        } else if self.remaining().is_zero() {
            Err(ProviderError::new(ProviderErrorKind::Timeout))
        } else {
            Ok(())
        }
    }
}

pub trait Provider: Send + Sync + 'static {
    fn descriptor(&self) -> ProviderDescriptor;

    /// Implementations must apply `context.remaining()` to online I/O and observe checkpoints.
    /// Vendor request/response types and model names remain private to the implementation.
    fn invoke(
        &self,
        request: ProviderRequest,
        context: ProviderCallContext,
    ) -> Result<ProviderResponse, ProviderError>;
}

pub(crate) fn validate_response(
    request: &ProviderRequest,
    response: &ProviderResponse,
) -> Result<(), ProviderError> {
    if response.output.operation != request.operation()
        || response.output.schema != *request.output_contract()
    {
        let mut error = ProviderError::new(ProviderErrorKind::MalformedResponse);
        if let Some(request_id) = &response.server_request_id {
            error = error.with_request_id(request_id.clone());
        }
        return Err(error);
    }
    Ok(())
}

pub(crate) fn duration_millis(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    pub(crate) fn test_contract() -> StructuredOutputContract {
        StructuredOutputContract::new("ui-reference-analysis", 1).unwrap()
    }

    pub(crate) fn test_image() -> ProviderImage {
        ProviderImage::new(
            "primary",
            "image/png",
            Arc::<[u8]>::from(b"private-image-bytes".as_slice()),
        )
        .unwrap()
    }

    pub(crate) fn test_request() -> ProviderRequest {
        ProviderRequest::visual_analysis(
            "fixture-run",
            "prompt-v1",
            "private prompt containing user@example.test",
            vec![test_image()],
            test_contract(),
        )
        .unwrap()
    }

    #[test]
    fn request_debug_and_log_metadata_exclude_sensitive_payloads() {
        let request = ProviderRequest::structured_generation(
            "fixture-run",
            "prompt-v2",
            "private-instruction-token",
            json!({"account_text": "private-player-name"}),
            vec![test_image()],
            test_contract(),
        )
        .unwrap();
        let debug = format!("{request:?}");
        let metadata = serde_json::to_string(&request.log_metadata()).unwrap();
        for private in [
            "private-instruction-token",
            "private-player-name",
            "private-image-bytes",
        ] {
            assert!(!debug.contains(private));
            assert!(!metadata.contains(private));
        }
        assert!(metadata.contains("prompt-v2"));
        assert!(metadata.contains("image_bytes"));
    }

    #[test]
    fn visual_analysis_can_carry_redacted_structured_context_without_becoming_generation() {
        let request = ProviderRequest::visual_analysis_with_context(
            "visual-context-run",
            "prompt-v1",
            "private visual instruction",
            Some(json!({"region_metrics": {"changed": 4}})),
            vec![test_image()],
            test_contract(),
        )
        .unwrap();
        assert_eq!(request.operation(), ProviderOperation::VisualAnalysis);
        assert_eq!(
            request.structured_inputs().unwrap()["region_metrics"]["changed"],
            4
        );
        assert!(request.log_metadata().has_structured_inputs);
        assert!(!format!("{request:?}").contains("region_metrics"));
    }

    #[test]
    fn persisted_metadata_labels_reject_free_form_or_multiline_text() {
        assert!(StructuredOutputContract::new("schema\nprivate", 1).is_err());
        assert!(
            ProviderRequest::visual_analysis(
                "player@example.test",
                "prompt-v1",
                "fixture",
                vec![test_image()],
                test_contract(),
            )
            .is_err()
        );
        assert!(
            ProviderRequest::visual_analysis(
                "fixture-run",
                "prompt version with spaces",
                "fixture",
                vec![test_image()],
                test_contract(),
            )
            .is_err()
        );
    }

    #[test]
    fn capability_check_rejects_missing_features_and_image_overflow() {
        let request = test_request();
        let no_images = ProviderCapabilities {
            image_input: false,
            structured_output: true,
            max_image_count: 0,
            operations: BTreeSet::from([ProviderOperation::VisualAnalysis]),
        };
        assert_eq!(
            no_images.validate_request(&request).unwrap_err().kind(),
            TaskFailureKind::ProviderCapabilityUnsupported
        );

        let too_few = ProviderCapabilities {
            image_input: true,
            structured_output: true,
            max_image_count: 0,
            operations: BTreeSet::from([ProviderOperation::VisualAnalysis]),
        };
        assert!(
            too_few
                .validate_request(&request)
                .unwrap_err()
                .message()
                .contains("provider limit is 0")
        );
    }

    #[test]
    fn provider_error_mapping_is_stable_and_preserves_safe_request_id() {
        let cases = [
            (ProviderErrorKind::Timeout, TaskFailureKind::ProviderTimeout),
            (ProviderErrorKind::Cancelled, TaskFailureKind::Cancelled),
            (
                ProviderErrorKind::RateLimited,
                TaskFailureKind::ProviderRateLimited,
            ),
            (
                ProviderErrorKind::Authentication,
                TaskFailureKind::ProviderAuthentication,
            ),
            (
                ProviderErrorKind::ServiceUnavailable,
                TaskFailureKind::ProviderServiceUnavailable,
            ),
            (
                ProviderErrorKind::MalformedResponse,
                TaskFailureKind::ProviderResponseMalformed,
            ),
        ];
        for (provider_kind, task_kind) in cases {
            let error = ProviderError::new(provider_kind)
                .with_request_id(ServerRequestId::new("request-fixture-001").unwrap());
            let failure = error.to_task_failure();
            assert_eq!(failure.kind(), task_kind);
            assert_eq!(failure.server_request_id(), Some("request-fixture-001"));
        }
    }
}
