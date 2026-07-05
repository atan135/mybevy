use bevy::prelude::{Color, Resource, Vec3};

use super::{
    FangyuanAuditFinding, FangyuanAuditReport, FangyuanAuditSeverity, FangyuanAuditSourceKind,
    FangyuanAuditStatus, FangyuanAuditSuggestion, FangyuanAuditSuggestionAction,
    FangyuanEquipmentBlueprint, FangyuanNpcBlueprint, FangyuanNpcCompileOptions,
    FangyuanNpcDegradeLevel, FangyuanPrimitive, FangyuanPrimitiveKind, FangyuanPrimitiveLifecycle,
    FangyuanPrimitiveRole, FangyuanSkillAuditDiagnosticCode, FangyuanSkillTemplate,
    FangyuanSkillVisualBlueprint, FangyuanTiandaoLifecycleState, FangyuanTiandaoManifestation,
    FangyuanVfxDiagnostic, FangyuanVfxInstance, FangyuanVfxInstanceStartError, FangyuanVfxRecipe,
    FangyuanVfxReplayContext, FangyuanVfxRuntime, audit_fangyuan_skill_visual_readability,
    audit_fangyuan_vfx_recipe, estimate_fangyuan_vfx_recipe_budget,
    fangyuan_default_equipment_blueprint, fangyuan_default_npc_blueprint,
    fangyuan_default_skill_templates, fangyuan_default_skill_visual_blueprints,
    fangyuan_default_tiandao_manifestation, fangyuan_vfx_impact_expand_recipe,
    fangyuan_vfx_projectile_recipe, fangyuan_vfx_range_marker_recipe, fangyuan_vfx_shield_recipe,
};

pub const FANGYUAN_OBJECT_BUDGET_DEFAULT_RECOMMENDED_TOTAL_COST: u32 = 96;
pub const FANGYUAN_OBJECT_BUDGET_DEFAULT_HARD_TOTAL_COST: u32 = 128;
pub const FANGYUAN_OBJECT_TRIAL_ROUTE_ID: &str = "fangyuan.object_trial";
const FANGYUAN_OBJECT_TRIAL_TICKS_PER_SECOND: u32 = 30;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FangyuanObjectClass {
    Vfx,
    Skill,
    Equipment,
    Npc,
    Tiandao,
}

