use bevy::{
    mesh::{MeshBuilder, SphereKind, SphereMeshBuilder},
    prelude::*,
};
use std::collections::HashMap;

use super::{
    FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE, FangyuanMaterialInstanceParams, FangyuanPrimitive,
    FangyuanPrimitiveKind,
};

pub const FANGYUAN_RENDER_UNIT_SPHERE_SECTORS: u32 = 24;
pub const FANGYUAN_RENDER_UNIT_SPHERE_STACKS: u32 = 12;

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
                .get_or_insert_with(|| meshes.add(Cuboid::from_size(Vec3::ONE)))
                .clone(),
            FangyuanPrimitiveKind::Sphere => self
                .unit_sphere_mesh
                .get_or_insert_with(|| {
                    meshes.add(
                        SphereMeshBuilder::new(
                            0.5,
                            SphereKind::Uv {
                                sectors: FANGYUAN_RENDER_UNIT_SPHERE_SECTORS,
                                stacks: FANGYUAN_RENDER_UNIT_SPHERE_STACKS,
                            },
                        )
                        .build(),
                    )
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
}
