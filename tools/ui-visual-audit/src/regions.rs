use crate::{
    AffineTransform, ComparisonError, ComparisonErrorCode, ComparisonExitCode, ImageInputReport,
    NORMALIZATION_REPORT_SCHEMA_VERSION, NORMALIZE_ALIGN_ALGORITHM_VERSION, NormalizationReport,
    NormalizationStatus, PixelRect, PixelSize,
    comparison::{
        create_output_directory, resolve_allowed_input_roots, resolve_allowed_root,
        resolve_input_file,
    },
    metrics::{
        DIFF_METRICS_PEAK_MEMORY_BUDGET_BYTES, DiffMetrics, compute_masked_diff,
        decode_aligned_png, load_config, validate_pixel_budget,
    },
};
use image::{ExtendedColorType, ImageEncoder, RgbaImage, codecs::png::PngEncoder};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{Cursor, Write},
    path::{Path, PathBuf},
};

pub const REGION_AUDIT_CONFIG_SCHEMA_VERSION: u32 = 1;
pub const REGION_AUDIT_REPORT_SCHEMA_VERSION: u32 = 1;
pub const REGION_AUDIT_ALGORITHM_VERSION: &str = "ui_region_audit_v1";
pub const REGION_AUDIT_REPORT_FILENAME: &str = "region-audit-report.json";

