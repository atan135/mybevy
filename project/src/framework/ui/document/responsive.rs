use super::{
    UiDocument, UiDocumentError, UiDocumentInputMode, UiDocumentPlatform, UiHeightClass, UiLayout,
    UiLayoutPatch, UiNode, UiNodeId, UiNodeOverride, UiOrientation, UiResponsiveCondition,
    UiSafeAreaClass, UiStyle, UiStylePatch, UiWidthClass, ValidatedUiDocument,
};
use crate::framework::ui::core::{
    UiHeightClass as RuntimeHeightClass, UiOrientation as RuntimeOrientation,
    UiWidthClass as RuntimeWidthClass,
    viewport::{
        height_class_for as runtime_height_class_for, orientation_for as runtime_orientation_for,
        responsive_classes_are_satisfiable, width_class_for as runtime_width_class_for,
    },
};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    str::FromStr,
};

pub const UI_RESPONSIVE_MAX_ABS_PRIORITY: i16 = 1_000;

const UI_RESPONSIVE_ID_MAX_BYTES: usize = 64;
const UI_PAGE_STATE_MAX_BYTES: usize = 128;
const STANDARD_PAGE_STATES: [&str; 4] = ["initial", "loading", "empty", "error"];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiResponsiveIdError {
    kind: &'static str,
    value: String,
}

impl UiResponsiveIdError {
    pub const fn kind(&self) -> &'static str {
        self.kind
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

impl fmt::Display for UiResponsiveIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid {} `{}`; expected lowercase snake_case ASCII segments",
            self.kind, self.value
        )
    }
}

impl std::error::Error for UiResponsiveIdError {}

fn valid_segments(value: &str) -> bool {
    value.split('.').all(|segment| {
        let mut bytes = segment.bytes();
        bytes.next().is_some_and(|byte| byte.is_ascii_lowercase())
            && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    })
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(transparent)]
pub struct UiResponsiveVariantId(
    #[cfg_attr(
        test,
        schemars(
            length(min = 1, max = 64),
            regex(pattern = "^[a-z][a-z0-9_]*(\\.[a-z][a-z0-9_]*)*$")
        )
    )]
    String,
);

impl UiResponsiveVariantId {
    pub fn new(value: impl Into<String>) -> Result<Self, UiResponsiveIdError> {
        let value = value.into();
        if !value.is_empty()
            && value.len() <= UI_RESPONSIVE_ID_MAX_BYTES
            && value.is_ascii()
            && valid_segments(&value)
        {
            Ok(Self(value))
        } else {
            Err(UiResponsiveIdError {
                kind: "responsive variant id",
                value,
            })
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for UiResponsiveVariantId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for UiResponsiveVariantId {
    type Err = UiResponsiveIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl<'de> Deserialize<'de> for UiResponsiveVariantId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(transparent)]
pub struct UiPageState(
    #[cfg_attr(
        test,
        schemars(
            length(min = 3, max = 128),
            regex(
                pattern = "^(initial|loading|empty|error|[a-z][a-z0-9_]*(\\.[a-z][a-z0-9_]*)+)$"
            )
        )
    )]
    String,
);

impl UiPageState {
    pub fn new(value: impl Into<String>) -> Result<Self, UiResponsiveIdError> {
        let value = value.into();
        let standard = STANDARD_PAGE_STATES.contains(&value.as_str());
        let business = value.contains('.') && valid_segments(&value);
        if value.len() <= UI_PAGE_STATE_MAX_BYTES && value.is_ascii() && (standard || business) {
            Ok(Self(value))
        } else {
            Err(UiResponsiveIdError {
                kind: "page state",
                value,
            })
        }
    }

    pub fn initial() -> Self {
        Self("initial".to_owned())
    }

    pub fn loading() -> Self {
        Self("loading".to_owned())
    }

    pub fn empty() -> Self {
        Self("empty".to_owned())
    }

