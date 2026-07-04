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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanObjectTrialSummary {
    pub route_id: String,
    pub active_vfx_count: usize,
    pub template_id: String,
    pub visual_id: String,
    pub equipment_count: usize,
    pub npc_count: usize,
    pub tiandao_count: usize,
    pub budget_cost: u32,
    pub finding_summary: String,
}

impl Default for FangyuanObjectTrialSummary {
    fn default() -> Self {
        Self {
            route_id: FangyuanObjectTrialRoute::None.as_str().to_string(),
            active_vfx_count: 0,
            template_id: "-".to_string(),
            visual_id: "-".to_string(),
            equipment_count: 0,
            npc_count: 0,
            tiandao_count: 0,
            budget_cost: 0,
            finding_summary: "ok".to_string(),
        }
    }
}

impl FangyuanObjectTrialSummary {
    fn from_budget_summary(route_id: &str, summary: &FangyuanObjectBudgetSummary) -> Self {
        Self {
            route_id: route_id.to_string(),
            active_vfx_count: summary.active_vfx_count,
            template_id: summary.template_id.clone(),
            visual_id: summary.visual_id.clone(),
            equipment_count: summary.equipment_count,
            npc_count: summary.npc_count,
            tiandao_count: summary.tiandao_count,
            budget_cost: summary.total_cost,
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
    profile: FangyuanObjectBudgetProfile,
    static_entries: Vec<FangyuanObjectBudgetEntry>,
    active_vfx_recipes: Vec<FangyuanVfxRecipe>,
    audit: FangyuanObjectBudgetAudit,
    summary: FangyuanObjectTrialSummary,
}

impl Default for FangyuanObjectTrialRuntime {
    fn default() -> Self {
        let snapshot = FangyuanObjectBudgetSnapshot::default();
        let profile = FangyuanObjectBudgetProfile::default();
        let audit = audit_fangyuan_object_budget(&snapshot, &profile);
        Self {
            route: FangyuanObjectTrialRoute::None,
            vfx_runtime: FangyuanVfxRuntime::default(),
            profile,
            static_entries: Vec::new(),
            active_vfx_recipes: Vec::new(),
            summary: FangyuanObjectTrialSummary::default(),
            audit,
        }
    }
}

impl FangyuanObjectTrialRuntime {
    pub fn with_profile(profile: FangyuanObjectBudgetProfile) -> Self {
        let snapshot = FangyuanObjectBudgetSnapshot::default();
        let audit = audit_fangyuan_object_budget(&snapshot, &profile);
        Self {
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
        self.static_entries.clear();
        self.active_vfx_recipes.clear();
        self.refresh_audit();
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
        self.summary = FangyuanObjectTrialSummary::from_budget_summary(
            self.route.as_str(),
            &self.audit.summary,
        );
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
    fn fangyuan_object_budget_trial_route_reload_and_clear_leave_no_budget_residue() {
        let mut runtime = FangyuanObjectTrialRuntime::default();

        runtime.enter_default_showcase(0).unwrap();
        runtime.clear_scene();

        assert_eq!(runtime.summary().route_id, "none");
        assert_eq!(runtime.summary().active_vfx_count, 0);
        assert_eq!(runtime.summary().budget_cost, 0);
        assert_eq!(runtime.audit().summary.total_count, 0);
        assert!(runtime.visual_primitives().is_empty());

        let summary = runtime.reload_default_showcase(10).unwrap();
        assert_eq!(summary.route_id, FANGYUAN_OBJECT_TRIAL_ROUTE_ID);
        assert_eq!(summary.active_vfx_count, 4);
        assert!(!runtime.visual_primitives().is_empty());

        runtime.exit_scene();
        assert_eq!(runtime.summary().route_id, "none");
        assert_eq!(runtime.summary().active_vfx_count, 0);
        assert_eq!(runtime.summary().budget_cost, 0);
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
