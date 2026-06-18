use bevy::{gltf::GltfAssetLabel, prelude::*, scene::SceneRoot as BevySceneRoot};
use serde::{Deserialize, Deserializer, de};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

use crate::framework::scene::prelude::{
    SceneEvent, SceneLayerRoot, SceneOwned, SceneRuntimeRoot, SceneSessionId,
};

pub(in crate::game::scenes) const SAMPLE_DUNGEON_ROOM_SCENE_ID: &str = "sample.dungeon_room";
const SAMPLE_DUNGEON_ROOM_LAYOUT_PATH: &str = "scenes/sample_dungeon_room/layout.ron";

pub(super) struct SampleDungeonRoomPlugin;

impl Plugin for SampleDungeonRoomPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, instantiate_sample_dungeon_room_prefabs);
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default)]
struct SampleDungeonRoomLayout {
    prefabs: Vec<SampleDungeonRoomPrefab>,
    lights: Vec<SampleDungeonRoomLight>,
}

impl Default for SampleDungeonRoomLayout {
    fn default() -> Self {
        Self {
            prefabs: Vec::new(),
            lights: Vec::new(),
        }
    }
}

impl SampleDungeonRoomLayout {
    fn load_first_package_ron(layout_path: impl AsRef<str>) -> Result<Self, SampleLayoutLoadError> {
        let layout_path = layout_path.as_ref();
        let fs_path = first_package_layout_fs_path(layout_path)
            .ok_or_else(|| SampleLayoutLoadError::LayoutNotFound(layout_path.to_string()))?;

        let layout_source =
            fs::read_to_string(&fs_path).map_err(|source| SampleLayoutLoadError::ReadFailed {
                path: fs_path.clone(),
                source,
            })?;

        ron::from_str::<Self>(&layout_source).map_err(|source| SampleLayoutLoadError::ParseFailed {
            path: fs_path,
            source,
        })
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default)]
struct SampleDungeonRoomPrefab {
    id: String,
    asset_path: String,
    layer: String,
    #[serde(deserialize_with = "deserialize_f32_array_3")]
    translation: [f32; 3],
    #[serde(deserialize_with = "deserialize_f32_array_3")]
    rotation: [f32; 3],
    #[serde(deserialize_with = "deserialize_f32_array_3")]
    scale: [f32; 3],
}

impl Default for SampleDungeonRoomPrefab {
    fn default() -> Self {
        Self {
            id: String::new(),
            asset_path: String::new(),
            layer: String::new(),
            translation: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
        }
    }
}

