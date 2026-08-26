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
    FANGYUAN_NORMAL_SAMPLING_POLICY, FangyuanAtlasAssetCache, FangyuanAtlasChannel,
    FangyuanAtlasImageState, FangyuanCubeFace, FangyuanMaterialProfileRegistry, FangyuanPrimitive,
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
    pub _padding: u32,
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
}

/// Stable semantic snapshot used by diagnostics and future instance data.
#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanAtlasMaterialSemantic {
    pub requested_preset_id: Option<String>,
    pub resolved_preset_id: String,
    pub fallback_reason: Option<FangyuanSurfacePresetFallbackReason>,
    pub uniform: FangyuanAtlasMaterialUniform,
    pub atlas_fallback_flags: super::FangyuanAtlasFallbackFlags,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FangyuanAtlasMaterialFallbackReason {
    CatalogUnavailable,
    AtlasNotReady,
    MissingBaseColorAtlas,
    MissingNormalAtlas,
    MissingNoiseAtlas,
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
                u._padding,
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
    let reference =
        fangyuan_standard_material_reference(primitive, surface_registry, material_registry, atlas);

    let reason = if !capabilities.custom_material || !capabilities.texture_bindings {
        Some(FangyuanAtlasMaterialFallbackReason::UnsupportedCapability)
    } else if !capabilities.shader_available {
        Some(FangyuanAtlasMaterialFallbackReason::ShaderUnavailable)
    } else if !atlas.status().catalog_available {
        Some(FangyuanAtlasMaterialFallbackReason::CatalogUnavailable)
    } else if atlas.base_color().is_none()
        || matches!(
            atlas.status().state(FangyuanAtlasChannel::BaseColor),
            FangyuanAtlasImageState::Missing | FangyuanAtlasImageState::Failed
        )
    {
        Some(FangyuanAtlasMaterialFallbackReason::MissingBaseColorAtlas)
    } else if atlas.normal().is_none()
        || matches!(
            atlas.status().state(FangyuanAtlasChannel::Normal),
            FangyuanAtlasImageState::Missing | FangyuanAtlasImageState::Failed
        )
    {
        Some(FangyuanAtlasMaterialFallbackReason::MissingNormalAtlas)
    } else if atlas.noise().is_none()
        || matches!(
            atlas.status().state(FangyuanAtlasChannel::Noise),
            FangyuanAtlasImageState::Missing | FangyuanAtlasImageState::Failed
        )
    {
        Some(FangyuanAtlasMaterialFallbackReason::MissingNoiseAtlas)
    } else if !atlas.status().is_ready() {
        Some(FangyuanAtlasMaterialFallbackReason::AtlasNotReady)
    } else {
        None
    };

    if let Some(reason) = reason {
        return FangyuanAtlasMaterialBuildResult::Fallback { reason, reference };
    }

    // The reference adapter owns this resolver result. The atlas path only
    // projects the shared snapshot into its uniform layout.
    let surface = &reference.surface;
    let repeat =
        fangyuan_atlas_local_repeat(primitive.kind(), primitive.scale(), surface.repeat_size_m)
            .unwrap_or(Vec2::ONE);
    let tint = reference.composed_params.color.to_linear();
    let emissive = reference.material.emissive;
    let normal_enabled =
        surface.flags.normal_enabled && surface.normal_strength > 0.0 && atlas.normal().is_some();
    let noise_enabled = primitive.surface_noise_enabled()
        && surface.flags.noise_available
        && surface.noise_tile_index.is_some()
        && atlas.noise().is_some();
    let green_flip_y =
        FANGYUAN_NORMAL_SAMPLING_POLICY.standard_material_flip_normal_map_y(None) == Some(true);

    let mut flags = debug_mode.flags();
    if normal_enabled {
        flags = flags.union(FangyuanAtlasMaterialFlags::NORMAL_ENABLED);
    }
    if noise_enabled {
        flags = flags.union(FangyuanAtlasMaterialFlags::NOISE_ENABLED);
    }
    if reference.diagnostic.fallback_reason.is_some() {
        flags = flags.union(FangyuanAtlasMaterialFlags::FALLBACK);
    }
    if green_flip_y {
        flags = flags.union(FangyuanAtlasMaterialFlags::GREEN_FLIP_Y);
    }
    if !atlas.status().is_ready() {
        flags = flags.union(FangyuanAtlasMaterialFlags::ATLAS_NOT_READY);
    }

    let noise_tile_index = surface
        .noise_tile_index
        .map(u32::from)
        .unwrap_or(FangyuanAtlasMaterialUniform::NOISE_TILE_SENTINEL);
    let uniform = FangyuanAtlasMaterialUniform {
        tint: Vec4::new(tint.red, tint.green, tint.blue, tint.alpha),
        surface: Vec4::new(
            repeat.x,
            repeat.y,
            surface.roughness.clamp(0.0, 1.0),
            surface.metallic.clamp(0.0, 1.0),
        ),
        // Noise strength is intentionally a reserved v0 value.  Stage 6 will
        // source catalog modulation constants without changing this layout.
        pbr: Vec4::new(
            surface.reflectance.clamp(0.0, 1.0),
            // Catalog schema v1 requires AO=1.0. Keep the shader contract
            // explicit even if future catalogs expose a diagnostic AO value.
            1.0,
            surface.normal_strength.max(0.0),
            if noise_enabled { 1.0 } else { 0.0 },
        ),
        emissive_alpha: Vec4::new(emissive.red, emissive.green, emissive.blue, tint.alpha),
        tile_index: u32::from(surface.surface_tile_index),
        noise_tile_index,
        flags: flags.bits(),
        _padding: 0,
    };
    let material = FangyuanAtlasMaterial {
        uniform,
        color_atlas: atlas
            .base_color()
            .expect("validated atlas base handle")
            .clone(),
        normal_atlas: atlas
            .normal()
            .expect("validated atlas normal handle")
            .clone(),
        noise_atlas: atlas.noise().expect("validated atlas noise handle").clone(),
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
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::fangyuan::{
        FangyuanPrimitiveKind, FangyuanPrimitiveLifecycle, FangyuanPrimitiveRole,
        load_first_package_fangyuan_surface_preset_registry,
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
        assert!(SHADER.contains("no manual sRGB"));
    }
}
