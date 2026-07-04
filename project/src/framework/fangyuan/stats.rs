use bevy::prelude::Color;
use std::collections::{BTreeSet, HashSet};

use super::{
    FANGYUAN_STATIC_INSTANCE_RENDER_STRIDE_BYTES, FangyuanMaterialProfileRegistry,
    FangyuanRenderMaterialKey,
    primitive::{
        FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE, FangyuanPrimitive, FangyuanPrimitiveKind,
        FangyuanPrimitiveRole, FangyuanPrimitiveSet,
    },
    static_instance_render::FangyuanStaticInstanceRenderStats,
    static_merge::{
        FangyuanStaticMergeBuildReport, FangyuanStaticMergeStats,
        FangyuanStaticMergeTransparentPath,
    },
    static_mesh_builder::FangyuanStaticMeshBuildStats,
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FangyuanRenderScaleReport {
    pub standard: FangyuanStandardRenderScaleStats,
    pub cpu_merge: FangyuanCpuMergeRenderScaleStats,
    pub static_instance: FangyuanStaticInstanceRenderScaleStats,
    pub pressure: FangyuanRenderScalePressureSummary,
}

impl FangyuanRenderScaleReport {
    pub fn from_reports(
        primitive_stats: &FangyuanPrimitiveSetStats,
        merge_report: &FangyuanStaticMergeBuildReport,
        mesh_stats: Option<&FangyuanStaticMeshBuildStats>,
        instance_stats: &FangyuanStaticInstanceRenderStats,
    ) -> Self {
        let mut static_instance =
            FangyuanStaticInstanceRenderScaleStats::from_render_stats(instance_stats);
        static_instance.material_count = primitive_stats.unique_material_resource_count;

        let standard = FangyuanStandardRenderScaleStats::from_primitive_stats(primitive_stats);
        let cpu_merge =
            FangyuanCpuMergeRenderScaleStats::from_merge_report(merge_report, mesh_stats);
        let pressure = FangyuanRenderScalePressureSummary::from_paths(
            standard.entity_count,
            cpu_merge.entity_count,
            static_instance.batch_count,
            static_instance.buffer_bytes,
        );

        Self {
            standard,
            cpu_merge,
            static_instance,
            pressure,
        }
    }

    pub fn format_summary(&self) -> String {
        format!(
            "standard: primitives={}, entities={}, meshes={}, batches={}, materials={}, profiles={}, opaque={}, transparent={}, emissive={}, pressure_units={}; cpu_merge: primitives={}, entities={}, meshes={}, batches={}, materials={}, profiles={}, opaque_batches={}, transparent_batches={}, vertices={}, indices={}, fallback={}, pressure_units={}; static_instance: instances={}, entities={}, meshes={}, batches={}, buffers={}, buffer_bytes={}, stride_bytes={}, materials={}, profiles={}, cubes={}, spheres={}, hash={}, pressure_units={}; trend: standard_to_cpu_merge_reduction={}x, standard_to_instance_reduction={}x, instance_buffer_kib={}, limiting_path={}",
            self.standard.primitive_count,
            self.standard.entity_count,
            self.standard.mesh_count,
            self.standard.batch_count,
            self.standard.material_count,
            self.standard.material_profile_count,
            self.standard.opaque_count,
            self.standard.transparent_count,
            self.standard.emissive_count,
            self.pressure.standard_pressure_units,
            self.cpu_merge.primitive_count,
            self.cpu_merge.entity_count,
            self.cpu_merge.mesh_count,
            self.cpu_merge.batch_count,
            self.cpu_merge.material_count,
            self.cpu_merge.material_profile_count,
            self.cpu_merge.opaque_batch_count,
            self.cpu_merge.transparent_batch_count,
            self.cpu_merge.vertex_count,
            self.cpu_merge.index_count,
            self.cpu_merge.fallback_count,
            self.pressure.cpu_merge_pressure_units,
            self.static_instance.instance_count,
            self.static_instance.entity_count,
            self.static_instance.mesh_count,
            self.static_instance.batch_count,
            self.static_instance.buffer_count,
            self.static_instance.buffer_bytes,
            FANGYUAN_STATIC_INSTANCE_RENDER_STRIDE_BYTES,
            self.static_instance.material_count,
            self.static_instance.material_profile_count,
            self.static_instance.cube_count,
            self.static_instance.sphere_count,
            self.static_instance.content_hash,
            self.pressure.static_instance_pressure_units,
            self.pressure.standard_to_cpu_merge_reduction,
            self.pressure.standard_to_instance_reduction,
            self.pressure.static_instance_buffer_kib,
            self.pressure.limiting_path,
        )
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FangyuanRenderScalePressureSummary {
    pub standard_pressure_units: usize,
    pub cpu_merge_pressure_units: usize,
    pub static_instance_pressure_units: usize,
    pub standard_to_cpu_merge_reduction: usize,
    pub standard_to_instance_reduction: usize,
    pub static_instance_buffer_kib: usize,
    pub limiting_path: &'static str,
}

impl FangyuanRenderScalePressureSummary {
    pub fn from_paths(
        standard_entities: usize,
        cpu_merge_batches: usize,
        static_instance_batches: usize,
        static_instance_buffer_bytes: usize,
    ) -> Self {
        let standard_pressure_units = standard_entities;
        let cpu_merge_pressure_units = cpu_merge_batches;
        let static_instance_pressure_units = static_instance_batches;
        let limiting_path = if static_instance_buffer_bytes > 0 {
            "static_instance_buffer_bytes"
        } else if cpu_merge_pressure_units > static_instance_pressure_units {
            "cpu_merge_batches"
        } else {
            "standard_entities"
        };

        Self {
            standard_pressure_units,
            cpu_merge_pressure_units,
            static_instance_pressure_units,
            standard_to_cpu_merge_reduction: stable_reduction_ratio(
                standard_pressure_units,
                cpu_merge_pressure_units,
            ),
            standard_to_instance_reduction: stable_reduction_ratio(
                standard_pressure_units,
                static_instance_pressure_units,
            ),
            static_instance_buffer_kib: static_instance_buffer_bytes.div_ceil(1024),
            limiting_path,
        }
    }
}

fn stable_reduction_ratio(before: usize, after: usize) -> usize {
    if before == 0 {
        return 1;
    }
    before / after.max(1)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FangyuanStandardRenderScaleStats {
    pub primitive_count: usize,
    pub entity_count: usize,
    pub mesh_count: usize,
    pub batch_count: usize,
    pub material_count: usize,
    pub material_profile_count: usize,
    pub opaque_count: usize,
    pub transparent_count: usize,
    pub emissive_count: usize,
}

impl FangyuanStandardRenderScaleStats {
    pub fn from_primitive_stats(stats: &FangyuanPrimitiveSetStats) -> Self {
        Self {
            primitive_count: stats.total,
            entity_count: stats.total,
            mesh_count: stats.total,
            batch_count: stats.unique_material_resource_count,
            material_count: stats.unique_material_resource_count,
            material_profile_count: stats.material_profile_count,
            opaque_count: stats.opaque_count,
            transparent_count: stats.transparent_count,
            emissive_count: stats.emissive_count,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FangyuanCpuMergeRenderScaleStats {
    pub primitive_count: usize,
    pub entity_count: usize,
    pub mesh_count: usize,
    pub batch_count: usize,
    pub material_count: usize,
    pub material_profile_count: usize,
    pub opaque_batch_count: usize,
    pub transparent_batch_count: usize,
    pub vertex_count: usize,
    pub index_count: usize,
    pub fallback_count: usize,
}

impl FangyuanCpuMergeRenderScaleStats {
    pub fn from_merge_report(
        report: &FangyuanStaticMergeBuildReport,
        mesh_stats: Option<&FangyuanStaticMeshBuildStats>,
    ) -> Self {
        let opaque_batch_count = report
            .groups
            .iter()
            .filter(|group| {
                group.key.transparent_path == FangyuanStaticMergeTransparentPath::Opaque
            })
            .count();
        let transparent_batch_count = report
            .groups
            .iter()
            .filter(|group| {
                group.key.transparent_path == FangyuanStaticMergeTransparentPath::Transparent
            })
            .count();
        let stats = mesh_stats
            .map(FangyuanCpuMergeRenderScaleStats::from_mesh_stats)
            .unwrap_or_else(|| FangyuanCpuMergeRenderScaleStats::from_merge_stats(&report.stats));

        Self {
            material_count: cpu_merge_material_count(report),
            material_profile_count: report.stats.material_profile_count,
            opaque_batch_count,
            transparent_batch_count,
            ..stats
        }
    }

    fn from_merge_stats(stats: &FangyuanStaticMergeStats) -> Self {
        Self {
            primitive_count: stats.cube_count + stats.sphere_count,
            entity_count: stats.merged_group_count,
            mesh_count: stats.merged_group_count,
            batch_count: stats.merged_group_count,
            material_count: stats.merged_group_count,
            material_profile_count: stats.material_profile_count,
            vertex_count: stats.estimated_vertex_count,
            index_count: stats.estimated_index_count,
            fallback_count: 0,
            ..Default::default()
        }
    }

    fn from_mesh_stats(stats: &FangyuanStaticMeshBuildStats) -> Self {
        Self {
            primitive_count: stats.merged_primitive_count,
            entity_count: stats.mesh_count,
            mesh_count: stats.mesh_count,
            batch_count: stats.mesh_count,
            material_count: stats.mesh_count,
            material_profile_count: 0,
            vertex_count: stats.vertex_count,
            index_count: stats.index_count,
            fallback_count: stats.fallback_count,
            ..Default::default()
        }
    }
}

fn cpu_merge_material_count(report: &FangyuanStaticMergeBuildReport) -> usize {
    report
        .groups
        .iter()
        .map(|group| {
            (
                group.key.transparent_path,
                group.key.material_profile.clone(),
                group.key.color,
                group.key.emissive,
            )
        })
        .collect::<BTreeSet<_>>()
        .len()
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FangyuanStaticInstanceRenderScaleStats {
    pub instance_count: usize,
    pub entity_count: usize,
    pub mesh_count: usize,
    pub batch_count: usize,
    pub buffer_count: usize,
    pub buffer_bytes: usize,
    pub material_count: usize,
    pub material_profile_count: usize,
    pub cube_count: usize,
    pub sphere_count: usize,
    pub content_hash: u64,
}

impl FangyuanStaticInstanceRenderScaleStats {
    pub fn from_render_stats(stats: &FangyuanStaticInstanceRenderStats) -> Self {
        Self {
            instance_count: stats.instance_count,
            entity_count: stats.instance_count,
            mesh_count: stats.batch_count,
            batch_count: stats.batch_count,
            buffer_count: stats.batch_count,
            buffer_bytes: stats.buffer_bytes,
            material_count: stats.material_profile_count.max(stats.batch_count),
            material_profile_count: stats.material_profile_count,
            cube_count: stats.cube_count,
            sphere_count: stats.sphere_count,
            content_hash: stats.content_hash,
        }
    }
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

#[cfg(test)]
pub fn generate_fangyuan_large_static_primitive_set(count: usize) -> FangyuanPrimitiveSet {
    use super::FangyuanPrimitiveLifecycle;

    const COLORS: [[f32; 3]; 8] = [
        [0.22, 0.34, 0.48],
        [0.78, 0.50, 0.24],
        [0.32, 0.62, 0.42],
        [0.58, 0.38, 0.70],
        [0.70, 0.72, 0.42],
        [0.36, 0.58, 0.68],
        [0.64, 0.42, 0.36],
        [0.46, 0.48, 0.52],
    ];

    let mut primitives = Vec::with_capacity(count);
    for index in 0..count {
        let column = index % 125;
        let row = (index / 125) % 80;
        let layer = index / (125 * 80);
        let x = column as f32 * 0.42 - 26.0;
        let z = row as f32 * 0.42 - 16.0;
        let y = 0.45 + layer as f32 * 0.55;
        let kind = if index % 10 == 0 {
            FangyuanPrimitiveKind::Sphere
        } else {
            FangyuanPrimitiveKind::Cube
        };
        let role = match index % 5 {
            0 => FangyuanPrimitiveRole::Core,
            1 => FangyuanPrimitiveRole::Structure,
            2 => FangyuanPrimitiveRole::Boundary,
            3 => FangyuanPrimitiveRole::Decoration,
            _ => FangyuanPrimitiveRole::Archive,
        };
        let color = COLORS[index % COLORS.len()];
        let alpha = if index % 17 == 0 { 0.58 } else { 1.0 };
        let emissive = if index % 23 == 0 { 1.25 } else { 0.0 };
        let material_profile_id = if index % 7 == 0 {
            None
        } else {
            Some(format!("scale/profile_{}", index % 6))
        };

        primitives.push(FangyuanPrimitive::with_runtime_metadata(
            kind,
            bevy::prelude::Vec3::new(x, y, z),
            bevy::prelude::Vec3::new(
                0.24 + (index % 3) as f32 * 0.03,
                0.24 + (index % 5) as f32 * 0.02,
                0.24 + (index % 7) as f32 * 0.015,
            ),
            Color::srgba(color[0], color[1], color[2], alpha),
            role,
            alpha,
            emissive,
            material_profile_id,
            FangyuanPrimitiveLifecycle::empty(),
        ));
    }

    FangyuanPrimitiveSet::from_primitives(primitives)
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
