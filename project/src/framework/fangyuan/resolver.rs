//! Read-only runtime resolution for the Fangyuan surface catalogs.
//!
//! The surface preset table is deliberately separate from
//! [`FangyuanMaterialProfileRegistry`]. Material profiles continue to own the
//! primitive colour/alpha/emissive policy, while this table owns the catalog
//! backed surface metadata and atlas indices.

use std::{collections::BTreeMap, fs, path::PathBuf};

use serde::{Deserialize, Serialize};

use super::{
    FangyuanCatalogAssetPaths, FangyuanCatalogProjection, FangyuanCatalogRenderModel,
    FangyuanNoiseAtlasCatalog, FangyuanSurfaceAtlasCatalog, FangyuanSurfaceMaterialPresetCatalog,
    fangyuan_bake_hash_bytes, validate_fangyuan_catalog_bundle,
    validate_surface_material_preset_id,
};

pub const FANGYUAN_SURFACE_PRESET_DEFAULT_ID: &str = "surface:default";
pub const FANGYUAN_SURFACE_PRESET_DEFAULT_ROUGHNESS: f32 = 0.6;
pub const FANGYUAN_SURFACE_PRESET_DEFAULT_METALLIC: f32 = 0.0;
pub const FANGYUAN_SURFACE_PRESET_DEFAULT_AMBIENT_OCCLUSION: f32 = 1.0;

/// Runtime-only flags derived from catalog fields. No arbitrary shader or
/// texture parameters can be supplied by a blueprint.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanSurfacePresetFlags {
    pub normal_enabled: bool,
    pub noise_available: bool,
    pub noise_enabled_by_default: bool,
    pub emissive: bool,
    pub opaque: bool,
    pub special_render_model: bool,
}

impl FangyuanSurfacePresetFlags {
    pub const fn is_noise_allowed(self) -> bool {
        self.noise_available
    }

    pub const fn bits(self) -> u16 {
        (self.normal_enabled as u16)
            | ((self.noise_available as u16) << 1)
            | ((self.noise_enabled_by_default as u16) << 2)
            | ((self.emissive as u16) << 3)
            | ((self.opaque as u16) << 4)
            | ((self.special_render_model as u16) << 5)
    }

    pub const fn contains(self, other: Self) -> bool {
        self.bits() & other.bits() == other.bits()
    }
}

/// A catalog preset after all string references have been resolved to stable
/// atlas indices. The value is immutable after registry construction.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedFangyuanSurfacePreset {
    pub preset_id: String,
    pub preset_index: u16,
    pub surface_tile_id: String,
    pub surface_tile_index: u16,
    pub noise_tile_id: Option<String>,
    pub noise_tile_index: Option<u16>,
    pub roughness: f32,
    pub metallic: f32,
    pub ambient_occlusion: f32,
    pub reflectance: f32,
    pub normal_strength: f32,
    pub repeat_size_m: [f32; 2],
    pub projection: FangyuanCatalogProjection,
    pub projection_hint: FangyuanCatalogProjection,
    pub render_model: FangyuanCatalogRenderModel,
    pub flags: FangyuanSurfacePresetFlags,
}

