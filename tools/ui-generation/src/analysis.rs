use crate::{
    contract::GenerationTask,
    lifecycle::TaskFailure,
    preprocess::{ArtifactKind, ReferencePreprocessManifest},
    provider::{
        ProviderExecution, ProviderOperation, StructuredOutputContract, StructuredProviderOutput,
    },
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::LazyLock,
};

pub const ANALYSIS_SCHEMA_ID: &str = "ui-reference-analysis";
pub const ANALYSIS_SCHEMA_VERSION: u32 = 1;
pub const MAX_ANALYSIS_JSON_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_ANALYSIS_REFERENCES: usize = 16;
pub const MAX_ANALYSIS_REGIONS: usize = 128;
pub const MAX_ANALYSIS_ELEMENTS: usize = 512;
pub const MAX_ANALYSIS_DEPTH: usize = 24;
pub const MAX_ANALYSIS_EVIDENCE: usize = 2_048;
pub const MAX_ANALYSIS_UNCERTAINTIES: usize = 256;
pub const MAX_TEXT_CANDIDATES: usize = 16;
pub const MAX_TEXT_LENGTH: usize = 1_024;

const MAX_DIAGNOSTICS: usize = 128;
const MAX_JSON_NODES: usize = 25_000;
const MAX_JSON_DEPTH: usize = 64;
const MAX_JSON_CONTAINER_ITEMS: usize = 4_096;
const MAX_JSON_STRING_BYTES: usize = 16_384;
const MAX_JSON_TOTAL_STRING_BYTES: usize = 512 * 1024;
const SAFE_ID_PATTERN: &str = "^[a-z][a-z0-9_]*(\\.[a-z][a-z0-9_]*)*$";
const SAFE_LABEL_PATTERN: &str = "^[A-Za-z0-9][A-Za-z0-9._:-]*$";
const SHA256_PATTERN: &str = "^[0-9a-f]{64}$";

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UiReferenceAnalysis {
    #[schemars(regex(pattern = "^ui-reference-analysis$"))]
    pub schema_id: String,
    #[schemars(range(min = 1, max = 1))]
    pub schema_version: u32,
    #[schemars(length(min = 1, max = 128), regex(pattern = SAFE_ID_PATTERN))]
    pub analysis_id: String,
    #[schemars(length(min = 1, max = 128), regex(pattern = SAFE_LABEL_PATTERN))]
    pub run_id: String,
    pub provider: AnalysisProviderProvenance,
    #[schemars(length(min = 1, max = 16))]
    pub references: Vec<AnalysisReference>,
    #[schemars(length(min = 1, max = 128))]
    pub regions: Vec<AnalysisRegion>,
    #[schemars(length(min = 1, max = 512))]
    pub elements: Vec<AnalysisElement>,
    #[schemars(length(min = 1, max = 128), regex(pattern = SAFE_ID_PATTERN))]
    pub root_element_id: String,
    #[schemars(length(min = 1, max = 2048))]
    pub evidence: Vec<AnalysisEvidence>,
    #[schemars(length(max = 256))]
    pub uncertainties: Vec<AnalysisUncertainty>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisProviderProvenance {
    #[schemars(length(min = 1, max = 64), regex(pattern = SAFE_LABEL_PATTERN))]
    pub provider_id: String,
    #[schemars(length(min = 1, max = 128), regex(pattern = SAFE_LABEL_PATTERN))]
    pub server_request_id: String,
    #[schemars(length(min = 1, max = 128), regex(pattern = SAFE_LABEL_PATTERN))]
    pub prompt_version: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisReference {
    #[schemars(length(min = 1, max = 128), regex(pattern = SAFE_ID_PATTERN))]
    pub reference_id: String,
    #[schemars(length(equal = 64), regex(pattern = SHA256_PATTERN))]
    pub source_sha256: String,
    #[schemars(length(equal = 64), regex(pattern = SHA256_PATTERN))]
    pub preprocess_cache_key: String,
    #[schemars(range(min = 1))]
    pub preprocess_protocol_version: u32,
    #[schemars(length(min = 1, max = 128), regex(pattern = SAFE_LABEL_PATTERN))]
    pub preprocess_implementation_version: String,
    #[schemars(length(equal = 64), regex(pattern = SHA256_PATTERN))]
    pub preprocess_manifest_sha256: String,
    #[schemars(length(equal = 64), regex(pattern = SHA256_PATTERN))]
    pub standard_preview_sha256: String,
    pub coordinate_space: AnalysisCoordinateSpace,
    #[schemars(length(min = 1, max = 256))]
    pub coordinate_convention: String,
    #[schemars(range(min = 1, max = 4096))]
    pub width: u32,
    #[schemars(range(min = 1, max = 4096))]
    pub height: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TrustedPreprocessEvidence {
    pub reference_id: String,
    pub source_sha256: String,
    pub preprocess_cache_key: String,
    pub preprocess_protocol_version: u32,
    pub preprocess_implementation_version: String,
    pub preprocess_manifest_sha256: String,
    pub standard_preview_sha256: String,
    pub coordinate_space: AnalysisCoordinateSpace,
    pub coordinate_convention: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TrustedProviderEvidence {
    run_id: String,
    provider_id: String,
    server_request_id: Option<String>,
    prompt_version: String,
}

impl TrustedProviderEvidence {
    pub fn from_execution(execution: &ProviderExecution) -> Self {
        Self {
            run_id: execution.trace.request.run_id.clone(),
            provider_id: execution.trace.provider_id.as_str().to_owned(),
            server_request_id: execution
                .response
                .server_request_id
                .as_ref()
                .map(|request_id| request_id.as_str().to_owned()),
            prompt_version: execution.trace.request.prompt_version.clone(),
        }
    }

    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    pub fn server_request_id(&self) -> Option<&str> {
        self.server_request_id.as_deref()
    }

    pub fn prompt_version(&self) -> &str {
        &self.prompt_version
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TrustedHumanInput {
    input_id: String,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    context: Option<String>,
}

impl TrustedHumanInput {
    pub fn input_id(&self) -> &str {
        &self.input_id
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn context(&self) -> Option<&str> {
        self.context.as_deref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TrustedAnalysisContext {
    provider: TrustedProviderEvidence,
    preprocess: Vec<TrustedPreprocessEvidence>,
    human_inputs: Vec<TrustedHumanInput>,
}

impl TrustedAnalysisContext {
    pub fn from_execution_and_task(
        execution: &ProviderExecution,
        task: &GenerationTask,
        preprocess: &[TrustedPreprocessEvidence],
    ) -> Result<Self, AnalysisValidationReport> {
        if execution.trace.request.run_id != task.run_id {
            return Err(AnalysisValidationReport::one(
                "ANALYSIS_TRUSTED_RUN_ID_MISMATCH",
                "$.run_id",
                "executed provider request run_id differs from the generation task",
            ));
        }
        let human_inputs = trusted_human_inputs_from_task(task).map_err(|failure| {
            AnalysisValidationReport::one(
                "ANALYSIS_TRUSTED_HUMAN_INPUT_INVALID",
                "$.visible_text",
                failure.message(),
            )
        })?;
        Ok(Self {
            provider: TrustedProviderEvidence::from_execution(execution),
            preprocess: preprocess.to_vec(),
            human_inputs,
        })
    }

    pub fn provider(&self) -> &TrustedProviderEvidence {
        &self.provider
    }

    pub fn preprocess(&self) -> &[TrustedPreprocessEvidence] {
        &self.preprocess
    }

    pub fn human_inputs(&self) -> &[TrustedHumanInput] {
        &self.human_inputs
    }
}

pub fn trusted_human_inputs_from_task(
    task: &GenerationTask,
) -> Result<Vec<TrustedHumanInput>, TaskFailure> {
    if task.visible_text.len() > MAX_ANALYSIS_EVIDENCE {
        return Err(TaskFailure::invalid(format!(
            "visible_text contains more than {MAX_ANALYSIS_EVIDENCE} trusted entries"
        )));
    }
    task.visible_text
        .iter()
        .enumerate()
        .map(|(index, input)| {
            if input.text.is_empty() || input.text.len() > MAX_TEXT_LENGTH {
                return Err(TaskFailure::invalid(format!(
                    "$.visible_text[{index}].text must contain 1..={MAX_TEXT_LENGTH} UTF-8 bytes"
                )));
            }
            if input
                .context
                .as_ref()
                .is_some_and(|context| context.len() > 512)
            {
                return Err(TaskFailure::invalid(format!(
                    "$.visible_text[{index}].context must not exceed 512 UTF-8 bytes"
                )));
            }
            Ok(TrustedHumanInput {
                input_id: format!("task.visible_text.{index:04}"),
                text: input.text.clone(),
                context: input.context.clone(),
            })
        })
        .collect()
}

impl TrustedPreprocessEvidence {
    pub fn from_manifest(
        manifest: &ReferencePreprocessManifest,
        canonical_manifest_bytes: &[u8],
    ) -> Result<Self, TaskFailure> {
        let serialized_manifest: ReferencePreprocessManifest =
            serde_json::from_slice(canonical_manifest_bytes).map_err(|_| {
                TaskFailure::invalid("trusted preprocess manifest bytes are not valid JSON")
            })?;
        if serialized_manifest != *manifest {
            return Err(TaskFailure::invalid(
                "trusted preprocess manifest bytes do not describe the supplied manifest",
            ));
        }
        let preview = manifest
            .artifacts
            .iter()
            .find(|artifact| {
                artifact.kind == ArtifactKind::StandardPreview && !artifact.auxiliary_only
            })
            .ok_or_else(|| {
                TaskFailure::invalid(
                    "preprocess manifest lacks an authoritative standard preview artifact",
                )
            })?;
        if preview.width != manifest.coordinate_mapping.preview_size.width
            || preview.height != manifest.coordinate_mapping.preview_size.height
        {
            return Err(TaskFailure::invalid(
                "preprocess manifest standard preview dimensions disagree with coordinate mapping",
            ));
        }
        Ok(Self {
            reference_id: manifest.reference_id.clone(),
            source_sha256: manifest.source_sha256.clone(),
            preprocess_cache_key: manifest.cache_key.clone(),
            preprocess_protocol_version: manifest.protocol_version,
            preprocess_implementation_version: manifest.implementation_version.clone(),
            preprocess_manifest_sha256: sha256_hex(canonical_manifest_bytes),
            standard_preview_sha256: preview.sha256.clone(),
            coordinate_space: AnalysisCoordinateSpace::StandardPreviewPixel,
            coordinate_convention: manifest.coordinate_mapping.coordinate_convention.clone(),
            width: preview.width,
            height: preview.height,
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisCoordinateSpace {
    StandardPreviewPixel,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisRegion {
    #[schemars(length(min = 1, max = 128), regex(pattern = SAFE_ID_PATTERN))]
    pub region_id: String,
    #[schemars(length(min = 1, max = 128), regex(pattern = SAFE_ID_PATTERN))]
    pub reference_id: String,
    #[schemars(length(min = 1, max = 128))]
    pub label: String,
    pub bounding_box: AnalysisBoundingBox,
    #[schemars(range(min = 0.0, max = 1.0))]
    pub confidence: f64,
    #[schemars(length(min = 1, max = 32))]
    pub evidence_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisBoundingBox {
    #[schemars(length(min = 1, max = 128), regex(pattern = SAFE_ID_PATTERN))]
    pub reference_id: String,
    pub coordinate_space: AnalysisCoordinateSpace,
    #[schemars(range(min = 0.0, max = 4096.0))]
    pub x: f64,
    #[schemars(range(min = 0.0, max = 4096.0))]
    pub y: f64,
    #[schemars(range(min = 0.0, max = 4096.0))]
    pub width: f64,
    #[schemars(range(min = 0.0, max = 4096.0))]
    pub height: f64,
    #[schemars(length(min = 1, max = 32))]
    pub evidence_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisElement {
    #[schemars(length(min = 1, max = 128), regex(pattern = SAFE_ID_PATTERN))]
    pub element_id: String,
    #[schemars(length(min = 1, max = 128), regex(pattern = SAFE_ID_PATTERN))]
    pub parent_id: Option<String>,
    #[schemars(length(min = 1, max = 128), regex(pattern = SAFE_ID_PATTERN))]
    pub region_id: String,
    pub kind: VisualElementKind,
    pub bounding_box: AnalysisBoundingBox,
    pub layout: LayoutBehaviorEvidence,
    #[schemars(length(max = 16))]
    pub alignment_clues: Vec<AlignmentClue>,
    pub repeated_pattern: Option<RepeatedPattern>,
    #[schemars(range(min = 0.0, max = 1.0))]
    pub confidence: f64,
    #[schemars(length(min = 1, max = 32))]
    pub evidence_ids: Vec<String>,
    #[schemars(length(max = 16))]
    pub component_candidates: Vec<ComponentCandidate>,
    pub text: Option<TextRecognition>,
    pub image: Option<ImageRecognition>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VisualElementKind {
    Container,
    Text,
    Image,
    Background,
    Surface,
    Border,
    Icon,
    StatusIndicator,
    NineSliceCandidate,
    Decoration,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LayoutBehaviorEvidence {
    pub kind: LayoutBehaviorKind,
    #[schemars(length(max = 4))]
    pub anchors: Vec<AnchorEdge>,
    pub flow_axis: Option<Axis>,
    #[schemars(length(max = 2))]
    pub scroll_axes: Vec<Axis>,
    #[schemars(length(min = 1, max = 32))]
    pub evidence_ids: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LayoutBehaviorKind {
    FixedAnchor,
    ContentFlow,
    ProportionalStretch,
    Scrollable,
    AbsoluteDecoration,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnchorEdge {
    Top,
    Right,
    Bottom,
    Left,
    HorizontalCenter,
    VerticalCenter,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AlignmentClue {
    pub axis: Axis,
    pub relation: AlignmentRelation,
    pub target: AlignmentTarget,
    #[schemars(range(min = -4096.0, max = 4096.0))]
    pub offset: f64,
    #[schemars(range(min = 0.0, max = 1.0))]
    pub confidence: f64,
    #[schemars(length(min = 1, max = 32))]
    pub evidence_ids: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AlignmentRelation {
    AlignedEdge,
    Centered,
    EqualSpacing,
    Baseline,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AlignmentTarget {
    Canvas {
        edge: AnchorEdge,
    },
    Region {
        #[schemars(length(min = 1, max = 128), regex(pattern = SAFE_ID_PATTERN))]
        region_id: String,
        edge: AnchorEdge,
    },
    Element {
        #[schemars(length(min = 1, max = 128), regex(pattern = SAFE_ID_PATTERN))]
        element_id: String,
        edge: AnchorEdge,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RepeatedPattern {
    #[schemars(length(min = 1, max = 128), regex(pattern = SAFE_ID_PATTERN))]
    pub pattern_id: String,
    #[schemars(range(max = 511))]
    pub item_index: u32,
    #[schemars(range(min = 2, max = 512))]
    pub observed_count: u32,
    #[schemars(range(min = 0.0, max = 1.0))]
    pub confidence: f64,
    #[schemars(length(min = 1, max = 32))]
    pub evidence_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentCandidate {
    pub kind: ComponentCandidateKind,
    #[schemars(range(min = 0.0, max = 1.0))]
    pub confidence: f64,
    #[schemars(length(min = 1, max = 32))]
    pub evidence_ids: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentCandidateKind {
    Button,
    Label,
    ImageFrame,
    Card,
    List,
    ListItem,
    Dialog,
    HudIndicator,
    Badge,
    Progress,
    ScrollRegion,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TextRecognition {
    #[schemars(length(max = 16))]
    pub original_candidates: Vec<TextCandidate>,
    #[schemars(length(min = 1, max = 1024))]
    pub human_provided_text: Option<String>,
    #[schemars(length(min = 1, max = 128), regex(pattern = SAFE_LABEL_PATTERN))]
    pub human_input_id: Option<String>,
    pub adopted: TextAdoption,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TextCandidate {
    pub source: TextCandidateSource,
    #[schemars(length(min = 1, max = 1024))]
    pub raw_text: String,
    #[schemars(range(min = 0.0, max = 1.0))]
    pub confidence: f64,
    #[schemars(length(min = 1, max = 32))]
    pub evidence_ids: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TextCandidateSource {
    Ocr,
    Model,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "strategy", rename_all = "snake_case", deny_unknown_fields)]
pub enum TextAdoption {
    HumanProvided,
    Candidate {
        #[schemars(range(max = 15))]
        candidate_index: usize,
    },
    Unresolved,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ImageRecognition {
    pub role: ImageRole,
    #[schemars(length(max = 8))]
    pub description_candidates: Vec<VisualDescriptionCandidate>,
    pub likely_nine_slice: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageRole {
    Background,
    Content,
    Icon,
    Decoration,
    NineSliceCandidate,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VisualDescriptionCandidate {
    #[schemars(length(min = 1, max = 512))]
    pub raw_description: String,
    #[schemars(range(min = 0.0, max = 1.0))]
    pub confidence: f64,
    #[schemars(length(min = 1, max = 32))]
    pub evidence_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisEvidence {
    #[schemars(length(min = 1, max = 128), regex(pattern = SAFE_ID_PATTERN))]
    pub evidence_id: String,
    pub source: EvidenceSource,
    #[schemars(length(min = 1, max = 512))]
    pub detail: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EvidenceSource {
    ReferenceRegion {
        #[schemars(length(min = 1, max = 128), regex(pattern = SAFE_ID_PATTERN))]
        reference_id: String,
        #[schemars(length(min = 1, max = 128), regex(pattern = SAFE_ID_PATTERN))]
        region_id: Option<String>,
    },
    ProviderResponse {
        #[schemars(length(min = 1, max = 64), regex(pattern = SAFE_LABEL_PATTERN))]
        provider_id: String,
        #[schemars(length(min = 1, max = 128), regex(pattern = SAFE_LABEL_PATTERN))]
        server_request_id: String,
    },
    HumanInput {
        #[schemars(length(min = 1, max = 128), regex(pattern = SAFE_LABEL_PATTERN))]
        input_id: String,
    },
}

#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisUncertainty {
    #[schemars(length(min = 1, max = 128), regex(pattern = SAFE_ID_PATTERN))]
    pub uncertainty_id: String,
    pub kind: UncertaintyKind,
    pub subject: UncertaintySubject,
    #[schemars(length(min = 1, max = 512))]
    pub impact: String,
    #[schemars(length(min = 1, max = 512))]
    pub follow_up_question: String,
    #[schemars(length(min = 1, max = 32))]
    pub evidence_ids: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UncertaintyKind {
    Occlusion,
    Blur,
    Cropping,
    UnknownFont,
    HiddenInteraction,
    TextConflict,
    AmbiguousLayout,
    LowConfidence,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UncertaintySubject {
    #[schemars(length(min = 1, max = 128), regex(pattern = SAFE_ID_PATTERN))]
    pub element_id: Option<String>,
    #[schemars(length(min = 1, max = 128), regex(pattern = SAFE_ID_PATTERN))]
    pub reference_id: Option<String>,
    #[schemars(length(min = 1, max = 128), regex(pattern = SAFE_ID_PATTERN))]
    pub region_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisDiagnosticSeverity {
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisDiagnostic {
    pub severity: AnalysisDiagnosticSeverity,
    pub code: String,
    pub path: String,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisValidationReport {
    pub valid: bool,
    pub diagnostics: Vec<AnalysisDiagnostic>,
}

impl AnalysisValidationReport {
    fn from_diagnostics(diagnostics: Vec<AnalysisDiagnostic>) -> Self {
        Self {
            valid: diagnostics.is_empty(),
            diagnostics,
        }
    }

    fn one(code: &str, path: &str, message: impl Into<String>) -> Self {
        Self::from_diagnostics(vec![diagnostic(code, path, message)])
    }

    pub fn has_code(&self, code: &str) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == code)
    }
}

static ANALYSIS_SCHEMA: LazyLock<Value> = LazyLock::new(|| {
    serde_json::to_value(schemars::schema_for!(UiReferenceAnalysis))
        .expect("analysis JSON Schema is serializable")
});

static ANALYSIS_SCHEMA_VALIDATOR: LazyLock<jsonschema::Validator> = LazyLock::new(|| {
    jsonschema::validator_for(&ANALYSIS_SCHEMA).expect("generated analysis JSON Schema is valid")
});

pub fn analysis_output_contract() -> StructuredOutputContract {
    StructuredOutputContract::new(ANALYSIS_SCHEMA_ID, ANALYSIS_SCHEMA_VERSION)
        .expect("analysis contract constants are valid")
}

pub fn analysis_schema() -> Value {
    ANALYSIS_SCHEMA.clone()
}

/// Parses untrusted structured output without granting provider or human-input provenance trust.
/// Use `parse_provider_execution_analysis` at the provider integration boundary.
pub fn parse_provider_analysis(
    output: &StructuredProviderOutput,
) -> Result<UiReferenceAnalysis, AnalysisValidationReport> {
    if output.operation != ProviderOperation::VisualAnalysis {
        return Err(AnalysisValidationReport::one(
            "ANALYSIS_PROVIDER_OPERATION_MISMATCH",
            "$.operation",
            "provider output must use the visual_analysis operation",
        ));
    }
    if output.schema != analysis_output_contract() {
        return Err(AnalysisValidationReport::one(
            "ANALYSIS_PROVIDER_SCHEMA_MISMATCH",
            "$.schema",
            "provider output schema ID/version does not match UiReferenceAnalysis",
        ));
    }
    parse_analysis_value(&output.value)
}

/// Applies a context that can only be constructed from an executed request and trusted task input.
pub fn parse_provider_analysis_with_trusted_context(
    output: &StructuredProviderOutput,
    trusted: &TrustedAnalysisContext,
) -> Result<UiReferenceAnalysis, AnalysisValidationReport> {
    let analysis = parse_provider_analysis(output)?;
    let report = analysis.validate_trusted_context(trusted);
    if report.valid {
        Ok(analysis)
    } else {
        Err(report)
    }
}

/// Safe integration entry point for Stage 2 execution, Stage 1 human input, and Stage 3 evidence.
pub fn parse_provider_execution_analysis(
    execution: &ProviderExecution,
    task: &GenerationTask,
    trusted_preprocess: &[TrustedPreprocessEvidence],
) -> Result<UiReferenceAnalysis, AnalysisValidationReport> {
    let trusted =
        TrustedAnalysisContext::from_execution_and_task(execution, task, trusted_preprocess)?;
    parse_provider_analysis_with_trusted_context(&execution.response.output, &trusted)
}

pub fn validate_analysis_json(bytes: &[u8]) -> AnalysisValidationReport {
    match parse_analysis_json(bytes) {
        Ok(_) => AnalysisValidationReport::from_diagnostics(Vec::new()),
        Err(report) => report,
    }
}

pub fn parse_analysis_json(bytes: &[u8]) -> Result<UiReferenceAnalysis, AnalysisValidationReport> {
    if bytes.len() > MAX_ANALYSIS_JSON_BYTES {
        return Err(AnalysisValidationReport::one(
            "ANALYSIS_JSON_BYTES_EXCEEDED",
            "$",
            format!(
                "analysis JSON has {} bytes; maximum is {MAX_ANALYSIS_JSON_BYTES}",
                bytes.len()
            ),
        ));
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|error| {
        AnalysisValidationReport::one(
            "ANALYSIS_JSON_MALFORMED",
            "$",
            format!("analysis JSON cannot be parsed: {error}"),
        )
    })?;
    parse_analysis_value(&value)
}

pub fn parse_analysis_value(
    value: &Value,
) -> Result<UiReferenceAnalysis, AnalysisValidationReport> {
    let budget_report = validate_json_budget(value);
    if !budget_report.valid {
        return Err(budget_report);
    }

    let schema_diagnostics = ANALYSIS_SCHEMA_VALIDATOR
        .iter_errors(value)
        .take(MAX_DIAGNOSTICS)
        .map(|error| {
            let pointer = error.instance_path().to_string();
            diagnostic(
                "ANALYSIS_JSON_SCHEMA_INVALID",
                &json_pointer_path(&pointer),
                error.to_string(),
            )
        })
        .collect::<Vec<_>>();
    if !schema_diagnostics.is_empty() {
        return Err(AnalysisValidationReport::from_diagnostics(
            schema_diagnostics,
        ));
    }

    let analysis: UiReferenceAnalysis = serde_json::from_value(value.clone()).map_err(|error| {
        AnalysisValidationReport::one(
            "ANALYSIS_SCHEMA_MODEL_DRIFT",
            "$",
            format!("schema accepted a value the Rust model rejected: {error}"),
        )
    })?;
    let semantic_report = analysis.validate_semantics();
    if semantic_report.valid {
        Ok(analysis)
    } else {
        Err(semantic_report)
    }
}

impl UiReferenceAnalysis {
    pub fn validate_semantics(&self) -> AnalysisValidationReport {
        let mut diagnostics = Vec::new();
        if self.schema_id != ANALYSIS_SCHEMA_ID || self.schema_version != ANALYSIS_SCHEMA_VERSION {
            push_diagnostic(
                &mut diagnostics,
                "ANALYSIS_SCHEMA_IDENTITY_MISMATCH",
                "$.schema_id",
                "analysis schema identity does not match the tool's supported contract",
            );
        }
        let references = collect_unique(
            self.references
                .iter()
                .enumerate()
                .map(|(index, reference)| {
                    (
                        reference.reference_id.as_str(),
                        format!("$.references[{index}].reference_id"),
                    )
                }),
            "ANALYSIS_REFERENCE_ID_DUPLICATED",
            &mut diagnostics,
        );
        let regions = collect_unique(
            self.regions.iter().enumerate().map(|(index, region)| {
                (
                    region.region_id.as_str(),
                    format!("$.regions[{index}].region_id"),
                )
            }),
            "ANALYSIS_REGION_ID_DUPLICATED",
            &mut diagnostics,
        );
        let elements = collect_unique(
            self.elements.iter().enumerate().map(|(index, element)| {
                (
                    element.element_id.as_str(),
                    format!("$.elements[{index}].element_id"),
                )
            }),
            "ANALYSIS_ELEMENT_ID_DUPLICATED",
            &mut diagnostics,
        );
        let evidence = collect_unique(
            self.evidence.iter().enumerate().map(|(index, item)| {
                (
                    item.evidence_id.as_str(),
                    format!("$.evidence[{index}].evidence_id"),
                )
            }),
            "ANALYSIS_EVIDENCE_ID_DUPLICATED",
            &mut diagnostics,
        );
        collect_unique(
            self.uncertainties.iter().enumerate().map(|(index, item)| {
                (
                    item.uncertainty_id.as_str(),
                    format!("$.uncertainties[{index}].uncertainty_id"),
                )
            }),
            "ANALYSIS_UNCERTAINTY_ID_DUPLICATED",
            &mut diagnostics,
        );

        for (index, region) in self.regions.iter().enumerate() {
            let path = format!("$.regions[{index}]");
            let Some(reference) = references.get(region.reference_id.as_str()).copied() else {
                push_diagnostic(
                    &mut diagnostics,
                    "ANALYSIS_REFERENCE_UNKNOWN",
                    &format!("{path}.reference_id"),
                    "region references an unknown input reference",
                );
                continue;
            };
            validate_bounding_box(
                &region.bounding_box,
                &self.references[reference],
                &format!("{path}.bounding_box"),
                &evidence,
                &mut diagnostics,
            );
            if region.bounding_box.reference_id != region.reference_id {
                push_diagnostic(
                    &mut diagnostics,
                    "ANALYSIS_REGION_REFERENCE_MISMATCH",
                    &format!("{path}.bounding_box.reference_id"),
                    "region bounding box must use the region reference_id",
                );
            }
            validate_evidence_ids(
                &region.evidence_ids,
                &format!("{path}.evidence_ids"),
                &evidence,
                &mut diagnostics,
            );
        }

        validate_evidence_sources(self, &references, &regions, &mut diagnostics);
        validate_element_graph(self, &elements, &mut diagnostics);

        let mut repeated_patterns: BTreeMap<&str, (u32, BTreeSet<u32>)> = BTreeMap::new();
        for (index, element) in self.elements.iter().enumerate() {
            let path = format!("$.elements[{index}]");
            let Some(region_index) = regions.get(element.region_id.as_str()).copied() else {
                push_diagnostic(
                    &mut diagnostics,
                    "ANALYSIS_REGION_UNKNOWN",
                    &format!("{path}.region_id"),
                    "element references an unknown region",
                );
                continue;
            };
            let region = &self.regions[region_index];
            if let Some(reference) = references
                .get(element.bounding_box.reference_id.as_str())
                .copied()
            {
                validate_bounding_box(
                    &element.bounding_box,
                    &self.references[reference],
                    &format!("{path}.bounding_box"),
                    &evidence,
                    &mut diagnostics,
                );
                if element.bounding_box.reference_id != region.reference_id {
                    push_diagnostic(
                        &mut diagnostics,
                        "ANALYSIS_ELEMENT_REGION_REFERENCE_MISMATCH",
                        &format!("{path}.bounding_box.reference_id"),
                        "element and containing region must use the same reference",
                    );
                }
            } else {
                push_diagnostic(
                    &mut diagnostics,
                    "ANALYSIS_REFERENCE_UNKNOWN",
                    &format!("{path}.bounding_box.reference_id"),
                    "element bounding box references an unknown input reference",
                );
            }
            validate_evidence_ids(
                &element.evidence_ids,
                &format!("{path}.evidence_ids"),
                &evidence,
                &mut diagnostics,
            );
            validate_layout(element, &path, &evidence, &mut diagnostics);
            validate_alignments(
                element,
                &path,
                &elements,
                &regions,
                &evidence,
                &mut diagnostics,
            );
            validate_component_candidates(element, &path, &evidence, &mut diagnostics);
            validate_text(element, &path, self, &evidence, &mut diagnostics);
            validate_image(element, &path, &evidence, &mut diagnostics);
            if let Some(pattern) = &element.repeated_pattern {
                validate_evidence_ids(
                    &pattern.evidence_ids,
                    &format!("{path}.repeated_pattern.evidence_ids"),
                    &evidence,
                    &mut diagnostics,
                );
                if pattern.item_index >= pattern.observed_count {
                    push_diagnostic(
                        &mut diagnostics,
                        "ANALYSIS_REPEAT_INDEX_OUT_OF_RANGE",
                        &format!("{path}.repeated_pattern.item_index"),
                        "repeat item_index must be smaller than observed_count",
                    );
                }
                let entry = repeated_patterns
                    .entry(&pattern.pattern_id)
                    .or_insert((pattern.observed_count, BTreeSet::new()));
                if entry.0 != pattern.observed_count {
                    push_diagnostic(
                        &mut diagnostics,
                        "ANALYSIS_REPEAT_COUNT_INCONSISTENT",
                        &format!("{path}.repeated_pattern.observed_count"),
                        "all members of a repeated pattern must use the same observed_count",
                    );
                }
                if !entry.1.insert(pattern.item_index) {
                    push_diagnostic(
                        &mut diagnostics,
                        "ANALYSIS_REPEAT_INDEX_DUPLICATED",
                        &format!("{path}.repeated_pattern.item_index"),
                        "repeat item_index must be unique within its pattern",
                    );
                }
            }
        }

        validate_uncertainties(
            self,
            &elements,
            &regions,
            &references,
            &evidence,
            &mut diagnostics,
        );
        AnalysisValidationReport::from_diagnostics(diagnostics)
    }

    pub fn validate_preprocess_evidence(
        &self,
        trusted: &[TrustedPreprocessEvidence],
    ) -> AnalysisValidationReport {
        let mut diagnostics = Vec::new();
        let trusted_by_id = collect_unique(
            trusted.iter().enumerate().map(|(index, item)| {
                (
                    item.reference_id.as_str(),
                    format!("$.trusted_preprocess[{index}].reference_id"),
                )
            }),
            "ANALYSIS_TRUSTED_REFERENCE_DUPLICATED",
            &mut diagnostics,
        );
        let analysis_ids = self
            .references
            .iter()
            .map(|reference| reference.reference_id.as_str())
            .collect::<BTreeSet<_>>();
        for (index, reference) in self.references.iter().enumerate() {
            let path = format!("$.references[{index}]");
            let Some(trusted_index) = trusted_by_id.get(reference.reference_id.as_str()).copied()
            else {
                push_diagnostic(
                    &mut diagnostics,
                    "ANALYSIS_PREPROCESS_EVIDENCE_MISSING",
                    &path,
                    "analysis reference has no trusted Stage 3 preprocess evidence",
                );
                continue;
            };
            let expected = &trusted[trusted_index];
            let matches = reference.source_sha256 == expected.source_sha256
                && reference.preprocess_cache_key == expected.preprocess_cache_key
                && reference.preprocess_protocol_version == expected.preprocess_protocol_version
                && reference.preprocess_implementation_version
                    == expected.preprocess_implementation_version
                && reference.preprocess_manifest_sha256 == expected.preprocess_manifest_sha256
                && reference.standard_preview_sha256 == expected.standard_preview_sha256
                && reference.coordinate_space == expected.coordinate_space
                && reference.coordinate_convention == expected.coordinate_convention
                && reference.width == expected.width
                && reference.height == expected.height;
            if !matches {
                push_diagnostic(
                    &mut diagnostics,
                    "ANALYSIS_PREPROCESS_EVIDENCE_MISMATCH",
                    &path,
                    "analysis reference hash, version, coordinate convention, or preview size differs from trusted Stage 3 evidence",
                );
            }
        }
        for (index, item) in trusted.iter().enumerate() {
            if !analysis_ids.contains(item.reference_id.as_str()) {
                push_diagnostic(
                    &mut diagnostics,
                    "ANALYSIS_PREPROCESS_REFERENCE_UNUSED",
                    &format!("$.trusted_preprocess[{index}].reference_id"),
                    "trusted Stage 3 evidence does not have a corresponding analysis reference",
                );
            }
        }
        AnalysisValidationReport::from_diagnostics(diagnostics)
    }

    pub fn validate_trusted_context(
        &self,
        trusted: &TrustedAnalysisContext,
    ) -> AnalysisValidationReport {
        let mut diagnostics = self
            .validate_preprocess_evidence(&trusted.preprocess)
            .diagnostics;
        let provider = &trusted.provider;
        if self.run_id != provider.run_id {
            push_diagnostic(
                &mut diagnostics,
                "ANALYSIS_RUN_ID_UNTRUSTED",
                "$.run_id",
                "analysis run_id differs from the executed provider request",
            );
        }
        if self.provider.provider_id != provider.provider_id {
            push_diagnostic(
                &mut diagnostics,
                "ANALYSIS_PROVIDER_ID_UNTRUSTED",
                "$.provider.provider_id",
                "analysis provider_id differs from the executed provider",
            );
        }
        if self.provider.prompt_version != provider.prompt_version {
            push_diagnostic(
                &mut diagnostics,
                "ANALYSIS_PROMPT_VERSION_UNTRUSTED",
                "$.provider.prompt_version",
                "analysis prompt_version differs from the executed request",
            );
        }
        match provider.server_request_id.as_deref() {
            Some(request_id) if self.provider.server_request_id == request_id => {}
            Some(_) => push_diagnostic(
                &mut diagnostics,
                "ANALYSIS_PROVIDER_REQUEST_ID_UNTRUSTED",
                "$.provider.server_request_id",
                "analysis server_request_id differs from the provider response",
            ),
            None => push_diagnostic(
                &mut diagnostics,
                "ANALYSIS_TRUSTED_PROVIDER_REQUEST_ID_MISSING",
                "$.trusted_provider.server_request_id",
                "provider response did not supply the request ID required for analysis provenance",
            ),
        }

        let trusted_humans = collect_unique(
            trusted
                .human_inputs
                .iter()
                .enumerate()
                .map(|(index, input)| {
                    (
                        input.input_id.as_str(),
                        format!("$.trusted_human_inputs[{index}].input_id"),
                    )
                }),
            "ANALYSIS_TRUSTED_HUMAN_INPUT_DUPLICATED",
            &mut diagnostics,
        );
        for (index, evidence) in self.evidence.iter().enumerate() {
            let path = format!("$.evidence[{index}].source");
            match &evidence.source {
                EvidenceSource::ProviderResponse {
                    provider_id,
                    server_request_id,
                } => {
                    let matches = provider_id == &provider.provider_id
                        && provider.server_request_id.as_deref() == Some(server_request_id);
                    if !matches {
                        push_diagnostic(
                            &mut diagnostics,
                            "ANALYSIS_PROVIDER_EVIDENCE_UNTRUSTED",
                            &path,
                            "provider evidence differs from the executed provider response",
                        );
                    }
                }
                EvidenceSource::HumanInput { input_id }
                    if !trusted_humans.contains_key(input_id.as_str()) =>
                {
                    push_diagnostic(
                        &mut diagnostics,
                        "ANALYSIS_HUMAN_EVIDENCE_UNTRUSTED",
                        &format!("{path}.input_id"),
                        "human evidence input_id does not exist in trusted task input",
                    );
                }
                _ => {}
            }
        }

        for (index, element) in self.elements.iter().enumerate() {
            let Some(text) = &element.text else {
                continue;
            };
            let (Some(human_text), Some(input_id)) =
                (&text.human_provided_text, &text.human_input_id)
            else {
                continue;
            };
            let path = format!("$.elements[{index}].text");
            match trusted_humans.get(input_id.as_str()).copied() {
                Some(trusted_index) if trusted.human_inputs[trusted_index].text == *human_text => {}
                Some(_) => push_diagnostic(
                    &mut diagnostics,
                    "ANALYSIS_HUMAN_TEXT_MISMATCH",
                    &format!("{path}.human_provided_text"),
                    "human_provided_text differs from trusted task input",
                ),
                None => push_diagnostic(
                    &mut diagnostics,
                    "ANALYSIS_HUMAN_TEXT_INPUT_UNTRUSTED",
                    &format!("{path}.human_input_id"),
                    "human_input_id does not exist in trusted task input",
                ),
            }
        }
        AnalysisValidationReport::from_diagnostics(diagnostics)
    }
}

fn validate_json_budget(value: &Value) -> AnalysisValidationReport {
    let mut stack = vec![(value, 0usize)];
    let mut nodes = 0usize;
    let mut string_bytes = 0usize;
    while let Some((current, depth)) = stack.pop() {
        nodes += 1;
        if nodes > MAX_JSON_NODES {
            return AnalysisValidationReport::one(
                "ANALYSIS_JSON_NODE_BUDGET_EXCEEDED",
                "$",
                format!("analysis JSON exceeds {MAX_JSON_NODES} structural nodes"),
            );
        }
        if depth > MAX_JSON_DEPTH {
            return AnalysisValidationReport::one(
                "ANALYSIS_JSON_DEPTH_EXCEEDED",
                "$",
                format!("analysis JSON exceeds structural depth {MAX_JSON_DEPTH}"),
            );
        }
        match current {
            Value::String(text) => {
                if text.len() > MAX_JSON_STRING_BYTES {
                    return AnalysisValidationReport::one(
                        "ANALYSIS_JSON_STRING_BUDGET_EXCEEDED",
                        "$",
                        format!(
                            "analysis JSON contains a string longer than {MAX_JSON_STRING_BYTES} bytes"
                        ),
                    );
                }
                string_bytes = string_bytes.saturating_add(text.len());
                if string_bytes > MAX_JSON_TOTAL_STRING_BYTES {
                    return AnalysisValidationReport::one(
                        "ANALYSIS_JSON_TOTAL_STRING_BUDGET_EXCEEDED",
                        "$",
                        format!(
                            "analysis JSON string content exceeds {MAX_JSON_TOTAL_STRING_BYTES} bytes"
                        ),
                    );
                }
            }
            Value::Array(items) => {
                if items.len() > MAX_JSON_CONTAINER_ITEMS {
                    return AnalysisValidationReport::one(
                        "ANALYSIS_JSON_CONTAINER_BUDGET_EXCEEDED",
                        "$",
                        format!(
                            "analysis JSON array contains more than {MAX_JSON_CONTAINER_ITEMS} items"
                        ),
                    );
                }
                stack.extend(items.iter().map(|item| (item, depth + 1)));
            }
            Value::Object(fields) => {
                if fields.len() > MAX_JSON_CONTAINER_ITEMS {
                    return AnalysisValidationReport::one(
                        "ANALYSIS_JSON_CONTAINER_BUDGET_EXCEEDED",
                        "$",
                        format!(
                            "analysis JSON object contains more than {MAX_JSON_CONTAINER_ITEMS} fields"
                        ),
                    );
                }
                for (key, child) in fields {
                    string_bytes = string_bytes.saturating_add(key.len());
                    if key.len() > MAX_JSON_STRING_BYTES
                        || string_bytes > MAX_JSON_TOTAL_STRING_BYTES
                    {
                        return AnalysisValidationReport::one(
                            "ANALYSIS_JSON_STRING_BUDGET_EXCEEDED",
                            "$",
                            "analysis JSON object keys exceed the string budget",
                        );
                    }
                    stack.push((child, depth + 1));
                }
            }
            _ => {}
        }
    }
    AnalysisValidationReport::from_diagnostics(Vec::new())
}

fn collect_unique<'a>(
    entries: impl Iterator<Item = (&'a str, String)>,
    duplicate_code: &str,
    diagnostics: &mut Vec<AnalysisDiagnostic>,
) -> BTreeMap<&'a str, usize> {
    let mut collected = BTreeMap::new();
    for (index, (id, path)) in entries.enumerate() {
        if collected.insert(id, index).is_some() {
            push_diagnostic(
                diagnostics,
                duplicate_code,
                &path,
                format!("identifier `{id}` is duplicated"),
            );
        }
    }
    collected
}

fn validate_bounding_box(
    bounding_box: &AnalysisBoundingBox,
    reference: &AnalysisReference,
    path: &str,
    evidence: &BTreeMap<&str, usize>,
    diagnostics: &mut Vec<AnalysisDiagnostic>,
) {
    let finite = bounding_box.x.is_finite()
        && bounding_box.y.is_finite()
        && bounding_box.width.is_finite()
        && bounding_box.height.is_finite();
    if !finite || bounding_box.width <= 0.0 || bounding_box.height <= 0.0 {
        push_diagnostic(
            diagnostics,
            "ANALYSIS_BOUNDING_BOX_INVALID",
            path,
            "bounding box coordinates must be finite and dimensions must be positive",
        );
    }
    validate_bounding_box_bounds(bounding_box, reference, path, diagnostics);
    validate_evidence_ids(
        &bounding_box.evidence_ids,
        &format!("{path}.evidence_ids"),
        evidence,
        diagnostics,
    );
}

fn validate_bounding_box_bounds(
    bounding_box: &AnalysisBoundingBox,
    reference: &AnalysisReference,
    path: &str,
    diagnostics: &mut Vec<AnalysisDiagnostic>,
) {
    let right = bounding_box.x + bounding_box.width;
    let bottom = bounding_box.y + bounding_box.height;
    if bounding_box.coordinate_space != reference.coordinate_space
        || bounding_box.x < 0.0
        || bounding_box.y < 0.0
        || !right.is_finite()
        || !bottom.is_finite()
        || right > f64::from(reference.width) + 1e-7
        || bottom > f64::from(reference.height) + 1e-7
    {
        push_diagnostic(
            diagnostics,
            "ANALYSIS_BOUNDING_BOX_OUT_OF_BOUNDS",
            path,
            "bounding box must stay inside the referenced standard preview coordinate space",
        );
    }
}

fn validate_evidence_sources(
    analysis: &UiReferenceAnalysis,
    references: &BTreeMap<&str, usize>,
    regions: &BTreeMap<&str, usize>,
    diagnostics: &mut Vec<AnalysisDiagnostic>,
) {
    for (index, evidence) in analysis.evidence.iter().enumerate() {
        let path = format!("$.evidence[{index}].source");
        match &evidence.source {
            EvidenceSource::ReferenceRegion {
                reference_id,
                region_id,
            } => {
                if !references.contains_key(reference_id.as_str()) {
                    push_diagnostic(
                        diagnostics,
                        "ANALYSIS_REFERENCE_UNKNOWN",
                        &format!("{path}.reference_id"),
                        "evidence references an unknown input reference",
                    );
                }
                if let Some(region_id) = region_id {
                    if let Some(region_index) = regions.get(region_id.as_str()).copied() {
                        if analysis.regions[region_index].reference_id != *reference_id {
                            push_diagnostic(
                                diagnostics,
                                "ANALYSIS_EVIDENCE_REGION_REFERENCE_MISMATCH",
                                &format!("{path}.region_id"),
                                "evidence region does not belong to its reference_id",
                            );
                        }
                    } else {
                        push_diagnostic(
                            diagnostics,
                            "ANALYSIS_REGION_UNKNOWN",
                            &format!("{path}.region_id"),
                            "evidence references an unknown region",
                        );
                    }
                }
            }
            EvidenceSource::ProviderResponse {
                provider_id,
                server_request_id,
            } => {
                if provider_id != &analysis.provider.provider_id
                    || server_request_id != &analysis.provider.server_request_id
                {
                    push_diagnostic(
                        diagnostics,
                        "ANALYSIS_PROVIDER_EVIDENCE_MISMATCH",
                        &path,
                        "provider evidence must identify the analysis provider request",
                    );
                }
            }
            EvidenceSource::HumanInput { .. } => {}
        }
    }
}

fn validate_element_graph(
    analysis: &UiReferenceAnalysis,
    elements: &BTreeMap<&str, usize>,
    diagnostics: &mut Vec<AnalysisDiagnostic>,
) {
    let roots = analysis
        .elements
        .iter()
        .enumerate()
        .filter(|(_, element)| element.parent_id.is_none())
        .collect::<Vec<_>>();
    if roots.len() != 1 {
        push_diagnostic(
            diagnostics,
            "ANALYSIS_GRAPH_ROOT_COUNT_INVALID",
            "$.elements",
            "element graph must contain exactly one parentless root",
        );
    }
    if roots
        .first()
        .is_none_or(|(_, element)| element.element_id != analysis.root_element_id)
    {
        push_diagnostic(
            diagnostics,
            "ANALYSIS_GRAPH_ROOT_MISMATCH",
            "$.root_element_id",
            "root_element_id must name the only parentless element",
        );
    }
    for (index, element) in analysis.elements.iter().enumerate() {
        if let Some(parent_id) = &element.parent_id
            && !elements.contains_key(parent_id.as_str())
        {
            push_diagnostic(
                diagnostics,
                "ANALYSIS_GRAPH_ORPHAN",
                &format!("$.elements[{index}].parent_id"),
                "parent_id references an unknown element",
            );
        }
        let mut seen = BTreeSet::new();
        let mut current = Some(element.element_id.as_str());
        let mut depth = 0usize;
        while let Some(element_id) = current {
            if !seen.insert(element_id) {
                push_diagnostic(
                    diagnostics,
                    "ANALYSIS_GRAPH_CYCLE",
                    &format!("$.elements[{index}].parent_id"),
                    "element parent graph contains a cycle",
                );
                break;
            }
            if depth > MAX_ANALYSIS_DEPTH {
                push_diagnostic(
                    diagnostics,
                    "ANALYSIS_GRAPH_DEPTH_EXCEEDED",
                    &format!("$.elements[{index}].parent_id"),
                    format!("element depth exceeds {MAX_ANALYSIS_DEPTH}"),
                );
                break;
            }
            current = elements
                .get(element_id)
                .and_then(|parent_index| analysis.elements[*parent_index].parent_id.as_deref());
            depth += 1;
        }
    }
}

fn validate_layout(
    element: &AnalysisElement,
    path: &str,
    evidence: &BTreeMap<&str, usize>,
    diagnostics: &mut Vec<AnalysisDiagnostic>,
) {
    validate_evidence_ids(
        &element.layout.evidence_ids,
        &format!("{path}.layout.evidence_ids"),
        evidence,
        diagnostics,
    );
    let invalid = match element.layout.kind {
        LayoutBehaviorKind::FixedAnchor => element.layout.anchors.is_empty(),
        LayoutBehaviorKind::ContentFlow => element.layout.flow_axis.is_none(),
        LayoutBehaviorKind::ProportionalStretch => false,
        LayoutBehaviorKind::Scrollable => element.layout.scroll_axes.is_empty(),
        LayoutBehaviorKind::AbsoluteDecoration => element.kind != VisualElementKind::Decoration,
    };
    if invalid {
        push_diagnostic(
            diagnostics,
            "ANALYSIS_LAYOUT_EVIDENCE_INCOMPLETE",
            &format!("{path}.layout"),
            "layout behavior is missing its required anchors/axis or conflicts with element kind",
        );
    }
}

fn validate_alignments(
    element: &AnalysisElement,
    path: &str,
    elements: &BTreeMap<&str, usize>,
    regions: &BTreeMap<&str, usize>,
    evidence: &BTreeMap<&str, usize>,
    diagnostics: &mut Vec<AnalysisDiagnostic>,
) {
    for (index, clue) in element.alignment_clues.iter().enumerate() {
        let clue_path = format!("{path}.alignment_clues[{index}]");
        validate_evidence_ids(
            &clue.evidence_ids,
            &format!("{clue_path}.evidence_ids"),
            evidence,
            diagnostics,
        );
        match &clue.target {
            AlignmentTarget::Element { element_id, .. }
                if !elements.contains_key(element_id.as_str()) =>
            {
                push_diagnostic(
                    diagnostics,
                    "ANALYSIS_ALIGNMENT_TARGET_UNKNOWN",
                    &format!("{clue_path}.target.element_id"),
                    "alignment clue references an unknown element",
                );
            }
            AlignmentTarget::Region { region_id, .. }
                if !regions.contains_key(region_id.as_str()) =>
            {
                push_diagnostic(
                    diagnostics,
                    "ANALYSIS_ALIGNMENT_TARGET_UNKNOWN",
                    &format!("{clue_path}.target.region_id"),
                    "alignment clue references an unknown region",
                );
            }
            _ => {}
        }
    }
}

fn validate_component_candidates(
    element: &AnalysisElement,
    path: &str,
    evidence: &BTreeMap<&str, usize>,
    diagnostics: &mut Vec<AnalysisDiagnostic>,
) {
    for (index, candidate) in element.component_candidates.iter().enumerate() {
        validate_evidence_ids(
            &candidate.evidence_ids,
            &format!("{path}.component_candidates[{index}].evidence_ids"),
            evidence,
            diagnostics,
        );
    }
}

fn validate_text(
    element: &AnalysisElement,
    path: &str,
    analysis: &UiReferenceAnalysis,
    evidence: &BTreeMap<&str, usize>,
    diagnostics: &mut Vec<AnalysisDiagnostic>,
) {
    let Some(text) = &element.text else {
        if element.kind == VisualElementKind::Text {
            push_diagnostic(
                diagnostics,
                "ANALYSIS_TEXT_DATA_MISSING",
                &format!("{path}.text"),
                "text elements must preserve recognition candidates and adoption strategy",
            );
        }
        return;
    };
    for (index, candidate) in text.original_candidates.iter().enumerate() {
        if candidate.raw_text.len() > MAX_TEXT_LENGTH {
            push_diagnostic(
                diagnostics,
                "ANALYSIS_TEXT_BYTES_EXCEEDED",
                &format!("{path}.text.original_candidates[{index}].raw_text"),
                format!("recognized text exceeds {MAX_TEXT_LENGTH} UTF-8 bytes"),
            );
        }
        validate_evidence_ids(
            &candidate.evidence_ids,
            &format!("{path}.text.original_candidates[{index}].evidence_ids"),
            evidence,
            diagnostics,
        );
    }
    if text
        .human_provided_text
        .as_ref()
        .is_some_and(|human| human.len() > MAX_TEXT_LENGTH)
    {
        push_diagnostic(
            diagnostics,
            "ANALYSIS_TEXT_BYTES_EXCEEDED",
            &format!("{path}.text.human_provided_text"),
            format!("human-provided text exceeds {MAX_TEXT_LENGTH} UTF-8 bytes"),
        );
    }
    match (&text.human_provided_text, &text.human_input_id) {
        (Some(_), Some(input_id)) => {
            let linked = analysis.evidence.iter().any(|evidence_item| {
                element.evidence_ids.contains(&evidence_item.evidence_id)
                    && matches!(
                        &evidence_item.source,
                        EvidenceSource::HumanInput {
                            input_id: evidence_input_id
                        } if evidence_input_id == input_id
                    )
            });
            if !linked {
                push_diagnostic(
                    diagnostics,
                    "ANALYSIS_HUMAN_TEXT_EVIDENCE_MISSING",
                    &format!("{path}.text.human_input_id"),
                    "human_input_id must be linked through the element evidence_ids",
                );
            }
        }
        (Some(_), None) => push_diagnostic(
            diagnostics,
            "ANALYSIS_HUMAN_TEXT_INPUT_ID_MISSING",
            &format!("{path}.text.human_input_id"),
            "human-provided text requires an explicit trusted input binding ID",
        ),
        (None, Some(_)) => push_diagnostic(
            diagnostics,
            "ANALYSIS_HUMAN_TEXT_BINDING_ORPHANED",
            &format!("{path}.text.human_input_id"),
            "human_input_id cannot be present without human_provided_text",
        ),
        (None, None) => {}
    }
    match (&text.human_provided_text, &text.adopted) {
        (Some(_), TextAdoption::HumanProvided) => {}
        (Some(_), _) => push_diagnostic(
            diagnostics,
            "ANALYSIS_HUMAN_TEXT_NOT_AUTHORITATIVE",
            &format!("{path}.text.adopted"),
            "human-provided text must remain the adopted authority",
        ),
        (None, TextAdoption::HumanProvided) => push_diagnostic(
            diagnostics,
            "ANALYSIS_HUMAN_TEXT_MISSING",
            &format!("{path}.text.adopted"),
            "human_provided adoption requires human_provided_text",
        ),
        (None, TextAdoption::Candidate { candidate_index })
            if *candidate_index >= text.original_candidates.len() =>
        {
            push_diagnostic(
                diagnostics,
                "ANALYSIS_TEXT_CANDIDATE_UNKNOWN",
                &format!("{path}.text.adopted.candidate_index"),
                "adopted candidate_index is outside original_candidates",
            );
        }
        _ => {}
    }
    if let Some(human) = &text.human_provided_text
        && text
            .original_candidates
            .iter()
            .any(|candidate| candidate.raw_text != *human)
        && !analysis.uncertainties.iter().any(|uncertainty| {
            uncertainty.kind == UncertaintyKind::TextConflict
                && uncertainty.subject.element_id.as_deref() == Some(&element.element_id)
        })
    {
        push_diagnostic(
            diagnostics,
            "ANALYSIS_TEXT_CONFLICT_UNREPORTED",
            &format!("{path}.text"),
            "model/OCR candidates that conflict with human text require a linked text_conflict uncertainty",
        );
    }
}

fn validate_image(
    element: &AnalysisElement,
    path: &str,
    evidence: &BTreeMap<&str, usize>,
    diagnostics: &mut Vec<AnalysisDiagnostic>,
) {
    let image_required = matches!(
        element.kind,
        VisualElementKind::Image
            | VisualElementKind::Background
            | VisualElementKind::Icon
            | VisualElementKind::NineSliceCandidate
    );
    let Some(image) = &element.image else {
        if image_required {
            push_diagnostic(
                diagnostics,
                "ANALYSIS_IMAGE_DATA_MISSING",
                &format!("{path}.image"),
                "image-like elements must preserve image recognition evidence",
            );
        }
        return;
    };
    for (index, candidate) in image.description_candidates.iter().enumerate() {
        validate_evidence_ids(
            &candidate.evidence_ids,
            &format!("{path}.image.description_candidates[{index}].evidence_ids"),
            evidence,
            diagnostics,
        );
    }
    if element.kind == VisualElementKind::NineSliceCandidate && !image.likely_nine_slice {
        push_diagnostic(
            diagnostics,
            "ANALYSIS_NINE_SLICE_FLAG_MISSING",
            &format!("{path}.image.likely_nine_slice"),
            "nine_slice_candidate elements must retain the positive nine-slice observation",
        );
    }
}

fn validate_uncertainties(
    analysis: &UiReferenceAnalysis,
    elements: &BTreeMap<&str, usize>,
    regions: &BTreeMap<&str, usize>,
    references: &BTreeMap<&str, usize>,
    evidence: &BTreeMap<&str, usize>,
    diagnostics: &mut Vec<AnalysisDiagnostic>,
) {
    for (index, uncertainty) in analysis.uncertainties.iter().enumerate() {
        let path = format!("$.uncertainties[{index}]");
        let subject = &uncertainty.subject;
        if subject.element_id.is_none()
            && subject.reference_id.is_none()
            && subject.region_id.is_none()
        {
            push_diagnostic(
                diagnostics,
                "ANALYSIS_UNCERTAINTY_SUBJECT_MISSING",
                &format!("{path}.subject"),
                "uncertainty must identify an element, reference, or region",
            );
        }
        if subject
            .element_id
            .as_ref()
            .is_some_and(|id| !elements.contains_key(id.as_str()))
        {
            push_diagnostic(
                diagnostics,
                "ANALYSIS_ELEMENT_UNKNOWN",
                &format!("{path}.subject.element_id"),
                "uncertainty references an unknown element",
            );
        }
        if subject
            .reference_id
            .as_ref()
            .is_some_and(|id| !references.contains_key(id.as_str()))
        {
            push_diagnostic(
                diagnostics,
                "ANALYSIS_REFERENCE_UNKNOWN",
                &format!("{path}.subject.reference_id"),
                "uncertainty references an unknown input reference",
            );
        }
        if subject
            .region_id
            .as_ref()
            .is_some_and(|id| !regions.contains_key(id.as_str()))
        {
            push_diagnostic(
                diagnostics,
                "ANALYSIS_REGION_UNKNOWN",
                &format!("{path}.subject.region_id"),
                "uncertainty references an unknown region",
            );
        }
        validate_evidence_ids(
            &uncertainty.evidence_ids,
            &format!("{path}.evidence_ids"),
            evidence,
            diagnostics,
        );
    }
}

fn validate_evidence_ids(
    evidence_ids: &[String],
    path: &str,
    evidence: &BTreeMap<&str, usize>,
    diagnostics: &mut Vec<AnalysisDiagnostic>,
) {
    let mut seen = BTreeSet::new();
    for (index, evidence_id) in evidence_ids.iter().enumerate() {
        if !evidence.contains_key(evidence_id.as_str()) {
            push_diagnostic(
                diagnostics,
                "ANALYSIS_EVIDENCE_UNKNOWN",
                &format!("{path}[{index}]"),
                format!("unknown evidence_id `{evidence_id}`"),
            );
        }
        if !seen.insert(evidence_id) {
            push_diagnostic(
                diagnostics,
                "ANALYSIS_EVIDENCE_REFERENCE_DUPLICATED",
                &format!("{path}[{index}]"),
                format!("evidence_id `{evidence_id}` is repeated in one evidence list"),
            );
        }
    }
}

fn push_diagnostic(
    diagnostics: &mut Vec<AnalysisDiagnostic>,
    code: &str,
    path: &str,
    message: impl Into<String>,
) {
    if diagnostics.len() < MAX_DIAGNOSTICS {
        diagnostics.push(diagnostic(code, path, message));
    }
}

fn diagnostic(code: &str, path: &str, message: impl Into<String>) -> AnalysisDiagnostic {
    AnalysisDiagnostic {
        severity: AnalysisDiagnosticSeverity::Error,
        code: code.to_owned(),
        path: path.to_owned(),
        message: message.into(),
    }
}

fn json_pointer_path(pointer: &str) -> String {
    if pointer.is_empty() {
        "$".to_owned()
    } else {
        format!("${pointer}")
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        lifecycle::CancellationToken,
        provider::{
            MockProvider, MockScenario, ProviderCapabilities, ProviderDescriptor,
            ProviderExecutionPolicy, ProviderId, ProviderImage, ProviderOperation,
            ProviderRegistry, ProviderRequest, ProviderRunner, ServerRequestId,
            StructuredProviderOutput,
        },
    };
    use std::{collections::BTreeSet, fs, path::Path, sync::Arc};

    fn fixture_bytes(name: &str) -> Vec<u8> {
        fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("fixtures/analysis")
                .join(name),
        )
        .unwrap()
    }

    fn fixture_value(name: &str) -> Value {
        serde_json::from_slice(&fixture_bytes(name)).unwrap()
    }

    fn generation_task_with_visible_text(text: &str) -> GenerationTask {
        let mut task: GenerationTask = serde_json::from_slice(
            &fs::read(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("fixtures")
                    .join("task.valid.json"),
            )
            .unwrap(),
        )
        .unwrap();
        task.run_id = "fixture-regular-page".to_owned();
        task.visible_text = vec![
            serde_json::from_value(serde_json::json!({
                "text": text,
                "context": "fixture title"
            }))
            .unwrap(),
        ];
        task
    }

    fn trusted_from(reference: &AnalysisReference) -> TrustedPreprocessEvidence {
        TrustedPreprocessEvidence {
            reference_id: reference.reference_id.clone(),
            source_sha256: reference.source_sha256.clone(),
            preprocess_cache_key: reference.preprocess_cache_key.clone(),
            preprocess_protocol_version: reference.preprocess_protocol_version,
            preprocess_implementation_version: reference.preprocess_implementation_version.clone(),
            preprocess_manifest_sha256: reference.preprocess_manifest_sha256.clone(),
            standard_preview_sha256: reference.standard_preview_sha256.clone(),
            coordinate_space: reference.coordinate_space,
            coordinate_convention: reference.coordinate_convention.clone(),
            width: reference.width,
            height: reference.height,
        }
    }

    fn preprocess_manifest_fixture() -> ReferencePreprocessManifest {
        serde_json::from_value(serde_json::json!({
            "protocol_version": 1,
            "implementation_version": "ui-reference-preprocess-1",
            "cache_key": "2222222222222222222222222222222222222222222222222222222222222222",
            "reference_id": "primary",
            "source_sha256": "1111111111111111111111111111111111111111111111111111111111111111",
            "source_byte_length": 40000,
            "source_raw_size": {"width": 100, "height": 100},
            "validation_profile": "page_reference",
            "embedded_metadata": {
                "format": "png",
                "decoded_color_type": "rgba8",
                "original_color_type": "rgba8",
                "has_alpha_channel": true,
                "exif_present": false,
                "exif_byte_length": 0,
                "exif_sha256": null,
                "embedded_orientation": null,
                "declared_orientation": "normal",
                "applied_orientation": "normal",
                "icc_profile_present": false,
                "icc_profile_byte_length": 0,
                "icc_profile_sha256": null,
                "declared_color_space": "srgb",
                "preview_sample_encoding": "rgba8_srgb_bytes"
            },
            "coordinate_mapping": {
                "raw_size": {"width": 100, "height": 100},
                "exif_normalized_size": {"width": 100, "height": 100},
                "explicit_crop": {"x": 0, "y": 0, "width": 100, "height": 100},
                "preview_size": {"width": 100, "height": 100},
                "target_logical_size": {"width": 100.0, "height": 100.0},
                "device_physical_size": {"width": 100.0, "height": 100.0},
                "device_physical_raster_size": {"width": 100, "height": 100},
                "applied_orientation": "normal",
                "coordinate_convention": "top-left origin; x right; y down; continuous pixel-edge coordinates; bounds are closed for points and half-open for rectangles",
                "exif_crop_to_preview_scale": {"x": 1.0, "y": 1.0},
                "exif_crop_to_logical_scale": {"x": 1.0, "y": 1.0},
                "logical_to_physical_scale": {"x": 1.0, "y": 1.0},
                "raster_rounding": "only raster output rounds"
            },
            "explicit_safe_area": null,
            "explicit_system_ui_exclusions": [],
            "options": {
                "crop": null,
                "safe_area": null,
                "system_ui_exclusions": [],
                "preview": {"max_edge": 2048},
                "auxiliary": {"grid_spacing": null, "number_regions": false, "high_contrast": false}
            },
            "artifacts": [{
                "kind": "standard_preview",
                "file_name": "preview.png",
                "sha256": "4444444444444444444444444444444444444444444444444444444444444444",
                "byte_length": 40000,
                "width": 100,
                "height": 100,
                "auxiliary_only": false
            }],
            "original_remains_authoritative": true
        }))
        .unwrap()
    }

    fn trusted_mock_execution() -> (
        ProviderExecution,
        GenerationTask,
        Vec<TrustedPreprocessEvidence>,
    ) {
        let output = StructuredProviderOutput {
            operation: ProviderOperation::VisualAnalysis,
            schema: analysis_output_contract(),
            value: fixture_value("regular_page.json"),
        };
        let provider_id = ProviderId::new("fixture-analysis").unwrap();
        let provider = Arc::new(MockProvider::new(
            ProviderDescriptor {
                id: provider_id.clone(),
                capabilities: ProviderCapabilities {
                    image_input: true,
                    structured_output: true,
                    max_image_count: 1,
                    operations: BTreeSet::from([ProviderOperation::VisualAnalysis]),
                },
            },
            [MockScenario::Success {
                output,
                request_id: Some(ServerRequestId::new("fixture-regular-001").unwrap()),
            }],
        ));
        let request = ProviderRequest::visual_analysis(
            "fixture-regular-page",
            "analysis-v1",
            "repository-authored offline fixture request",
            vec![
                ProviderImage::new(
                    "primary",
                    "image/png",
                    Arc::<[u8]>::from(b"fixture-preview".as_slice()),
                )
                .unwrap(),
            ],
            analysis_output_contract(),
        )
        .unwrap();
        let mut registry = ProviderRegistry::default();
        registry.register(provider).unwrap();
        let runner = ProviderRunner::new(registry, ProviderExecutionPolicy::default()).unwrap();
        let execution = runner
            .execute(&provider_id, request, &CancellationToken::default())
            .unwrap();
        let parsed = parse_provider_analysis(&execution.response.output).unwrap();
        let preprocess = vec![trusted_from(&parsed.references[0])];
        (
            execution,
            generation_task_with_visible_text("Start game"),
            preprocess,
        )
    }

    #[test]
    fn generated_schema_is_the_runtime_validation_contract() {
        let schema = analysis_schema();
        assert_eq!(
            schema["$schema"],
            Value::String("https://json-schema.org/draft/2020-12/schema".to_owned())
        );
        let validator = jsonschema::validator_for(&schema).unwrap();
        for fixture in [
            "regular_page.json",
            "long_list.json",
            "hud.json",
            "modal.json",
        ] {
            let value = fixture_value(fixture);
            assert!(validator.is_valid(&value), "schema rejected {fixture}");
            assert!(
                parse_analysis_value(&value).is_ok(),
                "model rejected {fixture}"
            );
        }
        for fixture in ["unknown_field.json", "over_budget.json"] {
            let value = fixture_value(fixture);
            assert!(!validator.is_valid(&value), "schema accepted {fixture}");
            assert!(
                parse_analysis_value(&value).is_err(),
                "model accepted {fixture}"
            );
        }
    }

    #[test]
    fn generated_schema_budget_constraints_match_public_constants() {
        fn assert_usize(value: &Value, expected: usize, path: &str) {
            assert_eq!(
                value.as_u64().and_then(|value| usize::try_from(value).ok()),
                Some(expected),
                "schema constraint drifted at {path}"
            );
        }

        let schema = analysis_schema();
        let properties = &schema["properties"];
        for (field, expected) in [
            ("references", MAX_ANALYSIS_REFERENCES),
            ("regions", MAX_ANALYSIS_REGIONS),
            ("elements", MAX_ANALYSIS_ELEMENTS),
            ("evidence", MAX_ANALYSIS_EVIDENCE),
            ("uncertainties", MAX_ANALYSIS_UNCERTAINTIES),
        ] {
            assert_usize(
                &properties[field]["maxItems"],
                expected,
                &format!("$.properties.{field}.maxItems"),
            );
        }
        let text = &schema["$defs"]["TextRecognition"]["properties"];
        assert_usize(
            &text["original_candidates"]["maxItems"],
            MAX_TEXT_CANDIDATES,
            "$.$defs.TextRecognition.properties.original_candidates.maxItems",
        );
        assert_usize(
            &text["human_provided_text"]["maxLength"],
            MAX_TEXT_LENGTH,
            "$.$defs.TextRecognition.properties.human_provided_text.maxLength",
        );
        assert_usize(
            &schema["$defs"]["TextCandidate"]["properties"]["raw_text"]["maxLength"],
            MAX_TEXT_LENGTH,
            "$.$defs.TextCandidate.properties.raw_text.maxLength",
        );
    }

    #[test]
    fn valid_fixtures_cover_page_list_hud_modal_and_required_semantics() {
        let regular = parse_analysis_json(&fixture_bytes("regular_page.json")).unwrap();
        assert!(regular.elements.iter().any(|element| {
            element.kind == VisualElementKind::Text
                && element
                    .text
                    .as_ref()
                    .is_some_and(|text| text.human_provided_text.is_some())
        }));
        let list = parse_analysis_json(&fixture_bytes("long_list.json")).unwrap();
        assert!(
            list.elements
                .iter()
                .any(|element| element.layout.kind == LayoutBehaviorKind::Scrollable)
        );
        assert!(
            list.elements
                .iter()
                .any(|element| element.repeated_pattern.is_some())
        );
        let hud = parse_analysis_json(&fixture_bytes("hud.json")).unwrap();
        assert!(hud.elements.iter().any(|element| {
            element.layout.kind == LayoutBehaviorKind::FixedAnchor
                && element.kind == VisualElementKind::StatusIndicator
        }));
        assert!(
            hud.elements
                .iter()
                .any(|element| { element.layout.kind == LayoutBehaviorKind::AbsoluteDecoration })
        );
        let modal = parse_analysis_json(&fixture_bytes("modal.json")).unwrap();
        for kind in [
            UncertaintyKind::Occlusion,
            UncertaintyKind::Blur,
            UncertaintyKind::Cropping,
            UncertaintyKind::UnknownFont,
            UncertaintyKind::HiddenInteraction,
            UncertaintyKind::TextConflict,
        ] {
            assert!(
                modal.uncertainties.iter().any(|item| item.kind == kind),
                "modal fixture lacks {kind:?}"
            );
        }
        assert!(modal.elements.iter().any(|element| {
            element.kind == VisualElementKind::NineSliceCandidate
                && element
                    .image
                    .as_ref()
                    .is_some_and(|image| image.likely_nine_slice)
        }));
    }

    #[test]
    fn malformed_budget_and_graph_fixtures_have_stable_diagnostics() {
        let unknown = parse_analysis_json(&fixture_bytes("unknown_field.json")).unwrap_err();
        assert!(unknown.has_code("ANALYSIS_JSON_SCHEMA_INVALID"));
        let over_budget = parse_analysis_json(&fixture_bytes("over_budget.json")).unwrap_err();
        assert!(over_budget.has_code("ANALYSIS_JSON_SCHEMA_INVALID"));
        let graph = parse_analysis_json(&fixture_bytes("graph_invalid.json")).unwrap_err();
        assert!(graph.has_code("ANALYSIS_GRAPH_CYCLE"));
        assert!(graph.has_code("ANALYSIS_GRAPH_ROOT_COUNT_INVALID"));
    }

    #[test]
    fn element_count_depth_confidence_and_text_budgets_are_enforced() {
        let over_budget = parse_analysis_json(&fixture_bytes("over_budget.json")).unwrap_err();
        assert!(over_budget.has_code("ANALYSIS_JSON_SCHEMA_INVALID"));

        let mut base = fixture_value("unknown_field.json");
        base.as_object_mut()
            .unwrap()
            .remove("model_private_reasoning");
        base["regions"][0]["confidence"] = serde_json::json!(1.01);
        assert!(
            parse_analysis_value(&base)
                .unwrap_err()
                .has_code("ANALYSIS_JSON_SCHEMA_INVALID")
        );
        base["regions"][0]["confidence"] = serde_json::json!(1.0);

        let element_template = base["elements"][0].clone();
        let mut at_element_limit = Vec::new();
        for index in 0..MAX_ANALYSIS_ELEMENTS {
            let mut element = element_template.clone();
            element["element_id"] = serde_json::json!(format!("page.node_{index}"));
            element["parent_id"] = if index == 0 {
                Value::Null
            } else {
                serde_json::json!("page.node_0")
            };
            at_element_limit.push(element);
        }
        base["elements"] = Value::Array(at_element_limit);
        base["root_element_id"] = serde_json::json!("page.node_0");
        assert!(parse_analysis_value(&base).is_ok());
        let mut beyond_element_limit = base["elements"].as_array().unwrap().clone();
        let mut extra = element_template.clone();
        extra["element_id"] = serde_json::json!("page.node_overflow");
        extra["parent_id"] = serde_json::json!("page.node_0");
        beyond_element_limit.push(extra);
        base["elements"] = Value::Array(beyond_element_limit);
        assert!(
            parse_analysis_value(&base)
                .unwrap_err()
                .has_code("ANALYSIS_JSON_SCHEMA_INVALID")
        );

        let mut at_depth_limit = Vec::new();
        for index in 0..=MAX_ANALYSIS_DEPTH {
            let mut element = element_template.clone();
            element["element_id"] = serde_json::json!(format!("depth.node_{index}"));
            element["parent_id"] = if index == 0 {
                Value::Null
            } else {
                serde_json::json!(format!("depth.node_{}", index - 1))
            };
            at_depth_limit.push(element);
        }
        base["elements"] = Value::Array(at_depth_limit.clone());
        base["root_element_id"] = serde_json::json!("depth.node_0");
        assert!(parse_analysis_value(&base).is_ok());
        let mut extra = element_template;
        extra["element_id"] = serde_json::json!(format!("depth.node_{}", MAX_ANALYSIS_DEPTH + 1));
        extra["parent_id"] = serde_json::json!(format!("depth.node_{}", MAX_ANALYSIS_DEPTH));
        at_depth_limit.push(extra);
        base["elements"] = Value::Array(at_depth_limit);
        assert!(
            parse_analysis_value(&base)
                .unwrap_err()
                .has_code("ANALYSIS_GRAPH_DEPTH_EXCEEDED")
        );
    }

    #[test]
    fn raw_json_budget_is_enforced_before_schema_validation() {
        let at_byte_limit = format!(
            "\"{}\"",
            "a".repeat(MAX_ANALYSIS_JSON_BYTES.saturating_sub(2))
        );
        assert_eq!(at_byte_limit.len(), MAX_ANALYSIS_JSON_BYTES);
        assert!(
            !validate_analysis_json(at_byte_limit.as_bytes())
                .has_code("ANALYSIS_JSON_BYTES_EXCEEDED")
        );
        let oversized = format!("{at_byte_limit} ");
        let report = validate_analysis_json(oversized.as_bytes());
        assert!(report.has_code("ANALYSIS_JSON_BYTES_EXCEEDED"));

        let at_depth_limit = format!(
            "{}null{}",
            "[".repeat(MAX_JSON_DEPTH),
            "]".repeat(MAX_JSON_DEPTH)
        );
        let at_depth_value: Value = serde_json::from_str(&at_depth_limit).unwrap();
        assert!(validate_json_budget(&at_depth_value).valid);
        let deeply_nested = format!(
            "{}null{}",
            "[".repeat(MAX_JSON_DEPTH + 1),
            "]".repeat(MAX_JSON_DEPTH + 1)
        );
        let report = validate_analysis_json(deeply_nested.as_bytes());
        assert!(report.has_code("ANALYSIS_JSON_DEPTH_EXCEEDED"));

        fn node_tree(total_nodes: usize) -> Value {
            let child_arrays = 7usize;
            let null_nodes = total_nodes - 1 - child_arrays;
            let mut remaining = null_nodes;
            let mut children = Vec::new();
            for child_index in 0..child_arrays {
                let slots = child_arrays - child_index;
                let count = remaining.div_ceil(slots);
                remaining -= count;
                children.push(Value::Array(vec![Value::Null; count]));
            }
            Value::Array(children)
        }
        assert!(validate_json_budget(&node_tree(MAX_JSON_NODES)).valid);
        assert!(
            validate_json_budget(&node_tree(MAX_JSON_NODES + 1))
                .has_code("ANALYSIS_JSON_NODE_BUDGET_EXCEEDED")
        );

        assert!(
            validate_json_budget(&Value::Array(vec![Value::Null; MAX_JSON_CONTAINER_ITEMS])).valid
        );
        assert!(
            validate_json_budget(&Value::Array(vec![
                Value::Null;
                MAX_JSON_CONTAINER_ITEMS + 1
            ]))
            .has_code("ANALYSIS_JSON_CONTAINER_BUDGET_EXCEEDED")
        );

        assert!(validate_json_budget(&Value::String("a".repeat(MAX_JSON_STRING_BYTES))).valid);
        assert!(
            validate_json_budget(&Value::String("a".repeat(MAX_JSON_STRING_BYTES + 1)))
                .has_code("ANALYSIS_JSON_STRING_BUDGET_EXCEEDED")
        );
        let exact_total = Value::Array(
            (0..(MAX_JSON_TOTAL_STRING_BYTES / MAX_JSON_STRING_BYTES))
                .map(|_| Value::String("a".repeat(MAX_JSON_STRING_BYTES)))
                .collect(),
        );
        assert!(validate_json_budget(&exact_total).valid);
        let mut over_total = exact_total.as_array().unwrap().clone();
        over_total.push(Value::String("a".to_owned()));
        assert!(
            validate_json_budget(&Value::Array(over_total))
                .has_code("ANALYSIS_JSON_TOTAL_STRING_BUDGET_EXCEEDED")
        );
    }

    #[test]
    fn provider_output_must_match_analysis_operation_and_contract() {
        let value = fixture_value("regular_page.json");
        let valid = StructuredProviderOutput {
            operation: ProviderOperation::VisualAnalysis,
            schema: analysis_output_contract(),
            value: value.clone(),
        };
        assert!(parse_provider_analysis(&valid).is_ok());

        let wrong_operation = StructuredProviderOutput {
            operation: ProviderOperation::StructuredGeneration,
            schema: analysis_output_contract(),
            value: value.clone(),
        };
        assert!(
            parse_provider_analysis(&wrong_operation)
                .unwrap_err()
                .has_code("ANALYSIS_PROVIDER_OPERATION_MISMATCH")
        );
        let wrong_schema = StructuredProviderOutput {
            operation: ProviderOperation::VisualAnalysis,
            schema: StructuredOutputContract::new(ANALYSIS_SCHEMA_ID, 2).unwrap(),
            value,
        };
        assert!(
            parse_provider_analysis(&wrong_schema)
                .unwrap_err()
                .has_code("ANALYSIS_PROVIDER_SCHEMA_MISMATCH")
        );
    }

    #[test]
    fn stage_two_execution_task_text_and_preprocess_drive_trusted_analysis() {
        let (execution, task, trusted) = trusted_mock_execution();
        assert!(parse_provider_execution_analysis(&execution, &task, &trusted).is_ok());
        let inputs = trusted_human_inputs_from_task(&task).unwrap();
        assert_eq!(inputs[0].input_id(), "task.visible_text.0000");
        assert_eq!(inputs[0].text(), "Start game");

        let mut mismatch = trusted.clone();
        mismatch[0].standard_preview_sha256 = "f".repeat(64);
        assert!(
            parse_provider_execution_analysis(&execution, &task, &mismatch)
                .unwrap_err()
                .has_code("ANALYSIS_PREPROCESS_EVIDENCE_MISMATCH")
        );
    }

    #[test]
    fn trusted_parser_rejects_provider_and_human_self_attestation() {
        let (execution, task, preprocess) = trusted_mock_execution();

        let mut provider_id = execution.clone();
        provider_id.response.output.value["provider"]["provider_id"] =
            serde_json::json!("forged-provider");
        for evidence in provider_id.response.output.value["evidence"]
            .as_array_mut()
            .unwrap()
        {
            if evidence["source"]["kind"] == "provider_response" {
                evidence["source"]["provider_id"] = serde_json::json!("forged-provider");
            }
        }
        assert!(
            parse_provider_execution_analysis(&provider_id, &task, &preprocess)
                .unwrap_err()
                .has_code("ANALYSIS_PROVIDER_ID_UNTRUSTED")
        );

        let mut request_id = execution.clone();
        request_id.response.output.value["provider"]["server_request_id"] =
            serde_json::json!("forged-request-001");
        for evidence in request_id.response.output.value["evidence"]
            .as_array_mut()
            .unwrap()
        {
            if evidence["source"]["kind"] == "provider_response" {
                evidence["source"]["server_request_id"] = serde_json::json!("forged-request-001");
            }
        }
        assert!(
            parse_provider_execution_analysis(&request_id, &task, &preprocess)
                .unwrap_err()
                .has_code("ANALYSIS_PROVIDER_REQUEST_ID_UNTRUSTED")
        );

        let mut prompt = execution.clone();
        prompt.response.output.value["provider"]["prompt_version"] =
            serde_json::json!("forged-prompt-v2");
        assert!(
            parse_provider_execution_analysis(&prompt, &task, &preprocess)
                .unwrap_err()
                .has_code("ANALYSIS_PROMPT_VERSION_UNTRUSTED")
        );

        let mut no_request_id = execution.clone();
        no_request_id.response.server_request_id = None;
        assert!(
            parse_provider_execution_analysis(&no_request_id, &task, &preprocess)
                .unwrap_err()
                .has_code("ANALYSIS_TRUSTED_PROVIDER_REQUEST_ID_MISSING")
        );

        let mut human_id = execution.clone();
        let text_element = human_id.response.output.value["elements"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|element| element["kind"] == "text")
            .unwrap();
        text_element["text"]["human_input_id"] = serde_json::json!("forged-human-input");
        for evidence in human_id.response.output.value["evidence"]
            .as_array_mut()
            .unwrap()
        {
            if evidence["source"]["kind"] == "human_input" {
                evidence["source"]["input_id"] = serde_json::json!("forged-human-input");
            }
        }
        let report = parse_provider_execution_analysis(&human_id, &task, &preprocess).unwrap_err();
        assert!(report.has_code("ANALYSIS_HUMAN_EVIDENCE_UNTRUSTED"));
        assert!(report.has_code("ANALYSIS_HUMAN_TEXT_INPUT_UNTRUSTED"));

        let mut human_text = execution;
        let text_element = human_text.response.output.value["elements"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|element| element["kind"] == "text")
            .unwrap();
        text_element["text"]["human_provided_text"] = serde_json::json!("Forged text");
        assert!(
            parse_provider_execution_analysis(&human_text, &task, &preprocess)
                .unwrap_err()
                .has_code("ANALYSIS_HUMAN_TEXT_MISMATCH")
        );
    }

    #[test]
    fn trusted_evidence_is_derived_from_the_exact_stage_three_manifest_bytes() {
        let manifest = preprocess_manifest_fixture();
        let bytes = serde_json::to_vec(&manifest).unwrap();
        let trusted = TrustedPreprocessEvidence::from_manifest(&manifest, &bytes).unwrap();
        assert_eq!(trusted.reference_id, "primary");
        assert_eq!(trusted.width, 100);
        assert_eq!(trusted.preprocess_manifest_sha256, sha256_hex(&bytes));
        assert!(TrustedPreprocessEvidence::from_manifest(&manifest, b"{}").is_err());
    }

    #[test]
    fn coordinates_are_bounded_by_the_preprocessed_preview() {
        let mut value = fixture_value("regular_page.json");
        value["elements"][0]["bounding_box"]["width"] = serde_json::json!(9999.0);
        let report = parse_analysis_value(&value).unwrap_err();
        assert!(report.has_code("ANALYSIS_JSON_SCHEMA_INVALID"));

        value["elements"][0]["bounding_box"]["width"] = serde_json::json!(1081.0);
        let report = parse_analysis_value(&value).unwrap_err();
        assert!(report.has_code("ANALYSIS_BOUNDING_BOX_OUT_OF_BOUNDS"));
    }

    #[test]
    fn human_text_cannot_be_silently_replaced() {
        let mut value = fixture_value("regular_page.json");
        let mut missing_binding = value.clone();
        let text_element = missing_binding["elements"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|element| element["kind"] == "text")
            .unwrap();
        text_element["text"]["human_input_id"] = Value::Null;
        assert!(
            parse_analysis_value(&missing_binding)
                .unwrap_err()
                .has_code("ANALYSIS_HUMAN_TEXT_INPUT_ID_MISSING")
        );
        {
            let text_element = value["elements"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|element| element["kind"] == "text")
                .unwrap();
            text_element["text"]["adopted"] = serde_json::json!({
                "strategy": "candidate",
                "candidate_index": 0
            });
        }
        let report = parse_analysis_value(&value).unwrap_err();
        assert!(report.has_code("ANALYSIS_HUMAN_TEXT_NOT_AUTHORITATIVE"));

        let text_element = value["elements"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|element| element["kind"] == "text")
            .unwrap();
        text_element["text"]["adopted"] = serde_json::json!({"strategy": "human_provided"});
        value["uncertainties"] = serde_json::json!([]);
        let report = parse_analysis_value(&value).unwrap_err();
        assert!(report.has_code("ANALYSIS_TEXT_CONFLICT_UNREPORTED"));
    }
}
