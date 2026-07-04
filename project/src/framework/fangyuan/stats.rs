use bevy::prelude::Color;
use std::collections::{BTreeSet, HashSet};

use super::{
    FangyuanMaterialProfileRegistry, FangyuanRenderMaterialKey,
    primitive::{
        FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE, FangyuanPrimitive, FangyuanPrimitiveKind,
        FangyuanPrimitiveRole, FangyuanPrimitiveSet,
    },
};

/// Primitive-set debug statistics computed from runtime primitive data.
///
/// This is a data-model entry point for later budget, LOD, and review reports.
/// It intentionally does not inspect render-only visual entities.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FangyuanPrimitiveSetStats {
    pub total: usize,
    pub cube_count: usize,
    pub sphere_count: usize,
    pub role_distribution: FangyuanPrimitiveRoleDistribution,
    /// Number of distinct display colors, keyed by exact sRGBA channel bits.
    pub color_count: usize,
    /// Number of primitives whose runtime alpha is below fully opaque.
    pub alpha_count: usize,
    /// Number of primitives routed to the transparent render path after material composition.
    pub transparent_count: usize,
    /// Number of primitives routed to the default opaque render path after material composition.
    pub opaque_count: usize,
    /// Number of primitives whose runtime emissive intensity is above default.
    pub emissive_count: usize,
    /// Sum of composed emissive intensity across runtime primitives.
    pub emissive_total: f32,
    /// Number of distinct non-default material profile identifiers.
    pub material_profile_count: usize,
    /// Number of unique StandardMaterial cache keys implied by composed material params.
    pub unique_material_resource_count: usize,
}

impl FangyuanPrimitiveSetStats {
    pub fn from_primitive_set(primitive_set: &FangyuanPrimitiveSet) -> Self {
        Self::from_primitives(primitive_set.primitives())
    }

    pub fn from_primitives(primitives: &[FangyuanPrimitive]) -> Self {
        Self::from_primitives_with_material_registry(
            primitives,
            &FangyuanMaterialProfileRegistry::default(),
        )
    }

    pub fn from_primitive_set_with_material_registry(
        primitive_set: &FangyuanPrimitiveSet,
        registry: &FangyuanMaterialProfileRegistry,
    ) -> Self {
        Self::from_primitives_with_material_registry(primitive_set.primitives(), registry)
    }

    pub fn from_primitives_with_material_registry(
        primitives: &[FangyuanPrimitive],
        registry: &FangyuanMaterialProfileRegistry,
    ) -> Self {
        let mut stats = Self::default();
        let mut colors = BTreeSet::new();
        let mut material_profiles = BTreeSet::new();
        let mut material_resources = HashSet::new();

        for primitive in primitives {
            stats.total += 1;
            match primitive.kind() {
                FangyuanPrimitiveKind::Cube => stats.cube_count += 1,
                FangyuanPrimitiveKind::Sphere => stats.sphere_count += 1,
            }

            stats.role_distribution.increment(primitive.role());
            colors.insert(FangyuanPrimitiveColorKey::from_color(primitive.color()));

            let material = registry.compose_primitive(primitive);
            if material.alpha < 1.0 {
                stats.alpha_count += 1;
                stats.transparent_count += 1;
            } else {
                stats.opaque_count += 1;
            }
            if material.emissive > FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE {
                stats.emissive_count += 1;
            }
            stats.emissive_total += material.emissive;
            material_resources.insert(FangyuanRenderMaterialKey::from_material_params(&material));
            if let Some(material_profile_id) = primitive.material_profile_id() {
                material_profiles.insert(material_profile_id);
            }
        }

        stats.color_count = colors.len();
        stats.material_profile_count = material_profiles.len();
        stats.unique_material_resource_count = material_resources.len();
        stats
    }
}

