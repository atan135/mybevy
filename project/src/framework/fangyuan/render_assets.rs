use bevy::{
    asset::RenderAssetUsages,
    image::{
        Image, ImageAddressMode, ImageFilterMode, ImageLoaderSettings, ImageSampler,
        ImageSamplerDescriptor,
    },
    prelude::*,
};
use std::{collections::HashMap, path::Path};

use super::{
    FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE, FangyuanCatalogAssetPaths, FangyuanMaterialInstanceParams,
    FangyuanPrimitive, FangyuanPrimitiveKind, FangyuanSurfacePresetRegistry,
    fangyuan_unit_cube_mesh, fangyuan_unit_sphere_mesh,
};

pub const FANGYUAN_RENDER_UNIT_SPHERE_SECTORS: u32 = 24;
pub const FANGYUAN_RENDER_UNIT_SPHERE_STACKS: u32 = 12;

/// Canonical first-package paths for the three shared Fangyuan atlas images.
///
/// These are intentionally runtime constants rather than blueprint fields.
/// Blueprints and resolver output carry stable tile IDs/indices; only this
/// adapter knows how those IDs map to the promoted first-package images.
pub const FANGYUAN_BASE_COLOR_ATLAS_PATH: &str = "fangyuan/atlases/base/surface_color.png";
pub const FANGYUAN_NORMAL_ATLAS_PATH: &str = "fangyuan/atlases/base/surface_normal.png";
pub const FANGYUAN_NOISE_ATLAS_PATH: &str = "fangyuan/atlases/noise/procedural_noise.png";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FangyuanAtlasDiagnosticPreset {
    pub label: &'static str,
    pub preset_id: &'static str,
    pub expected_surface_tile_index: u16,
    pub expected_noise_tile_id: Option<&'static str>,
}

/// Small stable set used by reference rendering and automated diagnostics.
/// Qingyu follows the promoted Qingyu tower fixture's cyan ceramic core.
pub const FANGYUAN_ATLAS_DIAGNOSTIC_PRESETS: &[FangyuanAtlasDiagnosticPreset] = &[
    FangyuanAtlasDiagnosticPreset {
        label: "origin",
        preset_id: "material:surface/x00/y00",
        expected_surface_tile_index: 0,
        expected_noise_tile_id: Some("noise_x04_y07"),
    },
    FangyuanAtlasDiagnosticPreset {
        label: "row_boundary",
        preset_id: "material:surface/x15/y00",
        expected_surface_tile_index: 15,
        expected_noise_tile_id: Some("noise_x00_y00"),
    },
    FangyuanAtlasDiagnosticPreset {
        label: "last_cell",
        preset_id: "material:surface/x15/y15",
        expected_surface_tile_index: 255,
        expected_noise_tile_id: Some("noise_x04_y07"),
    },
    FangyuanAtlasDiagnosticPreset {
        label: "masonry",
        preset_id: "material:surface/x13/y00",
        expected_surface_tile_index: 13,
        expected_noise_tile_id: Some("noise_x04_y07"),
    },
    FangyuanAtlasDiagnosticPreset {
        label: "metal",
        preset_id: "material:surface/x00/y01",
        expected_surface_tile_index: 16,
        expected_noise_tile_id: Some("noise_x00_y00"),
    },
    FangyuanAtlasDiagnosticPreset {
        label: "qingyu_core",
        preset_id: "material:surface/x06/y12",
        expected_surface_tile_index: 198,
        expected_noise_tile_id: Some("noise_x00_y07"),
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FangyuanStandardMaterialReferenceCapabilities {
    pub base_color: bool,
    pub perceptual_roughness: bool,
    pub metallic: bool,
    pub tangent_space_normal: bool,
    pub noise_modulation: bool,
    pub normal_strength: bool,
}

/// StandardMaterial remains the reference renderer for basic PBR comparison.
/// Noise modulation and adjustable normal strength require the atlas shader.
pub const FANGYUAN_STANDARD_MATERIAL_REFERENCE_CAPABILITIES:
    FangyuanStandardMaterialReferenceCapabilities = FangyuanStandardMaterialReferenceCapabilities {
    base_color: true,
    perceptual_roughness: true,
    metallic: true,
    tangent_space_normal: true,
    noise_modulation: false,
    normal_strength: false,
};

/// The atlas channels have different color-space semantics in the catalog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanAtlasChannel {
    BaseColor,
    Normal,
    Noise,
}

impl FangyuanAtlasChannel {
    pub const fn path(self) -> &'static str {
        match self {
            Self::BaseColor => FANGYUAN_BASE_COLOR_ATLAS_PATH,
            Self::Normal => FANGYUAN_NORMAL_ATLAS_PATH,
            Self::Noise => FANGYUAN_NOISE_ATLAS_PATH,
        }
    }

    pub const fn is_srgb(self) -> bool {
        matches!(self, Self::BaseColor)
    }
}

