use super::{
    UiBackgroundStyle, UiDocument, UiLayout, UiLayoutPatch, UiLayoutPosition, UiLength, UiNode,
    UiNodeId, UiResolvedBackground, UiResolvedStyleProperties, UiStyle, UiStylePatch,
    UiStyleProperties, UiValidationPhase,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const UI_DOCUMENT_BUDGET_PROFILE: &str = "mobile_baseline_v1";
pub const UI_DOCUMENT_SOURCE_BYTES_UNKNOWN: usize = 0;
pub const UI_DOCUMENT_MAX_BYTES: usize = 256 * 1024;
pub const UI_DOCUMENT_MAX_NODES: usize = 512;
pub const UI_DOCUMENT_MAX_TREE_DEPTH: usize = 24;
pub const UI_DOCUMENT_MAX_CHILDREN: usize = 128;
pub const UI_DOCUMENT_MAX_ASSETS: usize = 128;
pub const UI_DOCUMENT_MAX_STYLE_ENTRIES: usize = 256;
pub const UI_DOCUMENT_MAX_ACTION_REFERENCES: usize = 64;
pub const UI_DOCUMENT_MAX_RESPONSIVE_VARIANTS: usize = 32;
pub const UI_DOCUMENT_MAX_OVERRIDES: usize = 256;
pub const UI_DOCUMENT_MAX_STRING_BYTES: usize = 4 * 1024;
pub const UI_DOCUMENT_MAX_LITERAL_BYTES: usize = 16 * 1024;
pub const UI_DOCUMENT_MAX_ACTION_PARAM_BYTES: usize = 4 * 1024;
pub const UI_DOCUMENT_MAX_METADATA_BYTES: usize = 8 * 1024;
pub const UI_DOCUMENT_MAX_ANIMATIONS: usize = 32;
pub const UI_DOCUMENT_MAX_EFFECT_COMPLEXITY: usize = 256;
pub const UI_DOCUMENT_MAX_LAYOUT_PX: f32 = 16_384.0;

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UiDocumentBudgetUsage {
    pub source_bytes: usize,
    pub nodes: usize,
    pub max_tree_depth: usize,
    pub max_children: usize,
    pub assets: usize,
    pub token_style_entries: usize,
    pub action_references: usize,
    pub responsive_variants: usize,
    pub state_responsive_overrides: usize,
    pub strings: usize,
    pub max_string_bytes: usize,
    pub max_literal_bytes: usize,
    pub metadata_bytes: usize,
    pub max_action_param_bytes: usize,
    pub animations: usize,
    pub effect_complexity: usize,
}

impl UiDocumentBudgetUsage {
    pub(crate) fn permits_full_validation(&self) -> bool {
        self.nodes <= UI_DOCUMENT_MAX_NODES
            && self.max_tree_depth <= UI_DOCUMENT_MAX_TREE_DEPTH
            && self.max_children <= UI_DOCUMENT_MAX_CHILDREN
            && self.assets <= UI_DOCUMENT_MAX_ASSETS
            && self.token_style_entries <= UI_DOCUMENT_MAX_STYLE_ENTRIES
            && self.action_references <= UI_DOCUMENT_MAX_ACTION_REFERENCES
            && self.responsive_variants <= UI_DOCUMENT_MAX_RESPONSIVE_VARIANTS
            && self.state_responsive_overrides <= UI_DOCUMENT_MAX_OVERRIDES
            && self.animations <= UI_DOCUMENT_MAX_ANIMATIONS
            && self.effect_complexity <= UI_DOCUMENT_MAX_EFFECT_COMPLEXITY
    }
}

#[derive(Clone, Debug)]
pub(crate) struct UiBudgetViolation {
    pub code: &'static str,
    pub phase: UiValidationPhase,
    pub path: String,
    pub node_id: Option<UiNodeId>,
}

#[derive(Default)]
pub(crate) struct UiBudgetAnalysis {
    pub usage: UiDocumentBudgetUsage,
    pub violations: Vec<UiBudgetViolation>,
}

pub(crate) fn analyze_document_budget(
    source_bytes: usize,
    raw: &Value,
    document: &UiDocument,
) -> UiBudgetAnalysis {
    let mut analysis = UiBudgetAnalysis {
        usage: UiDocumentBudgetUsage {
            source_bytes,
            assets: document.assets.len(),
            token_style_entries: document.tokens.len().saturating_add(document.styles.len()),
            responsive_variants: document.responsive.len(),
            state_responsive_overrides: document
                .states
                .iter()
                .map(|state| state.overrides.len())
                .chain(
                    document
                        .responsive
                        .iter()
                        .map(|variant| variant.overrides.len()),
                )
                .fold(0usize, usize::saturating_add),
            metadata_bytes: serde_json::to_vec(&document.metadata)
                .map_or(usize::MAX, |value| value.len()),
            ..Default::default()
        },
        violations: Vec::new(),
    };

    if document.metadata.budget_profile != UI_DOCUMENT_BUDGET_PROFILE {
        push(
            &mut analysis,
            "UI_BUDGET_PROFILE_UNKNOWN",
            "$.metadata.budget_profile",
            None,
        );
    }
    check_limit(
        &mut analysis,
        document.assets.len(),
        UI_DOCUMENT_MAX_ASSETS,
        "UI_DOCUMENT_ASSET_COUNT_BUDGET_EXCEEDED",
        "$.assets",
    );
    check_limit(
        &mut analysis,
        document.tokens.len().saturating_add(document.styles.len()),
        UI_DOCUMENT_MAX_STYLE_ENTRIES,
        "UI_DOCUMENT_STYLE_ENTRY_BUDGET_EXCEEDED",
        "$.styles",
    );
    check_limit(
        &mut analysis,
        document.responsive.len(),
        UI_DOCUMENT_MAX_RESPONSIVE_VARIANTS,
        "UI_DOCUMENT_RESPONSIVE_BUDGET_EXCEEDED",
        "$.responsive",
    );
    let override_count = analysis.usage.state_responsive_overrides;
    check_limit(
        &mut analysis,
        override_count,
        UI_DOCUMENT_MAX_OVERRIDES,
        "UI_DOCUMENT_OVERRIDE_BUDGET_EXCEEDED",
        "$.responsive",
    );
    if analysis.usage.metadata_bytes > UI_DOCUMENT_MAX_METADATA_BYTES {
        push(
            &mut analysis,
            "UI_DOCUMENT_METADATA_BUDGET_EXCEEDED",
            "$.metadata",
            None,
        );
    }

    analyze_strings(raw, &mut analysis);
    analyze_nodes(document, &mut analysis);
    if analysis.usage.permits_full_validation() {
        analyze_effects(document, &mut analysis);
    } else {
        analysis.usage.effect_complexity = document
            .styles
            .values()
            .map(|style| declared_effect_complexity(&style.properties))
            .fold(0usize, usize::saturating_add);
    }

    let node_count = analysis.usage.nodes;
    let max_tree_depth = analysis.usage.max_tree_depth;
    let max_children = analysis.usage.max_children;
    let action_references = analysis.usage.action_references;
    let animations = analysis.usage.animations;
    let effect_complexity = analysis.usage.effect_complexity;
    check_limit(
        &mut analysis,
        node_count,
        UI_DOCUMENT_MAX_NODES,
        "UI_DOCUMENT_NODE_COUNT_BUDGET_EXCEEDED",
        "$.root",
    );
    check_limit(
        &mut analysis,
        max_tree_depth,
        UI_DOCUMENT_MAX_TREE_DEPTH,
        "UI_DOCUMENT_TREE_DEPTH_BUDGET_EXCEEDED",
        "$.root",
    );
    check_limit(
        &mut analysis,
        max_children,
        UI_DOCUMENT_MAX_CHILDREN,
        "UI_DOCUMENT_CHILDREN_BUDGET_EXCEEDED",
        "$.root",
    );
    check_limit(
        &mut analysis,
        action_references,
        UI_DOCUMENT_MAX_ACTION_REFERENCES,
        "UI_DOCUMENT_ACTION_BUDGET_EXCEEDED",
        "$.root",
    );
    check_limit(
        &mut analysis,
        animations,
        UI_DOCUMENT_MAX_ANIMATIONS,
        "UI_DOCUMENT_ANIMATION_BUDGET_EXCEEDED",
        "$",
    );
    check_limit(
        &mut analysis,
        effect_complexity,
        UI_DOCUMENT_MAX_EFFECT_COMPLEXITY,
        "UI_DOCUMENT_EFFECT_COMPLEXITY_BUDGET_EXCEEDED",
        "$",
    );
    analysis
}

fn analyze_strings(raw: &Value, analysis: &mut UiBudgetAnalysis) {
    let mut pending = vec![(raw, "$".to_owned())];
    while let Some((value, path)) = pending.pop() {
        match value {
            Value::String(value) => {
                analysis.usage.strings = analysis.usage.strings.saturating_add(1);
                if path.ends_with(".literal") {
                    analysis.usage.max_literal_bytes =
                        analysis.usage.max_literal_bytes.max(value.len());
                    if value.len() > UI_DOCUMENT_MAX_LITERAL_BYTES {
                        push(
                            analysis,
                            "UI_DOCUMENT_LITERAL_STRING_BUDGET_EXCEEDED",
                            &path,
                            None,
                        );
                    }
                } else {
                    analysis.usage.max_string_bytes =
                        analysis.usage.max_string_bytes.max(value.len());
                    if value.len() > UI_DOCUMENT_MAX_STRING_BYTES {
                        push(analysis, "UI_DOCUMENT_STRING_BUDGET_EXCEEDED", &path, None);
                    }
                }
            }
            Value::Array(values) => {
                for (index, value) in values.iter().enumerate().rev() {
                    pending.push((value, format!("{path}[{index}]")));
                }
            }
            Value::Object(values) => {
                for (key, value) in values.iter().rev() {
                    pending.push((value, format!("{path}.{key}")));
                }
            }
            _ => {}
        }
    }
}

fn analyze_nodes(document: &UiDocument, analysis: &mut UiBudgetAnalysis) {
    let mut pending = vec![(&document.root, "$.root".to_owned(), 1usize)];
    while let Some((node, path, depth)) = pending.pop() {
        analysis.usage.nodes = analysis.usage.nodes.saturating_add(1);
        analysis.usage.max_tree_depth = analysis.usage.max_tree_depth.max(depth);
        analysis.usage.max_children = analysis.usage.max_children.max(node.children().len());
        if let UiNode::Button { on_click, .. } = node {
            analysis.usage.action_references = analysis.usage.action_references.saturating_add(1);
            let bytes =
                serde_json::to_vec(&on_click.params).map_or(usize::MAX, |value| value.len());
            analysis.usage.max_action_param_bytes =
                analysis.usage.max_action_param_bytes.max(bytes);
            if bytes > UI_DOCUMENT_MAX_ACTION_PARAM_BYTES {
                push(
                    analysis,
                    "UI_DOCUMENT_ACTION_PARAM_BUDGET_EXCEEDED",
                    &format!("{path}.on_click.params"),
                    Some(node.id().clone()),
                );
            }
        }
        analyze_layout_bounds(
            node.layout(),
            &format!("{path}.layout"),
            node.id(),
            analysis,
        );
        for (index, child) in node.children().iter().enumerate().rev() {
            pending.push((
                child,
                node.child_path(&path, index),
                depth.saturating_add(1),
            ));
        }
    }
    for (state_index, state) in document.states.iter().enumerate() {
        for (index, node_override) in state.overrides.iter().enumerate() {
            if let Some(layout) = &node_override.set.layout {
                analyze_layout_patch_bounds(
                    layout,
                    &format!("$.states[{state_index}].overrides[{index}].set.layout"),
                    &node_override.node_id,
                    analysis,
                );
            }
        }
    }
    for (variant_index, variant) in document.responsive.iter().enumerate() {
        for (index, node_override) in variant.overrides.iter().enumerate() {
            if let Some(layout) = &node_override.set.layout {
                analyze_layout_patch_bounds(
                    layout,
                    &format!("$.responsive[{variant_index}].overrides[{index}].set.layout"),
                    &node_override.node_id,
                    analysis,
                );
            }
        }
    }
}

fn analyze_effects(document: &UiDocument, analysis: &mut UiBudgetAnalysis) {
    let mut total = document
        .styles
        .values()
        .map(|style| declared_effect_complexity(&style.properties))
        .fold(0usize, usize::saturating_add);
    let mut pending = vec![(&document.root, "$.root".to_owned())];
    while let Some((node, path)) = pending.pop() {
        total = total.saturating_add(resolved_style_complexity(
            document,
            node.style(),
            &format!("{path}.style"),
        ));
        if let Some(component) = node.component() {
            for (state, style) in &component.state_overrides {
                total = total.saturating_add(resolved_style_complexity(
                    document,
                    style,
                    &format!("{path}.component.state_overrides.{state}"),
                ));
            }
        }
        for (index, child) in node.children().iter().enumerate().rev() {
            pending.push((child, node.child_path(&path, index)));
        }
    }
    for (path, patch) in document
        .states
        .iter()
        .enumerate()
        .flat_map(|(group, state)| {
            state
                .overrides
                .iter()
                .enumerate()
                .filter_map(move |(index, item)| {
                    item.set.style.as_ref().map(|patch| {
                        (
                            format!("$.states[{group}].overrides[{index}].set.style"),
                            patch,
                        )
                    })
                })
        })
        .chain(
            document
                .responsive
                .iter()
                .enumerate()
                .flat_map(|(group, variant)| {
                    variant
                        .overrides
                        .iter()
                        .enumerate()
                        .filter_map(move |(index, item)| {
                            item.set.style.as_ref().map(|patch| {
                                (
                                    format!("$.responsive[{group}].overrides[{index}].set.style"),
                                    patch,
                                )
                            })
                        })
                }),
        )
    {
        total = total.saturating_add(resolved_patch_complexity(document, patch, &path));
    }
    analysis.usage.effect_complexity = total;
}

fn declared_effect_complexity(properties: &UiStyleProperties) -> usize {
    let gradient = match &properties.background {
        Some(UiBackgroundStyle::LinearGradient { stops, .. }) => stops.len(),
        _ => 0,
    };
    gradient
        .saturating_add(properties.shadows.as_ref().map_or(0, Vec::len))
        .saturating_add(usize::from(properties.material.is_some()))
}

fn resolved_style_complexity(document: &UiDocument, style: &UiStyle, path: &str) -> usize {
    document.resolve_style(style, path).map_or_else(
        |_| declared_effect_complexity(&style.inline),
        |resolved| resolved_effect_complexity(&resolved.properties),
    )
}

fn resolved_patch_complexity(document: &UiDocument, patch: &UiStylePatch, path: &str) -> usize {
    let style = UiStyle {
        component: patch.component.clone(),
        role: patch.role.clone(),
        text_role: patch.text_role.clone(),
        inline: patch.inline.clone().unwrap_or_default(),
    };
    resolved_style_complexity(document, &style, path)
}

fn resolved_effect_complexity(properties: &UiResolvedStyleProperties) -> usize {
    let gradient = match &properties.background {
        Some(UiResolvedBackground::LinearGradient { stops, .. }) => stops.len(),
        _ => 0,
    };
    gradient
        .saturating_add(properties.shadows.as_ref().map_or(0, Vec::len))
        .saturating_add(usize::from(properties.material.is_some()))
}

fn analyze_layout_bounds(
    layout: &UiLayout,
    path: &str,
    node_id: &UiNodeId,
    analysis: &mut UiBudgetAnalysis,
) {
    for (field, value) in [
        ("width", layout.width),
        ("height", layout.height),
        ("min_width", layout.min_width),
        ("min_height", layout.min_height),
        ("max_width", layout.max_width),
        ("max_height", layout.max_height),
    ] {
        check_layout_length(value, &format!("{path}.{field}"), node_id, analysis);
    }
    if let UiLayoutPosition::Absolute(absolute) = layout.position {
        for (field, value) in [
            ("left", absolute.left),
            ("right", absolute.right),
            ("top", absolute.top),
            ("bottom", absolute.bottom),
        ] {
            if let Some(value) = value {
                check_layout_length(
                    value,
                    &format!("{path}.position.absolute.{field}"),
                    node_id,
                    analysis,
                );
            }
        }
    }
}

fn analyze_layout_patch_bounds(
    layout: &UiLayoutPatch,
    path: &str,
    node_id: &UiNodeId,
    analysis: &mut UiBudgetAnalysis,
) {
    for (field, value) in [
        ("width", layout.width),
        ("height", layout.height),
        ("min_width", layout.min_width),
        ("min_height", layout.min_height),
        ("max_width", layout.max_width),
        ("max_height", layout.max_height),
    ] {
        if let Some(value) = value {
            check_layout_length(value, &format!("{path}.{field}"), node_id, analysis);
        }
    }
    if let Some(UiLayoutPosition::Absolute(absolute)) = layout.position {
        for (field, value) in [
            ("left", absolute.left),
            ("right", absolute.right),
            ("top", absolute.top),
            ("bottom", absolute.bottom),
        ] {
            if let Some(value) = value {
                check_layout_length(
                    value,
                    &format!("{path}.position.absolute.{field}"),
                    node_id,
                    analysis,
                );
            }
        }
    }
}

fn check_layout_length(
    value: UiLength,
    path: &str,
    node_id: &UiNodeId,
    analysis: &mut UiBudgetAnalysis,
) {
    if matches!(value, UiLength::Px(value) if value.is_finite() && value.abs() > UI_DOCUMENT_MAX_LAYOUT_PX)
    {
        analysis.violations.push(UiBudgetViolation {
            code: "UI_LAYOUT_OBVIOUSLY_OUT_OF_BOUNDS",
            phase: UiValidationPhase::Structure,
            path: path.to_owned(),
            node_id: Some(node_id.clone()),
        });
    }
}

fn check_limit(
    analysis: &mut UiBudgetAnalysis,
    found: usize,
    limit: usize,
    code: &'static str,
    path: &str,
) {
    if found > limit {
        push(analysis, code, path, None);
    }
}

fn push(
    analysis: &mut UiBudgetAnalysis,
    code: &'static str,
    path: &str,
    node_id: Option<UiNodeId>,
) {
    analysis.violations.push(UiBudgetViolation {
        code,
        phase: UiValidationPhase::Budget,
        path: path.to_owned(),
        node_id,
    });
}
