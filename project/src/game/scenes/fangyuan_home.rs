use bevy::{prelude::*, render::batching::NoAutomaticBatching};
use serde::{Deserialize, Deserializer, de};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

use crate::framework::{
    fangyuan::{
        FangyuanBlueprint, FangyuanObjectState, FangyuanPrimitive, FangyuanPrimitiveKind,
        FangyuanPrimitiveSet, FangyuanPrimitiveSetStats, FangyuanRenderAssetCache,
        fangyuan_render_transform_from_primitive, fangyuan_standard_material_from_color,
    },
    scene::prelude::{SceneEvent, SceneOwned, SceneRuntimeRoot, SceneSessionId},
};

pub(in crate::game) const FANGYUAN_HOME_SCENE_ID: &str = "dev.fangyuan_home";
pub(in crate::game) const FANGYUAN_HOME_DEFAULT_BLUEPRINT_PATH: &str = "fangyuan/home_preview.ron";
const FANGYUAN_HOME_LAYOUT_PATH: &str = "scenes/fangyuan_home/layout.ron";
#[cfg(test)]
const FANGYUAN_HOME_SCENE_MANIFEST_PATH: &str = "scenes/fangyuan_home/scene.ron";

pub(super) struct FangyuanHomePlugin;

impl Plugin for FangyuanHomePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<FangyuanHomeBlueprintRenderAssets>()
            .init_resource::<FangyuanHomeBlueprintStats>()
            .add_message::<FangyuanHomeBlueprintCommand>()
            .add_systems(
                Update,
                (
                    reset_fangyuan_home_blueprint_stats_on_exit,
                    handle_fangyuan_home_blueprint_commands,
                )
                    .chain(),
            )
            .add_systems(PostUpdate, instantiate_fangyuan_home_content);
    }
}

#[derive(Clone, Copy, Debug, Message, PartialEq, Eq)]
pub(in crate::game) enum FangyuanHomeBlueprintCommand {
    Reload,
    Clear,
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

