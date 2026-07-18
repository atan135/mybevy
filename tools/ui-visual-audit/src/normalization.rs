use crate::{
    ArtifactReport, ComparisonError, ComparisonErrorCode, ComparisonExitCode, ComparisonFailure,
    FailureType, ImageInputReport, PixelSize,
    comparison::{
        create_output_directory, resolve_allowed_input_roots, resolve_allowed_root,
        resolve_input_file,
    },
};
use image::{
    ExtendedColorType, ImageEncoder, ImageError, ImageFormat, ImageReader, Limits, RgbaImage,
    codecs::png::PngEncoder, imageops,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs,
    io::{Cursor, Write},
    path::{Path, PathBuf},
};

pub const NORMALIZATION_MANIFEST_SCHEMA_VERSION: u32 = 1;
pub const NORMALIZATION_REPORT_SCHEMA_VERSION: u32 = 1;
pub const NORMALIZE_ALIGN_ALGORITHM_VERSION: &str = "normalize_align_v1";
pub const NORMALIZATION_REPORT_FILENAME: &str = "normalization-report.json";

const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
const MAX_IMAGE_BYTES: u64 = 25 * 1024 * 1024;
const MAX_IMAGE_DIMENSION: u32 = 16_384;
const MAX_DECODE_ALLOC: u64 = 512 * 1024 * 1024;
const MAX_TOTAL_DECODED_PIXELS: u64 = 64 * 1024 * 1024;
const HARD_MAX_TRANSLATION: u32 = 16;
const MIN_SCREENSHOT_PIXELS: u64 = 4;
const NEAR_BLANK_MIN_PIXELS: u64 = 64;
const NEAR_BLANK_SAMPLE_LIMIT: usize = 4_096;
const NEAR_BLANK_DOMINANT_MILLIONTHS: u32 = 999_000;

