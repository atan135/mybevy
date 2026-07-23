use crate::{
    ArtifactReport, ComparisonError, ComparisonErrorCode, ComparisonExitCode,
    DIFF_METRICS_ALGORITHM_VERSION, DIFF_METRICS_REPORT_SCHEMA_VERSION, DiffAnalysisReport,
    DiffAnalysisStatus, PixelRect, REGION_AUDIT_ALGORITHM_VERSION,
    REGION_AUDIT_REPORT_SCHEMA_VERSION, RegionAuditReport, SEMANTIC_AUDIT_ALGORITHM_VERSION,
    SEMANTIC_AUDIT_REPORT_SCHEMA_VERSION, SEMANTIC_TREE_SCHEMA_VERSION, SemanticAuditReport,
    SemanticFinding, SemanticRect, SemanticTree,
    comparison::{
        create_output_directory, resolve_allowed_input_roots, resolve_allowed_root,
        resolve_input_file,
    },
};
use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use image::{
    ColorType, ExtendedColorType, ImageEncoder, ImageError, ImageFormat, ImageReader, Limits,
    RgbaImage, codecs::png::PngEncoder,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fs,
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use ui_generation::{
    credentials::{CredentialLocator, CredentialResolver, SecretString},
    lifecycle::{CancellationToken, TaskFailureKind},
    provider::{
        MockProvider, MockScenario, Provider, ProviderCallContext, ProviderCapabilities,
        ProviderDescriptor, ProviderError, ProviderErrorKind, ProviderExecutionPolicy, ProviderId,
        ProviderImage, ProviderOperation, ProviderRegistry, ProviderRequest, ProviderResponse,
        ProviderRunner, ProviderUsage, RetryPolicy, StructuredOutputContract,
        StructuredProviderOutput,
    },
    provider_budget::TaskExecutionLimits,
};
use url::Url;

pub const AI_ANALYSIS_BUNDLE_SCHEMA_VERSION: u32 = 1;
pub const AI_ANALYSIS_CONFIG_SCHEMA_VERSION: u32 = 1;
pub const AI_ANALYSIS_PROVIDER_OUTPUT_SCHEMA_VERSION: u32 = 1;
pub const AI_ANALYSIS_REPORT_SCHEMA_VERSION: u32 = 1;
pub const AI_ANALYSIS_ALGORITHM_VERSION: &str = "ui_ai_visual_analysis_v1";
pub const AI_ANALYSIS_OUTPUT_SCHEMA_ID: &str = "ui-ai-visual-analysis";
pub const AI_ANALYSIS_REPORT_FILENAME: &str = "ai-analysis-report.json";

pub const MAX_AI_CAPTURES: usize = 6;
pub const MAX_AI_IMAGES: usize = MAX_AI_CAPTURES * 4;
pub const MAX_AI_IMAGE_BYTES: u64 = 8 * 1024 * 1024;
pub const MAX_AI_TOTAL_IMAGE_BYTES: u64 = 32 * 1024 * 1024;
pub const MAX_AI_ISSUES: usize = 256;
pub const MAX_AI_SENSITIVE_VALUES: usize = 256;
pub const MAX_AI_SENSITIVE_VALUE_BYTES: usize = 4 * 1024;
pub const MAX_AI_SENSITIVE_TOTAL_BYTES: usize = 64 * 1024;

const MAX_BUNDLE_BYTES: u64 = 512 * 1024;
const MAX_CONFIG_BYTES: u64 = 256 * 1024;
const MAX_STRUCTURED_FILE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_PROVIDER_CONTEXT_BYTES: usize = 16 * 1024 * 1024;
const MAX_PROVIDER_RESPONSE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_STRING_BYTES: usize = 4 * 1024;
const MAX_EVIDENCE_PER_ISSUE: usize = 8;
const MAX_ALLOWED_DIFFERENCE_NOTES: usize = 16;
const MAX_LIKELY_FILES: usize = 64;
const MAX_SUGGESTED_FILES: usize = 16;
const MAX_AI_TOTAL_DECODED_PIXELS: u64 = 64 * 1024 * 1024;
const MAX_AI_TOTAL_DECODED_BYTES: u64 = MAX_AI_TOTAL_DECODED_PIXELS * 4;
const MAX_AI_IMAGE_DIMENSION: u32 = 16_384;
const MAX_PRIVACY_RECTS: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AiAnalysisRequest {
    pub repository_root: PathBuf,
    pub allowed_input_roots: Vec<PathBuf>,
    pub allowed_output_root: PathBuf,
    pub bundle: PathBuf,
    pub config: PathBuf,
    pub output_directory: PathBuf,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AiAnalysisBundle {
    pub schema_version: u32,
    pub run_id: String,
    pub captures: Vec<AiCaptureBundle>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AiCaptureBundle {
    pub capture_id: String,
    pub screen: String,
    pub device: String,
    pub state: String,
    pub images: AiCaptureImages,
    pub diff_metrics: PathBuf,
    pub region_metrics: PathBuf,
    pub semantic_report: PathBuf,
    pub ui_metadata: PathBuf,
    pub allowed_differences: AiAllowedDifferences,
    pub likely_files: Vec<String>,
    pub privacy: AiCapturePrivacy,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AiCaptureImages {
    pub reference: PathBuf,
    pub actual: PathBuf,
    pub overlay: PathBuf,
    pub heatmap: PathBuf,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AiAllowedDifferences {
    pub profile: String,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AiCapturePrivacy {
    pub redact_semantic_text: bool,
    pub redaction_rects: Vec<PixelRect>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AiAnalysisConfig {
    pub schema_version: u32,
    pub algorithm_version: String,
    pub provider: AiProviderConfig,
    pub policy: AiProviderPolicy,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum AiProviderConfig {
    Fixture {
        provider_id: String,
        audit_model_id: String,
        generation_model_id: Option<String>,
        response: PathBuf,
    },
    Mock {
        provider_id: String,
        audit_model_id: String,
        generation_model_id: Option<String>,
        scenario: AiMockScenario,
        response: Option<PathBuf>,
    },
    Online {
        enabled: bool,
        provider_id: String,
        audit_model_id: String,
        generation_model_id: Option<String>,
        endpoint: String,
        credential_environment: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AiMockScenario {
    Success,
    Timeout,
    RateLimited,
    AuthenticationFailure,
    ServiceUnavailable,
    MalformedResponse,
    Unsupported,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AiProviderPolicy {
    pub attempt_timeout_ms: u64,
    pub minimum_request_interval_ms: u64,
    pub max_attempts: u32,
    pub initial_backoff_ms: u64,
    pub max_backoff_ms: u64,
    pub max_output_tokens: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AiImageRole {
    Reference,
    Actual,
    Overlay,
    Heatmap,
}

impl AiImageRole {
    fn label(self) -> &'static str {
        match self {
            Self::Reference => "reference",
            Self::Actual => "actual",
            Self::Overlay => "overlay",
            Self::Heatmap => "heatmap",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AiProblemType {
    Layout,
    Typography,
    Color,
    Imagery,
    Spacing,
    ComponentState,
    HardFailureExplanation,
    Other,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AiSeverity {
    Minor,
    Medium,
    Severe,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AiProviderOutput {
    pub schema_version: u32,
    pub issues: Vec<AiProviderIssue>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AiProviderIssue {
    pub capture_id: String,
    pub problem_type: AiProblemType,
    pub severity: AiSeverity,
    pub problem: String,
    pub evidence: Vec<AiEvidence>,
    pub region: AiIssueRegion,
    pub reference_element: Option<String>,
    pub node_id: Option<String>,
    pub likely_cause: String,
    pub suggested_files: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AiEvidence {
    pub image_id: String,
    pub description: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AiIssueRegion {
    pub region_id: Option<String>,
    pub bounds: Option<PixelRect>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AiAnalysisStatus {
    Completed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AiProviderReport {
    pub mode: String,
    pub provider_id: String,
    pub audit_model_id: String,
    pub generation_model_id: Option<String>,
    pub self_review_is_sole_conclusion: bool,
    pub attempts: usize,
    pub input_units: Option<u64>,
    pub output_units: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AiInputReport {
    pub bundle_path: String,
    pub bundle_sha256: String,
    pub capture_count: usize,
    pub image_count: usize,
    pub image_bytes: u64,
    pub region_metric_count: usize,
    pub semantic_node_count: usize,
    pub provider_images: Vec<AiProviderImageReport>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AiProviderImageReport {
    pub image_id: String,
    pub source_sha256: String,
    pub provider_sha256: String,
    pub redaction_rect_count: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AiDeterministicHardFailure {
    pub capture_id: String,
    pub finding: SemanticFinding,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AiPrivacyReport {
    pub credentials_persisted: bool,
    pub image_bytes_persisted: bool,
    pub raw_provider_response_persisted: bool,
    pub prompt_persisted: bool,
    pub sensitive_text_redaction: String,
    pub provider_redacted_image_count: usize,
    pub provider_redaction_rect_count: usize,
    pub metadata_sensitive_string_count: usize,
    pub response_redaction_count: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AiAnalysisReport {
    pub schema_version: u32,
    pub algorithm_version: String,
    pub status: AiAnalysisStatus,
    pub provider: AiProviderReport,
    pub input: AiInputReport,
    pub issues: Vec<AiProviderIssue>,
    pub deterministic_hard_failures: Vec<AiDeterministicHardFailure>,
    pub deterministic_hard_failures_preserved: bool,
    pub visual_similarity_is_sole_conclusion: bool,
    pub privacy: AiPrivacyReport,
    pub artifacts: Vec<ArtifactReport>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AiAnalysisOutcome {
    pub report: AiAnalysisReport,
    pub exit_code: ComparisonExitCode,
}

struct LoadedCapture {
    bundle: AiCaptureBundle,
    images: Vec<LoadedImage>,
    diff_report: DiffAnalysisReport,
    region_report: RegionAuditReport,
    semantic_report: SemanticAuditReport,
    semantic_tree: SemanticTree,
    sanitized_ui_metadata: Value,
    sensitive_strings: BTreeSet<String>,
}

#[derive(Clone, Copy)]
struct ImagePreflight {
    format: ImageFormat,
    media_type: &'static str,
    width: u32,
    height: u32,
}

struct LoadedImage {
    id: String,
    role: AiImageRole,
    bytes: Arc<[u8]>,
    media_type: String,
    width: u32,
    height: u32,
}

struct PreparedProviderImages {
    images: Vec<ProviderImage>,
    reports: Vec<AiProviderImageReport>,
    redacted_image_count: usize,
    redaction_rect_count: usize,
}

struct CaptureEvidenceCatalog {
    image_ids: HashSet<String>,
    width: u32,
    height: u32,
    region_ids: HashSet<String>,
    node_ids: HashSet<String>,
    allowed_files: HashSet<String>,
}

struct BuiltProvider {
    provider: Arc<dyn Provider>,
    id: ProviderId,
    mode: String,
    audit_model_id: String,
    generation_model_id: Option<String>,
}

struct FixedProvider {
    descriptor: ProviderDescriptor,
    output: StructuredProviderOutput,
}

impl Provider for FixedProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        self.descriptor.clone()
    }

    fn invoke(
        &self,
        _request: ProviderRequest,
        context: ProviderCallContext,
    ) -> Result<ProviderResponse, ProviderError> {
        context.checkpoint()?;
        Ok(ProviderResponse {
            output: self.output.clone(),
            server_request_id: None,
            usage: ProviderUsage::default(),
        })
    }
}

struct OpenAiCompatibleProvider {
    descriptor: ProviderDescriptor,
    endpoint: Url,
    model: String,
    credential: SecretString,
    agent: ureq::Agent,
    max_output_tokens: u32,
}

impl Provider for OpenAiCompatibleProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        self.descriptor.clone()
    }

    fn invoke(
        &self,
        request: ProviderRequest,
        context: ProviderCallContext,
    ) -> Result<ProviderResponse, ProviderError> {
        context.checkpoint()?;
        let body = build_openai_request(&self.model, self.max_output_tokens, &request)?;
        let mut call = self
            .agent
            .post(self.endpoint.as_str())
            .timeout(context.remaining())
            .set("Content-Type", "application/json");
        call = self
            .credential
            .expose_to(|secret| call.set("Authorization", &format!("Bearer {secret}")));
        let response = call.send_json(body).map_err(classify_http_error)?;
        if (300..400).contains(&response.status()) {
            return Err(ProviderError::new(ProviderErrorKind::MalformedResponse));
        }
        let mut bytes = Vec::new();
        response
            .into_reader()
            .take(MAX_PROVIDER_RESPONSE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| ProviderError::new(ProviderErrorKind::MalformedResponse))?;
        if bytes.len() as u64 > MAX_PROVIDER_RESPONSE_BYTES {
            return Err(ProviderError::new(ProviderErrorKind::MalformedResponse));
        }
        parse_openai_response(&bytes, request.output_contract())
    }
}

pub fn analyze_with_ai(request: &AiAnalysisRequest) -> Result<AiAnalysisOutcome, ComparisonError> {
    let repository_root = fs::canonicalize(&request.repository_root).map_err(|error| {
        ComparisonError::input(
            ComparisonErrorCode::RepositoryRootInvalid,
            format!("repository root cannot be resolved: {error}"),
        )
        .at_path(&request.repository_root)
    })?;
    if !repository_root.is_dir() {
        return Err(ComparisonError::input(
            ComparisonErrorCode::RepositoryRootInvalid,
            "repository root is not a directory",
        ));
    }
    let input_roots = resolve_allowed_input_roots(&repository_root, &request.allowed_input_roots)?;
    let output_root = resolve_allowed_root(
        &repository_root,
        &request.allowed_output_root,
        ComparisonErrorCode::AllowedOutputRootInvalid,
        "allowed output root",
    )?;
    let bundle_path = resolve_input_file(&repository_root, &input_roots, &request.bundle)?;
    let config_path = resolve_input_file(&repository_root, &input_roots, &request.config)?;
    let bundle_bytes = read_limited(
        &bundle_path,
        MAX_BUNDLE_BYTES,
        ComparisonErrorCode::AiInputTooLarge,
    )?;
    let config_bytes = read_limited(
        &config_path,
        MAX_CONFIG_BYTES,
        ComparisonErrorCode::ConfigTooLarge,
    )?;
    let bundle: AiAnalysisBundle = serde_json::from_slice(&bundle_bytes).map_err(|_| {
        ComparisonError::input(
            ComparisonErrorCode::AiInputInvalid,
            "AI analysis bundle violates its strict schema",
        )
        .at_path(&bundle_path)
    })?;
    let config: AiAnalysisConfig = serde_json::from_slice(&config_bytes).map_err(|_| {
        ComparisonError::input(
            ComparisonErrorCode::AiConfigInvalid,
            "AI analysis config violates its strict schema",
        )
        .at_path(&config_path)
    })?;
    validate_bundle(&bundle)?;
    validate_config(&config)?;
    let online_mode = matches!(config.provider, AiProviderConfig::Online { .. });
    if online_mode
        && bundle
            .captures
            .iter()
            .any(|capture| !capture.privacy.redact_semantic_text)
    {
        return Err(ai_input_invalid(
            "online AI analysis requires semantic text image redaction for every capture",
        ));
    }

    let captures = load_captures(&repository_root, &input_roots, bundle.captures.clone())?;
    let sensitive_strings = merge_sensitive_strings(&captures)?;
    let prepared_images = prepare_provider_images(&captures, online_mode)?;
    let context = build_provider_context(&bundle, &captures, &prepared_images.reports)?;
    let contract = StructuredOutputContract::new(
        AI_ANALYSIS_OUTPUT_SCHEMA_ID,
        AI_ANALYSIS_PROVIDER_OUTPUT_SCHEMA_VERSION,
    )
    .map_err(|_| ai_internal("AI output contract could not be constructed"))?;
    let provider_request = ProviderRequest::visual_analysis_with_context(
        bundle.run_id.clone(),
        AI_ANALYSIS_ALGORITHM_VERSION,
        provider_instruction(),
        Some(context),
        prepared_images.images.clone(),
        contract.clone(),
    )
    .map_err(|_| ai_input_invalid("AI provider request labels are invalid"))?;

    let built = build_provider(
        &repository_root,
        &input_roots,
        &config.provider,
        &contract,
        config.policy.max_output_tokens,
    )?;
    let descriptor = built.provider.descriptor();
    let mut registry = ProviderRegistry::default();
    registry
        .register(built.provider)
        .map_err(|_| ai_config_invalid("AI provider registration failed"))?;
    let runner = ProviderRunner::new(registry, execution_policy(&config.policy)?)
        .map_err(|_| ai_config_invalid("AI provider execution policy is invalid"))?;
    let execution = runner
        .execute(&built.id, provider_request, &CancellationToken::default())
        .map_err(map_provider_failure)?;
    let mut provider_output: AiProviderOutput =
        serde_json::from_value(execution.response.output.value.clone()).map_err(|_| {
            ComparisonError::input(
                ComparisonErrorCode::AiProviderResponseInvalid,
                "AI provider returned a response outside the strict output schema",
            )
        })?;
    let evidence_catalog = build_evidence_catalog(&captures);
    validate_provider_output(&provider_output, &evidence_catalog)?;
    let response_redaction_count =
        redact_provider_output(&mut provider_output, &sensitive_strings)?;

    let deterministic_hard_failures = captures
        .iter()
        .flat_map(|capture| {
            capture
                .semantic_report
                .findings
                .iter()
                .cloned()
                .map(|finding| AiDeterministicHardFailure {
                    capture_id: capture.bundle.capture_id.clone(),
                    finding,
                })
        })
        .collect::<Vec<_>>();
    let image_count = captures.iter().map(|capture| capture.images.len()).sum();
    let image_bytes = captures
        .iter()
        .flat_map(|capture| capture.images.iter())
        .map(|image| image.bytes.len() as u64)
        .sum();
    let semantic_node_count = captures
        .iter()
        .map(|capture| capture.semantic_tree.nodes.len())
        .sum();
    let output_directory =
        create_output_directory(&repository_root, &output_root, &request.output_directory)?;
    let report_path = output_directory.join(AI_ANALYSIS_REPORT_FILENAME);
    let report = AiAnalysisReport {
        schema_version: AI_ANALYSIS_REPORT_SCHEMA_VERSION,
        algorithm_version: AI_ANALYSIS_ALGORITHM_VERSION.to_owned(),
        status: AiAnalysisStatus::Completed,
        provider: AiProviderReport {
            mode: built.mode,
            provider_id: descriptor.id.as_str().to_owned(),
            audit_model_id: built.audit_model_id,
            generation_model_id: built.generation_model_id,
            self_review_is_sole_conclusion: false,
            attempts: execution.trace.attempts.len(),
            input_units: execution.response.usage.input_units,
            output_units: execution.response.usage.output_units,
        },
        input: AiInputReport {
            bundle_path: path_for_report(&repository_root, &bundle_path),
            bundle_sha256: sha256(&bundle_bytes),
            capture_count: captures.len(),
            image_count,
            image_bytes,
            region_metric_count: captures
                .iter()
                .map(|capture| capture.region_report.region_results.len())
                .sum(),
            semantic_node_count,
            provider_images: prepared_images.reports,
        },
        issues: provider_output.issues,
        deterministic_hard_failures,
        deterministic_hard_failures_preserved: true,
        visual_similarity_is_sole_conclusion: false,
        privacy: AiPrivacyReport {
            credentials_persisted: false,
            image_bytes_persisted: false,
            raw_provider_response_persisted: false,
            prompt_persisted: false,
            sensitive_text_redaction:
                "online provider images use in-memory opaque text masks; request payloads are non-serializable; persisted provider prose removes metadata echoes and sensitive patterns"
                    .to_owned(),
            provider_redacted_image_count: prepared_images.redacted_image_count,
            provider_redaction_rect_count: prepared_images.redaction_rect_count,
            metadata_sensitive_string_count: sensitive_strings.len(),
            response_redaction_count,
        },
        artifacts: vec![ArtifactReport {
            artifact_type: "ai_analysis_report".to_owned(),
            path: AI_ANALYSIS_REPORT_FILENAME.to_owned(),
        }],
    };
    let report = persist_report(report, &report_path)?;
    Ok(AiAnalysisOutcome {
        report,
        exit_code: ComparisonExitCode::Success,
    })
}

fn validate_bundle(bundle: &AiAnalysisBundle) -> Result<(), ComparisonError> {
    if bundle.schema_version != AI_ANALYSIS_BUNDLE_SCHEMA_VERSION
        || !safe_label(&bundle.run_id, 128)
        || bundle.captures.is_empty()
        || bundle.captures.len() > MAX_AI_CAPTURES
    {
        return Err(ai_input_invalid(
            "AI analysis bundle version, run ID, or capture count is invalid",
        ));
    }
    let mut capture_ids = HashSet::new();
    for capture in &bundle.captures {
        if !safe_label(&capture.capture_id, 128)
            || capture.capture_id
                != format!("{}.{}.{}", capture.screen, capture.device, capture.state)
            || !safe_label(&capture.screen, 128)
            || !safe_label(&capture.device, 128)
            || !safe_label(&capture.state, 128)
            || !capture_ids.insert(capture.capture_id.clone())
            || !safe_label(&capture.allowed_differences.profile, 128)
            || capture.allowed_differences.notes.len() > MAX_ALLOWED_DIFFERENCE_NOTES
            || capture.likely_files.len() > MAX_LIKELY_FILES
            || capture.privacy.redaction_rects.len() > MAX_PRIVACY_RECTS
        {
            return Err(ai_input_invalid(
                "AI capture identifiers, allowed differences, or likely-file limits are invalid",
            ));
        }
        for note in &capture.allowed_differences.notes {
            if note.trim().is_empty() || note.len() > MAX_STRING_BYTES || note.contains('\0') {
                return Err(ai_input_invalid(
                    "allowed difference notes must be nonempty and bounded",
                ));
            }
        }
        for file in &capture.likely_files {
            if !is_repository_relative_file(file) {
                return Err(ai_input_invalid(
                    "capture likely files must be safe repository-relative project paths",
                ));
            }
        }
        for rect in &capture.privacy.redaction_rects {
            if rect.x < 0 || rect.y < 0 || rect.width == 0 || rect.height == 0 {
                return Err(ai_input_invalid(
                    "privacy redaction rectangles must use nonempty nonnegative bounds",
                ));
            }
        }
    }
    Ok(())
}

fn validate_config(config: &AiAnalysisConfig) -> Result<(), ComparisonError> {
    if config.schema_version != AI_ANALYSIS_CONFIG_SCHEMA_VERSION
        || config.algorithm_version != AI_ANALYSIS_ALGORITHM_VERSION
        || config.policy.attempt_timeout_ms == 0
        || config.policy.attempt_timeout_ms > 60 * 60 * 1000
        || config.policy.max_attempts == 0
        || config.policy.max_attempts > 10
        || config.policy.initial_backoff_ms > config.policy.max_backoff_ms
        || config.policy.max_backoff_ms > 60 * 60 * 1000
        || config.policy.minimum_request_interval_ms > 60 * 60 * 1000
        || config.policy.max_output_tokens == 0
        || config.policy.max_output_tokens > 16_384
    {
        return Err(ai_config_invalid(
            "AI analysis config version, algorithm, or execution policy is invalid",
        ));
    }
    let (provider_id, audit_model_id, generation_model_id) = provider_labels(&config.provider);
    if ProviderId::new(provider_id.to_owned()).is_err()
        || !safe_model_id(audit_model_id)
        || generation_model_id
            .as_deref()
            .is_some_and(|model| !safe_model_id(model))
    {
        return Err(ai_config_invalid(
            "AI provider and model identifiers must be bounded safe labels",
        ));
    }
    if let AiProviderConfig::Online {
        enabled,
        endpoint,
        credential_environment,
        ..
    } = &config.provider
    {
        if !enabled {
            return Err(ComparisonError::input(
                ComparisonErrorCode::AiProviderUnsupported,
                "online AI analysis requires explicit enabled=true",
            ));
        }
        let url = Url::parse(endpoint)
            .map_err(|_| ai_config_invalid("online AI endpoint must be an absolute HTTPS URL"))?;
        if url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || endpoint.len() > 2048
        {
            return Err(ai_config_invalid(
                "online AI endpoint must be an absolute HTTPS URL without credentials",
            ));
        }
        CredentialLocator::new(Some(credential_environment.clone()), None::<String>)
            .map_err(|_| ai_config_invalid("credential environment locator is invalid"))?;
    }
    Ok(())
}

fn provider_labels(config: &AiProviderConfig) -> (&str, &str, &Option<String>) {
    match config {
        AiProviderConfig::Fixture {
            provider_id,
            audit_model_id,
            generation_model_id,
            ..
        }
        | AiProviderConfig::Mock {
            provider_id,
            audit_model_id,
            generation_model_id,
            ..
        }
        | AiProviderConfig::Online {
            provider_id,
            audit_model_id,
            generation_model_id,
            ..
        } => (provider_id, audit_model_id, generation_model_id),
    }
}

fn load_captures(
    repository_root: &Path,
    input_roots: &[PathBuf],
    captures: Vec<AiCaptureBundle>,
) -> Result<Vec<LoadedCapture>, ComparisonError> {
    let mut total_image_bytes = 0_u64;
    let mut total_decoded_pixels = 0_u64;
    let mut total_decoded_bytes = 0_u64;
    let mut total_structured_bytes = 0_u64;
    captures
        .into_iter()
        .map(|bundle| {
            let role_paths = [
                (AiImageRole::Reference, &bundle.images.reference),
                (AiImageRole::Actual, &bundle.images.actual),
                (AiImageRole::Overlay, &bundle.images.overlay),
                (AiImageRole::Heatmap, &bundle.images.heatmap),
            ];
            let mut images = Vec::with_capacity(role_paths.len());
            for (role, requested) in role_paths {
                let path = resolve_input_file(repository_root, input_roots, requested)?;
                let bytes = read_limited(
                    &path,
                    MAX_AI_IMAGE_BYTES,
                    ComparisonErrorCode::AiImageTooLarge,
                )?;
                if bytes.is_empty() {
                    return Err(ComparisonError::input(
                        ComparisonErrorCode::AiInputInvalid,
                        "AI input image must be nonempty",
                    )
                    .at_path(&path));
                }
                total_image_bytes = total_image_bytes
                    .checked_add(bytes.len() as u64)
                    .ok_or_else(|| ai_input_invalid("AI image byte total overflowed"))?;
                if total_image_bytes > MAX_AI_TOTAL_IMAGE_BYTES {
                    return Err(ComparisonError::input(
                        ComparisonErrorCode::AiImageTooLarge,
                        format!(
                            "AI input images exceed the {MAX_AI_TOTAL_IMAGE_BYTES}-byte total limit"
                        ),
                    ));
                }
                let preflight = preflight_image_dimensions(&path, &bytes)?;
                reserve_decoded_budget(
                    preflight.width,
                    preflight.height,
                    &mut total_decoded_pixels,
                    &mut total_decoded_bytes,
                )?;
                validate_image_snapshot(&path, &bytes, preflight)?;
                images.push(LoadedImage {
                    id: format!("{}.{}", bundle.capture_id, role.label()),
                    role,
                    bytes: Arc::from(bytes),
                    media_type: preflight.media_type.to_owned(),
                    width: preflight.width,
                    height: preflight.height,
                });
            }
            if images
                .iter()
                .any(|image| image.width != images[0].width || image.height != images[0].height)
            {
                return Err(ai_input_invalid(
                    "reference, actual, overlay, and heatmap dimensions must match",
                ));
            }

            let diff_path = resolve_input_file(repository_root, input_roots, &bundle.diff_metrics)?;
            let region_path =
                resolve_input_file(repository_root, input_roots, &bundle.region_metrics)?;
            let semantic_path =
                resolve_input_file(repository_root, input_roots, &bundle.semantic_report)?;
            let metadata_path =
                resolve_input_file(repository_root, input_roots, &bundle.ui_metadata)?;
            let diff_bytes = read_limited(
                &diff_path,
                MAX_STRUCTURED_FILE_BYTES,
                ComparisonErrorCode::AiInputTooLarge,
            )?;
            let region_bytes = read_limited(
                &region_path,
                MAX_STRUCTURED_FILE_BYTES,
                ComparisonErrorCode::AiInputTooLarge,
            )?;
            let semantic_bytes = read_limited(
                &semantic_path,
                MAX_STRUCTURED_FILE_BYTES,
                ComparisonErrorCode::AiInputTooLarge,
            )?;
            let metadata_bytes = read_limited(
                &metadata_path,
                MAX_STRUCTURED_FILE_BYTES,
                ComparisonErrorCode::AiInputTooLarge,
            )?;
            total_structured_bytes = total_structured_bytes
                .checked_add(diff_bytes.len() as u64)
                .and_then(|value| value.checked_add(region_bytes.len() as u64))
                .and_then(|value| value.checked_add(semantic_bytes.len() as u64))
                .and_then(|value| value.checked_add(metadata_bytes.len() as u64))
                .ok_or_else(|| ai_input_invalid("AI structured input byte total overflowed"))?;
            if total_structured_bytes > MAX_PROVIDER_CONTEXT_BYTES as u64 {
                return Err(ComparisonError::input(
                    ComparisonErrorCode::AiInputTooLarge,
                    "AI structured inputs exceed the fixed context budget",
                ));
            }
            let diff_report: DiffAnalysisReport = serde_json::from_slice(&diff_bytes)
                .map_err(|_| ai_input_invalid("diff metrics report violates its strict schema"))?;
            let region_report: RegionAuditReport =
                serde_json::from_slice(&region_bytes).map_err(|_| {
                    ai_input_invalid("region metrics report violates its strict schema")
                })?;
            let semantic_report: SemanticAuditReport = serde_json::from_slice(&semantic_bytes)
                .map_err(|_| ai_input_invalid("semantic report violates its strict schema"))?;
            let mut ui_metadata: Value = serde_json::from_slice(&metadata_bytes)
                .map_err(|_| ai_input_invalid("UI metadata is not valid JSON"))?;
            let semantic_tree: SemanticTree = serde_json::from_value(
                ui_metadata
                    .get("semantic_tree")
                    .cloned()
                    .ok_or_else(|| ai_input_invalid("UI metadata is missing semantic_tree"))?,
            )
            .map_err(|_| ai_input_invalid("UI metadata semantic_tree violates schema v3"))?;
            validate_capture_bindings(
                &images,
                &diff_report,
                &region_report,
                &semantic_report,
                &semantic_tree,
                &metadata_bytes,
            )?;
            let mut sensitive_strings = BTreeSet::new();
            let mut sensitive_string_bytes = 0;
            redact_ui_metadata(
                &mut ui_metadata,
                None,
                &mut sensitive_strings,
                &mut sensitive_string_bytes,
            )?;
            Ok(LoadedCapture {
                bundle,
                images,
                diff_report,
                region_report,
                semantic_report,
                semantic_tree,
                sanitized_ui_metadata: ui_metadata,
                sensitive_strings,
            })
        })
        .collect()
}

fn build_provider_context(
    bundle: &AiAnalysisBundle,
    captures: &[LoadedCapture],
    provider_images: &[AiProviderImageReport],
) -> Result<Value, ComparisonError> {
    let captures = captures
        .iter()
        .map(|capture| {
            let provider_by_id = provider_images
                .iter()
                .filter(|image| image.image_id.starts_with(&format!("{}.", capture.bundle.capture_id)))
                .map(|image| (image.image_id.as_str(), image))
                .collect::<HashMap<_, _>>();
            json!({
                "capture_id": capture.bundle.capture_id,
                "screen": capture.bundle.screen,
                "device": capture.bundle.device,
                "state": capture.bundle.state,
                "images": capture.images.iter().map(|image| json!({
                    "image_id": image.id,
                    "role": image.role,
                    "width": image.width,
                    "height": image.height,
                    "source_sha256": sha256(&image.bytes),
                    "provider_sha256": provider_by_id.get(image.id.as_str()).map(|report| report.provider_sha256.clone()),
                    "redaction_rect_count": provider_by_id.get(image.id.as_str()).map(|report| report.redaction_rect_count),
                })).collect::<Vec<_>>(),
                "diff_metrics": capture.diff_report,
                "region_metrics": capture.region_report,
                "semantic_report": capture.semantic_report,
                "ui_metadata": capture.sanitized_ui_metadata,
                "allowed_differences": capture.bundle.allowed_differences,
                "likely_files": capture.bundle.likely_files,
            })
        })
        .collect::<Vec<_>>();
    let context = json!({
        "schema_version": AI_ANALYSIS_BUNDLE_SCHEMA_VERSION,
        "run_id": bundle.run_id,
        "captures": captures,
        "hard_failure_policy": {
            "provider_can_remove_or_downgrade": false,
            "provider_may_explain_or_raise_severity": true
        }
    });
    let length = serde_json::to_vec(&context)
        .map_err(|_| ai_internal("AI provider context could not be serialized"))?
        .len();
    if length > MAX_PROVIDER_CONTEXT_BYTES {
        return Err(ComparisonError::input(
            ComparisonErrorCode::AiInputTooLarge,
            "AI provider context exceeds the fixed serialized budget",
        ));
    }
    Ok(context)
}

fn validate_capture_bindings(
    images: &[LoadedImage],
    diff_report: &DiffAnalysisReport,
    region_report: &RegionAuditReport,
    semantic_report: &SemanticAuditReport,
    semantic_tree: &SemanticTree,
    metadata_bytes: &[u8],
) -> Result<(), ComparisonError> {
    let reference = images
        .iter()
        .find(|image| image.role == AiImageRole::Reference)
        .ok_or_else(|| ai_input_invalid("AI capture is missing its reference image"))?;
    let actual = images
        .iter()
        .find(|image| image.role == AiImageRole::Actual)
        .ok_or_else(|| ai_input_invalid("AI capture is missing its actual image"))?;
    if diff_report.schema_version != DIFF_METRICS_REPORT_SCHEMA_VERSION
        || diff_report.algorithm_version != DIFF_METRICS_ALGORITHM_VERSION
        || diff_report.status != DiffAnalysisStatus::Analyzed
        || diff_report.inputs.reference_sha256 != sha256(&reference.bytes)
        || diff_report.inputs.actual_sha256 != sha256(&actual.bytes)
    {
        return Err(ai_input_invalid(
            "diff metrics report is not a completed supported report",
        ));
    }
    for (role, artifact_type) in [
        (AiImageRole::Overlay, "overlay"),
        (AiImageRole::Heatmap, "heatmap"),
    ] {
        let image = images
            .iter()
            .find(|image| image.role == role)
            .ok_or_else(|| ai_input_invalid("AI capture is missing a diff artifact image"))?;
        let artifact = diff_report
            .artifacts
            .iter()
            .find(|artifact| artifact.artifact_type == artifact_type)
            .ok_or_else(|| {
                ai_input_invalid("diff report is missing a required artifact binding")
            })?;
        let source_sha256 = sha256(&image.bytes);
        if artifact.sha256.as_deref() != Some(source_sha256.as_str())
            || artifact.byte_length != Some(image.bytes.len() as u64)
            || artifact.dimensions
                != Some(crate::PixelSize {
                    width: image.width,
                    height: image.height,
                })
        {
            return Err(ai_input_invalid(
                "diff artifact hash, byte length, or dimensions do not bind the supplied image",
            ));
        }
    }
    if region_report.schema_version != REGION_AUDIT_REPORT_SCHEMA_VERSION
        || region_report.algorithm_version != REGION_AUDIT_ALGORITHM_VERSION
        || region_report.status != "analyzed"
        || region_report.dimensions.width != reference.width
        || region_report.dimensions.height != reference.height
        || region_report.inputs.aligned_reference_sha256 != sha256(&reference.bytes)
        || region_report.inputs.aligned_actual_sha256 != sha256(&actual.bytes)
    {
        return Err(ai_input_invalid(
            "region metrics do not bind the supplied aligned reference and actual images",
        ));
    }
    if semantic_tree.schema_version != SEMANTIC_TREE_SCHEMA_VERSION
        || semantic_report.schema_version != SEMANTIC_AUDIT_REPORT_SCHEMA_VERSION
        || semantic_report.algorithm_version != SEMANTIC_AUDIT_ALGORITHM_VERSION
        || semantic_report.input.node_count != semantic_tree.nodes.len()
        || semantic_report.input.target_root_id != semantic_tree.target_root_id
        || semantic_report.input.metadata_sha256 != sha256(metadata_bytes)
        || semantic_report.separation.semantic_hard_failure == semantic_report.findings.is_empty()
    {
        return Err(ai_input_invalid(
            "semantic report does not bind the supplied runtime semantic tree",
        ));
    }
    Ok(())
}

fn build_provider(
    repository_root: &Path,
    input_roots: &[PathBuf],
    config: &AiProviderConfig,
    contract: &StructuredOutputContract,
    max_output_tokens: u32,
) -> Result<BuiltProvider, ComparisonError> {
    let (provider_id, audit_model_id, generation_model_id) = provider_labels(config);
    let id = ProviderId::new(provider_id.to_owned())
        .map_err(|_| ai_config_invalid("AI provider ID is invalid"))?;
    let normal_capabilities = ProviderCapabilities {
        image_input: true,
        structured_output: true,
        max_image_count: MAX_AI_IMAGES,
        operations: BTreeSet::from([ProviderOperation::VisualAnalysis]),
    };
    let descriptor = ProviderDescriptor {
        id: id.clone(),
        capabilities: normal_capabilities.clone(),
    };
    let (provider, mode): (Arc<dyn Provider>, &str) = match config {
        AiProviderConfig::Fixture { response, .. } => {
            let path = resolve_input_file(repository_root, input_roots, response)?;
            let output = load_provider_output(&path)?;
            (
                Arc::new(FixedProvider {
                    descriptor,
                    output: StructuredProviderOutput {
                        operation: ProviderOperation::VisualAnalysis,
                        schema: contract.clone(),
                        value: serde_json::to_value(output)
                            .map_err(|_| ai_internal("fixture output serialization failed"))?,
                    },
                }),
                "fixture",
            )
        }
        AiProviderConfig::Mock {
            scenario, response, ..
        } => {
            let unsupported = *scenario == AiMockScenario::Unsupported;
            let output = match response {
                Some(path) => {
                    let path = resolve_input_file(repository_root, input_roots, path)?;
                    serde_json::to_value(load_provider_output(&path)?)
                        .map_err(|_| ai_internal("mock output serialization failed"))?
                }
                None => serde_json::to_value(AiProviderOutput {
                    schema_version: AI_ANALYSIS_PROVIDER_OUTPUT_SCHEMA_VERSION,
                    issues: Vec::new(),
                })
                .map_err(|_| ai_internal("mock output serialization failed"))?,
            };
            let output = StructuredProviderOutput {
                operation: ProviderOperation::VisualAnalysis,
                schema: contract.clone(),
                value: output,
            };
            let scenario = match scenario {
                AiMockScenario::Success => MockScenario::Success {
                    output,
                    request_id: None,
                },
                AiMockScenario::Timeout => MockScenario::Timeout,
                AiMockScenario::RateLimited => MockScenario::RateLimited {
                    retry_after: Duration::from_millis(1),
                    request_id: None,
                },
                AiMockScenario::AuthenticationFailure => {
                    MockScenario::AuthenticationFailure { request_id: None }
                }
                AiMockScenario::ServiceUnavailable => {
                    MockScenario::ServiceUnavailable { request_id: None }
                }
                AiMockScenario::MalformedResponse => {
                    MockScenario::MalformedResponse { request_id: None }
                }
                AiMockScenario::Unsupported => MockScenario::Success {
                    output,
                    request_id: None,
                },
            };
            let mock_descriptor = ProviderDescriptor {
                id: id.clone(),
                capabilities: if unsupported {
                    ProviderCapabilities {
                        structured_output: false,
                        ..normal_capabilities
                    }
                } else {
                    normal_capabilities
                },
            };
            (
                Arc::new(MockProvider::new(mock_descriptor, [scenario])),
                "mock",
            )
        }
        AiProviderConfig::Online {
            endpoint,
            credential_environment,
            ..
        } => {
            let locator =
                CredentialLocator::new(Some(credential_environment.clone()), None::<String>)
                    .map_err(|_| ai_config_invalid("credential environment locator is invalid"))?;
            let credential = CredentialResolver::environment_only()
                .resolve(&locator)
                .map_err(|_| {
                    ComparisonError::input(
                        ComparisonErrorCode::AiProviderAuthentication,
                        "online AI provider credential is unavailable",
                    )
                })?;
            let endpoint = Url::parse(endpoint)
                .map_err(|_| ai_config_invalid("online AI endpoint is invalid"))?;
            (
                Arc::new(OpenAiCompatibleProvider {
                    descriptor,
                    endpoint,
                    model: audit_model_id.to_owned(),
                    credential,
                    agent: ureq::AgentBuilder::new().redirects(0).build(),
                    max_output_tokens,
                }),
                "online",
            )
        }
    };
    Ok(BuiltProvider {
        provider,
        id,
        mode: mode.to_owned(),
        audit_model_id: audit_model_id.to_owned(),
        generation_model_id: generation_model_id.clone(),
    })
}

fn execution_policy(policy: &AiProviderPolicy) -> Result<ProviderExecutionPolicy, ComparisonError> {
    let task_limits = TaskExecutionLimits {
        max_provider_calls: policy.max_attempts,
        max_elapsed_ms: policy
            .attempt_timeout_ms
            .saturating_mul(u64::from(policy.max_attempts))
            .saturating_add(
                policy
                    .max_backoff_ms
                    .saturating_mul(u64::from(policy.max_attempts)),
            )
            .clamp(1, 60 * 60 * 1000),
        max_images: MAX_AI_IMAGES.saturating_mul(policy.max_attempts as usize),
        max_input_units: 1_000_000,
        max_output_units: 250_000,
        max_iterations: policy.max_attempts,
        max_estimated_cost_microunits: 10_000_000,
        input_cost_microunits_per_1k: 1_000,
        output_cost_microunits_per_1k: 2_000,
    };
    let execution = ProviderExecutionPolicy {
        attempt_timeout: Duration::from_millis(policy.attempt_timeout_ms),
        minimum_request_interval: Duration::from_millis(policy.minimum_request_interval_ms),
        retry: RetryPolicy {
            max_attempts: policy.max_attempts,
            initial_backoff: Duration::from_millis(policy.initial_backoff_ms),
            max_backoff: Duration::from_millis(policy.max_backoff_ms),
        },
        task_limits,
    };
    execution
        .validate()
        .map_err(|_| ai_config_invalid("AI provider execution policy is invalid"))?;
    Ok(execution)
}

fn validate_provider_output(
    output: &AiProviderOutput,
    captures: &HashMap<String, CaptureEvidenceCatalog>,
) -> Result<(), ComparisonError> {
    if output.schema_version != AI_ANALYSIS_PROVIDER_OUTPUT_SCHEMA_VERSION
        || output.issues.len() > MAX_AI_ISSUES
    {
        return Err(provider_response_invalid(
            "AI provider output version or issue count is invalid",
        ));
    }
    for issue in &output.issues {
        let capture = captures
            .get(issue.capture_id.as_str())
            .ok_or_else(|| evidence_invalid("AI issue references a capture that does not exist"))?;
        validate_string(&issue.problem, "AI issue problem")?;
        validate_string(&issue.likely_cause, "AI issue likely cause")?;
        if issue.problem_type == AiProblemType::HardFailureExplanation
            && issue.severity != AiSeverity::Severe
        {
            return Err(provider_response_invalid(
                "AI hard-failure explanations cannot lower deterministic severity",
            ));
        }
        if issue.evidence.is_empty()
            || issue.evidence.len() > MAX_EVIDENCE_PER_ISSUE
            || issue.suggested_files.len() > MAX_SUGGESTED_FILES
        {
            return Err(provider_response_invalid(
                "AI issue evidence or suggested-file count is invalid",
            ));
        }
        for evidence in &issue.evidence {
            if !capture.image_ids.contains(evidence.image_id.as_str()) {
                return Err(evidence_invalid(
                    "AI evidence references a screenshot artifact that does not exist",
                ));
            }
            validate_string(&evidence.description, "AI evidence description")?;
        }
        if let Some(region_id) = issue.region.region_id.as_deref()
            && (!safe_label(region_id, 128) || !capture.region_ids.contains(region_id))
        {
            return Err(evidence_invalid(
                "AI issue references an audit region that does not exist",
            ));
        }
        if let Some(bounds) = issue.region.bounds
            && (bounds.x < 0
                || bounds.y < 0
                || bounds.width == 0
                || bounds.height == 0
                || u64::try_from(bounds.x)
                    .unwrap_or(u64::MAX)
                    .saturating_add(u64::from(bounds.width))
                    > u64::from(capture.width)
                || u64::try_from(bounds.y)
                    .unwrap_or(u64::MAX)
                    .saturating_add(u64::from(bounds.height))
                    > u64::from(capture.height))
        {
            return Err(evidence_invalid(
                "AI issue bounds fall outside the referenced capture",
            ));
        }
        if issue.region.bounds.is_none() && issue.region.region_id.is_none() {
            return Err(provider_response_invalid(
                "AI issue must identify a declared region or bounded screenshot rectangle",
            ));
        }
        if let Some(reference_element) = issue.reference_element.as_deref() {
            validate_string(reference_element, "AI reference element")?;
        }
        if let Some(node_id) = issue.node_id.as_deref()
            && !capture.node_ids.contains(node_id)
        {
            return Err(evidence_invalid(
                "AI issue references a semantic node that does not exist",
            ));
        }
        for file in &issue.suggested_files {
            validate_repository_relative_file(file)?;
            if !capture.allowed_files.contains(file) {
                return Err(evidence_invalid(
                    "AI issue suggested a file outside capture source evidence",
                ));
            }
        }
    }
    Ok(())
}

fn build_evidence_catalog(captures: &[LoadedCapture]) -> HashMap<String, CaptureEvidenceCatalog> {
    captures
        .iter()
        .map(|capture| {
            let node_ids = capture
                .semantic_tree
                .nodes
                .iter()
                .flat_map(|node| {
                    std::iter::once(node.stable_id.clone()).chain(node.node_id.clone())
                })
                .collect();
            let allowed_files = capture
                .bundle
                .likely_files
                .iter()
                .chain(
                    capture
                        .semantic_tree
                        .nodes
                        .iter()
                        .flat_map(|node| node.likely_files.iter()),
                )
                .chain(
                    capture
                        .semantic_tree
                        .panels
                        .iter()
                        .flat_map(|panel| panel.likely_files.iter()),
                )
                .cloned()
                .collect();
            (
                capture.bundle.capture_id.clone(),
                CaptureEvidenceCatalog {
                    image_ids: capture
                        .images
                        .iter()
                        .map(|image| image.id.clone())
                        .collect(),
                    width: capture.images[0].width,
                    height: capture.images[0].height,
                    region_ids: capture
                        .region_report
                        .region_results
                        .iter()
                        .map(|region| region.region_id.clone())
                        .collect(),
                    node_ids,
                    allowed_files,
                },
            )
        })
        .collect()
}

fn load_provider_output(path: &Path) -> Result<AiProviderOutput, ComparisonError> {
    let bytes = read_limited(
        path,
        MAX_PROVIDER_RESPONSE_BYTES,
        ComparisonErrorCode::AiProviderResponseInvalid,
    )?;
    serde_json::from_slice(&bytes).map_err(|_| {
        provider_response_invalid("AI fixture/mock output violates the strict output schema")
            .at_path(path)
    })
}

fn provider_instruction() -> &'static str {
    "Inspect every supplied reference/actual/overlay/heatmap image together with the bounded region metrics, semantic findings, redacted UI metadata, allowed differences, and likely source files. Return only the strict ui-ai-visual-analysis JSON object. Cite only supplied image IDs, declared region IDs, semantic node IDs, and likely files. Deterministic hard failures cannot be removed or downgraded; AI may only explain them or raise severity. Do not quote account, credential, token, or personal UI text."
}

fn build_openai_request(
    model: &str,
    max_output_tokens: u32,
    request: &ProviderRequest,
) -> Result<Value, ProviderError> {
    let structured_inputs = request
        .structured_inputs()
        .ok_or_else(|| ProviderError::new(ProviderErrorKind::MalformedResponse))?;
    let mut content = vec![json!({
        "type": "text",
        "text": format!("{}\nINPUT_JSON:\n{}", request.instruction(), structured_inputs)
    })];
    for image in request.images() {
        content.push(json!({
            "type": "image_url",
            "image_url": {
                "url": format!(
                    "data:{};base64,{}",
                    image.media_type(),
                    BASE64.encode(image.bytes())
                )
            }
        }));
    }
    Ok(json!({
        "model": model,
        "max_completion_tokens": max_output_tokens,
        "messages": [{"role": "user", "content": content}],
        "response_format": {
            "type": "json_schema",
            "json_schema": {
                "name": AI_ANALYSIS_OUTPUT_SCHEMA_ID,
                "strict": true,
                "schema": provider_output_json_schema()
            }
        }
    }))
}

fn provider_output_json_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["schema_version", "issues"],
        "properties": {
            "schema_version": {"type": "integer", "const": 1},
            "issues": {
                "type": "array",
                "maxItems": MAX_AI_ISSUES,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["capture_id", "problem_type", "severity", "problem", "evidence", "region", "reference_element", "node_id", "likely_cause", "suggested_files"],
                    "properties": {
                        "capture_id": {"type": "string", "maxLength": 128},
                        "problem_type": {"type": "string", "enum": ["layout", "typography", "color", "imagery", "spacing", "component_state", "hard_failure_explanation", "other"]},
                        "severity": {"type": "string", "enum": ["minor", "medium", "severe"]},
                        "problem": {"type": "string", "maxLength": MAX_STRING_BYTES},
                        "evidence": {
                            "type": "array", "minItems": 1, "maxItems": MAX_EVIDENCE_PER_ISSUE,
                            "items": {"type": "object", "additionalProperties": false, "required": ["image_id", "description"], "properties": {
                                "image_id": {"type": "string", "maxLength": 128},
                                "description": {"type": "string", "maxLength": MAX_STRING_BYTES}
                            }}
                        },
                        "region": {"type": "object", "additionalProperties": false, "required": ["region_id", "bounds"], "properties": {
                            "region_id": {"type": ["string", "null"], "maxLength": 128},
                            "bounds": {"anyOf": [
                                {"type": "null"},
                                {"type": "object", "additionalProperties": false, "required": ["x", "y", "width", "height"], "properties": {
                                    "x": {"type": "integer", "minimum": 0}, "y": {"type": "integer", "minimum": 0},
                                    "width": {"type": "integer", "minimum": 1}, "height": {"type": "integer", "minimum": 1}
                                }}
                            ]}
                        }},
                        "reference_element": {"type": ["string", "null"], "maxLength": MAX_STRING_BYTES},
                        "node_id": {"type": ["string", "null"], "maxLength": 128},
                        "likely_cause": {"type": "string", "maxLength": MAX_STRING_BYTES},
                        "suggested_files": {"type": "array", "maxItems": MAX_SUGGESTED_FILES, "items": {"type": "string", "maxLength": 512}}
                    }
                }
            }
        }
    })
}

fn parse_openai_response(
    bytes: &[u8],
    contract: &StructuredOutputContract,
) -> Result<ProviderResponse, ProviderError> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| ProviderError::new(ProviderErrorKind::MalformedResponse))?;
    let content = value
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .ok_or_else(|| ProviderError::new(ProviderErrorKind::MalformedResponse))?;
    let output: Value = serde_json::from_str(content)
        .map_err(|_| ProviderError::new(ProviderErrorKind::MalformedResponse))?;
    let usage = value.get("usage");
    Ok(ProviderResponse {
        output: StructuredProviderOutput {
            operation: ProviderOperation::VisualAnalysis,
            schema: contract.clone(),
            value: output,
        },
        server_request_id: None,
        usage: ProviderUsage {
            input_units: usage
                .and_then(|usage| usage.get("prompt_tokens"))
                .and_then(Value::as_u64),
            output_units: usage
                .and_then(|usage| usage.get("completion_tokens"))
                .and_then(Value::as_u64),
        },
    })
}

fn classify_http_error(error: ureq::Error) -> ProviderError {
    match error {
        ureq::Error::Status(401 | 403, _) => ProviderError::new(ProviderErrorKind::Authentication),
        ureq::Error::Status(429, response) => {
            let mut error = ProviderError::new(ProviderErrorKind::RateLimited);
            if let Some(seconds) = response
                .header("Retry-After")
                .and_then(|value| value.parse::<u64>().ok())
            {
                error = error.with_retry_after(Duration::from_secs(seconds.min(3600)));
            }
            error
        }
        ureq::Error::Status(500..=599, _) => {
            ProviderError::new(ProviderErrorKind::ServiceUnavailable)
        }
        ureq::Error::Status(_, _) => ProviderError::new(ProviderErrorKind::MalformedResponse),
        ureq::Error::Transport(_) => ProviderError::new(ProviderErrorKind::ServiceUnavailable),
    }
}

fn map_provider_failure(
    failure: ui_generation::provider::ProviderExecutionFailure,
) -> ComparisonError {
    let code = provider_failure_code(failure.failure.kind());
    ComparisonError::input(
        code,
        "AI provider execution failed with a redacted classified error",
    )
}

fn provider_failure_code(kind: TaskFailureKind) -> ComparisonErrorCode {
    match kind {
        TaskFailureKind::ProviderTimeout => ComparisonErrorCode::AiProviderTimeout,
        TaskFailureKind::ProviderRateLimited => ComparisonErrorCode::AiProviderRateLimited,
        TaskFailureKind::ProviderAuthentication | TaskFailureKind::CredentialUnavailable => {
            ComparisonErrorCode::AiProviderAuthentication
        }
        TaskFailureKind::ProviderCapabilityUnsupported | TaskFailureKind::ProviderNotFound => {
            ComparisonErrorCode::AiProviderUnsupported
        }
        TaskFailureKind::ProviderResponseMalformed => {
            ComparisonErrorCode::AiProviderResponseInvalid
        }
        TaskFailureKind::ProviderServiceUnavailable => {
            ComparisonErrorCode::AiProviderServiceUnavailable
        }
        _ => ComparisonErrorCode::AiProviderResponseInvalid,
    }
}

fn persist_report(
    report: AiAnalysisReport,
    path: &Path,
) -> Result<AiAnalysisReport, ComparisonError> {
    let bytes = serde_json::to_vec_pretty(&report)
        .map_err(|_| ai_internal("AI analysis report serialization failed"))?;
    let temporary = path.with_extension("json.tmp");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|_| {
            ComparisonError::internal(
                ComparisonErrorCode::ArtifactWriteFailed,
                "AI report temporary artifact could not be created",
            )
        })?;
    if file.write_all(&bytes).is_err() || file.sync_all().is_err() {
        let _ = fs::remove_file(&temporary);
        return Err(ComparisonError::internal(
            ComparisonErrorCode::ArtifactWriteFailed,
            "AI report temporary artifact could not be written",
        ));
    }
    drop(file);
    if fs::hard_link(&temporary, path).is_err() {
        let _ = fs::remove_file(&temporary);
        return Err(ComparisonError::internal(
            ComparisonErrorCode::ArtifactWriteFailed,
            "AI report final artifact could not be created without clobbering",
        ));
    }
    let _ = fs::remove_file(&temporary);
    Ok(report)
}

fn read_limited(
    path: &Path,
    maximum: u64,
    code: ComparisonErrorCode,
) -> Result<Vec<u8>, ComparisonError> {
    let file = fs::File::open(path)
        .map_err(|_| ComparisonError::input(code, "AI input could not be opened").at_path(path))?;
    let mut bytes = Vec::new();
    file.take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ComparisonError::input(code, "AI input could not be read").at_path(path))?;
    if bytes.len() as u64 > maximum {
        return Err(ComparisonError::input(
            code,
            format!("AI input exceeds the fixed {maximum}-byte limit"),
        )
        .at_path(path));
    }
    Ok(bytes)
}

fn declared_image_format(path: &Path) -> Result<(ImageFormat, &'static str), ComparisonError> {
    let (expected_format, media_type) = match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => (ImageFormat::Png, "image/png"),
        Some("jpg" | "jpeg") => (ImageFormat::Jpeg, "image/jpeg"),
        _ => {
            return Err(ai_input_invalid(
                "AI input images must use PNG or JPEG file extensions",
            ));
        }
    };
    Ok((expected_format, media_type))
}

fn preflight_image_dimensions(
    path: &Path,
    bytes: &[u8],
) -> Result<ImagePreflight, ComparisonError> {
    let (expected_format, media_type) = declared_image_format(path)?;
    let detected = image::guess_format(bytes)
        .map_err(|_| ai_input_invalid("AI input image format could not be detected"))?;
    if detected != expected_format {
        return Err(ai_input_invalid(
            "AI input image bytes do not match the declared file extension",
        ));
    }
    let (width, height) = ImageReader::with_format(Cursor::new(bytes), expected_format)
        .into_dimensions()
        .map_err(|_| ai_input_invalid("AI input image header is truncated or corrupt"))?;
    if width == 0
        || height == 0
        || width > MAX_AI_IMAGE_DIMENSION
        || height > MAX_AI_IMAGE_DIMENSION
    {
        return Err(ComparisonError::input(
            ComparisonErrorCode::AiImageTooLarge,
            "AI input image dimensions exceed the fixed safety limit",
        )
        .at_path(path));
    }
    Ok(ImagePreflight {
        format: expected_format,
        media_type,
        width,
        height,
    })
}

fn reserve_decoded_budget(
    width: u32,
    height: u32,
    total_decoded_pixels: &mut u64,
    total_decoded_bytes: &mut u64,
) -> Result<(), ComparisonError> {
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| {
            ComparisonError::input(
                ComparisonErrorCode::AiImageTooLarge,
                "AI decoded pixel count overflowed the fixed budget",
            )
        })?;
    let decoded_bytes = pixels.checked_mul(4).ok_or_else(|| {
        ComparisonError::input(
            ComparisonErrorCode::AiImageTooLarge,
            "AI decoded byte count overflowed the fixed budget",
        )
    })?;
    let next_pixels = total_decoded_pixels.checked_add(pixels).ok_or_else(|| {
        ComparisonError::input(
            ComparisonErrorCode::AiImageTooLarge,
            "AI total decoded pixel count overflowed the fixed budget",
        )
    })?;
    let next_bytes = total_decoded_bytes
        .checked_add(decoded_bytes)
        .ok_or_else(|| {
            ComparisonError::input(
                ComparisonErrorCode::AiImageTooLarge,
                "AI total decoded byte count overflowed the fixed budget",
            )
        })?;
    if pixels > MAX_AI_TOTAL_DECODED_PIXELS
        || decoded_bytes > MAX_AI_TOTAL_DECODED_BYTES
        || next_pixels > MAX_AI_TOTAL_DECODED_PIXELS
        || next_bytes > MAX_AI_TOTAL_DECODED_BYTES
    {
        return Err(ComparisonError::input(
            ComparisonErrorCode::AiImageTooLarge,
            "AI input images exceed the fixed decoded pixel/memory budget",
        ));
    }
    *total_decoded_pixels = next_pixels;
    *total_decoded_bytes = next_bytes;
    Ok(())
}

fn validate_image_snapshot(
    path: &Path,
    bytes: &[u8],
    preflight: ImagePreflight,
) -> Result<(), ComparisonError> {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_AI_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_AI_IMAGE_DIMENSION);
    limits.max_alloc = Some(MAX_AI_TOTAL_DECODED_BYTES);
    let mut reader = ImageReader::with_format(Cursor::new(bytes), preflight.format);
    reader.limits(limits);
    let decoded = reader.decode().map_err(|error| match error {
        ImageError::Limits(_) => ComparisonError::input(
            ComparisonErrorCode::AiImageTooLarge,
            "AI input image exceeded the fixed decoder allocation limits",
        )
        .at_path(path),
        _ => ai_input_invalid("AI input image is truncated or corrupt").at_path(path),
    })?;
    if !matches!(
        decoded.color(),
        ColorType::L8 | ColorType::La8 | ColorType::Rgb8 | ColorType::Rgba8
    ) {
        return Err(ai_input_invalid(
            "AI input image color type must decode to bounded 8-bit pixels",
        ));
    }
    if decoded.width() != preflight.width || decoded.height() != preflight.height {
        return Err(ai_input_invalid(
            "AI input image dimensions changed between preflight and full decode",
        ));
    }
    Ok(())
}

#[cfg(test)]
fn decode_input_image(path: &Path, bytes: &[u8]) -> Result<(String, u32, u32), ComparisonError> {
    let preflight = preflight_image_dimensions(path, bytes)?;
    let mut total_decoded_pixels = 0;
    let mut total_decoded_bytes = 0;
    reserve_decoded_budget(
        preflight.width,
        preflight.height,
        &mut total_decoded_pixels,
        &mut total_decoded_bytes,
    )?;
    validate_image_snapshot(path, bytes, preflight)?;
    Ok((
        preflight.media_type.to_owned(),
        preflight.width,
        preflight.height,
    ))
}

fn validate_string(value: &str, label: &str) -> Result<(), ComparisonError> {
    if value.trim().is_empty() || value.len() > MAX_STRING_BYTES || value.contains('\0') {
        Err(provider_response_invalid(format!(
            "{label} must be nonempty and at most {MAX_STRING_BYTES} bytes"
        )))
    } else {
        Ok(())
    }
}

fn validate_repository_relative_file(value: &str) -> Result<(), ComparisonError> {
    if is_repository_relative_file(value) {
        Ok(())
    } else {
        Err(evidence_invalid(
            "AI suggested/likely files must be safe repository-relative project paths",
        ))
    }
}

fn is_repository_relative_file(value: &str) -> bool {
    let normalized = value.replace('\\', "/");
    !(value.is_empty()
        || value.len() > 512
        || Path::new(value).is_absolute()
        || normalized.split('/').any(|part| part == "..")
        || !(normalized.starts_with("project/src/") || normalized.starts_with("project/assets/")))
}

fn safe_label(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn safe_model_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'/' | b':')
        })
}

fn prepare_provider_images(
    captures: &[LoadedCapture],
    online_mode: bool,
) -> Result<PreparedProviderImages, ComparisonError> {
    let mut provider_images = Vec::new();
    let mut reports = Vec::new();
    let mut redacted_image_count = 0;
    let mut redaction_rect_count = 0;
    let mut provider_total_bytes = 0_u64;
    for capture in captures {
        let rects = if online_mode {
            provider_redaction_rects(capture)?
        } else {
            Vec::new()
        };
        for image in &capture.images {
            let (bytes, media_type) = if online_mode {
                let decoded = image::load_from_memory_with_format(
                    &image.bytes,
                    if image.media_type == "image/png" {
                        ImageFormat::Png
                    } else {
                        ImageFormat::Jpeg
                    },
                )
                .map_err(|_| ai_input_invalid("validated provider image could not be decoded"))?;
                let mut rgba = decoded.into_rgba8();
                for rect in &rects {
                    apply_opaque_redaction(&mut rgba, *rect);
                }
                let mut encoded = Vec::new();
                PngEncoder::new(&mut encoded)
                    .write_image(
                        rgba.as_raw(),
                        rgba.width(),
                        rgba.height(),
                        ExtendedColorType::Rgba8,
                    )
                    .map_err(|_| ai_internal("provider-redacted image could not be encoded"))?;
                (Arc::<[u8]>::from(encoded), "image/png".to_owned())
            } else {
                (Arc::clone(&image.bytes), image.media_type.clone())
            };
            let source_sha256 = sha256(&image.bytes);
            if bytes.len() as u64 > MAX_AI_IMAGE_BYTES {
                return Err(ComparisonError::input(
                    ComparisonErrorCode::AiImageTooLarge,
                    "provider-bound redacted image exceeds the fixed encoded byte limit",
                ));
            }
            provider_total_bytes = provider_total_bytes
                .checked_add(bytes.len() as u64)
                .ok_or_else(|| ai_input_invalid("provider image byte total overflowed"))?;
            if provider_total_bytes > MAX_AI_TOTAL_IMAGE_BYTES {
                return Err(ComparisonError::input(
                    ComparisonErrorCode::AiImageTooLarge,
                    "provider-bound images exceed the fixed total encoded byte limit",
                ));
            }
            let provider_sha256 = sha256(&bytes);
            reports.push(AiProviderImageReport {
                image_id: image.id.clone(),
                source_sha256,
                provider_sha256,
                redaction_rect_count: rects.len(),
            });
            provider_images.push(
                ProviderImage::new(image.id.clone(), media_type, bytes)
                    .map_err(|_| ai_input_invalid("AI provider image metadata is invalid"))?,
            );
            if online_mode && !rects.is_empty() {
                redacted_image_count += 1;
                redaction_rect_count += rects.len();
            }
        }
    }
    Ok(PreparedProviderImages {
        images: provider_images,
        reports,
        redacted_image_count,
        redaction_rect_count,
    })
}

fn provider_redaction_rects(capture: &LoadedCapture) -> Result<Vec<PixelRect>, ComparisonError> {
    let width = capture.images[0].width;
    let height = capture.images[0].height;
    validated_provider_redaction_rects(
        &capture.semantic_tree,
        &capture.bundle.privacy.redaction_rects,
        capture.bundle.privacy.redact_semantic_text,
        width,
        height,
    )
}

fn validated_provider_redaction_rects(
    semantic_tree: &SemanticTree,
    explicit_rects: &[PixelRect],
    redact_semantic_text: bool,
    width: u32,
    height: u32,
) -> Result<Vec<PixelRect>, ComparisonError> {
    let mut rects = explicit_rects.to_vec();
    if redact_semantic_text {
        rects.extend(semantic_text_redaction_rects(semantic_tree, width, height)?);
    }
    for rect in &rects {
        if u64::try_from(rect.x)
            .unwrap_or(u64::MAX)
            .saturating_add(u64::from(rect.width))
            > u64::from(width)
            || u64::try_from(rect.y)
                .unwrap_or(u64::MAX)
                .saturating_add(u64::from(rect.height))
                > u64::from(height)
        {
            return Err(ai_input_invalid(
                "privacy redaction rectangle falls outside the capture",
            ));
        }
    }
    rects.sort_by_key(|rect| (rect.y, rect.x, rect.height, rect.width));
    rects.dedup();
    if rects.len() > MAX_PRIVACY_RECTS {
        return Err(ai_input_invalid(
            "combined semantic and explicit privacy rectangles exceed the fixed limit",
        ));
    }
    Ok(rects)
}

fn semantic_text_redaction_rects(
    semantic_tree: &SemanticTree,
    width: u32,
    height: u32,
) -> Result<Vec<PixelRect>, ComparisonError> {
    let mut rects = Vec::new();
    for node in &semantic_tree.nodes {
        if !node.text_nonempty || !node.visible || node.fully_clipped {
            continue;
        }
        let measured = node.measured_text_bounds.ok_or_else(|| {
            ai_input_invalid(
                "visible semantic text requires measured bounds before online provider upload",
            )
        })?;
        let rect = map_logical_redaction(
            semantic_tree.viewport,
            measured,
            node.clip_bounds,
            width,
            height,
        )?
        .ok_or_else(|| {
            ai_input_invalid(
                "visible semantic text must map to a nonempty provider redaction rectangle",
            )
        })?;
        rects.push(rect);
    }
    Ok(rects)
}

fn map_logical_redaction(
    viewport: SemanticRect,
    measured: SemanticRect,
    clip: SemanticRect,
    width: u32,
    height: u32,
) -> Result<Option<PixelRect>, ComparisonError> {
    let viewport_width = viewport.max_x - viewport.min_x;
    let viewport_height = viewport.max_y - viewport.min_y;
    if viewport_width <= 0.0 || viewport_height <= 0.0 {
        return Err(ai_input_invalid(
            "semantic viewport cannot map privacy rectangles",
        ));
    }
    let min_x = measured.min_x.max(clip.min_x).max(viewport.min_x);
    let min_y = measured.min_y.max(clip.min_y).max(viewport.min_y);
    let max_x = measured.max_x.min(clip.max_x).min(viewport.max_x);
    let max_y = measured.max_y.min(clip.max_y).min(viewport.max_y);
    if max_x <= min_x || max_y <= min_y {
        return Ok(None);
    }
    let x = ((((min_x - viewport.min_x) / viewport_width) * f64::from(width)).floor())
        .clamp(0.0, f64::from(width)) as i64;
    let y = ((((min_y - viewport.min_y) / viewport_height) * f64::from(height)).floor())
        .clamp(0.0, f64::from(height)) as i64;
    let max_pixel_x = ((((max_x - viewport.min_x) / viewport_width) * f64::from(width)).ceil())
        .clamp(0.0, f64::from(width)) as i64;
    let max_pixel_y = ((((max_y - viewport.min_y) / viewport_height) * f64::from(height)).ceil())
        .clamp(0.0, f64::from(height)) as i64;
    Ok((max_pixel_x > x && max_pixel_y > y).then(|| PixelRect {
        x,
        y,
        width: u32::try_from(max_pixel_x - x).unwrap_or(u32::MAX),
        height: u32::try_from(max_pixel_y - y).unwrap_or(u32::MAX),
    }))
}

fn apply_opaque_redaction(image: &mut RgbaImage, rect: PixelRect) {
    let start_x = u32::try_from(rect.x).unwrap_or(0);
    let start_y = u32::try_from(rect.y).unwrap_or(0);
    for y in start_y..start_y.saturating_add(rect.height).min(image.height()) {
        for x in start_x..start_x.saturating_add(rect.width).min(image.width()) {
            image.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
        }
    }
}

fn redact_ui_metadata(
    value: &mut Value,
    parent_key: Option<&str>,
    sensitive_strings: &mut BTreeSet<String>,
    sensitive_string_bytes: &mut usize,
) -> Result<(), ComparisonError> {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                redact_ui_metadata(value, Some(key), sensitive_strings, sensitive_string_bytes)?;
            }
        }
        Value::Array(values) => {
            for value in values {
                redact_ui_metadata(value, parent_key, sensitive_strings, sensitive_string_bytes)?;
            }
        }
        Value::String(text) => {
            if parent_key.is_some_and(|key| !is_structural_ui_key(key) && is_sensitive_ui_key(key))
            {
                if !text.is_empty() {
                    insert_sensitive_string(
                        sensitive_strings,
                        sensitive_string_bytes,
                        text.clone(),
                    )?;
                }
                *text = "[REDACTED]".to_owned();
            } else {
                *text = redact_sensitive_text(text);
            }
        }
        _ => {}
    }
    Ok(())
}

fn insert_sensitive_string(
    sensitive_strings: &mut BTreeSet<String>,
    sensitive_string_bytes: &mut usize,
    text: String,
) -> Result<(), ComparisonError> {
    if text.len() > MAX_AI_SENSITIVE_VALUE_BYTES {
        return Err(ComparisonError::input(
            ComparisonErrorCode::AiInputTooLarge,
            "sensitive UI metadata value exceeds the fixed byte limit",
        ));
    }
    if sensitive_strings.contains(&text) {
        return Ok(());
    }
    if sensitive_strings.len() >= MAX_AI_SENSITIVE_VALUES {
        return Err(ComparisonError::input(
            ComparisonErrorCode::AiInputTooLarge,
            "sensitive UI metadata value count exceeds the fixed limit",
        ));
    }
    let next_bytes = sensitive_string_bytes
        .checked_add(text.len())
        .ok_or_else(|| {
            ComparisonError::input(
                ComparisonErrorCode::AiInputTooLarge,
                "sensitive UI metadata byte count overflowed the fixed limit",
            )
        })?;
    if next_bytes > MAX_AI_SENSITIVE_TOTAL_BYTES {
        return Err(ComparisonError::input(
            ComparisonErrorCode::AiInputTooLarge,
            "sensitive UI metadata values exceed the fixed total byte limit",
        ));
    }
    sensitive_strings.insert(text);
    *sensitive_string_bytes = next_bytes;
    Ok(())
}

fn merge_sensitive_strings(
    captures: &[LoadedCapture],
) -> Result<BTreeSet<String>, ComparisonError> {
    let mut merged = BTreeSet::new();
    let mut total_bytes = 0;
    for value in captures
        .iter()
        .flat_map(|capture| capture.sensitive_strings.iter())
    {
        insert_sensitive_string(&mut merged, &mut total_bytes, value.clone())?;
    }
    Ok(merged)
}

fn is_structural_ui_key(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "stable_id"
            | "node_id"
            | "document_id"
            | "panel_id"
            | "source_path"
            | "likely_files"
            | "capture_id"
            | "image_id"
            | "region_id"
            | "target_root_id"
            | "parent_id"
            | "capture_entity"
    )
}

fn is_sensitive_ui_key(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "text"
            | "value"
            | "placeholder"
            | "password"
            | "token"
            | "access_token"
            | "account"
            | "email"
            | "label"
            | "content"
            | "message"
            | "player"
            | "player_id"
            | "character"
            | "character_id"
            | "display"
            | "display_name"
            | "localized"
            | "visible_text"
            | "username"
            | "user_name"
            | "name"
            | "title"
            | "description"
            | "accessibility_label"
    )
}

struct SensitiveTextMatcher {
    matcher: Option<AhoCorasick>,
}

impl SensitiveTextMatcher {
    fn new(sensitive_strings: &BTreeSet<String>) -> Result<Self, ComparisonError> {
        let patterns = sensitive_strings
            .iter()
            .filter(|text| text.len() >= 3)
            .map(String::as_str)
            .collect::<Vec<_>>();
        let matcher = if patterns.is_empty() {
            None
        } else {
            Some(
                AhoCorasickBuilder::new()
                    .match_kind(MatchKind::LeftmostLongest)
                    // ASCII case folding catches Alice/alice; non-ASCII bytes remain exact.
                    .ascii_case_insensitive(true)
                    .build(patterns)
                    .map_err(|_| ai_internal("sensitive text matcher could not be built"))?,
            )
        };
        Ok(Self { matcher })
    }

    fn redact(&self, value: &str) -> (String, usize) {
        let Some(matcher) = &self.matcher else {
            return (value.to_owned(), 0);
        };
        let matches = matcher.find_iter(value).collect::<Vec<_>>();
        if matches.is_empty() {
            return (value.to_owned(), 0);
        }
        let mut redacted = String::with_capacity(value.len());
        let mut cursor = 0;
        for matched in &matches {
            redacted.push_str(&value[cursor..matched.start()]);
            redacted.push_str("[REDACTED]");
            cursor = matched.end();
        }
        redacted.push_str(&value[cursor..]);
        (redacted, matches.len())
    }
}

fn redact_provider_output(
    output: &mut AiProviderOutput,
    sensitive_strings: &BTreeSet<String>,
) -> Result<usize, ComparisonError> {
    let matcher = SensitiveTextMatcher::new(sensitive_strings)?;
    let mut count = 0;
    for issue in &mut output.issues {
        (issue.problem, count) = redact_and_accumulate(&issue.problem, &matcher, count);
        (issue.likely_cause, count) = redact_and_accumulate(&issue.likely_cause, &matcher, count);
        if let Some(reference_element) = &mut issue.reference_element {
            let (redacted, next_count) = redact_and_accumulate(reference_element, &matcher, count);
            *reference_element = redacted;
            count = next_count;
        }
        for evidence in &mut issue.evidence {
            (evidence.description, count) =
                redact_and_accumulate(&evidence.description, &matcher, count);
        }
    }
    Ok(count)
}

fn redact_sensitive_text(value: &str) -> String {
    redact_token_patterns(value).0
}

fn redact_and_accumulate(
    value: &str,
    matcher: &SensitiveTextMatcher,
    count: usize,
) -> (String, usize) {
    let (echo_redacted, echo_count) = matcher.redact(value);
    let (redacted, token_count) = redact_token_patterns(&echo_redacted);
    (
        redacted,
        count.saturating_add(echo_count).saturating_add(token_count),
    )
}

fn redact_token_patterns(value: &str) -> (String, usize) {
    let mut count = 0_usize;
    let mut redact_next = false;
    let redacted = value
        .split_whitespace()
        .map(|token| {
            let lower = token.to_ascii_lowercase();
            let digit_count = token.bytes().filter(u8::is_ascii_digit).count();
            let secret_assignment = [
                "api_key=",
                "api_key:",
                "api_key\":",
                "apikey=",
                "apikey:",
                "apikey\":",
                "password=",
                "password:",
                "password\":",
                "token=",
                "token:",
                "token\":",
                "access_token=",
                "access_token:",
                "access_token\":",
            ]
            .iter()
            .any(|pattern| lower.contains(pattern));
            let declares_next = lower == "bearer"
                || lower == "authorization:"
                || [
                    "api_key:",
                    "apikey:",
                    "password:",
                    "token:",
                    "access_token:",
                ]
                .iter()
                .any(|suffix| {
                    lower
                        .trim_matches(['{', '[', '(', '\"', '\''])
                        .ends_with(suffix)
                })
                || [
                    "api_key\":",
                    "apikey\":",
                    "password\":",
                    "token\":",
                    "access_token\":",
                ]
                .iter()
                .any(|suffix| lower.ends_with(suffix));
            let sensitive = redact_next
                || (token.contains('@') && token.contains('.'))
                || secret_assignment
                || lower.starts_with("sk-")
                || digit_count >= 10;
            redact_next = declares_next;
            if sensitive || declares_next {
                count = count.saturating_add(1);
                "[REDACTED]"
            } else {
                token
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    (redacted, count)
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn path_for_report(repository_root: &Path, path: &Path) -> String {
    path.strip_prefix(repository_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn ai_input_invalid(message: impl Into<String>) -> ComparisonError {
    ComparisonError::input(ComparisonErrorCode::AiInputInvalid, message)
}

fn ai_config_invalid(message: impl Into<String>) -> ComparisonError {
    ComparisonError::input(ComparisonErrorCode::AiConfigInvalid, message)
}

fn provider_response_invalid(message: impl Into<String>) -> ComparisonError {
    ComparisonError::input(ComparisonErrorCode::AiProviderResponseInvalid, message)
}

fn evidence_invalid(message: impl Into<String>) -> ComparisonError {
    ComparisonError::input(ComparisonErrorCode::AiEvidenceInvalid, message)
}

fn ai_internal(message: impl Into<String>) -> ComparisonError {
    ComparisonError::internal(ComparisonErrorCode::InternalFailure, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IdentitySource, SemanticNode, SemanticNodeRole};
    use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};
    use std::{
        net::TcpListener,
        sync::{Mutex, mpsc},
        thread,
    };
    use tempfile::tempdir;

    struct CapturingProvider {
        captured: Arc<Mutex<Vec<Vec<u8>>>>,
        descriptor: ProviderDescriptor,
    }

    impl Provider for CapturingProvider {
        fn descriptor(&self) -> ProviderDescriptor {
            self.descriptor.clone()
        }

        fn invoke(
            &self,
            request: ProviderRequest,
            context: ProviderCallContext,
        ) -> Result<ProviderResponse, ProviderError> {
            context.checkpoint()?;
            *self.captured.lock().unwrap() = request
                .images()
                .iter()
                .map(|image| image.bytes().to_vec())
                .collect();
            Ok(ProviderResponse {
                output: StructuredProviderOutput {
                    operation: ProviderOperation::VisualAnalysis,
                    schema: request.output_contract().clone(),
                    value: serde_json::to_value(AiProviderOutput {
                        schema_version: 1,
                        issues: Vec::new(),
                    })
                    .unwrap(),
                },
                server_request_id: None,
                usage: ProviderUsage::default(),
            })
        }
    }

    fn test_contract() -> StructuredOutputContract {
        StructuredOutputContract::new(AI_ANALYSIS_OUTPUT_SCHEMA_ID, 1).unwrap()
    }

    fn test_provider_request() -> ProviderRequest {
        let mut png = Vec::new();
        PngEncoder::new(&mut png)
            .write_image(&[0, 0, 0, 255], 1, 1, ExtendedColorType::Rgba8)
            .unwrap();
        ProviderRequest::visual_analysis_with_context(
            "ai-provider-test",
            AI_ANALYSIS_ALGORITHM_VERSION,
            provider_instruction(),
            Some(json!({"captures": []})),
            vec![ProviderImage::new("test.actual", "image/png", Arc::<[u8]>::from(png)).unwrap()],
            test_contract(),
        )
        .unwrap()
    }

    fn test_rect(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> SemanticRect {
        SemanticRect {
            min_x,
            min_y,
            max_x,
            max_y,
        }
    }

    fn semantic_tree_with_text_node(
        measured_text_bounds: Option<SemanticRect>,
        fully_clipped: bool,
    ) -> SemanticTree {
        let viewport = test_rect(0.0, 0.0, 100.0, 200.0);
        SemanticTree {
            schema_version: SEMANTIC_TREE_SCHEMA_VERSION,
            coordinate_space: "logical_pixels".to_owned(),
            rect_convention: "half_open".to_owned(),
            rounding: "nearest_1_64_half_away_from_zero".to_owned(),
            target_root_id: "root".to_owned(),
            viewport,
            safe_area: viewport,
            nodes: vec![SemanticNode {
                stable_id: "root/text[0]".to_owned(),
                identity_source: IdentitySource::HierarchyFallback,
                capture_entity: "1v1#test".to_owned(),
                entity_name: Some("Text".to_owned()),
                stack_index: 0,
                parent_id: None,
                depth: 0,
                role: SemanticNodeRole::Text,
                visible: true,
                fully_clipped,
                bounds: test_rect(10.0, 20.0, 60.0, 80.0),
                clip_bounds: test_rect(20.0, 30.0, 50.0, 70.0),
                measured_text_bounds,
                text_nonempty: true,
                has_visible_label: true,
                interaction: "none".to_owned(),
                disabled: false,
                loading: false,
                focused: false,
                scroll: None,
                document_id: Some("document".to_owned()),
                node_id: Some("text".to_owned()),
                source_path: Some("project/src/game/screens/page.rs".to_owned()),
                panel_id: Some("page".to_owned()),
                likely_files: vec!["project/src/game/screens/page.rs".to_owned()],
            }],
            panels: Vec::new(),
        }
    }

    fn png_crc32(bytes: &[u8]) -> u32 {
        let mut crc = u32::MAX;
        for byte in bytes {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                let mask = 0_u32.wrapping_sub(crc & 1);
                crc = (crc >> 1) ^ (0xedb8_8320 & mask);
            }
        }
        !crc
    }

    fn append_png_chunk(output: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        output.extend_from_slice(&(data.len() as u32).to_be_bytes());
        output.extend_from_slice(kind);
        output.extend_from_slice(data);
        let mut checksum_input = Vec::with_capacity(kind.len() + data.len());
        checksum_input.extend_from_slice(kind);
        checksum_input.extend_from_slice(data);
        output.extend_from_slice(&png_crc32(&checksum_input).to_be_bytes());
    }

    fn png_claiming_dimensions(width: u32, height: u32) -> Vec<u8> {
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut ihdr = Vec::with_capacity(13);
        ihdr.extend_from_slice(&width.to_be_bytes());
        ihdr.extend_from_slice(&height.to_be_bytes());
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
        append_png_chunk(&mut png, b"IHDR", &ihdr);
        append_png_chunk(&mut png, b"IDAT", &[]);
        append_png_chunk(&mut png, b"IEND", &[]);
        png
    }

    fn catalog() -> HashMap<String, CaptureEvidenceCatalog> {
        HashMap::from([(
            "login.compact.initial".to_owned(),
            CaptureEvidenceCatalog {
                image_ids: HashSet::from([
                    "login.compact.initial.reference".to_owned(),
                    "login.compact.initial.actual".to_owned(),
                    "login.compact.initial.overlay".to_owned(),
                    "login.compact.initial.heatmap".to_owned(),
                ]),
                width: 390,
                height: 844,
                region_ids: HashSet::from(["login.form".to_owned()]),
                node_ids: HashSet::from(["login.submit".to_owned()]),
                allowed_files: HashSet::from(["project/src/game/screens/auth/login.rs".to_owned()]),
            },
        )])
    }

    fn valid_output() -> AiProviderOutput {
        AiProviderOutput {
            schema_version: AI_ANALYSIS_PROVIDER_OUTPUT_SCHEMA_VERSION,
            issues: vec![AiProviderIssue {
                capture_id: "login.compact.initial".to_owned(),
                problem_type: AiProblemType::Spacing,
                severity: AiSeverity::Medium,
                problem: "Form spacing differs from the reference".to_owned(),
                evidence: vec![AiEvidence {
                    image_id: "login.compact.initial.overlay".to_owned(),
                    description: "Overlay shows a stable vertical displacement".to_owned(),
                }],
                region: AiIssueRegion {
                    region_id: Some("login.form".to_owned()),
                    bounds: Some(PixelRect {
                        x: 10,
                        y: 20,
                        width: 100,
                        height: 80,
                    }),
                },
                reference_element: Some("login form".to_owned()),
                node_id: Some("login.submit".to_owned()),
                likely_cause: "Container gap token differs".to_owned(),
                suggested_files: vec!["project/src/game/screens/auth/login.rs".to_owned()],
            }],
        }
    }

    #[test]
    fn provider_output_schema_rejects_unknown_fields_and_pass_claims() {
        let mut value = serde_json::to_value(valid_output()).unwrap();
        value["pass"] = json!(true);
        assert!(serde_json::from_value::<AiProviderOutput>(value).is_err());
        let mut issue = serde_json::to_value(valid_output()).unwrap();
        issue["issues"][0]["confidence"] = json!(0.99);
        assert!(serde_json::from_value::<AiProviderOutput>(issue).is_err());
    }

    #[test]
    fn evidence_validation_rejects_forged_capture_image_region_node_and_file() {
        let cases = [
            ("capture", "missing.capture"),
            ("image", "login.compact.initial.missing"),
            ("region", "missing.region"),
            ("node", "missing.node"),
            ("file", "project/src/game/screens/not_evidence.rs"),
        ];
        for (field, replacement) in cases {
            let mut output = valid_output();
            match field {
                "capture" => output.issues[0].capture_id = replacement.to_owned(),
                "image" => output.issues[0].evidence[0].image_id = replacement.to_owned(),
                "region" => output.issues[0].region.region_id = Some(replacement.to_owned()),
                "node" => output.issues[0].node_id = Some(replacement.to_owned()),
                "file" => output.issues[0].suggested_files = vec![replacement.to_owned()],
                _ => unreachable!(),
            }
            let error = validate_provider_output(&output, &catalog()).unwrap_err();
            assert_eq!(error.failure.code, ComparisonErrorCode::AiEvidenceInvalid);
        }
        validate_provider_output(&valid_output(), &catalog()).unwrap();
    }

    #[test]
    fn evidence_bounds_are_half_open_and_must_stay_inside_the_capture() {
        let mut output = valid_output();
        output.issues[0].region.bounds = Some(PixelRect {
            x: 389,
            y: 0,
            width: 2,
            height: 1,
        });
        assert_eq!(
            validate_provider_output(&output, &catalog())
                .unwrap_err()
                .failure
                .code,
            ComparisonErrorCode::AiEvidenceInvalid
        );
    }

    #[test]
    fn failure_classification_covers_provider_lifecycle_categories() {
        let cases = [
            (
                TaskFailureKind::ProviderTimeout,
                ComparisonErrorCode::AiProviderTimeout,
            ),
            (
                TaskFailureKind::ProviderRateLimited,
                ComparisonErrorCode::AiProviderRateLimited,
            ),
            (
                TaskFailureKind::ProviderAuthentication,
                ComparisonErrorCode::AiProviderAuthentication,
            ),
            (
                TaskFailureKind::ProviderServiceUnavailable,
                ComparisonErrorCode::AiProviderServiceUnavailable,
            ),
            (
                TaskFailureKind::ProviderResponseMalformed,
                ComparisonErrorCode::AiProviderResponseInvalid,
            ),
            (
                TaskFailureKind::ProviderCapabilityUnsupported,
                ComparisonErrorCode::AiProviderUnsupported,
            ),
        ];
        for (kind, expected) in cases {
            assert_eq!(provider_failure_code(kind), expected);
        }
    }

    #[test]
    fn output_redaction_removes_credentials_emails_and_token_shapes() {
        let mut output = valid_output();
        output.issues[0].problem =
            "contact player@example.test with token: private-value and secret player".to_owned();
        output.issues[0].likely_cause =
            "Bearer secret-value sk-private {\"access_token\":\"json-secret\"} {\"password\": \"json-password\"} 13800138000".to_owned();
        let count =
            redact_provider_output(&mut output, &BTreeSet::from(["Secret Player".to_owned()]))
                .unwrap();
        let persisted = serde_json::to_string(&output).unwrap();
        for sensitive in [
            "player@example.test",
            "private-value",
            "secret-value",
            "sk-private",
            "json-secret",
            "13800138000",
            "Secret Player",
            "secret player",
            "json-password",
        ] {
            assert!(!persisted.contains(sensitive));
        }
        assert!(persisted.contains("[REDACTED]"));
        assert!(count >= 6);
    }

    #[test]
    fn ui_metadata_redacts_text_values_but_keeps_structural_node_ids() {
        let mut metadata = json!({
            "semantic_tree": {"nodes": [{
                "stable_id": "root/login.submit",
                "node_id": "login.submit",
                "document_id": "login-document",
                "panel_id": "login-panel",
                "source_path": "project/src/game/screens/auth/login.rs",
                "likely_files": ["project/src/game/screens/auth/login.rs"],
                "text": "player@example.test"
            }]},
            "token": "private-token",
            "display_name": "Visible Player",
            "localized": "本地化敏感消息",
            "message": "Account message",
            "username": "Alice",
            "user_name": "Alice User",
            "name": "Alice Name",
            "title": "Private Title",
            "description": "Private Description",
            "accessibility_label": "Private Accessibility Label"
        });
        let mut sensitive_strings = BTreeSet::new();
        let mut sensitive_string_bytes = 0;
        redact_ui_metadata(
            &mut metadata,
            None,
            &mut sensitive_strings,
            &mut sensitive_string_bytes,
        )
        .unwrap();
        assert_eq!(
            metadata["semantic_tree"]["nodes"][0]["node_id"],
            "login.submit"
        );
        assert_eq!(
            metadata["semantic_tree"]["nodes"][0]["source_path"],
            "project/src/game/screens/auth/login.rs"
        );
        assert_eq!(metadata["semantic_tree"]["nodes"][0]["text"], "[REDACTED]");
        assert_eq!(metadata["token"], "[REDACTED]");
        assert_eq!(metadata["display_name"], "[REDACTED]");
        assert_eq!(metadata["localized"], "[REDACTED]");
        for key in [
            "username",
            "user_name",
            "name",
            "title",
            "description",
            "accessibility_label",
        ] {
            assert_eq!(metadata[key], "[REDACTED]");
        }
        assert!(sensitive_strings.contains("Visible Player"));
        assert_eq!(
            sensitive_string_bytes,
            sensitive_strings.iter().map(String::len).sum::<usize>()
        );
    }

    #[test]
    fn sensitive_value_collection_rejects_count_and_total_byte_limit_plus_one() {
        let mut count_values = BTreeSet::new();
        let mut count_bytes = 0;
        for index in 0..MAX_AI_SENSITIVE_VALUES {
            insert_sensitive_string(
                &mut count_values,
                &mut count_bytes,
                format!("sensitive-{index:04}"),
            )
            .unwrap();
        }
        assert_eq!(count_values.len(), MAX_AI_SENSITIVE_VALUES);
        assert_eq!(
            insert_sensitive_string(
                &mut count_values,
                &mut count_bytes,
                "count-plus-one".to_owned(),
            )
            .unwrap_err()
            .failure
            .code,
            ComparisonErrorCode::AiInputTooLarge
        );

        let mut total_values = BTreeSet::new();
        let mut total_bytes = 0;
        for index in 0..(MAX_AI_SENSITIVE_TOTAL_BYTES / MAX_AI_SENSITIVE_VALUE_BYTES) {
            let value = format!("{index:04}{}", "x".repeat(MAX_AI_SENSITIVE_VALUE_BYTES - 4));
            insert_sensitive_string(&mut total_values, &mut total_bytes, value).unwrap();
        }
        assert_eq!(total_bytes, MAX_AI_SENSITIVE_TOTAL_BYTES);
        assert_eq!(
            insert_sensitive_string(&mut total_values, &mut total_bytes, "x".to_owned())
                .unwrap_err()
                .failure
                .code,
            ComparisonErrorCode::AiInputTooLarge
        );
    }

    #[test]
    fn online_mode_is_explicit_https_and_keeps_generation_and_audit_models_independent() {
        let config = AiAnalysisConfig {
            schema_version: AI_ANALYSIS_CONFIG_SCHEMA_VERSION,
            algorithm_version: AI_ANALYSIS_ALGORITHM_VERSION.to_owned(),
            provider: AiProviderConfig::Online {
                enabled: true,
                provider_id: "openai-compatible".to_owned(),
                audit_model_id: "audit-model-v1".to_owned(),
                generation_model_id: Some("generation-model-v2".to_owned()),
                endpoint: "https://provider.example.test/v1/chat/completions".to_owned(),
                credential_environment: "UI_VISUAL_AUDIT_API_KEY".to_owned(),
            },
            policy: AiProviderPolicy {
                attempt_timeout_ms: 1_000,
                minimum_request_interval_ms: 0,
                max_attempts: 1,
                initial_backoff_ms: 0,
                max_backoff_ms: 0,
                max_output_tokens: 1024,
            },
        };
        validate_config(&config).unwrap();
        let mut queried = config.clone();
        if let AiProviderConfig::Online { endpoint, .. } = &mut queried.provider {
            *endpoint =
                "https://provider.example.test/v1/chat/completions?key=forbidden".to_owned();
        }
        assert_eq!(
            validate_config(&queried).unwrap_err().failure.code,
            ComparisonErrorCode::AiConfigInvalid
        );
        let mut disabled = config.clone();
        if let AiProviderConfig::Online { enabled, .. } = &mut disabled.provider {
            *enabled = false;
        }
        assert_eq!(
            validate_config(&disabled).unwrap_err().failure.code,
            ComparisonErrorCode::AiProviderUnsupported
        );
    }

    #[test]
    fn execution_policy_binds_iteration_budget_to_the_attempt_limit() {
        let policy = AiProviderPolicy {
            attempt_timeout_ms: 1_000,
            minimum_request_interval_ms: 0,
            max_attempts: 3,
            initial_backoff_ms: 100,
            max_backoff_ms: 1_000,
            max_output_tokens: 1_024,
        };

        let execution = execution_policy(&policy).unwrap();
        assert_eq!(execution.task_limits.max_provider_calls, 3);
        assert_eq!(execution.task_limits.max_iterations, 3);
    }

    #[test]
    fn provider_response_limit_and_strict_openai_response_are_enforced() {
        let contract = StructuredOutputContract::new(AI_ANALYSIS_OUTPUT_SCHEMA_ID, 1).unwrap();
        let request = test_provider_request();
        let request_body = build_openai_request("audit-model", 777, &request).unwrap();
        assert_eq!(request.operation(), ProviderOperation::VisualAnalysis);
        assert_eq!(request_body["max_completion_tokens"], 777);
        assert!(
            request_body["messages"][0]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("captures")
        );
        let body = json!({
            "choices": [{"message": {"content": serde_json::to_string(&valid_output()).unwrap()}}],
            "usage": {"prompt_tokens": 12, "completion_tokens": 8}
        });
        let response =
            parse_openai_response(&serde_json::to_vec(&body).unwrap(), &contract).unwrap();
        assert_eq!(response.output.operation, ProviderOperation::VisualAnalysis);
        assert_eq!(response.usage.input_units, Some(12));
        assert!(parse_openai_response(br#"{"choices":[]}"#, &contract).is_err());
    }

    #[test]
    fn fixed_size_input_limit_rejects_oversized_files_before_allocation() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("oversized.json");
        let file = fs::File::create(&path).unwrap();
        file.set_len(MAX_BUNDLE_BYTES + 1).unwrap();
        let error = read_limited(
            &path,
            MAX_BUNDLE_BYTES,
            ComparisonErrorCode::AiInputTooLarge,
        )
        .unwrap_err();
        assert_eq!(error.failure.code, ComparisonErrorCode::AiInputTooLarge);
    }

    #[test]
    fn image_validation_fully_decodes_the_same_bounded_bytes_snapshot() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("capture.png");
        let mut png = Vec::new();
        PngEncoder::new(&mut png)
            .write_image(
                &[10, 20, 30, 255, 40, 50, 60, 255],
                2,
                1,
                ExtendedColorType::Rgba8,
            )
            .unwrap();
        fs::write(&path, &png).unwrap();
        let snapshot = read_limited(
            &path,
            MAX_AI_IMAGE_BYTES,
            ComparisonErrorCode::AiImageTooLarge,
        )
        .unwrap();
        fs::write(&path, &snapshot[..snapshot.len() / 2]).unwrap();
        assert_eq!(decode_input_image(&path, &snapshot).unwrap().1, 2);
        let changed = fs::read(&path).unwrap();
        assert!(decode_input_image(&path, &changed).is_err());
        assert_eq!(sha256(&snapshot), sha256(&png));
    }

    #[test]
    fn image_limit_plus_one_and_truncated_png_fail_before_provider_construction() {
        let directory = tempdir().unwrap();
        let oversized = directory.path().join("oversized.png");
        fs::File::create(&oversized)
            .unwrap()
            .set_len(MAX_AI_IMAGE_BYTES + 1)
            .unwrap();
        assert_eq!(
            read_limited(
                &oversized,
                MAX_AI_IMAGE_BYTES,
                ComparisonErrorCode::AiImageTooLarge,
            )
            .unwrap_err()
            .failure
            .code,
            ComparisonErrorCode::AiImageTooLarge
        );
        let truncated = directory.path().join("truncated.png");
        fs::write(&truncated, b"\x89PNG\r\n\x1a\n").unwrap();
        assert!(decode_input_image(&truncated, &fs::read(&truncated).unwrap()).is_err());
    }

    #[test]
    fn huge_png_header_is_rejected_by_budget_before_full_decode() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("huge-header.png");
        let png = png_claiming_dimensions(MAX_AI_IMAGE_DIMENSION, MAX_AI_IMAGE_DIMENSION);
        fs::write(&path, &png).unwrap();
        let preflight = preflight_image_dimensions(&path, &png).unwrap();
        assert_eq!(preflight.width, MAX_AI_IMAGE_DIMENSION);
        assert_eq!(preflight.height, MAX_AI_IMAGE_DIMENSION);

        let mut total_pixels = 0;
        let mut total_bytes = 0;
        assert_eq!(
            reserve_decoded_budget(
                preflight.width,
                preflight.height,
                &mut total_pixels,
                &mut total_bytes,
            )
            .unwrap_err()
            .failure
            .code,
            ComparisonErrorCode::AiImageTooLarge
        );
        assert_eq!(
            decode_input_image(&path, &png).unwrap_err().failure.code,
            ComparisonErrorCode::AiImageTooLarge
        );
    }

    #[test]
    fn provider_receives_opaque_redacted_pixels_without_modifying_source_artifact() {
        let directory = tempdir().unwrap();
        let source_path = directory.path().join("source.png");
        let mut source = RgbaImage::from_pixel(4, 2, image::Rgba([10, 20, 30, 255]));
        source.put_pixel(3, 1, image::Rgba([200, 210, 220, 255]));
        source.save(&source_path).unwrap();
        let original_bytes = fs::read(&source_path).unwrap();
        let mut provider_pixels = source.clone();
        apply_opaque_redaction(
            &mut provider_pixels,
            PixelRect {
                x: 0,
                y: 0,
                width: 2,
                height: 1,
            },
        );
        let mut encoded = Vec::new();
        PngEncoder::new(&mut encoded)
            .write_image(provider_pixels.as_raw(), 4, 2, ExtendedColorType::Rgba8)
            .unwrap();
        let request = ProviderRequest::visual_analysis_with_context(
            "redaction-capture",
            AI_ANALYSIS_ALGORITHM_VERSION,
            provider_instruction(),
            Some(json!({"redacted": true})),
            vec![
                ProviderImage::new("capture.actual", "image/png", Arc::<[u8]>::from(encoded))
                    .unwrap(),
            ],
            test_contract(),
        )
        .unwrap();
        let captured = Arc::new(Mutex::new(Vec::new()));
        let id = ProviderId::new("capture-redacted").unwrap();
        let provider = Arc::new(CapturingProvider {
            captured: Arc::clone(&captured),
            descriptor: ProviderDescriptor {
                id: id.clone(),
                capabilities: ProviderCapabilities {
                    image_input: true,
                    structured_output: true,
                    max_image_count: 1,
                    operations: BTreeSet::from([ProviderOperation::VisualAnalysis]),
                },
            },
        });
        let mut registry = ProviderRegistry::default();
        registry.register(provider).unwrap();
        ProviderRunner::new(registry, ProviderExecutionPolicy::default())
            .unwrap()
            .execute(&id, request, &CancellationToken::default())
            .unwrap();
        let sent = image::load_from_memory(&captured.lock().unwrap()[0])
            .unwrap()
            .into_rgba8();
        assert_eq!(sent.get_pixel(0, 0).0, [0, 0, 0, 255]);
        assert_eq!(sent.get_pixel(2, 0).0, [10, 20, 30, 255]);
        assert_eq!(sent.get_pixel(3, 1).0, [200, 210, 220, 255]);
        assert_eq!(fs::read(source_path).unwrap(), original_bytes);
    }

    #[test]
    fn online_agent_never_follows_redirects_or_forwards_authorization() {
        let redirect_target = TcpListener::bind("127.0.0.1:0").unwrap();
        redirect_target.set_nonblocking(true).unwrap();
        let target_url = format!("http://{}/capture", redirect_target.local_addr().unwrap());
        let origin = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin_url = format!("http://{}/analyze", origin.local_addr().unwrap());
        let (request_sender, request_receiver) = mpsc::channel();
        let origin_thread = thread::spawn(move || {
            let (mut stream, _) = origin.accept().unwrap();
            let mut request = Vec::new();
            let mut buffer = [0_u8; 4096];
            let mut expected_length = None;
            loop {
                let count = stream.read(&mut buffer).unwrap();
                if count == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..count]);
                if expected_length.is_none()
                    && let Some(header_end) = request
                        .windows(4)
                        .position(|window| window == b"\r\n\r\n")
                        .map(|index| index + 4)
                {
                    let headers = String::from_utf8_lossy(&request[..header_end]);
                    let content_length = headers
                        .lines()
                        .find_map(|line| {
                            line.strip_prefix("Content-Length: ")
                                .or_else(|| line.strip_prefix("content-length: "))
                        })
                        .and_then(|value| value.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    expected_length = Some(header_end + content_length);
                }
                if expected_length.is_some_and(|length| request.len() >= length) {
                    break;
                }
            }
            request_sender.send(request).unwrap();
            write!(
                stream,
                "HTTP/1.1 302 Found\r\nLocation: {target_url}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            )
            .unwrap();
        });
        let id = ProviderId::new("redirect-zero").unwrap();
        let provider = Arc::new(OpenAiCompatibleProvider {
            descriptor: ProviderDescriptor {
                id: id.clone(),
                capabilities: ProviderCapabilities {
                    image_input: true,
                    structured_output: true,
                    max_image_count: 4,
                    operations: BTreeSet::from([ProviderOperation::VisualAnalysis]),
                },
            },
            endpoint: Url::parse(&origin_url).unwrap(),
            model: "fixture-model".to_owned(),
            credential: SecretString::new("fixture-secret".to_owned()).unwrap(),
            agent: ureq::AgentBuilder::new().redirects(0).build(),
            max_output_tokens: 128,
        });
        let mut registry = ProviderRegistry::default();
        registry.register(provider).unwrap();
        let mut policy = ProviderExecutionPolicy::default();
        policy.retry.max_attempts = 1;
        policy.attempt_timeout = Duration::from_secs(2);
        let failure = ProviderRunner::new(registry, policy)
            .unwrap()
            .execute(&id, test_provider_request(), &CancellationToken::default())
            .unwrap_err();
        assert_eq!(
            failure.failure.kind(),
            TaskFailureKind::ProviderResponseMalformed
        );
        origin_thread.join().unwrap();
        let origin_request = String::from_utf8(request_receiver.recv().unwrap()).unwrap();
        assert!(origin_request.contains("Authorization: Bearer fixture-secret"));
        assert!(matches!(
            redirect_target.accept(),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock
        ));
    }

    #[test]
    fn semantic_text_rect_mapping_uses_clip_and_logical_to_physical_scale() {
        let rect = map_logical_redaction(
            SemanticRect {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 100.0,
                max_y: 200.0,
            },
            SemanticRect {
                min_x: 10.0,
                min_y: 20.0,
                max_x: 60.0,
                max_y: 80.0,
            },
            SemanticRect {
                min_x: 20.0,
                min_y: 30.0,
                max_x: 50.0,
                max_y: 70.0,
            },
            200,
            400,
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            rect,
            PixelRect {
                x: 40,
                y: 60,
                width: 60,
                height: 80,
            }
        );
    }

    #[test]
    fn visible_semantic_text_without_measured_bounds_fails_closed() {
        let tree = semantic_tree_with_text_node(None, false);
        assert_eq!(
            semantic_text_redaction_rects(&tree, 200, 400)
                .unwrap_err()
                .failure
                .code,
            ComparisonErrorCode::AiInputInvalid
        );
    }

    #[test]
    fn fully_clipped_semantic_text_without_measured_bounds_can_skip_masking() {
        let tree = semantic_tree_with_text_node(None, true);
        assert!(
            semantic_text_redaction_rects(&tree, 200, 400)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn visible_semantic_text_maps_to_nonempty_provider_mask() {
        let tree = semantic_tree_with_text_node(Some(test_rect(10.0, 20.0, 60.0, 80.0)), false);
        assert_eq!(
            semantic_text_redaction_rects(&tree, 200, 400).unwrap(),
            vec![PixelRect {
                x: 40,
                y: 60,
                width: 60,
                height: 80,
            }]
        );
    }

    #[test]
    fn explicit_privacy_rectangle_remains_in_combined_provider_masks() {
        let mut tree = semantic_tree_with_text_node(None, true);
        tree.nodes.clear();
        let explicit = PixelRect {
            x: 1,
            y: 2,
            width: 3,
            height: 4,
        };
        assert_eq!(
            validated_provider_redaction_rects(&tree, &[explicit], true, 200, 400).unwrap(),
            vec![explicit]
        );
    }

    #[test]
    fn hard_failure_contract_has_no_provider_pass_or_downgrade_field() {
        let schema = provider_output_json_schema();
        let properties = schema["properties"].as_object().unwrap();
        assert!(!properties.contains_key("pass"));
        assert!(!properties.contains_key("deterministic_hard_failures"));
        assert!(provider_instruction().contains("cannot be removed or downgraded"));

        let mut downgrade = valid_output();
        downgrade.issues[0].problem_type = AiProblemType::HardFailureExplanation;
        downgrade.issues[0].severity = AiSeverity::Minor;
        assert_eq!(
            validate_provider_output(&downgrade, &catalog())
                .unwrap_err()
                .failure
                .code,
            ComparisonErrorCode::AiProviderResponseInvalid
        );
    }

    #[test]
    fn fixture_provider_runs_through_the_shared_bounded_runner() {
        let directory = tempdir().unwrap();
        let response_path = directory.path().join("response.json");
        fs::write(
            &response_path,
            serde_json::to_vec_pretty(&valid_output()).unwrap(),
        )
        .unwrap();
        let provider_config = AiProviderConfig::Fixture {
            provider_id: "fixture-ai".to_owned(),
            audit_model_id: "fixture-audit-v1".to_owned(),
            generation_model_id: Some("fixture-generation-v1".to_owned()),
            response: response_path.clone(),
        };
        let root = fs::canonicalize(directory.path()).unwrap();
        let built = build_provider(
            &root,
            std::slice::from_ref(&root),
            &provider_config,
            &test_contract(),
            1024,
        )
        .unwrap();
        let mut registry = ProviderRegistry::default();
        registry.register(built.provider).unwrap();
        let runner = ProviderRunner::new(
            registry,
            execution_policy(&AiProviderPolicy {
                attempt_timeout_ms: 100,
                minimum_request_interval_ms: 0,
                max_attempts: 1,
                initial_backoff_ms: 0,
                max_backoff_ms: 0,
                max_output_tokens: 1024,
            })
            .unwrap(),
        )
        .unwrap();
        let execution = runner
            .execute(
                &built.id,
                test_provider_request(),
                &CancellationToken::default(),
            )
            .unwrap();
        let output: AiProviderOutput =
            serde_json::from_value(execution.response.output.value).unwrap();
        assert_eq!(output, valid_output());
        assert_eq!(built.mode, "fixture");
        assert_eq!(built.audit_model_id, "fixture-audit-v1");
        assert_eq!(
            built.generation_model_id.as_deref(),
            Some("fixture-generation-v1")
        );
    }

    #[test]
    fn mock_adapter_exercises_every_required_failure_without_network() {
        let directory = tempdir().unwrap();
        let cases = [
            (
                AiMockScenario::Timeout,
                ComparisonErrorCode::AiProviderTimeout,
            ),
            (
                AiMockScenario::RateLimited,
                ComparisonErrorCode::AiProviderRateLimited,
            ),
            (
                AiMockScenario::AuthenticationFailure,
                ComparisonErrorCode::AiProviderAuthentication,
            ),
            (
                AiMockScenario::ServiceUnavailable,
                ComparisonErrorCode::AiProviderServiceUnavailable,
            ),
            (
                AiMockScenario::MalformedResponse,
                ComparisonErrorCode::AiProviderResponseInvalid,
            ),
            (
                AiMockScenario::Unsupported,
                ComparisonErrorCode::AiProviderUnsupported,
            ),
        ];
        for (scenario, expected) in cases {
            let attempt_timeout_ms = if scenario == AiMockScenario::Timeout {
                10
            } else {
                5_000
            };
            let provider_config = AiProviderConfig::Mock {
                provider_id: format!("mock-{scenario:?}").to_ascii_lowercase(),
                audit_model_id: "mock-audit-v1".to_owned(),
                generation_model_id: None,
                scenario,
                response: None,
            };
            let built = build_provider(
                directory.path(),
                &[directory.path().to_path_buf()],
                &provider_config,
                &test_contract(),
                1024,
            )
            .unwrap();
            let mut registry = ProviderRegistry::default();
            registry.register(built.provider).unwrap();
            let runner = ProviderRunner::new(
                registry,
                execution_policy(&AiProviderPolicy {
                    attempt_timeout_ms,
                    minimum_request_interval_ms: 0,
                    max_attempts: 1,
                    initial_backoff_ms: 0,
                    max_backoff_ms: 0,
                    max_output_tokens: 1024,
                })
                .unwrap(),
            )
            .unwrap();
            let failure = runner
                .execute(
                    &built.id,
                    test_provider_request(),
                    &CancellationToken::default(),
                )
                .unwrap_err();
            assert_eq!(
                map_provider_failure(failure).failure.code,
                expected,
                "mock scenario {scenario:?} must retain its own failure classification"
            );
        }
    }

    #[test]
    #[ignore = "explicit online sample; requires UI_VISUAL_AUDIT_ONLINE_SAMPLE_ENDPOINT, UI_VISUAL_AUDIT_API_KEY, and UI_VISUAL_AUDIT_AUDIT_MODEL"]
    fn explicit_online_openai_compatible_sample() {
        let endpoint = std::env::var("UI_VISUAL_AUDIT_ONLINE_SAMPLE_ENDPOINT").unwrap();
        let model = std::env::var("UI_VISUAL_AUDIT_AUDIT_MODEL").unwrap();
        let credential = CredentialResolver::environment_only()
            .resolve(
                &CredentialLocator::new(Some("UI_VISUAL_AUDIT_API_KEY"), None::<String>).unwrap(),
            )
            .unwrap();
        let id = ProviderId::new("online-opt-in-sample").unwrap();
        let provider = Arc::new(OpenAiCompatibleProvider {
            descriptor: ProviderDescriptor {
                id: id.clone(),
                capabilities: ProviderCapabilities {
                    image_input: true,
                    structured_output: true,
                    max_image_count: 4,
                    operations: BTreeSet::from([ProviderOperation::VisualAnalysis]),
                },
            },
            endpoint: Url::parse(&endpoint).unwrap(),
            model,
            credential,
            agent: ureq::AgentBuilder::new().redirects(0).build(),
            max_output_tokens: 1024,
        });
        let mut registry = ProviderRegistry::default();
        registry.register(provider).unwrap();
        let runner = ProviderRunner::new(registry, ProviderExecutionPolicy::default()).unwrap();
        let execution = runner
            .execute(&id, test_provider_request(), &CancellationToken::default())
            .unwrap();
        let output: AiProviderOutput =
            serde_json::from_value(execution.response.output.value).unwrap();
        assert_eq!(
            output.schema_version,
            AI_ANALYSIS_PROVIDER_OUTPUT_SCHEMA_VERSION
        );
    }
}
