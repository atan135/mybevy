pub mod comparison;
pub mod reference_manifest;

pub use comparison::{
    ArtifactReport, COMPARISON_CONFIG_SCHEMA_VERSION, COMPARISON_REPORT_FILENAME,
    COMPARISON_REPORT_SCHEMA_VERSION, ComparisonConfig, ComparisonError, ComparisonErrorCode,
    ComparisonErrorResponse, ComparisonExitCode, ComparisonFailure, ComparisonInputsReport,
    ComparisonOutcome, ComparisonReport, ComparisonRequest, ComparisonStatus, ConfigInputReport,
    DimensionsReport, EXACT_RGBA_ALGORITHM_VERSION, ExactMetrics, FailureType, ImageInputReport,
    RegionResult, compare_images,
};
pub use reference_manifest::{
    AllowedDifferences, AuthorizationStatus, BaselineRevision, ColorSpace, ErrorCode,
    ImageMetadata, LogicalSize, ManifestError, Orientation, PixelSize, ReferenceEntry,
    ReferenceImage, ReferenceKey, ReferenceManifest, ReferenceProvenance, ReferenceStorage,
    ResolvedReference, ValidatedReferenceManifest, Viewport, load_and_validate_manifest,
    parse_and_validate_manifest, validate_baseline_update, validate_manifest,
};
