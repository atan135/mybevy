use bevy::prelude::{Message, Resource};
use std::collections::{BTreeMap, VecDeque};

use super::{
    FangyuanAoiSelection, FangyuanAuditReport, FangyuanBakeArtifactStats,
    FangyuanBlueprintCacheManifest, FangyuanChunkDebugSummary, FangyuanHotspotEvaluation,
    FangyuanHotspotSeverity, FangyuanLodLevel, FangyuanObjectBudgetSummary,
    FangyuanPrimitiveSetStats, FangyuanRenderScaleReport,
};

pub const FANGYUAN_DEBUG_METRIC_DEFAULT_SAMPLE_INTERVAL_TICKS: u64 = 6;
pub const FANGYUAN_DEBUG_METRIC_DEFAULT_ROLLING_WINDOW_SAMPLES: usize = 30;
pub const FANGYUAN_DEBUG_METRIC_MIN_ROLLING_WINDOW_SAMPLES: usize = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FangyuanDebugMetricKey {
    Primitive,
    Instance,
    Batch,
    Mesh,
    BufferBytes,
    Lod,
    Aoi,
    Pressure,
    Cache,
    Bake,
    Audit,
}

impl FangyuanDebugMetricKey {
    pub const ALL: [Self; 11] = [
        Self::Primitive,
        Self::Instance,
        Self::Batch,
        Self::Mesh,
        Self::BufferBytes,
        Self::Lod,
        Self::Aoi,
        Self::Pressure,
        Self::Cache,
        Self::Bake,
        Self::Audit,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Primitive => "primitive",
            Self::Instance => "instance",
            Self::Batch => "batch",
            Self::Mesh => "mesh",
            Self::BufferBytes => "buffer_bytes",
            Self::Lod => "lod",
            Self::Aoi => "aoi",
            Self::Pressure => "pressure",
            Self::Cache => "cache",
            Self::Bake => "bake",
            Self::Audit => "audit",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FangyuanDebugMetricsSamplingConfig {
    pub sample_interval_ticks: u64,
    pub rolling_window_samples: usize,
}

impl FangyuanDebugMetricsSamplingConfig {
    pub const fn new(sample_interval_ticks: u64, rolling_window_samples: usize) -> Self {
        Self {
            sample_interval_ticks,
            rolling_window_samples,
        }
    }

    pub fn sanitized(self) -> Self {
        Self {
            sample_interval_ticks: self.sample_interval_ticks.max(1),
            rolling_window_samples: self
                .rolling_window_samples
                .max(FANGYUAN_DEBUG_METRIC_MIN_ROLLING_WINDOW_SAMPLES),
        }
    }

    pub fn should_sample(self, last_sample_tick: Option<u64>, current_tick: u64) -> bool {
        let config = self.sanitized();
        match last_sample_tick {
            None => true,
            Some(last_sample_tick) => {
                current_tick.saturating_sub(last_sample_tick) >= config.sample_interval_ticks
            }
        }
    }
}

impl Default for FangyuanDebugMetricsSamplingConfig {
    fn default() -> Self {
        Self {
            sample_interval_ticks: FANGYUAN_DEBUG_METRIC_DEFAULT_SAMPLE_INTERVAL_TICKS,
            rolling_window_samples: FANGYUAN_DEBUG_METRIC_DEFAULT_ROLLING_WINDOW_SAMPLES,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FangyuanDebugMetricStats {
    pub current: f64,
    pub peak: f64,
    pub average: f64,
    pub sample_count: usize,
}

impl FangyuanDebugMetricStats {
    fn from_samples(samples: &VecDeque<f64>) -> Self {
        let sample_count = samples.len();
        if sample_count == 0 {
            return Self::default();
        }

        let mut peak = 0.0;
        let mut total = 0.0;
        for value in samples {
            peak = f64::max(peak, *value);
            total += *value;
        }

        Self {
            current: samples.back().copied().unwrap_or_default(),
            peak,
            average: total / sample_count as f64,
            sample_count,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanDebugMetricRecord {
    pub key: FangyuanDebugMetricKey,
    pub name: &'static str,
    pub stats: FangyuanDebugMetricStats,
}

impl FangyuanDebugMetricRecord {
    fn from_samples(key: FangyuanDebugMetricKey, samples: &VecDeque<f64>) -> Self {
        Self {
            key,
            name: key.as_str(),
            stats: FangyuanDebugMetricStats::from_samples(samples),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanPrimitiveDebugMetrics {
    pub primitive_count: usize,
    pub cube_count: usize,
    pub sphere_count: usize,
    pub transparent_count: usize,
    pub material_count: usize,
}

impl FangyuanPrimitiveDebugMetrics {
    pub fn from_stats(stats: &FangyuanPrimitiveSetStats) -> Self {
        Self {
            primitive_count: stats.total,
            cube_count: stats.cube_count,
            sphere_count: stats.sphere_count,
            transparent_count: stats.transparent_count,
            material_count: stats.unique_material_resource_count,
        }
    }
}

impl Default for FangyuanPrimitiveDebugMetrics {
    fn default() -> Self {
        Self {
            primitive_count: 0,
            cube_count: 0,
            sphere_count: 0,
            transparent_count: 0,
            material_count: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanRenderDebugMetrics {
    pub render_mode: String,
    pub instance_count: usize,
    pub batch_count: usize,
    pub mesh_count: usize,
    pub buffer_bytes: usize,
    pub buffer_update_bytes: usize,
    pub draw_estimate: usize,
    pub material_profile_count: usize,
    pub pressure_units: usize,
    pub limiting_path: String,
}

impl FangyuanRenderDebugMetrics {
    pub fn from_scale_report(report: &FangyuanRenderScaleReport) -> Self {
        Self {
            render_mode: "static_instance".to_string(),
            instance_count: report.static_instance.instance_count,
            batch_count: report.static_instance.batch_count,
            mesh_count: report.static_instance.mesh_count,
            buffer_bytes: report.static_instance.buffer_bytes,
            buffer_update_bytes: report.static_instance.buffer_bytes,
            draw_estimate: report.static_instance.batch_count,
            material_profile_count: report.static_instance.material_profile_count,
            pressure_units: report.pressure.static_instance_pressure_units,
            limiting_path: report.pressure.limiting_path.to_string(),
        }
    }
}

impl Default for FangyuanRenderDebugMetrics {
    fn default() -> Self {
        Self {
            render_mode: "missing".to_string(),
            instance_count: 0,
            batch_count: 0,
            mesh_count: 0,
            buffer_bytes: 0,
            buffer_update_bytes: 0,
            draw_estimate: 0,
            material_profile_count: 0,
            pressure_units: 0,
            limiting_path: "-".to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanLodDebugMetrics {
    pub near_count: usize,
    pub mid_count: usize,
    pub far_count: usize,
    pub marker_count: usize,
    pub hidden_count: usize,
    pub dominant_lod: String,
}

impl FangyuanLodDebugMetrics {
    pub fn from_aoi_selection(selection: &FangyuanAoiSelection) -> Self {
        let mut metrics = Self::default();
        for decision in &selection.object_decisions {
            match decision.lod {
                FangyuanLodLevel::L0Full => metrics.near_count += 1,
                FangyuanLodLevel::L1Reduced => metrics.mid_count += 1,
                FangyuanLodLevel::L2Silhouette => metrics.far_count += 1,
                FangyuanLodLevel::L3Marker => metrics.marker_count += 1,
                FangyuanLodLevel::L4HiddenRuleOnly => metrics.hidden_count += 1,
            }
        }
        metrics.dominant_lod = dominant_lod_label(&metrics).to_string();
        metrics
    }

    pub fn visible_count(&self) -> usize {
        self.near_count + self.mid_count + self.far_count + self.marker_count
    }
}

impl Default for FangyuanLodDebugMetrics {
    fn default() -> Self {
        Self {
            near_count: 0,
            mid_count: 0,
            far_count: 0,
            marker_count: 0,
            hidden_count: 0,
            dominant_lod: "-".to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct FangyuanAoiDebugMetrics {
    pub load_chunks: usize,
    pub keep_chunks: usize,
    pub unload_chunks: usize,
    pub marker_chunks: usize,
    pub visible_objects: usize,
    pub radius: f32,
}

impl FangyuanAoiDebugMetrics {
    pub fn from_selection(selection: &FangyuanAoiSelection) -> Self {
        Self {
            load_chunks: selection.load_chunks.len(),
            keep_chunks: selection.keep_chunks.len(),
            unload_chunks: selection.unload_chunks.len(),
            marker_chunks: selection.marker_chunks.len(),
            visible_objects: selection.visible_object_ids().len(),
            radius: 0.0,
        }
    }

    pub fn from_chunk_summary(summary: &FangyuanChunkDebugSummary) -> Self {
        Self {
            keep_chunks: summary.loaded_chunks,
            visible_objects: summary.visible_objects,
            radius: 0.0,
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanPressureDebugMetrics {
    pub active: bool,
    pub severity: String,
    pub reason_count: usize,
    pub pressure_units: usize,
    pub degrade_reason: String,
}

impl FangyuanPressureDebugMetrics {
    pub fn from_hotspot_evaluation(evaluation: &FangyuanHotspotEvaluation) -> Self {
        Self {
            active: evaluation.active,
            severity: hotspot_severity_label(evaluation.severity).to_string(),
            reason_count: evaluation.pressure_reasons.len(),
            pressure_units: evaluation
                .pressure_reasons
                .iter()
                .map(|reason| reason.value as usize)
                .max()
                .unwrap_or_default(),
            degrade_reason: evaluation
                .degrade_plan
                .first()
                .map(|step| step.reason.clone())
                .unwrap_or_else(|| "-".to_string()),
        }
    }
}

impl Default for FangyuanPressureDebugMetrics {
    fn default() -> Self {
        Self {
            active: false,
            severity: hotspot_severity_label(FangyuanHotspotSeverity::Normal).to_string(),
            reason_count: 0,
            pressure_units: 0,
            degrade_reason: "-".to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct FangyuanCacheDebugMetrics {
    pub entry_count: usize,
    pub used_bytes: u64,
    pub max_bytes: u64,
    pub pressure_percent: u32,
    pub hit_count: usize,
    pub miss_count: usize,
}

impl FangyuanCacheDebugMetrics {
    pub fn from_manifest(manifest: &FangyuanBlueprintCacheManifest) -> Self {
        Self {
            entry_count: manifest.entries.len(),
            used_bytes: manifest.used_bytes,
            max_bytes: manifest.max_bytes,
            pressure_percent: percent(manifest.used_bytes, manifest.max_bytes),
            hit_count: 0,
            miss_count: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct FangyuanBakeDebugMetrics {
    pub artifact_count: usize,
    pub primitive_count: usize,
    pub artifact_bytes: usize,
    pub warning_count: usize,
}

impl FangyuanBakeDebugMetrics {
    pub fn from_artifact_stats(stats: impl IntoIterator<Item = FangyuanBakeArtifactStats>) -> Self {
        let mut metrics = Self::default();
        for stats in stats {
            metrics.artifact_count += 1;
            metrics.primitive_count += stats.primitive_count;
            metrics.artifact_bytes += stats.artifact_size;
            metrics.warning_count += stats.warning_count;
        }
        metrics
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanAuditDebugMetrics {
    pub status: String,
    pub error_count: usize,
    pub warning_count: usize,
    pub finding_count: usize,
}

impl FangyuanAuditDebugMetrics {
    pub fn from_report(report: &FangyuanAuditReport) -> Self {
        Self {
            status: audit_status_label(report.status).to_string(),
            error_count: report.summary.error_count,
            warning_count: report.summary.warning_count,
            finding_count: report.findings.len(),
        }
    }

    pub fn from_object_budget_summary(summary: &FangyuanObjectBudgetSummary) -> Self {
        Self {
            status: summary.audit_status.clone(),
            error_count: summary.audit_error_count,
            warning_count: summary.audit_warning_count,
            finding_count: summary.audit_error_count + summary.audit_warning_count,
        }
    }
}

impl Default for FangyuanAuditDebugMetrics {
    fn default() -> Self {
        Self {
            status: "passed".to_string(),
            error_count: 0,
            warning_count: 0,
            finding_count: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanTrialDebugMetrics {
    pub route_id: String,
    pub budget_profile: String,
    pub audit_status: String,
    pub active_vfx_count: usize,
    pub budget_cost: u32,
    pub budget_recommended: u32,
    pub budget_hard: u32,
    pub kept_count: usize,
    pub degraded_count: usize,
    pub rejected_count: usize,
    pub fallback_missing_count: usize,
    pub reason_summary: String,
}

impl Default for FangyuanTrialDebugMetrics {
    fn default() -> Self {
        Self {
            route_id: "none".to_string(),
            budget_profile: "standard".to_string(),
            audit_status: "pending".to_string(),
            active_vfx_count: 0,
            budget_cost: 0,
            budget_recommended: 0,
            budget_hard: 0,
            kept_count: 0,
            degraded_count: 0,
            rejected_count: 0,
            fallback_missing_count: 0,
            reason_summary: "ok".to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanDebugMetricsSnapshot {
    pub primitive: FangyuanPrimitiveDebugMetrics,
    pub render: FangyuanRenderDebugMetrics,
    pub lod: FangyuanLodDebugMetrics,
    pub aoi: FangyuanAoiDebugMetrics,
    pub pressure: FangyuanPressureDebugMetrics,
    pub cache: FangyuanCacheDebugMetrics,
    pub bake: FangyuanBakeDebugMetrics,
    pub audit: FangyuanAuditDebugMetrics,
    pub trial: FangyuanTrialDebugMetrics,
    pub module_status: BTreeMap<&'static str, FangyuanDebugModuleStatus>,
}

impl FangyuanDebugMetricsSnapshot {
    pub fn metric_value(&self, key: FangyuanDebugMetricKey) -> f64 {
        match key {
            FangyuanDebugMetricKey::Primitive => self.primitive.primitive_count as f64,
            FangyuanDebugMetricKey::Instance => self.render.instance_count as f64,
            FangyuanDebugMetricKey::Batch => self.render.batch_count as f64,
            FangyuanDebugMetricKey::Mesh => self.render.mesh_count as f64,
            FangyuanDebugMetricKey::BufferBytes => self.render.buffer_bytes as f64,
            FangyuanDebugMetricKey::Lod => self.lod.visible_count() as f64,
            FangyuanDebugMetricKey::Aoi => self.aoi.visible_objects as f64,
            FangyuanDebugMetricKey::Pressure => self.pressure.pressure_units as f64,
            FangyuanDebugMetricKey::Cache => self.cache.used_bytes as f64,
            FangyuanDebugMetricKey::Bake => self.bake.artifact_bytes as f64,
            FangyuanDebugMetricKey::Audit => self.audit.finding_count as f64,
        }
    }

    pub fn module_status(&self, module: &'static str) -> FangyuanDebugModuleStatus {
        self.module_status
            .get(module)
            .copied()
            .unwrap_or(FangyuanDebugModuleStatus::Missing)
    }
}

impl Default for FangyuanDebugMetricsSnapshot {
    fn default() -> Self {
        let mut module_status = BTreeMap::new();
        for module in FangyuanDebugMetricModule::ALL {
            module_status.insert(module.as_str(), FangyuanDebugModuleStatus::Missing);
        }

        Self {
            primitive: FangyuanPrimitiveDebugMetrics::default(),
            render: FangyuanRenderDebugMetrics::default(),
            lod: FangyuanLodDebugMetrics::default(),
            aoi: FangyuanAoiDebugMetrics::default(),
            pressure: FangyuanPressureDebugMetrics::default(),
            cache: FangyuanCacheDebugMetrics::default(),
            bake: FangyuanBakeDebugMetrics::default(),
            audit: FangyuanAuditDebugMetrics::default(),
            trial: FangyuanTrialDebugMetrics::default(),
            module_status,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanDebugModuleStatus {
    Missing,
    Present,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FangyuanDebugMetricModule {
    Primitive,
    Render,
    Lod,
    Aoi,
    Pressure,
    Cache,
    Bake,
    Audit,
    Trial,
}

impl FangyuanDebugMetricModule {
    pub const ALL: [Self; 9] = [
        Self::Primitive,
        Self::Render,
        Self::Lod,
        Self::Aoi,
        Self::Pressure,
        Self::Cache,
        Self::Bake,
        Self::Audit,
        Self::Trial,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Primitive => "primitive",
            Self::Render => "render",
            Self::Lod => "lod",
            Self::Aoi => "aoi",
            Self::Pressure => "pressure",
            Self::Cache => "cache",
            Self::Bake => "bake",
            Self::Audit => "audit",
            Self::Trial => "trial",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FangyuanDebugPanelModule {
    Render,
    Lod,
    Cache,
    Bake,
    Audit,
    Trial,
}

impl FangyuanDebugPanelModule {
    pub const ALL: [Self; 6] = [
        Self::Render,
        Self::Lod,
        Self::Cache,
        Self::Bake,
        Self::Audit,
        Self::Trial,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Render => "render",
            Self::Lod => "lod",
            Self::Cache => "cache",
            Self::Bake => "bake",
            Self::Audit => "audit",
            Self::Trial => "trial",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Render => "render",
            Self::Lod => "lod",
            Self::Cache => "cache",
            Self::Bake => "bake",
            Self::Audit => "audit",
            Self::Trial => "trial",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FangyuanDebugPanelToggles {
    pub render: bool,
    pub lod: bool,
    pub cache: bool,
    pub bake: bool,
    pub audit: bool,
    pub trial: bool,
}

impl FangyuanDebugPanelToggles {
    pub const fn all_enabled() -> Self {
        Self {
            render: true,
            lod: true,
            cache: true,
            bake: true,
            audit: true,
            trial: true,
        }
    }

    pub const fn is_enabled(self, module: FangyuanDebugPanelModule) -> bool {
        match module {
            FangyuanDebugPanelModule::Render => self.render,
            FangyuanDebugPanelModule::Lod => self.lod,
            FangyuanDebugPanelModule::Cache => self.cache,
            FangyuanDebugPanelModule::Bake => self.bake,
            FangyuanDebugPanelModule::Audit => self.audit,
            FangyuanDebugPanelModule::Trial => self.trial,
        }
    }

    pub fn set_enabled(&mut self, module: FangyuanDebugPanelModule, enabled: bool) {
        match module {
            FangyuanDebugPanelModule::Render => self.render = enabled,
            FangyuanDebugPanelModule::Lod => self.lod = enabled,
            FangyuanDebugPanelModule::Cache => self.cache = enabled,
            FangyuanDebugPanelModule::Bake => self.bake = enabled,
            FangyuanDebugPanelModule::Audit => self.audit = enabled,
            FangyuanDebugPanelModule::Trial => self.trial = enabled,
        }
    }

    pub fn toggle(&mut self, module: FangyuanDebugPanelModule) -> bool {
        let enabled = !self.is_enabled(module);
        self.set_enabled(module, enabled);
        enabled
    }

    pub fn enabled_modules_label(self) -> String {
        let labels = FangyuanDebugPanelModule::ALL
            .into_iter()
            .filter(|module| self.is_enabled(*module))
            .map(FangyuanDebugPanelModule::label)
            .collect::<Vec<_>>();

        if labels.is_empty() {
            "none".to_string()
        } else {
            labels.join(",")
        }
    }
}

impl Default for FangyuanDebugPanelToggles {
    fn default() -> Self {
        Self::all_enabled()
    }
}

#[derive(Clone, Debug, Resource, PartialEq, Eq)]
pub struct FangyuanDebugPanelState {
    pub visible: bool,
    pub compact: bool,
    pub toggles: FangyuanDebugPanelToggles,
}

impl FangyuanDebugPanelState {
    pub fn toggle_visible(&mut self) -> bool {
        self.visible = !self.visible;
        self.visible
    }

    pub fn set_compact(&mut self, compact: bool) {
        self.compact = compact;
    }

    pub fn toggle_module(&mut self, module: FangyuanDebugPanelModule) -> bool {
        self.toggles.toggle(module)
    }
}

impl Default for FangyuanDebugPanelState {
    fn default() -> Self {
        Self {
            visible: false,
            compact: false,
            toggles: FangyuanDebugPanelToggles::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanDebugPanelSnapshot {
    pub title: String,
    pub lines: Vec<String>,
    pub enabled_modules: String,
}

impl FangyuanDebugPanelSnapshot {
    pub fn text(&self) -> String {
        if self.lines.is_empty() {
            return format!("{}\nmodules {}", self.title, self.enabled_modules);
        }

        format!(
            "{}\nmodules {}\n{}",
            self.title,
            self.enabled_modules,
            self.lines.join("\n")
        )
    }
}

pub fn fangyuan_debug_panel_snapshot(
    metrics: &FangyuanDebugMetricsSnapshot,
    toggles: FangyuanDebugPanelToggles,
    compact: bool,
) -> FangyuanDebugPanelSnapshot {
    let mut lines = Vec::new();

    if toggles.render {
        lines.extend(fangyuan_debug_panel_render_lines(metrics, compact));
    }
    if toggles.lod {
        lines.extend(fangyuan_debug_panel_lod_lines(metrics, compact));
    }
    if toggles.cache {
        lines.extend(fangyuan_debug_panel_cache_lines(metrics, compact));
    }
    if toggles.bake {
        lines.extend(fangyuan_debug_panel_bake_lines(metrics, compact));
    }
    if toggles.audit {
        lines.extend(fangyuan_debug_panel_audit_lines(metrics, compact));
    }
    if toggles.trial {
        lines.extend(fangyuan_debug_panel_trial_lines(metrics, compact));
    }

    FangyuanDebugPanelSnapshot {
        title: if compact {
            "fangyuan debug compact".to_string()
        } else {
            "fangyuan debug panel".to_string()
        },
        lines,
        enabled_modules: toggles.enabled_modules_label(),
    }
}

fn fangyuan_debug_panel_render_lines(
    metrics: &FangyuanDebugMetricsSnapshot,
    compact: bool,
) -> Vec<String> {
    let status = metrics.module_status(FangyuanDebugMetricModule::Render.as_str());
    if status == FangyuanDebugModuleStatus::Missing {
        return vec!["render missing".to_string()];
    }

    let render = &metrics.render;
    if compact {
        return vec![format!(
            "render mode {} mesh {} inst_batch {} buf_upd {} draw {} matprof {}",
            compact_debug_panel_text(&render.render_mode, 20),
            render.mesh_count,
            render.batch_count,
            render.buffer_update_bytes,
            render.draw_estimate,
            render.material_profile_count
        )];
    }

    vec![
        format!(
            "render mode {} mesh {} instance_batch {}",
            render.render_mode, render.mesh_count, render.batch_count
        ),
        format!(
            "render buffer_update {} buffer_bytes {} draw_estimate {} material_profile {}",
            render.buffer_update_bytes,
            render.buffer_bytes,
            render.draw_estimate,
            render.material_profile_count
        ),
        format!(
            "render pressure {} limiting {}",
            render.pressure_units,
            compact_debug_panel_text(&render.limiting_path, 44)
        ),
    ]
}

fn fangyuan_debug_panel_lod_lines(
    metrics: &FangyuanDebugMetricsSnapshot,
    compact: bool,
) -> Vec<String> {
    let lod_status = metrics.module_status(FangyuanDebugMetricModule::Lod.as_str());
    let aoi_status = metrics.module_status(FangyuanDebugMetricModule::Aoi.as_str());
    let pressure_status = metrics.module_status(FangyuanDebugMetricModule::Pressure.as_str());
    if lod_status == FangyuanDebugModuleStatus::Missing
        && aoi_status == FangyuanDebugModuleStatus::Missing
        && pressure_status == FangyuanDebugModuleStatus::Missing
    {
        return vec!["lod missing".to_string()];
    }

    let lod = &metrics.lod;
    let aoi = &metrics.aoi;
    let pressure = &metrics.pressure;
    if compact {
        return vec![format!(
            "lod f{} r{} s{} m{} h{} chunks {} aoi {:.0} obj {} pressure {}",
            lod.near_count,
            lod.mid_count,
            lod.far_count,
            lod.marker_count,
            lod.hidden_count,
            aoi.keep_chunks + aoi.load_chunks,
            aoi.radius,
            aoi.visible_objects,
            compact_debug_panel_text(&pressure.severity, 16)
        )];
    }

    vec![
        format!(
            "lod distribution full {} reduced {} silhouette {} marker {} hidden {} dominant {}",
            lod.near_count,
            lod.mid_count,
            lod.far_count,
            lod.marker_count,
            lod.hidden_count,
            lod.dominant_lod
        ),
        format!(
            "lod loaded_chunks {} keep {} load {} unload {} markers {} visible {}",
            aoi.keep_chunks + aoi.load_chunks,
            aoi.keep_chunks,
            aoi.load_chunks,
            aoi.unload_chunks,
            aoi.marker_chunks,
            aoi.visible_objects
        ),
        format!(
            "lod aoi_radius {:.0} hotspot_pressure active {} severity {} units {} reasons {} degrade {}",
            aoi.radius,
            pressure.active,
            pressure.severity,
            pressure.pressure_units,
            pressure.reason_count,
            pressure_degrade_label(pressure)
        ),
    ]
}

fn fangyuan_debug_panel_cache_lines(
    metrics: &FangyuanDebugMetricsSnapshot,
    compact: bool,
) -> Vec<String> {
    if metrics.module_status(FangyuanDebugMetricModule::Cache.as_str())
        == FangyuanDebugModuleStatus::Missing
    {
        return vec!["cache missing hit/miss pending".to_string()];
    }

    let cache = &metrics.cache;
    if compact {
        return vec![format!(
            "cache entries {} hit {} miss {} pressure {}%",
            cache.entry_count, cache.hit_count, cache.miss_count, cache.pressure_percent
        )];
    }

    vec![format!(
        "cache entries {} hit {} miss {} bytes {}/{} pressure {}%",
        cache.entry_count,
        cache.hit_count,
        cache.miss_count,
        cache.used_bytes,
        cache.max_bytes,
        cache.pressure_percent
    )]
}

fn fangyuan_debug_panel_bake_lines(
    metrics: &FangyuanDebugMetricsSnapshot,
    compact: bool,
) -> Vec<String> {
    if metrics.module_status(FangyuanDebugMetricModule::Bake.as_str())
        == FangyuanDebugModuleStatus::Missing
    {
        return vec!["bake missing artifact none".to_string()];
    }

    let bake = &metrics.bake;
    if compact {
        return vec![format!(
            "bake artifacts {} bytes {} warn {}",
            bake.artifact_count, bake.artifact_bytes, bake.warning_count
        )];
    }

    vec![format!(
        "bake artifacts {} primitives {} bytes {} warnings {}",
        bake.artifact_count, bake.primitive_count, bake.artifact_bytes, bake.warning_count
    )]
}

fn fangyuan_debug_panel_audit_lines(
    metrics: &FangyuanDebugMetricsSnapshot,
    compact: bool,
) -> Vec<String> {
    if metrics.module_status(FangyuanDebugMetricModule::Audit.as_str())
        == FangyuanDebugModuleStatus::Missing
    {
        return vec!["audit missing".to_string()];
    }

    let audit = &metrics.audit;
    if compact {
        return vec![format!(
            "audit {} e{} w{} f{}",
            audit.status, audit.error_count, audit.warning_count, audit.finding_count
        )];
    }

    vec![format!(
        "audit status {} errors {} warnings {} findings {}",
        audit.status, audit.error_count, audit.warning_count, audit.finding_count
    )]
}

fn fangyuan_debug_panel_trial_lines(
    metrics: &FangyuanDebugMetricsSnapshot,
    compact: bool,
) -> Vec<String> {
    if metrics.module_status(FangyuanDebugMetricModule::Trial.as_str())
        == FangyuanDebugModuleStatus::Missing
    {
        return vec!["trial missing".to_string()];
    }

    let trial = &metrics.trial;
    if compact {
        return vec![format!(
            "trial {} profile {} vfx {} cost {}/{}",
            compact_debug_panel_text(&trial.route_id, 18),
            compact_debug_panel_text(&trial.budget_profile, 14),
            trial.active_vfx_count,
            trial.budget_cost,
            trial.budget_hard
        )];
    }

    vec![
        format!(
            "trial route {} profile {} audit {} active_vfx {}",
            trial.route_id, trial.budget_profile, trial.audit_status, trial.active_vfx_count
        ),
        format!(
            "trial budget {}/{}/{} kept {} degraded {} rejected {} fallback_missing {}",
            trial.budget_cost,
            trial.budget_recommended,
            trial.budget_hard,
            trial.kept_count,
            trial.degraded_count,
            trial.rejected_count,
            trial.fallback_missing_count
        ),
        format!(
            "trial reason {}",
            compact_debug_panel_text(&trial.reason_summary, 64)
        ),
    ]
}

fn pressure_degrade_label(pressure: &FangyuanPressureDebugMetrics) -> &str {
    if !pressure.degrade_reason.trim().is_empty() && pressure.degrade_reason != "-" {
        pressure.degrade_reason.as_str()
    } else if pressure.active {
        pressure.severity.as_str()
    } else {
        "-"
    }
}

fn compact_debug_panel_text(value: &str, max_chars: usize) -> String {
    let value = value.trim();
    if value.is_empty() {
        return "-".to_string();
    }
    let char_count = value.chars().count();
    if char_count <= max_chars {
        return value.to_string();
    }

    value
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>()
        + "..."
}

#[derive(Clone, Debug, PartialEq)]
pub enum FangyuanDebugMetricsInput {
    Primitive(FangyuanPrimitiveDebugMetrics),
    Render(FangyuanRenderDebugMetrics),
    Lod(FangyuanLodDebugMetrics),
    Aoi(FangyuanAoiDebugMetrics),
    Pressure(FangyuanPressureDebugMetrics),
    Cache(FangyuanCacheDebugMetrics),
    Bake(FangyuanBakeDebugMetrics),
    Audit(FangyuanAuditDebugMetrics),
    Trial(FangyuanTrialDebugMetrics),
}

impl FangyuanDebugMetricsInput {
    pub const fn module(&self) -> FangyuanDebugMetricModule {
        match self {
            Self::Primitive(_) => FangyuanDebugMetricModule::Primitive,
            Self::Render(_) => FangyuanDebugMetricModule::Render,
            Self::Lod(_) => FangyuanDebugMetricModule::Lod,
            Self::Aoi(_) => FangyuanDebugMetricModule::Aoi,
            Self::Pressure(_) => FangyuanDebugMetricModule::Pressure,
            Self::Cache(_) => FangyuanDebugMetricModule::Cache,
            Self::Bake(_) => FangyuanDebugMetricModule::Bake,
            Self::Audit(_) => FangyuanDebugMetricModule::Audit,
            Self::Trial(_) => FangyuanDebugMetricModule::Trial,
        }
    }
}

#[derive(Clone, Debug, Message, PartialEq)]
pub struct FangyuanDebugMetricsEvent {
    pub input: FangyuanDebugMetricsInput,
}

#[derive(Clone, Debug, Resource)]
pub struct FangyuanDebugMetricsBus {
    config: FangyuanDebugMetricsSamplingConfig,
    snapshot: FangyuanDebugMetricsSnapshot,
    records: BTreeMap<FangyuanDebugMetricKey, FangyuanDebugMetricRecord>,
    samples: BTreeMap<FangyuanDebugMetricKey, VecDeque<f64>>,
    last_sample_tick: Option<u64>,
    sample_generation: u64,
}

impl FangyuanDebugMetricsBus {
    pub fn new(config: FangyuanDebugMetricsSamplingConfig) -> Self {
        let config = config.sanitized();
        let snapshot = FangyuanDebugMetricsSnapshot::default();
        let mut bus = Self {
            config,
            snapshot,
            records: BTreeMap::new(),
            samples: BTreeMap::new(),
            last_sample_tick: None,
            sample_generation: 0,
        };
        bus.refresh_records_from_samples();
        bus
    }

    pub fn config(&self) -> FangyuanDebugMetricsSamplingConfig {
        self.config
    }

    pub fn snapshot(&self) -> &FangyuanDebugMetricsSnapshot {
        &self.snapshot
    }

    pub fn records(&self) -> &BTreeMap<FangyuanDebugMetricKey, FangyuanDebugMetricRecord> {
        &self.records
    }

    pub fn record(&self, key: FangyuanDebugMetricKey) -> &FangyuanDebugMetricRecord {
        self.records
            .get(&key)
            .expect("all stable fangyuan debug metric records should exist")
    }

    pub fn last_sample_tick(&self) -> Option<u64> {
        self.last_sample_tick
    }

    pub fn sample_generation(&self) -> u64 {
        self.sample_generation
    }

    pub fn submit(&mut self, input: FangyuanDebugMetricsInput) {
        let module = input.module();
        match input {
            FangyuanDebugMetricsInput::Primitive(metrics) => self.snapshot.primitive = metrics,
            FangyuanDebugMetricsInput::Render(metrics) => self.snapshot.render = metrics,
            FangyuanDebugMetricsInput::Lod(metrics) => self.snapshot.lod = metrics,
            FangyuanDebugMetricsInput::Aoi(metrics) => self.snapshot.aoi = metrics,
            FangyuanDebugMetricsInput::Pressure(metrics) => self.snapshot.pressure = metrics,
            FangyuanDebugMetricsInput::Cache(metrics) => self.snapshot.cache = metrics,
            FangyuanDebugMetricsInput::Bake(metrics) => self.snapshot.bake = metrics,
            FangyuanDebugMetricsInput::Audit(metrics) => self.snapshot.audit = metrics,
            FangyuanDebugMetricsInput::Trial(metrics) => self.snapshot.trial = metrics,
        }
        self.snapshot
            .module_status
            .insert(module.as_str(), FangyuanDebugModuleStatus::Present);
    }

    pub fn sample(&mut self, current_tick: u64) -> bool {
        if !self
            .config
            .should_sample(self.last_sample_tick, current_tick)
        {
            return false;
        }

        for key in FangyuanDebugMetricKey::ALL {
            let value = self.snapshot.metric_value(key);
            let samples = self.samples.entry(key).or_default();
            samples.push_back(value);
            while samples.len() > self.config.rolling_window_samples {
                samples.pop_front();
            }
        }
        self.last_sample_tick = Some(current_tick);
        self.sample_generation = self.sample_generation.saturating_add(1);
        self.refresh_records_from_samples();
        true
    }

    pub fn reset(&mut self) {
        self.snapshot = FangyuanDebugMetricsSnapshot::default();
        self.samples.clear();
        self.last_sample_tick = None;
        self.sample_generation = 0;
        self.refresh_records_from_samples();
    }

    fn refresh_records_from_samples(&mut self) {
        for key in FangyuanDebugMetricKey::ALL {
            let empty = VecDeque::new();
            let samples = self.samples.get(&key).unwrap_or(&empty);
            self.records
                .insert(key, FangyuanDebugMetricRecord::from_samples(key, samples));
        }
    }
}

impl Default for FangyuanDebugMetricsBus {
    fn default() -> Self {
        Self::new(FangyuanDebugMetricsSamplingConfig::default())
    }
}

fn dominant_lod_label(metrics: &FangyuanLodDebugMetrics) -> &'static str {
    [
        ("full", metrics.near_count),
        ("reduced", metrics.mid_count),
        ("silhouette", metrics.far_count),
        ("marker", metrics.marker_count),
        ("hidden_rule_only", metrics.hidden_count),
    ]
    .into_iter()
    .max_by_key(|(_, count)| *count)
    .filter(|(_, count)| *count > 0)
    .map(|(label, _)| label)
    .unwrap_or("-")
}

fn hotspot_severity_label(severity: FangyuanHotspotSeverity) -> &'static str {
    match severity {
        FangyuanHotspotSeverity::Normal => "normal",
        FangyuanHotspotSeverity::Warm => "warm",
        FangyuanHotspotSeverity::Hot => "hot",
        FangyuanHotspotSeverity::Critical => "critical",
    }
}

fn audit_status_label(status: super::FangyuanAuditStatus) -> &'static str {
    match status {
        super::FangyuanAuditStatus::Passed => "passed",
        super::FangyuanAuditStatus::PassedWithWarnings => "warning",
        super::FangyuanAuditStatus::Failed => "failed",
    }
}

fn percent(used: u64, max: u64) -> u32 {
    if max == 0 {
        return 0;
    }
    ((used.saturating_mul(100)) / max) as u32
}

#[cfg(test)]
mod tests {
    use bevy::prelude::*;

    use super::*;
    use crate::framework::fangyuan::{
        FangyuanAoiBand, FangyuanAoiObjectDecision, FangyuanAuditFinding, FangyuanAuditSeverity,
        FangyuanAuditSourceKind, FangyuanChunkDebugSummary, FangyuanLodObjectKind,
        FangyuanStaticInstanceRenderScaleStats,
    };

    #[test]
    fn fangyuan_debug_metrics_stable_metric_names_cover_required_keys() {
        let names = FangyuanDebugMetricKey::ALL
            .into_iter()
            .map(FangyuanDebugMetricKey::as_str)
            .collect::<Vec<_>>();

        assert_eq!(
            names,
            vec![
                "primitive",
                "instance",
                "batch",
                "mesh",
                "buffer_bytes",
                "lod",
                "aoi",
                "pressure",
                "cache",
                "bake",
                "audit",
            ]
        );

        let bus = FangyuanDebugMetricsBus::default();
        assert_eq!(bus.records().len(), FangyuanDebugMetricKey::ALL.len());
        for key in FangyuanDebugMetricKey::ALL {
            assert_eq!(bus.record(key).name, key.as_str());
        }
    }

    #[test]
    fn fangyuan_debug_metrics_missing_modules_have_stable_defaults() {
        let bus = FangyuanDebugMetricsBus::default();
        let snapshot = bus.snapshot();

        assert_eq!(snapshot.primitive.primitive_count, 0);
        assert_eq!(snapshot.render.limiting_path, "-");
        assert_eq!(snapshot.lod.dominant_lod, "-");
        assert_eq!(snapshot.pressure.severity, "normal");
        assert_eq!(snapshot.audit.status, "passed");
        for module in FangyuanDebugMetricModule::ALL {
            assert_eq!(
                snapshot.module_status(module.as_str()),
                FangyuanDebugModuleStatus::Missing
            );
        }
    }

    #[test]
    fn fangyuan_debug_metrics_aggregates_module_summaries_into_snapshot() {
        let mut bus = FangyuanDebugMetricsBus::new(FangyuanDebugMetricsSamplingConfig::new(1, 4));
        let primitive_stats = FangyuanPrimitiveSetStats {
            total: 12,
            cube_count: 8,
            sphere_count: 4,
            transparent_count: 3,
            unique_material_resource_count: 5,
            ..Default::default()
        };
        let render_report = FangyuanRenderScaleReport {
            static_instance: FangyuanStaticInstanceRenderScaleStats {
                instance_count: 12,
                mesh_count: 2,
                batch_count: 2,
                buffer_bytes: 384,
                ..Default::default()
            },
            pressure: super::super::FangyuanRenderScalePressureSummary {
                static_instance_pressure_units: 2,
                limiting_path: "static_instance_buffer_bytes",
                ..Default::default()
            },
            ..Default::default()
        };
        let selection = FangyuanAoiSelection {
            load_chunks: vec!["chunk_a".to_string()],
            keep_chunks: vec!["chunk_b".to_string()],
            marker_chunks: vec!["chunk_c".to_string()],
            object_decisions: vec![
                object_decision("near", FangyuanLodLevel::L0Full, true),
                object_decision("marker", FangyuanLodLevel::L3Marker, true),
                object_decision("hidden", FangyuanLodLevel::L4HiddenRuleOnly, false),
            ],
            ..Default::default()
        };

        bus.submit(FangyuanDebugMetricsInput::Primitive(
            FangyuanPrimitiveDebugMetrics::from_stats(&primitive_stats),
        ));
        bus.submit(FangyuanDebugMetricsInput::Render(
            FangyuanRenderDebugMetrics::from_scale_report(&render_report),
        ));
        bus.submit(FangyuanDebugMetricsInput::Lod(
            FangyuanLodDebugMetrics::from_aoi_selection(&selection),
        ));
        bus.submit(FangyuanDebugMetricsInput::Aoi(
            FangyuanAoiDebugMetrics::from_selection(&selection),
        ));
        assert!(bus.sample(10));

        let snapshot = bus.snapshot();
        assert_eq!(snapshot.primitive.primitive_count, 12);
        assert_eq!(snapshot.primitive.material_count, 5);
        assert_eq!(snapshot.render.instance_count, 12);
        assert_eq!(snapshot.render.batch_count, 2);
        assert_eq!(snapshot.render.buffer_bytes, 384);
        assert_eq!(
            snapshot.render.limiting_path,
            "static_instance_buffer_bytes"
        );
        assert_eq!(snapshot.lod.near_count, 1);
        assert_eq!(snapshot.lod.marker_count, 1);
        assert_eq!(snapshot.lod.hidden_count, 1);
        assert_eq!(snapshot.aoi.load_chunks, 1);
        assert_eq!(snapshot.aoi.visible_objects, 2);
        assert_eq!(
            bus.record(FangyuanDebugMetricKey::Primitive).stats.current,
            12.0
        );
        assert_eq!(
            snapshot.module_status("primitive"),
            FangyuanDebugModuleStatus::Present
        );
        assert_eq!(
            snapshot.module_status("cache"),
            FangyuanDebugModuleStatus::Missing
        );
    }

    #[test]
    fn fangyuan_debug_metrics_sampling_interval_window_peak_and_average_are_stable() {
        let mut bus = FangyuanDebugMetricsBus::new(FangyuanDebugMetricsSamplingConfig::new(2, 3));

        bus.submit(FangyuanDebugMetricsInput::Primitive(
            FangyuanPrimitiveDebugMetrics {
                primitive_count: 10,
                ..Default::default()
            },
        ));
        assert!(bus.sample(0));
        assert!(!bus.sample(1));

        bus.submit(FangyuanDebugMetricsInput::Primitive(
            FangyuanPrimitiveDebugMetrics {
                primitive_count: 30,
                ..Default::default()
            },
        ));
        assert!(bus.sample(2));

        bus.submit(FangyuanDebugMetricsInput::Primitive(
            FangyuanPrimitiveDebugMetrics {
                primitive_count: 20,
                ..Default::default()
            },
        ));
        assert!(bus.sample(4));

        bus.submit(FangyuanDebugMetricsInput::Primitive(
            FangyuanPrimitiveDebugMetrics {
                primitive_count: 40,
                ..Default::default()
            },
        ));
        assert!(bus.sample(6));

        let stats = bus.record(FangyuanDebugMetricKey::Primitive).stats;
        assert_eq!(stats.current, 40.0);
        assert_eq!(stats.peak, 40.0);
        assert_eq!(stats.sample_count, 3);
        assert_eq!(stats.average, 30.0);
        assert_eq!(bus.last_sample_tick(), Some(6));
        assert_eq!(bus.sample_generation(), 4);
    }

    #[test]
    fn fangyuan_debug_metrics_reset_clears_snapshot_samples_and_peaks() {
        let mut bus = FangyuanDebugMetricsBus::new(FangyuanDebugMetricsSamplingConfig::new(1, 4));
        bus.submit(FangyuanDebugMetricsInput::Primitive(
            FangyuanPrimitiveDebugMetrics {
                primitive_count: 42,
                ..Default::default()
            },
        ));
        bus.submit(FangyuanDebugMetricsInput::Aoi(
            FangyuanAoiDebugMetrics::from_chunk_summary(&FangyuanChunkDebugSummary {
                loaded_chunks: 2,
                loaded_chunk_ids: vec!["a".to_string(), "b".to_string()],
                visible_objects: 7,
                load_state: "loaded".to_string(),
                failure_reason: "-".to_string(),
            }),
        ));
        assert!(bus.sample(1));
        assert_eq!(
            bus.record(FangyuanDebugMetricKey::Primitive).stats.peak,
            42.0
        );

        bus.reset();

        assert_eq!(bus.snapshot().primitive.primitive_count, 0);
        assert_eq!(bus.snapshot().aoi.visible_objects, 0);
        assert_eq!(
            bus.record(FangyuanDebugMetricKey::Primitive).stats.peak,
            0.0
        );
        assert_eq!(
            bus.record(FangyuanDebugMetricKey::Primitive)
                .stats
                .sample_count,
            0
        );
        assert_eq!(bus.last_sample_tick(), None);
        assert_eq!(bus.sample_generation(), 0);
        assert_eq!(
            bus.snapshot().module_status("primitive"),
            FangyuanDebugModuleStatus::Missing
        );
    }

    #[test]
    fn fangyuan_debug_metrics_cache_bake_audit_inputs_keep_field_names_stable() {
        let mut cache_manifest = FangyuanBlueprintCacheManifest::new("fangyuan/cache", 1_000);
        cache_manifest.used_bytes = 250;
        let bake_stats = [
            FangyuanBakeArtifactStats {
                primitive_count: 3,
                artifact_size: 120,
                warning_count: 1,
                ..Default::default()
            },
            FangyuanBakeArtifactStats {
                primitive_count: 4,
                artifact_size: 80,
                warning_count: 2,
                ..Default::default()
            },
        ];
        let mut audit =
            FangyuanAuditReport::new(FangyuanAuditSourceKind::RuntimePrimitiveSet, None);
        audit.add_finding(FangyuanAuditFinding::new(
            FangyuanAuditSeverity::Warning,
            "test_warning",
            "warning",
            FangyuanAuditSourceKind::RuntimePrimitiveSet,
        ));

        let mut bus = FangyuanDebugMetricsBus::new(FangyuanDebugMetricsSamplingConfig::new(1, 4));
        bus.submit(FangyuanDebugMetricsInput::Cache(
            FangyuanCacheDebugMetrics::from_manifest(&cache_manifest),
        ));
        bus.submit(FangyuanDebugMetricsInput::Bake(
            FangyuanBakeDebugMetrics::from_artifact_stats(bake_stats),
        ));
        bus.submit(FangyuanDebugMetricsInput::Audit(
            FangyuanAuditDebugMetrics::from_report(&audit),
        ));
        assert!(bus.sample(0));

        let snapshot = bus.snapshot();
        assert_eq!(snapshot.cache.used_bytes, 250);
        assert_eq!(snapshot.cache.max_bytes, 1_000);
        assert_eq!(snapshot.cache.pressure_percent, 25);
        assert_eq!(snapshot.bake.artifact_count, 2);
        assert_eq!(snapshot.bake.primitive_count, 7);
        assert_eq!(snapshot.bake.artifact_bytes, 200);
        assert_eq!(snapshot.bake.warning_count, 3);
        assert_eq!(snapshot.audit.status, "warning");
        assert_eq!(snapshot.audit.warning_count, 1);
        assert_eq!(snapshot.audit.finding_count, 1);
        assert_eq!(bus.record(FangyuanDebugMetricKey::Cache).name, "cache");
        assert_eq!(bus.record(FangyuanDebugMetricKey::Bake).name, "bake");
        assert_eq!(bus.record(FangyuanDebugMetricKey::Audit).name, "audit");
    }

    #[test]
    fn fangyuan_debug_panel_formats_required_render_lod_cache_bake_fields() {
        let mut bus = FangyuanDebugMetricsBus::new(FangyuanDebugMetricsSamplingConfig::new(1, 4));
        bus.submit(FangyuanDebugMetricsInput::Render(
            FangyuanRenderDebugMetrics {
                render_mode: "static_instance".to_string(),
                instance_count: 21,
                batch_count: 4,
                mesh_count: 3,
                buffer_bytes: 2_048,
                buffer_update_bytes: 512,
                draw_estimate: 4,
                material_profile_count: 2,
                pressure_units: 4,
                limiting_path: "static_instance_buffer_bytes".to_string(),
            },
        ));
        bus.submit(FangyuanDebugMetricsInput::Lod(FangyuanLodDebugMetrics {
            near_count: 3,
            mid_count: 2,
            far_count: 1,
            marker_count: 1,
            hidden_count: 5,
            dominant_lod: "hidden_rule_only".to_string(),
        }));
        bus.submit(FangyuanDebugMetricsInput::Aoi(FangyuanAoiDebugMetrics {
            load_chunks: 1,
            keep_chunks: 2,
            unload_chunks: 1,
            marker_chunks: 1,
            visible_objects: 7,
            radius: 24.0,
        }));
        bus.submit(FangyuanDebugMetricsInput::Pressure(
            FangyuanPressureDebugMetrics {
                active: true,
                severity: "hot".to_string(),
                reason_count: 2,
                pressure_units: 8,
                degrade_reason: "transparent".to_string(),
            },
        ));
        bus.submit(FangyuanDebugMetricsInput::Cache(
            FangyuanCacheDebugMetrics {
                entry_count: 5,
                used_bytes: 250,
                max_bytes: 1_000,
                pressure_percent: 25,
                hit_count: 9,
                miss_count: 2,
            },
        ));
        bus.submit(FangyuanDebugMetricsInput::Bake(FangyuanBakeDebugMetrics {
            artifact_count: 2,
            primitive_count: 14,
            artifact_bytes: 4096,
            warning_count: 1,
        }));

        let panel = fangyuan_debug_panel_snapshot(
            bus.snapshot(),
            FangyuanDebugPanelToggles::default(),
            false,
        );
        let text = panel.text();

        assert!(text.contains("render mode static_instance mesh 3 instance_batch 4"));
        assert!(text.contains(
            "render buffer_update 512 buffer_bytes 2048 draw_estimate 4 material_profile 2"
        ));
        assert!(text.contains("lod distribution full 3 reduced 2 silhouette 1 marker 1 hidden 5"));
        assert!(text.contains("lod loaded_chunks 3 keep 2 load 1 unload 1 markers 1 visible 7"));
        assert!(text.contains(
            "lod aoi_radius 24 hotspot_pressure active true severity hot units 8 reasons 2 degrade transparent"
        ));
        assert!(text.contains("cache entries 5 hit 9 miss 2 bytes 250/1000 pressure 25%"));
        assert!(text.contains("bake artifacts 2 primitives 14 bytes 4096 warnings 1"));
        assert!(text.contains("audit missing"));
        assert!(text.contains("trial missing"));
    }

    #[test]
    fn fangyuan_debug_panel_toggles_hide_modules_and_track_enabled_labels() {
        let mut toggles = FangyuanDebugPanelToggles::default();

        assert!(!toggles.toggle(FangyuanDebugPanelModule::Cache));
        assert!(!toggles.is_enabled(FangyuanDebugPanelModule::Cache));
        assert_eq!(
            toggles.enabled_modules_label(),
            "render,lod,bake,audit,trial"
        );

        let panel =
            fangyuan_debug_panel_snapshot(&FangyuanDebugMetricsSnapshot::default(), toggles, false);
        let text = panel.text();

        assert!(text.contains("render missing"));
        assert!(text.contains("lod missing"));
        assert!(!text.contains("cache missing"));
        assert!(text.contains("bake missing artifact none"));
    }

    #[test]
    fn fangyuan_debug_panel_missing_modules_and_compact_mode_are_stable() {
        let mut bus = FangyuanDebugMetricsBus::default();
        bus.submit(FangyuanDebugMetricsInput::Render(
            FangyuanRenderDebugMetrics {
                render_mode: "static_instance_with_very_long_debug_name".to_string(),
                mesh_count: 8,
                batch_count: 6,
                buffer_update_bytes: 120,
                draw_estimate: 6,
                material_profile_count: 3,
                ..Default::default()
            },
        ));

        let panel = fangyuan_debug_panel_snapshot(
            bus.snapshot(),
            FangyuanDebugPanelToggles::default(),
            true,
        );
        let text = panel.text();

        assert!(text.starts_with("fangyuan debug compact"));
        assert!(text.contains("render mode static_instance_w... mesh 8"));
        assert!(text.contains("lod missing"));
        assert!(text.contains("cache missing hit/miss pending"));
        assert!(text.contains("bake missing artifact none"));
    }

    #[test]
    fn fangyuan_debug_metrics_can_be_registered_as_bevy_resource_and_message() {
        let mut app = App::new();
        app.init_resource::<FangyuanDebugMetricsBus>()
            .add_message::<FangyuanDebugMetricsEvent>();

        app.world_mut().write_message(FangyuanDebugMetricsEvent {
            input: FangyuanDebugMetricsInput::Primitive(FangyuanPrimitiveDebugMetrics {
                primitive_count: 5,
                ..Default::default()
            }),
        });

        assert_eq!(
            app.world()
                .resource::<FangyuanDebugMetricsBus>()
                .record(FangyuanDebugMetricKey::Primitive)
                .name,
            "primitive"
        );
    }

    fn object_decision(
        id: &str,
        lod: FangyuanLodLevel,
        visible: bool,
    ) -> FangyuanAoiObjectDecision {
        FangyuanAoiObjectDecision {
            object_id: id.to_string(),
            chunk_id: "chunk".to_string(),
            kind: FangyuanLodObjectKind::StaticObject,
            band: FangyuanAoiBand::Near,
            lod,
            visible,
            marker: lod == FangyuanLodLevel::L3Marker,
            rule_layer_retained: false,
        }
    }
}
