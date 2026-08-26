//! CPU-side atlas UV and primitive geometry contracts shared by reference and
//! custom material render paths.

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology, VertexAttributeValues},
    prelude::*,
};

pub const FANGYUAN_ATLAS_COLUMNS: u16 = 16;
pub const FANGYUAN_ATLAS_ROWS: u16 = 16;
pub const FANGYUAN_ATLAS_SIZE_PX: f32 = 1024.0;
pub const FANGYUAN_ATLAS_CELL_SIZE_PX: f32 = 64.0;
pub const FANGYUAN_ATLAS_CONTENT_MIN_CENTER_PX: f32 = 2.5;
pub const FANGYUAN_ATLAS_CONTENT_MAX_CENTER_PX: f32 = 61.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanCubeFace {
    Front,
    Back,
    Right,
    Left,
    Top,
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FangyuanCubeFaceUvContract {
    pub face: FangyuanCubeFace,
    pub normal: Vec3,
    /// Direction of increasing local U.
    pub tangent_u: Vec3,
    /// Direction toward the top of authored texture content. Local V increases
    /// in the opposite direction because atlas pixels use a top-left origin.
    pub authored_up: Vec3,
}

pub const FANGYUAN_CUBE_FACE_UV_CONTRACTS: [FangyuanCubeFaceUvContract; 6] = [
    FangyuanCubeFaceUvContract {
        face: FangyuanCubeFace::Front,
        normal: Vec3::Z,
        tangent_u: Vec3::X,
        authored_up: Vec3::Y,
    },
    FangyuanCubeFaceUvContract {
        face: FangyuanCubeFace::Back,
        normal: Vec3::NEG_Z,
        tangent_u: Vec3::NEG_X,
        authored_up: Vec3::Y,
    },
    FangyuanCubeFaceUvContract {
        face: FangyuanCubeFace::Right,
        normal: Vec3::X,
        tangent_u: Vec3::NEG_Z,
        authored_up: Vec3::Y,
    },
    FangyuanCubeFaceUvContract {
        face: FangyuanCubeFace::Left,
        normal: Vec3::NEG_X,
        tangent_u: Vec3::Z,
        authored_up: Vec3::Y,
    },
    FangyuanCubeFaceUvContract {
        face: FangyuanCubeFace::Top,
        normal: Vec3::Y,
        tangent_u: Vec3::X,
        authored_up: Vec3::NEG_Z,
    },
    FangyuanCubeFaceUvContract {
        face: FangyuanCubeFace::Bottom,
        normal: Vec3::NEG_Y,
        tangent_u: Vec3::X,
        authored_up: Vec3::Z,
    },
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FangyuanAtlasTileUvRect {
    pub min: Vec2,
    pub max: Vec2,
}

pub fn fangyuan_atlas_tile_content_uv_rect(tile_index: u16) -> Option<FangyuanAtlasTileUvRect> {
    if tile_index >= FANGYUAN_ATLAS_COLUMNS * FANGYUAN_ATLAS_ROWS {
        return None;
    }
    let x = f32::from(tile_index % FANGYUAN_ATLAS_COLUMNS);
    let y = f32::from(tile_index / FANGYUAN_ATLAS_COLUMNS);
    let cell_min = Vec2::new(x, y) * FANGYUAN_ATLAS_CELL_SIZE_PX;
    Some(FangyuanAtlasTileUvRect {
        min: (cell_min + Vec2::splat(FANGYUAN_ATLAS_CONTENT_MIN_CENTER_PX))
            / FANGYUAN_ATLAS_SIZE_PX,
        max: (cell_min + Vec2::splat(FANGYUAN_ATLAS_CONTENT_MAX_CENTER_PX))
            / FANGYUAN_ATLAS_SIZE_PX,
    })
}

/// Repeat in local tile space first, then map into texel-center content UV.
/// This deliberately never relies on whole-atlas sampler repeat.
pub fn fangyuan_atlas_uv_from_local(local_uv: Vec2, repeat: Vec2, tile_index: u16) -> Option<Vec2> {
    if !local_uv.is_finite() || !repeat.is_finite() || repeat.cmple(Vec2::ZERO).any() {
        return None;
    }
    let repeated = local_uv * repeat;
    let repeated = repeated - repeated.floor();
    let rect = fangyuan_atlas_tile_content_uv_rect(tile_index)?;
    Some(rect.min + repeated * (rect.max - rect.min))
}

pub fn fangyuan_cube_face_world_size(face: FangyuanCubeFace, primitive_world_size: Vec3) -> Vec2 {
    let size = primitive_world_size.abs();
    match face {
        FangyuanCubeFace::Front | FangyuanCubeFace::Back => Vec2::new(size.x, size.y),
        FangyuanCubeFace::Right | FangyuanCubeFace::Left => Vec2::new(size.z, size.y),
        FangyuanCubeFace::Top | FangyuanCubeFace::Bottom => Vec2::new(size.x, size.z),
    }
}

pub fn fangyuan_cube_face_repeat(
    face: FangyuanCubeFace,
    primitive_world_size: Vec3,
    reference_repeat_size_m: Vec2,
) -> Option<Vec2> {
    if !primitive_world_size.is_finite()
        || !reference_repeat_size_m.is_finite()
        || reference_repeat_size_m.cmple(Vec2::ZERO).any()
    {
        return None;
    }
    Some(fangyuan_cube_face_world_size(face, primitive_world_size) / reference_repeat_size_m)
}

/// v0 ellipsoid repeat approximation for the Y-up UV sphere. U spans the
/// equator; V spans one pole-to-pole half-meridian. Extreme non-uniform scales
/// retain UV/pole distortion as an explicit v0 limitation.
pub fn fangyuan_sphere_repeat(
    primitive_world_size: Vec3,
    reference_repeat_size_m: Vec2,
) -> Option<Vec2> {
    if !primitive_world_size.is_finite()
        || !reference_repeat_size_m.is_finite()
        || reference_repeat_size_m.cmple(Vec2::ZERO).any()
    {
        return None;
    }
    let size = primitive_world_size.abs();
    let horizontal_diameter = (size.x + size.z) * 0.5;
    let equator = std::f32::consts::PI * horizontal_diameter;
    let half_meridian = std::f32::consts::PI * (size.y + horizontal_diameter) * 0.25;
    Some(Vec2::new(equator, half_meridian) / reference_repeat_size_m)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FangyuanNormalGreenDirection {
    #[default]
    Unverified,
    PositiveY,
    NegativeY,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FangyuanNormalSamplingPolicy {
    pub green_direction: FangyuanNormalGreenDirection,
    pub default_enabled: bool,
    pub debug_direction_override_allowed: bool,
}

/// Relief-gradient checks against three explicit atlas fixtures confirm a
/// DirectX-authored normal map: positive green follows image-down, so Bevy's
/// right-handed tangent convention must flip Y when sampling.
pub const FANGYUAN_NORMAL_SAMPLING_POLICY: FangyuanNormalSamplingPolicy =
    FangyuanNormalSamplingPolicy {
        green_direction: FangyuanNormalGreenDirection::NegativeY,
        default_enabled: true,
        debug_direction_override_allowed: true,
    };

impl FangyuanNormalSamplingPolicy {
    pub const fn effective_direction(
        self,
        debug_override: Option<FangyuanNormalGreenDirection>,
    ) -> Option<FangyuanNormalGreenDirection> {
        if let Some(direction) = debug_override
            && self.debug_direction_override_allowed
            && !matches!(direction, FangyuanNormalGreenDirection::Unverified)
        {
            return Some(direction);
        }
        if !self.default_enabled {
            return None;
        }
        match self.green_direction {
            FangyuanNormalGreenDirection::PositiveY => {
                Some(FangyuanNormalGreenDirection::PositiveY)
            }
            FangyuanNormalGreenDirection::NegativeY => {
                Some(FangyuanNormalGreenDirection::NegativeY)
            }
            FangyuanNormalGreenDirection::Unverified => None,
        }
    }

    /// Direct mapping for Bevy 0.18.1 `StandardMaterial::flip_normal_map_y`.
    pub const fn standard_material_flip_normal_map_y(
        self,
        debug_override: Option<FangyuanNormalGreenDirection>,
    ) -> Option<bool> {
        match self.effective_direction(debug_override) {
            Some(FangyuanNormalGreenDirection::PositiveY) => Some(false),
            Some(FangyuanNormalGreenDirection::NegativeY) => Some(true),
            _ => None,
        }
    }
}

/// Reference form of Bevy's render-pipeline normal transform. Runtime ECS
/// rendering continues to use Bevy's own world transform and inverse-transpose
/// path; this helper keeps CPU diagnostics and later shader tests aligned.
pub fn fangyuan_normal_matrix(world_from_local: Mat4) -> Mat3 {
    Mat3::from_mat4(world_from_local).inverse().transpose()
}

pub fn fangyuan_unit_cube_mesh() -> Mesh {
    let mut positions = Vec::with_capacity(24);
    let mut normals = Vec::with_capacity(24);
    let mut uvs = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);
    let corners = [
        Vec2::new(0.0, 0.0),
        Vec2::new(1.0, 0.0),
        Vec2::new(1.0, 1.0),
        Vec2::new(0.0, 1.0),
    ];

    for contract in FANGYUAN_CUBE_FACE_UV_CONTRACTS {
        let base = positions.len() as u32;
        for uv in corners {
            let position = contract.normal * 0.5
                + contract.tangent_u * (uv.x - 0.5)
                + contract.authored_up * (0.5 - uv.y);
            positions.push(position.to_array());
            normals.push(contract.normal.to_array());
            uvs.push(uv.to_array());
        }
        indices.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_indices(Indices::U32(indices))
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.generate_tangents()
        .expect("Fangyuan unit cube UV contract must generate valid tangents");
    mesh
}

/// Build a Y-up UV sphere. U=0/1 is the duplicated +Z seam, V=0 is +Y,
/// V=1 is -Y. Pole vertices are duplicated per sector so each seam tangent is
/// stable; visible pole distortion remains an accepted v0 limitation.
pub fn fangyuan_unit_sphere_mesh(radius: f32, sectors: u32, stacks: u32) -> Mesh {
    assert!(radius.is_finite() && radius > 0.0);
    assert!(sectors >= 3);
    assert!(stacks >= 2);

    let mut positions = Vec::with_capacity(((stacks + 1) * (sectors + 1)) as usize);
    let mut normals = Vec::with_capacity(positions.capacity());
    let mut uvs = Vec::with_capacity(positions.capacity());
    let mut analytic_tangents = Vec::with_capacity(positions.capacity());
    let mut indices = Vec::with_capacity((stacks * sectors * 6) as usize);

    for stack in 0..=stacks {
        let v = stack as f32 / stacks as f32;
        let polar = std::f32::consts::PI * v;
        let y = polar.cos();
        let radial = polar.sin();
        for sector in 0..=sectors {
            let u = sector as f32 / sectors as f32;
            let longitude = std::f32::consts::TAU * u;
            let normal = Vec3::new(radial * longitude.sin(), y, radial * longitude.cos());
            positions.push((normal * radius).to_array());
            normals.push(normal.to_array());
            uvs.push([u, v]);
            analytic_tangents.push([longitude.cos(), 0.0, -longitude.sin(), 1.0]);
        }
    }

    for stack in 0..stacks {
        let mut top = stack * (sectors + 1);
        let mut bottom = top + sectors + 1;
        for _ in 0..sectors {
            if stack != 0 {
                indices.extend_from_slice(&[top, bottom, top + 1]);
            }
            if stack != stacks - 1 {
                indices.extend_from_slice(&[top + 1, bottom, bottom + 1]);
            }
            top += 1;
            bottom += 1;
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_indices(Indices::U32(indices))
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.generate_tangents()
        .expect("Fangyuan Y-up UV sphere contract must generate valid tangents");
    let Some(VertexAttributeValues::Float32x4(tangents)) =
        mesh.attribute_mut(Mesh::ATTRIBUTE_TANGENT)
    else {
        unreachable!("generated sphere tangents must use Float32x4");
    };
    for (tangent, analytic) in tangents.iter_mut().zip(analytic_tangents) {
        let tangent_xyz = Vec3::from_slice(&tangent[..3]);
        if !tangent_xyz.is_finite() || tangent_xyz.length_squared() < 0.9 {
            *tangent = analytic;
        }
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::MeshVertexAttribute;
    use std::{fs::File, io::BufReader, path::Path};

    fn float3_attribute(mesh: &Mesh, attribute: MeshVertexAttribute) -> &[[f32; 3]] {
        let Some(VertexAttributeValues::Float32x3(values)) = mesh.attribute(attribute) else {
            panic!("expected Float32x3 mesh attribute");
        };
        values
    }

    fn float2_attribute(mesh: &Mesh, attribute: MeshVertexAttribute) -> &[[f32; 2]] {
        let Some(VertexAttributeValues::Float32x2(values)) = mesh.attribute(attribute) else {
            panic!("expected Float32x2 mesh attribute");
        };
        values
    }

    fn float4_attribute(mesh: &Mesh, attribute: MeshVertexAttribute) -> &[[f32; 4]] {
        let Some(VertexAttributeValues::Float32x4(values)) = mesh.attribute(attribute) else {
            panic!("expected Float32x4 mesh attribute");
        };
        values
    }

    fn assert_outward_winding(mesh: &Mesh) {
        let positions = float3_attribute(mesh, Mesh::ATTRIBUTE_POSITION);
        let normals = float3_attribute(mesh, Mesh::ATTRIBUTE_NORMAL);
        let indices: Vec<usize> = match mesh.indices().expect("mesh must be indexed") {
            Indices::U16(indices) => indices.iter().map(|index| usize::from(*index)).collect(),
            Indices::U32(indices) => indices.iter().map(|index| *index as usize).collect(),
        };
        for triangle in indices.chunks_exact(3) {
            let a = Vec3::from_array(positions[triangle[0]]);
            let b = Vec3::from_array(positions[triangle[1]]);
            let c = Vec3::from_array(positions[triangle[2]]);
            let geometric = (b - a).cross(c - a);
            assert!(geometric.length_squared() > 1e-10);
            let expected = (Vec3::from_array(normals[triangle[0]])
                + Vec3::from_array(normals[triangle[1]])
                + Vec3::from_array(normals[triangle[2]]))
            .normalize();
            assert!(geometric.normalize().dot(expected) > 0.0);
        }
    }

    struct DecodedRgb8 {
        width: usize,
        height: usize,
        pixels: Vec<[f32; 3]>,
    }

    impl DecodedRgb8 {
        fn load(path: &Path) -> Self {
            let mut decoder = png::Decoder::new(BufReader::new(File::open(path).unwrap()));
            decoder.set_transformations(png::Transformations::normalize_to_color8());
            let mut reader = decoder.read_info().unwrap();
            let mut bytes = vec![0; reader.output_buffer_size().unwrap()];
            let output = reader.next_frame(&mut bytes).unwrap();
            let bytes = &bytes[..output.buffer_size()];
            let channels = match output.color_type {
                png::ColorType::Rgb => 3,
                png::ColorType::Rgba => 4,
                other => panic!("expected RGB/RGBA atlas, got {other:?}"),
            };
            let pixels = bytes
                .chunks_exact(channels)
                .map(|pixel| {
                    [
                        f32::from(pixel[0]) / 255.0,
                        f32::from(pixel[1]) / 255.0,
                        f32::from(pixel[2]) / 255.0,
                    ]
                })
                .collect();
            Self {
                width: output.width as usize,
                height: output.height as usize,
                pixels,
            }
        }

        fn pixel(&self, x: usize, y: usize) -> [f32; 3] {
            assert!(x < self.width && y < self.height);
            self.pixels[y * self.width + x]
        }

        fn luminance(&self, x: usize, y: usize) -> f32 {
            let [r, g, b] = self.pixel(x, y);
            (r + g + b) / 3.0
        }
    }

    fn relief_signed_product_agreement(
        base_color: &DecodedRgb8,
        normal: &DecodedRgb8,
        tile_x: usize,
        tile_y: usize,
    ) -> Vec2 {
        let left = tile_x * 64;
        let top = tile_y * 64;
        let mut signed_product = Vec2::ZERO;
        let mut product_strength = Vec2::ZERO;
        for local_y in (4..=59).step_by(2) {
            for local_x in (4..=59).step_by(2) {
                let x = left + local_x;
                let y = top + local_y;
                let gradient = Vec2::new(
                    base_color.luminance(x + 1, y) - base_color.luminance(x - 1, y),
                    base_color.luminance(x, y + 1) - base_color.luminance(x, y - 1),
                );
                let normal_rgb = normal.pixel(x, y);
                let tangent_xy = Vec2::new(
                    (normal_rgb[0] * 255.0 - 128.0) / 127.0,
                    (normal_rgb[1] * 255.0 - 128.0) / 127.0,
                );
                let product = gradient * tangent_xy;
                signed_product += product;
                product_strength += product.abs();
            }
        }
        assert!(product_strength.cmpgt(Vec2::ZERO).all());
        signed_product / product_strength
    }

    #[test]
    fn cube_faces_have_stable_authored_up_uv_normals_and_tangents() {
        let mesh = fangyuan_unit_cube_mesh();
        let positions = float3_attribute(&mesh, Mesh::ATTRIBUTE_POSITION);
        let normals = float3_attribute(&mesh, Mesh::ATTRIBUTE_NORMAL);
        let uvs = float2_attribute(&mesh, Mesh::ATTRIBUTE_UV_0);
        let tangents = float4_attribute(&mesh, Mesh::ATTRIBUTE_TANGENT);
        assert_eq!(positions.len(), 24);
        assert_eq!(normals.len(), 24);
        assert_eq!(uvs.len(), 24);
        assert_eq!(tangents.len(), 24);
        assert_outward_winding(&mesh);

        for (face_index, contract) in FANGYUAN_CUBE_FACE_UV_CONTRACTS.iter().enumerate() {
            let base = face_index * 4;
            assert_eq!(uvs[base], [0.0, 0.0]);
            assert_eq!(uvs[base + 2], [1.0, 1.0]);
            for normal in &normals[base..base + 4] {
                assert_eq!(Vec3::from_array(*normal), contract.normal);
            }
            let authored_up =
                Vec3::from_array(positions[base]) - Vec3::from_array(positions[base + 3]);
            let increasing_u =
                Vec3::from_array(positions[base + 1]) - Vec3::from_array(positions[base]);
            assert!(
                authored_up
                    .normalize()
                    .abs_diff_eq(contract.authored_up, 1e-6)
            );
            assert!(
                increasing_u
                    .normalize()
                    .abs_diff_eq(contract.tangent_u, 1e-6)
            );
            for tangent in &tangents[base..base + 4] {
                let tangent_xyz = Vec3::from_slice(&tangent[..3]).normalize();
                assert!(tangent_xyz.abs_diff_eq(contract.tangent_u, 1e-5));
                assert!(tangent[3].is_finite());
                assert!((tangent[3] - 1.0).abs() < 1e-5);
                let bitangent = tangent[3] * contract.normal.cross(tangent_xyz);
                assert!(bitangent.abs_diff_eq(contract.authored_up, 1e-5));
            }
        }
    }

    #[test]
    fn sphere_is_y_up_has_duplicated_seam_and_complete_tangents() {
        let sectors = 12;
        let stacks = 6;
        let mesh = fangyuan_unit_sphere_mesh(0.5, sectors, stacks);
        let positions = float3_attribute(&mesh, Mesh::ATTRIBUTE_POSITION);
        let normals = float3_attribute(&mesh, Mesh::ATTRIBUTE_NORMAL);
        let uvs = float2_attribute(&mesh, Mesh::ATTRIBUTE_UV_0);
        let tangents = float4_attribute(&mesh, Mesh::ATTRIBUTE_TANGENT);
        let row_len = (sectors + 1) as usize;
        assert_eq!(positions.len(), ((stacks + 1) * (sectors + 1)) as usize);
        assert_eq!(positions.len(), normals.len());
        assert_eq!(positions.len(), uvs.len());
        assert_eq!(positions.len(), tangents.len());
        assert_outward_winding(&mesh);
        assert!(Vec3::from_array(positions[0]).abs_diff_eq(Vec3::Y * 0.5, 1e-6));

        for row in 0..=stacks as usize {
            let first = row * row_len;
            let last = first + sectors as usize;
            assert!(
                Vec3::from_array(positions[first])
                    .abs_diff_eq(Vec3::from_array(positions[last]), 1e-5)
            );
            assert_eq!(uvs[first][0], 0.0);
            assert_eq!(uvs[last][0], 1.0);
        }
        assert!(tangents.iter().all(|tangent| {
            tangent.iter().all(|value| value.is_finite())
                && Vec3::from_slice(&tangent[..3]).length_squared() > 0.9
                && (tangent[3] - 1.0).abs() < 1e-5
        }));
    }

    #[test]
    fn repeat_uses_actual_non_uniform_cube_faces_and_sphere_spans() {
        let size = Vec3::new(4.0, 2.0, 6.0);
        let reference = Vec2::new(2.0, 1.0);
        assert_eq!(
            fangyuan_cube_face_repeat(FangyuanCubeFace::Front, size, reference),
            Some(Vec2::new(2.0, 2.0))
        );
        assert_eq!(
            fangyuan_cube_face_repeat(FangyuanCubeFace::Right, size, reference),
            Some(Vec2::new(3.0, 2.0))
        );
        assert_eq!(
            fangyuan_cube_face_repeat(FangyuanCubeFace::Top, size, reference),
            Some(Vec2::new(2.0, 6.0))
        );
        let sphere = fangyuan_sphere_repeat(Vec3::splat(2.0), Vec2::ONE).unwrap();
        assert!(sphere.abs_diff_eq(
            Vec2::new(2.0 * std::f32::consts::PI, std::f32::consts::PI),
            1e-6
        ));
    }

    #[test]
    fn tile_content_uv_uses_texel_centers_and_local_fract_without_crossing_cells() {
        let first = fangyuan_atlas_tile_content_uv_rect(0).unwrap();
        assert!(first.min.abs_diff_eq(Vec2::splat(2.5 / 1024.0), 1e-7));
        assert!(first.max.abs_diff_eq(Vec2::splat(61.5 / 1024.0), 1e-7));
        let last = fangyuan_atlas_tile_content_uv_rect(255).unwrap();
        assert!(last.min.abs_diff_eq(Vec2::splat(962.5 / 1024.0), 1e-7));
        assert!(last.max.abs_diff_eq(Vec2::splat(1021.5 / 1024.0), 1e-7));
        assert!(fangyuan_atlas_tile_content_uv_rect(256).is_none());

        let rect = fangyuan_atlas_tile_content_uv_rect(13).unwrap();
        for local in [
            Vec2::ZERO,
            Vec2::ONE,
            Vec2::new(3.25, -0.25),
            Vec2::new(100.9, 42.1),
        ] {
            let mapped = fangyuan_atlas_uv_from_local(local, Vec2::new(2.0, 3.0), 13).unwrap();
            assert!(mapped.cmpge(rect.min).all());
            assert!(mapped.cmple(rect.max).all());
        }
        assert!(
            fangyuan_atlas_uv_from_local(Vec2::ONE, Vec2::ONE, 13)
                .unwrap()
                .abs_diff_eq(rect.min, 1e-7)
        );
    }

    #[test]
    fn non_uniform_scale_normal_matrix_preserves_tangent_orthogonality() {
        let world_from_local = Mat4::from_scale_rotation_translation(
            Vec3::new(2.0, 3.0, 5.0),
            Quat::from_rotation_y(0.4),
            Vec3::new(7.0, 8.0, 9.0),
        );
        let local_tangent = Vec3::new(1.0, 0.0, 1.0).normalize();
        let local_normal = Vec3::new(-1.0, 0.0, 1.0).normalize();
        let world_tangent = world_from_local.transform_vector3(local_tangent);
        let world_normal = fangyuan_normal_matrix(world_from_local) * local_normal;
        assert!(world_tangent.dot(world_normal).abs() < 1e-5);
    }

    #[test]
    fn verified_negative_y_normal_direction_maps_to_bevy_flip_with_debug_override() {
        assert_eq!(
            FANGYUAN_NORMAL_SAMPLING_POLICY.green_direction,
            FangyuanNormalGreenDirection::NegativeY
        );
        assert!(FANGYUAN_NORMAL_SAMPLING_POLICY.default_enabled);
        assert_eq!(
            FANGYUAN_NORMAL_SAMPLING_POLICY.effective_direction(None),
            Some(FangyuanNormalGreenDirection::NegativeY)
        );
        assert_eq!(
            FANGYUAN_NORMAL_SAMPLING_POLICY.standard_material_flip_normal_map_y(None),
            Some(true)
        );
        assert_eq!(
            FANGYUAN_NORMAL_SAMPLING_POLICY
                .effective_direction(Some(FangyuanNormalGreenDirection::PositiveY)),
            Some(FangyuanNormalGreenDirection::PositiveY)
        );
        assert_eq!(
            FANGYUAN_NORMAL_SAMPLING_POLICY
                .effective_direction(Some(FangyuanNormalGreenDirection::NegativeY)),
            Some(FangyuanNormalGreenDirection::NegativeY)
        );
        assert_eq!(
            FANGYUAN_NORMAL_SAMPLING_POLICY.standard_material_flip_normal_map_y(Some(
                FangyuanNormalGreenDirection::PositiveY,
            )),
            Some(false)
        );
    }

    #[test]
    fn catalog_and_relief_pixels_confirm_directx_negative_y_normal_map() {
        let asset_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fangyuan/atlases/base");
        let catalog: super::super::FangyuanSurfaceAtlasCatalog = serde_json::from_reader(
            BufReader::new(File::open(asset_root.join("surface_atlas.json")).unwrap()),
        )
        .unwrap();
        assert_eq!(
            catalog.channels.normal.green_channel_direction.as_deref(),
            Some("negative_y")
        );

        let base_color = DecodedRgb8::load(&asset_root.join("surface_color.png"));
        let normal = DecodedRgb8::load(&asset_root.join("surface_normal.png"));
        assert_eq!((base_color.width, base_color.height), (1024, 1024));
        assert_eq!((normal.width, normal.height), (1024, 1024));
        for (tile_id, x, y) in [
            ("surface_x03_y09", 3, 9),
            ("surface_x06_y11", 6, 11),
            ("surface_x09_y05", 9, 5),
        ] {
            let agreement = relief_signed_product_agreement(&base_color, &normal, x, y);
            assert!(
                agreement.x > 0.08,
                "{tile_id} right/R relief signed-product agreement must stay positive: {agreement:?}"
            );
            assert!(
                agreement.y > 0.08,
                "{tile_id} down/G relief signed-product agreement must stay positive: {agreement:?}"
            );
        }
    }
}
