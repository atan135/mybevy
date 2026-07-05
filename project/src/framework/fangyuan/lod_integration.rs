use std::collections::{BTreeMap, BTreeSet};

use super::{
    FangyuanAoiBand, FangyuanAoiConfig, FangyuanAoiObjectDecision, FangyuanAoiObjectDescriptor,
    FangyuanAoiPriority, FangyuanAoiSelection, FangyuanChunkBounds, FangyuanChunkBudgetSummary,
    FangyuanChunkDebugSummary, FangyuanChunkManifestEntry, FangyuanChunkRegionMetadata,
    FangyuanHotspotEvaluation, FangyuanHotspotMetrics, FangyuanHotspotSeverity,
    FangyuanHotspotState, FangyuanHotspotThresholds, FangyuanLodLevel, FangyuanLodObjectKind,
    FangyuanObjectClass, FangyuanObjectTrialVisualPrimitive, FangyuanPrimitive,
    FangyuanPrimitiveRole, FangyuanPrimitiveSet, evaluate_fangyuan_hotspot, select_fangyuan_aoi,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FangyuanLodRenderPath {
    #[default]
    Standard,
    StaticMerge,
    StaticInstancing,
    Vfx,
    SkillLayer,
    Equipment,
    Npc,
    Tiandao,
    Marker,
    Hidden,
}

impl FangyuanLodRenderPath {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::StaticMerge => "static_merge",
            Self::StaticInstancing => "static_instancing",
            Self::Vfx => "vfx",
            Self::SkillLayer => "skill_layer",
            Self::Equipment => "equipment",
            Self::Npc => "npc",
            Self::Tiandao => "tiandao",
            Self::Marker => "marker",
            Self::Hidden => "hidden",
        }
    }

    pub const fn keeps_payload(self) -> bool {
        !matches!(self, Self::Hidden)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanLodRenderDescriptor {
    pub object_id: String,
    pub chunk_id: String,
    pub kind: FangyuanLodObjectKind,
    pub preferred_path: FangyuanLodRenderPath,
    pub position: [f32; 3],
    pub priority: FangyuanAoiPriority,
    pub primitive_count: usize,
    pub dynamic_primitive_count: usize,
    pub transparent_count: usize,
    pub emissive_total: u32,
    pub trail_count: usize,
    pub recycled: bool,
}

impl FangyuanLodRenderDescriptor {
    pub fn new(
        object_id: impl Into<String>,
        chunk_id: impl Into<String>,
        kind: FangyuanLodObjectKind,
        preferred_path: FangyuanLodRenderPath,
        position: [f32; 3],
    ) -> Self {
        Self {
            object_id: object_id.into(),
            chunk_id: chunk_id.into(),
            kind,
            preferred_path,
            position,
            priority: FangyuanAoiPriority::Normal,
            primitive_count: 1,
            dynamic_primitive_count: usize::from(matches!(
                kind,
                FangyuanLodObjectKind::Equipment
                    | FangyuanLodObjectKind::Npc
                    | FangyuanLodObjectKind::SkillVfx
                    | FangyuanLodObjectKind::TiandaoObject
            )),
            transparent_count: 0,
            emissive_total: 0,
            trail_count: 0,
            recycled: false,
        }
    }

    pub fn standard_static(
        object_id: impl Into<String>,
        chunk_id: impl Into<String>,
        position: [f32; 3],
    ) -> Self {
        Self::new(
            object_id,
            chunk_id,
            FangyuanLodObjectKind::StaticObject,
            FangyuanLodRenderPath::Standard,
            position,
        )
    }

    pub fn static_merge(
        object_id: impl Into<String>,
        chunk_id: impl Into<String>,
        position: [f32; 3],
    ) -> Self {
        Self::new(
            object_id,
            chunk_id,
            FangyuanLodObjectKind::StaticObject,
            FangyuanLodRenderPath::StaticMerge,
            position,
        )
    }

    pub fn static_instancing(
        object_id: impl Into<String>,
        chunk_id: impl Into<String>,
        position: [f32; 3],
    ) -> Self {
        Self::new(
            object_id,
            chunk_id,
            FangyuanLodObjectKind::StaticObject,
            FangyuanLodRenderPath::StaticInstancing,
            position,
        )
    }

    pub fn vfx(
        object_id: impl Into<String>,
        chunk_id: impl Into<String>,
        position: [f32; 3],
    ) -> Self {
        Self::new(
            object_id,
            chunk_id,
            FangyuanLodObjectKind::SkillVfx,
            FangyuanLodRenderPath::Vfx,
            position,
        )
        .with_dynamic_primitive_count(1)
    }

    pub fn skill_layer(
        object_id: impl Into<String>,
        chunk_id: impl Into<String>,
        position: [f32; 3],
    ) -> Self {
        Self::new(
            object_id,
            chunk_id,
            FangyuanLodObjectKind::SkillVfx,
            FangyuanLodRenderPath::SkillLayer,
            position,
        )
        .with_priority(FangyuanAoiPriority::AuthorityRule)
        .with_dynamic_primitive_count(1)
    }

    pub fn equipment(
        object_id: impl Into<String>,
        chunk_id: impl Into<String>,
        position: [f32; 3],
    ) -> Self {
        Self::new(
            object_id,
            chunk_id,
            FangyuanLodObjectKind::Equipment,
            FangyuanLodRenderPath::Equipment,
            position,
        )
    }

    pub fn npc(
        object_id: impl Into<String>,
        chunk_id: impl Into<String>,
        position: [f32; 3],
    ) -> Self {
        Self::new(
            object_id,
            chunk_id,
            FangyuanLodObjectKind::Npc,
            FangyuanLodRenderPath::Npc,
            position,
        )
    }

    pub fn tiandao(
        object_id: impl Into<String>,
        chunk_id: impl Into<String>,
        position: [f32; 3],
    ) -> Self {
        Self::new(
            object_id,
            chunk_id,
            FangyuanLodObjectKind::TiandaoObject,
            FangyuanLodRenderPath::Tiandao,
            position,
        )
    }

    pub fn with_priority(mut self, priority: FangyuanAoiPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_primitive_count(mut self, primitive_count: usize) -> Self {
        self.primitive_count = primitive_count;
        self
    }

    pub fn with_dynamic_primitive_count(mut self, dynamic_primitive_count: usize) -> Self {
        self.dynamic_primitive_count = dynamic_primitive_count;
        self
    }

    pub fn with_transparent_count(mut self, transparent_count: usize) -> Self {
        self.transparent_count = transparent_count;
        self
    }

    pub fn with_emissive_total(mut self, emissive_total: u32) -> Self {
        self.emissive_total = emissive_total;
        self
    }

    pub fn with_trail_count(mut self, trail_count: usize) -> Self {
        self.trail_count = trail_count;
        self
    }

    pub fn with_recycled(mut self, recycled: bool) -> Self {
        self.recycled = recycled;
        self
    }

    pub fn aoi_descriptor(&self) -> FangyuanAoiObjectDescriptor {
        FangyuanAoiObjectDescriptor::new(
            self.object_id.clone(),
            self.chunk_id.clone(),
            self.kind,
            self.position,
        )
        .with_priority(self.priority)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FangyuanLodDistribution {
    pub full: usize,
    pub reduced: usize,
    pub silhouette: usize,
    pub marker: usize,
    pub hidden: usize,
}

impl FangyuanLodDistribution {
    pub fn record(&mut self, lod: FangyuanLodLevel) {
        match lod {
            FangyuanLodLevel::L0Full => self.full += 1,
            FangyuanLodLevel::L1Reduced => self.reduced += 1,
            FangyuanLodLevel::L2Silhouette => self.silhouette += 1,
            FangyuanLodLevel::L3Marker => self.marker += 1,
            FangyuanLodLevel::L4HiddenRuleOnly => self.hidden += 1,
        }
    }

    pub fn total(&self) -> usize {
        self.full + self.reduced + self.silhouette + self.marker + self.hidden
    }

    pub fn label(&self) -> String {
        format!(
            "f{} r{} s{} m{} h{}",
            self.full, self.reduced, self.silhouette, self.marker, self.hidden
        )
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FangyuanLodRenderPathCounts {
    pub standard: usize,
    pub static_merge: usize,
    pub static_instancing: usize,
    pub vfx: usize,
    pub skill_layer: usize,
    pub equipment: usize,
    pub npc: usize,
    pub tiandao: usize,
    pub marker: usize,
    pub hidden: usize,
}

impl FangyuanLodRenderPathCounts {
    pub fn record(&mut self, path: FangyuanLodRenderPath) {
        match path {
            FangyuanLodRenderPath::Standard => self.standard += 1,
            FangyuanLodRenderPath::StaticMerge => self.static_merge += 1,
            FangyuanLodRenderPath::StaticInstancing => self.static_instancing += 1,
            FangyuanLodRenderPath::Vfx => self.vfx += 1,
            FangyuanLodRenderPath::SkillLayer => self.skill_layer += 1,
            FangyuanLodRenderPath::Equipment => self.equipment += 1,
            FangyuanLodRenderPath::Npc => self.npc += 1,
            FangyuanLodRenderPath::Tiandao => self.tiandao += 1,
            FangyuanLodRenderPath::Marker => self.marker += 1,
            FangyuanLodRenderPath::Hidden => self.hidden += 1,
        }
    }

    pub fn label(&self) -> String {
        format!(
            "std{} mg{} inst{} mk{} hid{}",
            self.standard, self.static_merge, self.static_instancing, self.marker, self.hidden
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanLodRenderDecision {
    pub object_id: String,
    pub chunk_id: String,
    pub kind: FangyuanLodObjectKind,
    pub band: FangyuanAoiBand,
    pub requested_lod: FangyuanLodLevel,
    pub effective_lod: FangyuanLodLevel,
    pub path: FangyuanLodRenderPath,
    pub visible: bool,
    pub marker: bool,
    pub hidden: bool,
    pub degraded_by_pressure: bool,
    pub degrade_reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanLodPressureSummary {
    pub active: bool,
    pub severity: FangyuanHotspotSeverity,
    pub pressure_label: String,
    pub bottleneck: String,
    pub degrade_reason: String,
}

impl Default for FangyuanLodPressureSummary {
    fn default() -> Self {
        Self {
            active: false,
            severity: FangyuanHotspotSeverity::Normal,
            pressure_label: "normal".to_string(),
            bottleneck: "-".to_string(),
            degrade_reason: "-".to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanLodIntegrationSummary {
    pub loaded_chunks: usize,
    pub loaded_chunk_ids: Vec<String>,
    pub aoi_radius: f32,
    pub lod_distribution: FangyuanLodDistribution,
    pub render_paths: FangyuanLodRenderPathCounts,
    pub visible_objects: usize,
    pub hidden_objects: usize,
    pub marker_objects: usize,
    pub pressure: FangyuanLodPressureSummary,
    pub decisions: Vec<FangyuanLodRenderDecision>,
}

impl Default for FangyuanLodIntegrationSummary {
    fn default() -> Self {
        Self {
            loaded_chunks: 0,
            loaded_chunk_ids: Vec::new(),
            aoi_radius: 0.0,
            lod_distribution: FangyuanLodDistribution::default(),
            render_paths: FangyuanLodRenderPathCounts::default(),
            visible_objects: 0,
            hidden_objects: 0,
            marker_objects: 0,
            pressure: FangyuanLodPressureSummary::default(),
            decisions: Vec::new(),
        }
    }
}

impl FangyuanLodIntegrationSummary {
    pub fn lod_distribution_label(&self) -> String {
        self.lod_distribution.label()
    }

    pub fn render_path_label(&self) -> String {
        self.render_paths.label()
    }

    pub fn pressure_label(&self) -> &str {
        self.pressure.pressure_label.as_str()
    }

    pub fn degrade_reason_label(&self) -> &str {
        self.pressure.degrade_reason.as_str()
    }

    pub fn combine_for_hud(&self, other: &Self) -> Self {
        let mut combined = self.clone();
        combined.loaded_chunks = combined.loaded_chunks.max(other.loaded_chunks);
        combined
            .loaded_chunk_ids
            .extend(other.loaded_chunk_ids.clone());
        combined.loaded_chunk_ids.sort();
        combined.loaded_chunk_ids.dedup();
        combined.aoi_radius = combined.aoi_radius.max(other.aoi_radius);

        combined.lod_distribution.full += other.lod_distribution.full;
        combined.lod_distribution.reduced += other.lod_distribution.reduced;
        combined.lod_distribution.silhouette += other.lod_distribution.silhouette;
        combined.lod_distribution.marker += other.lod_distribution.marker;
        combined.lod_distribution.hidden += other.lod_distribution.hidden;

        combined.render_paths.standard += other.render_paths.standard;
        combined.render_paths.static_merge += other.render_paths.static_merge;
        combined.render_paths.static_instancing += other.render_paths.static_instancing;
        combined.render_paths.vfx += other.render_paths.vfx;
        combined.render_paths.skill_layer += other.render_paths.skill_layer;
        combined.render_paths.equipment += other.render_paths.equipment;
        combined.render_paths.npc += other.render_paths.npc;
        combined.render_paths.tiandao += other.render_paths.tiandao;
        combined.render_paths.marker += other.render_paths.marker;
        combined.render_paths.hidden += other.render_paths.hidden;

        combined.visible_objects += other.visible_objects;
        combined.hidden_objects += other.hidden_objects;
        combined.marker_objects += other.marker_objects;
        if pressure_rank(other.pressure.severity) > pressure_rank(combined.pressure.severity) {
            combined.pressure = other.pressure.clone();
        }
        combined.decisions.extend(other.decisions.clone());
        combined.decisions.sort_by(|left, right| {
            left.chunk_id
                .cmp(&right.chunk_id)
                .then_with(|| left.object_id.cmp(&right.object_id))
        });
        combined
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FangyuanLodRenderSceneState {
    pub active_paths: BTreeMap<String, FangyuanLodRenderPath>,
    pub marker_ids: BTreeSet<String>,
    pub hidden_ids: BTreeSet<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FangyuanLodRenderCleanup {
    pub removed_object_ids: Vec<String>,
    pub replaced_paths: Vec<FangyuanLodRenderPathReplacement>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FangyuanLodRenderPathReplacement {
    pub object_id: String,
    pub from: FangyuanLodRenderPath,
    pub to: FangyuanLodRenderPath,
}

pub fn reconcile_fangyuan_lod_render_state(
    previous: &FangyuanLodRenderSceneState,
    summary: &FangyuanLodIntegrationSummary,
) -> (FangyuanLodRenderSceneState, FangyuanLodRenderCleanup) {
    let mut next = FangyuanLodRenderSceneState::default();
    let mut cleanup = FangyuanLodRenderCleanup::default();
    let mut seen = BTreeSet::new();

    for decision in &summary.decisions {
        seen.insert(decision.object_id.clone());
        if decision.path == FangyuanLodRenderPath::Hidden {
            next.hidden_ids.insert(decision.object_id.clone());
            if previous.active_paths.contains_key(&decision.object_id)
                && !cleanup.removed_object_ids.contains(&decision.object_id)
            {
                cleanup.removed_object_ids.push(decision.object_id.clone());
            }
            continue;
        }

        if decision.path == FangyuanLodRenderPath::Marker {
            next.marker_ids.insert(decision.object_id.clone());
        }
        if let Some(previous_path) = previous.active_paths.get(&decision.object_id)
            && *previous_path != decision.path
        {
            cleanup
                .replaced_paths
                .push(FangyuanLodRenderPathReplacement {
                    object_id: decision.object_id.clone(),
                    from: *previous_path,
                    to: decision.path,
                });
        }
        next.active_paths
            .insert(decision.object_id.clone(), decision.path);
    }

    for object_id in previous.active_paths.keys() {
        if !seen.contains(object_id) {
            cleanup.removed_object_ids.push(object_id.clone());
        }
    }
    cleanup.removed_object_ids.sort();
    cleanup.removed_object_ids.dedup();
    cleanup
        .replaced_paths
        .sort_by(|left, right| left.object_id.cmp(&right.object_id));

    (next, cleanup)
}

pub fn summarize_fangyuan_lod_integration(
    chunk_summary: &FangyuanChunkDebugSummary,
    aoi_radius: f32,
    selection: &FangyuanAoiSelection,
    descriptors: &[FangyuanLodRenderDescriptor],
    hotspot: &FangyuanHotspotEvaluation,
) -> FangyuanLodIntegrationSummary {
    let descriptor_by_id = descriptors
        .iter()
        .map(|descriptor| (descriptor.object_id.as_str(), descriptor))
        .collect::<BTreeMap<_, _>>();
    let decisions_by_id = selection
        .object_decisions
        .iter()
        .map(|decision| (decision.object_id.as_str(), decision))
        .collect::<BTreeMap<_, _>>();
    let unload_chunks = selection
        .unload_chunks
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();

    let mut summary = FangyuanLodIntegrationSummary {
        loaded_chunks: chunk_summary.loaded_chunks,
        loaded_chunk_ids: chunk_summary.loaded_chunk_ids.clone(),
        aoi_radius,
        pressure: pressure_summary_from_hotspot(hotspot),
        ..Default::default()
    };

    for descriptor in descriptors {
        let aoi_decision = decisions_by_id
            .get(descriptor.object_id.as_str())
            .copied()
            .cloned()
            .unwrap_or_else(|| hidden_decision_for_descriptor(descriptor));
        let mut render_decision = render_decision_from_aoi(descriptor, &aoi_decision, hotspot);

        if unload_chunks.contains(descriptor.chunk_id.as_str()) {
            render_decision.effective_lod = FangyuanLodLevel::L4HiddenRuleOnly;
            render_decision.path = FangyuanLodRenderPath::Hidden;
            render_decision.visible = false;
            render_decision.marker = false;
            render_decision.hidden = true;
            render_decision.degraded_by_pressure =
                render_decision.degraded_by_pressure || render_decision.degrade_reason != "-";
            render_decision.degrade_reason = "chunk_unloaded".to_string();
        }

        summary
            .lod_distribution
            .record(render_decision.effective_lod);
        summary.render_paths.record(render_decision.path);
        if render_decision.visible {
            summary.visible_objects += descriptor.primitive_count.max(1);
        }
        if render_decision.hidden {
            summary.hidden_objects += descriptor.primitive_count.max(1);
        }
        if render_decision.marker {
            summary.marker_objects += descriptor.primitive_count.max(1);
        }
        summary.decisions.push(render_decision);
    }

    for decision in &selection.object_decisions {
        if descriptor_by_id.contains_key(decision.object_id.as_str()) {
            continue;
        }
        let descriptor = FangyuanLodRenderDescriptor::new(
            decision.object_id.clone(),
            decision.chunk_id.clone(),
            decision.kind,
            FangyuanLodRenderPath::Standard,
            [0.0, 0.0, 0.0],
        );
        let render_decision = render_decision_from_aoi(&descriptor, decision, hotspot);
        summary
            .lod_distribution
            .record(render_decision.effective_lod);
        summary.render_paths.record(render_decision.path);
        if render_decision.visible {
            summary.visible_objects += 1;
        }
        if render_decision.hidden {
            summary.hidden_objects += 1;
        }
        if render_decision.marker {
            summary.marker_objects += 1;
        }
        summary.decisions.push(render_decision);
    }

    summary.decisions.sort_by(|left, right| {
        left.chunk_id
            .cmp(&right.chunk_id)
            .then_with(|| left.object_id.cmp(&right.object_id))
    });

    summary
}

pub fn summarize_fangyuan_lod_integration_from_descriptors(
    observer_position: [f32; 3],
    config: FangyuanAoiConfig,
    chunk_summary: &FangyuanChunkDebugSummary,
    descriptors: &[FangyuanLodRenderDescriptor],
    hotspot: &FangyuanHotspotEvaluation,
) -> FangyuanLodIntegrationSummary {
    let entries = manifest_entries_from_descriptors(descriptors);
    let aoi_objects = descriptors
        .iter()
        .map(FangyuanLodRenderDescriptor::aoi_descriptor)
        .collect::<Vec<_>>();
    let loaded_ids = if chunk_summary.loaded_chunk_ids.is_empty() {
        entries
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>()
    } else {
        chunk_summary
            .loaded_chunk_ids
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
    };
    let selection = select_fangyuan_aoi(
        observer_position,
        config,
        &entries,
        &aoi_objects,
        loaded_ids,
    );

    summarize_fangyuan_lod_integration(
        chunk_summary,
        config.marker_radius,
        &selection,
        descriptors,
        hotspot,
    )
}

pub fn integrate_fangyuan_lod_rendering<'a>(
    observer_position: [f32; 3],
    config: FangyuanAoiConfig,
    chunk_summary: &FangyuanChunkDebugSummary,
    entries: &[FangyuanChunkManifestEntry],
    descriptors: &[FangyuanLodRenderDescriptor],
    loaded_chunk_ids: impl IntoIterator<Item = &'a str>,
    hotspot: &FangyuanHotspotEvaluation,
) -> FangyuanLodIntegrationSummary {
    let aoi_objects = descriptors
        .iter()
        .map(FangyuanLodRenderDescriptor::aoi_descriptor)
        .collect::<Vec<_>>();
    let selection = select_fangyuan_aoi(
        observer_position,
        config,
        entries,
        &aoi_objects,
        loaded_chunk_ids,
    );

    summarize_fangyuan_lod_integration(
        chunk_summary,
        config.marker_radius,
        &selection,
        descriptors,
        hotspot,
    )
}

pub fn fangyuan_lod_descriptors_from_primitive_set(
    chunk_id: impl AsRef<str>,
    object_prefix: impl AsRef<str>,
    kind: FangyuanLodObjectKind,
    preferred_path: FangyuanLodRenderPath,
    primitive_set: &FangyuanPrimitiveSet,
) -> Vec<FangyuanLodRenderDescriptor> {
    primitive_set
        .primitives()
        .iter()
        .enumerate()
        .map(|(primitive_index, primitive)| {
            descriptor_from_primitive(
                format!("{}.{}", object_prefix.as_ref(), primitive_index),
                chunk_id.as_ref(),
                kind,
                preferred_path,
                primitive,
            )
        })
        .collect()
}

pub fn fangyuan_lod_descriptor_from_trial_visual(
    chunk_id: impl AsRef<str>,
    visual: &FangyuanObjectTrialVisualPrimitive,
) -> FangyuanLodRenderDescriptor {
    let (kind, preferred_path) = match visual.class {
        FangyuanObjectClass::Vfx | FangyuanObjectClass::Skill => {
            (FangyuanLodObjectKind::SkillVfx, FangyuanLodRenderPath::Vfx)
        }
        FangyuanObjectClass::Equipment => (
            FangyuanLodObjectKind::Equipment,
            FangyuanLodRenderPath::Equipment,
        ),
        FangyuanObjectClass::Npc => (FangyuanLodObjectKind::Npc, FangyuanLodRenderPath::Npc),
        FangyuanObjectClass::Tiandao => (
            FangyuanLodObjectKind::TiandaoObject,
            FangyuanLodRenderPath::Tiandao,
        ),
    };
    descriptor_from_primitive(
        format!("{}.{}", visual.object_id, visual.primitive_index),
        chunk_id.as_ref(),
        kind,
        preferred_path,
        &visual.primitive,
    )
}

#[derive(Clone, Debug, PartialEq)]
pub struct FangyuanPressureScenario {
    pub object_count: usize,
    pub skill_count: usize,
    pub npc_count: usize,
    pub chunk_count: usize,
    pub entries: Vec<FangyuanChunkManifestEntry>,
    pub descriptors: Vec<FangyuanLodRenderDescriptor>,
    pub metrics: FangyuanHotspotMetrics,
    pub hotspot: FangyuanHotspotEvaluation,
    pub summary: FangyuanLodIntegrationSummary,
}

pub fn generate_fangyuan_pressure_scenario(object_count: usize) -> FangyuanPressureScenario {
    let object_count = object_count.max(1);
    let chunk_count = object_count.div_ceil(50).max(1);
    let mut entries = Vec::with_capacity(chunk_count);
    for chunk_index in 0..chunk_count {
        let min_x = chunk_index as f32 * 24.0 - 12.0;
        entries.push(manifest_entry(
            format!("pressure_chunk_{chunk_index}"),
            [min_x, 0.0, -12.0],
            [min_x + 24.0, 8.0, 12.0],
            50,
        ));
    }

    let mut descriptors = Vec::with_capacity(object_count);
    for index in 0..object_count {
        let chunk_index = index % chunk_count;
        let chunk_id = format!("pressure_chunk_{chunk_index}");
        let local = (index / chunk_count) as f32;
        let x = chunk_index as f32 * 24.0 - 10.0 + local.rem_euclid(20.0);
        let z = (index % 17) as f32 - 8.0;
        let position = [x, 0.0, z];
        let descriptor = match index % 10 {
            0 => FangyuanLodRenderDescriptor::skill_layer(
                format!("skill_rule_{index}"),
                chunk_id,
                position,
            )
            .with_dynamic_primitive_count(3)
            .with_transparent_count(1),
            1 => FangyuanLodRenderDescriptor::vfx(format!("vfx_{index}"), chunk_id, position)
                .with_dynamic_primitive_count(4)
                .with_transparent_count(2)
                .with_emissive_total(3)
                .with_trail_count(1),
            2 => FangyuanLodRenderDescriptor::npc(format!("npc_{index}"), chunk_id, position)
                .with_dynamic_primitive_count(2)
                .with_transparent_count(1),
            3 => FangyuanLodRenderDescriptor::equipment(
                format!("equipment_{index}"),
                chunk_id,
                position,
            )
            .with_dynamic_primitive_count(2),
            4 => {
                FangyuanLodRenderDescriptor::tiandao(format!("tiandao_{index}"), chunk_id, position)
                    .with_dynamic_primitive_count(1)
                    .with_emissive_total(2)
            }
            5 | 6 => FangyuanLodRenderDescriptor::static_merge(
                format!("static_merge_{index}"),
                chunk_id,
                position,
            ),
            7 | 8 => FangyuanLodRenderDescriptor::static_instancing(
                format!("static_instance_{index}"),
                chunk_id,
                position,
            ),
            _ => FangyuanLodRenderDescriptor::standard_static(
                format!("static_standard_{index}"),
                chunk_id,
                position,
            ),
        };
        descriptors.push(descriptor);
    }

    let metrics = hotspot_metrics_from_descriptors(&descriptors, chunk_count);
    let hotspot = evaluate_fangyuan_hotspot(
        metrics,
        FangyuanHotspotThresholds::default(),
        FangyuanHotspotState::default(),
    );
    let loaded_chunk_ids = entries
        .iter()
        .take(chunk_count.min(4))
        .map(|entry| entry.id.clone())
        .collect::<Vec<_>>();
    let chunk_summary = FangyuanChunkDebugSummary {
        loaded_chunks: loaded_chunk_ids.len(),
        loaded_chunk_ids: loaded_chunk_ids.clone(),
        visible_objects: object_count,
        load_state: "loaded".to_string(),
        failure_reason: "-".to_string(),
    };
    let config = FangyuanAoiConfig {
        load_radius: 64.0,
        keep_radius: 80.0,
        marker_radius: 112.0,
        near_radius: 24.0,
        mid_radius: 56.0,
        far_radius: 88.0,
    };
    let summary = integrate_fangyuan_lod_rendering(
        [0.0, 0.0, 0.0],
        config,
        &chunk_summary,
        &entries,
        &descriptors,
        loaded_chunk_ids.iter().map(String::as_str),
        &hotspot,
    );
    let skill_count = descriptors
        .iter()
        .filter(|descriptor| descriptor.kind == FangyuanLodObjectKind::SkillVfx)
        .count();
    let npc_count = descriptors
        .iter()
        .filter(|descriptor| descriptor.kind == FangyuanLodObjectKind::Npc)
        .count();

    FangyuanPressureScenario {
        object_count,
        skill_count,
        npc_count,
        chunk_count,
        entries,
        descriptors,
        metrics,
        hotspot,
        summary,
    }
}

pub fn hotspot_metrics_from_descriptors(
    descriptors: &[FangyuanLodRenderDescriptor],
    chunk_load_pressure: usize,
) -> FangyuanHotspotMetrics {
    FangyuanHotspotMetrics {
        active_skill_count: descriptors
            .iter()
            .filter(|descriptor| descriptor.kind == FangyuanLodObjectKind::SkillVfx)
            .count() as u32,
        dynamic_primitive_count: descriptors
            .iter()
            .map(|descriptor| descriptor.dynamic_primitive_count)
            .sum::<usize>() as u32,
        transparent_count: descriptors
            .iter()
            .map(|descriptor| descriptor.transparent_count)
            .sum::<usize>() as u32,
        emissive_total: descriptors
            .iter()
            .map(|descriptor| descriptor.emissive_total)
            .sum(),
        trail_count: descriptors
            .iter()
            .map(|descriptor| descriptor.trail_count)
            .sum::<usize>() as u32,
        chunk_load_pressure: chunk_load_pressure as u32,
    }
}

fn render_decision_from_aoi(
    descriptor: &FangyuanLodRenderDescriptor,
    aoi_decision: &FangyuanAoiObjectDecision,
    hotspot: &FangyuanHotspotEvaluation,
) -> FangyuanLodRenderDecision {
    let (effective_lod, degrade_reason) =
        effective_lod_for_descriptor(descriptor, aoi_decision.lod, hotspot);
    let path = render_path_for_descriptor(descriptor, effective_lod, aoi_decision);
    let hidden = path == FangyuanLodRenderPath::Hidden;
    let marker = path == FangyuanLodRenderPath::Marker;
    let visible = path.keeps_payload();

    FangyuanLodRenderDecision {
        object_id: descriptor.object_id.clone(),
        chunk_id: descriptor.chunk_id.clone(),
        kind: descriptor.kind,
        band: aoi_decision.band,
        requested_lod: aoi_decision.lod,
        effective_lod,
        path,
        visible,
        marker,
        hidden,
        degraded_by_pressure: degrade_reason != "-",
        degrade_reason,
    }
}

fn effective_lod_for_descriptor(
    descriptor: &FangyuanLodRenderDescriptor,
    lod: FangyuanLodLevel,
    hotspot: &FangyuanHotspotEvaluation,
) -> (FangyuanLodLevel, String) {
    if descriptor.recycled {
        return (
            FangyuanLodLevel::L4HiddenRuleOnly,
            "tiandao_recycled".to_string(),
        );
    }
    if !hotspot.active {
        return (lod, "-".to_string());
    }

    let pressure = hotspot
        .pressure_reasons
        .first()
        .map(|reason| reason.kind.as_str())
        .unwrap_or("hotspot");
    let degraded = match (hotspot.severity, descriptor.kind, lod) {
        (FangyuanHotspotSeverity::Normal, _, _) => lod,
        (
            FangyuanHotspotSeverity::Warm,
            FangyuanLodObjectKind::SkillVfx
            | FangyuanLodObjectKind::Equipment
            | FangyuanLodObjectKind::Npc
            | FangyuanLodObjectKind::TiandaoObject,
            FangyuanLodLevel::L0Full,
        ) => FangyuanLodLevel::L1Reduced,
        (
            FangyuanHotspotSeverity::Hot,
            FangyuanLodObjectKind::SkillVfx,
            FangyuanLodLevel::L0Full | FangyuanLodLevel::L1Reduced | FangyuanLodLevel::L2Silhouette,
        ) => FangyuanLodLevel::L3Marker,
        (
            FangyuanHotspotSeverity::Hot,
            FangyuanLodObjectKind::Npc,
            FangyuanLodLevel::L0Full | FangyuanLodLevel::L1Reduced,
        ) => FangyuanLodLevel::L2Silhouette,
        (
            FangyuanHotspotSeverity::Hot,
            FangyuanLodObjectKind::TiandaoObject,
            FangyuanLodLevel::L0Full | FangyuanLodLevel::L1Reduced,
        ) => FangyuanLodLevel::L2Silhouette,
        (
            FangyuanHotspotSeverity::Critical,
            FangyuanLodObjectKind::SkillVfx,
            FangyuanLodLevel::L0Full
            | FangyuanLodLevel::L1Reduced
            | FangyuanLodLevel::L2Silhouette
            | FangyuanLodLevel::L3Marker,
        ) => FangyuanLodLevel::L4HiddenRuleOnly,
        (
            FangyuanHotspotSeverity::Critical,
            FangyuanLodObjectKind::Npc,
            FangyuanLodLevel::L0Full | FangyuanLodLevel::L1Reduced | FangyuanLodLevel::L2Silhouette,
        ) => FangyuanLodLevel::L3Marker,
        (
            FangyuanHotspotSeverity::Critical,
            FangyuanLodObjectKind::TiandaoObject,
            FangyuanLodLevel::L0Full
            | FangyuanLodLevel::L1Reduced
            | FangyuanLodLevel::L2Silhouette
            | FangyuanLodLevel::L3Marker,
        ) => FangyuanLodLevel::L4HiddenRuleOnly,
        _ => lod,
    };

    if degraded == lod {
        (lod, "-".to_string())
    } else {
        (degraded, pressure.to_string())
    }
}

fn render_path_for_descriptor(
    descriptor: &FangyuanLodRenderDescriptor,
    lod: FangyuanLodLevel,
    aoi_decision: &FangyuanAoiObjectDecision,
) -> FangyuanLodRenderPath {
    match lod {
        FangyuanLodLevel::L0Full | FangyuanLodLevel::L1Reduced => descriptor.preferred_path,
        FangyuanLodLevel::L2Silhouette => match descriptor.preferred_path {
            FangyuanLodRenderPath::StaticMerge => FangyuanLodRenderPath::StaticInstancing,
            FangyuanLodRenderPath::StaticInstancing => FangyuanLodRenderPath::StaticInstancing,
            FangyuanLodRenderPath::Standard => FangyuanLodRenderPath::Standard,
            FangyuanLodRenderPath::Vfx | FangyuanLodRenderPath::SkillLayer => {
                FangyuanLodRenderPath::SkillLayer
            }
            FangyuanLodRenderPath::Equipment => FangyuanLodRenderPath::Equipment,
            FangyuanLodRenderPath::Npc => FangyuanLodRenderPath::Npc,
            FangyuanLodRenderPath::Tiandao => FangyuanLodRenderPath::Tiandao,
            FangyuanLodRenderPath::Marker => FangyuanLodRenderPath::Marker,
            FangyuanLodRenderPath::Hidden => FangyuanLodRenderPath::Hidden,
        },
        FangyuanLodLevel::L3Marker => FangyuanLodRenderPath::Marker,
        FangyuanLodLevel::L4HiddenRuleOnly => {
            if aoi_decision.rule_layer_retained
                && matches!(
                    descriptor.preferred_path,
                    FangyuanLodRenderPath::SkillLayer | FangyuanLodRenderPath::Vfx
                )
            {
                FangyuanLodRenderPath::SkillLayer
            } else {
                FangyuanLodRenderPath::Hidden
            }
        }
    }
}

fn pressure_summary_from_hotspot(
    hotspot: &FangyuanHotspotEvaluation,
) -> FangyuanLodPressureSummary {
    let bottleneck = hotspot
        .pressure_reasons
        .first()
        .map(|reason| reason.kind.as_str().to_string())
        .unwrap_or_else(|| "-".to_string());
    let degrade_reason = hotspot
        .degrade_plan
        .first()
        .map(|step| {
            if bottleneck == "-" {
                step.target.as_str().to_string()
            } else {
                format!("{}:{}", bottleneck, step.target.as_str())
            }
        })
        .unwrap_or_else(|| "-".to_string());

    FangyuanLodPressureSummary {
        active: hotspot.active,
        severity: hotspot.severity,
        pressure_label: hotspot_severity_label(hotspot.severity).to_string(),
        bottleneck,
        degrade_reason,
    }
}

fn hidden_decision_for_descriptor(
    descriptor: &FangyuanLodRenderDescriptor,
) -> FangyuanAoiObjectDecision {
    FangyuanAoiObjectDecision {
        object_id: descriptor.object_id.clone(),
        chunk_id: descriptor.chunk_id.clone(),
        kind: descriptor.kind,
        band: FangyuanAoiBand::Outside,
        lod: FangyuanLodLevel::L4HiddenRuleOnly,
        visible: false,
        marker: false,
        rule_layer_retained: descriptor.priority.should_keep_rule_visible()
            || descriptor.kind == FangyuanLodObjectKind::SkillVfx,
    }
}

fn descriptor_from_primitive(
    object_id: String,
    chunk_id: &str,
    kind: FangyuanLodObjectKind,
    preferred_path: FangyuanLodRenderPath,
    primitive: &FangyuanPrimitive,
) -> FangyuanLodRenderDescriptor {
    FangyuanLodRenderDescriptor::new(
        object_id,
        chunk_id.to_string(),
        kind,
        preferred_path,
        primitive.local_position().to_array(),
    )
    .with_dynamic_primitive_count(usize::from(!matches!(
        kind,
        FangyuanLodObjectKind::StaticObject | FangyuanLodObjectKind::HomeDecoration
    )))
    .with_transparent_count(usize::from(primitive.alpha() < 0.999))
    .with_emissive_total(primitive.emissive().ceil().max(0.0) as u32)
    .with_trail_count(usize::from(
        primitive.role() == FangyuanPrimitiveRole::Trail,
    ))
}

fn manifest_entries_from_descriptors(
    descriptors: &[FangyuanLodRenderDescriptor],
) -> Vec<FangyuanChunkManifestEntry> {
    let mut bounds_by_chunk = BTreeMap::<String, ([f32; 3], [f32; 3], usize)>::new();
    for descriptor in descriptors {
        let entry = bounds_by_chunk
            .entry(descriptor.chunk_id.clone())
            .or_insert((descriptor.position, descriptor.position, 0));
        for axis in 0..3 {
            entry.0[axis] = entry.0[axis].min(descriptor.position[axis] - 1.0);
            entry.1[axis] = entry.1[axis].max(descriptor.position[axis] + 1.0);
        }
        entry.2 += 1;
    }

    bounds_by_chunk
        .into_iter()
        .map(|(chunk_id, (mut min, mut max, count))| {
            for axis in 0..3 {
                if min[axis] >= max[axis] {
                    min[axis] -= 1.0;
                    max[axis] += 1.0;
                }
            }
            manifest_entry(chunk_id, min, max, count)
        })
        .collect()
}

fn manifest_entry(
    id: impl Into<String>,
    min: [f32; 3],
    max: [f32; 3],
    object_count: usize,
) -> FangyuanChunkManifestEntry {
    FangyuanChunkManifestEntry {
        id: id.into(),
        bounds: FangyuanChunkBounds::new(min, max),
        region: FangyuanChunkRegionMetadata {
            region_id: "fangyuan.pressure".to_string(),
            layer: "runtime".to_string(),
            tags: Vec::new(),
        },
        dev_ron: None,
        bin: None,
        hash: None,
        data_version: None,
        budget: FangyuanChunkBudgetSummary {
            prefab_instance_count: object_count,
            tiandao_ref_count: 0,
            static_decoration_count: 0,
            total_ref_count: object_count,
            prefab_cost: object_count as u32,
            tiandao_cost: 0,
            static_decoration_cost: 0,
            total_cost: object_count as u32,
        },
    }
}

fn hotspot_severity_label(severity: FangyuanHotspotSeverity) -> &'static str {
    match severity {
        FangyuanHotspotSeverity::Normal => "normal",
        FangyuanHotspotSeverity::Warm => "warm",
        FangyuanHotspotSeverity::Hot => "hot",
        FangyuanHotspotSeverity::Critical => "critical",
    }
}

fn pressure_rank(severity: FangyuanHotspotSeverity) -> u8 {
    match severity {
        FangyuanHotspotSeverity::Normal => 0,
        FangyuanHotspotSeverity::Warm => 1,
        FangyuanHotspotSeverity::Hot => 2,
        FangyuanHotspotSeverity::Critical => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fangyuan_chunk_lod_integration_switches_paths_without_residuals() {
        let entry = manifest_entry("chunk_a", [-8.0, 0.0, -8.0], [8.0, 8.0, 8.0], 1);
        let descriptor =
            FangyuanLodRenderDescriptor::static_merge("tree_a", "chunk_a", [0.0, 0.0, 0.0]);
        let chunk_summary = loaded_chunk_summary(["chunk_a"], 1);
        let hotspot = no_hotspot();
        let config = FangyuanAoiConfig::default();

        let near = integrate_fangyuan_lod_rendering(
            [0.0, 0.0, 0.0],
            config,
            &chunk_summary,
            std::slice::from_ref(&entry),
            std::slice::from_ref(&descriptor),
            ["chunk_a"],
            &hotspot,
        );
        assert_eq!(near.decisions[0].path, FangyuanLodRenderPath::StaticMerge);
        let (near_state, near_cleanup) =
            reconcile_fangyuan_lod_render_state(&Default::default(), &near);
        assert!(near_cleanup.removed_object_ids.is_empty());

        let far = integrate_fangyuan_lod_rendering(
            [36.0, 0.0, 0.0],
            config,
            &chunk_summary,
            &[entry],
            &[descriptor],
            ["chunk_a"],
            &hotspot,
        );
        assert_eq!(
            far.decisions[0].path,
            FangyuanLodRenderPath::StaticInstancing
        );
        let (far_state, far_cleanup) = reconcile_fangyuan_lod_render_state(&near_state, &far);
        assert_eq!(far_cleanup.replaced_paths.len(), 1);
        assert_eq!(
            far_state.active_paths.get("tree_a"),
            Some(&FangyuanLodRenderPath::StaticInstancing)
        );
        assert!(!far_state.hidden_ids.contains("tree_a"));
    }

    #[test]
    fn fangyuan_chunk_lod_integration_unload_cleans_hidden_paths() {
        let descriptor =
            FangyuanLodRenderDescriptor::static_instancing("rock_a", "chunk_old", [0.0, 0.0, 0.0]);
        let chunk_summary = loaded_chunk_summary(["chunk_old"], 1);
        let selection = FangyuanAoiSelection {
            unload_chunks: vec!["chunk_old".to_string()],
            object_decisions: vec![FangyuanAoiObjectDecision {
                object_id: "rock_a".to_string(),
                chunk_id: "chunk_old".to_string(),
                kind: FangyuanLodObjectKind::StaticObject,
                band: FangyuanAoiBand::Outside,
                lod: FangyuanLodLevel::L4HiddenRuleOnly,
                visible: false,
                marker: false,
                rule_layer_retained: false,
            }],
            ..Default::default()
        };
        let summary = summarize_fangyuan_lod_integration(
            &chunk_summary,
            FangyuanAoiConfig::default().marker_radius,
            &selection,
            &[descriptor],
            &no_hotspot(),
        );
        let previous = FangyuanLodRenderSceneState {
            active_paths: BTreeMap::from([(
                "rock_a".to_string(),
                FangyuanLodRenderPath::StaticInstancing,
            )]),
            ..Default::default()
        };

        let (next, cleanup) = reconcile_fangyuan_lod_render_state(&previous, &summary);

        assert_eq!(summary.decisions[0].path, FangyuanLodRenderPath::Hidden);
        assert_eq!(cleanup.removed_object_ids, vec!["rock_a"]);
        assert!(!next.active_paths.contains_key("rock_a"));
        assert!(next.hidden_ids.contains("rock_a"));
    }

    #[test]
    fn fangyuan_chunk_lod_integration_degrades_vfx_to_rule_layer_under_pressure() {
        let descriptor = FangyuanLodRenderDescriptor::vfx("vfx_burst", "chunk_a", [0.0, 0.0, 0.0])
            .with_dynamic_primitive_count(80)
            .with_transparent_count(20)
            .with_trail_count(12);
        let summary = summarize_fangyuan_lod_integration_from_descriptors(
            [0.0, 0.0, 0.0],
            FangyuanAoiConfig::default(),
            &loaded_chunk_summary(["chunk_a"], 1),
            &[descriptor],
            &critical_hotspot(),
        );

        let decision = &summary.decisions[0];
        assert_eq!(decision.effective_lod, FangyuanLodLevel::L4HiddenRuleOnly);
        assert_eq!(decision.path, FangyuanLodRenderPath::SkillLayer);
        assert!(decision.degraded_by_pressure);
        assert_eq!(summary.pressure.bottleneck, "active_skill");
    }

    #[test]
    fn fangyuan_chunk_lod_integration_degrades_npc_to_marker_under_pressure() {
        let descriptor = FangyuanLodRenderDescriptor::npc("npc_guard", "chunk_a", [0.0, 0.0, 0.0])
            .with_dynamic_primitive_count(8);
        let summary = summarize_fangyuan_lod_integration_from_descriptors(
            [0.0, 0.0, 0.0],
            FangyuanAoiConfig::default(),
            &loaded_chunk_summary(["chunk_a"], 1),
            &[descriptor],
            &critical_hotspot(),
        );

        let decision = &summary.decisions[0];
        assert_eq!(decision.effective_lod, FangyuanLodLevel::L3Marker);
        assert_eq!(decision.path, FangyuanLodRenderPath::Marker);
        assert!(summary.marker_objects > 0);
    }

    #[test]
    fn fangyuan_chunk_lod_integration_hides_recycled_tiandao() {
        let descriptor =
            FangyuanLodRenderDescriptor::tiandao("tiandao.local_wind", "chunk_a", [0.0, 0.0, 0.0])
                .with_recycled(true);
        let summary = summarize_fangyuan_lod_integration_from_descriptors(
            [0.0, 0.0, 0.0],
            FangyuanAoiConfig::default(),
            &loaded_chunk_summary(["chunk_a"], 1),
            &[descriptor],
            &no_hotspot(),
        );

        let decision = &summary.decisions[0];
        assert_eq!(decision.path, FangyuanLodRenderPath::Hidden);
        assert_eq!(decision.degrade_reason, "tiandao_recycled");
        assert_eq!(summary.hidden_objects, 1);
        assert_eq!(summary.render_paths.tiandao, 0);
    }

    #[test]
    fn fangyuan_pressure_scenario_reports_100_300_1000_bottlenecks() {
        for count in [100, 300, 1000] {
            let scenario = generate_fangyuan_pressure_scenario(count);
            println!(
                "fangyuan pressure scenario count={} chunks={} pressure={} bottleneck={} degrade={} lod={} paths={}",
                scenario.object_count,
                scenario.chunk_count,
                scenario.summary.pressure.pressure_label,
                scenario.summary.pressure.bottleneck,
                scenario.summary.pressure.degrade_reason,
                scenario.summary.lod_distribution_label(),
                scenario.summary.render_path_label(),
            );

            assert_eq!(scenario.object_count, count);
            assert!(scenario.hotspot.active);
            assert_ne!(scenario.summary.pressure.bottleneck, "-");
            assert_ne!(scenario.summary.pressure.degrade_reason, "-");
            assert_eq!(scenario.descriptors.len(), count);
            assert!(scenario.summary.lod_distribution.total() > 0);
        }
    }

    fn no_hotspot() -> FangyuanHotspotEvaluation {
        evaluate_fangyuan_hotspot(
            FangyuanHotspotMetrics::empty(),
            FangyuanHotspotThresholds::default(),
            FangyuanHotspotState::default(),
        )
    }

    fn critical_hotspot() -> FangyuanHotspotEvaluation {
        evaluate_fangyuan_hotspot(
            FangyuanHotspotMetrics {
                active_skill_count: 40,
                dynamic_primitive_count: 500,
                transparent_count: 120,
                emissive_total: 180,
                trail_count: 64,
                chunk_load_pressure: 20,
            },
            FangyuanHotspotThresholds::default(),
            FangyuanHotspotState::default(),
        )
    }

    fn loaded_chunk_summary<const N: usize>(
        chunk_ids: [&str; N],
        visible_objects: usize,
    ) -> FangyuanChunkDebugSummary {
        FangyuanChunkDebugSummary {
            loaded_chunks: chunk_ids.len(),
            loaded_chunk_ids: chunk_ids.into_iter().map(str::to_string).collect(),
            visible_objects,
            load_state: "loaded".to_string(),
            failure_reason: "-".to_string(),
        }
    }
}
