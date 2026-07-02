use bevy::prelude::{Color, Vec3};

use super::{
    FANGYUAN_BLUEPRINT_HARD_PRIMITIVE_LIMIT, FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE,
    FANGYUAN_PRIMITIVE_MAX_EMISSIVE, FangyuanPrimitive, FangyuanPrimitiveKind,
    FangyuanPrimitiveRoleDistribution, FangyuanPrimitiveSet,
};

/// Unified Fangyuan audit report shared by later blueprint, prefab, layout, and
/// runtime primitive-set checks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanAuditReport {
    pub source_kind: FangyuanAuditSourceKind,
    pub source_path: Option<String>,
    pub status: FangyuanAuditStatus,
    pub summary: FangyuanAuditSummary,
    pub findings: Vec<FangyuanAuditFinding>,
    pub suggestions: Vec<FangyuanAuditSuggestion>,
}

impl FangyuanAuditReport {
    pub fn new(
        source_kind: FangyuanAuditSourceKind,
        source_path: impl Into<Option<String>>,
    ) -> Self {
        Self {
            source_kind,
            source_path: source_path.into(),
            status: FangyuanAuditStatus::Passed,
            summary: FangyuanAuditSummary::default(),
            findings: Vec::new(),
            suggestions: Vec::new(),
        }
    }

    pub fn add_finding(&mut self, finding: FangyuanAuditFinding) {
        self.findings.push(finding);
        self.refresh_summary_and_status();
    }

    pub fn add_suggestion(&mut self, suggestion: FangyuanAuditSuggestion) {
        if !self.suggestions.contains(&suggestion) {
            self.suggestions.push(suggestion);
        }
    }

    pub fn sort_findings(&mut self) {
        self.findings.sort();
    }

    pub fn apply_primitive_budget_stats(&mut self, stats: &FangyuanPrimitiveBudgetStats) {
        self.summary.authored_primitives = stats.authored_primitives;
        self.summary.generated_primitives = stats.generated_primitives;
        self.summary.skipped_primitives = stats.skipped_primitives;
        self.summary.cube_count = stats.cube_count;
        self.summary.sphere_count = stats.sphere_count;
        self.summary.color_count = stats.color_count;
        self.summary.material_count = stats.material_profile_count;
        self.summary.alpha_count = stats.alpha_count;
        self.summary.emissive_count = stats.emissive_count;
        self.summary.lifecycle_count = stats.lifecycle_count.total_with_lifecycle;
        self.summary.role_distribution = stats.role_distribution;
    }

    pub fn refresh_summary_and_status(&mut self) {
        self.summary = FangyuanAuditSummary::from_findings(&self.findings);
        self.status = FangyuanAuditStatus::from_summary(&self.summary);
    }
}

impl Default for FangyuanAuditReport {
    fn default() -> Self {
        Self::new(FangyuanAuditSourceKind::Unknown, None)
    }
}

pub const FANGYUAN_AUDIT_DEFAULT_RECOMMENDED_PRIMITIVE_LIMIT: usize = 800;
pub const FANGYUAN_AUDIT_DEFAULT_MAX_BOUNDS_WIDTH: f32 = 64.0;
pub const FANGYUAN_AUDIT_DEFAULT_MAX_BOUNDS_DEPTH: f32 = 64.0;
pub const FANGYUAN_AUDIT_DEFAULT_MAX_BOUNDS_HEIGHT: f32 = 64.0;
pub const FANGYUAN_AUDIT_DEFAULT_MAX_PRIMITIVE_EXTENT: f32 = 16.0;
pub const FANGYUAN_AUDIT_DEFAULT_MAX_PRIMITIVE_VOLUME: f32 = 4096.0;
pub const FANGYUAN_AUDIT_DEFAULT_MAX_TOTAL_VOLUME: f32 = 32768.0;
pub const FANGYUAN_AUDIT_DEFAULT_RECOMMENDED_ALPHA_COUNT: usize = 32;
pub const FANGYUAN_AUDIT_DEFAULT_MAX_ALPHA_COUNT: usize = 128;
pub const FANGYUAN_AUDIT_DEFAULT_RECOMMENDED_EMISSIVE_COUNT: usize = 24;
pub const FANGYUAN_AUDIT_DEFAULT_MAX_EMISSIVE_COUNT: usize = 96;
pub const FANGYUAN_AUDIT_DEFAULT_RECOMMENDED_MATERIAL_PROFILE_COUNT: usize = 8;
pub const FANGYUAN_AUDIT_DEFAULT_MAX_MATERIAL_PROFILE_COUNT: usize = 24;