    pub fn error() -> Self {
        Self("error".to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for UiPageState {
    fn default() -> Self {
        Self::initial()
    }
}

impl fmt::Display for UiPageState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for UiPageState {
    type Err = UiResponsiveIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl<'de> Deserialize<'de> for UiPageState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UiTargetProfile {
    logical_width: f32,
    logical_height: f32,
    width_class: UiWidthClass,
    height_class: UiHeightClass,
    orientation: UiOrientation,
    safe_area: UiSafeAreaClass,
    input_mode: UiDocumentInputMode,
    platform: UiDocumentPlatform,
}

impl UiTargetProfile {
    pub fn new(
        logical_width: f32,
        logical_height: f32,
        safe_area: UiSafeAreaClass,
        input_mode: UiDocumentInputMode,
        platform: UiDocumentPlatform,
    ) -> Result<Self, UiTargetProfileError> {
        if !logical_width.is_finite()
            || !logical_height.is_finite()
            || logical_width <= 0.0
            || logical_height <= 0.0
        {
            return Err(UiTargetProfileError {
                logical_width,
                logical_height,
            });
        }
        Ok(Self {
            logical_width,
            logical_height,
            width_class: document_width_class(runtime_width_class_for(logical_width)),
            height_class: document_height_class(runtime_height_class_for(logical_height)),
            orientation: document_orientation(runtime_orientation_for(
                logical_width,
                logical_height,
            )),
            safe_area,
            input_mode,
            platform,
        })
    }

    pub const fn logical_width(&self) -> f32 {
        self.logical_width
    }

    pub const fn logical_height(&self) -> f32 {
        self.logical_height
    }

    pub const fn width_class(&self) -> UiWidthClass {
        self.width_class
    }

    pub const fn height_class(&self) -> UiHeightClass {
        self.height_class
    }

    pub const fn orientation(&self) -> UiOrientation {
        self.orientation
    }

    pub const fn safe_area(&self) -> UiSafeAreaClass {
        self.safe_area
    }

    pub const fn input_mode(&self) -> UiDocumentInputMode {
        self.input_mode
    }

    pub const fn platform(&self) -> UiDocumentPlatform {
        self.platform
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiTargetProfileError {
    pub logical_width: f32,
    pub logical_height: f32,
}

impl fmt::Display for UiTargetProfileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "logical viewport dimensions must be finite and positive; received {}x{}",
            self.logical_width, self.logical_height
        )
    }
}

impl std::error::Error for UiTargetProfileError {}

const fn document_width_class(value: RuntimeWidthClass) -> UiWidthClass {
    match value {
        RuntimeWidthClass::Compact => UiWidthClass::Compact,
        RuntimeWidthClass::Medium => UiWidthClass::Medium,
        RuntimeWidthClass::Expanded => UiWidthClass::Expanded,
    }
}

const fn runtime_width_class(value: UiWidthClass) -> RuntimeWidthClass {
    match value {
        UiWidthClass::Compact => RuntimeWidthClass::Compact,
        UiWidthClass::Medium => RuntimeWidthClass::Medium,
        UiWidthClass::Expanded => RuntimeWidthClass::Expanded,
    }
}

const fn document_height_class(value: RuntimeHeightClass) -> UiHeightClass {
    match value {
        RuntimeHeightClass::Short => UiHeightClass::Short,
        RuntimeHeightClass::Regular => UiHeightClass::Regular,
        RuntimeHeightClass::Tall => UiHeightClass::Tall,
    }
}

const fn runtime_height_class(value: UiHeightClass) -> RuntimeHeightClass {
    match value {
        UiHeightClass::Short => RuntimeHeightClass::Short,
        UiHeightClass::Regular => RuntimeHeightClass::Regular,
        UiHeightClass::Tall => RuntimeHeightClass::Tall,
    }
}

const fn document_orientation(value: RuntimeOrientation) -> UiOrientation {
    match value {
        RuntimeOrientation::Portrait => UiOrientation::Portrait,
        RuntimeOrientation::Landscape => UiOrientation::Landscape,
    }
}

const fn runtime_orientation(value: UiOrientation) -> RuntimeOrientation {
    match value {
        UiOrientation::Portrait => RuntimeOrientation::Portrait,
        UiOrientation::Landscape => RuntimeOrientation::Landscape,
    }
}

impl UiResponsiveCondition {
    pub fn specificity(&self) -> u8 {
        [
            self.width_class.is_some(),
            self.height_class.is_some(),
            self.orientation.is_some(),
            self.safe_area.is_some(),
            self.input_mode.is_some(),
            self.platform.is_some(),
        ]
        .into_iter()
        .filter(|present| *present)
        .count() as u8
    }

    pub fn matches(&self, profile: &UiTargetProfile) -> bool {
        self.width_class
            .is_none_or(|value| value == profile.width_class)
            && self
                .height_class
                .is_none_or(|value| value == profile.height_class)
            && self
                .orientation
                .is_none_or(|value| value == profile.orientation)
            && self
                .safe_area
                .is_none_or(|value| value == profile.safe_area)
            && self
                .input_mode
                .is_none_or(|value| value == profile.input_mode)
            && self.platform.is_none_or(|value| value == profile.platform)
    }

    fn compatible_with(&self, other: &Self) -> bool {
        let Some(width_class) = intersection(self.width_class, other.width_class) else {
            return false;
        };
        let Some(height_class) = intersection(self.height_class, other.height_class) else {
            return false;
        };
        let Some(orientation) = intersection(self.orientation, other.orientation) else {
            return false;
        };
        intersection(self.safe_area, other.safe_area).is_some()
            && intersection(self.input_mode, other.input_mode).is_some()
            && intersection(self.platform, other.platform).is_some()
            && geometry_is_satisfiable(width_class, height_class, orientation)
    }

    fn is_satisfiable(&self) -> bool {
        geometry_is_satisfiable(self.width_class, self.height_class, self.orientation)
    }
}

fn intersection<T: Copy + Eq>(left: Option<T>, right: Option<T>) -> Option<Option<T>> {
    match (left, right) {
        (Some(left), Some(right)) if left != right => None,
        (Some(value), _) | (_, Some(value)) => Some(Some(value)),
        (None, None) => Some(None),
    }
}

fn geometry_is_satisfiable(
    width_class: Option<UiWidthClass>,
    height_class: Option<UiHeightClass>,
    orientation: Option<UiOrientation>,
) -> bool {
    responsive_classes_are_satisfiable(
        width_class.map(runtime_width_class),
        height_class.map(runtime_height_class),
        orientation.map(runtime_orientation),
    )
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiAppliedOverrideSource {
    Responsive,
    State,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UiAppliedOverride {
    pub source: UiAppliedOverrideSource,
    pub source_id: String,
    pub source_order: usize,
    pub priority: i16,
    pub specificity: u8,
    pub node_id: UiNodeId,
    pub fields: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UiEffectiveDocument {
    pub source_document_id: super::UiDocumentId,
    pub target_profile: UiTargetProfile,
    pub active_state: UiPageState,
    pub applied_overrides: Vec<UiAppliedOverride>,
    pub document: UiDocument,
}

impl UiEffectiveDocument {
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        let mut output = serde_json::to_string_pretty(self)?;
        output.push('\n');
        Ok(output)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UiEffectiveDocumentError {
    StateNotFound { state: UiPageState },
    InvalidEffectiveDocument { source: UiDocumentError },
}

impl UiEffectiveDocumentError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::StateNotFound { .. } => "UI_PAGE_STATE_NOT_FOUND",
            Self::InvalidEffectiveDocument { .. } => "UI_EFFECTIVE_DOCUMENT_INVALID",
        }
    }
}

impl fmt::Display for UiEffectiveDocumentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StateNotFound { state } => {
                write!(
                    formatter,
                    "page state `{state}` is not declared by the document"
                )
            }
            Self::InvalidEffectiveDocument { source } => {
                write!(
                    formatter,
                    "effective document is invalid after overrides: {source}"
                )
            }
        }
    }
}

impl std::error::Error for UiEffectiveDocumentError {}

impl ValidatedUiDocument {
    pub fn effective_document(
        &self,
        profile: &UiTargetProfile,
        active_state: &UiPageState,
    ) -> Result<UiEffectiveDocument, UiEffectiveDocumentError> {
        let source = self.document();
        let state = source
            .states
            .iter()
            .enumerate()
            .find(|(_, state)| state.id == *active_state);
        if state.is_none() && active_state.as_str() != "initial" {
            return Err(UiEffectiveDocumentError::StateNotFound {
                state: active_state.clone(),
            });
        }

        let mut effective = source.clone();
        let mut applied_overrides = Vec::new();
        let mut matching = source
            .responsive
            .iter()
            .enumerate()
            .filter(|(_, variant)| variant.when.matches(profile))
            .collect::<Vec<_>>();
        matching
            .sort_by_key(|(order, variant)| (variant.priority, variant.when.specificity(), *order));
        for (source_order, variant) in matching {
            apply_overrides(
                &mut effective.root,
                &variant.overrides,
                UiAppliedOverrideSource::Responsive,
                variant.id.as_str(),
                source_order,
                variant.priority,
                variant.when.specificity(),
                &mut applied_overrides,
            );
        }
        if let Some((source_order, state)) = state {
            apply_overrides(
                &mut effective.root,
                &state.overrides,
                UiAppliedOverrideSource::State,
                state.id.as_str(),
                source_order,
                0,
                0,
                &mut applied_overrides,
            );
        }
        effective.responsive.clear();
        effective.states.clear();
        let effective = ValidatedUiDocument::new(effective)
            .map_err(|source| UiEffectiveDocumentError::InvalidEffectiveDocument { source })?
            .into_document();
        Ok(UiEffectiveDocument {
            source_document_id: source.document_id.clone(),
            target_profile: *profile,
            active_state: active_state.clone(),
            applied_overrides,
            document: effective,
        })
    }
}

fn apply_overrides(
    root: &mut UiNode,
    overrides: &[UiNodeOverride],
    source: UiAppliedOverrideSource,
    source_id: &str,
    source_order: usize,
    priority: i16,
    specificity: u8,
    evidence: &mut Vec<UiAppliedOverride>,
) {
    for node_override in overrides {
        let Some(node) = find_node_mut(root, &node_override.node_id) else {
            continue;
        };
        if let Some(layout) = &node_override.set.layout {
            apply_layout_patch(node_layout_mut(node), layout);
        }
        if let Some(style) = &node_override.set.style {
            apply_style_patch(node_style_mut(node), style);
        }
        evidence.push(UiAppliedOverride {
            source: source.clone(),
            source_id: source_id.to_owned(),
            source_order,
            priority,
            specificity,
            node_id: node_override.node_id.clone(),
            fields: patch_writes(node_override).into_keys().collect(),
        });
    }
}

fn find_node_mut<'a>(node: &'a mut UiNode, id: &UiNodeId) -> Option<&'a mut UiNode> {
    if node.id() == id {
        return Some(node);
    }
    match node {
        UiNode::Container { children, .. } => children
            .iter_mut()
            .find_map(|child| find_node_mut(child, id)),
        UiNode::Button { component, .. }
        | UiNode::TextInput { component, .. }
        | UiNode::Checkbox { component, .. }
        | UiNode::Toggle { component, .. }
        | UiNode::Segmented { component, .. }
        | UiNode::Slider { component, .. }
        | UiNode::Stepper { component, .. }
        | UiNode::Scroll { component, .. }
        | UiNode::Modal { component, .. }
        | UiNode::ImageButton { component, .. }
        | UiNode::Badge { component, .. }
        | UiNode::Progress { component, .. }
        | UiNode::Tab { component, .. }
        | UiNode::Tooltip { component, .. }
        | UiNode::Select { component, .. } => component
            .children
            .iter_mut()
            .find_map(|child| find_node_mut(child, id)),
        _ => None,
    }
}