impl FangyuanObjectClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Vfx => "vfx",
            Self::Skill => "skill",
            Self::Equipment => "equipment",
            Self::Npc => "npc",
            Self::Tiandao => "tiandao",
        }
    }

    pub const fn retention_priority(self) -> u8 {
        match self {
            Self::Skill => 0,
            Self::Vfx => 1,
            Self::Equipment => 2,
            Self::Tiandao => 3,
            Self::Npc => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FangyuanObjectDegradeTarget {
    NpcDecoration,
    TiandaoTemporaryResidue,
    EquipmentAura,
    SkillPersonality,
}

impl FangyuanObjectDegradeTarget {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NpcDecoration => "npc_decoration",
            Self::TiandaoTemporaryResidue => "tiandao_temporary_residue",
            Self::EquipmentAura => "equipment_aura",
            Self::SkillPersonality => "skill_personality",
        }
    }

    pub const fn class(self) -> FangyuanObjectClass {
        match self {
            Self::NpcDecoration => FangyuanObjectClass::Npc,
            Self::TiandaoTemporaryResidue => FangyuanObjectClass::Tiandao,
            Self::EquipmentAura => FangyuanObjectClass::Equipment,
            Self::SkillPersonality => FangyuanObjectClass::Skill,
        }
    }

    pub const fn priority(self) -> u8 {
        match self {
            Self::NpcDecoration => 0,
            Self::TiandaoTemporaryResidue => 1,
            Self::EquipmentAura => 2,
            Self::SkillPersonality => 3,
        }
    }

    pub const fn estimated_cost_savings_per_object(self) -> u32 {
        match self {
            Self::NpcDecoration => 3,
            Self::TiandaoTemporaryResidue => 4,
            Self::EquipmentAura => 2,
            Self::SkillPersonality => 6,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FangyuanObjectClassBudget {
    pub recommended_count: usize,
    pub max_count: usize,
    pub recommended_cost: u32,
    pub max_cost: u32,
}

impl FangyuanObjectClassBudget {
    pub const fn new(
        recommended_count: usize,
        max_count: usize,
        recommended_cost: u32,
        max_cost: u32,
    ) -> Self {
        Self {
            recommended_count,
            max_count,
            recommended_cost,
            max_cost,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanObjectBudgetProfile {
    pub recommended_total_cost: u32,
    pub hard_total_cost: u32,
    pub vfx: FangyuanObjectClassBudget,
    pub skill: FangyuanObjectClassBudget,
    pub equipment: FangyuanObjectClassBudget,
    pub npc: FangyuanObjectClassBudget,
    pub tiandao: FangyuanObjectClassBudget,
}

impl Default for FangyuanObjectBudgetProfile {
    fn default() -> Self {
        Self {
            recommended_total_cost: FANGYUAN_OBJECT_BUDGET_DEFAULT_RECOMMENDED_TOTAL_COST,
            hard_total_cost: FANGYUAN_OBJECT_BUDGET_DEFAULT_HARD_TOTAL_COST,
            vfx: FangyuanObjectClassBudget::new(4, 16, 32, 48),
            skill: FangyuanObjectClassBudget::new(4, 8, 48, 64),
            equipment: FangyuanObjectClassBudget::new(2, 4, 16, 24),
            npc: FangyuanObjectClassBudget::new(4, 8, 24, 32),
            tiandao: FangyuanObjectClassBudget::new(4, 8, 24, 32),
        }
    }
}

impl FangyuanObjectBudgetProfile {
    pub const fn budget_for(&self, class: FangyuanObjectClass) -> FangyuanObjectClassBudget {
        match class {
            FangyuanObjectClass::Vfx => self.vfx,
            FangyuanObjectClass::Skill => self.skill,
            FangyuanObjectClass::Equipment => self.equipment,
            FangyuanObjectClass::Npc => self.npc,
            FangyuanObjectClass::Tiandao => self.tiandao,
        }
    }

    pub fn relaxed_trial() -> Self {
        Self {
            recommended_total_cost: 144,
            hard_total_cost: 192,
            vfx: FangyuanObjectClassBudget::new(8, 20, 56, 72),
            skill: FangyuanObjectClassBudget::new(6, 10, 72, 96),
            equipment: FangyuanObjectClassBudget::new(3, 6, 24, 36),
            npc: FangyuanObjectClassBudget::new(6, 10, 36, 48),
            tiandao: FangyuanObjectClassBudget::new(6, 10, 36, 48),
        }
    }

    pub fn strict_trial() -> Self {
        Self {
            recommended_total_cost: 32,
            hard_total_cost: 48,
            vfx: FangyuanObjectClassBudget::new(2, 4, 12, 18),
            skill: FangyuanObjectClassBudget::new(1, 2, 12, 18),
            equipment: FangyuanObjectClassBudget::new(1, 2, 4, 8),
            npc: FangyuanObjectClassBudget::new(1, 2, 4, 8),
            tiandao: FangyuanObjectClassBudget::new(1, 2, 4, 8),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanObjectBudgetEntry {
    pub object_id: String,
    pub class: FangyuanObjectClass,
    pub budget_cost: u32,
    pub active_vfx_count: usize,
    pub template_id: Option<String>,
    pub visual_id: Option<String>,
    pub audit_report: Option<FangyuanAuditReport>,
}

impl FangyuanObjectBudgetEntry {
    pub fn new(object_id: impl Into<String>, class: FangyuanObjectClass, budget_cost: u32) -> Self {
        Self {
            object_id: object_id.into(),
            class,
            budget_cost,
            active_vfx_count: 0,
            template_id: None,
            visual_id: None,
            audit_report: None,
        }
    }

    pub fn with_active_vfx_count(mut self, active_vfx_count: usize) -> Self {
        self.active_vfx_count = active_vfx_count;
        self
    }

    pub fn with_template_id(mut self, template_id: impl Into<String>) -> Self {
        self.template_id = Some(template_id.into());
        self
    }

    pub fn with_visual_id(mut self, visual_id: impl Into<String>) -> Self {
        self.visual_id = Some(visual_id.into());
        self
    }

    pub fn with_audit_report(mut self, audit_report: FangyuanAuditReport) -> Self {
        self.audit_report = Some(audit_report);
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FangyuanObjectBudgetSnapshot {
    pub entries: Vec<FangyuanObjectBudgetEntry>,
}

impl FangyuanObjectBudgetSnapshot {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_entries(entries: Vec<FangyuanObjectBudgetEntry>) -> Self {
        Self { entries }
    }

    pub fn push(&mut self, entry: FangyuanObjectBudgetEntry) {
        self.entries.push(entry);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn total_cost(&self) -> u32 {
        self.entries.iter().map(|entry| entry.budget_cost).sum()
    }

    pub fn total_count(&self) -> usize {
        self.entries.len()
    }

    pub fn active_vfx_count(&self) -> usize {
        self.entries
            .iter()
            .map(|entry| entry.active_vfx_count)
            .sum()
    }

    pub fn class_count(&self, class: FangyuanObjectClass) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.class == class)
            .count()
    }

    pub fn class_cost(&self, class: FangyuanObjectClass) -> u32 {
        self.entries
            .iter()
            .filter(|entry| entry.class == class)
            .map(|entry| entry.budget_cost)
            .sum()
    }

    pub fn first_template_id(&self) -> Option<&str> {
        self.entries
            .iter()
            .find_map(|entry| entry.template_id.as_deref())
    }

    pub fn first_visual_id(&self) -> Option<&str> {
        self.entries
            .iter()
            .find_map(|entry| entry.visual_id.as_deref())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FangyuanObjectBudgetSummary {
    pub total_count: usize,
    pub total_cost: u32,
    pub active_vfx_count: usize,
    pub vfx_count: usize,
    pub skill_count: usize,
    pub equipment_count: usize,
    pub npc_count: usize,
    pub tiandao_count: usize,
    pub template_id: String,
    pub visual_id: String,
    pub audit_status: String,
    pub audit_error_count: usize,
    pub audit_warning_count: usize,
    pub finding_summary: String,
}

impl FangyuanObjectBudgetSummary {
    fn from_snapshot_and_report(
        snapshot: &FangyuanObjectBudgetSnapshot,
        report: &FangyuanAuditReport,
    ) -> Self {
        Self {
            total_count: snapshot.total_count(),
            total_cost: snapshot.total_cost(),
            active_vfx_count: snapshot.active_vfx_count(),
            vfx_count: snapshot.class_count(FangyuanObjectClass::Vfx),
            skill_count: snapshot.class_count(FangyuanObjectClass::Skill),
            equipment_count: snapshot.class_count(FangyuanObjectClass::Equipment),
            npc_count: snapshot.class_count(FangyuanObjectClass::Npc),
            tiandao_count: snapshot.class_count(FangyuanObjectClass::Tiandao),
            template_id: snapshot.first_template_id().unwrap_or("-").to_string(),
            visual_id: snapshot.first_visual_id().unwrap_or("-").to_string(),
            audit_status: object_audit_status_label(report.status).to_string(),
            audit_error_count: report.summary.error_count,
            audit_warning_count: report.summary.warning_count,
            finding_summary: fangyuan_object_budget_finding_summary(report),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanObjectBudgetDegradeSuggestion {
    pub target: FangyuanObjectDegradeTarget,
    pub class: FangyuanObjectClass,
    pub priority: u8,
    pub estimated_cost_savings: u32,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanObjectBudgetAudit {
    pub report: FangyuanAuditReport,
    pub summary: FangyuanObjectBudgetSummary,
    pub degrade_suggestions: Vec<FangyuanObjectBudgetDegradeSuggestion>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FangyuanTrialBlueprintDomain {
    #[default]
    Home,
    Equipment,
    Skill,
    Appearance,
}

impl FangyuanTrialBlueprintDomain {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Equipment => "equipment",
            Self::Skill => "skill",
            Self::Appearance => "appearance",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanTrialBlueprintSelection {
    pub domain: FangyuanTrialBlueprintDomain,
    pub requested_id: String,
    pub selected_id: String,
    pub label: String,
    pub source: String,
}

impl FangyuanTrialBlueprintSelection {
    pub fn new(
        domain: FangyuanTrialBlueprintDomain,
        requested_id: impl Into<String>,
        selected_id: impl Into<String>,
        label: impl Into<String>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            domain,
            requested_id: requested_id.into(),
            selected_id: selected_id.into(),
            label: label.into(),
            source: source.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FangyuanTrialBudgetProfileKind {
    Relaxed,
    #[default]
    Standard,
    Strict,
}

impl FangyuanTrialBudgetProfileKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Relaxed => "relaxed",
            Self::Standard => "standard",
            Self::Strict => "strict",
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::Relaxed => Self::Standard,
            Self::Standard => Self::Strict,
            Self::Strict => Self::Relaxed,
        }
    }

    pub fn profile(self) -> FangyuanObjectBudgetProfile {
        match self {
            Self::Relaxed => FangyuanObjectBudgetProfile::relaxed_trial(),
            Self::Standard => FangyuanObjectBudgetProfile::default(),
            Self::Strict => FangyuanObjectBudgetProfile::strict_trial(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FangyuanTrialResultCounts {
    pub kept: usize,
    pub degraded: usize,
    pub rejected: usize,
}

impl FangyuanTrialResultCounts {
    pub const fn total(self) -> usize {
        self.kept + self.degraded + self.rejected
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FangyuanTrialFallbackState {
    pub missing_count: usize,
    pub label: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FangyuanTrialAuditPresentation {
    pub status: String,
    pub error_count: usize,
    pub warning_count: usize,
    pub suggestion_count: usize,
    pub budget_label: String,
    pub budget_cost: u32,
    pub budget_recommended: u32,
    pub budget_hard: u32,
    pub before_label: String,
    pub after_label: String,
    pub result_counts: FangyuanTrialResultCounts,
    pub fallback: FangyuanTrialFallbackState,
    pub plain_reasons: Vec<String>,
    pub primary_finding: String,
    pub primary_suggestion: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanObjectTrialVisualPrimitive {
    pub class: FangyuanObjectClass,
    pub object_id: String,
    pub primitive_index: usize,
    pub primitive: FangyuanPrimitive,
}

pub fn audit_fangyuan_object_budget(
    snapshot: &FangyuanObjectBudgetSnapshot,
    profile: &FangyuanObjectBudgetProfile,
) -> FangyuanObjectBudgetAudit {
    let mut report = FangyuanAuditReport::new(FangyuanAuditSourceKind::ObjectBudget, None);

    append_nested_audit_reports(&mut report, snapshot);
    add_total_budget_findings(&mut report, snapshot, profile);
    for class in [
        FangyuanObjectClass::Vfx,
        FangyuanObjectClass::Skill,
        FangyuanObjectClass::Equipment,
        FangyuanObjectClass::Npc,
        FangyuanObjectClass::Tiandao,
    ] {
        add_class_budget_findings(&mut report, snapshot, profile, class);
    }

    report.refresh_summary_and_status();
    report.sort_findings();
    report.sort_suggestions();
    let degrade_suggestions = fangyuan_object_budget_degrade_suggestions(snapshot, profile);
    let summary = FangyuanObjectBudgetSummary::from_snapshot_and_report(snapshot, &report);

    FangyuanObjectBudgetAudit {
        report,
        summary,
        degrade_suggestions,
    }
}

pub fn fangyuan_object_budget_degrade_suggestions(
    snapshot: &FangyuanObjectBudgetSnapshot,
    profile: &FangyuanObjectBudgetProfile,
) -> Vec<FangyuanObjectBudgetDegradeSuggestion> {
    if !is_object_budget_under_pressure(snapshot, profile) {
        return Vec::new();
    }

    let mut suggestions = Vec::new();
    for target in [
        FangyuanObjectDegradeTarget::NpcDecoration,
        FangyuanObjectDegradeTarget::TiandaoTemporaryResidue,
        FangyuanObjectDegradeTarget::EquipmentAura,
        FangyuanObjectDegradeTarget::SkillPersonality,
    ] {
        let class = target.class();
        let class_count = snapshot.class_count(class);
        if class_count == 0 {
            continue;
        }

        suggestions.push(FangyuanObjectBudgetDegradeSuggestion {
            target,
            class,
            priority: target.priority(),
            estimated_cost_savings: target.estimated_cost_savings_per_object()
                * class_count as u32,
            reason: format!(
                "degrade {} before higher-priority object classes under Fangyuan object budget pressure",
                target.as_str()
            ),
        });
    }
    suggestions.sort_by_key(|suggestion| suggestion.priority);
    suggestions
}

pub fn fangyuan_object_budget_entry_from_vfx_recipe(
    recipe: &FangyuanVfxRecipe,
) -> FangyuanObjectBudgetEntry {
    let estimate = estimate_fangyuan_vfx_recipe_budget(recipe);
    FangyuanObjectBudgetEntry::new(
        recipe.id.clone(),
        FangyuanObjectClass::Vfx,
        estimate.peak_primitives as u32,
    )
    .with_active_vfx_count(1)
    .with_audit_report(audit_fangyuan_vfx_recipe(recipe))
}

pub fn fangyuan_object_budget_entry_from_skill_visual(
    template: &FangyuanSkillTemplate,
    visual: &FangyuanSkillVisualBlueprint,
) -> FangyuanObjectBudgetEntry {
    let mut report = FangyuanAuditReport::new(FangyuanAuditSourceKind::ObjectBudget, None);
    for diagnostic in audit_fangyuan_skill_visual_readability(template, visual).diagnostics {
        let mut finding = FangyuanAuditFinding::new(
            FangyuanAuditSeverity::Warning,
            fangyuan_skill_audit_diagnostic_code(diagnostic.code),
            diagnostic.message,
            FangyuanAuditSourceKind::ObjectBudget,
        );
        finding.field_path = diagnostic.field_path;
        report.add_finding(finding);
    }
    report.refresh_summary_and_status();
    report.sort_findings();

    let vfx_cost = visual
        .vfx_recipe
        .as_ref()
        .map(estimate_fangyuan_vfx_recipe_budget)
        .map(|estimate| estimate.peak_primitives as u32)
        .unwrap_or(1);
    let budget_cost = vfx_cost + u32::from(visual.readability.transparent_primitive_budget);

    FangyuanObjectBudgetEntry::new(visual.id.clone(), FangyuanObjectClass::Skill, budget_cost)
        .with_template_id(visual.template_id.clone())
        .with_visual_id(visual.id.clone())
        .with_audit_report(report)
}

pub fn fangyuan_object_budget_entry_from_equipment_blueprint(
    blueprint: &FangyuanEquipmentBlueprint,
) -> FangyuanObjectBudgetEntry {
    let budget_cost = blueprint
        .compile_runtime()
        .map(|runtime| runtime.primitive_set.len() as u32)
        .unwrap_or(0);
    FangyuanObjectBudgetEntry::new(
        blueprint.id.clone(),
        FangyuanObjectClass::Equipment,
        budget_cost,
    )
    .with_audit_report(blueprint.audit_with_default_budget())
}

pub fn fangyuan_object_budget_entry_from_npc_blueprint(
    blueprint: &FangyuanNpcBlueprint,
    degrade_level: FangyuanNpcDegradeLevel,
) -> FangyuanObjectBudgetEntry {
    let budget_cost = blueprint
        .compile_for(FangyuanNpcCompileOptions::degraded(degrade_level))
        .map(|primitive_set| primitive_set.len() as u32)
        .unwrap_or(0);
    FangyuanObjectBudgetEntry::new(blueprint.id.clone(), FangyuanObjectClass::Npc, budget_cost)
        .with_audit_report(blueprint.audit_with_default_budget())
}

pub fn fangyuan_object_budget_entry_from_tiandao_manifestation(
    manifestation: &FangyuanTiandaoManifestation,
) -> FangyuanObjectBudgetEntry {
    let mut report = FangyuanAuditReport::new(FangyuanAuditSourceKind::ObjectBudget, None);
    if let Err(error) = manifestation.validate() {
        let mut finding = FangyuanAuditFinding::new(
            FangyuanAuditSeverity::Error,
            error.code(),
            "tiandao manifestation validation failed",
            FangyuanAuditSourceKind::ObjectBudget,
        );
        finding.field_path = Some("tiandao".to_string());
        report.add_finding(finding);
    }
    if manifestation.lifecycle_state == FangyuanTiandaoLifecycleState::Recycle
        && manifestation.budget_cost > 0
    {
        let mut finding = FangyuanAuditFinding::new(
            FangyuanAuditSeverity::Warning,
            "object_budget_recycled_tiandao_keeps_cost",
            "recycled tiandao manifestation should release object budget",
            FangyuanAuditSourceKind::ObjectBudget,
        );
        finding.field_path = Some("tiandao.lifecycle_state".to_string());
        report.add_finding(finding);
    }
    report.refresh_summary_and_status();
    report.sort_findings();

    FangyuanObjectBudgetEntry::new(
        manifestation.id.clone(),
        FangyuanObjectClass::Tiandao,
        manifestation.budget_cost,
    )
    .with_audit_report(report)
}

pub fn fangyuan_default_object_trial_static_entries() -> Vec<FangyuanObjectBudgetEntry> {
    let templates = fangyuan_default_skill_templates();
    let visuals = fangyuan_default_skill_visual_blueprints();
    let mut entries = Vec::new();
    for visual in visuals {
        if let Some(template) = templates.iter().find(|template| {
            template.id == visual.template_id && template.version == visual.template_version
        }) {
            entries.push(fangyuan_object_budget_entry_from_skill_visual(
                template, &visual,
            ));
        }
    }
    entries.push(fangyuan_object_budget_entry_from_equipment_blueprint(
        &fangyuan_default_equipment_blueprint(),
    ));
    entries.push(fangyuan_object_budget_entry_from_npc_blueprint(
        &fangyuan_default_npc_blueprint(),
        FangyuanNpcDegradeLevel::Full,
    ));
    entries.push(fangyuan_object_budget_entry_from_tiandao_manifestation(
        &fangyuan_default_tiandao_manifestation(),
    ));
    entries
}

pub fn fangyuan_default_trial_blueprint_selections() -> Vec<FangyuanTrialBlueprintSelection> {
    vec![
        FangyuanTrialBlueprintSelection::new(
            FangyuanTrialBlueprintDomain::Home,
            "fangyuan/layouts/home_layout.ron",
            "fangyuan/layouts/home_layout.ron",
            "default home layout",
            "first_package",
        ),
        FangyuanTrialBlueprintSelection::new(
            FangyuanTrialBlueprintDomain::Equipment,
            "equipment.default_practice_blade",
            "equipment.default_practice_blade",
            "practice blade",
            "built_in",
        ),
        FangyuanTrialBlueprintSelection::new(
            FangyuanTrialBlueprintDomain::Skill,
            "skill.visual.projectile",
            "skill.visual.projectile",
            "projectile visual",
            "built_in",
        ),
        FangyuanTrialBlueprintSelection::new(
            FangyuanTrialBlueprintDomain::Appearance,
            "npc.default_wayfarer",
            "npc.default_wayfarer",
            "wayfarer appearance",
            "built_in",
        ),
    ]
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanObjectTrialSummary {
    pub route_id: String,
    pub selection_label: String,
    pub budget_profile: String,
    pub audit_run: u64,
    pub audit_status: String,
    pub audit_error_count: usize,
    pub audit_warning_count: usize,
    pub audit_suggestion_count: usize,
    pub active_vfx_count: usize,
    pub template_id: String,
    pub visual_id: String,
    pub equipment_count: usize,
    pub npc_count: usize,
    pub tiandao_count: usize,
    pub budget_cost: u32,
    pub budget_recommended: u32,
    pub budget_hard: u32,
    pub before_label: String,
    pub after_label: String,
    pub kept_count: usize,
    pub degraded_count: usize,
    pub rejected_count: usize,
    pub fallback_missing_count: usize,
    pub fallback_summary: String,
    pub plain_reason_summary: String,
    pub primary_suggestion: String,
    pub finding_summary: String,
}

impl Default for FangyuanObjectTrialSummary {
    fn default() -> Self {
        Self {
            route_id: FangyuanObjectTrialRoute::None.as_str().to_string(),
            selection_label: "-".to_string(),
            budget_profile: FangyuanTrialBudgetProfileKind::Standard
                .as_str()
                .to_string(),
            audit_run: 0,
            audit_status: "pending".to_string(),
            audit_error_count: 0,
            audit_warning_count: 0,
            audit_suggestion_count: 0,
            active_vfx_count: 0,
            template_id: "-".to_string(),
            visual_id: "-".to_string(),
            equipment_count: 0,
            npc_count: 0,
            tiandao_count: 0,
            budget_cost: 0,
            budget_recommended: FANGYUAN_OBJECT_BUDGET_DEFAULT_RECOMMENDED_TOTAL_COST,
            budget_hard: FANGYUAN_OBJECT_BUDGET_DEFAULT_HARD_TOTAL_COST,
            before_label: "0 objects cost 0".to_string(),
            after_label: "keep 0 degrade 0 reject 0".to_string(),
            kept_count: 0,
            degraded_count: 0,
            rejected_count: 0,
            fallback_missing_count: 0,
            fallback_summary: "ok".to_string(),
            plain_reason_summary: "ok".to_string(),
            primary_suggestion: "-".to_string(),
            finding_summary: "ok".to_string(),
        }
    }
}

impl FangyuanObjectTrialSummary {
    fn from_budget_summary(
        route_id: &str,
        selection_label: String,
        budget_profile: FangyuanTrialBudgetProfileKind,
        audit_run: u64,
        summary: &FangyuanObjectBudgetSummary,
        presentation: FangyuanTrialAuditPresentation,
    ) -> Self {
        Self {
            route_id: route_id.to_string(),
            selection_label,
            budget_profile: budget_profile.as_str().to_string(),
            audit_run,
            audit_status: presentation.status,
            audit_error_count: presentation.error_count,
            audit_warning_count: presentation.warning_count,
            audit_suggestion_count: presentation.suggestion_count,
            active_vfx_count: summary.active_vfx_count,
            template_id: summary.template_id.clone(),
            visual_id: summary.visual_id.clone(),
            equipment_count: summary.equipment_count,
            npc_count: summary.npc_count,
            tiandao_count: summary.tiandao_count,
            budget_cost: summary.total_cost,
            budget_recommended: presentation.budget_recommended,
            budget_hard: presentation.budget_hard,
            before_label: presentation.before_label,
            after_label: presentation.after_label,
            kept_count: presentation.result_counts.kept,
            degraded_count: presentation.result_counts.degraded,
            rejected_count: presentation.result_counts.rejected,
            fallback_missing_count: presentation.fallback.missing_count,
            fallback_summary: presentation.fallback.label,
            plain_reason_summary: join_trial_plain_reasons(&presentation.plain_reasons),
            primary_suggestion: presentation.primary_suggestion,
            finding_summary: summary.finding_summary.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FangyuanObjectTrialRoute {
    #[default]
    None,
    HomeDebugTrial,
}

impl FangyuanObjectTrialRoute {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::HomeDebugTrial => FANGYUAN_OBJECT_TRIAL_ROUTE_ID,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum FangyuanObjectTrialError {
    StartVfx(FangyuanVfxInstanceStartError),
    TickVfx(FangyuanVfxDiagnostic),
}

#[derive(Debug, Resource)]
pub struct FangyuanObjectTrialRuntime {
    route: FangyuanObjectTrialRoute,
    vfx_runtime: FangyuanVfxRuntime,
    budget_profile: FangyuanTrialBudgetProfileKind,
    profile: FangyuanObjectBudgetProfile,
    selections: Vec<FangyuanTrialBlueprintSelection>,
    static_entries: Vec<FangyuanObjectBudgetEntry>,
    active_vfx_recipes: Vec<FangyuanVfxRecipe>,
    audit: FangyuanObjectBudgetAudit,
    summary: FangyuanObjectTrialSummary,
    audit_run: u64,
}

impl Default for FangyuanObjectTrialRuntime {
    fn default() -> Self {
        let snapshot = FangyuanObjectBudgetSnapshot::default();
        let profile = FangyuanObjectBudgetProfile::default();
        let audit = audit_fangyuan_object_budget(&snapshot, &profile);
        Self {
            route: FangyuanObjectTrialRoute::None,
            vfx_runtime: FangyuanVfxRuntime::default(),
            budget_profile: FangyuanTrialBudgetProfileKind::Standard,
            profile,
            selections: Vec::new(),
            static_entries: Vec::new(),
            active_vfx_recipes: Vec::new(),
            summary: FangyuanObjectTrialSummary::default(),
            audit,
            audit_run: 0,
        }
    }
}

impl FangyuanObjectTrialRuntime {
    pub fn with_profile(profile: FangyuanObjectBudgetProfile) -> Self {
        let snapshot = FangyuanObjectBudgetSnapshot::default();
        let audit = audit_fangyuan_object_budget(&snapshot, &profile);
        Self {
            budget_profile: FangyuanTrialBudgetProfileKind::Standard,
            profile,
            audit,
            ..Default::default()
        }
    }

    pub fn enter_default_showcase(
        &mut self,
        start_tick: u64,
    ) -> Result<FangyuanObjectTrialSummary, FangyuanObjectTrialError> {
        self.clear_scene();
        self.route = FangyuanObjectTrialRoute::HomeDebugTrial;
        self.selections = fangyuan_default_trial_blueprint_selections();
        self.static_entries = fangyuan_default_object_trial_static_entries();
        self.active_vfx_recipes = fangyuan_object_trial_vfx_recipes();
        for recipe in self.active_vfx_recipes.clone() {
            let instance_id = format!("trial.{}", recipe.id);
            let event_id = format!("trial.{}", recipe.id);
            let instance = FangyuanVfxInstance::new(
                instance_id,
                recipe,
                FangyuanVfxReplayContext::local("fangyuan_trial", event_id),
                start_tick,
            );
            self.vfx_runtime
                .start_instance(instance)
                .map_err(FangyuanObjectTrialError::StartVfx)?;
        }
        self.tick(start_tick)
    }

    pub fn reload_default_showcase(
        &mut self,
        start_tick: u64,
    ) -> Result<FangyuanObjectTrialSummary, FangyuanObjectTrialError> {
        self.enter_default_showcase(start_tick)
    }

    pub fn rerun_audit(&mut self) -> FangyuanObjectTrialSummary {
        self.refresh_audit();
        self.summary.clone()
    }

    pub fn switch_budget_profile(&mut self) -> FangyuanObjectTrialSummary {
        self.budget_profile = self.budget_profile.next();
        self.profile = self.budget_profile.profile();
        self.refresh_audit();
        self.summary.clone()
    }

    pub fn set_budget_profile(
        &mut self,
        budget_profile: FangyuanTrialBudgetProfileKind,
    ) -> FangyuanObjectTrialSummary {
        self.budget_profile = budget_profile;
        self.profile = budget_profile.profile();
        self.refresh_audit();
        self.summary.clone()
    }

    pub fn tick(
        &mut self,
        current_tick: u64,
    ) -> Result<FangyuanObjectTrialSummary, FangyuanObjectTrialError> {
        self.vfx_runtime
            .tick(current_tick, FANGYUAN_OBJECT_TRIAL_TICKS_PER_SECOND)
            .map_err(FangyuanObjectTrialError::TickVfx)?;
        self.refresh_audit();
        Ok(self.summary.clone())
    }

    pub fn clear_scene(&mut self) {
        self.route = FangyuanObjectTrialRoute::None;
        self.vfx_runtime.clear_scene();
        self.selections.clear();
        self.static_entries.clear();
        self.active_vfx_recipes.clear();
        self.refresh_audit();
        self.audit_run = 0;
        self.summary.audit_run = 0;
    }

    pub fn exit_scene(&mut self) {
        self.clear_scene();
    }

    pub fn summary(&self) -> &FangyuanObjectTrialSummary {
        &self.summary
    }

    pub fn audit(&self) -> &FangyuanObjectBudgetAudit {
        &self.audit
    }

    pub fn budget_profile(&self) -> FangyuanTrialBudgetProfileKind {
        self.budget_profile
    }

    pub fn selections(&self) -> &[FangyuanTrialBlueprintSelection] {
        &self.selections
    }

    pub fn audit_presentation(&self) -> FangyuanTrialAuditPresentation {
        fangyuan_trial_audit_presentation(
            &self.audit,
            &self.profile,
            self.budget_profile,
            &self.selections,
        )
    }

    pub fn visual_primitives(&self) -> Vec<FangyuanObjectTrialVisualPrimitive> {
        if self.route == FangyuanObjectTrialRoute::None {
            return Vec::new();
        }

        fangyuan_object_trial_visual_primitives(&self.vfx_runtime)
    }

    fn refresh_audit(&mut self) {
        let mut entries = self.static_entries.clone();
        if self.vfx_runtime.stats().active_instance_count > 0 {
            entries.extend(
                self.active_vfx_recipes
                    .iter()
                    .map(fangyuan_object_budget_entry_from_vfx_recipe),
            );
        }
        let snapshot = FangyuanObjectBudgetSnapshot::from_entries(entries);
        self.audit = audit_fangyuan_object_budget(&snapshot, &self.profile);
        self.audit_run += 1;
        let presentation = fangyuan_trial_audit_presentation(
            &self.audit,
            &self.profile,
            self.budget_profile,
            &self.selections,
        );
        self.summary = FangyuanObjectTrialSummary::from_budget_summary(
            self.route.as_str(),
            fangyuan_trial_selection_label(&self.selections),
            self.budget_profile,
            self.audit_run,
            &self.audit.summary,
            presentation,
        );
    }
}

pub fn fangyuan_trial_audit_presentation(
    audit: &FangyuanObjectBudgetAudit,
    profile: &FangyuanObjectBudgetProfile,
    budget_profile: FangyuanTrialBudgetProfileKind,
    selections: &[FangyuanTrialBlueprintSelection],
) -> FangyuanTrialAuditPresentation {
    let result_counts = fangyuan_trial_result_counts(audit);
    let fallback = fangyuan_trial_fallback_state(selections);
    let plain_reasons = fangyuan_trial_plain_reasons(audit);
    let primary_finding = audit
        .report
        .findings
        .iter()
        .find(|finding| finding.severity == FangyuanAuditSeverity::Error)
        .or_else(|| {
            audit
                .report
                .findings
                .iter()
                .find(|finding| finding.severity == FangyuanAuditSeverity::Warning)
        })
        .or_else(|| audit.report.findings.first())
        .map(|finding| finding.code.clone())
        .unwrap_or_else(|| "-".to_string());
    let primary_suggestion = audit
        .degrade_suggestions
        .first()
        .map(|suggestion| suggestion.target.as_str().to_string())
        .or_else(|| {
            audit
                .report
                .suggestions
                .first()
                .map(|suggestion| format!("{:?}", suggestion.action))
        })
        .or_else(|| {
            audit
                .report
                .findings
                .iter()
                .find_map(|finding| fangyuan_trial_plain_suggestion_for_code(&finding.code))
                .map(str::to_string)
        })
        .unwrap_or_else(|| "-".to_string());
    let suggestion_count = audit.report.suggestions.len()
        + audit.degrade_suggestions.len()
        + usize::from(
            audit.report.suggestions.is_empty()
                && audit.degrade_suggestions.is_empty()
                && !audit.report.findings.is_empty(),
        );

    FangyuanTrialAuditPresentation {
        status: object_audit_status_label(audit.report.status).to_string(),
        error_count: audit.report.summary.error_count,
        warning_count: audit.report.summary.warning_count,
        suggestion_count,
        budget_label: budget_profile.as_str().to_string(),
        budget_cost: audit.summary.total_cost,
        budget_recommended: profile.recommended_total_cost,
        budget_hard: profile.hard_total_cost,
        before_label: format!(
            "{} objects cost {}",
            audit.summary.total_count, audit.summary.total_cost
        ),
        after_label: format!(
            "keep {} degrade {} reject {}",
            result_counts.kept, result_counts.degraded, result_counts.rejected
        ),
        result_counts,
        fallback,
        plain_reasons,
        primary_finding,
        primary_suggestion,
    }
}

pub fn fangyuan_trial_result_counts(
    audit: &FangyuanObjectBudgetAudit,
) -> FangyuanTrialResultCounts {
    let total = audit.summary.total_count;
    if total == 0 {
        return FangyuanTrialResultCounts::default();
    }

    let degraded = audit.degrade_suggestions.len().min(total);
    let rejected = if audit.report.status == FangyuanAuditStatus::Failed {
        audit
            .report
            .summary
            .error_count
            .min(total.saturating_sub(degraded))
    } else {
        0
    };
    FangyuanTrialResultCounts {
        kept: total.saturating_sub(degraded + rejected),
        degraded,
        rejected,
    }
}

pub fn fangyuan_trial_plain_reasons(audit: &FangyuanObjectBudgetAudit) -> Vec<String> {
    let mut reasons = Vec::new();
    for finding in &audit.report.findings {
        if let Some(reason) = fangyuan_trial_plain_reason_for_code(&finding.code) {
            push_unique_reason(&mut reasons, reason);
        }
    }
    for suggestion in &audit.degrade_suggestions {
        let reason = match suggestion.target {
            FangyuanObjectDegradeTarget::NpcDecoration => "NPC 装饰先降级，保留核心玩法对象",
            FangyuanObjectDegradeTarget::TiandaoTemporaryResidue => {
                "天道残留可先隐藏，避免临时效果挤占预算"
            }
            FangyuanObjectDegradeTarget::EquipmentAura => "装备光环可减弱，优先保留装备主体",
            FangyuanObjectDegradeTarget::SkillPersonality => {
                "技能个性化特效可降级，规则层必须保持可读"
            }
        };
        push_unique_reason(&mut reasons, reason);
    }

    if reasons.is_empty() {
        reasons.push("预算正常，无需降级".to_string());
    }
    reasons
}

fn fangyuan_trial_plain_reason_for_code(code: &str) -> Option<&'static str> {
    match code {
        "primitive_count_above_recommended"
        | "primitive_count_above_hard_limit"
        | "object_budget_total_cost_above_recommended"
        | "object_budget_total_cost_above_hard_limit"
        | "object_budget_vfx_count_above_recommended"
        | "object_budget_vfx_count_above_hard_limit"
        | "object_budget_skill_count_above_recommended"
        | "object_budget_skill_count_above_hard_limit"
        | "object_budget_equipment_count_above_recommended"
        | "object_budget_equipment_count_above_hard_limit"
        | "object_budget_npc_count_above_recommended"
        | "object_budget_npc_count_above_hard_limit"
        | "object_budget_tiandao_count_above_recommended"
        | "object_budget_tiandao_count_above_hard_limit" => {
            Some("primitive 过多，低优先级装饰会先降级")
        }
        "alpha_count_above_recommended"
        | "alpha_count_above_hard_limit"
        | "transparent_count_above_recommended"
        | "transparent_count_above_hard_limit"
        | "skill_transparent_budget_exceeded" => Some("透明过量，容易造成排序和混合压力"),
        "emissive_count_above_recommended"
        | "emissive_count_above_hard_limit"
        | "emissive_intensity_above_limit"
        | "skill_emissive_budget_exceeded" => Some("发光过强，移动端亮度和性能风险较高"),
        "skill_rule_layer_occluded" => Some("规则层被遮挡，命中范围需要优先可见"),
        "skill_color_conflict" => Some("技能颜色和规则层冲突，需要提高可读性"),
        "skill_visual_range_missing"
        | "skill_visual_range_too_small"
        | "skill_visual_range_mismatch" => Some("技能视觉范围不清晰，可能误导玩家判断"),
        "skill_decor_bounds_exceeded" => Some("技能装饰超出规则范围，需收敛到可读区域"),
        "material_profile_count_above_recommended" | "material_profile_count_above_hard_limit" => {
            Some("材质种类过多，会增加渲染分支")
        }
        _ => None,
    }
}

fn fangyuan_trial_plain_suggestion_for_code(code: &str) -> Option<&'static str> {
    match code {
        "primitive_count_above_recommended"
        | "primitive_count_above_hard_limit"
        | "object_budget_total_cost_above_recommended"
        | "object_budget_total_cost_above_hard_limit"
        | "object_budget_vfx_count_above_recommended"
        | "object_budget_vfx_count_above_hard_limit"
        | "object_budget_skill_count_above_recommended"
        | "object_budget_skill_count_above_hard_limit"
        | "object_budget_equipment_count_above_recommended"
        | "object_budget_equipment_count_above_hard_limit"
        | "object_budget_npc_count_above_recommended"
        | "object_budget_npc_count_above_hard_limit"
        | "object_budget_tiandao_count_above_recommended"
        | "object_budget_tiandao_count_above_hard_limit" => {
            Some("减少低优先级 primitive 或切换宽松预算")
        }
        "alpha_count_above_recommended"
        | "alpha_count_above_hard_limit"
        | "transparent_count_above_recommended"
        | "transparent_count_above_hard_limit"
        | "skill_transparent_budget_exceeded" => Some("减少透明层或改为不透明替代"),
        "emissive_count_above_recommended"
        | "emissive_count_above_hard_limit"
        | "emissive_intensity_above_limit"
        | "skill_emissive_budget_exceeded" => Some("降低发光数量或强度"),
        "skill_rule_layer_occluded" => Some("把规则层提到视觉装饰之上"),
        "skill_color_conflict" => Some("调整技能颜色，提升规则层对比度"),
        "skill_visual_range_missing"
        | "skill_visual_range_too_small"
        | "skill_visual_range_mismatch" => Some("补齐技能范围可视边界"),
        "skill_decor_bounds_exceeded" => Some("收缩技能装饰范围"),
        "material_profile_count_above_recommended" | "material_profile_count_above_hard_limit" => {
            Some("合并相近材质 profile")
        }
        _ => None,
    }
}

fn fangyuan_trial_fallback_state(
    selections: &[FangyuanTrialBlueprintSelection],
) -> FangyuanTrialFallbackState {
    let mut missing = Vec::new();
    for selection in selections {
        if selection.requested_id != selection.selected_id {
            missing.push(format!(
                "{}:{}->{}",
                selection.domain.as_str(),
                selection.requested_id,
                selection.selected_id
            ));
        }
    }

    if missing.is_empty() {
        FangyuanTrialFallbackState {
            missing_count: 0,
            label: "ok".to_string(),
        }
    } else {
        FangyuanTrialFallbackState {
            missing_count: missing.len(),
            label: missing.join("|"),
        }
    }
}

fn fangyuan_trial_selection_label(selections: &[FangyuanTrialBlueprintSelection]) -> String {
    if selections.is_empty() {
        return "-".to_string();
    }

    selections
        .iter()
        .map(|selection| format!("{}:{}", selection.domain.as_str(), selection.selected_id))
        .collect::<Vec<_>>()
        .join(",")
}

fn join_trial_plain_reasons(reasons: &[String]) -> String {
    if reasons.is_empty() {
        return "ok".to_string();
    }
    reasons.join("|")
}

fn push_unique_reason(reasons: &mut Vec<String>, reason: &str) {
    if !reasons.iter().any(|existing| existing == reason) {
        reasons.push(reason.to_string());
    }
}

fn fangyuan_object_trial_visual_primitives(
    vfx_runtime: &FangyuanVfxRuntime,
) -> Vec<FangyuanObjectTrialVisualPrimitive> {
    let mut primitives = Vec::new();

    primitives.extend(vfx_runtime.active_states().iter().enumerate().map(
        |(primitive_index, state)| {
            let object_id = state
                .source
                .instance_id
                .as_ref()
                .map(|instance_id| instance_id.as_str())
                .unwrap_or(state.recipe_id.as_str())
                .to_string();
            FangyuanObjectTrialVisualPrimitive {
                class: FangyuanObjectClass::Vfx,
                object_id,
                primitive_index,
                primitive: offset_trial_primitive(
                    state.to_runtime_primitive(),
                    Vec3::new(-8.0, 0.0, -4.0),
                ),
            }
        },
    ));

    if let Ok(runtime) = fangyuan_default_equipment_blueprint().compile_runtime() {
        primitives.extend(
            runtime
                .primitive_set
                .into_primitives()
                .into_iter()
                .enumerate()
                .map(
                    |(primitive_index, primitive)| FangyuanObjectTrialVisualPrimitive {
                        class: FangyuanObjectClass::Equipment,
                        object_id: "equipment.default_practice_blade".to_string(),
                        primitive_index,
                        primitive: offset_trial_primitive(primitive, Vec3::new(-5.5, 0.0, 4.0)),
                    },
                ),
        );
    }

    if let Ok(primitive_set) = fangyuan_default_npc_blueprint().compile() {
        primitives.extend(primitive_set.into_primitives().into_iter().enumerate().map(
            |(primitive_index, primitive)| FangyuanObjectTrialVisualPrimitive {
                class: FangyuanObjectClass::Npc,
                object_id: "npc.default_wayfarer".to_string(),
                primitive_index,
                primitive: offset_trial_primitive(primitive, Vec3::new(0.0, 0.0, 4.0)),
            },
        ));
    }

    let manifestation = fangyuan_default_tiandao_manifestation();
    primitives.push(FangyuanObjectTrialVisualPrimitive {
        class: FangyuanObjectClass::Tiandao,
        object_id: manifestation.id.clone(),
        primitive_index: 0,
        primitive: offset_trial_primitive(
            fangyuan_tiandao_trial_marker(&manifestation),
            Vec3::new(5.0, 0.0, 4.0),
        ),
    });

    primitives
}

fn offset_trial_primitive(mut primitive: FangyuanPrimitive, offset: Vec3) -> FangyuanPrimitive {
    primitive.local_position += offset;
    primitive
}

fn fangyuan_tiandao_trial_marker(
    manifestation: &FangyuanTiandaoManifestation,
) -> FangyuanPrimitive {
    let score = manifestation.solidify_score.clamp(0.0, 1.0);
    let alpha = match manifestation.lifecycle_state {
        FangyuanTiandaoLifecycleState::Manifest => 0.62,
        FangyuanTiandaoLifecycleState::Decay => 0.38,
        FangyuanTiandaoLifecycleState::Solidify => 0.92,
        FangyuanTiandaoLifecycleState::Recycle => 0.18,
    };
    let radius = 0.42 + score * 0.28;
    FangyuanPrimitive::with_runtime_metadata(
        FangyuanPrimitiveKind::Sphere,
        Vec3::new(0.0, 0.55, 0.0),
        Vec3::new(radius, 0.16, radius),
        Color::srgba(0.48, 0.9, 0.76, alpha),
        FangyuanPrimitiveRole::Archive,
        alpha,
        0.45 + score,
        None,
        FangyuanPrimitiveLifecycle::new(Some(u64::from(manifestation.ttl)), None, None),
    )
}

fn append_nested_audit_reports(
    report: &mut FangyuanAuditReport,
    snapshot: &FangyuanObjectBudgetSnapshot,
) {
    for entry in &snapshot.entries {
        let Some(entry_report) = entry.audit_report.as_ref() else {
            continue;
        };
        for finding in &entry_report.findings {
            let mut finding = finding.clone();
            finding.source_path = finding
                .source_path
                .or_else(|| Some(entry.object_id.clone()));
            if let Some(field_path) = finding.field_path.take() {
                finding.field_path = Some(format!("objects[{}].{field_path}", entry.object_id));
            } else {
                finding.field_path = Some(format!("objects[{}]", entry.object_id));
            }
            report.add_finding(finding);
        }
        for suggestion in &entry_report.suggestions {
            report.add_suggestion(suggestion.clone());
        }
    }
}

fn add_total_budget_findings(
    report: &mut FangyuanAuditReport,
    snapshot: &FangyuanObjectBudgetSnapshot,
    profile: &FangyuanObjectBudgetProfile,
) {
    let total_cost = snapshot.total_cost();
    if total_cost > profile.hard_total_cost {
        add_object_budget_finding(
            report,
            FangyuanAuditSeverity::Error,
            "object_budget_total_cost_above_hard_limit",
            "object_budget.total_cost",
            format!(
                "object budget total cost exceeds hard limit: {total_cost} > {}",
                profile.hard_total_cost
            ),
        );
    } else if total_cost > profile.recommended_total_cost {
        add_object_budget_finding(
            report,
            FangyuanAuditSeverity::Warning,
            "object_budget_total_cost_above_recommended",
            "object_budget.total_cost",
            format!(
                "object budget total cost exceeds recommended limit: {total_cost} > {}",
                profile.recommended_total_cost
            ),
        );
    }
}

fn add_class_budget_findings(
    report: &mut FangyuanAuditReport,
    snapshot: &FangyuanObjectBudgetSnapshot,
    profile: &FangyuanObjectBudgetProfile,
    class: FangyuanObjectClass,
) {
    let budget = profile.budget_for(class);
    let count = snapshot.class_count(class);
    let cost = snapshot.class_cost(class);
    let name = class.as_str();

    if count > budget.max_count {
        add_object_budget_finding(
            report,
            FangyuanAuditSeverity::Error,
            format!("object_budget_{name}_count_above_hard_limit"),
            format!("object_budget.{name}.count"),
            format!(
                "{name} object count exceeds hard limit: {count} > {}",
                budget.max_count
            ),
        );
    } else if count > budget.recommended_count {
        add_object_budget_finding(
            report,
            FangyuanAuditSeverity::Warning,
            format!("object_budget_{name}_count_above_recommended"),
            format!("object_budget.{name}.count"),
            format!(
                "{name} object count exceeds recommended limit: {count} > {}",
                budget.recommended_count
            ),
        );
    }

    if cost > budget.max_cost {
        add_object_budget_finding(
            report,
            FangyuanAuditSeverity::Error,
            format!("object_budget_{name}_cost_above_hard_limit"),
            format!("object_budget.{name}.cost"),
            format!(
                "{name} object cost exceeds hard limit: {cost} > {}",
                budget.max_cost
            ),
        );
    } else if cost > budget.recommended_cost {
        add_object_budget_finding(
            report,
            FangyuanAuditSeverity::Warning,
            format!("object_budget_{name}_cost_above_recommended"),
            format!("object_budget.{name}.cost"),
            format!(
                "{name} object cost exceeds recommended limit: {cost} > {}",
                budget.recommended_cost
            ),
        );
    }
}

fn add_object_budget_finding(
    report: &mut FangyuanAuditReport,
    severity: FangyuanAuditSeverity,
    code: impl Into<String>,
    field_path: impl Into<String>,
    reason: impl Into<String>,
) {
    let field_path = field_path.into();
    let reason = reason.into();
    let mut finding = FangyuanAuditFinding::new(
        severity,
        code,
        reason.clone(),
        FangyuanAuditSourceKind::ObjectBudget,
    );
    finding.field_path = Some(field_path.clone());
    report.add_finding(finding);
    report.add_suggestion(FangyuanAuditSuggestion::new_with_effect(
        FangyuanAuditSuggestionAction::ReducePrimitives,
        Some(field_path),
        reason,
        "apply object class degrade order: npc_decoration -> tiandao_temporary_residue -> equipment_aura -> skill_personality",
    ));
}

fn is_object_budget_under_pressure(
    snapshot: &FangyuanObjectBudgetSnapshot,
    profile: &FangyuanObjectBudgetProfile,
) -> bool {
    if snapshot.total_cost() > profile.recommended_total_cost {
        return true;
    }
    [
        FangyuanObjectClass::Vfx,
        FangyuanObjectClass::Skill,
        FangyuanObjectClass::Equipment,
        FangyuanObjectClass::Npc,
        FangyuanObjectClass::Tiandao,
    ]
    .into_iter()
    .any(|class| {
        let budget = profile.budget_for(class);
        snapshot.class_count(class) > budget.recommended_count
            || snapshot.class_cost(class) > budget.recommended_cost
    })
}

fn fangyuan_object_budget_finding_summary(report: &FangyuanAuditReport) -> String {
    if report.findings.is_empty() {
        return "ok".to_string();
    }
    let primary_code = report
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
        .map(|finding| finding.code.as_str())
        .unwrap_or("-");
    format!(
        "{} e{} w{} {}",
        object_audit_status_label(report.status),
        report.summary.error_count,
        report.summary.warning_count,
        primary_code
    )
}

fn object_audit_status_label(status: FangyuanAuditStatus) -> &'static str {
    match status {
        FangyuanAuditStatus::Passed => "passed",
        FangyuanAuditStatus::PassedWithWarnings => "warning",
        FangyuanAuditStatus::Failed => "failed",
    }
}

fn fangyuan_skill_audit_diagnostic_code(code: FangyuanSkillAuditDiagnosticCode) -> &'static str {
    match code {
        FangyuanSkillAuditDiagnosticCode::VisualRangeMissing => "skill_visual_range_missing",
        FangyuanSkillAuditDiagnosticCode::VisualRangeTooSmall => "skill_visual_range_too_small",
        FangyuanSkillAuditDiagnosticCode::VisualRangeMismatch => "skill_visual_range_mismatch",
        FangyuanSkillAuditDiagnosticCode::DecorBoundsExceeded => "skill_decor_bounds_exceeded",
        FangyuanSkillAuditDiagnosticCode::RuleLayerOccluded => "skill_rule_layer_occluded",
        FangyuanSkillAuditDiagnosticCode::ColorConflict => "skill_color_conflict",
        FangyuanSkillAuditDiagnosticCode::TransparentBudgetExceeded => {
            "skill_transparent_budget_exceeded"
        }
        FangyuanSkillAuditDiagnosticCode::EmissiveBudgetExceeded => {
            "skill_emissive_budget_exceeded"
        }
    }
}

fn fangyuan_object_trial_vfx_recipes() -> Vec<FangyuanVfxRecipe> {
    vec![
        fangyuan_vfx_projectile_recipe(),
        fangyuan_vfx_range_marker_recipe(),
        fangyuan_vfx_shield_recipe(),
        fangyuan_vfx_impact_expand_recipe(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fangyuan_object_budget_cross_type_total_budget_orders_hotspot_degrade_plan() {
        let snapshot = FangyuanObjectBudgetSnapshot::from_entries(vec![
            FangyuanObjectBudgetEntry::new("npc.decor", FangyuanObjectClass::Npc, 10),
            FangyuanObjectBudgetEntry::new("tiandao.residue", FangyuanObjectClass::Tiandao, 10),
            FangyuanObjectBudgetEntry::new("equipment.aura", FangyuanObjectClass::Equipment, 10),
            FangyuanObjectBudgetEntry::new("skill.personality", FangyuanObjectClass::Skill, 10),
        ]);
        let profile = FangyuanObjectBudgetProfile {
            recommended_total_cost: 8,
            hard_total_cost: 16,
            ..Default::default()
        };

        let audit = audit_fangyuan_object_budget(&snapshot, &profile);

        assert_eq!(audit.report.status, FangyuanAuditStatus::Failed);
        assert!(has_finding(
            &audit.report,
            "object_budget_total_cost_above_hard_limit"
        ));
        assert_eq!(
            audit
                .degrade_suggestions
                .iter()
                .map(|suggestion| suggestion.target)
                .collect::<Vec<_>>(),
            vec![
                FangyuanObjectDegradeTarget::NpcDecoration,
                FangyuanObjectDegradeTarget::TiandaoTemporaryResidue,
                FangyuanObjectDegradeTarget::EquipmentAura,
                FangyuanObjectDegradeTarget::SkillPersonality,
            ]
        );
    }

    #[test]
    fn fangyuan_object_budget_single_type_over_budget_reports_class_finding() {
        let snapshot = FangyuanObjectBudgetSnapshot::from_entries(vec![
            FangyuanObjectBudgetEntry::new("vfx.0", FangyuanObjectClass::Vfx, 1),
            FangyuanObjectBudgetEntry::new("vfx.1", FangyuanObjectClass::Vfx, 1),
            FangyuanObjectBudgetEntry::new("vfx.2", FangyuanObjectClass::Vfx, 1),
        ]);
        let profile = FangyuanObjectBudgetProfile {
            vfx: FangyuanObjectClassBudget::new(1, 2, 8, 16),
            ..Default::default()
        };

        let audit = audit_fangyuan_object_budget(&snapshot, &profile);

        assert_eq!(audit.report.status, FangyuanAuditStatus::Failed);
        assert!(has_finding(
            &audit.report,
            "object_budget_vfx_count_above_hard_limit"
        ));
        assert_eq!(audit.summary.vfx_count, 3);
    }

    #[test]
    fn fangyuan_object_budget_default_trial_collects_all_object_types_and_audit_summary() {
        let mut runtime = FangyuanObjectTrialRuntime::default();

        let summary = runtime.enter_default_showcase(0).unwrap();

        assert_eq!(summary.route_id, FANGYUAN_OBJECT_TRIAL_ROUTE_ID);
        assert!(summary.selection_label.contains("home:"));
        assert_eq!(summary.budget_profile, "standard");
        assert_eq!(summary.audit_run, 1);
        assert_eq!(summary.audit_status, "warning");
        assert_eq!(summary.audit_error_count, 0);
        assert_eq!(summary.audit_warning_count, 1);
        assert!(summary.audit_suggestion_count > 0);
        assert_eq!(summary.active_vfx_count, 4);
        assert_eq!(summary.template_id, "skill.template.projectile");
        assert_eq!(summary.visual_id, "skill.visual.projectile");
        assert_eq!(summary.equipment_count, 1);
        assert_eq!(summary.npc_count, 1);
        assert_eq!(summary.tiandao_count, 1);
        assert_eq!(
            summary.finding_summary,
            "warning e0 w1 skill_color_conflict"
        );
        assert_eq!(
            runtime.audit().report.status,
            FangyuanAuditStatus::PassedWithWarnings
        );
        assert_eq!(summary.kept_count, runtime.audit().summary.total_count);
        assert_eq!(summary.degraded_count, 0);
        assert_eq!(summary.rejected_count, 0);
        assert_eq!(summary.fallback_missing_count, 0);
        assert_eq!(summary.fallback_summary, "ok");
        assert!(summary.plain_reason_summary.contains("技能颜色"));

        let visual_primitives = runtime.visual_primitives();
        assert!(
            visual_primitives
                .iter()
                .any(|primitive| primitive.class == FangyuanObjectClass::Vfx)
        );
        assert!(
            visual_primitives
                .iter()
                .any(|primitive| primitive.class == FangyuanObjectClass::Equipment)
        );
        assert!(
            visual_primitives
                .iter()
                .any(|primitive| primitive.class == FangyuanObjectClass::Npc)
        );
        assert!(
            visual_primitives
                .iter()
                .any(|primitive| primitive.class == FangyuanObjectClass::Tiandao)
        );
    }

    #[test]
    fn fangyuan_trial_selection_audit_display_and_fallback_are_presented_for_players() {
        let mut runtime = FangyuanObjectTrialRuntime::default();

        let summary = runtime.enter_default_showcase(0).unwrap();
        let presentation = runtime.audit_presentation();

        assert_eq!(runtime.selections().len(), 4);
        assert!(
            runtime
                .selections()
                .iter()
                .any(|selection| selection.domain == FangyuanTrialBlueprintDomain::Home)
        );
        assert_eq!(presentation.status, "warning");
        assert_eq!(presentation.error_count, 0);
        assert_eq!(presentation.warning_count, 1);
        assert!(presentation.suggestion_count > 0);
        assert_eq!(presentation.budget_label, "standard");
        assert_eq!(presentation.budget_cost, summary.budget_cost);
        assert_eq!(presentation.budget_recommended, summary.budget_recommended);
        assert_eq!(presentation.budget_hard, summary.budget_hard);
        assert_eq!(presentation.fallback.missing_count, 0);
        assert_eq!(presentation.fallback.label, "ok");
        assert!(
            presentation
                .plain_reasons
                .iter()
                .any(|reason| reason.contains("技能颜色"))
        );
        assert_eq!(presentation.result_counts.rejected, 0);
    }

    #[test]
    fn fangyuan_trial_budget_profile_switch_updates_kept_degraded_rejected_results() {
        let mut runtime = FangyuanObjectTrialRuntime::default();

        let standard = runtime.enter_default_showcase(0).unwrap();
        let strict = runtime.set_budget_profile(FangyuanTrialBudgetProfileKind::Strict);

        assert_eq!(standard.budget_profile, "standard");
        assert_eq!(strict.budget_profile, "strict");
        assert_eq!(
            runtime.budget_profile(),
            FangyuanTrialBudgetProfileKind::Strict
        );
        assert_eq!(strict.audit_status, "failed");
        assert!(strict.audit_error_count > 0);
        assert!(strict.degraded_count > 0);
        assert!(strict.rejected_count > 0);
        assert!(
            strict.kept_count < strict.kept_count + strict.degraded_count + strict.rejected_count
        );
        assert!(strict.plain_reason_summary.contains("primitive 过多"));

        let rerun = runtime.rerun_audit();
        assert_eq!(rerun.budget_profile, "strict");
        assert!(rerun.audit_run > strict.audit_run);
    }

    #[test]
    fn fangyuan_object_budget_trial_route_reload_and_clear_leave_no_budget_residue() {
        let mut runtime = FangyuanObjectTrialRuntime::default();

        runtime.enter_default_showcase(0).unwrap();
        runtime.clear_scene();

        assert_eq!(runtime.summary().route_id, "none");
        assert_eq!(runtime.summary().active_vfx_count, 0);
        assert_eq!(runtime.summary().budget_cost, 0);
        assert_eq!(runtime.summary().audit_run, 0);
        assert_eq!(runtime.audit().summary.total_count, 0);
        assert!(runtime.selections().is_empty());
        assert!(runtime.visual_primitives().is_empty());

        let summary = runtime.reload_default_showcase(10).unwrap();
        assert_eq!(summary.route_id, FANGYUAN_OBJECT_TRIAL_ROUTE_ID);
        assert_eq!(summary.active_vfx_count, 4);
        assert_eq!(summary.audit_run, 1);
        assert!(!runtime.visual_primitives().is_empty());

        runtime.exit_scene();
        assert_eq!(runtime.summary().route_id, "none");
        assert_eq!(runtime.summary().active_vfx_count, 0);
        assert_eq!(runtime.summary().budget_cost, 0);
        assert_eq!(runtime.summary().audit_run, 0);
        assert!(runtime.selections().is_empty());
        assert!(runtime.visual_primitives().is_empty());
    }

    #[test]
    fn fangyuan_object_budget_class_retention_priority_keeps_skill_before_npc_decor() {
        assert!(
            FangyuanObjectClass::Skill.retention_priority()
                < FangyuanObjectClass::Npc.retention_priority()
        );
        assert!(
            FangyuanObjectClass::Vfx.retention_priority()
                < FangyuanObjectClass::Tiandao.retention_priority()
        );
    }

    fn has_finding(report: &FangyuanAuditReport, code: &str) -> bool {
        report.findings.iter().any(|finding| finding.code == code)
    }
}
