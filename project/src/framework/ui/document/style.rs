use super::{UiAssetId, UiAssetKind, UiDocument, UiStyleId};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::{BTreeMap, BTreeSet};

pub const UI_DOCUMENT_MAX_MATERIALS: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UiColor {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
}

impl UiColor {
    pub const fn from_rgba8(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    pub const fn rgba8(self) -> [u8; 4] {
        [self.red, self.green, self.blue, self.alpha]
    }

    pub fn to_srgba(self) -> [f32; 4] {
        self.rgba8().map(|channel| f32::from(channel) / 255.0)
    }

    fn parse_hex(value: &str) -> Result<Self, String> {
        let digits = value
            .strip_prefix('#')
            .ok_or_else(|| "hex colors must start with `#`".to_owned())?;
        if !matches!(digits.len(), 6 | 8) || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("hex colors must contain exactly 6 or 8 hexadecimal digits".to_owned());
        }
        let channel = |start| {
            u8::from_str_radix(&digits[start..start + 2], 16)
                .map_err(|_| "hex color contains an invalid channel".to_owned())
        };
        Ok(Self::from_rgba8(
            channel(0)?,
            channel(2)?,
            channel(4)?,
            if digits.len() == 8 { channel(6)? } else { 255 },
        ))
    }

    fn from_srgb(value: UiSrgbColorInput) -> Result<Self, String> {
        let channels = [value.red, value.green, value.blue, value.alpha];
        if channels.iter().any(|channel| !channel.is_finite()) {
            return Err("sRGB channels must be finite".to_owned());
        }
        if channels
            .iter()
            .any(|channel| !(0.0..=1.0).contains(channel))
        {
            return Err("sRGB channels must be between 0 and 1".to_owned());
        }
        let quantize = |channel: f32| (channel * 255.0).round() as u8;
        Ok(Self::from_rgba8(
            quantize(value.red),
            quantize(value.green),
            quantize(value.blue),
            quantize(value.alpha),
        ))
    }
}

impl Serialize for UiColor {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!(
            "#{:02x}{:02x}{:02x}{:02x}",
            self.red, self.green, self.blue, self.alpha
        ))
    }
}

impl<'de> Deserialize<'de> for UiColor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match UiColorInput::deserialize(deserializer)? {
            UiColorInput::Hex(value) => Self::parse_hex(&value),
            UiColorInput::Srgb { srgb } => Self::from_srgb(srgb),
        }
        .map_err(serde::de::Error::custom)
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum UiColorInput {
    Hex(String),
    Srgb { srgb: UiSrgbColorInput },
}

#[derive(Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct UiSrgbColorInput {
    red: f32,
    green: f32,
    blue: f32,
    alpha: f32,
}

#[cfg(test)]
impl schemars::JsonSchema for UiColor {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "UiColor".into()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        UiColorSchema::json_schema(generator)
    }
}

#[cfg(test)]
#[allow(dead_code)]
#[derive(schemars::JsonSchema)]
#[serde(untagged)]
enum UiColorSchema {
    Hex(UiColorHexSchema),
    Srgb { srgb: UiSrgbColorInput },
}

