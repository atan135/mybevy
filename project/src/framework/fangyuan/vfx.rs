use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::{
    FangyuanAuditFinding, FangyuanAuditReport, FangyuanAuditSeverity, FangyuanAuditSourceKind,
    FangyuanAuditSuggestion, FangyuanAuditSuggestionAction, FangyuanPrimitive,
    FangyuanPrimitiveKind, FangyuanPrimitiveLifecycle, FangyuanPrimitiveRole,
};

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
const FANGYUAN_VFX_RECOMMENDED_DURATION_TICKS: u64 = 120;
const FANGYUAN_VFX_RECOMMENDED_PEAK_PRIMITIVES: usize = 24;
const FANGYUAN_VFX_RECOMMENDED_TRAIL_SEGMENTS: usize = 12;
const FANGYUAN_VFX_RECOMMENDED_ALPHA_PRIMITIVES: usize = 12;
const FANGYUAN_VFX_RECOMMENDED_EMISSIVE_PRIMITIVES: usize = 8;
const FANGYUAN_VFX_RECOMMENDED_MATERIAL_PROFILES: usize = 4;
const FANGYUAN_VFX_MAX_EMISSIVE_INTENSITY: f32 = 4.0;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanVfxRecipe {
    pub id: String,
    pub version: u32,
    pub duration_ticks: u64,
    #[serde(default)]
    pub seed_policy: FangyuanVfxSeedPolicy,
    #[serde(default)]
    pub emitters: Vec<FangyuanVfxEmitter>,
    #[serde(default)]
    pub curves: Vec<FangyuanVfxCurveBinding>,
    #[serde(default)]
    pub budget_hints: FangyuanVfxBudgetHints,
}

