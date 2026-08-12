use bevy::{
    asset::RenderAssetUsages, mesh::Indices, prelude::*, render::render_resource::PrimitiveTopology,
};
use serde::{Deserialize, Deserializer, de};
use std::{fs, path::PathBuf};

use crate::{
    framework::scene::prelude::{SceneEvent, SceneOwned, SceneRuntimeRoot, SceneSessionId},
    game::scenes::main_world_contract::MAIN_WORLD_CLIENT_SCENE_ID,
};

pub(in crate::game) const MAIN_WORLD_SCENE_ID: &str = MAIN_WORLD_CLIENT_SCENE_ID;
const MAIN_WORLD_LAYOUT_PATH: &str = "scenes/main_world/layout.ron";
const MAIN_WORLD_MIN_COORDINATE: f32 = -2000.0;
const MAIN_WORLD_MAX_COORDINATE: f32 = 2000.0;
const MAIN_WORLD_MARKERS_PER_AXIS: usize = 41;
const MAIN_WORLD_MARKER_COUNT: usize = 1681;
const MAIN_WORLD_LOW_MARKER_SECTORS: usize = 8;
const MAIN_WORLD_LOW_MARKER_STACKS: usize = 6;
const MAIN_WORLD_LOW_MARKER_VERTICES: usize =
    (MAIN_WORLD_LOW_MARKER_SECTORS + 1) * (MAIN_WORLD_LOW_MARKER_STACKS + 1);
const MAIN_WORLD_LOW_MARKER_TRIANGLES: usize =
    MAIN_WORLD_LOW_MARKER_SECTORS * 2 * (MAIN_WORLD_LOW_MARKER_STACKS - 1);

pub(super) struct MainWorldScenePlugin;

impl Plugin for MainWorldScenePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .add_systems(PostUpdate, instantiate_main_world_content);
    }
}

#[derive(Clone, Debug, Component, PartialEq, Eq)]
struct MainWorldContent {
    session_id: SceneSessionId,
}

#[derive(Clone, Debug, Component, PartialEq, Eq)]
struct MainWorldDistanceMarkerCollection;

#[derive(Clone, Debug, Deserialize)]
struct MainWorldLayout {
    scene_id: String,
    terrain: MainWorldTerrain,
    distance_markers: MainWorldDistanceMarkers,
    light: MainWorldLight,
}

