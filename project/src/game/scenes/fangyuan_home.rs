use bevy::prelude::*;
use serde::{Deserialize, Deserializer, de};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

use crate::framework::scene::prelude::{SceneEvent, SceneOwned, SceneRuntimeRoot, SceneSessionId};

pub(in crate::game) const FANGYUAN_HOME_SCENE_ID: &str = "dev.fangyuan_home";
const FANGYUAN_HOME_LAYOUT_PATH: &str = "scenes/fangyuan_home/layout.ron";
#[cfg(test)]
const FANGYUAN_HOME_SCENE_MANIFEST_PATH: &str = "scenes/fangyuan_home/scene.ron";

pub(super) struct FangyuanHomePlugin;

impl Plugin for FangyuanHomePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .add_systems(PostUpdate, instantiate_fangyuan_home_content);
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
struct FangyuanHomeLayout {
    version: String,
    scene_id: String,
    plane: FangyuanHomePlane,
    grid: FangyuanHomeGrid,
    boundary: FangyuanHomeBoundary,
    lights: Vec<FangyuanHomeLight>,
    default_blueprint_path: String,
}

impl FangyuanHomeLayout {
    fn load_first_package_ron(
        layout_path: impl AsRef<str>,
    ) -> Result<Self, FangyuanLayoutLoadError> {
        let layout_path = layout_path.as_ref();
        let fs_path = first_package_layout_fs_path(layout_path)
            .ok_or_else(|| FangyuanLayoutLoadError::LayoutNotFound(layout_path.to_string()))?;

        let layout_source =
            fs::read_to_string(&fs_path).map_err(|source| FangyuanLayoutLoadError::ReadFailed {
                path: fs_path.clone(),
                source,
            })?;

        ron::from_str::<Self>(&layout_source).map_err(|source| {
            FangyuanLayoutLoadError::ParseFailed {
                path: fs_path,
                source,
            }
        })
    }

    fn is_scene_id_valid(&self) -> bool {
        self.scene_id == FANGYUAN_HOME_SCENE_ID
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default)]
struct FangyuanHomePlane {
    width: f32,
    depth: f32,
    thickness: f32,
    #[serde(deserialize_with = "deserialize_f32_array_3")]
    color: [f32; 3],
}

impl Default for FangyuanHomePlane {
    fn default() -> Self {
        Self {
            width: 24.0,
            depth: 24.0,
            thickness: 0.2,
            color: [0.18, 0.20, 0.19],
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default)]
struct FangyuanHomeGrid {
    spacing: f32,
    major_every: u32,
    line_height: f32,
    minor_width: f32,
    major_width: f32,
    #[serde(deserialize_with = "deserialize_f32_array_3")]
    color_minor: [f32; 3],
    #[serde(deserialize_with = "deserialize_f32_array_3")]
    color_major: [f32; 3],
}

impl Default for FangyuanHomeGrid {
    fn default() -> Self {
        Self {
            spacing: 1.0,
            major_every: 4,
            line_height: 0.03,
            minor_width: 0.025,
            major_width: 0.06,
            color_minor: [0.36, 0.42, 0.40],
            color_major: [0.58, 0.68, 0.63],
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default)]
struct FangyuanHomeBoundary {
    thickness: f32,
    height: f32,
    #[serde(deserialize_with = "deserialize_f32_array_3")]
    color: [f32; 3],
}

impl Default for FangyuanHomeBoundary {
    fn default() -> Self {
        Self {
            thickness: 0.28,
            height: 0.85,
            color: [0.48, 0.55, 0.50],
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default)]
struct FangyuanHomeLight {
    id: String,
    kind: FangyuanHomeLightKind,
    #[serde(deserialize_with = "deserialize_f32_array_3")]
    translation: [f32; 3],
    #[serde(deserialize_with = "deserialize_f32_array_3")]
    rotation: [f32; 3],
    #[serde(deserialize_with = "deserialize_f32_array_3")]
    color: [f32; 3],
    intensity: f32,
    range: Option<f32>,
}

impl Default for FangyuanHomeLight {
    fn default() -> Self {
        Self {
            id: String::new(),
            kind: FangyuanHomeLightKind::Point,
            translation: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0],
            color: [1.0, 1.0, 1.0],
            intensity: 0.0,
            range: None,
        }
    }
}

impl FangyuanHomeLight {
    const DEFAULT_POINT_LIGHT_RANGE: f32 = 18.0;

