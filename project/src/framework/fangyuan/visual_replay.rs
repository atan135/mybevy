use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::{
    FangyuanBlueprintFallbackDomain, FangyuanBlueprintMissingFallbackMode,
    FangyuanCacheAuthoritySource, FangyuanLodLevel, FangyuanLodObjectKind,
    FangyuanSkillDegradeLevel, FangyuanSkillRuntimeContext, FangyuanSkillRuntimePresentation,
    FangyuanSkillTemplate, FangyuanSkillTemplateRegistry, FangyuanSkillVisualBlueprint,
    FangyuanVfxDiagnostic, FangyuanVfxDynamicPrimitiveState, FangyuanVfxReplayEvent,
    compile_fangyuan_skill_runtime_presentation, fangyuan_vfx_primitive_state_hash,
};

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanVisualReplay {
    pub replay_id: String,
    pub start_tick: u64,
    #[serde(default)]
    pub events: Vec<FangyuanVisualReplayEvent>,
}

impl FangyuanVisualReplay {
    pub fn new(
        replay_id: impl Into<String>,
        start_tick: u64,
        events: Vec<FangyuanVisualReplayEvent>,
    ) -> Self {
        let mut replay = Self {
            replay_id: replay_id.into(),
            start_tick,
            events,
        };
        replay.sort_events();
        replay
    }