/// Observable state for one shared image handle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FangyuanAtlasImageState {
    #[default]
    Uninitialized,
    Loading,
    Loaded,
    Missing,
    Failed,
}

/// Stable fallback flags emitted by the atlas adapter. A missing channel only
/// disables that channel; the primitive can still use the StandardMaterial
/// fallback path and the other loaded channels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FangyuanAtlasFallbackFlags(u8);

impl FangyuanAtlasFallbackFlags {
    pub const CATALOG_UNAVAILABLE: Self = Self(1 << 0);
    pub const MISSING_BASE_COLOR: Self = Self(1 << 1);
    pub const MISSING_NORMAL: Self = Self(1 << 2);
    pub const MISSING_NOISE: Self = Self(1 << 3);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// Runtime-facing status for the shared atlas image set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanAtlasAssetStatus {
    pub catalog_available: bool,
    pub base_color: FangyuanAtlasImageState,
    pub normal: FangyuanAtlasImageState,
    pub noise: FangyuanAtlasImageState,
    pub fallback_flags: FangyuanAtlasFallbackFlags,
}

impl Default for FangyuanAtlasAssetStatus {
    fn default() -> Self {
        Self {
            catalog_available: false,
            base_color: FangyuanAtlasImageState::Uninitialized,
            normal: FangyuanAtlasImageState::Uninitialized,
            noise: FangyuanAtlasImageState::Uninitialized,
            fallback_flags: FangyuanAtlasFallbackFlags::CATALOG_UNAVAILABLE,
        }
    }
}

impl FangyuanAtlasAssetStatus {
    pub const fn state(&self, channel: FangyuanAtlasChannel) -> FangyuanAtlasImageState {
        match channel {
            FangyuanAtlasChannel::BaseColor => self.base_color,
            FangyuanAtlasChannel::Normal => self.normal,
            FangyuanAtlasChannel::Noise => self.noise,
        }
    }

    pub const fn is_ready(&self) -> bool {
        matches!(self.base_color, FangyuanAtlasImageState::Loaded)
            && matches!(self.normal, FangyuanAtlasImageState::Loaded)
            && matches!(self.noise, FangyuanAtlasImageState::Loaded)
            && self.catalog_available
    }
}

/// Shared atlas handles and diagnostics. One cache is intended to serve all
/// Fangyuan primitives in a scene; callers must not load a texture per tile.
#[derive(Clone, Debug, Default)]
pub struct FangyuanAtlasAssetCache {
    base_color: Option<Handle<Image>>,
    normal: Option<Handle<Image>>,
    noise: Option<Handle<Image>>,
    status: FangyuanAtlasAssetStatus,
}

impl FangyuanAtlasAssetCache {
    #[cfg(test)]
    pub(crate) fn test_ready() -> Self {
        Self {
            base_color: Some(Handle::default()),
            normal: Some(Handle::default()),
            noise: Some(Handle::default()),
            status: FangyuanAtlasAssetStatus {
                catalog_available: true,
                base_color: FangyuanAtlasImageState::Loaded,
                normal: FangyuanAtlasImageState::Loaded,
                noise: FangyuanAtlasImageState::Loaded,
                fallback_flags: FangyuanAtlasFallbackFlags::empty(),
            },
        }
    }