impl FangyuanVfxRecipe {
    pub fn validate(&self) -> Result<(), FangyuanVfxDiagnostic> {
        if self.id.trim().is_empty() {
            return Err(FangyuanVfxDiagnostic::empty_recipe_id());
        }
        if self.duration_ticks == 0 {
            return Err(FangyuanVfxDiagnostic::new(
                FangyuanVfxDiagnosticCode::InvalidDuration,
                "recipe duration_ticks must be greater than zero",
            ));
        }
        if self.emitters.is_empty() {
            return Err(FangyuanVfxDiagnostic::new(
                FangyuanVfxDiagnosticCode::MissingEmitter,
                "recipe must contain at least one emitter",
            ));
        }

        for (emitter_index, emitter) in self.emitters.iter().enumerate() {
            emitter.validate(emitter_index)?;
        }

        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanVfxSeedPolicy {
    #[default]
    Deterministic,
    Fixed(u64),
    External,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanVfxBudgetHints {
    pub max_primitives: u16,
    pub max_trail_segments: u16,
    pub max_emitters: u16,
}

impl Default for FangyuanVfxBudgetHints {
    fn default() -> Self {
        Self {
            max_primitives: 32,
            max_trail_segments: 16,
            max_emitters: 8,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanVfxCurveBinding {
    pub id: String,
    pub curve: FangyuanVfxCurve,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanVfxCurve {
    #[default]
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    Step,
}

impl FangyuanVfxCurve {
    pub fn sample(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::EaseIn => t * t,
            Self::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Self::EaseInOut => t * t * (3.0 - 2.0 * t),
            Self::Step => {
                if t >= 1.0 {
                    1.0
                } else {
                    0.0
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanVfxEmitter {
    pub id: String,
    #[serde(default)]
    pub primitive_kind: FangyuanPrimitiveKind,
    #[serde(default)]
    pub role: FangyuanPrimitiveRole,
    #[serde(default)]
    pub delay_ticks: u64,
    #[serde(default)]
    pub duration_ticks: Option<u64>,
    #[serde(default)]
    pub position: [f32; 3],
    #[serde(default = "unit_vec3_array")]
    pub scale: [f32; 3],
    #[serde(default = "white_color_array")]
    pub color: [f32; 4],
    #[serde(default)]
    pub emissive: f32,
    #[serde(default)]
    pub material_profile_id: Option<String>,
    #[serde(default)]
    pub jitter: FangyuanVfxEmitterJitter,
    #[serde(default)]
    pub operators: Vec<FangyuanVfxOperator>,
}

impl FangyuanVfxEmitter {
    fn validate(&self, emitter_index: usize) -> Result<(), FangyuanVfxDiagnostic> {
        if self.id.trim().is_empty() {
            return Err(FangyuanVfxDiagnostic::with_emitter(
                FangyuanVfxDiagnosticCode::MissingEmitterId,
                "emitter id must not be empty",
                emitter_index,
            ));
        }

        validate_finite_vec3(self.position, FangyuanVfxDiagnosticCode::InvalidEmitter)?;
        validate_finite_vec3(self.scale, FangyuanVfxDiagnosticCode::InvalidEmitter)?;
        validate_finite_color(self.color, FangyuanVfxDiagnosticCode::InvalidEmitter)?;

        if self.operators.is_empty() {
            return Err(FangyuanVfxDiagnostic::with_emitter(
                FangyuanVfxDiagnosticCode::MissingOperator,
                "emitter must contain at least one operator",
                emitter_index,
            ));
        }

        for operator in &self.operators {
            operator.validate(emitter_index)?;
        }

        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanVfxEmitterJitter {
    #[serde(default)]
    pub position: [f32; 3],
    #[serde(default)]
    pub scale: f32,
    #[serde(default)]
    pub alpha: f32,
    #[serde(default)]
    pub color: [f32; 3],
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum FangyuanVfxOperator {
    Spawn {
        #[serde(default)]
        curve: FangyuanVfxCurve,
    },
    Move {
        from: [f32; 3],
        to: [f32; 3],
        #[serde(default)]
        curve: FangyuanVfxCurve,
    },
    Scale {
        from: [f32; 3],
        to: [f32; 3],
        #[serde(default)]
        curve: FangyuanVfxCurve,
    },
    Fade {
        from: f32,
        to: f32,
        #[serde(default)]
        curve: FangyuanVfxCurve,
    },
    ColorShift {
        from: [f32; 4],
        to: [f32; 4],
        #[serde(default)]
        curve: FangyuanVfxCurve,
    },
    EmissivePulse {
        amplitude: f32,
        frequency_hz: f32,
        #[serde(default)]
        curve: FangyuanVfxCurve,
    },
    Trail {
        segments: u16,
        spacing_ticks: u64,
        fade: f32,
    },
    ImpactExpand {
        radius_from: f32,
        radius_to: f32,
        #[serde(default)]
        curve: FangyuanVfxCurve,
    },
}

impl FangyuanVfxOperator {
    fn validate(&self, emitter_index: usize) -> Result<(), FangyuanVfxDiagnostic> {
        let invalid = |message| {
            FangyuanVfxDiagnostic::with_emitter(
                FangyuanVfxDiagnosticCode::InvalidOperator,
                message,
                emitter_index,
            )
        };

        match self {
            Self::Spawn { .. } => Ok(()),
            Self::Move { from, to, .. } | Self::Scale { from, to, .. } => {
                validate_finite_vec3(*from, FangyuanVfxDiagnosticCode::InvalidOperator)?;
                validate_finite_vec3(*to, FangyuanVfxDiagnosticCode::InvalidOperator)
            }
            Self::Fade { from, to, .. } => {
                if !from.is_finite() || !to.is_finite() {
                    Err(invalid("fade values must be finite"))
                } else {
                    Ok(())
                }
            }
            Self::ColorShift { from, to, .. } => {
                validate_finite_color(*from, FangyuanVfxDiagnosticCode::InvalidOperator)?;
                validate_finite_color(*to, FangyuanVfxDiagnosticCode::InvalidOperator)
            }
            Self::EmissivePulse {
                amplitude,
                frequency_hz,
                ..
            } => {
                if !amplitude.is_finite() || !frequency_hz.is_finite() || *frequency_hz < 0.0 {
                    Err(invalid(
                        "emissive pulse amplitude and non-negative frequency must be finite",
                    ))
                } else {
                    Ok(())
                }
            }
            Self::Trail {
                segments,
                spacing_ticks,
                fade,
            } => {
                if *segments == 0 {
                    Err(invalid("trail segments must be greater than zero"))
                } else if *spacing_ticks == 0 {
                    Err(invalid("trail spacing_ticks must be greater than zero"))
                } else if !fade.is_finite() {
                    Err(invalid("trail fade must be finite"))
                } else {
                    Ok(())
                }
            }
            Self::ImpactExpand {
                radius_from,
                radius_to,
                ..
            } => {
                if !radius_from.is_finite()
                    || !radius_to.is_finite()
                    || *radius_from < 0.0
                    || *radius_to < 0.0
                {
                    Err(invalid("impact radii must be finite and non-negative"))
                } else {
                    Ok(())
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FangyuanVfxClock {
    pub start_tick: u64,
    pub current_tick: u64,
    pub ticks_per_second: u32,
}

impl FangyuanVfxClock {
    pub const fn new(start_tick: u64, current_tick: u64, ticks_per_second: u32) -> Self {
        Self {
            start_tick,
            current_tick,
            ticks_per_second,
        }
    }

    pub fn elapsed_ticks(self) -> u64 {
        self.current_tick.saturating_sub(self.start_tick)
    }

    pub const fn is_before_start(self) -> bool {
        self.current_tick < self.start_tick
    }

    pub fn elapsed_seconds(self) -> f32 {
        if self.ticks_per_second == 0 {
            return 0.0;
        }
        self.elapsed_ticks() as f32 / self.ticks_per_second as f32
    }

    pub fn is_past_duration(self, duration_ticks: u64) -> bool {
        self.elapsed_ticks() > duration_ticks
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanVfxReplayContext {
    pub caster_id: String,
    pub event_id: String,
    pub external_seed: Option<u64>,
}

impl FangyuanVfxReplayContext {
    pub fn local(caster_id: impl Into<String>, event_id: impl Into<String>) -> Self {
        Self {
            caster_id: caster_id.into(),
            event_id: event_id.into(),
            external_seed: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanVfxPredictionBoundary {
    #[default]
    AuthorityConfirmed,
    LocalPredicted,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanVfxReplayEvent {
    pub authority_epoch: u64,
    pub start_tick: u64,
    pub frame_id: u32,
    pub fps: u16,
    pub action: String,
    pub caster_id: String,
    pub player_id: String,
    pub event_id: String,
    pub recipe_id: String,
    #[serde(default)]
    pub external_seed: Option<u64>,
    #[serde(default)]
    pub prediction_boundary: FangyuanVfxPredictionBoundary,
}

impl FangyuanVfxReplayEvent {
    pub fn replay_context(&self) -> FangyuanVfxReplayContext {
        FangyuanVfxReplayContext {
            caster_id: self.caster_id.clone(),
            event_id: self.event_id.clone(),
            external_seed: self.external_seed,
        }
    }

    pub fn clock_at(&self, current_tick: u64) -> FangyuanVfxClock {
        FangyuanVfxClock::new(self.start_tick, current_tick, u32::from(self.fps))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanVfxDynamicPrimitiveState {
    pub recipe_id: String,
    pub emitter_id: String,
    pub emitter_index: usize,
    pub primitive_index: u16,
    pub source_tick: u64,
    pub source: FangyuanVfxPrimitiveSource,
    pub local_position: Vec3,
    pub scale: Vec3,
    pub color: Color,
    pub alpha: f32,
    pub emissive: f32,
    pub primitive_kind: FangyuanPrimitiveKind,
    pub role: FangyuanPrimitiveRole,
    pub material_profile_id: Option<String>,
    pub lifecycle: FangyuanPrimitiveLifecycle,
    pub seed: u64,
}

impl FangyuanVfxDynamicPrimitiveState {
    pub fn position(&self) -> Vec3 {
        self.local_position
    }

    pub fn profile(&self) -> Option<&str> {
        self.material_profile_id.as_deref()
    }

    pub const fn lifetime(&self) -> FangyuanPrimitiveLifecycle {
        self.lifecycle
    }

    pub fn to_runtime_primitive(&self) -> FangyuanPrimitive {
        FangyuanPrimitive::with_runtime_metadata(
            self.primitive_kind,
            self.local_position,
            self.scale,
            self.color,
            self.role,
            self.alpha,
            self.emissive,
            self.material_profile_id.clone(),
            self.lifecycle,
        )
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FangyuanVfxBudgetEstimate {
    pub duration_ticks: u64,
    pub emitter_count: usize,
    pub peak_primitives: usize,
    pub trail_segments: usize,
    pub alpha_primitives: usize,
    pub transparent_primitives: usize,
    pub emissive_primitives: usize,
    pub material_profile_count: usize,
    pub roles: Vec<FangyuanPrimitiveRole>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FangyuanVfxBudgetProfile {
    pub recommended_duration_ticks: u64,
    pub recommended_peak_primitives: usize,
    pub recommended_trail_segments: usize,
    pub recommended_alpha_primitives: usize,
    pub recommended_emissive_primitives: usize,
    pub recommended_material_profiles: usize,
    pub max_emissive_intensity: f32,
}

impl Default for FangyuanVfxBudgetProfile {
    fn default() -> Self {
        Self {
            recommended_duration_ticks: FANGYUAN_VFX_RECOMMENDED_DURATION_TICKS,
            recommended_peak_primitives: FANGYUAN_VFX_RECOMMENDED_PEAK_PRIMITIVES,
            recommended_trail_segments: FANGYUAN_VFX_RECOMMENDED_TRAIL_SEGMENTS,
            recommended_alpha_primitives: FANGYUAN_VFX_RECOMMENDED_ALPHA_PRIMITIVES,
            recommended_emissive_primitives: FANGYUAN_VFX_RECOMMENDED_EMISSIVE_PRIMITIVES,
            recommended_material_profiles: FANGYUAN_VFX_RECOMMENDED_MATERIAL_PROFILES,
            max_emissive_intensity: FANGYUAN_VFX_MAX_EMISSIVE_INTENSITY,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FangyuanVfxBudgetPressure {
    pub max_primitives: Option<usize>,
    pub max_trail_segments: Option<u16>,
    pub skip_decoration: bool,
}

impl FangyuanVfxBudgetPressure {
    pub const fn none() -> Self {
        Self {
            max_primitives: None,
            max_trail_segments: None,
            skip_decoration: false,
        }
    }

    pub const fn constrained(max_primitives: usize, max_trail_segments: u16) -> Self {
        Self {
            max_primitives: Some(max_primitives),
            max_trail_segments: Some(max_trail_segments),
            skip_decoration: true,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FangyuanVfxInstanceId(String);

impl FangyuanVfxInstanceId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanVfxPrimitiveSource {
    pub instance_id: Option<FangyuanVfxInstanceId>,
    pub recipe_id: String,
    pub emitter_id: String,
    pub emitter_index: usize,
    pub primitive_index: u16,
    pub source_tick: u64,
}

impl FangyuanVfxPrimitiveSource {
    fn from_state_parts(
        recipe_id: String,
        emitter_id: String,
        emitter_index: usize,
        primitive_index: u16,
        source_tick: u64,
    ) -> Self {
        Self {
            instance_id: None,
            recipe_id,
            emitter_id,
            emitter_index,
            primitive_index,
            source_tick,
        }
    }

    fn with_instance_id(mut self, instance_id: FangyuanVfxInstanceId) -> Self {
        self.instance_id = Some(instance_id);
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanVfxStandardMeshFallbackPrimitive {
    pub source: FangyuanVfxPrimitiveSource,
    pub primitive_kind: FangyuanPrimitiveKind,
    pub transform: Transform,
    pub color: Color,
    pub alpha: f32,
    pub emissive: f32,
    pub material_profile_id: Option<String>,
    pub lifecycle: FangyuanPrimitiveLifecycle,
}

pub fn fangyuan_vfx_standard_mesh_fallback_primitives(
    states: &[FangyuanVfxDynamicPrimitiveState],
) -> Vec<FangyuanVfxStandardMeshFallbackPrimitive> {
    states
        .iter()
        .map(|state| FangyuanVfxStandardMeshFallbackPrimitive {
            source: state.source.clone(),
            primitive_kind: state.primitive_kind,
            transform: Transform::from_translation(state.local_position).with_scale(state.scale),
            color: state.color,
            alpha: state.alpha,
            emissive: state.emissive,
            material_profile_id: state.material_profile_id.clone(),
            lifecycle: state.lifecycle,
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FangyuanVfxInstanceStartError {
    DuplicateInstance(FangyuanVfxInstanceId),
    InvalidRecipe(FangyuanVfxDiagnostic),
}

#[derive(Clone, Debug)]
pub struct FangyuanVfxInstance {
    pub id: FangyuanVfxInstanceId,
    pub recipe: FangyuanVfxRecipe,
    pub context: FangyuanVfxReplayContext,
    pub start_tick: u64,
}

impl FangyuanVfxInstance {
    pub fn new(
        id: impl Into<String>,
        recipe: FangyuanVfxRecipe,
        context: FangyuanVfxReplayContext,
        start_tick: u64,
    ) -> Self {
        Self {
            id: FangyuanVfxInstanceId::new(id),
            recipe,
            context,
            start_tick,
        }
    }

    fn evaluate_with_budget_pressure(
        &self,
        current_tick: u64,
        ticks_per_second: u32,
        pressure: FangyuanVfxBudgetPressure,
    ) -> Result<Vec<FangyuanVfxDynamicPrimitiveState>, FangyuanVfxDiagnostic> {
        let clock = FangyuanVfxClock::new(self.start_tick, current_tick, ticks_per_second);
        let mut states = evaluate_fangyuan_vfx_recipe_with_budget_pressure(
            &self.recipe,
            clock,
            &self.context,
            pressure,
        )?;
        for state in &mut states {
            state.source = state.source.clone().with_instance_id(self.id.clone());
        }
        Ok(states)
    }

    fn is_finished(&self, current_tick: u64) -> bool {
        current_tick.saturating_sub(self.start_tick) > self.recipe.duration_ticks
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FangyuanVfxRuntimeStats {
    pub active_instance_count: usize,
    pub active_state_count: usize,
    pub total_started: u64,
    pub total_finished: u64,
    pub total_cleared: u64,
    pub fallback_primitive_count: usize,
}

#[derive(Clone, Debug, Default)]
pub struct FangyuanVfxRuntime {
    instances: BTreeMap<FangyuanVfxInstanceId, FangyuanVfxInstance>,
    active_states: Vec<FangyuanVfxDynamicPrimitiveState>,
    fallback_primitives: Vec<FangyuanVfxStandardMeshFallbackPrimitive>,
    stats: FangyuanVfxRuntimeStats,
}

impl FangyuanVfxRuntime {
    pub fn start_instance(
        &mut self,
        instance: FangyuanVfxInstance,
    ) -> Result<(), FangyuanVfxInstanceStartError> {
        instance
            .recipe
            .validate()
            .map_err(FangyuanVfxInstanceStartError::InvalidRecipe)?;
        if self.instances.contains_key(&instance.id) {
            return Err(FangyuanVfxInstanceStartError::DuplicateInstance(
                instance.id,
            ));
        }

        self.instances.insert(instance.id.clone(), instance);
        self.stats.total_started += 1;
        self.refresh_counts();
        Ok(())
    }

    pub fn tick(
        &mut self,
        current_tick: u64,
        ticks_per_second: u32,
    ) -> Result<&[FangyuanVfxDynamicPrimitiveState], FangyuanVfxDiagnostic> {
        self.tick_with_budget_pressure(
            current_tick,
            ticks_per_second,
            FangyuanVfxBudgetPressure::none(),
        )
    }

    pub fn tick_with_budget_pressure(
        &mut self,
        current_tick: u64,
        ticks_per_second: u32,
        pressure: FangyuanVfxBudgetPressure,
    ) -> Result<&[FangyuanVfxDynamicPrimitiveState], FangyuanVfxDiagnostic> {
        let mut finished = Vec::new();
        let mut active_states = Vec::new();

        for (id, instance) in &self.instances {
            if instance.is_finished(current_tick) {
                finished.push(id.clone());
                continue;
            }
            active_states.extend(instance.evaluate_with_budget_pressure(
                current_tick,
                ticks_per_second,
                pressure,
            )?);
        }

        for id in finished {
            self.instances.remove(&id);
            self.stats.total_finished += 1;
        }

        self.active_states = active_states;
        self.fallback_primitives =
            fangyuan_vfx_standard_mesh_fallback_primitives(&self.active_states);
        self.refresh_counts();
        Ok(&self.active_states)
    }

    pub fn clear_scene(&mut self) {
        let cleared = self.instances.len() as u64;
        self.instances.clear();
        self.active_states.clear();
        self.fallback_primitives.clear();
        self.stats.total_cleared += cleared;
        self.refresh_counts();
    }

    pub fn reload_scene(&mut self) {
        self.clear_scene();
    }

    pub fn active_states(&self) -> &[FangyuanVfxDynamicPrimitiveState] {
        &self.active_states
    }

    pub fn fallback_primitives(&self) -> &[FangyuanVfxStandardMeshFallbackPrimitive] {
        &self.fallback_primitives
    }

    pub fn stats(&self) -> FangyuanVfxRuntimeStats {
        self.stats
    }

    fn refresh_counts(&mut self) {
        self.stats.active_instance_count = self.instances.len();
        self.stats.active_state_count = self.active_states.len();
        self.stats.fallback_primitive_count = self.fallback_primitives.len();
    }
}

pub fn evaluate_fangyuan_vfx_recipe(
    recipe: &FangyuanVfxRecipe,
    clock: FangyuanVfxClock,
    context: &FangyuanVfxReplayContext,
) -> Result<Vec<FangyuanVfxDynamicPrimitiveState>, FangyuanVfxDiagnostic> {
    evaluate_fangyuan_vfx_recipe_with_budget_pressure(
        recipe,
        clock,
        context,
        FangyuanVfxBudgetPressure::none(),
    )
}

pub fn estimate_fangyuan_vfx_recipe_budget(
    recipe: &FangyuanVfxRecipe,
) -> FangyuanVfxBudgetEstimate {
    let mut estimate = FangyuanVfxBudgetEstimate {
        duration_ticks: recipe.duration_ticks,
        emitter_count: recipe.emitters.len(),
        ..Default::default()
    };
    let mut material_profiles = Vec::<&str>::new();

    for emitter in &recipe.emitters {
        if !estimate.roles.contains(&emitter.role) {
            estimate.roles.push(emitter.role);
        }
        if emitter.color[3] < 1.0 || emitter.operators.iter().any(operator_may_use_alpha) {
            estimate.alpha_primitives += 1;
            estimate.transparent_primitives += 1;
        }
        if emitter.emissive > 0.0 || emitter.operators.iter().any(operator_may_use_emissive) {
            estimate.emissive_primitives += 1;
        }
        if let Some(material_profile_id) = emitter.material_profile_id.as_deref()
            && !material_profiles.contains(&material_profile_id)
        {
            material_profiles.push(material_profile_id);
        }

        for operator in &emitter.operators {
            if let FangyuanVfxOperator::Trail { segments, .. } = operator {
                let segments = usize::from(*segments);
                estimate.trail_segments += segments;
                estimate.peak_primitives += segments;
                estimate.alpha_primitives += segments;
                estimate.transparent_primitives += segments;
                if !estimate.roles.contains(&FangyuanPrimitiveRole::Trail) {
                    estimate.roles.push(FangyuanPrimitiveRole::Trail);
                }
            }
        }
    }

    estimate.peak_primitives += estimate.emitter_count;
    estimate.material_profile_count = material_profiles.len();
    estimate.roles.sort_by_key(|role| role.as_str());
    estimate
}

pub fn audit_fangyuan_vfx_recipe(recipe: &FangyuanVfxRecipe) -> FangyuanAuditReport {
    audit_fangyuan_vfx_recipe_with_profile(recipe, &FangyuanVfxBudgetProfile::default())
}

pub fn audit_fangyuan_vfx_recipe_with_profile(
    recipe: &FangyuanVfxRecipe,
    profile: &FangyuanVfxBudgetProfile,
) -> FangyuanAuditReport {
    let estimate = estimate_fangyuan_vfx_recipe_budget(recipe);
    let mut report =
        FangyuanAuditReport::new(FangyuanAuditSourceKind::Unknown, Some(recipe.id.clone()));

    if estimate.duration_ticks > profile.recommended_duration_ticks {
        add_vfx_finding_with_suggestion(
            &mut report,
            "vfx_duration_above_recommended",
            "duration_ticks",
            "VFX duration exceeds the recommended budget",
            FangyuanAuditSuggestionAction::ShrinkBounds,
            "shorten duration or residue ticks to reduce long-lived VFX pressure",
        );
    }
    if estimate.peak_primitives > profile.recommended_peak_primitives {
        add_vfx_finding_with_suggestion(
            &mut report,
            "vfx_peak_primitives_above_recommended",
            "emitters[].operators",
            "peak VFX primitive count exceeds the recommended budget",
            FangyuanAuditSuggestionAction::ReducePrimitives,
            "reduce decoration emitters or trail segments before removing readable core states",
        );
    }
    if estimate.trail_segments > profile.recommended_trail_segments {
        add_vfx_finding_with_suggestion(
            &mut report,
            "vfx_trail_segments_above_recommended",
            "emitters[].operators[type=trail].segments",
            "trail segment count exceeds the recommended budget",
            FangyuanAuditSuggestionAction::ReducePrimitives,
            "reduce trail segments or increase spacing_ticks",
        );
    }
    if estimate.alpha_primitives > profile.recommended_alpha_primitives {
        add_vfx_finding_with_suggestion(
            &mut report,
            "vfx_alpha_above_recommended",
            "emitters[].color[3]",
            "alpha and transparent VFX count exceeds the recommended budget",
            FangyuanAuditSuggestionAction::RemoveAlpha,
            "lower alpha usage or make decorative residue opaque",
        );
    }
    if estimate.emissive_primitives > profile.recommended_emissive_primitives {
        add_vfx_finding_with_suggestion(
            &mut report,
            "vfx_emissive_count_above_recommended",
            "emitters[].emissive",
            "emissive VFX count exceeds the recommended budget",
            FangyuanAuditSuggestionAction::LowerEmissive,
            "lower emissive amplitude or limit it to core impact states",
        );
    }
    if estimate.material_profile_count > profile.recommended_material_profiles {
        add_vfx_finding_with_suggestion(
            &mut report,
            "vfx_material_profile_count_above_recommended",
            "emitters[].material_profile_id",
            "material profile count exceeds the recommended VFX budget",
            FangyuanAuditSuggestionAction::ReplaceMaterialProfile,
            "reuse fewer VFX material profiles",
        );
    }
    for (index, emitter) in recipe.emitters.iter().enumerate() {
        if emitter.emissive > profile.max_emissive_intensity {
            add_vfx_finding_with_suggestion(
                &mut report,
                "vfx_emissive_intensity_above_recommended",
                format!("emitters[{index}].emissive"),
                "emissive intensity exceeds the recommended VFX range",
                FangyuanAuditSuggestionAction::LowerEmissive,
                "lower emissive intensity to keep bloom and material cost predictable",
            );
        }
    }
    if !estimate.roles.iter().any(|role| {
        matches!(
            role,
            FangyuanPrimitiveRole::Core
                | FangyuanPrimitiveRole::Boundary
                | FangyuanPrimitiveRole::Warning
                | FangyuanPrimitiveRole::Impact
        )
    }) {
        add_vfx_finding_with_suggestion(
            &mut report,
            "vfx_role_missing_readable_anchor",
            "emitters[].role",
            "recipe has no readable core, boundary, warning, or impact role",
            FangyuanAuditSuggestionAction::ReduceWarningRole,
            "assign at least one primary gameplay-readable role before decoration",
        );
    }

    report.refresh_summary_and_status();
    report.sort_findings();
    report.sort_suggestions();
    report
}

pub fn fangyuan_vfx_primitive_state_hash(states: &[FangyuanVfxDynamicPrimitiveState]) -> u64 {
    let mut hash = FNV_OFFSET;
    for state in states {
        mix_str(&mut hash, &state.recipe_id);
        mix_str(&mut hash, &state.emitter_id);
        mix_u64(&mut hash, state.emitter_index as u64);
        mix_u64(&mut hash, u64::from(state.primitive_index));
        mix_u64(&mut hash, state.source_tick);
        mix_str(&mut hash, state.primitive_kind.as_str());
        mix_str(&mut hash, state.role.as_str());
        mix_f32(&mut hash, state.local_position.x);
        mix_f32(&mut hash, state.local_position.y);
        mix_f32(&mut hash, state.local_position.z);
        mix_f32(&mut hash, state.scale.x);
        mix_f32(&mut hash, state.scale.y);
        mix_f32(&mut hash, state.scale.z);
        let color = state.color.to_srgba();
        mix_f32(&mut hash, color.red);
        mix_f32(&mut hash, color.green);
        mix_f32(&mut hash, color.blue);
        mix_f32(&mut hash, state.alpha);
        mix_f32(&mut hash, state.emissive);
        if let Some(profile) = state.material_profile_id.as_deref() {
            mix_str(&mut hash, profile);
        }
        mix_u64(&mut hash, state.seed);
    }
    avalanche(hash)
}

pub fn evaluate_fangyuan_vfx_recipe_with_budget_pressure(
    recipe: &FangyuanVfxRecipe,
    clock: FangyuanVfxClock,
    context: &FangyuanVfxReplayContext,
    pressure: FangyuanVfxBudgetPressure,
) -> Result<Vec<FangyuanVfxDynamicPrimitiveState>, FangyuanVfxDiagnostic> {
    recipe.validate()?;

    if clock.ticks_per_second == 0 {
        return Err(FangyuanVfxDiagnostic::new(
            FangyuanVfxDiagnosticCode::InvalidClock,
            "ticks_per_second must be greater than zero",
        ));
    }

    if clock.is_before_start() {
        return Ok(Vec::new());
    }

    if clock.is_past_duration(recipe.duration_ticks) {
        return Ok(Vec::new());
    }

    let mut states = Vec::new();
    for (emitter_index, emitter) in recipe.emitters.iter().enumerate() {
        let elapsed_ticks = clock.elapsed_ticks();
        if elapsed_ticks < emitter.delay_ticks {
            continue;
        }

        let local_elapsed = elapsed_ticks - emitter.delay_ticks;
        let emitter_duration = emitter.duration_ticks.unwrap_or_else(|| {
            recipe
                .duration_ticks
                .saturating_sub(emitter.delay_ticks)
                .max(1)
        });
        if local_elapsed > emitter_duration {
            continue;
        }

        let seed = compose_fangyuan_vfx_seed(recipe, context, clock.start_tick, emitter_index);
        let base = evaluate_emitter_state(
            recipe,
            emitter,
            emitter_index,
            local_elapsed,
            emitter_duration,
            clock,
            seed,
            0,
        );
        if !(pressure.skip_decoration && base.role == FangyuanPrimitiveRole::Decoration) {
            states.push(base.clone());
        }

        let mut emitted_trails = states
            .iter()
            .filter(|state| state.role == FangyuanPrimitiveRole::Trail)
            .count();
        for operator in &emitter.operators {
            if let FangyuanVfxOperator::Trail {
                segments,
                spacing_ticks,
                fade,
            } = operator
            {
                let segment_budget = pressure
                    .max_trail_segments
                    .unwrap_or(recipe.budget_hints.max_trail_segments)
                    .min(recipe.budget_hints.max_trail_segments);
                let segment_count = (*segments).min(segment_budget);
                for segment in 1..=segment_count {
                    if pressure
                        .max_trail_segments
                        .is_some_and(|max| emitted_trails >= usize::from(max))
                    {
                        break;
                    }
                    let segment_offset = u64::from(segment).saturating_mul(*spacing_ticks);
                    if local_elapsed < segment_offset {
                        continue;
                    }

                    let segment_elapsed = local_elapsed - segment_offset;
                    let mut trail = evaluate_emitter_state(
                        recipe,
                        emitter,
                        emitter_index,
                        segment_elapsed,
                        emitter_duration,
                        clock,
                        seed,
                        segment,
                    );
                    trail.primitive_index = segment;
                    trail.source_tick = clock.current_tick.saturating_sub(segment_offset);
                    trail.role = FangyuanPrimitiveRole::Trail;
                    trail.alpha *= fade.clamp(0.0, 1.0).powi(i32::from(segment));
                    states.push(trail);
                    emitted_trails += 1;
                }
            }
        }
    }

    let max_primitives = pressure
        .max_primitives
        .unwrap_or(recipe.budget_hints.max_primitives as usize)
        .min(recipe.budget_hints.max_primitives as usize);
    apply_fangyuan_vfx_budget_pressure(&mut states, max_primitives);
    Ok(states)
}

pub fn fangyuan_vfx_projectile_recipe() -> FangyuanVfxRecipe {
    recipe_with_single_emitter(
        "vfx.projectile",
        "projectile",
        FangyuanPrimitiveKind::Sphere,
        FangyuanPrimitiveRole::Core,
        [0.2, 0.2, 0.2],
        [0.9, 0.25, 0.15, 1.0],
        vec![
            FangyuanVfxOperator::Spawn {
                curve: FangyuanVfxCurve::EaseOut,
            },
            FangyuanVfxOperator::Move {
                from: [0.0, 0.8, 0.0],
                to: [6.0, 0.8, 0.0],
                curve: FangyuanVfxCurve::Linear,
            },
            FangyuanVfxOperator::Trail {
                segments: 3,
                spacing_ticks: 2,
                fade: 0.55,
            },
        ],
    )
}

pub fn fangyuan_vfx_range_marker_recipe() -> FangyuanVfxRecipe {
    recipe_with_single_emitter(
        "vfx.range_marker",
        "range_marker",
        FangyuanPrimitiveKind::Cube,
        FangyuanPrimitiveRole::Warning,
        [1.0, 0.04, 1.0],
        [0.2, 0.8, 1.0, 0.45],
        vec![
            FangyuanVfxOperator::Spawn {
                curve: FangyuanVfxCurve::Step,
            },
            FangyuanVfxOperator::Scale {
                from: [1.0, 1.0, 1.0],
                to: [4.0, 1.0, 4.0],
                curve: FangyuanVfxCurve::EaseOut,
            },
        ],
    )
}

pub fn fangyuan_vfx_shield_recipe() -> FangyuanVfxRecipe {
    recipe_with_single_emitter(
        "vfx.shield",
        "shield",
        FangyuanPrimitiveKind::Sphere,
        FangyuanPrimitiveRole::Boundary,
        [2.0, 2.0, 2.0],
        [0.25, 0.65, 1.0, 0.35],
        vec![
            FangyuanVfxOperator::Spawn {
                curve: FangyuanVfxCurve::EaseOut,
            },
            FangyuanVfxOperator::EmissivePulse {
                amplitude: 1.5,
                frequency_hz: 3.0,
                curve: FangyuanVfxCurve::Linear,
            },
        ],
    )
}

pub fn fangyuan_vfx_impact_expand_recipe() -> FangyuanVfxRecipe {
    recipe_with_single_emitter(
        "vfx.impact_expand",
        "impact_expand",
        FangyuanPrimitiveKind::Sphere,
        FangyuanPrimitiveRole::Impact,
        [1.0, 1.0, 1.0],
        [1.0, 0.55, 0.15, 0.8],
        vec![FangyuanVfxOperator::ImpactExpand {
            radius_from: 0.1,
            radius_to: 2.5,
            curve: FangyuanVfxCurve::EaseOut,
        }],
    )
}

pub fn fangyuan_vfx_fade_recipe() -> FangyuanVfxRecipe {
    recipe_with_single_emitter(
        "vfx.fade",
        "fade",
        FangyuanPrimitiveKind::Cube,
        FangyuanPrimitiveRole::Decoration,
        [1.0, 1.0, 1.0],
        [0.8, 0.8, 0.8, 1.0],
        vec![FangyuanVfxOperator::Fade {
            from: 1.0,
            to: 0.0,
            curve: FangyuanVfxCurve::Linear,
        }],
    )
}

pub fn compose_fangyuan_vfx_seed(
    recipe: &FangyuanVfxRecipe,
    context: &FangyuanVfxReplayContext,
    start_tick: u64,
    emitter_index: usize,
) -> u64 {
    match recipe.seed_policy {
        FangyuanVfxSeedPolicy::Fixed(seed) => seed ^ emitter_index as u64,
        FangyuanVfxSeedPolicy::External => context
            .external_seed
            .unwrap_or(0)
            .wrapping_add(emitter_index as u64),
        FangyuanVfxSeedPolicy::Deterministic => {
            let mut hash = FNV_OFFSET;
            mix_str(&mut hash, &recipe.id);
            mix_u64(&mut hash, u64::from(recipe.version));
            mix_str(&mut hash, &context.caster_id);
            mix_str(&mut hash, &context.event_id);
            mix_u64(&mut hash, start_tick);
            mix_u64(&mut hash, emitter_index as u64);
            avalanche(hash)
        }
    }
}

fn add_vfx_finding_with_suggestion(
    report: &mut FangyuanAuditReport,
    code: impl Into<String>,
    field_path: impl Into<String>,
    reason: impl Into<String>,
    action: FangyuanAuditSuggestionAction,
    estimated_effect: impl Into<String>,
) {
    let field_path = field_path.into();
    let reason = reason.into();
    let mut finding = FangyuanAuditFinding::new(
        FangyuanAuditSeverity::Warning,
        code,
        reason.clone(),
        FangyuanAuditSourceKind::Unknown,
    );
    finding.field_path = Some(field_path.clone());
    report.add_finding(finding);
    report.add_suggestion(FangyuanAuditSuggestion::new_with_effect(
        action,
        Some(field_path),
        reason,
        estimated_effect,
    ));
}

fn operator_may_use_alpha(operator: &FangyuanVfxOperator) -> bool {
    matches!(
        operator,
        FangyuanVfxOperator::Fade { .. }
            | FangyuanVfxOperator::ColorShift { .. }
            | FangyuanVfxOperator::Trail { .. }
            | FangyuanVfxOperator::Spawn { .. }
    )
}

fn operator_may_use_emissive(operator: &FangyuanVfxOperator) -> bool {
    matches!(operator, FangyuanVfxOperator::EmissivePulse { .. })
}

fn apply_fangyuan_vfx_budget_pressure(
    states: &mut Vec<FangyuanVfxDynamicPrimitiveState>,
    max_primitives: usize,
) {
    if states.len() <= max_primitives {
        return;
    }

    states.sort_by_key(|state| fangyuan_vfx_role_keep_priority(state.role));
    states.truncate(max_primitives);
    states.sort_by_key(|state| {
        (
            state.emitter_index,
            state.primitive_index,
            state.source_tick,
            state.role.as_str(),
        )
    });
}

fn fangyuan_vfx_role_keep_priority(role: FangyuanPrimitiveRole) -> u8 {
    match role {
        FangyuanPrimitiveRole::Core
        | FangyuanPrimitiveRole::Boundary
        | FangyuanPrimitiveRole::Warning
        | FangyuanPrimitiveRole::Impact => 0,
        FangyuanPrimitiveRole::Trail => 1,
        FangyuanPrimitiveRole::Structure => 2,
        FangyuanPrimitiveRole::Decoration => 3,
        FangyuanPrimitiveRole::Socket | FangyuanPrimitiveRole::Archive => 4,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanVfxDiagnostic {
    pub code: FangyuanVfxDiagnosticCode,
    pub message: String,
    pub emitter_index: Option<usize>,
}

impl FangyuanVfxDiagnostic {
    pub fn new(code: FangyuanVfxDiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            emitter_index: None,
        }
    }

    fn with_emitter(
        code: FangyuanVfxDiagnosticCode,
        message: impl Into<String>,
        emitter_index: usize,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            emitter_index: Some(emitter_index),
        }
    }

    fn empty_recipe_id() -> Self {
        Self::new(
            FangyuanVfxDiagnosticCode::EmptyRecipeId,
            "recipe id must not be empty",
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanVfxDiagnosticCode {
    EmptyRecipeId,
    InvalidDuration,
    MissingEmitter,
    MissingEmitterId,
    MissingOperator,
    InvalidEmitter,
    InvalidOperator,
    InvalidClock,
}

fn evaluate_emitter_state(
    recipe: &FangyuanVfxRecipe,
    emitter: &FangyuanVfxEmitter,
    emitter_index: usize,
    local_elapsed: u64,
    emitter_duration: u64,
    clock: FangyuanVfxClock,
    seed: u64,
    primitive_index: u16,
) -> FangyuanVfxDynamicPrimitiveState {
    let progress = if emitter_duration == 0 {
        1.0
    } else {
        (local_elapsed as f32 / emitter_duration as f32).clamp(0.0, 1.0)
    };
    let random = FangyuanVfxSeedStream::new(seed ^ u64::from(primitive_index));
    let mut position = Vec3::from_array(emitter.position) + random.vec3(emitter.jitter.position);
    let mut scale =
        Vec3::from_array(emitter.scale) + Vec3::splat(random.scalar(emitter.jitter.scale));
    let mut color = emitter.color;
    let mut alpha = color[3] + random.scalar(emitter.jitter.alpha);
    let color_jitter = random.vec3(emitter.jitter.color);
    color[0] += color_jitter.x;
    color[1] += color_jitter.y;
    color[2] += color_jitter.z;
    let mut emissive = emitter.emissive;
    let mut role = emitter.role;

    for operator in &emitter.operators {
        match operator {
            FangyuanVfxOperator::Spawn { curve } => {
                alpha *= curve.sample(progress);
            }
            FangyuanVfxOperator::Move { from, to, curve } => {
                position += lerp_vec3(*from, *to, curve.sample(progress));
            }
            FangyuanVfxOperator::Scale { from, to, curve } => {
                scale *= lerp_vec3(*from, *to, curve.sample(progress));
            }
            FangyuanVfxOperator::Fade { from, to, curve } => {
                alpha *= lerp_f32(*from, *to, curve.sample(progress));
            }
            FangyuanVfxOperator::ColorShift { from, to, curve } => {
                color = lerp_color_array(*from, *to, curve.sample(progress));
            }
            FangyuanVfxOperator::EmissivePulse {
                amplitude,
                frequency_hz,
                curve,
            } => {
                let wave = (clock.elapsed_seconds() * frequency_hz * std::f32::consts::TAU).sin();
                emissive += amplitude * (0.5 + 0.5 * wave) * curve.sample(progress);
            }
            FangyuanVfxOperator::Trail { .. } => {}
            FangyuanVfxOperator::ImpactExpand {
                radius_from,
                radius_to,
                curve,
            } => {
                let radius = lerp_f32(*radius_from, *radius_to, curve.sample(progress));
                scale *= Vec3::splat(radius.max(0.0));
                role = FangyuanPrimitiveRole::Impact;
            }
        }
    }

    let alpha = alpha.clamp(0.0, 1.0);
    let color = Color::srgba(
        color[0].clamp(0.0, 1.0),
        color[1].clamp(0.0, 1.0),
        color[2].clamp(0.0, 1.0),
        alpha,
    );

    FangyuanVfxDynamicPrimitiveState {
        recipe_id: recipe.id.clone(),
        emitter_id: emitter.id.clone(),
        emitter_index,
        primitive_index,
        source_tick: clock.current_tick,
        source: FangyuanVfxPrimitiveSource::from_state_parts(
            recipe.id.clone(),
            emitter.id.clone(),
            emitter_index,
            primitive_index,
            clock.current_tick,
        ),
        local_position: position,
        scale,
        color,
        alpha,
        emissive: emissive.max(0.0),
        primitive_kind: emitter.primitive_kind,
        role,
        material_profile_id: emitter.material_profile_id.clone(),
        lifecycle: FangyuanPrimitiveLifecycle::new(
            Some(recipe.duration_ticks),
            Some(clock.start_tick),
            Some(clock.start_tick.saturating_add(recipe.duration_ticks)),
        ),
        seed,
    }
}

#[derive(Clone, Copy, Debug)]
struct FangyuanVfxSeedStream {
    seed: u64,
}

impl FangyuanVfxSeedStream {
    const fn new(seed: u64) -> Self {
        Self { seed }
    }

    fn scalar(self, amplitude: f32) -> f32 {
        if amplitude == 0.0 {
            return 0.0;
        }
        (unit_from_seed(self.seed) * 2.0 - 1.0) * amplitude
    }

    fn vec3(self, amplitude: [f32; 3]) -> Vec3 {
        Vec3::new(
            self.axis(0, amplitude[0]),
            self.axis(1, amplitude[1]),
            self.axis(2, amplitude[2]),
        )
    }

    fn axis(self, axis: u64, amplitude: f32) -> f32 {
        if amplitude == 0.0 {
            return 0.0;
        }
        let seed = avalanche(self.seed ^ axis.wrapping_mul(0x9e37_79b9_7f4a_7c15));
        (unit_from_seed(seed) * 2.0 - 1.0) * amplitude
    }
}

fn validate_finite_vec3(
    value: [f32; 3],
    code: FangyuanVfxDiagnosticCode,
) -> Result<(), FangyuanVfxDiagnostic> {
    if value.iter().all(|component| component.is_finite()) {
        Ok(())
    } else {
        Err(FangyuanVfxDiagnostic::new(
            code,
            "vector components must be finite",
        ))
    }
}

fn validate_finite_color(
    value: [f32; 4],
    code: FangyuanVfxDiagnosticCode,
) -> Result<(), FangyuanVfxDiagnostic> {
    if value.iter().all(|component| component.is_finite()) {
        Ok(())
    } else {
        Err(FangyuanVfxDiagnostic::new(
            code,
            "color components must be finite",
        ))
    }
}

fn lerp_vec3(from: [f32; 3], to: [f32; 3], t: f32) -> Vec3 {
    Vec3::from_array(from).lerp(Vec3::from_array(to), t)
}

fn lerp_color_array(from: [f32; 4], to: [f32; 4], t: f32) -> [f32; 4] {
    [
        lerp_f32(from[0], to[0], t),
        lerp_f32(from[1], to[1], t),
        lerp_f32(from[2], to[2], t),
        lerp_f32(from[3], to[3], t),
    ]
}

fn lerp_f32(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

fn unit_from_seed(seed: u64) -> f32 {
    let value = avalanche(seed) >> 40;
    value as f32 / 0x00ff_ffff_u32 as f32
}

fn mix_str(hash: &mut u64, value: &str) {
    for byte in value.as_bytes() {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(FNV_PRIME);
    }
}

fn mix_u64(hash: &mut u64, value: u64) {
    for byte in value.to_le_bytes() {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(FNV_PRIME);
    }
}

fn mix_f32(hash: &mut u64, value: f32) {
    let canonical = if value == 0.0 { 0.0 } else { value };
    mix_u64(hash, u64::from(canonical.to_bits()));
}

fn avalanche(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn unit_vec3_array() -> [f32; 3] {
    [1.0, 1.0, 1.0]
}

fn white_color_array() -> [f32; 4] {
    [1.0, 1.0, 1.0, 1.0]
}

fn recipe_with_single_emitter(
    recipe_id: &str,
    emitter_id: &str,
    primitive_kind: FangyuanPrimitiveKind,
    role: FangyuanPrimitiveRole,
    scale: [f32; 3],
    color: [f32; 4],
    operators: Vec<FangyuanVfxOperator>,
) -> FangyuanVfxRecipe {
    FangyuanVfxRecipe {
        id: recipe_id.to_string(),
        version: 1,
        duration_ticks: 20,
        seed_policy: FangyuanVfxSeedPolicy::Deterministic,
        emitters: vec![FangyuanVfxEmitter {
            id: emitter_id.to_string(),
            primitive_kind,
            role,
            delay_ticks: 0,
            duration_ticks: Some(20),
            position: [0.0, 0.0, 0.0],
            scale,
            color,
            emissive: 0.0,
            material_profile_id: Some("vfx/default".to_string()),
            jitter: FangyuanVfxEmitterJitter::default(),
            operators,
        }],
        curves: Vec::new(),
        budget_hints: FangyuanVfxBudgetHints {
            max_primitives: 16,
            max_trail_segments: 8,
            max_emitters: 1,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_recipe() -> FangyuanVfxRecipe {
        FangyuanVfxRecipe {
            id: "skill.impact.arc".to_string(),
            version: 3,
            duration_ticks: 30,
            seed_policy: FangyuanVfxSeedPolicy::Deterministic,
            emitters: vec![FangyuanVfxEmitter {
                id: "core".to_string(),
                primitive_kind: FangyuanPrimitiveKind::Sphere,
                role: FangyuanPrimitiveRole::Core,
                delay_ticks: 0,
                duration_ticks: Some(30),
                position: [0.0, 0.0, 0.0],
                scale: [1.0, 1.0, 1.0],
                color: [0.2, 0.4, 0.8, 1.0],
                emissive: 0.25,
                material_profile_id: Some("vfx_glow".to_string()),
                jitter: FangyuanVfxEmitterJitter {
                    position: [0.1, 0.0, 0.0],
                    scale: 0.05,
                    alpha: 0.0,
                    color: [0.05, 0.0, 0.0],
                },
                operators: vec![
                    FangyuanVfxOperator::Spawn {
                        curve: FangyuanVfxCurve::Linear,
                    },
                    FangyuanVfxOperator::Move {
                        from: [0.0, 0.0, 0.0],
                        to: [3.0, 0.0, 0.0],
                        curve: FangyuanVfxCurve::Linear,
                    },
                    FangyuanVfxOperator::Scale {
                        from: [1.0, 1.0, 1.0],
                        to: [2.0, 2.0, 2.0],
                        curve: FangyuanVfxCurve::EaseOut,
                    },
                    FangyuanVfxOperator::Fade {
                        from: 1.0,
                        to: 0.2,
                        curve: FangyuanVfxCurve::Linear,
                    },
                    FangyuanVfxOperator::ColorShift {
                        from: [0.2, 0.4, 0.8, 1.0],
                        to: [1.0, 0.6, 0.1, 0.5],
                        curve: FangyuanVfxCurve::Linear,
                    },
                    FangyuanVfxOperator::EmissivePulse {
                        amplitude: 1.0,
                        frequency_hz: 2.0,
                        curve: FangyuanVfxCurve::Linear,
                    },
                    FangyuanVfxOperator::Trail {
                        segments: 2,
                        spacing_ticks: 3,
                        fade: 0.5,
                    },
                    FangyuanVfxOperator::ImpactExpand {
                        radius_from: 0.75,
                        radius_to: 1.5,
                        curve: FangyuanVfxCurve::Linear,
                    },
                ],
            }],
            curves: vec![FangyuanVfxCurveBinding {
                id: "default".to_string(),
                curve: FangyuanVfxCurve::Linear,
            }],
            budget_hints: FangyuanVfxBudgetHints {
                max_primitives: 8,
                max_trail_segments: 4,
                max_emitters: 2,
            },
        }
    }

    #[test]
    fn fangyuan_vfx_recipe_default_recipe_evaluates_dynamic_state() {
        let recipe = sample_recipe();
        let clock = FangyuanVfxClock::new(100, 115, 30);
        let context = FangyuanVfxReplayContext::local("caster-a", "event-a");

        let states = evaluate_fangyuan_vfx_recipe(&recipe, clock, &context).unwrap();

        assert_eq!(states.len(), 3);
        let state = &states[0];
        assert_eq!(state.recipe_id, "skill.impact.arc");
        assert_eq!(state.emitter_id, "core");
        assert_eq!(state.primitive_kind, FangyuanPrimitiveKind::Sphere);
        assert_eq!(state.role, FangyuanPrimitiveRole::Impact);
        assert_eq!(state.material_profile_id.as_deref(), Some("vfx_glow"));
        assert_eq!(state.profile(), Some("vfx_glow"));
        assert_eq!(state.lifecycle.spawn_tick, Some(100));
        assert_eq!(state.lifecycle.despawn_tick, Some(130));
        assert_eq!(state.lifetime().lifetime, Some(30));
        assert_eq!(state.position(), state.local_position);
        assert_eq!(state.source.recipe_id, "skill.impact.arc");
        assert_eq!(state.source.emitter_id, "core");
        assert!(state.local_position.x > 1.4 && state.local_position.x < 1.7);
        assert!(state.scale.x > 1.6);
        assert!(state.alpha > 0.0 && state.alpha < 1.0);
        assert!(state.emissive >= 0.25);

        let primitive = state.to_runtime_primitive();
        assert_eq!(primitive.local_position(), state.local_position);
        assert_eq!(primitive.scale(), state.scale);
        assert_eq!(primitive.material_profile_id(), Some("vfx_glow"));
    }

    #[test]
    fn fangyuan_vfx_eval_projectile_range_marker_shield_impact_trail_and_fade_states() {
        let context = FangyuanVfxReplayContext::local("caster-a", "event-a");
        let clock = FangyuanVfxClock::new(0, 10, 20);

        let projectile =
            evaluate_fangyuan_vfx_recipe(&fangyuan_vfx_projectile_recipe(), clock, &context)
                .unwrap();
        assert!(projectile[0].local_position.x > 2.9);
        assert!(projectile.iter().any(|state| {
            state.role == FangyuanPrimitiveRole::Trail && state.primitive_index > 0
        }));

        let range_marker =
            evaluate_fangyuan_vfx_recipe(&fangyuan_vfx_range_marker_recipe(), clock, &context)
                .unwrap();
        assert_eq!(range_marker[0].role, FangyuanPrimitiveRole::Warning);
        assert!(range_marker[0].scale.x > 2.0);
        assert!(range_marker[0].scale.z > 2.0);

        let shield =
            evaluate_fangyuan_vfx_recipe(&fangyuan_vfx_shield_recipe(), clock, &context).unwrap();
        assert_eq!(shield[0].role, FangyuanPrimitiveRole::Boundary);
        assert!(shield[0].emissive >= 0.0);

        let impact =
            evaluate_fangyuan_vfx_recipe(&fangyuan_vfx_impact_expand_recipe(), clock, &context)
                .unwrap();
        assert_eq!(impact[0].role, FangyuanPrimitiveRole::Impact);
        assert!(impact[0].scale.x > 1.0);

        let fade =
            evaluate_fangyuan_vfx_recipe(&fangyuan_vfx_fade_recipe(), clock, &context).unwrap();
        assert_eq!(fade[0].role, FangyuanPrimitiveRole::Decoration);
        assert!(fade[0].alpha > 0.0 && fade[0].alpha < 1.0);
    }

    #[test]
    fn fangyuan_vfx_eval_empty_recipe_and_invalid_clock_return_diagnostics() {
        let mut recipe = sample_recipe();
        recipe.emitters.clear();
        let context = FangyuanVfxReplayContext::local("caster-a", "event-a");

        let diagnostic =
            evaluate_fangyuan_vfx_recipe(&recipe, FangyuanVfxClock::new(0, 0, 30), &context)
                .unwrap_err();
        assert_eq!(diagnostic.code, FangyuanVfxDiagnosticCode::MissingEmitter);

        let diagnostic = evaluate_fangyuan_vfx_recipe(
            &sample_recipe(),
            FangyuanVfxClock::new(0, 0, 0),
            &context,
        )
        .unwrap_err();
        assert_eq!(diagnostic.code, FangyuanVfxDiagnosticCode::InvalidClock);
    }

    #[test]
    fn fangyuan_vfx_eval_negative_time_equivalent_before_start_is_empty() {
        let recipe = sample_recipe();
        let context = FangyuanVfxReplayContext::local("caster-a", "event-a");

        let states =
            evaluate_fangyuan_vfx_recipe(&recipe, FangyuanVfxClock::new(100, 90, 30), &context)
                .unwrap();

        assert!(states.is_empty());
    }

    #[test]
    fn fangyuan_vfx_eval_standard_mesh_fallback_maps_dynamic_state() {
        let recipe = sample_recipe();
        let context = FangyuanVfxReplayContext::local("caster-a", "event-a");
        let states =
            evaluate_fangyuan_vfx_recipe(&recipe, FangyuanVfxClock::new(10, 20, 30), &context)
                .unwrap();

        let fallback = fangyuan_vfx_standard_mesh_fallback_primitives(&states);

        assert_eq!(fallback.len(), states.len());
        assert_eq!(fallback[0].primitive_kind, states[0].primitive_kind);
        assert_eq!(fallback[0].transform.translation, states[0].local_position);
        assert_eq!(fallback[0].transform.scale, states[0].scale);
        assert_eq!(
            fallback[0].material_profile_id.as_deref(),
            states[0].material_profile_id.as_deref()
        );
    }

    #[test]
    fn fangyuan_vfx_runtime_multi_vfx_tick_cleanup_clear_reload_and_fallback() {
        let mut runtime = FangyuanVfxRuntime::default();
        let context = FangyuanVfxReplayContext::local("caster-a", "event-a");
        runtime
            .start_instance(FangyuanVfxInstance::new(
                "projectile-a",
                fangyuan_vfx_projectile_recipe(),
                context.clone(),
                10,
            ))
            .unwrap();
        runtime
            .start_instance(FangyuanVfxInstance::new(
                "shield-a",
                fangyuan_vfx_shield_recipe(),
                context,
                12,
            ))
            .unwrap();

        let states = runtime.tick(16, 20).unwrap().to_vec();
        assert_eq!(runtime.stats().active_instance_count, 2);
        assert!(states.len() >= 2);
        assert_eq!(runtime.stats().active_state_count, states.len());
        assert_eq!(runtime.fallback_primitives().len(), states.len());
        assert!(states.iter().any(|state| {
            state
                .source
                .instance_id
                .as_ref()
                .is_some_and(|id| id.as_str() == "projectile-a")
        }));
        assert!(states.iter().any(|state| {
            state
                .source
                .instance_id
                .as_ref()
                .is_some_and(|id| id.as_str() == "shield-a")
        }));

        let states = runtime.tick(40, 20).unwrap().to_vec();
        assert!(states.is_empty());
        assert_eq!(runtime.stats().active_instance_count, 0);
        assert_eq!(runtime.stats().active_state_count, 0);
        assert_eq!(runtime.stats().fallback_primitive_count, 0);
        assert_eq!(runtime.stats().total_finished, 2);

        runtime
            .start_instance(FangyuanVfxInstance::new(
                "fade-a",
                fangyuan_vfx_fade_recipe(),
                FangyuanVfxReplayContext::local("caster-a", "event-b"),
                100,
            ))
            .unwrap();
        runtime.tick(105, 20).unwrap();
        runtime.clear_scene();
        assert_eq!(runtime.stats().active_instance_count, 0);
        assert_eq!(runtime.stats().active_state_count, 0);
        assert_eq!(runtime.stats().fallback_primitive_count, 0);
        assert_eq!(runtime.stats().total_cleared, 1);

        runtime
            .start_instance(FangyuanVfxInstance::new(
                "fade-b",
                fangyuan_vfx_fade_recipe(),
                FangyuanVfxReplayContext::local("caster-a", "event-c"),
                200,
            ))
            .unwrap();
        runtime.tick(205, 20).unwrap();
        runtime.reload_scene();
        assert_eq!(runtime.stats().active_instance_count, 0);
        assert!(runtime.active_states().is_empty());
        assert!(runtime.fallback_primitives().is_empty());
    }

    #[test]
    fn fangyuan_vfx_runtime_future_start_keeps_instance_without_states_until_start() {
        let mut runtime = FangyuanVfxRuntime::default();
        runtime
            .start_instance(FangyuanVfxInstance::new(
                "future-projectile",
                fangyuan_vfx_projectile_recipe(),
                FangyuanVfxReplayContext::local("caster-a", "event-future"),
                100,
            ))
            .unwrap();

        let states_before_start = runtime.tick(90, 20).unwrap().to_vec();
        assert!(states_before_start.is_empty());
        assert_eq!(runtime.stats().active_instance_count, 1);
        assert_eq!(runtime.stats().active_state_count, 0);
        assert_eq!(runtime.stats().fallback_primitive_count, 0);
        assert!(runtime.fallback_primitives().is_empty());

        let states_at_start = runtime.tick(100, 20).unwrap().to_vec();
        assert!(!states_at_start.is_empty());
        assert_eq!(runtime.stats().active_instance_count, 1);
        assert_eq!(runtime.stats().active_state_count, states_at_start.len());
        assert_eq!(
            runtime.stats().fallback_primitive_count,
            states_at_start.len()
        );
        assert_eq!(runtime.fallback_primitives().len(), states_at_start.len());
    }

    #[test]
    fn fangyuan_vfx_runtime_rejects_duplicate_instance() {
        let mut runtime = FangyuanVfxRuntime::default();
        let context = FangyuanVfxReplayContext::local("caster-a", "event-a");
        runtime
            .start_instance(FangyuanVfxInstance::new(
                "same",
                fangyuan_vfx_fade_recipe(),
                context.clone(),
                0,
            ))
            .unwrap();

        let error = runtime
            .start_instance(FangyuanVfxInstance::new(
                "same",
                fangyuan_vfx_fade_recipe(),
                context,
                1,
            ))
            .unwrap_err();

        assert_eq!(
            error,
            FangyuanVfxInstanceStartError::DuplicateInstance(FangyuanVfxInstanceId::new("same"))
        );
    }

    #[test]
    fn fangyuan_vfx_recipe_rejects_illegal_operator_payload() {
        let recipe = r#"{
            "id": "bad.operator",
            "version": 1,
            "duration_ticks": 10,
            "emitters": [{
                "id": "bad",
                "operators": [{ "type": "teleport", "to": [1.0, 0.0, 0.0] }]
            }]
        }"#;

        let error = serde_json::from_str::<FangyuanVfxRecipe>(recipe).unwrap_err();

        assert!(error.to_string().contains("teleport"));
    }

    #[test]
    fn fangyuan_vfx_recipe_rejects_invalid_recipe_diagnostics() {
        let mut recipe = sample_recipe();
        recipe.emitters[0].operators.clear();

        let diagnostic = recipe.validate().unwrap_err();

        assert_eq!(diagnostic.code, FangyuanVfxDiagnosticCode::MissingOperator);
        assert_eq!(diagnostic.emitter_index, Some(0));
    }

    #[test]
    fn fangyuan_vfx_recipe_clock_uses_discrete_ticks() {
        let clock = FangyuanVfxClock::new(10, 25, 30);

        assert_eq!(clock.elapsed_ticks(), 15);
        assert_eq!(clock.elapsed_seconds(), 0.5);
        assert!(!clock.is_past_duration(15));
        assert!(clock.is_past_duration(14));
    }

    #[test]
    fn fangyuan_vfx_determinism_same_seed_replays_identically() {
        let recipe = sample_recipe();
        let clock = FangyuanVfxClock::new(1000, 1012, 60);
        let context = FangyuanVfxReplayContext::local("caster-a", "event-a");

        let first = evaluate_fangyuan_vfx_recipe(&recipe, clock, &context).unwrap();
        let second = evaluate_fangyuan_vfx_recipe(&recipe, clock, &context).unwrap();

        assert_eq!(first, second);
        assert_eq!(
            compose_fangyuan_vfx_seed(&recipe, &context, 1000, 0),
            first[0].seed
        );
    }

    #[test]
    fn fangyuan_vfx_determinism_different_seed_changes_jittered_state() {
        let recipe = sample_recipe();
        let clock = FangyuanVfxClock::new(1000, 1012, 60);
        let first_context = FangyuanVfxReplayContext::local("caster-a", "event-a");
        let second_context = FangyuanVfxReplayContext::local("caster-b", "event-a");

        let first = evaluate_fangyuan_vfx_recipe(&recipe, clock, &first_context).unwrap();
        let second = evaluate_fangyuan_vfx_recipe(&recipe, clock, &second_context).unwrap();

        assert_ne!(first[0].seed, second[0].seed);
        assert_ne!(first[0].local_position, second[0].local_position);
    }

    #[test]
    fn fangyuan_vfx_determinism_skip_frame_matches_direct_evaluation() {
        let recipe = sample_recipe();
        let context = FangyuanVfxReplayContext::local("caster-a", "event-a");
        let skipped =
            evaluate_fangyuan_vfx_recipe(&recipe, FangyuanVfxClock::new(42, 61, 30), &context)
                .unwrap();

        let direct =
            evaluate_fangyuan_vfx_recipe(&recipe, FangyuanVfxClock::new(42, 61, 30), &context)
                .unwrap();

        assert_eq!(skipped, direct);
    }

    #[test]
    fn fangyuan_vfx_determinism_repeat_replay_after_duration_is_empty() {
        let recipe = sample_recipe();
        let context = FangyuanVfxReplayContext::local("caster-a", "event-a");
        let first =
            evaluate_fangyuan_vfx_recipe(&recipe, FangyuanVfxClock::new(10, 41, 30), &context)
                .unwrap();
        let second =
            evaluate_fangyuan_vfx_recipe(&recipe, FangyuanVfxClock::new(10, 41, 30), &context)
                .unwrap();

        assert!(first.is_empty());
        assert_eq!(first, second);
    }

    #[test]
    fn fangyuan_vfx_audit_recipe_reports_budget_warnings_and_suggestions() {
        let mut recipe = sample_recipe();
        recipe.duration_ticks = 240;
        recipe.emitters[0].emissive = 6.0;
        recipe.emitters[0]
            .operators
            .push(FangyuanVfxOperator::Trail {
                segments: 20,
                spacing_ticks: 1,
                fade: 0.7,
            });
        recipe.emitters.push(FangyuanVfxEmitter {
            id: "residue".to_string(),
            primitive_kind: FangyuanPrimitiveKind::Cube,
            role: FangyuanPrimitiveRole::Decoration,
            delay_ticks: 0,
            duration_ticks: Some(240),
            position: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
            color: [1.0, 1.0, 1.0, 0.35],
            emissive: 0.0,
            material_profile_id: Some("vfx/residue".to_string()),
            jitter: FangyuanVfxEmitterJitter::default(),
            operators: vec![FangyuanVfxOperator::Fade {
                from: 1.0,
                to: 0.0,
                curve: FangyuanVfxCurve::Linear,
            }],
        });
        let profile = FangyuanVfxBudgetProfile {
            recommended_duration_ticks: 60,
            recommended_peak_primitives: 4,
            recommended_trail_segments: 4,
            recommended_alpha_primitives: 2,
            recommended_emissive_primitives: 0,
            recommended_material_profiles: 1,
            max_emissive_intensity: 4.0,
        };

        let estimate = estimate_fangyuan_vfx_recipe_budget(&recipe);
        let report = audit_fangyuan_vfx_recipe_with_profile(&recipe, &profile);

        assert!(estimate.peak_primitives > profile.recommended_peak_primitives);
        assert!(estimate.trail_segments > profile.recommended_trail_segments);
        assert!(has_vfx_finding(&report, "vfx_duration_above_recommended"));
        assert!(has_vfx_finding(
            &report,
            "vfx_peak_primitives_above_recommended"
        ));
        assert!(has_vfx_finding(
            &report,
            "vfx_trail_segments_above_recommended"
        ));
        assert!(has_vfx_finding(&report, "vfx_alpha_above_recommended"));
        assert!(has_vfx_finding(
            &report,
            "vfx_emissive_count_above_recommended"
        ));
        assert!(has_vfx_finding(
            &report,
            "vfx_material_profile_count_above_recommended"
        ));
        assert!(has_vfx_finding(
            &report,
            "vfx_emissive_intensity_above_recommended"
        ));
        assert!(has_vfx_suggestion_effect(&report, "reduce trail segments"));
        assert!(has_vfx_suggestion_effect(&report, "lower alpha"));
        assert!(has_vfx_suggestion_effect(&report, "shorten duration"));
        assert!(has_vfx_suggestion_effect(&report, "lower emissive"));
    }

    #[test]
    fn fangyuan_vfx_audit_recipe_warns_when_role_has_no_readable_anchor() {
        let recipe = fangyuan_vfx_fade_recipe();

        let report = audit_fangyuan_vfx_recipe(&recipe);

        assert!(has_vfx_finding(&report, "vfx_role_missing_readable_anchor"));
    }

    #[test]
    fn fangyuan_vfx_replay_runtime_degrade_preserves_readable_states() {
        let mut recipe = sample_recipe();
        recipe.emitters.push(FangyuanVfxEmitter {
            id: "sparkle".to_string(),
            primitive_kind: FangyuanPrimitiveKind::Cube,
            role: FangyuanPrimitiveRole::Decoration,
            delay_ticks: 0,
            duration_ticks: Some(30),
            position: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
            color: [1.0, 1.0, 1.0, 0.5],
            emissive: 0.0,
            material_profile_id: Some("vfx/sparkle".to_string()),
            jitter: FangyuanVfxEmitterJitter::default(),
            operators: vec![FangyuanVfxOperator::Spawn {
                curve: FangyuanVfxCurve::Linear,
            }],
        });
        let clock = FangyuanVfxClock::new(0, 18, 30);
        let context = FangyuanVfxReplayContext::local("caster-a", "event-a");

        let full = evaluate_fangyuan_vfx_recipe(&recipe, clock, &context).unwrap();
        let degraded = evaluate_fangyuan_vfx_recipe_with_budget_pressure(
            &recipe,
            clock,
            &context,
            FangyuanVfxBudgetPressure::constrained(2, 1),
        )
        .unwrap();

        assert!(full.len() > degraded.len());
        assert!(degraded.iter().any(|state| {
            matches!(
                state.role,
                FangyuanPrimitiveRole::Core | FangyuanPrimitiveRole::Impact
            )
        }));
        assert!(
            !degraded
                .iter()
                .any(|state| state.role == FangyuanPrimitiveRole::Decoration)
        );
        assert!(
            degraded
                .iter()
                .filter(|state| state.role == FangyuanPrimitiveRole::Trail)
                .count()
                <= 1
        );
    }

    #[test]
    fn fangyuan_vfx_replay_tick_jump_matches_direct_hash() {
        let recipe = sample_recipe();
        let context = FangyuanVfxReplayContext::local("caster-a", "event-a");
        let mut runtime = FangyuanVfxRuntime::default();
        runtime
            .start_instance(FangyuanVfxInstance::new(
                "event-a",
                recipe.clone(),
                context.clone(),
                40,
            ))
            .unwrap();

        let jumped = runtime.tick(65, 30).unwrap().to_vec();
        let direct =
            evaluate_fangyuan_vfx_recipe(&recipe, FangyuanVfxClock::new(40, 65, 30), &context)
                .unwrap();

        assert_eq!(
            fangyuan_vfx_primitive_state_hash(&jumped),
            fangyuan_vfx_primitive_state_hash(&direct)
        );
    }

    #[test]
    fn fangyuan_vfx_replay_pause_resume_keeps_hash_stable() {
        let recipe = sample_recipe();
        let context = FangyuanVfxReplayContext::local("caster-a", "event-a");
        let mut runtime = FangyuanVfxRuntime::default();
        runtime
            .start_instance(FangyuanVfxInstance::new("event-a", recipe, context, 10))
            .unwrap();

        let paused = runtime.tick(18, 30).unwrap().to_vec();
        let paused_hash = fangyuan_vfx_primitive_state_hash(&paused);
        let same_tick_after_pause = runtime.tick(18, 30).unwrap().to_vec();
        let resumed = runtime.tick(22, 30).unwrap().to_vec();

        assert_eq!(
            paused_hash,
            fangyuan_vfx_primitive_state_hash(&same_tick_after_pause)
        );
        assert_ne!(paused_hash, fangyuan_vfx_primitive_state_hash(&resumed));
    }

    #[test]
    fn fangyuan_vfx_replay_delayed_event_fields_drive_clock_and_context() {
        let recipe = sample_recipe();
        let event = FangyuanVfxReplayEvent {
            authority_epoch: 7,
            start_tick: 120,
            frame_id: 120,
            fps: 30,
            action: "cast_vfx".to_string(),
            caster_id: "caster-a".to_string(),
            player_id: "player-a".to_string(),
            event_id: "event-delay".to_string(),
            recipe_id: recipe.id.clone(),
            external_seed: Some(42),
            prediction_boundary: FangyuanVfxPredictionBoundary::AuthorityConfirmed,
        };

        let before =
            evaluate_fangyuan_vfx_recipe(&recipe, event.clock_at(119), &event.replay_context())
                .unwrap();
        let active =
            evaluate_fangyuan_vfx_recipe(&recipe, event.clock_at(130), &event.replay_context())
                .unwrap();

        assert!(before.is_empty());
        assert!(!active.is_empty());
        assert_eq!(event.authority_epoch, 7);
        assert_eq!(event.frame_id, 120);
        assert_eq!(
            event.prediction_boundary,
            FangyuanVfxPredictionBoundary::AuthorityConfirmed
        );
    }

    #[test]
    fn fangyuan_vfx_replay_hash_is_stable_and_seed_conflicts_are_visible() {
        let mut recipe = sample_recipe();
        recipe.seed_policy = FangyuanVfxSeedPolicy::External;
        let event_a = FangyuanVfxReplayEvent {
            authority_epoch: 1,
            start_tick: 5,
            frame_id: 5,
            fps: 30,
            action: "cast_vfx".to_string(),
            caster_id: "caster-a".to_string(),
            player_id: "player-a".to_string(),
            event_id: "event-a".to_string(),
            recipe_id: recipe.id.clone(),
            external_seed: Some(99),
            prediction_boundary: FangyuanVfxPredictionBoundary::LocalPredicted,
        };
        let mut event_b = event_a.clone();
        event_b.event_id = "event-b".to_string();
        event_b.external_seed = Some(100);
        let mut event_conflict = event_a.clone();
        event_conflict.event_id = "event-conflict".to_string();

        let first =
            evaluate_fangyuan_vfx_recipe(&recipe, event_a.clock_at(16), &event_a.replay_context())
                .unwrap();
        let second =
            evaluate_fangyuan_vfx_recipe(&recipe, event_a.clock_at(16), &event_a.replay_context())
                .unwrap();
        let different_seed =
            evaluate_fangyuan_vfx_recipe(&recipe, event_b.clock_at(16), &event_b.replay_context())
                .unwrap();
        let seed_conflict = evaluate_fangyuan_vfx_recipe(
            &recipe,
            event_conflict.clock_at(16),
            &event_conflict.replay_context(),
        )
        .unwrap();

        assert_eq!(
            fangyuan_vfx_primitive_state_hash(&first),
            fangyuan_vfx_primitive_state_hash(&second)
        );
        assert_ne!(
            fangyuan_vfx_primitive_state_hash(&first),
            fangyuan_vfx_primitive_state_hash(&different_seed)
        );
        assert_eq!(first[0].seed, seed_conflict[0].seed);
        assert_ne!(event_a.event_id, event_conflict.event_id);
    }

    fn has_vfx_finding(report: &FangyuanAuditReport, code: &str) -> bool {
        report.findings.iter().any(|finding| finding.code == code)
    }

    fn has_vfx_suggestion_effect(report: &FangyuanAuditReport, needle: &str) -> bool {
        report.suggestions.iter().any(|suggestion| {
            suggestion
                .estimated_effect
                .as_deref()
                .is_some_and(|effect| effect.contains(needle))
        })
    }
}
