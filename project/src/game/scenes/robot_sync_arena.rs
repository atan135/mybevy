use bevy::{gltf::GltfAssetLabel, prelude::*, scene::SceneRoot as BevySceneRoot};
use serde::{Deserialize, Deserializer, de};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

use crate::framework::scene::prelude::{SceneEvent, SceneOwned, SceneRuntimeRoot, SceneSessionId};
use crate::game::features::robot_sync::coordinates::{
    ROBOT_SYNC_WORLD_UNITS_PER_SYNC_UNIT, robot_sync_axis_world_units_from_sync,
    robot_sync_world_position_from_sync,
};

pub(in crate::game) const ROBOT_SYNC_ARENA_SCENE_ID: &str = "arena.robot_sync";
const ROBOT_SYNC_ARENA_LAYOUT_PATH: &str = "scenes/robot_sync_arena/layout.ron";
#[cfg(test)]
const ROBOT_SYNC_ARENA_SCENE_MANIFEST_PATH: &str = "scenes/robot_sync_arena/scene.ron";
const ROBOT_SYNC_ARENA_FLOOR_TILE_ASSET_PATH: &str =
    "models/scenes/kaykit_dungeon_remastered/floor_tile_large.gltf";
const ARENA_BASE_THICKNESS: f32 = 4.0;
const ARENA_BASE_TOP_Y: f32 = -0.25;
const FLOOR_TILE_SOURCE_HALF_EXTENT: f32 = 2.0;
const FLOOR_TILE_SCALE_XZ: f32 = 50.0;
const FLOOR_TILE_SCALE_Y: f32 = 1.0;
const FLOOR_TILE_Y: f32 = 0.0;
const FLOOR_TILE_TOP_Y: f32 = 0.05;
const FLOOR_SURFACE_CLEARANCE: f32 = 0.05;
const GRID_LINE_HEIGHT: f32 = 0.18;
const GRID_LINE_Y: f32 = FLOOR_TILE_TOP_Y + FLOOR_SURFACE_CLEARANCE + GRID_LINE_HEIGHT * 0.5;
const BOUNDARY_WALL_HEIGHT: f32 = 5.0;
const BOUNDARY_WALL_Y: f32 =
    FLOOR_TILE_TOP_Y + FLOOR_SURFACE_CLEARANCE + BOUNDARY_WALL_HEIGHT * 0.5;
const SPAWN_MARKER_INNER_RADIUS_SYNC: f32 = 11.0;
const SPAWN_MARKER_OUTER_RADIUS_SYNC: f32 = 16.0;
const SPAWN_MARKER_INNER_RADIUS: f32 =
    SPAWN_MARKER_INNER_RADIUS_SYNC * ROBOT_SYNC_WORLD_UNITS_PER_SYNC_UNIT;
const SPAWN_MARKER_OUTER_RADIUS: f32 =
    SPAWN_MARKER_OUTER_RADIUS_SYNC * ROBOT_SYNC_WORLD_UNITS_PER_SYNC_UNIT;
const SPAWN_MARKER_VERTICAL_SCALE: f32 = 0.12;
const SPAWN_MARKER_Y: f32 = GRID_LINE_Y
    + GRID_LINE_HEIGHT * 0.5
    + spawn_marker_vertical_half_extent()
    + FLOOR_SURFACE_CLEARANCE;
const SPAWN_MARKER_COLOR: Color = Color::srgb(1.0, 0.84, 0.18);

pub(super) struct RobotSyncArenaPlugin;

impl Plugin for RobotSyncArenaPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .add_systems(PostUpdate, instantiate_robot_sync_arena_content);
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
struct RobotSyncArenaLayout {
    version: String,
    scene_id: String,
    arena: RobotSyncArenaBounds,
    grid: RobotSyncArenaGrid,
    boundary: RobotSyncArenaBoundary,
    spawn_points: Vec<RobotSyncArenaSpawnPoint>,
    robots: Vec<RobotSyncArenaRobot>,
    colors: RobotSyncArenaColors,
}

impl RobotSyncArenaLayout {
    fn load_first_package_ron(layout_path: impl AsRef<str>) -> Result<Self, RobotLayoutLoadError> {
        let layout_path = layout_path.as_ref();
        let fs_path = first_package_layout_fs_path(layout_path)
            .ok_or_else(|| RobotLayoutLoadError::LayoutNotFound(layout_path.to_string()))?;

        let layout_source =
            fs::read_to_string(&fs_path).map_err(|source| RobotLayoutLoadError::ReadFailed {
                path: fs_path.clone(),
                source,
            })?;

        ron::from_str::<Self>(&layout_source).map_err(|source| RobotLayoutLoadError::ParseFailed {
            path: fs_path,
            source,
        })
    }