    #[cfg(test)]
    pub(crate) fn test_optional_channels_missing() -> Self {
        Self {
            base_color: Some(Handle::default()),
            normal: None,
            noise: None,
            status: FangyuanAtlasAssetStatus {
                catalog_available: true,
                base_color: FangyuanAtlasImageState::Loaded,
                normal: FangyuanAtlasImageState::Missing,
                noise: FangyuanAtlasImageState::Failed,
                fallback_flags: FangyuanAtlasFallbackFlags::MISSING_NORMAL
                    .union(FangyuanAtlasFallbackFlags::MISSING_NOISE),
            },
        }
    }

    /// Load all three canonical images once, applying channel-specific color
    /// space and sampler settings. The catalog availability result comes from
    /// the already validated resolver boundary.
    pub fn ensure_loaded(&mut self, asset_server: &AssetServer, catalog_available: bool) {
        if self.base_color.is_none() {
            self.base_color = Some(load_fangyuan_atlas_image(
                asset_server,
                FangyuanAtlasChannel::BaseColor,
            ));
            self.status.base_color = FangyuanAtlasImageState::Loading;
        }
        if self.normal.is_none() {
            self.normal = Some(load_fangyuan_atlas_image(
                asset_server,
                FangyuanAtlasChannel::Normal,
            ));
            self.status.normal = FangyuanAtlasImageState::Loading;
        }
        if self.noise.is_none() {
            self.noise = Some(load_fangyuan_atlas_image(
                asset_server,
                FangyuanAtlasChannel::Noise,
            ));
            self.status.noise = FangyuanAtlasImageState::Loading;
        }
        self.status.catalog_available = catalog_available;
        self.recompute_fallback_flags();
    }

    /// Refresh image load states without creating new handles.
    pub fn refresh(&mut self, asset_server: &AssetServer) {
        self.status.base_color = image_load_state(asset_server, self.base_color.as_ref());
        self.status.normal = image_load_state(asset_server, self.normal.as_ref());
        self.status.noise = image_load_state(asset_server, self.noise.as_ref());
        self.recompute_fallback_flags();
    }

    /// Drop the local strong handles and reset diagnostics. AssetServer may
    /// retain the underlying asset while other consumers still hold it.
    pub fn clear(&mut self) {
        self.base_color = None;
        self.normal = None;
        self.noise = None;
        self.status = FangyuanAtlasAssetStatus::default();
    }

    pub fn reload(&mut self, asset_server: &AssetServer, catalog_available: bool) {
        self.clear();
        self.ensure_loaded(asset_server, catalog_available);
    }

    pub fn base_color(&self) -> Option<&Handle<Image>> {
        self.base_color.as_ref()
    }

    pub fn normal(&self) -> Option<&Handle<Image>> {
        self.normal.as_ref()
    }

    pub fn noise(&self) -> Option<&Handle<Image>> {
        self.noise.as_ref()
    }

    pub fn status(&self) -> &FangyuanAtlasAssetStatus {
        &self.status
    }

    pub fn handles(&self, channel: FangyuanAtlasChannel) -> Option<&Handle<Image>> {
        match channel {
            FangyuanAtlasChannel::BaseColor => self.base_color(),
            FangyuanAtlasChannel::Normal => self.normal(),
            FangyuanAtlasChannel::Noise => self.noise(),
        }
    }

