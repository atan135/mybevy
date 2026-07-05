use bevy::{prelude::*, render::batching::NoAutomaticBatching};
use serde::{Deserialize, Deserializer, de};
use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use crate::framework::{
    fangyuan::{
        FANGYUAN_HOME_PREFAB_PALETTE_PATH, FANGYUAN_HOME_SCENE_LAYOUT_PATH, FangyuanAoiConfig,
        FangyuanAuditFinding, FangyuanAuditReport, FangyuanAuditSeverity, FangyuanAuditStatus,
        FangyuanChunkAvailablePrefabs, FangyuanChunkClearReason, FangyuanChunkCommand,
        FangyuanChunkDebugSummary, FangyuanChunkEvent, FangyuanChunkManifestRuntime,
        FangyuanChunkRuntime, FangyuanChunkSourceLibrary, FangyuanDebugPanelState,
        FangyuanHotspotState, FangyuanHotspotThresholds, FangyuanLodIntegrationSummary,
        FangyuanLodObjectKind, FangyuanLodRenderDescriptor, FangyuanLodRenderPath,
        FangyuanMaterialInstanceParams, FangyuanMaterialProfileRegistry, FangyuanObjectClass,
        FangyuanObjectState, FangyuanObjectTrialRuntime, FangyuanObjectTrialSummary,
        FangyuanObjectTrialVisualPrimitive, FangyuanPrimitive, FangyuanPrimitiveKind,
        FangyuanPrimitiveSet, FangyuanPrimitiveSetStats, FangyuanRenderAssetCache,
        FangyuanSceneLayoutCompileReport, FangyuanStaticInstanceBufferSource,
        FangyuanStaticInstanceRenderBatch, FangyuanStaticInstanceRenderError,
        FangyuanStaticInstanceRenderOptions, FangyuanStaticInstanceRenderReport,
        FangyuanStaticInstanceRenderStats, FangyuanStaticMergeSourceRef, FangyuanStaticMeshBounds,
        FangyuanStaticMeshBuildError, FangyuanStaticMeshBuildOptions,
        FangyuanStaticMeshBuildReport, FangyuanStaticMeshBuildStats, FangyuanStaticMeshMaterial,
        FangyuanStaticMeshMetadata, FangyuanTrialBudgetProfileKind, evaluate_fangyuan_hotspot,
        fangyuan_lod_descriptor_from_trial_visual, fangyuan_lod_descriptors_from_primitive_set,
        fangyuan_render_transform_from_primitive, fangyuan_standard_material_from_color,
        fangyuan_standard_material_from_params,
        fangyuan_static_instance_render_report_from_primitive_set_with_source,
        fangyuan_static_meshes_from_primitive_set_with_source, format_fangyuan_audit_debug_lines,
        hotspot_metrics_from_descriptors, load_fangyuan_home_prefab_palette,
        load_fangyuan_home_scene_layout, process_fangyuan_chunk_commands,
        summarize_fangyuan_lod_integration_from_descriptors,
    },
    scene::prelude::{SceneEvent, SceneOwned, SceneRuntimeRoot, SceneSessionId},
};

#[cfg(test)]
use crate::framework::fangyuan::FangyuanBlueprint;

pub(in crate::game) const FANGYUAN_HOME_SCENE_ID: &str = "dev.fangyuan_home";
pub(in crate::game) const FANGYUAN_HOME_DEFAULT_BLUEPRINT_PATH: &str = "fangyuan/home_preview.ron";
const FANGYUAN_HOME_LAYOUT_PATH: &str = "scenes/fangyuan_home/layout.ron";
const FANGYUAN_HOME_RENDER_MODE_ENV: &str = "MYBEVY_FANGYUAN_HOME_RENDER_MODE";
#[cfg(test)]
const FANGYUAN_HOME_SCENE_MANIFEST_PATH: &str = "scenes/fangyuan_home/scene.ron";

pub(super) struct FangyuanHomePlugin;

impl Plugin for FangyuanHomePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<FangyuanHomeBlueprintRenderAssets>()
            .init_resource::<FangyuanHomeBlueprintRenderConfig>()
            .init_resource::<FangyuanHomeStaticMergeRuntime>()
            .init_resource::<FangyuanHomeStaticInstanceRuntime>()
            .init_resource::<FangyuanObjectTrialRuntime>()
            .init_resource::<FangyuanHomeObjectTrialRenderRuntime>()
            .init_resource::<FangyuanHomeLodIntegrationRuntime>()
            .init_resource::<FangyuanHomeBlueprintStats>()
            .init_resource::<FangyuanDebugPanelState>()
            .init_resource::<FangyuanChunkRuntime>()
            .init_resource::<FangyuanChunkSourceLibrary>()
            .init_resource::<FangyuanChunkManifestRuntime>()
            .init_resource::<FangyuanChunkAvailablePrefabs>()
            .add_message::<FangyuanHomeBlueprintCommand>()
            .add_message::<FangyuanChunkCommand>()
            .add_message::<FangyuanChunkEvent>()
            .add_systems(
                Update,
                (
                    reset_fangyuan_home_blueprint_stats_on_exit,
                    clear_fangyuan_home_chunks_on_exit,
                    clear_fangyuan_home_render_runtime_on_exit,
                    handle_fangyuan_home_blueprint_commands,
                    process_fangyuan_chunk_commands,
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
    RerunTrialAudit,
    SwitchTrialBudget,
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
    material_registry: FangyuanMaterialProfileRegistry,
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
        primitive: &FangyuanPrimitive,
        materials: &mut Assets<StandardMaterial>,
    ) -> Handle<StandardMaterial> {
        let params = self.material_registry.compose_primitive(primitive);
        self.material_from_params(&params, materials)
    }

    fn material_from_params(
        &mut self,
        params: &FangyuanMaterialInstanceParams,
        materials: &mut Assets<StandardMaterial>,
    ) -> Handle<StandardMaterial> {
        self.cache.material_from_params(params, materials)
    }

    fn material_for_runtime_fields(
        &mut self,
        color: Color,
        alpha: f32,
        emissive: f32,
        material_profile_id: Option<&str>,
        materials: &mut Assets<StandardMaterial>,
    ) -> Handle<StandardMaterial> {
        let params = self.material_registry.compose_runtime_fields(
            color,
            alpha,
            emissive,
            material_profile_id,
        );
        self.material_from_params(&params, materials)
    }

    fn material_for_static_merge_material(
        &mut self,
        material: &FangyuanStaticMeshMaterial,
        materials: &mut Assets<StandardMaterial>,
    ) -> Handle<StandardMaterial> {
        self.cache.material_from_color_and_emissive(
            Color::WHITE.with_alpha(material.alpha),
            material.emissive,
            materials,
        )
    }

    fn material_registry(&self) -> &FangyuanMaterialProfileRegistry {
        &self.material_registry
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

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::game) enum FangyuanHomeBlueprintRenderMode {
    #[default]
    Standard,
    CpuMerge,
    StaticInstance,
}

#[derive(Clone, Debug, Resource, PartialEq)]
pub(in crate::game) struct FangyuanHomeBlueprintRenderConfig {
    pub(in crate::game) mode: FangyuanHomeBlueprintRenderMode,
    pub(in crate::game) fallback_to_standard_on_merge_failure: bool,
    pub(in crate::game) fallback_to_standard_on_instance_failure: bool,
    pub(in crate::game) mesh_options: FangyuanStaticMeshBuildOptions,
    pub(in crate::game) instance_options: FangyuanStaticInstanceRenderOptions,
}

impl Default for FangyuanHomeBlueprintRenderConfig {
    fn default() -> Self {
        Self {
            mode: fangyuan_home_blueprint_render_mode_from_env(),
            fallback_to_standard_on_merge_failure: true,
            fallback_to_standard_on_instance_failure: true,
            mesh_options: FangyuanStaticMeshBuildOptions::default(),
            instance_options: FangyuanStaticInstanceRenderOptions::default(),
        }
    }
}

fn fangyuan_home_blueprint_render_mode_from_env() -> FangyuanHomeBlueprintRenderMode {
    let Ok(value) = env::var(FANGYUAN_HOME_RENDER_MODE_ENV) else {
        return FangyuanHomeBlueprintRenderMode::Standard;
    };
    parse_fangyuan_home_blueprint_render_mode(&value).unwrap_or_else(|| {
        warn!(
            "{}={} is not a supported Fangyuan home render mode; using standard",
            FANGYUAN_HOME_RENDER_MODE_ENV, value
        );
        FangyuanHomeBlueprintRenderMode::Standard
    })
}

fn parse_fangyuan_home_blueprint_render_mode(
    value: &str,
) -> Option<FangyuanHomeBlueprintRenderMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "standard" => Some(FangyuanHomeBlueprintRenderMode::Standard),
        "cpu_merge" | "cpu-merge" | "merge" => Some(FangyuanHomeBlueprintRenderMode::CpuMerge),
        "static_instance" | "static-instance" | "staticinstance" | "instancing" | "instance" => {
            Some(FangyuanHomeBlueprintRenderMode::StaticInstance)
        }
        _ => None,
    }
}

#[derive(Clone, Debug, Default, Resource)]
struct FangyuanHomeStaticMergeRuntime {
    mesh_handles: Vec<Handle<Mesh>>,
    material_handles: Vec<Handle<StandardMaterial>>,
    stats: FangyuanStaticMeshBuildStats,
    last_failure: Option<String>,
    fallback_count: usize,
}

impl FangyuanHomeStaticMergeRuntime {
    fn clear_assets(
        &mut self,
        meshes: &mut Assets<Mesh>,
        _materials: &mut Assets<StandardMaterial>,
    ) {
        for handle in self.mesh_handles.drain(..) {
            meshes.remove(&handle);
        }
        self.material_handles.clear();
        self.stats = FangyuanStaticMeshBuildStats::default();
        self.last_failure = None;
        self.fallback_count = 0;
    }

    fn record_success(
        &mut self,
        mesh_handles: Vec<Handle<Mesh>>,
        material_handles: Vec<Handle<StandardMaterial>>,
        stats: FangyuanStaticMeshBuildStats,
    ) {
        self.mesh_handles = mesh_handles;
        self.material_handles.clear();
        for handle in material_handles {
            if !self.material_handles.contains(&handle) {
                self.material_handles.push(handle);
            }
        }
        self.stats = stats;
        self.last_failure = None;
        self.fallback_count = 0;
    }

    fn record_failure(&mut self, error: &FangyuanStaticMeshBuildError, fallback_used: bool) {
        self.last_failure = Some(error.to_string());
        self.stats = FangyuanStaticMeshBuildStats {
            fallback_count: usize::from(fallback_used),
            ..Default::default()
        };
        self.fallback_count = usize::from(fallback_used);
    }
}

#[derive(Clone, Debug, Default, Resource)]
struct FangyuanHomeStaticInstanceRuntime {
    stats: FangyuanStaticInstanceRenderStats,
    last_failure: Option<String>,
    fallback_count: usize,
}

impl FangyuanHomeStaticInstanceRuntime {
    fn clear(&mut self) {
        self.stats = FangyuanStaticInstanceRenderStats::default();
        self.last_failure = None;
        self.fallback_count = 0;
    }

    fn record_success(&mut self, stats: FangyuanStaticInstanceRenderStats) {
        self.stats = stats;
        self.last_failure = None;
        self.fallback_count = 0;
    }

    fn record_failure(&mut self, error: &FangyuanStaticInstanceRenderError, fallback_used: bool) {
        self.last_failure = Some(error.to_string());
        self.stats = FangyuanStaticInstanceRenderStats::default();
        self.fallback_count = usize::from(fallback_used);
    }

    fn fallback_reason(&self) -> &str {
        self.last_failure.as_deref().unwrap_or("-")
    }
}

#[derive(Clone, Debug, Default, Resource)]
struct FangyuanHomeObjectTrialRenderRuntime {
    material_handles: Vec<Handle<StandardMaterial>>,
}

impl FangyuanHomeObjectTrialRenderRuntime {
    fn record_material(&mut self, material: Handle<StandardMaterial>) {
        self.material_handles.push(material);
    }

    fn clear_assets(&mut self, materials: &mut Assets<StandardMaterial>) {
        for handle in self.material_handles.drain(..) {
            materials.remove(&handle);
        }
    }

    #[cfg(test)]
    fn live_material_count(&self, materials: &Assets<StandardMaterial>) -> usize {
        self.material_handles
            .iter()
            .filter(|handle| materials.get(*handle).is_some())
            .count()
    }
}

#[derive(Clone, Debug, Default, Resource, PartialEq)]
pub(in crate::game) struct FangyuanHomeLodIntegrationRuntime {
    pub(in crate::game) summary: FangyuanLodIntegrationSummary,
}

impl FangyuanHomeLodIntegrationRuntime {
    fn record(&mut self, summary: FangyuanLodIntegrationSummary) {
        self.summary = summary;
    }

