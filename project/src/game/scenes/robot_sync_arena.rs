use bevy::prelude::*;
use serde::{Deserialize, Deserializer, de};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

use crate::framework::scene::prelude::{SceneEvent, SceneOwned, SceneRuntimeRoot, SceneSessionId};

pub(in crate::game) const ROBOT_SYNC_ARENA_SCENE_ID: &str = "arena.robot_sync";
const ROBOT_SYNC_ARENA_LAYOUT_PATH: &str = "scenes/robot_sync_arena/layout.ron";

pub(super) struct RobotSyncArenaPlugin;

impl Plugin for RobotSyncArenaPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, instantiate_robot_sync_arena_content);
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
    ArenaFill,
    Boundary,
    Grid,
    SpawnMarker,
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

        spawn_robot_sync_arena_content(&mut commands, runtime_root, &entered.session_id, &layout);
        instantiated_sessions.push(entered.session_id.clone());
    }
}

fn spawn_robot_sync_arena_content(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    layout: &RobotSyncArenaLayout,
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
    spawn_robot_sync_arena_visuals(commands, content, session_id, layout);
    content
}

fn spawn_robot_sync_arena_visuals(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    layout: &RobotSyncArenaLayout,
) {
    let arena_center = arena_center(&layout.arena.min, &layout.arena.max);
    spawn_robot_sync_arena_rect(
        commands,
        parent,
        session_id,
        RobotSyncArenaVisual::ArenaFill,
        "RobotSyncArenaFill".to_string(),
        color_from_rgb(layout.colors.arena_fill),
        Vec2::new(layout.arena.width, layout.arena.height),
        Vec3::new(arena_center.x, arena_center.y, 0.0),
    );

    spawn_robot_sync_arena_grid(commands, parent, session_id, layout);
    spawn_robot_sync_arena_boundary(commands, parent, session_id, layout);
    spawn_robot_sync_arena_spawn_markers(commands, parent, session_id, layout);
}

fn spawn_robot_sync_arena_grid(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    layout: &RobotSyncArenaLayout,
) {
    let arena_width = layout.arena.max[0] - layout.arena.min[0];
    let arena_height = layout.arena.max[1] - layout.arena.min[1];
    let arena_center = arena_center(&layout.arena.min, &layout.arena.max);
    let minor_color = color_from_rgb_alpha(layout.colors.grid_minor, 0.46);
    let major_color = color_from_rgb_alpha(layout.colors.grid_major, 0.68);

    for x in grid_line_positions(
        layout.arena.min[0],
        layout.arena.max[0],
        layout.grid.spacing,
        layout.grid.origin[0],
    ) {
        let major = is_major_grid_line(
            x,
            layout.grid.origin[0],
            layout.grid.spacing,
            layout.grid.major_every,
        );
        let thickness = if major { 2.0 } else { 1.0 };
        let color = if major { major_color } else { minor_color };
        let kind = if major { "major" } else { "minor" };
        spawn_robot_sync_arena_rect(
            commands,
            parent,
            session_id,
            RobotSyncArenaVisual::Grid,
            format!("RobotSyncArenaGrid({kind}:vertical:{x:.0})"),
            color,
            Vec2::new(thickness, arena_height),
            Vec3::new(x, arena_center.y, 0.1),
        );
    }

    for y in grid_line_positions(
        layout.arena.min[1],
        layout.arena.max[1],
        layout.grid.spacing,
        layout.grid.origin[1],
    ) {
        let major = is_major_grid_line(
            y,
            layout.grid.origin[1],
            layout.grid.spacing,
            layout.grid.major_every,
        );
        let thickness = if major { 2.0 } else { 1.0 };
        let color = if major { major_color } else { minor_color };
        let kind = if major { "major" } else { "minor" };
        spawn_robot_sync_arena_rect(
            commands,
            parent,
            session_id,
            RobotSyncArenaVisual::Grid,
            format!("RobotSyncArenaGrid({kind}:horizontal:{y:.0})"),
            color,
            Vec2::new(arena_width, thickness),
            Vec3::new(arena_center.x, y, 0.1),
        );
    }
}

fn spawn_robot_sync_arena_boundary(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    layout: &RobotSyncArenaLayout,
) {
    let min = layout.boundary.min;
    let max = layout.boundary.max;
    let thickness = layout.boundary.thickness.max(1.0);
    let width = max[0] - min[0];
    let height = max[1] - min[1];
    let center_x = (min[0] + max[0]) * 0.5;
    let center_y = (min[1] + max[1]) * 0.5;
    let color = color_from_rgb(layout.colors.boundary);

    let boundary_specs = [
        (
            "left",
            Vec2::new(thickness, height),
            Vec3::new(min[0] + thickness * 0.5, center_y, 0.3),
        ),
        (
            "right",
            Vec2::new(thickness, height),
            Vec3::new(max[0] - thickness * 0.5, center_y, 0.3),
        ),
        (
            "bottom",
            Vec2::new(width, thickness),
            Vec3::new(center_x, min[1] + thickness * 0.5, 0.3),
        ),
        (
            "top",
            Vec2::new(width, thickness),
            Vec3::new(center_x, max[1] - thickness * 0.5, 0.3),
        ),
    ];

    for (side, size, translation) in boundary_specs {
        spawn_robot_sync_arena_rect(
            commands,
            parent,
            session_id,
            RobotSyncArenaVisual::Boundary,
            format!("RobotSyncArenaBoundary({side})"),
            color,
            size,
            translation,
        );
    }
}