    fn recompute_fallback_flags(&mut self) {
        let mut flags = if self.status.catalog_available {
            FangyuanAtlasFallbackFlags::empty()
        } else {
            FangyuanAtlasFallbackFlags::CATALOG_UNAVAILABLE
        };
        if matches!(
            self.status.base_color,
            FangyuanAtlasImageState::Missing | FangyuanAtlasImageState::Failed
        ) {
            flags = flags.union(FangyuanAtlasFallbackFlags::MISSING_BASE_COLOR);
        }
        if matches!(
            self.status.normal,
            FangyuanAtlasImageState::Missing | FangyuanAtlasImageState::Failed
        ) {
            flags = flags.union(FangyuanAtlasFallbackFlags::MISSING_NORMAL);
        }
        if matches!(
            self.status.noise,
            FangyuanAtlasImageState::Missing | FangyuanAtlasImageState::Failed
        ) {
            flags = flags.union(FangyuanAtlasFallbackFlags::MISSING_NOISE);
        }
        self.status.fallback_flags = flags;
    }
}

fn load_fangyuan_atlas_image(
    asset_server: &AssetServer,
    channel: FangyuanAtlasChannel,
) -> Handle<Image> {
    asset_server.load_with_settings::<Image, ImageLoaderSettings>(channel.path(), move |settings| {
        settings.is_srgb = channel.is_srgb();
        settings.sampler = ImageSampler::Descriptor(fangyuan_atlas_sampler());
        settings.asset_usage = RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD;
    })
}

fn fangyuan_atlas_sampler() -> ImageSamplerDescriptor {
    ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::ClampToEdge,
        address_mode_v: ImageAddressMode::ClampToEdge,
        address_mode_w: ImageAddressMode::ClampToEdge,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        // Atlas tiles must not bleed through generated mip levels. The first
        // implementation deliberately uses the base level only.
        mipmap_filter: ImageFilterMode::Nearest,
        lod_min_clamp: 0.0,
        lod_max_clamp: 0.0,
        ..default()
    }
}

fn image_load_state(
    asset_server: &AssetServer,
    handle: Option<&Handle<Image>>,
) -> FangyuanAtlasImageState {
    let Some(handle) = handle else {
        return FangyuanAtlasImageState::Uninitialized;
    };
    match asset_server.load_state(handle.id()) {
        bevy::asset::LoadState::Loaded => FangyuanAtlasImageState::Loaded,
        bevy::asset::LoadState::Loading => FangyuanAtlasImageState::Loading,
        bevy::asset::LoadState::Failed(_) => FangyuanAtlasImageState::Failed,
        bevy::asset::LoadState::NotLoaded => FangyuanAtlasImageState::Missing,
    }
}

/// Static checks used by diagnostics and tests before any AssetServer work.
pub fn fangyuan_atlas_paths_are_canonical() -> bool {
    [
        FANGYUAN_BASE_COLOR_ATLAS_PATH,
        FANGYUAN_NORMAL_ATLAS_PATH,
        FANGYUAN_NOISE_ATLAS_PATH,
    ]
    .iter()
    .all(|path| {
        let path = Path::new(path);
        path.starts_with("fangyuan/atlases") && path.extension().is_some_and(|ext| ext == "png")
    })
}

/// Best-effort first-package catalog availability check used by scene
/// diagnostics. A failed check never prevents the image handles from being
/// requested; it only raises the catalog fallback flag.
pub fn fangyuan_atlas_catalog_available() -> bool {
    let surface_path =
        super::first_package_fangyuan_asset_fs_path("fangyuan/atlases/base/surface_atlas.json");
    let Some(fangyuan_root) = surface_path
        .as_deref()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .and_then(Path::parent)
    else {
        return false;
    };
    let paths = FangyuanCatalogAssetPaths::from_fangyuan_asset_root(fangyuan_root);
    FangyuanSurfacePresetRegistry::load_from_paths(&paths).is_ok()
}

/// Quantized RGBA key for the simple preview material cache.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FangyuanRenderColorKey([u8; 4]);

impl FangyuanRenderColorKey {
    pub fn from_color(color: Color) -> Self {
        let color = color.to_srgba();
        Self([
            quantize_color_channel(color.red),
            quantize_color_channel(color.green),
            quantize_color_channel(color.blue),
            quantize_color_channel(color.alpha),
        ])
    }
}

