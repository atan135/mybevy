//! Custom-material boundary for Fangyuan's shared texture atlases.
//!
//! This is deliberately a small binding skeleton.  Surface and material
//! semantics come from the same resolver/reference path used by diagnostics;
//! the later instance-buffer work can move [`FangyuanAtlasMaterialUniform`]
//! without changing that CPU contract.

use bevy::{
    asset::{AssetId, Handle},
    image::Image,
    pbr::{Material, MaterialPlugin},
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderType},
    shader::ShaderRef,
};
use std::collections::HashMap;

use super::{
    FANGYUAN_ATLAS_COLUMNS, FANGYUAN_ATLAS_ROWS, FANGYUAN_NORMAL_SAMPLING_POLICY,
    FangyuanAtlasAssetCache, FangyuanAtlasChannel, FangyuanAtlasImageState, FangyuanCubeFace,
    FangyuanMaterialProfileRegistry, FangyuanNormalGreenDirection, FangyuanPrimitive,
    FangyuanStandardMaterialReference, FangyuanSurfacePresetFallbackReason,
    FangyuanSurfacePresetRegistry, fangyuan_atlas_tile_content_uv_rect,
    fangyuan_atlas_uv_from_local, fangyuan_cube_face_repeat, fangyuan_sphere_repeat,
    fangyuan_standard_material_reference,
};

/// Compute the local repeat multiplier before mapping UVs into a tile.
///
/// Cubes use the stable front-face XY contract for this material-level value;
/// the geometry contract still exposes per-face repeat for future instance or
/// face attributes. Spheres use the shared Y-up circumference approximation.
pub fn fangyuan_atlas_local_repeat(
    kind: super::FangyuanPrimitiveKind,
    primitive_world_size: Vec3,
    reference_repeat_size_m: [f32; 2],
) -> Option<Vec2> {
    let reference = Vec2::from_array(reference_repeat_size_m);
    match kind {
        super::FangyuanPrimitiveKind::Cube => {
            fangyuan_cube_face_repeat(FangyuanCubeFace::Front, primitive_world_size, reference)
        }
        super::FangyuanPrimitiveKind::Sphere => {
            fangyuan_sphere_repeat(primitive_world_size, reference)
        }
    }
}

/// CPU mirror of the WGSL tile mapping. Keeping this helper beside the
/// material makes texel-center and fract behavior testable without a GPU.
pub fn fangyuan_atlas_material_uv(local_uv: Vec2, repeat: Vec2, tile_index: u16) -> Option<Vec2> {
    fangyuan_atlas_uv_from_local(local_uv, repeat, tile_index)
}

/// Decode a linear normal-map texel and apply the catalog strength while
/// preserving a unit-length tangent-space normal.  A zero strength is the
/// flat normal, so callers can use it as the disabled-channel fallback.
pub fn fangyuan_decode_normal_sample(sample: Vec3, strength: f32, flip_y: bool) -> Vec3 {
    let mut encoded = sample * 2.0 - Vec3::ONE;
    if flip_y {
        encoded.y = -encoded.y;
    }
    let strength = strength.clamp(0.0, 1.0);
    let flat = Vec3::Z;
    if strength <= f32::EPSILON {
        return flat;
    }
    let blended = flat.lerp(encoded, strength);
    if blended.length_squared() <= f32::EPSILON {
        flat
    } else {
        blended.normalize()
    }
}

/// Noise indices are row-major 16x16 cells.  The catalog validator owns the
/// full 122-valid/134-reserved range; this local guard prevents sentinel or
/// reserved cells from becoming texture coordinates in the shader.
pub fn fangyuan_noise_tile_content_uv_rect(tile_index: u16) -> Option<(Vec2, Vec2)> {
    let x = tile_index % 16;
    let y = tile_index / 16;
    let valid = (y <= 5 && x <= 11) || (6..=10).contains(&y) && x <= 9;
    if !valid {
        return None;
    }
    let rect = fangyuan_atlas_tile_content_uv_rect(tile_index)?;
    Some((rect.min, rect.max))
}

pub const FANGYUAN_ATLAS_GALLERY_SURFACE_TILE_COUNT: usize = 256;
pub const FANGYUAN_ATLAS_GALLERY_NOISE_TILE_COUNT: usize = 122;

/// Bounded development-gallery index lists. These are index-only values and
/// do not allocate materials, entities, or image handles per tile.
pub fn fangyuan_atlas_gallery_surface_tile_indices() -> Vec<u16> {
    (0..FANGYUAN_ATLAS_GALLERY_SURFACE_TILE_COUNT as u16).collect()
}

pub fn fangyuan_atlas_gallery_noise_tile_indices() -> Vec<u16> {
    (0..(FANGYUAN_ATLAS_COLUMNS * FANGYUAN_ATLAS_ROWS))
        .filter(|index| fangyuan_noise_tile_content_uv_rect(*index).is_some())
        .collect()
}

pub fn fangyuan_atlas_gallery_reserved_noise_tile_indices() -> Vec<u16> {
    (0..(FANGYUAN_ATLAS_COLUMNS * FANGYUAN_ATLAS_ROWS))
        .filter(|index| fangyuan_noise_tile_content_uv_rect(*index).is_none())
        .collect()
}

/// Material flags are intentionally data, rather than shader variants.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FangyuanAtlasMaterialFlags(u32);