/// Default primitive budget profile for Fangyuan audit checks.
///
/// Role, element, profession, and world-layer fields are reserved budget entry
/// points. The default profile intentionally does not depend on gameplay data.
#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanAuditBudgetProfile {
    pub hard_primitive_limit: usize,
    pub recommended_primitive_limit: usize,
    pub max_bounds: Vec3,
    pub max_primitive_extent: f32,
    pub max_primitive_volume: f32,
    pub max_total_volume: f32,
    pub recommended_alpha_count: usize,
    pub max_alpha_count: usize,
    pub recommended_emissive_count: usize,
    pub max_emissive_count: usize,
    pub max_emissive_intensity: f32,
    pub recommended_material_profile_count: usize,
    pub max_material_profile_count: usize,
    pub role_budget: FangyuanAuditRoleBudget,
    pub element_budget_tier: FangyuanAuditReservedBudgetTier,
    pub profession_budget_tier: FangyuanAuditReservedBudgetTier,
    pub world_layer_budget_tier: FangyuanAuditReservedBudgetTier,
}

impl Default for FangyuanAuditBudgetProfile {
    fn default() -> Self {
        Self {
            hard_primitive_limit: FANGYUAN_BLUEPRINT_HARD_PRIMITIVE_LIMIT,
            recommended_primitive_limit: FANGYUAN_AUDIT_DEFAULT_RECOMMENDED_PRIMITIVE_LIMIT,
            max_bounds: Vec3::new(
                FANGYUAN_AUDIT_DEFAULT_MAX_BOUNDS_WIDTH,
                FANGYUAN_AUDIT_DEFAULT_MAX_BOUNDS_HEIGHT,
                FANGYUAN_AUDIT_DEFAULT_MAX_BOUNDS_DEPTH,
            ),
            max_primitive_extent: FANGYUAN_AUDIT_DEFAULT_MAX_PRIMITIVE_EXTENT,
            max_primitive_volume: FANGYUAN_AUDIT_DEFAULT_MAX_PRIMITIVE_VOLUME,
            max_total_volume: FANGYUAN_AUDIT_DEFAULT_MAX_TOTAL_VOLUME,
            recommended_alpha_count: FANGYUAN_AUDIT_DEFAULT_RECOMMENDED_ALPHA_COUNT,
            max_alpha_count: FANGYUAN_AUDIT_DEFAULT_MAX_ALPHA_COUNT,
            recommended_emissive_count: FANGYUAN_AUDIT_DEFAULT_RECOMMENDED_EMISSIVE_COUNT,
            max_emissive_count: FANGYUAN_AUDIT_DEFAULT_MAX_EMISSIVE_COUNT,
            max_emissive_intensity: FANGYUAN_PRIMITIVE_MAX_EMISSIVE,
            recommended_material_profile_count:
                FANGYUAN_AUDIT_DEFAULT_RECOMMENDED_MATERIAL_PROFILE_COUNT,
            max_material_profile_count: FANGYUAN_AUDIT_DEFAULT_MAX_MATERIAL_PROFILE_COUNT,
            role_budget: FangyuanAuditRoleBudget::default(),
            element_budget_tier: FangyuanAuditReservedBudgetTier::Default,
            profession_budget_tier: FangyuanAuditReservedBudgetTier::Default,
            world_layer_budget_tier: FangyuanAuditReservedBudgetTier::Default,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FangyuanAuditRoleBudget {
    pub reserved: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FangyuanAuditReservedBudgetTier {
    #[default]
    Default,
    Reserved,
}

/// Primitive budget statistics with explicit accounting buckets.
///
/// `authored`, `generated`, `skipped`, and `expanded` are separate because
/// prefab/layout audit must not hide primitive cost inside container records.
/// Runtime primitive-set checks use already-expanded primitives as the budget
/// surface and fill `runtime_primitives` directly.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FangyuanPrimitiveBudgetStats {
    pub authored_primitives: usize,
    pub generated_primitives: usize,
    pub skipped_primitives: usize,
    pub expanded_primitives: usize,
    pub runtime_primitives: usize,
    pub cube_count: usize,
    pub sphere_count: usize,
    pub color_count: usize,
    pub total_volume: f32,
    pub max_primitive_extent: f32,
    pub max_primitive_volume: f32,
    pub bounds_size: Vec3,
    pub alpha_count: usize,
    pub emissive_count: usize,
    pub max_emissive: f32,
    pub material_profile_count: usize,
    pub role_distribution: FangyuanPrimitiveRoleDistribution,
    pub lifecycle_count: FangyuanPrimitiveLifecycleCount,
}

impl FangyuanPrimitiveBudgetStats {
    pub fn from_primitive_set(primitive_set: &FangyuanPrimitiveSet) -> Self {
        Self::from_runtime_primitives(primitive_set.primitives())
    }

    pub fn from_runtime_primitives(primitives: &[FangyuanPrimitive]) -> Self {
        use std::collections::BTreeSet;

        let mut stats = Self {
            runtime_primitives: primitives.len(),
            expanded_primitives: primitives.len(),
            ..Default::default()
        };
        let mut colors = BTreeSet::new();
        let mut material_profiles = BTreeSet::new();
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);

        for primitive in primitives {
            match primitive.kind() {
                FangyuanPrimitiveKind::Cube => stats.cube_count += 1,
                FangyuanPrimitiveKind::Sphere => stats.sphere_count += 1,
            }

            let scale = primitive.scale().abs();
            let extent = scale.max_element();
            let volume = scale.x * scale.y * scale.z;
            let center = primitive.local_position();
            let half = scale * 0.5;

            stats.total_volume += volume;
            stats.max_primitive_extent = stats.max_primitive_extent.max(extent);
            stats.max_primitive_volume = stats.max_primitive_volume.max(volume);
            stats.role_distribution.increment(primitive.role());
            colors.insert(FangyuanAuditPrimitiveColorKey::from_color(
                primitive.color(),
            ));
            min = min.min(center - half);
            max = max.max(center + half);

            if primitive.alpha() < 1.0 {
                stats.alpha_count += 1;
            }
            if primitive.emissive() > FANGYUAN_PRIMITIVE_DEFAULT_EMISSIVE {
                stats.emissive_count += 1;
            }
            stats.max_emissive = stats.max_emissive.max(primitive.emissive());
            if let Some(material_profile_id) = primitive.material_profile_id() {
                material_profiles.insert(material_profile_id);
            }

            stats.lifecycle_count.record(primitive.lifecycle());
        }

        if !primitives.is_empty() {
            stats.bounds_size = max - min;
        }
        stats.color_count = colors.len();
        stats.material_profile_count = material_profiles.len();
        stats
    }

    pub fn counted_primitives(&self) -> usize {
        self.runtime_primitives.max(self.expanded_primitives)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FangyuanPrimitiveLifecycleCount {
    pub total_with_lifecycle: usize,
    pub lifetime: usize,
    pub spawn_tick: usize,
    pub despawn_tick: usize,
}

impl FangyuanPrimitiveLifecycleCount {
    fn record(&mut self, lifecycle: super::FangyuanPrimitiveLifecycle) {
        if lifecycle.is_empty() {
            return;
        }

        self.total_with_lifecycle += 1;
        if lifecycle.lifetime.is_some() {
            self.lifetime += 1;
        }
        if lifecycle.spawn_tick.is_some() {
            self.spawn_tick += 1;
        }
        if lifecycle.despawn_tick.is_some() {
            self.despawn_tick += 1;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct FangyuanAuditPrimitiveColorKey([u32; 4]);

impl FangyuanAuditPrimitiveColorKey {
    fn from_color(color: Color) -> Self {
        let color = color.to_srgba();
        Self([
            canonical_f32_bits(color.red),
            canonical_f32_bits(color.green),
            canonical_f32_bits(color.blue),
            canonical_f32_bits(color.alpha),
        ])
    }
}

fn canonical_f32_bits(value: f32) -> u32 {
    if value == 0.0 {
        0.0f32.to_bits()
    } else {
        value.to_bits()
    }
}

pub fn audit_fangyuan_primitive_budget(
    stats: &FangyuanPrimitiveBudgetStats,
    profile: &FangyuanAuditBudgetProfile,
) -> FangyuanAuditReport {
    let mut report = FangyuanAuditReport::new(FangyuanAuditSourceKind::RuntimePrimitiveSet, None);

    report.apply_primitive_budget_stats(stats);

    add_primitive_count_findings(&mut report, stats, profile);
    add_bounds_findings(&mut report, stats, profile);
    add_scalar_limit_finding(
        &mut report,
        "primitive_volume_above_limit",
        stats.max_primitive_volume,
        profile.max_primitive_volume,
        "primitives[].scale",
        "largest primitive volume exceeds the hard budget",
        FangyuanAuditSuggestionAction::ShrinkBounds,
    );
    add_scalar_limit_finding(
        &mut report,
        "primitive_extent_above_limit",
        stats.max_primitive_extent,
        profile.max_primitive_extent,
        "primitives[].scale",
        "largest primitive extent exceeds the hard budget",
        FangyuanAuditSuggestionAction::ShrinkBounds,
    );
    add_scalar_limit_finding(
        &mut report,
        "total_volume_above_limit",
        stats.total_volume,
        profile.max_total_volume,
        "primitives",
        "total primitive volume exceeds the hard budget",
        FangyuanAuditSuggestionAction::ShrinkBounds,
    );
    add_count_budget_findings(
        &mut report,
        stats.alpha_count,
        profile.recommended_alpha_count,
        profile.max_alpha_count,
        "alpha_count_above_recommended",
        "alpha_count_above_hard_limit",
        "primitives[].alpha",
        "transparent primitive count exceeds the recommended budget",
        "transparent primitive count exceeds the hard budget",
        FangyuanAuditSuggestionAction::RemoveAlpha,
    );
    add_count_budget_findings(
        &mut report,
        stats.emissive_count,
        profile.recommended_emissive_count,
        profile.max_emissive_count,
        "emissive_count_above_recommended",
        "emissive_count_above_hard_limit",
        "primitives[].emissive",
        "emissive primitive count exceeds the recommended budget",
        "emissive primitive count exceeds the hard budget",
        FangyuanAuditSuggestionAction::LowerEmissive,
    );
    add_scalar_limit_finding(
        &mut report,
        "emissive_intensity_above_limit",
        stats.max_emissive,
        profile.max_emissive_intensity,
        "primitives[].emissive",
        "emissive intensity exceeds the hard budget",
        FangyuanAuditSuggestionAction::LowerEmissive,
    );
    add_count_budget_findings(
        &mut report,
        stats.material_profile_count,
        profile.recommended_material_profile_count,
        profile.max_material_profile_count,
        "material_profile_count_above_recommended",
        "material_profile_count_above_hard_limit",
        "primitives[].material_profile_id",
        "material profile count exceeds the recommended budget",
        "material profile count exceeds the hard budget",
        FangyuanAuditSuggestionAction::ReplaceMaterialProfile,
    );

    report.refresh_summary_and_status();
    report.apply_primitive_budget_stats(stats);
    report.sort_findings();
    report
}

pub fn audit_fangyuan_primitive_set_budget(
    primitive_set: &FangyuanPrimitiveSet,
    profile: &FangyuanAuditBudgetProfile,
) -> FangyuanAuditReport {
    audit_fangyuan_primitive_budget(
        &FangyuanPrimitiveBudgetStats::from_primitive_set(primitive_set),
        profile,
    )
}

fn add_primitive_count_findings(
    report: &mut FangyuanAuditReport,
    stats: &FangyuanPrimitiveBudgetStats,
    profile: &FangyuanAuditBudgetProfile,
) {
    add_count_budget_findings(
        report,
        stats.counted_primitives(),
        profile.recommended_primitive_limit,
        profile.hard_primitive_limit,
        "primitive_count_above_recommended",
        "primitive_count_above_hard_limit",
        "primitives",
        "primitive count exceeds the recommended budget",
        "primitive count exceeds the hard budget",
        FangyuanAuditSuggestionAction::ReducePrimitives,
    );
}

fn add_bounds_findings(
    report: &mut FangyuanAuditReport,
    stats: &FangyuanPrimitiveBudgetStats,
    profile: &FangyuanAuditBudgetProfile,
) {
    let axes = [
        ("width", stats.bounds_size.x, profile.max_bounds.x),
        ("height", stats.bounds_size.y, profile.max_bounds.y),
        ("depth", stats.bounds_size.z, profile.max_bounds.z),
    ];

    for (axis, value, limit) in axes {
        if value <= limit {
            continue;
        }

        add_finding_with_suggestion(
            report,
            FangyuanAuditSeverity::Error,
            "bounds_above_limit",
            Some("bounds".to_string()),
            format!("bounds {axis} {value:.2} exceeds hard limit {limit:.2}"),
            FangyuanAuditSuggestionAction::ShrinkBounds,
        );
    }
}

fn add_count_budget_findings(
    report: &mut FangyuanAuditReport,
    count: usize,
    recommended_limit: usize,
    hard_limit: usize,
    warning_code: &'static str,
    error_code: &'static str,
    field_path: &'static str,
    warning_reason: &'static str,
    error_reason: &'static str,
    suggestion: FangyuanAuditSuggestionAction,
) {
    if count > hard_limit {
        add_finding_with_suggestion(
            report,
            FangyuanAuditSeverity::Error,
            error_code,
            Some(field_path.to_string()),
            format!("{error_reason}: {count} > {hard_limit}"),
            suggestion,
        );
    } else if count > recommended_limit {
        add_finding_with_suggestion(
            report,
            FangyuanAuditSeverity::Warning,
            warning_code,
            Some(field_path.to_string()),
            format!("{warning_reason}: {count} > {recommended_limit}"),
            suggestion,
        );
    }
}

fn add_scalar_limit_finding(
    report: &mut FangyuanAuditReport,
    code: &'static str,
    value: f32,
    hard_limit: f32,
    field_path: &'static str,
    reason: &'static str,
    suggestion: FangyuanAuditSuggestionAction,
) {
    if value <= hard_limit {
        return;
    }

    add_finding_with_suggestion(
        report,
        FangyuanAuditSeverity::Error,
        code,
        Some(field_path.to_string()),
        format!("{reason}: {value:.2} > {hard_limit:.2}"),
        suggestion,
    );
}

fn add_finding_with_suggestion(
    report: &mut FangyuanAuditReport,
    severity: FangyuanAuditSeverity,
    code: &'static str,
    field_path: Option<String>,
    reason: String,
    suggestion: FangyuanAuditSuggestionAction,
) {
    let mut finding = FangyuanAuditFinding::new(
        severity,
        code,
        reason.clone(),
        FangyuanAuditSourceKind::RuntimePrimitiveSet,
    );
    finding.field_path = field_path.clone();
    report.add_finding(finding);
    report.add_suggestion(FangyuanAuditSuggestion::new(suggestion, field_path, reason));
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FangyuanAuditSummary {
    pub error_count: usize,
    pub warning_count: usize,
    pub info_count: usize,
    pub prefab_count: usize,
    pub reusable_prefab_count: usize,
    pub authored_primitives: usize,
    pub generated_primitives: usize,
    pub skipped_primitives: usize,
    pub cube_count: usize,
    pub sphere_count: usize,
    pub color_count: usize,
    pub material_count: usize,
    pub alpha_count: usize,
    pub emissive_count: usize,
    pub lifecycle_count: usize,
    pub role_distribution: FangyuanPrimitiveRoleDistribution,
}

impl FangyuanAuditSummary {
    pub fn from_findings(findings: &[FangyuanAuditFinding]) -> Self {
        let mut summary = Self::default();
        for finding in findings {
            match finding.severity {
                FangyuanAuditSeverity::Error => summary.error_count += 1,
                FangyuanAuditSeverity::Warning => summary.warning_count += 1,
                FangyuanAuditSeverity::Info => summary.info_count += 1,
            }
        }
        summary
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FangyuanAuditStatus {
    #[default]
    Passed,
    PassedWithWarnings,
    Failed,
}

impl FangyuanAuditStatus {
    pub fn from_summary(summary: &FangyuanAuditSummary) -> Self {
        if summary.error_count > 0 {
            Self::Failed
        } else if summary.warning_count > 0 {
            Self::PassedWithWarnings
        } else {
            Self::Passed
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum FangyuanAuditSeverity {
    Error,
    Warning,
    #[default]
    Info,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum FangyuanAuditSourceKind {
    Blueprint,
    PrefabPalette,
    SceneLayout,
    RuntimePrimitiveSet,
    #[default]
    Unknown,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FangyuanAuditFinding {
    pub severity: FangyuanAuditSeverity,
    pub code: String,
    pub field_path: Option<String>,
    pub reason: String,
    pub source_kind: FangyuanAuditSourceKind,
    pub source_path: Option<String>,
    pub primitive_index: Option<usize>,
    pub prefab_id: Option<String>,
    pub instance_id: Option<String>,
    pub instance_index: Option<usize>,
    pub prefab_primitive_index: Option<usize>,
}

impl FangyuanAuditFinding {
    pub fn new(
        severity: FangyuanAuditSeverity,
        code: impl Into<String>,
        reason: impl Into<String>,
        source_kind: FangyuanAuditSourceKind,
    ) -> Self {
        Self {
            severity,
            code: code.into(),
            reason: reason.into(),
            source_kind,
            ..Default::default()
        }
    }
}

impl Ord for FangyuanAuditFinding {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (
            self.severity,
            self.source_kind,
            self.field_path.as_deref(),
            self.code.as_str(),
            self.source_path.as_deref(),
            self.primitive_index,
            self.prefab_id.as_deref(),
            self.instance_id.as_deref(),
            self.instance_index,
            self.prefab_primitive_index,
            self.reason.as_str(),
        )
            .cmp(&(
                other.severity,
                other.source_kind,
                other.field_path.as_deref(),
                other.code.as_str(),
                other.source_path.as_deref(),
                other.primitive_index,
                other.prefab_id.as_deref(),
                other.instance_id.as_deref(),
                other.instance_index,
                other.prefab_primitive_index,
                other.reason.as_str(),
            ))
    }
}

impl PartialOrd for FangyuanAuditFinding {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanAuditSuggestion {
    pub action: FangyuanAuditSuggestionAction,
    pub field_path: Option<String>,
    pub reason: String,
    pub estimated_effect: Option<String>,
}

impl FangyuanAuditSuggestion {
    pub fn new(
        action: FangyuanAuditSuggestionAction,
        field_path: impl Into<Option<String>>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            action,
            field_path: field_path.into(),
            reason: reason.into(),
            estimated_effect: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanAuditSuggestionAction {
    ReducePrimitives,
    ShrinkBounds,
    LowerEmissive,
    RemoveAlpha,
    ReplaceMaterialProfile,
}

#[cfg(test)]
mod tests {
    use bevy::prelude::*;

    use super::*;

    #[test]
    fn fangyuan_audit_report_defaults_to_passed_without_findings() {
        let report = FangyuanAuditReport::new(
            FangyuanAuditSourceKind::Blueprint,
            Some("fangyuan/avatars/minimal_player.ron".to_string()),
        );

        assert_eq!(report.status, FangyuanAuditStatus::Passed);
        assert_eq!(report.summary, FangyuanAuditSummary::default());
        assert_eq!(report.source_kind, FangyuanAuditSourceKind::Blueprint);
        assert_eq!(
            report.source_path.as_deref(),
            Some("fangyuan/avatars/minimal_player.ron")
        );
    }

    #[test]
    fn fangyuan_audit_status_passes_with_warnings_when_no_error_exists() {
        let mut report = FangyuanAuditReport::default();
        report.add_finding(FangyuanAuditFinding::new(
            FangyuanAuditSeverity::Warning,
            "bounds.large",
            "bounds are larger than the mobile budget",
            FangyuanAuditSourceKind::SceneLayout,
        ));

        assert_eq!(report.status, FangyuanAuditStatus::PassedWithWarnings);
        assert_eq!(report.summary.warning_count, 1);
        assert_eq!(report.summary.error_count, 0);
    }

    #[test]
    fn fangyuan_audit_status_fails_when_error_exists() {
        let mut report = FangyuanAuditReport::default();
        report.add_finding(FangyuanAuditFinding::new(
            FangyuanAuditSeverity::Warning,
            "material.alpha",
            "transparent material may be expensive",
            FangyuanAuditSourceKind::Blueprint,
        ));
        report.add_finding(FangyuanAuditFinding::new(
            FangyuanAuditSeverity::Error,
            "primitive.count",
            "primitive count exceeds the hard limit",
            FangyuanAuditSourceKind::Blueprint,
        ));

        assert_eq!(report.status, FangyuanAuditStatus::Failed);
        assert_eq!(report.summary.error_count, 1);
        assert_eq!(report.summary.warning_count, 1);
    }

    #[test]
    fn fangyuan_audit_findings_sort_by_severity_and_stable_location_fields() {
        let mut report = FangyuanAuditReport::default();
        let mut warning = FangyuanAuditFinding::new(
            FangyuanAuditSeverity::Warning,
            "material.alpha",
            "alpha is not preferred",
            FangyuanAuditSourceKind::Blueprint,
        );
        warning.field_path = Some("primitives[1].alpha".to_string());
        warning.primitive_index = Some(1);

        let mut info = FangyuanAuditFinding::new(
            FangyuanAuditSeverity::Info,
            "material.profile",
            "default material profile used",
            FangyuanAuditSourceKind::PrefabPalette,
        );
        info.field_path = Some("prefabs[0].primitives[2].material_profile".to_string());
        info.prefab_id = Some("home_wall".to_string());
        info.prefab_primitive_index = Some(2);

        let mut error = FangyuanAuditFinding::new(
            FangyuanAuditSeverity::Error,
            "bounds.exceeded",
            "primitive is outside bounds",
            FangyuanAuditSourceKind::SceneLayout,
        );
        error.field_path = Some("instances[0].position".to_string());
        error.source_path = Some("fangyuan/layouts/home_layout.ron".to_string());
        error.instance_id = Some("entry_wall".to_string());
        error.instance_index = Some(0);

        report.findings = vec![info, warning, error];
        report.sort_findings();

        assert_eq!(report.findings[0].severity, FangyuanAuditSeverity::Error);
        assert_eq!(report.findings[1].severity, FangyuanAuditSeverity::Warning);
        assert_eq!(report.findings[2].severity, FangyuanAuditSeverity::Info);
        assert_eq!(
            report.findings[0].field_path.as_deref(),
            Some("instances[0].position")
        );
        assert_eq!(
            report.findings[0].instance_id.as_deref(),
            Some("entry_wall")
        );
        assert_eq!(report.findings[1].primitive_index, Some(1));
        assert_eq!(report.findings[2].prefab_id.as_deref(), Some("home_wall"));
        assert_eq!(report.findings[2].prefab_primitive_index, Some(2));
    }

    #[test]
    fn fangyuan_audit_suggestions_are_deduplicated_by_action_field_and_reason() {
        let mut report = FangyuanAuditReport::default();
        let suggestion = FangyuanAuditSuggestion::new(
            FangyuanAuditSuggestionAction::ReducePrimitives,
            Some("primitives".to_string()),
            "primitive count exceeds the recommended budget",
        );

        report.add_suggestion(suggestion.clone());
        report.add_suggestion(suggestion);
        report.add_suggestion(FangyuanAuditSuggestion::new(
            FangyuanAuditSuggestionAction::LowerEmissive,
            Some("primitives[0].emissive".to_string()),
            "emissive intensity is above the target range",
        ));

        assert_eq!(report.suggestions.len(), 2);
        assert_eq!(
            report.suggestions[0].action,
            FangyuanAuditSuggestionAction::ReducePrimitives
        );
        assert_eq!(
            report.suggestions[0].field_path.as_deref(),
            Some("primitives")
        );
        assert_eq!(
            report.suggestions[1].action,
            FangyuanAuditSuggestionAction::LowerEmissive
        );
    }

    #[test]
    fn fangyuan_budget_default_profile_uses_shared_hard_limit() {
        let profile = FangyuanAuditBudgetProfile::default();

        assert_eq!(
            profile.hard_primitive_limit,
            FANGYUAN_BLUEPRINT_HARD_PRIMITIVE_LIMIT
        );
        assert_eq!(profile.hard_primitive_limit, 1000);
        assert!(profile.recommended_primitive_limit < profile.hard_primitive_limit);
        assert!(profile.max_bounds.x > 0.0);
        assert!(profile.max_bounds.y > 0.0);
        assert!(profile.max_bounds.z > 0.0);
        assert_eq!(
            profile.element_budget_tier,
            FangyuanAuditReservedBudgetTier::Default
        );
        assert_eq!(
            profile.profession_budget_tier,
            FangyuanAuditReservedBudgetTier::Default
        );
        assert_eq!(
            profile.world_layer_budget_tier,
            FangyuanAuditReservedBudgetTier::Default
        );
    }

    #[test]
    fn fangyuan_budget_stats_summarize_runtime_primitives() {
        let primitive_set = FangyuanPrimitiveSet::from_primitives(vec![
            FangyuanPrimitive::with_runtime_metadata(
                super::super::FangyuanPrimitiveKind::Cube,
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(2.0, 3.0, 4.0),
                Color::srgba(1.0, 0.0, 0.0, 0.25),
                super::super::FangyuanPrimitiveRole::Decoration,
                0.25,
                4.0,
                Some("glow".to_string()),
                super::super::FangyuanPrimitiveLifecycle::new(Some(10), Some(1), Some(11)),
            ),
            FangyuanPrimitive::with_runtime_metadata(
                super::super::FangyuanPrimitiveKind::Sphere,
                Vec3::new(10.0, 0.0, 0.0),
                Vec3::splat(2.0),
                Color::WHITE,
                super::super::FangyuanPrimitiveRole::Core,
                1.0,
                0.0,
                Some("stone".to_string()),
                super::super::FangyuanPrimitiveLifecycle::empty(),
            ),
        ]);

        let stats = FangyuanPrimitiveBudgetStats::from_primitive_set(&primitive_set);

        assert_eq!(stats.runtime_primitives, 2);
        assert_eq!(stats.expanded_primitives, 2);
        assert_eq!(stats.total_volume, 32.0);
        assert_eq!(stats.max_primitive_extent, 4.0);
        assert_eq!(stats.max_primitive_volume, 24.0);
        assert_eq!(stats.alpha_count, 1);
        assert_eq!(stats.emissive_count, 1);
        assert_eq!(stats.max_emissive, 4.0);
        assert_eq!(stats.material_profile_count, 2);
        assert_eq!(
            stats
                .role_distribution
                .count(super::super::FangyuanPrimitiveRole::Decoration),
            1
        );
        assert_eq!(
            stats
                .role_distribution
                .count(super::super::FangyuanPrimitiveRole::Core),
            1
        );
        assert_eq!(stats.lifecycle_count.total_with_lifecycle, 1);
        assert_eq!(stats.lifecycle_count.lifetime, 1);
        assert_eq!(stats.lifecycle_count.spawn_tick, 1);
        assert_eq!(stats.lifecycle_count.despawn_tick, 1);
        assert_eq!(stats.bounds_size, Vec3::new(12.0, 3.0, 4.0));
    }

    #[test]
    fn fangyuan_budget_recommended_thresholds_create_warnings() {
        let profile = FangyuanAuditBudgetProfile {
            recommended_primitive_limit: 1,
            hard_primitive_limit: 10,
            recommended_alpha_count: 0,
            max_alpha_count: 10,
            recommended_emissive_count: 0,
            max_emissive_count: 10,
            recommended_material_profile_count: 0,
            max_material_profile_count: 10,
            ..Default::default()
        };
        let stats = FangyuanPrimitiveBudgetStats {
            runtime_primitives: 2,
            expanded_primitives: 2,
            alpha_count: 1,
            emissive_count: 1,
            material_profile_count: 1,
            ..Default::default()
        };

        let report = audit_fangyuan_primitive_budget(&stats, &profile);

        assert_eq!(report.status, FangyuanAuditStatus::PassedWithWarnings);
        assert_eq!(report.summary.warning_count, 4);
        assert_eq!(report.summary.error_count, 0);
        assert!(has_finding(&report, "primitive_count_above_recommended"));
        assert!(has_finding(&report, "alpha_count_above_recommended"));
        assert!(has_finding(&report, "emissive_count_above_recommended"));
        assert!(has_finding(
            &report,
            "material_profile_count_above_recommended"
        ));
        assert!(has_suggestion(
            &report,
            FangyuanAuditSuggestionAction::ReducePrimitives
        ));
        assert!(has_suggestion(
            &report,
            FangyuanAuditSuggestionAction::RemoveAlpha
        ));
        assert!(has_suggestion(
            &report,
            FangyuanAuditSuggestionAction::LowerEmissive
        ));
        assert!(has_suggestion(
            &report,
            FangyuanAuditSuggestionAction::ReplaceMaterialProfile
        ));
    }

    #[test]
    fn fangyuan_budget_hard_limits_create_errors() {
        let profile = FangyuanAuditBudgetProfile {
            recommended_primitive_limit: 1,
            hard_primitive_limit: 2,
            max_bounds: Vec3::splat(4.0),
            max_primitive_extent: 4.0,
            max_primitive_volume: 16.0,
            max_total_volume: 20.0,
            recommended_alpha_count: 1,
            max_alpha_count: 2,
            recommended_emissive_count: 1,
            max_emissive_count: 2,
            max_emissive_intensity: 3.0,
            recommended_material_profile_count: 1,
            max_material_profile_count: 2,
            ..Default::default()
        };
        let stats = FangyuanPrimitiveBudgetStats {
            runtime_primitives: 3,
            expanded_primitives: 3,
            bounds_size: Vec3::new(5.0, 3.0, 6.0),
            max_primitive_extent: 5.0,
            max_primitive_volume: 25.0,
            total_volume: 30.0,
            alpha_count: 3,
            emissive_count: 3,
            max_emissive: 4.0,
            material_profile_count: 3,
            ..Default::default()
        };

        let report = audit_fangyuan_primitive_budget(&stats, &profile);

        assert_eq!(report.status, FangyuanAuditStatus::Failed);
        assert!(report.summary.error_count >= 9);
        assert!(has_finding(&report, "primitive_count_above_hard_limit"));
        assert!(has_finding(&report, "bounds_above_limit"));
        assert!(has_finding(&report, "primitive_extent_above_limit"));
        assert!(has_finding(&report, "primitive_volume_above_limit"));
        assert!(has_finding(&report, "total_volume_above_limit"));
        assert!(has_finding(&report, "alpha_count_above_hard_limit"));
        assert!(has_finding(&report, "emissive_count_above_hard_limit"));
        assert!(has_finding(&report, "emissive_intensity_above_limit"));
        assert!(has_finding(
            &report,
            "material_profile_count_above_hard_limit"
        ));
        assert!(has_suggestion(
            &report,
            FangyuanAuditSuggestionAction::ReducePrimitives
        ));
        assert!(has_suggestion(
            &report,
            FangyuanAuditSuggestionAction::ShrinkBounds
        ));
    }

    #[test]
    fn fangyuan_budget_empty_primitive_set_passes() {
        let primitive_set = FangyuanPrimitiveSet::new();
        let report = audit_fangyuan_primitive_set_budget(
            &primitive_set,
            &FangyuanAuditBudgetProfile::default(),
        );

        assert_eq!(report.status, FangyuanAuditStatus::Passed);
        assert_eq!(report.summary.error_count, 0);
        assert_eq!(report.summary.warning_count, 0);
        assert!(report.findings.is_empty());
        assert!(report.suggestions.is_empty());
    }

    fn has_finding(report: &FangyuanAuditReport, code: &str) -> bool {
        report.findings.iter().any(|finding| finding.code == code)
    }

    fn has_suggestion(report: &FangyuanAuditReport, action: FangyuanAuditSuggestionAction) -> bool {
        report
            .suggestions
            .iter()
            .any(|suggestion| suggestion.action == action)
    }
}