fn spawn_robot_sync_arena_spawn_markers(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    layout: &RobotSyncArenaLayout,
) {
    for (index, spawn_point) in layout.spawn_points.iter().enumerate() {
        spawn_robot_sync_arena_rect(
            commands,
            parent,
            session_id,
            RobotSyncArenaVisual::SpawnMarker,
            format!("RobotSyncArenaSpawnMarker({})", spawn_point.id),
            spawn_marker_color(layout, index, spawn_point),
            Vec2::splat(24.0),
            Vec3::new(spawn_point.position[0], spawn_point.position[1], 0.5),
        );
    }
}

fn spawn_robot_sync_arena_rect(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    visual: RobotSyncArenaVisual,
    name: String,
    color: Color,
    size: Vec2,
    translation: Vec3,
) -> Entity {
    let entity = commands
        .spawn((
            Sprite::from_color(color, size),
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

fn arena_center(min: &[f32; 2], max: &[f32; 2]) -> Vec2 {
    Vec2::new((min[0] + max[0]) * 0.5, (min[1] + max[1]) * 0.5)
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

fn spawn_marker_color(
    layout: &RobotSyncArenaLayout,
    index: usize,
    spawn_point: &RobotSyncArenaSpawnPoint,
) -> Color {
    let rgb = match index {
        0 => layout.colors.spawn_a,
        1 => layout.colors.spawn_b,
        _ => layout
            .robots
            .iter()
            .find(|robot| robot.spawn_point == spawn_point.id)
            .map(|robot| robot.color)
            .unwrap_or(layout.colors.spawn_a),
    };
    color_from_rgb_alpha(rgb, 0.92)
}

fn color_from_rgb(rgb: [f32; 3]) -> Color {
    Color::srgb(rgb[0], rgb[1], rgb[2])
}

fn color_from_rgb_alpha(rgb: [f32; 3], alpha: f32) -> Color {
    Color::srgba(rgb[0], rgb[1], rgb[2], alpha)
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
        SceneEntered, spawn_scene_root, spawn_scene_runtime_root,
    };

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
        assert!(layout.robots.len() >= 2);
        assert_eq!(layout.colors.boundary, [0.80, 0.86, 0.92]);
    }

    #[test]
    fn entered_robot_sync_arena_spawns_content_under_runtime_root() {
        let mut app = App::new();
        app.add_message::<SceneEvent>()
            .add_systems(Update, instantiate_robot_sync_arena_content);

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
        assert_eq!(visual_entities.len(), 29);

        let mut arena_fill_count = 0;
        let mut boundary_count = 0;
        let mut grid_count = 0;
        let mut spawn_marker_count = 0;
        for (parent, owned, content, visual, name) in visual_entities {
            assert_eq!(parent.parent(), content_entity);
            assert_eq!(owned.session_id, session_id);
            assert_eq!(content.session_id, session_id);
            assert!(name.as_str().starts_with("RobotSyncArena"));
            match visual {
                RobotSyncArenaVisual::ArenaFill => arena_fill_count += 1,
                RobotSyncArenaVisual::Boundary => boundary_count += 1,
                RobotSyncArenaVisual::Grid => grid_count += 1,
                RobotSyncArenaVisual::SpawnMarker => spawn_marker_count += 1,
            }
        }
        assert_eq!(arena_fill_count, 1);
        assert_eq!(boundary_count, 4);
        assert_eq!(grid_count, 22);
        assert_eq!(spawn_marker_count, 2);

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
        assert_eq!(robot_a_spawn.2.translation.x, -180.0);
        assert_eq!(robot_a_spawn.2.translation.y, -180.0);
        assert_eq!(robot_a_spawn.2.translation.z, 0.5);
        assert_eq!(robot_a_spawn.3.parent(), content_entity);
        assert_eq!(robot_a_spawn.4.session_id, session_id);
        assert_eq!(robot_a_spawn.5.session_id, session_id);
    }

    #[test]
    fn duplicate_enter_events_for_same_session_do_not_duplicate_content() {
        let mut app = App::new();
        app.add_message::<SceneEvent>()
            .add_systems(Update, instantiate_robot_sync_arena_content);

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
        assert_eq!(visual_sessions, 29);
    }
}