fn node_layout_mut(node: &mut UiNode) -> &mut UiLayout {
    match node {
        UiNode::Container { layout, .. }
        | UiNode::Text { layout, .. }
        | UiNode::Image { layout, .. }
        | UiNode::Icon { layout, .. }
        | UiNode::Spacer { layout, .. }
        | UiNode::Button { layout, .. }
        | UiNode::TextInput { layout, .. }
        | UiNode::Checkbox { layout, .. }
        | UiNode::Toggle { layout, .. }
        | UiNode::Segmented { layout, .. }
        | UiNode::Slider { layout, .. }
        | UiNode::Stepper { layout, .. }
        | UiNode::Scroll { layout, .. }
        | UiNode::Modal { layout, .. }
        | UiNode::ImageButton { layout, .. }
        | UiNode::Badge { layout, .. }
        | UiNode::Progress { layout, .. }
        | UiNode::Tab { layout, .. }
        | UiNode::Tooltip { layout, .. }
        | UiNode::Select { layout, .. } => layout,
    }
}

fn node_style_mut(node: &mut UiNode) -> &mut UiStyle {
    match node {
        UiNode::Container { style, .. }
        | UiNode::Text { style, .. }
        | UiNode::Image { style, .. }
        | UiNode::Icon { style, .. }
        | UiNode::Spacer { style, .. }
        | UiNode::Button { style, .. }
        | UiNode::TextInput { style, .. }
        | UiNode::Checkbox { style, .. }
        | UiNode::Toggle { style, .. }
        | UiNode::Segmented { style, .. }
        | UiNode::Slider { style, .. }
        | UiNode::Stepper { style, .. }
        | UiNode::Scroll { style, .. }
        | UiNode::Modal { style, .. }
        | UiNode::ImageButton { style, .. }
        | UiNode::Badge { style, .. }
        | UiNode::Progress { style, .. }
        | UiNode::Tab { style, .. }
        | UiNode::Tooltip { style, .. }
        | UiNode::Select { style, .. } => style,
    }
}

