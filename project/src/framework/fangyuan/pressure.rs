use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::{
    FangyuanObjectBudgetProfile, FangyuanPrimitiveRole, FangyuanSkillDegradeLevel,
    FangyuanSkillTemplate, FangyuanSkillTemplateRegistry, FangyuanSkillVisualBlueprint,
    FangyuanTrialBudgetProfileKind, FangyuanVfxDiagnostic, FangyuanVfxDynamicPrimitiveState,
    compile_fangyuan_skill_runtime_presentation, fangyuan_default_skill_visual_blueprints,
};

pub const FANGYUAN_PRESSURE_DEFAULT_TICKS_PER_SECOND: u32 = 30;
pub const FANGYUAN_PRESSURE_DEFAULT_DURATION_TICKS: u64 = 90;
pub const FANGYUAN_PRESSURE_MIN_ACTOR_COUNT: usize = 1;
pub const FANGYUAN_PRESSURE_MAX_ACTOR_COUNT: usize = 1000;
pub const FANGYUAN_PRESSURE_SUPPORTED_ACTOR_COUNTS: [usize; 3] = [100, 300, 1000];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanPressureBudgetProfileKind {
    Relaxed,
    Standard,
    Strict,
}

impl FangyuanPressureBudgetProfileKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Relaxed => "relaxed",
            Self::Standard => "standard",
            Self::Strict => "strict",
        }
    }

    pub fn profile(self) -> FangyuanObjectBudgetProfile {
        match self {
            Self::Relaxed => FangyuanTrialBudgetProfileKind::Relaxed.profile(),
            Self::Standard => FangyuanTrialBudgetProfileKind::Standard.profile(),
            Self::Strict => FangyuanTrialBudgetProfileKind::Strict.profile(),
        }
    }
}

