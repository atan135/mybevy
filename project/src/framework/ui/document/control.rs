use super::{
    UiAssetId, UiColor, UiImagePresentation, UiNode, UiNodeId, UiStyle, UiTextContent,
    default_image_tint,
};
use crate::framework::ui::{
    core::UiPanelKind,
    widgets::{
        UiControlFlags, UiControlKind, UiControlState, UiScrollViewConfig, UiSlider, UiStepper,
        resolve_control_state,
    },
};
use bevy::prelude::{Interaction, Val};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const UI_CONTROL_MAX_OPTIONS: usize = 64;
pub const UI_CONTROL_MAX_TEXT_INPUT_CHARS: u32 = 4096;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UiComponentVariant {
    #[default]
    Default,
    Primary,
    Secondary,
    Destructive,
    Subtle,
    Outline,
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UiComponentSize {
    Small,
    #[default]
    Medium,
    Large,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UiComponentState {
    #[default]
    Normal,
    Hovered,
    Pressed,
    Focused,
    Selected,
    Disabled,
    Loading,
    Empty,
    Error,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UiControlSlot {
    Label,
    Leading,
    Trailing,
    Placeholder,
    Helper,
    Empty,
    Error,
    Title,
    Body,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum UiControlSlotContent {
    Text {
        content: UiTextContent,
    },
    Icon {
        asset: UiAssetId,
        #[serde(default = "default_image_tint")]
        tint: UiColor,
    },
    Image {
        asset: UiAssetId,
        #[serde(default)]
        presentation: UiImagePresentation,
        #[serde(default = "default_image_tint")]
        tint: UiColor,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiComponentSpec {
    #[serde(default)]
    pub variant: UiComponentVariant,
    #[serde(default)]
    pub size: UiComponentSize,
    #[serde(default = "default_component_states")]
    pub states: Vec<UiComponentState>,
    #[serde(default)]
    pub state_overrides: BTreeMap<UiComponentState, UiStyle>,
    #[serde(default)]
    pub slots: BTreeMap<UiControlSlot, UiControlSlotContent>,
    #[serde(default)]
    pub children: Vec<UiNode>,
}

impl Default for UiComponentSpec {
    fn default() -> Self {
        Self {
            variant: UiComponentVariant::Default,
            size: UiComponentSize::Medium,
            states: default_component_states(),
            state_overrides: BTreeMap::new(),
            slots: BTreeMap::new(),
            children: Vec::new(),
        }
    }
}

fn default_component_states() -> Vec<UiComponentState> {
    vec![UiComponentState::Normal]
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiControlOption {
    pub value: String,
    pub label: UiTextContent,
    #[serde(default)]
    pub disabled: bool,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UiTooltipToneSpec {
    #[default]
    Standard,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiControlFieldError {
    pub code: &'static str,
    pub path: String,
    pub node_id: UiNodeId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiWidgetVariantAdapter {
    Primary,
    Secondary,
    Document(UiComponentVariant),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct UiWidgetControlAdapter {
    pub kind: UiControlKind,
    pub variant: UiWidgetVariantAdapter,
    pub size: UiComponentSize,
    pub state: UiControlState,
    pub flags: UiControlFlags,
    pub slider: Option<UiSlider>,
    pub stepper: Option<UiStepper>,
    pub scroll: Option<UiScrollViewConfig>,
    pub panel_kind: Option<UiPanelKind>,
}

impl UiComponentState {
    const fn to_widget(self) -> UiControlState {
        match self {
            Self::Normal => UiControlState::Normal,
            Self::Hovered => UiControlState::Hovered,
            Self::Pressed => UiControlState::Pressed,
            Self::Focused => UiControlState::Focused,
            Self::Selected => UiControlState::Selected,
            Self::Disabled => UiControlState::Disabled,
            Self::Loading => UiControlState::Loading,
            Self::Empty => UiControlState::Empty,
            Self::Error => UiControlState::Error,
        }
    }
}

impl UiNode {
    pub fn component(&self) -> Option<&UiComponentSpec> {
        match self {
            Self::Button { component, .. }
            | Self::TextInput { component, .. }
            | Self::Checkbox { component, .. }
            | Self::Toggle { component, .. }
            | Self::Segmented { component, .. }
            | Self::Slider { component, .. }
            | Self::Stepper { component, .. }
            | Self::Scroll { component, .. }
            | Self::Modal { component, .. }
            | Self::ImageButton { component, .. }
            | Self::Badge { component, .. }
            | Self::Progress { component, .. }
            | Self::Tab { component, .. }
            | Self::Tooltip { component, .. }
            | Self::Select { component, .. } => Some(component),
            _ => None,
        }
    }

    pub(crate) fn widget_adapter(&self) -> Option<UiWidgetControlAdapter> {
        let component = self.component()?;
        let kind = control_kind(self)?;
        let flags = flags_from_states(&component.states);
        let interaction = if component.states.contains(&UiComponentState::Pressed) {
            Interaction::Pressed
        } else if component.states.contains(&UiComponentState::Hovered) {
            Interaction::Hovered
        } else {
            Interaction::None
        };
        let state = resolve_control_state(
            interaction,
            component.states.contains(&UiComponentState::Focused),
            flags,
        );
        let variant = match self {
            Self::Button {
                legacy_variant: Some(super::UiButtonVariant::Primary),
                ..
            } => UiWidgetVariantAdapter::Primary,
            Self::Button {
                legacy_variant: Some(super::UiButtonVariant::Secondary),
                ..
            } => UiWidgetVariantAdapter::Secondary,
            Self::Button { .. } | Self::ImageButton { .. }
                if matches!(
                    component.variant,
                    UiComponentVariant::Default | UiComponentVariant::Primary
                ) =>
            {
                UiWidgetVariantAdapter::Primary
            }
            Self::Button { .. } | Self::ImageButton { .. }
                if component.variant == UiComponentVariant::Secondary =>
            {
                UiWidgetVariantAdapter::Secondary
            }
            _ => UiWidgetVariantAdapter::Document(component.variant),
        };
        let scroll = match self {
            Self::Scroll {
                row_gap,
                max_height,
                block_lower,
                ..
            } => Some(UiScrollViewConfig {
                row_gap: *row_gap,
                max_height: max_height.map_or(Val::Auto, Val::Px),
                should_block_lower: *block_lower,
            }),
            _ => None,
        };
        let slider = match self {
            Self::Slider {
                value, min, max, ..
            } => Some(UiSlider::new(*value, *min, *max)),
            _ => None,
        };
        let stepper = match self {
            Self::Stepper {
                value,
                min,
                max,
                step,
                ..
            } => Some(UiStepper::new(*value, *min, *max, *step)),
            _ => None,
        };
        Some(UiWidgetControlAdapter {
            kind,
            variant,
            size: component.size,
            state,
            flags,
            slider,
            stepper,
            scroll,
            panel_kind: matches!(self, Self::Modal { .. }).then_some(UiPanelKind::Modal),
        })
    }
}

pub(crate) fn validate_control_node(
    node: &UiNode,
    path: &str,
    errors: &mut Vec<UiControlFieldError>,
) {
    let Some(component) = node.component() else {
        return;
    };
    let Some(kind) = control_kind(node) else {
        return;
    };
    debug_assert!(node.widget_adapter().is_some());
    let node_id = node.id();

    validate_variant(node, component, path, node_id, errors);
    validate_states(kind, component, path, node_id, errors);
    validate_slots(node, component, path, node_id, errors);
    validate_nesting(node, component, path, node_id, errors);
    validate_control_values(node, path, node_id, errors);
}

fn control_kind(node: &UiNode) -> Option<UiControlKind> {
    Some(match node {
        UiNode::Button { .. } => UiControlKind::Button,
        UiNode::TextInput { .. } => UiControlKind::TextInput,
        UiNode::Checkbox { .. } => UiControlKind::Checkbox,
        UiNode::Toggle { .. } => UiControlKind::Toggle,
        UiNode::Segmented { .. } => UiControlKind::Segmented,
        UiNode::Slider { .. } => UiControlKind::Slider,
        UiNode::Stepper { .. } => UiControlKind::Stepper,
        UiNode::Scroll { .. } => UiControlKind::Scroll,
        UiNode::Modal { .. } => UiControlKind::Modal,
        UiNode::ImageButton { .. } => UiControlKind::ImageButton,
        UiNode::Badge { .. } => UiControlKind::Badge,
        UiNode::Progress { .. } => UiControlKind::Progress,
        UiNode::Tab { .. } => UiControlKind::Tab,
        UiNode::Tooltip { .. } => UiControlKind::Tooltip,
        UiNode::Select { .. } => UiControlKind::Dropdown,
        _ => return None,
    })
}

fn validate_variant(
    node: &UiNode,
    component: &UiComponentSpec,
    path: &str,
    node_id: &UiNodeId,
    errors: &mut Vec<UiControlFieldError>,
) {
    let valid = match node {
        UiNode::Button { .. } | UiNode::ImageButton { .. } => matches!(
            component.variant,
            UiComponentVariant::Default
                | UiComponentVariant::Primary
                | UiComponentVariant::Secondary
                | UiComponentVariant::Destructive
        ),
        UiNode::Badge { .. } | UiNode::Progress { .. } => matches!(
            component.variant,
            UiComponentVariant::Default
                | UiComponentVariant::Info
                | UiComponentVariant::Success
                | UiComponentVariant::Warning
                | UiComponentVariant::Error
        ),
        UiNode::Tab { .. } => matches!(
            component.variant,
            UiComponentVariant::Default
                | UiComponentVariant::Primary
                | UiComponentVariant::Secondary
                | UiComponentVariant::Subtle
        ),
        _ => component.variant == UiComponentVariant::Default,
    };
    if !valid {
        push_error(
            errors,
            "UI_CONTROL_VARIANT_UNSUPPORTED",
            &format!("{path}.component.variant"),
            node_id,
        );
    }
}

fn validate_states(
    kind: UiControlKind,
    component: &UiComponentSpec,
    path: &str,
    node_id: &UiNodeId,
    errors: &mut Vec<UiControlFieldError>,
) {
    if component.states.is_empty() {
        push_error(
            errors,
            "UI_CONTROL_STATE_REQUIRED",
            &format!("{path}.component.states"),
            node_id,
        );
    }
    let mut seen = BTreeSet::new();
    for (index, state) in component.states.iter().copied().enumerate() {
        if !seen.insert(state) {
            push_error(
                errors,
                "UI_CONTROL_STATE_DUPLICATE",
                &format!("{path}.component.states[{index}]"),
                node_id,
            );
        }
        if !kind.supports_state(state.to_widget()) {
            push_error(
                errors,
                "UI_CONTROL_STATE_UNSUPPORTED",
                &format!("{path}.component.states[{index}]"),
                node_id,
            );
        }
    }
    if component.states.len() > 1 && component.states.contains(&UiComponentState::Normal) {
        push_error(
            errors,
            "UI_CONTROL_STATE_NORMAL_CONFLICT",
            &format!("{path}.component.states"),
            node_id,
        );
    }
    for state in component.state_overrides.keys().copied() {
        if !kind.supports_state(state.to_widget()) {
            push_error(
                errors,
                "UI_CONTROL_STATE_OVERRIDE_UNSUPPORTED",
                &format!("{path}.component.state_overrides.{state}"),
                node_id,
            );
        }
    }
}

fn validate_slots(
    node: &UiNode,
    component: &UiComponentSpec,
    path: &str,
    node_id: &UiNodeId,
    errors: &mut Vec<UiControlFieldError>,
) {
    for (slot, content) in &component.slots {
        if !allowed_slots(node).contains(slot) {
            push_error(
                errors,
                "UI_CONTROL_SLOT_UNSUPPORTED",
                &format!("{path}.component.slots.{slot}"),
                node_id,
            );
            continue;
        }
        let expects_icon = matches!(slot, UiControlSlot::Leading | UiControlSlot::Trailing);
        let valid_content = if expects_icon {
            matches!(content, UiControlSlotContent::Icon { .. })
        } else {
            matches!(content, UiControlSlotContent::Text { .. })
        };
        if !valid_content {
            push_error(
                errors,
                "UI_CONTROL_SLOT_KIND_MISMATCH",
                &format!("{path}.component.slots.{slot}"),
                node_id,
            );
        }
    }

    let label_required = !matches!(
        node,
        UiNode::Scroll { .. } | UiNode::Modal { .. } | UiNode::Tooltip { .. }
    );
    let legacy_label = matches!(node, UiNode::Button { label: Some(_), .. });
    let slot_label = component.slots.contains_key(&UiControlSlot::Label);
    if label_required && !legacy_label && !slot_label {
        push_error(
            errors,
            "UI_CONTROL_LABEL_REQUIRED",
            &format!("{path}.component.slots.label"),
            node_id,
        );
    }
    if legacy_label && slot_label {
        push_error(
            errors,
            "UI_CONTROL_LABEL_DUPLICATE",
            &format!("{path}.component.slots.label"),
            node_id,
        );
    }
    if matches!(node, UiNode::Modal { .. }) {
        for required in [UiControlSlot::Title, UiControlSlot::Body] {
            if !component.slots.contains_key(&required) {
                push_error(
                    errors,
                    "UI_CONTROL_SLOT_REQUIRED",
                    &format!("{path}.component.slots.{required}"),
                    node_id,
                );
            }
        }
    }
    if matches!(node, UiNode::Tooltip { .. }) && !component.slots.contains_key(&UiControlSlot::Body)
    {
        push_error(
            errors,
            "UI_CONTROL_SLOT_REQUIRED",
            &format!("{path}.component.slots.body"),
            node_id,
        );
    }
}

fn allowed_slots(node: &UiNode) -> &'static [UiControlSlot] {
    use UiControlSlot as Slot;
    match node {
        UiNode::Button { .. } => &[Slot::Label, Slot::Leading, Slot::Trailing],
        UiNode::ImageButton { .. } => &[Slot::Label],
        UiNode::TextInput { .. } => &[Slot::Label, Slot::Placeholder, Slot::Helper, Slot::Error],
        UiNode::Checkbox { .. } | UiNode::Toggle { .. } => &[Slot::Label],
        UiNode::Segmented { .. } | UiNode::Slider { .. } | UiNode::Stepper { .. } => {
            &[Slot::Label, Slot::Helper, Slot::Error]
        }
        UiNode::Scroll { .. } => &[Slot::Empty, Slot::Error],
        UiNode::Modal { .. } => &[Slot::Title, Slot::Body, Slot::Error],
        UiNode::Badge { .. } | UiNode::Progress { .. } => &[Slot::Label],
        UiNode::Tab { .. } => &[Slot::Label, Slot::Leading],
        UiNode::Tooltip { .. } => &[Slot::Body],
        UiNode::Select { .. } => &[Slot::Label, Slot::Placeholder, Slot::Empty, Slot::Error],
        _ => &[],
    }
}

fn validate_nesting(
    node: &UiNode,
    component: &UiComponentSpec,
    path: &str,
    node_id: &UiNodeId,
    errors: &mut Vec<UiControlFieldError>,
) {
    match node {
        UiNode::Scroll { .. } => {}
        UiNode::Tooltip { .. } if component.children.len() == 1 => {}
        UiNode::Tooltip { .. } => push_error(
            errors,
            "UI_CONTROL_TOOLTIP_TARGET_REQUIRED",
            &format!("{path}.component.children"),
            node_id,
        ),
        _ if !component.children.is_empty() => push_error(
            errors,
            "UI_CONTROL_NESTING_UNSUPPORTED",
            &format!("{path}.component.children"),
            node_id,
        ),
        _ => {}
    }
}

fn validate_control_values(
    node: &UiNode,
    path: &str,
    node_id: &UiNodeId,
    errors: &mut Vec<UiControlFieldError>,
) {
    match node {
        UiNode::TextInput {
            value, max_chars, ..
        } => {
            if max_chars.is_some_and(|value| value == 0 || value > UI_CONTROL_MAX_TEXT_INPUT_CHARS)
            {
                push_error(
                    errors,
                    "UI_CONTROL_VALUE_INVALID",
                    &format!("{path}.max_chars"),
                    node_id,
                );
            } else if max_chars.is_some_and(|max_chars| value.chars().count() > max_chars as usize)
            {
                push_error(
                    errors,
                    "UI_CONTROL_TEXT_INPUT_VALUE_TOO_LONG",
                    &format!("{path}.value"),
                    node_id,
                );
            }
        }
        UiNode::Checkbox {
            checked, component, ..
        } if *checked != component.states.contains(&UiComponentState::Selected) => push_error(
            errors,
            "UI_CONTROL_SELECTED_STATE_MISMATCH",
            &format!("{path}.component.states"),
            node_id,
        ),
        UiNode::Toggle { on, component, .. }
            if *on != component.states.contains(&UiComponentState::Selected) =>
        {
            push_error(
                errors,
                "UI_CONTROL_SELECTED_STATE_MISMATCH",
                &format!("{path}.component.states"),
                node_id,
            )
        }
        UiNode::Segmented {
            options, selected, ..
        }
        | UiNode::Select {
            options, selected, ..
        } => {
            validate_options(options, selected.as_deref(), path, node_id, errors);
        }
        UiNode::Slider {
            value, min, max, ..
        } => {
            if !value.is_finite()
                || !min.is_finite()
                || !max.is_finite()
                || min >= max
                || value < min
                || value > max
            {
                push_error(errors, "UI_CONTROL_RANGE_INVALID", path, node_id);
            }
        }
        UiNode::Stepper {
            value,
            min,
            max,
            step,
            ..
        } => {
            if min >= max || *step <= 0 || value < min || value > max {
                push_error(errors, "UI_CONTROL_RANGE_INVALID", path, node_id);
            }
        }
        UiNode::Scroll {
            row_gap,
            max_height,
            ..
        } => {
            if !row_gap.is_finite()
                || *row_gap < 0.0
                || max_height.is_some_and(|value| !value.is_finite() || value <= 0.0)
            {
                push_error(errors, "UI_CONTROL_SCROLL_CONFIG_INVALID", path, node_id);
            }
        }
        UiNode::Progress { value, .. } if !value.is_finite() || !(0.0..=1.0).contains(value) => {
            push_error(
                errors,
                "UI_CONTROL_VALUE_INVALID",
                &format!("{path}.value"),
                node_id,
            );
        }
        _ => {}
    }
}

fn validate_options(
    options: &[UiControlOption],
    selected: Option<&str>,
    path: &str,
    node_id: &UiNodeId,
    errors: &mut Vec<UiControlFieldError>,
) {
    if options.is_empty() || options.len() > UI_CONTROL_MAX_OPTIONS {
        push_error(
            errors,
            "UI_CONTROL_OPTIONS_INVALID",
            &format!("{path}.options"),
            node_id,
        );
    }
    let mut values = BTreeSet::new();
    for (index, option) in options.iter().enumerate() {
        if option.value.is_empty() || !values.insert(option.value.as_str()) {
            push_error(
                errors,
                "UI_CONTROL_OPTION_VALUE_INVALID",
                &format!("{path}.options[{index}].value"),
                node_id,
            );
        }
    }
    if selected.is_some_and(|selected| !values.contains(selected)) {
        push_error(
            errors,
            "UI_CONTROL_SELECTED_OPTION_UNKNOWN",
            &format!("{path}.selected"),
            node_id,
        );
    }
}

fn flags_from_states(states: &[UiComponentState]) -> UiControlFlags {
    UiControlFlags {
        selected: states.contains(&UiComponentState::Selected),
        disabled: states.contains(&UiComponentState::Disabled),
        loading: states.contains(&UiComponentState::Loading),
        empty: states.contains(&UiComponentState::Empty),
        error: states.contains(&UiComponentState::Error),
    }
}

fn push_error(
    errors: &mut Vec<UiControlFieldError>,
    code: &'static str,
    path: &str,
    node_id: &UiNodeId,
) {
    errors.push(UiControlFieldError {
        code,
        path: path.to_owned(),
        node_id: node_id.clone(),
    });
}

impl std::fmt::Display for UiComponentState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Normal => "normal",
            Self::Hovered => "hovered",
            Self::Pressed => "pressed",
            Self::Focused => "focused",
            Self::Selected => "selected",
            Self::Disabled => "disabled",
            Self::Loading => "loading",
            Self::Empty => "empty",
            Self::Error => "error",
        })
    }
}

impl std::fmt::Display for UiControlSlot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Label => "label",
            Self::Leading => "leading",
            Self::Trailing => "trailing",
            Self::Placeholder => "placeholder",
            Self::Helper => "helper",
            Self::Empty => "empty",
            Self::Error => "error",
            Self::Title => "title",
            Self::Body => "body",
        })
    }
}
