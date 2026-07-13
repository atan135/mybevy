use super::{UiActionId, UiAssetId, UiDocumentId, UiLayout, UiNodeId, UiStyleId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiAssetEntry {
    pub kind: UiAssetKind,
    pub source: UiAssetSource,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UiAssetKind {
    Image,
    Font,
    Icon,
    Atlas,
    Material,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum UiAssetSource {
    Packaged { path: String },
    ContentCache { logical_id: String },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum UiTokenValue {
    Color { value: String },
    Number { value: i64 },
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
        layout: UiLayout,
        #[serde(default)]
        style: UiStyle,
    },
    Image {
        id: UiNodeId,
        asset: UiAssetId,
        #[serde(default)]
        fit: UiImageFit,
        #[serde(default)]
        layout: UiLayout,
        #[serde(default)]
        style: UiStyle,
    },
    Button {
        id: UiNodeId,
        #[serde(default)]
        variant: UiButtonVariant,
        label: UiTextContent,
        on_click: UiActionInvocation,
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
            | Self::Button { id, .. } => id,
        }
    }

    pub fn children(&self) -> &[UiNode] {
        match self {
            Self::Container { children, .. } => children,
            _ => &[],
        }
    }

    pub fn layout(&self) -> &UiLayout {
        match self {
            Self::Container { layout, .. }
            | Self::Text { layout, .. }
            | Self::Image { layout, .. }
            | Self::Button { layout, .. } => layout,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiTextContent {
    pub literal: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiActionInvocation {
    pub action: UiActionId,
    #[serde(default)]
    pub args: BTreeMap<String, Value>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UiButtonVariant {
    #[default]
    Primary,
    Secondary,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UiImageFit {
    #[default]
    Contain,
    Cover,
    Stretch,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiStyle {
    #[serde(default)]
    pub role: Option<UiStyleId>,
    #[serde(default)]
    pub text_role: Option<UiStyleId>,
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

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiStylePatch {
    #[serde(default)]
    pub role: Option<UiStyleId>,
    #[serde(default)]
    pub text_role: Option<UiStyleId>,
}
