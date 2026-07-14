use crate::{
    contract::{
        AdditionalReferenceRole, GenerationTask, ImageColorSpace, ImageOrientation,
        MAX_REFERENCE_IMAGE_BYTES, PixelSize, TargetViewport, VerifiedReferenceImage,
    },
    directory::RunDirectoryPlan,
    inspect_task,
    lifecycle::{CancellationToken, TaskFailure, TaskFailureKind},
};
use image::{
    ColorType, DynamicImage, ExtendedColorType, GenericImageView, ImageDecoder, ImageEncoder,
    ImageError, ImageFormat, ImageReader, Limits, Rgba, RgbaImage,
    codecs::png::{CompressionType, FilterType, PngEncoder},
    imageops::FilterType as ResizeFilter,
    metadata::Orientation,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{BufReader, BufWriter, Cursor, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub const PREPROCESS_PROTOCOL_VERSION: u32 = 1;
pub const PREPROCESS_IMPLEMENTATION_VERSION: &str = "ui-reference-preprocess-1";

const MAX_OPTIONS_BYTES: u64 = 1024 * 1024;
pub const MAX_SYSTEM_UI_EXCLUSION_REGIONS: usize = 64;
const MAX_IMAGE_DIMENSION: u32 = 16_384;
const MAX_DECODED_PIXELS: u64 = 24_000_000;
const MAX_DECODE_ALLOC: u64 = 128 * 1024 * 1024;
const DEFAULT_PREVIEW_MAX_EDGE: u32 = 2_048;
const MAX_PREVIEW_PIXELS: u64 = 4_194_304;
const CACHE_MANIFEST_FILE: &str = "preprocess.json";
const STANDARD_PREVIEW_FILE: &str = "preview.png";
const STRUCTURE_PREVIEW_FILE: &str = "structure.png";
const HIGH_CONTRAST_FILE: &str = "high-contrast.png";
static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReferencePreprocessOptions {
    /// All regions use EXIF-normalized pixel-edge coordinates in the full oriented image.
    #[serde(default)]
    pub crop: Option<PixelRect>,
    #[serde(default)]
    pub safe_area: Option<PixelRect>,
    #[serde(default)]
    pub system_ui_exclusions: Vec<PixelRect>,
    #[serde(default)]
    pub preview: PreviewOptions,
    #[serde(default)]
    pub auxiliary: AuxiliaryOptions,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewOptions {
    #[serde(default = "default_preview_max_edge")]
    pub max_edge: u32,
}

impl Default for PreviewOptions {
    fn default() -> Self {
        Self {
            max_edge: DEFAULT_PREVIEW_MAX_EDGE,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AuxiliaryOptions {
    #[serde(default)]
    pub grid_spacing: Option<u32>,
    #[serde(default)]
    pub number_regions: bool,
    #[serde(default)]
    pub high_contrast: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PreprocessOptionsDocument {
    pub protocol_version: u32,
    #[serde(default)]
    pub defaults: ReferencePreprocessOptions,
    #[serde(default)]
    pub references: BTreeMap<String, ReferencePreprocessOptions>,
}

impl Default for PreprocessOptionsDocument {
    fn default() -> Self {
        Self {
            protocol_version: PREPROCESS_PROTOCOL_VERSION,
            defaults: ReferencePreprocessOptions::default(),
            references: BTreeMap::new(),
        }
    }
}

impl PreprocessOptionsDocument {
    pub fn load(path: Option<&Path>) -> Result<Self, TaskFailure> {
        let Some(path) = path else {
            return Ok(Self::default());
        };
        let metadata = fs::metadata(path).map_err(|error| {
            TaskFailure::new(
                TaskFailureKind::InvalidInput,
                format!("preprocess options cannot be read: {error}"),
                Some(path.display().to_string()),
            )
        })?;
        if !metadata.is_file() || metadata.len() > MAX_OPTIONS_BYTES {
            return Err(TaskFailure::new(
                TaskFailureKind::InvalidInput,
                format!(
                    "preprocess options must be a file no larger than {MAX_OPTIONS_BYTES} bytes"
                ),
                Some(path.display().to_string()),
            ));
        }
        let bytes = fs::read(path)
            .map_err(|error| input_io_failure("read preprocess options", path, error))?;
        let value: Self = serde_json::from_slice(&bytes).map_err(|error| {
            TaskFailure::new(
                TaskFailureKind::InvalidInput,
                format!("preprocess options do not match the strict contract: {error}"),
                Some(path.display().to_string()),
            )
        })?;
        if value.protocol_version != PREPROCESS_PROTOCOL_VERSION {
            return Err(TaskFailure::invalid(format!(
                "unsupported preprocess protocol_version {}; expected {PREPROCESS_PROTOCOL_VERSION}",
                value.protocol_version
            )));
        }
        Ok(value)
    }

    fn validate_basic(&self) -> Result<(), TaskFailure> {
        self.defaults.validate_basic("$.defaults")?;
        for (reference_id, options) in &self.references {
            options.validate_basic(&format!("$.references.{reference_id}"))?;
        }
        Ok(())
    }

    fn validate_reference_ids(&self, task: &GenerationTask) -> Result<(), TaskFailure> {
        let valid = std::iter::once(task.primary_reference.reference_id.as_str())
            .chain(
                task.additional_references
                    .iter()
                    .map(|reference| reference.image.reference_id.as_str()),
            )
            .collect::<BTreeSet<_>>();
        if let Some(unknown) = self
            .references
            .keys()
            .find(|id| !valid.contains(id.as_str()))
        {
            return Err(TaskFailure::invalid(format!(
                "preprocess options reference unknown reference_id `{unknown}`"
            )));
        }
        Ok(())
    }

    fn for_reference(&self, reference_id: &str) -> &ReferencePreprocessOptions {
        self.references.get(reference_id).unwrap_or(&self.defaults)
    }
}

impl ReferencePreprocessOptions {
    fn validate_basic(&self, path: &str) -> Result<(), TaskFailure> {
        validate_region_count(self.system_ui_exclusions.len(), path)?;
        if !(128..=4_096).contains(&self.preview.max_edge) {
            return Err(TaskFailure::invalid(format!(
                "{path}.preview.max_edge must be in 128..=4096"
            )));
        }
        if let Some(spacing) = self.auxiliary.grid_spacing
            && !(16..=512).contains(&spacing)
        {
            return Err(TaskFailure::invalid(format!(
                "{path}.auxiliary.grid_spacing must be in 16..=512"
            )));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl PixelRect {
    pub fn full(size: PixelSize) -> Self {
        Self {
            x: 0,
            y: 0,
            width: size.width,
            height: size.height,
        }
    }

    fn right(self) -> Option<u32> {
        self.x.checked_add(self.width)
    }

    fn bottom(self) -> Option<u32> {
        self.y.checked_add(self.height)
    }

    fn validate_inside(self, size: PixelSize, path: &str) -> Result<(), TaskFailure> {
        let valid = self.width > 0
            && self.height > 0
            && self.right().is_some_and(|right| right <= size.width)
            && self.bottom().is_some_and(|bottom| bottom <= size.height);
        if valid {
            Ok(())
        } else {
            Err(TaskFailure::invalid(format!(
                "{path} must be a non-empty pixel-edge rectangle inside the EXIF-normalized image"
            )))
        }
    }

    fn contains(self, other: Self) -> bool {
        other.x >= self.x
            && other.y >= self.y
            && other.right().zip(self.right()).is_some_and(|(a, b)| a <= b)
            && other
                .bottom()
                .zip(self.bottom())
                .is_some_and(|(a, b)| a <= b)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FloatPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FloatSize {
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FloatRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CoordinateSpace {
    RawImagePixel,
    ExifNormalizedPixel,
    PreviewPixel,
    TargetLogicalPixel,
    DevicePhysicalPixel,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CoordinateMapping {
    pub raw_size: PixelSize,
    pub exif_normalized_size: PixelSize,
    pub explicit_crop: PixelRect,
    pub preview_size: PixelSize,
    pub target_logical_size: FloatSize,
    pub device_physical_size: FloatSize,
    pub device_physical_raster_size: PixelSize,
    pub applied_orientation: AppliedOrientation,
    pub coordinate_convention: String,
    pub exif_crop_to_preview_scale: FloatPoint,
    pub exif_crop_to_logical_scale: FloatPoint,
    pub logical_to_physical_scale: FloatPoint,
    pub raster_rounding: String,
}

impl CoordinateMapping {
    pub fn new(
        raw_size: PixelSize,
        orientation: AppliedOrientation,
        crop: PixelRect,
        preview_size: PixelSize,
        viewport: TargetViewport,
    ) -> Result<Self, TaskFailure> {
        let exif_normalized_size = orientation.normalized_size(raw_size);
        crop.validate_inside(exif_normalized_size, "crop")?;
        if preview_size.width == 0 || preview_size.height == 0 {
            return Err(TaskFailure::invalid("preview dimensions must be non-zero"));
        }
        let target_logical_size = FloatSize {
            width: f64::from(viewport.logical_width),
            height: f64::from(viewport.logical_height),
        };
        let device_physical_size = FloatSize {
            width: target_logical_size.width * f64::from(viewport.device_scale),
            height: target_logical_size.height * f64::from(viewport.device_scale),
        };
        if !target_logical_size.width.is_finite()
            || !target_logical_size.height.is_finite()
            || !device_physical_size.width.is_finite()
            || !device_physical_size.height.is_finite()
        {
            return Err(TaskFailure::invalid(
                "viewport coordinate scale must be finite",
            ));
        }
        Ok(Self {
            raw_size,
            exif_normalized_size,
            explicit_crop: crop,
            preview_size,
            target_logical_size,
            device_physical_size,
            device_physical_raster_size: PixelSize {
                width: round_half_up(device_physical_size.width)?,
                height: round_half_up(device_physical_size.height)?,
            },
            applied_orientation: orientation,
            coordinate_convention: "top-left origin; x right; y down; continuous pixel-edge coordinates; bounds are closed for points and half-open for rectangles".to_owned(),
            exif_crop_to_preview_scale: FloatPoint {
                x: f64::from(preview_size.width) / f64::from(crop.width),
                y: f64::from(preview_size.height) / f64::from(crop.height),
            },
            exif_crop_to_logical_scale: FloatPoint {
                x: target_logical_size.width / f64::from(crop.width),
                y: target_logical_size.height / f64::from(crop.height),
            },
            logical_to_physical_scale: FloatPoint {
                x: f64::from(viewport.device_scale),
                y: f64::from(viewport.device_scale),
            },
            raster_rounding: "only raster output rounds; non-negative edges use floor(value + 0.5), then clamp to the destination bounds".to_owned(),
        })
    }

    pub fn map_point(
        &self,
        point: FloatPoint,
        from: CoordinateSpace,
        to: CoordinateSpace,
    ) -> Result<FloatPoint, TaskFailure> {
        let exif = self.to_exif(point, from)?;
        self.validate_point(exif, CoordinateSpace::ExifNormalizedPixel)?;
        let mapped = self.from_exif(exif, to)?;
        self.validate_point(mapped, to)?;
        Ok(mapped)
    }

    pub fn map_rect(&self, rect: PixelRect, to: CoordinateSpace) -> Result<FloatRect, TaskFailure> {
        rect.validate_inside(self.exif_normalized_size, "mapped rectangle")?;
        let left = f64::from(rect.x);
        let top = f64::from(rect.y);
        let right = f64::from(rect.right().unwrap());
        let bottom = f64::from(rect.bottom().unwrap());
        let mapped = [
            FloatPoint { x: left, y: top },
            FloatPoint { x: right, y: top },
            FloatPoint { x: left, y: bottom },
            FloatPoint {
                x: right,
                y: bottom,
            },
        ]
        .map(|point| self.map_point(point, CoordinateSpace::ExifNormalizedPixel, to));
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for point in mapped {
            let point = point?;
            min_x = min_x.min(point.x);
            min_y = min_y.min(point.y);
            max_x = max_x.max(point.x);
            max_y = max_y.max(point.y);
        }
        Ok(FloatRect {
            x: min_x,
            y: min_y,
            width: max_x - min_x,
            height: max_y - min_y,
        })
    }

    fn to_exif(&self, point: FloatPoint, from: CoordinateSpace) -> Result<FloatPoint, TaskFailure> {
        self.validate_point(point, from)?;
        let crop = self.explicit_crop;
        Ok(match from {
            CoordinateSpace::RawImagePixel => self
                .applied_orientation
                .raw_to_normalized(point, self.raw_size),
            CoordinateSpace::ExifNormalizedPixel => point,
            CoordinateSpace::PreviewPixel => FloatPoint {
                x: f64::from(crop.x)
                    + point.x * f64::from(crop.width) / f64::from(self.preview_size.width),
                y: f64::from(crop.y)
                    + point.y * f64::from(crop.height) / f64::from(self.preview_size.height),
            },
            CoordinateSpace::TargetLogicalPixel => FloatPoint {
                x: f64::from(crop.x)
                    + point.x * f64::from(crop.width) / self.target_logical_size.width,
                y: f64::from(crop.y)
                    + point.y * f64::from(crop.height) / self.target_logical_size.height,
            },
            CoordinateSpace::DevicePhysicalPixel => FloatPoint {
                x: f64::from(crop.x)
                    + point.x * f64::from(crop.width) / self.device_physical_size.width,
                y: f64::from(crop.y)
                    + point.y * f64::from(crop.height) / self.device_physical_size.height,
            },
        })
    }

    fn from_exif(&self, point: FloatPoint, to: CoordinateSpace) -> Result<FloatPoint, TaskFailure> {
        let crop = self.explicit_crop;
        let relative_x = point.x - f64::from(crop.x);
        let relative_y = point.y - f64::from(crop.y);
        Ok(match to {
            CoordinateSpace::RawImagePixel => self
                .applied_orientation
                .normalized_to_raw(point, self.raw_size),
            CoordinateSpace::ExifNormalizedPixel => point,
            CoordinateSpace::PreviewPixel => FloatPoint {
                x: relative_x * f64::from(self.preview_size.width) / f64::from(crop.width),
                y: relative_y * f64::from(self.preview_size.height) / f64::from(crop.height),
            },
            CoordinateSpace::TargetLogicalPixel => FloatPoint {
                x: relative_x * self.target_logical_size.width / f64::from(crop.width),
                y: relative_y * self.target_logical_size.height / f64::from(crop.height),
            },
            CoordinateSpace::DevicePhysicalPixel => FloatPoint {
                x: relative_x * self.device_physical_size.width / f64::from(crop.width),
                y: relative_y * self.device_physical_size.height / f64::from(crop.height),
            },
        })
    }

    fn validate_point(&self, point: FloatPoint, space: CoordinateSpace) -> Result<(), TaskFailure> {
        let (width, height, offset_x, offset_y) = match space {
            CoordinateSpace::RawImagePixel => (
                f64::from(self.raw_size.width),
                f64::from(self.raw_size.height),
                0.0,
                0.0,
            ),
            CoordinateSpace::ExifNormalizedPixel => (
                f64::from(self.exif_normalized_size.width),
                f64::from(self.exif_normalized_size.height),
                0.0,
                0.0,
            ),
            CoordinateSpace::PreviewPixel => (
                f64::from(self.preview_size.width),
                f64::from(self.preview_size.height),
                0.0,
                0.0,
            ),
            CoordinateSpace::TargetLogicalPixel => (
                self.target_logical_size.width,
                self.target_logical_size.height,
                0.0,
                0.0,
            ),
            CoordinateSpace::DevicePhysicalPixel => (
                self.device_physical_size.width,
                self.device_physical_size.height,
                0.0,
                0.0,
            ),
        };
        let epsilon = 1e-7;
        let valid = point.x.is_finite()
            && point.y.is_finite()
            && point.x >= offset_x - epsilon
            && point.y >= offset_y - epsilon
            && point.x <= offset_x + width + epsilon
            && point.y <= offset_y + height + epsilon;
        if valid {
            Ok(())
        } else {
            Err(TaskFailure::invalid(format!(
                "point ({}, {}) is outside {:?} bounds",
                point.x, point.y, space
            )))
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AppliedOrientation {
    Normal,
    Rotate90,
    Rotate180,
    Rotate270,
    MirrorHorizontal,
    MirrorVertical,
    Rotate90MirrorHorizontal,
    Rotate270MirrorHorizontal,
}

impl AppliedOrientation {
    fn from_image(value: Orientation) -> Self {
        match value {
            Orientation::NoTransforms => Self::Normal,
            Orientation::Rotate90 => Self::Rotate90,
            Orientation::Rotate180 => Self::Rotate180,
            Orientation::Rotate270 => Self::Rotate270,
            Orientation::FlipHorizontal => Self::MirrorHorizontal,
            Orientation::FlipVertical => Self::MirrorVertical,
            Orientation::Rotate90FlipH => Self::Rotate90MirrorHorizontal,
            Orientation::Rotate270FlipH => Self::Rotate270MirrorHorizontal,
        }
    }

    fn from_declared(value: ImageOrientation) -> Option<Self> {
        Some(match value {
            ImageOrientation::Normal => Self::Normal,
            ImageOrientation::Rotate90 => Self::Rotate90,
            ImageOrientation::Rotate180 => Self::Rotate180,
            ImageOrientation::Rotate270 => Self::Rotate270,
            ImageOrientation::MirrorHorizontal => Self::MirrorHorizontal,
            ImageOrientation::MirrorVertical => Self::MirrorVertical,
            ImageOrientation::Rotate90MirrorHorizontal => Self::Rotate90MirrorHorizontal,
            ImageOrientation::Rotate270MirrorHorizontal => Self::Rotate270MirrorHorizontal,
            ImageOrientation::Unknown => return None,
        })
    }

    fn to_image(self) -> Orientation {
        match self {
            Self::Normal => Orientation::NoTransforms,
            Self::Rotate90 => Orientation::Rotate90,
            Self::Rotate180 => Orientation::Rotate180,
            Self::Rotate270 => Orientation::Rotate270,
            Self::MirrorHorizontal => Orientation::FlipHorizontal,
            Self::MirrorVertical => Orientation::FlipVertical,
            Self::Rotate90MirrorHorizontal => Orientation::Rotate90FlipH,
            Self::Rotate270MirrorHorizontal => Orientation::Rotate270FlipH,
        }
    }

    fn normalized_size(self, raw: PixelSize) -> PixelSize {
        match self {
            Self::Rotate90
            | Self::Rotate270
            | Self::Rotate90MirrorHorizontal
            | Self::Rotate270MirrorHorizontal => PixelSize {
                width: raw.height,
                height: raw.width,
            },
            _ => raw,
        }
    }

    fn raw_to_normalized(self, point: FloatPoint, raw: PixelSize) -> FloatPoint {
        let w = f64::from(raw.width);
        let h = f64::from(raw.height);
        match self {
            Self::Normal => point,
            Self::Rotate90 => FloatPoint {
                x: h - point.y,
                y: point.x,
            },
            Self::Rotate180 => FloatPoint {
                x: w - point.x,
                y: h - point.y,
            },
            Self::Rotate270 => FloatPoint {
                x: point.y,
                y: w - point.x,
            },
            Self::MirrorHorizontal => FloatPoint {
                x: w - point.x,
                y: point.y,
            },
            Self::MirrorVertical => FloatPoint {
                x: point.x,
                y: h - point.y,
            },
            Self::Rotate90MirrorHorizontal => FloatPoint {
                x: point.y,
                y: point.x,
            },
            Self::Rotate270MirrorHorizontal => FloatPoint {
                x: h - point.y,
                y: w - point.x,
            },
        }
    }

    fn normalized_to_raw(self, point: FloatPoint, raw: PixelSize) -> FloatPoint {
        let inverse = match self {
            Self::Rotate90 => Self::Rotate270,
            Self::Rotate270 => Self::Rotate90,
            other => other,
        };
        inverse.raw_to_normalized(point, self.normalized_size(raw))
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceValidationProfile {
    PageReference,
    DetailReference,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EmbeddedMetadata {
    pub format: String,
    pub decoded_color_type: String,
    pub original_color_type: String,
    pub has_alpha_channel: bool,
    pub exif_present: bool,
    pub exif_byte_length: u64,
    pub exif_sha256: Option<String>,
    pub embedded_orientation: Option<AppliedOrientation>,
    pub declared_orientation: ImageOrientation,
    pub applied_orientation: AppliedOrientation,
    pub icc_profile_present: bool,
    pub icc_profile_byte_length: u64,
    pub icc_profile_sha256: Option<String>,
    pub declared_color_space: ImageColorSpace,
    pub preview_sample_encoding: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    StandardPreview,
    StructureOverlay,
    HighContrastPreview,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PreprocessArtifact {
    pub kind: ArtifactKind,
    pub file_name: String,
    pub sha256: String,
    pub byte_length: u64,
    pub width: u32,
    pub height: u32,
    pub auxiliary_only: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReferencePreprocessManifest {
    pub protocol_version: u32,
    pub implementation_version: String,
    pub cache_key: String,
    pub reference_id: String,
    pub source_sha256: String,
    pub source_byte_length: u64,
    pub source_raw_size: PixelSize,
    pub validation_profile: ReferenceValidationProfile,
    pub embedded_metadata: EmbeddedMetadata,
    pub coordinate_mapping: CoordinateMapping,
    pub explicit_safe_area: Option<PixelRect>,
    pub explicit_system_ui_exclusions: Vec<PixelRect>,
    pub options: ReferencePreprocessOptions,
    pub artifacts: Vec<PreprocessArtifact>,
    pub original_remains_authoritative: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PreprocessedReferenceResult {
    pub reference_id: String,
    pub cache_key: String,
    pub cache_hit: bool,
    pub output_directory: PathBuf,
    pub manifest: PathBuf,
    pub artifacts: Vec<PathBuf>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PreprocessTaskResult {
    pub run_id: String,
    pub output_root: PathBuf,
    pub manifest: PathBuf,
    pub references: Vec<PreprocessedReferenceResult>,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct CacheKeyInput<'a> {
    protocol_version: u32,
    implementation_version: &'a str,
    source_sha256: &'a str,
    reference_id: &'a str,
    declared_metadata: &'a crate::contract::ImageInputMetadata,
    viewport: TargetViewport,
    validation_profile: ReferenceValidationProfile,
    options: &'a ReferencePreprocessOptions,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct RunPreprocessManifest<'a> {
    protocol_version: u32,
    implementation_version: &'a str,
    run_id: &'a str,
    references: Vec<RunReferenceEntry<'a>>,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct RunReferenceEntry<'a> {
    reference_id: &'a str,
    source_path: &'a Path,
    source_sha256: &'a str,
    cache_key: &'a str,
    artifact_directory: PathBuf,
}

/// Decodes and normalizes all task references into an ignored run directory. It never writes to
/// `project/`, `android/`, or any approved asset directory.
pub fn preprocess_task(
    task_path: &Path,
    options_path: Option<&Path>,
    repository_root: &Path,
    cancellation: &CancellationToken,
) -> Result<PreprocessTaskResult, TaskFailure> {
    let options = PreprocessOptionsDocument::load(options_path)?;
    options.validate_basic()?;
    let inspection = inspect_task(task_path, repository_root, cancellation)?;
    options.validate_reference_ids(&inspection.task)?;
    let generation_root = controlled_generation_root(repository_root)?;
    let cache_root = controlled_child_directory(&generation_root, ".cache/preprocess")?;

    let mut cached = Vec::new();
    for verified in &inspection.verified_references {
        cancellation.checkpoint()?;
        let (viewport, profile) = reference_context(&inspection.task, &verified.reference_id)?;
        let reference_options = options.for_reference(&verified.reference_id);
        let entry = preprocess_reference_cached(
            verified,
            viewport,
            profile,
            reference_options,
            &cache_root,
            cancellation,
        )?;
        cached.push((verified, entry));
    }

    materialize_run(
        &inspection.directory_plan,
        &inspection.task.run_id,
        &cached,
        cancellation,
    )
}

fn reference_context(
    task: &GenerationTask,
    reference_id: &str,
) -> Result<(TargetViewport, ReferenceValidationProfile), TaskFailure> {
    let default_viewport = task.target_viewport.ok_or_else(|| {
        TaskFailure::new(
            TaskFailureKind::TargetViewportMissing,
            "target viewport is required for preprocessing",
            None,
        )
    })?;
    if task.primary_reference.reference_id == reference_id {
        return Ok((default_viewport, ReferenceValidationProfile::PageReference));
    }
    let additional = task
        .additional_references
        .iter()
        .find(|reference| reference.image.reference_id == reference_id)
        .ok_or_else(|| TaskFailure::invalid(format!("unknown reference_id `{reference_id}`")))?;
    match additional.role {
        AdditionalReferenceRole::Viewport { viewport } => {
            Ok((viewport, ReferenceValidationProfile::PageReference))
        }
        AdditionalReferenceRole::Detail { .. } => Ok((
            default_viewport,
            ReferenceValidationProfile::DetailReference,
        )),
        AdditionalReferenceRole::State { .. } => {
            Ok((default_viewport, ReferenceValidationProfile::PageReference))
        }
    }
}

#[derive(Debug)]
struct CachedReference {
    cache_key: String,
    cache_hit: bool,
    directory: PathBuf,
    manifest: ReferencePreprocessManifest,
}

fn preprocess_reference_cached(
    verified: &VerifiedReferenceImage,
    viewport: TargetViewport,
    profile: ReferenceValidationProfile,
    options: &ReferencePreprocessOptions,
    cache_root: &Path,
    cancellation: &CancellationToken,
) -> Result<CachedReference, TaskFailure> {
    if verified.byte_length == 0 || verified.byte_length > MAX_REFERENCE_IMAGE_BYTES {
        return Err(TaskFailure::new(
            TaskFailureKind::ImageDimensionsUnsafe,
            format!(
                "reference image `{}` must be 1..={MAX_REFERENCE_IMAGE_BYTES} encoded bytes",
                verified.reference_id
            ),
            Some(verified.resolved_path.display().to_string()),
        ));
    }
    let key_input = CacheKeyInput {
        protocol_version: PREPROCESS_PROTOCOL_VERSION,
        implementation_version: PREPROCESS_IMPLEMENTATION_VERSION,
        source_sha256: &verified.sha256,
        reference_id: &verified.reference_id,
        declared_metadata: &verified.metadata,
        viewport,
        validation_profile: profile,
        options,
    };
    let key_bytes = serde_json::to_vec(&key_input).map_err(|error| {
        TaskFailure::invalid(format!("cache key cannot be serialized: {error}"))
    })?;
    let cache_key = sha256_bytes(&key_bytes);
    let destination = cache_root.join(&cache_key);
    ensure_direct_child(cache_root, &destination)?;
    if destination.exists() {
        let manifest = load_and_validate_cache(&destination, &cache_key)?;
        return Ok(CachedReference {
            cache_key,
            cache_hit: true,
            directory: destination,
            manifest,
        });
    }

    cancellation.checkpoint()?;
    let staging = cache_root.join(format!(
        ".{cache_key}.tmp-{}-{}",
        std::process::id(),
        next_sequence()
    ));
    ensure_direct_child(cache_root, &staging)?;
    fs::create_dir(&staging).map_err(|error| {
        output_io_failure("create preprocess cache staging directory", &staging, error)
    })?;
    let build = build_reference_cache(
        verified,
        viewport,
        profile,
        options,
        &cache_key,
        &staging,
        cancellation,
    )
    .and_then(|_| load_and_validate_cache(&staging, &cache_key));
    let manifest = match build {
        Ok(manifest) => manifest,
        Err(error) => {
            let _ = fs::remove_dir_all(&staging);
            return Err(error);
        }
    };
    match fs::rename(&staging, &destination) {
        Ok(()) => Ok(CachedReference {
            cache_key,
            cache_hit: false,
            directory: destination,
            manifest,
        }),
        Err(_error) if destination.exists() => {
            let _ = fs::remove_dir_all(&staging);
            let manifest = load_and_validate_cache(&destination, &cache_key)?;
            Ok(CachedReference {
                cache_key,
                cache_hit: true,
                directory: destination,
                manifest,
            })
        }
        Err(error) => {
            let _ = fs::remove_dir_all(&staging);
            Err(output_io_failure(
                "commit preprocess cache entry",
                &destination,
                error,
            ))
        }
    }
}

fn build_reference_cache(
    verified: &VerifiedReferenceImage,
    viewport: TargetViewport,
    profile: ReferenceValidationProfile,
    options: &ReferencePreprocessOptions,
    cache_key: &str,
    staging: &Path,
    cancellation: &CancellationToken,
) -> Result<ReferencePreprocessManifest, TaskFailure> {
    let bytes = read_bounded_image(verified)?;
    cancellation.checkpoint()?;
    let format = detect_supported_format(&bytes, &verified.resolved_path)?;
    let inspected = inspect_encoded_image(&bytes, format, verified)?;
    validate_dimensions(inspected.raw_size, profile, &verified.resolved_path)?;
    let applied_orientation = resolve_orientation(
        inspected.embedded_orientation,
        verified.metadata.orientation,
        &verified.resolved_path,
    )?;
    let normalized_size = applied_orientation.normalized_size(inspected.raw_size);
    let crop = options
        .crop
        .unwrap_or_else(|| PixelRect::full(normalized_size));
    validate_regions(options, crop, normalized_size)?;
    let preview_size = preview_dimensions(crop, options.preview.max_edge)?;
    let mapping = CoordinateMapping::new(
        inspected.raw_size,
        applied_orientation,
        crop,
        preview_size,
        viewport,
    )?;

    let mut image = decode_image(&bytes, format, &verified.resolved_path)?;
    cancellation.checkpoint()?;
    image.apply_orientation(applied_orientation.to_image());
    if image.dimensions() != (normalized_size.width, normalized_size.height) {
        return Err(metadata_mismatch(
            &verified.resolved_path,
            "decoded EXIF-normalized dimensions do not match the recorded transform",
        ));
    }
    let cropped = image.crop_imm(crop.x, crop.y, crop.width, crop.height);
    validate_blank(&cropped, profile, &verified.resolved_path)?;
    let preview = if cropped.dimensions() == (preview_size.width, preview_size.height) {
        cropped.to_rgba8()
    } else {
        cropped
            .resize_exact(
                preview_size.width,
                preview_size.height,
                ResizeFilter::Lanczos3,
            )
            .to_rgba8()
    };

    let mut artifacts = Vec::new();
    artifacts.push(write_artifact(
        staging,
        STANDARD_PREVIEW_FILE,
        ArtifactKind::StandardPreview,
        &preview,
        false,
    )?);
    if options.auxiliary.grid_spacing.is_some() || options.auxiliary.number_regions {
        let mut structure = preview.clone();
        draw_structure_overlay(&mut structure, &mapping, options)?;
        artifacts.push(write_artifact(
            staging,
            STRUCTURE_PREVIEW_FILE,
            ArtifactKind::StructureOverlay,
            &structure,
            true,
        )?);
    }
    if options.auxiliary.high_contrast {
        let contrast = high_contrast_preview(&preview);
        artifacts.push(write_artifact(
            staging,
            HIGH_CONTRAST_FILE,
            ArtifactKind::HighContrastPreview,
            &contrast,
            true,
        )?);
    }
    cancellation.checkpoint()?;

    let manifest = ReferencePreprocessManifest {
        protocol_version: PREPROCESS_PROTOCOL_VERSION,
        implementation_version: PREPROCESS_IMPLEMENTATION_VERSION.to_owned(),
        cache_key: cache_key.to_owned(),
        reference_id: verified.reference_id.clone(),
        source_sha256: verified.sha256.clone(),
        source_byte_length: verified.byte_length,
        source_raw_size: inspected.raw_size,
        validation_profile: profile,
        embedded_metadata: EmbeddedMetadata {
            format: format_name(format).to_owned(),
            decoded_color_type: format!("{:?}", inspected.decoded_color_type),
            original_color_type: format!("{:?}", inspected.original_color_type),
            has_alpha_channel: inspected.decoded_color_type.has_alpha(),
            exif_present: inspected.exif_present,
            exif_byte_length: inspected.exif.as_ref().map_or(0, |value| value.len() as u64),
            exif_sha256: inspected.exif.as_deref().map(sha256_bytes),
            embedded_orientation: inspected.embedded_orientation,
            declared_orientation: verified.metadata.orientation,
            applied_orientation,
            icc_profile_present: inspected.icc_profile.is_some(),
            icc_profile_byte_length: inspected
                .icc_profile
                .as_ref()
                .map_or(0, |value| value.len() as u64),
            icc_profile_sha256: inspected.icc_profile.as_deref().map(sha256_bytes),
            declared_color_space: verified.metadata.color_space.clone(),
            preview_sample_encoding: "deterministic PNG RGBA8; encoded sample values are preserved and no unverified ICC conversion is claimed".to_owned(),
        },
        coordinate_mapping: mapping,
        explicit_safe_area: options.safe_area,
        explicit_system_ui_exclusions: options.system_ui_exclusions.clone(),
        options: options.clone(),
        artifacts,
        original_remains_authoritative: true,
    };
    write_json_file(&staging.join(CACHE_MANIFEST_FILE), &manifest)?;
    Ok(manifest)
}

struct EncodedImageInfo {
    raw_size: PixelSize,
    decoded_color_type: ColorType,
    original_color_type: ExtendedColorType,
    exif_present: bool,
    exif: Option<Vec<u8>>,
    embedded_orientation: Option<AppliedOrientation>,
    icc_profile: Option<Vec<u8>>,
}

fn inspect_encoded_image(
    bytes: &[u8],
    format: ImageFormat,
    verified: &VerifiedReferenceImage,
) -> Result<EncodedImageInfo, TaskFailure> {
    let reader = ImageReader::with_format(Cursor::new(bytes), format);
    let mut decoder = reader.into_decoder().map_err(|error| {
        corrupt_failure(
            &verified.resolved_path,
            format!("image header cannot be decoded: {error}"),
        )
    })?;
    let (width, height) = decoder.dimensions();
    let raw_size = PixelSize { width, height };
    if raw_size != verified.metadata.original_size {
        return Err(metadata_mismatch(
            &verified.resolved_path,
            format!(
                "declared original_size {}x{} does not match encoded dimensions {}x{}",
                verified.metadata.original_size.width,
                verified.metadata.original_size.height,
                width,
                height
            ),
        ));
    }
    let decoded_color_type = decoder.color_type();
    let original_color_type = decoder.original_color_type();
    let exif = decoder.exif_metadata().map_err(|error| {
        corrupt_failure(
            &verified.resolved_path,
            format!("EXIF metadata cannot be read: {error}"),
        )
    })?;
    let embedded_orientation = exif
        .as_deref()
        .and_then(Orientation::from_exif_chunk)
        .map(AppliedOrientation::from_image);
    let icc_profile = decoder.icc_profile().map_err(|error| {
        corrupt_failure(
            &verified.resolved_path,
            format!("ICC profile cannot be read: {error}"),
        )
    })?;
    Ok(EncodedImageInfo {
        raw_size,
        decoded_color_type,
        original_color_type,
        exif_present: exif.is_some(),
        exif,
        embedded_orientation,
        icc_profile,
    })
}

fn resolve_orientation(
    embedded: Option<AppliedOrientation>,
    declared: ImageOrientation,
    path: &Path,
) -> Result<AppliedOrientation, TaskFailure> {
    let declared = AppliedOrientation::from_declared(declared);
    match (embedded, declared) {
        (Some(actual), Some(declared)) if actual != declared => Err(metadata_mismatch(
            path,
            format!("declared orientation {declared:?} does not match embedded EXIF {actual:?}"),
        )),
        (Some(actual), _) => Ok(actual),
        (None, Some(declared)) => Ok(declared),
        (None, None) => Err(metadata_mismatch(
            path,
            "orientation is unknown and the image has no usable embedded EXIF orientation",
        )),
    }
}

fn validate_dimensions(
    size: PixelSize,
    profile: ReferenceValidationProfile,
    path: &Path,
) -> Result<(), TaskFailure> {
    let pixels = u64::from(size.width).saturating_mul(u64::from(size.height));
    if size.width == 0
        || size.height == 0
        || size.width > MAX_IMAGE_DIMENSION
        || size.height > MAX_IMAGE_DIMENSION
        || pixels > MAX_DECODED_PIXELS
    {
        return Err(TaskFailure::new(
            TaskFailureKind::ImageDimensionsUnsafe,
            format!(
                "decoded image must be non-zero, at most {MAX_IMAGE_DIMENSION}px per edge and {MAX_DECODED_PIXELS} pixels"
            ),
            Some(path.display().to_string()),
        ));
    }
    let minimum = match profile {
        ReferenceValidationProfile::PageReference => 64,
        ReferenceValidationProfile::DetailReference => 8,
    };
    if size.width < minimum || size.height < minimum {
        return Err(TaskFailure::new(
            TaskFailureKind::ImageTooSmall,
            format!("{profile:?} images must be at least {minimum}px on each edge"),
            Some(path.display().to_string()),
        ));
    }
    let ratio = f64::from(size.width.max(size.height)) / f64::from(size.width.min(size.height));
    let maximum_ratio = match profile {
        ReferenceValidationProfile::PageReference => 10.0,
        ReferenceValidationProfile::DetailReference => 64.0,
    };
    if ratio > maximum_ratio {
        return Err(TaskFailure::new(
            TaskFailureKind::ImageAspectRatioUnsupported,
            format!("{profile:?} aspect ratio {ratio:.3} exceeds {maximum_ratio:.1}"),
            Some(path.display().to_string()),
        ));
    }
    Ok(())
}

fn validate_regions(
    options: &ReferencePreprocessOptions,
    crop: PixelRect,
    normalized_size: PixelSize,
) -> Result<(), TaskFailure> {
    validate_region_count(options.system_ui_exclusions.len(), "preprocess options")?;
    crop.validate_inside(normalized_size, "crop")?;
    if let Some(safe_area) = options.safe_area {
        safe_area.validate_inside(normalized_size, "safe_area")?;
        if !crop.contains(safe_area) {
            return Err(TaskFailure::invalid(
                "safe_area must be fully contained by the explicit crop",
            ));
        }
    }
    for (index, exclusion) in options.system_ui_exclusions.iter().copied().enumerate() {
        exclusion.validate_inside(normalized_size, &format!("system_ui_exclusions[{index}]"))?;
        if !crop.contains(exclusion) {
            return Err(TaskFailure::invalid(format!(
                "system_ui_exclusions[{index}] must be fully contained by the explicit crop"
            )));
        }
    }
    Ok(())
}

fn validate_region_count(count: usize, path: &str) -> Result<(), TaskFailure> {
    if count > MAX_SYSTEM_UI_EXCLUSION_REGIONS {
        Err(TaskFailure::invalid(format!(
            "{path}.system_ui_exclusions contains {count} regions; at most {MAX_SYSTEM_UI_EXCLUSION_REGIONS} are allowed"
        )))
    } else {
        Ok(())
    }
}

fn preview_dimensions(crop: PixelRect, max_edge: u32) -> Result<PixelSize, TaskFailure> {
    let edge_scale = f64::from(max_edge) / f64::from(crop.width.max(crop.height));
    let pixel_scale =
        (MAX_PREVIEW_PIXELS as f64 / (f64::from(crop.width) * f64::from(crop.height))).sqrt();
    let scale = edge_scale.min(pixel_scale).min(1.0);
    Ok(PixelSize {
        width: round_half_up(f64::from(crop.width) * scale)?.max(1),
        height: round_half_up(f64::from(crop.height) * scale)?.max(1),
    })
}

fn validate_blank(
    image: &DynamicImage,
    profile: ReferenceValidationProfile,
    path: &Path,
) -> Result<(), TaskFailure> {
    let rgba = image.to_rgba8();
    let mut visible = false;
    let mut min = [u8::MAX; 4];
    let mut max = [u8::MIN; 4];
    for pixel in rgba.pixels() {
        if pixel[3] == 0 {
            continue;
        }
        visible = true;
        for channel in 0..4 {
            min[channel] = min[channel].min(pixel[channel]);
            max[channel] = max[channel].max(pixel[channel]);
        }
    }
    let page_is_uniform = profile == ReferenceValidationProfile::PageReference
        && visible
        && (0..4).all(|channel| max[channel].saturating_sub(min[channel]) <= 2);
    if !visible || page_is_uniform {
        let reason = if visible {
            "page reference has no deterministic visible variation (all visible RGBA channels vary by at most 2)"
        } else {
            "image has no visible pixels (all alpha values are zero)"
        };
        return Err(TaskFailure::new(
            TaskFailureKind::ImageBlank,
            reason,
            Some(path.display().to_string()),
        ));
    }
    Ok(())
}

fn decode_image(
    bytes: &[u8],
    format: ImageFormat,
    path: &Path,
) -> Result<DynamicImage, TaskFailure> {
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_ALLOC);
    reader.limits(limits);
    reader.decode().map_err(|error| match error {
        ImageError::Limits(_) => TaskFailure::new(
            TaskFailureKind::ImageDimensionsUnsafe,
            format!("image pixels exceed decoding safety limits: {error}"),
            Some(path.display().to_string()),
        ),
        _ => corrupt_failure(path, format!("image pixels cannot be decoded: {error}")),
    })
}

fn read_bounded_image(verified: &VerifiedReferenceImage) -> Result<Vec<u8>, TaskFailure> {
    let file = File::open(&verified.resolved_path).map_err(|error| {
        image_io_failure("open reference image", &verified.resolved_path, error)
    })?;
    let capacity = usize::try_from(verified.byte_length).map_err(|_| {
        TaskFailure::new(
            TaskFailureKind::ImageDimensionsUnsafe,
            "encoded image length cannot fit in memory",
            Some(verified.resolved_path.display().to_string()),
        )
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    file.take(MAX_REFERENCE_IMAGE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            image_io_failure("read reference image", &verified.resolved_path, error)
        })?;
    if bytes.len() as u64 != verified.byte_length || bytes.len() as u64 > MAX_REFERENCE_IMAGE_BYTES
    {
        return Err(TaskFailure::new(
            TaskFailureKind::ImageDimensionsUnsafe,
            "reference image length changed after verification or exceeded the encoded byte limit",
            Some(verified.resolved_path.display().to_string()),
        ));
    }
    if sha256_bytes(&bytes) != verified.sha256 {
        return Err(TaskFailure::new(
            TaskFailureKind::ImageHashMismatch,
            "reference image changed after task inspection",
            Some(verified.resolved_path.display().to_string()),
        ));
    }
    Ok(bytes)
}

fn detect_supported_format(bytes: &[u8], path: &Path) -> Result<ImageFormat, TaskFailure> {
    match image::guess_format(bytes) {
        Ok(format @ (ImageFormat::Png | ImageFormat::Jpeg)) => Ok(format),
        Ok(format) => Err(TaskFailure::new(
            TaskFailureKind::ImageUnsupportedFormat,
            format!(
                "image format {:?} is not supported; use PNG or JPEG",
                format
            ),
            Some(path.display().to_string()),
        )),
        Err(error) => Err(TaskFailure::new(
            TaskFailureKind::ImageUnsupportedFormat,
            format!("image format cannot be identified as PNG or JPEG: {error}"),
            Some(path.display().to_string()),
        )),
    }
}

fn format_name(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Png => "png",
        ImageFormat::Jpeg => "jpeg",
        _ => "unsupported",
    }
}

fn write_artifact(
    directory: &Path,
    file_name: &str,
    kind: ArtifactKind,
    image: &RgbaImage,
    auxiliary_only: bool,
) -> Result<PreprocessArtifact, TaskFailure> {
    let path = directory.join(file_name);
    let mut encoded = Vec::new();
    let encoder =
        PngEncoder::new_with_quality(&mut encoded, CompressionType::Best, FilterType::Adaptive);
    encoder
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            ColorType::Rgba8.into(),
        )
        .map_err(|error| {
            TaskFailure::new(
                TaskFailureKind::ImageCorrupt,
                format!("preview PNG cannot be encoded: {error}"),
                Some(path.display().to_string()),
            )
        })?;
    write_bytes_file(&path, &encoded)?;
    let (byte_length, sha256) = hash_file(&path)?;
    Ok(PreprocessArtifact {
        kind,
        file_name: file_name.to_owned(),
        sha256,
        byte_length,
        width: image.width(),
        height: image.height(),
        auxiliary_only,
    })
}

fn draw_structure_overlay(
    image: &mut RgbaImage,
    mapping: &CoordinateMapping,
    options: &ReferencePreprocessOptions,
) -> Result<(), TaskFailure> {
    if let Some(spacing) = options.auxiliary.grid_spacing {
        let color = Rgba([0, 255, 255, 150]);
        for x in (spacing..image.width()).step_by(spacing as usize) {
            draw_vertical(image, x, color);
        }
        for y in (spacing..image.height()).step_by(spacing as usize) {
            draw_horizontal(image, y, color);
        }
    }
    if options.auxiliary.number_regions {
        let mut regions = Vec::new();
        if let Some(safe) = options.safe_area {
            regions.push((safe, Rgba([0, 255, 80, 255])));
        }
        regions.extend(
            options
                .system_ui_exclusions
                .iter()
                .copied()
                .map(|region| (region, Rgba([255, 40, 40, 255]))),
        );
        for (index, (region, color)) in regions.into_iter().enumerate() {
            let mapped = mapping.map_rect(region, CoordinateSpace::PreviewPixel)?;
            draw_rect_and_number(image, mapped, color, (index + 1) as u32);
        }
    }
    Ok(())
}

fn high_contrast_preview(source: &RgbaImage) -> RgbaImage {
    let mut output = source.clone();
    for pixel in output.pixels_mut() {
        let luminance =
            (u32::from(pixel[0]) * 54 + u32::from(pixel[1]) * 183 + u32::from(pixel[2]) * 19) / 256;
        let value = if luminance >= 128 { 255 } else { 0 };
        *pixel = Rgba([value, value, value, pixel[3]]);
    }
    output
}

fn draw_vertical(image: &mut RgbaImage, x: u32, color: Rgba<u8>) {
    for y in 0..image.height() {
        blend_pixel(image, x.min(image.width() - 1), y, color);
    }
}

fn draw_horizontal(image: &mut RgbaImage, y: u32, color: Rgba<u8>) {
    for x in 0..image.width() {
        blend_pixel(image, x, y.min(image.height() - 1), color);
    }
}

fn draw_rect_and_number(image: &mut RgbaImage, rect: FloatRect, color: Rgba<u8>, number: u32) {
    let x0 = raster_edge(rect.x, image.width());
    let y0 = raster_edge(rect.y, image.height());
    let x1 = raster_edge(rect.x + rect.width, image.width()).saturating_sub(1);
    let y1 = raster_edge(rect.y + rect.height, image.height()).saturating_sub(1);
    if x0 > x1 || y0 > y1 {
        return;
    }
    for thickness in 0..2 {
        let left = (x0 + thickness).min(x1);
        let right = x1.saturating_sub(thickness).max(x0);
        let top = (y0 + thickness).min(y1);
        let bottom = y1.saturating_sub(thickness).max(y0);
        for x in left..=right {
            blend_pixel(image, x, top, color);
            blend_pixel(image, x, bottom, color);
        }
        for y in top..=bottom {
            blend_pixel(image, left, y, color);
            blend_pixel(image, right, y, color);
        }
    }
    draw_number(
        image,
        x0.saturating_add(3),
        y0.saturating_add(3),
        number,
        color,
    );
}

fn draw_number(image: &mut RgbaImage, mut x: u32, y: u32, number: u32, color: Rgba<u8>) {
    for digit in number.to_string().bytes() {
        let pattern = DIGITS[(digit - b'0') as usize];
        for row in 0..5_u32 {
            for column in 0..3_u32 {
                if pattern[row as usize] & (1 << (2 - column)) != 0
                    && x + column < image.width()
                    && y + row < image.height()
                {
                    blend_pixel(image, x + column, y + row, color);
                }
            }
        }
        x = x.saturating_add(4);
    }
}

const DIGITS: [[u8; 5]; 10] = [
    [0b111, 0b101, 0b101, 0b101, 0b111],
    [0b010, 0b110, 0b010, 0b010, 0b111],
    [0b111, 0b001, 0b111, 0b100, 0b111],
    [0b111, 0b001, 0b111, 0b001, 0b111],
    [0b101, 0b101, 0b111, 0b001, 0b001],
    [0b111, 0b100, 0b111, 0b001, 0b111],
    [0b111, 0b100, 0b111, 0b101, 0b111],
    [0b111, 0b001, 0b010, 0b010, 0b010],
    [0b111, 0b101, 0b111, 0b101, 0b111],
    [0b111, 0b101, 0b111, 0b001, 0b111],
];

fn blend_pixel(image: &mut RgbaImage, x: u32, y: u32, overlay: Rgba<u8>) {
    let target = image.get_pixel_mut(x, y);
    let alpha = u16::from(overlay[3]);
    for channel in 0..3 {
        target[channel] = (((u16::from(overlay[channel]) * alpha)
            + (u16::from(target[channel]) * (255 - alpha)))
            / 255) as u8;
    }
    target[3] = target[3].max(overlay[3]);
}

fn materialize_run(
    plan: &RunDirectoryPlan,
    run_id: &str,
    cached: &[(&VerifiedReferenceImage, CachedReference)],
    cancellation: &CancellationToken,
) -> Result<PreprocessTaskResult, TaskFailure> {
    let parent = plan.root.parent().ok_or_else(|| {
        TaskFailure::new(
            TaskFailureKind::UnsafeOutputPath,
            "run root has no parent",
            None,
        )
    })?;
    let staging = parent.join(format!(
        ".{run_id}.preprocess-tmp-{}-{}",
        std::process::id(),
        next_sequence()
    ));
    ensure_direct_child(parent, &staging)?;
    fs::create_dir(&staging)
        .map_err(|error| output_io_failure("create run staging directory", &staging, error))?;
    let write_result = (|| {
        for relative in [
            "input/preprocessed",
            "analysis",
            "draft",
            "assets",
            "preview",
            "logs",
        ] {
            fs::create_dir_all(staging.join(relative)).map_err(|error| {
                output_io_failure(
                    "create planned run directory",
                    &staging.join(relative),
                    error,
                )
            })?;
        }
        let output_base = staging.join("input/preprocessed");
        for (verified, entry) in cached {
            cancellation.checkpoint()?;
            let destination = output_base.join(&verified.reference_id);
            ensure_direct_child(&output_base, &destination)?;
            fs::create_dir(&destination).map_err(|error| {
                output_io_failure(
                    "create preprocessed reference directory",
                    &destination,
                    error,
                )
            })?;
            for artifact in &entry.manifest.artifacts {
                copy_regular_file(
                    &entry.directory.join(&artifact.file_name),
                    &destination.join(&artifact.file_name),
                )?;
            }
            copy_regular_file(
                &entry.directory.join(CACHE_MANIFEST_FILE),
                &destination.join(CACHE_MANIFEST_FILE),
            )?;
        }
        let run_manifest = RunPreprocessManifest {
            protocol_version: PREPROCESS_PROTOCOL_VERSION,
            implementation_version: PREPROCESS_IMPLEMENTATION_VERSION,
            run_id,
            references: cached
                .iter()
                .map(|(verified, entry)| RunReferenceEntry {
                    reference_id: &verified.reference_id,
                    source_path: &verified.resolved_path,
                    source_sha256: &verified.sha256,
                    cache_key: &entry.cache_key,
                    artifact_directory: PathBuf::from(&verified.reference_id),
                })
                .collect(),
        };
        write_json_file(&output_base.join("manifest.json"), &run_manifest)
    })();
    if let Err(error) = write_result {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    if let Err(error) = cancellation.checkpoint() {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    fs::rename(&staging, &plan.root).map_err(|error| {
        let _ = fs::remove_dir_all(&staging);
        output_io_failure("commit preprocessed run directory", &plan.root, error)
    })?;

    let output_base = plan.root.join("input/preprocessed");
    Ok(PreprocessTaskResult {
        run_id: run_id.to_owned(),
        output_root: plan.root.clone(),
        manifest: output_base.join("manifest.json"),
        references: cached
            .iter()
            .map(|(verified, entry)| {
                let output_directory = output_base.join(&verified.reference_id);
                PreprocessedReferenceResult {
                    reference_id: verified.reference_id.clone(),
                    cache_key: entry.cache_key.clone(),
                    cache_hit: entry.cache_hit,
                    manifest: output_directory.join(CACHE_MANIFEST_FILE),
                    artifacts: entry
                        .manifest
                        .artifacts
                        .iter()
                        .map(|artifact| output_directory.join(&artifact.file_name))
                        .collect(),
                    output_directory,
                }
            })
            .collect(),
    })
}

fn controlled_generation_root(repository_root: &Path) -> Result<PathBuf, TaskFailure> {
    let repository_root = fs::canonicalize(repository_root).map_err(|error| {
        TaskFailure::new(
            TaskFailureKind::UnsafeOutputPath,
            format!("repository root cannot be resolved: {error}"),
            Some(repository_root.display().to_string()),
        )
    })?;
    let summary = repository_root.join("summary");
    reject_symlink_if_existing(&summary)?;
    fs::create_dir_all(&summary)
        .map_err(|error| output_io_failure("create summary directory", &summary, error))?;
    let generation = summary.join("ui-generation");
    reject_symlink_if_existing(&generation)?;
    fs::create_dir_all(&generation)
        .map_err(|error| output_io_failure("create generation directory", &generation, error))?;
    let canonical = fs::canonicalize(&generation)
        .map_err(|error| output_io_failure("resolve generation directory", &generation, error))?;
    if !canonical.starts_with(&repository_root) {
        return Err(TaskFailure::new(
            TaskFailureKind::UnsafeOutputPath,
            "generation directory resolves outside the repository",
            Some(generation.display().to_string()),
        ));
    }
    Ok(canonical)
}

fn controlled_child_directory(root: &Path, relative: &str) -> Result<PathBuf, TaskFailure> {
    let child = root.join(relative);
    if !child.starts_with(root) {
        return Err(TaskFailure::new(
            TaskFailureKind::UnsafeOutputPath,
            "controlled output path escapes its root",
            Some(child.display().to_string()),
        ));
    }
    let mut current = root.to_path_buf();
    for component in Path::new(relative).components() {
        current.push(component);
        reject_symlink_if_existing(&current)?;
    }
    fs::create_dir_all(&child)
        .map_err(|error| output_io_failure("create controlled output directory", &child, error))?;
    let canonical = fs::canonicalize(&child)
        .map_err(|error| output_io_failure("resolve controlled output directory", &child, error))?;
    if !canonical.starts_with(root) {
        return Err(TaskFailure::new(
            TaskFailureKind::UnsafeOutputPath,
            "controlled output directory resolves outside its root",
            Some(child.display().to_string()),
        ));
    }
    Ok(canonical)
}

fn reject_symlink_if_existing(path: &Path) -> Result<(), TaskFailure> {
    if fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err(TaskFailure::new(
            TaskFailureKind::UnsafeOutputPath,
            "controlled output path must not be a symbolic link",
            Some(path.display().to_string()),
        ));
    }
    Ok(())
}

fn ensure_direct_child(root: &Path, child: &Path) -> Result<(), TaskFailure> {
    if child.parent() == Some(root) {
        Ok(())
    } else {
        Err(TaskFailure::new(
            TaskFailureKind::UnsafeOutputPath,
            "output path is not a direct child of its controlled root",
            Some(child.display().to_string()),
        ))
    }
}

fn load_and_validate_cache(
    directory: &Path,
    expected_key: &str,
) -> Result<ReferencePreprocessManifest, TaskFailure> {
    reject_symlink_if_existing(directory)?;
    if !directory.is_dir() {
        return Err(cache_corrupt(directory, "cache entry is not a directory"));
    }
    let manifest_path = directory.join(CACHE_MANIFEST_FILE);
    let metadata = fs::symlink_metadata(&manifest_path).map_err(|error| {
        cache_corrupt(directory, format!("manifest metadata unavailable: {error}"))
    })?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > MAX_OPTIONS_BYTES
    {
        return Err(cache_corrupt(
            directory,
            "cache manifest is not a bounded regular file",
        ));
    }
    let bytes = fs::read(&manifest_path)
        .map_err(|error| cache_corrupt(directory, format!("manifest cannot be read: {error}")))?;
    let manifest: ReferencePreprocessManifest = serde_json::from_slice(&bytes)
        .map_err(|error| cache_corrupt(directory, format!("manifest is malformed: {error}")))?;
    if manifest.protocol_version != PREPROCESS_PROTOCOL_VERSION
        || manifest.implementation_version != PREPROCESS_IMPLEMENTATION_VERSION
        || manifest.cache_key != expected_key
        || !manifest.original_remains_authoritative
    {
        return Err(cache_corrupt(
            directory,
            "cache manifest identity does not match its key",
        ));
    }
    if manifest.artifacts.is_empty()
        || manifest.artifacts[0].kind != ArtifactKind::StandardPreview
        || manifest.artifacts[0].auxiliary_only
    {
        return Err(cache_corrupt(
            directory,
            "cache does not retain a primary standard preview",
        ));
    }
    let mut names = BTreeSet::new();
    for artifact in &manifest.artifacts {
        let name = Path::new(&artifact.file_name);
        if name.components().count() != 1 || !names.insert(artifact.file_name.as_str()) {
            return Err(cache_corrupt(
                directory,
                "cache artifact name is unsafe or duplicated",
            ));
        }
        let path = directory.join(name);
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| cache_corrupt(directory, format!("artifact missing: {error}")))?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(cache_corrupt(
                directory,
                "cache artifact is not a regular file",
            ));
        }
        let (length, hash) =
            hash_file(&path).map_err(|error| cache_corrupt(directory, error.to_string()))?;
        if length != artifact.byte_length || hash != artifact.sha256 {
            return Err(cache_corrupt(
                directory,
                "cache artifact hash or length changed",
            ));
        }
    }
    Ok(manifest)
}

fn copy_regular_file(source: &Path, destination: &Path) -> Result<(), TaskFailure> {
    let metadata = fs::symlink_metadata(source).map_err(|error| {
        cache_corrupt(
            source,
            format!("cached artifact cannot be inspected: {error}"),
        )
    })?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(cache_corrupt(
            source,
            "cached artifact is not a regular file",
        ));
    }
    let source_file = File::open(source).map_err(|error| {
        cache_corrupt(source, format!("cached artifact cannot be opened: {error}"))
    })?;
    let destination_file = File::create(destination).map_err(|error| {
        output_io_failure("create copied preprocess artifact", destination, error)
    })?;
    let mut reader = BufReader::new(source_file);
    let mut writer = BufWriter::new(destination_file);
    let copied = std::io::copy(&mut reader, &mut writer).map_err(|error| {
        output_io_failure("copy cached preprocess artifact", destination, error)
    })?;
    write_all_and_flush(&mut writer, &[]).map_err(|error| {
        output_io_failure("flush copied preprocess artifact", destination, error)
    })?;
    writer.get_ref().sync_all().map_err(|error| {
        output_io_failure("sync copied preprocess artifact", destination, error)
    })?;
    if copied != metadata.len() {
        return Err(output_io_failure(
            "copy cached preprocess artifact",
            destination,
            std::io::Error::new(
                std::io::ErrorKind::WriteZero,
                "copied byte count does not match the source",
            ),
        ));
    }
    let (_, source_hash) = hash_file(source)?;
    let (_, destination_hash) = hash_file(destination)?;
    if source_hash != destination_hash {
        return Err(output_io_failure(
            "verify copied preprocess artifact",
            destination,
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "copied artifact hash does not match the cache source",
            ),
        ));
    }
    Ok(())
}

fn write_json_file(path: &Path, value: &impl Serialize) -> Result<(), TaskFailure> {
    let encoded = serde_json::to_vec_pretty(value).map_err(|error| {
        TaskFailure::new(
            TaskFailureKind::InvalidInput,
            format!("JSON artifact cannot be serialized: {error}"),
            Some(path.display().to_string()),
        )
    })?;
    write_bytes_file(path, &encoded)
}

fn write_bytes_file(path: &Path, bytes: &[u8]) -> Result<(), TaskFailure> {
    let file = File::create(path)
        .map_err(|error| output_io_failure("create staged artifact", path, error))?;
    let mut writer = BufWriter::new(file);
    write_all_and_flush(&mut writer, bytes)
        .map_err(|error| output_io_failure("write and flush staged artifact", path, error))?;
    writer
        .get_ref()
        .sync_all()
        .map_err(|error| output_io_failure("sync staged artifact", path, error))
}

fn write_all_and_flush(writer: &mut impl Write, bytes: &[u8]) -> std::io::Result<()> {
    writer.write_all(bytes)?;
    writer.flush()
}

fn hash_file(path: &Path) -> Result<(u64, String), TaskFailure> {
    let file = File::open(path)
        .map_err(|error| output_io_failure("open artifact for hashing", path, error))?;
    let mut reader = BufReader::new(file);
    let mut digest = Sha256::new();
    let mut length = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|error| output_io_failure("hash artifact", path, error))?;
        if count == 0 {
            break;
        }
        length = length.saturating_add(count as u64);
        digest.update(&buffer[..count]);
    }
    Ok((length, format!("{:x}", digest.finalize())))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn round_half_up(value: f64) -> Result<u32, TaskFailure> {
    if !value.is_finite() || value < 0.0 || value > f64::from(u32::MAX) - 0.5 {
        return Err(TaskFailure::invalid("coordinate cannot be rounded to u32"));
    }
    Ok((value + 0.5).floor() as u32)
}

fn raster_edge(value: f64, bound: u32) -> u32 {
    round_half_up(value).unwrap_or(0).min(bound)
}

fn next_sequence() -> u64 {
    STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed)
}

fn default_preview_max_edge() -> u32 {
    DEFAULT_PREVIEW_MAX_EDGE
}

fn input_io_failure(action: &str, path: &Path, error: std::io::Error) -> TaskFailure {
    TaskFailure::new(
        TaskFailureKind::InvalidInput,
        format!("{action}: {error}"),
        Some(path.display().to_string()),
    )
}

fn image_io_failure(action: &str, path: &Path, error: std::io::Error) -> TaskFailure {
    TaskFailure::new(
        TaskFailureKind::ImageUnreadable,
        format!("{action}: {error}"),
        Some(path.display().to_string()),
    )
}

fn output_io_failure(action: &str, path: &Path, error: std::io::Error) -> TaskFailure {
    TaskFailure::new(
        TaskFailureKind::UnsafeOutputPath,
        format!("{action}: {error}"),
        Some(path.display().to_string()),
    )
}

fn corrupt_failure(path: &Path, message: impl Into<String>) -> TaskFailure {
    TaskFailure::new(
        TaskFailureKind::ImageCorrupt,
        message,
        Some(path.display().to_string()),
    )
}

fn metadata_mismatch(path: &Path, message: impl Into<String>) -> TaskFailure {
    TaskFailure::new(
        TaskFailureKind::ImageMetadataMismatch,
        message,
        Some(path.display().to_string()),
    )
}

fn cache_corrupt(path: &Path, message: impl Into<String>) -> TaskFailure {
    TaskFailure::new(
        TaskFailureKind::PreprocessCacheCorrupt,
        message,
        Some(path.display().to_string()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{ImageAuthorization, ImageInputMetadata, ImageProvenance};
    use image::{Rgb, RgbImage, codecs::jpeg::JpegEncoder};
    use serde_json::json;

    fn viewport() -> TargetViewport {
        TargetViewport {
            logical_width: 100.0,
            logical_height: 200.0,
            device_scale: 3.0,
        }
    }

    fn gradient(width: u32, height: u32) -> RgbaImage {
        RgbaImage::from_fn(width, height, |x, y| {
            Rgba([(x % 251) as u8, (y % 239) as u8, ((x + y) % 233) as u8, 255])
        })
    }

    fn png_bytes(image: &RgbaImage) -> Vec<u8> {
        let mut bytes = Vec::new();
        PngEncoder::new_with_quality(&mut bytes, CompressionType::Best, FilterType::Adaptive)
            .write_image(
                image.as_raw(),
                image.width(),
                image.height(),
                ColorType::Rgba8.into(),
            )
            .unwrap();
        bytes
    }

    fn jpeg_with_orientation(width: u32, height: u32, orientation: u16) -> Vec<u8> {
        let rgb = RgbImage::from_fn(width, height, |x, y| {
            Rgb([(x % 251) as u8, (y % 239) as u8, ((x + y) % 233) as u8])
        });
        let mut jpeg = Vec::new();
        JpegEncoder::new_with_quality(&mut jpeg, 90)
            .encode(rgb.as_raw(), width, height, ExtendedColorType::Rgb8)
            .unwrap();
        let mut tiff = Vec::new();
        tiff.extend_from_slice(b"II");
        tiff.extend_from_slice(&42_u16.to_le_bytes());
        tiff.extend_from_slice(&8_u32.to_le_bytes());
        tiff.extend_from_slice(&1_u16.to_le_bytes());
        tiff.extend_from_slice(&0x0112_u16.to_le_bytes());
        tiff.extend_from_slice(&3_u16.to_le_bytes());
        tiff.extend_from_slice(&1_u32.to_le_bytes());
        tiff.extend_from_slice(&orientation.to_le_bytes());
        tiff.extend_from_slice(&0_u16.to_le_bytes());
        tiff.extend_from_slice(&0_u32.to_le_bytes());
        let mut payload = b"Exif\0\0".to_vec();
        payload.extend_from_slice(&tiff);
        let length = u16::try_from(payload.len() + 2).unwrap();
        let mut output = vec![0xff, 0xd8, 0xff, 0xe1];
        output.extend_from_slice(&length.to_be_bytes());
        output.extend_from_slice(&payload);
        output.extend_from_slice(&jpeg[2..]);
        output
    }

    fn verified(
        path: &Path,
        bytes: &[u8],
        size: PixelSize,
        orientation: ImageOrientation,
    ) -> VerifiedReferenceImage {
        VerifiedReferenceImage {
            reference_id: "primary".to_owned(),
            resolved_path: path.to_path_buf(),
            byte_length: bytes.len() as u64,
            sha256: sha256_bytes(bytes),
            metadata: ImageInputMetadata {
                original_size: size,
                orientation,
                color_space: ImageColorSpace::Srgb,
                sha256: sha256_bytes(bytes),
                provenance: ImageProvenance {
                    source: "programmatic test fixture".to_owned(),
                    source_uri: None,
                    authorization: ImageAuthorization::AnalysisOnly,
                    license_reference: None,
                },
            },
        }
    }

    #[test]
    fn orientation_and_all_coordinate_spaces_round_trip() {
        let mapping = CoordinateMapping::new(
            PixelSize {
                width: 80,
                height: 120,
            },
            AppliedOrientation::Rotate90,
            PixelRect {
                x: 10,
                y: 20,
                width: 80,
                height: 40,
            },
            PixelSize {
                width: 40,
                height: 20,
            },
            viewport(),
        )
        .unwrap();
        assert_eq!(
            mapping.exif_normalized_size,
            PixelSize {
                width: 120,
                height: 80
            }
        );
        let raw = FloatPoint { x: 30.0, y: 50.0 };
        let logical = mapping
            .map_point(
                raw,
                CoordinateSpace::RawImagePixel,
                CoordinateSpace::TargetLogicalPixel,
            )
            .unwrap();
        assert_eq!(logical, FloatPoint { x: 75.0, y: 50.0 });
        let physical = mapping
            .map_point(
                logical,
                CoordinateSpace::TargetLogicalPixel,
                CoordinateSpace::DevicePhysicalPixel,
            )
            .unwrap();
        assert_eq!(physical, FloatPoint { x: 225.0, y: 150.0 });
        let returned = mapping
            .map_point(
                physical,
                CoordinateSpace::DevicePhysicalPixel,
                CoordinateSpace::RawImagePixel,
            )
            .unwrap();
        assert!((returned.x - raw.x).abs() < 1e-9);
        assert!((returned.y - raw.y).abs() < 1e-9);
        let raw_rect = mapping
            .map_rect(
                PixelRect {
                    x: 20,
                    y: 30,
                    width: 40,
                    height: 20,
                },
                CoordinateSpace::RawImagePixel,
            )
            .unwrap();
        assert_eq!(
            raw_rect,
            FloatRect {
                x: 30.0,
                y: 60.0,
                width: 20.0,
                height: 40.0,
            }
        );

        let crop_outside_raw = FloatPoint { x: 10.0, y: 10.0 };
        let crop_outside_exif = mapping
            .map_point(
                crop_outside_raw,
                CoordinateSpace::RawImagePixel,
                CoordinateSpace::ExifNormalizedPixel,
            )
            .unwrap();
        assert_eq!(crop_outside_exif, FloatPoint { x: 110.0, y: 10.0 });
        assert_eq!(
            mapping
                .map_point(
                    crop_outside_exif,
                    CoordinateSpace::ExifNormalizedPixel,
                    CoordinateSpace::RawImagePixel,
                )
                .unwrap(),
            crop_outside_raw
        );
        assert!(
            mapping
                .map_point(
                    crop_outside_exif,
                    CoordinateSpace::ExifNormalizedPixel,
                    CoordinateSpace::PreviewPixel,
                )
                .is_err()
        );
        assert!(
            mapping
                .map_point(
                    crop_outside_raw,
                    CoordinateSpace::RawImagePixel,
                    CoordinateSpace::TargetLogicalPixel,
                )
                .is_err()
        );
        assert_eq!(
            mapping
                .map_rect(
                    PixelRect {
                        x: 100,
                        y: 0,
                        width: 20,
                        height: 20,
                    },
                    CoordinateSpace::RawImagePixel,
                )
                .unwrap(),
            FloatRect {
                x: 0.0,
                y: 0.0,
                width: 20.0,
                height: 20.0,
            }
        );
        assert!(
            mapping
                .map_rect(
                    PixelRect {
                        x: 100,
                        y: 0,
                        width: 20,
                        height: 20,
                    },
                    CoordinateSpace::PreviewPixel,
                )
                .is_err()
        );
    }

    #[test]
    fn all_eight_orientation_transforms_round_trip_edges() {
        let raw = PixelSize {
            width: 80,
            height: 120,
        };
        let orientations = [
            AppliedOrientation::Normal,
            AppliedOrientation::Rotate90,
            AppliedOrientation::Rotate180,
            AppliedOrientation::Rotate270,
            AppliedOrientation::MirrorHorizontal,
            AppliedOrientation::MirrorVertical,
            AppliedOrientation::Rotate90MirrorHorizontal,
            AppliedOrientation::Rotate270MirrorHorizontal,
        ];
        for orientation in orientations {
            let normalized = orientation.raw_to_normalized(FloatPoint { x: 17.0, y: 83.0 }, raw);
            let returned = orientation.normalized_to_raw(normalized, raw);
            assert!((returned.x - 17.0).abs() < 1e-9, "{orientation:?}");
            assert!((returned.y - 83.0).abs() < 1e-9, "{orientation:?}");
        }
    }

    #[test]
    fn jpeg_exif_orientation_is_read_and_applied() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("rotated.jpg");
        let bytes = jpeg_with_orientation(80, 120, 6);
        fs::write(&path, &bytes).unwrap();
        let verified_image = verified(
            &path,
            &bytes,
            PixelSize {
                width: 80,
                height: 120,
            },
            ImageOrientation::Rotate90,
        );
        let cache = root.path().join("cache");
        fs::create_dir(&cache).unwrap();
        let result = preprocess_reference_cached(
            &verified_image,
            viewport(),
            ReferenceValidationProfile::PageReference,
            &ReferencePreprocessOptions::default(),
            &cache,
            &CancellationToken::default(),
        )
        .unwrap();
        assert_eq!(
            result.manifest.embedded_metadata.embedded_orientation,
            Some(AppliedOrientation::Rotate90)
        );
        assert_eq!(
            result.manifest.coordinate_mapping.exif_normalized_size,
            PixelSize {
                width: 120,
                height: 80
            }
        );

        let mismatched = verified(
            &path,
            &bytes,
            PixelSize {
                width: 80,
                height: 120,
            },
            ImageOrientation::Normal,
        );
        let failure = preprocess_reference_cached(
            &mismatched,
            viewport(),
            ReferenceValidationProfile::PageReference,
            &ReferencePreprocessOptions::default(),
            &cache,
            &CancellationToken::default(),
        )
        .unwrap_err();
        assert_eq!(failure.kind(), TaskFailureKind::ImageMetadataMismatch);
    }

    #[test]
    fn explicit_crop_regions_downsample_and_cache_are_deterministic() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("large.png");
        let bytes = png_bytes(&gradient(400, 200));
        fs::write(&path, &bytes).unwrap();
        let verified = verified(
            &path,
            &bytes,
            PixelSize {
                width: 400,
                height: 200,
            },
            ImageOrientation::Normal,
        );
        let options = ReferencePreprocessOptions {
            crop: Some(PixelRect {
                x: 20,
                y: 10,
                width: 360,
                height: 180,
            }),
            safe_area: Some(PixelRect {
                x: 30,
                y: 20,
                width: 340,
                height: 160,
            }),
            system_ui_exclusions: vec![PixelRect {
                x: 30,
                y: 20,
                width: 40,
                height: 20,
            }],
            preview: PreviewOptions { max_edge: 180 },
            auxiliary: AuxiliaryOptions {
                grid_spacing: Some(32),
                number_regions: true,
                high_contrast: true,
            },
        };
        let cache = root.path().join("cache");
        fs::create_dir(&cache).unwrap();
        let first = preprocess_reference_cached(
            &verified,
            viewport(),
            ReferenceValidationProfile::PageReference,
            &options,
            &cache,
            &CancellationToken::default(),
        )
        .unwrap();
        let second = preprocess_reference_cached(
            &verified,
            viewport(),
            ReferenceValidationProfile::PageReference,
            &options,
            &cache,
            &CancellationToken::default(),
        )
        .unwrap();
        assert!(!first.cache_hit);
        assert!(second.cache_hit);
        assert_eq!(first.cache_key, second.cache_key);
        assert_eq!(
            first.manifest.coordinate_mapping.preview_size,
            PixelSize {
                width: 180,
                height: 90
            }
        );
        assert_eq!(first.manifest.artifacts.len(), 3);
        assert!(!first.manifest.artifacts[0].auxiliary_only);
        assert!(
            first.manifest.artifacts[1..]
                .iter()
                .all(|artifact| artifact.auxiliary_only)
        );

        let changed_options = ReferencePreprocessOptions {
            preview: PreviewOptions { max_edge: 160 },
            ..options.clone()
        };
        let changed = preprocess_reference_cached(
            &verified,
            viewport(),
            ReferenceValidationProfile::PageReference,
            &changed_options,
            &cache,
            &CancellationToken::default(),
        )
        .unwrap();
        assert_ne!(first.cache_key, changed.cache_key);

        let huge_preview = preview_dimensions(
            PixelRect {
                x: 0,
                y: 0,
                width: 8_000,
                height: 8_000,
            },
            4_096,
        )
        .unwrap();
        assert_eq!(
            huge_preview,
            PixelSize {
                width: 2_048,
                height: 2_048
            }
        );
    }

    #[test]
    fn damaged_unsupported_small_ratio_blank_and_metadata_fail_stably() {
        let root = tempfile::tempdir().unwrap();
        let cache = root.path().join("cache");
        fs::create_dir(&cache).unwrap();

        let unsupported_path = root.path().join("unsupported.gif");
        let unsupported = b"GIF89a0123456789".to_vec();
        fs::write(&unsupported_path, &unsupported).unwrap();
        let failure = preprocess_reference_cached(
            &verified(
                &unsupported_path,
                &unsupported,
                PixelSize {
                    width: 64,
                    height: 64,
                },
                ImageOrientation::Normal,
            ),
            viewport(),
            ReferenceValidationProfile::PageReference,
            &ReferencePreprocessOptions::default(),
            &cache,
            &CancellationToken::default(),
        )
        .unwrap_err();
        assert_eq!(failure.kind(), TaskFailureKind::ImageUnsupportedFormat);

        let damaged_path = root.path().join("damaged.png");
        let mut damaged = png_bytes(&gradient(64, 64));
        damaged.truncate(40);
        fs::write(&damaged_path, &damaged).unwrap();
        let failure = preprocess_reference_cached(
            &verified(
                &damaged_path,
                &damaged,
                PixelSize {
                    width: 64,
                    height: 64,
                },
                ImageOrientation::Normal,
            ),
            viewport(),
            ReferenceValidationProfile::PageReference,
            &ReferencePreprocessOptions::default(),
            &cache,
            &CancellationToken::default(),
        )
        .unwrap_err();
        assert_eq!(failure.kind(), TaskFailureKind::ImageCorrupt);

        for (name, image, size, profile, expected) in [
            (
                "small",
                gradient(32, 32),
                PixelSize {
                    width: 32,
                    height: 32,
                },
                ReferenceValidationProfile::PageReference,
                TaskFailureKind::ImageTooSmall,
            ),
            (
                "ratio",
                gradient(704, 64),
                PixelSize {
                    width: 704,
                    height: 64,
                },
                ReferenceValidationProfile::PageReference,
                TaskFailureKind::ImageAspectRatioUnsupported,
            ),
            (
                "blank",
                RgbaImage::from_pixel(64, 64, Rgba([255, 255, 255, 255])),
                PixelSize {
                    width: 64,
                    height: 64,
                },
                ReferenceValidationProfile::PageReference,
                TaskFailureKind::ImageBlank,
            ),
        ] {
            let path = root.path().join(format!("{name}.png"));
            let bytes = png_bytes(&image);
            fs::write(&path, &bytes).unwrap();
            let failure = preprocess_reference_cached(
                &verified(&path, &bytes, size, ImageOrientation::Normal),
                viewport(),
                profile,
                &ReferencePreprocessOptions::default(),
                &cache,
                &CancellationToken::default(),
            )
            .unwrap_err();
            assert_eq!(failure.kind(), expected, "{name}");
        }

        let valid_path = root.path().join("metadata.png");
        let valid = png_bytes(&gradient(64, 64));
        fs::write(&valid_path, &valid).unwrap();
        let failure = preprocess_reference_cached(
            &verified(
                &valid_path,
                &valid,
                PixelSize {
                    width: 65,
                    height: 64,
                },
                ImageOrientation::Normal,
            ),
            viewport(),
            ReferenceValidationProfile::PageReference,
            &ReferencePreprocessOptions::default(),
            &cache,
            &CancellationToken::default(),
        )
        .unwrap_err();
        assert_eq!(failure.kind(), TaskFailureKind::ImageMetadataMismatch);
    }

    #[test]
    fn uniform_detail_is_not_silently_rejected_as_blank() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("swatch.png");
        let image = RgbaImage::from_pixel(16, 16, Rgba([42, 77, 99, 255]));
        let bytes = png_bytes(&image);
        fs::write(&path, &bytes).unwrap();
        let cache = root.path().join("cache");
        fs::create_dir(&cache).unwrap();
        let result = preprocess_reference_cached(
            &verified(
                &path,
                &bytes,
                PixelSize {
                    width: 16,
                    height: 16,
                },
                ImageOrientation::Normal,
            ),
            viewport(),
            ReferenceValidationProfile::DetailReference,
            &ReferencePreprocessOptions::default(),
            &cache,
            &CancellationToken::default(),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn invalid_regions_and_corrupt_cache_are_rejected() {
        let size = PixelSize {
            width: 100,
            height: 200,
        };
        let options = ReferencePreprocessOptions {
            crop: Some(PixelRect {
                x: 10,
                y: 20,
                width: 80,
                height: 160,
            }),
            safe_area: Some(PixelRect {
                x: 0,
                y: 0,
                width: 30,
                height: 30,
            }),
            ..Default::default()
        };
        assert!(validate_regions(&options, options.crop.unwrap(), size).is_err());

        let at_limit = ReferencePreprocessOptions {
            system_ui_exclusions: vec![
                PixelRect {
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                };
                MAX_SYSTEM_UI_EXCLUSION_REGIONS
            ],
            ..Default::default()
        };
        assert!(at_limit.validate_basic("options").is_ok());
        let over_limit = ReferencePreprocessOptions {
            system_ui_exclusions: vec![
                PixelRect {
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                };
                MAX_SYSTEM_UI_EXCLUSION_REGIONS + 1
            ],
            ..Default::default()
        };
        let failure = over_limit.validate_basic("options").unwrap_err();
        assert_eq!(failure.kind(), TaskFailureKind::InvalidInput);
        assert!(failure.message().contains("at most 64"));

        let root = tempfile::tempdir().unwrap();
        let options_path = root.path().join("over-limit-options.json");
        let document = PreprocessOptionsDocument {
            protocol_version: PREPROCESS_PROTOCOL_VERSION,
            defaults: over_limit,
            references: BTreeMap::new(),
        };
        fs::write(&options_path, serde_json::to_vec_pretty(&document).unwrap()).unwrap();
        let failure = preprocess_task(
            &root.path().join("missing-task.json"),
            Some(&options_path),
            root.path(),
            &CancellationToken::default(),
        )
        .unwrap_err();
        assert_eq!(failure.kind(), TaskFailureKind::InvalidInput);
        assert!(failure.message().contains("at most 64"));

        let directory = root.path().join("key");
        fs::create_dir(&directory).unwrap();
        fs::write(directory.join(CACHE_MANIFEST_FILE), b"{}").unwrap();
        let failure = load_and_validate_cache(&directory, "key").unwrap_err();
        assert_eq!(failure.kind(), TaskFailureKind::PreprocessCacheCorrupt);
    }

    #[test]
    fn staged_writes_propagate_flush_failures() {
        #[derive(Default)]
        struct FlushFailureWriter {
            bytes: Vec<u8>,
        }

        impl Write for FlushFailureWriter {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.bytes.extend_from_slice(bytes);
                Ok(bytes.len())
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "injected flush failure",
                ))
            }
        }

        let mut writer = FlushFailureWriter::default();
        let failure = write_all_and_flush(&mut writer, b"staged artifact").unwrap_err();
        assert_eq!(failure.kind(), std::io::ErrorKind::Other);
        assert_eq!(writer.bytes, b"staged artifact");
    }

    #[test]
    fn preprocess_task_materializes_only_the_ignored_tool_run() {
        let repository = tempfile::tempdir().unwrap();
        fs::create_dir(repository.path().join("summary")).unwrap();
        let image_path = repository.path().join("reference.png");
        let image_bytes = png_bytes(&gradient(128, 256));
        fs::write(&image_path, &image_bytes).unwrap();
        let hash = sha256_bytes(&image_bytes);
        let task_path = repository.path().join("task.json");
        let task = json!({
            "contract_version": 1,
            "run_id": "stage3-e2e",
            "primary_reference": {
                "reference_id": "primary",
                "path": "reference.png",
                "metadata": {
                    "original_size": { "width": 128, "height": 256 },
                    "orientation": "normal",
                    "color_space": "srgb",
                    "sha256": hash,
                    "provenance": {
                        "source": "programmatic test fixture",
                        "authorization": "analysis_only"
                    }
                }
            },
            "additional_references": [],
            "target_viewport": {
                "logical_width": 360.0,
                "logical_height": 720.0,
                "device_scale": 3.0
            },
            "visible_text": [],
            "must_preserve": [],
            "visual_preferences": {}
        });
        fs::write(&task_path, serde_json::to_vec_pretty(&task).unwrap()).unwrap();

        let result = preprocess_task(
            &task_path,
            None,
            repository.path(),
            &CancellationToken::default(),
        )
        .unwrap();
        let expected_generation_root = repository
            .path()
            .join("summary/ui-generation")
            .canonicalize()
            .unwrap();
        assert!(result.output_root.starts_with(expected_generation_root));
        assert!(result.manifest.is_file());
        assert_eq!(result.references.len(), 1);
        assert!(result.references[0].artifacts[0].is_file());
        assert!(image_path.is_file());
        assert!(!repository.path().join("project").exists());
        assert!(!repository.path().join("android").exists());

        let failure = preprocess_task(
            &task_path,
            None,
            repository.path(),
            &CancellationToken::default(),
        )
        .unwrap_err();
        assert_eq!(failure.kind(), TaskFailureKind::OutputDirectoryConflict);
    }
}
