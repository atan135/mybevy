//! Schema-v1 data contracts for the Fangyuan surface and noise catalogs.
//!
//! This module intentionally stops at strict deserialization and local field
//! validation. Cross-catalog joins, image metadata, companion path handling,
//! and runtime resolution belong to later stages.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::BufReader,
    path::{Component, Path, PathBuf},
};

use png::Decoder;
use serde::{Deserialize, Serialize};

pub const FANGYUAN_CATALOG_SCHEMA_VERSION: u32 = 1;
pub const FANGYUAN_CATALOG_ID_MAX_LEN: usize = 64;
pub const FANGYUAN_SURFACE_CATALOG_ID: &str = "fangyuan.base.surface";
pub const FANGYUAN_SURFACE_PRESET_CATALOG_ID: &str = "fangyuan.base.surface_material_presets";
pub const FANGYUAN_NOISE_CATALOG_ID: &str = "fangyuan.noise.procedural";
pub const FANGYUAN_CATALOG_MAX_EMISSIVE_INTENSITY: f32 = 16.0;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanSurfaceAtlasCatalog {
    pub schema_version: u32,
    pub catalog_id: String,
    pub display_name_zh: String,
    pub language: String,
    pub material_presets_file: String,
    pub description_zh: String,
    pub semantic_review: serde_json::Value,
    pub grid: FangyuanSurfaceGrid,
    pub channels: FangyuanSurfaceChannels,
    pub rendering_contract: serde_json::Value,
    pub ai_selection_contract: serde_json::Value,
    pub material_families: BTreeMap<String, String>,
    pub row_groups: Vec<FangyuanSurfaceRowGroup>,
    pub tiles: Vec<FangyuanSurfaceTile>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanNoiseAtlasCatalog {
    pub schema_version: u32,
    pub catalog_id: String,
    pub display_name_zh: String,
    pub language: String,
    pub description_zh: String,
    pub semantic_review: serde_json::Value,
    pub source_layout: serde_json::Value,
    pub grid: FangyuanNoiseGrid,
    pub texture: FangyuanNoiseTexture,
    pub rendering_contract: serde_json::Value,
    pub ai_selection_contract: serde_json::Value,
    pub families: BTreeMap<String, String>,
    pub usage_profiles: BTreeMap<String, String>,
    pub row_groups: Vec<FangyuanNoiseRowGroup>,
    pub tiles: Vec<FangyuanNoiseTile>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanSurfaceMaterialPresetCatalog {
    pub schema_version: u32,
    pub catalog_id: String,
    pub display_name_zh: String,
    pub language: String,
    pub description_zh: String,
    pub companions: FangyuanCatalogCompanions,
    pub integration_status: serde_json::Value,
    pub parameter_semantics: BTreeMap<String, String>,
    pub authoring_contract: serde_json::Value,
    pub family_defaults: BTreeMap<String, serde_json::Value>,
    pub presets: Vec<FangyuanSurfaceMaterialPreset>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanSurfaceGrid {
    pub coordinate_space: String,
    pub origin: String,
    pub x_direction: String,
    pub y_direction: String,
    pub order: String,
    pub columns: u16,
    pub rows: u16,
    pub atlas_width_px: u32,
    pub atlas_height_px: u32,
    pub cell_width_px: u32,
    pub cell_height_px: u32,
    pub padding_px_each_side: u32,
    pub content_width_px: u32,
    pub content_height_px: u32,
    pub index_formula: String,
    pub cell_pixel_rect_formula: String,
    pub content_pixel_rect_formula: String,
    pub texel_center_uv_formula: FangyuanTexelCenterUvFormula,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanNoiseGrid {
    pub coordinate_space: String,
    pub origin: String,
    pub x_direction: String,
    pub y_direction: String,
    pub order: String,
    pub columns: u16,
    pub rows: u16,
    pub atlas_width_px: u32,
    pub atlas_height_px: u32,
    pub cell_width_px: u32,
    pub cell_height_px: u32,
    pub padding_px_each_side: u32,
    pub content_width_px: u32,
    pub content_height_px: u32,
    pub available_tile_count: u16,
    pub reserved_tile_count: u16,
    pub available_ranges: Vec<FangyuanNoiseRange>,
    pub reserved_ranges: Vec<FangyuanNoiseRange>,
    pub index_formula: String,
    pub content_pixel_rect_formula: String,
    pub texel_center_uv_formula: FangyuanTexelCenterUvFormula,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanTexelCenterUvFormula {
    pub min: String,
    pub max: String,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanSurfaceChannels {
    pub base_color: FangyuanSurfaceChannel,
    pub normal: FangyuanSurfaceChannel,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanSurfaceChannel {
    pub file: String,
    pub semantic: String,
    pub dimensions_px: [u32; 2],
    pub color_space: String,
    pub required_for_tile: bool,
    #[serde(default)]
    pub decode: Option<String>,
    #[serde(default)]
    pub green_channel_direction: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanSurfaceRowGroup {
    pub id: String,
    pub y: u16,
    pub name_zh: String,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanSurfaceTile {
    pub tile_id: String,
    pub index: u16,
    pub x: u16,
    pub y: u16,
    pub row_group: String,
    pub material_family: String,
    pub name_zh: String,
    pub description_zh: String,
    pub search_text_zh: String,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanCatalogCompanions {
    pub surface_catalog_file: String,
    pub noise_catalog_file: String,
    pub base_color_file: String,
    pub normal_file: String,
    pub orm_texture_file: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanSurfaceMaterialPreset {
    pub preset_id: String,
    pub surface_tile_id: String,
    pub index: u16,
    pub x: u16,
    pub y: u16,
    pub name_zh: String,
    pub material_family: String,
    pub pbr_constants: FangyuanPbrConstants,
    #[serde(default)]
    pub normal: Option<FangyuanPresetNormal>,
    pub uv: FangyuanPresetUv,
    pub render_model: FangyuanCatalogRenderModel,
    pub preferred_roles: Vec<String>,
    pub recommended_primitive_kinds: Vec<String>,
    #[serde(default)]
    pub noise_modulation: Option<FangyuanNoiseModulation>,
    pub authoring: FangyuanPresetAuthoring,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanPbrConstants {
    pub perceptual_roughness: f32,
    pub metallic: f32,
    pub ambient_occlusion: f32,
    pub reflectance: f32,
    pub emissive_intensity: f32,
    pub alpha: f32,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanPresetNormal {
    pub enabled: bool,
    pub strength: f32,
    #[serde(default)]
    pub source_tile_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanPresetUv {
    pub reference_repeat_size_m: [f32; 2],
    pub projection: FangyuanCatalogProjection,
    pub orientation_policy: FangyuanCatalogOrientation,
    pub scale_status: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanCatalogProjection {
    Planar,
    PlanarOrTriplanar,
    Triplanar,
    SphericalOrBackground,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanCatalogOrientation {
    FreeRotation,
    PreserveAuthoredUp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanCatalogRenderModel {
    StandardPbr,
    StandardPbrOpaqueSurface,
    StandardPbrEmissive,
    SpecialWater,
    SpecialUnlitSky,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanNoiseModulation {
    pub tile_id: String,
    pub targets: Vec<String>,
    pub strength: f32,
    pub uv_scale_multiplier: f32,
    pub enabled_by_default: bool,
    pub parameter_status: String,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanPresetAuthoring {
    pub adjustments_zh: Vec<String>,
    pub confidence: f32,
    pub parameter_source: String,
    pub physical_status: String,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanNoiseRange {
    pub y_min: u16,
    pub y_max: u16,
    pub x_min: u16,
    pub x_max: u16,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanNoiseTexture {
    pub file: String,
    pub semantic: String,
    pub dimensions_px: [u32; 2],
    pub pixel_format: String,
    pub color_space: String,
    pub alpha: String,
    pub reserved_value_rgb: [u8; 3],
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanNoiseRowGroup {
    pub id: String,
    pub y: u16,
    pub name_zh: String,
    #[serde(default)]
    pub available_x_min: Option<u16>,
    #[serde(default)]
    pub available_x_max: Option<u16>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanNoiseTile {
    pub tile_id: String,
    pub index: u16,
    pub x: u16,
    pub y: u16,
    pub row_group: String,
    pub family: String,
    pub name_zh: String,
    pub description_zh: String,
    pub channel_mode: String,
    pub frequency: String,
    pub directionality: String,
    pub recommended_filter: String,
    pub suggested_uses: Vec<String>,
    pub identification: FangyuanNoiseIdentification,
    pub measured: FangyuanNoiseMeasured,
    pub source: FangyuanNoiseSource,
    pub search_text_zh: String,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanNoiseIdentification {
    pub basis: String,
    pub confidence: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanNoiseMeasured {
    pub luminance_mean: f32,
    pub luminance_stddev: f32,
    pub luminance_min: f32,
    pub luminance_max: f32,
    pub mean_chroma: f32,
    pub horizontal_gradient: f32,
    pub vertical_gradient: f32,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanNoiseSource {
    pub region: String,
    pub local_x: u16,
    pub local_y: u16,
    pub crop_px: FangyuanNoiseCrop,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanNoiseCrop {
    pub left: u32,
    pub top: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanCatalogFieldError {
    pub catalog_id: String,
    pub field_path: String,
    pub value: String,
    pub reason: FangyuanCatalogFieldErrorReason,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FangyuanCatalogFieldErrorReason {
    Empty,
    TooLong { max_len: usize },
    NonAscii,
    Uppercase,
    InvalidSyntax,
    NonFinite,
    Negative,
    OutOfRange { min: f32, max: f32 },
    NonPositive,
    UnsupportedSchemaVersion { expected: u32 },
    UnexpectedCatalogId { expected: String },
}

impl std::fmt::Display for FangyuanCatalogFieldError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} {}={} ({:?})",
            self.catalog_id, self.field_path, self.value, self.reason
        )
    }
}

impl std::error::Error for FangyuanCatalogFieldError {}

pub fn validate_surface_tile_id(
    catalog_id: &str,
    field_path: &str,
    value: &str,
) -> Result<(), FangyuanCatalogFieldError> {
    validate_coordinate_id(catalog_id, field_path, value, "surface")
}

pub fn validate_noise_tile_id(
    catalog_id: &str,
    field_path: &str,
    value: &str,
) -> Result<(), FangyuanCatalogFieldError> {
    validate_coordinate_id(catalog_id, field_path, value, "noise")
}

pub fn validate_surface_material_preset_id(
    catalog_id: &str,
    field_path: &str,
    value: &str,
) -> Result<(), FangyuanCatalogFieldError> {
    validate_id_common(catalog_id, field_path, value, |candidate| {
        let Some(rest) = candidate.strip_prefix("material:surface/") else {
            return false;
        };
        let Some((x, y)) = rest.split_once('/') else {
            return false;
        };
        valid_coordinate_component(x, 'x') && valid_coordinate_component(y, 'y')
    })
}

fn validate_coordinate_id(
    catalog_id: &str,
    field_path: &str,
    value: &str,
    prefix: &str,
) -> Result<(), FangyuanCatalogFieldError> {
    validate_id_common(catalog_id, field_path, value, |candidate| {
        let Some(rest) = candidate.strip_prefix(prefix) else {
            return false;
        };
        let Some(rest) = rest.strip_prefix("_x") else {
            return false;
        };
        let Some((x, y)) = rest.split_once("_y") else {
            return false;
        };
        valid_two_digits(x) && valid_two_digits(y)
    })
}

fn validate_id_common(
    catalog_id: &str,
    field_path: &str,
    value: &str,
    syntax: impl FnOnce(&str) -> bool,
) -> Result<(), FangyuanCatalogFieldError> {
    let error = |reason| FangyuanCatalogFieldError {
        catalog_id: catalog_id.to_string(),
        field_path: field_path.to_string(),
        value: value.to_string(),
        reason,
    };
    if value.is_empty() {
        return Err(error(FangyuanCatalogFieldErrorReason::Empty));
    }
    if value.len() > FANGYUAN_CATALOG_ID_MAX_LEN {
        return Err(error(FangyuanCatalogFieldErrorReason::TooLong {
            max_len: FANGYUAN_CATALOG_ID_MAX_LEN,
        }));
    }
    if !value.is_ascii() {
        return Err(error(FangyuanCatalogFieldErrorReason::NonAscii));
    }
    if value.chars().any(char::is_uppercase) {
        return Err(error(FangyuanCatalogFieldErrorReason::Uppercase));
    }
    if !syntax(value) {
        return Err(error(FangyuanCatalogFieldErrorReason::InvalidSyntax));
    }
    Ok(())
}

fn valid_coordinate_component(value: &str, prefix: char) -> bool {
    value.len() == 3 && value.starts_with(prefix) && valid_two_digits(&value[1..])
}

fn valid_two_digits(value: &str) -> bool {
    value.len() == 2 && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn validate_catalog_header(
    schema_version: u32,
    catalog_id: &str,
    expected_catalog_id: &str,
) -> Result<(), FangyuanCatalogFieldError> {
    if schema_version != FANGYUAN_CATALOG_SCHEMA_VERSION {
        return Err(FangyuanCatalogFieldError {
            catalog_id: catalog_id.to_string(),
            field_path: "schema_version".to_string(),
            value: schema_version.to_string(),
            reason: FangyuanCatalogFieldErrorReason::UnsupportedSchemaVersion {
                expected: FANGYUAN_CATALOG_SCHEMA_VERSION,
            },
        });
    }
    if catalog_id != expected_catalog_id {
        return Err(FangyuanCatalogFieldError {
            catalog_id: catalog_id.to_string(),
            field_path: "catalog_id".to_string(),
            value: catalog_id.to_string(),
            reason: FangyuanCatalogFieldErrorReason::UnexpectedCatalogId {
                expected: expected_catalog_id.to_string(),
            },
        });
    }
    Ok(())
}

impl FangyuanSurfaceAtlasCatalog {
    pub fn validate_schema(&self) -> Result<(), FangyuanCatalogFieldError> {
        validate_catalog_header(
            self.schema_version,
            &self.catalog_id,
            FANGYUAN_SURFACE_CATALOG_ID,
        )?;
        for (index, tile) in self.tiles.iter().enumerate() {
            validate_surface_tile_id(
                &self.catalog_id,
                &format!("tiles[{index}].tile_id"),
                &tile.tile_id,
            )?;
        }
        Ok(())
    }
}

impl FangyuanSurfaceMaterialPresetCatalog {
    pub fn validate_schema(&self) -> Result<(), FangyuanCatalogFieldError> {
        validate_catalog_header(
            self.schema_version,
            &self.catalog_id,
            FANGYUAN_SURFACE_PRESET_CATALOG_ID,
        )?;
        for (index, preset) in self.presets.iter().enumerate() {
            validate_surface_material_preset_id(
                &self.catalog_id,
                &format!("presets[{index}].preset_id"),
                &preset.preset_id,
            )?;
            validate_surface_tile_id(
                &self.catalog_id,
                &format!("presets[{index}].surface_tile_id"),
                &preset.surface_tile_id,
            )?;
            validate_pbr_constants(&self.catalog_id, index, &preset.pbr_constants)?;
            if let Some(normal) = &preset.normal {
                validate_non_negative_finite(
                    &self.catalog_id,
                    &format!("presets[{index}].normal.strength"),
                    normal.strength,
                )?;
            }
            for (axis, value) in preset.uv.reference_repeat_size_m.into_iter().enumerate() {
                validate_positive_finite(
                    &self.catalog_id,
                    &format!("presets[{index}].uv.reference_repeat_size_m[{axis}]"),
                    value,
                )?;
            }
            if let Some(noise) = &preset.noise_modulation {
                validate_noise_tile_id(
                    &self.catalog_id,
                    &format!("presets[{index}].noise_modulation.tile_id"),
                    &noise.tile_id,
                )?;
                validate_non_negative_finite(
                    &self.catalog_id,
                    &format!("presets[{index}].noise_modulation.strength"),
                    noise.strength,
                )?;
                validate_positive_finite(
                    &self.catalog_id,
                    &format!("presets[{index}].noise_modulation.uv_scale_multiplier"),
                    noise.uv_scale_multiplier,
                )?;
            }
        }
        Ok(())
    }
}

impl FangyuanNoiseAtlasCatalog {
    pub fn validate_schema(&self) -> Result<(), FangyuanCatalogFieldError> {
        validate_catalog_header(
            self.schema_version,
            &self.catalog_id,
            FANGYUAN_NOISE_CATALOG_ID,
        )?;
        for (index, tile) in self.tiles.iter().enumerate() {
            validate_noise_tile_id(
                &self.catalog_id,
                &format!("tiles[{index}].tile_id"),
                &tile.tile_id,
            )?;
            validate_unit_finite(
                &self.catalog_id,
                &format!("tiles[{index}].identification.confidence"),
                tile.identification.confidence,
            )?;
            for (field, value) in [
                ("luminance_mean", tile.measured.luminance_mean),
                ("luminance_stddev", tile.measured.luminance_stddev),
                ("luminance_min", tile.measured.luminance_min),
                ("luminance_max", tile.measured.luminance_max),
                ("mean_chroma", tile.measured.mean_chroma),
                ("horizontal_gradient", tile.measured.horizontal_gradient),
                ("vertical_gradient", tile.measured.vertical_gradient),
            ] {
                validate_unit_finite(
                    &self.catalog_id,
                    &format!("tiles[{index}].measured.{field}"),
                    value,
                )?;
            }
        }
        Ok(())
    }
}

/// Filesystem locations used by the development-time catalog validator.
///
/// These paths are loader inputs only. AI and blueprint data continue to carry
/// stable IDs, never these paths.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanCatalogAssetPaths {
    pub atlas_root: PathBuf,
    pub surface_catalog_path: PathBuf,
    pub preset_catalog_path: PathBuf,
    pub noise_catalog_path: PathBuf,
}

impl FangyuanCatalogAssetPaths {
    pub fn from_fangyuan_asset_root(root: impl Into<PathBuf>) -> Self {
        let fangyuan_root = root.into();
        let atlas_root = fangyuan_root.join("atlases");
        Self {
            surface_catalog_path: atlas_root.join("base/surface_atlas.json"),
            preset_catalog_path: atlas_root.join("base/surface_material_presets.json"),
            noise_catalog_path: atlas_root.join("noise/noise_atlas.json"),
            atlas_root,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanCatalogValidationReport {
    pub findings: Vec<FangyuanCatalogValidationFinding>,
}

impl FangyuanCatalogValidationReport {
    pub fn is_empty(&self) -> bool {
        self.findings.is_empty()
    }

    fn push(
        &mut self,
        catalog_id: impl Into<String>,
        code: &'static str,
        field_path: impl Into<String>,
        value: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.findings.push(FangyuanCatalogValidationFinding {
            catalog_id: catalog_id.into(),
            code,
            field_path: field_path.into(),
            value: value.into(),
            message: message.into(),
        });
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanCatalogValidationFinding {
    pub catalog_id: String,
    pub code: &'static str,
    pub field_path: String,
    pub value: String,
    pub message: String,
}

impl std::fmt::Display for FangyuanCatalogValidationReport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} catalog validation finding(s)",
            self.findings.len()
        )
    }
}

impl std::error::Error for FangyuanCatalogValidationReport {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FangyuanPngMetadata {
    width: u32,
    height: u32,
}

/// Validate all three parsed catalogs as one atomic dependency bundle.
///
/// The function returns every finding and never returns a partially accepted
/// registry. Callers must treat any `Err` as catalog unavailable.
pub fn validate_fangyuan_catalog_bundle(
    surface: &FangyuanSurfaceAtlasCatalog,
    presets: &FangyuanSurfaceMaterialPresetCatalog,
    noise: &FangyuanNoiseAtlasCatalog,
    paths: &FangyuanCatalogAssetPaths,
) -> Result<(), FangyuanCatalogValidationReport> {
    let mut report = FangyuanCatalogValidationReport { findings: vec![] };
    validate_surface_catalog(&mut report, surface);
    validate_preset_catalog(&mut report, surface, presets);
    validate_noise_catalog(&mut report, noise);
    validate_noise_references(&mut report, presets, noise);
    validate_catalog_images(&mut report, surface, noise, paths);
    validate_preset_companions(&mut report, presets, paths);

    if report.is_empty() {
        Ok(())
    } else {
        Err(report)
    }
}

fn validate_surface_catalog(
    report: &mut FangyuanCatalogValidationReport,
    catalog: &FangyuanSurfaceAtlasCatalog,
) {
    if let Err(error) = catalog.validate_schema() {
        push_field_error(report, error);
    }
    if catalog.tiles.len() != 256 {
        report.push(
            &catalog.catalog_id,
            "surface_tile_count",
            "tiles",
            catalog.tiles.len().to_string(),
            "surface catalog must contain exactly 256 tiles",
        );
    }
    let mut ids = BTreeSet::new();
    let mut indexes = BTreeSet::new();
    let mut coordinates = BTreeSet::new();
    for (position, tile) in catalog.tiles.iter().enumerate() {
        if !ids.insert(tile.tile_id.clone()) {
            report.push(
                &catalog.catalog_id,
                "duplicate_id",
                format!("tiles[{position}].tile_id"),
                &tile.tile_id,
                "surface tile IDs must be unique",
            );
        }
        if !indexes.insert(tile.index) {
            report.push(
                &catalog.catalog_id,
                "duplicate_index",
                format!("tiles[{position}].index"),
                tile.index.to_string(),
                "surface tile indexes must be unique",
            );
        }
        if tile.x >= 16 || tile.y >= 16 {
            report.push(
                &catalog.catalog_id,
                "coordinate_out_of_bounds",
                format!("tiles[{position}]"),
                format!("x={},y={}", tile.x, tile.y),
                "surface tile coordinates must be within the 16x16 grid",
            );
        }
        if !coordinates.insert((tile.x, tile.y)) {
            report.push(
                &catalog.catalog_id,
                "duplicate_coordinate",
                format!("tiles[{position}]"),
                format!("x={},y={}", tile.x, tile.y),
                "surface tile coordinates must be unique",
            );
        }
        let expected_index = u32::from(tile.y) * 16 + u32::from(tile.x);
        if u32::from(tile.index) != expected_index {
            report.push(
                &catalog.catalog_id,
                "index_coordinate_mismatch",
                format!("tiles[{position}]"),
                format!("index={},x={},y={}", tile.index, tile.x, tile.y),
                "surface tile index must equal y * 16 + x",
            );
        }
    }
}

fn validate_preset_catalog(
    report: &mut FangyuanCatalogValidationReport,
    surface: &FangyuanSurfaceAtlasCatalog,
    catalog: &FangyuanSurfaceMaterialPresetCatalog,
) {
    if let Err(error) = catalog.validate_schema() {
        push_field_error(report, error);
    }
    if catalog.presets.len() != 256 {
        report.push(
            &catalog.catalog_id,
            "preset_count",
            "presets",
            catalog.presets.len().to_string(),
            "surface preset catalog must contain exactly 256 presets",
        );
    }
    let surface_by_id: BTreeMap<&str, &FangyuanSurfaceTile> = surface
        .tiles
        .iter()
        .map(|tile| (tile.tile_id.as_str(), tile))
        .collect();
    let mut preset_ids = BTreeSet::new();
    let mut referenced_surface_ids = BTreeSet::new();
    for (position, preset) in catalog.presets.iter().enumerate() {
        if !preset_ids.insert(preset.preset_id.clone()) {
            report.push(
                &catalog.catalog_id,
                "duplicate_id",
                format!("presets[{position}].preset_id"),
                &preset.preset_id,
                "surface preset IDs must be unique",
            );
        }
        let Some(surface_tile) = surface_by_id.get(preset.surface_tile_id.as_str()) else {
            report.push(
                &catalog.catalog_id,
                "missing_surface_tile",
                format!("presets[{position}].surface_tile_id"),
                &preset.surface_tile_id,
                "surface preset must reference an existing surface tile",
            );
            continue;
        };
        if !referenced_surface_ids.insert(preset.surface_tile_id.as_str()) {
            report.push(
                &catalog.catalog_id,
                "duplicate_surface_reference",
                format!("presets[{position}].surface_tile_id"),
                &preset.surface_tile_id,
                "each surface tile must be referenced by exactly one preset",
            );
        }
        if preset.index != surface_tile.index
            || preset.x != surface_tile.x
            || preset.y != surface_tile.y
        {
            report.push(
                &catalog.catalog_id,
                "preset_surface_coordinate_mismatch",
                format!("presets[{position}]"),
                format!(
                    "preset=({}, {}, {}), surface=({}, {}, {})",
                    preset.index,
                    preset.x,
                    preset.y,
                    surface_tile.index,
                    surface_tile.x,
                    surface_tile.y
                ),
                "preset index and coordinates must match the referenced surface tile",
            );
        }
        if let Some(normal) = &preset.normal {
            if let Some(source_tile_id) = &normal.source_tile_id {
                if source_tile_id != &preset.surface_tile_id {
                    report.push(
                        &catalog.catalog_id,
                        "normal_surface_mismatch",
                        format!("presets[{position}].normal.source_tile_id"),
                        source_tile_id,
                        "normal source must be empty or reference the preset surface tile",
                    );
                }
            }
        }
        validate_preset_parameters(report, catalog, position, preset);
    }
    if referenced_surface_ids.len() != surface_by_id.len() {
        report.push(
            &catalog.catalog_id,
            "surface_reference_coverage",
            "presets[].surface_tile_id",
            format!(
                "referenced={},expected={}",
                referenced_surface_ids.len(),
                surface_by_id.len()
            ),
            "surface preset references must cover every surface tile exactly once",
        );
        for tile_id in surface_by_id.keys() {
            if !referenced_surface_ids.contains(tile_id) {
                report.push(
                    &catalog.catalog_id,
                    "missing_surface_reference",
                    "presets[].surface_tile_id",
                    (*tile_id).to_string(),
                    "surface tile has no matching material preset",
                );
            }
        }
    }
}

fn validate_preset_parameters(
    report: &mut FangyuanCatalogValidationReport,
    catalog: &FangyuanSurfaceMaterialPresetCatalog,
    position: usize,
    preset: &FangyuanSurfaceMaterialPreset,
) {
    let constants = &preset.pbr_constants;
    for (field, value) in [
        ("perceptual_roughness", constants.perceptual_roughness),
        ("metallic", constants.metallic),
        ("ambient_occlusion", constants.ambient_occlusion),
        ("reflectance", constants.reflectance),
        ("alpha", constants.alpha),
    ] {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            report.push(
                &catalog.catalog_id,
                "parameter_out_of_range",
                format!("presets[{position}].pbr_constants.{field}"),
                value.to_string(),
                "PBR unit parameter must be finite and within 0..=1",
            );
        }
    }
    if !constants.emissive_intensity.is_finite()
        || !(0.0..=FANGYUAN_CATALOG_MAX_EMISSIVE_INTENSITY).contains(&constants.emissive_intensity)
    {
        report.push(
            &catalog.catalog_id,
            "parameter_out_of_range",
            format!("presets[{position}].pbr_constants.emissive_intensity"),
            constants.emissive_intensity.to_string(),
            "emissive intensity must be finite and within 0..=16",
        );
    }
    if let Some(normal) = &preset.normal {
        if !normal.strength.is_finite() || normal.strength < 0.0 {
            report.push(
                &catalog.catalog_id,
                "parameter_out_of_range",
                format!("presets[{position}].normal.strength"),
                normal.strength.to_string(),
                "normal strength must be finite and non-negative",
            );
        }
    }
    for (axis, value) in preset.uv.reference_repeat_size_m.into_iter().enumerate() {
        if !value.is_finite() || value <= 0.0 {
            report.push(
                &catalog.catalog_id,
                "parameter_out_of_range",
                format!("presets[{position}].uv.reference_repeat_size_m[{axis}]"),
                value.to_string(),
                "reference repeat size must be finite and positive",
            );
        }
    }
    if (constants.ambient_occlusion - 1.0).abs() > f32::EPSILON {
        report.push(
            &catalog.catalog_id,
            "ambient_occlusion_not_unit",
            format!("presets[{position}].pbr_constants.ambient_occlusion"),
            constants.ambient_occlusion.to_string(),
            "schema v1 requires ambient occlusion to remain 1.0",
        );
    }
    if let Some(noise) = &preset.noise_modulation {
        if !noise.strength.is_finite() || !(0.0..=1.0).contains(&noise.strength) {
            report.push(
                &catalog.catalog_id,
                "parameter_out_of_range",
                format!("presets[{position}].noise_modulation.strength"),
                noise.strength.to_string(),
                "noise strength must be finite and within 0..=1",
            );
        }
        if !noise.uv_scale_multiplier.is_finite() || noise.uv_scale_multiplier <= 0.0 {
            report.push(
                &catalog.catalog_id,
                "parameter_out_of_range",
                format!("presets[{position}].noise_modulation.uv_scale_multiplier"),
                noise.uv_scale_multiplier.to_string(),
                "noise UV scale multiplier must be finite and positive",
            );
        }
    }
}

fn validate_noise_catalog(
    report: &mut FangyuanCatalogValidationReport,
    catalog: &FangyuanNoiseAtlasCatalog,
) {
    if let Err(error) = catalog.validate_schema() {
        push_field_error(report, error);
    }
    if catalog.tiles.len() != 122 {
        report.push(
            &catalog.catalog_id,
            "noise_tile_count",
            "tiles",
            catalog.tiles.len().to_string(),
            "noise catalog must contain exactly 122 valid tiles",
        );
    }
    if catalog.grid.available_tile_count != 122 || catalog.grid.reserved_tile_count != 134 {
        report.push(
            &catalog.catalog_id,
            "noise_grid_count",
            "grid",
            format!(
                "available={},reserved={}",
                catalog.grid.available_tile_count, catalog.grid.reserved_tile_count
            ),
            "noise grid must declare 122 available and 134 reserved cells",
        );
    }
    validate_noise_grid_ranges(report, catalog);
    let mut ids = BTreeSet::new();
    let mut indexes = BTreeSet::new();
    let mut coordinates = BTreeSet::new();
    for (position, tile) in catalog.tiles.iter().enumerate() {
        if !ids.insert(tile.tile_id.clone()) {
            report.push(
                &catalog.catalog_id,
                "duplicate_id",
                format!("tiles[{position}].tile_id"),
                &tile.tile_id,
                "noise tile IDs must be unique",
            );
        }
        if !indexes.insert(tile.index) {
            report.push(
                &catalog.catalog_id,
                "duplicate_index",
                format!("tiles[{position}].index"),
                tile.index.to_string(),
                "noise tile indexes must be unique",
            );
        }
        if tile.x >= 16 || tile.y >= 16 {
            report.push(
                &catalog.catalog_id,
                "coordinate_out_of_bounds",
                format!("tiles[{position}]"),
                format!("x={},y={}", tile.x, tile.y),
                "noise tile coordinates must be within the 16x16 grid",
            );
        }
        if !coordinates.insert((tile.x, tile.y)) {
            report.push(
                &catalog.catalog_id,
                "duplicate_coordinate",
                format!("tiles[{position}]"),
                format!("x={},y={}", tile.x, tile.y),
                "noise tile coordinates must be unique",
            );
        }
        let expected_index = u32::from(tile.y) * 16 + u32::from(tile.x);
        if u32::from(tile.index) != expected_index {
            report.push(
                &catalog.catalog_id,
                "index_coordinate_mismatch",
                format!("tiles[{position}]"),
                format!("index={},x={},y={}", tile.index, tile.x, tile.y),
                "noise tile index must equal y * 16 + x",
            );
        }
        if !noise_cell_is_available(&catalog.grid, tile.x, tile.y) {
            report.push(
                &catalog.catalog_id,
                "reserved_noise_tile",
                format!("tiles[{position}]"),
                tile.tile_id.clone(),
                "noise tiles may not occupy reserved cells",
            );
        }
    }
}

fn validate_noise_grid_ranges(
    report: &mut FangyuanCatalogValidationReport,
    catalog: &FangyuanNoiseAtlasCatalog,
) {
    let mut available = BTreeSet::new();
    let mut reserved = BTreeSet::new();
    collect_noise_range_cells(
        report,
        &catalog.catalog_id,
        "grid.available_ranges",
        &catalog.grid.available_ranges,
        &mut available,
    );
    collect_noise_range_cells(
        report,
        &catalog.catalog_id,
        "grid.reserved_ranges",
        &catalog.grid.reserved_ranges,
        &mut reserved,
    );
    if available.len() != usize::from(catalog.grid.available_tile_count) {
        report.push(
            &catalog.catalog_id,
            "noise_available_count_mismatch",
            "grid.available_ranges",
            format!(
                "cells={},declared={}",
                available.len(),
                catalog.grid.available_tile_count
            ),
            "available range cells must match grid.available_tile_count",
        );
    }
    if reserved.len() != usize::from(catalog.grid.reserved_tile_count) {
        report.push(
            &catalog.catalog_id,
            "noise_reserved_count_mismatch",
            "grid.reserved_ranges",
            format!(
                "cells={},declared={}",
                reserved.len(),
                catalog.grid.reserved_tile_count
            ),
            "reserved range cells must match grid.reserved_tile_count",
        );
    }
    for cell in available.intersection(&reserved) {
        report.push(
            &catalog.catalog_id,
            "noise_range_overlap",
            "grid",
            format!("x={},y={}", cell.0, cell.1),
            "available and reserved noise ranges must not overlap",
        );
    }
    for y in 0..16u16 {
        for x in 0..16u16 {
            if !available.contains(&(x, y)) && !reserved.contains(&(x, y)) {
                report.push(
                    &catalog.catalog_id,
                    "noise_range_coverage",
                    "grid",
                    format!("x={},y={}", x, y),
                    "available and reserved noise ranges must cover every 16x16 cell",
                );
            }
        }
    }
}

fn collect_noise_range_cells(
    report: &mut FangyuanCatalogValidationReport,
    catalog_id: &str,
    field_path: &str,
    ranges: &[FangyuanNoiseRange],
    cells: &mut BTreeSet<(u16, u16)>,
) {
    for (index, range) in ranges.iter().enumerate() {
        if range.x_min > range.x_max
            || range.y_min > range.y_max
            || range.x_max >= 16
            || range.y_max >= 16
        {
            report.push(
                catalog_id,
                "noise_range_invalid",
                format!("{field_path}[{index}]"),
                format!(
                    "x={}..{},y={}..{}",
                    range.x_min, range.x_max, range.y_min, range.y_max
                ),
                "noise ranges must be ordered and remain within the 16x16 grid",
            );
            continue;
        }
        for y in range.y_min..=range.y_max {
            for x in range.x_min..=range.x_max {
                if !cells.insert((x, y)) {
                    report.push(
                        catalog_id,
                        "noise_range_overlap",
                        format!("{field_path}[{index}]"),
                        format!("x={},y={}", x, y),
                        "noise ranges of the same kind must not overlap",
                    );
                }
            }
        }
    }
}

fn noise_cell_is_available(grid: &FangyuanNoiseGrid, x: u16, y: u16) -> bool {
    grid.available_ranges.iter().any(|range| {
        (range.x_min..=range.x_max).contains(&x) && (range.y_min..=range.y_max).contains(&y)
    })
}

fn validate_noise_references(
    report: &mut FangyuanCatalogValidationReport,
    presets: &FangyuanSurfaceMaterialPresetCatalog,
    noise: &FangyuanNoiseAtlasCatalog,
) {
    let valid_noise_ids: BTreeSet<&str> = noise
        .tiles
        .iter()
        .map(|tile| tile.tile_id.as_str())
        .collect();
    for (position, preset) in presets.presets.iter().enumerate() {
        let Some(modulation) = &preset.noise_modulation else {
            continue;
        };
        if !valid_noise_ids.contains(modulation.tile_id.as_str()) {
            report.push(
                &presets.catalog_id,
                "invalid_noise_reference",
                format!("presets[{position}].noise_modulation.tile_id"),
                &modulation.tile_id,
                "noise modulation must reference an available noise tile",
            );
        }
    }
}

fn validate_catalog_images(
    report: &mut FangyuanCatalogValidationReport,
    surface: &FangyuanSurfaceAtlasCatalog,
    noise: &FangyuanNoiseAtlasCatalog,
    paths: &FangyuanCatalogAssetPaths,
) {
    let surface_dir = paths
        .surface_catalog_path
        .parent()
        .unwrap_or(Path::new("."));
    validate_png_asset(
        report,
        &surface.catalog_id,
        "channels.base_color.file",
        &surface.channels.base_color.file,
        surface_dir,
        surface.channels.base_color.dimensions_px,
        &surface.channels.base_color.color_space,
        "srgb",
    );
    validate_png_asset(
        report,
        &surface.catalog_id,
        "channels.normal.file",
        &surface.channels.normal.file,
        surface_dir,
        surface.channels.normal.dimensions_px,
        &surface.channels.normal.color_space,
        "linear",
    );
    let noise_dir = paths.noise_catalog_path.parent().unwrap_or(Path::new("."));
    validate_png_asset(
        report,
        &noise.catalog_id,
        "texture.file",
        &noise.texture.file,
        noise_dir,
        noise.texture.dimensions_px,
        &noise.texture.color_space,
        "linear",
    );
}

fn validate_png_asset(
    report: &mut FangyuanCatalogValidationReport,
    catalog_id: &str,
    field_path: &str,
    relative_file: &str,
    base_dir: &Path,
    declared_dimensions: [u32; 2],
    declared_color_space: &str,
    expected_color_space: &str,
) {
    let Ok(path) = normalize_catalog_relative_path(base_dir, relative_file, base_dir) else {
        report.push(
            catalog_id,
            "image_path_invalid",
            field_path,
            relative_file,
            "image path must be a safe catalog-relative path",
        );
        return;
    };
    let metadata = match read_png_metadata(&path) {
        Ok(metadata) => metadata,
        Err(message) => {
            report.push(
                catalog_id,
                "image_missing_or_invalid",
                field_path,
                path.display().to_string(),
                message,
            );
            return;
        }
    };
    if [metadata.width, metadata.height] != declared_dimensions
        || [metadata.width, metadata.height] != [1024, 1024]
    {
        report.push(
            catalog_id,
            "image_dimensions_mismatch",
            field_path,
            format!("{}x{}", metadata.width, metadata.height),
            format!(
                "PNG must be 1024x1024 and match declared {:?}",
                declared_dimensions
            ),
        );
    }
    if declared_color_space != expected_color_space {
        report.push(
            catalog_id,
            "image_color_space_mismatch",
            field_path,
            declared_color_space,
            format!("catalog color space must be {expected_color_space}"),
        );
    }
}

fn read_png_metadata(path: &Path) -> Result<FangyuanPngMetadata, String> {
    let file = File::open(path).map_err(|error| format!("cannot open PNG: {error}"))?;
    let decoder = Decoder::new(BufReader::new(file));
    let reader = decoder
        .read_info()
        .map_err(|error| format!("cannot decode PNG header: {error}"))?;
    let info = reader.info();
    Ok(FangyuanPngMetadata {
        width: info.width,
        height: info.height,
    })
}

fn validate_preset_companions(
    report: &mut FangyuanCatalogValidationReport,
    catalog: &FangyuanSurfaceMaterialPresetCatalog,
    paths: &FangyuanCatalogAssetPaths,
) {
    let base_dir = paths.preset_catalog_path.parent().unwrap_or(Path::new("."));
    for (field_path, value) in [
        (
            "companions.surface_catalog_file",
            Some(catalog.companions.surface_catalog_file.as_str()),
        ),
        (
            "companions.noise_catalog_file",
            Some(catalog.companions.noise_catalog_file.as_str()),
        ),
        (
            "companions.base_color_file",
            Some(catalog.companions.base_color_file.as_str()),
        ),
        (
            "companions.normal_file",
            Some(catalog.companions.normal_file.as_str()),
        ),
        (
            "companions.orm_texture_file",
            catalog.companions.orm_texture_file.as_deref(),
        ),
    ] {
        let Some(value) = value else { continue };
        let path = match normalize_catalog_relative_path(base_dir, value, &paths.atlas_root) {
            Ok(path) => path,
            Err(message) => {
                report.push(
                    &catalog.catalog_id,
                    "companion_path_invalid",
                    field_path,
                    value,
                    message,
                );
                continue;
            }
        };
        if !path.is_file() {
            report.push(
                &catalog.catalog_id,
                "companion_missing",
                field_path,
                path.display().to_string(),
                "companion path does not point to a file",
            );
        }
    }
}

pub fn normalize_catalog_relative_path(
    base_dir: &Path,
    relative: &str,
    controlled_root: &Path,
) -> Result<PathBuf, String> {
    if relative.trim().is_empty() {
        return Err("companion path must not be empty".to_string());
    }
    if relative.contains('\0') || relative.contains('\\') {
        return Err("companion path must use forward slash segments".to_string());
    }
    let lower = relative.to_ascii_lowercase();
    if relative.contains(':')
        || relative.contains("://")
        || lower.contains("%2f")
        || lower.contains("%5c")
    {
        return Err(
            "companion path must not be a URI, drive path, or encoded separator".to_string(),
        );
    }
    let candidate = lexical_normalize(base_dir, relative)?;
    let root = lexical_normalize_path(controlled_root)?;
    if !candidate.starts_with(&root) {
        return Err(format!(
            "companion path escapes controlled root {}",
            root.display()
        ));
    }
    Ok(candidate)
}

fn lexical_normalize(base_dir: &Path, relative: &str) -> Result<PathBuf, String> {
    let mut output = lexical_normalize_path(base_dir)?;
    for segment in relative.split('/') {
        match segment {
            "" => return Err("companion path contains an empty segment".to_string()),
            "." => {}
            ".." => {
                if !output.pop() {
                    return Err("companion path traverses above its base directory".to_string());
                }
            }
            normal => output.push(normal),
        }
    }
    Ok(output)
}

fn lexical_normalize_path(path: &Path) -> Result<PathBuf, String> {
    let mut output = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => output.push(prefix.as_os_str()),
            Component::RootDir => output.push(Path::new(std::path::MAIN_SEPARATOR_STR)),
            Component::Normal(segment) => output.push(segment),
            Component::CurDir => {}
            Component::ParentDir => {
                output.pop();
            }
        }
    }
    Ok(output)
}

fn push_field_error(
    report: &mut FangyuanCatalogValidationReport,
    error: FangyuanCatalogFieldError,
) {
    let message = error.to_string();
    report.push(
        error.catalog_id,
        "schema_field_invalid",
        error.field_path,
        error.value,
        message,
    );
}

fn validate_pbr_constants(
    catalog_id: &str,
    index: usize,
    constants: &FangyuanPbrConstants,
) -> Result<(), FangyuanCatalogFieldError> {
    for (field, value) in [
        ("perceptual_roughness", constants.perceptual_roughness),
        ("metallic", constants.metallic),
        ("ambient_occlusion", constants.ambient_occlusion),
        ("reflectance", constants.reflectance),
        ("alpha", constants.alpha),
    ] {
        validate_unit_finite(
            catalog_id,
            &format!("presets[{index}].pbr_constants.{field}"),
            value,
        )?;
    }
    validate_bounded_finite(
        catalog_id,
        &format!("presets[{index}].pbr_constants.emissive_intensity"),
        constants.emissive_intensity,
        0.0,
        FANGYUAN_CATALOG_MAX_EMISSIVE_INTENSITY,
    )?;
    Ok(())
}

fn validate_unit_finite(
    catalog_id: &str,
    field_path: &str,
    value: f32,
) -> Result<(), FangyuanCatalogFieldError> {
    if !value.is_finite() {
        return Err(FangyuanCatalogFieldError {
            catalog_id: catalog_id.to_string(),
            field_path: field_path.to_string(),
            value: value.to_string(),
            reason: FangyuanCatalogFieldErrorReason::NonFinite,
        });
    }
    if !(0.0..=1.0).contains(&value) {
        return Err(FangyuanCatalogFieldError {
            catalog_id: catalog_id.to_string(),
            field_path: field_path.to_string(),
            value: value.to_string(),
            reason: FangyuanCatalogFieldErrorReason::OutOfRange { min: 0.0, max: 1.0 },
        });
    }
    Ok(())
}

fn validate_bounded_finite(
    catalog_id: &str,
    field_path: &str,
    value: f32,
    min: f32,
    max: f32,
) -> Result<(), FangyuanCatalogFieldError> {
    if !value.is_finite() {
        return Err(FangyuanCatalogFieldError {
            catalog_id: catalog_id.to_string(),
            field_path: field_path.to_string(),
            value: value.to_string(),
            reason: FangyuanCatalogFieldErrorReason::NonFinite,
        });
    }
    if !(min..=max).contains(&value) {
        return Err(FangyuanCatalogFieldError {
            catalog_id: catalog_id.to_string(),
            field_path: field_path.to_string(),
            value: value.to_string(),
            reason: FangyuanCatalogFieldErrorReason::OutOfRange { min, max },
        });
    }
    Ok(())
}

fn validate_non_negative_finite(
    catalog_id: &str,
    field_path: &str,
    value: f32,
) -> Result<(), FangyuanCatalogFieldError> {
    if !value.is_finite() {
        return Err(FangyuanCatalogFieldError {
            catalog_id: catalog_id.to_string(),
            field_path: field_path.to_string(),
            value: value.to_string(),
            reason: FangyuanCatalogFieldErrorReason::NonFinite,
        });
    }
    if value < 0.0 {
        return Err(FangyuanCatalogFieldError {
            catalog_id: catalog_id.to_string(),
            field_path: field_path.to_string(),
            value: value.to_string(),
            reason: FangyuanCatalogFieldErrorReason::Negative,
        });
    }
    Ok(())
}

fn validate_positive_finite(
    catalog_id: &str,
    field_path: &str,
    value: f32,
) -> Result<(), FangyuanCatalogFieldError> {
    validate_non_negative_finite(catalog_id, field_path, value)?;
    if value == 0.0 {
        return Err(FangyuanCatalogFieldError {
            catalog_id: catalog_id.to_string(),
            field_path: field_path.to_string(),
            value: value.to_string(),
            reason: FangyuanCatalogFieldErrorReason::NonPositive,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(path: &str) -> String {
        std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
            .unwrap()
    }

    #[test]
    fn schema_v1_catalogs_parse_and_validate() {
        let surface: FangyuanSurfaceAtlasCatalog =
            serde_json::from_str(&asset("assets/fangyuan/atlases/base/surface_atlas.json"))
                .unwrap();
        let presets: FangyuanSurfaceMaterialPresetCatalog = serde_json::from_str(&asset(
            "assets/fangyuan/atlases/base/surface_material_presets.json",
        ))
        .unwrap();
        let noise: FangyuanNoiseAtlasCatalog =
            serde_json::from_str(&asset("assets/fangyuan/atlases/noise/noise_atlas.json")).unwrap();

        surface.validate_schema().unwrap();
        presets.validate_schema().unwrap();
        noise.validate_schema().unwrap();
        assert_eq!(surface.tiles.len(), 256);
        assert_eq!(presets.presets.len(), 256);
        assert_eq!(noise.tiles.len(), 122);
        assert_eq!(
            presets
                .presets
                .iter()
                .filter(|preset| preset.noise_modulation.is_none())
                .count(),
            8
        );
    }

    #[test]
    fn unknown_top_level_field_is_rejected() {
        let mut value: serde_json::Value =
            serde_json::from_str(&asset("assets/fangyuan/atlases/base/surface_atlas.json"))
                .unwrap();
        value["unexpected"] = serde_json::json!(true);
        let error = serde_json::from_value::<FangyuanSurfaceAtlasCatalog>(value).unwrap_err();
        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn missing_required_field_is_rejected() {
        let mut value: serde_json::Value = serde_json::from_str(&asset(
            "assets/fangyuan/atlases/base/surface_material_presets.json",
        ))
        .unwrap();
        value.as_object_mut().unwrap().remove("presets");
        let error =
            serde_json::from_value::<FangyuanSurfaceMaterialPresetCatalog>(value).unwrap_err();
        assert!(error.to_string().contains("missing field"));
    }

    #[test]
    fn id_errors_keep_catalog_path_and_value() {
        let error = validate_surface_material_preset_id(
            FANGYUAN_SURFACE_PRESET_CATALOG_ID,
            "presets[3].preset_id",
            "Material:surface/x00/y00",
        )
        .unwrap_err();
        assert_eq!(error.catalog_id, FANGYUAN_SURFACE_PRESET_CATALOG_ID);
        assert_eq!(error.field_path, "presets[3].preset_id");
        assert_eq!(error.value, "Material:surface/x00/y00");
        assert_eq!(error.reason, FangyuanCatalogFieldErrorReason::Uppercase);
    }

    #[test]
    fn invalid_float_values_are_rejected_by_schema_validation() {
        let mut value: serde_json::Value = serde_json::from_str(&asset(
            "assets/fangyuan/atlases/base/surface_material_presets.json",
        ))
        .unwrap();
        value["presets"][0]["pbr_constants"]["metallic"] = serde_json::json!(1.5);
        let catalog =
            serde_json::from_value::<FangyuanSurfaceMaterialPresetCatalog>(value).unwrap();
        let error = catalog.validate_schema().unwrap_err();
        assert_eq!(error.field_path, "presets[0].pbr_constants.metallic");
        assert!(matches!(
            error.reason,
            FangyuanCatalogFieldErrorReason::OutOfRange { .. }
        ));

        let mut value: serde_json::Value = serde_json::from_str(&asset(
            "assets/fangyuan/atlases/base/surface_material_presets.json",
        ))
        .unwrap();
        value["presets"][0]["normal"]["strength"] = serde_json::json!(-0.1);
        let catalog =
            serde_json::from_value::<FangyuanSurfaceMaterialPresetCatalog>(value).unwrap();
        let error = catalog.validate_schema().unwrap_err();
        assert_eq!(error.field_path, "presets[0].normal.strength");
        assert_eq!(error.reason, FangyuanCatalogFieldErrorReason::Negative);
    }

    #[test]
    fn wrong_schema_and_catalog_ids_are_rejected() {
        let mut value: serde_json::Value =
            serde_json::from_str(&asset("assets/fangyuan/atlases/noise/noise_atlas.json")).unwrap();
        value["schema_version"] = serde_json::json!(2);
        let catalog = serde_json::from_value::<FangyuanNoiseAtlasCatalog>(value).unwrap();
        assert!(matches!(
            catalog.validate_schema().unwrap_err().reason,
            FangyuanCatalogFieldErrorReason::UnsupportedSchemaVersion { .. }
        ));

        let mut value: serde_json::Value =
            serde_json::from_str(&asset("assets/fangyuan/atlases/noise/noise_atlas.json")).unwrap();
        value["catalog_id"] = serde_json::json!("fangyuan.noise.wrong");
        let catalog = serde_json::from_value::<FangyuanNoiseAtlasCatalog>(value).unwrap();
        assert!(matches!(
            catalog.validate_schema().unwrap_err().reason,
            FangyuanCatalogFieldErrorReason::UnexpectedCatalogId { .. }
        ));
    }

    fn fixture_catalogs() -> (
        FangyuanSurfaceAtlasCatalog,
        FangyuanSurfaceMaterialPresetCatalog,
        FangyuanNoiseAtlasCatalog,
    ) {
        (
            serde_json::from_str(&asset("assets/fangyuan/atlases/base/surface_atlas.json"))
                .unwrap(),
            serde_json::from_str(&asset(
                "assets/fangyuan/atlases/base/surface_material_presets.json",
            ))
            .unwrap(),
            serde_json::from_str(&asset("assets/fangyuan/atlases/noise/noise_atlas.json")).unwrap(),
        )
    }

    #[test]
    fn current_catalog_bundle_and_png_metadata_validate_atomically() {
        let (surface, presets, noise) = fixture_catalogs();
        let paths = FangyuanCatalogAssetPaths::from_fangyuan_asset_root(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fangyuan"),
        );
        validate_fangyuan_catalog_bundle(&surface, &presets, &noise, &paths).unwrap();
    }

    #[test]
    fn duplicate_and_reserved_entries_return_structured_findings() {
        let (mut surface, presets, mut noise) = fixture_catalogs();
        surface.tiles[1].tile_id = surface.tiles[0].tile_id.clone();
        noise.tiles[0].tile_id = "noise_x15_y15".to_string();
        noise.tiles[0].index = 255;
        noise.tiles[0].x = 15;
        noise.tiles[0].y = 15;
        let paths = FangyuanCatalogAssetPaths::from_fangyuan_asset_root(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fangyuan"),
        );
        let report =
            validate_fangyuan_catalog_bundle(&surface, &presets, &noise, &paths).unwrap_err();
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "duplicate_id")
        );
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "reserved_noise_tile")
        );
        assert!(!report.is_empty());
    }

    #[test]
    fn companion_paths_allow_catalog_relative_noise_but_reject_escape_and_uri() {
        let root = std::path::Path::new("workspace/assets/fangyuan/atlases");
        let base = root.join("base");
        assert_eq!(
            normalize_catalog_relative_path(&base, "../noise/noise_atlas.json", root).unwrap(),
            root.join("noise/noise_atlas.json")
        );
        for value in [
            "../../outside.json",
            "C:/outside.json",
            "https://example.invalid/catalog.json",
            "../noise/%2fescape.json",
            "../noise\\escape.json",
        ] {
            assert!(
                normalize_catalog_relative_path(&base, value, root).is_err(),
                "{value} should be rejected"
            );
        }
    }

    #[test]
    fn grid_bounds_coordinate_uniqueness_and_noise_range_coverage_are_checked() {
        let (mut surface, mut presets, mut noise) = fixture_catalogs();
        surface.tiles[0].x = 16;
        surface.tiles[0].index = 16;
        noise.tiles[0].x = 16;
        noise.tiles[0].index = 16;
        noise.grid.available_ranges[0].x_min = 1;
        noise.grid.available_ranges[0].x_max = 12;
        noise.grid.reserved_ranges[0].x_min = 12;
        presets.presets[0].surface_tile_id = "surface_x15_y15".to_string();
        let paths = FangyuanCatalogAssetPaths::from_fangyuan_asset_root(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fangyuan"),
        );
        let report =
            validate_fangyuan_catalog_bundle(&surface, &presets, &noise, &paths).unwrap_err();
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "coordinate_out_of_bounds")
        );
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "noise_range_overlap")
        );
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "noise_range_coverage")
        );
    }

    #[test]
    fn preset_surface_references_and_noise_range_counts_are_one_to_one() {
        let (surface, mut presets, mut noise) = fixture_catalogs();
        presets.presets[1].surface_tile_id = presets.presets[0].surface_tile_id.clone();
        noise.grid.available_ranges[0].x_max = 10;
        let paths = FangyuanCatalogAssetPaths::from_fangyuan_asset_root(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fangyuan"),
        );
        let report =
            validate_fangyuan_catalog_bundle(&surface, &presets, &noise, &paths).unwrap_err();
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "duplicate_surface_reference")
        );
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "surface_reference_coverage")
        );
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "noise_available_count_mismatch")
        );
    }
}
