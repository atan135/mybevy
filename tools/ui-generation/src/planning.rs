use crate::analysis::{
    AlignmentTarget, AnalysisElement, AnchorEdge, Axis, ComponentCandidateKind, LayoutBehaviorKind,
    UiReferenceAnalysis, VisualElementKind,
};
use project::framework::ui::document::tooling::{
    BUILT_IN_TOKENS, BUILT_IN_WIDGET_VARIANTS, UiToolingTokenKind, UiToolingTokenValue,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

pub const PLANNING_PROTOCOL_VERSION: u32 = 1;
pub const MAX_PLAN_TOKENS: usize = 4096;
pub const MAX_PLAN_COMPONENTS: usize = 512;
pub const MAX_PLAN_CONSTRAINTS: usize = 12_800;
pub const MAX_PLAN_STEPS: usize = 2048;
pub const MAX_PLAN_DIAGNOSTICS: usize = 2048;
const CLUSTER_TOLERANCE_PX: f64 = 1.0;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateTokenKind {
    Color,
    FontSize,
    Spacing,
    Radius,
    BorderWidth,
    Shadow,
    RepeatedSize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CandidateTokenValue {
    Scalar {
        px: f64,
    },
    Srgba {
        value: [f64; 4],
    },
    Shadow {
        x: f64,
        y: f64,
        blur: f64,
        alpha: f64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendationScope {
    ExistingGlobal,
    Page,
    Component,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenOrigin {
    ObservedGeometry,
    ExistingCatalogSuggestion,
    HeuristicAssumption,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CandidateToken {
    pub token_id: String,
    pub kind: CandidateTokenKind,
    pub value: CandidateTokenValue,
    pub origin: TokenOrigin,
    pub source_element_ids: Vec<String>,
    pub matched_existing_token: Option<String>,
    pub scope: RecommendationScope,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ComponentInstancePlan {
    pub component_id: String,
    pub component: String,
    pub variant: Option<String>,
    pub pattern_id: Option<String>,
    pub source_element_ids: Vec<String>,
    pub scope: RecommendationScope,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstraintKind {
    Parent,
    Width,
    Height,
    Anchor,
    Align,
    Gap,
    Flex,
    Scroll,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct LayoutConstraint {
    pub constraint_id: String,
    pub kind: ConstraintKind,
    pub subject_id: String,
    pub target_id: Option<String>,
    pub axis: Option<Axis>,
    pub value: Option<f64>,
    pub relation: String,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanPhase {
    Structure,
    Visual,
    Decoration,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PlanStep {
    pub step_id: String,
    pub phase: PlanPhase,
    pub subject_id: String,
    pub action: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PlanningDiagnostic {
    pub code: String,
    pub subject_id: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct UiGenerationPlan {
    pub protocol_version: u32,
    pub analysis_id: String,
    pub tokens: Vec<CandidateToken>,
    pub components: Vec<ComponentInstancePlan>,
    pub constraints: Vec<LayoutConstraint>,
    pub steps: Vec<PlanStep>,
    pub diagnostics: Vec<PlanningDiagnostic>,
}

pub fn plan_analysis(analysis: &UiReferenceAnalysis) -> UiGenerationPlan {
    let mut elements: Vec<_> = analysis.elements.iter().collect();
    elements.sort_by_key(|element| &element.element_id);
    let by_id: BTreeMap<_, _> = elements
        .iter()
        .map(|element| (element.element_id.as_str(), *element))
        .collect();

    UiGenerationPlan {
        protocol_version: PLANNING_PROTOCOL_VERSION,
        analysis_id: analysis.analysis_id.clone(),
        tokens: derive_tokens(&elements, &by_id),
        components: derive_components(&elements),
        constraints: derive_constraints(&elements, &by_id),
        steps: derive_steps(&elements),
        diagnostics: diagnose(&elements, &by_id),
    }
}

fn derive_tokens(
    elements: &[&AnalysisElement],
    by_id: &BTreeMap<&str, &AnalysisElement>,
) -> Vec<CandidateToken> {
    let mut samples: BTreeMap<CandidateTokenKind, (TokenOrigin, Vec<(f64, String)>)> =
        BTreeMap::new();
    for element in elements {
        samples
            .entry(CandidateTokenKind::RepeatedSize)
            .or_insert_with(|| (TokenOrigin::ObservedGeometry, Vec::new()))
            .1
            .extend([
                (element.bounding_box.width, element.element_id.clone()),
                (element.bounding_box.height, element.element_id.clone()),
            ]);
        if element.kind == VisualElementKind::Text {
            samples
                .entry(CandidateTokenKind::FontSize)
                .or_insert_with(|| (TokenOrigin::HeuristicAssumption, Vec::new()))
                .1
                .push((
                    (element.bounding_box.height * 0.42).round(),
                    element.element_id.clone(),
                ));
        }
        if matches!(
            element.kind,
            VisualElementKind::Surface
                | VisualElementKind::Border
                | VisualElementKind::NineSliceCandidate
        ) {
            samples
                .entry(CandidateTokenKind::Radius)
                .or_insert_with(|| (TokenOrigin::ExistingCatalogSuggestion, Vec::new()))
                .1
                .push((8.0, element.element_id.clone()));
            samples
                .entry(CandidateTokenKind::BorderWidth)
                .or_insert_with(|| (TokenOrigin::ExistingCatalogSuggestion, Vec::new()))
                .1
                .push((1.0, element.element_id.clone()));
        }
        if let Some(parent) = element.parent_id.as_deref().and_then(|id| by_id.get(id)) {
            samples
                .entry(CandidateTokenKind::Spacing)
                .or_insert_with(|| (TokenOrigin::ObservedGeometry, Vec::new()))
                .1
                .extend([
                    (
                        (element.bounding_box.x - parent.bounding_box.x).max(0.0),
                        element.element_id.clone(),
                    ),
                    (
                        (element.bounding_box.y - parent.bounding_box.y).max(0.0),
                        element.element_id.clone(),
                    ),
                ]);
        }
    }

    let mut output = Vec::new();
    for (kind, (origin, mut values)) in samples {
        values.retain(|(value, _)| value.is_finite() && *value > 0.0);
        values.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        let mut clusters: Vec<(Vec<f64>, BTreeSet<String>)> = Vec::new();
        for (value, source) in values {
            let append = clusters.last().is_some_and(|(numbers, _)| {
                let mean = numbers.iter().sum::<f64>() / numbers.len() as f64;
                (value - mean).abs() <= CLUSTER_TOLERANCE_PX
            });
            if append {
                let (numbers, sources) = clusters.last_mut().unwrap();
                numbers.push(value);
                sources.insert(source);
            } else {
                clusters.push((vec![value], BTreeSet::from([source])));
            }
        }
        for (index, (numbers, sources)) in clusters.into_iter().enumerate() {
            let value = round_quarter(numbers.iter().sum::<f64>() / numbers.len() as f64);
            let matched_existing_token = match_scalar_token(kind, value);
            output.push(CandidateToken {
                token_id: format!("candidate.{}.{index:03}", token_kind_name(kind)),
                kind,
                value: CandidateTokenValue::Scalar { px: value },
                origin,
                source_element_ids: sources.into_iter().collect(),
                scope: if matched_existing_token.is_some() {
                    RecommendationScope::ExistingGlobal
                } else {
                    RecommendationScope::Page
                },
                matched_existing_token,
            });
        }
    }

    add_semantic_visual_tokens(elements, &mut output);
    output.sort_by(|a, b| {
        a.kind
            .cmp(&b.kind)
            .then_with(|| a.token_id.cmp(&b.token_id))
    });
    debug_assert!(output.len() <= MAX_PLAN_TOKENS);
    output
}

fn add_semantic_visual_tokens(elements: &[&AnalysisElement], output: &mut Vec<CandidateToken>) {
    let groups = [
        (
            VisualElementKind::Background,
            "screen_background",
            [0.05, 0.08, 0.11, 1.0],
        ),
        (
            VisualElementKind::Surface,
            "panel_background",
            [0.10, 0.13, 0.16, 0.94],
        ),
        (
            VisualElementKind::Text,
            "text_primary",
            [0.92, 0.95, 0.95, 1.0],
        ),
    ];
    for (kind, name, value) in groups {
        let sources: Vec<_> = elements
            .iter()
            .filter(|element| element.kind == kind)
            .map(|element| element.element_id.clone())
            .collect();
        if !sources.is_empty() {
            output.push(CandidateToken {
                token_id: format!("candidate.color.{name}"),
                kind: CandidateTokenKind::Color,
                value: CandidateTokenValue::Srgba { value },
                origin: TokenOrigin::ExistingCatalogSuggestion,
                source_element_ids: sources,
                matched_existing_token: match_color_token(value),
                scope: RecommendationScope::ExistingGlobal,
            });
        }
    }
    let surfaces: Vec<_> = elements
        .iter()
        .filter(|element| element.kind == VisualElementKind::Surface)
        .map(|element| element.element_id.clone())
        .collect();
    if !surfaces.is_empty() {
        output.push(CandidateToken {
            token_id: "candidate.shadow.surface".into(),
            kind: CandidateTokenKind::Shadow,
            value: CandidateTokenValue::Shadow {
                x: 0.0,
                y: 4.0,
                blur: 12.0,
                alpha: 0.24,
            },
            origin: TokenOrigin::HeuristicAssumption,
            source_element_ids: surfaces,
            matched_existing_token: None,
            scope: RecommendationScope::Page,
        });
    }
}

fn match_scalar_token(kind: CandidateTokenKind, value: f64) -> Option<String> {
    let expected = match kind {
        CandidateTokenKind::FontSize => UiToolingTokenKind::FontSize,
        CandidateTokenKind::Spacing => UiToolingTokenKind::Spacing,
        CandidateTokenKind::Radius => UiToolingTokenKind::Radius,
        CandidateTokenKind::BorderWidth => UiToolingTokenKind::BorderWidth,
        CandidateTokenKind::RepeatedSize => UiToolingTokenKind::RepeatedSize,
        _ => return None,
    };
    BUILT_IN_TOKENS
        .iter()
        .filter_map(|token| match token.value {
            UiToolingTokenValue::Scalar(candidate)
                if token.kind == expected && (f64::from(candidate) - value).abs() <= 1.0 =>
            {
                Some((token.name, (f64::from(candidate) - value).abs()))
            }
            _ => None,
        })
        .min_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.cmp(b.0)))
        .map(|(name, _)| name.to_owned())
}

fn match_color_token(value: [f64; 4]) -> Option<String> {
    BUILT_IN_TOKENS.iter().find_map(|token| match token.value {
        UiToolingTokenValue::Srgba(candidate)
            if candidate
                .iter()
                .zip(value)
                .all(|(left, right)| (f64::from(*left) - right).abs() <= 0.001) =>
        {
            Some(token.name.to_owned())
        }
        _ => None,
    })
}

fn derive_components(elements: &[&AnalysisElement]) -> Vec<ComponentInstancePlan> {
    let mut groups: BTreeMap<String, Vec<&AnalysisElement>> = BTreeMap::new();
    for element in elements {
        let key = element
            .repeated_pattern
            .as_ref()
            .map(|pattern| pattern.pattern_id.clone())
            .unwrap_or_else(|| element.element_id.clone());
        groups.entry(key).or_default().push(element);
    }
    let output = groups
        .into_iter()
        .map(|(key, group)| {
            let best = group
                .iter()
                .flat_map(|element| element.component_candidates.iter())
                .filter(|candidate| candidate.kind != ComponentCandidateKind::Unknown)
                .max_by(|a, b| {
                    a.confidence
                        .total_cmp(&b.confidence)
                        .then_with(|| component_name(b.kind).cmp(component_name(a.kind)))
                });
            let component = best
                .map(|candidate| component_name(candidate.kind))
                .unwrap_or("container")
                .to_owned();
            let variant = BUILT_IN_WIDGET_VARIANTS
                .iter()
                .filter(|candidate| candidate.component == component)
                .map(|candidate| candidate.variant)
                .min()
                .map(str::to_owned);
            let pattern_id = group[0]
                .repeated_pattern
                .as_ref()
                .map(|pattern| pattern.pattern_id.clone());
            let mut source_element_ids: Vec<_> = group
                .iter()
                .map(|element| element.element_id.clone())
                .collect();
            source_element_ids.sort();
            ComponentInstancePlan {
                component_id: format!("component.{}", encode_id(&key)),
                component,
                variant: variant.clone(),
                pattern_id,
                source_element_ids,
                scope: if best.is_some() && variant.is_some() {
                    RecommendationScope::ExistingGlobal
                } else if group.len() > 1 {
                    RecommendationScope::Component
                } else {
                    RecommendationScope::Page
                },
            }
        })
        .collect::<Vec<_>>();
    debug_assert!(output.len() <= MAX_PLAN_COMPONENTS);
    output
}

fn derive_constraints(
    elements: &[&AnalysisElement],
    by_id: &BTreeMap<&str, &AnalysisElement>,
) -> Vec<LayoutConstraint> {
    let mut output = Vec::new();
    for element in elements {
        if let Some(parent_id) = &element.parent_id {
            push_constraint(
                &mut output,
                ConstraintKind::Parent,
                element,
                Some(parent_id.clone()),
                None,
                None,
                "child_of",
            );
        }
        push_constraint(
            &mut output,
            ConstraintKind::Width,
            element,
            None,
            Some(Axis::Horizontal),
            Some(element.bounding_box.width),
            "observed_size",
        );
        push_constraint(
            &mut output,
            ConstraintKind::Height,
            element,
            None,
            Some(Axis::Vertical),
            Some(element.bounding_box.height),
            "observed_size",
        );
        for anchor in &element.layout.anchors {
            push_constraint(
                &mut output,
                ConstraintKind::Anchor,
                element,
                element.parent_id.clone(),
                None,
                None,
                &format!("{anchor:?}").to_ascii_lowercase(),
            );
        }
        for clue in &element.alignment_clues {
            let target = match &clue.target {
                AlignmentTarget::Canvas { .. } => Some("canvas".into()),
                AlignmentTarget::Region { region_id, .. } => Some(region_id.clone()),
                AlignmentTarget::Element { element_id, .. } => Some(element_id.clone()),
            };
            push_constraint(
                &mut output,
                ConstraintKind::Align,
                element,
                target,
                Some(clue.axis),
                Some(clue.offset),
                &format!("{:?}", clue.relation).to_ascii_lowercase(),
            );
        }
        match element.layout.kind {
            LayoutBehaviorKind::ContentFlow | LayoutBehaviorKind::ProportionalStretch => {
                push_constraint(
                    &mut output,
                    ConstraintKind::Flex,
                    element,
                    element.parent_id.clone(),
                    element.layout.flow_axis,
                    None,
                    "flexible",
                )
            }
            LayoutBehaviorKind::Scrollable => push_constraint(
                &mut output,
                ConstraintKind::Scroll,
                element,
                None,
                element.layout.flow_axis,
                None,
                "scrollable",
            ),
            _ => {}
        }
        if let Some(parent) = element.parent_id.as_deref().and_then(|id| by_id.get(id)) {
            let axis = element.layout.flow_axis.unwrap_or(Axis::Vertical);
            let gap = match axis {
                Axis::Horizontal => element.bounding_box.x - parent.bounding_box.x,
                Axis::Vertical => element.bounding_box.y - parent.bounding_box.y,
            };
            push_constraint(
                &mut output,
                ConstraintKind::Gap,
                element,
                element.parent_id.clone(),
                Some(axis),
                Some(gap.max(0.0)),
                "parent_inset",
            );
        }
    }
    output.sort_by(|a, b| {
        a.subject_id
            .cmp(&b.subject_id)
            .then_with(|| a.kind.cmp(&b.kind))
            .then_with(|| a.constraint_id.cmp(&b.constraint_id))
    });
    debug_assert!(output.len() <= MAX_PLAN_CONSTRAINTS);
    for (index, constraint) in output.iter_mut().enumerate() {
        constraint.constraint_id = format!("constraint.{index:04}");
    }
    output
}

fn push_constraint(
    output: &mut Vec<LayoutConstraint>,
    kind: ConstraintKind,
    element: &AnalysisElement,
    target_id: Option<String>,
    axis: Option<Axis>,
    value: Option<f64>,
    relation: &str,
) {
    output.push(LayoutConstraint {
        constraint_id: String::new(),
        kind,
        subject_id: element.element_id.clone(),
        target_id,
        axis,
        value: value.map(round_quarter),
        relation: relation.into(),
    });
}

fn derive_steps(elements: &[&AnalysisElement]) -> Vec<PlanStep> {
    let mut output = Vec::new();
    for phase in [
        PlanPhase::Structure,
        PlanPhase::Visual,
        PlanPhase::Decoration,
    ] {
        for element in elements {
            let include = match phase {
                PlanPhase::Structure => element.kind != VisualElementKind::Decoration,
                PlanPhase::Visual => !matches!(
                    element.kind,
                    VisualElementKind::Container | VisualElementKind::Decoration
                ),
                PlanPhase::Decoration => element.kind == VisualElementKind::Decoration,
            };
            if include {
                output.push(PlanStep {
                    step_id: format!("step.{:04}", output.len()),
                    phase,
                    subject_id: element.element_id.clone(),
                    action: match phase {
                        PlanPhase::Structure => "build_layout",
                        PlanPhase::Visual => "apply_tokens",
                        PlanPhase::Decoration => "apply_decoration",
                    }
                    .into(),
                });
            }
        }
    }
    debug_assert!(output.len() <= MAX_PLAN_STEPS);
    output
}

fn diagnose(
    elements: &[&AnalysisElement],
    by_id: &BTreeMap<&str, &AnalysisElement>,
) -> Vec<PlanningDiagnostic> {
    let mut output = Vec::new();
    let absolute_count = elements
        .iter()
        .filter(|element| element.layout.kind == LayoutBehaviorKind::AbsoluteDecoration)
        .count();
    if !elements.is_empty() && absolute_count as f64 / elements.len() as f64 > 0.35 {
        output.push(diagnostic(
            "PLAN_EXCESSIVE_ABSOLUTE_POSITIONING",
            "analysis",
            "more than 35% of elements use absolute decoration positioning",
        ));
    }
    for element in elements {
        if let Some(parent) = element.parent_id.as_deref().and_then(|id| by_id.get(id))
            && (element.bounding_box.width > parent.bounding_box.width + 0.01
                || element.bounding_box.height > parent.bounding_box.height + 0.01)
        {
            output.push(diagnostic(
                "PLAN_IMPOSSIBLE_MIN_SIZE",
                &element.element_id,
                "observed child size exceeds its parent bounds",
            ));
        }
        let horizontal = element
            .alignment_clues
            .iter()
            .filter(|clue| clue.axis == Axis::Horizontal)
            .map(|clue| format!("{:?}:{:?}", clue.relation, clue.target))
            .collect::<BTreeSet<_>>();
        let vertical = element
            .alignment_clues
            .iter()
            .filter(|clue| clue.axis == Axis::Vertical)
            .map(|clue| format!("{:?}:{:?}", clue.relation, clue.target))
            .collect::<BTreeSet<_>>();
        if horizontal.len() > 1 || vertical.len() > 1 {
            output.push(diagnostic(
                "PLAN_CONTRADICTORY_ALIGNMENT",
                &element.element_id,
                "multiple distinct alignment relations constrain the same axis",
            ));
        }
        if element.layout.kind == LayoutBehaviorKind::FixedAnchor
            && element.layout.anchors.contains(&AnchorEdge::Left)
            && element.layout.anchors.contains(&AnchorEdge::Right)
        {
            output.push(diagnostic(
                "PLAN_OVERCONSTRAINED_FIXED_WIDTH",
                &element.element_id,
                "fixed element has both horizontal edge anchors and an observed width",
            ));
        }
    }
    output.sort_by(|a, b| {
        a.code
            .cmp(&b.code)
            .then_with(|| a.subject_id.cmp(&b.subject_id))
    });
    debug_assert!(output.len() <= MAX_PLAN_DIAGNOSTICS);
    output
}

fn diagnostic(code: &str, subject: &str, message: &str) -> PlanningDiagnostic {
    PlanningDiagnostic {
        code: code.into(),
        subject_id: subject.into(),
        message: message.into(),
    }
}
fn round_quarter(value: f64) -> f64 {
    (value * 4.0).round() / 4.0
}
fn encode_id(id: &str) -> String {
    id.as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn token_kind_name(kind: CandidateTokenKind) -> &'static str {
    match kind {
        CandidateTokenKind::Color => "color",
        CandidateTokenKind::FontSize => "font_size",
        CandidateTokenKind::Spacing => "spacing",
        CandidateTokenKind::Radius => "radius",
        CandidateTokenKind::BorderWidth => "border",
        CandidateTokenKind::Shadow => "shadow",
        CandidateTokenKind::RepeatedSize => "size",
    }
}
fn component_name(kind: ComponentCandidateKind) -> &'static str {
    match kind {
        ComponentCandidateKind::Button => "button",
        ComponentCandidateKind::Label => "label",
        ComponentCandidateKind::ImageFrame => "image_frame",
        ComponentCandidateKind::Card => "card",
        ComponentCandidateKind::List => "list",
        ComponentCandidateKind::ListItem => "list_item",
        ComponentCandidateKind::Dialog => "dialog",
        ComponentCandidateKind::HudIndicator => "hud_indicator",
        ComponentCandidateKind::Badge => "badge",
        ComponentCandidateKind::Progress => "progress",
        ComponentCandidateKind::ScrollRegion => "scroll_region",
        ComponentCandidateKind::Unknown => "container",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::parse_analysis_json;

    fn regular_page() -> UiReferenceAnalysis {
        parse_analysis_json(include_bytes!("../fixtures/analysis/regular_page.json")).unwrap()
    }

    #[test]
    fn planning_is_deterministic_and_phase_ordered() {
        let analysis = regular_page();
        let first = plan_analysis(&analysis);
        let second = plan_analysis(&analysis);
        assert_eq!(
            serde_json::to_vec(&first).unwrap(),
            serde_json::to_vec(&second).unwrap()
        );
        assert!(
            first
                .steps
                .windows(2)
                .all(|pair| pair[0].phase <= pair[1].phase)
        );
        assert!(
            first
                .tokens
                .iter()
                .any(|token| token.kind == CandidateTokenKind::Color
                    && token.matched_existing_token.is_some())
        );
        assert!(
            first
                .tokens
                .iter()
                .all(|token| token.matched_existing_token.is_some()
                    || token.scope != RecommendationScope::ExistingGlobal)
        );
    }

    #[test]
    fn repeated_patterns_preserve_mapping_and_reuse_widgets() {
        let analysis =
            parse_analysis_json(include_bytes!("../fixtures/analysis/long_list.json")).unwrap();
        let plan = plan_analysis(&analysis);
        let row = plan
            .components
            .iter()
            .find(|component| component.pattern_id.as_deref() == Some("pattern.list_row"))
            .unwrap();
        assert_eq!(row.source_element_ids, vec!["list.row_sample"]);
        assert_eq!(row.component, "list_item");
        assert_eq!(row.variant, None);
        assert_eq!(row.scope, RecommendationScope::Page);
    }

    #[test]
    fn constraints_cover_parent_size_alignment_gap_and_flex() {
        let plan = plan_analysis(&regular_page());
        for kind in [
            ConstraintKind::Parent,
            ConstraintKind::Width,
            ConstraintKind::Height,
            ConstraintKind::Anchor,
            ConstraintKind::Align,
            ConstraintKind::Gap,
            ConstraintKind::Flex,
        ] {
            assert!(
                plan.constraints
                    .iter()
                    .any(|constraint| constraint.kind == kind),
                "missing {kind:?}"
            );
        }
    }

    #[test]
    fn diagnostics_are_stable_for_conflict_absolute_and_impossible_size() {
        let mut analysis = regular_page();
        let child = analysis
            .elements
            .iter_mut()
            .find(|element| element.element_id == "page.title")
            .unwrap();
        child.bounding_box.width = 2000.0;
        child.layout.kind = LayoutBehaviorKind::AbsoluteDecoration;
        let clue = child.alignment_clues[0].clone();
        child.alignment_clues.push(crate::analysis::AlignmentClue {
            relation: crate::analysis::AlignmentRelation::AlignedEdge,
            ..clue
        });
        analysis.elements[0].layout.kind = LayoutBehaviorKind::AbsoluteDecoration;
        let codes: Vec<_> = plan_analysis(&analysis)
            .diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.code)
            .collect();
        assert_eq!(
            codes,
            vec![
                "PLAN_CONTRADICTORY_ALIGNMENT",
                "PLAN_EXCESSIVE_ABSOLUTE_POSITIONING",
                "PLAN_IMPOSSIBLE_MIN_SIZE"
            ]
        );
    }

    #[test]
    fn planner_outputs_stay_within_budgets() {
        let plan = plan_analysis(&regular_page());
        assert!(plan.tokens.len() <= MAX_PLAN_TOKENS);
        assert!(plan.components.len() <= MAX_PLAN_COMPONENTS);
        assert!(plan.constraints.len() <= MAX_PLAN_CONSTRAINTS);
        assert!(plan.steps.len() <= MAX_PLAN_STEPS);
        assert!(plan.diagnostics.len() <= MAX_PLAN_DIAGNOSTICS);
    }

    #[test]
    fn component_ids_are_unique_for_previously_colliding_source_ids() {
        let mut analysis = regular_page();
        let template = analysis.elements[1].clone();
        let mut dashed = template.clone();
        dashed.element_id = "a-b".into();
        let mut underscored = template;
        underscored.element_id = "a_b".into();
        analysis.elements = vec![dashed, underscored];

        let plan = plan_analysis(&analysis);
        let ids: BTreeSet<_> = plan
            .components
            .iter()
            .map(|component| component.component_id.as_str())
            .collect();
        assert_eq!(ids.len(), 2);
        assert_ne!(
            plan.components[0].component_id,
            plan.components[1].component_id
        );
    }

    #[test]
    fn upstream_maximum_input_is_not_silently_truncated() {
        let mut analysis = regular_page();
        let root = analysis.elements[0].clone();
        let template = analysis.elements[1].clone();
        let clue = template.alignment_clues[0].clone();
        let mut elements = vec![root];
        for index in 0..511 {
            let mut element = template.clone();
            element.element_id = format!("page.generated.{index:03}");
            element.bounding_box.width = (index * 2 + 1) as f64;
            element.layout.anchors = vec![
                AnchorEdge::Top,
                AnchorEdge::Left,
                AnchorEdge::Bottom,
                AnchorEdge::Right,
            ];
            element.alignment_clues = vec![clue.clone(); 16];
            elements.push(element);
        }
        analysis.elements = elements;

        let plan = plan_analysis(&analysis);
        assert_eq!(plan.components.len(), 512);
        assert!(plan.tokens.len() > 256);
        assert!(plan.constraints.len() > 4096);
        assert_eq!(
            plan.components
                .iter()
                .map(|component| &component.component_id)
                .collect::<BTreeSet<_>>()
                .len(),
            plan.components.len()
        );
    }

    #[test]
    fn token_origin_distinguishes_observation_catalog_and_assumption() {
        let long_list =
            parse_analysis_json(include_bytes!("../fixtures/analysis/long_list.json")).unwrap();
        let plan = plan_analysis(&long_list);
        assert!(
            plan.tokens
                .iter()
                .any(|token| token.origin == TokenOrigin::ObservedGeometry)
        );
        assert!(
            plan.tokens
                .iter()
                .any(|token| token.origin == TokenOrigin::ExistingCatalogSuggestion)
        );
        assert!(
            plan.tokens
                .iter()
                .any(|token| token.origin == TokenOrigin::HeuristicAssumption)
        );
    }
}
