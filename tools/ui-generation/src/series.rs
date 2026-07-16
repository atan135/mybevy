//! Trusted multi-reference evidence for one responsive `UiDocument` page series.
//!
//! Stage 7 deliberately treats a single visible reference as insufficient evidence for page
//! states and responsive variants. This module is the separate, explicit path for cases where
//! the task contains additional state or viewport references. It validates a caller-supplied
//! evidence matrix against the existing task, analysis, and formal document; it never invents
//! states, actions, bindings, or document nodes.

use crate::{
    analysis::{EvidenceSource, UiReferenceAnalysis},
    contract::{AdditionalReferenceRole, GenerationTask, TargetViewport},
    lifecycle::{TaskFailure, TaskFailureKind},
};
use project::framework::ui::document::{
    UiComponentSpec, UiControlSlot, UiDocument, UiNode, UiNodeId, UiResponsiveVariant,
    UiStateDefinition,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const PAGE_SERIES_EVIDENCE_VERSION: u32 = 1;
pub const MIN_TOUCH_TARGET_LOGICAL_PX: f32 = 44.0;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PageSeriesEvidence {
    pub version: u32,
    pub primary_reference_id: String,
    #[serde(default)]
    pub shared_nodes: Vec<SharedNodeEvidence>,
    #[serde(default)]
    pub visible_states: Vec<VisibleStateEvidence>,
    #[serde(default)]
    pub responsive_variants: Vec<ResponsiveVariantEvidence>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SharedNodeEvidence {
    /// The canonical element is represented by the one node in the generated document.
    pub canonical_element_id: String,
    /// A matching element in a second visible reference. It must not create a duplicate node.
    pub alternate_element_id: String,
    pub reference_id: String,
    pub evidence_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VisibleStateEvidence {
    pub definition: UiStateDefinition,
    pub reference_id: String,
    pub evidence_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResponsiveVariantEvidence {
    pub variant: UiResponsiveVariant,
    /// `observed` needs two distinct viewport references. `project_default` is the only valid
    /// single-viewport fallback and is disclosed in the result.
    pub derivation: ResponsiveDerivation,
    pub reference_ids: Vec<String>,
    pub evidence_ids: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponsiveDerivation {
    Observed,
    ProjectDefault,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PageSeriesSourceMapEntry {
    pub reference_element_id: String,
    pub node_id: String,
    pub reference_id: String,
    pub evidence_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SeriesDisclosure {
    pub code: String,
    pub subject_id: Option<String>,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AccessibilitySupplement {
    pub node_id: String,
    pub accessible_label_source: String,
    pub keyboard_focus_order: u16,
    pub touch_target: TouchTargetPolicy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TouchTargetPolicy {
    ExplicitMinimum,
    RuntimeComponentMinimum,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PageSeriesValidation {
    pub source_map: Vec<PageSeriesSourceMapEntry>,
    pub accessibility: Vec<AccessibilitySupplement>,
    pub disclosures: Vec<SeriesDisclosure>,
}

/// Validates a visible page series. `document.states` and `document.responsive` must be exactly
/// the declarations backed by this evidence matrix; this keeps the Stage 7 single-state gate
/// intact while allowing a separately trusted multi-reference path.
pub fn validate_page_series(
    task: &GenerationTask,
    analysis: &UiReferenceAnalysis,
    document: &UiDocument,
    evidence: &PageSeriesEvidence,
) -> Result<PageSeriesValidation, TaskFailure> {
    task.validate()?;
    if evidence.version != PAGE_SERIES_EVIDENCE_VERSION
        || evidence.primary_reference_id != task.primary_reference.reference_id
        || !analysis
            .references
            .iter()
            .any(|reference| reference.reference_id == evidence.primary_reference_id)
    {
        return Err(invalid(
            "page series must name the task primary reference and current evidence version",
        ));
    }

    let nodes = collect_nodes(&document.root);
    let elements = analysis
        .elements
        .iter()
        .map(|element| {
            (
                element.element_id.as_str(),
                element.bounding_box.reference_id.as_str(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let evidence_by_id = analysis
        .evidence
        .iter()
        .map(|item| (item.evidence_id.as_str(), &item.source))
        .collect::<BTreeMap<_, _>>();

    let mut source_map = Vec::new();
    for element in &analysis.elements {
        let element_id = element.element_id.as_str();
        let reference_id = element.bounding_box.reference_id.as_str();
        if reference_id == evidence.primary_reference_id.as_str() {
            let node_id = UiNodeId::new(element_id).map_err(|_| {
                invalid("primary analysis element IDs must be valid stable UiDocument node IDs")
            })?;
            if nodes.contains_key(&node_id) {
                source_map.push(PageSeriesSourceMapEntry {
                    reference_element_id: element_id.to_owned(),
                    node_id: node_id.to_string(),
                    reference_id: reference_id.to_owned(),
                    evidence_ids: sorted_unique(&element.evidence_ids)?,
                });
            }
        }
    }

    let mut shared_alternates = BTreeSet::new();
    let mut shared_reference_ids = BTreeSet::new();
    for mapping in &evidence.shared_nodes {
        let Some(canonical_reference) = elements.get(mapping.canonical_element_id.as_str()) else {
            return Err(invalid(
                "shared node canonical element is absent from trusted analysis",
            ));
        };
        let Some(alternate_reference) = elements.get(mapping.alternate_element_id.as_str()) else {
            return Err(invalid(
                "shared node alternate element is absent from trusted analysis",
            ));
        };
        if *canonical_reference != evidence.primary_reference_id
            || *alternate_reference != mapping.reference_id
            || mapping.reference_id == evidence.primary_reference_id
            || !task
                .additional_references
                .iter()
                .any(|reference| reference.image.reference_id == mapping.reference_id)
            || !shared_alternates.insert(mapping.alternate_element_id.as_str())
        {
            return Err(invalid(
                "shared node mappings must map one primary element to one distinct additional reference element",
            ));
        }
        let node_id = UiNodeId::new(mapping.canonical_element_id.clone())
            .map_err(|_| invalid("shared node canonical element ID is not a valid node ID"))?;
        if !nodes.contains_key(&node_id) {
            return Err(invalid(
                "shared node canonical element does not exist in the generated document",
            ));
        }
        validate_reference_evidence(
            &mapping.evidence_ids,
            &mapping.reference_id,
            &evidence_by_id,
        )?;
        shared_reference_ids.insert(mapping.reference_id.as_str());
        source_map.push(PageSeriesSourceMapEntry {
            reference_element_id: mapping.alternate_element_id.clone(),
            node_id: node_id.to_string(),
            reference_id: mapping.reference_id.clone(),
            evidence_ids: sorted_unique(&mapping.evidence_ids)?,
        });
    }
    source_map.sort_by(|left, right| {
        (&left.reference_id, &left.reference_element_id)
            .cmp(&(&right.reference_id, &right.reference_element_id))
    });

    let expected_states = evidence
        .visible_states
        .iter()
        .map(|item| item.definition.clone())
        .collect::<Vec<_>>();
    if document.states != expected_states {
        return Err(invalid(
            "document states must exactly match explicitly evidenced visible state definitions",
        ));
    }
    let mut state_ids = BTreeSet::new();
    for state in &evidence.visible_states {
        if state.definition.id.as_str() == "initial"
            || !state_ids.insert(state.definition.id.as_str())
            || !task.additional_references.iter().any(|reference| {
                reference.image.reference_id == state.reference_id
                    && matches!(
                        &reference.role,
                        AdditionalReferenceRole::State { state_id, .. }
                            if state_id == state.definition.id.as_str()
                    )
            })
        {
            return Err(invalid(
                "each non-initial page state must be backed by its matching task state reference",
            ));
        }
        validate_reference_evidence(&state.evidence_ids, &state.reference_id, &evidence_by_id)?;
        validate_override_nodes(&state.definition.overrides, &nodes)?;
        if !shared_reference_ids.contains(state.reference_id.as_str()) {
            return Err(invalid(
                "each visible state reference needs a shared-node source-map entry",
            ));
        }
    }

    let expected_responsive = evidence
        .responsive_variants
        .iter()
        .map(|item| item.variant.clone())
        .collect::<Vec<_>>();
    if document.responsive != expected_responsive {
        return Err(invalid(
            "document responsive variants must exactly match explicitly evidenced variants",
        ));
    }
    let mut variant_ids = BTreeSet::new();
    let mut disclosures = Vec::new();
    for responsive in &evidence.responsive_variants {
        if !variant_ids.insert(responsive.variant.id.as_str()) {
            return Err(invalid("page series responsive variant IDs must be unique"));
        }
        validate_override_nodes(&responsive.variant.overrides, &nodes)?;
        validate_responsive_references(task, responsive)?;
        for reference_id in &responsive.reference_ids {
            validate_reference_evidence(&responsive.evidence_ids, reference_id, &evidence_by_id)?;
            if reference_id != &evidence.primary_reference_id
                && !shared_reference_ids.contains(reference_id.as_str())
            {
                return Err(invalid(
                    "each additional responsive reference needs a shared-node source-map entry",
                ));
            }
        }
        if responsive.derivation == ResponsiveDerivation::ProjectDefault {
            disclosures.push(SeriesDisclosure {
                code: "SERIES_RESPONSIVE_PROJECT_DEFAULT_ASSUMPTION".to_owned(),
                subject_id: Some(responsive.variant.id.to_string()),
                message: "only one viewport is available; project default breakpoint classes are an explicit assumption".to_owned(),
            });
        }
    }

    for node in collect_interactive_nodes(&document.root) {
        disclosures.push(SeriesDisclosure {
            code: "SERIES_ACTION_UNBOUND".to_owned(),
            subject_id: Some(node.id().to_string()),
            message: "no project-registered default action was supplied; business interaction remains unbound".to_owned(),
        });
    }
    disclosures.sort_by(|left, right| {
        (&left.code, &left.subject_id).cmp(&(&right.code, &right.subject_id))
    });
    disclosures.dedup();

    Ok(PageSeriesValidation {
        source_map,
        accessibility: build_accessibility_supplements(&document.root)?,
        disclosures,
    })
}

fn validate_responsive_references(
    task: &GenerationTask,
    evidence: &ResponsiveVariantEvidence,
) -> Result<(), TaskFailure> {
    let reference_ids = sorted_unique(&evidence.reference_ids)?;
    match evidence.derivation {
        ResponsiveDerivation::Observed => {
            if reference_ids.len() < 2
                || !reference_ids.contains(&task.primary_reference.reference_id)
                || !reference_ids
                    .iter()
                    .all(|reference_id| viewport_for_reference(task, reference_id).is_some())
                || distinct_viewports(task, &reference_ids).len() < 2
            {
                return Err(invalid(
                    "observed responsive variants require two distinct trusted viewport references including primary",
                ));
            }
        }
        ResponsiveDerivation::ProjectDefault => {
            if reference_ids != [task.primary_reference.reference_id.clone()]
                || viewport_for_reference(task, &task.primary_reference.reference_id).is_none()
            {
                return Err(invalid(
                    "project-default responsive variants may only use the primary target viewport",
                ));
            }
        }
    }
    Ok(())
}

fn viewport_for_reference<'a>(
    task: &'a GenerationTask,
    reference_id: &str,
) -> Option<&'a TargetViewport> {
    if reference_id == task.primary_reference.reference_id {
        return task.target_viewport.as_ref();
    }
    task.additional_references.iter().find_map(|reference| {
        (reference.image.reference_id == reference_id).then(|| match &reference.role {
            AdditionalReferenceRole::Viewport { viewport } => Some(viewport),
            _ => None,
        })?
    })
}

fn distinct_viewports(task: &GenerationTask, references: &[String]) -> BTreeSet<(u32, u32)> {
    references
        .iter()
        .filter_map(|reference_id| viewport_for_reference(task, reference_id))
        .map(|viewport| {
            (
                viewport.logical_width.to_bits(),
                viewport.logical_height.to_bits(),
            )
        })
        .collect()
}

fn validate_reference_evidence(
    evidence_ids: &[String],
    reference_id: &str,
    evidence_by_id: &BTreeMap<&str, &EvidenceSource>,
) -> Result<(), TaskFailure> {
    let evidence_ids = sorted_unique(evidence_ids)?;
    if evidence_ids.is_empty()
        || !evidence_ids.iter().any(|evidence_id| {
            matches!(
                evidence_by_id.get(evidence_id.as_str()),
                Some(EvidenceSource::ReferenceRegion { reference_id: source, .. }) if source == reference_id
            )
        })
    {
        return Err(invalid("series evidence must include a trusted reference-region observation for its declared reference"));
    }
    if evidence_ids
        .iter()
        .any(|evidence_id| !evidence_by_id.contains_key(evidence_id.as_str()))
    {
        return Err(invalid(
            "series evidence refers to an unknown analysis evidence ID",
        ));
    }
    Ok(())
}

fn validate_override_nodes(
    overrides: &[project::framework::ui::document::UiNodeOverride],
    nodes: &BTreeMap<UiNodeId, &UiNode>,
) -> Result<(), TaskFailure> {
    if overrides.is_empty()
        || overrides
            .iter()
            .any(|override_| !nodes.contains_key(&override_.node_id))
    {
        return Err(invalid(
            "visible state and responsive variants need nonempty overrides for existing shared document nodes",
        ));
    }
    Ok(())
}

fn collect_nodes(root: &UiNode) -> BTreeMap<UiNodeId, &UiNode> {
    let mut nodes = BTreeMap::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        nodes.insert(node.id().clone(), node);
        stack.extend(node.children());
    }
    nodes
}

fn collect_interactive_nodes(root: &UiNode) -> Vec<&UiNode> {
    let mut nodes = Vec::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.component().is_some()
            && !matches!(
                node,
                UiNode::Scroll { .. }
                    | UiNode::Badge { .. }
                    | UiNode::Progress { .. }
                    | UiNode::Tooltip { .. }
            )
        {
            nodes.push(node);
        }
        stack.extend(node.children());
    }
    nodes.sort_by_key(|node| node.id().to_string());
    nodes
}

fn build_accessibility_supplements(
    root: &UiNode,
) -> Result<Vec<AccessibilitySupplement>, TaskFailure> {
    let mut supplements = Vec::new();
    for (index, node) in collect_interactive_nodes(root).into_iter().enumerate() {
        let label = accessible_label_source(node).ok_or_else(|| {
            invalid(
                "interactive page-series controls require an existing formal accessible label slot",
            )
        })?;
        let explicit_target = explicit_touch_target(node);
        supplements.push(AccessibilitySupplement {
            node_id: node.id().to_string(),
            accessible_label_source: label.to_owned(),
            keyboard_focus_order: u16::try_from(index + 1)
                .map_err(|_| invalid("focus order exceeds protocol budget"))?,
            touch_target: if explicit_target {
                TouchTargetPolicy::ExplicitMinimum
            } else {
                TouchTargetPolicy::RuntimeComponentMinimum
            },
        });
    }
    Ok(supplements)
}

fn accessible_label_source(node: &UiNode) -> Option<&'static str> {
    match node {
        UiNode::Button { label: Some(_), .. } => Some("button.label"),
        UiNode::Modal { component, .. } if component.slots.contains_key(&UiControlSlot::Title) => {
            Some("component.slots.title")
        }
        _ if node
            .component()
            .is_some_and(|component| has_label_slot(component)) =>
        {
            Some("component.slots.label")
        }
        _ => None,
    }
}

fn has_label_slot(component: &UiComponentSpec) -> bool {
    component.slots.contains_key(&UiControlSlot::Label)
}

fn explicit_touch_target(node: &UiNode) -> bool {
    let layout = node.layout();
    matches!(layout.min_width, project::framework::ui::document::UiLength::Px(width) if width >= MIN_TOUCH_TARGET_LOGICAL_PX)
        && matches!(layout.min_height, project::framework::ui::document::UiLength::Px(height) if height >= MIN_TOUCH_TARGET_LOGICAL_PX)
}

fn sorted_unique(values: &[String]) -> Result<Vec<String>, TaskFailure> {
    if values.iter().any(|value| value.is_empty()) {
        return Err(invalid("series IDs must not be empty"));
    }
    let unique = values.iter().cloned().collect::<BTreeSet<_>>();
    if unique.len() != values.len() {
        return Err(invalid("series IDs must not contain duplicates"));
    }
    Ok(unique.into_iter().collect())
}

fn invalid(message: impl Into<String>) -> TaskFailure {
    TaskFailure::new(TaskFailureKind::InvalidInput, message, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use project::framework::ui::document::UiPageState;
    use serde_json::json;

    const DOCUMENT: &str = r#"{
      "schema_version": 1,
      "document_id": "series.fixture",
      "root": {
        "type": "container", "id": "page.root", "children": [
          {"type": "button", "id": "page.action", "label": {"literal":"Continue"}, "on_click":{"action":"fixture.close"}, "layout":{"min_width":{"px":44},"min_height":{"px":44}}},
          {"type": "modal", "id": "page.modal", "component":{"slots":{"title":{"kind":"text","content":{"literal":"Notice"}},"body":{"kind":"text","content":{"literal":"Body"}}}}}
        ]
      },
      "states": [{"id":"loading","overrides":[{"node_id":"page.root","set":{"style":{"inline":{"opacity":{"kind":"literal","value":0.6}}}}}] }],
      "responsive": [{"id":"expanded_landscape","when":{"width_class":"expanded","orientation":"landscape"},"overrides":[{"node_id":"page.root","set":{"layout":{"direction":"row"}}}]}]
    }"#;

    fn task() -> GenerationTask {
        serde_json::from_value(json!({
          "contract_version": 1, "run_id": "series-fixture", "primary_reference": {
            "reference_id": "primary", "path": "primary.png", "metadata": {
              "original_size":{"width":390,"height":844}, "orientation":"normal", "color_space":"srgb", "sha256": "0".repeat(64),
              "provenance":{"source":"fixture","authorization":"analysis_only"}
            }},
          "additional_references": [{"reference_id":"loading_ref","path":"loading.png","metadata": {"original_size":{"width":390,"height":844},"orientation":"normal","color_space":"srgb","sha256":"1".repeat(64),"provenance":{"source":"fixture","authorization":"analysis_only"}},"priority":1,"role":{"kind":"state","state_id":"loading","transition_evidence":"fixture"}},
            {"reference_id":"tablet_ref","path":"tablet.png","metadata":{"original_size":{"width":1280,"height":800},"orientation":"normal","color_space":"srgb","sha256":"2".repeat(64),"provenance":{"source":"fixture","authorization":"analysis_only"}},"priority":2,"role":{"kind":"viewport","viewport":{"logical_width":1280.0,"logical_height":800.0,"device_scale":1.0}}}],
          "target_viewport":{"logical_width":390.0,"logical_height":844.0,"device_scale":3.0}
        })).unwrap()
    }

    fn analysis() -> UiReferenceAnalysis {
        serde_json::from_value(json!({
          "schema_id":"ui-reference-analysis", "schema_version":1, "analysis_id":"series.analysis", "run_id":"series-fixture",
          "provider":{"provider_id":"fixture", "server_request_id":"fixture-1", "prompt_version":"v1"},
          "references": [
            reference("primary", 390, 844, "0"), reference("loading_ref", 390, 844, "1"), reference("tablet_ref", 1280, 800, "2")
          ],
          "regions":[region("primary", "primary_region"), region("loading_ref", "loading_region"), region("tablet_ref", "tablet_region")],
            "elements":[element("page.root", None, "primary", "primary_region"), element("page.action", Some("page.root"), "primary", "primary_region"), element("loading.root", None, "loading_ref", "loading_region"), element("tablet.root", None, "tablet_ref", "tablet_region")],
          "root_element_id":"page.root",
          "evidence":[evidence("primary_evidence","primary","primary_region"), evidence("loading_evidence","loading_ref","loading_region"), evidence("tablet_evidence","tablet_ref","tablet_region")],
          "uncertainties": []
        })).unwrap()
    }

    fn reference(id: &str, width: u32, height: u32, hash: &str) -> serde_json::Value {
        json!({"reference_id":id,"source_sha256":hash.repeat(64),"preprocess_cache_key":"a".repeat(64),"preprocess_protocol_version":1,"preprocess_implementation_version":"fixture","preprocess_manifest_sha256":"b".repeat(64),"standard_preview_sha256":"c".repeat(64),"coordinate_space":"standard_preview_pixel","coordinate_convention":"fixture","width":width,"height":height})
    }
    fn region(reference_id: &str, id: &str) -> serde_json::Value {
        json!({"region_id":id,"reference_id":reference_id,"label":"fixture","bounding_box":box_(reference_id),"confidence":1.0,"evidence_ids":[format!("{}_evidence", reference_id.trim_end_matches("_ref"))]})
    }
    fn element(
        id: &str,
        parent: Option<&str>,
        reference_id: &str,
        region_id: &str,
    ) -> serde_json::Value {
        json!({"element_id":id,"parent_id":parent,"region_id":region_id,"kind":"background","bounding_box":box_(reference_id),"layout":{"kind":"content_flow","anchors":[],"flow_axis":"vertical","scroll_axes":[],"evidence_ids":[format!("{}_evidence", reference_id.trim_end_matches("_ref"))]},"alignment_clues":[],"repeated_pattern":null,"confidence":1.0,"evidence_ids":[format!("{}_evidence", reference_id.trim_end_matches("_ref"))],"component_candidates":[],"text":null,"image":null})
    }
    fn box_(reference_id: &str) -> serde_json::Value {
        json!({"reference_id":reference_id,"coordinate_space":"standard_preview_pixel","x":0.0,"y":0.0,"width":100.0,"height":100.0,"evidence_ids":[format!("{}_evidence", reference_id.trim_end_matches("_ref"))]})
    }
    fn evidence(id: &str, reference_id: &str, region_id: &str) -> serde_json::Value {
        json!({"evidence_id":id,"source":{"kind":"reference_region","reference_id":reference_id,"region_id":region_id},"detail":"fixture"})
    }

    fn evidence_matrix() -> PageSeriesEvidence {
        serde_json::from_value(json!({
          "version": 1, "primary_reference_id":"primary",
          "shared_nodes":[{"canonical_element_id":"page.root","alternate_element_id":"loading.root","reference_id":"loading_ref","evidence_ids":["loading_evidence"]},{"canonical_element_id":"page.root","alternate_element_id":"tablet.root","reference_id":"tablet_ref","evidence_ids":["tablet_evidence"]}],
          "visible_states":[{"definition":{"id":"loading","overrides":[{"node_id":"page.root","set":{"style":{"inline":{"opacity":{"kind":"literal","value":0.6}}}}}]},"reference_id":"loading_ref","evidence_ids":["loading_evidence"]}],
          "responsive_variants":[{"variant":{"id":"expanded_landscape","when":{"width_class":"expanded","orientation":"landscape"},"overrides":[{"node_id":"page.root","set":{"layout":{"direction":"row"}}}]},"derivation":"observed","reference_ids":["primary","tablet_ref"],"evidence_ids":["primary_evidence","tablet_evidence"]}]
        })).unwrap()
    }

    #[test]
    fn trusted_multi_reference_series_shares_nodes_and_preserves_state_scope() {
        let document = UiDocument::parse_and_validate_json(DOCUMENT)
            .unwrap()
            .into_document();
        let result =
            validate_page_series(&task(), &analysis(), &document, &evidence_matrix()).unwrap();
        assert_eq!(result.source_map.len(), 4);
        assert!(
            result
                .source_map
                .iter()
                .any(|entry| entry.reference_element_id == "loading.root"
                    && entry.node_id == "page.root")
        );
        assert!(result.source_map.iter().any(|entry| {
            entry.reference_element_id == "page.root"
                && entry.reference_id == "primary"
                && entry
                    .evidence_ids
                    .iter()
                    .map(String::as_str)
                    .eq(["primary_evidence"])
        }));
        assert!(
            result
                .disclosures
                .iter()
                .any(|item| item.code == "SERIES_ACTION_UNBOUND")
        );
        assert_eq!(result.accessibility.len(), 2);
        assert_eq!(result.accessibility[0].keyboard_focus_order, 1);
    }

    #[test]
    fn single_viewport_requires_explicit_project_default_disclosure() {
        let document = UiDocument::parse_and_validate_json(DOCUMENT)
            .unwrap()
            .into_document();
        let mut matrix = evidence_matrix();
        matrix.responsive_variants[0].derivation = ResponsiveDerivation::ProjectDefault;
        matrix.responsive_variants[0].reference_ids = vec!["primary".to_owned()];
        matrix.responsive_variants[0].evidence_ids = vec!["primary_evidence".to_owned()];
        let result = validate_page_series(&task(), &analysis(), &document, &matrix).unwrap();
        assert!(
            result
                .disclosures
                .iter()
                .any(|item| item.code == "SERIES_RESPONSIVE_PROJECT_DEFAULT_ASSUMPTION")
        );
    }

    #[test]
    fn unsupported_state_and_untrusted_responsive_evidence_are_rejected() {
        let document = UiDocument::parse_and_validate_json(DOCUMENT)
            .unwrap()
            .into_document();
        let mut matrix = evidence_matrix();
        matrix.visible_states[0].definition.id = UiPageState::error();
        assert!(validate_page_series(&task(), &analysis(), &document, &matrix).is_err());

        let mut matrix = evidence_matrix();
        matrix.responsive_variants[0].reference_ids = vec!["primary".to_owned()];
        assert!(validate_page_series(&task(), &analysis(), &document, &matrix).is_err());

        let mut matrix = evidence_matrix();
        matrix
            .shared_nodes
            .retain(|entry| entry.reference_id != "loading_ref");
        assert!(validate_page_series(&task(), &analysis(), &document, &matrix).is_err());
    }
}
