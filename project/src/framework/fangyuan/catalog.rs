//! Schema-v1 data contracts for the Fangyuan surface and noise catalogs.
//!
//! This module intentionally stops at strict deserialization and local field
//! validation. Cross-catalog joins, image metadata, companion path handling,
//! and runtime resolution belong to later stages.

use std::collections::BTreeMap;

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
}