    fn sort_events(&mut self) {
        self.events.sort_by(|left, right| {
            left.replay_event
                .start_tick
                .cmp(&right.replay_event.start_tick)
                .then_with(|| left.replay_event.frame_id.cmp(&right.replay_event.frame_id))
                .then_with(|| left.replay_event.event_id.cmp(&right.replay_event.event_id))
                .then_with(|| left.object_id.cmp(&right.object_id))
        });
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanVisualReplayEvent {
    pub replay_event: FangyuanVfxReplayEvent,
    pub skill_visual_id: String,
    pub object_id: String,
    #[serde(default)]
    pub lod: FangyuanVisualReplayLod,
    #[serde(default)]
    pub degrade_level: FangyuanSkillDegradeLevel,
    #[serde(default)]
    pub cache_path: FangyuanVisualReplayCachePath,
    #[serde(default)]
    pub fallback: Option<FangyuanVisualReplayFallback>,
}

impl FangyuanVisualReplayEvent {
    pub fn new(
        replay_event: FangyuanVfxReplayEvent,
        skill_visual_id: impl Into<String>,
        object_id: impl Into<String>,
    ) -> Self {
        Self {
            replay_event,
            skill_visual_id: skill_visual_id.into(),
            object_id: object_id.into(),
            lod: FangyuanVisualReplayLod::default(),
            degrade_level: FangyuanSkillDegradeLevel::None,
            cache_path: FangyuanVisualReplayCachePath::default(),
            fallback: None,
        }
    }

    pub fn with_lod(mut self, lod: FangyuanVisualReplayLod) -> Self {
        self.lod = lod;
        self
    }

    pub fn with_degrade_level(mut self, degrade_level: FangyuanSkillDegradeLevel) -> Self {
        self.degrade_level = degrade_level;
        self
    }

    pub fn with_cache_path(mut self, cache_path: FangyuanVisualReplayCachePath) -> Self {
        self.cache_path = cache_path;
        self
    }

    pub fn with_fallback(mut self, fallback: FangyuanVisualReplayFallback) -> Self {
        self.fallback = Some(fallback);
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FangyuanVisualReplayLod {
    pub object_kind: FangyuanLodObjectKind,
    pub level: FangyuanLodLevel,
    pub rule_layer_retained: bool,
}

impl Default for FangyuanVisualReplayLod {
    fn default() -> Self {
        Self {
            object_kind: FangyuanLodObjectKind::SkillVfx,
            level: FangyuanLodLevel::L0Full,
            rule_layer_retained: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanVisualReplayCachePath {
    #[default]
    FirstPackage,
    CacheHit,
    CacheMissFallback,
    ServerManifest,
    CacheBytesOnly,
}

impl FangyuanVisualReplayCachePath {
    pub const fn from_cache_authority_source(source: FangyuanCacheAuthoritySource) -> Self {
        match source {
            FangyuanCacheAuthoritySource::ServerManifest => Self::ServerManifest,
            FangyuanCacheAuthoritySource::ClientCacheBytesOnly => Self::CacheBytesOnly,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanVisualReplayFallback {
    pub domain: FangyuanBlueprintFallbackDomain,
    pub mode: FangyuanBlueprintMissingFallbackMode,
    pub recovery_key: String,
    pub rule_only: bool,
}

#[derive(Clone, Debug, Default)]
pub struct FangyuanVisualReplayCatalog {
    templates: FangyuanSkillTemplateRegistry,
    visuals: BTreeMap<String, FangyuanSkillVisualBlueprint>,
}

impl FangyuanVisualReplayCatalog {
    pub fn new(
        templates: FangyuanSkillTemplateRegistry,
        visuals: impl IntoIterator<Item = FangyuanSkillVisualBlueprint>,
    ) -> Self {
        Self {
            templates,
            visuals: visuals
                .into_iter()
                .map(|visual| (visual.id.clone(), visual))
                .collect(),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(
            FangyuanSkillTemplateRegistry::with_defaults(),
            super::fangyuan_default_skill_visual_blueprints(),
        )
    }

    pub fn visual(&self, id: &str) -> Option<&FangyuanSkillVisualBlueprint> {
        self.visuals.get(id)
    }

    pub fn template_for_visual(
        &self,
        visual: &FangyuanSkillVisualBlueprint,
    ) -> FangyuanVisualReplayTemplateResolution<'_> {
        let resolution = self
            .templates
            .resolve_or_fallback(Some(&visual.template_id), Some(visual.template_version));
        FangyuanVisualReplayTemplateResolution {
            template: resolution.template,
            fallback_used: resolution.fallback_reason.is_some(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FangyuanVisualReplayTemplateResolution<'a> {
    pub template: &'a FangyuanSkillTemplate,
    pub fallback_used: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanVisualReplayRunOptions {
    pub current_tick: u64,
    pub ticks_per_second: u32,
}

impl FangyuanVisualReplayRunOptions {
    pub const fn new(current_tick: u64, ticks_per_second: u32) -> Self {
        Self {
            current_tick,
            ticks_per_second,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanVisualReplayConsistencyReport {
    pub replay_id: String,
    pub start_tick: u64,
    pub event_count: usize,
    pub visual_hash: u64,
    pub mismatch_summary: Option<FangyuanVisualReplayMismatchSummary>,
    pub samples: Vec<FangyuanVisualReplaySample>,
}

impl FangyuanVisualReplayConsistencyReport {
    pub fn stable(replay: &FangyuanVisualReplay, samples: Vec<FangyuanVisualReplaySample>) -> Self {
        Self {
            replay_id: replay.replay_id.clone(),
            start_tick: replay.start_tick,
            event_count: replay.events.len(),
            visual_hash: fangyuan_visual_replay_hash(&samples),
            mismatch_summary: None,
            samples,
        }
    }

    pub fn summary_line(&self) -> String {
        format!(
            "replay={} start_tick={} events={} visual_hash={:016x} mismatch={}",
            self.replay_id,
            self.start_tick,
            self.event_count,
            self.visual_hash,
            self.mismatch_summary
                .as_ref()
                .map(FangyuanVisualReplayMismatchSummary::summary_line)
                .unwrap_or_else(|| "none".to_string())
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanVisualReplaySample {
    pub tick: u64,
    pub frame_id: u32,
    pub event_id: String,
    pub recipe_id: String,
    pub object_id: String,
    pub skill_visual_id: String,
    pub rule_layer_hash: u64,
    pub personality_layer_hash: u64,
    pub state_hash: u64,
    pub material_hash: u64,
    pub lod: FangyuanVisualReplayLod,
    pub degrade_level: FangyuanSkillDegradeLevel,
    pub cache_path: FangyuanVisualReplayCachePath,
    pub fallback: Option<FangyuanVisualReplayFallback>,
    pub rule_state_count: usize,
    pub personality_state_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanVisualReplayMismatchSummary {
    pub replay_id: String,
    pub tick: u64,
    pub frame_id: u32,
    pub event_id: String,
    pub recipe_id: String,
    pub object_id: String,
    pub expected_hash: u64,
    pub actual_hash: u64,
    pub field: FangyuanVisualReplayMismatchField,
}

impl FangyuanVisualReplayMismatchSummary {
    pub fn summary_line(&self) -> String {
        format!(
            "tick={} frame={} event={} recipe={} object={} field={:?} expected={:016x} actual={:016x}",
            self.tick,
            self.frame_id,
            self.event_id,
            self.recipe_id,
            self.object_id,
            self.field,
            self.expected_hash,
            self.actual_hash
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanVisualReplayMismatchField {
    MissingSample,
    ExtraSample,
    RuleLayer,
    PersonalityLayer,
    State,
    MaterialParams,
    Lod,
    Degrade,
    CachePath,
    Fallback,
    VisualHash,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FangyuanVisualReplayError {
    MissingVisual { skill_visual_id: String },
    CompileFailed { event_id: String, message: String },
}

pub fn summarize_fangyuan_visual_replay(
    replay: &FangyuanVisualReplay,
    catalog: &FangyuanVisualReplayCatalog,
    options: &FangyuanVisualReplayRunOptions,
) -> Result<FangyuanVisualReplayConsistencyReport, FangyuanVisualReplayError> {
    let mut events = replay.events.clone();
    events.sort_by(|left, right| {
        left.replay_event
            .start_tick
            .cmp(&right.replay_event.start_tick)
            .then_with(|| left.replay_event.frame_id.cmp(&right.replay_event.frame_id))
            .then_with(|| left.replay_event.event_id.cmp(&right.replay_event.event_id))
            .then_with(|| left.object_id.cmp(&right.object_id))
    });

    let samples = events
        .iter()
        .map(|event| summarize_visual_event(event, catalog, options))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(FangyuanVisualReplayConsistencyReport::stable(
        replay, samples,
    ))
}

pub fn compare_fangyuan_visual_replay_reports(
    expected: &FangyuanVisualReplayConsistencyReport,
    actual: &FangyuanVisualReplayConsistencyReport,
) -> Option<FangyuanVisualReplayMismatchSummary> {
    if expected.visual_hash == actual.visual_hash {
        return None;
    }

    let sample_len = expected.samples.len().max(actual.samples.len());
    for index in 0..sample_len {
        match (expected.samples.get(index), actual.samples.get(index)) {
            (Some(left), Some(right)) => {
                if left == right {
                    continue;
                }
                return Some(mismatch_for_sample(expected, left, right));
            }
            (Some(left), None) => {
                return Some(sample_presence_mismatch(
                    expected,
                    left,
                    FangyuanVisualReplayMismatchField::MissingSample,
                    left.state_hash,
                    0,
                ));
            }
            (None, Some(right)) => {
                return Some(sample_presence_mismatch(
                    expected,
                    right,
                    FangyuanVisualReplayMismatchField::ExtraSample,
                    0,
                    right.state_hash,
                ));
            }
            (None, None) => {}
        }
    }

    Some(FangyuanVisualReplayMismatchSummary {
        replay_id: expected.replay_id.clone(),
        tick: expected.start_tick,
        frame_id: 0,
        event_id: String::new(),
        recipe_id: String::new(),
        object_id: String::new(),
        expected_hash: expected.visual_hash,
        actual_hash: actual.visual_hash,
        field: FangyuanVisualReplayMismatchField::VisualHash,
    })
}

pub fn fangyuan_visual_replay_consistency_report(
    expected: &FangyuanVisualReplayConsistencyReport,
    actual: &FangyuanVisualReplayConsistencyReport,
) -> FangyuanVisualReplayConsistencyReport {
    let mismatch_summary = compare_fangyuan_visual_replay_reports(expected, actual);
    FangyuanVisualReplayConsistencyReport {
        replay_id: actual.replay_id.clone(),
        start_tick: actual.start_tick,
        event_count: actual.event_count,
        visual_hash: actual.visual_hash,
        mismatch_summary,
        samples: actual.samples.clone(),
    }
}

fn summarize_visual_event(
    event: &FangyuanVisualReplayEvent,
    catalog: &FangyuanVisualReplayCatalog,
    options: &FangyuanVisualReplayRunOptions,
) -> Result<FangyuanVisualReplaySample, FangyuanVisualReplayError> {
    let visual = catalog.visual(&event.skill_visual_id).ok_or_else(|| {
        FangyuanVisualReplayError::MissingVisual {
            skill_visual_id: event.skill_visual_id.clone(),
        }
    })?;
    let template = catalog.template_for_visual(visual);
    let mut context = FangyuanSkillRuntimeContext::local(
        event.replay_event.start_tick,
        options.current_tick,
        options.ticks_per_second,
        &event.replay_event.caster_id,
        &event.replay_event.event_id,
    )
    .with_degrade_level(event.degrade_level);
    context.external_seed = event.replay_event.external_seed;

    let presentation =
        compile_fangyuan_skill_runtime_presentation(template.template, visual, &context).map_err(
            |error| FangyuanVisualReplayError::CompileFailed {
                event_id: event.replay_event.event_id.clone(),
                message: diagnostic_message(error),
            },
        )?;

    Ok(sample_from_presentation(
        event,
        &presentation,
        visual,
        template.fallback_used,
    ))
}

fn sample_from_presentation(
    event: &FangyuanVisualReplayEvent,
    presentation: &FangyuanSkillRuntimePresentation,
    visual: &FangyuanSkillVisualBlueprint,
    template_fallback_used: bool,
) -> FangyuanVisualReplaySample {
    let playback_states = presentation.playback_states();
    let rule_layer_hash = fangyuan_vfx_primitive_state_hash(&presentation.rule_layer_states);
    let personality_layer_hash =
        fangyuan_vfx_primitive_state_hash(&presentation.personality_layer_states);
    let state_hash = fangyuan_vfx_primitive_state_hash(&playback_states);
    let material_hash = fangyuan_visual_material_hash(visual, &playback_states);
    let fallback = event.fallback.clone().or_else(|| {
        template_fallback_used.then(|| FangyuanVisualReplayFallback {
            domain: FangyuanBlueprintFallbackDomain::Skill,
            mode: FangyuanBlueprintMissingFallbackMode::RuleOnly,
            recovery_key: visual.template_id.clone(),
            rule_only: true,
        })
    });

    FangyuanVisualReplaySample {
        tick: event.replay_event.start_tick,
        frame_id: event.replay_event.frame_id,
        event_id: event.replay_event.event_id.clone(),
        recipe_id: event.replay_event.recipe_id.clone(),
        object_id: event.object_id.clone(),
        skill_visual_id: event.skill_visual_id.clone(),
        rule_layer_hash,
        personality_layer_hash,
        state_hash,
        material_hash,
        lod: event.lod,
        degrade_level: event.degrade_level,
        cache_path: event.cache_path,
        fallback,
        rule_state_count: presentation.rule_layer_states.len(),
        personality_state_count: presentation.personality_layer_states.len(),
    }
}

pub fn fangyuan_visual_material_hash(
    visual: &FangyuanSkillVisualBlueprint,
    states: &[FangyuanVfxDynamicPrimitiveState],
) -> u64 {
    let mut hash = FNV_OFFSET;
    mix_str(&mut hash, &visual.id);
    mix_str(&mut hash, &visual.template_id);
    mix_u64(&mut hash, u64::from(visual.template_version));
    for value in visual.color {
        mix_f32(&mut hash, value);
    }
    mix_f32(&mut hash, visual.readability.rule_alpha);
    mix_f32(&mut hash, visual.readability.rule_edge_width);
    mix_f32(&mut hash, visual.readability.personality_occlusion);
    mix_f32(&mut hash, visual.readability.decor_bounds_radius);
    mix_u64(
        &mut hash,
        u64::from(visual.readability.transparent_primitive_budget),
    );
    mix_option_str(&mut hash, visual.profile_ref.as_deref());
    mix_bool(&mut hash, visual.trail.enabled);
    mix_u64(&mut hash, visual.trail.segment_count.into());
    mix_bool(&mut hash, visual.decor.enabled);
    mix_u64(&mut hash, visual.decor.max_pieces.into());
    mix_bool(&mut hash, visual.impact_residue.enabled);
    mix_u64(&mut hash, visual.impact_residue.duration_ticks);
    mix_f32(&mut hash, visual.emissive.intensity);

    for state in states {
        mix_str(&mut hash, &state.recipe_id);
        mix_str(&mut hash, &state.emitter_id);
        mix_option_str(&mut hash, state.material_profile_id.as_deref());
        let color = state.color.to_srgba();
        mix_f32(&mut hash, color.red);
        mix_f32(&mut hash, color.green);
        mix_f32(&mut hash, color.blue);
        mix_f32(&mut hash, state.alpha);
        mix_f32(&mut hash, state.emissive);
    }

    avalanche(hash)
}

fn fangyuan_visual_replay_hash(samples: &[FangyuanVisualReplaySample]) -> u64 {
    let mut hash = FNV_OFFSET;
    for sample in samples {
        mix_sample(&mut hash, sample);
    }
    avalanche(hash)
}

fn mix_sample(hash: &mut u64, sample: &FangyuanVisualReplaySample) {
    mix_u64(hash, sample.tick);
    mix_u64(hash, u64::from(sample.frame_id));
    mix_str(hash, &sample.event_id);
    mix_str(hash, &sample.recipe_id);
    mix_str(hash, &sample.object_id);
    mix_str(hash, &sample.skill_visual_id);
    mix_u64(hash, sample.rule_layer_hash);
    mix_u64(hash, sample.personality_layer_hash);
    mix_u64(hash, sample.state_hash);
    mix_u64(hash, sample.material_hash);
    mix_str(hash, sample.lod.object_kind.as_str());
    mix_str(hash, sample.lod.level.as_str());
    mix_bool(hash, sample.lod.rule_layer_retained);
    mix_str(hash, degrade_level_str(sample.degrade_level));
    mix_str(hash, cache_path_str(sample.cache_path));
    if let Some(fallback) = &sample.fallback {
        mix_str(hash, fallback.domain.as_str());
        mix_str(hash, fallback.mode.as_str());
        mix_str(hash, &fallback.recovery_key);
        mix_bool(hash, fallback.rule_only);
    } else {
        mix_str(hash, "no_fallback");
    }
    mix_u64(hash, sample.rule_state_count as u64);
    mix_u64(hash, sample.personality_state_count as u64);
}

fn mismatch_for_sample(
    expected: &FangyuanVisualReplayConsistencyReport,
    left: &FangyuanVisualReplaySample,
    right: &FangyuanVisualReplaySample,
) -> FangyuanVisualReplayMismatchSummary {
    let (field, expected_hash, actual_hash) = if left.rule_layer_hash != right.rule_layer_hash {
        (
            FangyuanVisualReplayMismatchField::RuleLayer,
            left.rule_layer_hash,
            right.rule_layer_hash,
        )
    } else if left.personality_layer_hash != right.personality_layer_hash {
        (
            FangyuanVisualReplayMismatchField::PersonalityLayer,
            left.personality_layer_hash,
            right.personality_layer_hash,
        )
    } else if left.state_hash != right.state_hash {
        (
            FangyuanVisualReplayMismatchField::State,
            left.state_hash,
            right.state_hash,
        )
    } else if left.material_hash != right.material_hash {
        (
            FangyuanVisualReplayMismatchField::MaterialParams,
            left.material_hash,
            right.material_hash,
        )
    } else if left.lod != right.lod {
        (
            FangyuanVisualReplayMismatchField::Lod,
            hash_lod(left.lod),
            hash_lod(right.lod),
        )
    } else if left.degrade_level != right.degrade_level {
        (
            FangyuanVisualReplayMismatchField::Degrade,
            hash_text(degrade_level_str(left.degrade_level)),
            hash_text(degrade_level_str(right.degrade_level)),
        )
    } else if left.cache_path != right.cache_path {
        (
            FangyuanVisualReplayMismatchField::CachePath,
            hash_text(cache_path_str(left.cache_path)),
            hash_text(cache_path_str(right.cache_path)),
        )
    } else if left.fallback != right.fallback {
        (
            FangyuanVisualReplayMismatchField::Fallback,
            hash_fallback(left.fallback.as_ref()),
            hash_fallback(right.fallback.as_ref()),
        )
    } else {
        (
            FangyuanVisualReplayMismatchField::VisualHash,
            expected.visual_hash,
            0,
        )
    };

    FangyuanVisualReplayMismatchSummary {
        replay_id: expected.replay_id.clone(),
        tick: left.tick.min(right.tick),
        frame_id: left.frame_id,
        event_id: if left.event_id == right.event_id {
            left.event_id.clone()
        } else {
            format!("{}|{}", left.event_id, right.event_id)
        },
        recipe_id: if left.recipe_id == right.recipe_id {
            left.recipe_id.clone()
        } else {
            format!("{}|{}", left.recipe_id, right.recipe_id)
        },
        object_id: if left.object_id == right.object_id {
            left.object_id.clone()
        } else {
            format!("{}|{}", left.object_id, right.object_id)
        },
        expected_hash,
        actual_hash,
        field,
    }
}

fn sample_presence_mismatch(
    expected: &FangyuanVisualReplayConsistencyReport,
    sample: &FangyuanVisualReplaySample,
    field: FangyuanVisualReplayMismatchField,
    expected_hash: u64,
    actual_hash: u64,
) -> FangyuanVisualReplayMismatchSummary {
    FangyuanVisualReplayMismatchSummary {
        replay_id: expected.replay_id.clone(),
        tick: sample.tick,
        frame_id: sample.frame_id,
        event_id: sample.event_id.clone(),
        recipe_id: sample.recipe_id.clone(),
        object_id: sample.object_id.clone(),
        expected_hash,
        actual_hash,
        field,
    }
}

fn diagnostic_message(error: FangyuanVfxDiagnostic) -> String {
    if let Some(emitter_index) = error.emitter_index {
        format!("{} at emitter {}", error.message, emitter_index)
    } else {
        error.message
    }
}

fn hash_lod(lod: FangyuanVisualReplayLod) -> u64 {
    let mut hash = FNV_OFFSET;
    mix_str(&mut hash, lod.object_kind.as_str());
    mix_str(&mut hash, lod.level.as_str());
    mix_bool(&mut hash, lod.rule_layer_retained);
    avalanche(hash)
}

fn hash_fallback(fallback: Option<&FangyuanVisualReplayFallback>) -> u64 {
    let mut hash = FNV_OFFSET;
    if let Some(fallback) = fallback {
        mix_str(&mut hash, fallback.domain.as_str());
        mix_str(&mut hash, fallback.mode.as_str());
        mix_str(&mut hash, &fallback.recovery_key);
        mix_bool(&mut hash, fallback.rule_only);
    } else {
        mix_str(&mut hash, "no_fallback");
    }
    avalanche(hash)
}

fn hash_text(value: &str) -> u64 {
    let mut hash = FNV_OFFSET;
    mix_str(&mut hash, value);
    avalanche(hash)
}

fn degrade_level_str(level: FangyuanSkillDegradeLevel) -> &'static str {
    match level {
        FangyuanSkillDegradeLevel::None => "none",
        FangyuanSkillDegradeLevel::Low => "low",
        FangyuanSkillDegradeLevel::Medium => "medium",
        FangyuanSkillDegradeLevel::High => "high",
        FangyuanSkillDegradeLevel::Critical => "critical",
    }
}

fn cache_path_str(path: FangyuanVisualReplayCachePath) -> &'static str {
    match path {
        FangyuanVisualReplayCachePath::FirstPackage => "first_package",
        FangyuanVisualReplayCachePath::CacheHit => "cache_hit",
        FangyuanVisualReplayCachePath::CacheMissFallback => "cache_miss_fallback",
        FangyuanVisualReplayCachePath::ServerManifest => "server_manifest",
        FangyuanVisualReplayCachePath::CacheBytesOnly => "cache_bytes_only",
    }
}

fn mix_option_str(hash: &mut u64, value: Option<&str>) {
    if let Some(value) = value {
        mix_str(hash, value);
    } else {
        mix_str(hash, "none");
    }
}

fn mix_str(hash: &mut u64, value: &str) {
    for byte in value.as_bytes() {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(FNV_PRIME);
    }
    *hash ^= 0xff;
    *hash = hash.wrapping_mul(FNV_PRIME);
}

fn mix_bool(hash: &mut u64, value: bool) {
    *hash ^= u64::from(value);
    *hash = hash.wrapping_mul(FNV_PRIME);
}

fn mix_u64(hash: &mut u64, value: u64) {
    for byte in value.to_le_bytes() {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(FNV_PRIME);
    }
}

fn mix_f32(hash: &mut u64, value: f32) {
    mix_u64(hash, u64::from(value.to_bits()));
}

fn avalanche(mut hash: u64) -> u64 {
    hash ^= hash >> 33;
    hash = hash.wrapping_mul(0xff51_afd7_ed55_8ccd);
    hash ^= hash >> 33;
    hash = hash.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    hash ^ (hash >> 33)
}

trait FangyuanVisualReplayFallbackText {
    fn as_str(self) -> &'static str;
}

impl FangyuanVisualReplayFallbackText for FangyuanBlueprintFallbackDomain {
    fn as_str(self) -> &'static str {
        match self {
            FangyuanBlueprintFallbackDomain::Home => "home",
            FangyuanBlueprintFallbackDomain::Equipment => "equipment",
            FangyuanBlueprintFallbackDomain::Skill => "skill",
            FangyuanBlueprintFallbackDomain::Npc => "npc",
            FangyuanBlueprintFallbackDomain::Generic => "generic",
        }
    }
}

impl FangyuanVisualReplayFallbackText for FangyuanBlueprintMissingFallbackMode {
    fn as_str(self) -> &'static str {
        match self {
            FangyuanBlueprintMissingFallbackMode::DefaultAppearance => "default_appearance",
            FangyuanBlueprintMissingFallbackMode::Marker => "marker",
            FangyuanBlueprintMissingFallbackMode::RuleOnly => "rule_only",
            FangyuanBlueprintMissingFallbackMode::Pending => "pending",
            FangyuanBlueprintMissingFallbackMode::Hidden => "hidden",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::fangyuan::{
        FangyuanVfxPredictionBoundary, FangyuanVfxSeedPolicy,
        fangyuan_default_skill_visual_blueprints,
    };

    fn replay_event(
        event_id: &str,
        frame_id: u32,
        start_tick: u64,
        recipe_id: &str,
    ) -> FangyuanVfxReplayEvent {
        FangyuanVfxReplayEvent {
            authority_epoch: 3,
            start_tick,
            frame_id,
            fps: 30,
            action: "cast_vfx".to_string(),
            caster_id: "chr_caster".to_string(),
            player_id: "chr_caster".to_string(),
            event_id: event_id.to_string(),
            recipe_id: recipe_id.to_string(),
            external_seed: Some(77),
            prediction_boundary: FangyuanVfxPredictionBoundary::AuthorityConfirmed,
        }
    }

    fn projectile_replay(
        event_id: &str,
        frame_id: u32,
        start_tick: u64,
    ) -> FangyuanVisualReplayEvent {
        FangyuanVisualReplayEvent::new(
            replay_event(event_id, frame_id, start_tick, "vfx.projectile"),
            "skill.visual.projectile",
            format!("skill_object_{event_id}"),
        )
    }

    fn replay_with_events(events: Vec<FangyuanVisualReplayEvent>) -> FangyuanVisualReplay {
        FangyuanVisualReplay::new("authority_replay_a", 100, events)
    }

    fn report_for(
        replay: &FangyuanVisualReplay,
        current_tick: u64,
    ) -> FangyuanVisualReplayConsistencyReport {
        summarize_fangyuan_visual_replay(
            replay,
            &FangyuanVisualReplayCatalog::with_defaults(),
            &FangyuanVisualReplayRunOptions::new(current_tick, 30),
        )
        .unwrap()
    }

    #[test]
    fn fangyuan_visual_replay_same_authority_replay_outputs_stable_summary_hash() {
        let replay = replay_with_events(vec![
            projectile_replay("evt_delayed", 110, 110),
            projectile_replay("evt_first", 100, 100),
        ]);

        let first = report_for(&replay, 130);
        let second = report_for(&replay, 130);
        let consistency = fangyuan_visual_replay_consistency_report(&first, &second);

        assert_eq!(first.visual_hash, second.visual_hash);
        assert_eq!(consistency.mismatch_summary, None);
        assert_eq!(first.replay_id, "authority_replay_a");
        assert_eq!(first.start_tick, 100);
        assert_eq!(first.event_count, 2);
        assert!(first.summary_line().contains("events=2"));
        assert!(first.summary_line().contains("mismatch=none"));
    }

    #[test]
    fn fangyuan_visual_replay_mismatch_points_to_tick_event_recipe_and_object() {
        let replay = replay_with_events(vec![projectile_replay("evt_seed", 120, 120)]);
        let mut changed = replay.clone();
        changed.events[0].degrade_level = FangyuanSkillDegradeLevel::Critical;

        let first = report_for(&replay, 136);
        let second = report_for(&changed, 136);
        let consistency = fangyuan_visual_replay_consistency_report(&first, &second);
        let mismatch = consistency.mismatch_summary.unwrap();

        assert_eq!(mismatch.tick, 120);
        assert_eq!(mismatch.frame_id, 120);
        assert_eq!(mismatch.event_id, "evt_seed");
        assert_eq!(mismatch.recipe_id, "vfx.projectile");
        assert_eq!(mismatch.object_id, "skill_object_evt_seed");
        assert!(matches!(
            mismatch.field,
            FangyuanVisualReplayMismatchField::PersonalityLayer
                | FangyuanVisualReplayMismatchField::State
                | FangyuanVisualReplayMismatchField::MaterialParams
                | FangyuanVisualReplayMismatchField::Degrade
        ));
        assert!(mismatch.summary_line().contains("tick=120"));
    }

    #[test]
    fn fangyuan_visual_replay_delayed_input_is_empty_before_start_and_stable_after_skip() {
        let replay = replay_with_events(vec![projectile_replay("evt_delay", 150, 150)]);

        let before = report_for(&replay, 140);
        let skipped = report_for(&replay, 175);
        let direct = report_for(&replay, 175);

        assert_eq!(before.samples[0].rule_state_count, 0);
        assert_eq!(before.samples[0].personality_state_count, 0);
        assert_eq!(skipped.visual_hash, direct.visual_hash);
        assert_eq!(
            compare_fangyuan_visual_replay_reports(&skipped, &direct),
            None
        );
    }

    #[test]
    fn fangyuan_visual_replay_seed_difference_changes_visual_hash() {
        let mut replay = replay_with_events(vec![projectile_replay("evt_seed_a", 100, 100)]);
        let mut visual = fangyuan_default_skill_visual_blueprints()
            .into_iter()
            .find(|visual| visual.id == "skill.visual.projectile")
            .unwrap();
        visual.vfx_recipe.as_mut().unwrap().seed_policy = FangyuanVfxSeedPolicy::External;
        let catalog = FangyuanVisualReplayCatalog::new(
            FangyuanSkillTemplateRegistry::with_defaults(),
            vec![visual],
        );
        let options = FangyuanVisualReplayRunOptions::new(120, 30);
        let first = summarize_fangyuan_visual_replay(&replay, &catalog, &options).unwrap();

        replay.events[0].replay_event.external_seed = Some(987);
        let second = summarize_fangyuan_visual_replay(&replay, &catalog, &options).unwrap();

        assert_ne!(first.visual_hash, second.visual_hash);
        assert_ne!(
            first.samples[0].personality_layer_hash,
            second.samples[0].personality_layer_hash
        );
    }

    #[test]
    fn fangyuan_visual_replay_degrade_pressure_and_lod_are_in_visual_summary_hash() {
        let base = replay_with_events(vec![projectile_replay("evt_pressure", 100, 100)]);
        let degraded = replay_with_events(vec![
            projectile_replay("evt_pressure", 100, 100)
                .with_degrade_level(FangyuanSkillDegradeLevel::Critical)
                .with_lod(FangyuanVisualReplayLod {
                    object_kind: FangyuanLodObjectKind::SkillVfx,
                    level: FangyuanLodLevel::L4HiddenRuleOnly,
                    rule_layer_retained: true,
                }),
        ]);

        let first = report_for(&base, 120);
        let second = report_for(&degraded, 120);
        let mismatch =
            compare_fangyuan_visual_replay_reports(&first, &second).expect("hash should differ");

        assert_ne!(first.visual_hash, second.visual_hash);
        assert_eq!(
            second.samples[0].degrade_level,
            FangyuanSkillDegradeLevel::Critical
        );
        assert_eq!(
            second.samples[0].lod.level,
            FangyuanLodLevel::L4HiddenRuleOnly
        );
        assert!(matches!(
            mismatch.field,
            FangyuanVisualReplayMismatchField::PersonalityLayer
                | FangyuanVisualReplayMismatchField::State
                | FangyuanVisualReplayMismatchField::MaterialParams
                | FangyuanVisualReplayMismatchField::Lod
                | FangyuanVisualReplayMismatchField::Degrade
        ));
    }

    #[test]
    fn fangyuan_visual_replay_cache_hit_miss_and_fallback_paths_are_hash_inputs() {
        let hit = replay_with_events(vec![
            projectile_replay("evt_cache", 100, 100)
                .with_cache_path(FangyuanVisualReplayCachePath::CacheHit),
        ]);
        let fallback = replay_with_events(vec![
            projectile_replay("evt_cache", 100, 100)
                .with_cache_path(FangyuanVisualReplayCachePath::CacheMissFallback)
                .with_fallback(FangyuanVisualReplayFallback {
                    domain: FangyuanBlueprintFallbackDomain::Skill,
                    mode: FangyuanBlueprintMissingFallbackMode::RuleOnly,
                    recovery_key: "skill.visual.projectile".to_string(),
                    rule_only: true,
                }),
        ]);

        let first = report_for(&hit, 120);
        let second = report_for(&fallback, 120);
        let mismatch =
            compare_fangyuan_visual_replay_reports(&first, &second).expect("hash should differ");

        assert_ne!(first.visual_hash, second.visual_hash);
        assert_eq!(
            first.samples[0].cache_path,
            FangyuanVisualReplayCachePath::CacheHit
        );
        assert_eq!(
            second.samples[0].cache_path,
            FangyuanVisualReplayCachePath::CacheMissFallback
        );
        assert_eq!(second.samples[0].fallback.as_ref().unwrap().rule_only, true);
        assert!(matches!(
            mismatch.field,
            FangyuanVisualReplayMismatchField::CachePath
                | FangyuanVisualReplayMismatchField::Fallback
        ));
    }
}
