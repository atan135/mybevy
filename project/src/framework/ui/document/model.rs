use super::{
    UiActionId, UiActionValue, UiAssetEntry, UiAssetId, UiBindingDeclaration, UiBindingPath,
    UiColor, UiComponentSpec, UiControlOption, UiDocumentId, UiImageFailurePresentation,
    UiImagePresentation, UiLayout, UiNodeId, UiStyleDefinition, UiStyleId, UiStyleProperties,
    UiTextContent, UiTextTypography, UiTokenValue, UiTooltipToneSpec, default_image_tint,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const CURRENT_SCHEMA_VERSION: u32 = 1;
pub const MIN_SUPPORTED_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiDocument {
    #[cfg_attr(test, schemars(range(min = 1, max = 1)))]
    pub schema_version: u32,
    pub document_id: UiDocumentId,
    #[serde(default)]
    pub metadata: UiDocumentMetadata,
    #[serde(default)]
    pub assets: BTreeMap<UiAssetId, UiAssetEntry>,
    #[serde(default)]
    pub tokens: BTreeMap<UiStyleId, UiTokenValue>,
    #[serde(default)]
    pub styles: BTreeMap<UiStyleId, UiStyleDefinition>,
    #[serde(default)]
    pub bindings: BTreeMap<UiBindingPath, UiBindingDeclaration>,
    pub root: UiNode,
    #[serde(default)]
    pub states: Vec<UiStateDefinition>,
    #[serde(default)]
    pub responsive: Vec<UiResponsiveVariant>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiDocumentMetadata {
    #[serde(default)]
    pub title: String,
    #[serde(default = "default_budget_profile")]
    pub budget_profile: String,
}

impl Default for UiDocumentMetadata {
    fn default() -> Self {
        Self {
            title: String::new(),
            budget_profile: default_budget_profile(),
        }
    }
}

fn default_budget_profile() -> String {
    "mobile_baseline_v1".to_owned()
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum UiNode {
    Container {
        id: UiNodeId,
        #[serde(default)]
        layout: UiLayout,
        #[serde(default)]
        style: UiStyle,
        #[serde(default)]
        children: Vec<UiNode>,
    },
    Text {
        id: UiNodeId,
        content: UiTextContent,
        #[serde(default)]
        typography: UiTextTypography,
        #[serde(default)]
        layout: UiLayout,
        #[serde(default)]
        style: UiStyle,
    },
    Image {
        id: UiNodeId,
        asset: UiAssetId,
        #[serde(default)]
        presentation: UiImagePresentation,
        #[serde(default = "default_image_tint")]
        tint: UiColor,
        #[serde(default)]
        placeholder: Option<UiAssetId>,
        #[serde(default)]
        failure: UiImageFailurePresentation,
        #[serde(default)]
        layout: UiLayout,
        #[serde(default)]
        style: UiStyle,
    },
    Icon {
        id: UiNodeId,
        asset: UiAssetId,
        #[serde(default = "default_image_tint")]
        tint: UiColor,
        #[serde(default)]
        layout: UiLayout,
        #[serde(default)]
        style: UiStyle,
    },
    Spacer {
        id: UiNodeId,
        #[serde(default)]
        layout: UiLayout,
        #[serde(default)]
        style: UiStyle,
    },
    Button {
        id: UiNodeId,
        #[serde(default)]
        #[serde(rename = "variant")]
        legacy_variant: Option<UiButtonVariant>,
        #[serde(default)]
        component: UiComponentSpec,
        #[serde(default)]
        label: Option<UiTextContent>,
        on_click: UiActionInvocation,
        #[serde(default)]
        layout: UiLayout,
        #[serde(default)]
        style: UiStyle,
    },
    TextInput {
        id: UiNodeId,
        #[serde(default)]
        component: UiComponentSpec,
        #[serde(default)]
        value: String,
        #[serde(default)]
        max_chars: Option<u32>,
        #[serde(default)]
        readonly: bool,
        #[serde(default)]
        layout: UiLayout,
        #[serde(default)]
        style: UiStyle,
    },
    Checkbox {
        id: UiNodeId,
        #[serde(default)]
        component: UiComponentSpec,
        #[serde(default)]
        checked: bool,
        #[serde(default)]
        layout: UiLayout,
        #[serde(default)]
        style: UiStyle,
    },
    Toggle {
        id: UiNodeId,
        #[serde(default)]
        component: UiComponentSpec,
        #[serde(default)]
        on: bool,
        #[serde(default)]
        layout: UiLayout,
        #[serde(default)]
        style: UiStyle,
    },
    Segmented {
        id: UiNodeId,
        #[serde(default)]
        component: UiComponentSpec,
        #[serde(default)]
        options: Vec<UiControlOption>,
        #[serde(default)]
        selected: Option<String>,
        #[serde(default)]
        layout: UiLayout,
        #[serde(default)]
        style: UiStyle,
    },
    Slider {
        id: UiNodeId,
        #[serde(default)]
        component: UiComponentSpec,
        #[serde(default)]
        value: f32,
        #[serde(default)]
        min: f32,
        #[serde(default = "default_slider_max")]
        max: f32,
        #[serde(default)]
        layout: UiLayout,
        #[serde(default)]
        style: UiStyle,
    },
    Stepper {
        id: UiNodeId,
        #[serde(default)]
        component: UiComponentSpec,
        #[serde(default)]
        value: i32,
        #[serde(default)]
        min: i32,
        #[serde(default = "default_stepper_max")]
        max: i32,
        #[serde(default = "default_stepper_step")]
        step: i32,
        #[serde(default)]
        layout: UiLayout,
        #[serde(default)]
        style: UiStyle,
    },
    Scroll {
        id: UiNodeId,
        #[serde(default)]
        component: UiComponentSpec,
        #[serde(default)]
        row_gap: f32,
        #[serde(default)]
        max_height: Option<f32>,
        #[serde(default = "default_true")]
        block_lower: bool,
        #[serde(default)]
        layout: UiLayout,
        #[serde(default)]
        style: UiStyle,
    },
    Modal {
        id: UiNodeId,
        #[serde(default)]
        component: UiComponentSpec,
        #[serde(default)]
        cancellable: bool,
        #[serde(default)]
        layout: UiLayout,
        #[serde(default)]
        style: UiStyle,
    },
    ImageButton {
        id: UiNodeId,
        #[serde(default)]
        component: UiComponentSpec,
        asset: UiAssetId,
        #[serde(default)]
        presentation: UiImagePresentation,
        #[serde(default = "default_image_tint")]
        tint: UiColor,
        #[serde(default)]
        layout: UiLayout,
        #[serde(default)]
        style: UiStyle,
    },
    Badge {
        id: UiNodeId,
        #[serde(default)]
        component: UiComponentSpec,
        #[serde(default)]
        layout: UiLayout,
        #[serde(default)]
        style: UiStyle,
    },
    Progress {
        id: UiNodeId,
        #[serde(default)]
        component: UiComponentSpec,
        #[serde(default)]
        value: f32,
        #[serde(default)]
        layout: UiLayout,
        #[serde(default)]
        style: UiStyle,
    },
    Tab {
        id: UiNodeId,
        #[serde(default)]
        component: UiComponentSpec,
        #[serde(default)]
        value: String,
        #[serde(default)]
        layout: UiLayout,
        #[serde(default)]
        style: UiStyle,
    },
    Tooltip {
        id: UiNodeId,
        #[serde(default)]
        component: UiComponentSpec,
        #[serde(default)]
        tone: UiTooltipToneSpec,
        #[serde(default)]
        layout: UiLayout,
        #[serde(default)]
        style: UiStyle,
    },
    Select {
        id: UiNodeId,
        #[serde(default)]
        component: UiComponentSpec,
        #[serde(default)]
        options: Vec<UiControlOption>,
        #[serde(default)]
        selected: Option<String>,
        #[serde(default)]
        layout: UiLayout,
        #[serde(default)]
        style: UiStyle,
    },
}

impl UiNode {
    pub fn id(&self) -> &UiNodeId {
        match self {
            Self::Container { id, .. }
            | Self::Text { id, .. }
            | Self::Image { id, .. }
            | Self::Icon { id, .. }
            | Self::Spacer { id, .. }
            | Self::Button { id, .. }
            | Self::TextInput { id, .. }
            | Self::Checkbox { id, .. }
            | Self::Toggle { id, .. }
            | Self::Segmented { id, .. }
            | Self::Slider { id, .. }
            | Self::Stepper { id, .. }
            | Self::Scroll { id, .. }
            | Self::Modal { id, .. }
            | Self::ImageButton { id, .. }
            | Self::Badge { id, .. }
            | Self::Progress { id, .. }
            | Self::Tab { id, .. }
            | Self::Tooltip { id, .. }
            | Self::Select { id, .. } => id,
        }
    }

    pub fn children(&self) -> &[UiNode] {
        match self {
            Self::Container { children, .. } => children,
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
            | Self::Select { component, .. } => &component.children,
            _ => &[],
        }
    }

    pub(crate) fn child_path(&self, path: &str, index: usize) -> String {
        if matches!(self, Self::Container { .. }) {
            format!("{path}.children[{index}]")
        } else {
            format!("{path}.component.children[{index}]")
        }
    }

    pub fn layout(&self) -> &UiLayout {
        match self {
            Self::Container { layout, .. }
            | Self::Text { layout, .. }
            | Self::Image { layout, .. }
            | Self::Icon { layout, .. }
            | Self::Spacer { layout, .. }
            | Self::Button { layout, .. }
            | Self::TextInput { layout, .. }
            | Self::Checkbox { layout, .. }
            | Self::Toggle { layout, .. }
            | Self::Segmented { layout, .. }
            | Self::Slider { layout, .. }
            | Self::Stepper { layout, .. }
            | Self::Scroll { layout, .. }
            | Self::Modal { layout, .. }
            | Self::ImageButton { layout, .. }
            | Self::Badge { layout, .. }
            | Self::Progress { layout, .. }
            | Self::Tab { layout, .. }
            | Self::Tooltip { layout, .. }
            | Self::Select { layout, .. } => layout,
        }
    }

    pub fn style(&self) -> &UiStyle {
        match self {
            Self::Container { style, .. }
            | Self::Text { style, .. }
            | Self::Image { style, .. }
            | Self::Icon { style, .. }
            | Self::Spacer { style, .. }
            | Self::Button { style, .. }
            | Self::TextInput { style, .. }
            | Self::Checkbox { style, .. }
            | Self::Toggle { style, .. }
            | Self::Segmented { style, .. }
            | Self::Slider { style, .. }
            | Self::Stepper { style, .. }
            | Self::Scroll { style, .. }
            | Self::Modal { style, .. }
            | Self::ImageButton { style, .. }
            | Self::Badge { style, .. }
            | Self::Progress { style, .. }
            | Self::Tab { style, .. }
            | Self::Tooltip { style, .. }
            | Self::Select { style, .. } => style,
        }
    }
}

fn default_slider_max() -> f32 {
    1.0
}

fn default_stepper_max() -> i32 {
    100
}

fn default_stepper_step() -> i32 {
    1
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiActionInvocation {
    pub action: UiActionId,
    #[serde(default)]
    pub params: BTreeMap<String, UiActionValue>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UiButtonVariant {
    #[default]
    Primary,
    Secondary,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiStyle {
    #[serde(default)]
    pub component: Option<UiStyleId>,
    #[serde(default)]
    pub role: Option<UiStyleId>,
    #[serde(default)]
    pub text_role: Option<UiStyleId>,
    #[serde(default)]
    pub inline: UiStyleProperties,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiStateDefinition {
    pub id: String,
    #[serde(default)]
    pub overrides: Vec<UiNodeOverride>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiResponsiveVariant {
    pub id: String,
    pub when: UiResponsiveCondition,
    #[serde(default)]
    pub overrides: Vec<UiNodeOverride>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiResponsiveCondition {
    #[serde(default)]
    pub width_class: Option<UiWidthClass>,
    #[serde(default)]
    pub orientation: Option<UiOrientation>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UiWidthClass {
    Compact,
    Medium,
    Expanded,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UiOrientation {
    Portrait,
    Landscape,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiNodeOverride {
    pub node_id: UiNodeId,
    pub set: UiNodePatch,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiNodePatch {
    #[serde(default)]
    pub layout: Option<UiLayoutPatch>,
    #[serde(default)]
    pub style: Option<UiStylePatch>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiLayoutPatch {
    #[serde(default)]
    pub display: Option<super::UiDisplay>,
    #[serde(default)]
    pub position: Option<super::UiLayoutPosition>,
    #[serde(default)]
    pub direction: Option<super::UiFlexDirection>,
    #[serde(default)]
    pub width: Option<super::UiLength>,
    #[serde(default)]
    pub height: Option<super::UiLength>,
    #[serde(default)]
    pub min_width: Option<super::UiLength>,
    #[serde(default)]
    pub min_height: Option<super::UiLength>,
    #[serde(default)]
    pub max_width: Option<super::UiLength>,
    #[serde(default)]
    pub max_height: Option<super::UiLength>,
    #[serde(default)]
    pub aspect_ratio: Option<f32>,
    #[serde(default)]
    pub margin: Option<super::UiInsets>,
    #[serde(default)]
    pub padding: Option<super::UiInsets>,
    #[serde(default)]
    pub border: Option<super::UiInsets>,
    #[serde(default)]
    pub gap: Option<super::UiLength>,
    #[serde(default)]
    pub row_gap: Option<super::UiLength>,
    #[serde(default)]
    pub column_gap: Option<super::UiLength>,
    #[serde(default)]
    pub align_items: Option<super::UiAlignItems>,
    #[serde(default)]
    pub justify_items: Option<super::UiAlignItems>,
    #[serde(default)]
    pub align_self: Option<super::UiAlignSelf>,
    #[serde(default)]
    pub justify_self: Option<super::UiAlignSelf>,
    #[serde(default)]
    pub align_content: Option<super::UiContentAlignment>,
    #[serde(default)]
    pub justify_content: Option<super::UiContentAlignment>,
    #[serde(default)]
    pub wrap: Option<super::UiFlexWrap>,
    #[serde(default)]
    pub flex_grow: Option<f32>,
    #[serde(default)]
    pub flex_shrink: Option<f32>,
    #[serde(default)]
    pub flex_basis: Option<super::UiLength>,
    #[serde(default)]
    pub overflow: Option<super::UiOverflow>,
    #[serde(default)]
    pub scrollbar_width: Option<f32>,
    #[serde(default)]
    pub z_index: Option<i32>,
    #[serde(default)]
    pub grid_columns: Option<Vec<super::UiGridTrack>>,
    #[serde(default)]
    pub grid_rows: Option<Vec<super::UiGridTrack>>,
    #[serde(default)]
    pub grid_auto_columns: Option<Vec<super::UiGridTrackSize>>,
    #[serde(default)]
    pub grid_auto_rows: Option<Vec<super::UiGridTrackSize>>,
    #[serde(default)]
    pub grid_auto_flow: Option<super::UiGridAutoFlow>,
    #[serde(default)]
    pub grid_column: Option<super::UiGridPlacement>,
    #[serde(default)]
    pub grid_row: Option<super::UiGridPlacement>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiStylePatch {
    #[serde(default)]
    pub component: Option<UiStyleId>,
    #[serde(default)]
    pub role: Option<UiStyleId>,
    #[serde(default)]
    pub text_role: Option<UiStyleId>,
    #[serde(default)]
    pub inline: Option<UiStyleProperties>,
}