    fn is_scene_id_valid(&self) -> bool {
        self.scene_id == ROBOT_SYNC_ARENA_SCENE_ID
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
struct RobotSyncArenaBounds {
    width: f32,
    height: f32,
    #[serde(deserialize_with = "deserialize_f32_array_2")]
    half_extents: [f32; 2],
    #[serde(deserialize_with = "deserialize_f32_array_2")]
    min: [f32; 2],
    #[serde(deserialize_with = "deserialize_f32_array_2")]
    max: [f32; 2],
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
struct RobotSyncArenaGrid {
    spacing: f32,
    major_every: u32,
    #[serde(deserialize_with = "deserialize_f32_array_2")]
    origin: [f32; 2],
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
struct RobotSyncArenaBoundary {
    #[serde(deserialize_with = "deserialize_f32_array_2")]
    min: [f32; 2],
    #[serde(deserialize_with = "deserialize_f32_array_2")]
    max: [f32; 2],
    thickness: f32,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
struct RobotSyncArenaSpawnPoint {
    id: String,
    #[serde(deserialize_with = "deserialize_f32_array_2")]
    position: [f32; 2],
    facing_degrees: f32,
    tags: Vec<String>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
struct RobotSyncArenaRobot {
    id: String,
    spawn_point: String,
    radius: f32,
    #[serde(deserialize_with = "deserialize_f32_array_3")]
    color: [f32; 3],
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
struct RobotSyncArenaColors {
    #[serde(deserialize_with = "deserialize_f32_array_3")]
    background: [f32; 3],
    #[serde(deserialize_with = "deserialize_f32_array_3")]
    arena_fill: [f32; 3],
    #[serde(deserialize_with = "deserialize_f32_array_3")]
    grid_minor: [f32; 3],
    #[serde(deserialize_with = "deserialize_f32_array_3")]
    grid_major: [f32; 3],
    #[serde(deserialize_with = "deserialize_f32_array_3")]
    boundary: [f32; 3],
    #[serde(deserialize_with = "deserialize_f32_array_3")]
    spawn_a: [f32; 3],
    #[serde(deserialize_with = "deserialize_f32_array_3")]
    spawn_b: [f32; 3],
}

#[derive(Clone, Debug, Component, PartialEq, Eq)]
struct RobotSyncArenaContent {
    session_id: SceneSessionId,
}

#[derive(Clone, Copy, Debug, Component, PartialEq, Eq)]
enum RobotSyncArenaVisual {
    ArenaBase,
    FloorTile,
    Boundary,
    Grid,
    SpawnMarker,
    DirectionalLight,
}

#[derive(Debug)]
enum RobotLayoutLoadError {
    LayoutNotFound(String),
    ReadFailed {
        path: PathBuf,
        source: io::Error,
    },
    ParseFailed {
        path: PathBuf,
        source: ron::error::SpannedError,
    },
}

impl std::fmt::Display for RobotLayoutLoadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LayoutNotFound(path) => {
                write!(
                    formatter,
                    "robot sync arena layout was not found under assets: {path}"
                )
            }
            Self::ReadFailed { path, source } => {
                write!(
                    formatter,
                    "failed to read robot sync arena layout at {}: {source}",
                    path.display()
                )
            }
            Self::ParseFailed { path, source } => {
                write!(
                    formatter,
                    "failed to parse robot sync arena layout RON at {}: {source}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for RobotLayoutLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ReadFailed { source, .. } => Some(source),
            Self::ParseFailed { source, .. } => Some(source),
            Self::LayoutNotFound(_) => None,
        }
    }
}

fn instantiate_robot_sync_arena_content(
    mut commands: Commands,
    mut scene_events: MessageReader<SceneEvent>,
    runtime_roots: Query<(Entity, &SceneRuntimeRoot)>,
    existing_content: Query<&RobotSyncArenaContent>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut instantiated_sessions = Vec::new();

    for event in scene_events.read() {
        let SceneEvent::Entered(entered) = event else {
            continue;
        };

        if entered.scene_id.as_str() != ROBOT_SYNC_ARENA_SCENE_ID {
            continue;
        }

        if existing_content
            .iter()
            .any(|content| content.session_id == entered.session_id)
            || instantiated_sessions.contains(&entered.session_id)
        {
            continue;
        }

        let layout =
            match RobotSyncArenaLayout::load_first_package_ron(ROBOT_SYNC_ARENA_LAYOUT_PATH) {
                Ok(layout) => layout,
                Err(error) => {
                    warn!("{error}");
                    continue;
                }
            };

        if !layout.is_scene_id_valid() {
            warn!(
                "skipping robot sync arena content because layout scene_id `{}` does not match `{}`",
                layout.scene_id, ROBOT_SYNC_ARENA_SCENE_ID
            );
            continue;
        }

        let Some(runtime_root) =
            find_runtime_root_entity(&entered.session_id, runtime_roots.iter())
        else {
            warn!(
                "skipping robot sync arena content because session `{}` has no runtime root",
                entered.session_id
            );
            continue;
        };

        spawn_robot_sync_arena_content(
            &mut commands,
            runtime_root,
            &entered.session_id,
            &layout,
            &asset_server,
            &mut meshes,
            &mut materials,
        );
        instantiated_sessions.push(entered.session_id.clone());
    }
}

fn spawn_robot_sync_arena_content(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    layout: &RobotSyncArenaLayout,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> Entity {
    let content = commands
        .spawn((
            SceneOwned::new(session_id.clone()),
            RobotSyncArenaContent {
                session_id: session_id.clone(),
            },
            Transform::default(),
            Name::new(format!("RobotSyncArenaContent({session_id})")),
        ))
        .id();
    commands.entity(parent).add_child(content);
    spawn_robot_sync_arena_visuals(
        commands,
        content,
        session_id,
        layout,
        asset_server,
        meshes,
        materials,
    );
    content
}

fn spawn_robot_sync_arena_visuals(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    layout: &RobotSyncArenaLayout,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let arena_center = arena_world_center(&layout.arena.min, &layout.arena.max);
    spawn_robot_sync_arena_box(
        commands,
        parent,
        session_id,
        RobotSyncArenaVisual::ArenaBase,
        "RobotSyncArenaBase".to_string(),
        color_from_rgb(layout.colors.arena_fill),
        Vec3::new(
            robot_sync_axis_world_units_from_sync(layout.arena.width),
            ARENA_BASE_THICKNESS,
            robot_sync_axis_world_units_from_sync(layout.arena.height),
        ),
        Vec3::new(
            arena_center.x,
            ARENA_BASE_TOP_Y - ARENA_BASE_THICKNESS * 0.5,
            arena_center.y,
        ),
        meshes,
        materials,
    );

    spawn_robot_sync_arena_floor_tiles(commands, parent, session_id, layout, asset_server);
    spawn_robot_sync_arena_grid(commands, parent, session_id, layout, meshes, materials);
    spawn_robot_sync_arena_boundary(commands, parent, session_id, layout, meshes, materials);
    spawn_robot_sync_arena_spawn_markers(commands, parent, session_id, layout, meshes, materials);
    spawn_robot_sync_arena_directional_light(commands, parent, session_id);
}

fn spawn_robot_sync_arena_floor_tiles(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    layout: &RobotSyncArenaLayout,
    asset_server: &AssetServer,
) {
    let scene_handle = asset_server
        .load(GltfAssetLabel::Scene(0).from_asset(ROBOT_SYNC_ARENA_FLOOR_TILE_ASSET_PATH));

    for center in floor_tile_centers_for_bounds(
        arena_world_min(&layout.arena.min),
        arena_world_max(&layout.arena.max),
    ) {
        let entity = commands
            .spawn((
                BevySceneRoot(scene_handle.clone()),
                Transform {
                    translation: Vec3::new(center.x, FLOOR_TILE_Y, center.y),
                    scale: Vec3::new(FLOOR_TILE_SCALE_XZ, FLOOR_TILE_SCALE_Y, FLOOR_TILE_SCALE_XZ),
                    ..default()
                },
                SceneOwned::new(session_id.clone()),
                RobotSyncArenaContent {
                    session_id: session_id.clone(),
                },
                RobotSyncArenaVisual::FloorTile,
                Name::new(format!(
                    "RobotSyncArenaFloorTile({:.0},{:.0})",
                    center.x, center.y
                )),
            ))
            .id();
        commands.entity(parent).add_child(entity);
    }
}

fn spawn_robot_sync_arena_grid(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    layout: &RobotSyncArenaLayout,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let arena_world_min = arena_world_min(&layout.arena.min);
    let arena_world_max = arena_world_max(&layout.arena.max);
    let arena_width = arena_world_max.x - arena_world_min.x;
    let arena_height = arena_world_max.y - arena_world_min.y;
    let arena_center = arena_world_center(&layout.arena.min, &layout.arena.max);
    let grid_spacing = robot_sync_axis_world_units_from_sync(layout.grid.spacing);
    let grid_origin =
        robot_sync_world_position_from_sync(layout.grid.origin[0], layout.grid.origin[1]);
    let minor_color = color_from_rgb_alpha(layout.colors.grid_minor, 0.46);
    let major_color = color_from_rgb_alpha(layout.colors.grid_major, 0.68);

    for x in grid_line_positions(
        arena_world_min.x,
        arena_world_max.x,
        grid_spacing,
        grid_origin.x,
    ) {
        let major = is_major_grid_line(x, grid_origin.x, grid_spacing, layout.grid.major_every);
        let thickness = robot_sync_axis_world_units_from_sync(if major { 2.0 } else { 1.0 });
        let color = if major { major_color } else { minor_color };
        let kind = if major { "major" } else { "minor" };
        spawn_robot_sync_arena_box(
            commands,
            parent,
            session_id,
            RobotSyncArenaVisual::Grid,
            format!("RobotSyncArenaGrid({kind}:vertical:{x:.0})"),
            color,
            Vec3::new(thickness, GRID_LINE_HEIGHT, arena_height),
            Vec3::new(x, GRID_LINE_Y, arena_center.y),
            meshes,
            materials,
        );
    }

    for y in grid_line_positions(
        arena_world_min.y,
        arena_world_max.y,
        grid_spacing,
        grid_origin.y,
    ) {
        let major = is_major_grid_line(y, grid_origin.y, grid_spacing, layout.grid.major_every);
        let thickness = robot_sync_axis_world_units_from_sync(if major { 2.0 } else { 1.0 });
        let color = if major { major_color } else { minor_color };
        let kind = if major { "major" } else { "minor" };
        spawn_robot_sync_arena_box(
            commands,
            parent,
            session_id,
            RobotSyncArenaVisual::Grid,
            format!("RobotSyncArenaGrid({kind}:horizontal:{y:.0})"),
            color,
            Vec3::new(arena_width, GRID_LINE_HEIGHT, thickness),
            Vec3::new(arena_center.x, GRID_LINE_Y, y),
            meshes,
            materials,
        );
    }
}

fn spawn_robot_sync_arena_boundary(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    layout: &RobotSyncArenaLayout,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let min = arena_world_min(&layout.boundary.min);
    let max = arena_world_max(&layout.boundary.max);
    let thickness = robot_sync_axis_world_units_from_sync(layout.boundary.thickness.max(1.0));
    let width = max.x - min.x;
    let height = max.y - min.y;
    let center_x = (min.x + max.x) * 0.5;
    let center_y = (min.y + max.y) * 0.5;
    let color = color_from_rgb(layout.colors.boundary);

    let boundary_specs = [
        (
            "left",
            Vec3::new(thickness, BOUNDARY_WALL_HEIGHT, height),
            Vec3::new(min.x + thickness * 0.5, BOUNDARY_WALL_Y, center_y),
        ),
        (
            "right",
            Vec3::new(thickness, BOUNDARY_WALL_HEIGHT, height),
            Vec3::new(max.x - thickness * 0.5, BOUNDARY_WALL_Y, center_y),
        ),
        (
            "bottom",
            Vec3::new(width, BOUNDARY_WALL_HEIGHT, thickness),
            Vec3::new(center_x, BOUNDARY_WALL_Y, min.y + thickness * 0.5),
        ),
        (
            "top",
            Vec3::new(width, BOUNDARY_WALL_HEIGHT, thickness),
            Vec3::new(center_x, BOUNDARY_WALL_Y, max.y - thickness * 0.5),
        ),
    ];

    for (side, size, translation) in boundary_specs {
        spawn_robot_sync_arena_box(
            commands,
            parent,
            session_id,
            RobotSyncArenaVisual::Boundary,
            format!("RobotSyncArenaBoundary({side})"),
            color,
            size,
            translation,
            meshes,
            materials,
        );
    }
}

fn spawn_robot_sync_arena_spawn_markers(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    layout: &RobotSyncArenaLayout,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    for (index, spawn_point) in layout.spawn_points.iter().enumerate() {
        let position =
            robot_sync_world_position_from_sync(spawn_point.position[0], spawn_point.position[1]);
        spawn_robot_sync_arena_spawn_marker(
            commands,
            parent,
            session_id,
            format!("RobotSyncArenaSpawnMarker({})", spawn_point.id),
            Vec3::new(position.x, SPAWN_MARKER_Y + index as f32 * 0.02, position.y),
            meshes,
            materials,
        );
    }
}

fn spawn_robot_sync_arena_spawn_marker(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    name: String,
    translation: Vec3,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> Entity {
    let entity = commands
        .spawn((
            Mesh3d(meshes.add(Torus::new(
                SPAWN_MARKER_INNER_RADIUS,
                SPAWN_MARKER_OUTER_RADIUS,
            ))),
            MeshMaterial3d(materials.add(standard_material_from_color(SPAWN_MARKER_COLOR))),
            Transform::from_translation(translation).with_scale(Vec3::new(
                1.0,
                SPAWN_MARKER_VERTICAL_SCALE,
                1.0,
            )),
            SceneOwned::new(session_id.clone()),
            RobotSyncArenaContent {
                session_id: session_id.clone(),
            },
            RobotSyncArenaVisual::SpawnMarker,
            Name::new(name),
        ))
        .id();
    commands.entity(parent).add_child(entity);
    entity
}

fn spawn_robot_sync_arena_box(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    visual: RobotSyncArenaVisual,
    name: String,
    color: Color,
    size: Vec3,
    translation: Vec3,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> Entity {
    let entity = commands
        .spawn((
            Mesh3d(meshes.add(Cuboid::new(size.x, size.y, size.z))),
            MeshMaterial3d(materials.add(standard_material_from_color(color))),
            Transform::from_translation(translation),
            SceneOwned::new(session_id.clone()),
            RobotSyncArenaContent {
                session_id: session_id.clone(),
            },
            visual,
            Name::new(name),
        ))
        .id();
    commands.entity(parent).add_child(entity);
    entity
}

fn spawn_robot_sync_arena_directional_light(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
) -> Entity {
    let entity = commands
        .spawn((
            DirectionalLight {
                color: Color::srgb(0.92, 0.96, 1.0),
                illuminance: 4800.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_rotation(Quat::from_euler(
                EulerRot::XYZ,
                -55.0_f32.to_radians(),
                -35.0_f32.to_radians(),
                0.0,
            )),
            SceneOwned::new(session_id.clone()),
            RobotSyncArenaContent {
                session_id: session_id.clone(),
            },
            RobotSyncArenaVisual::DirectionalLight,
            Name::new("RobotSyncArenaDirectionalLight"),
        ))
        .id();
    commands.entity(parent).add_child(entity);
    entity
}

fn arena_center(min: &[f32; 2], max: &[f32; 2]) -> Vec2 {
    Vec2::new((min[0] + max[0]) * 0.5, (min[1] + max[1]) * 0.5)
}

fn arena_world_min(min: &[f32; 2]) -> Vec2 {
    robot_sync_world_position_from_sync(min[0], min[1])
}

fn arena_world_max(max: &[f32; 2]) -> Vec2 {
    robot_sync_world_position_from_sync(max[0], max[1])
}

fn arena_world_center(min: &[f32; 2], max: &[f32; 2]) -> Vec2 {
    let center = arena_center(min, max);
    robot_sync_world_position_from_sync(center.x, center.y)
}

const fn spawn_marker_vertical_half_extent() -> f32 {
    (SPAWN_MARKER_OUTER_RADIUS - SPAWN_MARKER_INNER_RADIUS) * 0.5 * SPAWN_MARKER_VERTICAL_SCALE
}

fn floor_tile_world_half_extent() -> f32 {
    FLOOR_TILE_SOURCE_HALF_EXTENT * FLOOR_TILE_SCALE_XZ
}

fn floor_tile_world_spacing() -> f32 {
    floor_tile_world_half_extent() * 2.0
}

fn floor_tile_centers_for_bounds(min: Vec2, max: Vec2) -> Vec<Vec2> {
    let x_centers = floor_tile_axis_centers(min.x, max.x);
    let z_centers = floor_tile_axis_centers(min.y, max.y);
    z_centers
        .into_iter()
        .flat_map(|z| x_centers.iter().copied().map(move |x| Vec2::new(x, z)))
        .collect()
}

fn floor_tile_axis_centers(min: f32, max: f32) -> Vec<f32> {
    if min > max {
        return Vec::new();
    }

    let span = max - min;
    let spacing = floor_tile_world_spacing();
    let count = (span / spacing).ceil().max(1.0) as usize;
    let center = (min + max) * 0.5;
    let first_offset = -((count - 1) as f32) * spacing * 0.5;
    (0..count)
        .map(|index| center + first_offset + index as f32 * spacing)
        .collect()
}

#[cfg(test)]
fn floor_tile_coverage(centers: &[Vec2]) -> Option<([f32; 2], [f32; 2])> {
    let half_extent = floor_tile_world_half_extent();
    let first = centers.first()?;
    let mut min = [first.x - half_extent, first.y - half_extent];
    let mut max = [first.x + half_extent, first.y + half_extent];

    for center in centers.iter().skip(1) {
        min[0] = min[0].min(center.x - half_extent);
        min[1] = min[1].min(center.y - half_extent);
        max[0] = max[0].max(center.x + half_extent);
        max[1] = max[1].max(center.y + half_extent);
    }

    Some((min, max))
}

fn grid_line_positions(min: f32, max: f32, spacing: f32, origin: f32) -> Vec<f32> {
    if spacing <= 0.0 || min > max {
        return Vec::new();
    }

    let first_index = ((min - origin) / spacing).ceil() as i32;
    let last_index = ((max - origin) / spacing).floor() as i32;
    (first_index..=last_index)
        .map(|index| origin + index as f32 * spacing)
        .collect()
}

fn is_major_grid_line(position: f32, origin: f32, spacing: f32, major_every: u32) -> bool {
    if spacing <= 0.0 || major_every == 0 {
        return false;
    }

    let grid_index = ((position - origin) / spacing).round() as i32;
    grid_index % major_every as i32 == 0
}

fn color_from_rgb(rgb: [f32; 3]) -> Color {
    Color::srgb(rgb[0], rgb[1], rgb[2])
}

fn color_from_rgb_alpha(rgb: [f32; 3], alpha: f32) -> Color {
    Color::srgba(rgb[0], rgb[1], rgb[2], alpha)
}

fn standard_material_from_color(color: Color) -> StandardMaterial {
    let alpha = color.to_srgba().alpha;
    StandardMaterial {
        base_color: color,
        perceptual_roughness: 0.95,
        alpha_mode: if alpha < 1.0 {
            AlphaMode::Blend
        } else {
            AlphaMode::Opaque
        },
        ..default()
    }
}

fn find_runtime_root_entity<'runtime>(
    session_id: &SceneSessionId,
    runtime_roots: impl IntoIterator<Item = (Entity, &'runtime SceneRuntimeRoot)>,
) -> Option<Entity> {
    runtime_roots
        .into_iter()
        .find(|(_, root)| root.is_session(session_id))
        .map(|(entity, _)| entity)
}

fn deserialize_f32_array_2<'de, D>(deserializer: D) -> Result<[f32; 2], D::Error>
where
    D: Deserializer<'de>,
{
    let values = Vec::<f32>::deserialize(deserializer)?;
    match values.as_slice() {
        [x, y] => Ok([*x, *y]),
        _ => Err(de::Error::invalid_length(
            values.len(),
            &"exactly two f32 values",
        )),
    }
}

fn deserialize_f32_array_3<'de, D>(deserializer: D) -> Result<[f32; 3], D::Error>
where
    D: Deserializer<'de>,
{
    let values = Vec::<f32>::deserialize(deserializer)?;
    match values.as_slice() {
        [r, g, b] => Ok([*r, *g, *b]),
        _ => Err(de::Error::invalid_length(
            values.len(),
            &"exactly three f32 values",
        )),
    }
}

fn first_package_layout_fs_path(layout_path: &str) -> Option<PathBuf> {
    first_package_asset_root_candidates()
        .into_iter()
        .map(|root| root.join(Path::new(layout_path)))
        .find(|candidate| candidate.is_file())
}

fn first_package_asset_root_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(current_dir) = std::env::current_dir() {
        candidates.push(current_dir.join("assets"));
        candidates.push(current_dir.join("project").join("assets"));
    }
    candidates.push(PathBuf::from("assets"));
    candidates.push(PathBuf::from("project").join("assets"));
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::scene::prelude::{
        SceneCameraMode, SceneCameraProjection, SceneEntered, SceneManifest, spawn_scene_root,
        spawn_scene_runtime_root,
    };
    use bevy::mesh::VertexAttributeValues;

    const EXPECTED_MESH_VISUALS: usize = 29;
    const EXPECTED_FLOOR_TILE_VISUALS: usize = 1;
    const EXPECTED_TOTAL_VISUALS: usize = EXPECTED_MESH_VISUALS + EXPECTED_FLOOR_TILE_VISUALS + 1;

    fn app_with_robot_sync_arena_system() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<bevy::scene::Scene>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .add_message::<SceneEvent>()
            .add_systems(Update, instantiate_robot_sync_arena_content);
        app
    }

    #[test]
    fn load_robot_sync_arena_manifest_from_first_package_assets() {
        let manifest =
            SceneManifest::load_first_package_ron(ROBOT_SYNC_ARENA_SCENE_MANIFEST_PATH).unwrap();

        assert_eq!(manifest.version, "1");
        assert_eq!(manifest.scene_id.as_str(), ROBOT_SYNC_ARENA_SCENE_ID);
        let camera = manifest.entry.camera.as_ref().unwrap();
        let camera_config = camera.config();
        assert_eq!(camera_config.mode, SceneCameraMode::Fixed3d);
        assert!(camera_config.is_3d());
        assert_eq!(
            camera_config.transform.translation,
            Vec3::new(0.0, 420.0, 520.0)
        );
        let SceneCameraProjection::Perspective3d {
            fov_y_radians,
            near,
            far,
        } = camera_config.projection
        else {
            panic!("robot sync arena camera should use a perspective 3D projection");
        };
        assert!((fov_y_radians - 0.78).abs() < f32::EPSILON);
        assert!((near - 0.1).abs() < f32::EPSILON);
        assert!((far - 2000.0).abs() < f32::EPSILON);
        assert_eq!(
            camera_config.target.as_ref().map(|target| target.as_str()),
            Some("anchor.camera_target")
        );
    }

    #[test]
    fn load_robot_sync_arena_layout_from_first_package_assets() {
        let layout =
            RobotSyncArenaLayout::load_first_package_ron(ROBOT_SYNC_ARENA_LAYOUT_PATH).unwrap();

        assert_eq!(layout.version, "1");
        assert_eq!(layout.scene_id, ROBOT_SYNC_ARENA_SCENE_ID);
        assert_eq!(layout.arena.width, 500.0);
        assert_eq!(layout.arena.height, 500.0);
        assert_eq!(layout.arena.half_extents, [250.0, 250.0]);
        assert_eq!(layout.arena.min, [-250.0, -250.0]);
        assert_eq!(layout.arena.max, [250.0, 250.0]);
        assert_eq!(layout.grid.spacing, 50.0);
        assert_eq!(layout.grid.major_every, 5);
        assert_eq!(layout.grid.origin, [0.0, 0.0]);
        assert_eq!(layout.boundary.min, [-250.0, -250.0]);
        assert_eq!(layout.boundary.max, [250.0, 250.0]);
        assert_eq!(layout.boundary.thickness, 4.0);
        assert!(layout.spawn_points.len() >= 2);
        assert_eq!(layout.spawn_points[0].position, [-120.0, 0.0]);
        assert_eq!(layout.spawn_points[1].position, [120.0, 0.0]);
        assert!(layout.robots.len() >= 2);
        assert_eq!(layout.colors.boundary, [0.80, 0.86, 0.92]);
    }

    #[test]
    fn robot_sync_floor_tiles_cover_arena_bounds() {
        let layout =
            RobotSyncArenaLayout::load_first_package_ron(ROBOT_SYNC_ARENA_LAYOUT_PATH).unwrap();

        let world_min = arena_world_min(&layout.arena.min);
        let world_max = arena_world_max(&layout.arena.max);
        let centers = floor_tile_centers_for_bounds(world_min, world_max);
        assert_eq!(centers.len(), 1);
        assert_eq!(floor_tile_world_half_extent(), 100.0);
        assert_eq!(floor_tile_world_spacing(), 200.0);
        assert_eq!(centers, vec![Vec2::ZERO]);

        let (coverage_min, coverage_max) =
            floor_tile_coverage(&centers).expect("floor tile coverage should exist");
        assert!(coverage_min[0] <= world_min.x);
        assert!(coverage_min[1] <= world_min.y);
        assert!(coverage_max[0] >= world_max.x);
        assert!(coverage_max[1] >= world_max.y);
        assert_eq!(world_min, Vec2::new(-25.0, -25.0));
        assert_eq!(world_max, Vec2::new(25.0, 25.0));
        assert_eq!(coverage_min, [-100.0, -100.0]);
        assert_eq!(coverage_max, [100.0, 100.0]);
    }

    #[test]
    fn entered_robot_sync_arena_spawns_content_under_runtime_root() {
        let mut app = app_with_robot_sync_arena_system();

        let session_id = SceneSessionId::from("robot-sync-session");
        let scene_root = spawn_scene_root(
            &mut app.world_mut().commands(),
            &ROBOT_SYNC_ARENA_SCENE_ID.into(),
            &session_id,
        );
        let runtime_root =
            spawn_scene_runtime_root(&mut app.world_mut().commands(), scene_root, &session_id);
        app.update();

        app.world_mut()
            .write_message(SceneEvent::Entered(SceneEntered {
                scene_id: ROBOT_SYNC_ARENA_SCENE_ID.into(),
                session_id: session_id.clone(),
                content_version: None,
            }));
        app.update();

        let mut content = app.world_mut().query_filtered::<(
            Entity,
            &ChildOf,
            &SceneOwned,
            &RobotSyncArenaContent,
            &Transform,
            &Name,
        ), Without<RobotSyncArenaVisual>>();
        let content_entities = content.iter(app.world()).collect::<Vec<_>>();
        assert_eq!(content_entities.len(), 1);

        let (content_entity, parent, owned, content, transform, name) = content_entities[0];
        assert_eq!(parent.parent(), runtime_root);
        assert_eq!(owned.session_id, session_id);
        assert_eq!(content.session_id, session_id);
        assert_eq!(transform, &Transform::default());
        assert_eq!(name.as_str(), "RobotSyncArenaContent(robot-sync-session)");

        let mut visuals = app.world_mut().query::<(
            &ChildOf,
            &SceneOwned,
            &RobotSyncArenaContent,
            &RobotSyncArenaVisual,
            &Name,
        )>();
        let visual_entities = visuals.iter(app.world()).collect::<Vec<_>>();
        assert_eq!(visual_entities.len(), EXPECTED_TOTAL_VISUALS);

        let mut arena_base_count = 0;
        let mut floor_tile_count = 0;
        let mut boundary_count = 0;
        let mut grid_count = 0;
        let mut spawn_marker_count = 0;
        let mut directional_light_count = 0;
        for (parent, owned, content, visual, name) in visual_entities {
            assert_eq!(parent.parent(), content_entity);
            assert_eq!(owned.session_id, session_id);
            assert_eq!(content.session_id, session_id);
            assert!(name.as_str().starts_with("RobotSyncArena"));
            match visual {
                RobotSyncArenaVisual::ArenaBase => arena_base_count += 1,
                RobotSyncArenaVisual::FloorTile => floor_tile_count += 1,
                RobotSyncArenaVisual::Boundary => boundary_count += 1,
                RobotSyncArenaVisual::Grid => grid_count += 1,
                RobotSyncArenaVisual::SpawnMarker => spawn_marker_count += 1,
                RobotSyncArenaVisual::DirectionalLight => directional_light_count += 1,
            }
        }
        assert_eq!(arena_base_count, 1);
        assert_eq!(floor_tile_count, 1);
        assert_eq!(boundary_count, 4);
        assert_eq!(grid_count, 22);
        assert_eq!(spawn_marker_count, 2);
        assert_eq!(directional_light_count, 1);

        let mut floor_tiles = app.world_mut().query::<(
            &BevySceneRoot,
            &Transform,
            &ChildOf,
            &SceneOwned,
            &RobotSyncArenaContent,
            &RobotSyncArenaVisual,
            &Name,
        )>();
        let floor_tile_entities = floor_tiles.iter(app.world()).collect::<Vec<_>>();
        assert_eq!(floor_tile_entities.len(), 1);
        assert!(floor_tile_entities.iter().all(
            |(_, transform, parent, owned, content, visual, name)| {
                **visual == RobotSyncArenaVisual::FloorTile
                    && parent.parent() == content_entity
                    && owned.session_id == session_id
                    && content.session_id == session_id
                    && name.as_str().starts_with("RobotSyncArenaFloorTile(")
                    && transform.translation.y == FLOOR_TILE_Y
                    && transform.scale
                        == Vec3::new(FLOOR_TILE_SCALE_XZ, FLOOR_TILE_SCALE_Y, FLOOR_TILE_SCALE_XZ)
            }
        ));
        assert!(
            floor_tile_entities.iter().any(|(_, _, _, _, _, _, name)| {
                name.as_str() == "RobotSyncArenaFloorTile(0,0)"
            })
        );

        let (arena_base_translation, arena_base_mesh) = {
            let mut arena_base = app
                .world_mut()
                .query::<(&RobotSyncArenaVisual, &Transform, &Mesh3d, &Name)>();
            let (_, arena_base_transform, arena_base_mesh, _) = arena_base
                .iter(app.world())
                .find(|(visual, _, _, name)| {
                    **visual == RobotSyncArenaVisual::ArenaBase
                        && name.as_str() == "RobotSyncArenaBase"
                })
                .expect("arena base visual should exist");
            (arena_base_transform.translation, arena_base_mesh.0.clone())
        };
        assert_eq!(
            arena_base_translation,
            Vec3::new(0.0, ARENA_BASE_TOP_Y - ARENA_BASE_THICKNESS * 0.5, 0.0)
        );
        assert_eq!(
            mesh_position_size(
                app.world()
                    .resource::<Assets<Mesh>>()
                    .get(&arena_base_mesh)
                    .unwrap()
            ),
            Vec3::new(50.0, ARENA_BASE_THICKNESS, 50.0)
        );

        let (vertical_grid_x, minor_grid_mesh, major_grid_mesh) = {
            let mut grid_visuals =
                app.world_mut()
                    .query::<(&RobotSyncArenaVisual, &Transform, &Mesh3d, &Name)>();
            let grid_entries = grid_visuals
                .iter(app.world())
                .filter(|(visual, _, _, _)| **visual == RobotSyncArenaVisual::Grid)
                .collect::<Vec<_>>();
            assert_eq!(grid_entries.len(), 22);
            let vertical_grid_x = grid_entries
                .iter()
                .filter(|(_, _, _, name)| name.as_str().contains(":vertical:"))
                .map(|(_, transform, _, _)| transform.translation.x)
                .collect::<Vec<_>>();
            let (_, _, minor_grid_mesh, _) = grid_entries
                .iter()
                .find(|(_, _, _, name)| name.as_str() == "RobotSyncArenaGrid(minor:vertical:-20)")
                .expect("minor vertical grid line should exist");
            let (_, _, major_grid_mesh, _) = grid_entries
                .iter()
                .find(|(_, _, _, name)| name.as_str() == "RobotSyncArenaGrid(major:vertical:-25)")
                .expect("major vertical grid line should exist");
            (
                vertical_grid_x,
                minor_grid_mesh.0.clone(),
                major_grid_mesh.0.clone(),
            )
        };
        assert_eq!(
            vertical_grid_x,
            vec![
                -25.0, -20.0, -15.0, -10.0, -5.0, 0.0, 5.0, 10.0, 15.0, 20.0, 25.0
            ]
        );
        assert_eq!(
            mesh_position_size(
                app.world()
                    .resource::<Assets<Mesh>>()
                    .get(&minor_grid_mesh)
                    .unwrap()
            ),
            Vec3::new(0.1, GRID_LINE_HEIGHT, 50.0)
        );
        assert_eq!(
            mesh_position_size(
                app.world()
                    .resource::<Assets<Mesh>>()
                    .get(&major_grid_mesh)
                    .unwrap()
            ),
            Vec3::new(0.2, GRID_LINE_HEIGHT, 50.0)
        );

        let mut mesh_visuals = app.world_mut().query::<(
            &Mesh3d,
            &MeshMaterial3d<StandardMaterial>,
            &RobotSyncArenaVisual,
            &SceneOwned,
        )>();
        let mesh_visual_entities = mesh_visuals.iter(app.world()).collect::<Vec<_>>();
        assert_eq!(mesh_visual_entities.len(), EXPECTED_MESH_VISUALS);
        assert!(mesh_visual_entities.iter().all(|(_, _, visual, owned)| {
            **visual != RobotSyncArenaVisual::DirectionalLight && owned.session_id == session_id
        }));

        let mut boundaries = app
            .world_mut()
            .query::<(&RobotSyncArenaVisual, &Transform, &Name)>();
        let boundary_entities = boundaries
            .iter(app.world())
            .filter(|(visual, _, _)| **visual == RobotSyncArenaVisual::Boundary)
            .collect::<Vec<_>>();
        assert_eq!(boundary_entities.len(), 4);
        assert!(boundary_entities.iter().all(|(_, transform, name)| {
            name.as_str().starts_with("RobotSyncArenaBoundary(")
                && transform.translation.y - BOUNDARY_WALL_HEIGHT * 0.5 > FLOOR_TILE_TOP_Y
        }));
        let (left_boundary_translation, left_boundary_mesh) = {
            let mut boundary_meshes =
                app.world_mut()
                    .query::<(&RobotSyncArenaVisual, &Transform, &Mesh3d, &Name)>();
            let (_, left_boundary_transform, left_boundary_mesh, _) = boundary_meshes
                .iter(app.world())
                .find(|(visual, _, _, name)| {
                    **visual == RobotSyncArenaVisual::Boundary
                        && name.as_str() == "RobotSyncArenaBoundary(left)"
                })
                .expect("left boundary should exist");
            (
                left_boundary_transform.translation,
                left_boundary_mesh.0.clone(),
            )
        };
        assert_eq!(
            left_boundary_translation,
            Vec3::new(-24.8, BOUNDARY_WALL_Y, 0.0)
        );
        assert_eq!(
            mesh_position_size(
                app.world()
                    .resource::<Assets<Mesh>>()
                    .get(&left_boundary_mesh)
                    .unwrap()
            ),
            Vec3::new(0.4, BOUNDARY_WALL_HEIGHT, 50.0)
        );

        let mut lights = app.world_mut().query::<(
            &DirectionalLight,
            &RobotSyncArenaVisual,
            &ChildOf,
            &SceneOwned,
            &RobotSyncArenaContent,
            &Name,
        )>();
        let light_entities = lights.iter(app.world()).collect::<Vec<_>>();
        assert_eq!(light_entities.len(), 1);
        let (light, visual, light_parent, light_owned, light_content, light_name) =
            light_entities[0];
        assert_eq!(*visual, RobotSyncArenaVisual::DirectionalLight);
        assert_eq!(light_parent.parent(), content_entity);
        assert_eq!(light_owned.session_id, session_id);
        assert_eq!(light_content.session_id, session_id);
        assert_eq!(light_name.as_str(), "RobotSyncArenaDirectionalLight");
        assert_eq!(light.illuminance, 4800.0);

        let mut robot_a_spawn = app.world_mut().query::<(
            &Name,
            &RobotSyncArenaVisual,
            &Transform,
            &ChildOf,
            &SceneOwned,
            &RobotSyncArenaContent,
        )>();
        let robot_a_spawn = robot_a_spawn
            .iter(app.world())
            .find(|(name, visual, _, _, _, _)| {
                **visual == RobotSyncArenaVisual::SpawnMarker
                    && name.as_str() == "RobotSyncArenaSpawnMarker(spawn.robot_a)"
            })
            .expect("spawn.robot_a marker should be generated");
        assert_eq!(robot_a_spawn.2.translation.x, -12.0);
        assert!(robot_a_spawn.2.translation.x < 0.0);
        assert!(robot_a_spawn.2.translation.y > FLOOR_TILE_TOP_Y);
        assert!(robot_a_spawn.2.translation.y > GRID_LINE_Y + GRID_LINE_HEIGHT * 0.5);
        assert!(
            robot_a_spawn.2.translation.y - spawn_marker_vertical_half_extent()
                > GRID_LINE_Y + GRID_LINE_HEIGHT * 0.5
        );
        assert_eq!(robot_a_spawn.2.translation.y, SPAWN_MARKER_Y);
        assert_eq!(robot_a_spawn.2.translation.z, 0.0);
        assert_eq!(robot_a_spawn.2.rotation, Quat::IDENTITY);
        assert_eq!(robot_a_spawn.3.parent(), content_entity);
        assert_eq!(robot_a_spawn.4.session_id, session_id);
        assert_eq!(robot_a_spawn.5.session_id, session_id);

        let mut robot_b_spawn = app
            .world_mut()
            .query::<(&Name, &RobotSyncArenaVisual, &Transform)>();
        let robot_b_spawn = robot_b_spawn
            .iter(app.world())
            .find(|(name, visual, _)| {
                **visual == RobotSyncArenaVisual::SpawnMarker
                    && name.as_str() == "RobotSyncArenaSpawnMarker(spawn.robot_b)"
            })
            .expect("spawn.robot_b marker should be generated");
        assert_eq!(robot_b_spawn.2.translation.x, 12.0);
        assert!(robot_b_spawn.2.translation.x > 0.0);
        assert!(robot_b_spawn.2.translation.y > FLOOR_TILE_TOP_Y);
        assert!(robot_b_spawn.2.translation.y > GRID_LINE_Y + GRID_LINE_HEIGHT * 0.5);
        assert!(
            robot_b_spawn.2.translation.y - spawn_marker_vertical_half_extent()
                > GRID_LINE_Y + GRID_LINE_HEIGHT * 0.5
        );
        assert_eq!(robot_b_spawn.2.translation.z, 0.0);

        let mut sprites = app.world_mut().query_filtered::<Entity, With<Sprite>>();
        assert_eq!(sprites.iter(app.world()).count(), 0);
        let mut mesh2d = app.world_mut().query_filtered::<Entity, With<Mesh2d>>();
        assert_eq!(mesh2d.iter(app.world()).count(), 0);
    }

    fn mesh_position_size(mesh: &Mesh) -> Vec3 {
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("mesh should have f32x3 positions");
        };
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for position in positions {
            let position = Vec3::from(*position);
            min = min.min(position);
            max = max.max(position);
        }
        max - min
    }

    #[test]
    fn duplicate_enter_events_for_same_session_do_not_duplicate_content() {
        let mut app = app_with_robot_sync_arena_system();

        let session_id = SceneSessionId::from("robot-sync-session");
        let scene_root = spawn_scene_root(
            &mut app.world_mut().commands(),
            &ROBOT_SYNC_ARENA_SCENE_ID.into(),
            &session_id,
        );
        spawn_scene_runtime_root(&mut app.world_mut().commands(), scene_root, &session_id);
        app.update();

        app.world_mut()
            .write_message(SceneEvent::Entered(SceneEntered {
                scene_id: ROBOT_SYNC_ARENA_SCENE_ID.into(),
                session_id: session_id.clone(),
                content_version: None,
            }));
        app.world_mut()
            .write_message(SceneEvent::Entered(SceneEntered {
                scene_id: ROBOT_SYNC_ARENA_SCENE_ID.into(),
                session_id: session_id.clone(),
                content_version: None,
            }));
        app.update();

        let mut content = app
            .world_mut()
            .query_filtered::<&RobotSyncArenaContent, Without<RobotSyncArenaVisual>>();
        let content_sessions = content
            .iter(app.world())
            .filter(|content| content.session_id == session_id)
            .count();
        assert_eq!(content_sessions, 1);

        let mut visuals = app
            .world_mut()
            .query_filtered::<&RobotSyncArenaContent, With<RobotSyncArenaVisual>>();
        let visual_sessions = visuals
            .iter(app.world())
            .filter(|content| content.session_id == session_id)
            .count();
        assert_eq!(visual_sessions, EXPECTED_TOTAL_VISUALS);
    }
}
