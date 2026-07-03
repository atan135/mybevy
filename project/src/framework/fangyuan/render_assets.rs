use bevy::{
    mesh::{MeshBuilder, SphereKind, SphereMeshBuilder},
    prelude::*,
};
use std::collections::HashMap;

use super::{FangyuanPrimitive, FangyuanPrimitiveKind};

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

/// Small render-only asset cache shared by Fangyuan preview features.
///
/// This cache deliberately covers only unit primitive meshes and base color
/// materials. It does not encode gameplay state, material profiles, lifecycle
/// playback, instancing, or mesh merging.
#[derive(Clone, Debug, Default)]
pub struct FangyuanRenderAssetCache {
    unit_cube_mesh: Option<Handle<Mesh>>,
    unit_sphere_mesh: Option<Handle<Mesh>>,
    materials_by_color: HashMap<FangyuanRenderColorKey, Handle<StandardMaterial>>,
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
        self.materials_by_color
            .entry(FangyuanRenderColorKey::from_color(color))
            .or_insert_with(|| materials.add(fangyuan_standard_material_from_color(color)))
            .clone()
    }

    pub fn material_count(&self) -> usize {
        self.materials_by_color.len()
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
    let alpha = color.to_srgba().alpha;
    StandardMaterial {
        base_color: color,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::fangyuan::{
        FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE, FangyuanPrimitiveLifecycle, FangyuanPrimitiveRole,
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