impl FangyuanPrimitiveSet {
    pub fn stats(&self) -> FangyuanPrimitiveSetStats {
        FangyuanPrimitiveSetStats::from_primitive_set(self)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FangyuanPrimitiveRoleDistribution {
    pub structure: usize,
    pub core: usize,
    pub boundary: usize,
    pub warning: usize,
    pub trail: usize,
    pub impact: usize,
    pub decoration: usize,
    pub socket: usize,
    pub archive: usize,
}

impl FangyuanPrimitiveRoleDistribution {
    pub fn count(&self, role: FangyuanPrimitiveRole) -> usize {
        match role {
            FangyuanPrimitiveRole::Structure => self.structure,
            FangyuanPrimitiveRole::Core => self.core,
            FangyuanPrimitiveRole::Boundary => self.boundary,
            FangyuanPrimitiveRole::Warning => self.warning,
            FangyuanPrimitiveRole::Trail => self.trail,
            FangyuanPrimitiveRole::Impact => self.impact,
            FangyuanPrimitiveRole::Decoration => self.decoration,
            FangyuanPrimitiveRole::Socket => self.socket,
            FangyuanPrimitiveRole::Archive => self.archive,
        }
    }

    pub fn total(&self) -> usize {
        self.structure
            + self.core
            + self.boundary
            + self.warning
            + self.trail
            + self.impact
            + self.decoration
            + self.socket
            + self.archive
    }

    pub(crate) fn increment(&mut self, role: FangyuanPrimitiveRole) {
        match role {
            FangyuanPrimitiveRole::Structure => self.structure += 1,
            FangyuanPrimitiveRole::Core => self.core += 1,
            FangyuanPrimitiveRole::Boundary => self.boundary += 1,
            FangyuanPrimitiveRole::Warning => self.warning += 1,
            FangyuanPrimitiveRole::Trail => self.trail += 1,
            FangyuanPrimitiveRole::Impact => self.impact += 1,
            FangyuanPrimitiveRole::Decoration => self.decoration += 1,
            FangyuanPrimitiveRole::Socket => self.socket += 1,
            FangyuanPrimitiveRole::Archive => self.archive += 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct FangyuanPrimitiveColorKey([u32; 4]);

impl FangyuanPrimitiveColorKey {
    fn from_color(color: Color) -> Self {
        let color = color.to_srgba();
        Self([
            canonical_f32_bits(color.red),
            canonical_f32_bits(color.green),
            canonical_f32_bits(color.blue),
            canonical_f32_bits(color.alpha),
        ])
    }
}

fn canonical_f32_bits(value: f32) -> u32 {
    if value == 0.0 {
        0.0f32.to_bits()
    } else {
        value.to_bits()
    }
}

#[cfg(test)]
mod tests {
    use bevy::prelude::*;

    use super::*;
    use crate::framework::fangyuan::load_fangyuan_minimal_player_primitive_set;

    #[test]
    fn stats_cover_minimal_player_primitive_set() {
        let primitive_set = load_fangyuan_minimal_player_primitive_set().unwrap();

        let stats = primitive_set.stats();

        assert_eq!(stats.total, 2);
        assert_eq!(stats.cube_count, 1);
        assert_eq!(stats.sphere_count, 1);
        assert_eq!(stats.role_distribution.total(), 2);
        assert_eq!(
            stats
                .role_distribution
                .count(FangyuanPrimitiveRole::Structure),
            1
        );
        assert_eq!(
            stats.role_distribution.count(FangyuanPrimitiveRole::Core),
            1
        );
        assert_eq!(stats.color_count, 2);
        assert_eq!(stats.alpha_count, 0);
        assert_eq!(stats.opaque_count, 2);
        assert_eq!(stats.transparent_count, 0);
        assert_eq!(stats.emissive_count, 0);
        assert_eq!(stats.emissive_total, 0.0);
        assert_eq!(stats.material_profile_count, 0);
        assert_eq!(stats.unique_material_resource_count, 2);
    }

    #[test]
    fn stats_count_material_usage_from_primitive_data() {
        let primitive_set = FangyuanPrimitiveSet::from_primitives(vec![
            FangyuanPrimitive::with_runtime_metadata(
                FangyuanPrimitiveKind::Cube,
                Vec3::ZERO,
                Vec3::ONE,
                Color::srgba(1.0, 0.0, 0.0, 1.0),
                FangyuanPrimitiveRole::Structure,
                0.5,
                2.0,
                Some("glow".to_string()),
                Default::default(),
            ),
            FangyuanPrimitive::with_runtime_metadata(
                FangyuanPrimitiveKind::Sphere,
                Vec3::Y,
                Vec3::ONE,
                Color::srgba(1.0, 0.0, 0.0, 1.0),
                FangyuanPrimitiveRole::Decoration,
                1.0,
                FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE,
                Some("glow".to_string()),
                Default::default(),
            ),
            FangyuanPrimitive::with_runtime_metadata(
                FangyuanPrimitiveKind::Sphere,
                Vec3::NEG_Y,
                Vec3::ONE,
                Color::srgba(0.0, 1.0, 0.0, 0.75),
                FangyuanPrimitiveRole::Core,
                0.75,
                1.0,
                Some("transparent".to_string()),
                Default::default(),
            ),
        ]);

        let stats = FangyuanPrimitiveSetStats::from_primitive_set(&primitive_set);

        assert_eq!(stats.total, 3);
        assert_eq!(stats.cube_count, 1);
        assert_eq!(stats.sphere_count, 2);
        assert_eq!(stats.color_count, 2);
        assert_eq!(stats.alpha_count, 2);
        assert_eq!(stats.opaque_count, 1);
        assert_eq!(stats.transparent_count, 2);
        assert_eq!(stats.emissive_count, 2);
        assert_eq!(stats.emissive_total, 3.0);
        assert_eq!(stats.material_profile_count, 2);
        assert_eq!(stats.unique_material_resource_count, 3);
        assert_eq!(
            stats
                .role_distribution
                .count(FangyuanPrimitiveRole::Decoration),
            1
        );
    }
}