/// Quantized material key for StandardMaterial cache entries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FangyuanRenderMaterialKey {
    color: FangyuanRenderColorKey,
    emissive: u16,
}

impl FangyuanRenderMaterialKey {
    pub fn from_color_and_emissive(color: Color, emissive: f32) -> Self {
        Self {
            color: FangyuanRenderColorKey::from_color(color),
            emissive: quantize_emissive(emissive),
        }
    }

    pub fn from_material_params(params: &FangyuanMaterialInstanceParams) -> Self {
        Self::from_color_and_emissive(params.color, params.emissive)
    }

    pub fn from_color(color: Color) -> Self {
        Self::from_color_and_emissive(color, FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE)
    }
}

/// Small render-only asset cache shared by Fangyuan preview features.
///
/// This cache deliberately covers only unit primitive meshes and base color
/// materials. It does not encode gameplay state, material profiles, lifecycle
/// playback, instancing, or mesh merging.
#[derive(Clone, Debug, Default)]
pub struct FangyuanRenderAssetCache {
    unit_cube_mesh: Option<Handle<Mesh>>,
    unit_sphere_mesh: Option<Handle<Mesh>>,
    materials_by_key: HashMap<FangyuanRenderMaterialKey, Handle<StandardMaterial>>,
}

impl FangyuanRenderAssetCache {
    pub fn unit_mesh(
        &mut self,
        kind: FangyuanPrimitiveKind,
        meshes: &mut Assets<Mesh>,
    ) -> Handle<Mesh> {
        match kind {
            FangyuanPrimitiveKind::Cube => self
                .unit_cube_mesh
                .get_or_insert_with(|| meshes.add(fangyuan_unit_cube_mesh()))
                .clone(),
            FangyuanPrimitiveKind::Sphere => self
                .unit_sphere_mesh
                .get_or_insert_with(|| {
                    meshes.add(fangyuan_unit_sphere_mesh(
                        0.5,
                        FANGYUAN_RENDER_UNIT_SPHERE_SECTORS,
                        FANGYUAN_RENDER_UNIT_SPHERE_STACKS,
                    ))
                })
                .clone(),
        }
    }

    pub fn material(
        &mut self,
        color: Color,
        materials: &mut Assets<StandardMaterial>,
    ) -> Handle<StandardMaterial> {
        self.material_from_color_and_emissive(color, FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE, materials)
    }

    pub fn material_from_params(
        &mut self,
        params: &FangyuanMaterialInstanceParams,
        materials: &mut Assets<StandardMaterial>,
    ) -> Handle<StandardMaterial> {
        self.materials_by_key
            .entry(FangyuanRenderMaterialKey::from_material_params(params))
            .or_insert_with(|| materials.add(fangyuan_standard_material_from_params(params)))
            .clone()
    }

    pub fn material_from_color_and_emissive(
        &mut self,
        color: Color,
        emissive: f32,
        materials: &mut Assets<StandardMaterial>,
    ) -> Handle<StandardMaterial> {
        self.materials_by_key
            .entry(FangyuanRenderMaterialKey::from_color_and_emissive(
                color, emissive,
            ))
            .or_insert_with(|| {
                materials.add(fangyuan_standard_material_from_color_and_emissive(
                    color, emissive,
                ))
            })
            .clone()
    }

    pub fn material_count(&self) -> usize {
        self.materials_by_key.len()
    }

    #[cfg(test)]
    pub fn unit_cube_mesh(&self) -> Option<&Handle<Mesh>> {
        self.unit_cube_mesh.as_ref()
    }

    #[cfg(test)]
    pub fn unit_sphere_mesh(&self) -> Option<&Handle<Mesh>> {
        self.unit_sphere_mesh.as_ref()
    }
}

pub fn fangyuan_render_transform_from_primitive(primitive: &FangyuanPrimitive) -> Transform {
    Transform::from_translation(primitive.local_position).with_scale(primitive.scale)
}