const NORMALIZED_REFERENCE_FILENAME: &str = "normalized-reference.png";
const NORMALIZED_ACTUAL_FILENAME: &str = "normalized-actual.png";
const CROPPED_REFERENCE_FILENAME: &str = "cropped-reference.png";
const CROPPED_ACTUAL_FILENAME: &str = "cropped-actual.png";
const ALIGNED_REFERENCE_FILENAME: &str = "aligned-reference.png";
const ALIGNED_ACTUAL_FILENAME: &str = "aligned-actual.png";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizationManifest {
    pub schema_version: u32,
    pub algorithm_version: String,
    pub orientation_policy: OrientationPolicy,
    pub color_policy: ColorPolicy,
    pub alpha_policy: AlphaPolicy,
    pub reference: ImageRoleManifest,
    pub actual: ImageRoleManifest,
    pub alignment: AlignmentManifest,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OrientationPolicy {
    ApplyExif,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorPolicy {
    SrgbOnly,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AlphaPolicy {
    StraightZeroTransparentRgb,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ImageRoleManifest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_sha256: Option<String>,
    pub crop: CropDeclaration,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CropDeclaration {
    None,
    SystemUi {
        left: u32,
        top: u32,
        right: u32,
        bottom: u32,
    },
    SafeArea {
        left: u32,
        top: u32,
        right: u32,
        bottom: u32,
    },
    FixedBorder {
        left: u32,
        top: u32,
        right: u32,
        bottom: u32,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CropKind {
    None,
    SystemUi,
    SafeArea,
    FixedBorder,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AlignmentManifest {
    pub mode: AlignmentMode,
    pub maximum_translation: IntegerOffsetLimit,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_translation: Option<IntegerOffset>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AlignmentMode {
    None,
    IntegerSearch,
    DeclaredInteger,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntegerOffsetLimit {
    pub x: u32,
    pub y: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IntegerOffset {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NormalizationRequest {
    pub repository_root: PathBuf,
    pub allowed_input_roots: Vec<PathBuf>,
    pub allowed_output_root: PathBuf,
    pub reference: PathBuf,
    pub actual: PathBuf,
    pub normalization_manifest: PathBuf,
    pub output_directory: PathBuf,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NormalizationStatus {
    Passed,
    ComparisonFailed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestInputReport {
    pub path: String,
    pub schema_version: u32,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizationInputsReport {
    pub reference: ImageInputReport,
    pub actual: ImageInputReport,
    pub normalization_manifest: ManifestInputReport,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CropReport {
    pub kind: CropKind,
    pub left: u32,
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
    pub before_dimensions: PixelSize,
    pub after_dimensions: PixelSize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QualityReport {
    pub total_pixels: u64,
    pub nontransparent_pixels: u64,
    pub sampled_pixels: u32,
    pub dominant_sample_ratio_millionths: u32,
    pub all_transparent: bool,
    pub near_blank: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PixelRect {
    pub x: i64,
    pub y: i64,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AffineTransform {
    pub xx: i32,
    pub xy: i32,
    pub x_offset: i64,
    pub yx: i32,
    pub yy: i32,
    pub y_offset: i64,
}

impl AffineTransform {
    pub fn map_point(self, x: i64, y: i64) -> (i64, i64) {
        (
            i64::from(self.xx) * x + i64::from(self.xy) * y + self.x_offset,
            i64::from(self.yx) * x + i64::from(self.yy) * y + self.y_offset,
        )
    }

    pub fn map_rect(self, rect: PixelRect) -> PixelRect {
        let right = rect.x + i64::from(rect.width);
        let bottom = rect.y + i64::from(rect.height);
        let points = [
            self.map_point(rect.x, rect.y),
            self.map_point(right, rect.y),
            self.map_point(rect.x, bottom),
            self.map_point(right, bottom),
        ];
        let min_x = points.iter().map(|point| point.0).min().unwrap_or(0);
        let max_x = points.iter().map(|point| point.0).max().unwrap_or(0);
        let min_y = points.iter().map(|point| point.1).min().unwrap_or(0);
        let max_y = points.iter().map(|point| point.1).max().unwrap_or(0);
        PixelRect {
            x: min_x,
            y: min_y,
            width: u32::try_from(max_x - min_x).unwrap_or(u32::MAX),
            height: u32::try_from(max_y - min_y).unwrap_or(u32::MAX),
        }
    }

    fn translated(self, x: i64, y: i64) -> Self {
        Self {
            x_offset: self.x_offset + x,
            y_offset: self.y_offset + y,
            ..self
        }
    }

    fn inverse(self) -> Self {
        let determinant = self.xx * self.yy - self.xy * self.yx;
        debug_assert!(matches!(determinant, -1 | 1));
        let xx = self.yy / determinant;
        let xy = -self.xy / determinant;
        let yx = -self.yx / determinant;
        let yy = self.xx / determinant;
        Self {
            xx,
            xy,
            x_offset: -(i64::from(xx) * self.x_offset + i64::from(xy) * self.y_offset),
            yx,
            yy,
            y_offset: -(i64::from(yx) * self.x_offset + i64::from(yy) * self.y_offset),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CoordinateMapping {
    pub original_to_aligned: AffineTransform,
    pub aligned_to_original: AffineTransform,
    pub valid_original_bounds: PixelRect,
    pub valid_aligned_bounds: PixelRect,
}

impl CoordinateMapping {
    pub fn map_original_rect_to_aligned(&self, bounds: PixelRect) -> PixelRect {
        self.original_to_aligned.map_rect(bounds)
    }

    pub fn map_aligned_rect_to_original(&self, bounds: PixelRect) -> PixelRect {
        self.aligned_to_original.map_rect(bounds)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InputNormalizationReport {
    pub sha256: String,
    pub original_dimensions: PixelSize,
    pub exif_orientation: u16,
    pub orientation_operation: String,
    pub oriented_dimensions: PixelSize,
    pub source_color_space: String,
    pub output_color_space: String,
    pub source_alpha: String,
    pub output_alpha: String,
    pub pixel_format: String,
    pub crop: CropReport,
    pub quality: QualityReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aligned_dimensions: Option<PixelSize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coordinate_mapping: Option<CoordinateMapping>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AlignmentReport {
    pub mode: AlignmentMode,
    pub maximum_translation: IntegerOffsetLimit,
    pub selected_translation: IntegerOffset,
    pub scale_x_millionths: u32,
    pub scale_y_millionths: u32,
    pub before_dimensions: PixelSize,
    pub aligned_dimensions: PixelSize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mean_absolute_channel_error_millionths: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NormalizationReport {
    pub schema_version: u32,
    pub algorithm_version: String,
    pub status: NormalizationStatus,
    pub inputs: NormalizationInputsReport,
    pub reference: InputNormalizationReport,
    pub actual: InputNormalizationReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alignment: Option<AlignmentReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<ComparisonFailure>,
    pub artifacts: Vec<ArtifactReport>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NormalizationOutcome {
    pub report: NormalizationReport,
    pub exit_code: ComparisonExitCode,
}

struct DecodedImage {
    report: ImageInputReport,
    sha256: String,
    exif_orientation: u16,
    source_color_space: String,
    source_alpha: String,
    image: RgbaImage,
}

struct PreparedImage {
    full: RgbaImage,
    cropped: RgbaImage,
    report: InputNormalizationReport,
    original_to_cropped: AffineTransform,
}

pub fn normalize_and_align(
    request: &NormalizationRequest,
) -> Result<NormalizationOutcome, ComparisonError> {
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
    let manifest_path = resolve_input_file(
        &repository_root,
        &input_roots,
        &request.normalization_manifest,
    )?;
    let (manifest, manifest_sha256) = load_manifest(&manifest_path)?;
    let reference = decode_and_normalize(&reference_path)?;
    let actual = decode_and_normalize(&actual_path)?;
    validate_total_pixel_budget([reference.report.dimensions, actual.report.dimensions])?;

    let output_directory =
        create_output_directory(&repository_root, &output_root, &request.output_directory)?;
    let mut artifacts = Vec::new();
    let inputs = NormalizationInputsReport {
        reference: reference.report.clone(),
        actual: actual.report.clone(),
        normalization_manifest: ManifestInputReport {
            path: manifest_path.display().to_string(),
            schema_version: manifest.schema_version,
            sha256: manifest_sha256,
        },
    };

    let mut reference = prepare_image(reference, &manifest.reference.crop)?;
    let mut actual = prepare_image(actual, &manifest.actual.crop)?;
    persist_image_set(
        &output_directory,
        [
            (NORMALIZED_REFERENCE_FILENAME, &reference.full),
            (NORMALIZED_ACTUAL_FILENAME, &actual.full),
            (CROPPED_REFERENCE_FILENAME, &reference.cropped),
            (CROPPED_ACTUAL_FILENAME, &actual.cropped),
        ],
    )?;
    for (artifact_type, filename) in [
        ("normalized_reference", NORMALIZED_REFERENCE_FILENAME),
        ("normalized_actual", NORMALIZED_ACTUAL_FILENAME),
        ("cropped_reference", CROPPED_REFERENCE_FILENAME),
        ("cropped_actual", CROPPED_ACTUAL_FILENAME),
    ] {
        artifacts.push(artifact(&output_directory, artifact_type, filename));
    }

    if let Some((code, message)) = identity_failure(&manifest, &reference.report, &actual.report) {
        return finish_failure(
            &output_directory,
            inputs,
            reference.report,
            actual.report,
            None,
            artifacts,
            (code, message),
        );
    }
    if let Some((code, message)) =
        quality_failure(&reference.report.quality, &actual.report.quality)
    {
        return finish_failure(
            &output_directory,
            inputs,
            reference.report,
            actual.report,
            None,
            artifacts,
            (code, message),
        );
    }

    let reference_size = size_of(&reference.cropped);
    let actual_size = size_of(&actual.cropped);
    if reference_size != actual_size {
        let (code, message) = if u64::from(reference_size.width) * u64::from(actual_size.height)
            != u64::from(actual_size.width) * u64::from(reference_size.height)
        {
            (
                ComparisonErrorCode::AspectRatioMismatch,
                "cropped reference and actual aspect ratios differ; resize and stretch are forbidden",
            )
        } else {
            (
                ComparisonErrorCode::DimensionsMismatch,
                "cropped reference and actual physical dimensions differ; resize and stretch are forbidden",
            )
        };
        return finish_failure(
            &output_directory,
            inputs,
            reference.report,
            actual.report,
            None,
            artifacts,
            (code, message),
        );
    }

    let selected = match manifest.alignment.mode {
        AlignmentMode::None => IntegerOffset { x: 0, y: 0 },
        AlignmentMode::IntegerSearch => search_translation(
            &reference.cropped,
            &actual.cropped,
            manifest.alignment.maximum_translation,
        ),
        AlignmentMode::DeclaredInteger => manifest
            .alignment
            .declared_translation
            .expect("validated declared translation"),
    };
    if selected.x.unsigned_abs() > manifest.alignment.maximum_translation.x
        || selected.y.unsigned_abs() > manifest.alignment.maximum_translation.y
        || selected.x.unsigned_abs() >= reference_size.width
        || selected.y.unsigned_abs() >= reference_size.height
    {
        return finish_failure(
            &output_directory,
            inputs,
            reference.report,
            actual.report,
            None,
            artifacts,
            (
                ComparisonErrorCode::MaximumTranslationExceeded,
                format!(
                    "requested translation ({}, {}) exceeds configured maximum ({}, {})",
                    selected.x,
                    selected.y,
                    manifest.alignment.maximum_translation.x,
                    manifest.alignment.maximum_translation.y
                ),
            ),
        );
    }

    let (aligned_reference, aligned_actual, reference_origin, actual_origin) =
        apply_translation(&reference.cropped, &actual.cropped, selected);
    let aligned_size = size_of(&aligned_reference);
    let score = mean_absolute_channel_error(&aligned_reference, &aligned_actual);
    let reference_transform = reference.original_to_cropped.translated(
        -i64::from(reference_origin.x),
        -i64::from(reference_origin.y),
    );
    let actual_transform = actual
        .original_to_cropped
        .translated(-i64::from(actual_origin.x), -i64::from(actual_origin.y));
    reference.report.aligned_dimensions = Some(aligned_size);
    actual.report.aligned_dimensions = Some(aligned_size);
    reference.report.coordinate_mapping =
        Some(coordinate_mapping(reference_transform, aligned_size));
    actual.report.coordinate_mapping = Some(coordinate_mapping(actual_transform, aligned_size));
    let alignment = AlignmentReport {
        mode: manifest.alignment.mode,
        maximum_translation: manifest.alignment.maximum_translation,
        selected_translation: selected,
        scale_x_millionths: 1_000_000,
        scale_y_millionths: 1_000_000,
        before_dimensions: reference_size,
        aligned_dimensions: aligned_size,
        mean_absolute_channel_error_millionths: Some(score),
    };
    persist_image_set(
        &output_directory,
        [
            (ALIGNED_REFERENCE_FILENAME, &aligned_reference),
            (ALIGNED_ACTUAL_FILENAME, &aligned_actual),
        ],
    )?;
    artifacts.push(artifact(
        &output_directory,
        "aligned_reference",
        ALIGNED_REFERENCE_FILENAME,
    ));
    artifacts.push(artifact(
        &output_directory,
        "aligned_actual",
        ALIGNED_ACTUAL_FILENAME,
    ));
    artifacts.push(artifact(
        &output_directory,
        "normalization_report",
        NORMALIZATION_REPORT_FILENAME,
    ));
    let report = NormalizationReport {
        schema_version: NORMALIZATION_REPORT_SCHEMA_VERSION,
        algorithm_version: NORMALIZE_ALIGN_ALGORITHM_VERSION.to_owned(),
        status: NormalizationStatus::Passed,
        inputs,
        reference: reference.report,
        actual: actual.report,
        alignment: Some(alignment),
        failure: None,
        artifacts,
    };
    persist_report(
        &output_directory.join(NORMALIZATION_REPORT_FILENAME),
        &report,
    )?;
    Ok(NormalizationOutcome {
        report,
        exit_code: ComparisonExitCode::Success,
    })
}

fn load_manifest(path: &Path) -> Result<(NormalizationManifest, String), ComparisonError> {
    let metadata = fs::metadata(path).map_err(|error| {
        ComparisonError::input(
            ComparisonErrorCode::ConfigReadFailed,
            format!("normalization manifest metadata cannot be read: {error}"),
        )
        .at_path(path)
    })?;
    if metadata.len() > MAX_MANIFEST_BYTES {
        return Err(ComparisonError::input(
            ComparisonErrorCode::ConfigTooLarge,
            format!("normalization manifest exceeds the {MAX_MANIFEST_BYTES}-byte limit"),
        )
        .at_path(path));
    }
    let bytes = fs::read(path).map_err(|error| {
        ComparisonError::input(
            ComparisonErrorCode::ConfigReadFailed,
            format!("normalization manifest cannot be read: {error}"),
        )
        .at_path(path)
    })?;
    let manifest: NormalizationManifest = serde_json::from_slice(&bytes).map_err(|error| {
        ComparisonError::input(
            ComparisonErrorCode::ConfigParseFailed,
            format!("normalization manifest is not valid schema JSON: {error}"),
        )
        .at_path(path)
    })?;
    if manifest.schema_version != NORMALIZATION_MANIFEST_SCHEMA_VERSION {
        return Err(ComparisonError::input(
            ComparisonErrorCode::ConfigInvalid,
            format!(
                "normalization manifest schema_version must be {NORMALIZATION_MANIFEST_SCHEMA_VERSION}, got {}",
                manifest.schema_version
            ),
        )
        .at_path(path));
    }
    if manifest.algorithm_version != NORMALIZE_ALIGN_ALGORITHM_VERSION {
        return Err(ComparisonError::input(
            ComparisonErrorCode::ConfigInvalid,
            format!(
                "normalization algorithm_version must be {NORMALIZE_ALIGN_ALGORITHM_VERSION}, got {}",
                manifest.algorithm_version
            ),
        )
        .at_path(path));
    }
    for expected in [
        manifest.reference.expected_sha256.as_deref(),
        manifest.actual.expected_sha256.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if !is_sha256(expected) {
            return Err(ComparisonError::input(
                ComparisonErrorCode::ConfigInvalid,
                "expected_sha256 must be 64 lowercase hexadecimal characters",
            )
            .at_path(path));
        }
    }
    if manifest.alignment.maximum_translation.x > HARD_MAX_TRANSLATION
        || manifest.alignment.maximum_translation.y > HARD_MAX_TRANSLATION
    {
        return Err(ComparisonError::input(
            ComparisonErrorCode::ConfigInvalid,
            format!("maximum_translation cannot exceed {HARD_MAX_TRANSLATION} pixels per axis"),
        )
        .at_path(path));
    }
    match (
        manifest.alignment.mode,
        manifest.alignment.declared_translation,
    ) {
        (AlignmentMode::DeclaredInteger, None) => {
            return Err(ComparisonError::input(
                ComparisonErrorCode::ConfigInvalid,
                "declared_integer alignment requires declared_translation",
            )
            .at_path(path));
        }
        (AlignmentMode::None | AlignmentMode::IntegerSearch, Some(_)) => {
            return Err(ComparisonError::input(
                ComparisonErrorCode::ConfigInvalid,
                "declared_translation is only valid with declared_integer alignment",
            )
            .at_path(path));
        }
        _ => {}
    }
    Ok((manifest, sha256(&bytes)))
}

fn decode_and_normalize(path: &Path) -> Result<DecodedImage, ComparisonError> {
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
    let format = detect_format(path, &bytes)?;
    let source_color_space = inspect_color_space(format, &bytes)?;
    let exif_orientation = read_exif_orientation(format, &bytes)?;
    let mut reader = ImageReader::with_format(Cursor::new(&bytes), format);
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_ALLOC);
    reader.limits(limits);
    let dynamic = reader.decode().map_err(|error| {
        let code = if matches!(error, ImageError::Limits(_)) {
            ComparisonErrorCode::ImageTooLarge
        } else {
            ComparisonErrorCode::ImageCorrupt
        };
        ComparisonError::input(code, format!("image cannot be decoded: {error}")).at_path(path)
    })?;
    let original_dimensions = PixelSize {
        width: dynamic.width(),
        height: dynamic.height(),
    };
    let source_alpha = if dynamic.color().has_alpha() {
        "straight_or_unspecified"
    } else {
        "opaque"
    }
    .to_owned();
    let mut rgba = dynamic.into_rgba8();
    for pixel in rgba.pixels_mut() {
        if pixel[3] == 0 {
            pixel[0] = 0;
            pixel[1] = 0;
            pixel[2] = 0;
        }
    }
    let rgba = apply_orientation(rgba, exif_orientation);
    Ok(DecodedImage {
        report: ImageInputReport {
            path: path.display().to_string(),
            format: match format {
                ImageFormat::Png => "png",
                ImageFormat::Jpeg => "jpeg",
                _ => unreachable!(),
            }
            .to_owned(),
            dimensions: original_dimensions,
            byte_length: metadata.len(),
        },
        sha256: sha256(&bytes),
        exif_orientation,
        source_color_space,
        source_alpha,
        image: rgba,
    })
}

fn prepare_image(
    decoded: DecodedImage,
    declaration: &CropDeclaration,
) -> Result<PreparedImage, ComparisonError> {
    let original_dimensions = decoded.report.dimensions;
    let oriented_dimensions = size_of(&decoded.image);
    let orientation_transform =
        orientation_transform(decoded.exif_orientation, original_dimensions);
    let (kind, left, top, right, bottom) = crop_values(declaration);
    if left
        .checked_add(right)
        .is_none_or(|value| value >= oriented_dimensions.width)
        || top
            .checked_add(bottom)
            .is_none_or(|value| value >= oriented_dimensions.height)
    {
        return Err(ComparisonError::input(
            ComparisonErrorCode::CropInvalid,
            "declared crop must leave at least one pixel on each axis",
        ));
    }
    let after_dimensions = PixelSize {
        width: oriented_dimensions.width - left - right,
        height: oriented_dimensions.height - top - bottom,
    };
    let cropped = imageops::crop_imm(
        &decoded.image,
        left,
        top,
        after_dimensions.width,
        after_dimensions.height,
    )
    .to_image();
    let quality = inspect_quality(&cropped);
    let report = InputNormalizationReport {
        sha256: decoded.sha256,
        original_dimensions,
        exif_orientation: decoded.exif_orientation,
        orientation_operation: orientation_label(decoded.exif_orientation).to_owned(),
        oriented_dimensions,
        source_color_space: decoded.source_color_space,
        output_color_space: "srgb".to_owned(),
        source_alpha: decoded.source_alpha,
        output_alpha: "straight_zero_transparent_rgb".to_owned(),
        pixel_format: "rgba8".to_owned(),
        crop: CropReport {
            kind,
            left,
            top,
            right,
            bottom,
            before_dimensions: oriented_dimensions,
            after_dimensions,
        },
        quality,
        aligned_dimensions: None,
        coordinate_mapping: None,
    };
    Ok(PreparedImage {
        full: decoded.image,
        cropped,
        report,
        original_to_cropped: orientation_transform.translated(-i64::from(left), -i64::from(top)),
    })
}

fn identity_failure(
    manifest: &NormalizationManifest,
    reference: &InputNormalizationReport,
    actual: &InputNormalizationReport,
) -> Option<(ComparisonErrorCode, String)> {
    match (
        manifest.reference.expected_sha256.as_deref(),
        manifest.actual.expected_sha256.as_deref(),
    ) {
        (Some(expected_reference), Some(expected_actual))
            if expected_reference != expected_actual
                && reference.sha256 == expected_actual
                && actual.sha256 == expected_reference =>
        {
            Some((
                ComparisonErrorCode::InputsSwapped,
                "reference and actual hashes match the opposite declared roles".to_owned(),
            ))
        }
        (Some(expected_reference), _) if reference.sha256 != expected_reference => Some((
            ComparisonErrorCode::InputIdentityMismatch,
            "reference SHA-256 does not match its declared identity".to_owned(),
        )),
        (_, Some(expected_actual)) if actual.sha256 != expected_actual => Some((
            ComparisonErrorCode::InputIdentityMismatch,
            "actual SHA-256 does not match its declared identity".to_owned(),
        )),
        _ => None,
    }
}

fn quality_failure(
    reference: &QualityReport,
    actual: &QualityReport,
) -> Option<(ComparisonErrorCode, String)> {
    for (role, quality) in [("reference", reference), ("actual", actual)] {
        if quality.total_pixels < MIN_SCREENSHOT_PIXELS {
            return Some((
                ComparisonErrorCode::ScreenshotTooSmall,
                format!("{role} is too small to be a valid screenshot"),
            ));
        }
        if quality.all_transparent {
            return Some((
                ComparisonErrorCode::ImageAllTransparent,
                format!("{role} is fully transparent"),
            ));
        }
        if quality.near_blank {
            return Some((
                ComparisonErrorCode::ImageNearBlank,
                format!("{role} is near blank under the deterministic dominant-pixel rule"),
            ));
        }
    }
    None
}

fn finish_failure(
    output_directory: &Path,
    inputs: NormalizationInputsReport,
    reference: InputNormalizationReport,
    actual: InputNormalizationReport,
    alignment: Option<AlignmentReport>,
    mut artifacts: Vec<ArtifactReport>,
    failure: (ComparisonErrorCode, impl Into<String>),
) -> Result<NormalizationOutcome, ComparisonError> {
    let (code, message) = failure;
    artifacts.push(artifact(
        output_directory,
        "normalization_report",
        NORMALIZATION_REPORT_FILENAME,
    ));
    let report = NormalizationReport {
        schema_version: NORMALIZATION_REPORT_SCHEMA_VERSION,
        algorithm_version: NORMALIZE_ALIGN_ALGORITHM_VERSION.to_owned(),
        status: NormalizationStatus::ComparisonFailed,
        inputs,
        reference,
        actual,
        alignment,
        failure: Some(ComparisonFailure::new(
            FailureType::Comparison,
            code,
            message,
        )),
        artifacts,
    };
    persist_report(
        &output_directory.join(NORMALIZATION_REPORT_FILENAME),
        &report,
    )?;
    Ok(NormalizationOutcome {
        report,
        exit_code: ComparisonExitCode::ComparisonFailure,
    })
}

fn crop_values(declaration: &CropDeclaration) -> (CropKind, u32, u32, u32, u32) {
    match *declaration {
        CropDeclaration::None => (CropKind::None, 0, 0, 0, 0),
        CropDeclaration::SystemUi {
            left,
            top,
            right,
            bottom,
        } => (CropKind::SystemUi, left, top, right, bottom),
        CropDeclaration::SafeArea {
            left,
            top,
            right,
            bottom,
        } => (CropKind::SafeArea, left, top, right, bottom),
        CropDeclaration::FixedBorder {
            left,
            top,
            right,
            bottom,
        } => (CropKind::FixedBorder, left, top, right, bottom),
    }
}

fn search_translation(
    reference: &RgbaImage,
    actual: &RgbaImage,
    maximum: IntegerOffsetLimit,
) -> IntegerOffset {
    let maximum = IntegerOffsetLimit {
        x: maximum.x.min(reference.width().saturating_sub(1)),
        y: maximum.y.min(reference.height().saturating_sub(1)),
    };
    let mut best = IntegerOffset { x: 0, y: 0 };
    let mut best_score = alignment_score(reference, actual, best);
    for y in -(maximum.y as i32)..=(maximum.y as i32) {
        for x in -(maximum.x as i32)..=(maximum.x as i32) {
            let candidate = IntegerOffset { x, y };
            let score = alignment_score(reference, actual, candidate);
            if score_better(score, candidate, best_score, best) {
                best = candidate;
                best_score = score;
            }
        }
    }
    best
}

fn alignment_score(reference: &RgbaImage, actual: &RgbaImage, offset: IntegerOffset) -> (u64, u64) {
    let (_, _, reference_origin, actual_origin) = translation_geometry(size_of(reference), offset);
    let width = reference.width() - reference_origin.x - actual_origin.x;
    let height = reference.height() - reference_origin.y - actual_origin.y;
    let mut sum = 0_u64;
    for y in 0..height {
        for x in 0..width {
            let left = reference.get_pixel(x + reference_origin.x, y + reference_origin.y);
            let right = actual.get_pixel(x + actual_origin.x, y + actual_origin.y);
            for channel in 0..4 {
                sum += u64::from(left[channel].abs_diff(right[channel]));
            }
        }
    }
    (sum, u64::from(width) * u64::from(height) * 4)
}

fn score_better(
    candidate: (u64, u64),
    candidate_offset: IntegerOffset,
    best: (u64, u64),
    best_offset: IntegerOffset,
) -> bool {
    let candidate_scaled = u128::from(candidate.0) * u128::from(best.1);
    let best_scaled = u128::from(best.0) * u128::from(candidate.1);
    candidate_scaled < best_scaled
        || (candidate_scaled == best_scaled
            && offset_tiebreak(candidate_offset) < offset_tiebreak(best_offset))
}

fn offset_tiebreak(offset: IntegerOffset) -> (u32, u32, u32, i32, i32) {
    (
        offset.x.unsigned_abs() + offset.y.unsigned_abs(),
        offset.y.unsigned_abs(),
        offset.x.unsigned_abs(),
        offset.y,
        offset.x,
    )
}

fn apply_translation(
    reference: &RgbaImage,
    actual: &RgbaImage,
    offset: IntegerOffset,
) -> (RgbaImage, RgbaImage, IntegerPoint, IntegerPoint) {
    let (width, height, reference_origin, actual_origin) =
        translation_geometry(size_of(reference), offset);
    (
        imageops::crop_imm(
            reference,
            reference_origin.x,
            reference_origin.y,
            width,
            height,
        )
        .to_image(),
        imageops::crop_imm(actual, actual_origin.x, actual_origin.y, width, height).to_image(),
        reference_origin,
        actual_origin,
    )
}

#[derive(Clone, Copy)]
struct IntegerPoint {
    x: u32,
    y: u32,
}

fn translation_geometry(
    size: PixelSize,
    offset: IntegerOffset,
) -> (u32, u32, IntegerPoint, IntegerPoint) {
    let reference_origin = IntegerPoint {
        x: offset.x.max(0) as u32,
        y: offset.y.max(0) as u32,
    };
    let actual_origin = IntegerPoint {
        x: (-offset.x).max(0) as u32,
        y: (-offset.y).max(0) as u32,
    };
    (
        size.width - reference_origin.x - actual_origin.x,
        size.height - reference_origin.y - actual_origin.y,
        reference_origin,
        actual_origin,
    )
}

fn coordinate_mapping(forward: AffineTransform, aligned_size: PixelSize) -> CoordinateMapping {
    let inverse = forward.inverse();
    let valid_aligned_bounds = PixelRect {
        x: 0,
        y: 0,
        width: aligned_size.width,
        height: aligned_size.height,
    };
    let valid_original_bounds = inverse.map_rect(valid_aligned_bounds);
    debug_assert_eq!(
        forward.map_rect(valid_original_bounds),
        valid_aligned_bounds
    );
    CoordinateMapping {
        original_to_aligned: forward,
        aligned_to_original: inverse,
        valid_original_bounds,
        valid_aligned_bounds,
    }
}

fn mean_absolute_channel_error(reference: &RgbaImage, actual: &RgbaImage) -> u32 {
    let (sum, denominator) = alignment_score(reference, actual, IntegerOffset { x: 0, y: 0 });
    ((u128::from(sum) * 1_000_000 + u128::from(denominator) / 2) / u128::from(denominator)) as u32
}

fn inspect_quality(image: &RgbaImage) -> QualityReport {
    let total_pixels = u64::from(image.width()) * u64::from(image.height());
    let nontransparent_pixels = image.pixels().filter(|pixel| pixel[3] != 0).count() as u64;
    let sample_count = usize::try_from(total_pixels)
        .unwrap_or(usize::MAX)
        .min(NEAR_BLANK_SAMPLE_LIMIT);
    let mut counts = HashMap::<[u8; 4], u32>::new();
    if sample_count > 0 {
        let bytes = image.as_raw();
        for sample in 0..sample_count {
            let index = if sample_count == 1 {
                0
            } else {
                sample * (usize::try_from(total_pixels).unwrap_or(usize::MAX) - 1)
                    / (sample_count - 1)
            };
            let offset = index * 4;
            let pixel = [
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
            ];
            *counts.entry(pixel).or_default() += 1;
        }
    }
    let dominant = counts.values().copied().max().unwrap_or(0);
    let dominant_sample_ratio_millionths = if sample_count == 0 {
        0
    } else {
        ((u64::from(dominant) * 1_000_000 + sample_count as u64 / 2) / sample_count as u64) as u32
    };
    let all_transparent = nontransparent_pixels == 0;
    QualityReport {
        total_pixels,
        nontransparent_pixels,
        sampled_pixels: sample_count as u32,
        dominant_sample_ratio_millionths,
        all_transparent,
        near_blank: !all_transparent
            && total_pixels >= NEAR_BLANK_MIN_PIXELS
            && dominant_sample_ratio_millionths >= NEAR_BLANK_DOMINANT_MILLIONTHS,
    }
}

fn apply_orientation(image: RgbaImage, orientation: u16) -> RgbaImage {
    match orientation {
        1 => image,
        2 => imageops::flip_horizontal(&image),
        3 => imageops::rotate180(&image),
        4 => imageops::flip_vertical(&image),
        5 => imageops::flip_horizontal(&imageops::rotate90(&image)),
        6 => imageops::rotate90(&image),
        7 => imageops::flip_vertical(&imageops::rotate90(&image)),
        8 => imageops::rotate270(&image),
        _ => unreachable!("EXIF orientation validated"),
    }
}

fn orientation_transform(orientation: u16, original: PixelSize) -> AffineTransform {
    let width = i64::from(original.width);
    let height = i64::from(original.height);
    match orientation {
        1 => AffineTransform {
            xx: 1,
            xy: 0,
            x_offset: 0,
            yx: 0,
            yy: 1,
            y_offset: 0,
        },
        2 => AffineTransform {
            xx: -1,
            xy: 0,
            x_offset: width,
            yx: 0,
            yy: 1,
            y_offset: 0,
        },
        3 => AffineTransform {
            xx: -1,
            xy: 0,
            x_offset: width,
            yx: 0,
            yy: -1,
            y_offset: height,
        },
        4 => AffineTransform {
            xx: 1,
            xy: 0,
            x_offset: 0,
            yx: 0,
            yy: -1,
            y_offset: height,
        },
        5 => AffineTransform {
            xx: 0,
            xy: 1,
            x_offset: 0,
            yx: 1,
            yy: 0,
            y_offset: 0,
        },
        6 => AffineTransform {
            xx: 0,
            xy: -1,
            x_offset: height,
            yx: 1,
            yy: 0,
            y_offset: 0,
        },
        7 => AffineTransform {
            xx: 0,
            xy: -1,
            x_offset: height,
            yx: -1,
            yy: 0,
            y_offset: width,
        },
        8 => AffineTransform {
            xx: 0,
            xy: 1,
            x_offset: 0,
            yx: -1,
            yy: 0,
            y_offset: width,
        },
        _ => unreachable!("EXIF orientation validated"),
    }
}

fn orientation_label(orientation: u16) -> &'static str {
    match orientation {
        1 => "identity",
        2 => "mirror_horizontal",
        3 => "rotate_180",
        4 => "mirror_vertical",
        5 => "transpose",
        6 => "rotate_90_clockwise",
        7 => "transverse",
        8 => "rotate_270_clockwise",
        _ => unreachable!(),
    }
}

fn read_exif_orientation(format: ImageFormat, bytes: &[u8]) -> Result<u16, ComparisonError> {
    let tiff = match format {
        ImageFormat::Jpeg => jpeg_exif_tiff(bytes),
        ImageFormat::Png => png_chunk(bytes, b"eXIf"),
        _ => None,
    };
    let Some(tiff) = tiff else {
        return Ok(1);
    };
    parse_tiff_orientation(tiff).map_err(|message| {
        ComparisonError::input(ComparisonErrorCode::ExifOrientationInvalid, message)
    })
}

fn jpeg_exif_tiff(bytes: &[u8]) -> Option<&[u8]> {
    if !bytes.starts_with(&[0xff, 0xd8]) {
        return None;
    }
    let mut cursor = 2;
    while cursor + 4 <= bytes.len() {
        if bytes[cursor] != 0xff {
            return None;
        }
        let marker = bytes[cursor + 1];
        if matches!(marker, 0xd9 | 0xda) {
            return None;
        }
        let length = u16::from_be_bytes([bytes[cursor + 2], bytes[cursor + 3]]) as usize;
        if length < 2 || cursor + 2 + length > bytes.len() {
            return None;
        }
        let payload = &bytes[cursor + 4..cursor + 2 + length];
        if marker == 0xe1 && payload.starts_with(b"Exif\0\0") {
            return Some(&payload[6..]);
        }
        cursor += 2 + length;
    }
    None
}

fn parse_tiff_orientation(bytes: &[u8]) -> Result<u16, &'static str> {
    if bytes.len() < 8 {
        return Err("EXIF TIFF header is truncated");
    }
    let little = match &bytes[0..2] {
        b"II" => true,
        b"MM" => false,
        _ => return Err("EXIF byte order is invalid"),
    };
    let read_u16 = |offset: usize| -> Option<u16> {
        let pair = [*bytes.get(offset)?, *bytes.get(offset + 1)?];
        Some(if little {
            u16::from_le_bytes(pair)
        } else {
            u16::from_be_bytes(pair)
        })
    };
    let read_u32 = |offset: usize| -> Option<u32> {
        let word = [
            *bytes.get(offset)?,
            *bytes.get(offset + 1)?,
            *bytes.get(offset + 2)?,
            *bytes.get(offset + 3)?,
        ];
        Some(if little {
            u32::from_le_bytes(word)
        } else {
            u32::from_be_bytes(word)
        })
    };
    if read_u16(2) != Some(42) {
        return Err("EXIF TIFF magic is invalid");
    }
    let ifd_offset = read_u32(4).ok_or("EXIF IFD offset is truncated")? as usize;
    let count = read_u16(ifd_offset).ok_or("EXIF IFD is truncated")? as usize;
    for index in 0..count {
        let offset = ifd_offset + 2 + index * 12;
        let tag = read_u16(offset).ok_or("EXIF entry is truncated")?;
        if tag == 0x0112 {
            if read_u16(offset + 2) != Some(3) || read_u32(offset + 4) != Some(1) {
                return Err("EXIF orientation has unsupported type or count");
            }
            let value = read_u16(offset + 8).ok_or("EXIF orientation value is truncated")?;
            return if (1..=8).contains(&value) {
                Ok(value)
            } else {
                Err("EXIF orientation must be between 1 and 8")
            };
        }
    }
    Ok(1)
}

fn inspect_color_space(format: ImageFormat, bytes: &[u8]) -> Result<String, ComparisonError> {
    match format {
        ImageFormat::Png => {
            if png_chunk(bytes, b"iCCP").is_some() {
                return Err(ComparisonError::input(
                    ComparisonErrorCode::UnsupportedColorProfile,
                    "embedded PNG ICC profiles are rejected because this tool has no ICC transform",
                ));
            }
            if png_chunk(bytes, b"cICP").is_some() {
                return Err(ComparisonError::input(
                    ComparisonErrorCode::UnsupportedColorProfile,
                    "PNG cICP profiles are rejected unless a future algorithm version defines conversion",
                ));
            }
            if let Some(gamma) = png_chunk(bytes, b"gAMA")
                && gamma.len() == 4
                && u32::from_be_bytes([gamma[0], gamma[1], gamma[2], gamma[3]]) != 45_455
                && png_chunk(bytes, b"sRGB").is_none()
            {
                return Err(ComparisonError::input(
                    ComparisonErrorCode::UnsupportedColorProfile,
                    "non-sRGB PNG gamma is rejected because this tool has no color transform",
                ));
            }
            Ok(if png_chunk(bytes, b"sRGB").is_some() {
                "srgb_declared"
            } else {
                "srgb_assumed_unprofiled"
            }
            .to_owned())
        }
        ImageFormat::Jpeg => {
            if jpeg_has_icc(bytes) {
                return Err(ComparisonError::input(
                    ComparisonErrorCode::UnsupportedColorProfile,
                    "embedded JPEG ICC profiles are rejected because this tool has no ICC transform",
                ));
            }
            Ok("srgb_assumed_unprofiled".to_owned())
        }
        _ => unreachable!(),
    }
}

fn png_chunk<'a>(bytes: &'a [u8], wanted: &[u8; 4]) -> Option<&'a [u8]> {
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return None;
    }
    let mut cursor = 8;
    while cursor + 12 <= bytes.len() {
        let length = u32::from_be_bytes([
            bytes[cursor],
            bytes[cursor + 1],
            bytes[cursor + 2],
            bytes[cursor + 3],
        ]) as usize;
        let end = cursor.checked_add(12 + length)?;
        if end > bytes.len() {
            return None;
        }
        if &bytes[cursor + 4..cursor + 8] == wanted {
            return Some(&bytes[cursor + 8..cursor + 8 + length]);
        }
        if &bytes[cursor + 4..cursor + 8] == b"IEND" {
            return None;
        }
        cursor = end;
    }
    None
}

fn jpeg_has_icc(bytes: &[u8]) -> bool {
    const ICC_MARKER: &[u8] = b"ICC_PROFILE\0";
    bytes
        .windows(ICC_MARKER.len())
        .any(|window| window == ICC_MARKER)
}

fn detect_format(path: &Path, bytes: &[u8]) -> Result<ImageFormat, ComparisonError> {
    let declared = match path.extension().and_then(|extension| extension.to_str()) {
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

fn persist_image_set<'a, const N: usize>(
    output_directory: &Path,
    images: [(&'a str, &'a RgbaImage); N],
) -> Result<(), ComparisonError> {
    let mut temporary_paths = Vec::with_capacity(N);
    for (filename, image) in images {
        let final_path = output_directory.join(filename);
        let temporary_path = output_directory.join(format!("{filename}.tmp"));
        if final_path.exists() || temporary_path.exists() {
            cleanup_files(&temporary_paths);
            return Err(ComparisonError::input(
                ComparisonErrorCode::ArtifactNameConflict,
                "normalization artifact name already exists",
            )
            .at_path(&final_path));
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
            .map_err(|error| {
                cleanup_files(&temporary_paths);
                ComparisonError::internal(
                    ComparisonErrorCode::ArtifactWriteFailed,
                    format!("temporary PNG artifact cannot be created: {error}"),
                )
                .at_path(&temporary_path)
            })?;
        let result = PngEncoder::new(&mut file)
            .write_image(
                image.as_raw(),
                image.width(),
                image.height(),
                ExtendedColorType::Rgba8,
            )
            .and_then(|_| file.flush().map_err(image::ImageError::IoError));
        if let Err(error) = result {
            drop(file);
            let _ = fs::remove_file(&temporary_path);
            cleanup_files(&temporary_paths);
            return Err(ComparisonError::internal(
                ComparisonErrorCode::ArtifactWriteFailed,
                format!("temporary PNG artifact cannot be written: {error}"),
            )
            .at_path(&temporary_path));
        }
        drop(file);
        temporary_paths.push((temporary_path, final_path));
    }
    let mut finalized = Vec::new();
    for (temporary, final_path) in &temporary_paths {
        if let Err(error) = fs::rename(temporary, final_path) {
            cleanup_files(&temporary_paths);
            cleanup_paths(&finalized);
            return Err(ComparisonError::internal(
                ComparisonErrorCode::ArtifactWriteFailed,
                format!("PNG artifact cannot be finalized: {error}"),
            )
            .at_path(final_path));
        }
        finalized.push(final_path.clone());
    }
    Ok(())
}

fn persist_report(path: &Path, report: &NormalizationReport) -> Result<(), ComparisonError> {
    let temporary = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(report).map_err(|error| {
        ComparisonError::internal(
            ComparisonErrorCode::InternalFailure,
            format!("normalization report cannot be serialized: {error}"),
        )
    })?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| {
            ComparisonError::internal(
                ComparisonErrorCode::ArtifactWriteFailed,
                format!("temporary normalization report cannot be created: {error}"),
            )
            .at_path(&temporary)
        })?;
    if let Err(error) = file.write_all(&bytes).and_then(|_| file.flush()) {
        drop(file);
        let _ = fs::remove_file(&temporary);
        return Err(ComparisonError::internal(
            ComparisonErrorCode::ArtifactWriteFailed,
            format!("temporary normalization report cannot be written: {error}"),
        )
        .at_path(&temporary));
    }
    drop(file);
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(ComparisonError::internal(
            ComparisonErrorCode::ArtifactWriteFailed,
            format!("normalization report cannot be finalized: {error}"),
        )
        .at_path(path));
    }
    Ok(())
}

fn artifact(output_directory: &Path, artifact_type: &str, filename: &str) -> ArtifactReport {
    ArtifactReport {
        artifact_type: artifact_type.to_owned(),
        path: output_directory.join(filename).display().to_string(),
    }
}

fn cleanup_files(paths: &[(PathBuf, PathBuf)]) {
    for (temporary, _) in paths {
        let _ = fs::remove_file(temporary);
    }
}

fn cleanup_paths(paths: &[PathBuf]) {
    for path in paths {
        let _ = fs::remove_file(path);
    }
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
                    "decoded image pixel count overflowed the normalization budget",
                )
            })?;
        total.checked_add(pixels).ok_or_else(|| {
            ComparisonError::input(
                ComparisonErrorCode::ImageTooLarge,
                "combined decoded image pixel count overflowed the normalization budget",
            )
        })
    })?;
    if total > MAX_TOTAL_DECODED_PIXELS {
        return Err(ComparisonError::input(
            ComparisonErrorCode::ImageTooLarge,
            format!(
                "combined decoded images exceed the {MAX_TOTAL_DECODED_PIXELS}-pixel normalization budget"
            ),
        ));
    }
    Ok(())
}

fn size_of(image: &RgbaImage) -> PixelSize {
    PixelSize {
        width: image.width(),
        height: image.height(),
    }
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_exif_orientation_transforms_have_exact_inverses() {
        let original = PixelSize {
            width: 120,
            height: 80,
        };
        let rect = PixelRect {
            x: 10,
            y: 20,
            width: 30,
            height: 15,
        };
        for orientation in 1..=8 {
            let forward = orientation_transform(orientation, original);
            assert_eq!(forward.inverse().map_rect(forward.map_rect(rect)), rect);
        }
    }

    #[test]
    fn alpha_normalization_zeroes_hidden_rgb() {
        let mut image = RgbaImage::from_raw(2, 1, vec![10, 20, 30, 0, 1, 2, 3, 255]).unwrap();
        for pixel in image.pixels_mut() {
            if pixel[3] == 0 {
                pixel[0] = 0;
                pixel[1] = 0;
                pixel[2] = 0;
            }
        }
        assert_eq!(image.as_raw(), &[0, 0, 0, 0, 1, 2, 3, 255]);
    }

    #[test]
    fn coordinate_mapping_round_trips_reference_and_actual_bounds_after_translation() {
        let reference = orientation_transform(
            1,
            PixelSize {
                width: 10,
                height: 8,
            },
        );
        let actual = reference.translated(-1, 0);
        let aligned_size = PixelSize {
            width: 9,
            height: 8,
        };
        let reference_mapping = coordinate_mapping(reference, aligned_size);
        let actual_mapping = coordinate_mapping(actual, aligned_size);
        let reference_element = PixelRect {
            x: 3,
            y: 2,
            width: 2,
            height: 3,
        };
        let actual_node = PixelRect {
            x: 4,
            ..reference_element
        };
        let aligned_reference = reference_mapping.map_original_rect_to_aligned(reference_element);
        let aligned_actual = actual_mapping.map_original_rect_to_aligned(actual_node);
        assert_eq!(aligned_reference, aligned_actual);
        assert_eq!(
            reference_mapping.map_aligned_rect_to_original(aligned_reference),
            reference_element
        );
        assert_eq!(
            actual_mapping.map_aligned_rect_to_original(aligned_actual),
            actual_node
        );
    }
}