impl Default for FangyuanPressureBudgetProfileKind {
    fn default() -> Self {
        Self::Standard
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanPressureSceneSize {
    pub width: f32,
    pub depth: f32,
}

impl FangyuanPressureSceneSize {
    pub const fn new(width: f32, depth: f32) -> Self {
        Self { width, depth }
    }
}

impl Default for FangyuanPressureSceneSize {
    fn default() -> Self {
        Self {
            width: 96.0,
            depth: 96.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanPressureScaleStep {
    Actors100,
    Actors300,
    Actors1000,
}

impl FangyuanPressureScaleStep {
    pub const fn actor_count(self) -> usize {
        match self {
            Self::Actors100 => 100,
            Self::Actors300 => 300,
            Self::Actors1000 => 1000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanPressureTestConfig {
    pub actor_count: usize,
    pub skill_template_id: String,
    pub trigger_interval_ticks: u64,
    pub seed: u64,
    pub scene_size: FangyuanPressureSceneSize,
    pub chunk_count: u32,
    #[serde(default)]
    pub budget_profile: FangyuanPressureBudgetProfileKind,
    #[serde(default = "default_pressure_duration_ticks")]
    pub duration_ticks: u64,
    #[serde(default = "default_pressure_ticks_per_second")]
    pub ticks_per_second: u32,
}

impl FangyuanPressureTestConfig {
    pub fn new(
        actor_count: usize,
        skill_template_id: impl Into<String>,
        trigger_interval_ticks: u64,
        seed: u64,
        scene_size: FangyuanPressureSceneSize,
        chunk_count: u32,
        budget_profile: FangyuanPressureBudgetProfileKind,
    ) -> Self {
        Self {
            actor_count,
            skill_template_id: skill_template_id.into(),
            trigger_interval_ticks,
            seed,
            scene_size,
            chunk_count,
            budget_profile,
            duration_ticks: FANGYUAN_PRESSURE_DEFAULT_DURATION_TICKS,
            ticks_per_second: FANGYUAN_PRESSURE_DEFAULT_TICKS_PER_SECOND,
        }
    }

    pub fn scale_step(
        step: FangyuanPressureScaleStep,
        skill_template_id: impl Into<String>,
        seed: u64,
        budget_profile: FangyuanPressureBudgetProfileKind,
    ) -> Self {
        let actor_count = step.actor_count();
        Self::new(
            actor_count,
            skill_template_id,
            pressure_interval_for_actor_count(actor_count),
            seed,
            FangyuanPressureSceneSize::default(),
            pressure_chunk_count_for_actor_count(actor_count),
            budget_profile,
        )
    }

    pub fn validate(&self) -> Result<(), FangyuanPressureConfigError> {
        if self.actor_count < FANGYUAN_PRESSURE_MIN_ACTOR_COUNT
            || self.actor_count > FANGYUAN_PRESSURE_MAX_ACTOR_COUNT
        {
            return Err(FangyuanPressureConfigError::InvalidActorCount {
                actor_count: self.actor_count,
                min: FANGYUAN_PRESSURE_MIN_ACTOR_COUNT,
                max: FANGYUAN_PRESSURE_MAX_ACTOR_COUNT,
            });
        }
        if self.skill_template_id.trim().is_empty() {
            return Err(FangyuanPressureConfigError::InvalidSkillTemplateId);
        }
        if self.trigger_interval_ticks == 0 {
            return Err(FangyuanPressureConfigError::InvalidTriggerInterval);
        }
        if self.duration_ticks == 0 {
            return Err(FangyuanPressureConfigError::InvalidDuration);
        }
        if self.ticks_per_second == 0 {
            return Err(FangyuanPressureConfigError::InvalidTicksPerSecond);
        }
        if self.chunk_count == 0 {
            return Err(FangyuanPressureConfigError::InvalidChunkCount);
        }
        if !self.scene_size.width.is_finite()
            || !self.scene_size.depth.is_finite()
            || self.scene_size.width <= 0.0
            || self.scene_size.depth <= 0.0
        {
            return Err(FangyuanPressureConfigError::InvalidSceneSize);
        }
        let profile = self.budget_profile.profile();
        if profile.recommended_total_cost == 0 || profile.hard_total_cost == 0 {
            return Err(FangyuanPressureConfigError::InvalidBudgetProfile);
        }
        if profile.recommended_total_cost > profile.hard_total_cost {
            return Err(FangyuanPressureConfigError::InvalidBudgetProfile);
        }

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FangyuanPressureConfigError {
    InvalidActorCount {
        actor_count: usize,
        min: usize,
        max: usize,
    },
    InvalidSkillTemplateId,
    InvalidTriggerInterval,
    InvalidDuration,
    InvalidTicksPerSecond,
    InvalidChunkCount,
    InvalidSceneSize,
    InvalidBudgetProfile,
}

impl FangyuanPressureConfigError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidActorCount { .. } => "invalid_actor_count",
            Self::InvalidSkillTemplateId => "invalid_skill_template_id",
            Self::InvalidTriggerInterval => "invalid_trigger_interval",
            Self::InvalidDuration => "invalid_duration",
            Self::InvalidTicksPerSecond => "invalid_ticks_per_second",
            Self::InvalidChunkCount => "invalid_chunk_count",
            Self::InvalidSceneSize => "invalid_scene_size",
            Self::InvalidBudgetProfile => "invalid_budget_profile",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum FangyuanPressureSimulationError {
    InvalidConfig(FangyuanPressureConfigError),
    MissingSkillTemplate { id: String },
    MissingSkillVisual { template_id: String },
    Vfx(FangyuanVfxDiagnostic),
}

impl FangyuanPressureSimulationError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidConfig(error) => error.code(),
            Self::MissingSkillTemplate { .. } => "missing_skill_template",
            Self::MissingSkillVisual { .. } => "missing_skill_visual",
            Self::Vfx(_) => "vfx_error",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanPressureActorPlan {
    pub actor_id: String,
    pub chunk_index: u32,
    pub first_trigger_tick: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanPressureTriggerEvent {
    pub actor_index: usize,
    pub actor_id: String,
    pub chunk_index: u32,
    pub start_tick: u64,
    pub event_id: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FangyuanPressureTickSample {
    pub tick: u64,
    pub started_events: usize,
    pub active_vfx: usize,
    pub dynamic_primitive: usize,
    pub trail: usize,
    pub transparent: usize,
    pub emissive: usize,
    pub pressure: u32,
    pub degrade_level: FangyuanSkillDegradeLevel,
    pub hash: u64,
}

impl FangyuanPressureTickSample {
    fn from_states(
        tick: u64,
        started_events: usize,
        active_vfx: usize,
        states: &[FangyuanVfxDynamicPrimitiveState],
        pressure: u32,
        degrade_level: FangyuanSkillDegradeLevel,
    ) -> Self {
        let mut sample = Self {
            tick,
            started_events,
            active_vfx,
            dynamic_primitive: states.len(),
            pressure,
            degrade_level,
            hash: hash_pressure_states(states),
            ..Default::default()
        };
        for state in states {
            if state.role == FangyuanPrimitiveRole::Trail {
                sample.trail += 1;
            }
            if state.alpha < 1.0 {
                sample.transparent += 1;
            }
            if state.emissive > 0.0 {
                sample.emissive += 1;
            }
        }
        sample
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FangyuanPressureMetricStats {
    pub peak: usize,
    pub average: f64,
}

impl FangyuanPressureMetricStats {
    fn from_values(values: impl IntoIterator<Item = usize>) -> Self {
        let mut peak = 0usize;
        let mut total = 0usize;
        let mut count = 0usize;
        for value in values {
            peak = peak.max(value);
            total = total.saturating_add(value);
            count += 1;
        }
        if count == 0 {
            return Self::default();
        }
        Self {
            peak,
            average: total as f64 / count as f64,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FangyuanPressureCurveSummary {
    pub active_vfx: FangyuanPressureMetricStats,
    pub dynamic_primitive: FangyuanPressureMetricStats,
    pub trail: FangyuanPressureMetricStats,
    pub transparent: FangyuanPressureMetricStats,
    pub emissive: FangyuanPressureMetricStats,
    pub pressure: FangyuanPressureMetricStats,
}

impl FangyuanPressureCurveSummary {
    fn from_samples(samples: &[FangyuanPressureTickSample]) -> Self {
        Self {
            active_vfx: FangyuanPressureMetricStats::from_values(
                samples.iter().map(|sample| sample.active_vfx),
            ),
            dynamic_primitive: FangyuanPressureMetricStats::from_values(
                samples.iter().map(|sample| sample.dynamic_primitive),
            ),
            trail: FangyuanPressureMetricStats::from_values(
                samples.iter().map(|sample| sample.trail),
            ),
            transparent: FangyuanPressureMetricStats::from_values(
                samples.iter().map(|sample| sample.transparent),
            ),
            emissive: FangyuanPressureMetricStats::from_values(
                samples.iter().map(|sample| sample.emissive),
            ),
            pressure: FangyuanPressureMetricStats::from_values(
                samples.iter().map(|sample| sample.pressure as usize),
            ),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanPressureDegradeSummary {
    pub none_ticks: usize,
    pub low_ticks: usize,
    pub medium_ticks: usize,
    pub high_ticks: usize,
    pub critical_ticks: usize,
    pub worst_level: FangyuanSkillDegradeLevel,
    pub reason: String,
}

impl Default for FangyuanPressureDegradeSummary {
    fn default() -> Self {
        Self {
            none_ticks: 0,
            low_ticks: 0,
            medium_ticks: 0,
            high_ticks: 0,
            critical_ticks: 0,
            worst_level: FangyuanSkillDegradeLevel::None,
            reason: "ok".to_string(),
        }
    }
}

impl FangyuanPressureDegradeSummary {
    fn from_samples(samples: &[FangyuanPressureTickSample]) -> Self {
        let mut summary = Self::default();
        for sample in samples {
            match sample.degrade_level {
                FangyuanSkillDegradeLevel::None => summary.none_ticks += 1,
                FangyuanSkillDegradeLevel::Low => summary.low_ticks += 1,
                FangyuanSkillDegradeLevel::Medium => summary.medium_ticks += 1,
                FangyuanSkillDegradeLevel::High => summary.high_ticks += 1,
                FangyuanSkillDegradeLevel::Critical => summary.critical_ticks += 1,
            }
            summary.worst_level = summary.worst_level.max(sample.degrade_level);
        }
        summary.reason = match summary.worst_level {
            FangyuanSkillDegradeLevel::None => "within_budget".to_string(),
            FangyuanSkillDegradeLevel::Low => "above_recommended_budget".to_string(),
            FangyuanSkillDegradeLevel::Medium => "above_hard_budget".to_string(),
            FangyuanSkillDegradeLevel::High => "above_double_hard_budget".to_string(),
            FangyuanSkillDegradeLevel::Critical => "above_quad_hard_budget".to_string(),
        };
        summary
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanPressureReport {
    pub config: FangyuanPressureTestConfig,
    pub skill_visual_id: String,
    pub total_trigger_events: usize,
    pub sample_count: usize,
    pub curve: FangyuanPressureCurveSummary,
    pub degrade: FangyuanPressureDegradeSummary,
    pub chunk_load: Vec<FangyuanPressureChunkLoad>,
    pub deterministic_hash: u64,
    pub summary_text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanPressureSimulationResult {
    pub report: FangyuanPressureReport,
    pub samples: Vec<FangyuanPressureTickSample>,
    pub actor_plan: Vec<FangyuanPressureActorPlan>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanPressureChunkLoad {
    pub chunk_index: u32,
    pub actor_count: usize,
}

#[derive(Clone, Debug, PartialEq)]
struct ActivePressureEvent {
    event: FangyuanPressureTriggerEvent,
}

pub fn run_fangyuan_pressure_test(
    config: FangyuanPressureTestConfig,
) -> Result<FangyuanPressureSimulationResult, FangyuanPressureSimulationError> {
    config
        .validate()
        .map_err(FangyuanPressureSimulationError::InvalidConfig)?;

    let registry = FangyuanSkillTemplateRegistry::with_defaults();
    let template = registry.get(&config.skill_template_id, 1).ok_or_else(|| {
        FangyuanPressureSimulationError::MissingSkillTemplate {
            id: config.skill_template_id.clone(),
        }
    })?;
    let blueprint = resolve_pressure_skill_visual(&config.skill_template_id)?;
    let actor_plan = fangyuan_pressure_actor_plan(&config);
    let mut active_events = Vec::<ActivePressureEvent>::new();
    let mut samples = Vec::with_capacity(config.duration_ticks as usize + 1);
    let mut trigger_sequence = 0usize;

    for tick in 0..=config.duration_ticks {
        let mut started_events = 0usize;
        for (actor_index, actor) in actor_plan.iter().enumerate() {
            if should_trigger_actor(actor, tick, config.trigger_interval_ticks) {
                let event = FangyuanPressureTriggerEvent {
                    actor_index,
                    actor_id: actor.actor_id.clone(),
                    chunk_index: actor.chunk_index,
                    start_tick: tick,
                    event_id: format!("pressure-{}-{tick}-{trigger_sequence}", actor.actor_id),
                };
                active_events.push(ActivePressureEvent { event });
                started_events += 1;
                trigger_sequence += 1;
            }
        }

        active_events.retain(|event| {
            tick.saturating_sub(event.event.start_tick) <= pressure_event_retention_ticks(template)
        });

        let estimated_pressure = estimate_pressure_units(&config, active_events.len());
        let degrade_level = pressure_degrade_level(&config, estimated_pressure);
        let mut states = Vec::<FangyuanVfxDynamicPrimitiveState>::new();
        for active_event in &active_events {
            states.extend(evaluate_pressure_event(
                template,
                &blueprint,
                &active_event.event,
                &config,
                tick,
                degrade_level,
            )?);
        }
        stable_sort_states(&mut states);

        samples.push(FangyuanPressureTickSample::from_states(
            tick,
            started_events,
            active_events.len(),
            &states,
            estimated_pressure,
            degrade_level,
        ));
    }

    let report = build_pressure_report(config, &blueprint, &actor_plan, &samples);
    Ok(FangyuanPressureSimulationResult {
        report,
        samples,
        actor_plan,
    })
}

pub fn fangyuan_pressure_actor_plan(
    config: &FangyuanPressureTestConfig,
) -> Vec<FangyuanPressureActorPlan> {
    let mut plan = Vec::with_capacity(config.actor_count);
    for actor_index in 0..config.actor_count {
        let actor_hash = mix_pressure_seed(config.seed, actor_index as u64);
        let first_trigger_tick = actor_hash % config.trigger_interval_ticks;
        let chunk_index = if config.chunk_count == 0 {
            0
        } else {
            (actor_hash % u64::from(config.chunk_count)) as u32
        };
        plan.push(FangyuanPressureActorPlan {
            actor_id: format!("actor-{actor_index:04}"),
            chunk_index,
            first_trigger_tick,
        });
    }
    plan
}

pub fn fangyuan_pressure_report_text(report: &FangyuanPressureReport) -> String {
    format!(
        "fangyuan_pressure actors {} skill {} interval {} seed {} scene {:.1}x{:.1} chunks {} budget {} samples {} triggers {} peak active_vfx {} dynamic {} trail {} transparent {} emissive {} pressure {} avg_pressure {:.2} degrade {} hash {}",
        report.config.actor_count,
        report.config.skill_template_id,
        report.config.trigger_interval_ticks,
        report.config.seed,
        report.config.scene_size.width,
        report.config.scene_size.depth,
        report.config.chunk_count,
        report.config.budget_profile.as_str(),
        report.sample_count,
        report.total_trigger_events,
        report.curve.active_vfx.peak,
        report.curve.dynamic_primitive.peak,
        report.curve.trail.peak,
        report.curve.transparent.peak,
        report.curve.emissive.peak,
        report.curve.pressure.peak,
        report.curve.pressure.average,
        degrade_label(report.degrade.worst_level),
        report.deterministic_hash,
    )
}

fn resolve_pressure_skill_visual(
    template_id: &str,
) -> Result<FangyuanSkillVisualBlueprint, FangyuanPressureSimulationError> {
    fangyuan_default_skill_visual_blueprints()
        .into_iter()
        .find(|blueprint| blueprint.template_id == template_id)
        .ok_or_else(|| FangyuanPressureSimulationError::MissingSkillVisual {
            template_id: template_id.to_string(),
        })
}

fn evaluate_pressure_event(
    template: &FangyuanSkillTemplate,
    blueprint: &FangyuanSkillVisualBlueprint,
    event: &FangyuanPressureTriggerEvent,
    config: &FangyuanPressureTestConfig,
    current_tick: u64,
    degrade_level: FangyuanSkillDegradeLevel,
) -> Result<Vec<FangyuanVfxDynamicPrimitiveState>, FangyuanPressureSimulationError> {
    let context = super::FangyuanSkillRuntimeContext {
        start_tick: event.start_tick,
        current_tick,
        ticks_per_second: config.ticks_per_second,
        caster_id: event.actor_id.clone(),
        event_id: event.event_id.clone(),
        external_seed: Some(mix_pressure_seed(config.seed, event.actor_index as u64)),
        degrade_level,
        equipment_sockets: None,
    };
    let actor_hash = mix_pressure_seed(config.seed, event.actor_index as u64);
    let position = actor_position(actor_hash, &config.scene_size);
    compile_fangyuan_skill_runtime_presentation(template, blueprint, &context)
        .map(|presentation| {
            let mut states = presentation.playback_states();
            for state in &mut states {
                state.local_position.x += position.0;
                state.local_position.z += position.1;
            }
            states
        })
        .map_err(FangyuanPressureSimulationError::Vfx)
}

fn should_trigger_actor(
    actor: &FangyuanPressureActorPlan,
    tick: u64,
    trigger_interval_ticks: u64,
) -> bool {
    if tick < actor.first_trigger_tick {
        return false;
    }
    tick.saturating_sub(actor.first_trigger_tick) % trigger_interval_ticks == 0
}

fn pressure_event_retention_ticks(template: &FangyuanSkillTemplate) -> u64 {
    template
        .timing
        .impact_tick_offset
        .saturating_add(template.timing.recovery_ticks)
        .saturating_add(template.danger_boundary.linger_ticks)
}

fn estimate_pressure_units(config: &FangyuanPressureTestConfig, active_vfx: usize) -> u32 {
    let profile = config.budget_profile.profile();
    let chunk_factor = config.chunk_count.max(1);
    let actor_factor = (config.actor_count as u32 / chunk_factor).max(1);
    active_vfx
        .saturating_mul(actor_factor as usize)
        .saturating_div(8)
        .min(u32::MAX as usize) as u32
        + active_vfx.min(u32::MAX as usize) as u32
        + profile.recommended_total_cost / 8
}

fn pressure_degrade_level(
    config: &FangyuanPressureTestConfig,
    pressure_units: u32,
) -> FangyuanSkillDegradeLevel {
    let profile = config.budget_profile.profile();
    if pressure_units > profile.hard_total_cost.saturating_mul(4) {
        FangyuanSkillDegradeLevel::Critical
    } else if pressure_units > profile.hard_total_cost.saturating_mul(2) {
        FangyuanSkillDegradeLevel::High
    } else if pressure_units > profile.hard_total_cost {
        FangyuanSkillDegradeLevel::Medium
    } else if pressure_units > profile.recommended_total_cost {
        FangyuanSkillDegradeLevel::Low
    } else {
        FangyuanSkillDegradeLevel::None
    }
}

fn actor_position(actor_hash: u64, scene_size: &FangyuanPressureSceneSize) -> (f32, f32) {
    let x_part = (actor_hash & 0xffff) as f32 / 65_535.0;
    let z_part = ((actor_hash >> 16) & 0xffff) as f32 / 65_535.0;
    (
        (x_part - 0.5) * scene_size.width,
        (z_part - 0.5) * scene_size.depth,
    )
}

fn stable_sort_states(states: &mut [FangyuanVfxDynamicPrimitiveState]) {
    states.sort_by(|a, b| {
        (
            a.source_tick,
            a.recipe_id.as_str(),
            a.emitter_index,
            a.primitive_index,
            a.role.as_str(),
        )
            .cmp(&(
                b.source_tick,
                b.recipe_id.as_str(),
                b.emitter_index,
                b.primitive_index,
                b.role.as_str(),
            ))
    });
}

fn build_pressure_report(
    config: FangyuanPressureTestConfig,
    blueprint: &FangyuanSkillVisualBlueprint,
    actor_plan: &[FangyuanPressureActorPlan],
    samples: &[FangyuanPressureTickSample],
) -> FangyuanPressureReport {
    let total_trigger_events = samples.iter().map(|sample| sample.started_events).sum();
    let curve = FangyuanPressureCurveSummary::from_samples(samples);
    let degrade = FangyuanPressureDegradeSummary::from_samples(samples);
    let chunk_load = pressure_chunk_load(actor_plan);
    let deterministic_hash = hash_pressure_samples(samples);
    let mut report = FangyuanPressureReport {
        config,
        skill_visual_id: blueprint.id.clone(),
        total_trigger_events,
        sample_count: samples.len(),
        curve,
        degrade,
        chunk_load,
        deterministic_hash,
        summary_text: String::new(),
    };
    report.summary_text = fangyuan_pressure_report_text(&report);
    report
}

fn pressure_chunk_load(actor_plan: &[FangyuanPressureActorPlan]) -> Vec<FangyuanPressureChunkLoad> {
    let mut counts = BTreeMap::<u32, usize>::new();
    for actor in actor_plan {
        *counts.entry(actor.chunk_index).or_default() += 1;
    }
    counts
        .into_iter()
        .map(|(chunk_index, actor_count)| FangyuanPressureChunkLoad {
            chunk_index,
            actor_count,
        })
        .collect()
}

fn pressure_interval_for_actor_count(actor_count: usize) -> u64 {
    match actor_count {
        0..=100 => 6,
        101..=300 => 4,
        _ => 2,
    }
}

fn pressure_chunk_count_for_actor_count(actor_count: usize) -> u32 {
    match actor_count {
        0..=100 => 4,
        101..=300 => 9,
        _ => 16,
    }
}

fn hash_pressure_samples(samples: &[FangyuanPressureTickSample]) -> u64 {
    let mut hash = FNV_OFFSET;
    for sample in samples {
        mix_u64(&mut hash, sample.tick);
        mix_u64(&mut hash, sample.started_events as u64);
        mix_u64(&mut hash, sample.active_vfx as u64);
        mix_u64(&mut hash, sample.dynamic_primitive as u64);
        mix_u64(&mut hash, sample.trail as u64);
        mix_u64(&mut hash, sample.transparent as u64);
        mix_u64(&mut hash, sample.emissive as u64);
        mix_u64(&mut hash, u64::from(sample.pressure));
        mix_u64(&mut hash, sample.degrade_level as u64);
        mix_u64(&mut hash, sample.hash);
    }
    avalanche(hash)
}

fn hash_pressure_states(states: &[FangyuanVfxDynamicPrimitiveState]) -> u64 {
    let mut hash = FNV_OFFSET;
    for state in states {
        mix_str(&mut hash, &state.recipe_id);
        mix_str(&mut hash, &state.emitter_id);
        mix_u64(&mut hash, state.emitter_index as u64);
        mix_u64(&mut hash, u64::from(state.primitive_index));
        mix_u64(&mut hash, state.source_tick);
        mix_str(&mut hash, state.role.as_str());
        mix_u32(&mut hash, state.local_position.x.to_bits());
        mix_u32(&mut hash, state.local_position.y.to_bits());
        mix_u32(&mut hash, state.local_position.z.to_bits());
        mix_u32(&mut hash, state.alpha.to_bits());
        mix_u32(&mut hash, state.emissive.to_bits());
        mix_u64(&mut hash, state.seed);
    }
    avalanche(hash)
}

fn mix_pressure_seed(seed: u64, value: u64) -> u64 {
    let mut hash = FNV_OFFSET;
    mix_u64(&mut hash, seed);
    mix_u64(&mut hash, value);
    avalanche(hash)
}

fn mix_str(hash: &mut u64, value: &str) {
    for byte in value.as_bytes() {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(FNV_PRIME);
    }
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

fn degrade_label(level: FangyuanSkillDegradeLevel) -> &'static str {
    match level {
        FangyuanSkillDegradeLevel::None => "none",
        FangyuanSkillDegradeLevel::Low => "low",
        FangyuanSkillDegradeLevel::Medium => "medium",
        FangyuanSkillDegradeLevel::High => "high",
        FangyuanSkillDegradeLevel::Critical => "critical",
    }
}

fn default_pressure_duration_ticks() -> u64 {
    FANGYUAN_PRESSURE_DEFAULT_DURATION_TICKS
}

fn default_pressure_ticks_per_second() -> u32 {
    FANGYUAN_PRESSURE_DEFAULT_TICKS_PER_SECOND
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::fangyuan::FANGYUAN_SKILL_PROJECTILE_TEMPLATE_ID;

    fn base_config(actor_count: usize) -> FangyuanPressureTestConfig {
        FangyuanPressureTestConfig {
            duration_ticks: 24,
            ..FangyuanPressureTestConfig::new(
                actor_count,
                FANGYUAN_SKILL_PROJECTILE_TEMPLATE_ID,
                6,
                42,
                FangyuanPressureSceneSize::new(64.0, 64.0),
                4,
                FangyuanPressureBudgetProfileKind::Standard,
            )
        }
    }

    #[test]
    fn fangyuan_pressure_config_parses_and_validates_required_fields() {
        let json = r#"{
            "actor_count": 100,
            "skill_template_id": "skill.template.projectile",
            "trigger_interval_ticks": 5,
            "seed": 77,
            "scene_size": { "width": 80.0, "depth": 72.0 },
            "chunk_count": 6,
            "budget_profile": "strict",
            "duration_ticks": 18,
            "ticks_per_second": 30
        }"#;

        let config: FangyuanPressureTestConfig = serde_json::from_str(json).unwrap();

        config.validate().unwrap();
        assert_eq!(config.actor_count, 100);
        assert_eq!(config.trigger_interval_ticks, 5);
        assert_eq!(
            config.budget_profile,
            FangyuanPressureBudgetProfileKind::Strict
        );
        assert_eq!(config.scene_size.width, 80.0);
    }

    #[test]
    fn fangyuan_pressure_seed_stability_replays_same_hash_and_actor_plan() {
        let config = base_config(100);

        let first = run_fangyuan_pressure_test(config.clone()).unwrap();
        let second = run_fangyuan_pressure_test(config).unwrap();

        assert_eq!(first.actor_plan, second.actor_plan);
        assert_eq!(
            first.report.deterministic_hash,
            second.report.deterministic_hash
        );
        assert_eq!(first.samples, second.samples);
    }

    #[test]
    fn fangyuan_pressure_seed_change_changes_curve_hash() {
        let first = run_fangyuan_pressure_test(base_config(100)).unwrap();
        let mut second_config = base_config(100);
        second_config.seed = 43;

        let second = run_fangyuan_pressure_test(second_config).unwrap();

        assert_ne!(
            first.report.deterministic_hash,
            second.report.deterministic_hash
        );
    }

    #[test]
    fn fangyuan_pressure_scale_steps_cover_100_300_and_1000_actor_simulations() {
        for step in [
            FangyuanPressureScaleStep::Actors100,
            FangyuanPressureScaleStep::Actors300,
            FangyuanPressureScaleStep::Actors1000,
        ] {
            let mut config = FangyuanPressureTestConfig::scale_step(
                step,
                FANGYUAN_SKILL_PROJECTILE_TEMPLATE_ID,
                99,
                FangyuanPressureBudgetProfileKind::Standard,
            );
            config.duration_ticks = 18;

            let result = run_fangyuan_pressure_test(config).unwrap();

            assert_eq!(result.report.config.actor_count, step.actor_count());
            assert_eq!(result.actor_plan.len(), step.actor_count());
            assert_eq!(result.report.sample_count, 19);
            assert!(result.report.total_trigger_events > 0);
            assert!(result.report.curve.active_vfx.peak > 0);
            assert!(result.report.curve.dynamic_primitive.peak > 0);
            assert!(result.report.curve.pressure.peak > 0);
            assert!(!result.report.chunk_load.is_empty());
        }
    }

    #[test]
    fn fangyuan_pressure_report_contains_curve_peaks_averages_and_summary_text() {
        let result = run_fangyuan_pressure_test(base_config(100)).unwrap();
        let report = result.report;

        assert_eq!(
            report.config.skill_template_id,
            FANGYUAN_SKILL_PROJECTILE_TEMPLATE_ID
        );
        assert_eq!(report.skill_visual_id, "skill.visual.projectile");
        assert_eq!(report.sample_count, 25);
        assert!(report.curve.active_vfx.peak >= report.curve.active_vfx.average as usize);
        assert!(report.curve.trail.peak > 0);
        assert!(report.curve.transparent.peak > 0);
        assert!(report.curve.emissive.peak <= report.curve.dynamic_primitive.peak);
        assert!(report.summary_text.contains("fangyuan_pressure actors 100"));
        assert!(report.summary_text.contains("peak active_vfx"));
        assert!(report.summary_text.contains("avg_pressure"));
    }

    #[test]
    fn fangyuan_pressure_failures_return_errors_without_panic() {
        let mut bad_actor_count = base_config(0);
        let error = bad_actor_count.validate().unwrap_err();
        assert_eq!(error.code(), "invalid_actor_count");

        bad_actor_count.actor_count = 100;
        bad_actor_count.chunk_count = 0;
        let error = run_fangyuan_pressure_test(bad_actor_count).unwrap_err();
        assert_eq!(error.code(), "invalid_chunk_count");

        let mut bad_interval = base_config(100);
        bad_interval.trigger_interval_ticks = 0;
        let error = run_fangyuan_pressure_test(bad_interval).unwrap_err();
        assert_eq!(error.code(), "invalid_trigger_interval");

        let mut missing_template = base_config(100);
        missing_template.skill_template_id = "missing.template".to_string();
        let error = run_fangyuan_pressure_test(missing_template).unwrap_err();
        assert_eq!(error.code(), "missing_skill_template");
    }
}