pub fn fangyuan_standard_material_from_color(color: Color) -> StandardMaterial {
    fangyuan_standard_material_from_color_and_emissive(color, FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE)
}

pub fn fangyuan_standard_material_from_params(
    params: &FangyuanMaterialInstanceParams,
) -> StandardMaterial {
    fangyuan_standard_material_from_color_and_emissive(params.color, params.emissive)
}

pub fn fangyuan_standard_material_from_color_and_emissive(
    color: Color,
    emissive: f32,
) -> StandardMaterial {
    let alpha = color.to_srgba().alpha;
    let emissive = sanitize_emissive(emissive);
    let emissive_color = if emissive > FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE {
        let linear = color.to_linear();
        LinearRgba::new(
            linear.red * emissive,
            linear.green * emissive,
            linear.blue * emissive,
            1.0,
        )
    } else {
        LinearRgba::BLACK
    };

    StandardMaterial {
        base_color: color,
        emissive: emissive_color,
        perceptual_roughness: 0.92,
        alpha_mode: if alpha < 1.0 {
            AlphaMode::Blend
        } else {
            AlphaMode::Opaque
        },
        ..default()
    }
}

fn quantize_color_channel(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn quantize_emissive(value: f32) -> u16 {
    (sanitize_emissive(value) * 256.0).round() as u16
}

fn sanitize_emissive(value: f32) -> f32 {
    if value.is_finite() {
        value.max(FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE)
    } else {
        FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::fangyuan::{
        FANGYUAN_MATERIAL_PROFILE_VERSION, FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE,
        FangyuanMaterialAlphaPolicy, FangyuanMaterialBaseParams, FangyuanMaterialEmissivePolicy,
        FangyuanMaterialProfile, FangyuanPrimitiveLifecycle, FangyuanPrimitiveRole,
    };

    #[test]
    fn fangyuan_player_preview_and_fangyuan_home_render_asset_cache_reuses_unit_meshes() {
        let mut cache = FangyuanRenderAssetCache::default();
        let mut meshes = Assets::<Mesh>::default();

        let cube_a = cache.unit_mesh(FangyuanPrimitiveKind::Cube, &mut meshes);
        let cube_b = cache.unit_mesh(FangyuanPrimitiveKind::Cube, &mut meshes);
        let sphere_a = cache.unit_mesh(FangyuanPrimitiveKind::Sphere, &mut meshes);
        let sphere_b = cache.unit_mesh(FangyuanPrimitiveKind::Sphere, &mut meshes);

        assert_eq!(cube_a, cube_b);
        assert_eq!(sphere_a, sphere_b);
        assert_ne!(cube_a, sphere_a);
        assert_eq!(cache.unit_cube_mesh(), Some(&cube_a));
        assert_eq!(cache.unit_sphere_mesh(), Some(&sphere_a));
    }

    #[test]
    fn fangyuan_player_preview_and_fangyuan_home_material_cache_uses_base_color_alpha() {
        let mut cache = FangyuanRenderAssetCache::default();
        let mut materials = Assets::<StandardMaterial>::default();
        let color = Color::srgba(0.2, 0.4, 0.6, 0.35);

        let material_a = cache.material(color, &mut materials);
        let material_b = cache.material(color, &mut materials);
        let material = materials.get(&material_a).unwrap();

        assert_eq!(material_a, material_b);
        assert_eq!(cache.material_count(), 1);
        assert_eq!(material.base_color, color);
        assert!(matches!(material.alpha_mode.clone(), AlphaMode::Blend));
        assert_eq!(material.emissive, LinearRgba::BLACK);
    }

    #[test]
    fn fangyuan_material_standard_cache_keys_profile_composed_alpha_and_emissive() {
        let mut cache = FangyuanRenderAssetCache::default();
        let mut materials = Assets::<StandardMaterial>::default();
        let profile = FangyuanMaterialProfile {
            stable_id: "fx/warm".to_string(),
            version: FANGYUAN_MATERIAL_PROFILE_VERSION.to_string(),
            base: FangyuanMaterialBaseParams {
                color: Color::srgba(0.5, 1.0, 0.25, 1.0),
                alpha: 0.5,
                emissive: 1.0,
            },
            alpha_policy: FangyuanMaterialAlphaPolicy::MultiplyClamp { min: 0.0, max: 1.0 },
            emissive_policy: FangyuanMaterialEmissivePolicy::AdditiveClamp { max: 4.0 },
            debug_label: "Warm".to_string(),
        };
        let primitive = FangyuanPrimitive::with_runtime_metadata(
            FangyuanPrimitiveKind::Cube,
            Vec3::ZERO,
            Vec3::ONE,
            Color::srgba(0.2, 0.4, 0.6, 1.0),
            FangyuanPrimitiveRole::Decoration,
            0.5,
            2.0,
            Some("fx/warm".to_string()),
            FangyuanPrimitiveLifecycle::empty(),
        );
        let params = profile.compose_primitive(&primitive);

        let material_a = cache.material_from_params(&params, &mut materials);
        let material_b = cache.material_from_params(&params, &mut materials);
        let material = materials.get(&material_a).unwrap();

        assert_eq!(material_a, material_b);
        assert_eq!(cache.material_count(), 1);
        assert_eq!(material.base_color, Color::srgba(0.1, 0.4, 0.15, 0.25));
        assert!(matches!(material.alpha_mode.clone(), AlphaMode::Blend));
        assert!(material.emissive.red > 0.0);
        assert!(material.emissive.green > material.emissive.red);
    }

    #[test]
    fn fangyuan_player_preview_and_fangyuan_home_render_transform_maps_runtime_primitive_fields() {
        let primitive = FangyuanPrimitive::with_runtime_metadata(
            FangyuanPrimitiveKind::Sphere,
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::new(0.5, 0.75, 1.25),
            Color::srgb(0.2, 0.4, 0.6),
            FangyuanPrimitiveRole::Decoration,
            0.45,
            FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE,
            Some("ignored_by_base_render_cache".to_string()),
            FangyuanPrimitiveLifecycle::new(Some(10), Some(1), Some(11)),
        );

        let transform = fangyuan_render_transform_from_primitive(&primitive);

        assert_eq!(transform.translation, primitive.local_position);
        assert_eq!(transform.scale, primitive.scale);
        assert_eq!(transform.rotation, Quat::IDENTITY);
    }

    #[test]
    fn fangyuan_atlas_paths_and_channel_color_spaces_are_canonical() {
        assert!(fangyuan_atlas_paths_are_canonical());
        assert_eq!(
            FangyuanAtlasChannel::BaseColor.path(),
            FANGYUAN_BASE_COLOR_ATLAS_PATH
        );
        assert_eq!(
            FangyuanAtlasChannel::Normal.path(),
            FANGYUAN_NORMAL_ATLAS_PATH
        );
        assert_eq!(
            FangyuanAtlasChannel::Noise.path(),
            FANGYUAN_NOISE_ATLAS_PATH
        );
        assert!(FangyuanAtlasChannel::BaseColor.is_srgb());
        assert!(!FangyuanAtlasChannel::Normal.is_srgb());
        assert!(!FangyuanAtlasChannel::Noise.is_srgb());
        assert!(fangyuan_atlas_catalog_available());

        let paths = FangyuanCatalogAssetPaths::from_fangyuan_asset_root(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fangyuan"),
        );
        let registry = FangyuanSurfacePresetRegistry::load_from_paths(&paths).unwrap();
        for diagnostic in FANGYUAN_ATLAS_DIAGNOSTIC_PRESETS {
            let preset = registry.get(diagnostic.preset_id).unwrap();
            assert_eq!(
                preset.surface_tile_index,
                diagnostic.expected_surface_tile_index
            );
            assert_eq!(
                preset.noise_tile_id.as_deref(),
                diagnostic.expected_noise_tile_id
            );
        }
        assert!(FANGYUAN_STANDARD_MATERIAL_REFERENCE_CAPABILITIES.base_color);
        assert!(FANGYUAN_STANDARD_MATERIAL_REFERENCE_CAPABILITIES.perceptual_roughness);
        assert!(FANGYUAN_STANDARD_MATERIAL_REFERENCE_CAPABILITIES.metallic);
        assert!(FANGYUAN_STANDARD_MATERIAL_REFERENCE_CAPABILITIES.tangent_space_normal);
        assert!(!FANGYUAN_STANDARD_MATERIAL_REFERENCE_CAPABILITIES.noise_modulation);
        assert!(!FANGYUAN_STANDARD_MATERIAL_REFERENCE_CAPABILITIES.normal_strength);
    }

    #[test]
    fn fangyuan_atlas_cache_reuses_shared_handles_and_reports_catalog_fallback() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<Image>();
        let asset_server = app.world().resource::<AssetServer>().clone();
        let mut cache = FangyuanAtlasAssetCache::default();

        cache.ensure_loaded(&asset_server, false);
        let base = cache.base_color().cloned().unwrap();
        let normal = cache.normal().cloned().unwrap();
        let noise = cache.noise().cloned().unwrap();
        assert!(
            cache
                .status()
                .fallback_flags
                .contains(FangyuanAtlasFallbackFlags::CATALOG_UNAVAILABLE)
        );
        assert_eq!(
            cache.status().state(FangyuanAtlasChannel::BaseColor),
            FangyuanAtlasImageState::Loading
        );

        cache.ensure_loaded(&asset_server, true);
        assert_eq!(cache.base_color(), Some(&base));
        assert_eq!(cache.normal(), Some(&normal));
        assert_eq!(cache.noise(), Some(&noise));
        assert!(
            !cache
                .status()
                .fallback_flags
                .contains(FangyuanAtlasFallbackFlags::CATALOG_UNAVAILABLE)
        );

        cache.clear();
        assert!(cache.base_color().is_none());
        assert_eq!(cache.status(), &FangyuanAtlasAssetStatus::default());
    }

    #[test]
    fn fangyuan_atlas_missing_channels_have_independent_fallback_flags() {
        let mut cache = FangyuanAtlasAssetCache::default();
        cache.status.catalog_available = true;
        cache.status.base_color = FangyuanAtlasImageState::Failed;
        cache.status.normal = FangyuanAtlasImageState::Missing;
        cache.status.noise = FangyuanAtlasImageState::Loaded;
        cache.recompute_fallback_flags();

        let flags = cache.status().fallback_flags;
        assert!(flags.contains(FangyuanAtlasFallbackFlags::MISSING_BASE_COLOR));
        assert!(flags.contains(FangyuanAtlasFallbackFlags::MISSING_NORMAL));
        assert!(!flags.contains(FangyuanAtlasFallbackFlags::MISSING_NOISE));
        assert!(!flags.contains(FangyuanAtlasFallbackFlags::CATALOG_UNAVAILABLE));
    }

    #[test]
    fn fangyuan_atlas_reload_reuses_asset_server_ids_without_cross_tile_loading() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<Image>();
        let asset_server = app.world().resource::<AssetServer>().clone();
        let mut cache = FangyuanAtlasAssetCache::default();

        cache.ensure_loaded(&asset_server, true);
        let first = cache.base_color().cloned().unwrap();
        cache.reload(&asset_server, true);
        let second = cache.base_color().cloned().unwrap();
        // AssetServer deduplicates the canonical path, so a reload resets our
        // local cache without creating duplicate GPU/image assets.
        assert_eq!(first, second);
        assert_eq!(cache.handles(FangyuanAtlasChannel::Normal), cache.normal());
        assert_eq!(cache.handles(FangyuanAtlasChannel::Noise), cache.noise());
    }
}
