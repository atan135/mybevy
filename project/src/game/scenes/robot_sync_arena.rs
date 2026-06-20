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

        spawn_robot_sync_arena_content(&mut commands, runtime_root, &entered.session_id);
        instantiated_sessions.push(entered.session_id.clone());
    }
}

fn spawn_robot_sync_arena_content(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
) -> Entity {
    let content = commands
        .spawn((
            SceneOwned::new(session_id.clone()),
            RobotSyncArenaContent {
                session_id: session_id.clone(),
            },
            Name::new(format!("RobotSyncArenaContent({session_id})")),
        ))
        .id();
    commands.entity(parent).add_child(content);
    content
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

        let mut content =
            app.world_mut()
                .query::<(Entity, &ChildOf, &SceneOwned, &RobotSyncArenaContent, &Name)>();
        let content_entities = content.iter(app.world()).collect::<Vec<_>>();
        assert_eq!(content_entities.len(), 1);

        let (_, parent, owned, content, name) = content_entities[0];
        assert_eq!(parent.parent(), runtime_root);
        assert_eq!(owned.session_id, session_id);
        assert_eq!(content.session_id, session_id);
        assert_eq!(name.as_str(), "RobotSyncArenaContent(robot-sync-session)");
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

        let mut content = app.world_mut().query::<&RobotSyncArenaContent>();
        let content_sessions = content
            .iter(app.world())
            .filter(|content| content.session_id == session_id)
            .count();
        assert_eq!(content_sessions, 1);
    }
}