impl FangyuanAtlasMaterialFlags {
    pub const NORMAL_ENABLED: Self = Self(1 << 0);
    pub const NOISE_ENABLED: Self = Self(1 << 1);
    pub const FALLBACK: Self = Self(1 << 2);
    pub const GREEN_FLIP_Y: Self = Self(1 << 3);
    pub const DEBUG: Self = Self(1 << 4);
    pub const ATLAS_NOT_READY: Self = Self(1 << 5);
    pub const DEBUG_BASE_ONLY: Self = Self(1 << 6);
    pub const DEBUG_FINAL: Self = Self(1 << 7);
    pub const DEBUG_NORMAL_ONLY: Self = Self(1 << 8);
    pub const DEBUG_NOISE_ONLY: Self = Self(1 << 9);
    pub const SPECIAL_UNLIT: Self = Self(1 << 10);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FangyuanAtlasDebugMode {
    #[default]
    Off,
    BaseOnly,
    Final,
    NormalOnly,
    NoiseOnly,
}

impl FangyuanAtlasDebugMode {
    const fn flags(self) -> FangyuanAtlasMaterialFlags {
        match self {
            Self::Off => FangyuanAtlasMaterialFlags::empty(),
            Self::BaseOnly => {
                FangyuanAtlasMaterialFlags::DEBUG.union(FangyuanAtlasMaterialFlags::DEBUG_BASE_ONLY)
            }
            Self::Final => {
                FangyuanAtlasMaterialFlags::DEBUG.union(FangyuanAtlasMaterialFlags::DEBUG_FINAL)
            }
            Self::NormalOnly => FangyuanAtlasMaterialFlags::DEBUG
                .union(FangyuanAtlasMaterialFlags::DEBUG_NORMAL_ONLY),
            Self::NoiseOnly => FangyuanAtlasMaterialFlags::DEBUG
                .union(FangyuanAtlasMaterialFlags::DEBUG_NOISE_ONLY),
        }
    }
}

/// Packed uniform layout.  Every field is either a vec4 or a scalar tail so
/// the WGSL layout is explicit and can later be copied into an instance array.
#[derive(Clone, Copy, Debug, PartialEq, ShaderType)]
pub struct FangyuanAtlasMaterialUniform {
    /// Composed profile/primitive tint in linear space, including alpha.
    pub tint: Vec4,
    /// local repeat x/y, perceptual roughness, metallic
    pub surface: Vec4,
    /// reflectance, ambient occlusion, normal strength, noise strength
    pub pbr: Vec4,
    /// composed emissive rgb and alpha
    pub emissive_alpha: Vec4,
    pub tile_index: u32,
    pub noise_tile_index: u32,
    pub flags: u32,
    /// Catalog UV scale multiplier for the optional noise tile.
    pub noise_uv_scale: f32,
}

impl FangyuanAtlasMaterialUniform {
    pub const NOISE_TILE_SENTINEL: u32 = u32::MAX;

    pub fn roughness(self) -> f32 {
        self.surface.z
    }

    pub fn metallic(self) -> f32 {
        self.surface.w
    }

    pub fn reflectance(self) -> f32 {
        self.pbr.x
    }

    pub fn ambient_occlusion(self) -> f32 {
        self.pbr.y
    }

    pub fn normal_strength(self) -> f32 {
        self.pbr.z
    }

    pub fn noise_strength(self) -> f32 {
        self.pbr.w
    }

    pub fn noise_tile_rect(self) -> Option<(Vec2, Vec2)> {
        let index = u16::try_from(self.noise_tile_index).ok()?;
        fangyuan_noise_tile_content_uv_rect(index)
    }
}

/// Stable semantic snapshot used by diagnostics and future instance data.
#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanAtlasMaterialSemantic {
    pub requested_preset_id: Option<String>,
    pub resolved_preset_id: String,
    pub fallback_reason: Option<FangyuanSurfacePresetFallbackReason>,
    pub uniform: FangyuanAtlasMaterialUniform,
    pub atlas_fallback_flags: super::FangyuanAtlasFallbackFlags,
    pub surface_tile_id: String,
    pub projection: super::FangyuanCatalogProjection,
    pub projection_requested: super::FangyuanCatalogProjection,
    pub projection_fallback_reason: Option<String>,
    pub render_model: super::FangyuanCatalogRenderModel,
    pub render_model_fallback: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FangyuanAtlasMaterialFallbackReason {
    CatalogUnavailable,
    AtlasNotReady,
    MissingBaseColorAtlas,
    MissingNormalAtlas,
    MissingNoiseAtlas,
    UnsupportedSpecialSurface,
    ShaderUnavailable,
    UnsupportedCapability,
}

impl std::fmt::Display for FangyuanAtlasMaterialFallbackReason {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::CatalogUnavailable => "catalog_unavailable",
            Self::AtlasNotReady => "atlas_not_ready",
            Self::MissingBaseColorAtlas => "missing_base_color_atlas",
            Self::MissingNormalAtlas => "missing_normal_atlas",
            Self::MissingNoiseAtlas => "missing_noise_atlas",
            Self::UnsupportedSpecialSurface => "unsupported_special_surface",
            Self::ShaderUnavailable => "shader_unavailable",
            Self::UnsupportedCapability => "unsupported_capability",
        };
        formatter.write_str(label)
    }
}

/// Capability gate kept explicit so a renderer can select the reference path
/// before creating an asset when a backend cannot support this material.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FangyuanAtlasMaterialCapabilities {
    pub custom_material: bool,
    pub texture_bindings: bool,
    pub shader_available: bool,
}

impl Default for FangyuanAtlasMaterialCapabilities {
    fn default() -> Self {
        Self {
            custom_material: true,
            texture_bindings: true,
            shader_available: true,
        }
    }
}