    fn default_blueprint_path(&self) -> &str {
        let path = self.default_blueprint_path.trim();
        if path.is_empty() {
            FANGYUAN_HOME_DEFAULT_BLUEPRINT_PATH
        } else {
            path
        }
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

#[derive(Clone, Debug, Resource, Default)]
struct FangyuanHomeBlueprintRenderAssets {
    cache: FangyuanRenderAssetCache,
}

impl FangyuanHomeBlueprintRenderAssets {
    fn unit_mesh(
        &mut self,
        kind: FangyuanPrimitiveKind,
        meshes: &mut Assets<Mesh>,
    ) -> Handle<Mesh> {
        self.cache.unit_mesh(kind, meshes)
    }

    fn material(
        &mut self,
        color: Color,
        materials: &mut Assets<StandardMaterial>,
    ) -> Handle<StandardMaterial> {
        self.cache.material(color, materials)
    }

    fn material_count(&self) -> usize {
        self.cache.material_count()
    }

    #[cfg(test)]
    fn unit_cube_mesh(&self) -> Option<&Handle<Mesh>> {
        self.cache.unit_cube_mesh()
    }

    #[cfg(test)]
    fn unit_sphere_mesh(&self) -> Option<&Handle<Mesh>> {
        self.cache.unit_sphere_mesh()
    }
}

const FANGYUAN_HOME_BLUEPRINT_STATE_PENDING: &str = "pending";
const FANGYUAN_HOME_BLUEPRINT_STATE_LOADED: &str = "loaded";
const FANGYUAN_HOME_BLUEPRINT_STATE_CLEARED: &str = "cleared";
const FANGYUAN_HOME_BLUEPRINT_STATE_FAILED: &str = "failed";

#[derive(Clone, Debug, Resource, PartialEq, Eq)]
pub(in crate::game) struct FangyuanHomeBlueprintStats {
    pub(in crate::game) session_id: Option<SceneSessionId>,
    pub(in crate::game) primitive_stats: FangyuanPrimitiveSetStats,
    pub(in crate::game) skipped: usize,
    pub(in crate::game) materials: usize,
    pub(in crate::game) blueprint_path: String,
    pub(in crate::game) top_level_valid: bool,
    state: String,
}

impl Default for FangyuanHomeBlueprintStats {
    fn default() -> Self {
        Self {
            session_id: None,
            primitive_stats: FangyuanPrimitiveSetStats::default(),
            skipped: 0,
            materials: 0,
            blueprint_path: FANGYUAN_HOME_DEFAULT_BLUEPRINT_PATH.to_string(),
            top_level_valid: false,
            state: FANGYUAN_HOME_BLUEPRINT_STATE_PENDING.to_string(),
        }
    }
}

impl FangyuanHomeBlueprintStats {
    pub(in crate::game) fn record_loaded(
        &mut self,
        session_id: &SceneSessionId,
        blueprint_path: &str,
        primitive_set: &FangyuanPrimitiveSet,
        skipped: usize,
    ) {
        let primitive_stats = primitive_set.stats();
        self.session_id = Some(session_id.clone());
        self.primitive_stats = primitive_stats.clone();
        self.skipped = skipped;
        self.materials = primitive_stats.color_count;
        self.blueprint_path = blueprint_path.to_string();
        self.top_level_valid = true;
        self.state = FANGYUAN_HOME_BLUEPRINT_STATE_LOADED.to_string();
    }

    pub(in crate::game) fn record_failed(
        &mut self,
        session_id: &SceneSessionId,
        blueprint_path: &str,
        skipped: usize,
        materials: usize,
    ) {
        self.session_id = Some(session_id.clone());
        self.primitive_stats = FangyuanPrimitiveSetStats::default();
        self.skipped = skipped;
        self.materials = materials;
        self.blueprint_path = blueprint_path.to_string();
        self.top_level_valid = false;
        self.state = FANGYUAN_HOME_BLUEPRINT_STATE_FAILED.to_string();
    }

    pub(in crate::game) fn record_cleared(&mut self, session_id: &SceneSessionId) {
        let skipped = self.skipped;
        let materials = self.materials;
        let blueprint_path = self.blueprint_path().to_string();
        let top_level_valid = self.top_level_valid;
        self.session_id = Some(session_id.clone());
        self.primitive_stats = FangyuanPrimitiveSetStats::default();
        self.skipped = skipped;
        self.materials = materials;
        self.blueprint_path = blueprint_path;
        self.top_level_valid = top_level_valid;
        self.state = FANGYUAN_HOME_BLUEPRINT_STATE_CLEARED.to_string();
    }

    pub(in crate::game) fn primitive_total(&self) -> usize {
        self.primitive_stats.total
    }

    pub(in crate::game) fn state_label(&self) -> &str {
        if self.state.is_empty() {
            FANGYUAN_HOME_BLUEPRINT_STATE_PENDING
        } else {
            self.state.as_str()
        }
    }

    pub(in crate::game) fn blueprint_path(&self) -> &str {
        if self.blueprint_path.trim().is_empty() {
            FANGYUAN_HOME_DEFAULT_BLUEPRINT_PATH
        } else {
            self.blueprint_path.as_str()
        }
    }

    fn reset_if_session(&mut self, session_id: &SceneSessionId) -> bool {
        if self
            .session_id
            .as_ref()
            .is_some_and(|stats_session_id| stats_session_id == session_id)
        {
            *self = Self::default();
            true
        } else {
            false
        }
    }
}

#[derive(Clone, Debug, Component, PartialEq, Eq)]
struct FangyuanHomeContent {
    session_id: SceneSessionId,
}

#[derive(Clone, Debug, Component, PartialEq, Eq)]
struct FangyuanHomeBlueprintContent {
    session_id: SceneSessionId,
}

#[derive(Clone, Debug, Component, PartialEq, Eq)]
struct FangyuanHomeObject {
    session_id: SceneSessionId,
}

#[derive(Clone, Debug, Component, PartialEq)]
struct FangyuanHomeBlueprintPrimitiveVisual {
    session_id: SceneSessionId,
    kind: FangyuanPrimitiveKind,
    index: usize,
    alpha: f32,
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
    mut blueprint_assets: ResMut<FangyuanHomeBlueprintRenderAssets>,
    mut blueprint_stats: ResMut<FangyuanHomeBlueprintStats>,
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
            &mut blueprint_assets,
            &mut blueprint_stats,
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
    blueprint_assets: &mut FangyuanHomeBlueprintRenderAssets,
    blueprint_stats: &mut FangyuanHomeBlueprintStats,
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
    spawn_fangyuan_home_blueprint_from_layout(
        commands,
        content,
        session_id,
        layout,
        meshes,
        materials,
        blueprint_assets,
        blueprint_stats,
    );

    content
}

fn spawn_fangyuan_home_blueprint_from_layout(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    layout: &FangyuanHomeLayout,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    blueprint_assets: &mut FangyuanHomeBlueprintRenderAssets,
    blueprint_stats: &mut FangyuanHomeBlueprintStats,
) -> Option<Entity> {
    spawn_fangyuan_home_blueprint_from_layout_with_loader(
        commands,
        parent,
        session_id,
        layout,
        meshes,
        materials,
        blueprint_assets,
        blueprint_stats,
        |blueprint_path| {
            FangyuanBlueprint::load_first_package_ron(blueprint_path)
                .map_err(|error| error.to_string())
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn spawn_fangyuan_home_blueprint_from_layout_with_loader(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    layout: &FangyuanHomeLayout,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    blueprint_assets: &mut FangyuanHomeBlueprintRenderAssets,
    blueprint_stats: &mut FangyuanHomeBlueprintStats,
    load_blueprint: impl FnOnce(&str) -> Result<FangyuanBlueprint, String>,
) -> Option<Entity> {
    let blueprint_path = layout.default_blueprint_path();
    if blueprint_path.trim().is_empty() {
        warn!("skipping fangyuan home blueprint because default_blueprint_path is empty");
        blueprint_stats.record_failed(
            session_id,
            blueprint_path,
            0,
            blueprint_assets.material_count(),
        );
        log_fangyuan_home_blueprint_stats(blueprint_stats);
        return None;
    }

    let blueprint = match load_blueprint(blueprint_path) {
        Ok(blueprint) => blueprint,
        Err(error) => {
            warn!("{error}");
            blueprint_stats.record_failed(
                session_id,
                blueprint_path,
                0,
                blueprint_assets.material_count(),
            );
            log_fangyuan_home_blueprint_stats(blueprint_stats);
            return None;
        }
    };
    let compile_report = match blueprint.compile_skipping_invalid_primitives() {
        Ok(report) => report,
        Err(error) => {
            warn!("{error}");
            blueprint_stats.record_failed(
                session_id,
                blueprint_path,
                blueprint.primitives.len(),
                blueprint_assets.material_count(),
            );
            log_fangyuan_home_blueprint_stats(blueprint_stats);
            return None;
        }
    };
    for warning in &compile_report.warnings {
        warn!("skipping fangyuan home blueprint primitive: {warning}");
    }

    let content = spawn_fangyuan_home_blueprint_content(
        commands,
        parent,
        session_id,
        &compile_report.primitive_set,
        meshes,
        materials,
        blueprint_assets,
    );
    blueprint_stats.record_loaded(
        session_id,
        blueprint_path,
        &compile_report.primitive_set,
        compile_report.skipped_primitives,
    );
    log_fangyuan_home_blueprint_stats(blueprint_stats);
    Some(content)
}

fn spawn_fangyuan_home_blueprint_content(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    primitive_set: &FangyuanPrimitiveSet,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    blueprint_assets: &mut FangyuanHomeBlueprintRenderAssets,
) -> Entity {
    let content = commands
        .spawn((
            SceneOwned::new(session_id.clone()),
            FangyuanHomeBlueprintContent {
                session_id: session_id.clone(),
            },
            FangyuanHomeObject {
                session_id: session_id.clone(),
            },
            primitive_set.clone(),
            FangyuanObjectState::default(),
            Transform::default(),
            Name::new(format!("FangyuanHomeObject({session_id})")),
        ))
        .id();
    commands.entity(parent).add_child(content);

    for (index, primitive) in primitive_set.primitives().iter().enumerate() {
        spawn_fangyuan_home_blueprint_primitive(
            commands,
            content,
            session_id,
            index,
            primitive,
            meshes,
            materials,
            blueprint_assets,
        );
    }

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
            MeshMaterial3d(materials.add(fangyuan_standard_material_from_color(color))),
            NoAutomaticBatching,
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

fn spawn_fangyuan_home_blueprint_primitive(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    index: usize,
    primitive: &FangyuanPrimitive,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    blueprint_assets: &mut FangyuanHomeBlueprintRenderAssets,
) -> Entity {
    let mesh = blueprint_assets.unit_mesh(primitive.kind, meshes);
    let material = blueprint_assets.material(primitive.color, materials);
    let transform = fangyuan_render_transform_from_primitive(primitive);
    let entity = commands
        .spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            NoAutomaticBatching,
            transform,
            SceneOwned::new(session_id.clone()),
            FangyuanHomeBlueprintPrimitiveVisual {
                session_id: session_id.clone(),
                kind: primitive.kind,
                index,
                alpha: primitive.alpha,
            },
            Name::new(format!(
                "FangyuanHomeBlueprintPrimitive({}:{})",
                primitive.kind.as_str(),
                index
            )),
        ))
        .id();
    commands.entity(parent).add_child(entity);
    entity
}

#[allow(dead_code)]
fn clear_fangyuan_home_blueprint_content<'world>(
    commands: &mut Commands,
    session_id: &SceneSessionId,
    blueprint_content: impl IntoIterator<Item = (Entity, &'world FangyuanHomeBlueprintContent)>,
) -> usize {
    let mut cleared = 0;
    for (entity, content) in blueprint_content {
        if content.session_id == *session_id {
            commands.entity(entity).try_despawn();
            cleared += 1;
        }
    }
    cleared
}

#[allow(clippy::too_many_arguments)]
fn handle_fangyuan_home_blueprint_commands(
    mut commands: Commands,
    mut blueprint_commands: MessageReader<FangyuanHomeBlueprintCommand>,
    content_roots: Query<
        (Entity, &FangyuanHomeContent),
        (
            Without<FangyuanHomeBlueprintContent>,
            Without<FangyuanHomeVisual>,
        ),
    >,
    blueprint_content: Query<(Entity, &FangyuanHomeBlueprintContent)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut blueprint_assets: ResMut<FangyuanHomeBlueprintRenderAssets>,
    mut blueprint_stats: ResMut<FangyuanHomeBlueprintStats>,
) {
    let mut requested_command = None;
    for command in blueprint_commands.read() {
        requested_command = Some(*command);
    }

    let Some(command) = requested_command else {
        return;
    };
    let Some(session_id) = blueprint_stats.session_id.clone() else {
        warn!(
            "ignoring fangyuan home blueprint command because no active blueprint session exists"
        );
        return;
    };

    match command {
        FangyuanHomeBlueprintCommand::Clear => {
            clear_fangyuan_home_blueprint_content(
                &mut commands,
                &session_id,
                blueprint_content.iter(),
            );
            blueprint_stats.materials = blueprint_assets.material_count();
            blueprint_stats.record_cleared(&session_id);
        }
        FangyuanHomeBlueprintCommand::Reload => {
            clear_fangyuan_home_blueprint_content(
                &mut commands,
                &session_id,
                blueprint_content.iter(),
            );

            let layout = match FangyuanHomeLayout::load_first_package_ron(FANGYUAN_HOME_LAYOUT_PATH)
            {
                Ok(layout) => layout,
                Err(error) => {
                    warn!("{error}");
                    let blueprint_path = blueprint_stats.blueprint_path().to_string();
                    blueprint_stats.record_failed(
                        &session_id,
                        &blueprint_path,
                        0,
                        blueprint_assets.material_count(),
                    );
                    return;
                }
            };

            if !layout.is_scene_id_valid() {
                warn!(
                    "skipping fangyuan home blueprint reload because layout scene_id `{}` does not match `{}`",
                    layout.scene_id, FANGYUAN_HOME_SCENE_ID
                );
                let blueprint_path = layout.default_blueprint_path();
                blueprint_stats.record_failed(
                    &session_id,
                    blueprint_path,
                    0,
                    blueprint_assets.material_count(),
                );
                return;
            }

            let Some((content_root, _)) = content_roots
                .iter()
                .find(|(_, content)| content.session_id == session_id)
            else {
                warn!(
                    "skipping fangyuan home blueprint reload because session `{}` has no content root",
                    session_id
                );
                let blueprint_path = layout.default_blueprint_path();
                blueprint_stats.record_failed(
                    &session_id,
                    blueprint_path,
                    0,
                    blueprint_assets.material_count(),
                );
                return;
            };

            spawn_fangyuan_home_blueprint_from_layout(
                &mut commands,
                content_root,
                &session_id,
                &layout,
                &mut meshes,
                &mut materials,
                &mut blueprint_assets,
                &mut blueprint_stats,
            );
        }
    }
}

fn reset_fangyuan_home_blueprint_stats_on_exit(
    mut scene_events: MessageReader<SceneEvent>,
    mut blueprint_stats: ResMut<FangyuanHomeBlueprintStats>,
) {
    for event in scene_events.read() {
        let SceneEvent::Exited(exited) = event else {
            continue;
        };

        if exited.scene_id.as_str() != FANGYUAN_HOME_SCENE_ID {
            continue;
        }

        if blueprint_stats.reset_if_session(&exited.session_id) {
            info!(
                "fangyuan home blueprint stats reset after scene exit: session={}",
                exited.session_id
            );
        }
    }
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

fn log_fangyuan_home_blueprint_stats(stats: &FangyuanHomeBlueprintStats) {
    let session = stats
        .session_id
        .as_ref()
        .map(SceneSessionId::as_str)
        .unwrap_or("<none>");
    info!(
        "fangyuan home blueprint stats: session={session}, state={}, path={}, primitives={}, cubes={}, spheres={}, skipped={}, materials={}, top_level_valid={}",
        stats.state_label(),
        stats.blueprint_path(),
        stats.primitive_total(),
        stats.primitive_stats.cube_count,
        stats.primitive_stats.sphere_count,
        stats.skipped,
        stats.materials,
        stats.top_level_valid
    );
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
    first_package_asset_fs_path(layout_path)
}

fn first_package_asset_fs_path(asset_path: &str) -> Option<PathBuf> {
    first_package_asset_root_candidates()
        .into_iter()
        .map(|root| root.join(Path::new(asset_path)))
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
    use crate::framework::fangyuan::{
        FANGYUAN_BLUEPRINT_HARD_PRIMITIVE_LIMIT, FANGYUAN_BLUEPRINT_VERSION,
        FangyuanBlueprintCompileReport, FangyuanPrimitiveBlueprint, FangyuanPrimitiveLifecycle,
        FangyuanPrimitiveRole, FangyuanRenderColorKey,
    };
    use crate::framework::scene::prelude::{
        SceneCameraMode, SceneCameraProjection, SceneCommand, SceneEnterRequest, SceneExitRequest,
        SceneManifest, ScenePlugin, SceneRegistry, SceneRoot, SceneRuntime, SceneRuntimeRoot,
        spawn_scene_root, spawn_scene_runtime_root,
    };
    use bevy::{asset::AssetPlugin, ecs::system::SystemState, mesh::VertexAttributeValues};

    const EXPECTED_GRID_VISUALS: usize = 50;
    const EXPECTED_BOUNDARY_VISUALS: usize = 4;
    const EXPECTED_LIGHT_VISUALS: usize = 2;
    const EXPECTED_DEFAULT_BLUEPRINT_PRIMITIVES: usize = 505;
    const EXPECTED_DEFAULT_BLUEPRINT_GENERATED: usize = 493;
    const EXPECTED_DEFAULT_BLUEPRINT_SKIPPED: usize = 12;
    const EXPECTED_TOTAL_VISUALS: usize =
        1 + EXPECTED_GRID_VISUALS + EXPECTED_BOUNDARY_VISUALS + EXPECTED_LIGHT_VISUALS;

    fn app_with_fangyuan_home_system() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<FangyuanHomeBlueprintRenderAssets>()
            .init_resource::<FangyuanHomeBlueprintStats>()
            .add_message::<SceneEvent>()
            .add_message::<FangyuanHomeBlueprintCommand>()
            .add_systems(
                Update,
                (
                    reset_fangyuan_home_blueprint_stats_on_exit,
                    instantiate_fangyuan_home_content,
                    handle_fangyuan_home_blueprint_commands,
                )
                    .chain(),
            );
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
    fn load_default_blueprint_from_first_package_assets() {
        let layout = FangyuanHomeLayout::load_first_package_ron(FANGYUAN_HOME_LAYOUT_PATH).unwrap();
        let blueprint =
            FangyuanBlueprint::load_first_package_ron(&layout.default_blueprint_path).unwrap();
        let compile_report = blueprint.compile_skipping_invalid_primitives().unwrap();

        assert_eq!(blueprint.version, FANGYUAN_BLUEPRINT_VERSION);
        assert_eq!(blueprint.name, "home_preview");
        assert_eq!(
            blueprint.max_primitives,
            FANGYUAN_BLUEPRINT_HARD_PRIMITIVE_LIMIT
        );
        assert_eq!(
            blueprint.bounds,
            crate::framework::fangyuan::FangyuanBlueprintBounds::new(40.0, 40.0, 20.0)
        );
        assert_eq!(
            blueprint.primitives.len(),
            EXPECTED_DEFAULT_BLUEPRINT_PRIMITIVES
        );
        assert!(blueprint.primitives.len() <= FANGYUAN_BLUEPRINT_HARD_PRIMITIVE_LIMIT);
        assert_eq!(
            compile_report.skipped_primitives,
            EXPECTED_DEFAULT_BLUEPRINT_SKIPPED
        );
        assert_eq!(
            compile_report.primitive_set.len(),
            EXPECTED_DEFAULT_BLUEPRINT_GENERATED
        );
        assert!(
            compile_report
                .primitive_set
                .primitives()
                .iter()
                .any(|primitive| { primitive.kind == FangyuanPrimitiveKind::Cube })
        );
        assert!(
            compile_report
                .primitive_set
                .primitives()
                .iter()
                .any(|primitive| { primitive.kind == FangyuanPrimitiveKind::Sphere })
        );
    }

    #[test]
    fn invalid_blueprint_version_or_count_does_not_validate_primitives() {
        let invalid_version = blueprint_with_primitives(vec![valid_cube_primitive()]);
        let invalid_version = FangyuanBlueprint {
            version: "2".to_string(),
            ..invalid_version
        };
        let invalid_version_result = invalid_version.compile_skipping_invalid_primitives();

        assert!(invalid_version_result.is_err());
        assert_eq!(
            invalid_version_result.unwrap_err().code(),
            "unsupported_version"
        );

        let overflow = FangyuanBlueprint {
            max_primitives: 1,
            primitives: vec![valid_cube_primitive(), valid_sphere_primitive()],
            ..blueprint_with_primitives(Vec::new())
        };
        let overflow_result = overflow.compile_skipping_invalid_primitives();

        assert!(overflow_result.is_err());
        assert_eq!(
            overflow_result.unwrap_err().code(),
            "primitive_count_exceeded"
        );
    }

    #[test]
    fn invalid_blueprint_primitives_are_skipped_and_valid_primitives_remain() {
        let blueprint = blueprint_with_primitives(vec![
            below_ground_primitive(),
            invalid_position_primitive(),
            invalid_size_primitive(),
            invalid_color_primitive(),
            valid_cube_primitive(),
            valid_sphere_primitive(),
        ]);
        let compile_report = blueprint.compile_skipping_invalid_primitives().unwrap();

        assert_eq!(compile_report.primitive_set.len(), 2);
        assert_eq!(compile_report.warnings.len(), 4);
        assert_eq!(compile_report.skipped_primitives, 4);
        assert_eq!(
            compile_report.primitive_set.primitives()[0].kind,
            FangyuanPrimitiveKind::Cube
        );
        assert_eq!(
            compile_report.primitive_set.primitives()[1].kind,
            FangyuanPrimitiveKind::Sphere
        );
        assert!(
            compile_report
                .warnings
                .iter()
                .any(|warning| warning.code() == "primitive_below_ground")
        );
        assert!(
            compile_report
                .warnings
                .iter()
                .any(|warning| warning.code() == "invalid_primitive_position")
        );
        assert!(
            compile_report
                .warnings
                .iter()
                .any(|warning| warning.code() == "invalid_primitive_size")
        );
        assert!(
            compile_report
                .warnings
                .iter()
                .any(|warning| warning.code() == "invalid_primitive_color")
        );
    }

    #[test]
    fn reserved_runtime_metadata_errors_are_skipped_and_counted_for_home_preview() {
        let blueprint = blueprint_with_primitives(vec![
            invalid_alpha_primitive(),
            invalid_emissive_primitive(),
            invalid_material_profile_primitive(),
            invalid_lifecycle_primitive(),
            valid_cube_primitive(),
        ]);
        let compile_report = blueprint.compile_skipping_invalid_primitives().unwrap();

        assert_eq!(compile_report.primitive_set.len(), 1);
        assert_eq!(compile_report.skipped_primitives, 4);
        assert!(
            compile_report
                .warnings
                .iter()
                .any(|warning| warning.code() == "invalid_primitive_alpha")
        );
        assert!(
            compile_report
                .warnings
                .iter()
                .any(|warning| warning.code() == "invalid_primitive_emissive")
        );
        assert!(
            compile_report
                .warnings
                .iter()
                .any(|warning| warning.code() == "invalid_primitive_material_profile")
        );
        assert!(
            compile_report
                .warnings
                .iter()
                .any(|warning| warning.code() == "invalid_primitive_lifecycle")
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
    fn entered_fangyuan_home_spawns_base_space_and_blueprint_under_runtime_root() {
        let mut app = app_with_fangyuan_home_system();
        let default_compile_report = default_blueprint_compile_report();

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
        ), (
            Without<FangyuanHomeVisual>,
            Without<FangyuanHomeBlueprintContent>,
        )>();
        let content_entities = content.iter(app.world()).collect::<Vec<_>>();
        assert_eq!(content_entities.len(), 1);

        let (content_entity, parent, owned, content, transform, name) = content_entities[0];
        assert_eq!(parent.parent(), runtime_root);
        assert_eq!(owned.session_id, session_id);
        assert_eq!(content.session_id, session_id);
        assert_eq!(transform, &Transform::default());
        assert_eq!(name.as_str(), "FangyuanHomeContent(fangyuan-session)");

        let mut blueprint_content = app.world_mut().query::<(
            Entity,
            &ChildOf,
            &SceneOwned,
            &FangyuanHomeBlueprintContent,
            &FangyuanHomeObject,
            &FangyuanPrimitiveSet,
            &FangyuanObjectState,
            &Transform,
            &Name,
        )>();
        let blueprint_content_entities = blueprint_content.iter(app.world()).collect::<Vec<_>>();
        assert_eq!(blueprint_content_entities.len(), 1);

        let (
            blueprint_entity,
            parent,
            owned,
            blueprint_content,
            home_object,
            primitive_set,
            object_state,
            transform,
            name,
        ) = blueprint_content_entities[0];
        assert_eq!(parent.parent(), content_entity);
        assert_eq!(owned.session_id, session_id);
        assert_eq!(blueprint_content.session_id, session_id);
        assert_eq!(home_object.session_id, session_id);
        assert_eq!(primitive_set, &default_compile_report.primitive_set);
        assert_eq!(object_state, &FangyuanObjectState::default());
        assert_eq!(transform, &Transform::default());
        assert_eq!(name.as_str(), "FangyuanHomeObject(fangyuan-session)");

        let mut visuals = app.world_mut().query::<(
            Entity,
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
        for (entity, parent, owned, content, visual, name) in visual_entities {
            assert_eq!(parent.parent(), content_entity);
            assert_eq!(owned.session_id, session_id);
            assert_eq!(content.session_id, session_id);
            assert!(name.as_str().starts_with("FangyuanHome"));
            match visual {
                FangyuanHomeVisual::Plane => {
                    plane_count += 1;
                    assert!(app.world().entity(entity).contains::<NoAutomaticBatching>());
                }
                FangyuanHomeVisual::Grid => {
                    grid_count += 1;
                    assert!(app.world().entity(entity).contains::<NoAutomaticBatching>());
                }
                FangyuanHomeVisual::Boundary => {
                    boundary_count += 1;
                    assert!(app.world().entity(entity).contains::<NoAutomaticBatching>());
                }
                FangyuanHomeVisual::DirectionalLight => directional_light_count += 1,
                FangyuanHomeVisual::PointLight => point_light_count += 1,
            }
        }
        assert_eq!(plane_count, 1);
        assert_eq!(grid_count, EXPECTED_GRID_VISUALS);
        assert_eq!(boundary_count, EXPECTED_BOUNDARY_VISUALS);
        assert_eq!(directional_light_count, 1);
        assert_eq!(point_light_count, 1);

        let mut blueprint_primitives = app.world_mut().query::<(
            &ChildOf,
            &SceneOwned,
            &FangyuanHomeBlueprintPrimitiveVisual,
            &Transform,
            &Mesh3d,
            &MeshMaterial3d<StandardMaterial>,
            &NoAutomaticBatching,
            &Name,
        )>();
        let blueprint_primitive_entities =
            blueprint_primitives.iter(app.world()).collect::<Vec<_>>();
        assert_eq!(
            blueprint_primitive_entities.len(),
            EXPECTED_DEFAULT_BLUEPRINT_GENERATED
        );
        let mut cube_count = 0;
        let mut sphere_count = 0;
        let mut cube_mesh: Option<Handle<Mesh>> = None;
        let mut sphere_mesh: Option<Handle<Mesh>> = None;
        let mut materials_by_color: std::collections::HashMap<
            FangyuanRenderColorKey,
            Handle<StandardMaterial>,
        > = std::collections::HashMap::new();
        for (parent, owned, primitive, transform, mesh, material, _, name) in
            blueprint_primitive_entities
        {
            assert_eq!(parent.parent(), blueprint_entity);
            assert_eq!(owned.session_id, session_id);
            assert_eq!(primitive.session_id, session_id);
            assert!(primitive.index < EXPECTED_DEFAULT_BLUEPRINT_GENERATED);
            assert!(name.as_str().starts_with("FangyuanHomeBlueprintPrimitive("));
            let expected_primitive =
                &default_compile_report.primitive_set.primitives()[primitive.index];
            assert_eq!(primitive.kind, expected_primitive.kind);
            assert_eq!(primitive.alpha, expected_primitive.alpha);
            assert_eq!(transform.translation, expected_primitive.local_position);
            assert_eq!(transform.scale, expected_primitive.scale);
            assert_eq!(transform.rotation, Quat::IDENTITY);
            assert!(
                app.world()
                    .resource::<Assets<Mesh>>()
                    .get(&mesh.0)
                    .is_some(),
                "blueprint primitive mesh should be inserted"
            );
            let material_key = FangyuanRenderColorKey::from_color(expected_primitive.color);
            match materials_by_color.get(&material_key) {
                Some(existing_material) => assert_eq!(&material.0, existing_material),
                None => {
                    materials_by_color.insert(material_key, material.0.clone());
                }
            }
            match primitive.kind {
                FangyuanPrimitiveKind::Cube => {
                    cube_count += 1;
                    if let Some(cube_mesh) = &cube_mesh {
                        assert_eq!(&mesh.0, cube_mesh);
                    } else {
                        cube_mesh = Some(mesh.0.clone());
                    }
                }
                FangyuanPrimitiveKind::Sphere => {
                    sphere_count += 1;
                    if let Some(sphere_mesh) = &sphere_mesh {
                        assert_eq!(&mesh.0, sphere_mesh);
                    } else {
                        sphere_mesh = Some(mesh.0.clone());
                    }
                }
            }
        }
        assert!(cube_count > 0);
        assert!(sphere_count > 0);
        let cube_mesh = cube_mesh.expect("default blueprint should include cubes");
        let sphere_mesh = sphere_mesh.expect("default blueprint should include spheres");
        assert_ne!(cube_mesh, sphere_mesh);
        let render_assets = app.world().resource::<FangyuanHomeBlueprintRenderAssets>();
        assert_eq!(render_assets.unit_cube_mesh(), Some(&cube_mesh));
        assert_eq!(render_assets.unit_sphere_mesh(), Some(&sphere_mesh));
        assert_eq!(
            mesh_position_size(
                app.world()
                    .resource::<Assets<Mesh>>()
                    .get(&cube_mesh)
                    .unwrap()
            ),
            Vec3::ONE
        );
        assert_eq!(
            mesh_position_size(
                app.world()
                    .resource::<Assets<Mesh>>()
                    .get(&sphere_mesh)
                    .unwrap()
            ),
            Vec3::ONE
        );
        assert!(materials_by_color.len() > 1);
        assert_eq!(
            app.world()
                .resource::<FangyuanHomeBlueprintRenderAssets>()
                .material_count(),
            materials_by_color.len()
        );
        assert_eq!(
            app.world().resource::<FangyuanHomeBlueprintStats>(),
            &expected_loaded_blueprint_stats(&session_id, &default_compile_report)
        );
        assert_eq!(
            app.world()
                .resource::<FangyuanHomeBlueprintStats>()
                .materials,
            materials_by_color.len()
        );

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

        let mut content = app.world_mut().query_filtered::<&FangyuanHomeContent, (
            Without<FangyuanHomeVisual>,
            Without<FangyuanHomeBlueprintContent>,
        )>();
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
        assert_eq!(fangyuan_blueprint_content_count(&mut app, &session_id), 1);
        assert_eq!(fangyuan_home_object_count(&mut app, &session_id), 1);
        assert_eq!(
            fangyuan_blueprint_primitive_count(&mut app, &session_id),
            EXPECTED_DEFAULT_BLUEPRINT_GENERATED
        );
    }

    #[test]
    fn blueprint_primitives_reuse_meshes_and_materials_without_runtime_components() {
        let mut app = app_with_fangyuan_home_system();
        let session_id = SceneSessionId::from("fangyuan-reuse-session");
        let parent = app.world_mut().spawn_empty().id();
        let blueprint = blueprint_with_primitives(vec![
            cube_primitive_at(-2.0, [1.0, 2.0, 3.0], [0.25, 0.35, 0.45, 1.0]),
            cube_primitive_at(-1.0, [0.8, 1.2, 1.4], [0.25, 0.35, 0.45, 1.0]),
            cube_primitive_at(0.0, [1.4, 0.9, 0.7], [0.85, 0.55, 0.25, 1.0]),
            sphere_primitive_at(1.0, [1.0, 1.0, 1.0], [0.25, 0.35, 0.45, 1.0]),
            sphere_primitive_at(2.0, [1.5, 1.6, 1.7], [0.85, 0.55, 0.25, 1.0]),
        ]);
        let compile_report = blueprint.compile_skipping_invalid_primitives().unwrap();
        assert_eq!(compile_report.skipped_primitives, 0);

        let content = spawn_blueprint_content_for_test(
            &mut app,
            parent,
            &session_id,
            &compile_report.primitive_set,
        );
        assert_ne!(content, parent);

        let primitive_records = blueprint_primitive_records(&mut app, &session_id);
        assert_eq!(primitive_records.len(), 5);
        assert_eq!(
            app.world()
                .resource::<FangyuanHomeBlueprintRenderAssets>()
                .material_count(),
            2
        );

        let cube_meshes = primitive_records
            .iter()
            .filter(|record| record.kind == FangyuanPrimitiveKind::Cube)
            .map(|record| record.mesh.clone())
            .collect::<Vec<_>>();
        let sphere_meshes = primitive_records
            .iter()
            .filter(|record| record.kind == FangyuanPrimitiveKind::Sphere)
            .map(|record| record.mesh.clone())
            .collect::<Vec<_>>();
        assert_eq!(cube_meshes.len(), 3);
        assert_eq!(sphere_meshes.len(), 2);
        assert!(cube_meshes.windows(2).all(|pair| pair[0] == pair[1]));
        assert!(sphere_meshes.windows(2).all(|pair| pair[0] == pair[1]));
        assert_ne!(cube_meshes[0], sphere_meshes[0]);

        assert_eq!(
            primitive_records[0].material, primitive_records[1].material,
            "same RGBA color should reuse a material"
        );
        assert_eq!(
            primitive_records[0].material, primitive_records[3].material,
            "same RGBA color should reuse across primitive kinds"
        );
        assert_ne!(
            primitive_records[0].material, primitive_records[2].material,
            "different RGBA colors should use different materials"
        );
        assert_eq!(
            primitive_records[2].material, primitive_records[4].material,
            "matching alternate RGBA color should reuse a material"
        );

        let mut entity_query = app.world_mut().query::<(
            Entity,
            &FangyuanHomeBlueprintPrimitiveVisual,
            Option<&FangyuanHomeVisual>,
            Option<&FangyuanHomeContent>,
            Option<&FangyuanHomeBlueprintContent>,
            Option<&FangyuanHomeObject>,
            Option<&FangyuanPrimitiveSet>,
            Option<&FangyuanObjectState>,
        )>();
        for (
            entity,
            primitive,
            visual,
            content,
            blueprint_content,
            home_object,
            primitive_set,
            object_state,
        ) in entity_query.iter(app.world())
        {
            if primitive.session_id != session_id {
                continue;
            }
            let entity_ref = app.world().entity(entity);
            assert!(entity_ref.contains::<Mesh3d>());
            assert!(entity_ref.contains::<MeshMaterial3d<StandardMaterial>>());
            assert!(entity_ref.contains::<NoAutomaticBatching>());
            assert!(entity_ref.contains::<Transform>());
            assert!(entity_ref.contains::<SceneOwned>());
            assert!(
                visual.is_none(),
                "primitive entities must not carry base visual/runtime markers"
            );
            assert!(
                content.is_none(),
                "primitive entities must not carry base content markers"
            );
            assert!(
                blueprint_content.is_none(),
                "primitive entities must not carry blueprint content markers"
            );
            assert!(
                home_object.is_none(),
                "primitive entities must not carry home object markers"
            );
            assert!(
                primitive_set.is_none(),
                "primitive entities must not own the logical primitive set"
            );
            assert!(
                object_state.is_none(),
                "primitive entities must not carry Fangyuan object state"
            );
        }
    }

    #[test]
    fn blueprint_primitives_map_runtime_fields_and_ignore_reserved_metadata_for_material_cache() {
        let mut app = app_with_fangyuan_home_system();
        let session_id = SceneSessionId::from("fangyuan-runtime-field-session");
        let parent = app.world_mut().spawn_empty().id();
        let color = Color::srgba(0.2, 0.4, 0.6, 0.35);
        let primitive_set = FangyuanPrimitiveSet::from_primitives(vec![
            FangyuanPrimitive::with_runtime_metadata(
                FangyuanPrimitiveKind::Cube,
                Vec3::new(-1.0, 0.5, 0.25),
                Vec3::new(1.0, 2.0, 3.0),
                color,
                FangyuanPrimitiveRole::Structure,
                0.25,
                0.0,
                None,
                FangyuanPrimitiveLifecycle::empty(),
            ),
            FangyuanPrimitive::with_runtime_metadata(
                FangyuanPrimitiveKind::Sphere,
                Vec3::new(1.0, 1.5, -0.25),
                Vec3::splat(0.75),
                color,
                FangyuanPrimitiveRole::Decoration,
                0.75,
                4.0,
                Some("home_reserved_profile".to_string()),
                FangyuanPrimitiveLifecycle::new(Some(20), Some(2), Some(22)),
            ),
        ]);

        spawn_blueprint_content_for_test(&mut app, parent, &session_id, &primitive_set);

        let mut primitives = app.world_mut().query::<(
            &FangyuanHomeBlueprintPrimitiveVisual,
            &Transform,
            &MeshMaterial3d<StandardMaterial>,
        )>();
        let mut records = primitives
            .iter(app.world())
            .filter(|(visual, _, _)| visual.session_id == session_id)
            .map(|(visual, transform, material)| {
                (
                    visual.index,
                    visual.alpha,
                    transform.translation,
                    transform.rotation,
                    transform.scale,
                    material.0.clone(),
                )
            })
            .collect::<Vec<_>>();
        records.sort_by_key(|record| record.0);

        assert_eq!(records.len(), 2);
        assert_eq!(
            app.world()
                .resource::<FangyuanHomeBlueprintRenderAssets>()
                .material_count(),
            1
        );
        for (expected_index, ((index, alpha, translation, rotation, scale, _), primitive)) in
            records.iter().zip(primitive_set.primitives()).enumerate()
        {
            assert_eq!(*index, expected_index);
            assert_eq!(*alpha, primitive.alpha);
            assert_eq!(*translation, primitive.local_position);
            assert_eq!(*rotation, Quat::IDENTITY);
            assert_eq!(*scale, primitive.scale);
        }
        assert_eq!(records[0].5, records[1].5);

        let material = app
            .world()
            .resource::<Assets<StandardMaterial>>()
            .get(&records[0].5)
            .unwrap();
        assert_eq!(material.base_color, color);
        assert!(matches!(material.alpha_mode.clone(), AlphaMode::Blend));
    }

    #[test]
    fn generated_blueprint_stats_record_default_counts() {
        let mut app = app_with_fangyuan_home_system();
        let session_id = spawn_and_enter_fangyuan_home(&mut app, "fangyuan-stats-session");

        let compile_report = default_blueprint_compile_report();
        assert_eq!(
            app.world().resource::<FangyuanHomeBlueprintStats>(),
            &expected_loaded_blueprint_stats(&session_id, &compile_report)
        );
        let stats = app.world().resource::<FangyuanHomeBlueprintStats>();
        assert_eq!(
            stats.primitive_total(),
            EXPECTED_DEFAULT_BLUEPRINT_GENERATED
        );
        assert_eq!(stats.primitive_stats, compile_report.primitive_set.stats());
        assert_eq!(
            stats.materials,
            unique_material_count(&compile_report.primitive_set)
        );
        assert_eq!(stats.skipped, EXPECTED_DEFAULT_BLUEPRINT_SKIPPED);
        assert_eq!(stats.blueprint_path(), FANGYUAN_HOME_DEFAULT_BLUEPRINT_PATH);
        assert_eq!(stats.state_label(), FANGYUAN_HOME_BLUEPRINT_STATE_LOADED);
    }

    #[test]
    fn blueprint_stats_reset_after_active_session_exits() {
        let mut app = app_with_fangyuan_home_system();
        let session_id = spawn_and_enter_fangyuan_home(&mut app, "fangyuan-stats-exit-session");

        assert!(
            app.world()
                .resource::<FangyuanHomeBlueprintStats>()
                .primitive_total()
                > 0
        );

        app.world_mut().write_message(SceneEvent::Exited(
            crate::framework::scene::prelude::SceneExited {
                scene_id: FANGYUAN_HOME_SCENE_ID.into(),
                session_id: SceneSessionId::from("other-fangyuan-session"),
            },
        ));
        app.update();

        assert_eq!(
            app.world()
                .resource::<FangyuanHomeBlueprintStats>()
                .session_id
                .as_ref(),
            Some(&session_id)
        );

        app.world_mut().write_message(SceneEvent::Exited(
            crate::framework::scene::prelude::SceneExited {
                scene_id: FANGYUAN_HOME_SCENE_ID.into(),
                session_id: session_id.clone(),
            },
        ));
        app.update();

        assert_eq!(
            app.world().resource::<FangyuanHomeBlueprintStats>(),
            &FangyuanHomeBlueprintStats::default()
        );
    }

    #[test]
    fn near_thousand_primitive_blueprint_generates_clears_and_exits() {
        const PRESSURE_PRIMITIVES: usize = 990;

        let mut app = app_with_scene_lifecycle();
        let session_id = SceneSessionId::from("fangyuan-pressure-session");
        let mut request = SceneEnterRequest::new(FANGYUAN_HOME_SCENE_ID);
        request.session_id = Some(session_id.clone());
        app.world_mut().write_message(SceneCommand::Enter(request));
        app.update();
        assert_eq!(fangyuan_content_count(&mut app, &session_id), 1);
        assert_eq!(fangyuan_home_object_count(&mut app, &session_id), 1);

        let blueprint = pressure_blueprint(PRESSURE_PRIMITIVES);
        let compile_report = blueprint.compile_skipping_invalid_primitives().unwrap();
        assert_eq!(compile_report.primitive_set.len(), PRESSURE_PRIMITIVES);
        assert_eq!(compile_report.skipped_primitives, 0);

        let base_content = fangyuan_content_entity(&mut app, &session_id)
            .expect("fangyuan content root should exist before pressure preview");
        clear_blueprint_content_once(&mut app, &session_id);
        assert_eq!(fangyuan_home_object_count(&mut app, &session_id), 0);
        assert_eq!(fangyuan_blueprint_primitive_count(&mut app, &session_id), 0);

        spawn_blueprint_content_for_test(
            &mut app,
            base_content,
            &session_id,
            &compile_report.primitive_set,
        );

        assert_eq!(fangyuan_home_object_count(&mut app, &session_id), 1);
        assert_eq!(
            fangyuan_blueprint_primitive_count(&mut app, &session_id),
            PRESSURE_PRIMITIVES
        );
        let cached_materials = app
            .world()
            .resource::<FangyuanHomeBlueprintRenderAssets>()
            .material_count();
        assert!(cached_materials >= unique_material_count(&compile_report.primitive_set));
        assert!(
            cached_materials < PRESSURE_PRIMITIVES,
            "pressure path should reuse color materials instead of creating one per primitive"
        );
        clear_blueprint_content_once(&mut app, &session_id);
        assert_eq!(fangyuan_content_count(&mut app, &session_id), 1);
        assert_eq!(fangyuan_home_object_count(&mut app, &session_id), 0);
        assert_eq!(fangyuan_blueprint_primitive_count(&mut app, &session_id), 0);

        app.world_mut()
            .write_message(SceneCommand::Exit(SceneExitRequest::default()));
        app.update();
        app.update();

        let counts = scene_entity_counts_for_session_from_world(&mut app, &session_id);
        assert!(counts.is_empty());
        assert_eq!(fangyuan_content_count(&mut app, &session_id), 0);
        assert_eq!(fangyuan_blueprint_content_count(&mut app, &session_id), 0);
        assert_eq!(fangyuan_home_object_count(&mut app, &session_id), 0);
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
        assert_eq!(fangyuan_blueprint_content_count(&mut app, &session_id), 1);
        assert_eq!(fangyuan_home_object_count(&mut app, &session_id), 1);
        assert_eq!(
            fangyuan_blueprint_primitive_count(&mut app, &session_id),
            EXPECTED_DEFAULT_BLUEPRINT_GENERATED
        );

        app.world_mut()
            .write_message(SceneCommand::Exit(SceneExitRequest::default()));
        app.update();
        app.update();

        let counts = scene_entity_counts_for_session_from_world(&mut app, &session_id);
        assert!(counts.is_empty());
        assert_eq!(fangyuan_content_count(&mut app, &session_id), 0);
        assert_eq!(fangyuan_visual_count(&mut app, &session_id), 0);
        assert_eq!(fangyuan_blueprint_content_count(&mut app, &session_id), 0);
        assert_eq!(fangyuan_home_object_count(&mut app, &session_id), 0);
        assert_eq!(fangyuan_blueprint_primitive_count(&mut app, &session_id), 0);
        assert_eq!(
            app.world().resource::<FangyuanHomeBlueprintStats>(),
            &FangyuanHomeBlueprintStats::default()
        );
        assert_eq!(
            app.world().resource::<SceneRuntime>().active_session_id(),
            None
        );
    }

    #[test]
    fn clearing_blueprint_content_does_not_remove_base_space() {
        let mut app = app_with_fangyuan_home_system();

        let session_id = SceneSessionId::from("fangyuan-clear-session");
        let scene_root = spawn_scene_root(
            &mut app.world_mut().commands(),
            &FANGYUAN_HOME_SCENE_ID.into(),
            &session_id,
        );
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

        assert_eq!(fangyuan_content_count(&mut app, &session_id), 1);
        assert_eq!(
            fangyuan_visual_count(&mut app, &session_id),
            EXPECTED_TOTAL_VISUALS
        );
        assert_eq!(fangyuan_blueprint_content_count(&mut app, &session_id), 1);
        assert_eq!(fangyuan_home_object_count(&mut app, &session_id), 1);
        assert_eq!(
            fangyuan_blueprint_primitive_count(&mut app, &session_id),
            EXPECTED_DEFAULT_BLUEPRINT_GENERATED
        );
        let stats_before_clear = app.world().resource::<FangyuanHomeBlueprintStats>().clone();

        let clear_session_id = session_id.clone();
        app.add_systems(
            Update,
            move |mut commands: Commands,
                  blueprint_content: Query<(Entity, &FangyuanHomeBlueprintContent)>| {
                clear_fangyuan_home_blueprint_content(
                    &mut commands,
                    &clear_session_id,
                    blueprint_content.iter(),
                );
            },
        );
        app.update();
        app.update();

        assert_eq!(fangyuan_content_count(&mut app, &session_id), 1);
        assert_eq!(
            fangyuan_visual_count(&mut app, &session_id),
            EXPECTED_TOTAL_VISUALS
        );
        assert_eq!(fangyuan_blueprint_content_count(&mut app, &session_id), 0);
        assert_eq!(fangyuan_home_object_count(&mut app, &session_id), 0);
        assert_eq!(fangyuan_blueprint_primitive_count(&mut app, &session_id), 0);
        assert_eq!(
            app.world().resource::<FangyuanHomeBlueprintStats>(),
            &stats_before_clear
        );

        let mut visual_counts = app
            .world_mut()
            .query::<(&FangyuanHomeVisual, &FangyuanHomeContent)>();
        let mut plane_count = 0;
        let mut grid_count = 0;
        let mut boundary_count = 0;
        let mut light_count = 0;
        for (visual, content) in visual_counts.iter(app.world()) {
            if content.session_id != session_id {
                continue;
            }
            match visual {
                FangyuanHomeVisual::Plane => plane_count += 1,
                FangyuanHomeVisual::Grid => grid_count += 1,
                FangyuanHomeVisual::Boundary => boundary_count += 1,
                FangyuanHomeVisual::DirectionalLight | FangyuanHomeVisual::PointLight => {
                    light_count += 1
                }
            }
        }
        assert_eq!(plane_count, 1);
        assert_eq!(grid_count, EXPECTED_GRID_VISUALS);
        assert_eq!(boundary_count, EXPECTED_BOUNDARY_VISUALS);
        assert_eq!(light_count, EXPECTED_LIGHT_VISUALS);
    }

    #[test]
    fn reload_blueprint_command_replaces_content_without_duplicate_primitives() {
        let mut app = app_with_fangyuan_home_system();
        let session_id = spawn_and_enter_fangyuan_home(&mut app, "fangyuan-reload-session");
        let compile_report = default_blueprint_compile_report();

        assert_eq!(fangyuan_content_count(&mut app, &session_id), 1);
        assert_eq!(
            fangyuan_visual_count(&mut app, &session_id),
            EXPECTED_TOTAL_VISUALS
        );
        assert_eq!(fangyuan_blueprint_content_count(&mut app, &session_id), 1);
        assert_eq!(fangyuan_home_object_count(&mut app, &session_id), 1);
        assert_eq!(
            fangyuan_blueprint_primitive_count(&mut app, &session_id),
            EXPECTED_DEFAULT_BLUEPRINT_GENERATED
        );
        let previous_home_object = fangyuan_home_object_entity(&mut app, &session_id)
            .expect("initial home object should exist before reload");

        app.world_mut()
            .write_message(FangyuanHomeBlueprintCommand::Reload);
        app.update();

        assert_eq!(fangyuan_content_count(&mut app, &session_id), 1);
        assert_eq!(
            fangyuan_visual_count(&mut app, &session_id),
            EXPECTED_TOTAL_VISUALS
        );
        assert_eq!(fangyuan_blueprint_content_count(&mut app, &session_id), 1);
        assert_eq!(fangyuan_home_object_count(&mut app, &session_id), 1);
        assert_eq!(
            fangyuan_blueprint_primitive_count(&mut app, &session_id),
            EXPECTED_DEFAULT_BLUEPRINT_GENERATED
        );
        let reloaded_home_object = fangyuan_home_object_entity(&mut app, &session_id)
            .expect("reloaded home object should exist after reload");
        assert_ne!(reloaded_home_object, previous_home_object);
        let reloaded_home_object_ref = app.world().entity(reloaded_home_object);
        assert_eq!(
            reloaded_home_object_ref
                .get::<FangyuanPrimitiveSet>()
                .unwrap()
                .len(),
            EXPECTED_DEFAULT_BLUEPRINT_GENERATED
        );
        assert_eq!(
            reloaded_home_object_ref
                .get::<FangyuanObjectState>()
                .unwrap(),
            &FangyuanObjectState::default()
        );
        assert_eq!(
            app.world().resource::<FangyuanHomeBlueprintStats>(),
            &expected_loaded_blueprint_stats(&session_id, &compile_report)
        );
    }

    #[test]
    fn clear_blueprint_command_removes_only_blueprint_content() {
        let mut app = app_with_fangyuan_home_system();
        let session_id = spawn_and_enter_fangyuan_home(&mut app, "fangyuan-command-clear-session");
        let compile_report = default_blueprint_compile_report();

        app.world_mut()
            .write_message(FangyuanHomeBlueprintCommand::Clear);
        app.update();

        assert_eq!(fangyuan_content_count(&mut app, &session_id), 1);
        assert_eq!(
            fangyuan_visual_count(&mut app, &session_id),
            EXPECTED_TOTAL_VISUALS
        );
        assert_eq!(fangyuan_blueprint_content_count(&mut app, &session_id), 0);
        assert_eq!(fangyuan_home_object_count(&mut app, &session_id), 0);
        assert_eq!(fangyuan_home_object_entity(&mut app, &session_id), None);
        assert_eq!(fangyuan_blueprint_primitive_count(&mut app, &session_id), 0);
        assert_eq!(
            app.world().resource::<FangyuanHomeBlueprintStats>(),
            &expected_cleared_blueprint_stats(&session_id, &compile_report)
        );
    }

    #[test]
    fn reload_blueprint_command_regenerates_preview_after_clear() {
        let mut app = app_with_fangyuan_home_system();
        let session_id = spawn_and_enter_fangyuan_home(&mut app, "fangyuan-clear-reload-session");
        let compile_report = default_blueprint_compile_report();

        assert_eq!(fangyuan_content_count(&mut app, &session_id), 1);
        assert_eq!(fangyuan_blueprint_content_count(&mut app, &session_id), 1);
        assert_eq!(fangyuan_home_object_count(&mut app, &session_id), 1);
        assert_eq!(
            fangyuan_blueprint_primitive_count(&mut app, &session_id),
            EXPECTED_DEFAULT_BLUEPRINT_GENERATED
        );

        app.world_mut()
            .write_message(FangyuanHomeBlueprintCommand::Clear);
        app.update();

        assert_eq!(fangyuan_content_count(&mut app, &session_id), 1);
        assert_eq!(
            fangyuan_visual_count(&mut app, &session_id),
            EXPECTED_TOTAL_VISUALS
        );
        assert_eq!(fangyuan_blueprint_content_count(&mut app, &session_id), 0);
        assert_eq!(fangyuan_home_object_count(&mut app, &session_id), 0);
        assert_eq!(fangyuan_home_object_entity(&mut app, &session_id), None);
        assert_eq!(fangyuan_blueprint_primitive_count(&mut app, &session_id), 0);
        assert_eq!(
            app.world().resource::<FangyuanHomeBlueprintStats>(),
            &expected_cleared_blueprint_stats(&session_id, &compile_report)
        );

        app.world_mut()
            .write_message(FangyuanHomeBlueprintCommand::Reload);
        app.update();

        assert_eq!(fangyuan_content_count(&mut app, &session_id), 1);
        assert_eq!(
            fangyuan_visual_count(&mut app, &session_id),
            EXPECTED_TOTAL_VISUALS
        );
        assert_eq!(fangyuan_blueprint_content_count(&mut app, &session_id), 1);
        assert_eq!(fangyuan_home_object_count(&mut app, &session_id), 1);
        assert_eq!(
            fangyuan_blueprint_primitive_count(&mut app, &session_id),
            EXPECTED_DEFAULT_BLUEPRINT_GENERATED
        );
        assert_eq!(
            app.world().resource::<FangyuanHomeBlueprintStats>(),
            &expected_loaded_blueprint_stats(&session_id, &compile_report)
        );
    }

    #[test]
    fn reload_failure_clears_old_blueprint_but_keeps_base_space() {
        let mut app = app_with_fangyuan_home_system();
        let session_id = spawn_and_enter_fangyuan_home(&mut app, "fangyuan-reload-fails-session");
        let content_root = fangyuan_content_entity(&mut app, &session_id)
            .expect("content root should exist before failed reload");
        let layout = FangyuanHomeLayout::load_first_package_ron(FANGYUAN_HOME_LAYOUT_PATH).unwrap();

        clear_blueprint_content_once(&mut app, &session_id);
        spawn_blueprint_from_layout_for_test(&mut app, content_root, &session_id, &layout, |_| {
            Err("test injected blueprint load failure".to_string())
        });

        assert_eq!(fangyuan_content_count(&mut app, &session_id), 1);
        assert_eq!(
            fangyuan_visual_count(&mut app, &session_id),
            EXPECTED_TOTAL_VISUALS
        );
        assert_eq!(fangyuan_blueprint_content_count(&mut app, &session_id), 0);
        assert_eq!(fangyuan_home_object_count(&mut app, &session_id), 0);
        assert_eq!(fangyuan_blueprint_primitive_count(&mut app, &session_id), 0);
        assert_eq!(
            app.world().resource::<FangyuanHomeBlueprintStats>(),
            &expected_failed_blueprint_stats(
                &session_id,
                FANGYUAN_HOME_DEFAULT_BLUEPRINT_PATH,
                0,
                unique_material_count(&default_blueprint_compile_report().primitive_set),
            )
        );
    }

    #[test]
    fn invalid_or_malformed_blueprint_sources_do_not_spawn_preview_content() {
        let mut app = app_with_fangyuan_home_system();
        let session_id = SceneSessionId::from("fangyuan-invalid-blueprint-session");
        let parent = app.world_mut().spawn_empty().id();
        let layout = FangyuanHomeLayout {
            default_blueprint_path: "fangyuan/test_invalid.ron".to_string(),
            ..FangyuanHomeLayout::default()
        };

        spawn_blueprint_from_layout_for_test(&mut app, parent, &session_id, &layout, |_| {
            FangyuanBlueprint::from_ron_str("this is not valid RON")
                .map_err(|error| error.to_string())
        });

        assert_eq!(fangyuan_blueprint_content_count(&mut app, &session_id), 0);
        assert_eq!(fangyuan_home_object_count(&mut app, &session_id), 0);
        assert_eq!(fangyuan_blueprint_primitive_count(&mut app, &session_id), 0);
        assert_eq!(
            app.world().resource::<FangyuanHomeBlueprintStats>(),
            &expected_failed_blueprint_stats(&session_id, "fangyuan/test_invalid.ron", 0, 0)
        );

        let invalid_top_level = blueprint_with_primitives(vec![valid_cube_primitive()]);
        let invalid_top_level = FangyuanBlueprint {
            version: "2".to_string(),
            ..invalid_top_level
        };
        spawn_blueprint_from_layout_for_test(&mut app, parent, &session_id, &layout, |_| {
            Ok(invalid_top_level)
        });

        assert_eq!(fangyuan_blueprint_content_count(&mut app, &session_id), 0);
        assert_eq!(fangyuan_home_object_count(&mut app, &session_id), 0);
        assert_eq!(fangyuan_blueprint_primitive_count(&mut app, &session_id), 0);
        assert_eq!(
            app.world().resource::<FangyuanHomeBlueprintStats>(),
            &expected_failed_blueprint_stats(&session_id, "fangyuan/test_invalid.ron", 1, 0)
        );
    }

    fn fangyuan_content_count(app: &mut App, session_id: &SceneSessionId) -> usize {
        let mut content = app.world_mut().query_filtered::<&FangyuanHomeContent, (
            Without<FangyuanHomeVisual>,
            Without<FangyuanHomeBlueprintContent>,
        )>();
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

    fn fangyuan_blueprint_content_count(app: &mut App, session_id: &SceneSessionId) -> usize {
        let mut blueprint_content = app.world_mut().query::<&FangyuanHomeBlueprintContent>();
        blueprint_content
            .iter(app.world())
            .filter(|content| content.session_id == *session_id)
            .count()
    }

    fn fangyuan_home_object_count(app: &mut App, session_id: &SceneSessionId) -> usize {
        let mut objects = app.world_mut().query::<&FangyuanHomeObject>();
        objects
            .iter(app.world())
            .filter(|object| object.session_id == *session_id)
            .count()
    }

    fn fangyuan_home_object_entity(app: &mut App, session_id: &SceneSessionId) -> Option<Entity> {
        let mut objects = app.world_mut().query::<(Entity, &FangyuanHomeObject)>();
        objects
            .iter(app.world())
            .find(|(_, object)| object.session_id == *session_id)
            .map(|(entity, _)| entity)
    }

    fn fangyuan_blueprint_primitive_count(app: &mut App, session_id: &SceneSessionId) -> usize {
        let mut primitives = app
            .world_mut()
            .query::<&FangyuanHomeBlueprintPrimitiveVisual>();
        primitives
            .iter(app.world())
            .filter(|primitive| primitive.session_id == *session_id)
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

    fn fangyuan_content_entity(app: &mut App, session_id: &SceneSessionId) -> Option<Entity> {
        let mut content = app
            .world_mut()
            .query_filtered::<(Entity, &FangyuanHomeContent), (
                Without<FangyuanHomeVisual>,
                Without<FangyuanHomeBlueprintContent>,
            )>();
        content
            .iter(app.world())
            .find(|(_, content)| content.session_id == *session_id)
            .map(|(entity, _)| entity)
    }

    fn spawn_and_enter_fangyuan_home(app: &mut App, session_name: &str) -> SceneSessionId {
        let session_id = SceneSessionId::from(session_name);
        let scene_root = spawn_scene_root(
            &mut app.world_mut().commands(),
            &FANGYUAN_HOME_SCENE_ID.into(),
            &session_id,
        );
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
        session_id
    }

    fn clear_blueprint_content_once(app: &mut App, session_id: &SceneSessionId) -> usize {
        let mut state: SystemState<(Commands, Query<(Entity, &FangyuanHomeBlueprintContent)>)> =
            SystemState::new(app.world_mut());
        let cleared = {
            let (mut commands, blueprint_content) = state.get_mut(app.world_mut());
            clear_fangyuan_home_blueprint_content(
                &mut commands,
                session_id,
                blueprint_content.iter(),
            )
        };
        state.apply(app.world_mut());
        app.update();
        cleared
    }

    fn spawn_blueprint_content_for_test(
        app: &mut App,
        parent: Entity,
        session_id: &SceneSessionId,
        primitive_set: &FangyuanPrimitiveSet,
    ) -> Entity {
        let mut state: SystemState<(
            Commands,
            ResMut<Assets<Mesh>>,
            ResMut<Assets<StandardMaterial>>,
            ResMut<FangyuanHomeBlueprintRenderAssets>,
        )> = SystemState::new(app.world_mut());
        let content = {
            let (mut commands, mut meshes, mut materials, mut blueprint_assets) =
                state.get_mut(app.world_mut());
            spawn_fangyuan_home_blueprint_content(
                &mut commands,
                parent,
                session_id,
                primitive_set,
                &mut meshes,
                &mut materials,
                &mut blueprint_assets,
            )
        };
        state.apply(app.world_mut());
        app.update();
        content
    }

    fn spawn_blueprint_from_layout_for_test(
        app: &mut App,
        parent: Entity,
        session_id: &SceneSessionId,
        layout: &FangyuanHomeLayout,
        load_blueprint: impl FnOnce(&str) -> Result<FangyuanBlueprint, String>,
    ) -> Option<Entity> {
        let mut state: SystemState<(
            Commands,
            ResMut<Assets<Mesh>>,
            ResMut<Assets<StandardMaterial>>,
            ResMut<FangyuanHomeBlueprintRenderAssets>,
            ResMut<FangyuanHomeBlueprintStats>,
        )> = SystemState::new(app.world_mut());
        let content = {
            let (
                mut commands,
                mut meshes,
                mut materials,
                mut blueprint_assets,
                mut blueprint_stats,
            ) = state.get_mut(app.world_mut());
            spawn_fangyuan_home_blueprint_from_layout_with_loader(
                &mut commands,
                parent,
                session_id,
                layout,
                &mut meshes,
                &mut materials,
                &mut blueprint_assets,
                &mut blueprint_stats,
                load_blueprint,
            )
        };
        state.apply(app.world_mut());
        app.update();
        content
    }

    #[derive(Clone, Debug)]
    struct BlueprintPrimitiveRecord {
        kind: FangyuanPrimitiveKind,
        mesh: Handle<Mesh>,
        material: Handle<StandardMaterial>,
    }

    fn blueprint_primitive_records(
        app: &mut App,
        session_id: &SceneSessionId,
    ) -> Vec<BlueprintPrimitiveRecord> {
        let mut primitives = app.world_mut().query::<(
            &FangyuanHomeBlueprintPrimitiveVisual,
            &Mesh3d,
            &MeshMaterial3d<StandardMaterial>,
        )>();
        primitives
            .iter(app.world())
            .filter(|(primitive, _, _)| primitive.session_id == *session_id)
            .map(|(primitive, mesh, material)| BlueprintPrimitiveRecord {
                kind: primitive.kind,
                mesh: mesh.0.clone(),
                material: material.0.clone(),
            })
            .collect()
    }

    fn default_blueprint_compile_report() -> FangyuanBlueprintCompileReport {
        let layout = FangyuanHomeLayout::load_first_package_ron(FANGYUAN_HOME_LAYOUT_PATH).unwrap();
        FangyuanBlueprint::load_first_package_ron(&layout.default_blueprint_path)
            .unwrap()
            .compile_skipping_invalid_primitives()
            .unwrap()
    }

    fn expected_loaded_blueprint_stats(
        session_id: &SceneSessionId,
        compile_report: &FangyuanBlueprintCompileReport,
    ) -> FangyuanHomeBlueprintStats {
        let mut stats = FangyuanHomeBlueprintStats::default();
        stats.record_loaded(
            session_id,
            FANGYUAN_HOME_DEFAULT_BLUEPRINT_PATH,
            &compile_report.primitive_set,
            compile_report.skipped_primitives,
        );
        stats
    }

    fn expected_cleared_blueprint_stats(
        session_id: &SceneSessionId,
        compile_report: &FangyuanBlueprintCompileReport,
    ) -> FangyuanHomeBlueprintStats {
        let mut stats = expected_loaded_blueprint_stats(session_id, compile_report);
        stats.record_cleared(session_id);
        stats
    }

    fn expected_failed_blueprint_stats(
        session_id: &SceneSessionId,
        blueprint_path: &str,
        skipped: usize,
        materials: usize,
    ) -> FangyuanHomeBlueprintStats {
        let mut stats = FangyuanHomeBlueprintStats::default();
        stats.record_failed(session_id, blueprint_path, skipped, materials);
        stats
    }

    fn unique_material_count(primitive_set: &FangyuanPrimitiveSet) -> usize {
        primitive_set
            .primitives()
            .iter()
            .map(|primitive| FangyuanRenderColorKey::from_color(primitive.color))
            .collect::<std::collections::HashSet<_>>()
            .len()
    }

    fn blueprint_with_primitives(primitives: Vec<FangyuanPrimitiveBlueprint>) -> FangyuanBlueprint {
        FangyuanBlueprint {
            version: FANGYUAN_BLUEPRINT_VERSION.to_string(),
            name: "test_blueprint".to_string(),
            description: String::new(),
            max_primitives: FANGYUAN_BLUEPRINT_HARD_PRIMITIVE_LIMIT,
            bounds: crate::framework::fangyuan::FangyuanBlueprintBounds::new(40.0, 40.0, 20.0),
            primitives,
        }
    }

    fn valid_cube_primitive() -> FangyuanPrimitiveBlueprint {
        FangyuanPrimitiveBlueprint::new(
            FangyuanPrimitiveKind::Cube,
            [0.0, 0.5, 0.0],
            [1.0, 1.0, 1.0],
            [0.25, 0.35, 0.45, 1.0],
        )
    }

    fn valid_sphere_primitive() -> FangyuanPrimitiveBlueprint {
        FangyuanPrimitiveBlueprint::new(
            FangyuanPrimitiveKind::Sphere,
            [1.0, 1.0, -1.0],
            [1.2, 1.4, 1.6],
            [0.85, 0.55, 0.25, 1.0],
        )
    }

    fn cube_primitive_at(x: f32, size: [f32; 3], color: [f32; 4]) -> FangyuanPrimitiveBlueprint {
        blueprint_primitive_at(FangyuanPrimitiveKind::Cube, x, size, color)
    }

    fn sphere_primitive_at(x: f32, size: [f32; 3], color: [f32; 4]) -> FangyuanPrimitiveBlueprint {
        blueprint_primitive_at(FangyuanPrimitiveKind::Sphere, x, size, color)
    }

    fn blueprint_primitive_at(
        kind: FangyuanPrimitiveKind,
        x: f32,
        size: [f32; 3],
        color: [f32; 4],
    ) -> FangyuanPrimitiveBlueprint {
        FangyuanPrimitiveBlueprint::new(kind, [x, 1.0, 0.0], size, color)
    }

    fn pressure_blueprint(count: usize) -> FangyuanBlueprint {
        let mut primitives = Vec::with_capacity(count);
        for index in 0..count {
            let column = index % 45;
            let row = index / 45;
            let x = column as f32 * 0.8 - 17.6;
            let z = row as f32 * 0.8 - 8.8;
            let size = [
                0.25 + (index % 3) as f32 * 0.05,
                0.25 + (index % 5) as f32 * 0.04,
                0.25 + (index % 7) as f32 * 0.03,
            ];
            let color = match index % 4 {
                0 => [0.25, 0.35, 0.45, 1.0],
                1 => [0.85, 0.55, 0.25, 1.0],
                2 => [0.35, 0.65, 0.40, 1.0],
                _ => [0.65, 0.35, 0.70, 1.0],
            };
            let kind = if index % 2 == 0 {
                FangyuanPrimitiveKind::Cube
            } else {
                FangyuanPrimitiveKind::Sphere
            };
            primitives.push(FangyuanPrimitiveBlueprint::new(
                kind,
                [x, 1.0, z],
                size,
                color,
            ));
        }
        blueprint_with_primitives(primitives)
    }

    fn below_ground_primitive() -> FangyuanPrimitiveBlueprint {
        let mut primitive = valid_cube_primitive();
        primitive.position = [0.0, 0.2, 0.0];
        primitive
    }

    fn invalid_position_primitive() -> FangyuanPrimitiveBlueprint {
        let mut primitive = valid_cube_primitive();
        primitive.position = [21.0, 0.5, 0.0];
        primitive
    }

    fn invalid_size_primitive() -> FangyuanPrimitiveBlueprint {
        let mut primitive = valid_cube_primitive();
        primitive.size = [1.0, 0.05, 1.0];
        primitive
    }

    fn invalid_color_primitive() -> FangyuanPrimitiveBlueprint {
        let mut primitive = valid_cube_primitive();
        primitive.color = [0.4, 0.4, 1.2, 1.0];
        primitive
    }

    fn invalid_alpha_primitive() -> FangyuanPrimitiveBlueprint {
        let mut primitive = valid_cube_primitive();
        primitive.alpha = Some(1.2);
        primitive
    }

    fn invalid_emissive_primitive() -> FangyuanPrimitiveBlueprint {
        let mut primitive = valid_cube_primitive();
        primitive.emissive = Some(-0.1);
        primitive
    }

    fn invalid_material_profile_primitive() -> FangyuanPrimitiveBlueprint {
        let mut primitive = valid_cube_primitive();
        primitive.material_profile_id = Some("invalid profile".to_string());
        primitive
    }

    fn invalid_lifecycle_primitive() -> FangyuanPrimitiveBlueprint {
        let mut primitive = valid_cube_primitive();
        primitive.lifecycle = Some(crate::framework::fangyuan::FangyuanPrimitiveLifecycle::new(
            Some(0),
            Some(1),
            Some(2),
        ));
        primitive
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