#[derive(Clone, Debug, Deserialize)]
struct MainWorldTerrain {
    #[serde(deserialize_with = "deserialize_f32_array_3")]
    size: [f32; 3],
    top_y: f32,
    #[serde(deserialize_with = "deserialize_f32_array_3")]
    color: [f32; 3],
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum MainWorldMarkerQuality {
    Low,
}

#[derive(Clone, Debug, Deserialize)]
struct MainWorldDistanceMarkers {
    start: f32,
    end: f32,
    spacing: f32,
    radius: f32,
    #[serde(deserialize_with = "deserialize_f32_array_3")]
    color: [f32; 3],
    quality: MainWorldMarkerQuality,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct MainWorldDistanceMarkerMeshStats {
    marker_count: usize,
    vertices_per_marker: usize,
    triangles_per_marker: usize,
    vertex_count: usize,
    index_count: usize,
    first_center: Vec3,
    last_center: Vec3,
    bounds_min: Vec3,
    bounds_max: Vec3,
}

#[derive(Debug)]
struct MainWorldDistanceMarkerMesh {
    mesh: Mesh,
    stats: MainWorldDistanceMarkerMeshStats,
}

struct MainWorldMarkerTemplate {
    positions: Vec<Vec3>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

#[derive(Clone, Debug, Deserialize)]
struct MainWorldLight {
    #[serde(deserialize_with = "deserialize_f32_array_3")]
    rotation_degrees: [f32; 3],
    #[serde(deserialize_with = "deserialize_f32_array_3")]
    color: [f32; 3],
    illuminance: f32,
}

impl MainWorldLayout {
    fn validate(&self) -> Result<(), String> {
        const TERRAIN_SIZE: [f32; 3] = [4000.0, 0.4, 4000.0];
        const TERRAIN_TOP_Y: f32 = 0.0;

        if self.terrain.size != TERRAIN_SIZE {
            return Err(format!(
                "terrain size must be {TERRAIN_SIZE:?}, got {:?}",
                self.terrain.size
            ));
        }
        if self.terrain.top_y != TERRAIN_TOP_Y {
            return Err(format!(
                "terrain top_y must be {TERRAIN_TOP_Y}, got {}",
                self.terrain.top_y
            ));
        }

        let markers = &self.distance_markers;
        if !markers.start.is_finite() || !markers.end.is_finite() || markers.start > markers.end {
            return Err("distance marker range must be finite and ordered".to_owned());
        }
        if markers.start != MAIN_WORLD_MIN_COORDINATE || markers.end != MAIN_WORLD_MAX_COORDINATE {
            return Err(format!(
                "distance marker range must be {MAIN_WORLD_MIN_COORDINATE}..{MAIN_WORLD_MAX_COORDINATE}"
            ));
        }
        if !markers.spacing.is_finite() || markers.spacing <= 0.0 {
            return Err("distance marker spacing must be finite and greater than zero".to_owned());
        }
        if !markers.radius.is_finite() || markers.radius <= 0.0 {
            return Err("distance marker radius must be finite and greater than zero".to_owned());
        }

        let intervals = (markers.end - markers.start) / markers.spacing;
        if !intervals.is_finite() || (intervals - intervals.round()).abs() > f32::EPSILON {
            return Err("distance marker range must be evenly divisible by spacing".to_owned());
        }
        if intervals.round() != (MAIN_WORLD_MARKERS_PER_AXIS - 1) as f32 {
            return Err(format!(
                "distance markers must form {MAIN_WORLD_MARKERS_PER_AXIS} points per axis"
            ));
        }
        let axis_count = intervals.round() as usize + 1;
        let total_count = axis_count
            .checked_mul(axis_count)
            .ok_or_else(|| "distance marker count overflowed".to_owned())?;
        if axis_count != MAIN_WORLD_MARKERS_PER_AXIS || total_count != MAIN_WORLD_MARKER_COUNT {
            return Err(format!(
                "distance markers must form {MAIN_WORLD_MARKERS_PER_AXIS} points per axis and {MAIN_WORLD_MARKER_COUNT} total, got {axis_count} and {total_count}"
            ));
        }

        Ok(())
    }
}

fn build_main_world_distance_marker_mesh(
    markers: &MainWorldDistanceMarkers,
) -> Result<MainWorldDistanceMarkerMesh, String> {
    let intervals = (markers.end - markers.start) / markers.spacing;
    if !markers.start.is_finite()
        || !markers.end.is_finite()
        || markers.start != MAIN_WORLD_MIN_COORDINATE
        || markers.end != MAIN_WORLD_MAX_COORDINATE
        || !markers.spacing.is_finite()
        || markers.spacing <= 0.0
        || !markers.radius.is_finite()
        || markers.radius <= 0.0
        || intervals.round() != (MAIN_WORLD_MARKERS_PER_AXIS - 1) as f32
    {
        return Err("distance marker mesh configuration does not match the world contract".into());
    }

    let (sectors, stacks, expected_vertices, expected_triangles) = match markers.quality {
        MainWorldMarkerQuality::Low => (
            MAIN_WORLD_LOW_MARKER_SECTORS,
            MAIN_WORLD_LOW_MARKER_STACKS,
            MAIN_WORLD_LOW_MARKER_VERTICES,
            MAIN_WORLD_LOW_MARKER_TRIANGLES,
        ),
    };
    let template = build_main_world_marker_template(markers.radius, sectors, stacks);
    if template.positions.len() != expected_vertices
        || template.indices.len() != expected_triangles * 3
    {
        return Err("distance marker template does not match its frozen quality budget".into());
    }
    let vertex_count = template.positions.len() * MAIN_WORLD_MARKER_COUNT;
    let index_count = template.indices.len() * MAIN_WORLD_MARKER_COUNT;
    if vertex_count > u32::MAX as usize {
        return Err("distance marker mesh exceeds u32 index capacity".into());
    }

    let mut positions = Vec::with_capacity(vertex_count);
    let mut normals = Vec::with_capacity(vertex_count);
    let mut uvs = Vec::with_capacity(vertex_count);
    let mut indices = Vec::with_capacity(index_count);
    for z_index in 0..MAIN_WORLD_MARKERS_PER_AXIS {
        let z = markers.start + z_index as f32 * markers.spacing;
        for x_index in 0..MAIN_WORLD_MARKERS_PER_AXIS {
            let x = markers.start + x_index as f32 * markers.spacing;
            let center = Vec3::new(x, markers.radius, z);
            let base_index = positions.len() as u32;

            positions.extend(
                template
                    .positions
                    .iter()
                    .map(|position| (*position + center).to_array()),
            );
            normals.extend_from_slice(&template.normals);
            uvs.extend_from_slice(&template.uvs);
            indices.extend(template.indices.iter().map(|index| base_index + index));
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));

    Ok(MainWorldDistanceMarkerMesh {
        mesh,
        stats: MainWorldDistanceMarkerMeshStats {
            marker_count: MAIN_WORLD_MARKER_COUNT,
            vertices_per_marker: expected_vertices,
            triangles_per_marker: expected_triangles,
            vertex_count,
            index_count,
            first_center: Vec3::new(markers.start, markers.radius, markers.start),
            last_center: Vec3::new(markers.end, markers.radius, markers.end),
            bounds_min: Vec3::new(
                markers.start - markers.radius,
                0.0,
                markers.start - markers.radius,
            ),
            bounds_max: Vec3::new(
                markers.end + markers.radius,
                markers.radius * 2.0,
                markers.end + markers.radius,
            ),
        },
    })
}

fn build_main_world_marker_template(
    radius: f32,
    sectors: usize,
    stacks: usize,
) -> MainWorldMarkerTemplate {
    let mut positions = Vec::with_capacity((sectors + 1) * (stacks + 1));
    let mut normals = Vec::with_capacity(positions.capacity());
    let mut uvs = Vec::with_capacity(positions.capacity());
    let mut indices = Vec::with_capacity(sectors * 2 * (stacks - 1) * 3);

    for stack in 0..=stacks {
        let v = stack as f32 / stacks as f32;
        let phi = std::f32::consts::FRAC_PI_2 - v * std::f32::consts::PI;
        let ring_radius = phi.cos();
        let y = phi.sin();
        for sector in 0..=sectors {
            let u = sector as f32 / sectors as f32;
            let theta = u * std::f32::consts::TAU;
            let normal = if stack == 0 {
                Vec3::Y
            } else if stack == stacks {
                Vec3::NEG_Y
            } else {
                Vec3::new(ring_radius * theta.cos(), y, ring_radius * theta.sin())
            };
            positions.push(normal * radius);
            normals.push(normal.to_array());
            uvs.push([u, v]);
        }
    }

    let row = sectors + 1;
    for stack in 0..stacks {
        for sector in 0..sectors {
            let top_left = (stack * row + sector) as u32;
            let bottom_left = top_left + row as u32;
            if stack > 0 {
                indices.extend_from_slice(&[top_left, bottom_left, top_left + 1]);
            }
            if stack + 1 < stacks {
                indices.extend_from_slice(&[top_left + 1, bottom_left, bottom_left + 1]);
            }
        }
    }

    MainWorldMarkerTemplate {
        positions,
        normals,
        uvs,
        indices,
    }
}

fn instantiate_main_world_content(
    mut commands: Commands,
    mut scene_events: MessageReader<SceneEvent>,
    runtime_roots: Query<(Entity, &SceneRuntimeRoot)>,
    existing_content: Query<&MainWorldContent>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut instantiated_sessions = Vec::new();

    for event in scene_events.read() {
        let SceneEvent::Entered(entered) = event else {
            continue;
        };
        if entered.scene_id.as_str() != MAIN_WORLD_SCENE_ID
            || existing_content
                .iter()
                .any(|content| content.session_id == entered.session_id)
            || instantiated_sessions.contains(&entered.session_id)
        {
            continue;
        }

        let Some(parent) = runtime_roots
            .iter()
            .find(|(_, root)| root.is_session(&entered.session_id))
            .map(|(entity, _)| entity)
        else {
            warn!(
                "main world session `{}` has no runtime root",
                entered.session_id
            );
            continue;
        };
        let layout = match load_main_world_layout() {
            Ok(layout) => layout,
            Err(error) => {
                warn!("main world layout could not be loaded: {error}");
                continue;
            }
        };
        if layout.scene_id != MAIN_WORLD_SCENE_ID {
            warn!("main world layout scene id does not match its registered scene");
            continue;
        }
        let distance_marker_mesh =
            match build_main_world_distance_marker_mesh(&layout.distance_markers) {
                Ok(mesh) => mesh,
                Err(error) => {
                    warn!("main world distance marker mesh could not be generated: {error}");
                    continue;
                }
            };
        let distance_marker_stats = distance_marker_mesh.stats;

        let session_id = entered.session_id.clone();
        let content = commands
            .spawn((
                SceneOwned::new(session_id.clone()),
                MainWorldContent {
                    session_id: session_id.clone(),
                },
                Name::new(format!("MainWorldContent({session_id})")),
            ))
            .id();
        commands.entity(parent).add_child(content);

        spawn_main_world_visuals(
            &mut commands,
            content,
            &session_id,
            &layout,
            distance_marker_mesh.mesh,
            &mut meshes,
            &mut materials,
        );
        info!(
            scene_id = MAIN_WORLD_SCENE_ID,
            session_id = %session_id,
            objects = 3,
            marker_count = distance_marker_stats.marker_count,
            marker_vertices = distance_marker_stats.vertex_count,
            marker_triangles = distance_marker_stats.index_count / 3,
            "main world visuals instantiated: terrain, distance marker collection, directional light"
        );
        instantiated_sessions.push(session_id);
    }
}

fn spawn_main_world_visuals(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    layout: &MainWorldLayout,
    distance_marker_mesh: Mesh,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    spawn_owned_child(
        commands,
        parent,
        (
            Mesh3d(meshes.add(Cuboid::new(
                layout.terrain.size[0],
                layout.terrain.size[1],
                layout.terrain.size[2],
            ))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: color(layout.terrain.color),
                perceptual_roughness: 0.92,
                ..default()
            })),
            Transform::from_xyz(
                0.0,
                layout.terrain.top_y - layout.terrain.size[1] * 0.5,
                0.0,
            ),
            Name::new("MainWorldTerrain"),
        ),
        session_id,
    );
    spawn_owned_child(
        commands,
        parent,
        (
            MainWorldDistanceMarkerCollection,
            Mesh3d(meshes.add(distance_marker_mesh)),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: color(layout.distance_markers.color),
                emissive: color(layout.distance_markers.color).into(),
                ..default()
            })),
            Transform::IDENTITY,
            Name::new("MainWorldDistanceMarkerCollection"),
        ),
        session_id,
    );
    spawn_owned_child(
        commands,
        parent,
        (
            DirectionalLight {
                color: color(layout.light.color),
                illuminance: layout.light.illuminance,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_rotation(Quat::from_euler(
                EulerRot::XYZ,
                layout.light.rotation_degrees[0].to_radians(),
                layout.light.rotation_degrees[1].to_radians(),
                layout.light.rotation_degrees[2].to_radians(),
            )),
            Name::new("MainWorldDirectionalLight"),
        ),
        session_id,
    );
}

