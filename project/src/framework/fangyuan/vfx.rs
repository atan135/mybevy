use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::{FangyuanPrimitiveKind, FangyuanPrimitiveLifecycle, FangyuanPrimitiveRole};

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

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

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanVfxDynamicPrimitiveState {
    pub recipe_id: String,
    pub emitter_id: String,
    pub emitter_index: usize,
    pub primitive_index: u16,
    pub source_tick: u64,
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

pub fn evaluate_fangyuan_vfx_recipe(
    recipe: &FangyuanVfxRecipe,
    clock: FangyuanVfxClock,
    context: &FangyuanVfxReplayContext,
) -> Result<Vec<FangyuanVfxDynamicPrimitiveState>, FangyuanVfxDiagnostic> {
    recipe.validate()?;

    if clock.ticks_per_second == 0 {
        return Err(FangyuanVfxDiagnostic::new(
            FangyuanVfxDiagnosticCode::InvalidClock,
            "ticks_per_second must be greater than zero",
        ));
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
        states.push(base.clone());

        for operator in &emitter.operators {
            if let FangyuanVfxOperator::Trail {
                segments,
                spacing_ticks,
                fade,
            } = operator
            {
                let segment_count = (*segments).min(recipe.budget_hints.max_trail_segments);
                for segment in 1..=segment_count {
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
                }
            }
        }
    }

    states.truncate(recipe.budget_hints.max_primitives as usize);
    Ok(states)
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
        assert_eq!(state.lifecycle.spawn_tick, Some(100));
        assert_eq!(state.lifecycle.despawn_tick, Some(130));
        assert!(state.local_position.x > 1.4 && state.local_position.x < 1.7);
        assert!(state.scale.x > 1.6);
        assert!(state.alpha > 0.0 && state.alpha < 1.0);
        assert!(state.emissive >= 0.25);
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
}