const IGNORED_REGIONS_FILENAME: &str = "ignored-regions.png";
const AUDIT_COVERAGE_FILENAME: &str = "audit-coverage.png";
const MAX_CONFIG_BYTES: u64 = 512 * 1024;
const MAX_NORMALIZATION_REPORT_BYTES: u64 = 2 * 1024 * 1024;
const MAX_REGIONS: usize = 256;
const MAX_IGNORE_REGIONS: usize = 64;
const MAX_BOUNDS_SOURCES: usize = 512;
const MAX_POLYGON_POINTS: usize = 256;
const REGION_AUDIT_BYTES_PER_PIXEL: u64 = 72;
const REGION_AUDIT_FIXED_MEMORY_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegionAuditRequest {
    pub repository_root: PathBuf,
    pub allowed_input_roots: Vec<PathBuf>,
    pub allowed_output_root: PathBuf,
    pub reference: PathBuf,
    pub actual: PathBuf,
    pub diff_config: PathBuf,
    pub region_config: PathBuf,
    pub normalization_report: PathBuf,
    pub output_directory: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceBinding {
    pub sha256: String,
    pub revision: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CoordinateSpace {
    Aligned,
    ReferenceOriginal,
    ActualOriginal,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundsSourceKind {
    ReferenceElement,
    DeclarativeNode,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditScope {
    FullImage,
    DeclaredRegionsOnly,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClippingPolicy {
    RejectOutOfBounds,
    ClipToAligned,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RegionLevel {
    Critical,
    Normal,
    Decorative,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticRole {
    KeyText,
    KeyButton,
    Content,
    Decoration,
    Dynamic,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PixelPoint {
    pub x: i64,
    pub y: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RegionShape {
    Rectangle { bounds: PixelRect },
    Polygon { points: Vec<PixelPoint> },
    MaskImage { path: String, sha256: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BoundsSource {
    pub source_kind: BoundsSourceKind,
    pub source_id: String,
    pub coordinate_space: CoordinateSpace,
    pub bounds: PixelRect,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AuditRegionSource {
    BoundsSource {
        source_kind: BoundsSourceKind,
        source_id: String,
    },
    Manual {
        coordinate_space: CoordinateSpace,
        shape: RegionShape,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AuditRegionDeclaration {
    pub region_id: String,
    pub label: String,
    pub semantic_role: SemanticRole,
    pub level: RegionLevel,
    pub clipping: ClippingPolicy,
    pub source: AuditRegionSource,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IgnoreRegionDeclaration {
    pub ignore_id: String,
    pub reason: String,
    pub reference_binding: ReferenceBinding,
    pub coordinate_space: CoordinateSpace,
    pub clipping: ClippingPolicy,
    pub shape: RegionShape,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RegionThreshold {
    pub weight: u16,
    pub max_raw_changed_ratio_millionths: u32,
    pub max_alpha_changed_ratio_millionths: u32,
    pub max_tolerated_changed_ratio_millionths: u32,
    pub minimum_ssim_millionths: i32,
    pub max_geometry_changed_ratio_millionths: u32,
    pub max_large_area_ratio_millionths: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ThresholdProfiles {
    pub critical: RegionThreshold,
    pub normal: RegionThreshold,
    pub decorative: RegionThreshold,
}

impl ThresholdProfiles {
    fn get(&self, level: RegionLevel) -> &RegionThreshold {
        match level {
            RegionLevel::Critical => &self.critical,
            RegionLevel::Normal => &self.normal,
            RegionLevel::Decorative => &self.decorative,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RegionAuditConfig {
    pub schema_version: u32,
    pub algorithm_version: String,
    pub reference_binding: ReferenceBinding,
    pub audit_scope: AuditScope,
    pub maximum_ignored_ratio_millionths: u32,
    pub threshold_profiles: ThresholdProfiles,
    pub bounds_sources: Vec<BoundsSource>,
    pub regions: Vec<AuditRegionDeclaration>,
    pub ignore_regions: Vec<IgnoreRegionDeclaration>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RegionLocalStatus {
    Passed,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ThresholdViolation {
    pub metric: String,
    pub observed_millionths: i64,
    pub threshold_millionths: i64,
    pub comparison: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DifferenceLocation {
    pub aligned: PixelPoint,
    pub reference_original: PixelPoint,
    pub actual_original: PixelPoint,
    pub maximum_channel_error: u8,
    pub tolerance_aware_difference: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RegionAuditResult {
    pub region_id: String,
    pub label: String,
    pub semantic_role: SemanticRole,
    pub level: RegionLevel,
    pub weight: u16,
    pub threshold: RegionThreshold,
    pub source_description: String,
    pub mapped_aligned_bounds: PixelRect,
    pub selected_pixels_before_exclusions: u64,
    pub excluded_pixels: u64,
    pub evaluated_pixels: u64,
    pub overlaps_prior_regions_pixels: u64,
    pub metrics: DiffMetrics,
    pub local_status: RegionLocalStatus,
    pub threshold_violations: Vec<ThresholdViolation>,
    pub primary_difference_locations: Vec<DifferenceLocation>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IgnoreRegionResult {
    pub ignore_id: String,
    pub reason: String,
    pub coordinate_space: CoordinateSpace,
    pub mapped_aligned_bounds: PixelRect,
    pub selected_pixels: u64,
    pub newly_ignored_pixels: u64,
    pub overlap_with_prior_ignores_pixels: u64,
    pub reference_binding: ReferenceBinding,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CoverageReport {
    pub image_pixels: u64,
    pub audit_scope: AuditScope,
    pub declared_include_union_pixels: u64,
    pub ignored_union_pixels: u64,
    pub ignored_ratio_millionths: u32,
    pub maximum_ignored_ratio_millionths: u32,
    pub effective_audited_union_pixels: u64,
    pub uncovered_pixels: u64,
    pub ignored_evidence: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WeightSummary {
    pub merge_policy: String,
    pub total_declared_weight: u64,
    pub passed_weight: u64,
    pub failed_weight: u64,
    pub critical_regions: u32,
    pub normal_regions: u32,
    pub decorative_regions: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RegionAuditInputReport {
    pub reference: ImageInputReport,
    pub actual: ImageInputReport,
    pub diff_config_path: String,
    pub diff_config_sha256: String,
    pub region_config_path: String,
    pub region_config_sha256: String,
    pub normalization_report_path: String,
    pub normalization_report_sha256: String,
    pub aligned_reference_sha256: String,
    pub aligned_actual_sha256: String,
    pub mask_images: Vec<ImageInputReport>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RegionArtifactReport {
    pub artifact_type: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<PixelSize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub byte_length: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RegionAuditReport {
    pub schema_version: u32,
    pub algorithm_version: String,
    pub status: String,
    pub scope_boundary: String,
    pub inputs: RegionAuditInputReport,
    pub dimensions: PixelSize,
    pub reference_binding: ReferenceBinding,
    pub coordinate_mapping_version: String,
    pub coverage: CoverageReport,
    pub ignore_regions: Vec<IgnoreRegionResult>,
    pub region_results: Vec<RegionAuditResult>,
    pub weight_summary: WeightSummary,
    pub artifacts: Vec<RegionArtifactReport>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegionAuditOutcome {
    pub report: RegionAuditReport,
    pub exit_code: ComparisonExitCode,
}

struct MappingPair {
    reference: AffineTransform,
    reference_inverse: AffineTransform,
    actual: AffineTransform,
    actual_inverse: AffineTransform,
}

#[derive(Debug)]
struct ResolvedMask {
    pixels: Vec<bool>,
    bounds: PixelRect,
    input: Option<ImageInputReport>,
}

pub fn audit_regions(request: &RegionAuditRequest) -> Result<RegionAuditOutcome, ComparisonError> {
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
    let reference_path = resolve_input_file(&repository_root, &input_roots, &request.reference)?;
    let actual_path = resolve_input_file(&repository_root, &input_roots, &request.actual)?;
    let diff_config_path =
        resolve_input_file(&repository_root, &input_roots, &request.diff_config)?;
    let region_config_path =
        resolve_input_file(&repository_root, &input_roots, &request.region_config)?;
    let normalization_report_path = resolve_input_file(
        &repository_root,
        &input_roots,
        &request.normalization_report,
    )?;

    let diff_config = load_config(&diff_config_path)?;
    let config = load_region_config(&region_config_path)?;
    validate_region_config(&config)?;
    let normalization = load_normalization_report(&normalization_report_path)?;
    let mappings = validate_normalization_binding(
        &normalization,
        &config.reference_binding,
        &reference_path,
        &actual_path,
    )?;
    let reference = decode_aligned_png(&reference_path)?;
    let actual = decode_aligned_png(&actual_path)?;
    validate_pixel_budget(reference.report.dimensions, actual.report.dimensions)?;
    if reference.report.dimensions != actual.report.dimensions {
        return Err(ComparisonError::input(
            ComparisonErrorCode::DimensionsMismatch,
            "aligned reference and actual dimensions must match for region audit",
        ));
    }
    let size = reference.report.dimensions;
    validate_region_memory_budget(size)?;
    validate_normalized_dimensions(&normalization, size)?;
    let output_directory =
        create_output_directory(&repository_root, &output_root, &request.output_directory)?;
    ensure_no_output_alias(
        &output_directory,
        [
            &reference_path,
            &actual_path,
            &diff_config_path,
            &region_config_path,
            &normalization_report_path,
        ],
    )?;

    let bounds_sources: HashMap<(BoundsSourceKind, String), &BoundsSource> = config
        .bounds_sources
        .iter()
        .map(|source| ((source.source_kind, source.source_id.clone()), source))
        .collect();
    let mut mask_inputs = Vec::new();
    let mut ignored_union = vec![false; pixel_len(size)];
    let mut ignore_results = Vec::new();
    for declaration in &config.ignore_regions {
        if declaration.reference_binding != config.reference_binding {
            return Err(ComparisonError::input(
                ComparisonErrorCode::MaskBindingMismatch,
                format!(
                    "ignore region {} was not confirmed for the active reference hash and revision",
                    declaration.ignore_id
                ),
            ));
        }
        let resolved = resolve_shape(
            &declaration.shape,
            declaration.coordinate_space,
            declaration.clipping,
            size,
            &mappings,
            &repository_root,
            &input_roots,
        )?;
        if let Some(input) = resolved.input.clone() {
            mask_inputs.push(input);
        }
        let selected = count_true(&resolved.pixels);
        let overlap = resolved
            .pixels
            .iter()
            .zip(&ignored_union)
            .filter(|(selected, ignored)| **selected && **ignored)
            .count() as u64;
        for (union, selected) in ignored_union.iter_mut().zip(&resolved.pixels) {
            *union |= *selected;
        }
        ignore_results.push(IgnoreRegionResult {
            ignore_id: declaration.ignore_id.clone(),
            reason: declaration.reason.trim().to_owned(),
            coordinate_space: declaration.coordinate_space,
            mapped_aligned_bounds: resolved.bounds,
            selected_pixels: selected,
            newly_ignored_pixels: selected - overlap,
            overlap_with_prior_ignores_pixels: overlap,
            reference_binding: declaration.reference_binding.clone(),
        });
    }
    let image_pixels = u64::from(size.width) * u64::from(size.height);
    let ignored_pixels = count_true(&ignored_union);
    let ignored_ratio = ratio_millionths(ignored_pixels, image_pixels);
    let ignored_exceeds_limit = u128::from(ignored_pixels) * 1_000_000
        > u128::from(config.maximum_ignored_ratio_millionths) * u128::from(image_pixels);
    if ignored_pixels == image_pixels || ignored_exceeds_limit {
        return Err(ComparisonError::input(
            ComparisonErrorCode::IgnoreRatioExceeded,
            format!(
                "ignored pixels ratio {ignored_ratio} exceeds configured maximum {}",
                config.maximum_ignored_ratio_millionths
            ),
        ));
    }

    let mut declared_union = vec![false; pixel_len(size)];
    let mut prior_regions_union = vec![false; pixel_len(size)];
    let mut coverage_levels = vec![0_u8; pixel_len(size)];
    let mut region_results = Vec::new();
    for declaration in &config.regions {
        let (resolved, source_description) = resolve_region_source(
            &declaration.source,
            declaration.clipping,
            size,
            &mappings,
            &bounds_sources,
            &repository_root,
            &input_roots,
        )?;
        if let Some(input) = resolved.input.clone() {
            mask_inputs.push(input);
        }
        let selected_before = count_true(&resolved.pixels);
        let overlaps_prior = resolved
            .pixels
            .iter()
            .zip(&prior_regions_union)
            .filter(|(selected, prior)| **selected && **prior)
            .count() as u64;
        for (union, selected) in declared_union.iter_mut().zip(&resolved.pixels) {
            *union |= *selected;
        }
        let level_value = match declaration.level {
            RegionLevel::Critical => 3,
            RegionLevel::Normal => 2,
            RegionLevel::Decorative => 1,
        };
        for (level, selected) in coverage_levels.iter_mut().zip(&resolved.pixels) {
            if *selected {
                *level = (*level).max(level_value);
            }
        }
        for (union, selected) in prior_regions_union.iter_mut().zip(&resolved.pixels) {
            *union |= *selected;
        }
        let evaluated: Vec<bool> = resolved
            .pixels
            .iter()
            .zip(&ignored_union)
            .map(|(selected, ignored)| *selected && !*ignored)
            .collect();
        let evaluated_pixels = count_true(&evaluated);
        if evaluated_pixels == 0 {
            return Err(ComparisonError::input(
                ComparisonErrorCode::RegionEmpty,
                format!(
                    "region {} contains no evaluated pixels after exclusions",
                    declaration.region_id
                ),
            ));
        }
        let computed =
            compute_masked_diff(&reference.rgba, &actual.rgba, &diff_config, &evaluated)?;
        let threshold = config.threshold_profiles.get(declaration.level);
        let violations = threshold_violations(&computed.metrics, threshold);
        let primary = primary_locations(
            &computed.max_diff,
            &computed.raw_changed,
            &computed.tolerated_changed,
            &evaluated,
            size,
            &mappings,
        );
        region_results.push(RegionAuditResult {
            region_id: declaration.region_id.clone(),
            label: declaration.label.clone(),
            semantic_role: declaration.semantic_role,
            level: declaration.level,
            weight: threshold.weight,
            threshold: threshold.clone(),
            source_description,
            mapped_aligned_bounds: resolved.bounds,
            selected_pixels_before_exclusions: selected_before,
            excluded_pixels: selected_before - evaluated_pixels,
            evaluated_pixels,
            overlaps_prior_regions_pixels: overlaps_prior,
            metrics: computed.metrics,
            local_status: if violations.is_empty() {
                RegionLocalStatus::Passed
            } else {
                RegionLocalStatus::Failed
            },
            threshold_violations: violations,
            primary_difference_locations: primary,
        });
    }

    let scope_union: Vec<bool> = match config.audit_scope {
        AuditScope::FullImage => {
            if count_true(&declared_union) != image_pixels {
                return Err(ComparisonError::input(
                    ComparisonErrorCode::AuditScopeIncomplete,
                    "full_image scope requires declared include regions to cover every aligned pixel",
                ));
            }
            declared_union.clone()
        }
        AuditScope::DeclaredRegionsOnly => declared_union.clone(),
    };
    if !scope_union.iter().any(|value| *value) {
        return Err(ComparisonError::input(
            ComparisonErrorCode::RegionEmpty,
            "declared_regions_only scope requires at least one non-empty include region",
        ));
    }
    let effective_union = scope_union
        .iter()
        .zip(&ignored_union)
        .filter(|(included, ignored)| **included && !**ignored)
        .count() as u64;
    if effective_union == 0 {
        return Err(ComparisonError::input(
            ComparisonErrorCode::RegionEmpty,
            "audit scope contains no evaluated pixels after exclusions",
        ));
    }
    let declared_pixels = count_true(&declared_union);
    let coverage = CoverageReport {
        image_pixels,
        audit_scope: config.audit_scope,
        declared_include_union_pixels: declared_pixels,
        ignored_union_pixels: ignored_pixels,
        ignored_ratio_millionths: ignored_ratio,
        maximum_ignored_ratio_millionths: config.maximum_ignored_ratio_millionths,
        effective_audited_union_pixels: effective_union,
        uncovered_pixels: image_pixels - count_true(&scope_union),
        ignored_evidence:
            "ignored-regions.png uses opaque magenta for every excluded aligned pixel".to_owned(),
    };
    let weight_summary = weight_summary(&region_results);
    let ignored_png = render_ignored_regions(&actual.rgba, &ignored_union)?;
    let coverage_png = render_coverage(&actual.rgba, &ignored_union, &coverage_levels)?;
    let report_path = output_directory.join(REGION_AUDIT_REPORT_FILENAME);
    let ignored_path = output_directory.join(IGNORED_REGIONS_FILENAME);
    let coverage_path = output_directory.join(AUDIT_COVERAGE_FILENAME);
    let artifacts = vec![
        RegionArtifactReport {
            artifact_type: "ignored_regions".to_owned(),
            path: ignored_path.display().to_string(),
            dimensions: Some(size),
            byte_length: Some(ignored_png.len() as u64),
        },
        RegionArtifactReport {
            artifact_type: "audit_coverage".to_owned(),
            path: coverage_path.display().to_string(),
            dimensions: Some(size),
            byte_length: Some(coverage_png.len() as u64),
        },
        RegionArtifactReport {
            artifact_type: "region_audit_report".to_owned(),
            path: report_path.display().to_string(),
            dimensions: None,
            byte_length: None,
        },
    ];
    let report = RegionAuditReport {
        schema_version: REGION_AUDIT_REPORT_SCHEMA_VERSION,
        algorithm_version: REGION_AUDIT_ALGORITHM_VERSION.to_owned(),
        status: "analyzed".to_owned(),
        scope_boundary:
            "region_local_rules_only_no_global_pass_failed_needs_review_or_invalid_gate".to_owned(),
        inputs: RegionAuditInputReport {
            reference: reference.report,
            actual: actual.report,
            diff_config_path: diff_config_path.display().to_string(),
            diff_config_sha256: hash_file(&diff_config_path)?,
            region_config_path: region_config_path.display().to_string(),
            region_config_sha256: hash_file(&region_config_path)?,
            normalization_report_path: normalization_report_path.display().to_string(),
            normalization_report_sha256: hash_file(&normalization_report_path)?,
            aligned_reference_sha256: hash_file(&reference_path)?,
            aligned_actual_sha256: hash_file(&actual_path)?,
            mask_images: mask_inputs,
        },
        dimensions: size,
        reference_binding: config.reference_binding,
        coordinate_mapping_version: NORMALIZE_ALIGN_ALGORITHM_VERSION.to_owned(),
        coverage,
        ignore_regions: ignore_results,
        region_results,
        weight_summary,
        artifacts,
    };
    persist_bundle(
        vec![(ignored_path, ignored_png), (coverage_path, coverage_png)],
        &report_path,
        &report,
    )?;
    Ok(RegionAuditOutcome {
        report,
        // Stage 9 owns the global threshold exit. Local failures remain explicit in region_results.
        exit_code: ComparisonExitCode::Success,
    })
}

fn load_region_config(path: &Path) -> Result<RegionAuditConfig, ComparisonError> {
    let bytes = read_bounded(path, MAX_CONFIG_BYTES, "region config")?;
    serde_json::from_slice(&bytes).map_err(|error| {
        ComparisonError::input(
            ComparisonErrorCode::ConfigParseFailed,
            format!("region config is not valid strict schema JSON: {error}"),
        )
        .at_path(path)
    })
}

fn load_normalization_report(path: &Path) -> Result<NormalizationReport, ComparisonError> {
    let bytes = read_bounded(path, MAX_NORMALIZATION_REPORT_BYTES, "normalization report")?;
    serde_json::from_slice(&bytes).map_err(|error| {
        ComparisonError::input(
            ComparisonErrorCode::NormalizationReportInvalid,
            format!("normalization report is not valid strict report JSON: {error}"),
        )
        .at_path(path)
    })
}

fn read_bounded(path: &Path, maximum: u64, label: &str) -> Result<Vec<u8>, ComparisonError> {
    let metadata = fs::metadata(path).map_err(|error| {
        ComparisonError::input(
            ComparisonErrorCode::InputMissing,
            format!("{label} metadata cannot be read: {error}"),
        )
        .at_path(path)
    })?;
    if metadata.len() > maximum {
        return Err(ComparisonError::input(
            ComparisonErrorCode::ConfigTooLarge,
            format!("{label} exceeds the {maximum}-byte limit"),
        )
        .at_path(path));
    }
    fs::read(path).map_err(|error| {
        ComparisonError::input(
            ComparisonErrorCode::ConfigReadFailed,
            format!("{label} cannot be read: {error}"),
        )
        .at_path(path)
    })
}

fn validate_region_config(config: &RegionAuditConfig) -> Result<(), ComparisonError> {
    let invalid =
        |message: String| ComparisonError::input(ComparisonErrorCode::RegionConfigInvalid, message);
    if config.schema_version != REGION_AUDIT_CONFIG_SCHEMA_VERSION
        || config.algorithm_version != REGION_AUDIT_ALGORITHM_VERSION
    {
        return Err(invalid(format!(
            "region config must use schema {} and algorithm {}",
            REGION_AUDIT_CONFIG_SCHEMA_VERSION, REGION_AUDIT_ALGORITHM_VERSION
        )));
    }
    validate_binding(&config.reference_binding, "reference_binding")?;
    if config.maximum_ignored_ratio_millionths >= 1_000_000 {
        return Err(invalid(
            "maximum_ignored_ratio_millionths must be below 1000000".to_owned(),
        ));
    }
    if config.regions.is_empty() || config.regions.len() > MAX_REGIONS {
        return Err(invalid(format!(
            "regions must contain 1..={MAX_REGIONS} entries"
        )));
    }
    if config.ignore_regions.len() > MAX_IGNORE_REGIONS
        || config.bounds_sources.len() > MAX_BOUNDS_SOURCES
    {
        return Err(invalid(
            "region declaration count exceeds the fixed limit".to_owned(),
        ));
    }
    validate_threshold_profiles(&config.threshold_profiles)?;
    let mut source_ids = HashSet::new();
    for source in &config.bounds_sources {
        validate_id(&source.source_id, "bounds source id")?;
        validate_nonempty_rect(source.bounds, "bounds source")?;
        if !source_ids.insert((source.source_kind, source.source_id.as_str())) {
            return Err(invalid(format!(
                "duplicate bounds source {:?}/{}",
                source.source_kind, source.source_id
            )));
        }
        let valid_space = matches!(
            (source.source_kind, source.coordinate_space),
            (
                BoundsSourceKind::ReferenceElement,
                CoordinateSpace::ReferenceOriginal | CoordinateSpace::Aligned
            ) | (
                BoundsSourceKind::DeclarativeNode,
                CoordinateSpace::ActualOriginal | CoordinateSpace::Aligned
            )
        );
        if !valid_space {
            return Err(invalid(format!(
                "bounds source {} uses a coordinate space incompatible with its owner",
                source.source_id
            )));
        }
    }
    let mut region_ids = HashSet::new();
    for region in &config.regions {
        validate_id(&region.region_id, "region id")?;
        if region.label.trim().is_empty() || !region_ids.insert(region.region_id.as_str()) {
            return Err(invalid(format!(
                "region {} has an empty label or duplicate id",
                region.region_id
            )));
        }
        if matches!(
            region.semantic_role,
            SemanticRole::KeyText | SemanticRole::KeyButton
        ) && region.level != RegionLevel::Critical
        {
            return Err(invalid(format!(
                "key text/button region {} must use critical level",
                region.region_id
            )));
        }
        match &region.source {
            AuditRegionSource::BoundsSource {
                source_kind,
                source_id,
            } => {
                if !source_ids.contains(&(*source_kind, source_id.as_str())) {
                    return Err(ComparisonError::input(
                        ComparisonErrorCode::RegionSourceMissing,
                        format!(
                            "region {} references missing bounds source",
                            region.region_id
                        ),
                    ));
                }
            }
            AuditRegionSource::Manual { shape, .. } => validate_shape(shape)?,
        }
    }
    let mut ignore_ids = HashSet::new();
    for ignore in &config.ignore_regions {
        validate_id(&ignore.ignore_id, "ignore id")?;
        if !ignore_ids.insert(ignore.ignore_id.as_str()) {
            return Err(invalid(format!("duplicate ignore id {}", ignore.ignore_id)));
        }
        if ignore.reason.trim().is_empty() {
            return Err(ComparisonError::input(
                ComparisonErrorCode::IgnoreReasonMissing,
                format!(
                    "ignore region {} requires a non-empty reason",
                    ignore.ignore_id
                ),
            ));
        }
        validate_binding(&ignore.reference_binding, "ignore reference_binding")?;
        validate_shape(&ignore.shape)?;
    }
    Ok(())
}

fn validate_threshold_profiles(profiles: &ThresholdProfiles) -> Result<(), ComparisonError> {
    for (name, threshold) in [
        ("critical", &profiles.critical),
        ("normal", &profiles.normal),
        ("decorative", &profiles.decorative),
    ] {
        if threshold.weight == 0
            || threshold.weight > 1000
            || threshold.max_raw_changed_ratio_millionths > 1_000_000
            || threshold.max_alpha_changed_ratio_millionths > 1_000_000
            || threshold.max_tolerated_changed_ratio_millionths > 1_000_000
            || !(-1_000_000..=1_000_000).contains(&threshold.minimum_ssim_millionths)
            || threshold.max_geometry_changed_ratio_millionths > 1_000_000
            || threshold.max_large_area_ratio_millionths > 1_000_000
        {
            return Err(ComparisonError::input(
                ComparisonErrorCode::RegionConfigInvalid,
                format!("{name} threshold contains an out-of-range value"),
            ));
        }
    }
    if !(profiles.critical.weight > profiles.normal.weight
        && profiles.normal.weight > profiles.decorative.weight)
    {
        return Err(ComparisonError::input(
            ComparisonErrorCode::RegionConfigInvalid,
            "weights must be strictly ordered critical > normal > decorative",
        ));
    }
    for (critical, normal, decorative) in [
        (
            profiles.critical.max_raw_changed_ratio_millionths,
            profiles.normal.max_raw_changed_ratio_millionths,
            profiles.decorative.max_raw_changed_ratio_millionths,
        ),
        (
            profiles.critical.max_alpha_changed_ratio_millionths,
            profiles.normal.max_alpha_changed_ratio_millionths,
            profiles.decorative.max_alpha_changed_ratio_millionths,
        ),
        (
            profiles.critical.max_tolerated_changed_ratio_millionths,
            profiles.normal.max_tolerated_changed_ratio_millionths,
            profiles.decorative.max_tolerated_changed_ratio_millionths,
        ),
        (
            profiles.critical.max_geometry_changed_ratio_millionths,
            profiles.normal.max_geometry_changed_ratio_millionths,
            profiles.decorative.max_geometry_changed_ratio_millionths,
        ),
        (
            profiles.critical.max_large_area_ratio_millionths,
            profiles.normal.max_large_area_ratio_millionths,
            profiles.decorative.max_large_area_ratio_millionths,
        ),
    ] {
        if !(critical <= normal && normal <= decorative) {
            return Err(ComparisonError::input(
                ComparisonErrorCode::RegionConfigInvalid,
                "critical/normal/decorative maximum thresholds must be nondecreasing",
            ));
        }
    }
    if !(profiles.critical.minimum_ssim_millionths >= profiles.normal.minimum_ssim_millionths
        && profiles.normal.minimum_ssim_millionths >= profiles.decorative.minimum_ssim_millionths)
    {
        return Err(ComparisonError::input(
            ComparisonErrorCode::RegionConfigInvalid,
            "critical/normal/decorative minimum SSIM must be nonincreasing",
        ));
    }
    Ok(())
}

fn validate_id(value: &str, label: &str) -> Result<(), ComparisonError> {
    let valid = !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'));
    if valid {
        Ok(())
    } else {
        Err(ComparisonError::input(
            ComparisonErrorCode::RegionConfigInvalid,
            format!("{label} must be 1..=128 ASCII identifier characters"),
        ))
    }
}

fn validate_binding(binding: &ReferenceBinding, label: &str) -> Result<(), ComparisonError> {
    if binding.revision == 0
        || binding.sha256.len() != 64
        || !binding
            .sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(ComparisonError::input(
            ComparisonErrorCode::RegionConfigInvalid,
            format!("{label} must contain a positive revision and lowercase SHA-256"),
        ));
    }
    Ok(())
}

fn validate_shape(shape: &RegionShape) -> Result<(), ComparisonError> {
    match shape {
        RegionShape::Rectangle { bounds } => validate_nonempty_rect(*bounds, "rectangle"),
        RegionShape::Polygon { points } => {
            if !(3..=MAX_POLYGON_POINTS).contains(&points.len()) {
                return Err(ComparisonError::input(
                    ComparisonErrorCode::RegionConfigInvalid,
                    format!("polygon must contain 3..={MAX_POLYGON_POINTS} points"),
                ));
            }
            let area_twice: i128 = points
                .iter()
                .zip(points.iter().cycle().skip(1))
                .map(|(a, b)| i128::from(a.x) * i128::from(b.y) - i128::from(b.x) * i128::from(a.y))
                .sum();
            if area_twice == 0 {
                return Err(ComparisonError::input(
                    ComparisonErrorCode::RegionConfigInvalid,
                    "polygon area must be nonzero",
                ));
            }
            Ok(())
        }
        RegionShape::MaskImage { path, sha256 } => {
            if path.is_empty()
                || path.contains('\\')
                || sha256.len() != 64
                || !sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            {
                return Err(ComparisonError::input(
                    ComparisonErrorCode::RegionConfigInvalid,
                    "mask image requires a forward-slash path and lowercase SHA-256",
                ));
            }
            Ok(())
        }
    }
}

fn validate_nonempty_rect(rect: PixelRect, label: &str) -> Result<(), ComparisonError> {
    if rect.width == 0 || rect.height == 0 {
        Err(ComparisonError::input(
            ComparisonErrorCode::RegionConfigInvalid,
            format!("{label} width and height must be positive"),
        ))
    } else {
        Ok(())
    }
}

fn validate_normalization_binding(
    report: &NormalizationReport,
    binding: &ReferenceBinding,
    reference_path: &Path,
    actual_path: &Path,
) -> Result<MappingPair, ComparisonError> {
    if report.schema_version != NORMALIZATION_REPORT_SCHEMA_VERSION
        || report.algorithm_version != NORMALIZE_ALIGN_ALGORITHM_VERSION
        || report.status != NormalizationStatus::Passed
        || report.reference.sha256 != binding.sha256
    {
        return Err(ComparisonError::input(
            ComparisonErrorCode::ReferenceBindingMismatch,
            "normalization report is not a successful run for the bound reference hash",
        ));
    }
    for (artifact_type, expected) in [
        ("aligned_reference", reference_path),
        ("aligned_actual", actual_path),
    ] {
        let artifact = report
            .artifacts
            .iter()
            .find(|artifact| artifact.artifact_type == artifact_type)
            .ok_or_else(|| {
                ComparisonError::input(
                    ComparisonErrorCode::NormalizationReportInvalid,
                    format!("normalization report lacks {artifact_type} artifact"),
                )
            })?;
        let artifact_path = fs::canonicalize(&artifact.path).map_err(|error| {
            ComparisonError::input(
                ComparisonErrorCode::NormalizationReportInvalid,
                format!("normalization artifact cannot be resolved: {error}"),
            )
        })?;
        if artifact_path != expected {
            return Err(ComparisonError::input(
                ComparisonErrorCode::NormalizationReportInvalid,
                format!("{artifact_type} does not match the requested aligned input"),
            ));
        }
    }
    let reference = report
        .reference
        .coordinate_mapping
        .as_ref()
        .ok_or_else(|| {
            ComparisonError::input(
                ComparisonErrorCode::NormalizationReportInvalid,
                "reference coordinate mapping is missing",
            )
        })?;
    let actual = report.actual.coordinate_mapping.as_ref().ok_or_else(|| {
        ComparisonError::input(
            ComparisonErrorCode::NormalizationReportInvalid,
            "actual coordinate mapping is missing",
        )
    })?;
    Ok(MappingPair {
        reference: reference.original_to_aligned,
        reference_inverse: reference.aligned_to_original,
        actual: actual.original_to_aligned,
        actual_inverse: actual.aligned_to_original,
    })
}

fn validate_normalized_dimensions(
    report: &NormalizationReport,
    size: PixelSize,
) -> Result<(), ComparisonError> {
    let aligned = report.alignment.as_ref().ok_or_else(|| {
        ComparisonError::input(
            ComparisonErrorCode::NormalizationReportInvalid,
            "normalization report lacks alignment result",
        )
    })?;
    if aligned.aligned_dimensions != size
        || report.reference.aligned_dimensions != Some(size)
        || report.actual.aligned_dimensions != Some(size)
    {
        return Err(ComparisonError::input(
            ComparisonErrorCode::NormalizationReportInvalid,
            "normalization report aligned dimensions do not match the requested images",
        ));
    }
    Ok(())
}

fn resolve_region_source(
    source: &AuditRegionSource,
    clipping: ClippingPolicy,
    size: PixelSize,
    mappings: &MappingPair,
    bounds_sources: &HashMap<(BoundsSourceKind, String), &BoundsSource>,
    repository_root: &Path,
    input_roots: &[PathBuf],
) -> Result<(ResolvedMask, String), ComparisonError> {
    match source {
        AuditRegionSource::BoundsSource {
            source_kind,
            source_id,
        } => {
            let bounds = bounds_sources
                .get(&(*source_kind, source_id.clone()))
                .ok_or_else(|| {
                    ComparisonError::input(
                        ComparisonErrorCode::RegionSourceMissing,
                        format!("bounds source {:?}/{source_id} is missing", source_kind),
                    )
                })?;
            let shape = RegionShape::Rectangle {
                bounds: bounds.bounds,
            };
            Ok((
                resolve_shape(
                    &shape,
                    bounds.coordinate_space,
                    clipping,
                    size,
                    mappings,
                    repository_root,
                    input_roots,
                )?,
                format!("{:?}:{}", source_kind, source_id).to_lowercase(),
            ))
        }
        AuditRegionSource::Manual {
            coordinate_space,
            shape,
        } => Ok((
            resolve_shape(
                shape,
                *coordinate_space,
                clipping,
                size,
                mappings,
                repository_root,
                input_roots,
            )?,
            format!("manual:{coordinate_space:?}").to_lowercase(),
        )),
    }
}

fn resolve_shape(
    shape: &RegionShape,
    coordinate_space: CoordinateSpace,
    clipping: ClippingPolicy,
    size: PixelSize,
    mappings: &MappingPair,
    repository_root: &Path,
    input_roots: &[PathBuf],
) -> Result<ResolvedMask, ComparisonError> {
    let transform = match coordinate_space {
        CoordinateSpace::Aligned => AffineTransform {
            xx: 1,
            xy: 0,
            x_offset: 0,
            yx: 0,
            yy: 1,
            y_offset: 0,
        },
        CoordinateSpace::ReferenceOriginal => mappings.reference,
        CoordinateSpace::ActualOriginal => mappings.actual,
    };
    match shape {
        RegionShape::Rectangle { bounds } => {
            let mapped = transform.map_rect(*bounds);
            rasterize_rectangle(mapped, clipping, size)
        }
        RegionShape::Polygon { points } => {
            let mapped: Vec<PixelPoint> = points
                .iter()
                .map(|point| {
                    let (x, y) = transform.map_point(point.x, point.y);
                    PixelPoint { x, y }
                })
                .collect();
            rasterize_polygon(&mapped, clipping, size)
        }
        RegionShape::MaskImage { path, sha256 } => {
            if coordinate_space != CoordinateSpace::Aligned {
                return Err(ComparisonError::input(
                    ComparisonErrorCode::RegionConfigInvalid,
                    "mask_image coordinate_space must be aligned",
                ));
            }
            let mask_path = resolve_input_file(repository_root, input_roots, Path::new(path))?;
            let actual_hash = hash_file(&mask_path)?;
            if &actual_hash != sha256 {
                return Err(ComparisonError::input(
                    ComparisonErrorCode::MaskHashMismatch,
                    "mask image SHA-256 does not match its declaration",
                )
                .at_path(&mask_path));
            }
            let decoded = decode_aligned_png(&mask_path)?;
            if decoded.report.dimensions != size {
                return Err(ComparisonError::input(
                    ComparisonErrorCode::MaskDimensionsMismatch,
                    "mask image dimensions must equal aligned image dimensions",
                )
                .at_path(&mask_path));
            }
            let pixels: Vec<bool> = decoded.rgba.pixels().map(|pixel| pixel[3] != 0).collect();
            let bounds = mask_bounds(&pixels, size).ok_or_else(|| {
                ComparisonError::input(
                    ComparisonErrorCode::RegionEmpty,
                    "mask image selects no pixels",
                )
            })?;
            Ok(ResolvedMask {
                pixels,
                bounds,
                input: Some(decoded.report),
            })
        }
    }
}

fn rasterize_rectangle(
    rect: PixelRect,
    clipping: ClippingPolicy,
    size: PixelSize,
) -> Result<ResolvedMask, ComparisonError> {
    let right = rect.x.checked_add(i64::from(rect.width)).ok_or_else(|| {
        ComparisonError::input(
            ComparisonErrorCode::RegionOutOfBounds,
            "rectangle x overflow",
        )
    })?;
    let bottom = rect.y.checked_add(i64::from(rect.height)).ok_or_else(|| {
        ComparisonError::input(
            ComparisonErrorCode::RegionOutOfBounds,
            "rectangle y overflow",
        )
    })?;
    let outside = rect.x < 0
        || rect.y < 0
        || right > i64::from(size.width)
        || bottom > i64::from(size.height);
    if outside && clipping == ClippingPolicy::RejectOutOfBounds {
        return Err(ComparisonError::input(
            ComparisonErrorCode::RegionOutOfBounds,
            "mapped rectangle extends outside aligned image bounds",
        ));
    }
    let left = rect.x.clamp(0, i64::from(size.width));
    let top = rect.y.clamp(0, i64::from(size.height));
    let right = right.clamp(0, i64::from(size.width));
    let bottom = bottom.clamp(0, i64::from(size.height));
    if left >= right || top >= bottom {
        return Err(ComparisonError::input(
            ComparisonErrorCode::RegionEmpty,
            "mapped rectangle has no pixels inside aligned image bounds",
        ));
    }
    let mut pixels = vec![false; pixel_len(size)];
    for y in top as u32..bottom as u32 {
        let row = y as usize * size.width as usize;
        for x in left as u32..right as u32 {
            pixels[row + x as usize] = true;
        }
    }
    Ok(ResolvedMask {
        pixels,
        bounds: PixelRect {
            x: left,
            y: top,
            width: (right - left) as u32,
            height: (bottom - top) as u32,
        },
        input: None,
    })
}

fn rasterize_polygon(
    points: &[PixelPoint],
    clipping: ClippingPolicy,
    size: PixelSize,
) -> Result<ResolvedMask, ComparisonError> {
    let outside = points.iter().any(|point| {
        point.x < 0
            || point.y < 0
            || point.x > i64::from(size.width)
            || point.y > i64::from(size.height)
    });
    if outside && clipping == ClippingPolicy::RejectOutOfBounds {
        return Err(ComparisonError::input(
            ComparisonErrorCode::RegionOutOfBounds,
            "mapped polygon extends outside aligned image bounds",
        ));
    }
    let mut pixels = vec![false; pixel_len(size)];
    for y in 0..size.height {
        for x in 0..size.width {
            if polygon_contains_pixel_center(points, x, y) {
                pixels[(y * size.width + x) as usize] = true;
            }
        }
    }
    let bounds = mask_bounds(&pixels, size).ok_or_else(|| {
        ComparisonError::input(
            ComparisonErrorCode::RegionEmpty,
            "mapped polygon selects no aligned pixels",
        )
    })?;
    Ok(ResolvedMask {
        pixels,
        bounds,
        input: None,
    })
}

fn polygon_contains_pixel_center(points: &[PixelPoint], x: u32, y: u32) -> bool {
    let px = i128::from(x) * 2 + 1;
    let py = i128::from(y) * 2 + 1;
    let mut inside = false;
    for (a, b) in points.iter().zip(points.iter().cycle().skip(1)) {
        let ax = i128::from(a.x) * 2;
        let ay = i128::from(a.y) * 2;
        let bx = i128::from(b.x) * 2;
        let by = i128::from(b.y) * 2;
        let cross = (px - ax) * (by - ay) - (py - ay) * (bx - ax);
        if cross == 0
            && px >= ax.min(bx)
            && px <= ax.max(bx)
            && py >= ay.min(by)
            && py <= ay.max(by)
        {
            return true;
        }
        if (ay > py) != (by > py) {
            let intersects = if by > ay {
                (px - ax) * (by - ay) < (bx - ax) * (py - ay)
            } else {
                (px - ax) * (by - ay) > (bx - ax) * (py - ay)
            };
            if intersects {
                inside = !inside;
            }
        }
    }
    inside
}

fn threshold_violations(
    metrics: &DiffMetrics,
    threshold: &RegionThreshold,
) -> Vec<ThresholdViolation> {
    let mut violations = Vec::new();
    let maxima = [
        (
            "raw_changed_ratio",
            metrics.raw.changed_pixel_ratio_millionths,
            threshold.max_raw_changed_ratio_millionths,
        ),
        (
            "alpha_changed_ratio",
            metrics.alpha.changed_pixel_ratio_millionths,
            threshold.max_alpha_changed_ratio_millionths,
        ),
        (
            "tolerated_changed_ratio",
            metrics.tolerated.changed_pixel_ratio_millionths,
            threshold.max_tolerated_changed_ratio_millionths,
        ),
        (
            "geometry_changed_ratio",
            metrics
                .categories
                .geometry_edges
                .mismatched_edge_ratio_millionths,
            threshold.max_geometry_changed_ratio_millionths,
        ),
        (
            "large_area_ratio",
            metrics
                .categories
                .large_area_content
                .covered_pixel_ratio_millionths,
            threshold.max_large_area_ratio_millionths,
        ),
    ];
    for (metric, observed, maximum) in maxima {
        if observed > maximum {
            violations.push(ThresholdViolation {
                metric: metric.to_owned(),
                observed_millionths: i64::from(observed),
                threshold_millionths: i64::from(maximum),
                comparison: "observed_must_be_less_than_or_equal_to_threshold".to_owned(),
            });
        }
    }
    if metrics.perceptual.score_millionths < threshold.minimum_ssim_millionths {
        violations.push(ThresholdViolation {
            metric: "ssim".to_owned(),
            observed_millionths: i64::from(metrics.perceptual.score_millionths),
            threshold_millionths: i64::from(threshold.minimum_ssim_millionths),
            comparison: "observed_must_be_greater_than_or_equal_to_threshold".to_owned(),
        });
    }
    violations
}

fn primary_locations(
    max_diff: &[u8],
    raw_changed: &[bool],
    tolerated_changed: &[bool],
    included: &[bool],
    size: PixelSize,
    mappings: &MappingPair,
) -> Vec<DifferenceLocation> {
    let key = |(maximum, tolerated, x, y): &(u8, bool, u32, u32)| {
        (
            std::cmp::Reverse(*tolerated),
            std::cmp::Reverse(*maximum),
            *y,
            *x,
        )
    };
    let mut candidates = Vec::<(u8, bool, u32, u32)>::with_capacity(5);
    for (index, (((maximum, raw), tolerated), selected)) in max_diff
        .iter()
        .zip(raw_changed)
        .zip(tolerated_changed)
        .zip(included)
        .enumerate()
    {
        if !*selected || !*raw {
            continue;
        }
        candidates.push((
            *maximum,
            *tolerated,
            index as u32 % size.width,
            index as u32 / size.width,
        ));
        candidates.sort_by_key(key);
        candidates.truncate(5);
    }
    candidates
        .into_iter()
        .map(|(maximum, tolerated, x, y)| {
            let (reference_x, reference_y) = mappings
                .reference_inverse
                .map_point(i64::from(x), i64::from(y));
            let (actual_x, actual_y) = mappings
                .actual_inverse
                .map_point(i64::from(x), i64::from(y));
            DifferenceLocation {
                aligned: PixelPoint {
                    x: i64::from(x),
                    y: i64::from(y),
                },
                reference_original: PixelPoint {
                    x: reference_x,
                    y: reference_y,
                },
                actual_original: PixelPoint {
                    x: actual_x,
                    y: actual_y,
                },
                maximum_channel_error: maximum,
                tolerance_aware_difference: tolerated,
            }
        })
        .collect()
}

fn weight_summary(results: &[RegionAuditResult]) -> WeightSummary {
    let mut report = WeightSummary {
        merge_policy: "independent_region_weights_sum_without_pixel_average_or_global_gate"
            .to_owned(),
        total_declared_weight: 0,
        passed_weight: 0,
        failed_weight: 0,
        critical_regions: 0,
        normal_regions: 0,
        decorative_regions: 0,
    };
    for result in results {
        let weight = u64::from(result.weight);
        report.total_declared_weight += weight;
        match result.local_status {
            RegionLocalStatus::Passed => report.passed_weight += weight,
            RegionLocalStatus::Failed => report.failed_weight += weight,
        }
        match result.level {
            RegionLevel::Critical => report.critical_regions += 1,
            RegionLevel::Normal => report.normal_regions += 1,
            RegionLevel::Decorative => report.decorative_regions += 1,
        }
    }
    report
}

fn render_ignored_regions(
    actual: &RgbaImage,
    ignored: &[bool],
) -> Result<Vec<u8>, ComparisonError> {
    let mut rendered = actual.clone();
    for (index, pixel) in rendered.pixels_mut().enumerate() {
        if ignored[index] {
            *pixel = image::Rgba([255, 0, 255, 255]);
        }
    }
    encode_png(&rendered)
}

fn render_coverage(
    actual: &RgbaImage,
    ignored: &[bool],
    levels: &[u8],
) -> Result<Vec<u8>, ComparisonError> {
    let mut rendered = actual.clone();
    for pixel in rendered.pixels_mut() {
        for channel in 0..3 {
            pixel[channel] /= 3;
        }
        pixel[3] = 255;
    }
    for (index, pixel) in rendered.pixels_mut().enumerate() {
        if ignored[index] {
            *pixel = image::Rgba([255, 0, 255, 255]);
        } else {
            *pixel = match levels[index] {
                3 => image::Rgba([255, 48, 48, 255]),
                2 => image::Rgba([255, 192, 32, 255]),
                1 => image::Rgba([32, 200, 255, 255]),
                _ => *pixel,
            };
        }
    }
    encode_png(&rendered)
}

fn encode_png(image: &RgbaImage) -> Result<Vec<u8>, ComparisonError> {
    let mut bytes = Vec::new();
    PngEncoder::new(Cursor::new(&mut bytes))
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            ExtendedColorType::Rgba8,
        )
        .map_err(|error| {
            ComparisonError::internal(
                ComparisonErrorCode::ArtifactWriteFailed,
                format!("region artifact cannot be encoded: {error}"),
            )
        })?;
    Ok(bytes)
}

fn persist_bundle(
    mut artifacts: Vec<(PathBuf, Vec<u8>)>,
    report_path: &Path,
    report: &RegionAuditReport,
) -> Result<(), ComparisonError> {
    let report_bytes = serde_json::to_vec_pretty(report).map_err(|error| {
        ComparisonError::internal(
            ComparisonErrorCode::InternalFailure,
            format!("region report cannot be serialized: {error}"),
        )
    })?;
    artifacts.push((report_path.to_owned(), report_bytes));
    write_transaction(artifacts)
}

fn write_transaction(artifacts: Vec<(PathBuf, Vec<u8>)>) -> Result<(), ComparisonError> {
    let temporary: Vec<PathBuf> = artifacts
        .iter()
        .map(|(path, _)| PathBuf::from(format!("{}.tmp", path.display())))
        .collect();
    let mut created = Vec::new();
    let mut finalized = Vec::new();
    for ((_, bytes), temporary_path) in artifacts.iter().zip(&temporary) {
        let mut file = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(temporary_path)
        {
            Ok(file) => file,
            Err(error) => {
                cleanup(&created, &finalized);
                return Err(ComparisonError::internal(
                    ComparisonErrorCode::ArtifactWriteFailed,
                    format!("temporary region artifact cannot be created: {error}"),
                )
                .at_path(temporary_path));
            }
        };
        created.push(temporary_path.clone());
        if let Err(error) = file.write_all(bytes).and_then(|_| file.flush()) {
            drop(file);
            cleanup(&created, &finalized);
            return Err(ComparisonError::internal(
                ComparisonErrorCode::ArtifactWriteFailed,
                format!("temporary region artifact cannot be written: {error}"),
            )
            .at_path(temporary_path));
        }
    }
    for ((final_path, _), temporary_path) in artifacts.iter().zip(&temporary) {
        if let Err(error) = fs::hard_link(temporary_path, final_path) {
            cleanup(&created, &finalized);
            return Err(ComparisonError::internal(
                ComparisonErrorCode::ArtifactWriteFailed,
                format!("region artifact cannot be finalized without clobbering: {error}"),
            )
            .at_path(final_path));
        }
        finalized.push(final_path.clone());
        if let Err(error) = fs::remove_file(temporary_path) {
            cleanup(&created, &finalized);
            return Err(ComparisonError::internal(
                ComparisonErrorCode::ArtifactWriteFailed,
                format!("region artifact temporary link cannot be removed: {error}"),
            )
            .at_path(temporary_path));
        }
    }
    Ok(())
}

fn cleanup(created: &[PathBuf], finalized: &[PathBuf]) {
    for path in created.iter().chain(finalized) {
        let _ = fs::remove_file(path);
    }
}

fn ensure_no_output_alias<const N: usize>(
    output: &Path,
    inputs: [&PathBuf; N],
) -> Result<(), ComparisonError> {
    for filename in [
        IGNORED_REGIONS_FILENAME,
        AUDIT_COVERAGE_FILENAME,
        REGION_AUDIT_REPORT_FILENAME,
    ] {
        let candidate = output.join(filename);
        if inputs.iter().any(|input| input.as_path() == candidate) {
            return Err(ComparisonError::input(
                ComparisonErrorCode::ArtifactNameConflict,
                "region artifact would overwrite an input file",
            ));
        }
    }
    Ok(())
}

fn hash_file(path: &Path) -> Result<String, ComparisonError> {
    let bytes = fs::read(path).map_err(|error| {
        ComparisonError::input(
            ComparisonErrorCode::InputMissing,
            format!("input cannot be hashed: {error}"),
        )
        .at_path(path)
    })?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn pixel_len(size: PixelSize) -> usize {
    size.width as usize * size.height as usize
}

fn validate_region_memory_budget(size: PixelSize) -> Result<u64, ComparisonError> {
    let pixels = u64::from(size.width)
        .checked_mul(u64::from(size.height))
        .ok_or_else(|| {
            ComparisonError::input(
                ComparisonErrorCode::ImageTooLarge,
                "region audit pixel count overflowed the memory budget",
            )
        })?;
    let estimated = pixels
        .checked_mul(REGION_AUDIT_BYTES_PER_PIXEL)
        .and_then(|bytes| bytes.checked_add(REGION_AUDIT_FIXED_MEMORY_BYTES))
        .ok_or_else(|| {
            ComparisonError::input(
                ComparisonErrorCode::ImageTooLarge,
                "region audit working memory estimate overflowed",
            )
        })?;
    if estimated > DIFF_METRICS_PEAK_MEMORY_BUDGET_BYTES {
        return Err(ComparisonError::input(
            ComparisonErrorCode::ImageTooLarge,
            format!(
                "region audit estimated peak {estimated} exceeds the {}-byte budget",
                DIFF_METRICS_PEAK_MEMORY_BUDGET_BYTES
            ),
        ));
    }
    Ok(estimated)
}

fn count_true(mask: &[bool]) -> u64 {
    mask.iter().filter(|value| **value).count() as u64
}

fn ratio_millionths(numerator: u64, denominator: u64) -> u32 {
    if denominator == 0 {
        return 0;
    }
    ((u128::from(numerator) * 1_000_000 + u128::from(denominator) / 2) / u128::from(denominator))
        as u32
}

fn mask_bounds(mask: &[bool], size: PixelSize) -> Option<PixelRect> {
    let mut min_x = size.width;
    let mut min_y = size.height;
    let mut max_x = 0;
    let mut max_y = 0;
    let mut found = false;
    for (index, selected) in mask.iter().enumerate() {
        if *selected {
            let x = index as u32 % size.width;
            let y = index as u32 / size.width;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
            found = true;
        }
    }
    found.then_some(PixelRect {
        x: i64::from(min_x),
        y: i64::from(min_y),
        width: max_x - min_x + 1,
        height: max_y - min_y + 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polygon_rasterization_and_clipped_rectangle_are_deterministic() {
        let size = PixelSize {
            width: 4,
            height: 4,
        };
        let polygon = rasterize_polygon(
            &[
                PixelPoint { x: 0, y: 0 },
                PixelPoint { x: 4, y: 0 },
                PixelPoint { x: 0, y: 4 },
            ],
            ClippingPolicy::RejectOutOfBounds,
            size,
        )
        .unwrap();
        assert_eq!(count_true(&polygon.pixels), 10);
        let clipped = rasterize_rectangle(
            PixelRect {
                x: -2,
                y: 1,
                width: 4,
                height: 2,
            },
            ClippingPolicy::ClipToAligned,
            size,
        )
        .unwrap();
        assert_eq!(count_true(&clipped.pixels), 4);
        assert_eq!(
            clipped.bounds,
            PixelRect {
                x: 0,
                y: 1,
                width: 2,
                height: 2
            }
        );
    }

    #[test]
    fn reject_policy_and_empty_clip_have_distinct_region_codes() {
        let size = PixelSize {
            width: 4,
            height: 4,
        };
        let rejected = rasterize_rectangle(
            PixelRect {
                x: -1,
                y: 0,
                width: 2,
                height: 2,
            },
            ClippingPolicy::RejectOutOfBounds,
            size,
        )
        .unwrap_err();
        assert_eq!(
            rejected.failure.code,
            ComparisonErrorCode::RegionOutOfBounds
        );
        let empty = rasterize_rectangle(
            PixelRect {
                x: 10,
                y: 10,
                width: 2,
                height: 2,
            },
            ClippingPolicy::ClipToAligned,
            size,
        )
        .unwrap_err();
        assert_eq!(empty.failure.code, ComparisonErrorCode::RegionEmpty);
    }

    #[test]
    fn reference_and_actual_original_spaces_use_their_own_stage_four_maps() {
        let mappings = MappingPair {
            reference: AffineTransform {
                xx: 1,
                xy: 0,
                x_offset: -2,
                yx: 0,
                yy: 1,
                y_offset: -1,
            },
            reference_inverse: AffineTransform {
                xx: 1,
                xy: 0,
                x_offset: 2,
                yx: 0,
                yy: 1,
                y_offset: 1,
            },
            actual: AffineTransform {
                xx: 1,
                xy: 0,
                x_offset: -4,
                yx: 0,
                yy: 1,
                y_offset: -3,
            },
            actual_inverse: AffineTransform {
                xx: 1,
                xy: 0,
                x_offset: 4,
                yx: 0,
                yy: 1,
                y_offset: 3,
            },
        };
        let size = PixelSize {
            width: 20,
            height: 20,
        };
        let shape = RegionShape::Rectangle {
            bounds: PixelRect {
                x: 5,
                y: 5,
                width: 4,
                height: 3,
            },
        };
        let reference = resolve_shape(
            &shape,
            CoordinateSpace::ReferenceOriginal,
            ClippingPolicy::RejectOutOfBounds,
            size,
            &mappings,
            Path::new("."),
            &[],
        )
        .unwrap();
        let actual = resolve_shape(
            &shape,
            CoordinateSpace::ActualOriginal,
            ClippingPolicy::RejectOutOfBounds,
            size,
            &mappings,
            Path::new("."),
            &[],
        )
        .unwrap();
        assert_eq!((reference.bounds.x, reference.bounds.y), (3, 4));
        assert_eq!((actual.bounds.x, actual.bounds.y), (1, 2));
    }

    #[test]
    fn artifact_transaction_preserves_unknown_final_and_temporary_files() {
        let temporary = tempfile::tempdir().unwrap();
        let final_path = temporary.path().join("ignored-regions.png");
        let other_path = temporary.path().join("audit-coverage.png");
        fs::write(&final_path, b"existing-final").unwrap();
        let error = write_transaction(vec![
            (final_path.clone(), b"new-final".to_vec()),
            (other_path.clone(), b"new-other".to_vec()),
        ])
        .unwrap_err();
        assert_eq!(error.failure.code, ComparisonErrorCode::ArtifactWriteFailed);
        assert_eq!(fs::read(&final_path).unwrap(), b"existing-final");
        assert!(!other_path.exists());

        let protected_temporary = temporary.path().join("region-audit-report.json.tmp");
        fs::write(&protected_temporary, b"existing-temporary").unwrap();
        let report_path = temporary.path().join("region-audit-report.json");
        let error = write_transaction(vec![(report_path.clone(), b"report".to_vec())]).unwrap_err();
        assert_eq!(error.failure.code, ComparisonErrorCode::ArtifactWriteFailed);
        assert_eq!(
            fs::read(&protected_temporary).unwrap(),
            b"existing-temporary"
        );
        assert!(!report_path.exists());
    }

    #[test]
    fn region_memory_budget_pins_the_exact_72_byte_per_pixel_boundary() {
        const MAXIMUM_PIXELS: u32 = 7_398_286;
        assert_eq!(
            u64::from(MAXIMUM_PIXELS) * REGION_AUDIT_BYTES_PER_PIXEL
                + REGION_AUDIT_FIXED_MEMORY_BYTES,
            DIFF_METRICS_PEAK_MEMORY_BUDGET_BYTES - 16
        );
        assert_eq!(
            validate_region_memory_budget(PixelSize {
                width: 1,
                height: MAXIMUM_PIXELS,
            })
            .unwrap(),
            DIFF_METRICS_PEAK_MEMORY_BUDGET_BYTES - 16
        );
        let error = validate_region_memory_budget(PixelSize {
            width: 1,
            height: MAXIMUM_PIXELS + 1,
        })
        .unwrap_err();
        assert_eq!(error.failure.code, ComparisonErrorCode::ImageTooLarge);
    }
}
