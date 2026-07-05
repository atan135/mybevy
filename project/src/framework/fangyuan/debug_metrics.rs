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
    pub instance_count: usize,
    pub batch_count: usize,
    pub mesh_count: usize,
    pub buffer_bytes: usize,
    pub pressure_units: usize,
    pub limiting_path: String,
}

impl FangyuanRenderDebugMetrics {
    pub fn from_scale_report(report: &FangyuanRenderScaleReport) -> Self {
        Self {
            instance_count: report.static_instance.instance_count,
            batch_count: report.static_instance.batch_count,
            mesh_count: report.static_instance.mesh_count,
            buffer_bytes: report.static_instance.buffer_bytes,
            pressure_units: report.pressure.static_instance_pressure_units,
            limiting_path: report.pressure.limiting_path.to_string(),
        }
    }
}

impl Default for FangyuanRenderDebugMetrics {
    fn default() -> Self {
        Self {
            instance_count: 0,
            batch_count: 0,
            mesh_count: 0,
            buffer_bytes: 0,
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

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct FangyuanAoiDebugMetrics {
    pub load_chunks: usize,
    pub keep_chunks: usize,
    pub unload_chunks: usize,
    pub marker_chunks: usize,
    pub visible_objects: usize,
}

impl FangyuanAoiDebugMetrics {
    pub fn from_selection(selection: &FangyuanAoiSelection) -> Self {
        Self {
            load_chunks: selection.load_chunks.len(),
            keep_chunks: selection.keep_chunks.len(),
            unload_chunks: selection.unload_chunks.len(),
            marker_chunks: selection.marker_chunks.len(),
            visible_objects: selection.visible_object_ids().len(),
        }
    }

    pub fn from_chunk_summary(summary: &FangyuanChunkDebugSummary) -> Self {
        Self {
            keep_chunks: summary.loaded_chunks,
            visible_objects: summary.visible_objects,
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
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct FangyuanCacheDebugMetrics {
    pub entry_count: usize,
    pub used_bytes: u64,
    pub max_bytes: u64,
    pub pressure_percent: u32,
}

impl FangyuanCacheDebugMetrics {
    pub fn from_manifest(manifest: &FangyuanBlueprintCacheManifest) -> Self {
        Self {
            entry_count: manifest.entries.len(),
            used_bytes: manifest.used_bytes,
            max_bytes: manifest.max_bytes,
            pressure_percent: percent(manifest.used_bytes, manifest.max_bytes),
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
pub struct FangyuanDebugMetricsSnapshot {
    pub primitive: FangyuanPrimitiveDebugMetrics,
    pub render: FangyuanRenderDebugMetrics,
    pub lod: FangyuanLodDebugMetrics,
    pub aoi: FangyuanAoiDebugMetrics,
    pub pressure: FangyuanPressureDebugMetrics,
    pub cache: FangyuanCacheDebugMetrics,
    pub bake: FangyuanBakeDebugMetrics,
    pub audit: FangyuanAuditDebugMetrics,
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
}

impl FangyuanDebugMetricModule {
    pub const ALL: [Self; 8] = [
        Self::Primitive,
        Self::Render,
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
            Self::Render => "render",
            Self::Lod => "lod",
            Self::Aoi => "aoi",
            Self::Pressure => "pressure",
            Self::Cache => "cache",
            Self::Bake => "bake",
            Self::Audit => "audit",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FangyuanDebugMetricsInput {
    Primitive(FangyuanPrimitiveDebugMetrics),
    Render(FangyuanRenderDebugMetrics),
    Lod(FangyuanLodDebugMetrics),
    Aoi(FangyuanAoiDebugMetrics),
    Pressure(FangyuanPressureDebugMetrics),
    Cache(FangyuanCacheDebugMetrics),
    Bake(FangyuanBakeDebugMetrics),
    Audit(FangyuanAuditDebugMetrics),
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
        }
    }
}

#[derive(Clone, Debug, Message, PartialEq, Eq)]
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