impl ResolvedFangyuanSurfacePreset {
    /// Safe visual fallback. Primitive tint is applied by the primitive
    /// material composition path; this table intentionally carries no colour.
    pub fn default_surface() -> Self {
        Self {
            preset_id: FANGYUAN_SURFACE_PRESET_DEFAULT_ID.to_string(),
            preset_index: 0,
            surface_tile_id: FANGYUAN_SURFACE_PRESET_DEFAULT_ID.to_string(),
            surface_tile_index: 0,
            noise_tile_id: None,
            noise_tile_index: None,
            roughness: FANGYUAN_SURFACE_PRESET_DEFAULT_ROUGHNESS,
            metallic: FANGYUAN_SURFACE_PRESET_DEFAULT_METALLIC,
            ambient_occlusion: FANGYUAN_SURFACE_PRESET_DEFAULT_AMBIENT_OCCLUSION,
            reflectance: 0.5,
            normal_strength: 0.0,
            repeat_size_m: [1.0, 1.0],
            projection: FangyuanCatalogProjection::Planar,
            projection_hint: FangyuanCatalogProjection::Planar,
            render_model: FangyuanCatalogRenderModel::StandardPbr,
            flags: FangyuanSurfacePresetFlags {
                opaque: true,
                ..Default::default()
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FangyuanSurfacePresetFallbackReason {
    UnknownPresetId,
    InvalidPresetId,
    NoiseSuggestionUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanSurfacePresetFallback {
    pub requested_id: String,
    pub resolved_id: String,
    pub reason: FangyuanSurfacePresetFallbackReason,
    pub source_location: Option<String>,
}

/// Result of one runtime lookup. A missing or malformed ID only affects this
/// primitive; the registry remains usable for all other objects.
#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanSurfacePresetResolution {
    pub preset: ResolvedFangyuanSurfacePreset,
    pub requested_id: Option<String>,
    pub requested_preset_id: Option<String>,
    pub resolved_id: String,
    pub resolved_preset_id: String,
    pub fallback: Option<FangyuanSurfacePresetFallback>,
    pub fallback_reason: Option<FangyuanSurfacePresetFallbackReason>,
    pub source_location: Option<String>,
}

impl FangyuanSurfacePresetResolution {
    pub fn used_fallback(&self) -> bool {
        self.fallback.is_some()
    }

    pub fn reason(&self) -> Option<&FangyuanSurfacePresetFallbackReason> {
        self.fallback.as_ref().map(|fallback| &fallback.reason)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanSurfacePresetCacheIdentity {
    pub content_hash: u64,
    pub dependency_hash: u64,
    pub cache_hash: u64,
}

impl FangyuanSurfacePresetCacheIdentity {
    pub fn matches(&self, other: &Self) -> bool {
        self == other
    }
}

/// Hash all catalog JSON and image bytes as labelled dependencies. Length
/// prefixes avoid collisions caused by concatenating adjacent inputs.
pub fn fangyuan_surface_preset_cache_identity_from_bytes(
    surface_catalog_json: &[u8],
    preset_catalog_json: &[u8],
    noise_catalog_json: &[u8],
    base_color_png: &[u8],
    normal_png: &[u8],
    noise_png: &[u8],
) -> FangyuanSurfacePresetCacheIdentity {
    let parts = [
        ("surface_catalog.json", surface_catalog_json),
        ("surface_material_presets.json", preset_catalog_json),
        ("noise_atlas.json", noise_catalog_json),
        ("surface_color.png", base_color_png),
        ("surface_normal.png", normal_png),
        ("procedural_noise.png", noise_png),
    ];
    let content_hash = hash_labelled_parts(&parts);
    let dependency_hash = hash_labelled_hashes(&parts);
    let cache_hash = hash_labelled_parts(&[
        ("content_hash", content_hash.to_be_bytes().as_slice()),
        ("dependency_hash", dependency_hash.to_be_bytes().as_slice()),
    ]);
    FangyuanSurfacePresetCacheIdentity {
        content_hash,
        dependency_hash,
        cache_hash,
    }
}

pub fn fangyuan_surface_preset_cache_identity(
    surface_catalog_json: &[u8],
    preset_catalog_json: &[u8],
    noise_catalog_json: &[u8],
    base_color_png: &[u8],
    normal_png: &[u8],
    noise_png: &[u8],
) -> FangyuanSurfacePresetCacheIdentity {
    fangyuan_surface_preset_cache_identity_from_bytes(
        surface_catalog_json,
        preset_catalog_json,
        noise_catalog_json,
        base_color_png,
        normal_png,
        noise_png,
    )
}

fn hash_labelled_parts(parts: &[(&str, &[u8])]) -> u64 {
    let mut bytes = Vec::new();
    for (label, value) in parts {
        bytes.extend_from_slice(&(label.len() as u64).to_be_bytes());
        bytes.extend_from_slice(label.as_bytes());
        bytes.extend_from_slice(&(value.len() as u64).to_be_bytes());
        bytes.extend_from_slice(value);
    }
    fangyuan_bake_hash_bytes(&bytes)
}

fn hash_labelled_hashes(parts: &[(&str, &[u8])]) -> u64 {
    let mut bytes = Vec::new();
    for (label, value) in parts {
        let hash = fangyuan_bake_hash_bytes(value);
        bytes.extend_from_slice(&(label.len() as u64).to_be_bytes());
        bytes.extend_from_slice(label.as_bytes());
        bytes.extend_from_slice(&hash.to_be_bytes());
    }
    fangyuan_bake_hash_bytes(&bytes)
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanSurfacePresetRegistry {
    presets: BTreeMap<String, ResolvedFangyuanSurfacePreset>,
    ordered_preset_ids: Vec<String>,
    identity: FangyuanSurfacePresetCacheIdentity,
    default_surface: ResolvedFangyuanSurfacePreset,
}

pub type FangyuanSurfacePresetTable = FangyuanSurfacePresetRegistry;

impl FangyuanSurfacePresetRegistry {
    pub fn from_catalog_bundle(
        surface: FangyuanSurfaceAtlasCatalog,
        presets: FangyuanSurfaceMaterialPresetCatalog,
        noise: FangyuanNoiseAtlasCatalog,
        paths: &FangyuanCatalogAssetPaths,
    ) -> Result<Self, FangyuanSurfacePresetRegistryError> {
        validate_fangyuan_catalog_bundle(&surface, &presets, &noise, paths)
            .map_err(FangyuanSurfacePresetRegistryError::ValidationFailed)?;

        let surface_json = read_bytes(&paths.surface_catalog_path)?;
        let preset_json = read_bytes(&paths.preset_catalog_path)?;
        let noise_json = read_bytes(&paths.noise_catalog_path)?;
        let surface_dir = paths
            .surface_catalog_path
            .parent()
            .unwrap_or(std::path::Path::new("."));
        let noise_dir = paths
            .noise_catalog_path
            .parent()
            .unwrap_or(std::path::Path::new("."));
        let base_color_path = super::normalize_catalog_relative_path(
            surface_dir,
            &surface.channels.base_color.file,
            &paths.atlas_root,
        )
        .map_err(|reason| FangyuanSurfacePresetRegistryError::AssetPath {
            path: surface.channels.base_color.file.clone(),
            reason,
        })?;
        let normal_path = super::normalize_catalog_relative_path(
            surface_dir,
            &surface.channels.normal.file,
            &paths.atlas_root,
        )
        .map_err(|reason| FangyuanSurfacePresetRegistryError::AssetPath {
            path: surface.channels.normal.file.clone(),
            reason,
        })?;
        let noise_path = super::normalize_catalog_relative_path(
            noise_dir,
            &noise.texture.file,
            &paths.atlas_root,
        )
        .map_err(|reason| FangyuanSurfacePresetRegistryError::AssetPath {
            path: noise.texture.file.clone(),
            reason,
        })?;
        let identity = fangyuan_surface_preset_cache_identity_from_bytes(
            &surface_json,
            &preset_json,
            &noise_json,
            &read_bytes(&base_color_path)?,
            &read_bytes(&normal_path)?,
            &read_bytes(&noise_path)?,
        );
        Ok(Self::from_validated_catalogs(
            surface, presets, noise, identity,
        ))
    }

    pub fn load_from_paths(
        paths: &FangyuanCatalogAssetPaths,
    ) -> Result<Self, FangyuanSurfacePresetRegistryError> {
        let surface: FangyuanSurfaceAtlasCatalog = parse_json(&paths.surface_catalog_path)?;
        let presets: FangyuanSurfaceMaterialPresetCatalog = parse_json(&paths.preset_catalog_path)?;
        let noise: FangyuanNoiseAtlasCatalog = parse_json(&paths.noise_catalog_path)?;
        Self::from_catalog_bundle(surface, presets, noise, paths)
    }

    pub fn from_validated_catalogs(
        surface: FangyuanSurfaceAtlasCatalog,
        presets: FangyuanSurfaceMaterialPresetCatalog,
        noise: FangyuanNoiseAtlasCatalog,
        identity: FangyuanSurfacePresetCacheIdentity,
    ) -> Self {
        let surface_indices: BTreeMap<String, u16> = surface
            .tiles
            .iter()
            .map(|tile| (tile.tile_id.clone(), tile.index))
            .collect();
        let noise_indices: BTreeMap<String, u16> = noise
            .tiles
            .iter()
            .map(|tile| (tile.tile_id.clone(), tile.index))
            .collect();
        let mut ordered = presets.presets.clone();
        ordered.sort_by_key(|preset| (preset.index, preset.preset_id.clone()));
        let ordered_preset_ids = ordered
            .iter()
            .map(|preset| preset.preset_id.clone())
            .collect();
        let entries = ordered
            .into_iter()
            .map(|preset| {
                let surface_tile_index = surface_indices
                    .get(&preset.surface_tile_id)
                    .copied()
                    .unwrap_or(preset.index);
                let (noise_tile_id, noise_tile_index) = preset
                    .noise_modulation
                    .as_ref()
                    .and_then(|noise| {
                        noise_indices
                            .get(&noise.tile_id)
                            .copied()
                            .map(|index| (Some(noise.tile_id.clone()), Some(index)))
                    })
                    .unwrap_or((None, None));
                let normal_enabled = preset.normal.as_ref().is_some_and(|normal| normal.enabled);
                let normal_strength = preset.normal.as_ref().map_or(0.0, |normal| normal.strength);
                let flags = FangyuanSurfacePresetFlags {
                    normal_enabled,
                    noise_available: noise_tile_index.is_some(),
                    noise_enabled_by_default: preset
                        .noise_modulation
                        .as_ref()
                        .is_some_and(|noise| noise.enabled_by_default),
                    emissive: matches!(
                        preset.render_model,
                        FangyuanCatalogRenderModel::StandardPbrEmissive
                    ),
                    opaque: matches!(
                        preset.render_model,
                        FangyuanCatalogRenderModel::StandardPbr
                            | FangyuanCatalogRenderModel::StandardPbrOpaqueSurface
                    ),
                    special_render_model: matches!(
                        preset.render_model,
                        FangyuanCatalogRenderModel::SpecialWater
                            | FangyuanCatalogRenderModel::SpecialUnlitSky
                    ),
                };
                let resolved = ResolvedFangyuanSurfacePreset {
                    preset_id: preset.preset_id.clone(),
                    preset_index: preset.index,
                    surface_tile_id: preset.surface_tile_id,
                    surface_tile_index,
                    noise_tile_id,
                    noise_tile_index,
                    roughness: preset.pbr_constants.perceptual_roughness,
                    metallic: preset.pbr_constants.metallic,
                    ambient_occlusion: preset.pbr_constants.ambient_occlusion,
                    reflectance: preset.pbr_constants.reflectance,
                    normal_strength,
                    repeat_size_m: preset.uv.reference_repeat_size_m,
                    projection: preset.uv.projection,
                    projection_hint: preset.uv.projection,
                    render_model: preset.render_model,
                    flags,
                };
                (resolved.preset_id.clone(), resolved)
            })
            .collect();
        Self {
            presets: entries,
            ordered_preset_ids,
            identity,
            default_surface: ResolvedFangyuanSurfacePreset::default_surface(),
        }
    }

    pub fn len(&self) -> usize {
        self.presets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.presets.is_empty()
    }

    pub fn default_surface(&self) -> &ResolvedFangyuanSurfacePreset {
        &self.default_surface
    }

    pub fn get(&self, preset_id: &str) -> Option<&ResolvedFangyuanSurfacePreset> {
        self.presets.get(preset_id)
    }

    pub fn preset_ids(&self) -> &[String] {
        &self.ordered_preset_ids
    }

    pub fn cache_identity(&self) -> &FangyuanSurfacePresetCacheIdentity {
        &self.identity
    }

    pub fn needs_reload(&self, identity: &FangyuanSurfacePresetCacheIdentity) -> bool {
        !self.identity.matches(identity)
    }

    pub fn resolve(&self, preset_id: Option<&str>) -> FangyuanSurfacePresetResolution {
        self.resolve_with_source_location(preset_id, None)
    }

    pub fn resolve_with_source_location(
        &self,
        preset_id: Option<&str>,
        source_location: Option<String>,
    ) -> FangyuanSurfacePresetResolution {
        let Some(preset_id) = preset_id else {
            return FangyuanSurfacePresetResolution {
                preset: self.default_surface.clone(),
                requested_id: None,
                requested_preset_id: None,
                resolved_id: self.default_surface.preset_id.clone(),
                resolved_preset_id: self.default_surface.preset_id.clone(),
                fallback: None,
                fallback_reason: None,
                source_location,
            };
        };
        match self.presets.get(preset_id) {
            Some(preset) => FangyuanSurfacePresetResolution {
                preset: preset.clone(),
                requested_id: Some(preset_id.to_string()),
                requested_preset_id: Some(preset_id.to_string()),
                resolved_id: preset.preset_id.clone(),
                resolved_preset_id: preset.preset_id.clone(),
                fallback: None,
                fallback_reason: None,
                source_location,
            },
            None => {
                let reason = preset_id_fallback_reason(preset_id);
                FangyuanSurfacePresetResolution {
                    preset: self.default_surface.clone(),
                    requested_id: Some(preset_id.to_string()),
                    requested_preset_id: Some(preset_id.to_string()),
                    resolved_id: self.default_surface.preset_id.clone(),
                    resolved_preset_id: self.default_surface.preset_id.clone(),
                    fallback: Some(FangyuanSurfacePresetFallback {
                        requested_id: preset_id.to_string(),
                        resolved_id: self.default_surface.preset_id.clone(),
                        reason: reason.clone(),
                        source_location: source_location.clone(),
                    }),
                    fallback_reason: Some(reason),
                    source_location,
                }
            }
        }
    }

    pub fn resolve_with_noise(
        &self,
        preset_id: Option<&str>,
        noise_enabled: bool,
        source_location: Option<String>,
    ) -> FangyuanSurfacePresetResolution {
        let mut resolution = self.resolve_with_source_location(preset_id, source_location);
        if noise_enabled
            && resolution.fallback.is_none()
            && !resolution.preset.flags.noise_available
        {
            let requested_id = resolution.requested_id.clone().unwrap_or_default();
            resolution.fallback = Some(FangyuanSurfacePresetFallback {
                requested_id,
                resolved_id: resolution.resolved_id.clone(),
                reason: FangyuanSurfacePresetFallbackReason::NoiseSuggestionUnavailable,
                source_location: resolution.source_location.clone(),
            });
            resolution.fallback_reason =
                Some(FangyuanSurfacePresetFallbackReason::NoiseSuggestionUnavailable);
        }
        resolution
    }
}

fn preset_id_fallback_reason(preset_id: &str) -> FangyuanSurfacePresetFallbackReason {
    if validate_surface_material_preset_id(
        super::FANGYUAN_SURFACE_PRESET_CATALOG_ID,
        "preset_id",
        preset_id,
    )
    .is_err()
    {
        FangyuanSurfacePresetFallbackReason::InvalidPresetId
    } else {
        FangyuanSurfacePresetFallbackReason::UnknownPresetId
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum FangyuanSurfacePresetRegistryError {
    Io { path: PathBuf, message: String },
    Parse { path: PathBuf, message: String },
    AssetPath { path: String, reason: String },
    ValidationFailed(super::FangyuanCatalogValidationReport),
}

impl std::fmt::Display for FangyuanSurfacePresetRegistryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, message } => {
                write!(formatter, "failed to read {}: {message}", path.display())
            }
            Self::Parse { path, message } => {
                write!(formatter, "failed to parse {}: {message}", path.display())
            }
            Self::AssetPath { path, reason } => {
                write!(formatter, "invalid catalog asset path {path}: {reason}")
            }
            Self::ValidationFailed(report) => report.fmt(formatter),
        }
    }
}

impl std::error::Error for FangyuanSurfacePresetRegistryError {}

fn read_bytes(path: &std::path::Path) -> Result<Vec<u8>, FangyuanSurfacePresetRegistryError> {
    fs::read(path).map_err(|error| FangyuanSurfacePresetRegistryError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}

fn parse_json<T: for<'de> Deserialize<'de>>(
    path: &std::path::Path,
) -> Result<T, FangyuanSurfacePresetRegistryError> {
    let bytes = read_bytes(path)?;
    serde_json::from_slice(&bytes).map_err(|error| FangyuanSurfacePresetRegistryError::Parse {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::fangyuan::{
        FANGYUAN_HOME_PREVIEW_BLUEPRINT_PATH, FANGYUAN_HOME_SCENE_LAYOUT_PATH, FangyuanBlueprint,
        FangyuanSceneLayout, load_fangyuan_home_prefab_palette,
        load_fangyuan_minimal_player_blueprint,
    };
    use std::path::Path;

    fn asset(path: &str) -> Vec<u8> {
        fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap()
    }

    fn paths() -> FangyuanCatalogAssetPaths {
        FangyuanCatalogAssetPaths::from_fangyuan_asset_root(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fangyuan"),
        )
    }

    #[test]
    fn current_catalog_bundle_builds_separate_registry_with_canonical_indices() {
        let registry = FangyuanSurfacePresetRegistry::load_from_paths(&paths()).unwrap();
        assert_eq!(registry.len(), 256);
        let noise: FangyuanNoiseAtlasCatalog =
            serde_json::from_slice(&asset("assets/fangyuan/atlases/noise/noise_atlas.json"))
                .unwrap();
        assert_eq!(noise.tiles.len(), 122);
        assert_eq!(
            registry.preset_ids().first().unwrap(),
            "material:surface/x00/y00"
        );
        assert_eq!(
            registry
                .get("material:surface/x00/y00")
                .unwrap()
                .preset_index,
            0
        );
        assert_eq!(
            registry
                .get("material:surface/x15/y15")
                .unwrap()
                .preset_index,
            255
        );
        assert_ne!(registry.cache_identity().content_hash, 0);
        assert_ne!(registry.cache_identity().dependency_hash, 0);
        assert_ne!(registry.cache_identity().cache_hash, 0);
        assert_eq!(
            registry.default_surface().preset_id,
            FANGYUAN_SURFACE_PRESET_DEFAULT_ID
        );
        assert_eq!(registry.default_surface().preset_index, 0);
    }

    #[test]
    fn fangyuan_atlas_integration_loads_legacy_assets_and_fixture_fallback() {
        let player = load_fangyuan_minimal_player_blueprint().unwrap();
        let home = FangyuanBlueprint::load_first_package_ron(FANGYUAN_HOME_PREVIEW_BLUEPRINT_PATH)
            .unwrap();
        let palette = load_fangyuan_home_prefab_palette().unwrap();
        let layout =
            FangyuanSceneLayout::load_first_package_ron(FANGYUAN_HOME_SCENE_LAYOUT_PATH).unwrap();
        assert_eq!(player.name, "minimal_player");
        assert_eq!(home.name, "home_preview");
        assert_eq!(palette.name, "home_prefabs");
        layout.validate_against_palette(&palette).unwrap();

        let fixture = FangyuanBlueprint::load_first_package_ron(
            "fangyuan/avatars/surface_contract_fixture.ron",
        )
        .unwrap();
        let primitives = fixture.compile().unwrap();
        assert_eq!(primitives.len(), 4);
        assert_eq!(
            primitives.primitives()[0].kind(),
            super::super::FangyuanPrimitiveKind::Cube
        );
        assert_eq!(
            primitives.primitives()[1].kind(),
            super::super::FangyuanPrimitiveKind::Sphere
        );

        let registry = FangyuanSurfacePresetRegistry::load_from_paths(&paths()).unwrap();
        assert_eq!(
            registry
                .resolve(Some("material:surface/x00/y00"))
                .preset
                .preset_index,
            0
        );
        assert_eq!(
            registry
                .resolve(Some("material:surface/x01/y00"))
                .preset
                .preset_index,
            1
        );
        assert_eq!(
            registry
                .resolve(Some("material:surface/x02/y00"))
                .preset
                .preset_index,
            2
        );
        let noise = registry.resolve_with_noise(
            Some("material:surface/x00/y00"),
            true,
            Some("surface_contract_fixture.ron:primitives[0]".to_string()),
        );
        assert!(noise.preset.flags.noise_available);
        let fallback = registry.resolve(Some("material:surface/x99/y99"));
        assert_eq!(
            fallback.resolved_preset_id,
            FANGYUAN_SURFACE_PRESET_DEFAULT_ID
        );
        assert_eq!(
            fallback.fallback_reason,
            Some(FangyuanSurfacePresetFallbackReason::UnknownPresetId)
        );
    }

    #[test]
    fn invalid_catalog_fails_before_registry_construction() {
        let (mut surface, presets, noise): (
            FangyuanSurfaceAtlasCatalog,
            FangyuanSurfaceMaterialPresetCatalog,
            FangyuanNoiseAtlasCatalog,
        ) = (
            serde_json::from_str(
                &std::fs::read_to_string(
                    Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("assets/fangyuan/atlases/base/surface_atlas.json"),
                )
                .unwrap(),
            )
            .unwrap(),
            serde_json::from_str(
                &std::fs::read_to_string(
                    Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("assets/fangyuan/atlases/base/surface_material_presets.json"),
                )
                .unwrap(),
            )
            .unwrap(),
            serde_json::from_str(
                &std::fs::read_to_string(
                    Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("assets/fangyuan/atlases/noise/noise_atlas.json"),
                )
                .unwrap(),
            )
            .unwrap(),
        );
        surface.tiles[0].tile_id = "surface_x99_y99".to_string();
        let result =
            FangyuanSurfacePresetRegistry::from_catalog_bundle(surface, presets, noise, &paths());
        assert!(matches!(
            result,
            Err(FangyuanSurfacePresetRegistryError::ValidationFailed(_))
        ));
    }

    #[test]
    fn unknown_preset_keeps_requested_id_default_and_source_location() {
        let registry = FangyuanSurfacePresetRegistry::load_from_paths(&paths()).unwrap();
        let resolution = registry.resolve_with_source_location(
            Some("material:surface/x99/y99"),
            Some("blueprints/home.ron:primitives[2]".to_string()),
        );
        assert_eq!(
            resolution.requested_id.as_deref(),
            Some("material:surface/x99/y99")
        );
        assert_eq!(resolution.resolved_id, FANGYUAN_SURFACE_PRESET_DEFAULT_ID);
        assert_eq!(
            resolution.fallback.as_ref().unwrap().reason,
            FangyuanSurfacePresetFallbackReason::UnknownPresetId
        );
        assert_eq!(
            resolution.fallback_reason,
            Some(FangyuanSurfacePresetFallbackReason::UnknownPresetId)
        );
        assert_eq!(
            resolution.source_location.as_deref(),
            Some("blueprints/home.ron:primitives[2]")
        );
    }

    #[test]
    fn default_surface_is_safe_and_noise_is_opt_in() {
        let default_surface = ResolvedFangyuanSurfacePreset::default_surface();
        assert_eq!(default_surface.roughness, 0.6);
        assert_eq!(default_surface.metallic, 0.0);
        assert_eq!(default_surface.ambient_occlusion, 1.0);
        assert_eq!(default_surface.normal_strength, 0.0);
        assert_eq!(default_surface.noise_tile_index, None);

        let registry = FangyuanSurfacePresetRegistry::load_from_paths(&paths()).unwrap();
        let without_noise = registry.resolve(Some("material:surface/x00/y00"));
        assert!(!without_noise.preset.flags.noise_enabled_by_default);

        let no_suggestion_id = registry
            .preset_ids()
            .iter()
            .find(|id| !registry.get(id).unwrap().flags.noise_available)
            .map(String::as_str)
            .expect("fixture contains presets without noise suggestions");
        let noise_fallback = registry.resolve_with_noise(
            Some(no_suggestion_id),
            true,
            Some("fixture.ron:primitive[0]".to_string()),
        );
        assert_eq!(
            noise_fallback.fallback_reason,
            Some(FangyuanSurfacePresetFallbackReason::NoiseSuggestionUnavailable)
        );
        assert_eq!(
            noise_fallback
                .fallback
                .as_ref()
                .map(|fallback| &fallback.reason),
            noise_fallback.fallback_reason.as_ref()
        );
    }

    #[test]
    fn cache_identity_is_stable_and_sensitive_to_json_and_images() {
        let parts = [
            asset("assets/fangyuan/atlases/base/surface_atlas.json"),
            asset("assets/fangyuan/atlases/base/surface_material_presets.json"),
            asset("assets/fangyuan/atlases/noise/noise_atlas.json"),
            asset("assets/fangyuan/atlases/base/surface_color.png"),
            asset("assets/fangyuan/atlases/base/surface_normal.png"),
            asset("assets/fangyuan/atlases/noise/procedural_noise.png"),
        ];
        let first = fangyuan_surface_preset_cache_identity_from_bytes(
            &parts[0], &parts[1], &parts[2], &parts[3], &parts[4], &parts[5],
        );
        let second = fangyuan_surface_preset_cache_identity_from_bytes(
            &parts[0], &parts[1], &parts[2], &parts[3], &parts[4], &parts[5],
        );
        assert_eq!(first, second);
        let mut changed = parts[1].clone();
        changed.push(b' ');
        let changed_identity = fangyuan_surface_preset_cache_identity_from_bytes(
            &parts[0], &changed, &parts[2], &parts[3], &parts[4], &parts[5],
        );
        assert_ne!(first, changed_identity);
        let mut changed_image = parts[4].clone();
        changed_image.push(0);
        let changed_image_identity = fangyuan_surface_preset_cache_identity_from_bytes(
            &parts[0],
            &parts[1],
            &parts[2],
            &parts[3],
            &changed_image,
            &parts[5],
        );
        assert_ne!(
            first.dependency_hash,
            changed_image_identity.dependency_hash
        );
    }
}