    fn clear(&mut self) {
        self.summary = FangyuanLodIntegrationSummary::default();
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(in crate::game) struct FangyuanHomeBlueprintRenderSummary {
    pub(in crate::game) mode: String,
    pub(in crate::game) material_profile_count: usize,
    pub(in crate::game) opaque_count: usize,
    pub(in crate::game) transparent_count: usize,
    pub(in crate::game) emissive_total: f32,
    pub(in crate::game) unique_material_resource_count: usize,
    pub(in crate::game) static_instance_batch_count: usize,
    pub(in crate::game) static_instance_count: usize,
    pub(in crate::game) static_instance_buffer_bytes: usize,
    pub(in crate::game) static_instance_fallback_reason: String,
}

impl Default for FangyuanHomeBlueprintRenderSummary {
    fn default() -> Self {
        Self::standard()
    }
}

impl FangyuanHomeBlueprintRenderSummary {
    fn standard() -> Self {
        Self {
            mode: "standard".to_string(),
            material_profile_count: 0,
            opaque_count: 0,
            transparent_count: 0,
            emissive_total: 0.0,
            unique_material_resource_count: 0,
            static_instance_batch_count: 0,
            static_instance_count: 0,
            static_instance_buffer_bytes: 0,
            static_instance_fallback_reason: "-".to_string(),
        }
    }

    fn cpu_merge() -> Self {
        Self {
            mode: "cpu_merge".to_string(),
            ..Self::standard()
        }
    }

    fn with_material_stats(mut self, primitive_stats: &FangyuanPrimitiveSetStats) -> Self {
        self.material_profile_count = primitive_stats.material_profile_count;
        self.opaque_count = primitive_stats.opaque_count;
        self.transparent_count = primitive_stats.transparent_count;
        self.emissive_total = primitive_stats.emissive_total;
        self.unique_material_resource_count = primitive_stats.unique_material_resource_count;
        self
    }

    fn static_instance(stats: &FangyuanStaticInstanceRenderStats) -> Self {
        Self {
            mode: "static_instance".to_string(),
            static_instance_batch_count: stats.batch_count,
            static_instance_count: stats.instance_count,
            static_instance_buffer_bytes: stats.buffer_bytes,
            static_instance_fallback_reason: "-".to_string(),
            ..Self::standard()
        }
    }

    fn static_instance_fallback(reason: &str) -> Self {
        Self {
            mode: "static_instance->standard".to_string(),
            static_instance_fallback_reason: reason.to_string(),
            ..Self::standard()
        }
    }

    fn static_instance_failed(reason: &str) -> Self {
        Self {
            mode: "static_instance_failed".to_string(),
            static_instance_fallback_reason: reason.to_string(),
            ..Self::standard()
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct FangyuanHomeBlueprintSpawnedContent {
    entity: Entity,
    render_summary: FangyuanHomeBlueprintRenderSummary,
    lod_descriptors: Vec<FangyuanLodRenderDescriptor>,
}

const FANGYUAN_HOME_BLUEPRINT_STATE_PENDING: &str = "pending";
const FANGYUAN_HOME_BLUEPRINT_STATE_LOADED: &str = "loaded";
const FANGYUAN_HOME_BLUEPRINT_STATE_CLEARED: &str = "cleared";
const FANGYUAN_HOME_BLUEPRINT_STATE_FAILED: &str = "failed";
const FANGYUAN_HOME_AUDIT_STATUS_PENDING: &str = "pending";
const FANGYUAN_HOME_AUDIT_STATUS_PASSED: &str = "passed";
const FANGYUAN_HOME_AUDIT_STATUS_WARNING: &str = "warning";
const FANGYUAN_HOME_AUDIT_STATUS_FAILED: &str = "failed";
const FANGYUAN_HOME_AUDIT_PRIMARY_CODE_NONE: &str = "-";
const FANGYUAN_HOME_AUDIT_DEBUG_MAX_FINDINGS: usize = 4;
const FANGYUAN_HOME_AUDIT_DEBUG_MAX_SUGGESTIONS: usize = 4;

#[derive(Clone, Debug, Resource, PartialEq)]
pub(in crate::game) struct FangyuanHomeBlueprintStats {
    pub(in crate::game) session_id: Option<SceneSessionId>,
    pub(in crate::game) primitive_stats: FangyuanPrimitiveSetStats,
    pub(in crate::game) skipped: usize,
    pub(in crate::game) materials: usize,
    pub(in crate::game) material_profile_count: usize,
    pub(in crate::game) opaque_count: usize,
    pub(in crate::game) transparent_count: usize,
    pub(in crate::game) emissive_total: f32,
    pub(in crate::game) unique_material_resource_count: usize,
    pub(in crate::game) blueprint_path: String,
    pub(in crate::game) layout_path: String,
    pub(in crate::game) palette_path: String,
    pub(in crate::game) palette_count: usize,
    pub(in crate::game) prefab_count: usize,
    pub(in crate::game) instance_count: usize,
    pub(in crate::game) generated_primitives: usize,
    pub(in crate::game) used_prefab_count: usize,
    pub(in crate::game) top_level_valid: bool,
    pub(in crate::game) layout_valid: bool,
    pub(in crate::game) palette_valid: bool,
    pub(in crate::game) audit_status: String,
    pub(in crate::game) audit_error_count: usize,
    pub(in crate::game) audit_warning_count: usize,
    pub(in crate::game) audit_primary_code: String,
    pub(in crate::game) audit_primary_field_path: String,
    pub(in crate::game) audit_primary_reason: String,
    pub(in crate::game) render_mode: String,
    pub(in crate::game) static_instance_batch_count: usize,
    pub(in crate::game) static_instance_count: usize,
    pub(in crate::game) static_instance_buffer_bytes: usize,
    pub(in crate::game) static_instance_fallback_reason: String,
    pub(in crate::game) lod_distribution: String,
    pub(in crate::game) lod_render_paths: String,
    pub(in crate::game) lod_aoi_radius: f32,
    pub(in crate::game) lod_pressure: String,
    pub(in crate::game) lod_degrade_reason: String,
    pub(in crate::game) trial_route_id: String,
    pub(in crate::game) trial_selection_label: String,
    pub(in crate::game) trial_budget_profile: String,
    pub(in crate::game) trial_audit_run: u64,
    pub(in crate::game) trial_audit_status: String,
    pub(in crate::game) trial_audit_error_count: usize,
    pub(in crate::game) trial_audit_warning_count: usize,
    pub(in crate::game) trial_audit_suggestion_count: usize,
    pub(in crate::game) active_vfx_count: usize,
    pub(in crate::game) trial_template_id: String,
    pub(in crate::game) trial_visual_id: String,
    pub(in crate::game) trial_equipment_count: usize,
    pub(in crate::game) trial_npc_count: usize,
    pub(in crate::game) trial_tiandao_count: usize,
    pub(in crate::game) trial_budget_cost: u32,
    pub(in crate::game) trial_budget_recommended: u32,
    pub(in crate::game) trial_budget_hard: u32,
    pub(in crate::game) trial_before_label: String,
    pub(in crate::game) trial_after_label: String,
    pub(in crate::game) trial_kept_count: usize,
    pub(in crate::game) trial_degraded_count: usize,
    pub(in crate::game) trial_rejected_count: usize,
    pub(in crate::game) trial_fallback_missing_count: usize,
    pub(in crate::game) trial_fallback_summary: String,
    pub(in crate::game) trial_plain_reason_summary: String,
    pub(in crate::game) trial_primary_suggestion: String,
    pub(in crate::game) trial_finding_summary: String,
    state: String,
}

impl Default for FangyuanHomeBlueprintStats {
    fn default() -> Self {
        Self {
            session_id: None,
            primitive_stats: FangyuanPrimitiveSetStats::default(),
            skipped: 0,
            materials: 0,
            material_profile_count: 0,
            opaque_count: 0,
            transparent_count: 0,
            emissive_total: 0.0,
            unique_material_resource_count: 0,
            blueprint_path: FANGYUAN_HOME_DEFAULT_BLUEPRINT_PATH.to_string(),
            layout_path: FANGYUAN_HOME_SCENE_LAYOUT_PATH.to_string(),
            palette_path: FANGYUAN_HOME_PREFAB_PALETTE_PATH.to_string(),
            palette_count: 0,
            prefab_count: 0,
            instance_count: 0,
            generated_primitives: 0,
            used_prefab_count: 0,
            top_level_valid: false,
            layout_valid: false,
            palette_valid: false,
            audit_status: FANGYUAN_HOME_AUDIT_STATUS_PENDING.to_string(),
            audit_error_count: 0,
            audit_warning_count: 0,
            audit_primary_code: FANGYUAN_HOME_AUDIT_PRIMARY_CODE_NONE.to_string(),
            audit_primary_field_path: String::new(),
            audit_primary_reason: String::new(),
            render_mode: "standard".to_string(),
            static_instance_batch_count: 0,
            static_instance_count: 0,
            static_instance_buffer_bytes: 0,
            static_instance_fallback_reason: "-".to_string(),
            lod_distribution: "f0 r0 s0 m0 h0".to_string(),
            lod_render_paths: "std0 mg0 inst0 mk0 hid0".to_string(),
            lod_aoi_radius: 0.0,
            lod_pressure: "normal".to_string(),
            lod_degrade_reason: "-".to_string(),
            trial_route_id: "none".to_string(),
            trial_selection_label: "-".to_string(),
            trial_budget_profile: FangyuanTrialBudgetProfileKind::Standard
                .as_str()
                .to_string(),
            trial_audit_run: 0,
            trial_audit_status: "pending".to_string(),
            trial_audit_error_count: 0,
            trial_audit_warning_count: 0,
            trial_audit_suggestion_count: 0,
            active_vfx_count: 0,
            trial_template_id: "-".to_string(),
            trial_visual_id: "-".to_string(),
            trial_equipment_count: 0,
            trial_npc_count: 0,
            trial_tiandao_count: 0,
            trial_budget_cost: 0,
            trial_budget_recommended: 96,
            trial_budget_hard: 128,
            trial_before_label: "0 objects cost 0".to_string(),
            trial_after_label: "keep 0 degrade 0 reject 0".to_string(),
            trial_kept_count: 0,
            trial_degraded_count: 0,
            trial_rejected_count: 0,
            trial_fallback_missing_count: 0,
            trial_fallback_summary: "ok".to_string(),
            trial_plain_reason_summary: "ok".to_string(),
            trial_primary_suggestion: "-".to_string(),
            trial_finding_summary: "ok".to_string(),
            state: FANGYUAN_HOME_BLUEPRINT_STATE_PENDING.to_string(),
        }
    }
}

impl FangyuanHomeBlueprintStats {
    pub(in crate::game) fn record_layout_loaded(
        &mut self,
        session_id: &SceneSessionId,
        layout_path: &str,
        palette_path: &str,
        audit_report: &FangyuanAuditReport,
        compile_report: &FangyuanSceneLayoutCompileReport,
        mut render_summary: FangyuanHomeBlueprintRenderSummary,
    ) {
        if compile_report.primitive_stats.total > 0
            && render_summary.unique_material_resource_count == 0
        {
            render_summary = render_summary.with_material_stats(&compile_report.primitive_stats);
        }
        self.session_id = Some(session_id.clone());
        self.primitive_stats = compile_report.primitive_stats.clone();
        self.skipped = compile_report.skipped_primitives;
        self.record_material_stats_from_summary(&render_summary);
        self.blueprint_path = FANGYUAN_HOME_DEFAULT_BLUEPRINT_PATH.to_string();
        self.layout_path = layout_path.to_string();
        self.palette_path = palette_path.to_string();
        self.palette_count = compile_report.palette_count;
        self.prefab_count = compile_report.prefab_count;
        self.instance_count = compile_report.instance_count;
        self.generated_primitives = compile_report.generated_primitives;
        self.used_prefab_count = compile_report.used_prefab_count;
        self.top_level_valid = compile_report.top_level_validated;
        self.layout_valid = compile_report.layout_validated;
        self.palette_valid = compile_report.palette_validated;
        self.record_audit_report(audit_report);
        self.record_render_summary(render_summary);
        self.state = FANGYUAN_HOME_BLUEPRINT_STATE_LOADED.to_string();
    }

    pub(in crate::game) fn record_layout_failed(
        &mut self,
        session_id: &SceneSessionId,
        layout_path: &str,
        palette_path: &str,
        materials: usize,
        audit_report: Option<&FangyuanAuditReport>,
    ) {
        self.session_id = Some(session_id.clone());
        self.primitive_stats = FangyuanPrimitiveSetStats::default();
        self.skipped = 0;
        self.materials = materials;
        self.material_profile_count = 0;
        self.opaque_count = 0;
        self.transparent_count = 0;
        self.emissive_total = 0.0;
        self.unique_material_resource_count = materials;
        self.blueprint_path = FANGYUAN_HOME_DEFAULT_BLUEPRINT_PATH.to_string();
        self.layout_path = layout_path.to_string();
        self.palette_path = palette_path.to_string();
        self.palette_count = 0;
        self.prefab_count = 0;
        self.instance_count = 0;
        self.generated_primitives = 0;
        self.used_prefab_count = 0;
        self.top_level_valid = false;
        self.layout_valid = false;
        self.palette_valid = false;
        if let Some(audit_report) = audit_report {
            self.record_audit_report(audit_report);
        } else {
            self.audit_status = FANGYUAN_HOME_AUDIT_STATUS_FAILED.to_string();
            self.audit_error_count = 1;
            self.audit_warning_count = 0;
            self.audit_primary_code = "load_or_compile_failed".to_string();
            self.audit_primary_field_path.clear();
            self.audit_primary_reason.clear();
        }
        self.record_render_summary(FangyuanHomeBlueprintRenderSummary::standard());
        self.record_lod_summary(&FangyuanLodIntegrationSummary::default());
        self.state = FANGYUAN_HOME_BLUEPRINT_STATE_FAILED.to_string();
    }

    #[cfg(test)]
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
        self.materials = primitive_stats.unique_material_resource_count;
        self.material_profile_count = primitive_stats.material_profile_count;
        self.opaque_count = primitive_stats.opaque_count;
        self.transparent_count = primitive_stats.transparent_count;
        self.emissive_total = primitive_stats.emissive_total;
        self.unique_material_resource_count = primitive_stats.unique_material_resource_count;
        self.blueprint_path = blueprint_path.to_string();
        self.layout_path = String::new();
        self.palette_path = String::new();
        self.palette_count = 0;
        self.prefab_count = 0;
        self.instance_count = 0;
        self.generated_primitives = primitive_set.len();
        self.used_prefab_count = 0;
        self.top_level_valid = true;
        self.layout_valid = false;
        self.palette_valid = false;
        self.audit_status = FANGYUAN_HOME_AUDIT_STATUS_PENDING.to_string();
        self.audit_error_count = 0;
        self.audit_warning_count = 0;
        self.audit_primary_code = FANGYUAN_HOME_AUDIT_PRIMARY_CODE_NONE.to_string();
        self.audit_primary_field_path.clear();
        self.audit_primary_reason.clear();
        self.record_render_summary(FangyuanHomeBlueprintRenderSummary::standard());
        self.state = FANGYUAN_HOME_BLUEPRINT_STATE_LOADED.to_string();
    }

    #[cfg(test)]
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
        self.material_profile_count = 0;
        self.opaque_count = 0;
        self.transparent_count = 0;
        self.emissive_total = 0.0;
        self.unique_material_resource_count = materials;
        self.blueprint_path = blueprint_path.to_string();
        self.layout_path = String::new();
        self.palette_path = String::new();
        self.palette_count = 0;
        self.prefab_count = 0;
        self.instance_count = 0;
        self.generated_primitives = 0;
        self.used_prefab_count = 0;
        self.top_level_valid = false;
        self.layout_valid = false;
        self.palette_valid = false;
        self.audit_status = FANGYUAN_HOME_AUDIT_STATUS_FAILED.to_string();
        self.audit_error_count = 1;
        self.audit_warning_count = 0;
        self.audit_primary_code = "load_or_compile_failed".to_string();
        self.audit_primary_field_path.clear();
        self.audit_primary_reason.clear();
        self.record_render_summary(FangyuanHomeBlueprintRenderSummary::standard());
        self.record_lod_summary(&FangyuanLodIntegrationSummary::default());
        self.state = FANGYUAN_HOME_BLUEPRINT_STATE_FAILED.to_string();
    }

    pub(in crate::game) fn record_cleared(&mut self, session_id: &SceneSessionId) {
        let skipped = self.skipped;
        let materials = self.materials;
        let material_profile_count = self.material_profile_count;
        let opaque_count = self.opaque_count;
        let transparent_count = self.transparent_count;
        let emissive_total = self.emissive_total;
        let unique_material_resource_count = self.unique_material_resource_count;
        let blueprint_path = self.blueprint_path().to_string();
        let layout_path = self.layout_path().to_string();
        let palette_path = self.palette_path().to_string();
        let palette_count = self.palette_count;
        let prefab_count = self.prefab_count;
        let instance_count = self.instance_count;
        let used_prefab_count = self.used_prefab_count;
        let top_level_valid = self.top_level_valid;
        let layout_valid = self.layout_valid;
        let palette_valid = self.palette_valid;
        let audit_status = self.audit_status_label().to_string();
        let audit_error_count = self.audit_error_count;
        let audit_warning_count = self.audit_warning_count;
        let audit_primary_code = self.audit_primary_code().to_string();
        let audit_primary_field_path = self.audit_primary_field_path.clone();
        let audit_primary_reason = self.audit_primary_reason.clone();
        let render_mode = self.render_mode.clone();
        let static_instance_batch_count = self.static_instance_batch_count;
        let static_instance_count = self.static_instance_count;
        let static_instance_buffer_bytes = self.static_instance_buffer_bytes;
        let static_instance_fallback_reason = self.static_instance_fallback_reason.clone();
        let lod_distribution = self.lod_distribution.clone();
        let lod_render_paths = self.lod_render_paths.clone();
        let lod_aoi_radius = self.lod_aoi_radius;
        let lod_pressure = self.lod_pressure.clone();
        let lod_degrade_reason = self.lod_degrade_reason.clone();
        let trial_route_id = self.trial_route_id.clone();
        let trial_selection_label = self.trial_selection_label.clone();
        let trial_budget_profile = self.trial_budget_profile.clone();
        let trial_audit_run = self.trial_audit_run;
        let trial_audit_status = self.trial_audit_status.clone();
        let trial_audit_error_count = self.trial_audit_error_count;
        let trial_audit_warning_count = self.trial_audit_warning_count;
        let trial_audit_suggestion_count = self.trial_audit_suggestion_count;
        let active_vfx_count = self.active_vfx_count;
        let trial_template_id = self.trial_template_id.clone();
        let trial_visual_id = self.trial_visual_id.clone();
        let trial_equipment_count = self.trial_equipment_count;
        let trial_npc_count = self.trial_npc_count;
        let trial_tiandao_count = self.trial_tiandao_count;
        let trial_budget_cost = self.trial_budget_cost;
        let trial_budget_recommended = self.trial_budget_recommended;
        let trial_budget_hard = self.trial_budget_hard;
        let trial_before_label = self.trial_before_label.clone();
        let trial_after_label = self.trial_after_label.clone();
        let trial_kept_count = self.trial_kept_count;
        let trial_degraded_count = self.trial_degraded_count;
        let trial_rejected_count = self.trial_rejected_count;
        let trial_fallback_missing_count = self.trial_fallback_missing_count;
        let trial_fallback_summary = self.trial_fallback_summary.clone();
        let trial_plain_reason_summary = self.trial_plain_reason_summary.clone();
        let trial_primary_suggestion = self.trial_primary_suggestion.clone();
        let trial_finding_summary = self.trial_finding_summary.clone();
        self.session_id = Some(session_id.clone());
        self.primitive_stats = FangyuanPrimitiveSetStats::default();
        self.skipped = skipped;
        self.materials = materials;
        self.material_profile_count = material_profile_count;
        self.opaque_count = opaque_count;
        self.transparent_count = transparent_count;
        self.emissive_total = emissive_total;
        self.unique_material_resource_count = unique_material_resource_count;
        self.blueprint_path = blueprint_path;
        self.layout_path = layout_path;
        self.palette_path = palette_path;
        self.palette_count = palette_count;
        self.prefab_count = prefab_count;
        self.instance_count = instance_count;
        self.generated_primitives = 0;
        self.used_prefab_count = used_prefab_count;
        self.top_level_valid = top_level_valid;
        self.layout_valid = layout_valid;
        self.palette_valid = palette_valid;
        self.audit_status = audit_status;
        self.audit_error_count = audit_error_count;
        self.audit_warning_count = audit_warning_count;
        self.audit_primary_code = audit_primary_code;
        self.audit_primary_field_path = audit_primary_field_path;
        self.audit_primary_reason = audit_primary_reason;
        self.render_mode = render_mode;
        self.static_instance_batch_count = static_instance_batch_count;
        self.static_instance_count = static_instance_count;
        self.static_instance_buffer_bytes = static_instance_buffer_bytes;
        self.static_instance_fallback_reason = static_instance_fallback_reason;
        self.lod_distribution = lod_distribution;
        self.lod_render_paths = lod_render_paths;
        self.lod_aoi_radius = lod_aoi_radius;
        self.lod_pressure = lod_pressure;
        self.lod_degrade_reason = lod_degrade_reason;
        self.trial_route_id = trial_route_id;
        self.trial_selection_label = trial_selection_label;
        self.trial_budget_profile = trial_budget_profile;
        self.trial_audit_run = trial_audit_run;
        self.trial_audit_status = trial_audit_status;
        self.trial_audit_error_count = trial_audit_error_count;
        self.trial_audit_warning_count = trial_audit_warning_count;
        self.trial_audit_suggestion_count = trial_audit_suggestion_count;
        self.active_vfx_count = active_vfx_count;
        self.trial_template_id = trial_template_id;
        self.trial_visual_id = trial_visual_id;
        self.trial_equipment_count = trial_equipment_count;
        self.trial_npc_count = trial_npc_count;
        self.trial_tiandao_count = trial_tiandao_count;
        self.trial_budget_cost = trial_budget_cost;
        self.trial_budget_recommended = trial_budget_recommended;
        self.trial_budget_hard = trial_budget_hard;
        self.trial_before_label = trial_before_label;
        self.trial_after_label = trial_after_label;
        self.trial_kept_count = trial_kept_count;
        self.trial_degraded_count = trial_degraded_count;
        self.trial_rejected_count = trial_rejected_count;
        self.trial_fallback_missing_count = trial_fallback_missing_count;
        self.trial_fallback_summary = trial_fallback_summary;
        self.trial_plain_reason_summary = trial_plain_reason_summary;
        self.trial_primary_suggestion = trial_primary_suggestion;
        self.trial_finding_summary = trial_finding_summary;
        self.state = FANGYUAN_HOME_BLUEPRINT_STATE_CLEARED.to_string();
    }

    pub(in crate::game) fn record_trial_summary(&mut self, summary: &FangyuanObjectTrialSummary) {
        self.trial_route_id = summary.route_id.clone();
        self.trial_selection_label = summary.selection_label.clone();
        self.trial_budget_profile = summary.budget_profile.clone();
        self.trial_audit_run = summary.audit_run;
        self.trial_audit_status = summary.audit_status.clone();
        self.trial_audit_error_count = summary.audit_error_count;
        self.trial_audit_warning_count = summary.audit_warning_count;
        self.trial_audit_suggestion_count = summary.audit_suggestion_count;
        self.active_vfx_count = summary.active_vfx_count;
        self.trial_template_id = summary.template_id.clone();
        self.trial_visual_id = summary.visual_id.clone();
        self.trial_equipment_count = summary.equipment_count;
        self.trial_npc_count = summary.npc_count;
        self.trial_tiandao_count = summary.tiandao_count;
        self.trial_budget_cost = summary.budget_cost;
        self.trial_budget_recommended = summary.budget_recommended;
        self.trial_budget_hard = summary.budget_hard;
        self.trial_before_label = summary.before_label.clone();
        self.trial_after_label = summary.after_label.clone();
        self.trial_kept_count = summary.kept_count;
        self.trial_degraded_count = summary.degraded_count;
        self.trial_rejected_count = summary.rejected_count;
        self.trial_fallback_missing_count = summary.fallback_missing_count;
        self.trial_fallback_summary = summary.fallback_summary.clone();
        self.trial_plain_reason_summary = summary.plain_reason_summary.clone();
        self.trial_primary_suggestion = summary.primary_suggestion.clone();
        self.trial_finding_summary = summary.finding_summary.clone();
    }

    pub(in crate::game) fn record_lod_summary(&mut self, summary: &FangyuanLodIntegrationSummary) {
        self.lod_distribution = summary.lod_distribution_label();
        self.lod_render_paths = summary.render_path_label();
        self.lod_aoi_radius = summary.aoi_radius;
        self.lod_pressure = summary.pressure_label().to_string();
        self.lod_degrade_reason = summary.degrade_reason_label().to_string();
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

    pub(in crate::game) fn audit_status_label(&self) -> &str {
        if self.audit_status.trim().is_empty() {
            FANGYUAN_HOME_AUDIT_STATUS_PENDING
        } else {
            self.audit_status.as_str()
        }
    }

    pub(in crate::game) fn audit_primary_code(&self) -> &str {
        if self.audit_primary_code.trim().is_empty() {
            FANGYUAN_HOME_AUDIT_PRIMARY_CODE_NONE
        } else {
            self.audit_primary_code.as_str()
        }
    }

    pub(in crate::game) fn blueprint_path(&self) -> &str {
        if self.blueprint_path.trim().is_empty() {
            FANGYUAN_HOME_DEFAULT_BLUEPRINT_PATH
        } else {
            self.blueprint_path.as_str()
        }
    }

    pub(in crate::game) fn layout_path(&self) -> &str {
        if self.layout_path.trim().is_empty() {
            FANGYUAN_HOME_SCENE_LAYOUT_PATH
        } else {
            self.layout_path.as_str()
        }
    }

    pub(in crate::game) fn palette_path(&self) -> &str {
        if self.palette_path.trim().is_empty() {
            FANGYUAN_HOME_PREFAB_PALETTE_PATH
        } else {
            self.palette_path.as_str()
        }
    }

    fn record_audit_report(&mut self, audit_report: &FangyuanAuditReport) {
        self.audit_status = audit_status_label(audit_report.status).to_string();
        self.audit_error_count = audit_report.summary.error_count;
        self.audit_warning_count = audit_report.summary.warning_count;
        if let Some(primary) = primary_audit_finding(audit_report) {
            self.audit_primary_code = primary.code.clone();
            self.audit_primary_field_path = primary.field_path.clone().unwrap_or_default();
            self.audit_primary_reason = primary.reason.clone();
        } else {
            self.audit_primary_code = FANGYUAN_HOME_AUDIT_PRIMARY_CODE_NONE.to_string();
            self.audit_primary_field_path.clear();
            self.audit_primary_reason.clear();
        }
    }

    fn record_render_summary(&mut self, render_summary: FangyuanHomeBlueprintRenderSummary) {
        self.render_mode = render_summary.mode;
        self.static_instance_batch_count = render_summary.static_instance_batch_count;
        self.static_instance_count = render_summary.static_instance_count;
        self.static_instance_buffer_bytes = render_summary.static_instance_buffer_bytes;
        self.static_instance_fallback_reason = render_summary.static_instance_fallback_reason;
    }

    fn record_material_stats_from_summary(
        &mut self,
        render_summary: &FangyuanHomeBlueprintRenderSummary,
    ) {
        self.material_profile_count = render_summary.material_profile_count;
        self.opaque_count = render_summary.opaque_count;
        self.transparent_count = render_summary.transparent_count;
        self.emissive_total = render_summary.emissive_total;
        self.unique_material_resource_count = render_summary.unique_material_resource_count;
        self.materials = render_summary.unique_material_resource_count;
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

#[derive(Clone, Debug, PartialEq)]
struct FangyuanHomeLayoutLoadResult {
    audit_report: FangyuanAuditReport,
    compile_report: Option<FangyuanSceneLayoutCompileReport>,
    failure: Option<String>,
}

impl FangyuanHomeLayoutLoadResult {
    fn loaded(
        audit_report: FangyuanAuditReport,
        compile_report: FangyuanSceneLayoutCompileReport,
    ) -> Self {
        Self {
            audit_report,
            compile_report: Some(compile_report),
            failure: None,
        }
    }

    fn audit_failed(audit_report: FangyuanAuditReport) -> Self {
        let (code, field_path, reason) = primary_audit_finding(&audit_report)
            .map(|finding| {
                (
                    finding.code.clone(),
                    finding.field_path.clone().unwrap_or_default(),
                    finding.reason.clone(),
                )
            })
            .unwrap_or_else(|| {
                (
                    FANGYUAN_HOME_AUDIT_PRIMARY_CODE_NONE.to_string(),
                    String::new(),
                    String::new(),
                )
            });
        Self {
            audit_report,
            compile_report: None,
            failure: Some(format!(
                "fangyuan home scene layout audit failed: code={code}, field_path={field_path}, reason={reason}"
            )),
        }
    }

    fn compile_failed(audit_report: FangyuanAuditReport, failure: String) -> Self {
        Self {
            audit_report,
            compile_report: None,
            failure: Some(failure),
        }
    }
}

fn audit_status_label(status: FangyuanAuditStatus) -> &'static str {
    match status {
        FangyuanAuditStatus::Passed => FANGYUAN_HOME_AUDIT_STATUS_PASSED,
        FangyuanAuditStatus::PassedWithWarnings => FANGYUAN_HOME_AUDIT_STATUS_WARNING,
        FangyuanAuditStatus::Failed => FANGYUAN_HOME_AUDIT_STATUS_FAILED,
    }
}

fn primary_audit_finding(report: &FangyuanAuditReport) -> Option<&FangyuanAuditFinding> {
    report
        .findings
        .iter()
        .find(|finding| finding.severity == FangyuanAuditSeverity::Error)
        .or_else(|| {
            report
                .findings
                .iter()
                .find(|finding| finding.severity == FangyuanAuditSeverity::Warning)
        })
        .or_else(|| report.findings.first())
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

#[derive(Clone, Debug, Component, PartialEq)]
struct FangyuanHomeBlueprintMergedMeshVisual {
    session_id: SceneSessionId,
    mesh_index: usize,
    primitive_count: usize,
    vertex_count: usize,
    index_count: usize,
    bounds: FangyuanStaticMeshBounds,
    debug_name: String,
}

#[derive(Clone, Debug, Component, PartialEq)]
struct FangyuanHomeBlueprintStaticInstanceVisual {
    session_id: SceneSessionId,
    batch_index: usize,
    instance_index: usize,
    kind: FangyuanPrimitiveKind,
    color: Color,
    source: FangyuanStaticMergeSourceRef,
    buffer_source: FangyuanStaticInstanceBufferSource,
    buffer_bytes: usize,
    debug_name: String,
}

#[derive(Clone, Debug, Component, PartialEq)]
struct FangyuanHomeObjectTrialVisual {
    session_id: SceneSessionId,
    class: FangyuanObjectClass,
    object_id: String,
    primitive_index: usize,
    kind: FangyuanPrimitiveKind,
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
    render_config: Res<FangyuanHomeBlueprintRenderConfig>,
    mut static_merge_runtime: ResMut<FangyuanHomeStaticMergeRuntime>,
    mut static_instance_runtime: ResMut<FangyuanHomeStaticInstanceRuntime>,
    mut trial_runtime: ResMut<FangyuanObjectTrialRuntime>,
    mut trial_render_runtime: ResMut<FangyuanHomeObjectTrialRenderRuntime>,
    mut lod_runtime: ResMut<FangyuanHomeLodIntegrationRuntime>,
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
            &render_config,
            &mut static_merge_runtime,
            &mut static_instance_runtime,
            &mut trial_runtime,
            &mut trial_render_runtime,
            &mut lod_runtime,
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
    render_config: &FangyuanHomeBlueprintRenderConfig,
    static_merge_runtime: &mut FangyuanHomeStaticMergeRuntime,
    static_instance_runtime: &mut FangyuanHomeStaticInstanceRuntime,
    trial_runtime: &mut FangyuanObjectTrialRuntime,
    trial_render_runtime: &mut FangyuanHomeObjectTrialRenderRuntime,
    lod_runtime: &mut FangyuanHomeLodIntegrationRuntime,
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
    let blueprint_lod_descriptors = spawn_fangyuan_home_blueprint_from_layout(
        commands,
        content,
        session_id,
        layout,
        meshes,
        materials,
        blueprint_assets,
        render_config,
        static_merge_runtime,
        static_instance_runtime,
        blueprint_stats,
    )
    .map(|spawned| spawned.lod_descriptors)
    .unwrap_or_default();
    let trial_lod_descriptors = start_fangyuan_home_trial_runtime(
        commands,
        content,
        session_id,
        meshes,
        materials,
        blueprint_assets,
        trial_render_runtime,
        trial_runtime,
        blueprint_stats,
        0,
    );
    record_fangyuan_home_lod_integration_summary(
        lod_runtime,
        blueprint_stats,
        blueprint_lod_descriptors,
        trial_lod_descriptors,
    );

    content
}

fn spawn_fangyuan_home_blueprint_from_layout(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    _layout: &FangyuanHomeLayout,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    blueprint_assets: &mut FangyuanHomeBlueprintRenderAssets,
    render_config: &FangyuanHomeBlueprintRenderConfig,
    static_merge_runtime: &mut FangyuanHomeStaticMergeRuntime,
    static_instance_runtime: &mut FangyuanHomeStaticInstanceRuntime,
    blueprint_stats: &mut FangyuanHomeBlueprintStats,
) -> Option<FangyuanHomeBlueprintSpawnedContent> {
    spawn_fangyuan_home_blueprint_from_layout_with_loader(
        commands,
        parent,
        session_id,
        meshes,
        materials,
        blueprint_assets,
        render_config,
        static_merge_runtime,
        static_instance_runtime,
        blueprint_stats,
        || {
            let scene_layout = load_fangyuan_home_scene_layout().map_err(|error| {
                format!(
                    "failed to load fangyuan home scene layout: layout_path={}, palette_path={}, error={error}",
                    FANGYUAN_HOME_SCENE_LAYOUT_PATH, FANGYUAN_HOME_PREFAB_PALETTE_PATH
                )
            })?;
            let prefab_palette = load_fangyuan_home_prefab_palette().map_err(|error| {
                format!(
                    "failed to load fangyuan home prefab palette: layout_path={}, palette_path={}, error={error}",
                    FANGYUAN_HOME_SCENE_LAYOUT_PATH, FANGYUAN_HOME_PREFAB_PALETTE_PATH
                )
            })?;
            let audit_report = scene_layout.audit_with_default_budget(&prefab_palette);
            if audit_report.status == FangyuanAuditStatus::Failed {
                return Ok(FangyuanHomeLayoutLoadResult::audit_failed(audit_report));
            }
            match scene_layout.compile_with_palette(&prefab_palette) {
                Ok(compile_report) => Ok(FangyuanHomeLayoutLoadResult::loaded(
                    audit_report,
                    compile_report,
                )),
                Err(error) => {
                    let failure = format!(
                        "failed to compile fangyuan home scene layout: layout_path={}, palette_path={}, code={}, field_path={}, reason={}",
                        FANGYUAN_HOME_SCENE_LAYOUT_PATH,
                        FANGYUAN_HOME_PREFAB_PALETTE_PATH,
                        error.code(),
                        error.field_path(),
                        error.reason()
                    );
                    Ok(FangyuanHomeLayoutLoadResult::compile_failed(
                        audit_report,
                        failure,
                    ))
                }
            }
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn spawn_fangyuan_home_blueprint_from_layout_with_loader(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    blueprint_assets: &mut FangyuanHomeBlueprintRenderAssets,
    render_config: &FangyuanHomeBlueprintRenderConfig,
    static_merge_runtime: &mut FangyuanHomeStaticMergeRuntime,
    static_instance_runtime: &mut FangyuanHomeStaticInstanceRuntime,
    blueprint_stats: &mut FangyuanHomeBlueprintStats,
    load_scene_layout: impl FnOnce() -> Result<FangyuanHomeLayoutLoadResult, String>,
) -> Option<FangyuanHomeBlueprintSpawnedContent> {
    let load_result = match load_scene_layout() {
        Ok(load_result) => load_result,
        Err(error) => {
            warn!("{error}");
            blueprint_stats.record_layout_failed(
                session_id,
                FANGYUAN_HOME_SCENE_LAYOUT_PATH,
                FANGYUAN_HOME_PREFAB_PALETTE_PATH,
                blueprint_assets.material_count(),
                None,
            );
            log_fangyuan_home_blueprint_stats(blueprint_stats);
            return None;
        }
    };

    log_fangyuan_home_audit_result(&load_result.audit_report);
    let Some(compile_report) = load_result.compile_report else {
        if let Some(error) = load_result.failure.as_deref() {
            warn!("{error}");
        }
        blueprint_stats.record_layout_failed(
            session_id,
            FANGYUAN_HOME_SCENE_LAYOUT_PATH,
            FANGYUAN_HOME_PREFAB_PALETTE_PATH,
            blueprint_assets.material_count(),
            Some(&load_result.audit_report),
        );
        log_fangyuan_home_blueprint_stats(blueprint_stats);
        return None;
    };

    for warning in &compile_report.warnings {
        warn!("skipping fangyuan home scene layout primitive: {warning}");
    }

    let content = spawn_fangyuan_home_blueprint_content(
        commands,
        parent,
        session_id,
        &compile_report.primitive_set,
        meshes,
        materials,
        blueprint_assets,
        render_config,
        static_merge_runtime,
        static_instance_runtime,
    );
    blueprint_stats.record_layout_loaded(
        session_id,
        FANGYUAN_HOME_SCENE_LAYOUT_PATH,
        FANGYUAN_HOME_PREFAB_PALETTE_PATH,
        &load_result.audit_report,
        &compile_report,
        content.render_summary.clone(),
    );
    log_fangyuan_home_blueprint_stats(blueprint_stats);
    Some(content)
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
fn spawn_fangyuan_home_simple_blueprint_from_layout_with_loader(
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
        warn!("skipping fangyuan home simple blueprint primitive: {warning}");
    }

    let content = spawn_fangyuan_home_blueprint_content(
        commands,
        parent,
        session_id,
        &compile_report.primitive_set,
        meshes,
        materials,
        blueprint_assets,
        &FangyuanHomeBlueprintRenderConfig::default(),
        &mut FangyuanHomeStaticMergeRuntime::default(),
        &mut FangyuanHomeStaticInstanceRuntime::default(),
    );
    blueprint_stats.record_loaded(
        session_id,
        blueprint_path,
        &compile_report.primitive_set,
        compile_report.skipped_primitives,
    );
    log_fangyuan_home_blueprint_stats(blueprint_stats);
    Some(content.entity)
}

fn spawn_fangyuan_home_blueprint_content(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    primitive_set: &FangyuanPrimitiveSet,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    blueprint_assets: &mut FangyuanHomeBlueprintRenderAssets,
    render_config: &FangyuanHomeBlueprintRenderConfig,
    static_merge_runtime: &mut FangyuanHomeStaticMergeRuntime,
    static_instance_runtime: &mut FangyuanHomeStaticInstanceRuntime,
) -> FangyuanHomeBlueprintSpawnedContent {
    static_merge_runtime.clear_assets(meshes, materials);
    static_instance_runtime.clear();
    let primitive_stats = FangyuanPrimitiveSetStats::from_primitive_set_with_material_registry(
        primitive_set,
        blueprint_assets.material_registry(),
    );

    match render_config.mode {
        FangyuanHomeBlueprintRenderMode::Standard => {
            let entity = spawn_fangyuan_home_blueprint_standard_content(
                commands,
                parent,
                session_id,
                primitive_set,
                meshes,
                materials,
                blueprint_assets,
            );
            FangyuanHomeBlueprintSpawnedContent {
                entity,
                render_summary: FangyuanHomeBlueprintRenderSummary::standard()
                    .with_material_stats(&primitive_stats),
                lod_descriptors: fangyuan_home_blueprint_lod_descriptors(
                    primitive_set,
                    FangyuanLodRenderPath::Standard,
                ),
            }
        }
        FangyuanHomeBlueprintRenderMode::CpuMerge => {
            match fangyuan_static_meshes_from_primitive_set_with_source(
                primitive_set,
                Some(FANGYUAN_HOME_SCENE_LAYOUT_PATH.to_string()),
                &render_config.mesh_options,
            ) {
                Ok(report) => {
                    let entity = spawn_fangyuan_home_blueprint_cpu_merge_content(
                        commands,
                        parent,
                        session_id,
                        primitive_set,
                        meshes,
                        materials,
                        blueprint_assets,
                        static_merge_runtime,
                        report,
                    );
                    FangyuanHomeBlueprintSpawnedContent {
                        entity,
                        render_summary: FangyuanHomeBlueprintRenderSummary::cpu_merge()
                            .with_material_stats(&primitive_stats),
                        lod_descriptors: fangyuan_home_blueprint_lod_descriptors(
                            primitive_set,
                            FangyuanLodRenderPath::StaticMerge,
                        ),
                    }
                }
                Err(error) => {
                    warn!(
                        "fangyuan home CPU merge failed; fallback_to_standard={}, error={error}",
                        render_config.fallback_to_standard_on_merge_failure
                    );
                    static_merge_runtime.record_failure(
                        &error,
                        render_config.fallback_to_standard_on_merge_failure,
                    );
                    if !render_config.fallback_to_standard_on_merge_failure {
                        let entity = spawn_fangyuan_home_empty_blueprint_content(
                            commands,
                            parent,
                            session_id,
                            primitive_set,
                            "FangyuanHomeObjectMergeFailed",
                        );
                        return FangyuanHomeBlueprintSpawnedContent {
                            entity,
                            render_summary: FangyuanHomeBlueprintRenderSummary::cpu_merge()
                                .with_material_stats(&primitive_stats),
                            lod_descriptors: fangyuan_home_blueprint_lod_descriptors(
                                primitive_set,
                                FangyuanLodRenderPath::Hidden,
                            ),
                        };
                    }
                    let entity = spawn_fangyuan_home_blueprint_standard_content(
                        commands,
                        parent,
                        session_id,
                        primitive_set,
                        meshes,
                        materials,
                        blueprint_assets,
                    );
                    FangyuanHomeBlueprintSpawnedContent {
                        entity,
                        render_summary: FangyuanHomeBlueprintRenderSummary::standard()
                            .with_material_stats(&primitive_stats),
                        lod_descriptors: fangyuan_home_blueprint_lod_descriptors(
                            primitive_set,
                            FangyuanLodRenderPath::Standard,
                        ),
                    }
                }
            }
        }
        FangyuanHomeBlueprintRenderMode::StaticInstance => {
            match fangyuan_static_instance_render_report_from_primitive_set_with_source(
                primitive_set,
                Some(FANGYUAN_HOME_SCENE_LAYOUT_PATH.to_string()),
                &render_config.instance_options,
            ) {
                Ok(report) => {
                    let stats = report.stats.clone();
                    let entity = spawn_fangyuan_home_blueprint_static_instance_content(
                        commands,
                        parent,
                        session_id,
                        primitive_set,
                        meshes,
                        materials,
                        blueprint_assets,
                        static_instance_runtime,
                        report,
                    );
                    FangyuanHomeBlueprintSpawnedContent {
                        entity,
                        render_summary: FangyuanHomeBlueprintRenderSummary::static_instance(&stats)
                            .with_material_stats(&primitive_stats),
                        lod_descriptors: fangyuan_home_blueprint_lod_descriptors(
                            primitive_set,
                            FangyuanLodRenderPath::StaticInstancing,
                        ),
                    }
                }
                Err(error) => {
                    warn!(
                        "fangyuan home static instance prototype failed; fallback_to_standard={}, error={error}",
                        render_config.fallback_to_standard_on_instance_failure
                    );
                    static_instance_runtime.record_failure(
                        &error,
                        render_config.fallback_to_standard_on_instance_failure,
                    );
                    if !render_config.fallback_to_standard_on_instance_failure {
                        let entity = spawn_fangyuan_home_empty_blueprint_content(
                            commands,
                            parent,
                            session_id,
                            primitive_set,
                            "FangyuanHomeObjectStaticInstanceFailed",
                        );
                        return FangyuanHomeBlueprintSpawnedContent {
                            entity,
                            render_summary:
                                FangyuanHomeBlueprintRenderSummary::static_instance_failed(
                                    static_instance_runtime.fallback_reason(),
                                )
                                .with_material_stats(&primitive_stats),
                            lod_descriptors: fangyuan_home_blueprint_lod_descriptors(
                                primitive_set,
                                FangyuanLodRenderPath::Hidden,
                            ),
                        };
                    }
                    let entity = spawn_fangyuan_home_blueprint_standard_content(
                        commands,
                        parent,
                        session_id,
                        primitive_set,
                        meshes,
                        materials,
                        blueprint_assets,
                    );
                    FangyuanHomeBlueprintSpawnedContent {
                        entity,
                        render_summary:
                            FangyuanHomeBlueprintRenderSummary::static_instance_fallback(
                                static_instance_runtime.fallback_reason(),
                            )
                            .with_material_stats(&primitive_stats),
                        lod_descriptors: fangyuan_home_blueprint_lod_descriptors(
                            primitive_set,
                            FangyuanLodRenderPath::Standard,
                        ),
                    }
                }
            }
        }
    }
}

fn spawn_fangyuan_home_blueprint_static_instance_content(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    primitive_set: &FangyuanPrimitiveSet,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    blueprint_assets: &mut FangyuanHomeBlueprintRenderAssets,
    static_instance_runtime: &mut FangyuanHomeStaticInstanceRuntime,
    report: FangyuanStaticInstanceRenderReport,
) -> Entity {
    let content = spawn_fangyuan_home_empty_blueprint_content(
        commands,
        parent,
        session_id,
        primitive_set,
        "FangyuanHomeObject",
    );
    let stats = report.stats.clone();

    for batch in report.batches {
        spawn_fangyuan_home_blueprint_static_instance_batch(
            commands,
            content,
            session_id,
            batch,
            meshes,
            materials,
            blueprint_assets,
        );
    }

    static_instance_runtime.record_success(stats);
    content
}

fn fangyuan_home_blueprint_lod_descriptors(
    primitive_set: &FangyuanPrimitiveSet,
    preferred_path: FangyuanLodRenderPath,
) -> Vec<FangyuanLodRenderDescriptor> {
    let kind = if preferred_path == FangyuanLodRenderPath::Hidden {
        FangyuanLodObjectKind::StaticObject
    } else {
        FangyuanLodObjectKind::HomeDecoration
    };
    fangyuan_lod_descriptors_from_primitive_set(
        "home_chunk_preview",
        "home.layout",
        kind,
        preferred_path,
        primitive_set,
    )
}

fn spawn_fangyuan_home_blueprint_static_instance_batch(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    batch: FangyuanStaticInstanceRenderBatch,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    blueprint_assets: &mut FangyuanHomeBlueprintRenderAssets,
) {
    let mesh = blueprint_assets.unit_mesh(batch.key.primitive_kind, meshes);
    for (instance_index, instance) in batch.instances.into_iter().enumerate() {
        let material = blueprint_assets.material_for_runtime_fields(
            instance.color,
            instance.alpha,
            instance.emissive,
            instance.material_profile_id.as_deref(),
            materials,
        );
        spawn_fangyuan_home_blueprint_static_instance_visual(
            commands,
            parent,
            session_id,
            batch.batch_index,
            instance_index,
            mesh.clone(),
            material,
            instance.position,
            instance.scale,
            instance.color,
            batch.key.primitive_kind,
            batch.buffer_source.clone(),
            batch.buffer_bytes,
            batch.debug_name.clone(),
            instance.source,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_fangyuan_home_blueprint_static_instance_visual(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    batch_index: usize,
    instance_index: usize,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    position: Vec3,
    scale: Vec3,
    color: Color,
    kind: FangyuanPrimitiveKind,
    buffer_source: FangyuanStaticInstanceBufferSource,
    buffer_bytes: usize,
    debug_name: String,
    source: FangyuanStaticMergeSourceRef,
) -> Entity {
    let entity = commands
        .spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            NoAutomaticBatching,
            Transform::from_translation(position).with_scale(scale),
            SceneOwned::new(session_id.clone()),
            FangyuanHomeBlueprintStaticInstanceVisual {
                session_id: session_id.clone(),
                batch_index,
                instance_index,
                kind,
                color,
                source,
                buffer_source,
                buffer_bytes,
                debug_name: debug_name.clone(),
            },
            Name::new(format!(
                "FangyuanHomeBlueprintStaticInstance({batch_index}:{instance_index}:{debug_name})"
            )),
        ))
        .id();
    commands.entity(parent).add_child(entity);
    entity
}

fn spawn_fangyuan_home_empty_blueprint_content(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    primitive_set: &FangyuanPrimitiveSet,
    name_prefix: &str,
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
            Name::new(format!("{name_prefix}({session_id})")),
        ))
        .id();
    commands.entity(parent).add_child(content);
    content
}

fn spawn_fangyuan_home_blueprint_standard_content(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    primitive_set: &FangyuanPrimitiveSet,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    blueprint_assets: &mut FangyuanHomeBlueprintRenderAssets,
) -> Entity {
    let content = spawn_fangyuan_home_empty_blueprint_content(
        commands,
        parent,
        session_id,
        primitive_set,
        "FangyuanHomeObject",
    );
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

fn spawn_fangyuan_home_blueprint_cpu_merge_content(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    primitive_set: &FangyuanPrimitiveSet,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    blueprint_assets: &mut FangyuanHomeBlueprintRenderAssets,
    static_merge_runtime: &mut FangyuanHomeStaticMergeRuntime,
    report: FangyuanStaticMeshBuildReport,
) -> Entity {
    let content = spawn_fangyuan_home_empty_blueprint_content(
        commands,
        parent,
        session_id,
        primitive_set,
        "FangyuanHomeObject",
    );
    let mut mesh_handles = Vec::with_capacity(report.meshes.len());
    let mut material_handles = Vec::with_capacity(report.meshes.len());
    let stats = report.stats.clone();

    for (mesh_index, group_mesh) in report.meshes.into_iter().enumerate() {
        let mesh_handle = meshes.add(group_mesh.mesh);
        let material_handle =
            blueprint_assets.material_for_static_merge_material(&group_mesh.material, materials);
        spawn_fangyuan_home_blueprint_merged_mesh(
            commands,
            content,
            session_id,
            mesh_index,
            mesh_handle.clone(),
            material_handle.clone(),
            group_mesh.metadata,
        );
        mesh_handles.push(mesh_handle);
        material_handles.push(material_handle);
    }

    static_merge_runtime.record_success(mesh_handles, material_handles, stats);
    content
}

fn spawn_fangyuan_home_blueprint_merged_mesh(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    mesh_index: usize,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    metadata: FangyuanStaticMeshMetadata,
) -> Entity {
    let entity = commands
        .spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            NoAutomaticBatching,
            Transform::default(),
            SceneOwned::new(session_id.clone()),
            FangyuanHomeBlueprintMergedMeshVisual {
                session_id: session_id.clone(),
                mesh_index,
                primitive_count: metadata.primitive_count,
                vertex_count: metadata.vertex_count,
                index_count: metadata.index_count,
                bounds: metadata.bounds,
                debug_name: metadata.debug_name.clone(),
            },
            Name::new(format!(
                "FangyuanHomeBlueprintMergedMesh({mesh_index}:{})",
                metadata.debug_name
            )),
        ))
        .id();
    commands.entity(parent).add_child(entity);
    entity
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
    let material = blueprint_assets.material(primitive, materials);
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

fn spawn_fangyuan_home_trial_visuals(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    visual_primitives: Vec<FangyuanObjectTrialVisualPrimitive>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    blueprint_assets: &mut FangyuanHomeBlueprintRenderAssets,
    trial_render_runtime: &mut FangyuanHomeObjectTrialRenderRuntime,
) -> usize {
    let mut spawned = 0;
    for visual_primitive in visual_primitives {
        spawn_fangyuan_home_trial_visual(
            commands,
            parent,
            session_id,
            visual_primitive,
            meshes,
            materials,
            blueprint_assets,
            trial_render_runtime,
        );
        spawned += 1;
    }
    spawned
}

fn spawn_fangyuan_home_trial_visual(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    visual_primitive: FangyuanObjectTrialVisualPrimitive,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    blueprint_assets: &mut FangyuanHomeBlueprintRenderAssets,
    trial_render_runtime: &mut FangyuanHomeObjectTrialRenderRuntime,
) -> Entity {
    let mesh = blueprint_assets.unit_mesh(visual_primitive.primitive.kind, meshes);
    let material_params = blueprint_assets
        .material_registry()
        .compose_primitive(&visual_primitive.primitive);
    let material = materials.add(fangyuan_standard_material_from_params(&material_params));
    trial_render_runtime.record_material(material.clone());
    let transform = fangyuan_render_transform_from_primitive(&visual_primitive.primitive);
    let entity = commands
        .spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            NoAutomaticBatching,
            transform,
            SceneOwned::new(session_id.clone()),
            FangyuanHomeObjectTrialVisual {
                session_id: session_id.clone(),
                class: visual_primitive.class,
                object_id: visual_primitive.object_id.clone(),
                primitive_index: visual_primitive.primitive_index,
                kind: visual_primitive.primitive.kind,
            },
            Name::new(format!(
                "FangyuanHomeObjectTrialVisual({}:{}:{})",
                visual_primitive.class.as_str(),
                visual_primitive.object_id,
                visual_primitive.primitive_index
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
    trial_visuals: Query<(Entity, &FangyuanHomeObjectTrialVisual)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut blueprint_assets: ResMut<FangyuanHomeBlueprintRenderAssets>,
    render_config: Res<FangyuanHomeBlueprintRenderConfig>,
    mut static_merge_runtime: ResMut<FangyuanHomeStaticMergeRuntime>,
    mut static_instance_runtime: ResMut<FangyuanHomeStaticInstanceRuntime>,
    mut trial_runtime: ResMut<FangyuanObjectTrialRuntime>,
    mut trial_render_runtime: ResMut<FangyuanHomeObjectTrialRenderRuntime>,
    mut lod_runtime: ResMut<FangyuanHomeLodIntegrationRuntime>,
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
            static_merge_runtime.clear_assets(&mut meshes, &mut materials);
            static_instance_runtime.clear();
            clear_fangyuan_home_trial_runtime(
                &mut commands,
                &session_id,
                trial_visuals.iter(),
                &mut trial_runtime,
                &mut trial_render_runtime,
                &mut materials,
                &mut blueprint_stats,
            );
            lod_runtime.clear();
            blueprint_stats.record_lod_summary(&lod_runtime.summary);
            blueprint_stats.materials = blueprint_assets.material_count();
            blueprint_stats.record_cleared(&session_id);
        }
        FangyuanHomeBlueprintCommand::RerunTrialAudit => {
            let summary = trial_runtime.rerun_audit();
            blueprint_stats.record_trial_summary(&summary);
            log_fangyuan_home_blueprint_stats(&blueprint_stats);
        }
        FangyuanHomeBlueprintCommand::SwitchTrialBudget => {
            let summary = trial_runtime.switch_budget_profile();
            blueprint_stats.record_trial_summary(&summary);
            log_fangyuan_home_blueprint_stats(&blueprint_stats);
        }
        FangyuanHomeBlueprintCommand::Reload => {
            clear_fangyuan_home_blueprint_content(
                &mut commands,
                &session_id,
                blueprint_content.iter(),
            );
            static_merge_runtime.clear_assets(&mut meshes, &mut materials);
            static_instance_runtime.clear();
            clear_fangyuan_home_trial_runtime(
                &mut commands,
                &session_id,
                trial_visuals.iter(),
                &mut trial_runtime,
                &mut trial_render_runtime,
                &mut materials,
                &mut blueprint_stats,
            );
            lod_runtime.clear();
            blueprint_stats.record_lod_summary(&lod_runtime.summary);

            let layout = match FangyuanHomeLayout::load_first_package_ron(FANGYUAN_HOME_LAYOUT_PATH)
            {
                Ok(layout) => layout,
                Err(error) => {
                    warn!("{error}");
                    blueprint_stats.record_layout_failed(
                        &session_id,
                        FANGYUAN_HOME_SCENE_LAYOUT_PATH,
                        FANGYUAN_HOME_PREFAB_PALETTE_PATH,
                        blueprint_assets.material_count(),
                        None,
                    );
                    blueprint_stats.record_lod_summary(&lod_runtime.summary);
                    blueprint_stats.record_trial_summary(trial_runtime.summary());
                    return;
                }
            };

            if !layout.is_scene_id_valid() {
                warn!(
                    "skipping fangyuan home blueprint reload because layout scene_id `{}` does not match `{}`",
                    layout.scene_id, FANGYUAN_HOME_SCENE_ID
                );
                let blueprint_path = layout.default_blueprint_path();
                warn!(
                    "fangyuan home simple blueprint fallback is disabled for reload; keeping layout/palette failure state instead of loading `{blueprint_path}`"
                );
                blueprint_stats.record_layout_failed(
                    &session_id,
                    FANGYUAN_HOME_SCENE_LAYOUT_PATH,
                    FANGYUAN_HOME_PREFAB_PALETTE_PATH,
                    blueprint_assets.material_count(),
                    None,
                );
                blueprint_stats.record_lod_summary(&lod_runtime.summary);
                blueprint_stats.record_trial_summary(trial_runtime.summary());
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
                blueprint_stats.record_layout_failed(
                    &session_id,
                    FANGYUAN_HOME_SCENE_LAYOUT_PATH,
                    FANGYUAN_HOME_PREFAB_PALETTE_PATH,
                    blueprint_assets.material_count(),
                    None,
                );
                blueprint_stats.record_lod_summary(&lod_runtime.summary);
                blueprint_stats.record_trial_summary(trial_runtime.summary());
                return;
            };

            let blueprint_lod_descriptors = spawn_fangyuan_home_blueprint_from_layout(
                &mut commands,
                content_root,
                &session_id,
                &layout,
                &mut meshes,
                &mut materials,
                &mut blueprint_assets,
                &render_config,
                &mut static_merge_runtime,
                &mut static_instance_runtime,
                &mut blueprint_stats,
            )
            .map(|spawned| spawned.lod_descriptors)
            .unwrap_or_default();
            let trial_lod_descriptors = start_fangyuan_home_trial_runtime(
                &mut commands,
                content_root,
                &session_id,
                &mut meshes,
                &mut materials,
                &mut blueprint_assets,
                &mut trial_render_runtime,
                &mut trial_runtime,
                &mut blueprint_stats,
                0,
            );
            record_fangyuan_home_lod_integration_summary(
                &mut lod_runtime,
                &mut blueprint_stats,
                blueprint_lod_descriptors,
                trial_lod_descriptors,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn start_fangyuan_home_trial_runtime(
    commands: &mut Commands,
    parent: Entity,
    session_id: &SceneSessionId,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    blueprint_assets: &mut FangyuanHomeBlueprintRenderAssets,
    trial_render_runtime: &mut FangyuanHomeObjectTrialRenderRuntime,
    trial_runtime: &mut FangyuanObjectTrialRuntime,
    blueprint_stats: &mut FangyuanHomeBlueprintStats,
    start_tick: u64,
) -> Vec<FangyuanLodRenderDescriptor> {
    match trial_runtime.enter_default_showcase(start_tick) {
        Ok(summary) => {
            let visual_primitives = trial_runtime.visual_primitives();
            let lod_descriptors = visual_primitives
                .iter()
                .map(|visual| {
                    fangyuan_lod_descriptor_from_trial_visual("home_chunk_preview", visual)
                })
                .collect::<Vec<_>>();
            spawn_fangyuan_home_trial_visuals(
                commands,
                parent,
                session_id,
                visual_primitives,
                meshes,
                materials,
                blueprint_assets,
                trial_render_runtime,
            );
            blueprint_stats.record_trial_summary(&summary);
            lod_descriptors
        }
        Err(error) => {
            warn!("failed to start fangyuan home object trial runtime: {error:?}");
            trial_runtime.clear_scene();
            blueprint_stats.record_trial_summary(trial_runtime.summary());
            Vec::new()
        }
    }
}

fn record_fangyuan_home_lod_integration_summary(
    lod_runtime: &mut FangyuanHomeLodIntegrationRuntime,
    blueprint_stats: &mut FangyuanHomeBlueprintStats,
    mut blueprint_descriptors: Vec<FangyuanLodRenderDescriptor>,
    trial_descriptors: Vec<FangyuanLodRenderDescriptor>,
) {
    blueprint_descriptors.extend(trial_descriptors);
    let chunk_summary = FangyuanChunkDebugSummary {
        loaded_chunks: usize::from(!blueprint_descriptors.is_empty()),
        loaded_chunk_ids: if blueprint_descriptors.is_empty() {
            Vec::new()
        } else {
            vec!["home_chunk_preview".to_string()]
        },
        visible_objects: blueprint_descriptors.len(),
        load_state: if blueprint_descriptors.is_empty() {
            "pending".to_string()
        } else {
            "loaded".to_string()
        },
        failure_reason: "-".to_string(),
    };
    let metrics = hotspot_metrics_from_descriptors(&blueprint_descriptors, 1);
    let hotspot = evaluate_fangyuan_hotspot(
        metrics,
        FangyuanHotspotThresholds::default(),
        FangyuanHotspotState::default(),
    );
    let summary = summarize_fangyuan_lod_integration_from_descriptors(
        [0.0, 0.0, 0.0],
        FangyuanAoiConfig::default(),
        &chunk_summary,
        &blueprint_descriptors,
        &hotspot,
    );
    blueprint_stats.record_lod_summary(&summary);
    lod_runtime.record(summary);
}

fn clear_fangyuan_home_trial_runtime<'world>(
    commands: &mut Commands,
    session_id: &SceneSessionId,
    trial_visuals: impl IntoIterator<Item = (Entity, &'world FangyuanHomeObjectTrialVisual)>,
    trial_runtime: &mut FangyuanObjectTrialRuntime,
    trial_render_runtime: &mut FangyuanHomeObjectTrialRenderRuntime,
    materials: &mut Assets<StandardMaterial>,
    blueprint_stats: &mut FangyuanHomeBlueprintStats,
) -> usize {
    let cleared = clear_fangyuan_home_trial_visuals(commands, session_id, trial_visuals);
    trial_render_runtime.clear_assets(materials);
    trial_runtime.clear_scene();
    blueprint_stats.record_trial_summary(trial_runtime.summary());
    cleared
}

fn clear_fangyuan_home_trial_visuals<'world>(
    commands: &mut Commands,
    session_id: &SceneSessionId,
    trial_visuals: impl IntoIterator<Item = (Entity, &'world FangyuanHomeObjectTrialVisual)>,
) -> usize {
    let mut cleared = 0;
    for (entity, visual) in trial_visuals {
        if visual.session_id == *session_id {
            commands.entity(entity).try_despawn();
            cleared += 1;
        }
    }
    cleared
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

fn clear_fangyuan_home_chunks_on_exit(
    mut scene_events: MessageReader<SceneEvent>,
    mut chunk_commands: MessageWriter<FangyuanChunkCommand>,
) {
    for event in scene_events.read() {
        let SceneEvent::Exited(exited) = event else {
            continue;
        };

        if exited.scene_id.as_str() != FANGYUAN_HOME_SCENE_ID {
            continue;
        }

        chunk_commands.write(FangyuanChunkCommand::clear(
            FangyuanChunkClearReason::SceneExit,
        ));
    }
}

fn clear_fangyuan_home_render_runtime_on_exit(
    mut commands: Commands,
    mut scene_events: MessageReader<SceneEvent>,
    trial_visuals: Query<(Entity, &FangyuanHomeObjectTrialVisual)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut static_merge_runtime: ResMut<FangyuanHomeStaticMergeRuntime>,
    mut static_instance_runtime: ResMut<FangyuanHomeStaticInstanceRuntime>,
    mut trial_runtime: ResMut<FangyuanObjectTrialRuntime>,
    mut trial_render_runtime: ResMut<FangyuanHomeObjectTrialRenderRuntime>,
    mut lod_runtime: ResMut<FangyuanHomeLodIntegrationRuntime>,
) {
    for event in scene_events.read() {
        let SceneEvent::Exited(exited) = event else {
            continue;
        };

        if exited.scene_id.as_str() != FANGYUAN_HOME_SCENE_ID {
            continue;
        }

        static_merge_runtime.clear_assets(&mut meshes, &mut materials);
        static_instance_runtime.clear();
        clear_fangyuan_home_trial_visuals(&mut commands, &exited.session_id, trial_visuals.iter());
        trial_render_runtime.clear_assets(&mut materials);
        trial_runtime.exit_scene();
        lod_runtime.clear();
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
        "fangyuan home layout stats: session={session}, state={}, layout_path={}, palette_path={}, audit_status={}, audit_errors={}, audit_warnings={}, audit_code={}, audit_field_path={}, audit_reason={}, generated={}, primitives={}, skipped={}, palettes={}, prefabs={}, used_prefabs={}, instances={}, materials={}, material_profiles={}, opaque={}, transparent={}, emissive_total={:.2}, material_resources={}, render_mode={}, static_instance_batches={}, static_instance_count={}, static_instance_buffer_bytes={}, static_instance_fallback={}, trial_route={}, trial_selection={}, trial_profile={}, trial_audit_run={}, trial_status={}, trial_errors={}, trial_warnings={}, trial_suggestions={}, active_vfx={}, trial_template={}, trial_visual={}, trial_equipment={}, trial_npc={}, trial_tiandao={}, trial_budget_cost={}, trial_budget_limits={}/{}, trial_results=k{} d{} r{}, trial_fallback_missing={}, trial_fallback={}, trial_reasons={}, trial_suggestion={}, trial_findings={}, top_level_valid={}, layout_valid={}, palette_valid={}",
        stats.state_label(),
        stats.layout_path(),
        stats.palette_path(),
        stats.audit_status_label(),
        stats.audit_error_count,
        stats.audit_warning_count,
        stats.audit_primary_code(),
        stats.audit_primary_field_path,
        stats.audit_primary_reason,
        stats.generated_primitives,
        stats.primitive_total(),
        stats.skipped,
        stats.palette_count,
        stats.prefab_count,
        stats.used_prefab_count,
        stats.instance_count,
        stats.materials,
        stats.material_profile_count,
        stats.opaque_count,
        stats.transparent_count,
        stats.emissive_total,
        stats.unique_material_resource_count,
        stats.render_mode,
        stats.static_instance_batch_count,
        stats.static_instance_count,
        stats.static_instance_buffer_bytes,
        stats.static_instance_fallback_reason,
        stats.trial_route_id,
        stats.trial_selection_label,
        stats.trial_budget_profile,
        stats.trial_audit_run,
        stats.trial_audit_status,
        stats.trial_audit_error_count,
        stats.trial_audit_warning_count,
        stats.trial_audit_suggestion_count,
        stats.active_vfx_count,
        stats.trial_template_id,
        stats.trial_visual_id,
        stats.trial_equipment_count,
        stats.trial_npc_count,
        stats.trial_tiandao_count,
        stats.trial_budget_cost,
        stats.trial_budget_recommended,
        stats.trial_budget_hard,
        stats.trial_kept_count,
        stats.trial_degraded_count,
        stats.trial_rejected_count,
        stats.trial_fallback_missing_count,
        stats.trial_fallback_summary,
        stats.trial_plain_reason_summary,
        stats.trial_primary_suggestion,
        stats.trial_finding_summary,
        stats.top_level_valid,
        stats.layout_valid,
        stats.palette_valid
    );
}

fn log_fangyuan_home_audit_result(report: &FangyuanAuditReport) {
    for line in format_fangyuan_audit_debug_lines(
        report,
        FANGYUAN_HOME_AUDIT_DEBUG_MAX_FINDINGS,
        FANGYUAN_HOME_AUDIT_DEBUG_MAX_SUGGESTIONS,
    ) {
        info!(
            "fangyuan home layout audit: layout_path={}, palette_path={}, {line}",
            FANGYUAN_HOME_SCENE_LAYOUT_PATH, FANGYUAN_HOME_PREFAB_PALETTE_PATH
        );
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
        FANGYUAN_SCENE_LAYOUT_VERSION, FangyuanBlueprint, FangyuanBlueprintBounds,
        FangyuanPrefabDefinition, FangyuanPrefabPalette, FangyuanPrimitiveBlueprint,
        FangyuanPrimitiveLifecycle, FangyuanPrimitiveRole, FangyuanRenderMaterialKey,
        FangyuanRenderScaleReport, FangyuanSceneLayout, FangyuanSceneLayoutInstance,
        fangyuan_static_instance_render_report_from_primitive_set_with_source,
        fangyuan_static_merge_groups_from_primitive_set, fangyuan_static_meshes_from_primitive_set,
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
    const EXPECTED_DEFAULT_LAYOUT_GENERATED: usize = 138;
    const EXPECTED_DEFAULT_LAYOUT_SKIPPED: usize = 0;
    const EXPECTED_TOTAL_VISUALS: usize =
        1 + EXPECTED_GRID_VISUALS + EXPECTED_BOUNDARY_VISUALS + EXPECTED_LIGHT_VISUALS;

    fn app_with_fangyuan_home_system() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<FangyuanHomeBlueprintRenderAssets>()
            .init_resource::<FangyuanHomeBlueprintRenderConfig>()
            .init_resource::<FangyuanHomeStaticMergeRuntime>()
            .init_resource::<FangyuanHomeStaticInstanceRuntime>()
            .init_resource::<FangyuanObjectTrialRuntime>()
            .init_resource::<FangyuanHomeObjectTrialRenderRuntime>()
            .init_resource::<FangyuanHomeLodIntegrationRuntime>()
            .init_resource::<FangyuanHomeBlueprintStats>()
            .add_message::<SceneEvent>()
            .add_message::<FangyuanHomeBlueprintCommand>()
            .add_systems(
                Update,
                (
                    reset_fangyuan_home_blueprint_stats_on_exit,
                    clear_fangyuan_home_render_runtime_on_exit,
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
    fn entered_fangyuan_home_spawns_base_space_and_layout_under_runtime_root() {
        let mut app = app_with_fangyuan_home_system();
        let default_compile_report = default_layout_compile_report();

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
            EXPECTED_DEFAULT_LAYOUT_GENERATED
        );
        let mut cube_count = 0;
        let mut sphere_count = 0;
        let mut cube_mesh: Option<Handle<Mesh>> = None;
        let mut sphere_mesh: Option<Handle<Mesh>> = None;
        let mut materials_by_key: std::collections::HashMap<
            FangyuanRenderMaterialKey,
            Handle<StandardMaterial>,
        > = std::collections::HashMap::new();
        for (parent, owned, primitive, transform, mesh, material, _, name) in
            blueprint_primitive_entities
        {
            assert_eq!(parent.parent(), blueprint_entity);
            assert_eq!(owned.session_id, session_id);
            assert_eq!(primitive.session_id, session_id);
            assert!(primitive.index < EXPECTED_DEFAULT_LAYOUT_GENERATED);
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
            let material_key = FangyuanRenderMaterialKey::from_color(expected_primitive.color);
            match materials_by_key.get(&material_key) {
                Some(existing_material) => assert_eq!(&material.0, existing_material),
                None => {
                    materials_by_key.insert(material_key, material.0.clone());
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
        assert!(materials_by_key.len() > 1);
        assert_eq!(
            app.world()
                .resource::<FangyuanHomeBlueprintRenderAssets>()
                .material_count(),
            materials_by_key.len()
        );
        assert_eq!(
            app.world().resource::<FangyuanHomeBlueprintStats>(),
            &expected_loaded_layout_stats(&session_id, &default_compile_report)
        );
        assert_eq!(
            app.world()
                .resource::<FangyuanHomeBlueprintStats>()
                .materials,
            materials_by_key.len()
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
            EXPECTED_DEFAULT_LAYOUT_GENERATED
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
                0.25,
                0.0,
                None,
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
        let actual_color = material.base_color.to_srgba();
        let expected_color = color.with_alpha(0.25).to_srgba();
        assert!((actual_color.red - expected_color.red).abs() <= 0.00001);
        assert!((actual_color.green - expected_color.green).abs() <= 0.00001);
        assert!((actual_color.blue - expected_color.blue).abs() <= 0.00001);
        assert!((actual_color.alpha - expected_color.alpha).abs() <= 0.00001);
        assert!(matches!(material.alpha_mode.clone(), AlphaMode::Blend));
    }

    #[test]
    fn generated_layout_stats_record_default_counts() {
        let mut app = app_with_fangyuan_home_system();
        let session_id = spawn_and_enter_fangyuan_home(&mut app, "fangyuan-stats-session");

        let compile_report = default_layout_compile_report();
        let audit_report = default_layout_audit_report();
        assert_eq!(
            app.world().resource::<FangyuanHomeBlueprintStats>(),
            &expected_loaded_layout_stats(&session_id, &compile_report)
        );
        let stats = app.world().resource::<FangyuanHomeBlueprintStats>();
        assert_eq!(stats.primitive_total(), EXPECTED_DEFAULT_LAYOUT_GENERATED);
        assert_eq!(stats.primitive_stats, compile_report.primitive_set.stats());
        assert_eq!(
            stats.materials,
            unique_material_count(&compile_report.primitive_set)
        );
        assert_eq!(stats.skipped, EXPECTED_DEFAULT_LAYOUT_SKIPPED);
        assert_eq!(stats.layout_path(), FANGYUAN_HOME_SCENE_LAYOUT_PATH);
        assert_eq!(stats.palette_path(), FANGYUAN_HOME_PREFAB_PALETTE_PATH);
        assert_eq!(stats.blueprint_path(), FANGYUAN_HOME_DEFAULT_BLUEPRINT_PATH);
        assert_eq!(stats.palette_count, compile_report.palette_count);
        assert_eq!(stats.prefab_count, compile_report.prefab_count);
        assert_eq!(stats.instance_count, compile_report.instance_count);
        assert_eq!(
            stats.generated_primitives,
            compile_report.generated_primitives
        );
        assert_eq!(stats.used_prefab_count, compile_report.used_prefab_count);
        assert!(stats.top_level_valid);
        assert!(stats.layout_valid);
        assert!(stats.palette_valid);
        assert_eq!(stats.state_label(), FANGYUAN_HOME_BLUEPRINT_STATE_LOADED);
        assert_eq!(
            stats.audit_status_label(),
            FANGYUAN_HOME_AUDIT_STATUS_PASSED
        );
        assert_eq!(stats.audit_error_count, audit_report.summary.error_count);
        assert_eq!(
            stats.audit_warning_count,
            audit_report.summary.warning_count
        );
        assert_eq!(
            stats.audit_primary_code(),
            FANGYUAN_HOME_AUDIT_PRIMARY_CODE_NONE
        );
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
            EXPECTED_DEFAULT_LAYOUT_GENERATED
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
            EXPECTED_DEFAULT_LAYOUT_GENERATED
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
    fn reload_layout_command_replaces_content_without_duplicate_primitives() {
        let mut app = app_with_fangyuan_home_system();
        let session_id = spawn_and_enter_fangyuan_home(&mut app, "fangyuan-reload-session");
        let compile_report = default_layout_compile_report();

        assert_eq!(fangyuan_content_count(&mut app, &session_id), 1);
        assert_eq!(
            fangyuan_visual_count(&mut app, &session_id),
            EXPECTED_TOTAL_VISUALS
        );
        assert_eq!(fangyuan_blueprint_content_count(&mut app, &session_id), 1);
        assert_eq!(fangyuan_home_object_count(&mut app, &session_id), 1);
        assert_eq!(
            fangyuan_blueprint_primitive_count(&mut app, &session_id),
            EXPECTED_DEFAULT_LAYOUT_GENERATED
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
            EXPECTED_DEFAULT_LAYOUT_GENERATED
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
            EXPECTED_DEFAULT_LAYOUT_GENERATED
        );
        assert_eq!(
            reloaded_home_object_ref
                .get::<FangyuanObjectState>()
                .unwrap(),
            &FangyuanObjectState::default()
        );
        assert_eq!(
            app.world().resource::<FangyuanHomeBlueprintStats>(),
            &expected_loaded_layout_stats(&session_id, &compile_report)
        );
    }

    #[test]
    fn clear_blueprint_command_removes_only_layout_content() {
        let mut app = app_with_fangyuan_home_system();
        let session_id = spawn_and_enter_fangyuan_home(&mut app, "fangyuan-command-clear-session");
        let compile_report = default_layout_compile_report();

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
            &expected_cleared_layout_stats(&session_id, &compile_report)
        );
        let stats = app.world().resource::<FangyuanHomeBlueprintStats>();
        assert_eq!(
            stats.audit_status_label(),
            FANGYUAN_HOME_AUDIT_STATUS_PASSED
        );
        assert_eq!(
            stats.audit_primary_code(),
            FANGYUAN_HOME_AUDIT_PRIMARY_CODE_NONE
        );
    }

    #[test]
    fn fangyuan_trial_ecs_commands_update_audit_budget_and_return_clear_state() {
        let mut app = app_with_fangyuan_home_system();
        let session_id = spawn_and_enter_fangyuan_home(&mut app, "fangyuan-trial-command-session");
        let initial_visuals = fangyuan_home_trial_visual_count(&mut app, &session_id);
        assert!(initial_visuals > 0);
        let initial_run = app
            .world()
            .resource::<FangyuanHomeBlueprintStats>()
            .trial_audit_run;

        app.world_mut()
            .write_message(FangyuanHomeBlueprintCommand::RerunTrialAudit);
        app.update();
        {
            let stats = app.world().resource::<FangyuanHomeBlueprintStats>();
            assert_eq!(stats.trial_route_id, "fangyuan.object_trial");
            assert_eq!(stats.trial_budget_profile, "standard");
            assert!(stats.trial_audit_run > initial_run);
            assert_eq!(stats.trial_audit_status, "warning");
            assert_eq!(stats.trial_audit_error_count, 0);
            assert!(stats.trial_audit_warning_count > 0);
            assert!(stats.trial_plain_reason_summary.contains("技能颜色"));
            assert_eq!(stats.trial_fallback_missing_count, 0);
        }
        assert_eq!(
            fangyuan_home_trial_visual_count(&mut app, &session_id),
            initial_visuals
        );

        app.world_mut()
            .write_message(FangyuanHomeBlueprintCommand::SwitchTrialBudget);
        app.update();
        {
            let stats = app.world().resource::<FangyuanHomeBlueprintStats>();
            assert_eq!(stats.trial_budget_profile, "strict");
            assert_eq!(stats.trial_audit_status, "failed");
            assert!(stats.trial_audit_error_count > 0);
            assert!(stats.trial_degraded_count > 0);
            assert!(stats.trial_rejected_count > 0);
            assert!(stats.trial_plain_reason_summary.contains("primitive 过多"));
        }

        app.world_mut()
            .write_message(FangyuanHomeBlueprintCommand::Clear);
        app.update();
        assert_fangyuan_home_trial_cleared(&mut app, &session_id);
    }

    #[test]
    fn reload_layout_command_regenerates_preview_after_clear() {
        let mut app = app_with_fangyuan_home_system();
        let session_id = spawn_and_enter_fangyuan_home(&mut app, "fangyuan-clear-reload-session");
        let compile_report = default_layout_compile_report();

        assert_eq!(fangyuan_content_count(&mut app, &session_id), 1);
        assert_eq!(fangyuan_blueprint_content_count(&mut app, &session_id), 1);
        assert_eq!(fangyuan_home_object_count(&mut app, &session_id), 1);
        assert_eq!(
            fangyuan_blueprint_primitive_count(&mut app, &session_id),
            EXPECTED_DEFAULT_LAYOUT_GENERATED
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
            &expected_cleared_layout_stats(&session_id, &compile_report)
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
            EXPECTED_DEFAULT_LAYOUT_GENERATED
        );
        assert_eq!(
            app.world().resource::<FangyuanHomeBlueprintStats>(),
            &expected_loaded_layout_stats(&session_id, &compile_report)
        );
        let stats = app.world().resource::<FangyuanHomeBlueprintStats>();
        assert_eq!(
            stats.audit_status_label(),
            FANGYUAN_HOME_AUDIT_STATUS_PASSED
        );
        assert_eq!(stats.audit_error_count, 0);
        assert_eq!(stats.audit_warning_count, 0);
    }

    #[test]
    fn cpu_merge_mode_merges_default_home_layout_without_primitive_visual_entities() {
        let mut app = app_with_fangyuan_home_system();
        enable_cpu_merge_mode(&mut app);

        let session_id = spawn_and_enter_fangyuan_home(&mut app, "fangyuan-cpu-merge-session");
        let compile_report = default_layout_compile_report();

        assert_eq!(fangyuan_content_count(&mut app, &session_id), 1);
        assert_eq!(fangyuan_blueprint_content_count(&mut app, &session_id), 1);
        assert_eq!(fangyuan_home_object_count(&mut app, &session_id), 1);
        assert_eq!(fangyuan_blueprint_primitive_count(&mut app, &session_id), 0);
        let merged_records = fangyuan_blueprint_merged_mesh_records(&mut app, &session_id);
        assert!(!merged_records.is_empty());
        assert!(
            merged_records.len() < EXPECTED_DEFAULT_LAYOUT_GENERATED,
            "CPU merge path should reduce static visual entity count"
        );
        assert_eq!(
            merged_records
                .iter()
                .map(|record| record.primitive_count)
                .sum::<usize>(),
            EXPECTED_DEFAULT_LAYOUT_GENERATED
        );
        assert!(merged_records.iter().all(|record| {
            record.vertex_count > 0
                && record.index_count > 0
                && !record.bounds.is_empty()
                && record.debug_name.contains("fangyuan_static")
        }));
        assert_eq!(
            app.world().resource::<FangyuanHomeBlueprintStats>(),
            &expected_loaded_layout_stats_with_render_summary(
                &session_id,
                &compile_report,
                FangyuanHomeBlueprintRenderSummary::cpu_merge(),
            )
        );
        let runtime = app.world().resource::<FangyuanHomeStaticMergeRuntime>();
        assert_eq!(
            runtime.stats.merged_primitive_count,
            EXPECTED_DEFAULT_LAYOUT_GENERATED
        );
        assert_eq!(runtime.stats.mesh_count, merged_records.len());
        assert_eq!(runtime.last_failure, None);
        assert_eq!(
            runtime.material_handles.len(),
            1,
            "CPU merge should keep colors in vertex data and reuse the opaque material"
        );
        assert_eq!(
            static_merge_runtime_asset_counts(&app),
            (merged_records.len(), runtime.material_handles.len())
        );
    }

    #[test]
    fn cpu_merge_reload_clear_and_clear_reload_do_not_accumulate_mesh_assets() {
        let mut app = app_with_fangyuan_home_system();
        enable_cpu_merge_mode(&mut app);
        let session_id = spawn_and_enter_fangyuan_home(&mut app, "fangyuan-cpu-reload-session");
        let initial_merged = fangyuan_blueprint_merged_mesh_count(&mut app, &session_id);
        assert!(initial_merged > 0);
        assert_eq!(static_merge_runtime_asset_counts(&app), (initial_merged, 1));

        app.world_mut()
            .write_message(FangyuanHomeBlueprintCommand::Reload);
        app.update();

        assert_eq!(fangyuan_blueprint_primitive_count(&mut app, &session_id), 0);
        assert_eq!(
            fangyuan_blueprint_merged_mesh_count(&mut app, &session_id),
            initial_merged
        );
        assert_eq!(static_merge_runtime_asset_counts(&app), (initial_merged, 1));

        app.world_mut()
            .write_message(FangyuanHomeBlueprintCommand::Clear);
        app.update();

        assert_eq!(fangyuan_blueprint_content_count(&mut app, &session_id), 0);
        assert_eq!(
            fangyuan_blueprint_merged_mesh_count(&mut app, &session_id),
            0
        );
        assert_eq!(static_merge_runtime_asset_counts(&app), (0, 0));

        app.world_mut()
            .write_message(FangyuanHomeBlueprintCommand::Reload);
        app.update();

        assert_eq!(fangyuan_blueprint_primitive_count(&mut app, &session_id), 0);
        assert_eq!(
            fangyuan_blueprint_merged_mesh_count(&mut app, &session_id),
            initial_merged
        );
        assert_eq!(static_merge_runtime_asset_counts(&app), (initial_merged, 1));
    }

    #[test]
    fn cpu_merge_build_failure_falls_back_to_standard_and_clears_old_merge_assets() {
        let mut app = app_with_fangyuan_home_system();
        enable_cpu_merge_mode(&mut app);
        let session_id = spawn_and_enter_fangyuan_home(&mut app, "fangyuan-cpu-fallback-session");
        let initial_merged = fangyuan_blueprint_merged_mesh_count(&mut app, &session_id);
        assert!(initial_merged > 0);
        assert_eq!(fangyuan_blueprint_primitive_count(&mut app, &session_id), 0);
        assert_eq!(static_merge_runtime_asset_counts(&app), (initial_merged, 1));

        set_cpu_merge_budget(&mut app, 1, 1);
        app.world_mut()
            .write_message(FangyuanHomeBlueprintCommand::Reload);
        app.update();

        assert_eq!(
            fangyuan_blueprint_merged_mesh_count(&mut app, &session_id),
            0
        );
        assert_eq!(
            fangyuan_blueprint_primitive_count(&mut app, &session_id),
            EXPECTED_DEFAULT_LAYOUT_GENERATED
        );
        assert_eq!(static_merge_runtime_asset_counts(&app), (0, 0));
        let runtime = app.world().resource::<FangyuanHomeStaticMergeRuntime>();
        assert_eq!(runtime.fallback_count, 1);
        assert_eq!(runtime.stats.fallback_count, 1);
        assert!(
            runtime
                .last_failure
                .as_deref()
                .is_some_and(|failure| failure.contains("exceeds budget"))
        );
    }

    #[test]
    fn cpu_merge_fallback_can_switch_back_to_merge_after_budget_is_restored() {
        let mut app = app_with_fangyuan_home_system();
        set_cpu_merge_budget(&mut app, 1, 1);
        let session_id = spawn_and_enter_fangyuan_home(&mut app, "fangyuan-cpu-switch-session");

        assert_eq!(
            fangyuan_blueprint_merged_mesh_count(&mut app, &session_id),
            0
        );
        assert_eq!(
            fangyuan_blueprint_primitive_count(&mut app, &session_id),
            EXPECTED_DEFAULT_LAYOUT_GENERATED
        );
        assert_eq!(static_merge_runtime_asset_counts(&app), (0, 0));
        assert!(
            app.world()
                .resource::<FangyuanHomeStaticMergeRuntime>()
                .last_failure
                .is_some()
        );

        app.world_mut()
            .insert_resource(FangyuanHomeBlueprintRenderConfig {
                mode: FangyuanHomeBlueprintRenderMode::CpuMerge,
                ..Default::default()
            });
        app.world_mut()
            .write_message(FangyuanHomeBlueprintCommand::Reload);
        app.update();

        let merged_count = fangyuan_blueprint_merged_mesh_count(&mut app, &session_id);
        assert!(merged_count > 0);
        assert_eq!(fangyuan_blueprint_primitive_count(&mut app, &session_id), 0);
        assert_eq!(static_merge_runtime_asset_counts(&app), (merged_count, 1));
        assert_eq!(
            app.world()
                .resource::<FangyuanHomeStaticMergeRuntime>()
                .last_failure,
            None
        );
    }

    #[test]
    fn static_instance_mode_spawns_shared_mesh_prototype_from_default_home_layout() {
        let mut app = app_with_fangyuan_home_system();
        enable_static_instance_mode(&mut app);

        let session_id =
            spawn_and_enter_fangyuan_home(&mut app, "fangyuan-static-instance-session");
        let compile_report = default_layout_compile_report();

        assert_eq!(fangyuan_blueprint_primitive_count(&mut app, &session_id), 0);
        assert_eq!(
            fangyuan_blueprint_merged_mesh_count(&mut app, &session_id),
            0
        );
        let records = fangyuan_blueprint_static_instance_records(&mut app, &session_id);
        assert_eq!(records.len(), EXPECTED_DEFAULT_LAYOUT_GENERATED);
        assert!(
            records
                .iter()
                .any(|record| record.kind == FangyuanPrimitiveKind::Cube)
        );
        assert!(
            records
                .iter()
                .any(|record| record.kind == FangyuanPrimitiveKind::Sphere)
        );
        assert!(records.iter().all(|record| {
            record.buffer_source.instance_count > 0
                && record.buffer_bytes > 0
                && record.debug_name.contains("fangyuan_static_instance")
        }));

        let mut static_instances = app.world_mut().query::<(
            &FangyuanHomeBlueprintStaticInstanceVisual,
            &Transform,
            &Mesh3d,
            &MeshMaterial3d<StandardMaterial>,
        )>();
        let mut cube_mesh: Option<Handle<Mesh>> = None;
        let mut sphere_mesh: Option<Handle<Mesh>> = None;
        for (visual, transform, mesh, material) in static_instances.iter(app.world()) {
            if visual.session_id != session_id {
                continue;
            }
            let expected_primitive = compile_report
                .primitive_set
                .primitives()
                .iter()
                .find(|primitive| {
                    primitive.local_position == transform.translation
                        && primitive.scale == transform.scale
                        && primitive.kind == visual.kind
                        && primitive.color.with_alpha(primitive.alpha) == visual.color
                })
                .expect("static instance visual should consume primitive position scale and color");
            assert_eq!(transform.rotation, Quat::IDENTITY);
            assert!(matches!(
                visual.source.source_kind,
                crate::framework::fangyuan::FangyuanStaticMergeSourceKind::RuntimePrimitiveSet
            ));
            assert_color_nearly_eq(
                material_color(&app, &material.0),
                expected_primitive
                    .color
                    .with_alpha(expected_primitive.alpha),
            );
            match visual.kind {
                FangyuanPrimitiveKind::Cube => {
                    if let Some(cube_mesh) = &cube_mesh {
                        assert_eq!(&mesh.0, cube_mesh);
                    } else {
                        cube_mesh = Some(mesh.0.clone());
                    }
                }
                FangyuanPrimitiveKind::Sphere => {
                    if let Some(sphere_mesh) = &sphere_mesh {
                        assert_eq!(&mesh.0, sphere_mesh);
                    } else {
                        sphere_mesh = Some(mesh.0.clone());
                    }
                }
            }
        }
        let cube_mesh = cube_mesh.expect("static instance layout should include cubes");
        let sphere_mesh = sphere_mesh.expect("static instance layout should include spheres");
        assert_ne!(cube_mesh, sphere_mesh);
        let render_assets = app.world().resource::<FangyuanHomeBlueprintRenderAssets>();
        assert_eq!(render_assets.unit_cube_mesh(), Some(&cube_mesh));
        assert_eq!(render_assets.unit_sphere_mesh(), Some(&sphere_mesh));

        let runtime = app.world().resource::<FangyuanHomeStaticInstanceRuntime>();
        assert_eq!(
            runtime.stats.instance_count,
            EXPECTED_DEFAULT_LAYOUT_GENERATED
        );
        assert_eq!(
            runtime.stats.batch_count,
            unique_static_instance_batch_count(&records)
        );
        assert_eq!(runtime.last_failure, None);
        let stats = app.world().resource::<FangyuanHomeBlueprintStats>();
        assert_eq!(stats.render_mode, "static_instance");
        assert_eq!(
            stats.static_instance_count,
            EXPECTED_DEFAULT_LAYOUT_GENERATED
        );
        assert_eq!(stats.static_instance_batch_count, runtime.stats.batch_count);
        assert_eq!(
            stats.static_instance_buffer_bytes,
            runtime.stats.buffer_bytes
        );
        assert_eq!(stats.static_instance_fallback_reason, "-");
    }

    #[test]
    fn static_instance_reload_clear_exit_and_mode_switch_clear_runtime_state() {
        let mut app = app_with_fangyuan_home_system();
        enable_static_instance_mode(&mut app);
        let session_id = spawn_and_enter_fangyuan_home(&mut app, "fangyuan-static-lifecycle");
        let initial_records = fangyuan_blueprint_static_instance_count(&mut app, &session_id);
        assert_eq!(initial_records, EXPECTED_DEFAULT_LAYOUT_GENERATED);
        assert!(
            app.world()
                .resource::<FangyuanHomeStaticInstanceRuntime>()
                .stats
                .instance_count
                > 0
        );

        app.world_mut()
            .write_message(FangyuanHomeBlueprintCommand::Reload);
        app.update();

        assert_eq!(
            fangyuan_blueprint_static_instance_count(&mut app, &session_id),
            initial_records
        );
        assert_eq!(fangyuan_blueprint_primitive_count(&mut app, &session_id), 0);

        app.world_mut()
            .write_message(FangyuanHomeBlueprintCommand::Clear);
        app.update();

        assert_eq!(
            fangyuan_blueprint_static_instance_count(&mut app, &session_id),
            0
        );
        assert_eq!(
            app.world()
                .resource::<FangyuanHomeStaticInstanceRuntime>()
                .stats
                .instance_count,
            0
        );

        app.world_mut()
            .write_message(FangyuanHomeBlueprintCommand::Reload);
        app.update();
        assert_eq!(
            fangyuan_blueprint_static_instance_count(&mut app, &session_id),
            initial_records
        );

        app.world_mut()
            .resource_mut::<FangyuanHomeBlueprintRenderConfig>()
            .mode = FangyuanHomeBlueprintRenderMode::CpuMerge;
        app.world_mut()
            .write_message(FangyuanHomeBlueprintCommand::Reload);
        app.update();

        assert_eq!(
            fangyuan_blueprint_static_instance_count(&mut app, &session_id),
            0
        );
        assert!(fangyuan_blueprint_merged_mesh_count(&mut app, &session_id) > 0);
        assert_eq!(
            app.world()
                .resource::<FangyuanHomeStaticInstanceRuntime>()
                .stats
                .instance_count,
            0
        );
        assert_eq!(
            app.world()
                .resource::<FangyuanHomeBlueprintStats>()
                .render_mode,
            "cpu_merge"
        );

        app.world_mut().write_message(SceneEvent::Exited(
            crate::framework::scene::prelude::SceneExited {
                scene_id: FANGYUAN_HOME_SCENE_ID.into(),
                session_id: session_id.clone(),
            },
        ));
        app.update();

        assert_eq!(
            app.world()
                .resource::<FangyuanHomeStaticInstanceRuntime>()
                .stats
                .instance_count,
            0
        );
        assert_eq!(static_merge_runtime_asset_counts(&app), (0, 0));
    }

    #[test]
    fn stage9_reload_clear_lobby_return_mode_switch_and_reenter_do_not_leave_residual_content() {
        let mut app = app_with_scene_lifecycle();
        enable_static_instance_mode(&mut app);

        let session_id = SceneSessionId::from("fangyuan-stage9-session-a");
        let mut request = SceneEnterRequest::new(FANGYUAN_HOME_SCENE_ID);
        request.session_id = Some(session_id.clone());
        app.world_mut().write_message(SceneCommand::Enter(request));
        app.update();

        assert_eq!(
            app.world()
                .resource::<SceneRuntime>()
                .active_session_id()
                .map(|session| session.as_str()),
            Some("fangyuan-stage9-session-a")
        );
        assert_eq!(fangyuan_content_count(&mut app, &session_id), 1);
        assert_eq!(
            fangyuan_visual_count(&mut app, &session_id),
            EXPECTED_TOTAL_VISUALS
        );
        assert_eq!(fangyuan_blueprint_content_count(&mut app, &session_id), 1);
        assert_eq!(fangyuan_home_object_count(&mut app, &session_id), 1);
        assert_eq!(fangyuan_blueprint_primitive_count(&mut app, &session_id), 0);
        assert_eq!(
            fangyuan_blueprint_merged_mesh_count(&mut app, &session_id),
            0
        );
        assert_eq!(
            fangyuan_blueprint_static_instance_count(&mut app, &session_id),
            EXPECTED_DEFAULT_LAYOUT_GENERATED
        );
        assert_eq!(
            app.world()
                .resource::<FangyuanHomeBlueprintStats>()
                .render_mode,
            "static_instance"
        );
        assert_fangyuan_home_trial_active(&mut app, &session_id);
        let initial_trial_visuals = fangyuan_home_trial_visual_count(&mut app, &session_id);
        assert!(initial_trial_visuals > 0);

        app.world_mut()
            .write_message(FangyuanHomeBlueprintCommand::Reload);
        app.update();

        assert_eq!(fangyuan_content_count(&mut app, &session_id), 1);
        assert_eq!(fangyuan_blueprint_content_count(&mut app, &session_id), 1);
        assert_eq!(fangyuan_home_object_count(&mut app, &session_id), 1);
        assert_eq!(fangyuan_blueprint_primitive_count(&mut app, &session_id), 0);
        assert_eq!(
            fangyuan_blueprint_merged_mesh_count(&mut app, &session_id),
            0
        );
        assert_eq!(
            fangyuan_blueprint_static_instance_count(&mut app, &session_id),
            EXPECTED_DEFAULT_LAYOUT_GENERATED
        );
        assert_fangyuan_home_trial_active(&mut app, &session_id);
        assert_eq!(
            fangyuan_home_trial_visual_count(&mut app, &session_id),
            initial_trial_visuals
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
        assert_eq!(fangyuan_blueprint_primitive_count(&mut app, &session_id), 0);
        assert_eq!(
            fangyuan_blueprint_merged_mesh_count(&mut app, &session_id),
            0
        );
        assert_eq!(
            fangyuan_blueprint_static_instance_count(&mut app, &session_id),
            0
        );
        assert_eq!(
            app.world()
                .resource::<FangyuanHomeStaticInstanceRuntime>()
                .stats
                .instance_count,
            0
        );
        assert_eq!(static_merge_runtime_asset_counts(&app), (0, 0));
        assert_fangyuan_home_trial_cleared(&mut app, &session_id);

        app.world_mut()
            .write_message(FangyuanHomeBlueprintCommand::Reload);
        app.update();

        assert_eq!(fangyuan_blueprint_content_count(&mut app, &session_id), 1);
        assert_eq!(fangyuan_home_object_count(&mut app, &session_id), 1);
        assert_eq!(
            fangyuan_blueprint_static_instance_count(&mut app, &session_id),
            EXPECTED_DEFAULT_LAYOUT_GENERATED
        );
        assert_fangyuan_home_trial_active(&mut app, &session_id);

        enable_cpu_merge_mode(&mut app);
        app.world_mut()
            .write_message(FangyuanHomeBlueprintCommand::Reload);
        app.update();

        let merged_meshes = fangyuan_blueprint_merged_mesh_count(&mut app, &session_id);
        assert!(merged_meshes > 0);
        assert_eq!(fangyuan_content_count(&mut app, &session_id), 1);
        assert_eq!(fangyuan_blueprint_content_count(&mut app, &session_id), 1);
        assert_eq!(fangyuan_home_object_count(&mut app, &session_id), 1);
        assert_eq!(fangyuan_blueprint_primitive_count(&mut app, &session_id), 0);
        assert_eq!(
            fangyuan_blueprint_static_instance_count(&mut app, &session_id),
            0
        );
        assert_eq!(static_merge_runtime_asset_counts(&app), (merged_meshes, 1));
        assert_eq!(
            app.world()
                .resource::<FangyuanHomeStaticInstanceRuntime>()
                .stats
                .instance_count,
            0
        );
        assert_eq!(
            app.world()
                .resource::<FangyuanHomeBlueprintStats>()
                .render_mode,
            "cpu_merge"
        );
        assert_fangyuan_home_trial_active(&mut app, &session_id);
        assert_eq!(
            fangyuan_home_trial_visual_count(&mut app, &session_id),
            initial_trial_visuals
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
        assert_eq!(fangyuan_home_trial_visual_count(&mut app, &session_id), 0);
        assert_eq!(
            fangyuan_blueprint_merged_mesh_count(&mut app, &session_id),
            0
        );
        assert_eq!(
            fangyuan_blueprint_static_instance_count(&mut app, &session_id),
            0
        );
        assert_eq!(static_merge_runtime_asset_counts(&app), (0, 0));
        assert_eq!(
            app.world()
                .resource::<FangyuanHomeStaticInstanceRuntime>()
                .stats
                .instance_count,
            0
        );
        assert_eq!(
            app.world().resource::<FangyuanHomeBlueprintStats>(),
            &FangyuanHomeBlueprintStats::default()
        );
        assert_fangyuan_home_trial_cleared(&mut app, &session_id);
        assert_eq!(
            app.world().resource::<SceneRuntime>().active_session_id(),
            None
        );

        let reentered_session_id = SceneSessionId::from("fangyuan-stage9-session-b");
        let mut request = SceneEnterRequest::new(FANGYUAN_HOME_SCENE_ID);
        request.session_id = Some(reentered_session_id.clone());
        app.world_mut().write_message(SceneCommand::Enter(request));
        app.update();

        let reentered_merged_meshes =
            fangyuan_blueprint_merged_mesh_count(&mut app, &reentered_session_id);
        assert!(reentered_merged_meshes > 0);
        assert_eq!(fangyuan_content_count(&mut app, &reentered_session_id), 1);
        assert_eq!(
            fangyuan_visual_count(&mut app, &reentered_session_id),
            EXPECTED_TOTAL_VISUALS
        );
        assert_eq!(
            fangyuan_blueprint_content_count(&mut app, &reentered_session_id),
            1
        );
        assert_eq!(
            fangyuan_home_object_count(&mut app, &reentered_session_id),
            1
        );
        assert_eq!(
            fangyuan_blueprint_primitive_count(&mut app, &reentered_session_id),
            0
        );
        assert_eq!(
            fangyuan_blueprint_static_instance_count(&mut app, &reentered_session_id),
            0
        );
        assert_eq!(
            static_merge_runtime_asset_counts(&app),
            (reentered_merged_meshes, 1)
        );
        assert_eq!(
            app.world()
                .resource::<FangyuanHomeBlueprintStats>()
                .render_mode,
            "cpu_merge"
        );
        assert_fangyuan_home_trial_active(&mut app, &reentered_session_id);

        app.world_mut().write_message(SceneEvent::Entered(
            crate::framework::scene::prelude::SceneEntered {
                scene_id: FANGYUAN_HOME_SCENE_ID.into(),
                session_id: reentered_session_id.clone(),
                content_version: None,
            },
        ));
        app.update();

        assert_eq!(fangyuan_content_count(&mut app, &reentered_session_id), 1);
        assert_eq!(
            fangyuan_visual_count(&mut app, &reentered_session_id),
            EXPECTED_TOTAL_VISUALS
        );
        assert_eq!(
            fangyuan_blueprint_content_count(&mut app, &reentered_session_id),
            1
        );
        assert_eq!(
            fangyuan_home_object_count(&mut app, &reentered_session_id),
            1
        );
        assert_eq!(
            fangyuan_blueprint_merged_mesh_count(&mut app, &reentered_session_id),
            reentered_merged_meshes
        );
        assert_eq!(
            fangyuan_blueprint_static_instance_count(&mut app, &reentered_session_id),
            0
        );
        assert_eq!(
            static_merge_runtime_asset_counts(&app),
            (reentered_merged_meshes, 1)
        );
        assert_fangyuan_home_trial_active(&mut app, &reentered_session_id);
    }

    #[test]
    fn static_instance_budget_or_initialization_failure_falls_back_to_standard() {
        let mut app = app_with_fangyuan_home_system();
        set_static_instance_buffer_budget(&mut app, 1);
        let session_id = spawn_and_enter_fangyuan_home(&mut app, "fangyuan-static-budget");

        assert_eq!(
            fangyuan_blueprint_static_instance_count(&mut app, &session_id),
            0
        );
        assert_eq!(
            fangyuan_blueprint_primitive_count(&mut app, &session_id),
            EXPECTED_DEFAULT_LAYOUT_GENERATED
        );
        let runtime = app.world().resource::<FangyuanHomeStaticInstanceRuntime>();
        assert_eq!(runtime.fallback_count, 1);
        assert!(
            runtime
                .last_failure
                .as_deref()
                .is_some_and(|reason| reason.contains("buffer_bytes"))
        );
        let stats = app.world().resource::<FangyuanHomeBlueprintStats>();
        assert_eq!(stats.render_mode, "static_instance->standard");
        assert!(
            stats
                .static_instance_fallback_reason
                .contains("buffer_bytes")
        );

        app.world_mut()
            .resource_mut::<FangyuanHomeBlueprintRenderConfig>()
            .instance_options = FangyuanStaticInstanceRenderOptions {
            allow_cube: false,
            allow_sphere: false,
            ..Default::default()
        };
        app.world_mut()
            .write_message(FangyuanHomeBlueprintCommand::Reload);
        app.update();

        assert_eq!(
            fangyuan_blueprint_static_instance_count(&mut app, &session_id),
            0
        );
        assert_eq!(
            fangyuan_blueprint_primitive_count(&mut app, &session_id),
            EXPECTED_DEFAULT_LAYOUT_GENERATED
        );
        assert!(
            app.world()
                .resource::<FangyuanHomeStaticInstanceRuntime>()
                .last_failure
                .as_deref()
                .is_some_and(|reason| reason.contains("at least one primitive kind"))
        );
    }

    #[test]
    fn static_instance_unsupported_kind_can_fail_without_leaving_half_old_content() {
        let mut app = app_with_fangyuan_home_system();
        enable_static_instance_mode(&mut app);
        let session_id = spawn_and_enter_fangyuan_home(&mut app, "fangyuan-static-unsupported");
        assert_eq!(
            fangyuan_blueprint_static_instance_count(&mut app, &session_id),
            EXPECTED_DEFAULT_LAYOUT_GENERATED
        );

        {
            let mut config = app
                .world_mut()
                .resource_mut::<FangyuanHomeBlueprintRenderConfig>();
            config.instance_options.allow_sphere = false;
            config.fallback_to_standard_on_instance_failure = false;
        }
        app.world_mut()
            .write_message(FangyuanHomeBlueprintCommand::Reload);
        app.update();

        assert_eq!(
            fangyuan_blueprint_static_instance_count(&mut app, &session_id),
            0
        );
        assert_eq!(fangyuan_blueprint_primitive_count(&mut app, &session_id), 0);
        assert_eq!(
            fangyuan_blueprint_merged_mesh_count(&mut app, &session_id),
            0
        );
        assert_eq!(fangyuan_home_object_count(&mut app, &session_id), 1);
        let runtime = app.world().resource::<FangyuanHomeStaticInstanceRuntime>();
        assert_eq!(runtime.fallback_count, 0);
        assert!(
            runtime
                .last_failure
                .as_deref()
                .is_some_and(|reason| reason.contains("unsupported kind: sphere"))
        );
        let stats = app.world().resource::<FangyuanHomeBlueprintStats>();
        assert_eq!(stats.render_mode, "static_instance_failed");
        assert!(
            stats
                .static_instance_fallback_reason
                .contains("unsupported kind: sphere")
        );
    }

    #[test]
    fn default_home_layout_reports_explainable_standard_merge_and_instance_stats() {
        let compile_report = default_layout_compile_report();
        let primitive_stats = compile_report.primitive_set.stats();
        let merge_report =
            fangyuan_static_merge_groups_from_primitive_set(&compile_report.primitive_set);
        let mesh_report =
            fangyuan_static_meshes_from_primitive_set(&compile_report.primitive_set).unwrap();
        let instance_report =
            fangyuan_static_instance_render_report_from_primitive_set_with_source(
                &compile_report.primitive_set,
                Some(FANGYUAN_HOME_SCENE_LAYOUT_PATH.to_string()),
                &FangyuanStaticInstanceRenderOptions::default(),
            )
            .unwrap();
        let scale_report = FangyuanRenderScaleReport::from_reports(
            &primitive_stats,
            &merge_report,
            Some(&mesh_report.stats),
            &instance_report.stats,
        );

        println!(
            "fangyuan default home render scale summary: {}",
            scale_report.format_summary()
        );

        assert_eq!(primitive_stats, compile_report.primitive_stats);
        assert_eq!(primitive_stats.total, EXPECTED_DEFAULT_LAYOUT_GENERATED);
        assert_eq!(
            merge_report.stats.cube_count + merge_report.stats.sphere_count,
            EXPECTED_DEFAULT_LAYOUT_GENERATED
        );
        assert_eq!(
            mesh_report.stats.merged_primitive_count,
            EXPECTED_DEFAULT_LAYOUT_GENERATED
        );
        assert_eq!(
            instance_report.stats.instance_count,
            EXPECTED_DEFAULT_LAYOUT_GENERATED
        );
        assert_eq!(
            instance_report.stats.cube_count + instance_report.stats.sphere_count,
            primitive_stats.cube_count + primitive_stats.sphere_count
        );
        assert_eq!(
            merge_report.stats.material_profile_count,
            instance_report.stats.material_profile_count
        );
        assert_eq!(
            scale_report.standard.material_count,
            primitive_stats.unique_material_resource_count
        );
        assert_eq!(
            scale_report.static_instance.material_count,
            primitive_stats.unique_material_resource_count
        );
        assert_eq!(
            scale_report.cpu_merge.primitive_count,
            EXPECTED_DEFAULT_LAYOUT_GENERATED
        );
        assert_eq!(
            scale_report.static_instance.buffer_bytes,
            EXPECTED_DEFAULT_LAYOUT_GENERATED
                * crate::framework::fangyuan::FANGYUAN_STATIC_INSTANCE_RENDER_STRIDE_BYTES
        );
        assert!(
            scale_report.cpu_merge.batch_count <= scale_report.standard.entity_count,
            "CPU merge emits one mesh per merge group, not one entity per primitive"
        );
        assert!(
            scale_report.static_instance.batch_count <= scale_report.standard.entity_count,
            "static instance emits one buffer descriptor per kind/profile/transparency batch"
        );
        assert_eq!(
            scale_report.pressure.standard_pressure_units,
            EXPECTED_DEFAULT_LAYOUT_GENERATED
        );
        assert_eq!(
            scale_report.pressure.cpu_merge_pressure_units,
            scale_report.cpu_merge.batch_count
        );
        assert_eq!(
            scale_report.pressure.static_instance_pressure_units,
            scale_report.static_instance.batch_count
        );
        assert!(
            scale_report.pressure.standard_to_instance_reduction >= 1,
            "default home pressure trend must be stable even when the scene is small"
        );
        assert_eq!(
            scale_report.pressure.static_instance_buffer_kib,
            scale_report.static_instance.buffer_bytes.div_ceil(1024)
        );
    }

    #[test]
    fn reload_failure_clears_old_layout_content_but_keeps_base_space() {
        let mut app = app_with_fangyuan_home_system();
        let session_id = spawn_and_enter_fangyuan_home(&mut app, "fangyuan-reload-fails-session");
        let content_root = fangyuan_content_entity(&mut app, &session_id)
            .expect("content root should exist before failed reload");
        let cached_materials = app
            .world()
            .resource::<FangyuanHomeBlueprintRenderAssets>()
            .material_count();

        clear_blueprint_content_once(&mut app, &session_id);
        spawn_layout_from_loader_for_test(&mut app, content_root, &session_id, || {
            Err("test injected scene layout load failure".to_string())
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
            &expected_failed_layout_stats(&session_id, cached_materials, None)
        );
    }

    #[test]
    fn missing_prefab_layout_compile_failure_does_not_spawn_content() {
        let mut app = app_with_fangyuan_home_system();
        let session_id = SceneSessionId::from("fangyuan-missing-prefab-session");
        let parent = app.world_mut().spawn_empty().id();
        let layout = test_scene_layout(vec![FangyuanSceneLayoutInstance {
            id: Some("missing_prefab_instance".to_string()),
            name: None,
            prefab: "missing_prefab".to_string(),
            position: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
            tags: Vec::new(),
        }]);
        let palette = test_prefab_palette(vec![test_prefab(
            "available_prefab",
            vec![valid_cube_primitive()],
        )]);
        let audit_report = layout.audit_with_default_budget(&palette);

        spawn_layout_from_loader_for_test(&mut app, parent, &session_id, || {
            Ok(FangyuanHomeLayoutLoadResult::audit_failed(audit_report))
        });

        assert_eq!(fangyuan_blueprint_content_count(&mut app, &session_id), 0);
        assert_eq!(fangyuan_home_object_count(&mut app, &session_id), 0);
        assert_eq!(fangyuan_blueprint_primitive_count(&mut app, &session_id), 0);
        assert_eq!(
            app.world().resource::<FangyuanHomeBlueprintStats>(),
            &expected_failed_layout_stats(
                &session_id,
                0,
                Some(&layout.audit_with_default_budget(&palette))
            )
        );
        let stats = app.world().resource::<FangyuanHomeBlueprintStats>();
        assert_eq!(
            stats.audit_status_label(),
            FANGYUAN_HOME_AUDIT_STATUS_FAILED
        );
        assert_eq!(stats.audit_error_count, 1);
        assert_eq!(stats.audit_warning_count, 0);
        assert_eq!(stats.audit_primary_code(), "missing_prefab");
        assert_eq!(stats.audit_primary_field_path, "instances[0].prefab");
    }

    #[test]
    fn warning_audit_status_spawns_content_and_records_primary_code() {
        let mut app = app_with_fangyuan_home_system();
        let session_id = SceneSessionId::from("fangyuan-warning-audit-session");
        let parent = app.world_mut().spawn_empty().id();
        let layout = test_scene_layout(vec![FangyuanSceneLayoutInstance {
            id: Some("warning_instance".to_string()),
            name: None,
            prefab: "warning_prefab".to_string(),
            position: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
            tags: Vec::new(),
        }]);
        let palette = test_prefab_palette(vec![test_prefab(
            "warning_prefab",
            vec![
                valid_cube_primitive(),
                cube_primitive_at(19.8, [1.0, 1.0, 1.0], [0.25, 0.35, 0.45, 1.0]),
            ],
        )]);
        let mut layout = layout;
        layout.instances[0].position = [2.0, 0.0, 0.0];
        let audit_report = layout.audit_with_default_budget(&palette);
        let compile_report = layout.compile_with_palette(&palette).unwrap();

        spawn_layout_from_loader_for_test(&mut app, parent, &session_id, || {
            Ok(FangyuanHomeLayoutLoadResult::loaded(
                audit_report,
                compile_report,
            ))
        });

        assert_eq!(fangyuan_blueprint_content_count(&mut app, &session_id), 1);
        assert_eq!(fangyuan_home_object_count(&mut app, &session_id), 1);
        assert_eq!(fangyuan_blueprint_primitive_count(&mut app, &session_id), 1);
        let stats = app.world().resource::<FangyuanHomeBlueprintStats>();
        assert_eq!(
            stats.audit_status_label(),
            FANGYUAN_HOME_AUDIT_STATUS_WARNING
        );
        assert_eq!(stats.audit_error_count, 0);
        assert_eq!(stats.audit_warning_count, 1);
        assert_eq!(stats.audit_primary_code(), "invalid_primitive_position");
        assert_eq!(
            stats.audit_primary_field_path,
            "instances[0].prefab.primitives[1].position[0]"
        );
        assert_eq!(stats.state_label(), FANGYUAN_HOME_BLUEPRINT_STATE_LOADED);
    }

    #[test]
    fn failed_audit_status_does_not_spawn_misleading_success_stats() {
        let mut app = app_with_fangyuan_home_system();
        let session_id = SceneSessionId::from("fangyuan-failed-audit-session");
        let parent = app.world_mut().spawn_empty().id();
        let layout = test_scene_layout(Vec::new());
        let palette = test_prefab_palette(vec![test_prefab(
            "available_prefab",
            vec![valid_cube_primitive()],
        )]);
        let invalid_layout = FangyuanSceneLayout {
            version: "2".to_string(),
            ..layout
        };
        let audit_report = invalid_layout.audit_with_default_budget(&palette);

        spawn_layout_from_loader_for_test(&mut app, parent, &session_id, || {
            Ok(FangyuanHomeLayoutLoadResult::audit_failed(audit_report))
        });

        assert_eq!(fangyuan_blueprint_content_count(&mut app, &session_id), 0);
        assert_eq!(fangyuan_home_object_count(&mut app, &session_id), 0);
        assert_eq!(fangyuan_blueprint_primitive_count(&mut app, &session_id), 0);
        let stats = app.world().resource::<FangyuanHomeBlueprintStats>();
        assert_eq!(stats.state_label(), FANGYUAN_HOME_BLUEPRINT_STATE_FAILED);
        assert_eq!(
            stats.audit_status_label(),
            FANGYUAN_HOME_AUDIT_STATUS_FAILED
        );
        assert_eq!(stats.audit_error_count, 1);
        assert_eq!(stats.generated_primitives, 0);
        assert_eq!(stats.primitive_total(), 0);
        assert_eq!(stats.audit_primary_code(), "unsupported_version");
        assert_eq!(stats.audit_primary_field_path, "version");
    }

    #[test]
    fn invalid_or_malformed_simple_blueprint_sources_do_not_spawn_preview_content() {
        let mut app = app_with_fangyuan_home_system();
        let session_id = SceneSessionId::from("fangyuan-invalid-blueprint-session");
        let parent = app.world_mut().spawn_empty().id();
        let layout = FangyuanHomeLayout {
            default_blueprint_path: "fangyuan/test_invalid.ron".to_string(),
            ..FangyuanHomeLayout::default()
        };

        spawn_simple_blueprint_from_layout_for_test(&mut app, parent, &session_id, &layout, |_| {
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
        spawn_simple_blueprint_from_layout_for_test(&mut app, parent, &session_id, &layout, |_| {
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

    fn fangyuan_blueprint_merged_mesh_count(app: &mut App, session_id: &SceneSessionId) -> usize {
        let mut meshes = app
            .world_mut()
            .query::<&FangyuanHomeBlueprintMergedMeshVisual>();
        meshes
            .iter(app.world())
            .filter(|mesh| mesh.session_id == *session_id)
            .count()
    }

    fn fangyuan_blueprint_merged_mesh_records(
        app: &mut App,
        session_id: &SceneSessionId,
    ) -> Vec<FangyuanHomeBlueprintMergedMeshVisual> {
        let mut meshes = app
            .world_mut()
            .query::<&FangyuanHomeBlueprintMergedMeshVisual>();
        meshes
            .iter(app.world())
            .filter(|mesh| mesh.session_id == *session_id)
            .cloned()
            .collect()
    }

    fn fangyuan_blueprint_static_instance_count(
        app: &mut App,
        session_id: &SceneSessionId,
    ) -> usize {
        let mut instances = app
            .world_mut()
            .query::<&FangyuanHomeBlueprintStaticInstanceVisual>();
        instances
            .iter(app.world())
            .filter(|instance| instance.session_id == *session_id)
            .count()
    }

    fn fangyuan_blueprint_static_instance_records(
        app: &mut App,
        session_id: &SceneSessionId,
    ) -> Vec<FangyuanHomeBlueprintStaticInstanceVisual> {
        let mut instances = app
            .world_mut()
            .query::<&FangyuanHomeBlueprintStaticInstanceVisual>();
        instances
            .iter(app.world())
            .filter(|instance| instance.session_id == *session_id)
            .cloned()
            .collect()
    }

    fn fangyuan_home_trial_visual_count(app: &mut App, session_id: &SceneSessionId) -> usize {
        let mut visuals = app.world_mut().query::<&FangyuanHomeObjectTrialVisual>();
        visuals
            .iter(app.world())
            .filter(|visual| visual.session_id == *session_id)
            .count()
    }

    fn fangyuan_home_trial_visual_class_count(
        app: &mut App,
        session_id: &SceneSessionId,
        class: FangyuanObjectClass,
    ) -> usize {
        let mut visuals = app.world_mut().query::<&FangyuanHomeObjectTrialVisual>();
        visuals
            .iter(app.world())
            .filter(|visual| visual.session_id == *session_id && visual.class == class)
            .count()
    }

    fn fangyuan_home_trial_live_material_count(app: &App) -> usize {
        app.world()
            .resource::<FangyuanHomeObjectTrialRenderRuntime>()
            .live_material_count(app.world().resource::<Assets<StandardMaterial>>())
    }

    fn unique_static_instance_batch_count(
        records: &[FangyuanHomeBlueprintStaticInstanceVisual],
    ) -> usize {
        records
            .iter()
            .map(|record| record.batch_index)
            .collect::<std::collections::HashSet<_>>()
            .len()
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

    fn enable_cpu_merge_mode(app: &mut App) {
        app.world_mut()
            .resource_mut::<FangyuanHomeBlueprintRenderConfig>()
            .mode = FangyuanHomeBlueprintRenderMode::CpuMerge;
    }

    fn enable_static_instance_mode(app: &mut App) {
        app.world_mut()
            .resource_mut::<FangyuanHomeBlueprintRenderConfig>()
            .mode = FangyuanHomeBlueprintRenderMode::StaticInstance;
    }

    #[test]
    fn fangyuan_home_render_mode_env_parser_accepts_manual_validation_modes() {
        assert_eq!(
            parse_fangyuan_home_blueprint_render_mode("standard"),
            Some(FangyuanHomeBlueprintRenderMode::Standard)
        );
        assert_eq!(
            parse_fangyuan_home_blueprint_render_mode("cpu-merge"),
            Some(FangyuanHomeBlueprintRenderMode::CpuMerge)
        );
        assert_eq!(
            parse_fangyuan_home_blueprint_render_mode("instancing"),
            Some(FangyuanHomeBlueprintRenderMode::StaticInstance)
        );
        assert_eq!(parse_fangyuan_home_blueprint_render_mode(""), None);
    }

    fn set_cpu_merge_budget(app: &mut App, max_vertices: usize, max_indices: usize) {
        let mut config = app
            .world_mut()
            .resource_mut::<FangyuanHomeBlueprintRenderConfig>();
        config.mode = FangyuanHomeBlueprintRenderMode::CpuMerge;
        config.mesh_options.max_vertices_per_mesh = max_vertices;
        config.mesh_options.max_indices_per_mesh = max_indices;
    }

    fn set_static_instance_buffer_budget(app: &mut App, max_buffer_bytes: usize) {
        let mut config = app
            .world_mut()
            .resource_mut::<FangyuanHomeBlueprintRenderConfig>();
        config.mode = FangyuanHomeBlueprintRenderMode::StaticInstance;
        config.instance_options.max_buffer_bytes = max_buffer_bytes;
    }

    fn material_color(app: &App, material: &Handle<StandardMaterial>) -> Color {
        app.world()
            .resource::<Assets<StandardMaterial>>()
            .get(material)
            .expect("material should exist")
            .base_color
    }

    fn assert_color_nearly_eq(actual: Color, expected: Color) {
        let actual = actual.to_srgba();
        let expected = expected.to_srgba();
        assert_f32_nearly_eq(actual.red, expected.red);
        assert_f32_nearly_eq(actual.green, expected.green);
        assert_f32_nearly_eq(actual.blue, expected.blue);
        assert_f32_nearly_eq(actual.alpha, expected.alpha);
    }

    fn assert_f32_nearly_eq(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= 0.0001,
            "expected {actual} to be near {expected}"
        );
    }

    fn static_merge_runtime_asset_counts(app: &App) -> (usize, usize) {
        let runtime = app.world().resource::<FangyuanHomeStaticMergeRuntime>();
        let meshes = app.world().resource::<Assets<Mesh>>();
        let materials = app.world().resource::<Assets<StandardMaterial>>();
        (
            runtime
                .mesh_handles
                .iter()
                .filter(|handle| meshes.get(*handle).is_some())
                .count(),
            runtime
                .material_handles
                .iter()
                .filter(|handle| materials.get(*handle).is_some())
                .count(),
        )
    }

    fn clear_blueprint_content_once(app: &mut App, session_id: &SceneSessionId) -> usize {
        let mut state: SystemState<(
            Commands,
            Query<(Entity, &FangyuanHomeBlueprintContent)>,
            Query<(Entity, &FangyuanHomeObjectTrialVisual)>,
            ResMut<Assets<StandardMaterial>>,
            ResMut<FangyuanObjectTrialRuntime>,
            ResMut<FangyuanHomeObjectTrialRenderRuntime>,
            ResMut<FangyuanHomeBlueprintStats>,
        )> = SystemState::new(app.world_mut());
        let cleared = {
            let (
                mut commands,
                blueprint_content,
                trial_visuals,
                mut materials,
                mut trial_runtime,
                mut trial_render_runtime,
                mut blueprint_stats,
            ) = state.get_mut(app.world_mut());
            let cleared = clear_fangyuan_home_blueprint_content(
                &mut commands,
                session_id,
                blueprint_content.iter(),
            );
            clear_fangyuan_home_trial_runtime(
                &mut commands,
                session_id,
                trial_visuals.iter(),
                &mut trial_runtime,
                &mut trial_render_runtime,
                &mut materials,
                &mut blueprint_stats,
            );
            cleared
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
            Res<FangyuanHomeBlueprintRenderConfig>,
            ResMut<FangyuanHomeStaticMergeRuntime>,
            ResMut<FangyuanHomeStaticInstanceRuntime>,
        )> = SystemState::new(app.world_mut());
        let content = {
            let (
                mut commands,
                mut meshes,
                mut materials,
                mut blueprint_assets,
                render_config,
                mut static_merge_runtime,
                mut static_instance_runtime,
            ) = state.get_mut(app.world_mut());
            spawn_fangyuan_home_blueprint_content(
                &mut commands,
                parent,
                session_id,
                primitive_set,
                &mut meshes,
                &mut materials,
                &mut blueprint_assets,
                &render_config,
                &mut static_merge_runtime,
                &mut static_instance_runtime,
            )
        };
        state.apply(app.world_mut());
        app.update();
        content.entity
    }

    fn spawn_layout_from_loader_for_test(
        app: &mut App,
        parent: Entity,
        session_id: &SceneSessionId,
        load_scene_layout: impl FnOnce() -> Result<FangyuanHomeLayoutLoadResult, String>,
    ) -> Option<Entity> {
        let mut state: SystemState<(
            Commands,
            ResMut<Assets<Mesh>>,
            ResMut<Assets<StandardMaterial>>,
            ResMut<FangyuanHomeBlueprintRenderAssets>,
            Res<FangyuanHomeBlueprintRenderConfig>,
            ResMut<FangyuanHomeStaticMergeRuntime>,
            ResMut<FangyuanHomeStaticInstanceRuntime>,
            ResMut<FangyuanHomeBlueprintStats>,
        )> = SystemState::new(app.world_mut());
        let content = {
            let (
                mut commands,
                mut meshes,
                mut materials,
                mut blueprint_assets,
                render_config,
                mut static_merge_runtime,
                mut static_instance_runtime,
                mut blueprint_stats,
            ) = state.get_mut(app.world_mut());
            spawn_fangyuan_home_blueprint_from_layout_with_loader(
                &mut commands,
                parent,
                session_id,
                &mut meshes,
                &mut materials,
                &mut blueprint_assets,
                &render_config,
                &mut static_merge_runtime,
                &mut static_instance_runtime,
                &mut blueprint_stats,
                load_scene_layout,
            )
        };
        state.apply(app.world_mut());
        app.update();
        content.map(|content| content.entity)
    }

    fn spawn_simple_blueprint_from_layout_for_test(
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
            spawn_fangyuan_home_simple_blueprint_from_layout_with_loader(
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

    fn default_layout_compile_report() -> FangyuanSceneLayoutCompileReport {
        let layout = load_fangyuan_home_scene_layout().unwrap();
        let palette = load_fangyuan_home_prefab_palette().unwrap();
        layout.compile_with_palette(&palette).unwrap()
    }

    fn default_layout_audit_report() -> FangyuanAuditReport {
        let layout = load_fangyuan_home_scene_layout().unwrap();
        let palette = load_fangyuan_home_prefab_palette().unwrap();
        layout.audit_with_default_budget(&palette)
    }

    fn expected_loaded_layout_stats(
        session_id: &SceneSessionId,
        compile_report: &FangyuanSceneLayoutCompileReport,
    ) -> FangyuanHomeBlueprintStats {
        expected_loaded_layout_stats_with_render_summary(
            session_id,
            compile_report,
            FangyuanHomeBlueprintRenderSummary::default(),
        )
    }

    fn expected_loaded_layout_stats_with_render_summary(
        session_id: &SceneSessionId,
        compile_report: &FangyuanSceneLayoutCompileReport,
        render_summary: FangyuanHomeBlueprintRenderSummary,
    ) -> FangyuanHomeBlueprintStats {
        let mut stats = FangyuanHomeBlueprintStats::default();
        let audit_report = default_layout_audit_report();
        stats.record_layout_loaded(
            session_id,
            FANGYUAN_HOME_SCENE_LAYOUT_PATH,
            FANGYUAN_HOME_PREFAB_PALETTE_PATH,
            &audit_report,
            compile_report,
            render_summary.clone(),
        );
        record_expected_trial_summary(&mut stats);
        record_expected_lod_summary(&mut stats, compile_report, &render_summary);
        stats
    }

    fn expected_cleared_layout_stats(
        session_id: &SceneSessionId,
        compile_report: &FangyuanSceneLayoutCompileReport,
    ) -> FangyuanHomeBlueprintStats {
        let mut stats = expected_loaded_layout_stats(session_id, compile_report);
        clear_expected_trial_summary(&mut stats);
        stats.record_lod_summary(&FangyuanLodIntegrationSummary::default());
        stats.record_cleared(session_id);
        stats
    }

    fn expected_failed_layout_stats(
        session_id: &SceneSessionId,
        materials: usize,
        audit_report: Option<&FangyuanAuditReport>,
    ) -> FangyuanHomeBlueprintStats {
        let mut stats = FangyuanHomeBlueprintStats::default();
        stats.record_layout_failed(
            session_id,
            FANGYUAN_HOME_SCENE_LAYOUT_PATH,
            FANGYUAN_HOME_PREFAB_PALETTE_PATH,
            materials,
            audit_report,
        );
        stats
    }

    fn record_expected_lod_summary(
        stats: &mut FangyuanHomeBlueprintStats,
        compile_report: &FangyuanSceneLayoutCompileReport,
        render_summary: &FangyuanHomeBlueprintRenderSummary,
    ) {
        let render_path = match render_summary.mode.as_str() {
            "cpu_merge" => FangyuanLodRenderPath::StaticMerge,
            "static_instance" => FangyuanLodRenderPath::StaticInstancing,
            mode if mode.contains("->standard") || mode == "standard" => {
                FangyuanLodRenderPath::Standard
            }
            _ => FangyuanLodRenderPath::Hidden,
        };
        let mut descriptors =
            fangyuan_home_blueprint_lod_descriptors(&compile_report.primitive_set, render_path);
        let mut trial_runtime = FangyuanObjectTrialRuntime::default();
        trial_runtime
            .enter_default_showcase(0)
            .expect("default Fangyuan home trial should start");
        descriptors.extend(
            trial_runtime.visual_primitives().iter().map(|visual| {
                fangyuan_lod_descriptor_from_trial_visual("home_chunk_preview", visual)
            }),
        );

        let chunk_summary = FangyuanChunkDebugSummary {
            loaded_chunks: usize::from(!descriptors.is_empty()),
            loaded_chunk_ids: if descriptors.is_empty() {
                Vec::new()
            } else {
                vec!["home_chunk_preview".to_string()]
            },
            visible_objects: descriptors.len(),
            load_state: if descriptors.is_empty() {
                "pending".to_string()
            } else {
                "loaded".to_string()
            },
            failure_reason: "-".to_string(),
        };
        let metrics = hotspot_metrics_from_descriptors(&descriptors, 1);
        let hotspot = evaluate_fangyuan_hotspot(
            metrics,
            FangyuanHotspotThresholds::default(),
            FangyuanHotspotState::default(),
        );
        let summary = summarize_fangyuan_lod_integration_from_descriptors(
            [0.0, 0.0, 0.0],
            FangyuanAoiConfig::default(),
            &chunk_summary,
            &descriptors,
            &hotspot,
        );
        stats.record_lod_summary(&summary);
    }

    fn record_expected_trial_summary(stats: &mut FangyuanHomeBlueprintStats) {
        let mut trial_runtime = FangyuanObjectTrialRuntime::default();
        let summary = trial_runtime
            .enter_default_showcase(0)
            .expect("default Fangyuan home trial should start");
        stats.record_trial_summary(&summary);
    }

    fn clear_expected_trial_summary(stats: &mut FangyuanHomeBlueprintStats) {
        let mut trial_runtime = FangyuanObjectTrialRuntime::default();
        trial_runtime.clear_scene();
        stats.record_trial_summary(trial_runtime.summary());
    }

    fn assert_fangyuan_home_trial_active(app: &mut App, session_id: &SceneSessionId) {
        let stats = app.world().resource::<FangyuanHomeBlueprintStats>();
        assert_eq!(stats.trial_route_id, "fangyuan.object_trial");
        assert!(stats.trial_selection_label.contains("home:"));
        assert_eq!(stats.trial_budget_profile, "standard");
        assert!(stats.trial_audit_run > 0);
        assert_eq!(stats.trial_audit_status, "warning");
        assert_eq!(stats.trial_audit_error_count, 0);
        assert!(stats.trial_audit_warning_count > 0);
        assert!(stats.trial_audit_suggestion_count > 0);
        assert_eq!(stats.active_vfx_count, 4);
        assert_eq!(stats.trial_template_id, "skill.template.projectile");
        assert_eq!(stats.trial_visual_id, "skill.visual.projectile");
        assert_eq!(stats.trial_equipment_count, 1);
        assert_eq!(stats.trial_npc_count, 1);
        assert_eq!(stats.trial_tiandao_count, 1);
        assert!(stats.trial_budget_cost > 0);
        assert!(stats.trial_budget_recommended > 0);
        assert!(stats.trial_budget_hard >= stats.trial_budget_recommended);
        assert!(stats.trial_before_label.contains("objects cost"));
        assert!(stats.trial_after_label.contains("keep"));
        assert!(stats.trial_kept_count > 0);
        assert_eq!(stats.trial_degraded_count, 0);
        assert_eq!(stats.trial_rejected_count, 0);
        assert_eq!(stats.trial_fallback_missing_count, 0);
        assert_eq!(stats.trial_fallback_summary, "ok");
        assert!(stats.trial_plain_reason_summary.contains("技能颜色"));
        assert!(stats.trial_finding_summary.contains("skill_color_conflict"));

        let trial_runtime = app.world().resource::<FangyuanObjectTrialRuntime>();
        assert_eq!(trial_runtime.summary().route_id, stats.trial_route_id);
        assert_eq!(
            trial_runtime.summary().active_vfx_count,
            stats.active_vfx_count
        );
        assert_eq!(trial_runtime.summary().budget_cost, stats.trial_budget_cost);
        let expected_visual_count = trial_runtime.visual_primitives().len();
        assert!(expected_visual_count > 0);
        assert_eq!(
            fangyuan_home_trial_visual_count(app, session_id),
            expected_visual_count
        );
        assert_eq!(
            fangyuan_home_trial_live_material_count(app),
            expected_visual_count
        );
        assert!(
            fangyuan_home_trial_visual_class_count(app, session_id, FangyuanObjectClass::Vfx) > 0
        );
        assert!(
            fangyuan_home_trial_visual_class_count(app, session_id, FangyuanObjectClass::Equipment)
                > 0
        );
        assert!(
            fangyuan_home_trial_visual_class_count(app, session_id, FangyuanObjectClass::Npc) > 0
        );
        assert!(
            fangyuan_home_trial_visual_class_count(app, session_id, FangyuanObjectClass::Tiandao)
                > 0
        );
    }

    fn assert_fangyuan_home_trial_cleared(app: &mut App, session_id: &SceneSessionId) {
        let stats = app.world().resource::<FangyuanHomeBlueprintStats>();
        assert_eq!(stats.trial_route_id, "none");
        assert_eq!(stats.active_vfx_count, 0);
        assert_eq!(stats.trial_budget_cost, 0);
        assert_eq!(stats.trial_audit_run, 0);
        assert_eq!(stats.trial_fallback_summary, "ok");
        assert_eq!(stats.trial_finding_summary, "ok");

        let trial_runtime = app.world().resource::<FangyuanObjectTrialRuntime>();
        assert_eq!(trial_runtime.summary().route_id, "none");
        assert_eq!(trial_runtime.summary().active_vfx_count, 0);
        assert_eq!(trial_runtime.summary().budget_cost, 0);
        assert!(trial_runtime.visual_primitives().is_empty());
        assert_eq!(fangyuan_home_trial_visual_count(app, session_id), 0);
        assert_eq!(fangyuan_home_trial_live_material_count(app), 0);
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

    fn test_scene_layout(instances: Vec<FangyuanSceneLayoutInstance>) -> FangyuanSceneLayout {
        FangyuanSceneLayout {
            version: FANGYUAN_SCENE_LAYOUT_VERSION.to_string(),
            name: "test_home_layout".to_string(),
            description: String::new(),
            bounds: FangyuanBlueprintBounds::new(40.0, 40.0, 20.0),
            palette: Some(FANGYUAN_HOME_PREFAB_PALETTE_PATH.to_string()),
            palettes: Vec::new(),
            max_primitives: FANGYUAN_BLUEPRINT_HARD_PRIMITIVE_LIMIT,
            instances,
        }
    }

    fn test_prefab_palette(prefabs: Vec<FangyuanPrefabDefinition>) -> FangyuanPrefabPalette {
        FangyuanPrefabPalette {
            version: FANGYUAN_SCENE_LAYOUT_VERSION.to_string(),
            name: "test_home_prefabs".to_string(),
            description: String::new(),
            max_primitives: FANGYUAN_BLUEPRINT_HARD_PRIMITIVE_LIMIT,
            bounds: FangyuanBlueprintBounds::new(40.0, 40.0, 20.0),
            prefabs,
        }
    }

    fn test_prefab(
        id: &str,
        primitives: Vec<FangyuanPrimitiveBlueprint>,
    ) -> FangyuanPrefabDefinition {
        FangyuanPrefabDefinition {
            id: id.to_string(),
            name: id.to_string(),
            description: String::new(),
            bounds: None,
            pivot: None,
            tags: Vec::new(),
            max_primitives: None,
            primitives,
        }
    }

    fn unique_material_count(primitive_set: &FangyuanPrimitiveSet) -> usize {
        primitive_set
            .primitives()
            .iter()
            .map(|primitive| FangyuanRenderMaterialKey::from_color(primitive.color))
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
