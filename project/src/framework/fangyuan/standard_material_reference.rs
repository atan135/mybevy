//! Bounded `StandardMaterial` reference path for atlas diagnostics.
//!
//! This adapter deliberately does not serve normal runtime primitives, CPU
//! merge, or static instancing. It exists to compare a small number of
//! resolved surface presets against the later custom atlas shader.

use bevy::{asset::AssetId, mesh::VertexAttributeValues, prelude::*};
use std::collections::HashMap;

use super::{
    FANGYUAN_NORMAL_SAMPLING_POLICY, FANGYUAN_STANDARD_MATERIAL_REFERENCE_CAPABILITIES,
    FangyuanAtlasAssetCache, FangyuanAtlasFallbackFlags, FangyuanAtlasImageState,
    FangyuanCatalogAssetPaths, FangyuanCatalogRenderModel, FangyuanMaterialInstanceParams,
    FangyuanMaterialProfileRegistry, FangyuanPrimitive, FangyuanPrimitiveKind,
    FangyuanSurfacePresetFallbackReason, FangyuanSurfacePresetRegistry,
    fangyuan_atlas_tile_content_uv_rect, fangyuan_standard_material_from_params,
    fangyuan_unit_cube_mesh,
};

pub const FANGYUAN_STANDARD_MATERIAL_REFERENCE_MAX_ASSETS: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanStandardMaterialReferenceUnsupportedChannel {
    NormalStrength,
    NoiseModulation,
    SpecialRenderModel,
}

impl FangyuanStandardMaterialReferenceUnsupportedChannel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NormalStrength => "normal_strength",
            Self::NoiseModulation => "noise_modulation",
            Self::SpecialRenderModel => "special_render_model",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanStandardMaterialReferenceDiagnostic {
    pub requested_preset_id: Option<String>,
    pub resolved_preset_id: String,
    pub fallback_reason: Option<FangyuanSurfacePresetFallbackReason>,
    pub surface_tile_index: u16,
    pub perceptual_roughness: f32,
    pub metallic: f32,
    pub ambient_occlusion: f32,
    pub reflectance: f32,
    pub composed_tint: Color,
    pub composed_alpha: f32,
    pub composed_emissive: f32,
    pub base_color_atlas_enabled: bool,
    pub base_color_atlas_bound: bool,
    pub base_color_atlas_ready: bool,
    pub normal_atlas_enabled: bool,
    pub normal_atlas_bound: bool,
    pub normal_atlas_ready: bool,
    pub normal_uses_base_tile_uv: bool,
    pub normal_flip_y: bool,
    pub surface_color_alpha_ignored: bool,
    pub atlas_fallback_flags: FangyuanAtlasFallbackFlags,
    pub unsupported_channels: Vec<FangyuanStandardMaterialReferenceUnsupportedChannel>,
}

impl FangyuanStandardMaterialReferenceDiagnostic {
    pub fn format_line(&self) -> String {
        let requested = self.requested_preset_id.as_deref().unwrap_or("<default>");
        let fallback = self
            .fallback_reason
            .as_ref()
            .map(format_surface_fallback_reason)
            .unwrap_or("none");
        let unsupported = if self.unsupported_channels.is_empty() {
            "none".to_string()
        } else {
            self.unsupported_channels
                .iter()
                .map(|channel| channel.as_str())
                .collect::<Vec<_>>()
                .join(",")
        };
        format!(
            "surface requested={requested} resolved={} tile={} roughness={:.3} metallic={:.3} ao={:.3} reflectance={:.3} fallback={fallback} base=bound:{}/ready:{} normal=bound:{}/ready:{} flip_y={} unsupported={unsupported}",
            self.resolved_preset_id,
            self.surface_tile_index,
            self.perceptual_roughness,
            self.metallic,
            self.ambient_occlusion,
            self.reflectance,
            self.base_color_atlas_bound,
            self.base_color_atlas_ready,
            self.normal_atlas_bound,
            self.normal_atlas_ready,
            self.normal_flip_y,
        )
    }
}

