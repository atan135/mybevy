use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use super::{FangyuanChunkBounds, FangyuanChunkManifestEntry};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanLodLevel {
    L0Full,
    L1Reduced,
    L2Silhouette,
    L3Marker,
    L4HiddenRuleOnly,
}

impl FangyuanLodLevel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::L0Full => "full",
            Self::L1Reduced => "reduced",
            Self::L2Silhouette => "silhouette",
            Self::L3Marker => "marker",
            Self::L4HiddenRuleOnly => "hidden_rule_only",
        }
    }

    pub const fn keeps_render_payload(self) -> bool {
        !matches!(self, Self::L4HiddenRuleOnly)
    }

    pub const fn is_marker_or_lower(self) -> bool {
        matches!(self, Self::L3Marker | Self::L4HiddenRuleOnly)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanLodObjectKind {
    StaticObject,
    HomeDecoration,
    Equipment,
    Npc,
    SkillVfx,
    TiandaoObject,
}

impl FangyuanLodObjectKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StaticObject => "static_object",
            Self::HomeDecoration => "home_decoration",
            Self::Equipment => "equipment",
            Self::Npc => "npc",
            Self::SkillVfx => "skill_vfx",
            Self::TiandaoObject => "tiandao_object",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FangyuanLodMapping {
    pub near: FangyuanLodLevel,
    pub mid: FangyuanLodLevel,
    pub far: FangyuanLodLevel,
    pub marker: FangyuanLodLevel,
    pub outside_aoi: FangyuanLodLevel,
}

impl FangyuanLodMapping {
    pub const fn select_for_band(self, band: FangyuanAoiBand) -> FangyuanLodLevel {
        match band {
            FangyuanAoiBand::Near => self.near,
            FangyuanAoiBand::Mid => self.mid,
            FangyuanAoiBand::Far => self.far,
            FangyuanAoiBand::Marker => self.marker,
            FangyuanAoiBand::Outside => self.outside_aoi,
        }
    }
}

pub const fn default_fangyuan_lod_mapping(kind: FangyuanLodObjectKind) -> FangyuanLodMapping {
    match kind {
        FangyuanLodObjectKind::StaticObject => FangyuanLodMapping {
            near: FangyuanLodLevel::L0Full,
            mid: FangyuanLodLevel::L1Reduced,
            far: FangyuanLodLevel::L2Silhouette,
            marker: FangyuanLodLevel::L3Marker,
            outside_aoi: FangyuanLodLevel::L4HiddenRuleOnly,
        },
        FangyuanLodObjectKind::HomeDecoration => FangyuanLodMapping {
            near: FangyuanLodLevel::L0Full,
            mid: FangyuanLodLevel::L2Silhouette,
            far: FangyuanLodLevel::L3Marker,
            marker: FangyuanLodLevel::L3Marker,
            outside_aoi: FangyuanLodLevel::L4HiddenRuleOnly,
        },
        FangyuanLodObjectKind::Equipment => FangyuanLodMapping {
            near: FangyuanLodLevel::L0Full,
            mid: FangyuanLodLevel::L1Reduced,
            far: FangyuanLodLevel::L2Silhouette,
            marker: FangyuanLodLevel::L3Marker,
            outside_aoi: FangyuanLodLevel::L4HiddenRuleOnly,
        },
        FangyuanLodObjectKind::Npc => FangyuanLodMapping {
            near: FangyuanLodLevel::L0Full,
            mid: FangyuanLodLevel::L1Reduced,
            far: FangyuanLodLevel::L2Silhouette,
            marker: FangyuanLodLevel::L3Marker,
            outside_aoi: FangyuanLodLevel::L4HiddenRuleOnly,
        },
        FangyuanLodObjectKind::SkillVfx => FangyuanLodMapping {
            near: FangyuanLodLevel::L0Full,
            mid: FangyuanLodLevel::L1Reduced,
            far: FangyuanLodLevel::L3Marker,
            marker: FangyuanLodLevel::L4HiddenRuleOnly,
            outside_aoi: FangyuanLodLevel::L4HiddenRuleOnly,
        },
        FangyuanLodObjectKind::TiandaoObject => FangyuanLodMapping {
            near: FangyuanLodLevel::L0Full,
            mid: FangyuanLodLevel::L1Reduced,
            far: FangyuanLodLevel::L2Silhouette,
            marker: FangyuanLodLevel::L3Marker,
            outside_aoi: FangyuanLodLevel::L4HiddenRuleOnly,
        },
    }
}

pub fn default_fangyuan_lod_level(
    kind: FangyuanLodObjectKind,
    band: FangyuanAoiBand,
) -> FangyuanLodLevel {
    default_fangyuan_lod_mapping(kind).select_for_band(band)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FangyuanAoiConfig {
    pub load_radius: f32,
    pub keep_radius: f32,
    pub marker_radius: f32,
    pub near_radius: f32,
    pub mid_radius: f32,
    pub far_radius: f32,
}

impl FangyuanAoiConfig {
    pub const fn new(
        load_radius: f32,
        keep_radius: f32,
        marker_radius: f32,
        near_radius: f32,
        mid_radius: f32,
        far_radius: f32,
    ) -> Self {
        Self {
            load_radius,
            keep_radius,
            marker_radius,
            near_radius,
            mid_radius,
            far_radius,
        }
    }

    pub fn validate(&self) -> Result<(), FangyuanAoiConfigError> {
        validate_non_negative_finite("load_radius", self.load_radius)?;
        validate_non_negative_finite("keep_radius", self.keep_radius)?;
        validate_non_negative_finite("marker_radius", self.marker_radius)?;
        validate_non_negative_finite("near_radius", self.near_radius)?;
        validate_non_negative_finite("mid_radius", self.mid_radius)?;
        validate_non_negative_finite("far_radius", self.far_radius)?;

        if self.load_radius > self.keep_radius {
            return Err(FangyuanAoiConfigError::InvalidOrdering {
                smaller_field: "load_radius",
                smaller_value: self.load_radius,
                larger_field: "keep_radius",
                larger_value: self.keep_radius,
            });
        }
        if self.keep_radius > self.marker_radius {
            return Err(FangyuanAoiConfigError::InvalidOrdering {
                smaller_field: "keep_radius",
                smaller_value: self.keep_radius,
                larger_field: "marker_radius",
                larger_value: self.marker_radius,
            });
        }
        if self.near_radius > self.mid_radius {
            return Err(FangyuanAoiConfigError::InvalidOrdering {
                smaller_field: "near_radius",
                smaller_value: self.near_radius,
                larger_field: "mid_radius",
                larger_value: self.mid_radius,
            });
        }
        if self.mid_radius > self.far_radius {
            return Err(FangyuanAoiConfigError::InvalidOrdering {
                smaller_field: "mid_radius",
                smaller_value: self.mid_radius,
                larger_field: "far_radius",
                larger_value: self.far_radius,
            });
        }
        if self.far_radius > self.marker_radius {
            return Err(FangyuanAoiConfigError::InvalidOrdering {
                smaller_field: "far_radius",
                smaller_value: self.far_radius,
                larger_field: "marker_radius",
                larger_value: self.marker_radius,
            });
        }

        Ok(())
    }
}

impl Default for FangyuanAoiConfig {
    fn default() -> Self {
        Self {
            load_radius: 32.0,
            keep_radius: 40.0,
            marker_radius: 56.0,
            near_radius: 12.0,
            mid_radius: 28.0,
            far_radius: 40.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FangyuanAoiConfigError {
    InvalidScalar {
        field: &'static str,
        value: f32,
    },
    InvalidOrdering {
        smaller_field: &'static str,
        smaller_value: f32,
        larger_field: &'static str,
        larger_value: f32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanAoiBand {
    Near,
    Mid,
    Far,
    Marker,
    Outside,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanAoiObjectDescriptor {
    pub object_id: String,
    pub chunk_id: String,
    pub kind: FangyuanLodObjectKind,
    pub position: [f32; 3],
    pub priority: FangyuanAoiPriority,
}

impl FangyuanAoiObjectDescriptor {
    pub fn new(
        object_id: impl Into<String>,
        chunk_id: impl Into<String>,
        kind: FangyuanLodObjectKind,
        position: [f32; 3],
    ) -> Self {
        Self {
            object_id: object_id.into(),
            chunk_id: chunk_id.into(),
            kind,
            position,
            priority: FangyuanAoiPriority::Normal,
        }
    }

    pub fn with_priority(mut self, priority: FangyuanAoiPriority) -> Self {
        self.priority = priority;
        self
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FangyuanAoiPriority {
    Low,
    #[default]
    Normal,
    SelfAvatar,
    AuthorityRule,
}

impl FangyuanAoiPriority {
    pub const fn should_force_load(self) -> bool {
        matches!(self, Self::SelfAvatar | Self::AuthorityRule)
    }

    pub const fn should_keep_rule_visible(self) -> bool {
        matches!(self, Self::AuthorityRule)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FangyuanAoiSelection {
    pub load_chunks: Vec<String>,
    pub keep_chunks: Vec<String>,
    pub unload_chunks: Vec<String>,
    pub marker_chunks: Vec<String>,
    pub object_decisions: Vec<FangyuanAoiObjectDecision>,
}

impl FangyuanAoiSelection {
    pub fn visible_object_ids(&self) -> Vec<String> {
        self.object_decisions
            .iter()
            .filter(|decision| decision.visible)
            .map(|decision| decision.object_id.clone())
            .collect()
    }

    pub fn marker_object_ids(&self) -> Vec<String> {
        self.object_decisions
            .iter()
            .filter(|decision| decision.marker)
            .map(|decision| decision.object_id.clone())
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanAoiObjectDecision {
    pub object_id: String,
    pub chunk_id: String,
    pub kind: FangyuanLodObjectKind,
    pub band: FangyuanAoiBand,
    pub lod: FangyuanLodLevel,
    pub visible: bool,
    pub marker: bool,
    pub rule_layer_retained: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FangyuanAoiChunkState {
    Load,
    Keep,
    Marker,
    Outside,
}

pub fn select_fangyuan_aoi<'a>(
    observer_position: [f32; 3],
    config: FangyuanAoiConfig,
    entries: &[FangyuanChunkManifestEntry],
    objects: &[FangyuanAoiObjectDescriptor],
    loaded_chunk_ids: impl IntoIterator<Item = &'a str>,
) -> FangyuanAoiSelection {
    if config.validate().is_err() || !position_is_finite(observer_position) {
        let mut unload_chunks = loaded_chunk_ids
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        unload_chunks.sort();
        return FangyuanAoiSelection {
            unload_chunks,
            ..Default::default()
        };
    }

    let loaded = loaded_chunk_ids.into_iter().collect::<HashSet<_>>();
    let force_loaded_chunks = objects
        .iter()
        .filter(|object| object.priority.should_force_load())
        .map(|object| object.chunk_id.as_str())
        .collect::<HashSet<_>>();
    let chunk_states = entries
        .iter()
        .map(|entry| {
            let distance = distance_to_chunk_bounds(observer_position, &entry.bounds);
            let state = if distance <= config.load_radius
                || force_loaded_chunks.contains(entry.id.as_str())
            {
                FangyuanAoiChunkState::Load
            } else if loaded.contains(entry.id.as_str()) && distance <= config.keep_radius {
                FangyuanAoiChunkState::Keep
            } else if distance <= config.marker_radius {
                FangyuanAoiChunkState::Marker
            } else {
                FangyuanAoiChunkState::Outside
            };
            (entry.id.as_str(), state)
        })
        .collect::<HashMap<_, _>>();

    let mut load_chunks = Vec::new();
    let mut keep_chunks = Vec::new();
    let mut marker_chunks = Vec::new();
    for entry in entries {
        match chunk_states
            .get(entry.id.as_str())
            .copied()
            .unwrap_or(FangyuanAoiChunkState::Outside)
        {
            FangyuanAoiChunkState::Load => {
                if loaded.contains(entry.id.as_str()) {
                    keep_chunks.push(entry.id.clone());
                } else {
                    load_chunks.push(entry.id.clone());
                }
            }
            FangyuanAoiChunkState::Keep => keep_chunks.push(entry.id.clone()),
            FangyuanAoiChunkState::Marker => marker_chunks.push(entry.id.clone()),
            FangyuanAoiChunkState::Outside => {}
        }
    }

    let desired_loaded = load_chunks
        .iter()
        .chain(keep_chunks.iter())
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut unload_chunks = loaded
        .iter()
        .filter(|chunk_id| !desired_loaded.contains(**chunk_id))
        .map(|chunk_id| (*chunk_id).to_string())
        .collect::<Vec<_>>();

    let known_chunks = entries
        .iter()
        .map(|entry| entry.id.as_str())
        .collect::<HashSet<_>>();
    let mut object_decisions = objects
        .iter()
        .map(|object| {
            let chunk_state = chunk_states
                .get(object.chunk_id.as_str())
                .copied()
                .unwrap_or(if known_chunks.contains(object.chunk_id.as_str()) {
                    FangyuanAoiChunkState::Outside
                } else {
                    FangyuanAoiChunkState::Marker
                });
            let band = object_aoi_band(observer_position, object, &config, chunk_state);
            let lod = default_fangyuan_lod_level(object.kind, band);
            let rule_layer_retained = object.priority.should_keep_rule_visible()
                || object.kind == FangyuanLodObjectKind::SkillVfx;
            let visible = lod.keeps_render_payload() || rule_layer_retained;
            let marker = lod == FangyuanLodLevel::L3Marker;

            FangyuanAoiObjectDecision {
                object_id: object.object_id.clone(),
                chunk_id: object.chunk_id.clone(),
                kind: object.kind,
                band,
                lod,
                visible,
                marker,
                rule_layer_retained,
            }
        })
        .collect::<Vec<_>>();

    load_chunks.sort();
    keep_chunks.sort();
    unload_chunks.sort();
    marker_chunks.sort();
    object_decisions.sort_by(|left, right| {
        left.chunk_id
            .cmp(&right.chunk_id)
            .then_with(|| left.object_id.cmp(&right.object_id))
    });

    FangyuanAoiSelection {
        load_chunks,
        keep_chunks,
        unload_chunks,
        marker_chunks,
        object_decisions,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FangyuanHotspotMetrics {
    pub active_skill_count: u32,
    pub dynamic_primitive_count: u32,
    pub transparent_count: u32,
    pub emissive_total: u32,
    pub trail_count: u32,
    pub chunk_load_pressure: u32,
}

impl FangyuanHotspotMetrics {
    pub const fn empty() -> Self {
        Self {
            active_skill_count: 0,
            dynamic_primitive_count: 0,
            transparent_count: 0,
            emissive_total: 0,
            trail_count: 0,
            chunk_load_pressure: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FangyuanHotspotThresholds {
    pub active_skill_count: u32,
    pub dynamic_primitive_count: u32,
    pub transparent_count: u32,
    pub emissive_total: u32,
    pub trail_count: u32,
    pub chunk_load_pressure: u32,
    pub recover_margin_percent: u32,
}

impl FangyuanHotspotThresholds {
    pub fn recover_threshold(self, pressure: FangyuanHotspotPressureKind) -> u32 {
        let enter = self.enter_threshold(pressure);
        enter.saturating_mul(100u32.saturating_sub(self.recover_margin_percent)) / 100
    }

    pub const fn enter_threshold(self, pressure: FangyuanHotspotPressureKind) -> u32 {
        match pressure {
            FangyuanHotspotPressureKind::ActiveSkill => self.active_skill_count,
            FangyuanHotspotPressureKind::DynamicPrimitive => self.dynamic_primitive_count,
            FangyuanHotspotPressureKind::Transparent => self.transparent_count,
            FangyuanHotspotPressureKind::Emissive => self.emissive_total,
            FangyuanHotspotPressureKind::Trail => self.trail_count,
            FangyuanHotspotPressureKind::ChunkLoad => self.chunk_load_pressure,
        }
    }
}

impl Default for FangyuanHotspotThresholds {
    fn default() -> Self {
        Self {
            active_skill_count: 8,
            dynamic_primitive_count: 96,
            transparent_count: 48,
            emissive_total: 120,
            trail_count: 32,
            chunk_load_pressure: 6,
            recover_margin_percent: 25,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FangyuanHotspotPressureKind {
    ActiveSkill,
    DynamicPrimitive,
    Transparent,
    Emissive,
    Trail,
    ChunkLoad,
}

impl FangyuanHotspotPressureKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ActiveSkill => "active_skill",
            Self::DynamicPrimitive => "dynamic_primitive",
            Self::Transparent => "transparent_count",
            Self::Emissive => "emissive_total",
            Self::Trail => "trail_count",
            Self::ChunkLoad => "chunk_load_pressure",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FangyuanHotspotState {
    pub active: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanHotspotEvaluation {
    pub active: bool,
    pub severity: FangyuanHotspotSeverity,
    pub pressure_reasons: Vec<FangyuanHotspotReason>,
    pub tiandao_pressure: FangyuanTiandaoPressureSummary,
    pub degrade_plan: Vec<FangyuanHotspotDegradeStep>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanHotspotSeverity {
    Normal,
    Warm,
    Hot,
    Critical,
}

impl Default for FangyuanHotspotSeverity {
    fn default() -> Self {
        Self::Normal
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanHotspotReason {
    pub kind: FangyuanHotspotPressureKind,
    pub value: u32,
    pub enter_threshold: u32,
    pub recover_threshold: u32,
    pub active_by_hysteresis: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanTiandaoPressureSummary {
    pub label: String,
    pub explanation: String,
    pub primary_reason: Option<FangyuanHotspotPressureKind>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanHotspotDegradeStep {
    pub target: FangyuanHotspotDegradeTarget,
    pub order: u8,
    pub reason: String,
    pub preserves_rule_layer: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FangyuanHotspotDegradeTarget {
    Decoration,
    Transparent,
    Emissive,
    Trail,
    Residue,
    DistantPersonality,
    RuleLayerCompression,
}

impl FangyuanHotspotDegradeTarget {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Decoration => "decoration",
            Self::Transparent => "transparent",
            Self::Emissive => "emissive",
            Self::Trail => "trail",
            Self::Residue => "residue",
            Self::DistantPersonality => "distant_personality",
            Self::RuleLayerCompression => "rule_layer_compression",
        }
    }

    pub const fn order(self) -> u8 {
        match self {
            Self::Decoration => 0,
            Self::Transparent => 1,
            Self::Emissive => 2,
            Self::Trail => 3,
            Self::Residue => 4,
            Self::DistantPersonality => 5,
            Self::RuleLayerCompression => 6,
        }
    }

    pub const fn preserves_rule_layer(self) -> bool {
        !matches!(self, Self::RuleLayerCompression)
    }
}

pub fn evaluate_fangyuan_hotspot(
    metrics: FangyuanHotspotMetrics,
    thresholds: FangyuanHotspotThresholds,
    previous_state: FangyuanHotspotState,
) -> FangyuanHotspotEvaluation {
    let pressure_reasons = hotspot_pressure_reasons(metrics, thresholds, previous_state);
    let active = !pressure_reasons.is_empty();
    let severity = hotspot_severity(&pressure_reasons);
    let tiandao_pressure = FangyuanTiandaoPressureSummary {
        label: tiandao_pressure_label(severity).to_string(),
        explanation: tiandao_pressure_explanation(&pressure_reasons),
        primary_reason: pressure_reasons.first().map(|reason| reason.kind),
    };
    let degrade_plan = if active {
        fangyuan_hotspot_degrade_plan(&pressure_reasons)
    } else {
        Vec::new()
    };

    FangyuanHotspotEvaluation {
        active,
        severity,
        pressure_reasons,
        tiandao_pressure,
        degrade_plan,
    }
}

pub fn fangyuan_hotspot_degrade_plan(
    pressure_reasons: &[FangyuanHotspotReason],
) -> Vec<FangyuanHotspotDegradeStep> {
    let reason_text = if pressure_reasons.is_empty() {
        "prevent hotspot pressure before it reaches user-visible instability".to_string()
    } else {
        let joined = pressure_reasons
            .iter()
            .map(|reason| reason.kind.as_str())
            .collect::<Vec<_>>()
            .join(",");
        format!("respond to hotspot pressure from {joined}")
    };
    let mut plan = [
        FangyuanHotspotDegradeTarget::Decoration,
        FangyuanHotspotDegradeTarget::Transparent,
        FangyuanHotspotDegradeTarget::Emissive,
        FangyuanHotspotDegradeTarget::Trail,
        FangyuanHotspotDegradeTarget::Residue,
        FangyuanHotspotDegradeTarget::DistantPersonality,
        FangyuanHotspotDegradeTarget::RuleLayerCompression,
    ]
    .into_iter()
    .map(|target| FangyuanHotspotDegradeStep {
        target,
        order: target.order(),
        reason: if target == FangyuanHotspotDegradeTarget::RuleLayerCompression {
            format!("{reason_text}; compress rule-layer presentation only after personality and residue are reduced")
        } else {
            reason_text.clone()
        },
        preserves_rule_layer: target.preserves_rule_layer(),
    })
    .collect::<Vec<_>>();
    plan.sort_by_key(|step| step.order);
    plan
}

fn hotspot_pressure_reasons(
    metrics: FangyuanHotspotMetrics,
    thresholds: FangyuanHotspotThresholds,
    previous_state: FangyuanHotspotState,
) -> Vec<FangyuanHotspotReason> {
    let mut reasons = Vec::new();
    for (kind, value) in [
        (
            FangyuanHotspotPressureKind::ActiveSkill,
            metrics.active_skill_count,
        ),
        (
            FangyuanHotspotPressureKind::DynamicPrimitive,
            metrics.dynamic_primitive_count,
        ),
        (
            FangyuanHotspotPressureKind::Transparent,
            metrics.transparent_count,
        ),
        (
            FangyuanHotspotPressureKind::Emissive,
            metrics.emissive_total,
        ),
        (FangyuanHotspotPressureKind::Trail, metrics.trail_count),
        (
            FangyuanHotspotPressureKind::ChunkLoad,
            metrics.chunk_load_pressure,
        ),
    ] {
        let enter_threshold = thresholds.enter_threshold(kind);
        let recover_threshold = thresholds.recover_threshold(kind);
        let entered = value > enter_threshold;
        let active_by_hysteresis =
            previous_state.active && value > recover_threshold && enter_threshold > 0;

        if entered || active_by_hysteresis {
            reasons.push(FangyuanHotspotReason {
                kind,
                value,
                enter_threshold,
                recover_threshold,
                active_by_hysteresis: active_by_hysteresis && !entered,
            });
        }
    }

    reasons.sort_by_key(|reason| {
        (
            !matches!(
                reason.kind,
                FangyuanHotspotPressureKind::ActiveSkill
                    | FangyuanHotspotPressureKind::DynamicPrimitive
                    | FangyuanHotspotPressureKind::ChunkLoad
            ),
            reason.kind,
        )
    });
    reasons
}

fn hotspot_severity(reasons: &[FangyuanHotspotReason]) -> FangyuanHotspotSeverity {
    let worst_percent = reasons
        .iter()
        .filter(|reason| reason.enter_threshold > 0)
        .map(|reason| reason.value.saturating_mul(100) / reason.enter_threshold)
        .max()
        .unwrap_or(0);

    match worst_percent {
        0..=100 => {
            if reasons.is_empty() {
                FangyuanHotspotSeverity::Normal
            } else {
                FangyuanHotspotSeverity::Warm
            }
        }
        101..=149 => FangyuanHotspotSeverity::Hot,
        _ => FangyuanHotspotSeverity::Critical,
    }
}

fn tiandao_pressure_label(severity: FangyuanHotspotSeverity) -> &'static str {
    match severity {
        FangyuanHotspotSeverity::Normal => "tiandao_pressure_stable",
        FangyuanHotspotSeverity::Warm => "tiandao_pressure_warm",
        FangyuanHotspotSeverity::Hot => "tiandao_pressure_hot",
        FangyuanHotspotSeverity::Critical => "tiandao_pressure_critical",
    }
}

fn tiandao_pressure_explanation(reasons: &[FangyuanHotspotReason]) -> String {
    if reasons.is_empty() {
        return "tiandao pressure is stable; no degradation is required".to_string();
    }

    let primary = &reasons[0];
    let hysteresis = if primary.active_by_hysteresis {
        " and remains active until it falls below recovery hysteresis"
    } else {
        ""
    };
    format!(
        "tiandao pressure is elevated by {} {} > enter {}{}",
        primary.kind.as_str(),
        primary.value,
        primary.enter_threshold,
        hysteresis
    )
}

fn object_aoi_band(
    observer_position: [f32; 3],
    object: &FangyuanAoiObjectDescriptor,
    config: &FangyuanAoiConfig,
    chunk_state: FangyuanAoiChunkState,
) -> FangyuanAoiBand {
    if object.priority.should_force_load() {
        return FangyuanAoiBand::Near;
    }

    let distance = distance_between(observer_position, object.position);
    if !distance.is_finite() {
        return FangyuanAoiBand::Outside;
    }

    if distance <= config.near_radius {
        FangyuanAoiBand::Near
    } else if distance <= config.mid_radius {
        FangyuanAoiBand::Mid
    } else if distance <= config.far_radius {
        FangyuanAoiBand::Far
    } else if matches!(chunk_state, FangyuanAoiChunkState::Marker)
        || distance <= config.marker_radius
    {
        FangyuanAoiBand::Marker
    } else {
        FangyuanAoiBand::Outside
    }
}

fn validate_non_negative_finite(
    field: &'static str,
    value: f32,
) -> Result<(), FangyuanAoiConfigError> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(FangyuanAoiConfigError::InvalidScalar { field, value })
    }
}

fn position_is_finite(position: [f32; 3]) -> bool {
    position.into_iter().all(f32::is_finite)
}

fn distance_to_chunk_bounds(position: [f32; 3], bounds: &FangyuanChunkBounds) -> f32 {
    position
        .into_iter()
        .enumerate()
        .map(|(axis, value)| {
            if value < bounds.min[axis] {
                bounds.min[axis] - value
            } else if value > bounds.max[axis] {
                value - bounds.max[axis]
            } else {
                0.0
            }
        })
        .map(|delta| delta * delta)
        .sum::<f32>()
        .sqrt()
}

fn distance_between(left: [f32; 3], right: [f32; 3]) -> f32 {
    left.into_iter()
        .zip(right)
        .map(|(left, right)| {
            let delta = left - right;
            delta * delta
        })
        .sum::<f32>()
        .sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::fangyuan::{FangyuanChunkBudgetSummary, FangyuanChunkRegionMetadata};

    #[test]
    fn fangyuan_lod_default_mappings_cover_required_object_types() {
        let cases = [
            (
                FangyuanLodObjectKind::StaticObject,
                FangyuanLodLevel::L2Silhouette,
            ),
            (
                FangyuanLodObjectKind::HomeDecoration,
                FangyuanLodLevel::L3Marker,
            ),
            (
                FangyuanLodObjectKind::Equipment,
                FangyuanLodLevel::L2Silhouette,
            ),
            (FangyuanLodObjectKind::Npc, FangyuanLodLevel::L2Silhouette),
            (FangyuanLodObjectKind::SkillVfx, FangyuanLodLevel::L3Marker),
            (
                FangyuanLodObjectKind::TiandaoObject,
                FangyuanLodLevel::L2Silhouette,
            ),
        ];

        for (kind, far_lod) in cases {
            let mapping = default_fangyuan_lod_mapping(kind);
            assert_eq!(mapping.near, FangyuanLodLevel::L0Full, "{}", kind.as_str());
            assert_eq!(mapping.far, far_lod, "{}", kind.as_str());
            assert_eq!(mapping.outside_aoi, FangyuanLodLevel::L4HiddenRuleOnly);
        }

        assert_eq!(FangyuanLodLevel::L0Full.as_str(), "full");
        assert_eq!(FangyuanLodLevel::L1Reduced.as_str(), "reduced");
        assert_eq!(FangyuanLodLevel::L2Silhouette.as_str(), "silhouette");
        assert_eq!(FangyuanLodLevel::L3Marker.as_str(), "marker");
        assert_eq!(
            FangyuanLodLevel::L4HiddenRuleOnly.as_str(),
            "hidden_rule_only"
        );
    }

    #[test]
    fn fangyuan_lod_rule_layer_can_be_retained_when_render_payload_is_hidden() {
        assert!(!FangyuanLodLevel::L4HiddenRuleOnly.keeps_render_payload());
        assert!(FangyuanLodLevel::L3Marker.keeps_render_payload());
        assert!(FangyuanLodLevel::L3Marker.is_marker_or_lower());
        assert!(FangyuanLodLevel::L4HiddenRuleOnly.is_marker_or_lower());
    }

    #[test]
    fn fangyuan_aoi_selects_load_keep_unload_marker_and_object_lod() {
        let entries = vec![
            manifest_entry("chunk_near", -8.0, 8.0),
            manifest_entry("chunk_keep", 34.0, 42.0),
            manifest_entry("chunk_marker", 50.0, 54.0),
            manifest_entry("chunk_far", 90.0, 98.0),
        ];
        let objects = vec![
            object(
                "hero",
                "chunk_far",
                FangyuanLodObjectKind::Npc,
                [94.0, 0.0, 0.0],
            )
            .with_priority(FangyuanAoiPriority::SelfAvatar),
            object(
                "tree",
                "chunk_near",
                FangyuanLodObjectKind::HomeDecoration,
                [2.0, 0.0, 0.0],
            ),
            object(
                "distant",
                "chunk_keep",
                FangyuanLodObjectKind::StaticObject,
                [38.0, 0.0, 0.0],
            ),
            object(
                "marker",
                "chunk_marker",
                FangyuanLodObjectKind::Equipment,
                [52.0, 0.0, 0.0],
            ),
            object(
                "hidden",
                "chunk_far",
                FangyuanLodObjectKind::HomeDecoration,
                [96.0, 0.0, 0.0],
            ),
        ];
        let config = FangyuanAoiConfig::default();

        let selection = select_fangyuan_aoi(
            [0.0, 0.0, 0.0],
            config,
            &entries,
            &objects,
            ["chunk_keep", "chunk_old"],
        );

        assert_eq!(selection.load_chunks, vec!["chunk_far", "chunk_near"]);
        assert_eq!(selection.keep_chunks, vec!["chunk_keep"]);
        assert_eq!(selection.unload_chunks, vec!["chunk_old"]);
        assert_eq!(selection.marker_chunks, vec!["chunk_marker"]);

        let decisions = decisions_by_id(&selection);
        assert_eq!(decisions["tree"].band, FangyuanAoiBand::Near);
        assert_eq!(decisions["tree"].lod, FangyuanLodLevel::L0Full);
        assert_eq!(decisions["distant"].band, FangyuanAoiBand::Far);
        assert_eq!(decisions["distant"].lod, FangyuanLodLevel::L2Silhouette);
        assert_eq!(decisions["marker"].band, FangyuanAoiBand::Marker);
        assert_eq!(decisions["marker"].lod, FangyuanLodLevel::L3Marker);
        assert_eq!(decisions["hero"].band, FangyuanAoiBand::Near);
        assert_eq!(decisions["hero"].lod, FangyuanLodLevel::L0Full);
        assert!(decisions["hero"].visible);
    }

    #[test]
    fn fangyuan_aoi_hysteresis_keeps_loaded_chunk_inside_keep_radius() {
        let entries = vec![manifest_entry("edge", 36.0, 38.0)];
        let selection = select_fangyuan_aoi(
            [0.0, 0.0, 0.0],
            FangyuanAoiConfig::default(),
            &entries,
            &[],
            ["edge"],
        );

        assert!(selection.load_chunks.is_empty());
        assert_eq!(selection.keep_chunks, vec!["edge"]);
        assert!(selection.unload_chunks.is_empty());

        let selection = select_fangyuan_aoi(
            [0.0, 0.0, 0.0],
            FangyuanAoiConfig::default(),
            &entries,
            &[],
            [],
        );
        assert!(selection.load_chunks.is_empty());
        assert!(selection.keep_chunks.is_empty());
        assert_eq!(selection.marker_chunks, vec!["edge"]);
    }

    #[test]
    fn fangyuan_aoi_rule_priority_retains_rule_layer_beyond_marker_range() {
        let entries = vec![manifest_entry("rule_chunk", 90.0, 98.0)];
        let objects = vec![
            object(
                "skill_rule",
                "rule_chunk",
                FangyuanLodObjectKind::SkillVfx,
                [96.0, 0.0, 0.0],
            )
            .with_priority(FangyuanAoiPriority::AuthorityRule),
        ];

        let selection = select_fangyuan_aoi(
            [0.0, 0.0, 0.0],
            FangyuanAoiConfig::default(),
            &entries,
            &objects,
            [],
        );

        assert_eq!(selection.load_chunks, vec!["rule_chunk"]);
        let decision = &selection.object_decisions[0];
        assert_eq!(decision.object_id, "skill_rule");
        assert_eq!(decision.band, FangyuanAoiBand::Near);
        assert!(decision.rule_layer_retained);
        assert!(decision.visible);
    }

    #[test]
    fn fangyuan_aoi_invalid_config_unloads_loaded_chunks_as_fallback() {
        let config = FangyuanAoiConfig {
            load_radius: f32::NAN,
            ..Default::default()
        };
        let selection = select_fangyuan_aoi([0.0, 0.0, 0.0], config, &[], &[], ["b", "a"]);

        assert_eq!(selection.unload_chunks, vec!["a", "b"]);
        assert!(selection.load_chunks.is_empty());
    }

    #[test]
    fn fangyuan_hotspot_reports_reasons_tiandao_summary_and_ordered_degrade_plan() {
        let metrics = FangyuanHotspotMetrics {
            active_skill_count: 10,
            dynamic_primitive_count: 120,
            transparent_count: 64,
            emissive_total: 140,
            trail_count: 40,
            chunk_load_pressure: 8,
        };
        let evaluation = evaluate_fangyuan_hotspot(
            metrics,
            FangyuanHotspotThresholds::default(),
            FangyuanHotspotState::default(),
        );

        assert!(evaluation.active);
        assert_eq!(evaluation.severity, FangyuanHotspotSeverity::Hot);
        assert!(evaluation.pressure_reasons.iter().any(|reason| {
            reason.kind == FangyuanHotspotPressureKind::ActiveSkill && reason.value == 10
        }));
        assert_eq!(
            evaluation.tiandao_pressure.primary_reason,
            Some(FangyuanHotspotPressureKind::ActiveSkill)
        );
        assert!(
            evaluation
                .tiandao_pressure
                .explanation
                .contains("active_skill")
        );
        assert_eq!(
            evaluation
                .degrade_plan
                .iter()
                .map(|step| step.target)
                .collect::<Vec<_>>(),
            vec![
                FangyuanHotspotDegradeTarget::Decoration,
                FangyuanHotspotDegradeTarget::Transparent,
                FangyuanHotspotDegradeTarget::Emissive,
                FangyuanHotspotDegradeTarget::Trail,
                FangyuanHotspotDegradeTarget::Residue,
                FangyuanHotspotDegradeTarget::DistantPersonality,
                FangyuanHotspotDegradeTarget::RuleLayerCompression,
            ]
        );
        assert!(
            evaluation
                .degrade_plan
                .iter()
                .take(6)
                .all(|step| step.preserves_rule_layer)
        );
        assert!(!evaluation.degrade_plan.last().unwrap().preserves_rule_layer);
    }

    #[test]
    fn fangyuan_hotspot_hysteresis_holds_until_recover_threshold() {
        let thresholds = FangyuanHotspotThresholds {
            active_skill_count: 10,
            recover_margin_percent: 30,
            ..Default::default()
        };
        let held = evaluate_fangyuan_hotspot(
            FangyuanHotspotMetrics {
                active_skill_count: 8,
                ..FangyuanHotspotMetrics::empty()
            },
            thresholds,
            FangyuanHotspotState { active: true },
        );
        assert!(held.active);
        assert_eq!(held.pressure_reasons[0].recover_threshold, 7);
        assert!(held.pressure_reasons[0].active_by_hysteresis);

        let recovered = evaluate_fangyuan_hotspot(
            FangyuanHotspotMetrics {
                active_skill_count: 7,
                ..FangyuanHotspotMetrics::empty()
            },
            thresholds,
            FangyuanHotspotState { active: true },
        );
        assert!(!recovered.active);
        assert!(recovered.degrade_plan.is_empty());
    }

    #[test]
    fn fangyuan_hotspot_rule_layer_is_last_degrade_target() {
        let plan = fangyuan_hotspot_degrade_plan(&[]);

        assert_eq!(
            plan.last().map(|step| step.target),
            Some(FangyuanHotspotDegradeTarget::RuleLayerCompression)
        );
        assert!(
            plan.iter()
                .position(|step| step.target == FangyuanHotspotDegradeTarget::DistantPersonality)
                .unwrap()
                < plan
                    .iter()
                    .position(
                        |step| step.target == FangyuanHotspotDegradeTarget::RuleLayerCompression
                    )
                    .unwrap()
        );
    }

    fn manifest_entry(id: &str, min_x: f32, max_x: f32) -> FangyuanChunkManifestEntry {
        FangyuanChunkManifestEntry {
            id: id.to_string(),
            bounds: FangyuanChunkBounds::new([min_x, 0.0, -8.0], [max_x, 6.0, 8.0]),
            region: FangyuanChunkRegionMetadata {
                region_id: "home.default".to_string(),
                layer: "ground".to_string(),
                tags: Vec::new(),
            },
            dev_ron: None,
            bin: None,
            hash: None,
            data_version: None,
            budget: FangyuanChunkBudgetSummary {
                prefab_instance_count: 1,
                tiandao_ref_count: 0,
                static_decoration_count: 0,
                total_ref_count: 1,
                prefab_cost: 1,
                tiandao_cost: 0,
                static_decoration_cost: 0,
                total_cost: 1,
            },
        }
    }

    fn object(
        object_id: &str,
        chunk_id: &str,
        kind: FangyuanLodObjectKind,
        position: [f32; 3],
    ) -> FangyuanAoiObjectDescriptor {
        FangyuanAoiObjectDescriptor::new(object_id, chunk_id, kind, position)
    }

    fn decisions_by_id(
        selection: &FangyuanAoiSelection,
    ) -> HashMap<String, FangyuanAoiObjectDecision> {
        selection
            .object_decisions
            .iter()
            .map(|decision| (decision.object_id.clone(), decision.clone()))
            .collect()
    }
}