    fn transform(&self) -> Transform {
        Transform {
            translation: Vec3::from_array(self.translation),
            rotation: rotation_from_degrees(self.rotation),
            scale: Vec3::ONE,
        }
    }

    fn color(&self) -> Color {
        color_from_rgb(self.color)
    }

    fn point_light(&self) -> PointLight {
        PointLight {
            color: self.color(),
            intensity: self.intensity,
            range: self.range.unwrap_or(Self::DEFAULT_POINT_LIGHT_RANGE),
            shadows_enabled: false,
            ..default()
        }
    }

    fn directional_light(&self) -> DirectionalLight {
        DirectionalLight {
            color: self.color(),
            illuminance: self.intensity,
            shadows_enabled: false,
            ..default()
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum FangyuanHomeLightKind {
    Directional,
    #[default]
    Point,
}

impl<'de> Deserialize<'de> for FangyuanHomeLightKind {
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
struct FangyuanHomeContent {
    session_id: SceneSessionId,
}

#[derive(Clone, Copy, Debug, Component, PartialEq, Eq)]
enum FangyuanHomeVisual {
    Plane,
    Grid,
    Boundary,
    DirectionalLight,
    PointLight,
}

#[derive(Debug)]
enum FangyuanLayoutLoadError {
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

impl std::fmt::Display for FangyuanLayoutLoadError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LayoutNotFound(path) => {
                write!(
                    formatter,
                    "fangyuan home layout was not found under assets: {path}"
                )
            }
            Self::ReadFailed { path, source } => {
                write!(
                    formatter,
                    "failed to read fangyuan home layout at {}: {source}",
                    path.display()
                )
            }
            Self::ParseFailed { path, source } => {
                write!(
                    formatter,
                    "failed to parse fangyuan home layout RON at {}: {source}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for FangyuanLayoutLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ReadFailed { source, .. } => Some(source),
            Self::ParseFailed { source, .. } => Some(source),
            Self::LayoutNotFound(_) => None,
        }
    }
}

fn instantiate_fangyuan_home_content(
    mut commands: Commands,
    mut scene_events: MessageReader<SceneEvent>,
    runtime_roots: Query<(Entity, &SceneRuntimeRoot)>,
    existing_content: Query<&FangyuanHomeContent>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut instantiated_sessions = Vec::new();

    for event in scene_events.read() {
        let SceneEvent::Entered(entered) = event else {
            continue;
        };

        if entered.scene_id.as_str() != FANGYUAN_HOME_SCENE_ID {
            continue;
        }

        if existing_content
            .iter()
            .any(|content| content.session_id == entered.session_id)
            || instantiated_sessions.contains(&entered.session_id)
        {
            continue;
        }

        let layout = match FangyuanHomeLayout::load_first_package_ron(FANGYUAN_HOME_LAYOUT_PATH) {
            Ok(layout) => layout,
            Err(error) => {
                warn!("{error}");
                continue;
            }
        };

        if !layout.is_scene_id_valid() {
            warn!(
                "skipping fangyuan home content because layout scene_id `{}` does not match `{}`",
                layout.scene_id, FANGYUAN_HOME_SCENE_ID
            );
            continue;
        }

        let Some(runtime_root) =
            find_runtime_root_entity(&entered.session_id, runtime_roots.iter())
        else {
            warn!(
                "skipping fangyuan home content because session `{}` has no runtime root",
                entered.session_id
            );
            continue;
        };

        spawn_fangyuan_home_content(
            &mut commands,
            runtime_root,
            &entered.session_id,
            &layout,
            &mut meshes,
            &mut materials,
        );
        instantiated_sessions.push(entered.session_id.clone());
    }
}

fn spawn_fangyuan_home_content(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    layout: &FangyuanHomeLayout,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> Entity {
    let content = commands
        .spawn((
            SceneOwned::new(session_id.clone()),
            FangyuanHomeContent {
                session_id: session_id.clone(),
            },
            Transform::default(),
            Name::new(format!("FangyuanHomeContent({session_id})")),
        ))
        .id();
    commands.entity(parent).add_child(content);

    spawn_fangyuan_home_plane(commands, content, session_id, layout, meshes, materials);
    spawn_fangyuan_home_grid(commands, content, session_id, layout, meshes, materials);
    spawn_fangyuan_home_boundary(commands, content, session_id, layout, meshes, materials);
    spawn_fangyuan_home_lights(commands, content, session_id, layout);

    content
}

fn spawn_fangyuan_home_plane(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    layout: &FangyuanHomeLayout,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> Entity {
    let thickness = layout.plane.thickness.max(0.01);
    spawn_fangyuan_home_box(
        commands,
        parent,
        session_id,
        FangyuanHomeVisual::Plane,
        "FangyuanHomePlane".to_string(),
        color_from_rgb(layout.plane.color),
        Vec3::new(
            layout.plane.width.max(0.01),
            thickness,
            layout.plane.depth.max(0.01),
        ),
        Vec3::new(0.0, -thickness * 0.5, 0.0),
        meshes,
        materials,
    )
}

fn spawn_fangyuan_home_grid(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    layout: &FangyuanHomeLayout,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let half_width = layout.plane.width * 0.5;
    let half_depth = layout.plane.depth * 0.5;
    let spacing = layout.grid.spacing.max(0.01);
    let line_height = layout.grid.line_height.max(0.005);
    let y = line_height * 0.5 + 0.01;
    let minor_color = color_from_rgb_alpha(layout.grid.color_minor, 0.72);
    let major_color = color_from_rgb_alpha(layout.grid.color_major, 0.9);

    for x in centered_grid_line_positions(half_width, spacing) {
        let major = is_major_grid_line(x, spacing, layout.grid.major_every);
        let thickness = grid_line_width(&layout.grid, major);
        let color = if major { major_color } else { minor_color };
        let kind = if major { "major" } else { "minor" };
        spawn_fangyuan_home_box(
            commands,
            parent,
            session_id,
            FangyuanHomeVisual::Grid,
            format!("FangyuanHomeGrid({kind}:vertical:{x:.2})"),
            color,
            Vec3::new(thickness, line_height, layout.plane.depth),
            Vec3::new(x, y, 0.0),
            meshes,
            materials,
        );
    }

    for z in centered_grid_line_positions(half_depth, spacing) {
        let major = is_major_grid_line(z, spacing, layout.grid.major_every);
        let thickness = grid_line_width(&layout.grid, major);
        let color = if major { major_color } else { minor_color };
        let kind = if major { "major" } else { "minor" };
        spawn_fangyuan_home_box(
            commands,
            parent,
            session_id,
            FangyuanHomeVisual::Grid,
            format!("FangyuanHomeGrid({kind}:horizontal:{z:.2})"),
            color,
            Vec3::new(layout.plane.width, line_height, thickness),
            Vec3::new(0.0, y, z),
            meshes,
            materials,
        );
    }
}

fn spawn_fangyuan_home_boundary(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    layout: &FangyuanHomeLayout,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let half_width = layout.plane.width * 0.5;
    let half_depth = layout.plane.depth * 0.5;
    let thickness = layout.boundary.thickness.max(0.01);
    let height = layout.boundary.height.max(0.01);
    let y = height * 0.5;
    let color = color_from_rgb(layout.boundary.color);

    let boundary_specs = [
        (
            "west",
            Vec3::new(thickness, height, layout.plane.depth + thickness * 2.0),
            Vec3::new(-half_width - thickness * 0.5, y, 0.0),
        ),
        (
            "east",
            Vec3::new(thickness, height, layout.plane.depth + thickness * 2.0),
            Vec3::new(half_width + thickness * 0.5, y, 0.0),
        ),
        (
            "north",
            Vec3::new(layout.plane.width, height, thickness),
            Vec3::new(0.0, y, -half_depth - thickness * 0.5),
        ),
        (
            "south",
            Vec3::new(layout.plane.width, height, thickness),
            Vec3::new(0.0, y, half_depth + thickness * 0.5),
        ),
    ];

    for (side, size, translation) in boundary_specs {
        spawn_fangyuan_home_box(
            commands,
            parent,
            session_id,
            FangyuanHomeVisual::Boundary,
            format!("FangyuanHomeBoundary({side})"),
            color,
            size,
            translation,
            meshes,
            materials,
        );
    }
}

fn spawn_fangyuan_home_lights(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    layout: &FangyuanHomeLayout,
) {
    for light in &layout.lights {
        let common = (
            light.transform(),
            SceneOwned::new(session_id.clone()),
            FangyuanHomeContent {
                session_id: session_id.clone(),
            },
            Name::new(format!("FangyuanHomeLight({})", light.id)),
        );
        let entity = match light.kind {
            FangyuanHomeLightKind::Directional => commands
                .spawn((
                    light.directional_light(),
                    common,
                    FangyuanHomeVisual::DirectionalLight,
                ))
                .id(),
            FangyuanHomeLightKind::Point => commands
                .spawn((light.point_light(), common, FangyuanHomeVisual::PointLight))
                .id(),
        };
        commands.entity(parent).add_child(entity);
    }
}

fn spawn_fangyuan_home_box(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    visual: FangyuanHomeVisual,
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
            FangyuanHomeContent {
                session_id: session_id.clone(),
            },
            visual,
            Name::new(name),
        ))
        .id();
    commands.entity(parent).add_child(entity);
    entity
}

fn centered_grid_line_positions(half_extent: f32, spacing: f32) -> Vec<f32> {
    if half_extent < 0.0 || spacing <= 0.0 {
        return Vec::new();
    }

    let min_index = (-half_extent / spacing).ceil() as i32;
    let max_index = (half_extent / spacing).floor() as i32;
    (min_index..=max_index)
        .map(|index| index as f32 * spacing)
        .collect()
}

fn is_major_grid_line(position: f32, spacing: f32, major_every: u32) -> bool {
    if spacing <= 0.0 || major_every == 0 {
        return false;
    }

    let grid_index = (position / spacing).round() as i32;
    grid_index % major_every as i32 == 0
}

fn grid_line_width(grid: &FangyuanHomeGrid, major: bool) -> f32 {
    if major {
        grid.major_width
    } else {
        grid.minor_width
    }
    .max(0.005)
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
        perceptual_roughness: 0.92,
        alpha_mode: if alpha < 1.0 {
            AlphaMode::Blend
        } else {
            AlphaMode::Opaque
        },
        ..default()
    }
}

fn rotation_from_degrees(rotation: [f32; 3]) -> Quat {
    Quat::from_euler(
        EulerRot::XYZ,
        rotation[0].to_radians(),
        rotation[1].to_radians(),
        rotation[2].to_radians(),
    )
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
        SceneCameraMode, SceneCameraProjection, SceneCommand, SceneEnterRequest, SceneExitRequest,
        SceneManifest, ScenePlugin, SceneRegistry, SceneRoot, SceneRuntime, SceneRuntimeRoot,
        spawn_scene_root, spawn_scene_runtime_root,
    };
    use bevy::{asset::AssetPlugin, mesh::VertexAttributeValues};

    const EXPECTED_GRID_VISUALS: usize = 50;
    const EXPECTED_BOUNDARY_VISUALS: usize = 4;
    const EXPECTED_LIGHT_VISUALS: usize = 2;
    const EXPECTED_TOTAL_VISUALS: usize =
        1 + EXPECTED_GRID_VISUALS + EXPECTED_BOUNDARY_VISUALS + EXPECTED_LIGHT_VISUALS;

    fn app_with_fangyuan_home_system() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .add_message::<SceneEvent>()
            .add_systems(Update, instantiate_fangyuan_home_content);
        app
    }

    fn app_with_scene_lifecycle() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), ScenePlugin))
            .add_plugins(FangyuanHomePlugin);
        app.world_mut()
            .resource_mut::<SceneRegistry>()
            .register_manifest_scene(
                FANGYUAN_HOME_SCENE_ID,
                crate::framework::scene::prelude::SceneKind::World,
                FANGYUAN_HOME_SCENE_MANIFEST_PATH,
            )
            .unwrap();
        app
    }

    #[test]
    fn load_fangyuan_home_manifest_from_first_package_assets() {
        let manifest =
            SceneManifest::load_first_package_ron(FANGYUAN_HOME_SCENE_MANIFEST_PATH).unwrap();

        assert_eq!(manifest.version, "1");
        assert_eq!(manifest.scene_id.as_str(), FANGYUAN_HOME_SCENE_ID);
        assert_eq!(
            manifest
                .entry
                .default_spawn
                .as_ref()
                .map(|spawn| spawn.as_str()),
            Some("spawn.default")
        );
        assert_eq!(manifest.layers.len(), 1);
        assert_eq!(manifest.layers[0].id.as_str(), "base_space");
        assert!(!manifest.layers[0].required);
        assert!(manifest.layers[0].assets.is_empty());
        assert!(
            manifest
                .anchors
                .iter()
                .any(|anchor| anchor.id.as_str() == "anchor.center")
        );

        let camera = manifest.entry.camera.as_ref().unwrap();
        let camera_config = camera.config();
        assert_eq!(camera_config.mode, SceneCameraMode::Fixed3d);
        assert!(camera_config.is_3d());
        assert_eq!(
            camera_config.transform.translation,
            Vec3::new(0.0, 18.0, 24.0)
        );
        let SceneCameraProjection::Perspective3d {
            fov_y_radians,
            near,
            far,
        } = camera_config.projection
        else {
            panic!("fangyuan home camera should use a perspective 3D projection");
        };
        assert!((fov_y_radians - 0.82).abs() < f32::EPSILON);
        assert!((near - 0.1).abs() < f32::EPSILON);
        assert!((far - 160.0).abs() < f32::EPSILON);
        assert_eq!(
            camera_config.target.as_ref().map(|target| target.as_str()),
            Some("anchor.center")
        );
    }

    #[test]
    fn load_fangyuan_home_layout_from_first_package_assets() {
        let layout = FangyuanHomeLayout::load_first_package_ron(FANGYUAN_HOME_LAYOUT_PATH).unwrap();

        assert_eq!(layout.version, "1");
        assert_eq!(layout.scene_id, FANGYUAN_HOME_SCENE_ID);
        assert!(layout.is_scene_id_valid());
        assert_eq!(layout.plane.width, 24.0);
        assert_eq!(layout.plane.depth, 24.0);
        assert_eq!(layout.grid.spacing, 1.0);
        assert_eq!(layout.grid.major_every, 4);
        assert_eq!(layout.boundary.thickness, 0.28);
        assert_eq!(layout.boundary.height, 0.85);
        assert_eq!(layout.default_blueprint_path, "fangyuan/home_preview.ron");
        assert_eq!(layout.lights.len(), 2);
        assert!(
            layout
                .lights
                .iter()
                .any(|light| light.kind == FangyuanHomeLightKind::Directional)
        );
        assert!(
            layout
                .lights
                .iter()
                .any(|light| light.kind == FangyuanHomeLightKind::Point)
        );
    }

    #[test]
    fn grid_line_positions_cover_layout_bounds() {
        let layout = FangyuanHomeLayout::load_first_package_ron(FANGYUAN_HOME_LAYOUT_PATH).unwrap();
        let positions = centered_grid_line_positions(layout.plane.width * 0.5, layout.grid.spacing);

        assert_eq!(positions.len(), 25);
        assert_eq!(positions.first().copied(), Some(-12.0));
        assert_eq!(positions.last().copied(), Some(12.0));
        assert!(positions.contains(&0.0));
        assert!(is_major_grid_line(-12.0, 1.0, 4));
        assert!(!is_major_grid_line(-11.0, 1.0, 4));
    }

    #[test]
    fn entered_fangyuan_home_spawns_base_space_under_runtime_root() {
        let mut app = app_with_fangyuan_home_system();

        let session_id = SceneSessionId::from("fangyuan-session");
        let scene_root = spawn_scene_root(
            &mut app.world_mut().commands(),
            &FANGYUAN_HOME_SCENE_ID.into(),
            &session_id,
        );
        let runtime_root =
            spawn_scene_runtime_root(&mut app.world_mut().commands(), scene_root, &session_id);
        app.update();

        app.world_mut().write_message(SceneEvent::Entered(
            crate::framework::scene::prelude::SceneEntered {
                scene_id: FANGYUAN_HOME_SCENE_ID.into(),
                session_id: session_id.clone(),
                content_version: None,
            },
        ));
        app.update();

        let mut content = app.world_mut().query_filtered::<(
            Entity,
            &ChildOf,
            &SceneOwned,
            &FangyuanHomeContent,
            &Transform,
            &Name,
        ), Without<FangyuanHomeVisual>>();
        let content_entities = content.iter(app.world()).collect::<Vec<_>>();
        assert_eq!(content_entities.len(), 1);

        let (content_entity, parent, owned, content, transform, name) = content_entities[0];
        assert_eq!(parent.parent(), runtime_root);
        assert_eq!(owned.session_id, session_id);
        assert_eq!(content.session_id, session_id);
        assert_eq!(transform, &Transform::default());
        assert_eq!(name.as_str(), "FangyuanHomeContent(fangyuan-session)");

        let mut visuals = app.world_mut().query::<(
            &ChildOf,
            &SceneOwned,
            &FangyuanHomeContent,
            &FangyuanHomeVisual,
            &Name,
        )>();
        let visual_entities = visuals.iter(app.world()).collect::<Vec<_>>();
        assert_eq!(visual_entities.len(), EXPECTED_TOTAL_VISUALS);

        let mut plane_count = 0;
        let mut grid_count = 0;
        let mut boundary_count = 0;
        let mut directional_light_count = 0;
        let mut point_light_count = 0;
        for (parent, owned, content, visual, name) in visual_entities {
            assert_eq!(parent.parent(), content_entity);
            assert_eq!(owned.session_id, session_id);
            assert_eq!(content.session_id, session_id);
            assert!(name.as_str().starts_with("FangyuanHome"));
            match visual {
                FangyuanHomeVisual::Plane => plane_count += 1,
                FangyuanHomeVisual::Grid => grid_count += 1,
                FangyuanHomeVisual::Boundary => boundary_count += 1,
                FangyuanHomeVisual::DirectionalLight => directional_light_count += 1,
                FangyuanHomeVisual::PointLight => point_light_count += 1,
            }
        }
        assert_eq!(plane_count, 1);
        assert_eq!(grid_count, EXPECTED_GRID_VISUALS);
        assert_eq!(boundary_count, EXPECTED_BOUNDARY_VISUALS);
        assert_eq!(directional_light_count, 1);
        assert_eq!(point_light_count, 1);

        let (plane_translation, plane_mesh) = {
            let mut planes = app
                .world_mut()
                .query::<(&FangyuanHomeVisual, &Transform, &Mesh3d, &Name)>();
            let (_, transform, mesh, _) = planes
                .iter(app.world())
                .find(|(visual, _, _, name)| {
                    **visual == FangyuanHomeVisual::Plane && name.as_str() == "FangyuanHomePlane"
                })
                .expect("base plane should exist");
            (transform.translation, mesh.0.clone())
        };
        assert_eq!(plane_translation, Vec3::new(0.0, -0.1, 0.0));
        assert_eq!(
            mesh_position_size(
                app.world()
                    .resource::<Assets<Mesh>>()
                    .get(&plane_mesh)
                    .unwrap()
            ),
            Vec3::new(24.0, 0.2, 24.0)
        );

        let mut lights = app.world_mut().query::<(
            Option<&DirectionalLight>,
            Option<&PointLight>,
            &FangyuanHomeVisual,
            &ChildOf,
            &SceneOwned,
            &FangyuanHomeContent,
            &Name,
        )>();
        let light_entities = lights
            .iter(app.world())
            .filter(|(_, _, visual, _, _, _, _)| {
                **visual == FangyuanHomeVisual::DirectionalLight
                    || **visual == FangyuanHomeVisual::PointLight
            })
            .collect::<Vec<_>>();
        assert_eq!(light_entities.len(), EXPECTED_LIGHT_VISUALS);
        assert!(
            light_entities
                .iter()
                .all(|(_, _, _, parent, owned, content, name)| {
                    parent.parent() == content_entity
                        && owned.session_id == session_id
                        && content.session_id == session_id
                        && name.as_str().starts_with("FangyuanHomeLight(")
                })
        );
        assert!(
            light_entities
                .iter()
                .any(|(directional, _, visual, _, _, _, name)| {
                    **visual == FangyuanHomeVisual::DirectionalLight
                        && directional.is_some()
                        && name.as_str() == "FangyuanHomeLight(sun)"
                })
        );
        assert!(
            light_entities
                .iter()
                .any(|(_, point, visual, _, _, _, name)| {
                    **visual == FangyuanHomeVisual::PointLight
                        && point.is_some()
                        && name.as_str() == "FangyuanHomeLight(center_fill)"
                })
        );
    }

    #[test]
    fn duplicate_enter_events_for_same_session_do_not_duplicate_content() {
        let mut app = app_with_fangyuan_home_system();

        let session_id = SceneSessionId::from("fangyuan-session");
        let scene_root = spawn_scene_root(
            &mut app.world_mut().commands(),
            &FANGYUAN_HOME_SCENE_ID.into(),
            &session_id,
        );
        spawn_scene_runtime_root(&mut app.world_mut().commands(), scene_root, &session_id);
        app.update();

        for _ in 0..2 {
            app.world_mut().write_message(SceneEvent::Entered(
                crate::framework::scene::prelude::SceneEntered {
                    scene_id: FANGYUAN_HOME_SCENE_ID.into(),
                    session_id: session_id.clone(),
                    content_version: None,
                },
            ));
        }
        app.update();

        let mut content = app
            .world_mut()
            .query_filtered::<&FangyuanHomeContent, Without<FangyuanHomeVisual>>();
        let content_sessions = content
            .iter(app.world())
            .filter(|content| content.session_id == session_id)
            .count();
        assert_eq!(content_sessions, 1);

        let mut visuals = app
            .world_mut()
            .query_filtered::<&FangyuanHomeContent, With<FangyuanHomeVisual>>();
        let visual_sessions = visuals
            .iter(app.world())
            .filter(|content| content.session_id == session_id)
            .count();
        assert_eq!(visual_sessions, EXPECTED_TOTAL_VISUALS);
    }

    #[test]
    fn scene_lifecycle_exit_cleans_fangyuan_home_scene_owned_content() {
        let mut app = app_with_scene_lifecycle();
        let session_id = SceneSessionId::from("fangyuan-lifecycle-session");

        let mut request = SceneEnterRequest::new(FANGYUAN_HOME_SCENE_ID);
        request.session_id = Some(session_id.clone());
        app.world_mut().write_message(SceneCommand::Enter(request));
        app.update();

        assert_eq!(
            app.world()
                .resource::<SceneRuntime>()
                .active_session_id()
                .map(|session| session.as_str()),
            Some("fangyuan-lifecycle-session")
        );

        let counts = scene_entity_counts_for_session_from_world(&mut app, &session_id);
        assert_eq!(counts.scene_roots, 1);
        assert_eq!(counts.runtime_roots, 1);
        assert!(counts.layer_roots >= 1);
        assert_eq!(fangyuan_content_count(&mut app, &session_id), 1);
        assert_eq!(
            fangyuan_visual_count(&mut app, &session_id),
            EXPECTED_TOTAL_VISUALS
        );

        app.world_mut()
            .write_message(SceneCommand::Exit(SceneExitRequest::default()));
        app.update();
        app.update();

        let counts = scene_entity_counts_for_session_from_world(&mut app, &session_id);
        assert!(counts.is_empty());
        assert_eq!(fangyuan_content_count(&mut app, &session_id), 0);
        assert_eq!(fangyuan_visual_count(&mut app, &session_id), 0);
        assert_eq!(
            app.world().resource::<SceneRuntime>().active_session_id(),
            None
        );
    }

    fn fangyuan_content_count(app: &mut App, session_id: &SceneSessionId) -> usize {
        let mut content = app
            .world_mut()
            .query_filtered::<&FangyuanHomeContent, Without<FangyuanHomeVisual>>();
        content
            .iter(app.world())
            .filter(|content| content.session_id == *session_id)
            .count()
    }

    fn fangyuan_visual_count(app: &mut App, session_id: &SceneSessionId) -> usize {
        let mut visuals = app
            .world_mut()
            .query_filtered::<&FangyuanHomeContent, With<FangyuanHomeVisual>>();
        visuals
            .iter(app.world())
            .filter(|content| content.session_id == *session_id)
            .count()
    }

    fn scene_entity_counts_for_session_from_world(
        app: &mut App,
        session_id: &SceneSessionId,
    ) -> crate::framework::scene::prelude::SceneEntityCounts {
        let mut owned_entities = app.world_mut().query::<&SceneOwned>();
        let mut scene_roots = app.world_mut().query::<&SceneRoot>();
        let mut layer_roots = app
            .world_mut()
            .query::<&crate::framework::scene::prelude::SceneLayerRoot>();
        let mut runtime_roots = app.world_mut().query::<&SceneRuntimeRoot>();

        let world = app.world();
        crate::framework::scene::prelude::SceneEntityCounts {
            total_scene_owned: owned_entities
                .iter(world)
                .filter(|owned| owned.is_session(session_id))
                .count(),
            scene_roots: scene_roots
                .iter(world)
                .filter(|root| root.is_session(session_id))
                .count(),
            layer_roots: layer_roots
                .iter(world)
                .filter(|root| root.is_session(session_id))
                .count(),
            runtime_roots: runtime_roots
                .iter(world)
                .filter(|root| root.is_session(session_id))
                .count(),
        }
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
}
