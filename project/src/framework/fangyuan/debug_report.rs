use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use super::{
    FANGYUAN_PRESSURE_SUPPORTED_ACTOR_COUNTS, FangyuanAoiDebugMetrics, FangyuanAuditDebugMetrics,
    FangyuanAuditReport, FangyuanBakeDebugMetrics, FangyuanCacheDebugMetrics,
    FangyuanDebugMetricsSnapshot, FangyuanLodDebugMetrics, FangyuanObjectBudgetSummary,
    FangyuanPressureDebugMetrics, FangyuanPressureMetricStats, FangyuanPressureReport,
    FangyuanPressureTestConfig, FangyuanRenderDebugMetrics, FangyuanSkillDegradeLevel,
    FangyuanTrialDebugMetrics, FangyuanVisualReplayConsistencyReport,
    FangyuanVisualReplayMismatchField, FangyuanVisualReplayMismatchSummary,
};

pub const FANGYUAN_DEBUG_REPORT_SCHEMA_VERSION: u32 = 1;
pub const FANGYUAN_PRESSURE_BASELINE_SCHEMA_VERSION: u32 = 1;
pub const FANGYUAN_DEBUG_REPORT_OUTPUT_DIR: &str = "artifacts/fangyuan-debug";
pub const FANGYUAN_DEBUG_REPORT_JSON_PATTERN: &str = "artifacts/fangyuan-debug/{report_id}.json";
pub const FANGYUAN_PRESSURE_BASELINE_JSON_PATH: &str =
    "artifacts/fangyuan-debug/pressure-baseline.json";
pub const FANGYUAN_DEBUG_REPORT_LARGE_ARTIFACT_POLICY: &str =
    "write local run outputs under artifacts/fangyuan-debug; artifacts/ is ignored";

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanDebugReportOutputPolicy {
    pub default_output_dir: String,
    pub report_json_pattern: String,
    pub baseline_json_path: String,
    pub large_artifact_policy: String,
}

