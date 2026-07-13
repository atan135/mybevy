use super::{UiAssetId, UiColor, UiDocument, UiNode};
use bevy::prelude::{Justify, LineBreak, TextLayout};
use serde::{Deserialize, Serialize};

pub const UI_TEXT_FALLBACK_MAX_BYTES: usize = 4 * 1024;
pub const UI_TEXT_LITERAL_MAX_BYTES: usize = 16 * 1024;
pub const UI_TEXT_MAX_LINES: u16 = 128;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(untagged)]
pub enum UiTextContent {
    Literal(UiLiteralTextSource),
    I18n(UiI18nTextSource),
    Binding(UiBindingTextSource),
}

impl UiTextContent {
    pub fn format(&self) -> &UiTextFormat {
        match self {
            Self::Literal(source) => &source.format,
            Self::I18n(source) => &source.format,
            Self::Binding(source) => &source.format,
        }
    }

    pub fn fallback_text(&self) -> &str {
        match self {
            Self::Literal(source) => &source.literal,
            Self::I18n(source) => &source.fallback,
            Self::Binding(source) => &source.fallback,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiLiteralTextSource {
    pub literal: String,
    #[serde(default)]
    pub format: UiTextFormat,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiI18nTextSource {
    pub i18n_key: super::UiI18nKey,
    pub fallback: String,
    #[serde(default)]
    pub format: UiTextFormat,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiBindingTextSource {
    pub binding_path: super::UiBindingPath,
    #[serde(default)]
    pub fallback: String,
    #[serde(default)]
    pub format: UiTextFormat,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum UiTextFormat {
    #[default]
    Plain,
    Number {
        #[serde(default)]
        min_fraction_digits: u8,
        #[serde(default)]
        max_fraction_digits: u8,
        #[serde(default)]
        grouping: bool,
    },
    Percent {
        #[serde(default)]
        min_fraction_digits: u8,
        #[serde(default)]
        max_fraction_digits: u8,
    },
    Bytes {
        #[serde(default)]
        precision: u8,
        #[serde(default)]
        binary_units: bool,
    },
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UiTextFontRole {
    Display,
    Heading,
    #[default]
    Body,
    Caption,
    Control,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum UiTextLineHeight {
    #[default]
    Normal,
    Relative(f32),
    Pixels(f32),
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UiTextAlignment {
    #[default]
    Left,
    Center,
    Right,
    Justified,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UiTextWrap {
    Word,
    Character,
    #[default]
    WordOrCharacter,
    NoWrap,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UiTextOverflow {
    #[default]
    Visible,
    Clip,
    Ellipsis,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiTextTypography {
    #[serde(default)]
    pub font_role: UiTextFontRole,
    #[serde(default)]
    pub weight: super::UiTextWeight,
    #[serde(default)]
    pub line_height: UiTextLineHeight,
    #[serde(default)]
    pub alignment: UiTextAlignment,
    #[serde(default)]
    pub wrap: UiTextWrap,
    #[serde(default)]
    pub max_lines: Option<u16>,
    #[serde(default)]
    pub overflow: UiTextOverflow,
}

impl Default for UiTextTypography {
    fn default() -> Self {
        Self {
            font_role: UiTextFontRole::Body,
            weight: super::UiTextWeight::Regular,
            line_height: UiTextLineHeight::Normal,
            alignment: UiTextAlignment::Left,
            wrap: UiTextWrap::WordOrCharacter,
            max_lines: None,
            overflow: UiTextOverflow::Visible,
        }
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub(crate) struct UiTextTypographyAdapter {
    pub style: crate::framework::ui::style::UiTextStyleToken,
    pub bevy_layout: TextLayout,
    pub max_lines: Option<u16>,
    pub overflow: UiTextOverflow,
}

impl UiTextTypography {
    #[allow(dead_code)]
    pub(crate) fn to_framework_adapter(&self, font_size: f32) -> UiTextTypographyAdapter {
        use crate::framework::ui::style::{
            UiFontFamily, UiFontRole, UiFontWeight, UiTextLineHeight as FrameworkLineHeight,
            UiTextStyleToken, UiTextTruncation,
        };

        let font_role = match self.font_role {
            UiTextFontRole::Display => UiFontRole::Display,
            UiTextFontRole::Heading => UiFontRole::Heading,
            UiTextFontRole::Body => UiFontRole::Body,
            UiTextFontRole::Caption => UiFontRole::Caption,
            UiTextFontRole::Control => UiFontRole::Control,
        };
        let font_weight = match self.weight {
            super::UiTextWeight::Regular => UiFontWeight::Regular,
            super::UiTextWeight::Medium => UiFontWeight::Medium,
            super::UiTextWeight::Bold => UiFontWeight::Bold,
        };
        let line_height = match self.line_height {
            UiTextLineHeight::Normal => FrameworkLineHeight::Relative(1.2),
            UiTextLineHeight::Relative(value) => FrameworkLineHeight::Relative(value),
            UiTextLineHeight::Pixels(value) => FrameworkLineHeight::Pixels(value),
        };
        let alignment = match self.alignment {
            UiTextAlignment::Left => crate::framework::ui::style::UiTextAlignment::Left,
            UiTextAlignment::Center => crate::framework::ui::style::UiTextAlignment::Center,
            UiTextAlignment::Right => crate::framework::ui::style::UiTextAlignment::Right,
            UiTextAlignment::Justified => crate::framework::ui::style::UiTextAlignment::Justified,
        };
        let wrap = match self.wrap {
            UiTextWrap::Word => crate::framework::ui::style::UiTextWrap::Word,
            UiTextWrap::Character => crate::framework::ui::style::UiTextWrap::Character,
            UiTextWrap::WordOrCharacter => crate::framework::ui::style::UiTextWrap::WordOrCharacter,
            UiTextWrap::NoWrap => crate::framework::ui::style::UiTextWrap::NoWrap,
        };
        let style = UiTextStyleToken {
            font_role,
            font_family: UiFontFamily::ProductCjk,
            font_weight,
            font_size,
            line_height,
            alignment,
            wrap,
            truncation: match self.overflow {
                UiTextOverflow::Visible => UiTextTruncation::None,
                UiTextOverflow::Clip | UiTextOverflow::Ellipsis => UiTextTruncation::Clip,
            },
        };
        let bevy_layout = TextLayout::new(
            match self.alignment {
                UiTextAlignment::Left => Justify::Left,
                UiTextAlignment::Center => Justify::Center,
                UiTextAlignment::Right => Justify::Right,
                UiTextAlignment::Justified => Justify::Justified,
            },
            match self.wrap {
                UiTextWrap::Word => LineBreak::WordBoundary,
                UiTextWrap::Character => LineBreak::AnyCharacter,
                UiTextWrap::WordOrCharacter => LineBreak::WordOrCharacter,
                UiTextWrap::NoWrap => LineBreak::NoWrap,
            },
        );
        UiTextTypographyAdapter {
            style,
            bevy_layout,
            max_lines: self.max_lines,
            overflow: self.overflow,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum UiImageFailurePresentation {
    ErrorColor {
        #[serde(default = "default_image_failure_color")]
        color: UiColor,
    },
    Placeholder,
    Asset {
        asset: UiAssetId,
    },
    Hide,
}

impl Default for UiImageFailurePresentation {
    fn default() -> Self {
        Self::ErrorColor {
            color: default_image_failure_color(),
        }
    }
}

fn default_image_failure_color() -> UiColor {
    UiColor::from_rgba8(140, 31, 36, 255)
}

pub(crate) fn default_image_tint() -> UiColor {
    UiColor::from_rgba8(255, 255, 255, 255)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub(crate) enum UiImageContentState {
    Loading,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub(crate) enum UiResolvedImageFallback<'a> {
    Asset(&'a UiAssetId),
    Solid(UiColor),
    Hidden,
}

#[allow(dead_code)]
pub(crate) fn resolve_image_fallback<'a>(
    placeholder: Option<&'a UiAssetId>,
    failure: &'a UiImageFailurePresentation,
    state: UiImageContentState,
) -> UiResolvedImageFallback<'a> {
    match state {
        UiImageContentState::Loading => placeholder.map_or(
            UiResolvedImageFallback::Solid(UiColor::from_rgba8(71, 77, 87, 184)),
            UiResolvedImageFallback::Asset,
        ),
        UiImageContentState::Failed => match failure {
            UiImageFailurePresentation::ErrorColor { color } => {
                UiResolvedImageFallback::Solid(*color)
            }
            UiImageFailurePresentation::Placeholder => placeholder.map_or(
                UiResolvedImageFallback::Solid(default_image_failure_color()),
                UiResolvedImageFallback::Asset,
            ),
            UiImageFailurePresentation::Asset { asset } => UiResolvedImageFallback::Asset(asset),
            UiImageFailurePresentation::Hide => UiResolvedImageFallback::Hidden,
        },
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiContentFieldError {
    pub code: &'static str,
    pub path: String,
}

pub trait UiDocumentI18nCatalog {
    fn lookup(&self, key: &super::UiI18nKey) -> Option<&str>;
}

impl UiDocument {
    pub(crate) fn validate_content(&self) -> Vec<UiContentFieldError> {
        let mut errors = Vec::new();
        validate_node_content(&self.root, "$.root", &mut errors);
        errors
    }

    pub fn validate_content_with_catalog(
        &self,
        catalog: &impl UiDocumentI18nCatalog,
    ) -> Vec<UiContentFieldError> {
        let mut errors = Vec::new();
        validate_node_catalog(&self.root, "$.root", catalog, &mut errors);
        errors
    }
}

fn validate_node_content(node: &UiNode, path: &str, errors: &mut Vec<UiContentFieldError>) {
    match node {
        UiNode::Text {
            content,
            typography,
            ..
        } => {
            validate_text_content(content, &format!("{path}.content"), errors);
            validate_typography(typography, &format!("{path}.typography"), errors);
        }
        UiNode::Button { label, .. } => {
            validate_text_content(label, &format!("{path}.label"), errors);
        }
        _ => {}
    }
    for (index, child) in node.children().iter().enumerate() {
        validate_node_content(child, &format!("{path}.children[{index}]"), errors);
    }
}

fn validate_text_content(
    content: &UiTextContent,
    path: &str,
    errors: &mut Vec<UiContentFieldError>,
) {
    let (text, text_path, max_bytes, source_allows_format) = match content {
        UiTextContent::Literal(source) => (
            &source.literal,
            format!("{path}.literal"),
            UI_TEXT_LITERAL_MAX_BYTES,
            false,
        ),
        UiTextContent::I18n(source) => {
            if source.fallback.is_empty() {
                errors.push(content_error(
                    "UI_TEXT_I18N_FALLBACK_REQUIRED",
                    &format!("{path}.fallback"),
                ));
            }
            (
                &source.fallback,
                format!("{path}.fallback"),
                UI_TEXT_FALLBACK_MAX_BYTES,
                false,
            )
        }
        UiTextContent::Binding(source) => (
            &source.fallback,
            format!("{path}.fallback"),
            UI_TEXT_FALLBACK_MAX_BYTES,
            true,
        ),
    };
    if text.len() > max_bytes {
        errors.push(content_error("UI_TEXT_TOO_LONG", &text_path));
    }
    if !source_allows_format && !matches!(content.format(), UiTextFormat::Plain) {
        errors.push(content_error(
            "UI_TEXT_FORMAT_SOURCE_INCOMPATIBLE",
            &format!("{path}.format"),
        ));
    }
    match content.format() {
        UiTextFormat::Number {
            min_fraction_digits,
            max_fraction_digits,
            ..
        }
        | UiTextFormat::Percent {
            min_fraction_digits,
            max_fraction_digits,
        } if min_fraction_digits > max_fraction_digits || *max_fraction_digits > 6 => {
            errors.push(content_error(
                "UI_TEXT_FORMAT_OPTIONS_INVALID",
                &format!("{path}.format"),
            ));
        }
        UiTextFormat::Bytes { precision, .. } if *precision > 3 => errors.push(content_error(
            "UI_TEXT_FORMAT_OPTIONS_INVALID",
            &format!("{path}.format"),
        )),
        _ => {}
    }
}

fn validate_typography(
    typography: &UiTextTypography,
    path: &str,
    errors: &mut Vec<UiContentFieldError>,
) {
    let invalid_line_height = match typography.line_height {
        UiTextLineHeight::Normal => false,
        UiTextLineHeight::Relative(value) | UiTextLineHeight::Pixels(value) => {
            !value.is_finite() || value <= 0.0
        }
    };
    if invalid_line_height {
        errors.push(content_error(
            "UI_TEXT_LINE_HEIGHT_INVALID",
            &format!("{path}.line_height"),
        ));
    }
    if typography
        .max_lines
        .is_some_and(|lines| lines == 0 || lines > UI_TEXT_MAX_LINES)
    {
        errors.push(content_error(
            "UI_TEXT_MAX_LINES_INVALID",
            &format!("{path}.max_lines"),
        ));
    }
    if typography.max_lines.is_some() && typography.overflow == UiTextOverflow::Visible {
        errors.push(content_error(
            "UI_TEXT_MAX_LINES_REQUIRES_OVERFLOW",
            &format!("{path}.overflow"),
        ));
    }
    if typography.overflow == UiTextOverflow::Ellipsis && typography.max_lines.is_none() {
        errors.push(content_error(
            "UI_TEXT_ELLIPSIS_REQUIRES_MAX_LINES",
            &format!("{path}.max_lines"),
        ));
    }
}

fn validate_node_catalog(
    node: &UiNode,
    path: &str,
    catalog: &impl UiDocumentI18nCatalog,
    errors: &mut Vec<UiContentFieldError>,
) {
    let text = match node {
        UiNode::Text { content, .. } => Some((content, "content")),
        UiNode::Button { label, .. } => Some((label, "label")),
        _ => None,
    };
    if let Some((UiTextContent::I18n(source), field)) = text
        && catalog.lookup(&source.i18n_key).is_none()
    {
        errors.push(content_error(
            "UI_TEXT_I18N_KEY_MISSING",
            &format!("{path}.{field}.i18n_key"),
        ));
    }
    for (index, child) in node.children().iter().enumerate() {
        validate_node_catalog(child, &format!("{path}.children[{index}]"), catalog, errors);
    }
}

pub(crate) fn validate_content_json_shape(value: &serde_json::Value) -> Vec<UiContentFieldError> {
    let mut errors = Vec::new();
    if let Some(root) = value.get("root") {
        validate_node_json_shape(root, "$.root", &mut errors);
    }
    errors
}

fn validate_node_json_shape(
    node: &serde_json::Value,
    path: &str,
    errors: &mut Vec<UiContentFieldError>,
) {
    let Some(object) = node.as_object() else {
        return;
    };
    let text = match object.get("type").and_then(serde_json::Value::as_str) {
        Some("text") => object.get("content").map(|content| (content, "content")),
        Some("button") => object.get("label").map(|content| (content, "label")),
        _ => None,
    };
    if let Some((content, field)) = text
        && let Some(content) = content.as_object()
    {
        let source_count = ["literal", "i18n_key", "binding_path"]
            .into_iter()
            .filter(|key| content.contains_key(*key))
            .count();
        if source_count != 1 {
            errors.push(content_error(
                "UI_TEXT_SOURCE_NOT_EXCLUSIVE",
                &format!("{path}.{field}"),
            ));
        }
        if let Some(format) = content.get("format").and_then(serde_json::Value::as_object)
            && !matches!(
                format.get("kind").and_then(serde_json::Value::as_str),
                Some("plain" | "number" | "percent" | "bytes")
            )
        {
            errors.push(content_error(
                "UI_TEXT_FORMAT_NOT_ALLOWED",
                &format!("{path}.{field}.format"),
            ));
        }
    }
    if let Some(children) = object.get("children").and_then(serde_json::Value::as_array) {
        for (index, child) in children.iter().enumerate() {
            validate_node_json_shape(child, &format!("{path}.children[{index}]"), errors);
        }
    }
}

fn content_error(code: &'static str, path: &str) -> UiContentFieldError {
    UiContentFieldError {
        code,
        path: path.to_owned(),
    }
}
