use bevy::{
    asset::RenderAssetUsages, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology,
};

use super::{
    FANGYUAN_STATIC_MERGE_CUBE_INDEX_COUNT, FANGYUAN_STATIC_MERGE_CUBE_VERTEX_COUNT,
    FANGYUAN_STATIC_MERGE_SPHERE_SECTORS, FANGYUAN_STATIC_MERGE_SPHERE_STACKS, FangyuanPrimitive,
    FangyuanPrimitiveKind, FangyuanPrimitiveSet, FangyuanStaticMergeBuildOptions,
    FangyuanStaticMergeColorKey, FangyuanStaticMergeF32Key, FangyuanStaticMergeGroupKey,
    FangyuanStaticMergeInput, FangyuanStaticMergeSourceRef, FangyuanStaticMergeTransparentPath,
    fangyuan_static_merge_groups_from_inputs_with_options,
};

pub const FANGYUAN_STATIC_MERGE_DEFAULT_MAX_VERTICES_PER_MESH: usize = 60_000;
pub const FANGYUAN_STATIC_MERGE_DEFAULT_MAX_INDICES_PER_MESH: usize = 180_000;

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanStaticMeshBuildOptions {
    pub group_options: FangyuanStaticMergeBuildOptions,
    pub sphere_sectors: usize,
    pub sphere_stacks: usize,
    pub max_vertices_per_mesh: usize,
    pub max_indices_per_mesh: usize,
}