#[derive(Asset, AsBindGroup, Debug, Clone, PartialEq, TypePath)]
pub struct FangyuanAtlasMaterial {
    #[uniform(0)]
    pub uniform: FangyuanAtlasMaterialUniform,
    #[texture(1)]
    #[sampler(2)]
    pub color_atlas: Handle<Image>,
    #[texture(3)]
    #[sampler(4)]
    pub normal_atlas: Handle<Image>,
    #[texture(5)]
    #[sampler(6)]
    pub noise_atlas: Handle<Image>,
    pub alpha_mode: AlphaMode,
}

impl Material for FangyuanAtlasMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/fangyuan_atlas_material.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        self.alpha_mode
    }

    // The skeleton does not provide Bevy's StandardMaterial prepass bindings.
    fn enable_prepass() -> bool {
        false
    }
}

/// Convenience constructor for the supported Bevy plugin API.
pub fn fangyuan_atlas_material_plugin() -> MaterialPlugin<FangyuanAtlasMaterial> {
    MaterialPlugin::default()
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FangyuanAtlasMaterialKey {
    pub color_atlas: AssetId<Image>,
    pub normal_atlas: AssetId<Image>,
    pub noise_atlas: AssetId<Image>,
    pub uniform_bits: [u32; 20],
}

impl FangyuanAtlasMaterialKey {
    pub fn from_material(material: &FangyuanAtlasMaterial) -> Self {
        let u = material.uniform;
        Self {
            color_atlas: material.color_atlas.id(),
            normal_atlas: material.normal_atlas.id(),
            noise_atlas: material.noise_atlas.id(),
            uniform_bits: [
                u.tint.x.to_bits(),
                u.tint.y.to_bits(),
                u.tint.z.to_bits(),
                u.tint.w.to_bits(),
                u.surface.x.to_bits(),
                u.surface.y.to_bits(),
                u.surface.z.to_bits(),
                u.surface.w.to_bits(),
                u.pbr.x.to_bits(),
                u.pbr.y.to_bits(),
                u.pbr.z.to_bits(),
                u.pbr.w.to_bits(),
                u.emissive_alpha.x.to_bits(),
                u.emissive_alpha.y.to_bits(),
                u.emissive_alpha.z.to_bits(),
                u.emissive_alpha.w.to_bits(),
                u.tile_index,
                u.noise_tile_index,
                u.flags,
                u.noise_uv_scale.to_bits(),
            ],
        }
    }
}

pub const FANGYUAN_ATLAS_MATERIAL_MAX_ASSETS: usize = 16;

#[derive(Clone, Debug)]
pub struct FangyuanAtlasMaterialCache {
    max_assets: usize,
    materials: HashMap<FangyuanAtlasMaterialKey, Handle<FangyuanAtlasMaterial>>,
}

impl FangyuanAtlasMaterialCache {
    pub fn with_max_assets(max_assets: usize) -> Self {
        Self {
            max_assets: max_assets.max(1),
            materials: HashMap::new(),
        }
    }

    pub fn get_or_insert(
        &mut self,
        material: &FangyuanAtlasMaterial,
        assets: &mut Assets<FangyuanAtlasMaterial>,
    ) -> Result<Handle<FangyuanAtlasMaterial>, FangyuanAtlasMaterialCacheError> {
        let key = FangyuanAtlasMaterialKey::from_material(material);
        if let Some(handle) = self.materials.get(&key) {
            return Ok(handle.clone());
        }
        if self.materials.len() >= self.max_assets {
            return Err(FangyuanAtlasMaterialCacheError::CapacityExceeded {
                limit: self.max_assets,
            });
        }
        let handle = assets.add(material.clone());
        self.materials.insert(key, handle.clone());
        Ok(handle)
    }

    pub fn clear(&mut self, assets: &mut Assets<FangyuanAtlasMaterial>) {
        for handle in self.materials.drain().map(|(_, handle)| handle) {
            assets.remove(&handle);
        }
    }

    pub fn len(&self) -> usize {
        self.materials.len()
    }
}

impl Default for FangyuanAtlasMaterialCache {
    fn default() -> Self {
        Self::with_max_assets(FANGYUAN_ATLAS_MATERIAL_MAX_ASSETS)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FangyuanAtlasMaterialCacheError {
    CapacityExceeded { limit: usize },
}

/// Successful material construction.  The key is intentionally derived from
/// shared image IDs and packed semantics, so preset IDs do not allocate image
/// or sampler resources of their own.
#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanAtlasMaterialBuild {
    pub material: FangyuanAtlasMaterial,
    pub semantic: FangyuanAtlasMaterialSemantic,
}

pub enum FangyuanAtlasMaterialBuildResult {
    Ready(FangyuanAtlasMaterialBuild),
    Fallback {
        reason: FangyuanAtlasMaterialFallbackReason,
        reference: FangyuanStandardMaterialReference,
    },
}

/// Resolve one primitive through the canonical reference adapter and map the
/// result to the atlas binding skeleton.
pub fn fangyuan_atlas_material(
    primitive: &FangyuanPrimitive,
    surface_registry: &FangyuanSurfacePresetRegistry,
    material_registry: &FangyuanMaterialProfileRegistry,
    atlas: &FangyuanAtlasAssetCache,
) -> FangyuanAtlasMaterialBuildResult {
    fangyuan_atlas_material_with_options(
        primitive,
        surface_registry,
        material_registry,
        atlas,
        FangyuanAtlasMaterialCapabilities::default(),
        FangyuanAtlasDebugMode::Off,
    )
}

pub fn fangyuan_atlas_material_with_options(
    primitive: &FangyuanPrimitive,
    surface_registry: &FangyuanSurfacePresetRegistry,
    material_registry: &FangyuanMaterialProfileRegistry,
    atlas: &FangyuanAtlasAssetCache,
    capabilities: FangyuanAtlasMaterialCapabilities,
    debug_mode: FangyuanAtlasDebugMode,
) -> FangyuanAtlasMaterialBuildResult {
    fangyuan_atlas_material_with_normal_override(
        primitive,
        surface_registry,
        material_registry,
        atlas,
        capabilities,
        debug_mode,
        None,
    )
}

/// Variant used by the diagnostic gallery to compare both normal green
/// conventions.  Production callers should keep the default `None`, which
/// uses the verified NegativeY catalog policy.
pub fn fangyuan_atlas_material_with_normal_override(
    primitive: &FangyuanPrimitive,
    surface_registry: &FangyuanSurfacePresetRegistry,
    material_registry: &FangyuanMaterialProfileRegistry,
    atlas: &FangyuanAtlasAssetCache,
    capabilities: FangyuanAtlasMaterialCapabilities,
    debug_mode: FangyuanAtlasDebugMode,
    normal_green_override: Option<FangyuanNormalGreenDirection>,
) -> FangyuanAtlasMaterialBuildResult {
    let reference =
        fangyuan_standard_material_reference(primitive, surface_registry, material_registry, atlas);

    let base_state = atlas.status().state(FangyuanAtlasChannel::BaseColor);
    let normal_state = atlas.status().state(FangyuanAtlasChannel::Normal);
    let noise_state = atlas.status().state(FangyuanAtlasChannel::Noise);
    let reason = if !capabilities.custom_material || !capabilities.texture_bindings {
        Some(FangyuanAtlasMaterialFallbackReason::UnsupportedCapability)
    } else if !capabilities.shader_available {
        Some(FangyuanAtlasMaterialFallbackReason::ShaderUnavailable)
    } else if !atlas.status().catalog_available {
        Some(FangyuanAtlasMaterialFallbackReason::CatalogUnavailable)
    } else if atlas.base_color().is_none()
        || matches!(
            base_state,
            FangyuanAtlasImageState::Missing | FangyuanAtlasImageState::Failed
        )
    {
        Some(FangyuanAtlasMaterialFallbackReason::MissingBaseColorAtlas)
    } else if !matches!(base_state, FangyuanAtlasImageState::Loaded)
        || matches!(
            normal_state,
            FangyuanAtlasImageState::Uninitialized | FangyuanAtlasImageState::Loading
        )
        || matches!(
            noise_state,
            FangyuanAtlasImageState::Uninitialized | FangyuanAtlasImageState::Loading
        )
    {
        Some(FangyuanAtlasMaterialFallbackReason::AtlasNotReady)
    } else {
        None
    };

    if let Some(reason) = reason {
        return FangyuanAtlasMaterialBuildResult::Fallback { reason, reference };
    }

    // Dedicated sky/background shading is intentionally deferred. Keep the
    // reference adapter's explicit unlit/default fallback instead of binding
    // a PBR atlas material that would claim unsupported semantics.
    if reference.diagnostic.render_model_fallback.as_deref() == Some("unlit_default") {
        return FangyuanAtlasMaterialBuildResult::Fallback {
            reason: FangyuanAtlasMaterialFallbackReason::UnsupportedSpecialSurface,
            reference,
        };
    }

    // The reference adapter owns this resolver result. The atlas path only
    // projects the shared snapshot into its uniform layout.
    let surface = &reference.surface;
    let repeat =
        fangyuan_atlas_local_repeat(primitive.kind(), primitive.scale(), surface.repeat_size_m)
            .unwrap_or(Vec2::ONE);
    let tint = reference.composed_params.color.to_linear();
    let emissive = reference.material.emissive;
    let normal_enabled = surface.flags.normal_enabled
        && surface.normal_strength > 0.0
        && matches!(normal_state, FangyuanAtlasImageState::Loaded)
        && atlas.normal().is_some();
    let noise_enabled = primitive.surface_noise_enabled()
        && surface.flags.noise_available
        && surface.noise_tile_index.is_some()
        && surface.noise_strength > 0.0
        && surface
            .noise_tile_index
            .and_then(fangyuan_noise_tile_content_uv_rect)
            .is_some()
        && matches!(noise_state, FangyuanAtlasImageState::Loaded)
        && atlas.noise().is_some();
    let green_flip_y = FANGYUAN_NORMAL_SAMPLING_POLICY
        .standard_material_flip_normal_map_y(normal_green_override)
        == Some(true);

    let mut flags = debug_mode.flags();
    if normal_enabled {
        flags = flags.union(FangyuanAtlasMaterialFlags::NORMAL_ENABLED);
    }
    if noise_enabled {
        flags = flags.union(FangyuanAtlasMaterialFlags::NOISE_ENABLED);
    }
    if reference.diagnostic.fallback_reason.is_some()
        || atlas.status().fallback_flags != super::FangyuanAtlasFallbackFlags::empty()
    {
        flags = flags.union(FangyuanAtlasMaterialFlags::FALLBACK);
    }
    if green_flip_y {
        flags = flags.union(FangyuanAtlasMaterialFlags::GREEN_FLIP_Y);
    }
    if !atlas.status().is_ready() {
        flags = flags.union(FangyuanAtlasMaterialFlags::ATLAS_NOT_READY);
    }

    let noise_tile_index = if noise_enabled {
        surface
            .noise_tile_index
            .map(u32::from)
            .unwrap_or(FangyuanAtlasMaterialUniform::NOISE_TILE_SENTINEL)
    } else {
        FangyuanAtlasMaterialUniform::NOISE_TILE_SENTINEL
    };
    let uniform = FangyuanAtlasMaterialUniform {
        tint: Vec4::new(tint.red, tint.green, tint.blue, tint.alpha),
        surface: Vec4::new(
            repeat.x,
            repeat.y,
            surface.roughness.clamp(0.0, 1.0),
            surface.metallic.clamp(0.0, 1.0),
        ),
        // Noise strength and UV scale are catalog-resolved visual constants;
        // they never participate in authority, collision, or replay state.
        pbr: Vec4::new(
            surface.reflectance.clamp(0.0, 1.0),
            // Catalog schema v1 requires AO=1.0. Keep the shader contract
            // explicit even if future catalogs expose a diagnostic AO value.
            1.0,
            surface.normal_strength.clamp(0.0, 1.0),
            if noise_enabled {
                surface.noise_strength.clamp(0.0, 1.0)
            } else {
                0.0
            },
        ),
        emissive_alpha: Vec4::new(emissive.red, emissive.green, emissive.blue, tint.alpha),
        tile_index: u32::from(surface.surface_tile_index),
        noise_tile_index,
        flags: flags.bits(),
        noise_uv_scale: surface.noise_uv_scale_multiplier.max(f32::EPSILON),
    };
    let material = FangyuanAtlasMaterial {
        uniform,
        color_atlas: atlas
            .base_color()
            .expect("validated atlas base handle")
            .clone(),
        // Missing optional channels use a default handle and remain disabled
        // through flags. Loading/uninitialized channels have already taken the
        // whole-material AtlasNotReady fallback above.
        normal_atlas: if matches!(normal_state, FangyuanAtlasImageState::Loaded) {
            atlas.normal().cloned().unwrap_or_default()
        } else {
            Handle::default()
        },
        noise_atlas: if matches!(noise_state, FangyuanAtlasImageState::Loaded) {
            atlas.noise().cloned().unwrap_or_default()
        } else {
            Handle::default()
        },
        alpha_mode: reference.material.alpha_mode,
    };
    FangyuanAtlasMaterialBuildResult::Ready(FangyuanAtlasMaterialBuild {
        material,
        semantic: FangyuanAtlasMaterialSemantic {
            requested_preset_id: reference.diagnostic.requested_preset_id.clone(),
            resolved_preset_id: reference.diagnostic.resolved_preset_id.clone(),
            fallback_reason: reference.diagnostic.fallback_reason.clone(),
            uniform,
            atlas_fallback_flags: atlas.status().fallback_flags,
            surface_tile_id: reference.diagnostic.surface_tile_id.clone(),
            projection: reference.diagnostic.projection,
            projection_requested: reference.diagnostic.projection_requested,
            projection_fallback_reason: reference.diagnostic.projection_fallback_reason.clone(),
            render_model: reference.diagnostic.render_model,
            render_model_fallback: reference.diagnostic.render_model_fallback.clone(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::fangyuan::{
        FangyuanAtlasFallbackFlags, FangyuanPrimitiveKind, FangyuanPrimitiveLifecycle,
        FangyuanPrimitiveRole, load_first_package_fangyuan_surface_preset_registry,
    };
    use bevy::asset::AssetPlugin;

    fn primitive(preset: Option<&str>, noise: bool) -> FangyuanPrimitive {
        FangyuanPrimitive::with_surface_metadata(
            FangyuanPrimitiveKind::Cube,
            Vec3::ZERO,
            Vec3::ONE,
            Color::WHITE,
            FangyuanPrimitiveRole::Decoration,
            1.0,
            0.0,
            None,
            preset.map(str::to_string),
            noise,
            FangyuanPrimitiveLifecycle::empty(),
        )
    }

    fn loaded_atlas_cache() -> (App, FangyuanAtlasAssetCache) {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>();
        let asset_server = app.world().resource::<AssetServer>().clone();
        let mut atlas = FangyuanAtlasAssetCache::default();
        atlas.ensure_loaded(&asset_server, true);
        (app, atlas)
    }

    fn ready_atlas_cache() -> FangyuanAtlasAssetCache {
        FangyuanAtlasAssetCache::test_ready()
    }

    #[test]
    fn material_semantics_and_keys_are_stable() {
        let registry = load_first_package_fangyuan_surface_preset_registry().unwrap();
        let profiles = FangyuanMaterialProfileRegistry::default();
        let atlas = ready_atlas_cache();
        let first = fangyuan_atlas_material(
            &primitive(Some("material:surface/x00/y01"), false),
            &registry,
            &profiles,
            &atlas,
        );
        let second = fangyuan_atlas_material(
            &primitive(Some("material:surface/x00/y01"), false),
            &registry,
            &profiles,
            &atlas,
        );
        let (
            FangyuanAtlasMaterialBuildResult::Ready(first),
            FangyuanAtlasMaterialBuildResult::Ready(second),
        ) = (first, second)
        else {
            panic!("expected atlas material builds");
        };
        assert_eq!(first.semantic, second.semantic);
        assert_eq!(
            FangyuanAtlasMaterialKey::from_material(&first.material),
            FangyuanAtlasMaterialKey::from_material(&second.material)
        );
        assert_eq!(
            first.material.color_atlas.id(),
            second.material.color_atlas.id()
        );
        assert_eq!(first.material.uniform.tile_index, 16);
    }

    #[test]
    fn flags_encode_channels_flip_debug_and_not_ready_without_variants() {
        let registry = load_first_package_fangyuan_surface_preset_registry().unwrap();
        let profiles = FangyuanMaterialProfileRegistry::default();
        let atlas = ready_atlas_cache();
        let FangyuanAtlasMaterialBuildResult::Ready(build) = fangyuan_atlas_material_with_options(
            &primitive(Some("material:surface/x06/y12"), true),
            &registry,
            &profiles,
            &atlas,
            FangyuanAtlasMaterialCapabilities::default(),
            FangyuanAtlasDebugMode::Final,
        ) else {
            panic!("expected atlas material build");
        };
        let flags = FangyuanAtlasMaterialFlags(build.material.uniform.flags);
        assert!(flags.contains(FangyuanAtlasMaterialFlags::NORMAL_ENABLED));
        assert!(flags.contains(FangyuanAtlasMaterialFlags::NOISE_ENABLED));
        assert!(flags.contains(FangyuanAtlasMaterialFlags::GREEN_FLIP_Y));
        assert!(flags.contains(FangyuanAtlasMaterialFlags::DEBUG));
        assert!(flags.contains(FangyuanAtlasMaterialFlags::DEBUG_FINAL));
        assert!(!flags.contains(FangyuanAtlasMaterialFlags::ATLAS_NOT_READY));
    }

    #[test]
    fn unavailable_catalog_or_capability_returns_reference_fallback() {
        let registry = load_first_package_fangyuan_surface_preset_registry().unwrap();
        let profiles = FangyuanMaterialProfileRegistry::default();
        let atlas = FangyuanAtlasAssetCache::default();
        let result = fangyuan_atlas_material(&primitive(None, false), &registry, &profiles, &atlas);
        assert!(matches!(
            result,
            FangyuanAtlasMaterialBuildResult::Fallback {
                reason: FangyuanAtlasMaterialFallbackReason::CatalogUnavailable,
                ..
            }
        ));

        let (_app, loaded) = loaded_atlas_cache();
        let not_ready =
            fangyuan_atlas_material(&primitive(None, false), &registry, &profiles, &loaded);
        assert!(matches!(
            not_ready,
            FangyuanAtlasMaterialBuildResult::Fallback {
                reason: FangyuanAtlasMaterialFallbackReason::AtlasNotReady,
                ..
            }
        ));
        let ready = ready_atlas_cache();
        let result = fangyuan_atlas_material_with_options(
            &primitive(None, false),
            &registry,
            &profiles,
            &ready,
            FangyuanAtlasMaterialCapabilities {
                custom_material: false,
                ..default()
            },
            FangyuanAtlasDebugMode::Off,
        );
        assert!(matches!(
            result,
            FangyuanAtlasMaterialBuildResult::Fallback {
                reason: FangyuanAtlasMaterialFallbackReason::UnsupportedCapability,
                ..
            }
        ));
    }

    #[test]
    fn missing_optional_channels_disable_only_their_shader_paths() {
        let registry = load_first_package_fangyuan_surface_preset_registry().unwrap();
        let profiles = FangyuanMaterialProfileRegistry::default();
        let atlas = FangyuanAtlasAssetCache::test_optional_channels_missing();

        let FangyuanAtlasMaterialBuildResult::Ready(build) = fangyuan_atlas_material(
            &primitive(Some("material:surface/x06/y12"), true),
            &registry,
            &profiles,
            &atlas,
        ) else {
            panic!("missing optional channels should not force reference fallback");
        };

        let flags = FangyuanAtlasMaterialFlags(build.material.uniform.flags);
        assert!(!flags.contains(FangyuanAtlasMaterialFlags::NORMAL_ENABLED));
        assert!(!flags.contains(FangyuanAtlasMaterialFlags::NOISE_ENABLED));
        assert!(flags.contains(FangyuanAtlasMaterialFlags::FALLBACK));
        assert!(
            build
                .semantic
                .atlas_fallback_flags
                .contains(FangyuanAtlasFallbackFlags::MISSING_NORMAL)
        );
        assert!(
            build
                .semantic
                .atlas_fallback_flags
                .contains(FangyuanAtlasFallbackFlags::MISSING_NOISE)
        );
        assert_eq!(build.material.normal_atlas, Handle::default());
        assert_eq!(build.material.noise_atlas, Handle::default());
    }

    #[test]
    fn material_cache_reuses_bindings_and_is_bounded() {
        let registry = load_first_package_fangyuan_surface_preset_registry().unwrap();
        let profiles = FangyuanMaterialProfileRegistry::default();
        let atlas = ready_atlas_cache();
        let FangyuanAtlasMaterialBuildResult::Ready(build) = fangyuan_atlas_material(
            &primitive(Some("material:surface/x00/y01"), false),
            &registry,
            &profiles,
            &atlas,
        ) else {
            panic!("expected atlas material build");
        };
        let mut assets = Assets::<FangyuanAtlasMaterial>::default();
        let mut cache = FangyuanAtlasMaterialCache::with_max_assets(1);
        let first = cache.get_or_insert(&build.material, &mut assets).unwrap();
        let repeated = cache.get_or_insert(&build.material, &mut assets).unwrap();
        assert_eq!(first, repeated);
        assert_eq!(cache.len(), 1);
        cache.clear(&mut assets);
        assert_eq!(cache.len(), 0);
        assert!(assets.get(&first).is_none());
    }

    #[test]
    fn changed_tile_changes_material_key_and_reload_can_clear_cache() {
        let registry = load_first_package_fangyuan_surface_preset_registry().unwrap();
        let profiles = FangyuanMaterialProfileRegistry::default();
        let (app, _loading) = loaded_atlas_cache();
        let mut atlas = ready_atlas_cache();
        let first = fangyuan_atlas_material(
            &primitive(Some("material:surface/x00/y01"), false),
            &registry,
            &profiles,
            &atlas,
        );
        let second = fangyuan_atlas_material(
            &primitive(Some("material:surface/x15/y15"), false),
            &registry,
            &profiles,
            &atlas,
        );
        let (
            FangyuanAtlasMaterialBuildResult::Ready(first),
            FangyuanAtlasMaterialBuildResult::Ready(second),
        ) = (first, second)
        else {
            panic!("expected atlas material builds");
        };
        assert_ne!(
            FangyuanAtlasMaterialKey::from_material(&first.material),
            FangyuanAtlasMaterialKey::from_material(&second.material)
        );
        let mut cache = FangyuanAtlasMaterialCache::default();
        let mut assets = Assets::<FangyuanAtlasMaterial>::default();
        cache.get_or_insert(&first.material, &mut assets).unwrap();
        assert_eq!(cache.len(), 1);
        let server = app.world().resource::<AssetServer>().clone();
        atlas.reload(&server, true);
        cache.clear(&mut assets);
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn tile_uv_uses_texel_centers_and_stays_inside_requested_cell() {
        let first = fangyuan_atlas_material_uv(Vec2::ZERO, Vec2::ONE, 0).unwrap();
        let first_rect = super::fangyuan_atlas_tile_content_uv_rect(0).unwrap();
        assert!(first.abs_diff_eq(first_rect.min, 1e-7));

        let last =
            fangyuan_atlas_material_uv(Vec2::new(0.999_999, 0.999_999), Vec2::ONE, 255).unwrap();
        let last_rect = super::fangyuan_atlas_tile_content_uv_rect(255).unwrap();
        assert!(last.x < last_rect.max.x && last.y < last_rect.max.y);
        assert!(last.x > last_rect.min.x && last.y > last_rect.min.y);

        let repeated =
            fangyuan_atlas_material_uv(Vec2::new(1.0, 1.0), Vec2::splat(3.0), 16).unwrap();
        let repeated_rect = super::fangyuan_atlas_tile_content_uv_rect(16).unwrap();
        assert!(repeated.abs_diff_eq(repeated_rect.min, 1e-7));
        assert!(repeated.x >= repeated_rect.min.x && repeated.x <= repeated_rect.max.x);
        assert!(repeated.y >= repeated_rect.min.y && repeated.y <= repeated_rect.max.y);
    }

    #[test]
    fn local_repeat_scales_world_size_and_reference_repeat_without_cross_tile_state() {
        assert_eq!(
            fangyuan_atlas_local_repeat(
                FangyuanPrimitiveKind::Cube,
                Vec3::new(2.0, 3.0, 4.0),
                [1.0, 1.0]
            ),
            Some(Vec2::new(2.0, 3.0))
        );
        assert_eq!(
            fangyuan_atlas_local_repeat(
                FangyuanPrimitiveKind::Cube,
                Vec3::new(2.0, 3.0, 4.0),
                [2.0, 3.0]
            ),
            Some(Vec2::ONE)
        );
        let sphere = fangyuan_atlas_local_repeat(
            FangyuanPrimitiveKind::Sphere,
            Vec3::splat(2.0),
            [1.0, 1.0],
        )
        .unwrap();
        assert!(sphere.x > 6.0 && sphere.y > 2.0);
        assert!(
            fangyuan_atlas_local_repeat(FangyuanPrimitiveKind::Cube, Vec3::NAN, [1.0, 1.0])
                .is_none()
        );
    }

    #[test]
    fn uniform_projects_reference_pbr_and_profile_alpha_emissive_once() {
        let registry = load_first_package_fangyuan_surface_preset_registry().unwrap();
        let profiles = FangyuanMaterialProfileRegistry::default();
        let atlas = ready_atlas_cache();
        let primitive = primitive(Some("material:surface/x00/y01"), false);
        let reference =
            fangyuan_standard_material_reference(&primitive, &registry, &profiles, &atlas);
        let FangyuanAtlasMaterialBuildResult::Ready(build) =
            fangyuan_atlas_material(&primitive, &registry, &profiles, &atlas)
        else {
            panic!("expected atlas material build");
        };
        assert_eq!(
            build.material.uniform.roughness(),
            reference.surface.roughness
        );
        assert_eq!(
            build.material.uniform.metallic(),
            reference.surface.metallic
        );
        assert_eq!(
            build.material.uniform.reflectance(),
            reference.surface.reflectance
        );
        assert_eq!(build.material.uniform.ambient_occlusion(), 1.0);
        let tint = reference.composed_params.color.to_linear();
        assert_eq!(
            build.material.uniform.tint,
            Vec4::new(tint.red, tint.green, tint.blue, tint.alpha)
        );
        assert_eq!(
            build.material.uniform.emissive_alpha.w,
            reference.composed_params.alpha
        );
    }

    #[test]
    fn shader_source_documents_exact_tile_mapping_srgb_and_debug_outputs() {
        const SHADER: &str = include_str!("../../../assets/shaders/fangyuan_atlas_material.wgsl");
        assert!(SHADER.contains("fract(local_uv * material.surface.xy)"));
        assert!(SHADER.contains("vec2<f32>(2.5)"));
        assert!(SHADER.contains("vec2<f32>(61.5)"));
        assert!(SHADER.contains("/ 1024.0"));
        assert!(SHADER.contains("textureSample(color_atlas, color_sampler, uv).rgb"));
        assert!(SHADER.contains("sampled * material.tint.rgb"));
        assert!(SHADER.contains("material.flags & 64u"));
        assert!(SHADER.contains("material.flags & 128u"));
        assert!(SHADER.contains("material.noise_uv_scale"));
        assert!(SHADER.contains("decode_normal_sample"));
        assert!(SHADER.contains("noise_tile_is_valid"));
        assert!(SHADER.contains("controlled_roughness"));
        assert!(SHADER.contains("no manual sRGB"));
    }

    #[test]
    fn normal_only_and_noise_only_debug_modes_are_data_flags() {
        assert!(
            FangyuanAtlasDebugMode::NormalOnly
                .flags()
                .contains(FangyuanAtlasMaterialFlags::DEBUG_NORMAL_ONLY)
        );
        assert!(
            FangyuanAtlasDebugMode::NoiseOnly
                .flags()
                .contains(FangyuanAtlasMaterialFlags::DEBUG_NOISE_ONLY)
        );
        assert!(
            !FangyuanAtlasDebugMode::Off
                .flags()
                .contains(FangyuanAtlasMaterialFlags::DEBUG)
        );
    }

    #[test]
    fn normal_green_debug_override_switches_uniform_flag_without_shader_variant() {
        let registry = load_first_package_fangyuan_surface_preset_registry().unwrap();
        let profiles = FangyuanMaterialProfileRegistry::default();
        let atlas = ready_atlas_cache();
        let source = primitive(Some("material:surface/x00/y01"), false);
        let FangyuanAtlasMaterialBuildResult::Ready(default_build) =
            fangyuan_atlas_material(&source, &registry, &profiles, &atlas)
        else {
            panic!("expected default atlas build");
        };
        let FangyuanAtlasMaterialBuildResult::Ready(positive_build) =
            fangyuan_atlas_material_with_normal_override(
                &source,
                &registry,
                &profiles,
                &atlas,
                FangyuanAtlasMaterialCapabilities::default(),
                FangyuanAtlasDebugMode::NormalOnly,
                Some(FangyuanNormalGreenDirection::PositiveY),
            )
        else {
            panic!("expected override atlas build");
        };
        let default_flags = FangyuanAtlasMaterialFlags(default_build.material.uniform.flags);
        let positive_flags = FangyuanAtlasMaterialFlags(positive_build.material.uniform.flags);
        assert!(default_flags.contains(FangyuanAtlasMaterialFlags::GREEN_FLIP_Y));
        assert!(!positive_flags.contains(FangyuanAtlasMaterialFlags::GREEN_FLIP_Y));
        assert!(positive_flags.contains(FangyuanAtlasMaterialFlags::DEBUG_NORMAL_ONLY));
        assert_eq!(
            default_build.material.uniform.tile_index,
            positive_build.material.uniform.tile_index
        );
    }

    #[test]
    fn normal_decode_applies_negative_y_strength_and_unit_length() {
        let flat = fangyuan_decode_normal_sample(Vec3::new(0.5, 0.5, 1.0), 0.0, true);
        assert_eq!(flat, Vec3::Z);
        let zero_encoded = fangyuan_decode_normal_sample(Vec3::splat(0.5), 1.0, false);
        assert_eq!(zero_encoded, Vec3::Z);
        let flipped = fangyuan_decode_normal_sample(Vec3::new(0.5, 0.75, 1.0), 1.0, true);
        assert!(flipped.y < 0.0);
        assert!((flipped.length() - 1.0).abs() < 1e-6);
        let clamped = fangyuan_decode_normal_sample(Vec3::new(1.0, 1.0, 1.0), 4.0, false);
        assert!((clamped.length() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn noise_rect_rejects_sentinel_and_reserved_cells() {
        assert!(fangyuan_noise_tile_content_uv_rect(0).is_some());
        assert!(fangyuan_noise_tile_content_uv_rect(91).is_some());
        assert!(fangyuan_noise_tile_content_uv_rect(95).is_none());
        assert!(fangyuan_noise_tile_content_uv_rect(96).is_some());
        assert!(fangyuan_noise_tile_content_uv_rect(169).is_some());
        assert!(fangyuan_noise_tile_content_uv_rect(170).is_none());
        assert!(fangyuan_noise_tile_content_uv_rect(255).is_none());
        assert!(
            FangyuanAtlasMaterialUniform {
                tint: Vec4::ONE,
                surface: Vec4::ONE,
                pbr: Vec4::ZERO,
                emissive_alpha: Vec4::ZERO,
                tile_index: 0,
                noise_tile_index: FangyuanAtlasMaterialUniform::NOISE_TILE_SENTINEL,
                flags: 0,
                noise_uv_scale: 1.0,
            }
            .noise_tile_rect()
            .is_none()
        );
    }

    #[test]
    fn noise_opt_in_and_strength_are_projected_from_catalog() {
        let registry = load_first_package_fangyuan_surface_preset_registry().unwrap();
        let profiles = FangyuanMaterialProfileRegistry::default();
        let atlas = ready_atlas_cache();
        let source = primitive(Some("material:surface/x06/y12"), true);
        let FangyuanAtlasMaterialBuildResult::Ready(build) =
            fangyuan_atlas_material(&source, &registry, &profiles, &atlas)
        else {
            panic!("expected atlas material build");
        };
        assert!(build.material.uniform.noise_strength() > 0.0);
        assert_eq!(build.material.uniform.noise_uv_scale, 3.0);
        assert!(build.material.uniform.noise_tile_rect().is_some());

        let disabled = primitive(Some("material:surface/x06/y12"), false);
        let FangyuanAtlasMaterialBuildResult::Ready(disabled) =
            fangyuan_atlas_material(&disabled, &registry, &profiles, &atlas)
        else {
            panic!("expected atlas material build");
        };
        assert_eq!(disabled.material.uniform.noise_strength(), 0.0);
        assert_eq!(
            disabled.material.uniform.noise_tile_index,
            FangyuanAtlasMaterialUniform::NOISE_TILE_SENTINEL
        );
        assert!(
            !FangyuanAtlasMaterialFlags(disabled.material.uniform.flags)
                .contains(FangyuanAtlasMaterialFlags::NOISE_ENABLED)
        );
    }
}