fn spawn_owned_child<B: Bundle>(
    commands: &mut Commands,
    parent: Entity,
    bundle: B,
    session_id: &SceneSessionId,
) {
    let child = commands
        .spawn((bundle, SceneOwned::new(session_id.clone())))
        .id();
    commands.entity(parent).add_child(child);
}

fn color(value: [f32; 3]) -> Color {
    Color::srgb(value[0], value[1], value[2])
}

fn deserialize_f32_array_3<'de, D>(deserializer: D) -> Result<[f32; 3], D::Error>
where
    D: Deserializer<'de>,
{
    let values = Vec::<f32>::deserialize(deserializer)?;
    match values.as_slice() {
        [x, y, z] => Ok([*x, *y, *z]),
        _ => Err(de::Error::invalid_length(
            values.len(),
            &"exactly three f32 values",
        )),
    }
}

fn load_main_world_layout() -> Result<MainWorldLayout, String> {
    let Some(path) = first_package_asset_paths()
        .into_iter()
        .map(|root| root.join(MAIN_WORLD_LAYOUT_PATH))
        .find(|path| path.is_file())
    else {
        return Err(format!(
            "layout `{MAIN_WORLD_LAYOUT_PATH}` was not found under assets"
        ));
    };
    let source = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    parse_main_world_layout(&source)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

fn parse_main_world_layout(source: &str) -> Result<MainWorldLayout, String> {
    let layout: MainWorldLayout =
        ron::from_str(source).map_err(|error| format!("invalid RON: {error}"))?;
    layout.validate()?;
    Ok(layout)
}

fn first_package_asset_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(current_dir) = std::env::current_dir() {
        paths.push(current_dir.join("assets"));
        paths.push(current_dir.join("project").join("assets"));
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::scene::prelude::{
        SceneCommand, SceneDefinition, SceneEntered, SceneExitRequest, SceneKind, ScenePlugin,
        SceneRegistry, SceneRuntime, SceneRuntimeRoot,
    };
    use bevy::asset::AssetPlugin;
    use bevy::camera::primitives::MeshAabb;
    use bevy::mesh::VertexAttributeValues;

    #[test]
    fn main_world_layout_uses_the_contract_client_scene_id() {
        let layout = load_main_world_layout().unwrap();
        assert_eq!(layout.scene_id, MAIN_WORLD_SCENE_ID);
        assert_eq!(layout.terrain.size, [4000.0, 0.4, 4000.0]);
        assert_eq!(layout.terrain.top_y, 0.0);
        assert_eq!(layout.distance_markers.start, -2000.0);
        assert_eq!(layout.distance_markers.end, 2000.0);
        assert_eq!(layout.distance_markers.spacing, 100.0);
        assert_eq!(layout.distance_markers.radius, 0.5);
        assert_eq!(layout.distance_markers.quality, MainWorldMarkerQuality::Low);
        assert_eq!(MAIN_WORLD_MARKERS_PER_AXIS, 41);
        assert_eq!(MAIN_WORLD_MARKER_COUNT, 1681);
    }

    #[test]
    fn main_world_layout_rejects_invalid_distance_marker_contracts() {
        let valid = r#"(
            scene_id: "world.main",
            terrain: (size: [4000.0, 0.4, 4000.0], top_y: 0.0, color: [0.18, 0.42, 0.24]),
            distance_markers: (
                start: -2000.0,
                end: 2000.0,
                spacing: 100.0,
                radius: 0.5,
                color: [0.18, 0.64, 0.92],
                quality: low,
            ),
            light: (
                rotation_degrees: [-48.0, -28.0, 0.0],
                color: [1.0, 0.96, 0.86],
                illuminance: 7000.0,
            ),
        )"#;

        parse_main_world_layout(valid).unwrap();
        for (field, invalid_value) in [
            ("start", "0.0"),
            ("end", "-2100.0"),
            ("spacing", "0.0"),
            ("spacing", "110.0"),
            ("spacing", "0.0000000001"),
            ("radius", "0.0"),
        ] {
            let original = match field {
                "start" => "-2000.0",
                "end" => "2000.0",
                "spacing" => "100.0",
                "radius" => "0.5",
                _ => unreachable!(),
            };
            let invalid = valid.replacen(
                &format!("{field}: {original}"),
                &format!("{field}: {invalid_value}"),
                1,
            );
            assert!(
                parse_main_world_layout(&invalid).is_err(),
                "{field}={invalid_value} should be rejected"
            );
        }

        let mut non_finite = parse_main_world_layout(valid).unwrap();
        non_finite.distance_markers.start = f32::NAN;
        assert!(non_finite.validate().is_err());
    }

    #[test]
    fn distance_marker_mesh_merges_all_markers_with_valid_attributes_and_bounds() {
        let layout = load_main_world_layout().unwrap();
        let generated = build_main_world_distance_marker_mesh(&layout.distance_markers).unwrap();
        let stats = generated.stats;

        assert_eq!(stats.marker_count, 1681);
        assert_eq!(stats.vertices_per_marker, 63);
        assert_eq!(stats.triangles_per_marker, 80);
        assert_eq!(stats.vertex_count, 1681 * 63);
        assert_eq!(stats.index_count, 1681 * 80 * 3);
        assert!(stats.vertex_count > u16::MAX as usize);
        assert_eq!(stats.first_center, Vec3::new(-2000.0, 0.5, -2000.0));
        assert_eq!(stats.last_center, Vec3::new(2000.0, 0.5, 2000.0));
        assert_eq!(stats.bounds_min, Vec3::new(-2000.5, 0.0, -2000.5));
        assert_eq!(stats.bounds_max, Vec3::new(2000.5, 1.0, 2000.5));

        let mesh = &generated.mesh;
        let positions = match mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap() {
            VertexAttributeValues::Float32x3(values) => values,
            other => panic!("unexpected position attribute: {other:?}"),
        };
        let normal_len = match mesh.attribute(Mesh::ATTRIBUTE_NORMAL).unwrap() {
            VertexAttributeValues::Float32x3(values) => values.len(),
            other => panic!("unexpected normal attribute: {other:?}"),
        };
        let uv_len = match mesh.attribute(Mesh::ATTRIBUTE_UV_0).unwrap() {
            VertexAttributeValues::Float32x2(values) => values.len(),
            other => panic!("unexpected UV attribute: {other:?}"),
        };
        assert_eq!(positions.len(), stats.vertex_count);
        assert_eq!(normal_len, stats.vertex_count);
        assert_eq!(uv_len, stats.vertex_count);

        let marker_centers = positions
            .chunks_exact(stats.vertices_per_marker)
            .map(|vertices| {
                let top = Vec3::from_array(vertices[0]);
                Vec3::new(top.x, top.y - layout.distance_markers.radius, top.z)
            })
            .collect::<Vec<_>>();
        assert_eq!(marker_centers.len(), MAIN_WORLD_MARKER_COUNT);
        for (marker_index, center) in marker_centers.into_iter().enumerate() {
            let x_index = marker_index % MAIN_WORLD_MARKERS_PER_AXIS;
            let z_index = marker_index / MAIN_WORLD_MARKERS_PER_AXIS;
            assert_eq!(
                center,
                Vec3::new(
                    MAIN_WORLD_MIN_COORDINATE + x_index as f32 * 100.0,
                    0.5,
                    MAIN_WORLD_MIN_COORDINATE + z_index as f32 * 100.0,
                )
            );
        }

        let Some(Indices::U32(indices)) = mesh.indices() else {
            panic!("distance marker mesh must use u32 indices");
        };
        assert_eq!(indices.len(), stats.index_count);
        assert!(
            indices
                .iter()
                .all(|index| (*index as usize) < stats.vertex_count)
        );
        assert_eq!(mesh.compute_aabb().unwrap().min(), stats.bounds_min.into());
        assert_eq!(mesh.compute_aabb().unwrap().max(), stats.bounds_max.into());
    }

    #[test]
    fn distance_marker_mesh_builder_is_pure_and_rejects_invalid_configuration() {
        let layout = load_main_world_layout().unwrap();
        let mut invalid = layout.distance_markers.clone();
        invalid.radius = 0.0;
        assert!(build_main_world_distance_marker_mesh(&invalid).is_err());

        let mut shifted = layout.distance_markers.clone();
        shifted.start = -1900.0;
        shifted.end = 2100.0;
        assert!(build_main_world_distance_marker_mesh(&shifted).is_err());
    }

    #[test]
    fn entered_session_owns_one_marker_collection_without_duplicate_entities_or_assets() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .add_message::<SceneEvent>()
            .add_plugins(MainWorldScenePlugin);

        let session_id = SceneSessionId::from("main-world-test-1");
        app.world_mut()
            .spawn(SceneRuntimeRoot::new(session_id.clone()));
        app.world_mut()
            .write_message(SceneEvent::Entered(SceneEntered {
                scene_id: MAIN_WORLD_SCENE_ID.into(),
                session_id: session_id.clone(),
                content_version: None,
            }));
        app.update();

        let world = app.world_mut();
        let mut contents = world.query::<(Entity, &MainWorldContent, &SceneOwned)>();
        let (content_entity, content, content_owned) = contents.single(world).unwrap();
        assert_eq!(content.session_id, session_id);
        assert_eq!(content_owned.session_id, session_id);
        assert_eq!(
            world
                .query::<(&Mesh3d, &SceneOwned)>()
                .iter(world)
                .filter(|(_, owned)| owned.session_id == session_id)
                .count(),
            2
        );
        let mut marker_collections = world.query::<(
            &MainWorldDistanceMarkerCollection,
            &Mesh3d,
            &MeshMaterial3d<StandardMaterial>,
            &SceneOwned,
            &Name,
        )>();
        let (_, marker_mesh, marker_material, owned, name) =
            marker_collections.single(world).unwrap();
        assert_eq!(owned.session_id, session_id);
        assert_eq!(name.as_str(), "MainWorldDistanceMarkerCollection");
        assert!(
            world
                .resource::<Assets<Mesh>>()
                .get(&marker_mesh.0)
                .is_some()
        );
        assert!(
            world
                .resource::<Assets<StandardMaterial>>()
                .get(&marker_material.0)
                .is_some()
        );
        assert_eq!(world.resource::<Assets<Mesh>>().len(), 2);
        assert_eq!(world.resource::<Assets<StandardMaterial>>().len(), 2);
        let mut visual_children = world.query::<(&ChildOf, &SceneOwned, &Name)>();
        let visual_children = visual_children
            .iter(world)
            .filter(|(_, owned, name)| {
                owned.session_id == session_id
                    && matches!(
                        name.as_str(),
                        "MainWorldTerrain"
                            | "MainWorldDistanceMarkerCollection"
                            | "MainWorldDirectionalLight"
                    )
            })
            .collect::<Vec<_>>();
        assert_eq!(visual_children.len(), 3);
        assert!(visual_children.iter().all(|(parent, owned, _)| {
            parent.parent() == content_entity && owned.session_id == session_id
        }));
        assert_eq!(
            world
                .query::<(&DirectionalLight, &SceneOwned)>()
                .iter(world)
                .filter(|(_, owned)| owned.session_id == session_id)
                .count(),
            1
        );

        app.world_mut()
            .write_message(SceneEvent::Entered(SceneEntered {
                scene_id: MAIN_WORLD_SCENE_ID.into(),
                session_id: session_id.clone(),
                content_version: None,
            }));
        app.update();

        assert_eq!(
            app.world_mut()
                .query::<&MainWorldContent>()
                .iter(app.world())
                .count(),
            1
        );
        assert_eq!(
            app.world_mut()
                .query::<&MainWorldDistanceMarkerCollection>()
                .iter(app.world())
                .count(),
            1
        );
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 2);
        assert_eq!(app.world().resource::<Assets<StandardMaterial>>().len(), 2);
    }

    #[test]
    fn framework_manifest_entry_spawns_and_exit_cleans_main_world_content() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin))
            .add_plugins(MainWorldScenePlugin);
        app.world_mut()
            .resource_mut::<SceneRegistry>()
            .register(SceneDefinition::first_package_manifest(
                MAIN_WORLD_SCENE_ID,
                SceneKind::World,
                "scenes/main_world/scene.ron",
            ))
            .unwrap();

        let session_id = SceneSessionId::from("main-world-framework-integration");
        let mut request =
            crate::framework::scene::prelude::SceneEnterRequest::new(MAIN_WORLD_SCENE_ID);
        request.session_id = Some(session_id.clone());
        app.world_mut().write_message(SceneCommand::Enter(request));
        app.update();
        app.update();

        let runtime = app.world().resource::<SceneRuntime>();
        let active = runtime.active().unwrap_or_else(|| {
            panic!(
                "main world framework entry did not activate: state={:?}, error={:?}",
                runtime.state(),
                runtime.last_error
            )
        });
        assert_eq!(active.scene_id.as_str(), MAIN_WORLD_SCENE_ID);
        assert_eq!(active.session_id, session_id);
        assert_eq!(
            app.world_mut()
                .query::<&MainWorldContent>()
                .iter(app.world())
                .count(),
            1
        );

        app.world_mut()
            .write_message(SceneCommand::Exit(SceneExitRequest {
                scene_id: Some(MAIN_WORLD_SCENE_ID.into()),
                session_id: Some(session_id.clone()),
                ..SceneExitRequest::default()
            }));
        app.update();
        app.update();

        assert!(app.world().resource::<SceneRuntime>().active().is_none());
        assert_eq!(
            app.world_mut()
                .query::<&MainWorldContent>()
                .iter(app.world())
                .count(),
            0
        );

        let second_session = SceneSessionId::from("main-world-framework-reentry");
        let mut request =
            crate::framework::scene::prelude::SceneEnterRequest::new(MAIN_WORLD_SCENE_ID);
        request.session_id = Some(second_session.clone());
        app.world_mut().write_message(SceneCommand::Enter(request));
        app.update();
        app.update();

        assert_eq!(
            app.world_mut()
                .query::<(&MainWorldContent, &SceneOwned)>()
                .iter(app.world())
                .filter(|(content, owned)| {
                    content.session_id == second_session && owned.session_id == second_session
                })
                .count(),
            1
        );
        assert_eq!(
            app.world_mut()
                .query::<(&MainWorldDistanceMarkerCollection, &SceneOwned)>()
                .iter(app.world())
                .filter(|(_, owned)| owned.session_id == second_session)
                .count(),
            1
        );
        assert_eq!(
            app.world_mut()
                .query::<&SceneOwned>()
                .iter(app.world())
                .filter(|owned| owned.session_id == session_id)
                .count(),
            0
        );
    }

    #[test]
    fn failed_manifest_load_never_spawns_main_world_content() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin))
            .add_plugins(MainWorldScenePlugin);
        app.world_mut()
            .resource_mut::<SceneRegistry>()
            .register(SceneDefinition::first_package_manifest(
                MAIN_WORLD_SCENE_ID,
                SceneKind::World,
                "scenes/main_world/missing-scene.ron",
            ))
            .unwrap();

        let session_id = SceneSessionId::from("main-world-failed-load");
        let mut request =
            crate::framework::scene::prelude::SceneEnterRequest::new(MAIN_WORLD_SCENE_ID);
        request.session_id = Some(session_id.clone());
        app.world_mut().write_message(SceneCommand::Enter(request));
        app.update();
        app.update();

        assert!(app.world().resource::<SceneRuntime>().active().is_none());
        assert!(app.world().resource::<SceneRuntime>().last_error.is_some());
        assert_eq!(
            app.world_mut()
                .query::<&MainWorldContent>()
                .iter(app.world())
                .count(),
            0
        );
        assert_eq!(
            app.world_mut()
                .query::<&SceneOwned>()
                .iter(app.world())
                .filter(|owned| owned.session_id == session_id)
                .count(),
            0
        );
    }
}