impl Default for FangyuanStaticMeshBuildOptions {
    fn default() -> Self {
        Self {
            group_options: FangyuanStaticMergeBuildOptions::default(),
            sphere_sectors: FANGYUAN_STATIC_MERGE_SPHERE_SECTORS,
            sphere_stacks: FANGYUAN_STATIC_MERGE_SPHERE_STACKS,
            max_vertices_per_mesh: FANGYUAN_STATIC_MERGE_DEFAULT_MAX_VERTICES_PER_MESH,
            max_indices_per_mesh: FANGYUAN_STATIC_MERGE_DEFAULT_MAX_INDICES_PER_MESH,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanStaticMeshBuildReport {
    pub meshes: Vec<FangyuanStaticMeshGroupMesh>,
    pub skipped: Vec<super::FangyuanStaticMergeSkippedPrimitive>,
    pub stats: FangyuanStaticMeshBuildStats,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanStaticMeshGroupMesh {
    pub key: FangyuanStaticMergeGroupKey,
    pub mesh: Mesh,
    pub material: FangyuanStaticMeshMaterial,
    pub metadata: FangyuanStaticMeshMetadata,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FangyuanStaticMeshMaterial {
    pub color: Color,
    pub alpha: f32,
    pub emissive: f32,
    pub transparent_path: FangyuanStaticMergeTransparentPath,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanStaticMeshMetadata {
    pub bounds: FangyuanStaticMeshBounds,
    pub source_ranges: Vec<FangyuanStaticMeshSourceRange>,
    pub primitive_count: usize,
    pub debug_name: String,
    pub vertex_count: usize,
    pub index_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FangyuanStaticMeshBounds {
    pub min: Vec3,
    pub max: Vec3,
}

impl FangyuanStaticMeshBounds {
    pub fn empty() -> Self {
        Self {
            min: Vec3::splat(f32::INFINITY),
            max: Vec3::splat(f32::NEG_INFINITY),
        }
    }

    pub fn include_point(&mut self, point: Vec3) {
        self.min = self.min.min(point);
        self.max = self.max.max(point);
    }

    pub fn size(&self) -> Vec3 {
        if self.is_empty() {
            Vec3::ZERO
        } else {
            self.max - self.min
        }
    }

    pub fn is_empty(&self) -> bool {
        self.min.x > self.max.x || self.min.y > self.max.y || self.min.z > self.max.z
    }
}

impl Default for FangyuanStaticMeshBounds {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanStaticMeshSourceRange {
    pub source: FangyuanStaticMergeSourceRef,
    pub primitive_index: usize,
    pub vertex_start: usize,
    pub vertex_count: usize,
    pub index_start: usize,
    pub index_count: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FangyuanStaticMeshBuildStats {
    pub authored_primitives: usize,
    pub expanded_primitives: usize,
    pub mesh_count: usize,
    pub merged_primitive_count: usize,
    pub skipped_primitives: usize,
    pub cube_count: usize,
    pub sphere_count: usize,
    pub vertex_count: usize,
    pub index_count: usize,
    pub fallback_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FangyuanStaticMeshBuildError {
    EmptyInput,
    InvalidOptions {
        reason: String,
    },
    BudgetExceeded {
        debug_name: String,
        primitive_count: usize,
        vertex_count: usize,
        index_count: usize,
        max_vertices: usize,
        max_indices: usize,
    },
    IndexOverflow {
        debug_name: String,
        vertex_count: usize,
    },
}

impl std::fmt::Display for FangyuanStaticMeshBuildError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyInput => write!(formatter, "fangyuan static mesh build input is empty"),
            Self::InvalidOptions { reason } => {
                write!(
                    formatter,
                    "invalid fangyuan static mesh build options: {reason}"
                )
            }
            Self::BudgetExceeded {
                debug_name,
                primitive_count,
                vertex_count,
                index_count,
                max_vertices,
                max_indices,
            } => write!(
                formatter,
                "fangyuan static mesh `{debug_name}` exceeds budget: primitives={primitive_count}, vertices={vertex_count}/{max_vertices}, indices={index_count}/{max_indices}"
            ),
            Self::IndexOverflow {
                debug_name,
                vertex_count,
            } => write!(
                formatter,
                "fangyuan static mesh `{debug_name}` has too many vertices for u32 indices: vertices={vertex_count}"
            ),
        }
    }
}

impl std::error::Error for FangyuanStaticMeshBuildError {}

pub fn fangyuan_static_meshes_from_primitive_set(
    primitive_set: &FangyuanPrimitiveSet,
) -> Result<FangyuanStaticMeshBuildReport, FangyuanStaticMeshBuildError> {
    fangyuan_static_meshes_from_primitive_set_with_source(
        primitive_set,
        None::<String>,
        &FangyuanStaticMeshBuildOptions::default(),
    )
}

pub fn fangyuan_static_meshes_from_primitive_set_with_source(
    primitive_set: &FangyuanPrimitiveSet,
    source_path: impl Into<Option<String>>,
    options: &FangyuanStaticMeshBuildOptions,
) -> Result<FangyuanStaticMeshBuildReport, FangyuanStaticMeshBuildError> {
    let source_path = source_path.into();
    let inputs = primitive_set.primitives().iter().cloned().enumerate().map(
        |(primitive_index, primitive)| {
            FangyuanStaticMergeInput::new(
                primitive,
                FangyuanStaticMergeSourceRef::runtime_primitive_set(
                    source_path.clone(),
                    primitive_index,
                ),
            )
        },
    );

    fangyuan_static_meshes_from_inputs_with_options(inputs, options)
}

pub fn fangyuan_static_meshes_from_inputs_with_options(
    inputs: impl IntoIterator<Item = FangyuanStaticMergeInput>,
    options: &FangyuanStaticMeshBuildOptions,
) -> Result<FangyuanStaticMeshBuildReport, FangyuanStaticMeshBuildError> {
    validate_options(options)?;

    let inputs = inputs.into_iter().collect::<Vec<_>>();
    if inputs.is_empty() {
        return Ok(FangyuanStaticMeshBuildReport {
            meshes: Vec::new(),
            skipped: Vec::new(),
            stats: FangyuanStaticMeshBuildStats::default(),
        });
    }

    let grouped_report = fangyuan_static_merge_groups_from_inputs_with_options(
        inputs.clone(),
        &options.group_options,
    );
    let mut meshes = Vec::with_capacity(grouped_report.groups.len());
    let mut stats = FangyuanStaticMeshBuildStats {
        authored_primitives: grouped_report.stats.authored_primitives,
        expanded_primitives: grouped_report.stats.expanded_primitives,
        skipped_primitives: grouped_report.stats.skipped_primitives,
        cube_count: grouped_report.stats.cube_count,
        sphere_count: grouped_report.stats.sphere_count,
        ..Default::default()
    };

    for group in grouped_report.groups {
        let group_sources = group.source_refs.clone();
        let group_inputs = inputs
            .iter()
            .filter(|input| group_sources.contains(&input.source))
            .cloned()
            .collect::<Vec<_>>();
        let mesh = build_group_mesh(group.key, group_inputs, options)?;
        stats.mesh_count += 1;
        stats.merged_primitive_count += mesh.metadata.primitive_count;
        stats.vertex_count += mesh.metadata.vertex_count;
        stats.index_count += mesh.metadata.index_count;
        meshes.push(mesh);
    }

    Ok(FangyuanStaticMeshBuildReport {
        meshes,
        skipped: grouped_report.skipped,
        stats,
    })
}

fn build_group_mesh(
    key: FangyuanStaticMergeGroupKey,
    mut inputs: Vec<FangyuanStaticMergeInput>,
    options: &FangyuanStaticMeshBuildOptions,
) -> Result<FangyuanStaticMeshGroupMesh, FangyuanStaticMeshBuildError> {
    inputs.sort_by(|left, right| left.source.cmp(&right.source));
    let primitive_count = inputs.len();
    let vertex_count = inputs
        .iter()
        .map(|input| vertex_count_for_kind(input.primitive.kind, options))
        .sum::<usize>();
    let index_count = inputs
        .iter()
        .map(|input| index_count_for_kind(input.primitive.kind, options))
        .sum::<usize>();
    let debug_name = static_mesh_debug_name(&key, primitive_count);

    if vertex_count > options.max_vertices_per_mesh || index_count > options.max_indices_per_mesh {
        return Err(FangyuanStaticMeshBuildError::BudgetExceeded {
            debug_name,
            primitive_count,
            vertex_count,
            index_count,
            max_vertices: options.max_vertices_per_mesh,
            max_indices: options.max_indices_per_mesh,
        });
    }
    if vertex_count > u32::MAX as usize {
        return Err(FangyuanStaticMeshBuildError::IndexOverflow {
            debug_name,
            vertex_count,
        });
    }

    let mut builder = FangyuanStaticMeshVertexBuilder::with_capacity(vertex_count, index_count);
    for (primitive_index, input) in inputs.iter().enumerate() {
        let vertex_start = builder.positions.len();
        let index_start = builder.indices.len();
        match input.primitive.kind {
            FangyuanPrimitiveKind::Cube => builder.push_cube(&input.primitive),
            FangyuanPrimitiveKind::Sphere => builder.push_sphere(
                &input.primitive,
                options.sphere_sectors,
                options.sphere_stacks,
            ),
        }
        builder.source_ranges.push(FangyuanStaticMeshSourceRange {
            source: input.source.clone(),
            primitive_index,
            vertex_start,
            vertex_count: builder.positions.len() - vertex_start,
            index_start,
            index_count: builder.indices.len() - index_start,
        });
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, builder.positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, builder.normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, builder.uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, builder.colors);
    mesh.insert_indices(Indices::U32(builder.indices));

    let metadata = FangyuanStaticMeshMetadata {
        bounds: builder.bounds,
        source_ranges: builder.source_ranges,
        primitive_count,
        debug_name: static_mesh_debug_name(&key, primitive_count),
        vertex_count,
        index_count,
    };
    let material = FangyuanStaticMeshMaterial::from_group_key(&key);

    Ok(FangyuanStaticMeshGroupMesh {
        key,
        mesh,
        material,
        metadata,
    })
}

impl FangyuanStaticMeshMaterial {
    pub fn from_group_key(key: &FangyuanStaticMergeGroupKey) -> Self {
        let color = color_from_key(key.color);
        Self {
            color,
            alpha: color.to_srgba().alpha,
            emissive: f32_from_key(key.emissive),
            transparent_path: key.transparent_path,
        }
    }
}

struct FangyuanStaticMeshVertexBuilder {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
    bounds: FangyuanStaticMeshBounds,
    source_ranges: Vec<FangyuanStaticMeshSourceRange>,
}

impl FangyuanStaticMeshVertexBuilder {
    fn with_capacity(vertex_count: usize, index_count: usize) -> Self {
        Self {
            positions: Vec::with_capacity(vertex_count),
            normals: Vec::with_capacity(vertex_count),
            uvs: Vec::with_capacity(vertex_count),
            colors: Vec::with_capacity(vertex_count),
            indices: Vec::with_capacity(index_count),
            bounds: FangyuanStaticMeshBounds::empty(),
            source_ranges: Vec::new(),
        }
    }

    fn push_cube(&mut self, primitive: &FangyuanPrimitive) {
        let half = primitive.scale * 0.5;
        let min = primitive.local_position - half;
        let max = primitive.local_position + half;
        let color = primitive_color(primitive);

        let faces = [
            (
                Vec3::X,
                [
                    Vec3::new(max.x, min.y, max.z),
                    Vec3::new(max.x, min.y, min.z),
                    Vec3::new(max.x, max.y, min.z),
                    Vec3::new(max.x, max.y, max.z),
                ],
            ),
            (
                Vec3::NEG_X,
                [
                    Vec3::new(min.x, min.y, min.z),
                    Vec3::new(min.x, min.y, max.z),
                    Vec3::new(min.x, max.y, max.z),
                    Vec3::new(min.x, max.y, min.z),
                ],
            ),
            (
                Vec3::Y,
                [
                    Vec3::new(min.x, max.y, max.z),
                    Vec3::new(max.x, max.y, max.z),
                    Vec3::new(max.x, max.y, min.z),
                    Vec3::new(min.x, max.y, min.z),
                ],
            ),
            (
                Vec3::NEG_Y,
                [
                    Vec3::new(min.x, min.y, min.z),
                    Vec3::new(max.x, min.y, min.z),
                    Vec3::new(max.x, min.y, max.z),
                    Vec3::new(min.x, min.y, max.z),
                ],
            ),
            (
                Vec3::Z,
                [
                    Vec3::new(min.x, min.y, max.z),
                    Vec3::new(max.x, min.y, max.z),
                    Vec3::new(max.x, max.y, max.z),
                    Vec3::new(min.x, max.y, max.z),
                ],
            ),
            (
                Vec3::NEG_Z,
                [
                    Vec3::new(max.x, min.y, min.z),
                    Vec3::new(min.x, min.y, min.z),
                    Vec3::new(min.x, max.y, min.z),
                    Vec3::new(max.x, max.y, min.z),
                ],
            ),
        ];

        for (normal, corners) in faces {
            self.push_quad(corners, normal, color);
        }
    }

    fn push_sphere(&mut self, primitive: &FangyuanPrimitive, sectors: usize, stacks: usize) {
        let color = primitive_color(primitive);
        let base_index = self.positions.len() as u32;
        let inv_sector = 1.0 / sectors as f32;
        let inv_stack = 1.0 / stacks as f32;

        for stack in 0..=stacks {
            let v = stack as f32 * inv_stack;
            let phi = std::f32::consts::FRAC_PI_2 - v * std::f32::consts::PI;
            let xy = phi.cos();
            let y = phi.sin();

            for sector in 0..=sectors {
                let u = sector as f32 * inv_sector;
                let theta = u * std::f32::consts::TAU;
                let unit = Vec3::new(xy * theta.cos(), y, xy * theta.sin());
                let position = primitive.local_position + unit * primitive.scale * 0.5;
                self.push_vertex(position, unit.normalize_or_zero(), [u, v], color);
            }
        }

        let row = sectors + 1;
        for stack in 0..stacks {
            for sector in 0..sectors {
                let first = base_index + (stack * row + sector) as u32;
                let second = first + row as u32;
                self.indices.extend_from_slice(&[
                    first,
                    second,
                    first + 1,
                    first + 1,
                    second,
                    second + 1,
                ]);
            }
        }
    }

    fn push_quad(&mut self, corners: [Vec3; 4], normal: Vec3, color: [f32; 4]) {
        let base_index = self.positions.len() as u32;
        let uvs = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
        for (corner, uv) in corners.into_iter().zip(uvs) {
            self.push_vertex(corner, normal, uv, color);
        }
        self.indices.extend_from_slice(&[
            base_index,
            base_index + 1,
            base_index + 2,
            base_index,
            base_index + 2,
            base_index + 3,
        ]);
    }

    fn push_vertex(&mut self, position: Vec3, normal: Vec3, uv: [f32; 2], color: [f32; 4]) {
        self.positions.push(position.to_array());
        self.normals.push(normal.to_array());
        self.uvs.push(uv);
        self.colors.push(color);
        self.bounds.include_point(position);
    }
}

fn validate_options(
    options: &FangyuanStaticMeshBuildOptions,
) -> Result<(), FangyuanStaticMeshBuildError> {
    if options.sphere_sectors < 3 {
        return Err(FangyuanStaticMeshBuildError::InvalidOptions {
            reason: "sphere_sectors must be at least 3".to_string(),
        });
    }
    if options.sphere_stacks < 2 {
        return Err(FangyuanStaticMeshBuildError::InvalidOptions {
            reason: "sphere_stacks must be at least 2".to_string(),
        });
    }
    if options.max_vertices_per_mesh == 0 || options.max_indices_per_mesh == 0 {
        return Err(FangyuanStaticMeshBuildError::InvalidOptions {
            reason: "mesh vertex and index budgets must be positive".to_string(),
        });
    }

    Ok(())
}

fn vertex_count_for_kind(
    kind: FangyuanPrimitiveKind,
    options: &FangyuanStaticMeshBuildOptions,
) -> usize {
    match kind {
        FangyuanPrimitiveKind::Cube => FANGYUAN_STATIC_MERGE_CUBE_VERTEX_COUNT,
        FangyuanPrimitiveKind::Sphere => (options.sphere_sectors + 1) * (options.sphere_stacks + 1),
    }
}

fn index_count_for_kind(
    kind: FangyuanPrimitiveKind,
    options: &FangyuanStaticMeshBuildOptions,
) -> usize {
    match kind {
        FangyuanPrimitiveKind::Cube => FANGYUAN_STATIC_MERGE_CUBE_INDEX_COUNT,
        FangyuanPrimitiveKind::Sphere => options.sphere_sectors * options.sphere_stacks * 6,
    }
}

fn static_mesh_debug_name(key: &FangyuanStaticMergeGroupKey, primitive_count: usize) -> String {
    format!(
        "{}:{}:{}:{}:n{}",
        key.debug_label,
        key.region_placeholder,
        key.primitive_kind.as_str(),
        key.material_profile,
        primitive_count
    )
}

fn primitive_color(primitive: &FangyuanPrimitive) -> [f32; 4] {
    let color = primitive.color.to_srgba();
    [color.red, color.green, color.blue, primitive.alpha]
}

fn color_from_key(key: FangyuanStaticMergeColorKey) -> Color {
    let channels = key.channels();
    Color::srgba(channels[0], channels[1], channels[2], channels[3])
}

fn f32_from_key(key: FangyuanStaticMergeF32Key) -> f32 {
    key.to_f32()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::fangyuan::{
        FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE, FANGYUAN_STATIC_MERGE_DEFAULT_MAX_VERTICES_PER_MESH,
        FANGYUAN_STATIC_MERGE_SPHERE_INDEX_COUNT, FANGYUAN_STATIC_MERGE_SPHERE_VERTEX_COUNT,
        FangyuanPrimitiveLifecycle, FangyuanPrimitiveRole,
    };
    use bevy::mesh::{MeshVertexAttribute, VertexAttributeValues};

    #[test]
    fn fangyuan_static_merge_cpu_builder_merges_cube_vertices_normals_uvs_and_colors() {
        let primitive = static_primitive(
            FangyuanPrimitiveKind::Cube,
            Vec3::new(1.0, 2.0, -1.0),
            Vec3::new(2.0, 4.0, 6.0),
            [0.25, 0.5, 0.75, 1.0],
        );
        let primitive_set = FangyuanPrimitiveSet::from_primitives(vec![primitive]);

        let report = fangyuan_static_meshes_from_primitive_set(&primitive_set).unwrap();

        assert_eq!(report.meshes.len(), 1);
        assert_eq!(report.stats.mesh_count, 1);
        assert_eq!(report.stats.merged_primitive_count, 1);
        assert_eq!(
            report.stats.vertex_count,
            FANGYUAN_STATIC_MERGE_CUBE_VERTEX_COUNT
        );
        assert_eq!(
            report.stats.index_count,
            FANGYUAN_STATIC_MERGE_CUBE_INDEX_COUNT
        );
        let group = &report.meshes[0];
        assert_eq!(group.metadata.primitive_count, 1);
        assert_eq!(
            group.metadata.vertex_count,
            FANGYUAN_STATIC_MERGE_CUBE_VERTEX_COUNT
        );
        assert_eq!(
            group.metadata.index_count,
            FANGYUAN_STATIC_MERGE_CUBE_INDEX_COUNT
        );
        assert_eq!(group.metadata.bounds.min, Vec3::new(0.0, 0.0, -4.0));
        assert_eq!(group.metadata.bounds.max, Vec3::new(2.0, 4.0, 2.0));
        assert_eq!(group.metadata.bounds.size(), Vec3::new(2.0, 4.0, 6.0));
        assert_eq!(group.metadata.source_ranges.len(), 1);
        assert_eq!(group.metadata.source_ranges[0].vertex_start, 0);
        assert_eq!(group.metadata.source_ranges[0].vertex_count, 24);
        assert_eq!(group.metadata.source_ranges[0].index_count, 36);
        assert!(group.metadata.debug_name.contains("fangyuan_static"));
        assert!(group.metadata.debug_name.contains("cube"));

        assert_attribute_len(&group.mesh, Mesh::ATTRIBUTE_POSITION, 24);
        assert_attribute_len(&group.mesh, Mesh::ATTRIBUTE_NORMAL, 24);
        assert_attribute_len(&group.mesh, Mesh::ATTRIBUTE_UV_0, 24);
        assert_attribute_len(&group.mesh, Mesh::ATTRIBUTE_COLOR, 24);
        assert_eq!(group.mesh.indices().unwrap().len(), 36);
        let Some(VertexAttributeValues::Float32x4(colors)) =
            group.mesh.attribute(Mesh::ATTRIBUTE_COLOR)
        else {
            panic!("cube mesh should have f32x4 color attribute");
        };
        assert!(colors.iter().all(|color| *color == [0.25, 0.5, 0.75, 1.0]));
    }

    #[test]
    fn fangyuan_static_merge_cpu_builder_uses_low_cost_sphere_segments_and_budget() {
        let primitive = static_primitive(
            FangyuanPrimitiveKind::Sphere,
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::splat(2.0),
            [0.8, 0.4, 0.2, 1.0],
        );
        let primitive_set = FangyuanPrimitiveSet::from_primitives(vec![primitive]);

        let report = fangyuan_static_meshes_from_primitive_set(&primitive_set).unwrap();

        assert_eq!(report.meshes.len(), 1);
        assert_eq!(
            report.stats.vertex_count,
            FANGYUAN_STATIC_MERGE_SPHERE_VERTEX_COUNT
        );
        assert_eq!(
            report.stats.index_count,
            FANGYUAN_STATIC_MERGE_SPHERE_INDEX_COUNT
        );
        assert_eq!(
            report.meshes[0].metadata.bounds.min,
            Vec3::new(-1.0, 0.0, -1.0)
        );
        assert_eq!(
            report.meshes[0].metadata.bounds.max,
            Vec3::new(1.0, 2.0, 1.0)
        );
        assert_attribute_len(
            &report.meshes[0].mesh,
            Mesh::ATTRIBUTE_POSITION,
            FANGYUAN_STATIC_MERGE_SPHERE_VERTEX_COUNT,
        );
        assert_eq!(
            report.meshes[0].mesh.indices().unwrap().len(),
            FANGYUAN_STATIC_MERGE_SPHERE_INDEX_COUNT
        );
        assert!(
            FANGYUAN_STATIC_MERGE_SPHERE_VERTEX_COUNT
                <= FANGYUAN_STATIC_MERGE_DEFAULT_MAX_VERTICES_PER_MESH
        );
    }

    #[test]
    fn fangyuan_static_merge_cpu_builder_rejects_group_over_budget() {
        let primitive_set = FangyuanPrimitiveSet::from_primitives(vec![static_primitive(
            FangyuanPrimitiveKind::Sphere,
            Vec3::Y,
            Vec3::ONE,
            [1.0, 1.0, 1.0, 1.0],
        )]);
        let options = FangyuanStaticMeshBuildOptions {
            max_vertices_per_mesh: FANGYUAN_STATIC_MERGE_SPHERE_VERTEX_COUNT - 1,
            ..Default::default()
        };

        let error =
            fangyuan_static_meshes_from_primitive_set_with_source(&primitive_set, None, &options)
                .unwrap_err();

        assert!(matches!(
            error,
            FangyuanStaticMeshBuildError::BudgetExceeded { .. }
        ));
    }

    #[test]
    fn fangyuan_static_merge_cpu_builder_preserves_source_ranges_for_grouped_primitives() {
        let inputs = vec![
            FangyuanStaticMergeInput::new(
                static_primitive(
                    FangyuanPrimitiveKind::Cube,
                    Vec3::ZERO,
                    Vec3::ONE,
                    [0.2, 0.3, 0.4, 1.0],
                ),
                FangyuanStaticMergeSourceRef::runtime_primitive_set(
                    Some("test.ron".to_string()),
                    0,
                ),
            ),
            FangyuanStaticMergeInput::new(
                static_primitive(
                    FangyuanPrimitiveKind::Cube,
                    Vec3::new(2.0, 0.0, 0.0),
                    Vec3::ONE,
                    [0.2, 0.3, 0.4, 1.0],
                ),
                FangyuanStaticMergeSourceRef::runtime_primitive_set(
                    Some("test.ron".to_string()),
                    1,
                ),
            ),
        ];

        let report =
            fangyuan_static_meshes_from_inputs_with_options(inputs, &Default::default()).unwrap();

        assert_eq!(report.meshes.len(), 1);
        let ranges = &report.meshes[0].metadata.source_ranges;
        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges[0].source.primitive_index, 0);
        assert_eq!(ranges[0].vertex_start, 0);
        assert_eq!(ranges[0].vertex_count, 24);
        assert_eq!(ranges[0].index_start, 0);
        assert_eq!(ranges[0].index_count, 36);
        assert_eq!(ranges[1].source.primitive_index, 1);
        assert_eq!(ranges[1].vertex_start, 24);
        assert_eq!(ranges[1].index_start, 36);
        assert_eq!(report.meshes[0].metadata.primitive_count, 2);
    }

    fn assert_attribute_len(mesh: &Mesh, attribute: MeshVertexAttribute, expected: usize) {
        let len = match mesh.attribute(attribute).unwrap() {
            VertexAttributeValues::Float32x2(values) => values.len(),
            VertexAttributeValues::Float32x3(values) => values.len(),
            VertexAttributeValues::Float32x4(values) => values.len(),
            other => panic!("unexpected attribute values: {other:?}"),
        };
        assert_eq!(len, expected);
    }

    fn static_primitive(
        kind: FangyuanPrimitiveKind,
        position: Vec3,
        scale: Vec3,
        color: [f32; 4],
    ) -> FangyuanPrimitive {
        FangyuanPrimitive::with_runtime_metadata(
            kind,
            position,
            scale,
            Color::srgba(color[0], color[1], color[2], color[3]),
            FangyuanPrimitiveRole::default_for_kind(kind),
            color[3],
            FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE,
            None,
            FangyuanPrimitiveLifecycle::empty(),
        )
    }
}