#[cfg(test)]
#[allow(dead_code)]
#[derive(schemars::JsonSchema)]
#[serde(transparent)]
struct UiColorHexSchema(#[schemars(regex(pattern = "^#[0-9A-Fa-f]{6}([0-9A-Fa-f]{2})?$"))] String);

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum UiTokenValue {
    Color { value: UiColor },
    Number { value: f32 },
    Reference { token: UiStyleId },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum UiColorValue {
    Literal { value: UiColor },
    Token { token: UiStyleId },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum UiScalarValue {
    Literal { value: f32 },
    Token { token: UiStyleId },
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiStyleDefinition {
    #[serde(default)]
    pub extends: Option<UiStyleId>,
    #[serde(default)]
    pub properties: UiStyleProperties,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiStyleProperties {
    #[serde(default)]
    pub background: Option<UiBackgroundStyle>,
    #[serde(default)]
    pub border: Option<UiBorderStyle>,
    #[serde(default)]
    pub corner_radius: Option<UiCornerRadii>,
    #[serde(default)]
    pub text: Option<UiTextVisualStyle>,
    #[serde(default)]
    pub opacity: Option<UiScalarValue>,
    #[serde(default)]
    pub shadows: Option<Vec<UiShadowStyle>>,
    #[serde(default)]
    pub material: Option<UiMaterialStyle>,
}

impl UiStyleProperties {
    fn merge_from(&mut self, higher_priority: &Self) {
        if higher_priority.background.is_some() {
            self.background.clone_from(&higher_priority.background);
        }
        if higher_priority.border.is_some() {
            self.border.clone_from(&higher_priority.border);
        }
        if higher_priority.corner_radius.is_some() {
            self.corner_radius
                .clone_from(&higher_priority.corner_radius);
        }
        if let Some(higher_text) = &higher_priority.text {
            if let Some(text) = &mut self.text {
                text.merge_from(higher_text);
            } else {
                self.text = Some(higher_text.clone());
            }
        }
        if higher_priority.opacity.is_some() {
            self.opacity.clone_from(&higher_priority.opacity);
        }
        if higher_priority.shadows.is_some() {
            self.shadows.clone_from(&higher_priority.shadows);
        }
        if higher_priority.material.is_some() {
            self.material.clone_from(&higher_priority.material);
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum UiBackgroundStyle {
    Solid {
        color: UiColorValue,
    },
    LinearGradient {
        angle_degrees: f32,
        stops: Vec<UiGradientStop>,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiGradientStop {
    pub position: f32,
    pub color: UiColorValue,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiBorderStyle {
    pub width: UiScalarValue,
    pub color: UiColorValue,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiCornerRadii {
    pub top_left: UiScalarValue,
    pub top_right: UiScalarValue,
    pub bottom_right: UiScalarValue,
    pub bottom_left: UiScalarValue,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UiTextWeight {
    Regular,
    Medium,
    Bold,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiTextVisualStyle {
    #[serde(default)]
    pub color: Option<UiColorValue>,
    #[serde(default)]
    pub font: Option<UiAssetId>,
    #[serde(default)]
    pub font_size: Option<UiScalarValue>,
    #[serde(default)]
    pub line_height: Option<UiScalarValue>,
    #[serde(default)]
    pub letter_spacing: Option<UiScalarValue>,
    #[serde(default)]
    pub weight: Option<UiTextWeight>,
}

impl UiTextVisualStyle {
    fn merge_from(&mut self, higher_priority: &Self) {
        if higher_priority.color.is_some() {
            self.color.clone_from(&higher_priority.color);
        }
        if higher_priority.font.is_some() {
            self.font.clone_from(&higher_priority.font);
        }
        if higher_priority.font_size.is_some() {
            self.font_size.clone_from(&higher_priority.font_size);
        }
        if higher_priority.line_height.is_some() {
            self.line_height.clone_from(&higher_priority.line_height);
        }
        if higher_priority.letter_spacing.is_some() {
            self.letter_spacing
                .clone_from(&higher_priority.letter_spacing);
        }
        if higher_priority.weight.is_some() {
            self.weight = higher_priority.weight;
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiShadowStyle {
    pub color: UiColorValue,
    pub x_offset: UiScalarValue,
    pub y_offset: UiScalarValue,
    pub blur: UiScalarValue,
    pub spread: UiScalarValue,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UiMaterialStyle {
    pub asset: UiAssetId,
    pub parameters: UiMaterialParameters,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum UiMaterialParameters {
    FrostedPanelV1 {
        blur_px: UiScalarValue,
        opacity: UiScalarValue,
        tint: UiColorValue,
    },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiResolvedStyle {
    pub component: Option<UiStyleId>,
    pub role: Option<UiStyleId>,
    pub text_role: Option<UiStyleId>,
    pub properties: UiResolvedStyleProperties,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiResolvedStyleProperties {
    pub background: Option<UiResolvedBackground>,
    pub border: Option<UiResolvedBorder>,
    pub corner_radius: Option<[f32; 4]>,
    pub text: Option<UiResolvedTextVisual>,
    pub opacity: Option<f32>,
    pub shadows: Option<Vec<UiResolvedShadow>>,
    pub material: Option<UiResolvedMaterial>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum UiResolvedBackground {
    Solid(UiColor),
    LinearGradient {
        angle_degrees: f32,
        stops: Vec<(f32, UiColor)>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiResolvedBorder {
    pub width: f32,
    pub color: UiColor,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiResolvedTextVisual {
    pub color: Option<UiColor>,
    pub font: Option<UiAssetId>,
    pub font_size: Option<f32>,
    pub line_height: Option<f32>,
    pub letter_spacing: Option<f32>,
    pub weight: Option<UiTextWeight>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiResolvedShadow {
    pub color: UiColor,
    pub x_offset: f32,
    pub y_offset: f32,
    pub blur: f32,
    pub spread: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiResolvedMaterial {
    pub asset: UiAssetId,
    pub parameters: UiResolvedMaterialParameters,
}

#[derive(Clone, Debug, PartialEq)]
pub enum UiResolvedMaterialParameters {
    FrostedPanelV1 {
        blur_px: f32,
        opacity: f32,
        tint: UiColor,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiVisualFieldError {
    pub code: &'static str,
    pub path: String,
}

#[derive(Clone, Copy)]
enum ResolvedToken {
    Color(UiColor),
    Number(f32),
}

struct StyleResolver<'a> {
    document: &'a UiDocument,
    tokens: BTreeMap<UiStyleId, ResolvedToken>,
    components: BTreeMap<UiStyleId, UiStyleProperties>,
}

impl<'a> StyleResolver<'a> {
    fn new(document: &'a UiDocument) -> Self {
        Self {
            document,
            tokens: BTreeMap::new(),
            components: BTreeMap::new(),
        }
    }

    fn resolve_token(
        &mut self,
        id: &UiStyleId,
        stack: &mut BTreeSet<UiStyleId>,
        path: &str,
    ) -> Result<ResolvedToken, UiVisualFieldError> {
        if let Some(value) = self.tokens.get(id).copied() {
            return Ok(value);
        }
        if !stack.insert(id.clone()) {
            return Err(field_error("UI_STYLE_TOKEN_CYCLE", path));
        }
        let value = self
            .document
            .tokens
            .get(id)
            .ok_or_else(|| field_error("UI_STYLE_TOKEN_UNKNOWN", path))?;
        let resolved = match value {
            UiTokenValue::Color { value } => ResolvedToken::Color(*value),
            UiTokenValue::Number { value } if value.is_finite() => ResolvedToken::Number(*value),
            UiTokenValue::Number { .. } => {
                return Err(field_error("UI_STYLE_VALUE_NON_FINITE", path));
            }
            UiTokenValue::Reference { token } => self.resolve_token(token, stack, path)?,
        };
        stack.remove(id);
        self.tokens.insert(id.clone(), resolved);
        Ok(resolved)
    }

    fn resolve_component(
        &mut self,
        id: &UiStyleId,
        stack: &mut BTreeSet<UiStyleId>,
        path: &str,
    ) -> Result<UiStyleProperties, UiVisualFieldError> {
        if let Some(value) = self.components.get(id) {
            return Ok(value.clone());
        }
        if !stack.insert(id.clone()) {
            return Err(field_error("UI_STYLE_REFERENCE_CYCLE", path));
        }
        let definition = self
            .document
            .styles
            .get(id)
            .ok_or_else(|| field_error("UI_STYLE_COMPONENT_UNKNOWN", path))?;
        let mut properties = if let Some(parent) = &definition.extends {
            self.resolve_component(parent, stack, path)?
        } else {
            UiStyleProperties::default()
        };
        properties.merge_from(&definition.properties);
        stack.remove(id);
        self.components.insert(id.clone(), properties.clone());
        Ok(properties)
    }

    fn resolve_color(
        &mut self,
        value: &UiColorValue,
        path: &str,
    ) -> Result<UiColor, UiVisualFieldError> {
        match value {
            UiColorValue::Literal { value } => Ok(*value),
            UiColorValue::Token { token } => {
                match self.resolve_token(token, &mut BTreeSet::new(), path)? {
                    ResolvedToken::Color(value) => Ok(value),
                    ResolvedToken::Number(_) => {
                        Err(field_error("UI_STYLE_TOKEN_TYPE_MISMATCH", path))
                    }
                }
            }
        }
    }

    fn resolve_scalar(
        &mut self,
        value: &UiScalarValue,
        path: &str,
    ) -> Result<f32, UiVisualFieldError> {
        let value = match value {
            UiScalarValue::Literal { value } => *value,
            UiScalarValue::Token { token } => {
                match self.resolve_token(token, &mut BTreeSet::new(), path)? {
                    ResolvedToken::Number(value) => value,
                    ResolvedToken::Color(_) => {
                        return Err(field_error("UI_STYLE_TOKEN_TYPE_MISMATCH", path));
                    }
                }
            }
        };
        if value.is_finite() {
            Ok(value)
        } else {
            Err(field_error("UI_STYLE_VALUE_NON_FINITE", path))
        }
    }

    fn resolve_properties(
        &mut self,
        source: &UiStyleProperties,
        path: &str,
    ) -> Result<UiResolvedStyleProperties, UiVisualFieldError> {
        let background = match &source.background {
            None => None,
            Some(UiBackgroundStyle::Solid { color }) => Some(UiResolvedBackground::Solid(
                self.resolve_color(color, &format!("{path}.background.color"))?,
            )),
            Some(UiBackgroundStyle::LinearGradient {
                angle_degrees,
                stops,
            }) => {
                if !angle_degrees.is_finite() {
                    return Err(field_error(
                        "UI_STYLE_VALUE_NON_FINITE",
                        &format!("{path}.background.angle_degrees"),
                    ));
                }
                if stops.len() < 2
                    || stops.len() > crate::framework::ui::style::effects::MAX_GRADIENT_STOPS
                {
                    return Err(field_error(
                        "UI_STYLE_GRADIENT_STOP_BUDGET_EXCEEDED",
                        &format!("{path}.background.stops"),
                    ));
                }
                let mut previous = -1.0;
                let mut resolved = Vec::with_capacity(stops.len());
                for (index, stop) in stops.iter().enumerate() {
                    if !stop.position.is_finite()
                        || !(0.0..=1.0).contains(&stop.position)
                        || stop.position < previous
                    {
                        return Err(field_error(
                            "UI_STYLE_GRADIENT_STOPS_INVALID",
                            &format!("{path}.background.stops[{index}].position"),
                        ));
                    }
                    previous = stop.position;
                    resolved.push((
                        stop.position,
                        self.resolve_color(
                            &stop.color,
                            &format!("{path}.background.stops[{index}].color"),
                        )?,
                    ));
                }
                Some(UiResolvedBackground::LinearGradient {
                    angle_degrees: *angle_degrees,
                    stops: resolved,
                })
            }
        };

        let border = source
            .border
            .as_ref()
            .map(|border| {
                Ok(UiResolvedBorder {
                    width: non_negative(
                        self.resolve_scalar(&border.width, &format!("{path}.border.width"))?,
                        &format!("{path}.border.width"),
                    )?,
                    color: self.resolve_color(&border.color, &format!("{path}.border.color"))?,
                })
            })
            .transpose()?;

        let corner_radius = source
            .corner_radius
            .as_ref()
            .map(|radius| {
                let values = [
                    (&radius.top_left, "top_left"),
                    (&radius.top_right, "top_right"),
                    (&radius.bottom_right, "bottom_right"),
                    (&radius.bottom_left, "bottom_left"),
                ];
                let mut output = [0.0; 4];
                for (index, (value, name)) in values.into_iter().enumerate() {
                    output[index] = non_negative(
                        self.resolve_scalar(value, &format!("{path}.corner_radius.{name}"))?,
                        &format!("{path}.corner_radius.{name}"),
                    )?;
                }
                Ok(output)
            })
            .transpose()?;

        let text = source
            .text
            .as_ref()
            .map(|text| {
                let resolve_optional_scalar =
                    |resolver: &mut Self,
                     value: &Option<UiScalarValue>,
                     name: &str|
                     -> Result<Option<f32>, UiVisualFieldError> {
                        value
                            .as_ref()
                            .map(|value| {
                                resolver.resolve_scalar(value, &format!("{path}.text.{name}"))
                            })
                            .transpose()
                    };
                Ok(UiResolvedTextVisual {
                    color: text
                        .color
                        .as_ref()
                        .map(|value| self.resolve_color(value, &format!("{path}.text.color")))
                        .transpose()?,
                    font: text.font.clone(),
                    font_size: positive_optional(
                        resolve_optional_scalar(self, &text.font_size, "font_size")?,
                        &format!("{path}.text.font_size"),
                    )?,
                    line_height: positive_optional(
                        resolve_optional_scalar(self, &text.line_height, "line_height")?,
                        &format!("{path}.text.line_height"),
                    )?,
                    letter_spacing: resolve_optional_scalar(
                        self,
                        &text.letter_spacing,
                        "letter_spacing",
                    )?,
                    weight: text.weight,
                })
            })
            .transpose()?;

        let opacity = source
            .opacity
            .as_ref()
            .map(|value| self.resolve_scalar(value, &format!("{path}.opacity")))
            .transpose()?;
        if opacity.is_some_and(|value| !(0.0..=1.0).contains(&value)) {
            return Err(field_error(
                "UI_STYLE_VALUE_OUT_OF_RANGE",
                &format!("{path}.opacity"),
            ));
        }

        let shadows = source
            .shadows
            .as_ref()
            .map(|shadows| {
                if shadows.len() > crate::framework::ui::style::effects::MAX_BOX_SHADOW_LAYERS {
                    return Err(field_error(
                        "UI_STYLE_SHADOW_BUDGET_EXCEEDED",
                        &format!("{path}.shadows"),
                    ));
                }
                shadows
                    .iter()
                    .enumerate()
                    .map(|(index, shadow)| {
                        let shadow_path = format!("{path}.shadows[{index}]");
                        Ok(UiResolvedShadow {
                            color: self
                                .resolve_color(&shadow.color, &format!("{shadow_path}.color"))?,
                            x_offset: self.resolve_scalar(
                                &shadow.x_offset,
                                &format!("{shadow_path}.x_offset"),
                            )?,
                            y_offset: self.resolve_scalar(
                                &shadow.y_offset,
                                &format!("{shadow_path}.y_offset"),
                            )?,
                            blur: non_negative(
                                self.resolve_scalar(&shadow.blur, &format!("{shadow_path}.blur"))?,
                                &format!("{shadow_path}.blur"),
                            )?,
                            spread: self
                                .resolve_scalar(&shadow.spread, &format!("{shadow_path}.spread"))?,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;

        let material = source
            .material
            .as_ref()
            .map(|material| {
                let parameters = match &material.parameters {
                    UiMaterialParameters::FrostedPanelV1 {
                        blur_px,
                        opacity,
                        tint,
                    } => {
                        let blur_px = non_negative(
                            self.resolve_scalar(
                                blur_px,
                                &format!("{path}.material.parameters.blur_px"),
                            )?,
                            &format!("{path}.material.parameters.blur_px"),
                        )?;
                        let opacity = self.resolve_scalar(
                            opacity,
                            &format!("{path}.material.parameters.opacity"),
                        )?;
                        if !(0.0..=1.0).contains(&opacity) {
                            return Err(field_error(
                                "UI_STYLE_VALUE_OUT_OF_RANGE",
                                &format!("{path}.material.parameters.opacity"),
                            ));
                        }
                        UiResolvedMaterialParameters::FrostedPanelV1 {
                            blur_px,
                            opacity,
                            tint: self
                                .resolve_color(tint, &format!("{path}.material.parameters.tint"))?,
                        }
                    }
                };
                Ok(UiResolvedMaterial {
                    asset: material.asset.clone(),
                    parameters,
                })
            })
            .transpose()?;

        Ok(UiResolvedStyleProperties {
            background,
            border,
            corner_radius,
            text,
            opacity,
            shadows,
            material,
        })
    }
}

impl UiDocument {
    pub fn resolve_style(
        &self,
        style: &super::UiStyle,
        path: &str,
    ) -> Result<UiResolvedStyle, UiVisualFieldError> {
        let mut resolver = StyleResolver::new(self);
        let mut properties = if let Some(component) = &style.component {
            resolver.resolve_component(component, &mut BTreeSet::new(), path)?
        } else {
            UiStyleProperties::default()
        };
        properties.merge_from(&style.inline);
        Ok(UiResolvedStyle {
            component: style.component.clone(),
            role: style.role.clone(),
            text_role: style.text_role.clone(),
            properties: resolver.resolve_properties(&properties, path)?,
        })
    }

    pub(crate) fn validate_style_tables(&self) -> Vec<UiVisualFieldError> {
        let mut errors = Vec::new();
        let mut resolver = StyleResolver::new(self);
        for token in self.tokens.keys() {
            if let Err(error) =
                resolver.resolve_token(token, &mut BTreeSet::new(), &format!("$.tokens.{token}"))
            {
                errors.push(error);
            }
        }
        for style in self.styles.keys() {
            let path = format!("$.styles.{style}");
            match resolver.resolve_component(style, &mut BTreeSet::new(), &path) {
                Ok(properties) => match resolver.resolve_properties(&properties, &path) {
                    Ok(resolved) => {
                        validate_resolved_asset_refs(self, &resolved, &path, &mut errors)
                    }
                    Err(error) => errors.push(error),
                },
                Err(error) => errors.push(error),
            }
        }
        errors
    }

    pub(crate) fn validate_style_asset_refs(
        &self,
        style: &super::UiStyle,
        path: &str,
    ) -> Vec<UiVisualFieldError> {
        let Ok(resolved) = self.resolve_style(style, path) else {
            return Vec::new();
        };
        let mut errors = Vec::new();
        validate_resolved_asset_refs(self, &resolved.properties, path, &mut errors);
        errors
    }
}

fn validate_resolved_asset_refs(
    document: &UiDocument,
    resolved: &UiResolvedStyleProperties,
    path: &str,
    errors: &mut Vec<UiVisualFieldError>,
) {
    if let Some(font) = resolved.text.as_ref().and_then(|text| text.font.as_ref()) {
        check_asset_kind(
            document,
            font,
            UiAssetKind::Font,
            &format!("{path}.text.font"),
            errors,
        );
    }
    if let Some(material) = &resolved.material {
        check_asset_kind(
            document,
            &material.asset,
            UiAssetKind::Material,
            &format!("{path}.material.asset"),
            errors,
        );
    }
}

fn check_asset_kind(
    document: &UiDocument,
    asset: &UiAssetId,
    expected: UiAssetKind,
    path: &str,
    errors: &mut Vec<UiVisualFieldError>,
) {
    match document.assets.get(asset) {
        None => errors.push(field_error("UI_ASSET_UNKNOWN", path)),
        Some(entry) if entry.kind != expected => {
            errors.push(field_error("UI_ASSET_KIND_MISMATCH", path))
        }
        Some(_) => {}
    }
}

fn field_error(code: &'static str, path: &str) -> UiVisualFieldError {
    UiVisualFieldError {
        code,
        path: path.to_owned(),
    }
}

fn non_negative(value: f32, path: &str) -> Result<f32, UiVisualFieldError> {
    if value < 0.0 {
        Err(field_error("UI_STYLE_VALUE_OUT_OF_RANGE", path))
    } else {
        Ok(value)
    }
}

fn positive_optional(value: Option<f32>, path: &str) -> Result<Option<f32>, UiVisualFieldError> {
    if value.is_some_and(|value| value <= 0.0) {
        Err(field_error("UI_STYLE_VALUE_OUT_OF_RANGE", path))
    } else {
        Ok(value)
    }
}