impl Default for FangyuanDebugReportOutputPolicy {
    fn default() -> Self {
        Self {
            default_output_dir: FANGYUAN_DEBUG_REPORT_OUTPUT_DIR.to_string(),
            report_json_pattern: FANGYUAN_DEBUG_REPORT_JSON_PATTERN.to_string(),
            baseline_json_path: FANGYUAN_PRESSURE_BASELINE_JSON_PATH.to_string(),
            large_artifact_policy: FANGYUAN_DEBUG_REPORT_LARGE_ARTIFACT_POLICY.to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanDebugReport {
    pub schema_version: u32,
    pub report_kind: String,
    pub report_id: String,
    pub output_policy: FangyuanDebugReportOutputPolicy,
    pub audit: FangyuanDebugReportAuditSummary,
    pub budget: FangyuanDebugReportBudgetSummary,
    pub render: FangyuanDebugReportRenderSummary,
    pub lod: FangyuanDebugReportLodSummary,
    pub aoi: FangyuanDebugReportAoiSummary,
    pub cache: FangyuanDebugReportCacheSummary,
    pub bake: FangyuanDebugReportBakeSummary,
    pub pressure: FangyuanDebugReportPressureSummary,
    pub replay: FangyuanDebugReportReplaySummary,
}

impl FangyuanDebugReport {
    pub fn new(report_id: impl Into<String>) -> Self {
        Self {
            report_id: report_id.into(),
            ..Default::default()
        }
    }

    pub fn from_metrics_snapshot(
        report_id: impl Into<String>,
        snapshot: &FangyuanDebugMetricsSnapshot,
    ) -> Self {
        Self {
            report_id: report_id.into(),
            audit: FangyuanDebugReportAuditSummary::from(&snapshot.audit),
            budget: FangyuanDebugReportBudgetSummary::from(&snapshot.trial),
            render: FangyuanDebugReportRenderSummary::from(&snapshot.render),
            lod: FangyuanDebugReportLodSummary::from(&snapshot.lod),
            aoi: FangyuanDebugReportAoiSummary::from(&snapshot.aoi),
            cache: FangyuanDebugReportCacheSummary::from(&snapshot.cache),
            bake: FangyuanDebugReportBakeSummary::from(&snapshot.bake),
            pressure: FangyuanDebugReportPressureSummary::from_debug_metrics(&snapshot.pressure),
            ..Default::default()
        }
    }

    pub fn with_audit_report(mut self, report: &FangyuanAuditReport) -> Self {
        self.audit = FangyuanDebugReportAuditSummary::from(report);
        self
    }

    pub fn with_object_budget_summary(mut self, summary: &FangyuanObjectBudgetSummary) -> Self {
        self.budget = FangyuanDebugReportBudgetSummary::from(summary);
        self
    }

    pub fn with_pressure_report(mut self, report: &FangyuanPressureReport) -> Self {
        self.pressure = FangyuanDebugReportPressureSummary::from(report);
        self
    }

    pub fn with_visual_replay_report(
        mut self,
        report: &FangyuanVisualReplayConsistencyReport,
    ) -> Self {
        self.replay = FangyuanDebugReportReplaySummary::from(report);
        self
    }
}

impl Default for FangyuanDebugReport {
    fn default() -> Self {
        Self {
            schema_version: FANGYUAN_DEBUG_REPORT_SCHEMA_VERSION,
            report_kind: "fangyuan_debug_report".to_string(),
            report_id: "unspecified".to_string(),
            output_policy: FangyuanDebugReportOutputPolicy::default(),
            audit: FangyuanDebugReportAuditSummary::default(),
            budget: FangyuanDebugReportBudgetSummary::default(),
            render: FangyuanDebugReportRenderSummary::default(),
            lod: FangyuanDebugReportLodSummary::default(),
            aoi: FangyuanDebugReportAoiSummary::default(),
            cache: FangyuanDebugReportCacheSummary::default(),
            bake: FangyuanDebugReportBakeSummary::default(),
            pressure: FangyuanDebugReportPressureSummary::default(),
            replay: FangyuanDebugReportReplaySummary::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanDebugReportAuditSummary {
    pub status: String,
    pub error_count: usize,
    pub warning_count: usize,
    pub finding_count: usize,
}

impl Default for FangyuanDebugReportAuditSummary {
    fn default() -> Self {
        Self {
            status: "passed".to_string(),
            error_count: 0,
            warning_count: 0,
            finding_count: 0,
        }
    }
}

impl From<&FangyuanAuditDebugMetrics> for FangyuanDebugReportAuditSummary {
    fn from(metrics: &FangyuanAuditDebugMetrics) -> Self {
        Self {
            status: metrics.status.clone(),
            error_count: metrics.error_count,
            warning_count: metrics.warning_count,
            finding_count: metrics.finding_count,
        }
    }
}

impl From<&FangyuanAuditReport> for FangyuanDebugReportAuditSummary {
    fn from(report: &FangyuanAuditReport) -> Self {
        Self {
            status: format!("{:?}", report.status).to_ascii_lowercase(),
            error_count: report.summary.error_count,
            warning_count: report.summary.warning_count,
            finding_count: report.findings.len(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanDebugReportBudgetSummary {
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

impl Default for FangyuanDebugReportBudgetSummary {
    fn default() -> Self {
        Self::from(&FangyuanTrialDebugMetrics::default())
    }
}

impl From<&FangyuanTrialDebugMetrics> for FangyuanDebugReportBudgetSummary {
    fn from(metrics: &FangyuanTrialDebugMetrics) -> Self {
        Self {
            route_id: metrics.route_id.clone(),
            budget_profile: metrics.budget_profile.clone(),
            audit_status: metrics.audit_status.clone(),
            active_vfx_count: metrics.active_vfx_count,
            budget_cost: metrics.budget_cost,
            budget_recommended: metrics.budget_recommended,
            budget_hard: metrics.budget_hard,
            kept_count: metrics.kept_count,
            degraded_count: metrics.degraded_count,
            rejected_count: metrics.rejected_count,
            fallback_missing_count: metrics.fallback_missing_count,
            reason_summary: metrics.reason_summary.clone(),
        }
    }
}

impl From<&FangyuanObjectBudgetSummary> for FangyuanDebugReportBudgetSummary {
    fn from(summary: &FangyuanObjectBudgetSummary) -> Self {
        Self {
            route_id: "fangyuan.object_budget".to_string(),
            budget_profile: "unknown".to_string(),
            audit_status: summary.audit_status.clone(),
            active_vfx_count: summary.active_vfx_count,
            budget_cost: summary.total_cost,
            budget_recommended: 0,
            budget_hard: 0,
            kept_count: summary.total_count,
            degraded_count: 0,
            rejected_count: 0,
            fallback_missing_count: 0,
            reason_summary: summary.finding_summary.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanDebugReportRenderSummary {
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

impl Default for FangyuanDebugReportRenderSummary {
    fn default() -> Self {
        Self::from(&FangyuanRenderDebugMetrics::default())
    }
}

impl From<&FangyuanRenderDebugMetrics> for FangyuanDebugReportRenderSummary {
    fn from(metrics: &FangyuanRenderDebugMetrics) -> Self {
        Self {
            render_mode: metrics.render_mode.clone(),
            instance_count: metrics.instance_count,
            batch_count: metrics.batch_count,
            mesh_count: metrics.mesh_count,
            buffer_bytes: metrics.buffer_bytes,
            buffer_update_bytes: metrics.buffer_update_bytes,
            draw_estimate: metrics.draw_estimate,
            material_profile_count: metrics.material_profile_count,
            pressure_units: metrics.pressure_units,
            limiting_path: metrics.limiting_path.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanDebugReportLodSummary {
    pub near_count: usize,
    pub mid_count: usize,
    pub far_count: usize,
    pub marker_count: usize,
    pub hidden_count: usize,
    pub visible_count: usize,
    pub dominant_lod: String,
}

impl Default for FangyuanDebugReportLodSummary {
    fn default() -> Self {
        Self::from(&FangyuanLodDebugMetrics::default())
    }
}

impl From<&FangyuanLodDebugMetrics> for FangyuanDebugReportLodSummary {
    fn from(metrics: &FangyuanLodDebugMetrics) -> Self {
        Self {
            near_count: metrics.near_count,
            mid_count: metrics.mid_count,
            far_count: metrics.far_count,
            marker_count: metrics.marker_count,
            hidden_count: metrics.hidden_count,
            visible_count: metrics.visible_count(),
            dominant_lod: metrics.dominant_lod.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanDebugReportAoiSummary {
    pub load_chunks: usize,
    pub keep_chunks: usize,
    pub unload_chunks: usize,
    pub marker_chunks: usize,
    pub visible_objects: usize,
    pub radius: f32,
}

impl Default for FangyuanDebugReportAoiSummary {
    fn default() -> Self {
        Self::from(&FangyuanAoiDebugMetrics::default())
    }
}

impl From<&FangyuanAoiDebugMetrics> for FangyuanDebugReportAoiSummary {
    fn from(metrics: &FangyuanAoiDebugMetrics) -> Self {
        Self {
            load_chunks: metrics.load_chunks,
            keep_chunks: metrics.keep_chunks,
            unload_chunks: metrics.unload_chunks,
            marker_chunks: metrics.marker_chunks,
            visible_objects: metrics.visible_objects,
            radius: metrics.radius,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanDebugReportCacheSummary {
    pub entry_count: usize,
    pub used_bytes: u64,
    pub max_bytes: u64,
    pub pressure_percent: u32,
    pub hit_count: usize,
    pub miss_count: usize,
}

impl From<&FangyuanCacheDebugMetrics> for FangyuanDebugReportCacheSummary {
    fn from(metrics: &FangyuanCacheDebugMetrics) -> Self {
        Self {
            entry_count: metrics.entry_count,
            used_bytes: metrics.used_bytes,
            max_bytes: metrics.max_bytes,
            pressure_percent: metrics.pressure_percent,
            hit_count: metrics.hit_count,
            miss_count: metrics.miss_count,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanDebugReportBakeSummary {
    pub artifact_count: usize,
    pub primitive_count: usize,
    pub artifact_bytes: usize,
    pub warning_count: usize,
}

impl From<&FangyuanBakeDebugMetrics> for FangyuanDebugReportBakeSummary {
    fn from(metrics: &FangyuanBakeDebugMetrics) -> Self {
        Self {
            artifact_count: metrics.artifact_count,
            primitive_count: metrics.primitive_count,
            artifact_bytes: metrics.artifact_bytes,
            warning_count: metrics.warning_count,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanDebugReportPressureSummary {
    pub included: bool,
    pub active: bool,
    pub severity: String,
    pub reason_count: usize,
    pub pressure_units: usize,
    pub degrade_reason: String,
    pub actor_count: usize,
    pub skill_template_id: String,
    pub skill_visual_id: String,
    pub seed: u64,
    pub trigger_interval_ticks: u64,
    pub scene_width: f32,
    pub scene_depth: f32,
    pub chunk_count: u32,
    pub budget_profile: String,
    pub duration_ticks: u64,
    pub ticks_per_second: u32,
    pub sample_count: usize,
    pub total_trigger_events: usize,
    pub curve: FangyuanDebugReportPressureCurve,
    pub degrade: FangyuanDebugReportPressureDegrade,
    pub deterministic_hash: u64,
    pub summary_text: String,
}

impl FangyuanDebugReportPressureSummary {
    pub fn from_debug_metrics(metrics: &FangyuanPressureDebugMetrics) -> Self {
        Self {
            active: metrics.active,
            severity: metrics.severity.clone(),
            reason_count: metrics.reason_count,
            pressure_units: metrics.pressure_units,
            degrade_reason: metrics.degrade_reason.clone(),
            ..Default::default()
        }
    }
}

impl Default for FangyuanDebugReportPressureSummary {
    fn default() -> Self {
        Self {
            included: false,
            active: false,
            severity: "normal".to_string(),
            reason_count: 0,
            pressure_units: 0,
            degrade_reason: "-".to_string(),
            actor_count: 0,
            skill_template_id: "-".to_string(),
            skill_visual_id: "-".to_string(),
            seed: 0,
            trigger_interval_ticks: 0,
            scene_width: 0.0,
            scene_depth: 0.0,
            chunk_count: 0,
            budget_profile: "standard".to_string(),
            duration_ticks: 0,
            ticks_per_second: 0,
            sample_count: 0,
            total_trigger_events: 0,
            curve: FangyuanDebugReportPressureCurve::default(),
            degrade: FangyuanDebugReportPressureDegrade::default(),
            deterministic_hash: 0,
            summary_text: "none".to_string(),
        }
    }
}

impl From<&FangyuanPressureReport> for FangyuanDebugReportPressureSummary {
    fn from(report: &FangyuanPressureReport) -> Self {
        Self {
            included: true,
            active: report.curve.pressure.peak > 0,
            severity: degrade_level_label(report.degrade.worst_level).to_string(),
            reason_count: usize::from(
                report.degrade.worst_level != FangyuanSkillDegradeLevel::None,
            ),
            pressure_units: report.curve.pressure.peak,
            degrade_reason: report.degrade.reason.clone(),
            actor_count: report.config.actor_count,
            skill_template_id: report.config.skill_template_id.clone(),
            skill_visual_id: report.skill_visual_id.clone(),
            seed: report.config.seed,
            trigger_interval_ticks: report.config.trigger_interval_ticks,
            scene_width: report.config.scene_size.width,
            scene_depth: report.config.scene_size.depth,
            chunk_count: report.config.chunk_count,
            budget_profile: report.config.budget_profile.as_str().to_string(),
            duration_ticks: report.config.duration_ticks,
            ticks_per_second: report.config.ticks_per_second,
            sample_count: report.sample_count,
            total_trigger_events: report.total_trigger_events,
            curve: FangyuanDebugReportPressureCurve::from(&report.curve),
            degrade: FangyuanDebugReportPressureDegrade::from_report(report),
            deterministic_hash: report.deterministic_hash,
            summary_text: report.summary_text.clone(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanDebugReportPressureCurve {
    pub active_vfx: FangyuanDebugReportMetricStats,
    pub dynamic_primitive: FangyuanDebugReportMetricStats,
    pub trail: FangyuanDebugReportMetricStats,
    pub transparent: FangyuanDebugReportMetricStats,
    pub emissive: FangyuanDebugReportMetricStats,
    pub pressure: FangyuanDebugReportMetricStats,
}

impl From<&super::FangyuanPressureCurveSummary> for FangyuanDebugReportPressureCurve {
    fn from(curve: &super::FangyuanPressureCurveSummary) -> Self {
        Self {
            active_vfx: FangyuanDebugReportMetricStats::from(curve.active_vfx),
            dynamic_primitive: FangyuanDebugReportMetricStats::from(curve.dynamic_primitive),
            trail: FangyuanDebugReportMetricStats::from(curve.trail),
            transparent: FangyuanDebugReportMetricStats::from(curve.transparent),
            emissive: FangyuanDebugReportMetricStats::from(curve.emissive),
            pressure: FangyuanDebugReportMetricStats::from(curve.pressure),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanDebugReportMetricStats {
    pub peak: usize,
    pub average: f64,
}

impl From<FangyuanPressureMetricStats> for FangyuanDebugReportMetricStats {
    fn from(stats: FangyuanPressureMetricStats) -> Self {
        Self {
            peak: stats.peak,
            average: stats.average,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanDebugReportPressureDegrade {
    pub none_ticks: usize,
    pub low_ticks: usize,
    pub medium_ticks: usize,
    pub high_ticks: usize,
    pub critical_ticks: usize,
    pub worst_level: String,
    pub reason: String,
}

impl FangyuanDebugReportPressureDegrade {
    fn from_report(report: &FangyuanPressureReport) -> Self {
        Self {
            none_ticks: report.degrade.none_ticks,
            low_ticks: report.degrade.low_ticks,
            medium_ticks: report.degrade.medium_ticks,
            high_ticks: report.degrade.high_ticks,
            critical_ticks: report.degrade.critical_ticks,
            worst_level: degrade_level_label(report.degrade.worst_level).to_string(),
            reason: report.degrade.reason.clone(),
        }
    }
}

impl Default for FangyuanDebugReportPressureDegrade {
    fn default() -> Self {
        Self {
            none_ticks: 0,
            low_ticks: 0,
            medium_ticks: 0,
            high_ticks: 0,
            critical_ticks: 0,
            worst_level: "none".to_string(),
            reason: "ok".to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanDebugReportReplaySummary {
    pub included: bool,
    pub replay_id: String,
    pub start_tick: u64,
    pub event_count: usize,
    pub sample_count: usize,
    pub visual_hash: u64,
    pub mismatch: Option<FangyuanDebugReportReplayMismatch>,
    pub summary_text: String,
}

impl Default for FangyuanDebugReportReplaySummary {
    fn default() -> Self {
        Self {
            included: false,
            replay_id: "-".to_string(),
            start_tick: 0,
            event_count: 0,
            sample_count: 0,
            visual_hash: 0,
            mismatch: None,
            summary_text: "none".to_string(),
        }
    }
}

impl From<&FangyuanVisualReplayConsistencyReport> for FangyuanDebugReportReplaySummary {
    fn from(report: &FangyuanVisualReplayConsistencyReport) -> Self {
        Self {
            included: true,
            replay_id: report.replay_id.clone(),
            start_tick: report.start_tick,
            event_count: report.event_count,
            sample_count: report.samples.len(),
            visual_hash: report.visual_hash,
            mismatch: report
                .mismatch_summary
                .as_ref()
                .map(FangyuanDebugReportReplayMismatch::from),
            summary_text: report.summary_line(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanDebugReportReplayMismatch {
    pub replay_id: String,
    pub tick: u64,
    pub frame_id: u32,
    pub event_id: String,
    pub recipe_id: String,
    pub object_id: String,
    pub expected_hash: u64,
    pub actual_hash: u64,
    pub field: String,
    pub summary_text: String,
}

impl From<&FangyuanVisualReplayMismatchSummary> for FangyuanDebugReportReplayMismatch {
    fn from(mismatch: &FangyuanVisualReplayMismatchSummary) -> Self {
        Self {
            replay_id: mismatch.replay_id.clone(),
            tick: mismatch.tick,
            frame_id: mismatch.frame_id,
            event_id: mismatch.event_id.clone(),
            recipe_id: mismatch.recipe_id.clone(),
            object_id: mismatch.object_id.clone(),
            expected_hash: mismatch.expected_hash,
            actual_hash: mismatch.actual_hash,
            field: replay_mismatch_field_label(mismatch.field).to_string(),
            summary_text: mismatch.summary_line(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanPressureBaselineSnapshot {
    pub schema_version: u32,
    pub snapshot_kind: String,
    pub scenario_set: String,
    pub supported_actor_counts: Vec<usize>,
    pub output_policy: FangyuanDebugReportOutputPolicy,
    pub entries: Vec<FangyuanPressureBaselineEntry>,
    pub last_comparison: Vec<FangyuanPressureBaselineComparison>,
}

impl FangyuanPressureBaselineSnapshot {
    pub fn new(entries: Vec<FangyuanPressureBaselineEntry>) -> Self {
        let mut snapshot = Self {
            entries,
            ..Default::default()
        };
        snapshot.sort_entries();
        snapshot
    }

    pub fn from_pressure_reports<'a>(
        reports: impl IntoIterator<Item = &'a FangyuanPressureReport>,
    ) -> Self {
        Self::new(
            reports
                .into_iter()
                .map(FangyuanPressureBaselineEntry::from)
                .collect(),
        )
    }

    pub fn entry_for_actor_count(
        &self,
        actor_count: usize,
    ) -> Option<&FangyuanPressureBaselineEntry> {
        self.entries
            .iter()
            .find(|entry| entry.actor_count == actor_count)
    }

    pub fn missing_supported_actor_counts(&self) -> Vec<usize> {
        self.supported_actor_counts
            .iter()
            .copied()
            .filter(|actor_count| self.entry_for_actor_count(*actor_count).is_none())
            .collect()
    }

    pub fn compare_to(&self, current: &Self) -> FangyuanPressureBaselineComparisonReport {
        let mut actor_counts = BTreeSet::<usize>::new();
        actor_counts.extend(self.supported_actor_counts.iter().copied());
        actor_counts.extend(current.supported_actor_counts.iter().copied());
        actor_counts.extend(self.entries.iter().map(|entry| entry.actor_count));
        actor_counts.extend(current.entries.iter().map(|entry| entry.actor_count));

        let comparisons = actor_counts
            .into_iter()
            .map(|actor_count| {
                FangyuanPressureBaselineComparison::compare(
                    actor_count,
                    self.entry_for_actor_count(actor_count),
                    current.entry_for_actor_count(actor_count),
                )
            })
            .collect::<Vec<_>>();

        FangyuanPressureBaselineComparisonReport {
            schema_version: FANGYUAN_PRESSURE_BASELINE_SCHEMA_VERSION,
            comparison_kind: "fangyuan_pressure_baseline_comparison".to_string(),
            scenario_set: current.scenario_set.clone(),
            compared_actor_counts: comparisons
                .iter()
                .map(|comparison| comparison.actor_count)
                .collect(),
            has_regression: comparisons.iter().any(|comparison| {
                comparison.result == FangyuanPressureBaselineComparisonResult::Regressed
            }),
            comparisons,
        }
    }

    pub fn with_last_comparison(
        mut self,
        comparison: FangyuanPressureBaselineComparisonReport,
    ) -> Self {
        self.last_comparison = comparison.comparisons;
        self
    }

    fn sort_entries(&mut self) {
        self.entries
            .sort_by(|left, right| left.actor_count.cmp(&right.actor_count));
    }
}

impl Default for FangyuanPressureBaselineSnapshot {
    fn default() -> Self {
        Self {
            schema_version: FANGYUAN_PRESSURE_BASELINE_SCHEMA_VERSION,
            snapshot_kind: "fangyuan_pressure_baseline".to_string(),
            scenario_set: "pressure_100_300_1000".to_string(),
            supported_actor_counts: FANGYUAN_PRESSURE_SUPPORTED_ACTOR_COUNTS.to_vec(),
            output_policy: FangyuanDebugReportOutputPolicy::default(),
            entries: Vec::new(),
            last_comparison: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanPressureBaselineEntry {
    pub actor_count: usize,
    pub config: FangyuanPressureBaselineConfig,
    pub skill_visual_id: String,
    pub sample_count: usize,
    pub total_trigger_events: usize,
    pub active_vfx: FangyuanDebugReportMetricStats,
    pub dynamic_primitive: FangyuanDebugReportMetricStats,
    pub trail: FangyuanDebugReportMetricStats,
    pub transparent: FangyuanDebugReportMetricStats,
    pub emissive: FangyuanDebugReportMetricStats,
    pub pressure: FangyuanDebugReportMetricStats,
    pub worst_degrade_level: String,
    pub degrade_reason: String,
    pub deterministic_hash: u64,
    pub summary_text: String,
}

impl From<&FangyuanPressureReport> for FangyuanPressureBaselineEntry {
    fn from(report: &FangyuanPressureReport) -> Self {
        Self {
            actor_count: report.config.actor_count,
            config: FangyuanPressureBaselineConfig::from(&report.config),
            skill_visual_id: report.skill_visual_id.clone(),
            sample_count: report.sample_count,
            total_trigger_events: report.total_trigger_events,
            active_vfx: FangyuanDebugReportMetricStats::from(report.curve.active_vfx),
            dynamic_primitive: FangyuanDebugReportMetricStats::from(report.curve.dynamic_primitive),
            trail: FangyuanDebugReportMetricStats::from(report.curve.trail),
            transparent: FangyuanDebugReportMetricStats::from(report.curve.transparent),
            emissive: FangyuanDebugReportMetricStats::from(report.curve.emissive),
            pressure: FangyuanDebugReportMetricStats::from(report.curve.pressure),
            worst_degrade_level: degrade_level_label(report.degrade.worst_level).to_string(),
            degrade_reason: report.degrade.reason.clone(),
            deterministic_hash: report.deterministic_hash,
            summary_text: report.summary_text.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanPressureBaselineConfig {
    pub actor_count: usize,
    pub skill_template_id: String,
    pub trigger_interval_ticks: u64,
    pub seed: u64,
    pub scene_width: f32,
    pub scene_depth: f32,
    pub chunk_count: u32,
    pub budget_profile: String,
    pub duration_ticks: u64,
    pub ticks_per_second: u32,
    pub config_hash: u64,
}

impl From<&FangyuanPressureTestConfig> for FangyuanPressureBaselineConfig {
    fn from(config: &FangyuanPressureTestConfig) -> Self {
        Self {
            actor_count: config.actor_count,
            skill_template_id: config.skill_template_id.clone(),
            trigger_interval_ticks: config.trigger_interval_ticks,
            seed: config.seed,
            scene_width: config.scene_size.width,
            scene_depth: config.scene_size.depth,
            chunk_count: config.chunk_count,
            budget_profile: config.budget_profile.as_str().to_string(),
            duration_ticks: config.duration_ticks,
            ticks_per_second: config.ticks_per_second,
            config_hash: fangyuan_pressure_config_hash(config),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanPressureBaselineComparisonReport {
    pub schema_version: u32,
    pub comparison_kind: String,
    pub scenario_set: String,
    pub compared_actor_counts: Vec<usize>,
    pub has_regression: bool,
    pub comparisons: Vec<FangyuanPressureBaselineComparison>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanPressureBaselineComparison {
    pub actor_count: usize,
    pub result: FangyuanPressureBaselineComparisonResult,
    pub baseline_hash: Option<u64>,
    pub current_hash: Option<u64>,
    pub baseline_config_hash: Option<u64>,
    pub current_config_hash: Option<u64>,
    pub active_vfx_peak_delta: i64,
    pub active_vfx_average_delta: f64,
    pub dynamic_primitive_peak_delta: i64,
    pub dynamic_primitive_average_delta: f64,
    pub pressure_peak_delta: i64,
    pub pressure_average_delta: f64,
    pub summary: String,
}

impl FangyuanPressureBaselineComparison {
    fn compare(
        actor_count: usize,
        baseline: Option<&FangyuanPressureBaselineEntry>,
        current: Option<&FangyuanPressureBaselineEntry>,
    ) -> Self {
        match (baseline, current) {
            (None, None) => Self::missing_both(actor_count),
            (None, Some(current)) => Self::missing_baseline(actor_count, current),
            (Some(baseline), None) => Self::missing_current(actor_count, baseline),
            (Some(baseline), Some(current)) => Self::from_entries(actor_count, baseline, current),
        }
    }

    fn from_entries(
        actor_count: usize,
        baseline: &FangyuanPressureBaselineEntry,
        current: &FangyuanPressureBaselineEntry,
    ) -> Self {
        let active_vfx_peak_delta =
            current.active_vfx.peak as i64 - baseline.active_vfx.peak as i64;
        let dynamic_primitive_peak_delta =
            current.dynamic_primitive.peak as i64 - baseline.dynamic_primitive.peak as i64;
        let pressure_peak_delta = current.pressure.peak as i64 - baseline.pressure.peak as i64;
        let active_vfx_average_delta = current.active_vfx.average - baseline.active_vfx.average;
        let dynamic_primitive_average_delta =
            current.dynamic_primitive.average - baseline.dynamic_primitive.average;
        let pressure_average_delta = current.pressure.average - baseline.pressure.average;

        let result = if baseline.config.config_hash == current.config.config_hash
            && baseline.deterministic_hash == current.deterministic_hash
            && active_vfx_peak_delta == 0
            && dynamic_primitive_peak_delta == 0
            && pressure_peak_delta == 0
            && f64_near_zero(active_vfx_average_delta)
            && f64_near_zero(dynamic_primitive_average_delta)
            && f64_near_zero(pressure_average_delta)
        {
            FangyuanPressureBaselineComparisonResult::Matched
        } else if pressure_peak_delta > 0
            || pressure_average_delta > 0.0
            || active_vfx_peak_delta > 0
            || dynamic_primitive_peak_delta > 0
        {
            FangyuanPressureBaselineComparisonResult::Regressed
        } else if pressure_peak_delta < 0
            || pressure_average_delta < 0.0
            || active_vfx_peak_delta < 0
            || dynamic_primitive_peak_delta < 0
        {
            FangyuanPressureBaselineComparisonResult::Improved
        } else {
            FangyuanPressureBaselineComparisonResult::Changed
        };

        let summary = format!(
            "actors={} result={} baseline_hash={} current_hash={} pressure_peak_delta={} pressure_average_delta={:.2}",
            actor_count,
            result.as_str(),
            baseline.deterministic_hash,
            current.deterministic_hash,
            pressure_peak_delta,
            pressure_average_delta,
        );

        Self {
            actor_count,
            result,
            baseline_hash: Some(baseline.deterministic_hash),
            current_hash: Some(current.deterministic_hash),
            baseline_config_hash: Some(baseline.config.config_hash),
            current_config_hash: Some(current.config.config_hash),
            active_vfx_peak_delta,
            active_vfx_average_delta,
            dynamic_primitive_peak_delta,
            dynamic_primitive_average_delta,
            pressure_peak_delta,
            pressure_average_delta,
            summary,
        }
    }

    fn missing_both(actor_count: usize) -> Self {
        Self {
            actor_count,
            result: FangyuanPressureBaselineComparisonResult::MissingBaselineAndCurrent,
            baseline_hash: None,
            current_hash: None,
            baseline_config_hash: None,
            current_config_hash: None,
            active_vfx_peak_delta: 0,
            active_vfx_average_delta: 0.0,
            dynamic_primitive_peak_delta: 0,
            dynamic_primitive_average_delta: 0.0,
            pressure_peak_delta: 0,
            pressure_average_delta: 0.0,
            summary: format!("actors={actor_count} result=missing_baseline_and_current"),
        }
    }

    fn missing_baseline(actor_count: usize, current: &FangyuanPressureBaselineEntry) -> Self {
        Self {
            actor_count,
            result: FangyuanPressureBaselineComparisonResult::MissingBaseline,
            baseline_hash: None,
            current_hash: Some(current.deterministic_hash),
            baseline_config_hash: None,
            current_config_hash: Some(current.config.config_hash),
            active_vfx_peak_delta: 0,
            active_vfx_average_delta: 0.0,
            dynamic_primitive_peak_delta: 0,
            dynamic_primitive_average_delta: 0.0,
            pressure_peak_delta: 0,
            pressure_average_delta: 0.0,
            summary: format!(
                "actors={} result=missing_baseline current_hash={}",
                actor_count, current.deterministic_hash
            ),
        }
    }

    fn missing_current(actor_count: usize, baseline: &FangyuanPressureBaselineEntry) -> Self {
        Self {
            actor_count,
            result: FangyuanPressureBaselineComparisonResult::MissingCurrent,
            baseline_hash: Some(baseline.deterministic_hash),
            current_hash: None,
            baseline_config_hash: Some(baseline.config.config_hash),
            current_config_hash: None,
            active_vfx_peak_delta: 0,
            active_vfx_average_delta: 0.0,
            dynamic_primitive_peak_delta: 0,
            dynamic_primitive_average_delta: 0.0,
            pressure_peak_delta: 0,
            pressure_average_delta: 0.0,
            summary: format!(
                "actors={} result=missing_current baseline_hash={}",
                actor_count, baseline.deterministic_hash
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanPressureBaselineComparisonResult {
    Matched,
    Changed,
    Improved,
    Regressed,
    MissingBaseline,
    MissingCurrent,
    MissingBaselineAndCurrent,
}

impl FangyuanPressureBaselineComparisonResult {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Matched => "matched",
            Self::Changed => "changed",
            Self::Improved => "improved",
            Self::Regressed => "regressed",
            Self::MissingBaseline => "missing_baseline",
            Self::MissingCurrent => "missing_current",
            Self::MissingBaselineAndCurrent => "missing_baseline_and_current",
        }
    }
}

pub fn fangyuan_debug_report_output_dir() -> &'static str {
    FANGYUAN_DEBUG_REPORT_OUTPUT_DIR
}

pub fn fangyuan_debug_report_json_path(report_id: &str) -> String {
    format!(
        "{}/{}.json",
        FANGYUAN_DEBUG_REPORT_OUTPUT_DIR,
        sanitize_report_id(report_id)
    )
}

pub fn fangyuan_pressure_baseline_json_path() -> &'static str {
    FANGYUAN_PRESSURE_BASELINE_JSON_PATH
}

pub fn fangyuan_pressure_config_hash(config: &FangyuanPressureTestConfig) -> u64 {
    let mut hash = FNV_OFFSET;
    mix_usize(&mut hash, config.actor_count);
    mix_str(&mut hash, &config.skill_template_id);
    mix_u64(&mut hash, config.trigger_interval_ticks);
    mix_u64(&mut hash, config.seed);
    mix_u32(&mut hash, config.scene_size.width.to_bits());
    mix_u32(&mut hash, config.scene_size.depth.to_bits());
    mix_u32(&mut hash, config.chunk_count);
    mix_str(&mut hash, config.budget_profile.as_str());
    mix_u64(&mut hash, config.duration_ticks);
    mix_u32(&mut hash, config.ticks_per_second);
    avalanche(hash)
}

fn sanitize_report_id(report_id: &str) -> String {
    let mut sanitized = String::new();
    for character in report_id.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
            sanitized.push(character);
        } else {
            sanitized.push('_');
        }
    }
    if sanitized.is_empty() {
        "fangyuan-debug-report".to_string()
    } else {
        sanitized
    }
}

fn degrade_level_label(level: FangyuanSkillDegradeLevel) -> &'static str {
    match level {
        FangyuanSkillDegradeLevel::None => "none",
        FangyuanSkillDegradeLevel::Low => "low",
        FangyuanSkillDegradeLevel::Medium => "medium",
        FangyuanSkillDegradeLevel::High => "high",
        FangyuanSkillDegradeLevel::Critical => "critical",
    }
}

fn replay_mismatch_field_label(field: FangyuanVisualReplayMismatchField) -> &'static str {
    match field {
        FangyuanVisualReplayMismatchField::MissingSample => "missing_sample",
        FangyuanVisualReplayMismatchField::ExtraSample => "extra_sample",
        FangyuanVisualReplayMismatchField::RuleLayer => "rule_layer",
        FangyuanVisualReplayMismatchField::PersonalityLayer => "personality_layer",
        FangyuanVisualReplayMismatchField::State => "state",
        FangyuanVisualReplayMismatchField::MaterialParams => "material_params",
        FangyuanVisualReplayMismatchField::Lod => "lod",
        FangyuanVisualReplayMismatchField::Degrade => "degrade",
        FangyuanVisualReplayMismatchField::CachePath => "cache_path",
        FangyuanVisualReplayMismatchField::Fallback => "fallback",
        FangyuanVisualReplayMismatchField::VisualHash => "visual_hash",
    }
}

fn f64_near_zero(value: f64) -> bool {
    value.abs() <= f64::EPSILON
}

fn mix_str(hash: &mut u64, value: &str) {
    for byte in value.as_bytes() {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(FNV_PRIME);
    }
}

fn mix_usize(hash: &mut u64, value: usize) {
    mix_u64(hash, value as u64);
}

fn mix_u32(hash: &mut u64, value: u32) {
    mix_u64(hash, u64::from(value));
}

fn mix_u64(hash: &mut u64, value: u64) {
    for byte in value.to_le_bytes() {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(FNV_PRIME);
    }
}

fn avalanche(mut value: u64) -> u64 {
    value ^= value >> 33;
    value = value.wrapping_mul(0xff51_afd7_ed55_8ccd);
    value ^= value >> 33;
    value = value.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    value ^ (value >> 33)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::fangyuan::{
        FANGYUAN_SKILL_PROJECTILE_TEMPLATE_ID, FangyuanPressureBudgetProfileKind,
        FangyuanPressureCurveSummary, FangyuanPressureDegradeSummary, FangyuanPressureSceneSize,
    };
    use serde_json::Value;

    fn pressure_report(
        actor_count: usize,
        peak_pressure: usize,
        hash: u64,
    ) -> FangyuanPressureReport {
        let mut config = FangyuanPressureTestConfig::new(
            actor_count,
            FANGYUAN_SKILL_PROJECTILE_TEMPLATE_ID,
            6,
            42,
            FangyuanPressureSceneSize::new(96.0, 96.0),
            4,
            FangyuanPressureBudgetProfileKind::Standard,
        );
        config.duration_ticks = 12;
        let curve = FangyuanPressureCurveSummary {
            active_vfx: FangyuanPressureMetricStats {
                peak: actor_count / 10,
                average: actor_count as f64 / 20.0,
            },
            dynamic_primitive: FangyuanPressureMetricStats {
                peak: actor_count / 5,
                average: actor_count as f64 / 10.0,
            },
            trail: FangyuanPressureMetricStats {
                peak: actor_count / 20,
                average: actor_count as f64 / 30.0,
            },
            transparent: FangyuanPressureMetricStats {
                peak: actor_count / 25,
                average: actor_count as f64 / 40.0,
            },
            emissive: FangyuanPressureMetricStats {
                peak: actor_count / 50,
                average: actor_count as f64 / 60.0,
            },
            pressure: FangyuanPressureMetricStats {
                peak: peak_pressure,
                average: peak_pressure as f64 / 2.0,
            },
        };
        let degrade = FangyuanPressureDegradeSummary {
            none_ticks: 9,
            low_ticks: 3,
            medium_ticks: 0,
            high_ticks: 0,
            critical_ticks: 0,
            worst_level: FangyuanSkillDegradeLevel::Low,
            reason: "above_recommended_budget".to_string(),
        };
        FangyuanPressureReport {
            config,
            skill_visual_id: "skill.projectile.visual".to_string(),
            total_trigger_events: actor_count / 2,
            sample_count: 13,
            curve,
            degrade,
            chunk_load: Vec::new(),
            deterministic_hash: hash,
            summary_text: format!("pressure actors={actor_count} hash={hash}"),
        }
    }

    fn comparison_by_actor(
        report: &FangyuanPressureBaselineComparisonReport,
        actor_count: usize,
    ) -> &FangyuanPressureBaselineComparison {
        report
            .comparisons
            .iter()
            .find(|comparison| comparison.actor_count == actor_count)
            .expect("comparison should exist")
    }

    #[test]
    fn fangyuan_debug_report_schema_fields_are_stable() {
        let value = serde_json::to_value(FangyuanDebugReport::default()).unwrap();
        let object = value.as_object().unwrap();
        let expected_keys = BTreeSet::from([
            "schema_version",
            "report_kind",
            "report_id",
            "output_policy",
            "audit",
            "budget",
            "render",
            "lod",
            "aoi",
            "cache",
            "bake",
            "pressure",
            "replay",
        ]);
        let actual_keys = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
        assert_eq!(actual_keys, expected_keys);

        let mut unknown = value;
        unknown
            .as_object_mut()
            .unwrap()
            .insert("unexpected_field".to_string(), Value::Bool(true));
        assert!(serde_json::from_value::<FangyuanDebugReport>(unknown).is_err());
    }

    #[test]
    fn fangyuan_debug_report_empty_data_defaults_are_serializable() {
        let report = FangyuanDebugReport::default();

        assert_eq!(report.schema_version, FANGYUAN_DEBUG_REPORT_SCHEMA_VERSION);
        assert_eq!(report.audit.status, "passed");
        assert_eq!(report.render.render_mode, "missing");
        assert_eq!(report.replay.included, false);
        assert_eq!(
            fangyuan_debug_report_output_dir(),
            "artifacts/fangyuan-debug"
        );
        assert_eq!(
            fangyuan_debug_report_json_path("local run/phone"),
            "artifacts/fangyuan-debug/local_run_phone.json"
        );
        assert_eq!(
            fangyuan_pressure_baseline_json_path(),
            "artifacts/fangyuan-debug/pressure-baseline.json"
        );

        let json = serde_json::to_string_pretty(&report).unwrap();
        let round_trip: FangyuanDebugReport = serde_json::from_str(&json).unwrap();
        assert_eq!(round_trip, report);
    }

    #[test]
    fn fangyuan_debug_report_pressure_summary_is_embedded() {
        let pressure = pressure_report(100, 77, 0xabc);
        let report = FangyuanDebugReport::from_metrics_snapshot(
            "pressure-100",
            &FangyuanDebugMetricsSnapshot::default(),
        )
        .with_pressure_report(&pressure);

        assert!(report.pressure.included);
        assert_eq!(report.pressure.actor_count, 100);
        assert_eq!(
            report.pressure.skill_template_id,
            FANGYUAN_SKILL_PROJECTILE_TEMPLATE_ID
        );
        assert_eq!(report.pressure.curve.pressure.peak, 77);
        assert_eq!(report.pressure.curve.active_vfx.average, 5.0);
        assert_eq!(report.pressure.degrade.worst_level, "low");
        assert_eq!(report.pressure.deterministic_hash, 0xabc);
    }

    #[test]
    fn fangyuan_debug_report_visual_replay_mismatch_is_embedded() {
        let replay = FangyuanVisualReplayConsistencyReport {
            replay_id: "authority_replay_a".to_string(),
            start_tick: 100,
            event_count: 1,
            visual_hash: 0x222,
            mismatch_summary: Some(FangyuanVisualReplayMismatchSummary {
                replay_id: "authority_replay_a".to_string(),
                tick: 120,
                frame_id: 7,
                event_id: "evt_seed".to_string(),
                recipe_id: "vfx.projectile".to_string(),
                object_id: "skill_object_evt_seed".to_string(),
                expected_hash: 0x111,
                actual_hash: 0x222,
                field: FangyuanVisualReplayMismatchField::VisualHash,
            }),
            samples: Vec::new(),
        };

        let report = FangyuanDebugReport::new("replay-mismatch").with_visual_replay_report(&replay);
        let mismatch = report.replay.mismatch.as_ref().unwrap();

        assert!(report.replay.included);
        assert_eq!(report.replay.replay_id, "authority_replay_a");
        assert_eq!(report.replay.visual_hash, 0x222);
        assert_eq!(mismatch.field, "visual_hash");
        assert_eq!(mismatch.tick, 120);
        assert!(mismatch.summary_text.contains("event=evt_seed"));
    }

    #[test]
    fn fangyuan_debug_report_baseline_compare_detects_hash_and_metric_changes() {
        let baseline_100 = pressure_report(100, 80, 0x100);
        let baseline_300 = pressure_report(300, 120, 0x300);
        let baseline_1000 = pressure_report(1000, 240, 0x1000);
        let current_100 = pressure_report(100, 80, 0x100);
        let current_300 = pressure_report(300, 150, 0x301);

        let baseline = FangyuanPressureBaselineSnapshot::from_pressure_reports([
            &baseline_100,
            &baseline_300,
            &baseline_1000,
        ]);
        let current =
            FangyuanPressureBaselineSnapshot::from_pressure_reports([&current_100, &current_300]);
        let comparison = baseline.compare_to(&current);

        assert_eq!(
            baseline.missing_supported_actor_counts(),
            Vec::<usize>::new()
        );
        assert_eq!(current.missing_supported_actor_counts(), vec![1000]);
        assert_eq!(
            comparison_by_actor(&comparison, 100).result,
            FangyuanPressureBaselineComparisonResult::Matched
        );
        assert_eq!(
            comparison_by_actor(&comparison, 300).result,
            FangyuanPressureBaselineComparisonResult::Regressed
        );
        assert_eq!(
            comparison_by_actor(&comparison, 300).pressure_peak_delta,
            30
        );
        assert_eq!(
            comparison_by_actor(&comparison, 1000).result,
            FangyuanPressureBaselineComparisonResult::MissingCurrent
        );
        assert!(comparison.has_regression);

        let compared_snapshot = current.with_last_comparison(comparison);
        let json = serde_json::to_string(&compared_snapshot).unwrap();
        assert!(json.contains("\"last_comparison\""));
        assert!(json.contains("\"actor_count\":300"));
        assert!(json.contains("\"result\":\"regressed\""));
    }
}