fn apply_layout_patch(target: &mut UiLayout, patch: &UiLayoutPatch) {
    macro_rules! apply {
        ($($field:ident),+ $(,)?) => {
            $(if let Some(value) = &patch.$field { target.$field = value.clone(); })+
        };
    }
    apply!(
        display,
        position,
        direction,
        width,
        height,
        min_width,
        min_height,
        max_width,
        max_height,
        margin,
        padding,
        border,
        gap,
        align_items,
        justify_items,
        align_self,
        justify_self,
        align_content,
        justify_content,
        wrap,
        flex_grow,
        flex_shrink,
        flex_basis,
        overflow,
        scrollbar_width,
        z_index,
        grid_columns,
        grid_rows,
        grid_auto_columns,
        grid_auto_rows,
        grid_auto_flow,
        grid_column,
        grid_row,
    );
    if let Some(value) = patch.aspect_ratio {
        target.aspect_ratio = Some(value);
    }
    if let Some(value) = patch.row_gap {
        target.row_gap = Some(value);
    }
    if let Some(value) = patch.column_gap {
        target.column_gap = Some(value);
    }
}

fn apply_style_patch(target: &mut UiStyle, patch: &UiStylePatch) {
    if let Some(component) = &patch.component {
        target.component = Some(component.clone());
    }
    if let Some(role) = &patch.role {
        target.role = Some(role.clone());
    }
    if let Some(text_role) = &patch.text_role {
        target.text_role = Some(text_role.clone());
    }
    if let Some(inline) = &patch.inline {
        target.inline.merge_from(inline);
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct UiResponsiveStateError {
    pub code: &'static str,
    pub path: String,
    pub related_path: Option<String>,
    pub node_id: Option<UiNodeId>,
    pub field: Option<String>,
}

pub(crate) fn validate_responsive_state_document(
    document: &UiDocument,
    node_paths: &BTreeMap<UiNodeId, String>,
) -> Vec<UiResponsiveStateError> {
    let mut errors = Vec::new();
    let mut state_ids = BTreeMap::new();
    for (state_index, state) in document.states.iter().enumerate() {
        let path = format!("$.states[{state_index}]");
        if let Some(first) = state_ids.insert(state.id.clone(), path.clone()) {
            errors.push(error(
                "UI_PAGE_STATE_DUPLICATE",
                format!("{path}.id"),
                Some(format!("{first}.id")),
                None,
                None,
            ));
        }
        validate_override_group(
            &state.overrides,
            &format!("{path}.overrides"),
            node_paths,
            &mut errors,
        );
    }

    let mut variant_ids = BTreeMap::new();
    for (variant_index, variant) in document.responsive.iter().enumerate() {
        let path = format!("$.responsive[{variant_index}]");
        if let Some(first) = variant_ids.insert(variant.id.clone(), path.clone()) {
            errors.push(error(
                "UI_RESPONSIVE_VARIANT_DUPLICATE",
                format!("{path}.id"),
                Some(format!("{first}.id")),
                None,
                None,
            ));
        }
        if variant.when.specificity() == 0 {
            errors.push(error(
                "UI_RESPONSIVE_CONDITION_EMPTY",
                format!("{path}.when"),
                None,
                None,
                None,
            ));
        }
        if !variant.when.is_satisfiable() {
            errors.push(error(
                "UI_RESPONSIVE_CONDITION_UNSATISFIABLE",
                format!("{path}.when"),
                None,
                None,
                None,
            ));
        }
        if variant.priority.unsigned_abs() > UI_RESPONSIVE_MAX_ABS_PRIORITY as u16 {
            errors.push(error(
                "UI_RESPONSIVE_PRIORITY_OUT_OF_RANGE",
                format!("{path}.priority"),
                None,
                None,
                None,
            ));
        }
        validate_override_group(
            &variant.overrides,
            &format!("{path}.overrides"),
            node_paths,
            &mut errors,
        );
    }

    for left_index in 0..document.responsive.len() {
        let left = &document.responsive[left_index];
        for right_index in left_index + 1..document.responsive.len() {
            let right = &document.responsive[right_index];
            if left.priority != right.priority
                || left.when.specificity() != right.when.specificity()
                || !left.when.compatible_with(&right.when)
            {
                continue;
            }
            let left_writes = group_writes(
                &left.overrides,
                &format!("$.responsive[{left_index}].overrides"),
            );
            let right_writes = group_writes(
                &right.overrides,
                &format!("$.responsive[{right_index}].overrides"),
            );
            push_conflicts(&left_writes, &right_writes, &mut errors);
        }
    }
    errors
}

fn validate_override_group(
    overrides: &[UiNodeOverride],
    path: &str,
    node_paths: &BTreeMap<UiNodeId, String>,
    errors: &mut Vec<UiResponsiveStateError>,
) {
    let writes = group_writes(overrides, path);
    for (index, node_override) in overrides.iter().enumerate() {
        let override_path = format!("{path}[{index}]");
        if !node_paths.contains_key(&node_override.node_id) {
            errors.push(error(
                "UI_OVERRIDE_NODE_NOT_FOUND",
                format!("{override_path}.node_id"),
                None,
                Some(node_override.node_id.clone()),
                None,
            ));
        }
        if patch_writes(node_override).is_empty() {
            errors.push(error(
                "UI_OVERRIDE_PATCH_EMPTY",
                format!("{override_path}.set"),
                None,
                Some(node_override.node_id.clone()),
                None,
            ));
        }
    }
    push_conflicts(&writes, &writes, errors);
}

#[derive(Clone)]
struct Write {
    path: String,
    value: Value,
}

type GroupWrites = BTreeMap<(UiNodeId, String), Vec<Write>>;

fn group_writes(overrides: &[UiNodeOverride], path: &str) -> GroupWrites {
    let mut writes = BTreeMap::new();
    for (index, node_override) in overrides.iter().enumerate() {
        for (field, value) in patch_writes(node_override) {
            writes
                .entry((node_override.node_id.clone(), field.clone()))
                .or_insert_with(Vec::new)
                .push(Write {
                    path: format!("{path}[{index}].set.{field}"),
                    value,
                });
        }
    }
    writes
}

fn push_conflicts(
    left: &GroupWrites,
    right: &GroupWrites,
    errors: &mut Vec<UiResponsiveStateError>,
) {
    let mut seen = BTreeSet::new();
    for ((node_id, field), left_writes) in left {
        let Some(right_writes) = right.get(&(node_id.clone(), field.clone())) else {
            continue;
        };
        for left_write in left_writes {
            for right_write in right_writes {
                if left_write.path == right_write.path || left_write.value == right_write.value {
                    continue;
                }
                let ordered = if left_write.path < right_write.path {
                    (&left_write.path, &right_write.path)
                } else {
                    (&right_write.path, &left_write.path)
                };
                if seen.insert((ordered.0.clone(), ordered.1.clone())) {
                    errors.push(error(
                        "UI_OVERRIDE_FIELD_CONFLICT",
                        ordered.1.clone(),
                        Some(ordered.0.clone()),
                        Some(node_id.clone()),
                        Some(field.clone()),
                    ));
                }
            }
        }
    }
}

fn patch_writes(node_override: &UiNodeOverride) -> BTreeMap<String, Value> {
    let mut writes = BTreeMap::new();
    if let Some(layout) = &node_override.set.layout {
        macro_rules! write_fields {
            ($($field:ident),+ $(,)?) => {
                $(if let Some(value) = &layout.$field {
                    writes.insert(
                        format!("layout.{}", stringify!($field)),
                        serde_json::to_value(value).expect("document patch values serialize"),
                    );
                })+
            };
        }
        write_fields!(
            display,
            position,
            direction,
            width,
            height,
            min_width,
            min_height,
            max_width,
            max_height,
            aspect_ratio,
            margin,
            padding,
            border,
            gap,
            row_gap,
            column_gap,
            align_items,
            justify_items,
            align_self,
            justify_self,
            align_content,
            justify_content,
            wrap,
            flex_grow,
            flex_shrink,
            flex_basis,
            overflow,
            scrollbar_width,
            z_index,
            grid_columns,
            grid_rows,
            grid_auto_columns,
            grid_auto_rows,
            grid_auto_flow,
            grid_column,
            grid_row,
        );
    }
    if let Some(style) = &node_override.set.style {
        for (field, value) in [
            ("style.component", style.component.as_ref().map(to_value)),
            ("style.role", style.role.as_ref().map(to_value)),
            ("style.text_role", style.text_role.as_ref().map(to_value)),
        ] {
            if let Some(value) = value {
                writes.insert(field.to_owned(), value);
            }
        }
        if let Some(inline) = &style.inline {
            for (field, value) in [
                ("background", inline.background.as_ref().map(to_value)),
                ("border", inline.border.as_ref().map(to_value)),
                ("corner_radius", inline.corner_radius.as_ref().map(to_value)),
                ("opacity", inline.opacity.as_ref().map(to_value)),
                ("shadows", inline.shadows.as_ref().map(to_value)),
                ("material", inline.material.as_ref().map(to_value)),
            ] {
                if let Some(value) = value {
                    writes.insert(format!("style.inline.{field}"), value);
                }
            }
            if let Some(text) = &inline.text {
                for (field, value) in [
                    ("color", text.color.as_ref().map(to_value)),
                    ("font", text.font.as_ref().map(to_value)),
                    ("font_size", text.font_size.as_ref().map(to_value)),
                    ("line_height", text.line_height.as_ref().map(to_value)),
                    ("letter_spacing", text.letter_spacing.as_ref().map(to_value)),
                    ("weight", text.weight.as_ref().map(to_value)),
                ] {
                    if let Some(value) = value {
                        writes.insert(format!("style.inline.text.{field}"), value);
                    }
                }
            }
        }
    }
    writes
}

fn to_value<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).expect("document patch values serialize")
}

fn error(
    code: &'static str,
    path: String,
    related_path: Option<String>,
    node_id: Option<UiNodeId>,
    field: Option<String>,
) -> UiResponsiveStateError {
    UiResponsiveStateError {
        code,
        path,
        related_path,
        node_id,
        field,
    }
}
