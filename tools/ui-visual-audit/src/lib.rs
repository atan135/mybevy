pub mod ai;
pub mod comparison;
pub mod gate;
pub mod metrics;
pub mod normalization;
pub mod reference_manifest;
pub mod regions;
pub mod semantic;

pub use ai::{
    AI_ANALYSIS_ALGORITHM_VERSION, AI_ANALYSIS_BUNDLE_SCHEMA_VERSION,
    AI_ANALYSIS_CONFIG_SCHEMA_VERSION, AI_ANALYSIS_OUTPUT_SCHEMA_ID,
    AI_ANALYSIS_PROVIDER_OUTPUT_SCHEMA_VERSION, AI_ANALYSIS_REPORT_FILENAME,
    AI_ANALYSIS_REPORT_SCHEMA_VERSION, AiAllowedDifferences, AiAnalysisBundle, AiAnalysisConfig,
    AiAnalysisOutcome, AiAnalysisReport, AiAnalysisRequest, AiAnalysisStatus, AiCaptureBundle,
    AiCaptureImages, AiCapturePrivacy, AiDeterministicHardFailure, AiEvidence, AiImageRole,
    AiInputReport, AiIssueRegion, AiMockScenario, AiPrivacyReport, AiProblemType, AiProviderConfig,
    AiProviderImageReport, AiProviderIssue, AiProviderOutput, AiProviderPolicy, AiProviderReport,
    AiSeverity, MAX_AI_CAPTURES, MAX_AI_IMAGE_BYTES, MAX_AI_IMAGES, MAX_AI_ISSUES,
    MAX_AI_SENSITIVE_TOTAL_BYTES, MAX_AI_SENSITIVE_VALUE_BYTES, MAX_AI_SENSITIVE_VALUES,
    MAX_AI_TOTAL_IMAGE_BYTES, analyze_with_ai,
};

pub use comparison::{
    ArtifactReport, COMPARISON_CONFIG_SCHEMA_VERSION, COMPARISON_REPORT_FILENAME,
    COMPARISON_REPORT_SCHEMA_VERSION, ComparisonConfig, ComparisonError, ComparisonErrorCode,
    ComparisonErrorResponse, ComparisonExitCode, ComparisonFailure, ComparisonInputsReport,
    ComparisonOutcome, ComparisonReport, ComparisonRequest, ComparisonStatus, ConfigInputReport,
    DimensionsReport, EXACT_RGBA_ALGORITHM_VERSION, ExactMetrics, FailureType, ImageInputReport,
    RegionResult, compare_images,
};
pub use gate::{
    GateBundle, GateConfig, GateExitCode, GateFailureType, GateOutcome, GateRequest, GateState,
    VISUAL_GATE_ALGORITHM_VERSION, VISUAL_GATE_BUNDLE_SCHEMA_VERSION,
    VISUAL_GATE_CONFIG_SCHEMA_VERSION, VISUAL_GATE_PEAK_MEMORY_BUDGET_BYTES,
    VISUAL_GATE_REPORT_FILENAME, VISUAL_GATE_REPORT_SCHEMA_VERSION, VisualGateReport,
    evaluate_visual_gate,
};
pub use metrics::{
    DIFF_METRICS_ALGORITHM_VERSION, DIFF_METRICS_CONFIG_SCHEMA_VERSION,
    DIFF_METRICS_PEAK_MEMORY_BUDGET_BYTES, DIFF_METRICS_REPORT_FILENAME,
    DIFF_METRICS_REPORT_SCHEMA_VERSION, DiffAnalysisOutcome, DiffAnalysisReport,
    DiffAnalysisRequest, DiffAnalysisStatus, DiffMetricsConfig, analyze_aligned_diff,
};
pub use normalization::{
    AffineTransform, AlignmentMode, AlignmentReport, AlphaPolicy, ColorPolicy, CoordinateMapping,
    CropDeclaration, CropKind, CropReport, ImageRoleManifest, InputNormalizationReport,
    IntegerOffset, NORMALIZATION_MANIFEST_SCHEMA_VERSION, NORMALIZATION_REPORT_FILENAME,
    NORMALIZATION_REPORT_SCHEMA_VERSION, NORMALIZE_ALIGN_ALGORITHM_VERSION, NormalizationManifest,
    NormalizationOutcome, NormalizationReport, NormalizationRequest, NormalizationStatus,
    OrientationPolicy, PixelRect, QualityReport, normalize_and_align,
};
pub use reference_manifest::{
    AllowedDifferences, AuthorizationStatus, BaselineRevision, ColorSpace, ErrorCode,
    ImageMetadata, LogicalSize, ManifestError, Orientation, PixelSize, ReferenceEntry,
    ReferenceImage, ReferenceKey, ReferenceManifest, ReferenceProvenance, ReferenceStorage,
    ResolvedReference, ValidatedReferenceManifest, Viewport, load_and_validate_manifest,
    parse_and_validate_manifest, validate_baseline_update, validate_manifest,
};
pub use regions::{
    AuditRegionDeclaration, AuditRegionSource, AuditScope, BoundsSource, BoundsSourceKind,
    ClippingPolicy, CoordinateSpace, DifferenceLocation, IgnoreRegionDeclaration,
    IgnoreRegionResult, PixelPoint, REGION_AUDIT_ALGORITHM_VERSION,
    REGION_AUDIT_CONFIG_SCHEMA_VERSION, REGION_AUDIT_REPORT_FILENAME,
    REGION_AUDIT_REPORT_SCHEMA_VERSION, ReferenceBinding, RegionArtifactReport, RegionAuditConfig,
    RegionAuditOutcome, RegionAuditReport, RegionAuditRequest, RegionAuditResult, RegionLevel,
    RegionLocalStatus, RegionShape, RegionThreshold, SemanticRole, ThresholdProfiles,
    ThresholdViolation, WeightSummary, audit_regions,
};
pub use semantic::{
    IdentitySource, MAX_SEMANTIC_FINDINGS, MAX_SEMANTIC_OVERLAP_CANDIDATES,
    SEMANTIC_AUDIT_ALGORITHM_VERSION, SEMANTIC_AUDIT_CONFIG_SCHEMA_VERSION,
    SEMANTIC_AUDIT_PEAK_MEMORY_BUDGET_BYTES, SEMANTIC_AUDIT_REPORT_FILENAME,
    SEMANTIC_AUDIT_REPORT_SCHEMA_VERSION, SEMANTIC_TREE_SCHEMA_VERSION, SemanticAuditConfig,
    SemanticAuditOutcome, SemanticAuditReport, SemanticAuditRequest, SemanticAuditStatus,
    SemanticFinding, SemanticFindingCode, SemanticLayerPolicy, SemanticLocation, SemanticNode,
    SemanticNodeRole, SemanticPanel, SemanticPanelKind, SemanticRect, SemanticRuleSummary,
    SemanticScroll, SemanticSeparationContract, SemanticTree, audit_semantics,
};
