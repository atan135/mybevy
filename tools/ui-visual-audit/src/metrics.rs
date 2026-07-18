use crate::{
    ComparisonError, ComparisonErrorCode, ComparisonExitCode, ComparisonFailure, FailureType,
    ImageInputReport, PixelSize,
    comparison::{
        create_output_directory, resolve_allowed_input_roots, resolve_allowed_root,
        resolve_input_file,
    },
};
use image::{
    DynamicImage, ExtendedColorType, ImageEncoder, ImageError, ImageFormat, ImageReader, Limits,
    RgbaImage, codecs::png::PngEncoder,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    fs,
    io::{Cursor, Write},
    path::{Path, PathBuf},
    time::Instant,
};

pub const DIFF_METRICS_CONFIG_SCHEMA_VERSION: u32 = 1;
pub const DIFF_METRICS_REPORT_SCHEMA_VERSION: u32 = 1;
pub const DIFF_METRICS_ALGORITHM_VERSION: &str = "ui_diff_metrics_v1";
pub const DIFF_METRICS_REPORT_FILENAME: &str = "diff-metrics-report.json";
pub const DIFF_METRICS_PEAK_MEMORY_BUDGET_BYTES: u64 = 512 * 1024 * 1024;

const SIDE_BY_SIDE_FILENAME: &str = "side-by-side.png";
const OVERLAY_FILENAME: &str = "overlay.png";
const HEATMAP_FILENAME: &str = "heatmap.png";
const BINARY_DIFF_FILENAME: &str = "binary-diff.png";
const MAX_CONFIG_BYTES: u64 = 64 * 1024;
const MAX_IMAGE_BYTES: u64 = 25 * 1024 * 1024;
const MAX_IMAGE_DIMENSION: u32 = 16_384;
const MAX_DECODE_ALLOC: u64 = 512 * 1024 * 1024;
const MAX_TOTAL_DECODED_PIXELS: u64 = 64 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiffMetricsConfig {
    pub schema_version: u32,
    pub algorithm_version: String,
    pub over_threshold_channel_abs: u8,
    pub small_channel_tolerance: u8,
    pub edge_antialias_tolerance: u8,
    pub edge_luma_threshold: u16,
    pub ssim_window_size: u8,
    pub large_area_min_pixels: u32,
    pub large_area_min_ratio_millionths: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DiffAnalysisRequest {
    pub repository_root: PathBuf,
    pub allowed_input_roots: Vec<PathBuf>,
    pub allowed_output_root: PathBuf,
    pub reference: PathBuf,
    pub actual: PathBuf,
    pub config: PathBuf,
    pub output_directory: PathBuf,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffAnalysisStatus {
    Analyzed,
    ComparisonFailed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiffConfigInputReport {
    pub path: String,
    pub schema_version: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiffInputsReport {
    pub reference: ImageInputReport,
    pub actual: ImageInputReport,
    pub config: DiffConfigInputReport,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChannelAbsoluteMetrics {
    pub sum_absolute_error: u64,
    pub mean_absolute_error_millionths: u64,
    pub max_absolute_error: u8,
    pub over_threshold_pixels: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PerChannelAbsoluteMetrics {
    pub red: ChannelAbsoluteMetrics,
    pub green: ChannelAbsoluteMetrics,
    pub blue: ChannelAbsoluteMetrics,
    pub alpha: ChannelAbsoluteMetrics,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RawDiffMetrics {
    pub evaluated_pixels: u64,
    pub changed_pixels: u64,
    pub changed_pixel_ratio_millionths: u32,
    pub over_threshold_pixels: u64,
    pub over_threshold_pixel_ratio_millionths: u32,
    pub mean_absolute_rgba_error_millionths: u64,
    pub maximum_absolute_channel_error: u8,
    pub channels: PerChannelAbsoluteMetrics,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AlphaDiffMetrics {
    pub changed_pixels: u64,
    pub changed_pixel_ratio_millionths: u32,
    pub mean_absolute_error_millionths: u64,
    pub maximum_absolute_error: u8,
    pub over_threshold_pixels: u64,
    pub over_threshold_pixel_ratio_millionths: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ToleratedDiffMetrics {
    pub changed_pixels: u64,
    pub changed_pixel_ratio_millionths: u32,
    pub ignored_small_channel_pixels: u64,
    pub ignored_matching_edge_antialias_pixels: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PerceptualMetric {
    pub metric: String,
    pub version: String,
    pub rationale: String,
    pub minimum_score_millionths: i32,
    pub maximum_score_millionths: i32,
    pub score_millionths: i32,
    pub window_count: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PixelBounds {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GeometryDifferenceMetrics {
    pub definition: String,
    pub reference_edge_pixels: u64,
    pub actual_edge_pixels: u64,
    pub mismatched_edge_pixels: u64,
    pub mismatched_edge_ratio_millionths: u32,
    pub bounds: Option<PixelBounds>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ColorDifferenceMetrics {
    pub definition: String,
    pub different_pixels: u64,
    pub different_pixel_ratio_millionths: u32,
    pub bounds: Option<PixelBounds>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LargeAreaDifferenceMetrics {
    pub definition: String,
    pub minimum_component_pixels: u64,
    pub component_count: u64,
    pub covered_pixels: u64,
    pub covered_pixel_ratio_millionths: u32,
    pub largest_component_pixels: u64,
    pub largest_component_bounds: Option<PixelBounds>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DifferenceCategories {
    pub geometry_edges: GeometryDifferenceMetrics,
    pub color: ColorDifferenceMetrics,
    pub large_area_content: LargeAreaDifferenceMetrics,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiffMetrics {
    pub raw: RawDiffMetrics,
    pub alpha: AlphaDiffMetrics,
    pub tolerated: ToleratedDiffMetrics,
    pub perceptual: PerceptualMetric,
    pub categories: DifferenceCategories,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeterminismParameters {
    pub traversal: String,
    pub worker_threads: u8,
    pub rounding: String,
    pub color_conversion: String,
    pub alpha_compositing: String,
    pub edge_operator: String,
    pub edge_luma_threshold: u16,
    pub ssim_window_size: u8,
    pub ssim_constants: String,
    pub over_threshold_channel_abs: u8,
    pub small_channel_tolerance: u8,
    pub edge_antialias_tolerance: u8,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PeakWorkingMemoryReport {
    pub basis: String,
    pub kind: String,
    pub bytes: u64,
    pub budget_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PerformanceReport {
    pub elapsed_milliseconds: u64,
    pub elapsed_basis: String,
    pub peak_working_memory: PeakWorkingMemoryReport,
    pub generated_png_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiffArtifactReport {
    pub artifact_type: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<PixelSize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub byte_length: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiffAnalysisReport {
    pub schema_version: u32,
    pub algorithm_version: String,
    pub status: DiffAnalysisStatus,
    pub inputs: DiffInputsReport,
    pub dimensions: crate::DimensionsReport,
    pub parameters: DeterminismParameters,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<DiffMetrics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub performance: Option<PerformanceReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<ComparisonFailure>,
    pub artifacts: Vec<DiffArtifactReport>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DiffAnalysisOutcome {
    pub report: DiffAnalysisReport,
    pub exit_code: ComparisonExitCode,
}

struct DecodedAlignedInput {
    report: ImageInputReport,
    rgba: RgbaImage,
}

struct ComputedDiff {
    metrics: DiffMetrics,
    raw_changed: Vec<bool>,
    tolerated_changed: Vec<bool>,
    max_diff: Vec<u8>,
}

struct EncodedArtifact {
    artifact_type: &'static str,
    filename: &'static str,
    dimensions: PixelSize,
    bytes: Vec<u8>,
}

pub fn analyze_aligned_diff(
    request: &DiffAnalysisRequest,
) -> Result<DiffAnalysisOutcome, ComparisonError> {
    let started = Instant::now();
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
        )
        .at_path(&repository_root));
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
    let config_path = resolve_input_file(&repository_root, &input_roots, &request.config)?;
    let config = load_config(&config_path)?;
    let reference = decode_aligned_png(&reference_path)?;
    let actual = decode_aligned_png(&actual_path)?;
    let estimated_peak =
        validate_pixel_budget(reference.report.dimensions, actual.report.dimensions)?;
    let output_directory =
        create_output_directory(&repository_root, &output_root, &request.output_directory)?;
    ensure_outputs_do_not_alias_inputs(
        &output_directory,
        [
            reference_path.as_path(),
            actual_path.as_path(),
            config_path.as_path(),
        ],
    )?;

    let inputs = DiffInputsReport {
        reference: reference.report.clone(),
        actual: actual.report.clone(),
        config: DiffConfigInputReport {
            path: config_path.display().to_string(),
            schema_version: config.schema_version,
        },
    };
    let dimensions = crate::DimensionsReport {
        reference: reference.report.dimensions,
        actual: actual.report.dimensions,
        mask: None,
    };
    let parameters = determinism_parameters(&config);

    if dimensions.reference != dimensions.actual {
        let report_path = output_directory.join(DIFF_METRICS_REPORT_FILENAME);
        let report = DiffAnalysisReport {
            schema_version: DIFF_METRICS_REPORT_SCHEMA_VERSION,
            algorithm_version: DIFF_METRICS_ALGORITHM_VERSION.to_owned(),
            status: DiffAnalysisStatus::ComparisonFailed,
            inputs,
            dimensions,
            parameters,
            metrics: None,
            performance: None,
            failure: Some(ComparisonFailure::new(
                FailureType::Comparison,
                ComparisonErrorCode::DimensionsMismatch,
                "aligned reference and actual dimensions must match",
            )),
            artifacts: vec![report_artifact(&report_path)],
        };
        write_report_only(&report_path, &report)?;
        return Ok(DiffAnalysisOutcome {
            report,
            exit_code: ComparisonExitCode::ComparisonFailure,
        });
    }

    let computed = compute_diff(&reference.rgba, &actual.rgba, &config);
    let encoded = render_artifacts(
        &reference.rgba,
        &actual.rgba,
        &computed.raw_changed,
        &computed.tolerated_changed,
        &computed.max_diff,
    )?;
    let generated_png_bytes = encoded
        .iter()
        .map(|artifact| artifact.bytes.len() as u64)
        .sum();
    let mut artifacts: Vec<DiffArtifactReport> = encoded
        .iter()
        .map(|artifact| DiffArtifactReport {
            artifact_type: artifact.artifact_type.to_owned(),
            path: output_directory
                .join(artifact.filename)
                .display()
                .to_string(),
            dimensions: Some(artifact.dimensions),
            byte_length: Some(artifact.bytes.len() as u64),
        })
        .collect();
    let report_path = output_directory.join(DIFF_METRICS_REPORT_FILENAME);
    artifacts.push(report_artifact(&report_path));
    let report = DiffAnalysisReport {
        schema_version: DIFF_METRICS_REPORT_SCHEMA_VERSION,
        algorithm_version: DIFF_METRICS_ALGORITHM_VERSION.to_owned(),
        status: DiffAnalysisStatus::Analyzed,
        inputs,
        dimensions,
        parameters,
        metrics: Some(computed.metrics),
        performance: Some(PerformanceReport {
            elapsed_milliseconds: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
            elapsed_basis:
                "input_validation_decode_analysis_and_png_encoding_excludes_artifact_persistence"
                    .to_owned(),
            peak_working_memory: PeakWorkingMemoryReport {
                basis: "upper_bound_from_owned_rgba_luma_edge_mask_component_and_png_buffers_v1"
                    .to_owned(),
                kind: "estimated_not_os_measured".to_owned(),
                bytes: estimated_peak,
                budget_bytes: DIFF_METRICS_PEAK_MEMORY_BUDGET_BYTES,
            },
            generated_png_bytes,
        }),
        failure: None,
        artifacts,
    };
    write_bundle(&output_directory, encoded, &report)?;
    Ok(DiffAnalysisOutcome {
        report,
        exit_code: ComparisonExitCode::Success,
    })
}

fn determinism_parameters(config: &DiffMetricsConfig) -> DeterminismParameters {
    DeterminismParameters {
        traversal: "single_thread_row_major".to_owned(),
        worker_threads: 1,
        rounding: "integer_round_half_up".to_owned(),
        color_conversion: "bt601_integer_luma_77_150_29_div256".to_owned(),
        alpha_compositing: "straight_rgba_over_white_integer".to_owned(),
        edge_operator: "sobel_3x3_clamped_l1".to_owned(),
        edge_luma_threshold: config.edge_luma_threshold,
        ssim_window_size: config.ssim_window_size,
        ssim_constants: "wang2004_k1_0.01_k2_0.03_l255_population".to_owned(),
        over_threshold_channel_abs: config.over_threshold_channel_abs,
        small_channel_tolerance: config.small_channel_tolerance,
        edge_antialias_tolerance: config.edge_antialias_tolerance,
    }
}

fn load_config(path: &Path) -> Result<DiffMetricsConfig, ComparisonError> {
    let metadata = fs::metadata(path).map_err(|error| {
        ComparisonError::input(
            ComparisonErrorCode::ConfigReadFailed,
            format!("diff config metadata cannot be read: {error}"),
        )
        .at_path(path)
    })?;
    if metadata.len() > MAX_CONFIG_BYTES {
        return Err(ComparisonError::input(
            ComparisonErrorCode::ConfigTooLarge,
            format!("diff config exceeds the {MAX_CONFIG_BYTES}-byte limit"),
        )
        .at_path(path));
    }
    let bytes = fs::read(path).map_err(|error| {
        ComparisonError::input(
            ComparisonErrorCode::ConfigReadFailed,
            format!("diff config cannot be read: {error}"),
        )
        .at_path(path)
    })?;
    let config: DiffMetricsConfig = serde_json::from_slice(&bytes).map_err(|error| {
        ComparisonError::input(
            ComparisonErrorCode::ConfigParseFailed,
            format!("diff config is not valid schema JSON: {error}"),
        )
        .at_path(path)
    })?;
    let valid = config.schema_version == DIFF_METRICS_CONFIG_SCHEMA_VERSION
        && config.algorithm_version == DIFF_METRICS_ALGORITHM_VERSION
        && config.over_threshold_channel_abs > 0
        && config.small_channel_tolerance <= 4
        && config.small_channel_tolerance < config.over_threshold_channel_abs
        && config.edge_antialias_tolerance >= config.small_channel_tolerance
        && config.edge_antialias_tolerance <= 16
        && (1..=2_040).contains(&config.edge_luma_threshold)
        && config.ssim_window_size == 8
        && config.large_area_min_pixels > 0
        && config.large_area_min_ratio_millionths <= 100_000;
    if !valid {
        return Err(ComparisonError::input(
            ComparisonErrorCode::ConfigInvalid,
            "diff config violates ui_diff_metrics_v1 parameter bounds",
        )
        .at_path(path));
    }
    Ok(config)
}

fn decode_aligned_png(path: &Path) -> Result<DecodedAlignedInput, ComparisonError> {
    let metadata = fs::metadata(path).map_err(|error| {
        ComparisonError::input(
            ComparisonErrorCode::InputMissing,
            format!("aligned image metadata cannot be read: {error}"),
        )
        .at_path(path)
    })?;
    if metadata.len() > MAX_IMAGE_BYTES {
        return Err(ComparisonError::input(
            ComparisonErrorCode::ImageTooLarge,
            format!("aligned image exceeds the {MAX_IMAGE_BYTES}-byte limit"),
        )
        .at_path(path));
    }
    if path.extension().and_then(|value| value.to_str()) != Some("png") {
        return Err(ComparisonError::input(
            ComparisonErrorCode::ImageUnsupportedFormat,
            "aligned diff input must use lowercase .png",
        )
        .at_path(path));
    }
    let bytes = fs::read(path).map_err(|error| {
        ComparisonError::input(
            ComparisonErrorCode::InputMissing,
            format!("aligned image cannot be read: {error}"),
        )
        .at_path(path)
    })?;
    validate_aligned_png_contract(path, &bytes)?;
    let mut reader = ImageReader::with_format(Cursor::new(&bytes), ImageFormat::Png);
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_ALLOC);
    reader.limits(limits);
    let image = reader.decode().map_err(|error| {
        let code = if matches!(error, ImageError::Limits(_)) {
            ComparisonErrorCode::ImageTooLarge
        } else {
            ComparisonErrorCode::ImageCorrupt
        };
        ComparisonError::input(code, format!("aligned image cannot be decoded: {error}"))
            .at_path(path)
    })?;
    let rgba = match image {
        DynamicImage::ImageRgba8(rgba) => rgba,
        _ => {
            return Err(ComparisonError::input(
                ComparisonErrorCode::ImageUnsupportedFormat,
                "aligned image must decode directly as RGBA8 without implicit conversion",
            )
            .at_path(path));
        }
    };
    if rgba
        .pixels()
        .any(|pixel| pixel[3] == 0 && pixel.0[..3].iter().any(|channel| *channel != 0))
    {
        return Err(ComparisonError::input(
            ComparisonErrorCode::AlignedAlphaInvalid,
            "aligned PNG contains hidden RGB in a fully transparent pixel",
        )
        .at_path(path));
    }
    Ok(DecodedAlignedInput {
        report: ImageInputReport {
            path: path.display().to_string(),
            format: "png_rgba8_srgb".to_owned(),
            dimensions: PixelSize {
                width: rgba.width(),
                height: rgba.height(),
            },
            byte_length: metadata.len(),
        },
        rgba,
    })
}

fn validate_aligned_png_contract(path: &Path, bytes: &[u8]) -> Result<(), ComparisonError> {
    if bytes.len() < 33 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" {
        return Err(ComparisonError::input(
            ComparisonErrorCode::ImageCorrupt,
            "aligned input is not a complete PNG",
        )
        .at_path(path));
    }
    if &bytes[12..16] != b"IHDR" || bytes[24] != 8 || bytes[25] != 6 {
        return Err(ComparisonError::input(
            ComparisonErrorCode::ImageUnsupportedFormat,
            "aligned PNG must have 8-bit RGBA color type",
        )
        .at_path(path));
    }
    let mut offset = 8_usize;
    while offset.saturating_add(12) <= bytes.len() {
        let length = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        let chunk_end = offset
            .checked_add(12)
            .and_then(|value| value.checked_add(length))
            .ok_or_else(|| {
                ComparisonError::input(
                    ComparisonErrorCode::ImageCorrupt,
                    "aligned PNG chunk length overflowed",
                )
                .at_path(path)
            })?;
        if chunk_end > bytes.len() {
            return Err(ComparisonError::input(
                ComparisonErrorCode::ImageCorrupt,
                "aligned PNG contains a truncated chunk",
            )
            .at_path(path));
        }
        let kind = &bytes[offset + 4..offset + 8];
        if matches!(kind, b"iCCP" | b"cICP") {
            return Err(ComparisonError::input(
                ComparisonErrorCode::UnsupportedColorProfile,
                "aligned PNG cannot contain an unconverted ICC or CICP profile",
            )
            .at_path(path));
        }
        if kind == b"gAMA"
            && (length != 4 || bytes[offset + 8..offset + 12] != 45_455_u32.to_be_bytes())
        {
            return Err(ComparisonError::input(
                ComparisonErrorCode::UnsupportedColorProfile,
                "aligned PNG gamma must be the canonical sRGB value",
            )
            .at_path(path));
        }
        offset = chunk_end;
        if kind == b"IEND" {
            return Ok(());
        }
    }
    Err(ComparisonError::input(
        ComparisonErrorCode::ImageCorrupt,
        "aligned PNG has no complete IEND chunk",
    )
    .at_path(path))
}

fn validate_pixel_budget(reference: PixelSize, actual: PixelSize) -> Result<u64, ComparisonError> {
    let mut largest = 0_u64;
    let total = [reference, actual]
        .into_iter()
        .try_fold(0_u64, |total, size| {
            let pixels = u64::from(size.width)
                .checked_mul(u64::from(size.height))
                .ok_or_else(|| {
                    ComparisonError::input(
                        ComparisonErrorCode::ImageTooLarge,
                        "aligned image pixel count overflowed",
                    )
                })?;
            largest = largest.max(pixels);
            total.checked_add(pixels).ok_or_else(|| {
                ComparisonError::input(
                    ComparisonErrorCode::ImageTooLarge,
                    "combined aligned image pixel count overflowed",
                )
            })
        })?;
    if total > MAX_TOTAL_DECODED_PIXELS {
        return Err(ComparisonError::input(
            ComparisonErrorCode::ImageTooLarge,
            format!("combined aligned images exceed the {MAX_TOTAL_DECODED_PIXELS}-pixel budget"),
        ));
    }
    if reference.width.checked_mul(2).is_none() {
        return Err(ComparisonError::input(
            ComparisonErrorCode::ImageTooLarge,
            "side-by-side artifact width overflowed",
        ));
    }
    estimated_peak_working_memory_bytes(largest)
}

fn estimated_peak_working_memory_bytes(pixel_count: u64) -> Result<u64, ComparisonError> {
    let estimate = pixel_count
        .checked_mul(64)
        .and_then(|bytes| bytes.checked_add(4 * 1024 * 1024))
        .ok_or_else(|| {
            ComparisonError::input(
                ComparisonErrorCode::ImageTooLarge,
                "diff metrics peak working-memory estimate overflowed",
            )
        })?;
    if estimate > DIFF_METRICS_PEAK_MEMORY_BUDGET_BYTES {
        return Err(ComparisonError::input(
            ComparisonErrorCode::ImageTooLarge,
            format!(
                "diff metrics estimated peak working memory {estimate} exceeds the {}-byte budget",
                DIFF_METRICS_PEAK_MEMORY_BUDGET_BYTES
            ),
        ));
    }
    Ok(estimate)
}

fn ensure_outputs_do_not_alias_inputs(
    output: &Path,
    inputs: [&Path; 3],
) -> Result<(), ComparisonError> {
    for filename in [
        SIDE_BY_SIDE_FILENAME,
        OVERLAY_FILENAME,
        HEATMAP_FILENAME,
        BINARY_DIFF_FILENAME,
        DIFF_METRICS_REPORT_FILENAME,
    ] {
        let artifact = output.join(filename);
        if inputs.iter().any(|input| **input == artifact) {
            return Err(ComparisonError::input(
                ComparisonErrorCode::ArtifactNameConflict,
                "diff artifact would overwrite an input file",
            )
            .at_path(&artifact));
        }
    }
    Ok(())
}

fn compute_diff(
    reference: &RgbaImage,
    actual: &RgbaImage,
    config: &DiffMetricsConfig,
) -> ComputedDiff {
    let pixel_count = u64::from(reference.width()) * u64::from(reference.height());
    let reference_luma = composited_luma(reference);
    let actual_luma = composited_luma(actual);
    let reference_edges = sobel_edges(
        &reference_luma,
        reference.width(),
        reference.height(),
        config.edge_luma_threshold,
    );
    let actual_edges = sobel_edges(
        &actual_luma,
        actual.width(),
        actual.height(),
        config.edge_luma_threshold,
    );
    let mut sums = [0_u64; 4];
    let mut maxima = [0_u8; 4];
    let mut channel_over = [0_u64; 4];
    let mut raw_changed = vec![false; pixel_count as usize];
    let mut tolerated_changed = vec![false; pixel_count as usize];
    let mut max_diff = vec![0_u8; pixel_count as usize];
    let mut changed_pixels = 0_u64;
    let mut over_threshold_pixels = 0_u64;
    let mut alpha_changed = 0_u64;
    let mut ignored_small = 0_u64;
    let mut ignored_edge = 0_u64;
    let mut tolerated_changed_pixels = 0_u64;
    let mut geometry_pixels = 0_u64;
    let mut reference_edge_pixels = 0_u64;
    let mut actual_edge_pixels = 0_u64;
    let mut color_pixels = 0_u64;
    let mut geometry_bounds = BoundsAccumulator::default();
    let mut color_bounds = BoundsAccumulator::default();

    for (index, (reference_pixel, actual_pixel)) in
        reference.pixels().zip(actual.pixels()).enumerate()
    {
        let mut diffs = [0_u8; 4];
        for channel in 0..4 {
            let difference = reference_pixel[channel].abs_diff(actual_pixel[channel]);
            diffs[channel] = difference;
            sums[channel] += u64::from(difference);
            maxima[channel] = maxima[channel].max(difference);
            if difference > config.over_threshold_channel_abs {
                channel_over[channel] += 1;
            }
        }
        let maximum = *diffs.iter().max().unwrap();
        max_diff[index] = maximum;
        let changed = maximum != 0;
        raw_changed[index] = changed;
        if changed {
            changed_pixels += 1;
        }
        if diffs[3] != 0 {
            alpha_changed += 1;
        }
        if diffs
            .iter()
            .any(|value| *value > config.over_threshold_channel_abs)
        {
            over_threshold_pixels += 1;
        }
        let small = diffs
            .iter()
            .all(|value| *value <= config.small_channel_tolerance);
        let matching_edge = reference_edges[index] && actual_edges[index];
        let edge_antialias = matching_edge
            && diffs[..3]
                .iter()
                .all(|value| *value <= config.edge_antialias_tolerance)
            && diffs[3] <= config.small_channel_tolerance;
        let tolerated = changed && !small && !edge_antialias;
        tolerated_changed[index] = tolerated;
        if changed && small {
            ignored_small += 1;
        } else if changed && edge_antialias {
            ignored_edge += 1;
        }
        if tolerated {
            tolerated_changed_pixels += 1;
        }
        let x = index as u32 % reference.width();
        let y = index as u32 / reference.width();
        if reference_edges[index] {
            reference_edge_pixels += 1;
        }
        if actual_edges[index] {
            actual_edge_pixels += 1;
        }
        if reference_edges[index] != actual_edges[index] {
            geometry_pixels += 1;
            geometry_bounds.include(x, y);
        }
        if tolerated && reference_edges[index] == actual_edges[index] {
            color_pixels += 1;
            color_bounds.include(x, y);
        }
    }

    let channels = PerChannelAbsoluteMetrics {
        red: channel_metrics(sums[0], maxima[0], channel_over[0], pixel_count),
        green: channel_metrics(sums[1], maxima[1], channel_over[1], pixel_count),
        blue: channel_metrics(sums[2], maxima[2], channel_over[2], pixel_count),
        alpha: channel_metrics(sums[3], maxima[3], channel_over[3], pixel_count),
    };
    let total_sum = sums.iter().sum::<u64>();
    let large_area = large_area_metrics(
        &tolerated_changed,
        reference.width(),
        reference.height(),
        config,
    );
    let (ssim, window_count) = windowed_ssim_millionths(
        &reference_luma,
        &actual_luma,
        reference.width(),
        reference.height(),
        usize::from(config.ssim_window_size),
    );
    ComputedDiff {
        metrics: DiffMetrics {
            raw: RawDiffMetrics {
                evaluated_pixels: pixel_count,
                changed_pixels,
                changed_pixel_ratio_millionths: ratio_millionths(changed_pixels, pixel_count),
                over_threshold_pixels,
                over_threshold_pixel_ratio_millionths: ratio_millionths(
                    over_threshold_pixels,
                    pixel_count,
                ),
                mean_absolute_rgba_error_millionths: divide_round(
                    total_sum.saturating_mul(1_000_000),
                    pixel_count.saturating_mul(4),
                ),
                maximum_absolute_channel_error: *maxima.iter().max().unwrap(),
                channels,
            },
            alpha: AlphaDiffMetrics {
                changed_pixels: alpha_changed,
                changed_pixel_ratio_millionths: ratio_millionths(alpha_changed, pixel_count),
                mean_absolute_error_millionths: divide_round(
                    sums[3].saturating_mul(1_000_000),
                    pixel_count,
                ),
                maximum_absolute_error: maxima[3],
                over_threshold_pixels: channel_over[3],
                over_threshold_pixel_ratio_millionths: ratio_millionths(
                    channel_over[3],
                    pixel_count,
                ),
            },
            tolerated: ToleratedDiffMetrics {
                changed_pixels: tolerated_changed_pixels,
                changed_pixel_ratio_millionths: ratio_millionths(
                    tolerated_changed_pixels,
                    pixel_count,
                ),
                ignored_small_channel_pixels: ignored_small,
                ignored_matching_edge_antialias_pixels: ignored_edge,
            },
            perceptual: PerceptualMetric {
                metric: "SSIM".to_owned(),
                version: "wang2004_ui_luma_fixed_window_v1".to_owned(),
                rationale: "8x8 local luminance structure is sensitive to UI geometry while separating raw color and alpha evidence".to_owned(),
                minimum_score_millionths: -1_000_000,
                maximum_score_millionths: 1_000_000,
                score_millionths: ssim,
                window_count,
            },
            categories: DifferenceCategories {
                geometry_edges: GeometryDifferenceMetrics {
                    definition: "xor of fixed-threshold Sobel edge membership at the same aligned coordinate".to_owned(),
                    reference_edge_pixels,
                    actual_edge_pixels,
                    mismatched_edge_pixels: geometry_pixels,
                    mismatched_edge_ratio_millionths: ratio_millionths(
                        geometry_pixels,
                        pixel_count,
                    ),
                    bounds: geometry_bounds.finish(),
                },
                color: ColorDifferenceMetrics {
                    definition: "tolerated changed pixels whose reference and actual Sobel edge membership agrees".to_owned(),
                    different_pixels: color_pixels,
                    different_pixel_ratio_millionths: ratio_millionths(
                        color_pixels,
                        pixel_count,
                    ),
                    bounds: color_bounds.finish(),
                },
                large_area_content: large_area,
            },
        },
        raw_changed,
        tolerated_changed,
        max_diff,
    }
}

fn channel_metrics(
    sum: u64,
    maximum: u8,
    over_threshold: u64,
    pixels: u64,
) -> ChannelAbsoluteMetrics {
    ChannelAbsoluteMetrics {
        sum_absolute_error: sum,
        mean_absolute_error_millionths: divide_round(sum.saturating_mul(1_000_000), pixels),
        max_absolute_error: maximum,
        over_threshold_pixels: over_threshold,
    }
}

fn composited_luma(image: &RgbaImage) -> Vec<u8> {
    image
        .pixels()
        .map(|pixel| {
            let rgb_luma = (77_u32 * u32::from(pixel[0])
                + 150_u32 * u32::from(pixel[1])
                + 29_u32 * u32::from(pixel[2])
                + 128)
                >> 8;
            let alpha = u32::from(pixel[3]);
            ((rgb_luma * alpha + 255 * (255 - alpha) + 127) / 255) as u8
        })
        .collect()
}

fn sobel_edges(luma: &[u8], width: u32, height: u32, threshold: u16) -> Vec<bool> {
    let mut edges = vec![false; luma.len()];
    for y in 0..height {
        for x in 0..width {
            let sample = |dx: i32, dy: i32| -> i32 {
                let sx = (x as i32 + dx).clamp(0, width as i32 - 1) as u32;
                let sy = (y as i32 + dy).clamp(0, height as i32 - 1) as u32;
                i32::from(luma[(sy * width + sx) as usize])
            };
            let gx = -sample(-1, -1) + sample(1, -1) - 2 * sample(-1, 0) + 2 * sample(1, 0)
                - sample(-1, 1)
                + sample(1, 1);
            let gy = -sample(-1, -1) - 2 * sample(0, -1) - sample(1, -1)
                + sample(-1, 1)
                + 2 * sample(0, 1)
                + sample(1, 1);
            edges[(y * width + x) as usize] =
                gx.unsigned_abs() + gy.unsigned_abs() >= u32::from(threshold);
        }
    }
    edges
}

fn windowed_ssim_millionths(
    reference: &[u8],
    actual: &[u8],
    width: u32,
    height: u32,
    window: usize,
) -> (i32, u64) {
    let mut score_sum = 0_i64;
    let mut windows = 0_u64;
    for top in (0..height as usize).step_by(window) {
        for left in (0..width as usize).step_by(window) {
            let bottom = (top + window).min(height as usize);
            let right = (left + window).min(width as usize);
            let mut sx = 0_i128;
            let mut sy = 0_i128;
            let mut sxx = 0_i128;
            let mut syy = 0_i128;
            let mut sxy = 0_i128;
            let mut count = 0_i128;
            for y in top..bottom {
                for x in left..right {
                    let index = y * width as usize + x;
                    let xv = i128::from(reference[index]);
                    let yv = i128::from(actual[index]);
                    sx += xv;
                    sy += yv;
                    sxx += xv * xv;
                    syy += yv * yv;
                    sxy += xv * yv;
                    count += 1;
                }
            }
            // C1=(0.01*255)^2=65025/10000 and C2=(0.03*255)^2=585225/10000.
            let luminance_numerator = 20_000 * sx * sy + 65_025 * count * count;
            let luminance_denominator = 10_000 * (sx * sx + sy * sy) + 65_025 * count * count;
            let covariance = count * sxy - sx * sy;
            let variance = count * sxx - sx * sx + count * syy - sy * sy;
            let structure_numerator = 20_000 * covariance + 585_225 * count * count;
            let structure_denominator = 10_000 * variance + 585_225 * count * count;
            let numerator = luminance_numerator * structure_numerator;
            let denominator = luminance_denominator * structure_denominator;
            let score = signed_divide_round(numerator * 1_000_000, denominator)
                .clamp(-1_000_000, 1_000_000) as i64;
            score_sum += score;
            windows += 1;
        }
    }
    (
        signed_divide_round(i128::from(score_sum), i128::from(windows)) as i32,
        windows,
    )
}

fn large_area_metrics(
    mask: &[bool],
    width: u32,
    height: u32,
    config: &DiffMetricsConfig,
) -> LargeAreaDifferenceMetrics {
    let pixel_count = u64::from(width) * u64::from(height);
    let ratio_minimum = divide_round(
        pixel_count.saturating_mul(u64::from(config.large_area_min_ratio_millionths)),
        1_000_000,
    );
    let minimum = u64::from(config.large_area_min_pixels).max(ratio_minimum);
    let mut visited = vec![false; mask.len()];
    let mut component_count = 0_u64;
    let mut covered = 0_u64;
    let mut largest = 0_u64;
    let mut largest_bounds = None;
    for start in 0..mask.len() {
        if !mask[start] || visited[start] {
            continue;
        }
        let mut queue = VecDeque::new();
        queue.push_back(start);
        visited[start] = true;
        let mut size = 0_u64;
        let mut bounds = BoundsAccumulator::default();
        while let Some(index) = queue.pop_front() {
            size += 1;
            let x = index as u32 % width;
            let y = index as u32 / width;
            bounds.include(x, y);
            if x > 0 {
                push_neighbor(index - 1, mask, &mut visited, &mut queue);
            }
            if x + 1 < width {
                push_neighbor(index + 1, mask, &mut visited, &mut queue);
            }
            if y > 0 {
                push_neighbor(index - width as usize, mask, &mut visited, &mut queue);
            }
            if y + 1 < height {
                push_neighbor(index + width as usize, mask, &mut visited, &mut queue);
            }
        }
        if size >= minimum {
            component_count += 1;
            covered += size;
            if size > largest {
                largest = size;
                largest_bounds = bounds.finish();
            }
        }
    }
    LargeAreaDifferenceMetrics {
        definition: "4-connected tolerated-difference components meeting the fixed pixel and image-ratio minimum".to_owned(),
        minimum_component_pixels: minimum,
        component_count,
        covered_pixels: covered,
        covered_pixel_ratio_millionths: ratio_millionths(covered, pixel_count),
        largest_component_pixels: largest,
        largest_component_bounds: largest_bounds,
    }
}

fn push_neighbor(index: usize, mask: &[bool], visited: &mut [bool], queue: &mut VecDeque<usize>) {
    if mask[index] && !visited[index] {
        visited[index] = true;
        queue.push_back(index);
    }
}

#[derive(Default)]
struct BoundsAccumulator {
    minimum_x: Option<u32>,
    minimum_y: Option<u32>,
    maximum_x: u32,
    maximum_y: u32,
}

impl BoundsAccumulator {
    fn include(&mut self, x: u32, y: u32) {
        self.minimum_x = Some(self.minimum_x.map_or(x, |current| current.min(x)));
        self.minimum_y = Some(self.minimum_y.map_or(y, |current| current.min(y)));
        self.maximum_x = self.maximum_x.max(x);
        self.maximum_y = self.maximum_y.max(y);
    }

    fn finish(self) -> Option<PixelBounds> {
        Some(PixelBounds {
            x: self.minimum_x?,
            y: self.minimum_y?,
            width: self.maximum_x - self.minimum_x? + 1,
            height: self.maximum_y - self.minimum_y? + 1,
        })
    }
}

fn render_artifacts(
    reference: &RgbaImage,
    actual: &RgbaImage,
    _raw_changed: &[bool],
    tolerated_changed: &[bool],
    max_diff: &[u8],
) -> Result<Vec<EncodedArtifact>, ComparisonError> {
    let width = reference.width();
    let height = reference.height();
    let mut side_by_side = RgbaImage::new(width * 2, height);
    for y in 0..height {
        for x in 0..width {
            side_by_side.put_pixel(x, y, *reference.get_pixel(x, y));
            side_by_side.put_pixel(width + x, y, *actual.get_pixel(x, y));
        }
    }
    let mut overlay = RgbaImage::new(width, height);
    let mut heatmap = RgbaImage::new(width, height);
    let mut binary = RgbaImage::new(width, height);
    for (index, (reference_pixel, actual_pixel)) in
        reference.pixels().zip(actual.pixels()).enumerate()
    {
        let x = index as u32 % width;
        let y = index as u32 / width;
        let average =
            |left: u8, right: u8| -> u8 { (u16::from(left) + u16::from(right)).div_ceil(2) as u8 };
        overlay.put_pixel(
            x,
            y,
            image::Rgba([
                average(reference_pixel[0], actual_pixel[0]),
                average(reference_pixel[1], actual_pixel[1]),
                average(reference_pixel[2], actual_pixel[2]),
                average(reference_pixel[3], actual_pixel[3]),
            ]),
        );
        let difference = max_diff[index];
        let heat = if difference == 0 {
            [0, 0, 0, 255]
        } else {
            [
                difference.saturating_mul(2),
                255_u8.saturating_sub(difference.saturating_mul(2)),
                0,
                255,
            ]
        };
        heatmap.put_pixel(x, y, image::Rgba(heat));
        let value = if tolerated_changed[index] { 255 } else { 0 };
        binary.put_pixel(x, y, image::Rgba([value, value, value, 255]));
    }
    Ok(vec![
        encoded_artifact("side_by_side", SIDE_BY_SIDE_FILENAME, &side_by_side)?,
        encoded_artifact("overlay", OVERLAY_FILENAME, &overlay)?,
        encoded_artifact("heatmap", HEATMAP_FILENAME, &heatmap)?,
        encoded_artifact("binary_diff", BINARY_DIFF_FILENAME, &binary)?,
    ])
}

fn encoded_artifact(
    artifact_type: &'static str,
    filename: &'static str,
    image: &RgbaImage,
) -> Result<EncodedArtifact, ComparisonError> {
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            ExtendedColorType::Rgba8,
        )
        .map_err(|error| {
            ComparisonError::internal(
                ComparisonErrorCode::ArtifactWriteFailed,
                format!("{artifact_type} PNG cannot be encoded: {error}"),
            )
        })?;
    Ok(EncodedArtifact {
        artifact_type,
        filename,
        dimensions: PixelSize {
            width: image.width(),
            height: image.height(),
        },
        bytes,
    })
}

fn write_bundle(
    output: &Path,
    artifacts: Vec<EncodedArtifact>,
    report: &DiffAnalysisReport,
) -> Result<(), ComparisonError> {
    let mut files: Vec<(PathBuf, Vec<u8>)> = artifacts
        .into_iter()
        .map(|artifact| (output.join(artifact.filename), artifact.bytes))
        .collect();
    files.push((
        output.join(DIFF_METRICS_REPORT_FILENAME),
        serde_json::to_vec_pretty(report).map_err(|error| {
            ComparisonError::internal(
                ComparisonErrorCode::InternalFailure,
                format!("diff report cannot be serialized: {error}"),
            )
        })?,
    ));
    write_transaction(files)
}

fn write_report_only(path: &Path, report: &DiffAnalysisReport) -> Result<(), ComparisonError> {
    let bytes = serde_json::to_vec_pretty(report).map_err(|error| {
        ComparisonError::internal(
            ComparisonErrorCode::InternalFailure,
            format!("diff report cannot be serialized: {error}"),
        )
    })?;
    write_transaction(vec![(path.to_path_buf(), bytes)])
}

fn write_transaction(files: Vec<(PathBuf, Vec<u8>)>) -> Result<(), ComparisonError> {
    let temporary: Vec<PathBuf> = files
        .iter()
        .map(|(path, _)| PathBuf::from(format!("{}.tmp", path.display())))
        .collect();
    let mut created_temporaries = Vec::new();
    let mut finalized = Vec::new();
    for ((_, bytes), temporary_path) in files.iter().zip(&temporary) {
        let mut file = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(temporary_path)
        {
            Ok(file) => file,
            Err(error) => {
                cleanup_transaction_paths(&created_temporaries, &finalized);
                return Err(ComparisonError::internal(
                    ComparisonErrorCode::ArtifactWriteFailed,
                    format!("temporary diff artifact cannot be created: {error}"),
                )
                .at_path(temporary_path));
            }
        };
        created_temporaries.push(temporary_path.clone());
        if let Err(error) = file.write_all(bytes).and_then(|_| file.flush()) {
            drop(file);
            cleanup_transaction_paths(&created_temporaries, &finalized);
            return Err(ComparisonError::internal(
                ComparisonErrorCode::ArtifactWriteFailed,
                format!("temporary diff artifact cannot be written: {error}"),
            )
            .at_path(temporary_path));
        }
    }
    for ((final_path, _), temporary_path) in files.iter().zip(&temporary) {
        if let Err(error) = fs::hard_link(temporary_path, final_path) {
            cleanup_transaction_paths(&created_temporaries, &finalized);
            return Err(ComparisonError::internal(
                ComparisonErrorCode::ArtifactWriteFailed,
                format!("diff artifact cannot be finalized without clobbering: {error}"),
            )
            .at_path(final_path));
        }
        finalized.push(final_path.clone());
        if let Err(error) = fs::remove_file(temporary_path) {
            cleanup_transaction_paths(&created_temporaries, &finalized);
            return Err(ComparisonError::internal(
                ComparisonErrorCode::ArtifactWriteFailed,
                format!("finalized diff artifact temporary link cannot be removed: {error}"),
            )
            .at_path(temporary_path));
        }
    }
    Ok(())
}

fn cleanup_transaction_paths(created_temporaries: &[PathBuf], finalized: &[PathBuf]) {
    for path in created_temporaries.iter().chain(finalized) {
        let _ = fs::remove_file(path);
    }
}

fn report_artifact(path: &Path) -> DiffArtifactReport {
    DiffArtifactReport {
        artifact_type: "diff_metrics_report".to_owned(),
        path: path.display().to_string(),
        dimensions: None,
        byte_length: None,
    }
}

fn ratio_millionths(numerator: u64, denominator: u64) -> u32 {
    divide_round(numerator.saturating_mul(1_000_000), denominator) as u32
}

fn divide_round(numerator: u64, denominator: u64) -> u64 {
    if denominator == 0 {
        return 0;
    }
    numerator.saturating_add(denominator / 2) / denominator
}

fn signed_divide_round(numerator: i128, denominator: i128) -> i128 {
    debug_assert!(denominator > 0);
    if numerator >= 0 {
        (numerator + denominator / 2) / denominator
    } else {
        (numerator - denominator / 2) / denominator
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_ssim_has_pinned_identity_and_constant_shift_values() {
        let identity = vec![128_u8; 64];
        let shifted = vec![138_u8; 64];
        assert_eq!(
            windowed_ssim_millionths(&identity, &identity, 8, 8, 8),
            (1_000_000, 1)
        );
        assert_eq!(
            windowed_ssim_millionths(&identity, &shifted, 8, 8, 8),
            (997_178, 1)
        );
    }

    #[test]
    fn integer_rounding_is_half_up_and_signed() {
        assert_eq!(divide_round(1, 2), 1);
        assert_eq!(signed_divide_round(1, 2), 1);
        assert_eq!(signed_divide_round(-1, 2), -1);
    }

    #[test]
    fn peak_memory_budget_accepts_the_boundary_and_rejects_one_more_pixel() {
        let boundary_pixels = 8_323_072;
        assert_eq!(
            estimated_peak_working_memory_bytes(boundary_pixels).unwrap(),
            DIFF_METRICS_PEAK_MEMORY_BUDGET_BYTES
        );
        let error = estimated_peak_working_memory_bytes(boundary_pixels + 1).unwrap_err();
        assert_eq!(error.failure.code, ComparisonErrorCode::ImageTooLarge);
        assert_eq!(error.exit_code(), ComparisonExitCode::InputFailure);
    }

    #[test]
    fn artifact_transaction_removes_temporary_and_partial_files_on_finalize_failure() {
        let temporary = tempfile::tempdir().unwrap();
        let first = temporary.path().join("first.bin");
        let blocked = temporary.path().join("blocked.bin");
        fs::create_dir(&blocked).unwrap();
        let error = write_transaction(vec![
            (first.clone(), vec![1, 2, 3]),
            (blocked.clone(), vec![4, 5, 6]),
        ])
        .unwrap_err();
        assert_eq!(error.failure.code, ComparisonErrorCode::ArtifactWriteFailed);
        assert!(!first.exists());
        assert!(!temporary.path().join("first.bin.tmp").exists());
        assert!(!temporary.path().join("blocked.bin.tmp").exists());
        assert!(blocked.is_dir());
    }

    #[test]
    fn artifact_transaction_never_clobbers_or_removes_an_existing_final_file() {
        let temporary = tempfile::tempdir().unwrap();
        let final_path = temporary.path().join("final.bin");
        fs::write(&final_path, b"other-writer").unwrap();
        let error =
            write_transaction(vec![(final_path.clone(), b"our-data".to_vec())]).unwrap_err();
        assert_eq!(error.failure.code, ComparisonErrorCode::ArtifactWriteFailed);
        assert_eq!(fs::read(&final_path).unwrap(), b"other-writer");
        assert!(!temporary.path().join("final.bin.tmp").exists());
    }

    #[test]
    fn artifact_transaction_never_removes_an_existing_temporary_file() {
        let temporary = tempfile::tempdir().unwrap();
        let final_path = temporary.path().join("final.bin");
        let temporary_path = temporary.path().join("final.bin.tmp");
        fs::write(&temporary_path, b"other-writer-temp").unwrap();
        let error =
            write_transaction(vec![(final_path.clone(), b"our-data".to_vec())]).unwrap_err();
        assert_eq!(error.failure.code, ComparisonErrorCode::ArtifactWriteFailed);
        assert_eq!(fs::read(&temporary_path).unwrap(), b"other-writer-temp");
        assert!(!final_path.exists());
    }
}