fn format_surface_fallback_reason(reason: &FangyuanSurfacePresetFallbackReason) -> &'static str {
    match reason {
        FangyuanSurfacePresetFallbackReason::UnknownPresetId => "unknown_preset_id",
        FangyuanSurfacePresetFallbackReason::InvalidPresetId => "invalid_preset_id",
        FangyuanSurfacePresetFallbackReason::NoiseSuggestionUnavailable => {
            "noise_suggestion_unavailable"
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FangyuanStandardMaterialReferenceMeshKey {
    pub kind: FangyuanPrimitiveKind,
    pub surface_tile_index: u16,
}

#[derive(Clone, Debug)]
pub struct FangyuanStandardMaterialReference {
    pub mesh_key: FangyuanStandardMaterialReferenceMeshKey,
    pub material: StandardMaterial,
    pub composed_params: FangyuanMaterialInstanceParams,
    /// The canonical resolved surface snapshot shared with the atlas path.
    pub surface: super::ResolvedFangyuanSurfacePreset,
    pub diagnostic: FangyuanStandardMaterialReferenceDiagnostic,
}

/// Resolves one diagnostic primitive through the canonical surface and
/// material-profile registries, then maps those semantics to StandardMaterial.
pub fn fangyuan_standard_material_reference(
    primitive: &FangyuanPrimitive,
    surface_registry: &FangyuanSurfacePresetRegistry,
    material_registry: &FangyuanMaterialProfileRegistry,
    atlas: &FangyuanAtlasAssetCache,
) -> FangyuanStandardMaterialReference {
    let surface = surface_registry.resolve_with_noise(
        primitive.surface_material_preset_id(),
        primitive.surface_noise_enabled(),
        None,
    );
    let composed_params = material_registry.compose_primitive(primitive);
    let base_color_atlas_bound = atlas.base_color().is_some();
    let base_color_atlas_ready = matches!(
        atlas.status().state(super::FangyuanAtlasChannel::BaseColor),
        FangyuanAtlasImageState::Loaded
    );
    let normal_flip_y =
        FANGYUAN_NORMAL_SAMPLING_POLICY.standard_material_flip_normal_map_y(None) == Some(true);
    let normal_atlas_bound = surface.preset.flags.normal_enabled
        && normal_flip_y
        && atlas.normal().is_some()
        && FANGYUAN_STANDARD_MATERIAL_REFERENCE_CAPABILITIES.tangent_space_normal;
    let normal_atlas_ready = normal_atlas_bound
        && matches!(
            atlas.status().state(super::FangyuanAtlasChannel::Normal),
            FangyuanAtlasImageState::Loaded
        );

    let mut unsupported_channels = Vec::new();
    if surface.preset.flags.normal_enabled
        && surface.preset.normal_strength > 0.0
        && (surface.preset.normal_strength - 1.0).abs() > f32::EPSILON
    {
        unsupported_channels
            .push(FangyuanStandardMaterialReferenceUnsupportedChannel::NormalStrength);
    }
    if primitive.surface_noise_enabled() {
        unsupported_channels
            .push(FangyuanStandardMaterialReferenceUnsupportedChannel::NoiseModulation);
    }
    if matches!(
        surface.preset.render_model,
        FangyuanCatalogRenderModel::SpecialWater | FangyuanCatalogRenderModel::SpecialUnlitSky
    ) {
        unsupported_channels
            .push(FangyuanStandardMaterialReferenceUnsupportedChannel::SpecialRenderModel);
    }

    let mut material = fangyuan_standard_material_from_params(&composed_params);
    material.base_color_texture = atlas.base_color().cloned();
    material.perceptual_roughness = surface.preset.roughness;
    material.metallic = surface.preset.metallic;
    material.reflectance = surface.preset.reflectance;
    material.normal_map_texture = normal_atlas_bound
        .then(|| atlas.normal().cloned())
        .flatten();
    material.flip_normal_map_y = normal_atlas_bound && normal_flip_y;
    material.occlusion_texture = None;

    let diagnostic = FangyuanStandardMaterialReferenceDiagnostic {
        requested_preset_id: surface.requested_preset_id,
        resolved_preset_id: surface.resolved_preset_id,
        fallback_reason: surface.fallback_reason,
        surface_tile_index: surface.preset.surface_tile_index,
        perceptual_roughness: surface.preset.roughness,
        metallic: surface.preset.metallic,
        ambient_occlusion: surface.preset.ambient_occlusion,
        reflectance: surface.preset.reflectance,
        composed_tint: composed_params.color,
        composed_alpha: composed_params.alpha,
        composed_emissive: composed_params.emissive,
        base_color_atlas_enabled: base_color_atlas_bound,
        base_color_atlas_bound,
        base_color_atlas_ready,
        normal_atlas_enabled: normal_atlas_bound,
        normal_atlas_bound,
        normal_atlas_ready,
        normal_uses_base_tile_uv: normal_atlas_bound,
        normal_flip_y: material.flip_normal_map_y,
        // The promoted v0 base atlas is RGB, so sampled alpha is implicitly 1.
        surface_color_alpha_ignored: true,
        atlas_fallback_flags: atlas.status().fallback_flags,
        unsupported_channels,
    };

    FangyuanStandardMaterialReference {
        mesh_key: FangyuanStandardMaterialReferenceMeshKey {
            kind: primitive.kind(),
            surface_tile_index: surface.preset.surface_tile_index,
        },
        material,
        composed_params,
        surface: surface.preset,
        diagnostic,
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct FangyuanStandardMaterialReferenceMaterialKey {
    surface_tile_index: u16,
    color: [u32; 4],
    emissive: u32,
    perceptual_roughness: u32,
    metallic: u32,
    reflectance: u32,
    base_color_texture: Option<AssetId<Image>>,
    normal_map_texture: Option<AssetId<Image>>,
    flip_normal_map_y: bool,
}

impl FangyuanStandardMaterialReferenceMaterialKey {
    fn from_reference(reference: &FangyuanStandardMaterialReference) -> Self {
        let color = reference.material.base_color.to_srgba();
        Self {
            surface_tile_index: reference.mesh_key.surface_tile_index,
            color: [
                color.red.to_bits(),
                color.green.to_bits(),
                color.blue.to_bits(),
                color.alpha.to_bits(),
            ],
            emissive: reference.composed_params.emissive.to_bits(),
            perceptual_roughness: reference.material.perceptual_roughness.to_bits(),
            metallic: reference.material.metallic.to_bits(),
            reflectance: reference.material.reflectance.to_bits(),
            base_color_texture: reference
                .material
                .base_color_texture
                .as_ref()
                .map(Handle::id),
            normal_map_texture: reference
                .material
                .normal_map_texture
                .as_ref()
                .map(Handle::id),
            flip_normal_map_y: reference.material.flip_normal_map_y,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanStandardMaterialReferenceAssets {
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
}

#[derive(Clone, Debug)]
pub struct FangyuanStandardMaterialReferenceCache {
    max_assets: usize,
    meshes: HashMap<FangyuanStandardMaterialReferenceMeshKey, Handle<Mesh>>,
    materials: HashMap<FangyuanStandardMaterialReferenceMaterialKey, Handle<StandardMaterial>>,
}

impl FangyuanStandardMaterialReferenceCache {
    pub fn with_max_assets(max_assets: usize) -> Self {
        Self {
            max_assets: max_assets.max(1),
            meshes: HashMap::new(),
            materials: HashMap::new(),
        }
    }

    pub fn get_or_insert(
        &mut self,
        reference: &FangyuanStandardMaterialReference,
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<StandardMaterial>,
    ) -> Result<FangyuanStandardMaterialReferenceAssets, FangyuanStandardMaterialReferenceCacheError>
    {
        let material_key = FangyuanStandardMaterialReferenceMaterialKey::from_reference(reference);
        if !self.meshes.contains_key(&reference.mesh_key) && self.meshes.len() >= self.max_assets {
            return Err(
                FangyuanStandardMaterialReferenceCacheError::CapacityExceeded {
                    asset_kind: "mesh",
                    limit: self.max_assets,
                },
            );
        }
        if !self.materials.contains_key(&material_key) && self.materials.len() >= self.max_assets {
            return Err(
                FangyuanStandardMaterialReferenceCacheError::CapacityExceeded {
                    asset_kind: "material",
                    limit: self.max_assets,
                },
            );
        }

        let mesh = if let Some(handle) = self.meshes.get(&reference.mesh_key) {
            handle.clone()
        } else {
            let mesh = fangyuan_standard_material_reference_mesh(reference.mesh_key)?;
            let handle = meshes.add(mesh);
            self.meshes.insert(reference.mesh_key, handle.clone());
            handle
        };
        let material = if let Some(handle) = self.materials.get(&material_key) {
            handle.clone()
        } else {
            let handle = materials.add(reference.material.clone());
            self.materials.insert(material_key, handle.clone());
            handle
        };
        Ok(FangyuanStandardMaterialReferenceAssets { mesh, material })
    }

    pub fn clear(&mut self, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) {
        for handle in self.meshes.drain().map(|(_, handle)| handle) {
            meshes.remove(&handle);
        }
        for handle in self.materials.drain().map(|(_, handle)| handle) {
            materials.remove(&handle);
        }
    }

    pub fn mesh_count(&self) -> usize {
        self.meshes.len()
    }

    pub fn material_count(&self) -> usize {
        self.materials.len()
    }
}

impl Default for FangyuanStandardMaterialReferenceCache {
    fn default() -> Self {
        Self::with_max_assets(FANGYUAN_STANDARD_MATERIAL_REFERENCE_MAX_ASSETS)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FangyuanStandardMaterialReferenceCacheError {
    CapacityExceeded {
        asset_kind: &'static str,
        limit: usize,
    },
    InvalidSurfaceTileIndex(u16),
}

impl std::fmt::Display for FangyuanStandardMaterialReferenceCacheError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CapacityExceeded { asset_kind, limit } => write!(
                formatter,
                "Fangyuan StandardMaterial reference {asset_kind} cache exceeds diagnostic limit {limit}"
            ),
            Self::InvalidSurfaceTileIndex(index) => write!(
                formatter,
                "Fangyuan StandardMaterial reference tile index {index} is outside the atlas"
            ),
        }
    }
}

impl std::error::Error for FangyuanStandardMaterialReferenceCacheError {}

fn fangyuan_standard_material_reference_mesh(
    key: FangyuanStandardMaterialReferenceMeshKey,
) -> Result<Mesh, FangyuanStandardMaterialReferenceCacheError> {
    let Some(rect) = fangyuan_atlas_tile_content_uv_rect(key.surface_tile_index) else {
        return Err(
            FangyuanStandardMaterialReferenceCacheError::InvalidSurfaceTileIndex(
                key.surface_tile_index,
            ),
        );
    };
    let mut mesh = match key.kind {
        FangyuanPrimitiveKind::Cube => fangyuan_unit_cube_mesh(),
        FangyuanPrimitiveKind::Sphere => super::fangyuan_unit_sphere_mesh(
            0.5,
            super::FANGYUAN_RENDER_UNIT_SPHERE_SECTORS,
            super::FANGYUAN_RENDER_UNIT_SPHERE_STACKS,
        ),
    };
    let Some(VertexAttributeValues::Float32x2(uvs)) = mesh.attribute_mut(Mesh::ATTRIBUTE_UV_0)
    else {
        unreachable!("Fangyuan reference primitives always contain Float32x2 UV0");
    };
    for uv in uvs {
        let local = Vec2::from_array(*uv);
        *uv = (rect.min + local * (rect.max - rect.min)).to_array();
    }
    Ok(mesh)
}

/// Loads the same validated first-package surface registry used by runtime
/// diagnostics. This function never reads authoring tasks or run bundles.
pub fn load_first_package_fangyuan_surface_preset_registry()
-> Result<FangyuanSurfacePresetRegistry, String> {
    let surface_path =
        super::first_package_fangyuan_asset_fs_path("fangyuan/atlases/base/surface_atlas.json")
            .ok_or_else(|| "first-package Fangyuan surface catalog was not found".to_string())?;
    let fangyuan_root = surface_path
        .parent()
        .and_then(std::path::Path::parent)
        .and_then(std::path::Path::parent)
        .ok_or_else(|| "first-package Fangyuan atlas path has no asset root".to_string())?;
    let paths = FangyuanCatalogAssetPaths::from_fangyuan_asset_root(fangyuan_root);
    FangyuanSurfacePresetRegistry::load_from_paths(&paths).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::fangyuan::{
        FANGYUAN_MATERIAL_PROFILE_VERSION, FangyuanMaterialAlphaPolicy, FangyuanMaterialBaseParams,
        FangyuanMaterialEmissivePolicy, FangyuanMaterialProfile, FangyuanNormalGreenDirection,
        FangyuanPrimitiveLifecycle, FangyuanPrimitiveRole,
    };
    use bevy::asset::AssetPlugin;
    use std::{fs::File, io::BufReader, path::Path};

    fn loaded_atlas_cache() -> (App, FangyuanAtlasAssetCache) {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>();
        let asset_server = app.world().resource::<AssetServer>().clone();
        let mut atlas = FangyuanAtlasAssetCache::default();
        atlas.ensure_loaded(&asset_server, true);
        (app, atlas)
    }

    fn primitive(
        kind: FangyuanPrimitiveKind,
        color: Color,
        alpha: f32,
        emissive: f32,
        material_profile_id: Option<&str>,
        surface_preset_id: Option<&str>,
        noise: bool,
    ) -> FangyuanPrimitive {
        FangyuanPrimitive::with_surface_metadata(
            kind,
            Vec3::ZERO,
            Vec3::ONE,
            color,
            FangyuanPrimitiveRole::Decoration,
            alpha,
            emissive,
            material_profile_id.map(str::to_string),
            surface_preset_id.map(str::to_string),
            noise,
            FangyuanPrimitiveLifecycle::empty(),
        )
    }

    #[test]
    fn default_and_unknown_presets_preserve_resolution_diagnostics() {
        let registry = load_first_package_fangyuan_surface_preset_registry().unwrap();
        let material_registry = FangyuanMaterialProfileRegistry::default();
        let (_app, atlas) = loaded_atlas_cache();
        let default = fangyuan_standard_material_reference(
            &primitive(
                FangyuanPrimitiveKind::Cube,
                Color::WHITE,
                1.0,
                0.0,
                None,
                None,
                false,
            ),
            &registry,
            &material_registry,
            &atlas,
        );
        assert_eq!(default.diagnostic.requested_preset_id, None);
        assert_eq!(
            default.diagnostic.resolved_preset_id,
            registry.default_surface().preset_id
        );
        assert_eq!(default.diagnostic.surface_tile_index, 0);
        assert_eq!(default.diagnostic.fallback_reason, None);

        let unknown = fangyuan_standard_material_reference(
            &primitive(
                FangyuanPrimitiveKind::Sphere,
                Color::WHITE,
                1.0,
                0.0,
                None,
                Some("material:surface/x99/y99"),
                false,
            ),
            &registry,
            &material_registry,
            &atlas,
        );
        assert_eq!(
            unknown.diagnostic.requested_preset_id.as_deref(),
            Some("material:surface/x99/y99")
        );
        assert_eq!(
            unknown.diagnostic.resolved_preset_id,
            registry.default_surface().preset_id
        );
        assert_eq!(
            unknown.diagnostic.fallback_reason,
            Some(FangyuanSurfacePresetFallbackReason::UnknownPresetId)
        );
        let line = unknown.diagnostic.format_line();
        assert!(line.contains("requested=material:surface/x99/y99"));
        assert!(line.contains("fallback=unknown_preset_id"));
    }

    #[test]
    fn known_preset_maps_composed_tint_alpha_emissive_and_pbr_once() {
        let registry = load_first_package_fangyuan_surface_preset_registry().unwrap();
        let mut material_registry = FangyuanMaterialProfileRegistry::default();
        material_registry
            .insert_profile(FangyuanMaterialProfile {
                stable_id: "diagnostic/tint".to_string(),
                version: FANGYUAN_MATERIAL_PROFILE_VERSION.to_string(),
                base: FangyuanMaterialBaseParams {
                    color: Color::srgb(0.5, 1.0, 0.25),
                    alpha: 0.5,
                    emissive: 1.0,
                },
                emissive_policy: FangyuanMaterialEmissivePolicy::AdditiveClamp { max: 4.0 },
                alpha_policy: FangyuanMaterialAlphaPolicy::MultiplyClamp { min: 0.0, max: 1.0 },
                debug_label: "diagnostic tint".to_string(),
            })
            .unwrap();
        let (_app, atlas) = loaded_atlas_cache();
        let reference = fangyuan_standard_material_reference(
            &primitive(
                FangyuanPrimitiveKind::Cube,
                Color::srgb(0.4, 0.2, 0.8),
                0.6,
                2.0,
                Some("diagnostic/tint"),
                Some("material:surface/x00/y01"),
                true,
            ),
            &registry,
            &material_registry,
            &atlas,
        );

        assert_eq!(
            reference.material.base_color,
            Color::srgba(0.2, 0.2, 0.2, 0.3)
        );
        assert_eq!(reference.composed_params.alpha, 0.3);
        assert_eq!(reference.composed_params.emissive, 3.0);
        assert!(matches!(reference.material.alpha_mode, AlphaMode::Blend));
        assert_eq!(reference.material.perceptual_roughness, 0.84);
        assert_eq!(reference.material.metallic, 0.9);
        assert_eq!(reference.material.reflectance, 0.5);
        assert_eq!(reference.diagnostic.ambient_occlusion, 1.0);
        assert!(reference.material.base_color_texture.is_some());
        assert!(reference.material.normal_map_texture.is_some());
        assert!(reference.material.flip_normal_map_y);
        assert!(reference.diagnostic.base_color_atlas_bound);
        assert!(!reference.diagnostic.base_color_atlas_ready);
        assert!(reference.diagnostic.normal_atlas_bound);
        assert!(!reference.diagnostic.normal_atlas_ready);
        assert!(reference.diagnostic.normal_uses_base_tile_uv);
        assert!(
            reference
                .diagnostic
                .unsupported_channels
                .contains(&FangyuanStandardMaterialReferenceUnsupportedChannel::NormalStrength)
        );
        assert!(
            reference
                .diagnostic
                .unsupported_channels
                .contains(&FangyuanStandardMaterialReferenceUnsupportedChannel::NoiseModulation)
        );
        let linear = reference.material.base_color.to_linear();
        assert_eq!(
            reference.material.emissive,
            LinearRgba::new(linear.red * 3.0, linear.green * 3.0, linear.blue * 3.0, 1.0)
        );
    }

    #[test]
    fn reference_mesh_and_material_cache_reuses_keys_and_is_bounded() {
        let registry = load_first_package_fangyuan_surface_preset_registry().unwrap();
        let material_registry = FangyuanMaterialProfileRegistry::default();
        let (_app, atlas) = loaded_atlas_cache();
        let cube = fangyuan_standard_material_reference(
            &primitive(
                FangyuanPrimitiveKind::Cube,
                Color::WHITE,
                1.0,
                0.0,
                None,
                Some("material:surface/x00/y01"),
                false,
            ),
            &registry,
            &material_registry,
            &atlas,
        );
        let sphere = fangyuan_standard_material_reference(
            &primitive(
                FangyuanPrimitiveKind::Sphere,
                Color::WHITE,
                1.0,
                0.0,
                None,
                Some("material:surface/x00/y01"),
                false,
            ),
            &registry,
            &material_registry,
            &atlas,
        );
        let mut meshes = Assets::<Mesh>::default();
        let mut materials = Assets::<StandardMaterial>::default();
        let mut cache = FangyuanStandardMaterialReferenceCache::with_max_assets(1);
        let first = cache
            .get_or_insert(&cube, &mut meshes, &mut materials)
            .unwrap();
        let repeated = cache
            .get_or_insert(&cube, &mut meshes, &mut materials)
            .unwrap();
        assert_eq!(first, repeated);
        assert_eq!(cache.mesh_count(), 1);
        assert_eq!(cache.material_count(), 1);

        let mesh = meshes.get(&first.mesh).unwrap();
        let Some(VertexAttributeValues::Float32x2(uvs)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0)
        else {
            panic!("reference mesh must contain UV0");
        };
        let rect = fangyuan_atlas_tile_content_uv_rect(16).unwrap();
        assert!(Vec2::from_array(uvs[0]).abs_diff_eq(rect.min, 1e-7));
        assert!(Vec2::from_array(uvs[2]).abs_diff_eq(rect.max, 1e-7));

        assert!(matches!(
            cache.get_or_insert(&sphere, &mut meshes, &mut materials),
            Err(
                FangyuanStandardMaterialReferenceCacheError::CapacityExceeded {
                    asset_kind: "mesh",
                    limit: 1,
                }
            )
        ));
        cache.clear(&mut meshes, &mut materials);
        assert_eq!(cache.mesh_count(), 0);
        assert_eq!(cache.material_count(), 0);
        assert!(meshes.get(&first.mesh).is_none());
        assert!(materials.get(&first.material).is_none());
    }

    #[test]
    fn promoted_base_atlas_is_rgb_so_surface_alpha_cannot_change_opacity() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets/fangyuan/atlases/base/surface_color.png");
        let decoder = png::Decoder::new(BufReader::new(File::open(path).unwrap()));
        let reader = decoder.read_info().unwrap();
        assert!(matches!(
            reader.info().color_type,
            png::ColorType::Rgb | png::ColorType::Rgba
        ));

        let registry = load_first_package_fangyuan_surface_preset_registry().unwrap();
        let material_registry = FangyuanMaterialProfileRegistry::default();
        let (_app, atlas) = loaded_atlas_cache();
        let reference = fangyuan_standard_material_reference(
            &primitive(
                FangyuanPrimitiveKind::Cube,
                Color::WHITE,
                0.42,
                0.0,
                None,
                Some("material:surface/x00/y01"),
                false,
            ),
            &registry,
            &material_registry,
            &atlas,
        );
        assert_eq!(reference.material.base_color.to_srgba().alpha, 0.42);
        assert_eq!(reference.diagnostic.composed_alpha, 0.42);
        assert!(reference.diagnostic.surface_color_alpha_ignored);
        assert!(
            reference
                .diagnostic
                .format_line()
                .contains("base=bound:true/ready:false")
        );
    }

    #[test]
    fn standard_reference_capabilities_do_not_claim_shader_only_effects() {
        assert!(FANGYUAN_STANDARD_MATERIAL_REFERENCE_CAPABILITIES.base_color);
        assert!(FANGYUAN_STANDARD_MATERIAL_REFERENCE_CAPABILITIES.perceptual_roughness);
        assert!(FANGYUAN_STANDARD_MATERIAL_REFERENCE_CAPABILITIES.metallic);
        assert!(FANGYUAN_STANDARD_MATERIAL_REFERENCE_CAPABILITIES.tangent_space_normal);
        assert!(!FANGYUAN_STANDARD_MATERIAL_REFERENCE_CAPABILITIES.normal_strength);
        assert!(!FANGYUAN_STANDARD_MATERIAL_REFERENCE_CAPABILITIES.noise_modulation);
        assert_eq!(
            FANGYUAN_NORMAL_SAMPLING_POLICY.effective_direction(None),
            Some(FangyuanNormalGreenDirection::NegativeY)
        );
    }
}
