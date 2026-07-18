use crate::PixelSize;
use image::{DynamicImage, ImageError, ImageFormat, ImageReader, Limits};
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    fmt, fs,
    io::{Cursor, Write},
    path::{Component, Path, PathBuf},
};

pub const COMPARISON_CONFIG_SCHEMA_VERSION: u32 = 1;
pub const COMPARISON_REPORT_SCHEMA_VERSION: u32 = 1;
pub const EXACT_RGBA_ALGORITHM_VERSION: &str = "exact_rgba_v1";
pub const COMPARISON_REPORT_FILENAME: &str = "comparison-report.json";

const MAX_CONFIG_BYTES: u64 = 64 * 1024;
const MAX_IMAGE_BYTES: u64 = 25 * 1024 * 1024;
const MAX_IMAGE_DIMENSION: u32 = 16_384;
const MAX_DECODE_ALLOC: u64 = 512 * 1024 * 1024;
const MAX_TOTAL_DECODED_PIXELS: u64 = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum ComparisonExitCode {
    Success = 0,
    InputFailure = 2,
    ComparisonFailure = 3,
    ThresholdFailure = 4,
    InternalError = 5,
}

impl ComparisonExitCode {
    pub const fn as_i32(self) -> i32 {
        self as i32
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureType {
    Input,
    Comparison,
    Threshold,
    Internal,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonErrorCode {
    CliArgumentsInvalid,
    RepositoryRootInvalid,
    AllowedInputRootInvalid,
    AllowedOutputRootInvalid,
    RootOutsideRepository,
    InputPathUnsafe,
    InputOutsideAllowedRoot,
    InputMissing,
    InputNotFile,
    ImageTooLarge,
    ImageUnsupportedFormat,
    ImageFormatMismatch,
    ImageCorrupt,
    ConfigTooLarge,
    ConfigReadFailed,
    ConfigParseFailed,
    ConfigInvalid,
    OutputPathUnsafe,
    OutputOutsideAllowedRoot,
    OutputNotDirectory,
    OutputDirectoryNotEmpty,
    ArtifactNameConflict,
    DimensionsMismatch,
    MaskDimensionsMismatch,
    MaskExcludesAllPixels,
    ThresholdExceeded,
    ArtifactWriteFailed,
    InternalFailure,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComparisonFailure {
    pub failure_type: FailureType,
    pub code: ComparisonErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl ComparisonFailure {
    fn new(
        failure_type: FailureType,
        code: ComparisonErrorCode,
        message: impl Into<String>,
    ) -> Self {
        Self {
            failure_type,
            code,
            message: message.into(),
            path: None,
        }
    }

    fn at_path(mut self, path: &Path) -> Self {
        self.path = Some(path.display().to_string());
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComparisonError {
    pub failure: ComparisonFailure,
}

impl ComparisonError {
    pub fn cli_arguments_invalid(message: impl Into<String>) -> Self {
        Self::input(
            ComparisonErrorCode::CliArgumentsInvalid,
            message.into().trim().to_owned(),
        )
    }

    pub fn exit_code(&self) -> ComparisonExitCode {
        match self.failure.failure_type {
            FailureType::Input => ComparisonExitCode::InputFailure,
            FailureType::Comparison => ComparisonExitCode::ComparisonFailure,
            FailureType::Threshold => ComparisonExitCode::ThresholdFailure,
            FailureType::Internal => ComparisonExitCode::InternalError,
        }
    }

    pub fn internal_failure(message: impl Into<String>) -> Self {
        Self::internal(ComparisonErrorCode::InternalFailure, message)
    }

    fn input(code: ComparisonErrorCode, message: impl Into<String>) -> Self {
        Self {
            failure: ComparisonFailure::new(FailureType::Input, code, message),
        }
    }

    fn internal(code: ComparisonErrorCode, message: impl Into<String>) -> Self {
        Self {
            failure: ComparisonFailure::new(FailureType::Internal, code, message),
        }
    }

    fn at_path(mut self, path: &Path) -> Self {
        self.failure = self.failure.at_path(path);
        self
    }
}

impl fmt::Display for ComparisonError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:?}: {}",
            self.failure.code, self.failure.message
        )
    }
}

impl Error for ComparisonError {}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComparisonConfig {
    pub schema_version: u32,
    pub algorithm_version: String,
    pub max_changed_pixel_ratio: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComparisonRequest {
    pub repository_root: PathBuf,
    pub allowed_input_roots: Vec<PathBuf>,
    pub allowed_output_root: PathBuf,
    pub reference: PathBuf,
    pub actual: PathBuf,
    pub config: PathBuf,
    pub mask: Option<PathBuf>,
    pub output_directory: PathBuf,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonStatus {
    Passed,
    ComparisonFailed,
    ThresholdFailed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ImageInputReport {
    pub path: String,
    pub format: String,
    pub dimensions: PixelSize,
    pub byte_length: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigInputReport {
    pub path: String,
    pub schema_version: u32,
    pub max_changed_pixel_ratio_millionths: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComparisonInputsReport {
    pub reference: ImageInputReport,
    pub actual: ImageInputReport,
    pub config: ConfigInputReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mask: Option<ImageInputReport>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DimensionsReport {
    pub reference: PixelSize,
    pub actual: PixelSize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mask: Option<PixelSize>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExactMetrics {
    pub evaluated_pixels: u64,
    pub changed_pixels: u64,
    pub changed_pixel_ratio_millionths: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RegionResult {
    pub region_id: String,
    pub origin_x: u32,
    pub origin_y: u32,
    pub width: u32,
    pub height: u32,
    pub metrics: ExactMetrics,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactReport {
    pub artifact_type: String,
    pub path: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComparisonReport {
    pub schema_version: u32,
    pub algorithm_version: String,
    pub status: ComparisonStatus,
    pub inputs: ComparisonInputsReport,
    pub dimensions: DimensionsReport,
    pub metrics: Option<ExactMetrics>,
    pub region_results: Vec<RegionResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<ComparisonFailure>,
    pub artifacts: Vec<ArtifactReport>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ComparisonOutcome {
    pub report: ComparisonReport,
    pub exit_code: ComparisonExitCode,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComparisonErrorResponse {
    pub schema_version: u32,
    pub status: String,
    pub failure: ComparisonFailure,
}

impl From<&ComparisonError> for ComparisonErrorResponse {
    fn from(error: &ComparisonError) -> Self {
        Self {
            schema_version: COMPARISON_REPORT_SCHEMA_VERSION,
            status: "error".to_owned(),
            failure: error.failure.clone(),
        }
    }
}

struct DecodedInput {
    report: ImageInputReport,
    image: DynamicImage,
}

pub fn compare_images(request: &ComparisonRequest) -> Result<ComparisonOutcome, ComparisonError> {
    let repository_root = canonical_directory(
        &request.repository_root,
        ComparisonErrorCode::RepositoryRootInvalid,
        "repository root",
    )?;
    let allowed_input_roots =
        resolve_allowed_input_roots(&repository_root, &request.allowed_input_roots)?;
    let allowed_output_root = resolve_allowed_root(
        &repository_root,
        &request.allowed_output_root,
        ComparisonErrorCode::AllowedOutputRootInvalid,
        "allowed output root",
    )?;

    let reference_path =
        resolve_input_file(&repository_root, &allowed_input_roots, &request.reference)?;
    let actual_path = resolve_input_file(&repository_root, &allowed_input_roots, &request.actual)?;
    let config_path = resolve_input_file(&repository_root, &allowed_input_roots, &request.config)?;
    let mask_path = request
        .mask
        .as_ref()
        .map(|path| resolve_input_file(&repository_root, &allowed_input_roots, path))
        .transpose()?;

    let config = load_config(&config_path)?;
    let reference = decode_image(&reference_path)?;
    let actual = decode_image(&actual_path)?;
    let mask = mask_path.as_deref().map(decode_image).transpose()?;
    validate_total_pixel_budget(
        [
            Some(reference.report.dimensions),
            Some(actual.report.dimensions),
            mask.as_ref().map(|input| input.report.dimensions),
        ]
        .into_iter()
        .flatten(),
    )?;

    let output_directory = create_output_directory(
        &repository_root,
        &allowed_output_root,
        &request.output_directory,
    )?;
    let report_path = output_directory.join(COMPARISON_REPORT_FILENAME);
    for input in [
        Some(reference_path.as_path()),
        Some(actual_path.as_path()),
        Some(config_path.as_path()),
        mask_path.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if report_path == input {
            return Err(ComparisonError::input(
                ComparisonErrorCode::ArtifactNameConflict,
                "comparison report would overwrite an input file",
            )
            .at_path(&report_path));
        }
    }

    let dimensions = DimensionsReport {
        reference: reference.report.dimensions,
        actual: actual.report.dimensions,
        mask: mask.as_ref().map(|input| input.report.dimensions),
    };
    let inputs = ComparisonInputsReport {
        reference: reference.report.clone(),
        actual: actual.report.clone(),
        config: ConfigInputReport {
            path: config_path.display().to_string(),
            schema_version: config.schema_version,
            max_changed_pixel_ratio_millionths: ratio_to_millionths(config.max_changed_pixel_ratio),
        },
        mask: mask.as_ref().map(|input| input.report.clone()),
    };
    let artifacts = vec![ArtifactReport {
        artifact_type: "comparison_report".to_owned(),
        path: report_path.display().to_string(),
    }];

    let outcome = if dimensions.reference != dimensions.actual {
        comparison_failure_report(
            inputs,
            dimensions,
            artifacts,
            ComparisonErrorCode::DimensionsMismatch,
            "reference and actual dimensions must match before comparison",
        )
    } else if dimensions
        .mask
        .is_some_and(|mask_size| mask_size != dimensions.reference)
    {
        comparison_failure_report(
            inputs,
            dimensions,
            artifacts,
            ComparisonErrorCode::MaskDimensionsMismatch,
            "mask dimensions must match reference and actual dimensions",
        )
    } else {
        compare_exact_rgba(
            reference, actual, mask, config, inputs, dimensions, artifacts,
        )
    };

    persist_report(&report_path, &outcome.report)?;
    Ok(outcome)
}

fn compare_exact_rgba(
    reference: DecodedInput,
    actual: DecodedInput,
    mask: Option<DecodedInput>,
    config: ComparisonConfig,
    inputs: ComparisonInputsReport,
    dimensions: DimensionsReport,
    artifacts: Vec<ArtifactReport>,
) -> ComparisonOutcome {
    let reference = reference.image.into_rgba8();
    let actual = actual.image.into_rgba8();
    let mask = mask.map(|input| input.image.into_rgba8());
    let mut evaluated_pixels = 0_u64;
    let mut changed_pixels = 0_u64;

    if let Some(mask) = &mask {
        for ((reference_pixel, actual_pixel), mask_pixel) in
            reference.pixels().zip(actual.pixels()).zip(mask.pixels())
        {
            if mask_pixel_is_included(mask_pixel.0) {
                evaluated_pixels += 1;
                if reference_pixel.0 != actual_pixel.0 {
                    changed_pixels += 1;
                }
            }
        }
    } else {
        for (reference_pixel, actual_pixel) in reference.pixels().zip(actual.pixels()) {
            evaluated_pixels += 1;
            if reference_pixel.0 != actual_pixel.0 {
                changed_pixels += 1;
            }
        }
    }

    if evaluated_pixels == 0 {
        return comparison_failure_report(
            inputs,
            dimensions,
            artifacts,
            ComparisonErrorCode::MaskExcludesAllPixels,
            "mask excludes every pixel from comparison",
        );
    }

    let ratio = changed_pixels as f64 / evaluated_pixels as f64;
    let metrics = ExactMetrics {
        evaluated_pixels,
        changed_pixels,
        changed_pixel_ratio_millionths: ratio_to_millionths(ratio),
    };
    let threshold_failed = ratio > config.max_changed_pixel_ratio;
    let failure = threshold_failed.then(|| {
        ComparisonFailure::new(
            FailureType::Threshold,
            ComparisonErrorCode::ThresholdExceeded,
            format!(
                "changed pixel ratio {ratio:.6} exceeds configured maximum {:.6}",
                config.max_changed_pixel_ratio
            ),
        )
    });
    let region = RegionResult {
        region_id: "full_image".to_owned(),
        origin_x: 0,
        origin_y: 0,
        width: dimensions.reference.width,
        height: dimensions.reference.height,
        metrics: metrics.clone(),
    };

    ComparisonOutcome {
        report: ComparisonReport {
            schema_version: COMPARISON_REPORT_SCHEMA_VERSION,
            algorithm_version: EXACT_RGBA_ALGORITHM_VERSION.to_owned(),
            status: if threshold_failed {
                ComparisonStatus::ThresholdFailed
            } else {
                ComparisonStatus::Passed
            },
            inputs,
            dimensions,
            metrics: Some(metrics),
            region_results: vec![region],
            failure,
            artifacts,
        },
        exit_code: if threshold_failed {
            ComparisonExitCode::ThresholdFailure
        } else {
            ComparisonExitCode::Success
        },
    }
}

fn comparison_failure_report(
    inputs: ComparisonInputsReport,
    dimensions: DimensionsReport,
    artifacts: Vec<ArtifactReport>,
    code: ComparisonErrorCode,
    message: impl Into<String>,
) -> ComparisonOutcome {
    ComparisonOutcome {
        report: ComparisonReport {
            schema_version: COMPARISON_REPORT_SCHEMA_VERSION,
            algorithm_version: EXACT_RGBA_ALGORITHM_VERSION.to_owned(),
            status: ComparisonStatus::ComparisonFailed,
            inputs,
            dimensions,
            metrics: None,
            region_results: Vec::new(),
            failure: Some(ComparisonFailure::new(
                FailureType::Comparison,
                code,
                message,
            )),
            artifacts,
        },
        exit_code: ComparisonExitCode::ComparisonFailure,
    }
}

fn resolve_allowed_input_roots(
    repository_root: &Path,
    requested: &[PathBuf],
) -> Result<Vec<PathBuf>, ComparisonError> {
    if requested.is_empty() {
        return Err(ComparisonError::input(
            ComparisonErrorCode::AllowedInputRootInvalid,
            "at least one allowed input root is required",
        ));
    }
    requested
        .iter()
        .map(|root| {
            resolve_allowed_root(
                repository_root,
                root,
                ComparisonErrorCode::AllowedInputRootInvalid,
                "allowed input root",
            )
        })
        .collect()
}

fn resolve_allowed_root(
    repository_root: &Path,
    requested: &Path,
    invalid_code: ComparisonErrorCode,
    label: &str,
) -> Result<PathBuf, ComparisonError> {
    let candidate = resolve_from_repository(repository_root, requested)?;
    let canonical = canonical_directory(&candidate, invalid_code, label)?;
    if !canonical.starts_with(repository_root) {
        return Err(ComparisonError::input(
            ComparisonErrorCode::RootOutsideRepository,
            format!("{label} resolves outside the repository root"),
        )
        .at_path(&canonical));
    }
    Ok(canonical)
}

fn canonical_directory(
    path: &Path,
    code: ComparisonErrorCode,
    label: &str,
) -> Result<PathBuf, ComparisonError> {
    let canonical = fs::canonicalize(path).map_err(|error| {
        ComparisonError::input(code, format!("{label} cannot be resolved: {error}")).at_path(path)
    })?;
    if !canonical.is_dir() {
        return Err(
            ComparisonError::input(code, format!("{label} is not a directory")).at_path(&canonical),
        );
    }
    Ok(canonical)
}

fn resolve_from_repository(
    repository_root: &Path,
    requested: &Path,
) -> Result<PathBuf, ComparisonError> {
    if requested
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(ComparisonError::input(
            ComparisonErrorCode::InputPathUnsafe,
            "paths cannot contain parent traversal",
        )
        .at_path(requested));
    }
    Ok(if requested.is_absolute() {
        requested.to_owned()
    } else {
        repository_root.join(requested)
    })
}

fn resolve_input_file(
    repository_root: &Path,
    allowed_roots: &[PathBuf],
    requested: &Path,
) -> Result<PathBuf, ComparisonError> {
    let candidate = resolve_from_repository(repository_root, requested)?;
    let canonical = fs::canonicalize(&candidate).map_err(|error| {
        ComparisonError::input(
            ComparisonErrorCode::InputMissing,
            format!("input file cannot be resolved: {error}"),
        )
        .at_path(&candidate)
    })?;
    if !allowed_roots.iter().any(|root| canonical.starts_with(root)) {
        return Err(ComparisonError::input(
            ComparisonErrorCode::InputOutsideAllowedRoot,
            "input resolves outside every allowed input root",
        )
        .at_path(&canonical));
    }
    if !canonical.is_file() {
        return Err(ComparisonError::input(
            ComparisonErrorCode::InputNotFile,
            "input path is not a regular file",
        )
        .at_path(&canonical));
    }
    Ok(canonical)
}

fn create_output_directory(
    repository_root: &Path,
    allowed_root: &Path,
    requested: &Path,
) -> Result<PathBuf, ComparisonError> {
    if requested
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(ComparisonError::input(
            ComparisonErrorCode::OutputPathUnsafe,
            "output directory cannot contain parent traversal",
        )
        .at_path(requested));
    }
    let candidate = if requested.is_absolute() {
        requested.to_owned()
    } else {
        repository_root.join(requested)
    };
    let mut existing_ancestor = candidate.as_path();
    while !existing_ancestor.exists() {
        existing_ancestor = existing_ancestor.parent().ok_or_else(|| {
            ComparisonError::input(
                ComparisonErrorCode::OutputPathUnsafe,
                "output directory has no resolvable ancestor",
            )
            .at_path(&candidate)
        })?;
    }
    let canonical_ancestor = fs::canonicalize(existing_ancestor).map_err(|error| {
        ComparisonError::input(
            ComparisonErrorCode::AllowedOutputRootInvalid,
            format!("output directory ancestor cannot be resolved: {error}"),
        )
        .at_path(existing_ancestor)
    })?;
    if !canonical_ancestor.starts_with(allowed_root) {
        return Err(ComparisonError::input(
            ComparisonErrorCode::OutputOutsideAllowedRoot,
            "output directory resolves outside the allowed output root",
        )
        .at_path(&canonical_ancestor));
    }

    if candidate.exists() {
        if !candidate.is_dir() {
            return Err(ComparisonError::input(
                ComparisonErrorCode::OutputNotDirectory,
                "output path exists but is not a directory",
            )
            .at_path(&candidate));
        }
        let entries = fs::read_dir(&candidate).map_err(|error| {
            ComparisonError::input(
                ComparisonErrorCode::AllowedOutputRootInvalid,
                format!("output directory cannot be inspected: {error}"),
            )
            .at_path(&candidate)
        })?;
        let entries = entries.collect::<Result<Vec<_>, _>>().map_err(|error| {
            ComparisonError::input(
                ComparisonErrorCode::AllowedOutputRootInvalid,
                format!("output directory cannot be inspected: {error}"),
            )
            .at_path(&candidate)
        })?;
        let reserved_name_exists = entries.iter().any(|entry| {
            matches!(
                entry.file_name().to_str(),
                Some(COMPARISON_REPORT_FILENAME | "comparison-report.json.tmp")
            )
        });
        if reserved_name_exists {
            return Err(ComparisonError::input(
                ComparisonErrorCode::ArtifactNameConflict,
                "output directory already contains a reserved comparison report name",
            )
            .at_path(&candidate));
        }
        if !entries.is_empty() {
            return Err(ComparisonError::input(
                ComparisonErrorCode::OutputDirectoryNotEmpty,
                "output directory must be empty to prevent artifact overwrite",
            )
            .at_path(&candidate));
        }
    } else {
        fs::create_dir_all(&candidate).map_err(|error| {
            ComparisonError::input(
                ComparisonErrorCode::AllowedOutputRootInvalid,
                format!("output directory cannot be created: {error}"),
            )
            .at_path(&candidate)
        })?;
    }

    let canonical = fs::canonicalize(&candidate).map_err(|error| {
        ComparisonError::input(
            ComparisonErrorCode::AllowedOutputRootInvalid,
            format!("output directory cannot be resolved after creation: {error}"),
        )
        .at_path(&candidate)
    })?;
    if !canonical.starts_with(allowed_root) {
        return Err(ComparisonError::input(
            ComparisonErrorCode::OutputOutsideAllowedRoot,
            "output directory resolves outside the allowed output root",
        )
        .at_path(&canonical));
    }
    Ok(canonical)
}

fn load_config(path: &Path) -> Result<ComparisonConfig, ComparisonError> {
    let metadata = fs::metadata(path).map_err(|error| {
        ComparisonError::input(
            ComparisonErrorCode::ConfigReadFailed,
            format!("comparison config metadata cannot be read: {error}"),
        )
        .at_path(path)
    })?;
    if metadata.len() > MAX_CONFIG_BYTES {
        return Err(ComparisonError::input(
            ComparisonErrorCode::ConfigTooLarge,
            format!("comparison config exceeds the {MAX_CONFIG_BYTES}-byte limit"),
        )
        .at_path(path));
    }
    let bytes = fs::read(path).map_err(|error| {
        ComparisonError::input(
            ComparisonErrorCode::ConfigReadFailed,
            format!("comparison config cannot be read: {error}"),
        )
        .at_path(path)
    })?;
    let config: ComparisonConfig = serde_json::from_slice(&bytes).map_err(|error| {
        ComparisonError::input(
            ComparisonErrorCode::ConfigParseFailed,
            format!("comparison config is not valid schema JSON: {error}"),
        )
        .at_path(path)
    })?;
    if config.schema_version != COMPARISON_CONFIG_SCHEMA_VERSION {
        return Err(ComparisonError::input(
            ComparisonErrorCode::ConfigInvalid,
            format!(
                "config schema_version must be {COMPARISON_CONFIG_SCHEMA_VERSION}, got {}",
                config.schema_version
            ),
        )
        .at_path(path));
    }
    if config.algorithm_version != EXACT_RGBA_ALGORITHM_VERSION {
        return Err(ComparisonError::input(
            ComparisonErrorCode::ConfigInvalid,
            format!(
                "algorithm_version must be {EXACT_RGBA_ALGORITHM_VERSION}, got {}",
                config.algorithm_version
            ),
        )
        .at_path(path));
    }
    if !config.max_changed_pixel_ratio.is_finite()
        || !(0.0..=1.0).contains(&config.max_changed_pixel_ratio)
    {
        return Err(ComparisonError::input(
            ComparisonErrorCode::ConfigInvalid,
            "max_changed_pixel_ratio must be finite and between 0 and 1",
        )
        .at_path(path));
    }
    Ok(config)
}

fn decode_image(path: &Path) -> Result<DecodedInput, ComparisonError> {
    let metadata = fs::metadata(path).map_err(|error| {
        ComparisonError::input(
            ComparisonErrorCode::InputMissing,
            format!("image metadata cannot be read: {error}"),
        )
        .at_path(path)
    })?;
    if metadata.len() > MAX_IMAGE_BYTES {
        return Err(ComparisonError::input(
            ComparisonErrorCode::ImageTooLarge,
            format!("image exceeds the {MAX_IMAGE_BYTES}-byte limit"),
        )
        .at_path(path));
    }
    let bytes = fs::read(path).map_err(|error| {
        ComparisonError::input(
            ComparisonErrorCode::InputMissing,
            format!("image cannot be read: {error}"),
        )
        .at_path(path)
    })?;
    let format = detect_image_format(path, &bytes)?;
    let mut reader = ImageReader::with_format(Cursor::new(&bytes), format);
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
        ComparisonError::input(code, format!("image cannot be decoded: {error}")).at_path(path)
    })?;
    let dimensions = PixelSize {
        width: image.width(),
        height: image.height(),
    };
    Ok(DecodedInput {
        report: ImageInputReport {
            path: path.display().to_string(),
            format: image_format_label(format).to_owned(),
            dimensions,
            byte_length: metadata.len(),
        },
        image,
    })
}

fn validate_total_pixel_budget(
    dimensions: impl IntoIterator<Item = PixelSize>,
) -> Result<(), ComparisonError> {
    let total = dimensions.into_iter().try_fold(0_u64, |total, size| {
        let pixels = u64::from(size.width)
            .checked_mul(u64::from(size.height))
            .ok_or_else(|| {
                ComparisonError::input(
                    ComparisonErrorCode::ImageTooLarge,
                    "decoded image pixel count overflowed the comparison budget",
                )
            })?;
        total.checked_add(pixels).ok_or_else(|| {
            ComparisonError::input(
                ComparisonErrorCode::ImageTooLarge,
                "combined decoded image pixel count overflowed the comparison budget",
            )
        })
    })?;
    if total > MAX_TOTAL_DECODED_PIXELS {
        return Err(ComparisonError::input(
            ComparisonErrorCode::ImageTooLarge,
            format!(
                "combined decoded images exceed the {MAX_TOTAL_DECODED_PIXELS}-pixel comparison budget"
            ),
        ));
    }
    Ok(())
}

fn detect_image_format(path: &Path, bytes: &[u8]) -> Result<ImageFormat, ComparisonError> {
    let declared = match path.extension().and_then(|value| value.to_str()) {
        Some("png") => ImageFormat::Png,
        Some("jpg" | "jpeg") => ImageFormat::Jpeg,
        _ => {
            return Err(ComparisonError::input(
                ComparisonErrorCode::ImageUnsupportedFormat,
                "image extension must be lowercase .png, .jpg, or .jpeg",
            )
            .at_path(path));
        }
    };
    let detected = image::guess_format(bytes).map_err(|error| {
        ComparisonError::input(
            ComparisonErrorCode::ImageCorrupt,
            format!("image format cannot be detected: {error}"),
        )
        .at_path(path)
    })?;
    if !matches!(detected, ImageFormat::Png | ImageFormat::Jpeg) {
        return Err(ComparisonError::input(
            ComparisonErrorCode::ImageUnsupportedFormat,
            "image content must be PNG or JPEG",
        )
        .at_path(path));
    }
    if declared != detected {
        return Err(ComparisonError::input(
            ComparisonErrorCode::ImageFormatMismatch,
            "image extension does not match encoded content",
        )
        .at_path(path));
    }
    Ok(detected)
}

fn image_format_label(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Png => "png",
        ImageFormat::Jpeg => "jpeg",
        _ => "unsupported",
    }
}

fn persist_report(path: &Path, report: &ComparisonReport) -> Result<(), ComparisonError> {
    let temporary = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(report).map_err(|error| {
        ComparisonError::internal(
            ComparisonErrorCode::InternalFailure,
            format!("comparison report cannot be serialized: {error}"),
        )
    })?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| {
            ComparisonError::internal(
                ComparisonErrorCode::ArtifactWriteFailed,
                format!("temporary comparison report cannot be created: {error}"),
            )
            .at_path(&temporary)
        })?;
    if let Err(error) = file.write_all(&bytes).and_then(|_| file.flush()) {
        drop(file);
        let _ = fs::remove_file(&temporary);
        return Err(ComparisonError::internal(
            ComparisonErrorCode::ArtifactWriteFailed,
            format!("temporary comparison report cannot be written: {error}"),
        )
        .at_path(&temporary));
    }
    drop(file);
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(ComparisonError::internal(
            ComparisonErrorCode::ArtifactWriteFailed,
            format!("comparison report cannot be finalized: {error}"),
        )
        .at_path(path));
    }
    Ok(())
}

fn mask_pixel_is_included(pixel: [u8; 4]) -> bool {
    pixel[3] != 0 && pixel[0..3].iter().any(|channel| *channel != 0)
}

fn ratio_to_millionths(ratio: f64) -> u32 {
    (ratio * 1_000_000.0).round() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_exit_codes_cover_every_failure_class() {
        assert_eq!(ComparisonExitCode::Success.as_i32(), 0);
        assert_eq!(ComparisonExitCode::InputFailure.as_i32(), 2);
        assert_eq!(ComparisonExitCode::ComparisonFailure.as_i32(), 3);
        assert_eq!(ComparisonExitCode::ThresholdFailure.as_i32(), 4);
        assert_eq!(ComparisonExitCode::InternalError.as_i32(), 5);

        let internal = ComparisonError::internal(
            ComparisonErrorCode::InternalFailure,
            "injected test failure",
        );
        assert_eq!(internal.exit_code(), ComparisonExitCode::InternalError);
        assert_eq!(
            serde_json::to_value(ComparisonErrorResponse::from(&internal)).unwrap()["failure"]["failure_type"],
            "internal"
        );
    }

    #[test]
    fn ratio_serialization_uses_deterministic_integer_millionths() {
        assert_eq!(ratio_to_millionths(0.0), 0);
        assert_eq!(ratio_to_millionths(1.0 / 3.0), 333_333);
        assert_eq!(ratio_to_millionths(1.0), 1_000_000);
    }

    #[test]
    fn combined_pixel_budget_accounts_for_reference_actual_and_mask() {
        validate_total_pixel_budget([
            PixelSize {
                width: 1280,
                height: 2772,
            },
            PixelSize {
                width: 1280,
                height: 2772,
            },
            PixelSize {
                width: 1280,
                height: 2772,
            },
        ])
        .unwrap();
        let error = validate_total_pixel_budget([
            PixelSize {
                width: 16_384,
                height: 16_384,
            },
            PixelSize {
                width: 1,
                height: 1,
            },
        ])
        .unwrap_err();
        assert_eq!(error.failure.code, ComparisonErrorCode::ImageTooLarge);
    }

    #[test]
    fn mask_contract_requires_visible_non_black_pixels() {
        assert!(!mask_pixel_is_included([255, 255, 255, 0]));
        assert!(!mask_pixel_is_included([0, 0, 0, 255]));
        assert!(mask_pixel_is_included([1, 0, 0, 255]));
    }

    #[test]
    fn artifact_write_failure_is_internal_and_never_panics() {
        let temporary = tempfile::tempdir().unwrap();
        let missing_parent = temporary.path().join("missing");
        let report_path = missing_parent.join(COMPARISON_REPORT_FILENAME);
        let placeholder_image = ImageInputReport {
            path: temporary.path().join("input.png").display().to_string(),
            format: "png".to_owned(),
            dimensions: PixelSize {
                width: 1,
                height: 1,
            },
            byte_length: 1,
        };
        let report = ComparisonReport {
            schema_version: COMPARISON_REPORT_SCHEMA_VERSION,
            algorithm_version: EXACT_RGBA_ALGORITHM_VERSION.to_owned(),
            status: ComparisonStatus::Passed,
            inputs: ComparisonInputsReport {
                reference: placeholder_image.clone(),
                actual: placeholder_image,
                config: ConfigInputReport {
                    path: temporary.path().join("config.json").display().to_string(),
                    schema_version: COMPARISON_CONFIG_SCHEMA_VERSION,
                    max_changed_pixel_ratio_millionths: 0,
                },
                mask: None,
            },
            dimensions: DimensionsReport {
                reference: PixelSize {
                    width: 1,
                    height: 1,
                },
                actual: PixelSize {
                    width: 1,
                    height: 1,
                },
                mask: None,
            },
            metrics: Some(ExactMetrics {
                evaluated_pixels: 1,
                changed_pixels: 0,
                changed_pixel_ratio_millionths: 0,
            }),
            region_results: Vec::new(),
            failure: None,
            artifacts: Vec::new(),
        };

        let error = persist_report(&report_path, &report).unwrap_err();
        assert_eq!(error.failure.code, ComparisonErrorCode::ArtifactWriteFailed);
        assert_eq!(error.exit_code(), ComparisonExitCode::InternalError);
    }
}