impl SampleDungeonRoomPrefab {
    fn transform(&self) -> Transform {
        Transform {
            translation: Vec3::from_array(self.translation),
            rotation: Quat::from_euler(
                EulerRot::XYZ,
                self.rotation[0].to_radians(),
                self.rotation[1].to_radians(),
                self.rotation[2].to_radians(),
            ),
            scale: Vec3::from_array(self.scale),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default)]
struct SampleDungeonRoomLight {
    id: String,
    kind: SampleDungeonRoomLightKind,
    #[serde(deserialize_with = "deserialize_f32_array_3")]
    translation: [f32; 3],
    #[serde(deserialize_with = "deserialize_f32_array_3")]
    rotation: [f32; 3],
    #[serde(deserialize_with = "deserialize_f32_array_3")]
    color: [f32; 3],
    intensity: f32,
    range: Option<f32>,
}

impl Default for SampleDungeonRoomLight {
    fn default() -> Self {
        Self {
            id: String::new(),
            kind: SampleDungeonRoomLightKind::Point,
            translation: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0],
            color: [1.0, 1.0, 1.0],
            intensity: 0.0,
            range: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SampleDungeonRoomLightKind {
    Directional,
    #[default]
    Point,
}

impl<'de> Deserialize<'de> for SampleDungeonRoomLightKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        match value.trim() {
            "directional" => Ok(Self::Directional),
            "point" => Ok(Self::Point),
            other => Err(de::Error::unknown_variant(other, &["directional", "point"])),
        }
    }
}

#[derive(Clone, Debug, Component, PartialEq, Eq)]
struct SampleDungeonRoomContent {
    session_id: SceneSessionId,
}

#[derive(Debug)]
enum SampleLayoutLoadError {
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

impl std::fmt::Display for SampleLayoutLoadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LayoutNotFound(path) => {
                write!(
                    formatter,
                    "sample dungeon room layout was not found under assets: {path}"
                )
            }
            Self::ReadFailed { path, source } => {
                write!(
                    formatter,
                    "failed to read sample dungeon room layout at {}: {source}",
                    path.display()
                )
            }
            Self::ParseFailed { path, source } => {
                write!(
                    formatter,
                    "failed to parse sample dungeon room layout RON at {}: {source}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for SampleLayoutLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ReadFailed { source, .. } => Some(source),
            Self::ParseFailed { source, .. } => Some(source),
            Self::LayoutNotFound(_) => None,
        }
    }
}

fn instantiate_sample_dungeon_room_prefabs(
    mut commands: Commands,
    mut scene_events: MessageReader<SceneEvent>,
    asset_server: Res<AssetServer>,
    layer_roots: Query<(Entity, &SceneLayerRoot)>,
    runtime_roots: Query<(Entity, &SceneRuntimeRoot)>,
    existing_content: Query<&SampleDungeonRoomContent>,
) {
    let mut instantiated_sessions = Vec::new();

    for event in scene_events.read() {
        let SceneEvent::Entered(entered) = event else {
            continue;
        };

        if entered.scene_id.as_str() != SAMPLE_DUNGEON_ROOM_SCENE_ID {
            continue;
        }

        if existing_content
            .iter()
            .any(|content| content.session_id == entered.session_id)
            || instantiated_sessions.contains(&entered.session_id)
        {
            continue;
        }

        let layout = match SampleDungeonRoomLayout::load_first_package_ron(
            SAMPLE_DUNGEON_ROOM_LAYOUT_PATH,
        ) {
            Ok(layout) => layout,
            Err(error) => {
                warn!("{error}");
                continue;
            }
        };
        instantiated_sessions.push(entered.session_id.clone());

        for prefab in &layout.prefabs {
            let parent = parent_for_prefab(
                &prefab.layer,
                &entered.session_id,
                layer_roots.iter(),
                runtime_roots.iter(),
            );
            let Some(parent) = parent else {
                warn!(
                    "skipping sample dungeon prefab `{}` because session `{}` has no layer or runtime root",
                    prefab.id, entered.session_id
                );
                continue;
            };

            let scene_handle =
                asset_server.load(GltfAssetLabel::Scene(0).from_asset(prefab.asset_path.clone()));
            let prefab_entity = commands
                .spawn((
                    BevySceneRoot(scene_handle),
                    prefab.transform(),
                    SceneOwned::new(entered.session_id.clone()),
                    SampleDungeonRoomContent {
                        session_id: entered.session_id.clone(),
                    },
                    Name::new(format!("SampleDungeonPrefab({})", prefab.id)),
                ))
                .id();
            commands.entity(parent).add_child(prefab_entity);
        }
    }
}

fn parent_for_prefab<'layer, 'runtime>(
    layer_id: &str,
    session_id: &SceneSessionId,
    layer_roots: impl IntoIterator<Item = (Entity, &'layer SceneLayerRoot)>,
    runtime_roots: impl IntoIterator<Item = (Entity, &'runtime SceneRuntimeRoot)>,
) -> Option<Entity> {
    find_layer_root_entity(layer_id, session_id, layer_roots)
        .or_else(|| find_runtime_root_entity(session_id, runtime_roots))
}

fn find_layer_root_entity<'layer>(
    layer_id: &str,
    session_id: &SceneSessionId,
    layer_roots: impl IntoIterator<Item = (Entity, &'layer SceneLayerRoot)>,
) -> Option<Entity> {
    layer_roots
        .into_iter()
        .find(|(_, root)| root.is_session(session_id) && root.layer_id.as_str() == layer_id)
        .map(|(entity, _)| entity)
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
        SceneLayerState, spawn_scene_layer_root, spawn_scene_root, spawn_scene_runtime_root,
    };

    #[test]
    fn parse_layout_reads_prefabs_and_lights() {
        let layout = ron::from_str::<SampleDungeonRoomLayout>(
            r#"
            (
                prefabs: [
                    (
                        id: "floor.center",
                        asset_path: "models/scenes/kaykit_dungeon_remastered/floor_tile_large.gltf",
                        layer: "terrain",
                        translation: [1.0, 2.0, 3.0],
                        rotation: [0.0, 90.0, 180.0],
                        scale: [1.0, 2.0, 3.0],
                    ),
                ],
                lights: [
                    (
                        id: "sun",
                        kind: "directional",
                        translation: [0.0, 6.0, 0.0],
                        rotation: [-45.0, -25.0, 0.0],
                        color: [1.0, 0.94, 0.82],
                        intensity: 2500.0,
                        range: None,
                    ),
                    (
                        id: "torch.light",
                        kind: "point",
                        translation: [4.2, 1.8, 2.0],
                        rotation: [0.0, 0.0, 0.0],
                        color: [1.0, 0.58, 0.28],
                        intensity: 300.0,
                        range: Some(4.5),
                    ),
                ],
            )
            "#,
        )
        .unwrap();

        assert_eq!(layout.prefabs.len(), 1);
        assert_eq!(layout.prefabs[0].id, "floor.center");
        assert_eq!(
            layout.prefabs[0].asset_path,
            "models/scenes/kaykit_dungeon_remastered/floor_tile_large.gltf"
        );
        assert_eq!(layout.prefabs[0].layer, "terrain");
        assert_eq!(layout.prefabs[0].translation, [1.0, 2.0, 3.0]);
        assert_eq!(layout.prefabs[0].rotation, [0.0, 90.0, 180.0]);
        assert_eq!(layout.prefabs[0].scale, [1.0, 2.0, 3.0]);

        assert_eq!(layout.lights.len(), 2);
        assert_eq!(layout.lights[0].id, "sun");
        assert_eq!(
            layout.lights[0].kind,
            SampleDungeonRoomLightKind::Directional
        );
        assert_eq!(layout.lights[0].intensity, 2500.0);
        assert_eq!(layout.lights[0].range, None);
        assert_eq!(layout.lights[1].kind, SampleDungeonRoomLightKind::Point);
        assert_eq!(layout.lights[1].range, Some(4.5));
    }

    #[test]
    fn load_sample_layout_from_first_package_assets() {
        let layout =
            SampleDungeonRoomLayout::load_first_package_ron(SAMPLE_DUNGEON_ROOM_LAYOUT_PATH)
                .unwrap();

        assert!(
            layout
                .prefabs
                .iter()
                .any(|prefab| prefab.id == "floor.center")
        );
        assert!(layout.lights.iter().any(|light| light.id == "sun"));
    }

    #[test]
    fn parent_for_prefab_matches_layer_or_falls_back_to_runtime_root() {
        let mut app = App::new();
        let session_id = SceneSessionId::from("sample-session");
        let scene_root = spawn_scene_root(
            &mut app.world_mut().commands(),
            &SAMPLE_DUNGEON_ROOM_SCENE_ID.into(),
            &session_id,
        );
        let terrain_root = spawn_scene_layer_root(
            &mut app.world_mut().commands(),
            scene_root,
            &session_id,
            "terrain",
            SceneLayerState::Active,
            true,
        );
        let runtime_root =
            spawn_scene_runtime_root(&mut app.world_mut().commands(), scene_root, &session_id);
        app.update();

        let mut layer_roots = app.world_mut().query::<(Entity, &SceneLayerRoot)>();
        let mut runtime_roots = app.world_mut().query::<(Entity, &SceneRuntimeRoot)>();
        let world = app.world();

        assert_eq!(
            parent_for_prefab(
                "terrain",
                &session_id,
                layer_roots.iter(world),
                runtime_roots.iter(world)
            ),
            Some(terrain_root)
        );
        assert_eq!(
            parent_for_prefab(
                "missing",
                &session_id,
                layer_roots.iter(world),
                runtime_roots.iter(world)
            ),
            Some(runtime_root)
        );
    }
}
