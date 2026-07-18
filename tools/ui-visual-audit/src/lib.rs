pub mod comparison;
pub mod metrics;
pub mod normalization;
pub mod reference_manifest;
pub mod regions;

pub use comparison::{
    ArtifactReport, COMPARISON_CONFIG_SCHEMA_VERSION, COMPARISON_REPORT_FILENAME,
    COMPARISON_REPORT_SCHEMA_VERSION, ComparisonConfig, ComparisonError, ComparisonErrorCode,
    ComparisonErrorResponse, ComparisonExitCode, ComparisonFailure, ComparisonInputsReport,
    ComparisonOutcome, ComparisonReport, ComparisonRequest, ComparisonStatus, ConfigInputReport,
    DimensionsReport, EXACT_RGBA_ALGORITHM_VERSION, ExactMetrics, FailureType, ImageInputReport,
    RegionResult, compare_images,
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
