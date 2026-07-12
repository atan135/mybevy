use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::{
    fonts::UiTextStyleToken,
    theme::{ButtonColors, UiTheme},
};
use crate::framework::ui::core::UiMetrics;

pub(crate) const UI_STYLE_VARIANT_GALLERY_PARENT: &str = "gallery.parent";
pub(crate) const UI_STYLE_VARIANT_GALLERY_NESTED: &str = "gallery.nested";

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct UiStyleVariantId(String);

impl UiStyleVariantId {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UiSurfaceStyleRole {
    Screen,
    Panel,
    Elevated,
    Overlay,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UiBorderStyleRole {
    Panel,
    Control,
    Emphasis,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UiTextStyleRole {
    Primary,
    Caption,
    Muted,
    Error,
    Button,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UiButtonStyleRole {
    Primary,
    Secondary,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UiInputStyleRole {
    Standard,
    Error,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UiCardStyleRole {
    Standard,
    Emphasis,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UiDialogStyleRole {
    Standard,
    Destructive,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct UiStyleRef<R> {
    pub role: R,
    pub variant: Option<UiStyleVariantId>,
}

impl<R> UiStyleRef<R> {
    pub(crate) fn new(role: R) -> Self {
        Self {
            role,
            variant: None,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn with_variant(mut self, variant: impl Into<String>) -> Self {
        self.variant = Some(UiStyleVariantId::new(variant));
        self
    }
}

#[derive(Clone, Debug, Component, Default, Eq, PartialEq)]
pub(crate) struct UiStyleBinding {
    pub surface: Option<UiStyleRef<UiSurfaceStyleRole>>,
    pub border: Option<UiStyleRef<UiBorderStyleRole>>,
    pub text: Option<UiStyleRef<UiTextStyleRole>>,
    pub button: Option<UiStyleRef<UiButtonStyleRole>>,
    pub input: Option<UiStyleRef<UiInputStyleRole>>,
    pub card: Option<UiStyleRef<UiCardStyleRole>>,
    pub dialog: Option<UiStyleRef<UiDialogStyleRole>>,
}

impl UiStyleBinding {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn with_surface(mut self, role: UiSurfaceStyleRole) -> Self {
        self.surface = Some(UiStyleRef::new(role));
        self
    }

    pub(crate) fn with_border(mut self, role: UiBorderStyleRole) -> Self {
        self.border = Some(UiStyleRef::new(role));
        self
    }

    pub(crate) fn with_text(mut self, role: UiTextStyleRole) -> Self {
        self.text = Some(UiStyleRef::new(role));
        self
    }

    pub(crate) fn with_button(mut self, role: UiButtonStyleRole) -> Self {
        self.button = Some(UiStyleRef::new(role));
        self
    }

    #[allow(dead_code)]
    pub(crate) fn with_input(mut self, role: UiInputStyleRole) -> Self {
        self.input = Some(UiStyleRef::new(role));
        self
    }

    #[allow(dead_code)]
    pub(crate) fn with_card(mut self, role: UiCardStyleRole) -> Self {
        self.card = Some(UiStyleRef::new(role));
        self
    }

    #[allow(dead_code)]
    pub(crate) fn with_dialog(mut self, role: UiDialogStyleRole) -> Self {
        self.dialog = Some(UiStyleRef::new(role));
        self
    }
}

#[derive(Clone, Debug, Component, Eq, PartialEq)]
pub(crate) struct UiStyleScope {
    pub variant: UiStyleVariantId,
}

impl UiStyleScope {
    pub(crate) fn new(variant: impl Into<String>) -> Self {
        Self {
            variant: UiStyleVariantId::new(variant),
        }
    }
}

#[derive(Clone, Copy, Debug, Component, PartialEq)]
pub(crate) struct UiResolvedSurfaceStyle {
    pub background: Color,
}

#[derive(Clone, Copy, Debug, Component, PartialEq)]
pub(crate) struct UiResolvedBorderStyle {
    pub color: Color,
    pub width: f32,
    pub radius: f32,
}

#[derive(Clone, Copy, Debug, Component, PartialEq)]
pub(crate) struct UiResolvedTextStyle {
    pub color: Color,
    pub font_size: f32,
}

#[derive(Clone, Copy, Debug, Component, PartialEq)]
pub(crate) struct UiResolvedButtonStyle {
    pub backgrounds: ButtonColors,
    pub icon_tints: ButtonColors,
    pub text_color: Color,
    pub border_color: Color,
    pub border_width: f32,
    pub radius: f32,
    pub padding_x: f32,
}

#[derive(Clone, Copy, Debug, Component, PartialEq)]
pub(crate) struct UiResolvedInputStyle {
    pub backgrounds: ButtonColors,
    pub border_idle: Color,
    pub border_hovered: Color,
    pub border_pressed: Color,
    pub border_focused: Color,
    pub border_disabled: Color,
    pub border_error: Color,
    pub text: Color,
    pub placeholder: Color,
    pub error_text: Color,
    pub selection_text: Color,
    pub selection_background: Color,
    pub border_width: f32,
    pub radius: f32,
}

#[derive(Clone, Copy, Debug, Component, PartialEq)]
pub(crate) struct UiResolvedCardStyle {
    pub background: Color,
    pub border: Color,
    pub border_width: f32,
    pub radius: f32,
    pub padding: f32,
}

#[derive(Clone, Copy, Debug, Component, PartialEq)]
pub(crate) struct UiResolvedDialogStyle {
    pub background: Color,
    pub border: Color,
    pub border_width: f32,
    pub radius: f32,
    pub padding: f32,
}

#[derive(Clone, Debug, Component, PartialEq, Serialize)]
pub(crate) struct UiResolvedStyleDebugSnapshot {
    pub scopes: Vec<String>,
    pub entries: Vec<UiResolvedStyleDebugEntry>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct UiResolvedStyleDebugEntry {
    pub request: String,
    pub requested_variant: Option<String>,
    pub sources: Vec<String>,
    pub final_tokens: Vec<UiResolvedStyleDebugToken>,
    pub fallback: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct UiResolvedStyleDebugToken {
    pub name: &'static str,
    pub value: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UiStyleErrorCode {
    UnknownToken,
    UnknownVariant,
    VariantCycle,
    DuplicateToken,
    DuplicateVariant,
    DuplicateOverride,
    TokenTypeMismatch,
    InvalidValue,
}

impl UiStyleErrorCode {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::UnknownToken => "ui_style_unknown_token",
            Self::UnknownVariant => "ui_style_unknown_variant",
            Self::VariantCycle => "ui_style_variant_cycle",
            Self::DuplicateToken => "ui_style_duplicate_token",
            Self::DuplicateVariant => "ui_style_duplicate_variant",
            Self::DuplicateOverride => "ui_style_duplicate_override",
            Self::TokenTypeMismatch => "ui_style_token_type_mismatch",
            Self::InvalidValue => "ui_style_invalid_value",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct UiStyleError {
    pub code: UiStyleErrorCode,
    pub detail: String,
}

impl UiStyleError {
    fn new(code: UiStyleErrorCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
}

impl std::fmt::Display for UiStyleError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code.as_str(), self.detail)
    }
}

impl std::error::Error for UiStyleError {}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct UiStyleSheetConfig {
    #[serde(default)]
    tokens: Vec<UiStyleTokenConfig>,
    #[serde(default)]
    variants: Vec<UiStyleVariantConfig>,
}

impl Default for UiStyleSheetConfig {
    fn default() -> Self {
        default_style_sheet_config()
    }
}

#[derive(Clone, Debug, Deserialize)]
struct UiStyleTokenConfig {
    name: String,
    value: UiStyleTokenValueConfig,
}

#[derive(Clone, Copy, Debug, Deserialize)]
enum UiStyleTokenValueConfig {
    Color(UiStyleColorConfig),
    Scalar(f32),
}

#[derive(Clone, Copy, Debug, Deserialize)]
struct UiStyleColorConfig {
    r: f32,
    g: f32,
    b: f32,
    #[serde(default = "default_alpha")]
    a: f32,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct UiStyleVariantConfig {
    name: String,
    #[serde(default)]
    extends: Option<String>,
    #[serde(default)]
    probes: Vec<UiStyleTokenProbeConfig>,
    #[serde(default)]
    overrides: Vec<UiStyleOverrideConfig>,
}

// A probe is also used by concrete overrides below. Keeping the expected kind
// explicit makes wrong-category token references fail during compilation.
#[derive(Clone, Debug, Deserialize)]
struct UiStyleTokenProbeConfig {
    token: String,
    expected: UiStyleTokenKind,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum UiStyleTokenKind {
    Color,
    Scalar,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UiStyleState {
    Idle,
    Hovered,
    Pressed,
    Focused,
    Selected,
    Disabled,
    Loading,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
enum UiInputBorderPart {
    Idle,
    Hovered,
    Pressed,
    Focused,
    Disabled,
    Error,
}

#[derive(Clone, Debug, Deserialize)]
enum UiStyleOverrideConfig {
    SurfaceBackground {
        role: UiSurfaceStyleRole,
        token: String,
    },
    BorderColor {
        role: UiBorderStyleRole,
        token: String,
    },
    BorderWidth {
        role: UiBorderStyleRole,
        token: String,
    },
    BorderRadius {
        role: UiBorderStyleRole,
        token: String,
    },
    TextColor {
        role: UiTextStyleRole,
        token: String,
    },
    TextSize {
        role: UiTextStyleRole,
        token: String,
    },
    ButtonBackground {
        role: UiButtonStyleRole,
        state: UiStyleState,
        token: String,
    },
    ButtonIconTint {
        role: UiButtonStyleRole,
        state: UiStyleState,
        token: String,
    },
    ButtonTextColor {
        role: UiButtonStyleRole,
        token: String,
    },
    ButtonBorderColor {
        role: UiButtonStyleRole,
        token: String,
    },
    ButtonBorderWidth {
        role: UiButtonStyleRole,
        token: String,
    },
    ButtonRadius {
        role: UiButtonStyleRole,
        token: String,
    },
    ButtonPaddingX {
        role: UiButtonStyleRole,
        token: String,
    },
    InputBackground {
        role: UiInputStyleRole,
        state: UiStyleState,
        token: String,
    },
    InputBorder {
        role: UiInputStyleRole,
        part: UiInputBorderPart,
        token: String,
    },
    InputText {
        role: UiInputStyleRole,
        token: String,
    },
    InputPlaceholder {
        role: UiInputStyleRole,
        token: String,
    },
    InputErrorText {
        role: UiInputStyleRole,
        token: String,
    },
    InputSelectionText {
        role: UiInputStyleRole,
        token: String,
    },
    InputSelectionBackground {
        role: UiInputStyleRole,
        token: String,
    },
    InputBorderWidth {
        role: UiInputStyleRole,
        token: String,
    },
    InputRadius {
        role: UiInputStyleRole,
        token: String,
    },
    CardBackground {
        role: UiCardStyleRole,
        token: String,
    },
    CardBorder {
        role: UiCardStyleRole,
        token: String,
    },
    CardBorderWidth {
        role: UiCardStyleRole,
        token: String,
    },
    CardRadius {
        role: UiCardStyleRole,
        token: String,
    },
    CardPadding {
        role: UiCardStyleRole,
        token: String,
    },
    DialogBackground {
        role: UiDialogStyleRole,
        token: String,
    },
    DialogBorder {
        role: UiDialogStyleRole,
        token: String,
    },
    DialogBorderWidth {
        role: UiDialogStyleRole,
        token: String,
    },
    DialogRadius {
        role: UiDialogStyleRole,
        token: String,
    },
    DialogPadding {
        role: UiDialogStyleRole,
        token: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum UiStyleTokenValue {
    Color(Color),
    Scalar(f32),
}

impl UiStyleTokenValue {
    fn kind(self) -> UiStyleTokenKind {
        match self {
            Self::Color(_) => UiStyleTokenKind::Color,
            Self::Scalar(_) => UiStyleTokenKind::Scalar,
        }
    }
}

#[derive(Clone, Debug)]
struct UiCompiledStyleVariant {
    extends: Option<String>,
    overrides: Vec<UiCompiledStyleOverride>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum UiCompiledStyleOverride {
    SurfaceBackground(UiSurfaceStyleRole, Color),
    BorderColor(UiBorderStyleRole, Color),
    BorderWidth(UiBorderStyleRole, f32),
    BorderRadius(UiBorderStyleRole, f32),
    TextColor(UiTextStyleRole, Color),
    TextSize(UiTextStyleRole, f32),
    ButtonBackground(UiButtonStyleRole, UiStyleState, Color),
    ButtonIconTint(UiButtonStyleRole, UiStyleState, Color),
    ButtonTextColor(UiButtonStyleRole, Color),
    ButtonBorderColor(UiButtonStyleRole, Color),
    ButtonBorderWidth(UiButtonStyleRole, f32),
    ButtonRadius(UiButtonStyleRole, f32),
    ButtonPaddingX(UiButtonStyleRole, f32),
    InputBackground(UiInputStyleRole, UiStyleState, Color),
    InputBorder(UiInputStyleRole, UiInputBorderPart, Color),
    InputText(UiInputStyleRole, Color),
    InputPlaceholder(UiInputStyleRole, Color),
    InputErrorText(UiInputStyleRole, Color),
    InputSelectionText(UiInputStyleRole, Color),
    InputSelectionBackground(UiInputStyleRole, Color),
    InputBorderWidth(UiInputStyleRole, f32),
    InputRadius(UiInputStyleRole, f32),
    CardBackground(UiCardStyleRole, Color),
    CardBorder(UiCardStyleRole, Color),
    CardBorderWidth(UiCardStyleRole, f32),
    CardRadius(UiCardStyleRole, f32),
    CardPadding(UiCardStyleRole, f32),
    DialogBackground(UiDialogStyleRole, Color),
    DialogBorder(UiDialogStyleRole, Color),
    DialogBorderWidth(UiDialogStyleRole, f32),
    DialogRadius(UiDialogStyleRole, f32),
    DialogPadding(UiDialogStyleRole, f32),
}

#[derive(Clone, Debug, Default)]
pub(crate) struct UiStyleSheet {
    variants: BTreeMap<String, UiCompiledStyleVariant>,
}

impl UiStyleSheet {
    pub(super) fn built_in() -> Self {
        Self::compile(default_style_sheet_config())
            .expect("built-in scoped UI styles must pass compiler validation")
    }

    pub(super) fn compile(config: UiStyleSheetConfig) -> Result<Self, UiStyleError> {
        let mut tokens = BTreeMap::new();
        for token in config.tokens {
            validate_name("token", &token.name)?;
            let value = token.value.compile(&token.name)?;
            if tokens.insert(token.name.clone(), value).is_some() {
                return Err(UiStyleError::new(
                    UiStyleErrorCode::DuplicateToken,
                    format!("token '{}' is declared more than once", token.name),
                ));
            }
        }

        let mut variants = BTreeMap::new();
        let mut probes = BTreeMap::new();
        for variant in config.variants {
            validate_name("variant", &variant.name)?;
            if variants.contains_key(&variant.name) {
                return Err(UiStyleError::new(
                    UiStyleErrorCode::DuplicateVariant,
                    format!("variant '{}' is declared more than once", variant.name),
                ));
            }
            probes.insert(variant.name.clone(), variant.probes);
            let compiled_overrides = compile_overrides(&variant.name, variant.overrides, &tokens)?;
            variants.insert(
                variant.name,
                UiCompiledStyleVariant {
                    extends: variant.extends,
                    overrides: compiled_overrides,
                },
            );
        }

        for (name, variant) in &variants {
            if let Some(parent) = &variant.extends {
                if !variants.contains_key(parent) {
                    return Err(UiStyleError::new(
                        UiStyleErrorCode::UnknownVariant,
                        format!("variant '{name}' extends unknown variant '{parent}'"),
                    ));
                }
            }
        }
        validate_variant_cycles(&variants)?;

        for (variant, probes) in probes {
            for probe in probes {
                let Some(value) = tokens.get(&probe.token).copied() else {
                    return Err(UiStyleError::new(
                        UiStyleErrorCode::UnknownToken,
                        format!(
                            "variant '{variant}' references unknown token '{}'",
                            probe.token
                        ),
                    ));
                };
                if value.kind() != probe.expected {
                    return Err(UiStyleError::new(
                        UiStyleErrorCode::TokenTypeMismatch,
                        format!(
                            "variant '{variant}' expected {:?} token '{}', found {:?}",
                            probe.expected,
                            probe.token,
                            value.kind()
                        ),
                    ));
                }
            }
        }

        Ok(Self { variants })
    }

    #[allow(dead_code)]
    pub(crate) fn contains_variant(&self, id: &UiStyleVariantId) -> bool {
        self.variants.contains_key(id.as_str())
    }

    fn layers<'a>(
        &'a self,
        requested: Option<&UiStyleVariantId>,
        scopes: &[String],
    ) -> UiStyleLayers<'a> {
        let mut resolved = UiStyleLayers::default();
        if let Some(requested) = requested {
            resolved.append(self, requested.as_str(), "request");
        }
        for scope in scopes {
            resolved.append(self, scope, "scope");
        }
        resolved
    }

    fn variant_chain<'a>(
        &'a self,
        name: &str,
    ) -> Result<Vec<(&'a str, &'a UiCompiledStyleVariant)>, UiStyleError> {
        fn collect<'a>(
            sheet: &'a UiStyleSheet,
            name: &str,
            visiting: &mut BTreeSet<String>,
            result: &mut Vec<(&'a str, &'a UiCompiledStyleVariant)>,
        ) -> Result<(), UiStyleError> {
            let Some((stored_name, variant)) = sheet.variants.get_key_value(name) else {
                return Err(UiStyleError::new(
                    UiStyleErrorCode::UnknownVariant,
                    format!("runtime requested unknown variant '{name}'"),
                ));
            };
            if !visiting.insert(name.to_owned()) {
                return Err(UiStyleError::new(
                    UiStyleErrorCode::VariantCycle,
                    format!("runtime variant cycle includes '{name}'"),
                ));
            }
            if let Some(parent) = variant.extends.as_deref() {
                collect(sheet, parent, visiting, result)?;
            }
            visiting.remove(name);
            result.push((stored_name.as_str(), variant));
            Ok(())
        }

        let mut result = Vec::new();
        collect(self, name, &mut BTreeSet::new(), &mut result)?;
        Ok(result)
    }
}

#[derive(Default)]
struct UiStyleLayers<'a> {
    variants: Vec<&'a UiCompiledStyleVariant>,
    sources: Vec<String>,
    fallback: bool,
    error: Option<UiStyleError>,
}

impl<'a> UiStyleLayers<'a> {
    fn append(&mut self, sheet: &'a UiStyleSheet, name: &str, origin: &str) {
        match sheet.variant_chain(name) {
            Ok(chain) => {
                for (variant_name, variant) in chain {
                    self.sources
                        .push(format!("{origin}:{name}/variant:{variant_name}"));
                    self.variants.push(variant);
                }
            }
            Err(error) => {
                self.fallback = true;
                if self.error.is_none() {
                    self.error = Some(error);
                }
            }
        }
    }
}

struct UiResolvedBinding {
    surface: Option<UiResolvedSurfaceStyle>,
    border: Option<UiResolvedBorderStyle>,
    text: Option<UiResolvedTextStyle>,
    button: Option<UiResolvedButtonStyle>,
    input: Option<UiResolvedInputStyle>,
    card: Option<UiResolvedCardStyle>,
    dialog: Option<UiResolvedDialogStyle>,
    debug: UiResolvedStyleDebugSnapshot,
}

impl UiStyleSheet {
    fn resolve_binding(
        &self,
        theme: &UiTheme,
        metrics: &UiMetrics,
        binding: &UiStyleBinding,
        scopes: Vec<String>,
    ) -> UiResolvedBinding {
        let mut entries = Vec::new();
        let surface = binding.surface.as_ref().map(|request| {
            let (style, debug) = self.resolve_surface(theme, request, &scopes);
            entries.push(debug);
            style
        });
        let border = binding.border.as_ref().map(|request| {
            let (style, debug) = self.resolve_border(theme, request, &scopes);
            entries.push(debug);
            style
        });
        let text = binding.text.as_ref().map(|request| {
            let (style, debug) = self.resolve_text(theme, request, &scopes);
            entries.push(debug);
            style
        });
        let button = binding.button.as_ref().map(|request| {
            let (style, debug) = self.resolve_button(theme, metrics, request, &scopes);
            entries.push(debug);
            style
        });
        let input = binding.input.as_ref().map(|request| {
            let (style, debug) = self.resolve_input(theme, request, &scopes);
            entries.push(debug);
            style
        });
        let card = binding.card.as_ref().map(|request| {
            let (style, debug) = self.resolve_card(theme, metrics, request, &scopes);
            entries.push(debug);
            style
        });
        let dialog = binding.dialog.as_ref().map(|request| {
            let (style, debug) = self.resolve_dialog(theme, metrics, request, &scopes);
            entries.push(debug);
            style
        });
        UiResolvedBinding {
            surface,
            border,
            text,
            button,
            input,
            card,
            dialog,
            debug: UiResolvedStyleDebugSnapshot { scopes, entries },
        }
    }

    fn resolve_surface(
        &self,
        theme: &UiTheme,
        request: &UiStyleRef<UiSurfaceStyleRole>,
        scopes: &[String],
    ) -> (UiResolvedSurfaceStyle, UiResolvedStyleDebugEntry) {
        let mut style = base_surface_style(theme, request.role);
        let layers = self.layers(request.variant.as_ref(), scopes);
        for variant in &layers.variants {
            for value in &variant.overrides {
                if let UiCompiledStyleOverride::SurfaceBackground(role, background) = value {
                    if *role == request.role {
                        style.background = *background;
                    }
                }
            }
        }
        let tokens = vec![debug_color("background", style.background)];
        (style, debug_entry("surface", request, layers, tokens))
    }

    fn resolve_border(
        &self,
        theme: &UiTheme,
        request: &UiStyleRef<UiBorderStyleRole>,
        scopes: &[String],
    ) -> (UiResolvedBorderStyle, UiResolvedStyleDebugEntry) {
        let mut style = base_border_style(theme, request.role);
        let layers = self.layers(request.variant.as_ref(), scopes);
        for variant in &layers.variants {
            for value in &variant.overrides {
                match value {
                    UiCompiledStyleOverride::BorderColor(role, color) if *role == request.role => {
                        style.color = *color;
                    }
                    UiCompiledStyleOverride::BorderWidth(role, width) if *role == request.role => {
                        style.width = *width;
                    }
                    UiCompiledStyleOverride::BorderRadius(role, radius)
                        if *role == request.role =>
                    {
                        style.radius = *radius;
                    }
                    _ => {}
                }
            }
        }
        let tokens = vec![
            debug_color("color", style.color),
            debug_scalar("width", style.width),
            debug_scalar("radius", style.radius),
        ];
        (style, debug_entry("border", request, layers, tokens))
    }

    fn resolve_text(
        &self,
        theme: &UiTheme,
        request: &UiStyleRef<UiTextStyleRole>,
        scopes: &[String],
    ) -> (UiResolvedTextStyle, UiResolvedStyleDebugEntry) {
        let mut style = base_text_style(theme, request.role);
        let layers = self.layers(request.variant.as_ref(), scopes);
        for variant in &layers.variants {
            for value in &variant.overrides {
                match value {
                    UiCompiledStyleOverride::TextColor(role, color) if *role == request.role => {
                        style.color = *color;
                    }
                    UiCompiledStyleOverride::TextSize(role, size) if *role == request.role => {
                        style.font_size = *size;
                    }
                    _ => {}
                }
            }
        }
        let tokens = vec![
            debug_color("color", style.color),
            debug_scalar("font_size", style.font_size),
        ];
        (style, debug_entry("text", request, layers, tokens))
    }

    fn resolve_button(
        &self,
        theme: &UiTheme,
        metrics: &UiMetrics,
        request: &UiStyleRef<UiButtonStyleRole>,
        scopes: &[String],
    ) -> (UiResolvedButtonStyle, UiResolvedStyleDebugEntry) {
        let mut style = base_button_style(theme, metrics, request.role);
        let layers = self.layers(request.variant.as_ref(), scopes);
        for variant in &layers.variants {
            for value in &variant.overrides {
                match value {
                    UiCompiledStyleOverride::ButtonBackground(role, state, color)
                        if *role == request.role =>
                    {
                        set_state_color(&mut style.backgrounds, *state, *color);
                    }
                    UiCompiledStyleOverride::ButtonIconTint(role, state, color)
                        if *role == request.role =>
                    {
                        set_state_color(&mut style.icon_tints, *state, *color);
                    }
                    UiCompiledStyleOverride::ButtonTextColor(role, color)
                        if *role == request.role =>
                    {
                        style.text_color = *color;
                    }
                    UiCompiledStyleOverride::ButtonBorderColor(role, color)
                        if *role == request.role =>
                    {
                        style.border_color = *color;
                    }
                    UiCompiledStyleOverride::ButtonBorderWidth(role, width)
                        if *role == request.role =>
                    {
                        style.border_width = *width;
                    }
                    UiCompiledStyleOverride::ButtonRadius(role, radius)
                        if *role == request.role =>
                    {
                        style.radius = *radius;
                    }
                    UiCompiledStyleOverride::ButtonPaddingX(role, padding)
                        if *role == request.role =>
                    {
                        style.padding_x = *padding;
                    }
                    _ => {}
                }
            }
        }
        let tokens = vec![
            debug_color("background.idle", style.backgrounds.idle),
            debug_color("background.selected", style.backgrounds.selected),
            debug_color("text", style.text_color),
            debug_color("border", style.border_color),
            debug_scalar("radius", style.radius),
        ];
        (style, debug_entry("button", request, layers, tokens))
    }

    fn resolve_input(
        &self,
        theme: &UiTheme,
        request: &UiStyleRef<UiInputStyleRole>,
        scopes: &[String],
    ) -> (UiResolvedInputStyle, UiResolvedStyleDebugEntry) {
        let mut style = base_input_style(theme, request.role);
        let layers = self.layers(request.variant.as_ref(), scopes);
        for variant in &layers.variants {
            for value in &variant.overrides {
                match value {
                    UiCompiledStyleOverride::InputBackground(role, state, color)
                        if *role == request.role =>
                    {
                        set_state_color(&mut style.backgrounds, *state, *color);
                    }
                    UiCompiledStyleOverride::InputBorder(role, part, color)
                        if *role == request.role =>
                    {
                        set_input_border_color(&mut style, *part, *color);
                    }
                    UiCompiledStyleOverride::InputText(role, color) if *role == request.role => {
                        style.text = *color;
                    }
                    UiCompiledStyleOverride::InputPlaceholder(role, color)
                        if *role == request.role =>
                    {
                        style.placeholder = *color;
                    }
                    UiCompiledStyleOverride::InputErrorText(role, color)
                        if *role == request.role =>
                    {
                        style.error_text = *color;
                    }
                    UiCompiledStyleOverride::InputSelectionText(role, color)
                        if *role == request.role =>
                    {
                        style.selection_text = *color;
                    }
                    UiCompiledStyleOverride::InputSelectionBackground(role, color)
                        if *role == request.role =>
                    {
                        style.selection_background = *color;
                    }
                    UiCompiledStyleOverride::InputBorderWidth(role, width)
                        if *role == request.role =>
                    {
                        style.border_width = *width;
                    }
                    UiCompiledStyleOverride::InputRadius(role, radius) if *role == request.role => {
                        style.radius = *radius;
                    }
                    _ => {}
                }
            }
        }
        let tokens = vec![
            debug_color("background.idle", style.backgrounds.idle),
            debug_color("border.idle", style.border_idle),
            debug_color("border.error", style.border_error),
            debug_color("text", style.text),
        ];
        (style, debug_entry("input", request, layers, tokens))
    }

    fn resolve_card(
        &self,
        theme: &UiTheme,
        metrics: &UiMetrics,
        request: &UiStyleRef<UiCardStyleRole>,
        scopes: &[String],
    ) -> (UiResolvedCardStyle, UiResolvedStyleDebugEntry) {
        let mut style = base_card_style(theme, metrics, request.role);
        let layers = self.layers(request.variant.as_ref(), scopes);
        for variant in &layers.variants {
            for value in &variant.overrides {
                match value {
                    UiCompiledStyleOverride::CardBackground(role, color)
                        if *role == request.role =>
                    {
                        style.background = *color;
                    }
                    UiCompiledStyleOverride::CardBorder(role, color) if *role == request.role => {
                        style.border = *color;
                    }
                    UiCompiledStyleOverride::CardBorderWidth(role, width)
                        if *role == request.role =>
                    {
                        style.border_width = *width;
                    }
                    UiCompiledStyleOverride::CardRadius(role, radius) if *role == request.role => {
                        style.radius = *radius;
                    }
                    UiCompiledStyleOverride::CardPadding(role, padding)
                        if *role == request.role =>
                    {
                        style.padding = *padding;
                    }
                    _ => {}
                }
            }
        }
        let tokens = vec![
            debug_color("background", style.background),
            debug_color("border", style.border),
            debug_scalar("radius", style.radius),
            debug_scalar("padding", style.padding),
        ];
        (style, debug_entry("card", request, layers, tokens))
    }

    fn resolve_dialog(
        &self,
        theme: &UiTheme,
        metrics: &UiMetrics,
        request: &UiStyleRef<UiDialogStyleRole>,
        scopes: &[String],
    ) -> (UiResolvedDialogStyle, UiResolvedStyleDebugEntry) {
        let mut style = base_dialog_style(theme, metrics, request.role);
        let layers = self.layers(request.variant.as_ref(), scopes);
        for variant in &layers.variants {
            for value in &variant.overrides {
                match value {
                    UiCompiledStyleOverride::DialogBackground(role, color)
                        if *role == request.role =>
                    {
                        style.background = *color;
                    }
                    UiCompiledStyleOverride::DialogBorder(role, color) if *role == request.role => {
                        style.border = *color;
                    }
                    UiCompiledStyleOverride::DialogBorderWidth(role, width)
                        if *role == request.role =>
                    {
                        style.border_width = *width;
                    }
                    UiCompiledStyleOverride::DialogRadius(role, radius)
                        if *role == request.role =>
                    {
                        style.radius = *radius;
                    }
                    UiCompiledStyleOverride::DialogPadding(role, padding)
                        if *role == request.role =>
                    {
                        style.padding = *padding;
                    }
                    _ => {}
                }
            }
        }
        let tokens = vec![
            debug_color("background", style.background),
            debug_color("border", style.border),
            debug_scalar("radius", style.radius),
        ];
        (style, debug_entry("dialog", request, layers, tokens))
    }
}

fn debug_entry<R: std::fmt::Debug>(
    kind: &str,
    request: &UiStyleRef<R>,
    layers: UiStyleLayers<'_>,
    final_tokens: Vec<UiResolvedStyleDebugToken>,
) -> UiResolvedStyleDebugEntry {
    let mut sources = vec![format!("base:{kind}.{:?}", request.role).to_ascii_lowercase()];
    sources.extend(layers.sources);
    UiResolvedStyleDebugEntry {
        request: format!("{kind}.{:?}", request.role).to_ascii_lowercase(),
        requested_variant: request
            .variant
            .as_ref()
            .map(|variant| variant.as_str().to_owned()),
        sources,
        final_tokens,
        fallback: layers.fallback,
        error: layers.error.map(|error| error.code.as_str().to_owned()),
    }
}

fn debug_color(name: &'static str, color: Color) -> UiResolvedStyleDebugToken {
    let value = color.to_srgba();
    UiResolvedStyleDebugToken {
        name,
        value: format!(
            "#{:02X}{:02X}{:02X}{:02X}",
            (value.red.clamp(0.0, 1.0) * 255.0).round() as u8,
            (value.green.clamp(0.0, 1.0) * 255.0).round() as u8,
            (value.blue.clamp(0.0, 1.0) * 255.0).round() as u8,
            (value.alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
        ),
    }
}

fn debug_scalar(name: &'static str, value: f32) -> UiResolvedStyleDebugToken {
    UiResolvedStyleDebugToken {
        name,
        value: format!("{value:.2}"),
    }
}

fn base_surface_style(theme: &UiTheme, role: UiSurfaceStyleRole) -> UiResolvedSurfaceStyle {
    UiResolvedSurfaceStyle {
        background: match role {
            UiSurfaceStyleRole::Screen => theme.colors.screen_background,
            UiSurfaceStyleRole::Panel => theme.colors.panel_background,
            UiSurfaceStyleRole::Elevated => theme.colors.secondary_button.idle,
            UiSurfaceStyleRole::Overlay => theme.colors.modal_overlay_background,
        },
    }
}

fn base_border_style(theme: &UiTheme, role: UiBorderStyleRole) -> UiResolvedBorderStyle {
    match role {
        UiBorderStyleRole::Panel => UiResolvedBorderStyle {
            color: theme.colors.panel_border,
            width: theme.panel.border,
            radius: theme.panel.radius,
        },
        UiBorderStyleRole::Control => UiResolvedBorderStyle {
            color: theme.colors.panel_border,
            width: 1.0,
            radius: theme.button.radius,
        },
        UiBorderStyleRole::Emphasis => UiResolvedBorderStyle {
            color: theme.colors.primary_button.focused,
            width: theme.panel.border.max(1.0),
            radius: theme.panel.radius,
        },
    }
}

fn base_text_style(theme: &UiTheme, role: UiTextStyleRole) -> UiResolvedTextStyle {
    match role {
        UiTextStyleRole::Primary => UiResolvedTextStyle {
            color: theme.colors.text_primary,
            font_size: theme.text.body,
        },
        UiTextStyleRole::Caption => UiResolvedTextStyle {
            color: theme.colors.text_primary,
            font_size: theme.text.caption,
        },
        UiTextStyleRole::Muted => UiResolvedTextStyle {
            color: theme.colors.text_muted,
            font_size: theme.text.caption,
        },
        UiTextStyleRole::Error => UiResolvedTextStyle {
            color: theme.colors.text_error,
            font_size: theme.text.body,
        },
        UiTextStyleRole::Button => UiResolvedTextStyle {
            color: theme.colors.text_primary,
            font_size: theme.text.button,
        },
    }
}

fn base_button_style(
    theme: &UiTheme,
    metrics: &UiMetrics,
    role: UiButtonStyleRole,
) -> UiResolvedButtonStyle {
    UiResolvedButtonStyle {
        backgrounds: match role {
            UiButtonStyleRole::Primary => theme.colors.primary_button,
            UiButtonStyleRole::Secondary => theme.colors.secondary_button,
        },
        icon_tints: theme.colors.icon_tint,
        text_color: theme.colors.text_primary,
        border_color: theme.colors.panel_border,
        border_width: 0.0,
        radius: theme.button.radius,
        padding_x: (metrics.control_gap * 2.0).clamp(12.0, 24.0),
    }
}

fn base_input_style(theme: &UiTheme, role: UiInputStyleRole) -> UiResolvedInputStyle {
    UiResolvedInputStyle {
        backgrounds: theme.colors.secondary_button,
        border_idle: if role == UiInputStyleRole::Error {
            theme.colors.error
        } else {
            theme.colors.panel_border
        },
        border_hovered: theme.colors.secondary_button.focused,
        border_pressed: theme.colors.primary_button.pressed,
        border_focused: theme.colors.primary_button.focused,
        border_disabled: theme.colors.secondary_button.disabled,
        border_error: theme.colors.error,
        text: theme.colors.text_primary,
        placeholder: theme.colors.text_muted,
        error_text: theme.colors.text_error,
        selection_text: theme.colors.screen_background,
        selection_background: theme.colors.primary_button.focused,
        border_width: 1.0,
        radius: theme.button.radius,
    }
}

fn base_card_style(
    theme: &UiTheme,
    metrics: &UiMetrics,
    role: UiCardStyleRole,
) -> UiResolvedCardStyle {
    UiResolvedCardStyle {
        background: match role {
            UiCardStyleRole::Standard => theme.colors.panel_background,
            UiCardStyleRole::Emphasis => theme.colors.secondary_button.idle,
        },
        border: match role {
            UiCardStyleRole::Standard => theme.colors.panel_border,
            UiCardStyleRole::Emphasis => theme.colors.primary_button.focused,
        },
        border_width: theme.panel.border,
        radius: theme.panel.radius,
        padding: metrics.panel_padding,
    }
}

fn base_dialog_style(
    theme: &UiTheme,
    metrics: &UiMetrics,
    role: UiDialogStyleRole,
) -> UiResolvedDialogStyle {
    UiResolvedDialogStyle {
        background: theme.colors.panel_background,
        border: match role {
            UiDialogStyleRole::Standard => theme.colors.panel_border,
            UiDialogStyleRole::Destructive => theme.colors.error,
        },
        border_width: theme.panel.border,
        radius: theme.panel.radius,
        padding: metrics.panel_padding,
    }
}

fn set_state_color(colors: &mut ButtonColors, state: UiStyleState, value: Color) {
    match state {
        UiStyleState::Idle => colors.idle = value,
        UiStyleState::Hovered => colors.hovered = value,
        UiStyleState::Pressed => colors.pressed = value,
        UiStyleState::Focused => colors.focused = value,
        UiStyleState::Selected => colors.selected = value,
        UiStyleState::Disabled => colors.disabled = value,
        UiStyleState::Loading => colors.loading = value,
    }
}

fn set_input_border_color(style: &mut UiResolvedInputStyle, part: UiInputBorderPart, value: Color) {
    match part {
        UiInputBorderPart::Idle => style.border_idle = value,
        UiInputBorderPart::Hovered => style.border_hovered = value,
        UiInputBorderPart::Pressed => style.border_pressed = value,
        UiInputBorderPart::Focused => style.border_focused = value,
        UiInputBorderPart::Disabled => style.border_disabled = value,
        UiInputBorderPart::Error => style.border_error = value,
    }
}

pub(super) fn resolve_ui_style_bindings(
    mut commands: Commands,
    theme: Res<UiTheme>,
    metrics: Res<UiMetrics>,
    bindings: Query<(Entity, &UiStyleBinding)>,
    mut removed_bindings: RemovedComponents<UiStyleBinding>,
    parents: Query<&ChildOf>,
    scopes: Query<&UiStyleScope>,
    current_surface: Query<&UiResolvedSurfaceStyle>,
    current_border: Query<&UiResolvedBorderStyle>,
    current_text: Query<&UiResolvedTextStyle>,
    current_button: Query<&UiResolvedButtonStyle>,
    current_input: Query<&UiResolvedInputStyle>,
    current_card: Query<&UiResolvedCardStyle>,
    current_dialog: Query<&UiResolvedDialogStyle>,
    current_debug: Query<&UiResolvedStyleDebugSnapshot>,
) {
    for entity in removed_bindings.read() {
        if bindings.get(entity).is_ok() {
            continue;
        }
        let has_resolved_output = current_surface.get(entity).is_ok()
            || current_border.get(entity).is_ok()
            || current_text.get(entity).is_ok()
            || current_button.get(entity).is_ok()
            || current_input.get(entity).is_ok()
            || current_card.get(entity).is_ok()
            || current_dialog.get(entity).is_ok()
            || current_debug.get(entity).is_ok();
        if has_resolved_output {
            commands.entity(entity).remove::<(
                UiResolvedSurfaceStyle,
                UiResolvedBorderStyle,
                UiResolvedTextStyle,
                UiResolvedButtonStyle,
                UiResolvedInputStyle,
                UiResolvedCardStyle,
                UiResolvedDialogStyle,
                UiResolvedStyleDebugSnapshot,
            )>();
        }
    }

    for (entity, binding) in &bindings {
        let scope_chain = collect_scope_chain(entity, &parents, &scopes);
        let resolved = theme
            .styles
            .resolve_binding(&theme, &metrics, binding, scope_chain);
        sync_component(
            &mut commands,
            entity,
            current_surface.get(entity).ok(),
            resolved.surface,
        );
        sync_component(
            &mut commands,
            entity,
            current_border.get(entity).ok(),
            resolved.border,
        );
        sync_component(
            &mut commands,
            entity,
            current_text.get(entity).ok(),
            resolved.text,
        );
        sync_component(
            &mut commands,
            entity,
            current_button.get(entity).ok(),
            resolved.button,
        );
        sync_component(
            &mut commands,
            entity,
            current_input.get(entity).ok(),
            resolved.input,
        );
        sync_component(
            &mut commands,
            entity,
            current_card.get(entity).ok(),
            resolved.card,
        );
        sync_component(
            &mut commands,
            entity,
            current_dialog.get(entity).ok(),
            resolved.dialog,
        );
        sync_component(
            &mut commands,
            entity,
            current_debug.get(entity).ok(),
            Some(resolved.debug),
        );
    }
}

fn collect_scope_chain(
    entity: Entity,
    parents: &Query<&ChildOf>,
    scopes: &Query<&UiStyleScope>,
) -> Vec<String> {
    let mut lineage = vec![entity];
    lineage.extend(parents.iter_ancestors(entity));
    lineage.reverse();
    lineage
        .into_iter()
        .filter_map(|ancestor| scopes.get(ancestor).ok())
        .map(|scope| scope.variant.as_str().to_owned())
        .collect()
}

fn sync_component<T: Component + PartialEq>(
    commands: &mut Commands,
    entity: Entity,
    current: Option<&T>,
    next: Option<T>,
) {
    match (current, next) {
        (Some(current), Some(next)) if current == &next => {}
        (_, Some(next)) => {
            commands.entity(entity).insert(next);
        }
        (Some(_), None) => {
            commands.entity(entity).remove::<T>();
        }
        (None, None) => {}
    }
}

pub(super) fn apply_resolved_ui_styles(
    mut styles: ParamSet<(
        Query<(&UiResolvedSurfaceStyle, &mut BackgroundColor)>,
        Query<(
            &UiResolvedBorderStyle,
            Option<&mut BorderColor>,
            Option<&mut Node>,
        )>,
        Query<(&UiResolvedTextStyle, Option<&mut TextColor>)>,
        Query<(&UiResolvedTextStyle, &mut TextFont), Without<UiTextStyleToken>>,
        Query<(
            &UiResolvedButtonStyle,
            Option<&mut BorderColor>,
            Option<&mut Node>,
        )>,
        Query<(&UiResolvedInputStyle, Option<&mut Node>)>,
        Query<(
            &UiResolvedCardStyle,
            Option<&mut BackgroundColor>,
            Option<&mut BorderColor>,
            Option<&mut Node>,
        )>,
        Query<(
            &UiResolvedDialogStyle,
            Option<&mut BackgroundColor>,
            Option<&mut BorderColor>,
            Option<&mut Node>,
        )>,
    )>,
) {
    for (style, mut background) in &mut styles.p0() {
        set_if_different(&mut background.0, style.background);
    }
    for (style, border, node) in &mut styles.p1() {
        if let Some(mut border) = border {
            set_if_different(&mut *border, BorderColor::all(style.color));
        }
        if let Some(mut node) = node {
            set_if_different(&mut node.border, UiRect::all(px(style.width)));
            set_if_different(&mut node.border_radius, BorderRadius::all(px(style.radius)));
        }
    }
    for (style, color) in &mut styles.p2() {
        if let Some(mut color) = color {
            if color.0 != style.color {
                color.0 = style.color;
            }
        }
    }
    for (style, mut font) in &mut styles.p3() {
        if font.font_size != style.font_size {
            font.font_size = style.font_size;
        }
    }
    for (style, border, node) in &mut styles.p4() {
        if let Some(mut border) = border {
            set_if_different(&mut *border, BorderColor::all(style.border_color));
        }
        if let Some(mut node) = node {
            set_if_different(&mut node.border, UiRect::all(px(style.border_width)));
            set_if_different(&mut node.border_radius, BorderRadius::all(px(style.radius)));
            set_if_different(&mut node.padding.left, px(style.padding_x));
            set_if_different(&mut node.padding.right, px(style.padding_x));
        }
    }
    for (style, node) in &mut styles.p5() {
        if let Some(mut node) = node {
            set_if_different(&mut node.border, UiRect::all(px(style.border_width)));
            set_if_different(&mut node.border_radius, BorderRadius::all(px(style.radius)));
        }
    }
    for (style, background, border, node) in &mut styles.p6() {
        if let Some(mut background) = background {
            set_if_different(&mut background.0, style.background);
        }
        if let Some(mut border) = border {
            set_if_different(&mut *border, BorderColor::all(style.border));
        }
        if let Some(mut node) = node {
            set_if_different(&mut node.border, UiRect::all(px(style.border_width)));
            set_if_different(&mut node.border_radius, BorderRadius::all(px(style.radius)));
            set_if_different(&mut node.padding, UiRect::all(px(style.padding)));
        }
    }
    for (style, background, border, node) in &mut styles.p7() {
        if let Some(mut background) = background {
            set_if_different(&mut background.0, style.background);
        }
        if let Some(mut border) = border {
            set_if_different(&mut *border, BorderColor::all(style.border));
        }
        if let Some(mut node) = node {
            set_if_different(&mut node.border, UiRect::all(px(style.border_width)));
            set_if_different(&mut node.border_radius, BorderRadius::all(px(style.radius)));
            set_if_different(&mut node.padding, UiRect::all(px(style.padding)));
        }
    }
}

fn set_if_different<T: PartialEq>(current: &mut T, next: T) {
    if current != &next {
        *current = next;
    }
}

fn default_style_sheet_config() -> UiStyleSheetConfig {
    fn color(name: &str, rgb: (f32, f32, f32)) -> UiStyleTokenConfig {
        UiStyleTokenConfig {
            name: name.to_owned(),
            value: UiStyleTokenValueConfig::Color(UiStyleColorConfig {
                r: rgb.0,
                g: rgb.1,
                b: rgb.2,
                a: 1.0,
            }),
        }
    }

    let tokens = vec![
        color("gallery.parent.surface", (0.08, 0.19, 0.18)),
        color("gallery.parent.border", (0.24, 0.76, 0.68)),
        color("gallery.parent.text", (0.84, 0.98, 0.94)),
        color("gallery.parent.button.idle", (0.10, 0.34, 0.31)),
        color("gallery.parent.button.hovered", (0.13, 0.46, 0.41)),
        color("gallery.parent.button.pressed", (0.07, 0.25, 0.23)),
        color("gallery.parent.button.focused", (0.20, 0.58, 0.51)),
        color("gallery.parent.button.selected", (0.10, 0.52, 0.45)),
        color("gallery.parent.button.disabled", (0.08, 0.20, 0.19)),
        color("gallery.parent.button.loading", (0.10, 0.29, 0.28)),
        color("gallery.nested.surface", (0.20, 0.16, 0.08)),
        color("gallery.nested.border", (0.93, 0.70, 0.22)),
        color("gallery.nested.text", (1.0, 0.93, 0.72)),
        color("gallery.nested.button.selected", (0.62, 0.42, 0.08)),
    ];

    let parent_button_states = [
        (UiStyleState::Idle, "gallery.parent.button.idle"),
        (UiStyleState::Hovered, "gallery.parent.button.hovered"),
        (UiStyleState::Pressed, "gallery.parent.button.pressed"),
        (UiStyleState::Focused, "gallery.parent.button.focused"),
        (UiStyleState::Selected, "gallery.parent.button.selected"),
        (UiStyleState::Disabled, "gallery.parent.button.disabled"),
        (UiStyleState::Loading, "gallery.parent.button.loading"),
    ];
    let mut parent_overrides = vec![
        UiStyleOverrideConfig::SurfaceBackground {
            role: UiSurfaceStyleRole::Panel,
            token: "gallery.parent.surface".to_owned(),
        },
        UiStyleOverrideConfig::BorderColor {
            role: UiBorderStyleRole::Panel,
            token: "gallery.parent.border".to_owned(),
        },
        UiStyleOverrideConfig::TextColor {
            role: UiTextStyleRole::Primary,
            token: "gallery.parent.text".to_owned(),
        },
        UiStyleOverrideConfig::TextColor {
            role: UiTextStyleRole::Caption,
            token: "gallery.parent.text".to_owned(),
        },
        UiStyleOverrideConfig::ButtonTextColor {
            role: UiButtonStyleRole::Secondary,
            token: "gallery.parent.text".to_owned(),
        },
        UiStyleOverrideConfig::ButtonBorderColor {
            role: UiButtonStyleRole::Secondary,
            token: "gallery.parent.border".to_owned(),
        },
    ];
    parent_overrides.extend(parent_button_states.into_iter().flat_map(|(state, token)| {
        [
            UiStyleOverrideConfig::ButtonBackground {
                role: UiButtonStyleRole::Secondary,
                state,
                token: token.to_owned(),
            },
            UiStyleOverrideConfig::ButtonIconTint {
                role: UiButtonStyleRole::Secondary,
                state,
                token: "gallery.parent.text".to_owned(),
            },
        ]
    }));

    UiStyleSheetConfig {
        tokens,
        variants: vec![
            UiStyleVariantConfig {
                name: UI_STYLE_VARIANT_GALLERY_PARENT.to_owned(),
                extends: None,
                probes: Vec::new(),
                overrides: parent_overrides,
            },
            UiStyleVariantConfig {
                name: UI_STYLE_VARIANT_GALLERY_NESTED.to_owned(),
                extends: Some(UI_STYLE_VARIANT_GALLERY_PARENT.to_owned()),
                probes: Vec::new(),
                overrides: vec![
                    UiStyleOverrideConfig::SurfaceBackground {
                        role: UiSurfaceStyleRole::Panel,
                        token: "gallery.nested.surface".to_owned(),
                    },
                    UiStyleOverrideConfig::BorderColor {
                        role: UiBorderStyleRole::Panel,
                        token: "gallery.nested.border".to_owned(),
                    },
                    UiStyleOverrideConfig::TextColor {
                        role: UiTextStyleRole::Primary,
                        token: "gallery.nested.text".to_owned(),
                    },
                    UiStyleOverrideConfig::TextColor {
                        role: UiTextStyleRole::Caption,
                        token: "gallery.nested.text".to_owned(),
                    },
                    UiStyleOverrideConfig::ButtonBackground {
                        role: UiButtonStyleRole::Secondary,
                        state: UiStyleState::Selected,
                        token: "gallery.nested.button.selected".to_owned(),
                    },
                    UiStyleOverrideConfig::ButtonTextColor {
                        role: UiButtonStyleRole::Secondary,
                        token: "gallery.nested.text".to_owned(),
                    },
                    UiStyleOverrideConfig::ButtonBorderColor {
                        role: UiButtonStyleRole::Secondary,
                        token: "gallery.nested.border".to_owned(),
                    },
                ],
            },
        ],
    }
}

fn compile_overrides(
    variant: &str,
    overrides: Vec<UiStyleOverrideConfig>,
    tokens: &BTreeMap<String, UiStyleTokenValue>,
) -> Result<Vec<UiCompiledStyleOverride>, UiStyleError> {
    let mut seen = BTreeSet::new();
    overrides
        .into_iter()
        .map(|value| {
            let key = value.key();
            if !seen.insert(key.clone()) {
                return Err(UiStyleError::new(
                    UiStyleErrorCode::DuplicateOverride,
                    format!("variant '{variant}' repeats override '{key}'"),
                ));
            }
            value.compile(variant, tokens)
        })
        .collect()
}

impl UiStyleOverrideConfig {
    fn key(&self) -> String {
        match self {
            Self::SurfaceBackground { role, .. } => format!("surface.{role:?}.background"),
            Self::BorderColor { role, .. } => format!("border.{role:?}.color"),
            Self::BorderWidth { role, .. } => format!("border.{role:?}.width"),
            Self::BorderRadius { role, .. } => format!("border.{role:?}.radius"),
            Self::TextColor { role, .. } => format!("text.{role:?}.color"),
            Self::TextSize { role, .. } => format!("text.{role:?}.size"),
            Self::ButtonBackground { role, state, .. } => {
                format!("button.{role:?}.background.{state:?}")
            }
            Self::ButtonIconTint { role, state, .. } => {
                format!("button.{role:?}.icon_tint.{state:?}")
            }
            Self::ButtonTextColor { role, .. } => format!("button.{role:?}.text"),
            Self::ButtonBorderColor { role, .. } => format!("button.{role:?}.border"),
            Self::ButtonBorderWidth { role, .. } => format!("button.{role:?}.border_width"),
            Self::ButtonRadius { role, .. } => format!("button.{role:?}.radius"),
            Self::ButtonPaddingX { role, .. } => format!("button.{role:?}.padding_x"),
            Self::InputBackground { role, state, .. } => {
                format!("input.{role:?}.background.{state:?}")
            }
            Self::InputBorder { role, part, .. } => format!("input.{role:?}.border.{part:?}"),
            Self::InputText { role, .. } => format!("input.{role:?}.text"),
            Self::InputPlaceholder { role, .. } => format!("input.{role:?}.placeholder"),
            Self::InputErrorText { role, .. } => format!("input.{role:?}.error_text"),
            Self::InputSelectionText { role, .. } => format!("input.{role:?}.selection_text"),
            Self::InputSelectionBackground { role, .. } => {
                format!("input.{role:?}.selection_background")
            }
            Self::InputBorderWidth { role, .. } => format!("input.{role:?}.border_width"),
            Self::InputRadius { role, .. } => format!("input.{role:?}.radius"),
            Self::CardBackground { role, .. } => format!("card.{role:?}.background"),
            Self::CardBorder { role, .. } => format!("card.{role:?}.border"),
            Self::CardBorderWidth { role, .. } => format!("card.{role:?}.border_width"),
            Self::CardRadius { role, .. } => format!("card.{role:?}.radius"),
            Self::CardPadding { role, .. } => format!("card.{role:?}.padding"),
            Self::DialogBackground { role, .. } => format!("dialog.{role:?}.background"),
            Self::DialogBorder { role, .. } => format!("dialog.{role:?}.border"),
            Self::DialogBorderWidth { role, .. } => format!("dialog.{role:?}.border_width"),
            Self::DialogRadius { role, .. } => format!("dialog.{role:?}.radius"),
            Self::DialogPadding { role, .. } => format!("dialog.{role:?}.padding"),
        }
    }

    fn compile(
        self,
        variant: &str,
        tokens: &BTreeMap<String, UiStyleTokenValue>,
    ) -> Result<UiCompiledStyleOverride, UiStyleError> {
        macro_rules! color {
            ($token:expr) => {
                expect_color_token(tokens, variant, &$token)?
            };
        }
        macro_rules! scalar {
            ($token:expr) => {
                expect_scalar_token(tokens, variant, &$token)?
            };
        }
        macro_rules! positive {
            ($token:expr) => {
                expect_positive_scalar_token(tokens, variant, &$token)?
            };
        }
        Ok(match self {
            Self::SurfaceBackground { role, token } => {
                UiCompiledStyleOverride::SurfaceBackground(role, color!(token))
            }
            Self::BorderColor { role, token } => {
                UiCompiledStyleOverride::BorderColor(role, color!(token))
            }
            Self::BorderWidth { role, token } => {
                UiCompiledStyleOverride::BorderWidth(role, scalar!(token))
            }
            Self::BorderRadius { role, token } => {
                UiCompiledStyleOverride::BorderRadius(role, scalar!(token))
            }
            Self::TextColor { role, token } => {
                UiCompiledStyleOverride::TextColor(role, color!(token))
            }
            Self::TextSize { role, token } => {
                UiCompiledStyleOverride::TextSize(role, positive!(token))
            }
            Self::ButtonBackground { role, state, token } => {
                UiCompiledStyleOverride::ButtonBackground(role, state, color!(token))
            }
            Self::ButtonIconTint { role, state, token } => {
                UiCompiledStyleOverride::ButtonIconTint(role, state, color!(token))
            }
            Self::ButtonTextColor { role, token } => {
                UiCompiledStyleOverride::ButtonTextColor(role, color!(token))
            }
            Self::ButtonBorderColor { role, token } => {
                UiCompiledStyleOverride::ButtonBorderColor(role, color!(token))
            }
            Self::ButtonBorderWidth { role, token } => {
                UiCompiledStyleOverride::ButtonBorderWidth(role, scalar!(token))
            }
            Self::ButtonRadius { role, token } => {
                UiCompiledStyleOverride::ButtonRadius(role, scalar!(token))
            }
            Self::ButtonPaddingX { role, token } => {
                UiCompiledStyleOverride::ButtonPaddingX(role, scalar!(token))
            }
            Self::InputBackground { role, state, token } => {
                UiCompiledStyleOverride::InputBackground(role, state, color!(token))
            }
            Self::InputBorder { role, part, token } => {
                UiCompiledStyleOverride::InputBorder(role, part, color!(token))
            }
            Self::InputText { role, token } => {
                UiCompiledStyleOverride::InputText(role, color!(token))
            }
            Self::InputPlaceholder { role, token } => {
                UiCompiledStyleOverride::InputPlaceholder(role, color!(token))
            }
            Self::InputErrorText { role, token } => {
                UiCompiledStyleOverride::InputErrorText(role, color!(token))
            }
            Self::InputSelectionText { role, token } => {
                UiCompiledStyleOverride::InputSelectionText(role, color!(token))
            }
            Self::InputSelectionBackground { role, token } => {
                UiCompiledStyleOverride::InputSelectionBackground(role, color!(token))
            }
            Self::InputBorderWidth { role, token } => {
                UiCompiledStyleOverride::InputBorderWidth(role, scalar!(token))
            }
            Self::InputRadius { role, token } => {
                UiCompiledStyleOverride::InputRadius(role, scalar!(token))
            }
            Self::CardBackground { role, token } => {
                UiCompiledStyleOverride::CardBackground(role, color!(token))
            }
            Self::CardBorder { role, token } => {
                UiCompiledStyleOverride::CardBorder(role, color!(token))
            }
            Self::CardBorderWidth { role, token } => {
                UiCompiledStyleOverride::CardBorderWidth(role, scalar!(token))
            }
            Self::CardRadius { role, token } => {
                UiCompiledStyleOverride::CardRadius(role, scalar!(token))
            }
            Self::CardPadding { role, token } => {
                UiCompiledStyleOverride::CardPadding(role, scalar!(token))
            }
            Self::DialogBackground { role, token } => {
                UiCompiledStyleOverride::DialogBackground(role, color!(token))
            }
            Self::DialogBorder { role, token } => {
                UiCompiledStyleOverride::DialogBorder(role, color!(token))
            }
            Self::DialogBorderWidth { role, token } => {
                UiCompiledStyleOverride::DialogBorderWidth(role, scalar!(token))
            }
            Self::DialogRadius { role, token } => {
                UiCompiledStyleOverride::DialogRadius(role, scalar!(token))
            }
            Self::DialogPadding { role, token } => {
                UiCompiledStyleOverride::DialogPadding(role, scalar!(token))
            }
        })
    }
}

fn expect_color_token(
    tokens: &BTreeMap<String, UiStyleTokenValue>,
    variant: &str,
    token: &str,
) -> Result<Color, UiStyleError> {
    match tokens.get(token).copied() {
        Some(UiStyleTokenValue::Color(value)) => Ok(value),
        Some(value) => Err(UiStyleError::new(
            UiStyleErrorCode::TokenTypeMismatch,
            format!(
                "variant '{variant}' expected Color token '{token}', found {:?}",
                value.kind()
            ),
        )),
        None => Err(UiStyleError::new(
            UiStyleErrorCode::UnknownToken,
            format!("variant '{variant}' references unknown token '{token}'"),
        )),
    }
}

fn expect_scalar_token(
    tokens: &BTreeMap<String, UiStyleTokenValue>,
    variant: &str,
    token: &str,
) -> Result<f32, UiStyleError> {
    match tokens.get(token).copied() {
        Some(UiStyleTokenValue::Scalar(value)) if value >= 0.0 => Ok(value),
        Some(UiStyleTokenValue::Scalar(_)) => Err(UiStyleError::new(
            UiStyleErrorCode::InvalidValue,
            format!("variant '{variant}' uses negative scalar token '{token}' for a size"),
        )),
        Some(value) => Err(UiStyleError::new(
            UiStyleErrorCode::TokenTypeMismatch,
            format!(
                "variant '{variant}' expected Scalar token '{token}', found {:?}",
                value.kind()
            ),
        )),
        None => Err(UiStyleError::new(
            UiStyleErrorCode::UnknownToken,
            format!("variant '{variant}' references unknown token '{token}'"),
        )),
    }
}

fn expect_positive_scalar_token(
    tokens: &BTreeMap<String, UiStyleTokenValue>,
    variant: &str,
    token: &str,
) -> Result<f32, UiStyleError> {
    let value = expect_scalar_token(tokens, variant, token)?;
    if value > 0.0 {
        Ok(value)
    } else {
        Err(UiStyleError::new(
            UiStyleErrorCode::InvalidValue,
            format!("variant '{variant}' requires positive scalar token '{token}'"),
        ))
    }
}

impl UiStyleTokenValueConfig {
    fn compile(self, name: &str) -> Result<UiStyleTokenValue, UiStyleError> {
        match self {
            Self::Color(color) => color.compile(name).map(UiStyleTokenValue::Color),
            Self::Scalar(value) if value.is_finite() => Ok(UiStyleTokenValue::Scalar(value)),
            Self::Scalar(_) => Err(UiStyleError::new(
                UiStyleErrorCode::InvalidValue,
                format!("scalar token '{name}' must be finite"),
            )),
        }
    }
}

impl UiStyleColorConfig {
    fn compile(self, name: &str) -> Result<Color, UiStyleError> {
        if ![self.r, self.g, self.b, self.a]
            .into_iter()
            .all(f32::is_finite)
        {
            return Err(UiStyleError::new(
                UiStyleErrorCode::InvalidValue,
                format!("color token '{name}' channels must be finite"),
            ));
        }
        if ![self.r, self.g, self.b, self.a]
            .into_iter()
            .all(|channel| (0.0..=1.0).contains(&channel))
        {
            return Err(UiStyleError::new(
                UiStyleErrorCode::InvalidValue,
                format!("color token '{name}' channels must be within 0..=1"),
            ));
        }
        Ok(Color::srgba(self.r, self.g, self.b, self.a))
    }
}

fn default_alpha() -> f32 {
    1.0
}

fn validate_name(kind: &str, name: &str) -> Result<(), UiStyleError> {
    if name.trim().is_empty() || name.trim() != name {
        return Err(UiStyleError::new(
            UiStyleErrorCode::InvalidValue,
            format!("{kind} name must be non-empty and have no outer whitespace"),
        ));
    }
    Ok(())
}

fn validate_variant_cycles(
    variants: &BTreeMap<String, UiCompiledStyleVariant>,
) -> Result<(), UiStyleError> {
    fn visit(
        name: &str,
        variants: &BTreeMap<String, UiCompiledStyleVariant>,
        visiting: &mut BTreeSet<String>,
        visited: &mut BTreeSet<String>,
    ) -> Result<(), UiStyleError> {
        if visited.contains(name) {
            return Ok(());
        }
        if !visiting.insert(name.to_owned()) {
            let mut cycle = visiting.iter().cloned().collect::<Vec<_>>();
            cycle.push(name.to_owned());
            return Err(UiStyleError::new(
                UiStyleErrorCode::VariantCycle,
                format!("variant inheritance cycle: {}", cycle.join(" -> ")),
            ));
        }
        if let Some(parent) = variants
            .get(name)
            .and_then(|variant| variant.extends.as_deref())
        {
            visit(parent, variants, visiting, visited)?;
        }
        visiting.remove(name);
        visited.insert(name.to_owned());
        Ok(())
    }

    let mut visited = BTreeSet::new();
    for name in variants.keys() {
        visit(name, variants, &mut BTreeSet::new(), &mut visited)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::ui::{
        core::UiMetrics,
        i18n::UiI18n,
        style::UiFontAssets,
        widgets::{
            DisabledButton, FocusedButton, LoadingButton, SelectedButton, UiIconButton, UiIconId,
            UiIconLabelPlacement, UiIconVisual, UiTextInput, UiTextInputValue,
            controls::{
                SecondaryButton, UiButtonStyleLabel, sync_button_style_labels,
                sync_icon_button_nodes, update_button_visuals, update_icon_button_visuals,
                update_text_input_visuals,
            },
            disabled_secondary_action_button_key, icon_button_key, icon_label_button_key,
            secondary_action_button_key,
        },
    };
    use bevy::asset::AssetPlugin;

    fn compile(source: &str) -> Result<UiStyleSheet, UiStyleError> {
        let config: UiStyleSheetConfig = ron::from_str(source).expect("style config should parse");
        UiStyleSheet::compile(config)
    }

    fn scoped_style_app() -> App {
        let mut app = App::new();
        app.insert_resource(UiTheme::default())
            .insert_resource(UiMetrics::default())
            .add_systems(
                Update,
                (resolve_ui_style_bindings, apply_resolved_ui_styles).chain(),
            );
        app
    }

    fn stateful_style_app() -> App {
        let mut app = App::new();
        app.insert_resource(UiTheme::default())
            .insert_resource(UiMetrics::default())
            .add_systems(
                Update,
                (
                    resolve_ui_style_bindings,
                    apply_resolved_ui_styles,
                    sync_icon_button_nodes,
                    sync_button_style_labels,
                    update_button_visuals,
                    update_text_input_visuals,
                )
                    .chain(),
            );
        app
    }

    fn scoped_widget_style_app(theme: UiTheme) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>()
            .insert_resource(theme)
            .insert_resource(UiMetrics::default())
            .add_systems(
                Update,
                (
                    resolve_ui_style_bindings,
                    apply_resolved_ui_styles,
                    sync_icon_button_nodes,
                    sync_button_style_labels,
                )
                    .chain(),
            );
        app.finish();
        app.cleanup();
        app
    }

    fn button_style_label(world: &World, button: Entity) -> Entity {
        world
            .get::<Children>(button)
            .unwrap()
            .iter()
            .find(|child| world.get::<UiButtonStyleLabel>(*child).is_some())
            .expect("button should own a visible style label")
    }

    fn button_binding_with_variant(variant: &str) -> UiStyleBinding {
        let mut binding = UiStyleBinding::new().with_button(UiButtonStyleRole::Secondary);
        binding.button.as_mut().unwrap().variant = Some(UiStyleVariantId::new(variant));
        binding
    }

    #[test]
    fn typed_style_binding_composes_roles_without_strings() {
        let mut binding = UiStyleBinding::new()
            .with_surface(UiSurfaceStyleRole::Panel)
            .with_border(UiBorderStyleRole::Panel)
            .with_text(UiTextStyleRole::Primary)
            .with_button(UiButtonStyleRole::Secondary)
            .with_input(UiInputStyleRole::Standard)
            .with_card(UiCardStyleRole::Standard)
            .with_dialog(UiDialogStyleRole::Standard);
        binding.surface =
            Some(UiStyleRef::new(UiSurfaceStyleRole::Panel).with_variant("page.compact"));

        assert_eq!(
            binding.surface.as_ref().unwrap().role,
            UiSurfaceStyleRole::Panel
        );
        assert_eq!(binding.button.unwrap().role, UiButtonStyleRole::Secondary);
        assert_eq!(
            binding
                .surface
                .as_ref()
                .unwrap()
                .variant
                .as_ref()
                .unwrap()
                .as_str(),
            "page.compact"
        );
    }

    #[test]
    fn compiler_accepts_typed_tokens_and_variant_inheritance() {
        let sheet = compile(
            r#"(
                tokens: [
                    (name: "surface.parent", value: Color((r: 0.1, g: 0.2, b: 0.3))),
                    (name: "radius.compact", value: Scalar(6.0)),
                ],
                variants: [
                    (name: "parent", probes: [
                        (token: "surface.parent", expected: color),
                    ]),
                    (name: "nested", extends: Some("parent"), probes: [
                        (token: "radius.compact", expected: scalar),
                    ]),
                ],
            )"#,
        )
        .unwrap();

        assert!(sheet.contains_variant(&UiStyleVariantId::new("parent")));
        assert!(sheet.contains_variant(&UiStyleVariantId::new("nested")));
    }

    #[test]
    fn compiler_returns_stable_unknown_token_code() {
        let error = compile(
            r#"(
                variants: [(name: "bad", probes: [
                    (token: "missing", expected: color),
                ])],
            )"#,
        )
        .unwrap_err();

        assert_eq!(error.code, UiStyleErrorCode::UnknownToken);
        assert_eq!(error.code.as_str(), "ui_style_unknown_token");
    }

    #[test]
    fn compiler_returns_stable_unknown_variant_code() {
        let error =
            compile(r#"(variants: [(name: "child", extends: Some("missing"))])"#).unwrap_err();

        assert_eq!(error.code, UiStyleErrorCode::UnknownVariant);
    }

    #[test]
    fn compiler_returns_stable_cycle_code() {
        let error = compile(
            r#"(variants: [
                (name: "a", extends: Some("b")),
                (name: "b", extends: Some("a")),
            ])"#,
        )
        .unwrap_err();

        assert_eq!(error.code, UiStyleErrorCode::VariantCycle);
    }

    #[test]
    fn compiler_returns_stable_duplicate_variant_code() {
        let error = compile(r#"(variants: [(name: "same"), (name: "same")])"#).unwrap_err();

        assert_eq!(error.code, UiStyleErrorCode::DuplicateVariant);
    }

    #[test]
    fn compiler_returns_stable_type_mismatch_code() {
        let error = compile(
            r#"(
                tokens: [(name: "radius", value: Scalar(4.0))],
                variants: [(name: "bad", probes: [
                    (token: "radius", expected: color),
                ])],
            )"#,
        )
        .unwrap_err();

        assert_eq!(error.code, UiStyleErrorCode::TokenTypeMismatch);
    }

    #[test]
    fn compiler_rejects_out_of_range_color_channels() {
        let error = compile(
            r#"(tokens: [
                (name: "bad", value: Color((r: 1.1, g: 0.2, b: 0.3))),
            ])"#,
        )
        .unwrap_err();

        assert_eq!(error.code, UiStyleErrorCode::InvalidValue);
        assert!(error.detail.contains("0..=1"));
    }

    #[test]
    fn compiler_rejects_negative_size_override() {
        let error = compile(
            r#"(
                tokens: [(name: "negative", value: Scalar(-2.0))],
                variants: [(name: "bad", overrides: [
                    BorderWidth(role: panel, token: "negative"),
                ])],
            )"#,
        )
        .unwrap_err();

        assert_eq!(error.code, UiStyleErrorCode::InvalidValue);
    }

    #[test]
    fn compiler_reports_duplicate_override_separately() {
        let error = compile(
            r#"(
                tokens: [(name: "color", value: Color((r: 0.1, g: 0.2, b: 0.3)))],
                variants: [(name: "bad", overrides: [
                    SurfaceBackground(role: panel, token: "color"),
                    SurfaceBackground(role: panel, token: "color"),
                ])],
            )"#,
        )
        .unwrap_err();

        assert_eq!(error.code, UiStyleErrorCode::DuplicateOverride);
        assert_eq!(error.code.as_str(), "ui_style_duplicate_override");
    }

    #[test]
    fn ecs_resolver_inherits_nested_scope_and_restores_after_removal() {
        let mut app = scoped_style_app();
        let parent = app
            .world_mut()
            .spawn(UiStyleScope::new(UI_STYLE_VARIANT_GALLERY_PARENT))
            .id();
        let nested = app
            .world_mut()
            .spawn(UiStyleScope::new(UI_STYLE_VARIANT_GALLERY_NESTED))
            .id();
        let target = app
            .world_mut()
            .spawn((
                UiStyleBinding::new()
                    .with_surface(UiSurfaceStyleRole::Panel)
                    .with_border(UiBorderStyleRole::Panel),
                BackgroundColor(Color::NONE),
                BorderColor::all(Color::NONE),
                Node::default(),
            ))
            .id();
        app.world_mut().entity_mut(parent).add_child(nested);
        app.world_mut().entity_mut(nested).add_child(target);

        app.update();

        assert_eq!(
            app.world().get::<BackgroundColor>(target).unwrap().0,
            Color::srgb(0.20, 0.16, 0.08)
        );
        let snapshot = app
            .world()
            .get::<UiResolvedStyleDebugSnapshot>(target)
            .unwrap();
        assert_eq!(
            snapshot.scopes,
            vec![
                UI_STYLE_VARIANT_GALLERY_PARENT.to_owned(),
                UI_STYLE_VARIANT_GALLERY_NESTED.to_owned(),
            ]
        );
        assert!(snapshot.entries.iter().all(|entry| !entry.fallback));

        app.world_mut().entity_mut(nested).remove::<UiStyleScope>();
        app.update();
        assert_eq!(
            app.world().get::<BackgroundColor>(target).unwrap().0,
            Color::srgb(0.08, 0.19, 0.18)
        );

        app.world_mut().entity_mut(parent).remove::<UiStyleScope>();
        app.update();
        assert_eq!(
            app.world().get::<BackgroundColor>(target).unwrap().0,
            UiTheme::default().colors.panel_background
        );
        assert!(
            app.world()
                .get::<UiResolvedStyleDebugSnapshot>(target)
                .unwrap()
                .scopes
                .is_empty()
        );
    }

    #[test]
    fn ecs_resolver_stable_second_frame_does_not_mark_outputs_changed() {
        let mut app = scoped_style_app();
        let target = app
            .world_mut()
            .spawn((
                UiStyleScope::new(UI_STYLE_VARIANT_GALLERY_PARENT),
                UiStyleBinding::new()
                    .with_surface(UiSurfaceStyleRole::Panel)
                    .with_border(UiBorderStyleRole::Panel),
                BackgroundColor(Color::NONE),
                BorderColor::all(Color::NONE),
                Node::default(),
            ))
            .id();
        app.update();
        app.world_mut().clear_trackers();

        app.update();

        let entity = app.world().entity(target);
        assert!(
            !entity
                .get_ref::<UiResolvedSurfaceStyle>()
                .unwrap()
                .is_changed()
        );
        assert!(
            !entity
                .get_ref::<UiResolvedBorderStyle>()
                .unwrap()
                .is_changed()
        );
        assert!(
            !entity
                .get_ref::<UiResolvedStyleDebugSnapshot>()
                .unwrap()
                .is_changed()
        );
        assert!(!entity.get_ref::<BackgroundColor>().unwrap().is_changed());
        assert!(!entity.get_ref::<BorderColor>().unwrap().is_changed());
        assert!(!entity.get_ref::<Node>().unwrap().is_changed());
    }

    #[test]
    fn raw_text_font_without_style_token_consumes_resolved_size_and_stays_stable() {
        let mut app = scoped_style_app();
        let styles = compile(
            r#"(
                tokens: [(name: "size", value: Scalar(29.0))],
                variants: [(name: "raw", overrides: [
                    TextSize(role: primary, token: "size"),
                ])],
            )"#,
        )
        .unwrap();
        app.world_mut().resource_mut::<UiTheme>().styles = styles;
        let mut binding = UiStyleBinding::new().with_text(UiTextStyleRole::Primary);
        binding.text.as_mut().unwrap().variant = Some(UiStyleVariantId::new("raw"));
        let target = app
            .world_mut()
            .spawn((
                Text::new("Raw text"),
                TextFont::from_font_size(3.0),
                TextColor(Color::NONE),
                binding,
            ))
            .id();

        app.update();

        let entity = app.world().entity(target);
        assert!(!entity.contains::<UiTextStyleToken>());
        assert_eq!(entity.get::<TextFont>().unwrap().font_size, 29.0);

        app.world_mut().clear_trackers();
        app.update();
        assert!(
            !app.world()
                .entity(target)
                .get_ref::<TextFont>()
                .unwrap()
                .is_changed()
        );
    }

    #[test]
    fn scoped_button_text_reaches_visible_labels_and_unbind_restores_role_colors() {
        let theme = UiTheme::default();
        let metrics = UiMetrics::default();
        let fonts = UiFontAssets::test_registry();
        let i18n = UiI18n::test_with_texts(
            "en_us",
            &[
                ("scope.normal", "Normal"),
                ("scope.disabled", "Disabled"),
                ("scope.icon", "Icon label"),
            ],
        );
        let mut app = scoped_widget_style_app(theme.clone());
        let normal_bundle =
            secondary_action_button_key(&theme, &metrics, &fonts, &i18n, "scope.normal", "Normal");
        let normal = app
            .world_mut()
            .spawn((
                normal_bundle,
                UiStyleBinding::new().with_button(UiButtonStyleRole::Secondary),
            ))
            .id();
        let disabled_bundle = disabled_secondary_action_button_key(
            &theme,
            &metrics,
            &fonts,
            &i18n,
            "scope.disabled",
            "Disabled",
        );
        let disabled = app
            .world_mut()
            .spawn((
                disabled_bundle,
                UiStyleBinding::new().with_button(UiButtonStyleRole::Secondary),
            ))
            .id();
        let icon_bundle = icon_label_button_key(
            &theme,
            &metrics,
            &fonts,
            app.world().resource::<AssetServer>(),
            &i18n,
            UiIconId::HELP,
            UiIconLabelPlacement::Leading,
            "scope.icon",
            "Icon label",
        );
        let icon = app
            .world_mut()
            .spawn((
                icon_bundle,
                UiStyleBinding::new().with_button(UiButtonStyleRole::Secondary),
            ))
            .id();
        let scope = app
            .world_mut()
            .spawn(UiStyleScope::new(UI_STYLE_VARIANT_GALLERY_PARENT))
            .id();
        for button in [normal, disabled, icon] {
            app.world_mut().entity_mut(scope).add_child(button);
        }

        app.update();

        let normal_label = button_style_label(app.world(), normal);
        let disabled_label = button_style_label(app.world(), disabled);
        let icon_label = button_style_label(app.world(), icon);
        let scoped_text = Color::srgb(0.84, 0.98, 0.94);
        for label in [normal_label, disabled_label, icon_label] {
            assert_eq!(app.world().get::<TextColor>(label).unwrap().0, scoped_text);
        }

        for button in [normal, disabled, icon] {
            app.world_mut()
                .entity_mut(button)
                .remove::<UiStyleBinding>();
        }
        app.update();

        assert_eq!(
            app.world().get::<TextColor>(normal_label).unwrap().0,
            theme.colors.text_primary
        );
        assert_eq!(
            app.world().get::<TextColor>(icon_label).unwrap().0,
            theme.colors.text_primary
        );
        assert_eq!(
            app.world().get::<TextColor>(disabled_label).unwrap().0,
            theme.colors.text_muted
        );
    }

    #[test]
    fn scoped_icon_button_layout_converges_on_initial_and_hot_reload_frames() {
        let first_styles = compile(
            r#"(
                tokens: [
                    (name: "radius", value: Scalar(17.0)),
                    (name: "padding", value: Scalar(19.0)),
                ],
                variants: [(name: "buttons", overrides: [
                    ButtonRadius(role: secondary, token: "radius"),
                    ButtonPaddingX(role: secondary, token: "padding"),
                ])],
            )"#,
        )
        .unwrap();
        let mut theme = UiTheme::default();
        theme.styles = first_styles;
        let metrics = UiMetrics::default();
        let fonts = UiFontAssets::test_registry();
        let i18n = UiI18n::test_with_texts(
            "en_us",
            &[("scope.icon", "Icon"), ("scope.labeled", "Labeled")],
        );
        let mut app = scoped_widget_style_app(theme.clone());
        let icon_bundle = icon_button_key(
            &theme,
            &metrics,
            &fonts,
            app.world().resource::<AssetServer>(),
            &i18n,
            UiIconId::HELP,
            "scope.icon",
            "Icon",
        );
        let icon = app
            .world_mut()
            .spawn((icon_bundle, button_binding_with_variant("buttons")))
            .id();
        let labeled_bundle = icon_label_button_key(
            &theme,
            &metrics,
            &fonts,
            app.world().resource::<AssetServer>(),
            &i18n,
            UiIconId::HELP,
            UiIconLabelPlacement::Trailing,
            "scope.labeled",
            "Labeled",
        );
        let labeled = app
            .world_mut()
            .spawn((labeled_bundle, button_binding_with_variant("buttons")))
            .id();

        app.update();

        let icon_node = app.world().get::<Node>(icon).unwrap();
        assert_eq!(icon_node.padding, UiRect::ZERO);
        assert_eq!(icon_node.border_radius, BorderRadius::all(px(17)));
        let labeled_node = app.world().get::<Node>(labeled).unwrap();
        assert_eq!(labeled_node.padding, UiRect::axes(px(19), px(0)));
        assert_eq!(labeled_node.border_radius, BorderRadius::all(px(17)));

        let next_styles = compile(
            r#"(
                tokens: [
                    (name: "radius", value: Scalar(7.0)),
                    (name: "padding", value: Scalar(13.0)),
                ],
                variants: [(name: "buttons", overrides: [
                    ButtonRadius(role: secondary, token: "radius"),
                    ButtonPaddingX(role: secondary, token: "padding"),
                ])],
            )"#,
        )
        .unwrap();
        app.world_mut().resource_mut::<UiTheme>().styles = next_styles;
        app.update();

        let icon_node = app.world().get::<Node>(icon).unwrap();
        assert_eq!(icon_node.padding, UiRect::ZERO);
        assert_eq!(icon_node.border_radius, BorderRadius::all(px(7)));
        let labeled_node = app.world().get::<Node>(labeled).unwrap();
        assert_eq!(labeled_node.padding, UiRect::axes(px(13), px(0)));
        assert_eq!(labeled_node.border_radius, BorderRadius::all(px(7)));

        app.world_mut().clear_trackers();
        app.update();
        assert!(
            !app.world()
                .entity(icon)
                .get_ref::<Node>()
                .unwrap()
                .is_changed()
        );
        assert!(
            !app.world()
                .entity(labeled)
                .get_ref::<Node>()
                .unwrap()
                .is_changed()
        );
    }

    #[test]
    fn removing_binding_cleans_all_resolved_outputs_without_touching_runtime_state() {
        let mut app = scoped_style_app();
        let target = app
            .world_mut()
            .spawn((
                Button,
                UiTextInput,
                Interaction::Pressed,
                FocusedButton,
                SelectedButton,
                DisabledButton,
                LoadingButton,
                UiTextInputValue("Pilot 01".to_owned()),
                UiStyleBinding::new()
                    .with_surface(UiSurfaceStyleRole::Panel)
                    .with_border(UiBorderStyleRole::Panel)
                    .with_text(UiTextStyleRole::Primary)
                    .with_button(UiButtonStyleRole::Secondary)
                    .with_input(UiInputStyleRole::Standard)
                    .with_card(UiCardStyleRole::Standard)
                    .with_dialog(UiDialogStyleRole::Standard),
                BackgroundColor(Color::NONE),
                BorderColor::all(Color::NONE),
                TextColor(Color::NONE),
                TextFont::default(),
                Node::default(),
            ))
            .id();
        app.update();

        let entity = app.world().entity(target);
        assert!(entity.contains::<UiResolvedSurfaceStyle>());
        assert!(entity.contains::<UiResolvedBorderStyle>());
        assert!(entity.contains::<UiResolvedTextStyle>());
        assert!(entity.contains::<UiResolvedButtonStyle>());
        assert!(entity.contains::<UiResolvedInputStyle>());
        assert!(entity.contains::<UiResolvedCardStyle>());
        assert!(entity.contains::<UiResolvedDialogStyle>());
        assert!(entity.contains::<UiResolvedStyleDebugSnapshot>());

        app.world_mut()
            .entity_mut(target)
            .remove::<UiStyleBinding>();
        app.update();

        let entity = app.world().entity(target);
        assert!(!entity.contains::<UiResolvedSurfaceStyle>());
        assert!(!entity.contains::<UiResolvedBorderStyle>());
        assert!(!entity.contains::<UiResolvedTextStyle>());
        assert!(!entity.contains::<UiResolvedButtonStyle>());
        assert!(!entity.contains::<UiResolvedInputStyle>());
        assert!(!entity.contains::<UiResolvedCardStyle>());
        assert!(!entity.contains::<UiResolvedDialogStyle>());
        assert!(!entity.contains::<UiResolvedStyleDebugSnapshot>());
        assert_eq!(entity.get::<Interaction>(), Some(&Interaction::Pressed));
        assert!(entity.contains::<FocusedButton>());
        assert!(entity.contains::<SelectedButton>());
        assert!(entity.contains::<DisabledButton>());
        assert!(entity.contains::<LoadingButton>());
        assert_eq!(entity.get::<UiTextInputValue>().unwrap().0, "Pilot 01");

        let snapshot_count = {
            let world = app.world_mut();
            let mut query = world.query::<&UiResolvedStyleDebugSnapshot>();
            query.iter(world).count()
        };
        assert_eq!(
            snapshot_count, 0,
            "audit metadata query must not see stale snapshots"
        );
    }

    #[test]
    fn same_frame_binding_reinsert_is_resolved_instead_of_cleaned() {
        let mut app = scoped_style_app();
        let target = app
            .world_mut()
            .spawn((
                UiStyleBinding::new().with_surface(UiSurfaceStyleRole::Panel),
                BackgroundColor(Color::NONE),
                TextColor(Color::NONE),
                TextFont::default(),
            ))
            .id();
        app.update();

        app.world_mut()
            .entity_mut(target)
            .remove::<UiStyleBinding>()
            .insert(UiStyleBinding::new().with_text(UiTextStyleRole::Muted));
        app.update();

        let entity = app.world().entity(target);
        assert!(!entity.contains::<UiResolvedSurfaceStyle>());
        assert!(entity.contains::<UiResolvedTextStyle>());
        let snapshot = entity.get::<UiResolvedStyleDebugSnapshot>().unwrap();
        assert_eq!(snapshot.entries.len(), 1);
        assert_eq!(snapshot.entries[0].request, "text.muted");
    }

    #[test]
    fn theme_refresh_updates_resolved_button_without_overwriting_runtime_state() {
        let mut app = stateful_style_app();
        let button = app
            .world_mut()
            .spawn((
                Button,
                SecondaryButton,
                SelectedButton,
                Interaction::None,
                UiStyleBinding::new().with_button(UiButtonStyleRole::Secondary),
                BackgroundColor(Color::NONE),
                BorderColor::all(Color::NONE),
                Node::default(),
            ))
            .id();
        app.update();

        let replacement = Color::srgb(0.72, 0.15, 0.18);
        app.world_mut()
            .resource_mut::<UiTheme>()
            .colors
            .secondary_button
            .selected = replacement;
        app.update();

        let entity = app.world().entity(button);
        assert_eq!(entity.get::<BackgroundColor>().unwrap().0, replacement);
        assert_eq!(entity.get::<Interaction>(), Some(&Interaction::None));
        assert!(entity.contains::<SelectedButton>());
        assert_eq!(
            entity
                .get::<UiResolvedButtonStyle>()
                .unwrap()
                .backgrounds
                .selected,
            replacement
        );

        app.world_mut().resource_mut::<UiMetrics>().control_gap = 12.0;
        app.update();
        let entity = app.world().entity(button);
        assert_eq!(
            entity.get::<UiResolvedButtonStyle>().unwrap().padding_x,
            24.0
        );
        assert_eq!(entity.get::<Node>().unwrap().padding.left, px(24));
        assert_eq!(entity.get::<Interaction>(), Some(&Interaction::None));
        assert!(entity.contains::<SelectedButton>());
    }

    #[test]
    fn scoped_button_uses_existing_selected_state_priority() {
        let mut app = stateful_style_app();
        let scope = app
            .world_mut()
            .spawn(UiStyleScope::new(UI_STYLE_VARIANT_GALLERY_PARENT))
            .id();
        let button = app
            .world_mut()
            .spawn((
                Button,
                SecondaryButton,
                SelectedButton,
                Interaction::None,
                UiStyleBinding::new().with_button(UiButtonStyleRole::Secondary),
                BackgroundColor(Color::NONE),
                BorderColor::all(Color::NONE),
                Node::default(),
            ))
            .id();
        app.world_mut().entity_mut(scope).add_child(button);

        app.update();

        let resolved = *app.world().get::<UiResolvedButtonStyle>(button).unwrap();
        assert_eq!(
            app.world().get::<BackgroundColor>(button).unwrap().0,
            crate::framework::ui::widgets::controls::button_background_color(
                resolved.backgrounds,
                Interaction::None,
                false,
                false,
                true,
                false,
            )
        );
        assert_eq!(
            app.world().get::<BackgroundColor>(button).unwrap().0,
            Color::srgb(0.10, 0.52, 0.45)
        );
    }

    #[test]
    fn scoped_icon_button_uses_same_selected_state_and_is_stable() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>()
            .insert_resource(UiTheme::default())
            .insert_resource(UiMetrics::default())
            .add_systems(
                Update,
                (
                    resolve_ui_style_bindings,
                    apply_resolved_ui_styles,
                    update_icon_button_visuals,
                )
                    .chain(),
            );
        app.finish();
        app.cleanup();

        let theme = UiTheme::default();
        let metrics = UiMetrics::default();
        let fonts = UiFontAssets::test_registry();
        let i18n = UiI18n::test_with_texts("en_us", &[("scope.help", "Help")]);
        let bundle = icon_button_key(
            &theme,
            &metrics,
            &fonts,
            app.world().resource::<AssetServer>(),
            &i18n,
            UiIconId::HELP,
            "scope.help",
            "Help",
        );
        let scope = app
            .world_mut()
            .spawn(UiStyleScope::new(UI_STYLE_VARIANT_GALLERY_PARENT))
            .id();
        let button = app
            .world_mut()
            .spawn((
                bundle,
                SelectedButton,
                UiStyleBinding::new().with_button(UiButtonStyleRole::Secondary),
            ))
            .id();
        app.world_mut().entity_mut(scope).add_child(button);

        app.update();

        let icon = app
            .world()
            .get::<Children>(button)
            .unwrap()
            .iter()
            .find(|child| app.world().get::<UiIconVisual>(*child).is_some())
            .unwrap();
        assert_eq!(
            app.world().get::<BackgroundColor>(button).unwrap().0,
            Color::srgb(0.10, 0.52, 0.45)
        );
        assert_eq!(
            app.world()
                .get::<UiIconButton>(button)
                .unwrap()
                .visual_state,
            crate::framework::ui::widgets::UiButtonVisualState::Selected
        );
        assert_eq!(
            app.world().get::<ImageNode>(icon).unwrap().color,
            Color::srgb(0.84, 0.98, 0.94)
        );

        app.world_mut().clear_trackers();
        app.update();
        let button_ref = app.world().entity(button);
        assert!(
            !button_ref
                .get_ref::<UiResolvedButtonStyle>()
                .unwrap()
                .is_changed()
        );
        assert!(
            !button_ref
                .get_ref::<UiResolvedStyleDebugSnapshot>()
                .unwrap()
                .is_changed()
        );
        assert!(
            !button_ref
                .get_ref::<BackgroundColor>()
                .unwrap()
                .is_changed()
        );
        assert!(
            !app.world()
                .entity(icon)
                .get_ref::<ImageNode>()
                .unwrap()
                .is_changed()
        );
    }

    #[test]
    fn input_theme_refresh_preserves_value_focus_and_interaction() {
        let mut app = stateful_style_app();
        let input = app
            .world_mut()
            .spawn((
                Button,
                UiTextInput,
                FocusedButton,
                Interaction::None,
                UiTextInputValue("Pilot 01".to_owned()),
                UiStyleBinding::new().with_input(UiInputStyleRole::Standard),
                BackgroundColor(Color::NONE),
                BorderColor::all(Color::NONE),
                Node::default(),
            ))
            .id();
        app.update();

        let focused = Color::srgb(0.31, 0.48, 0.57);
        app.world_mut()
            .resource_mut::<UiTheme>()
            .colors
            .secondary_button
            .focused = focused;
        app.update();

        let entity = app.world().entity(input);
        assert_eq!(entity.get::<UiTextInputValue>().unwrap().0, "Pilot 01");
        assert_eq!(entity.get::<Interaction>(), Some(&Interaction::None));
        assert!(entity.contains::<FocusedButton>());
        assert_eq!(entity.get::<BackgroundColor>().unwrap().0, focused);
    }

    #[test]
    fn runtime_unknown_variant_falls_back_with_stable_debug_error() {
        let mut app = scoped_style_app();
        let mut binding = UiStyleBinding::new().with_surface(UiSurfaceStyleRole::Panel);
        binding.surface.as_mut().unwrap().variant = Some(UiStyleVariantId::new("missing.variant"));
        let target = app
            .world_mut()
            .spawn((binding, BackgroundColor(Color::NONE)))
            .id();

        app.update();

        assert_eq!(
            app.world().get::<BackgroundColor>(target).unwrap().0,
            UiTheme::default().colors.panel_background
        );
        let entry = &app
            .world()
            .get::<UiResolvedStyleDebugSnapshot>(target)
            .unwrap()
            .entries[0];
        assert!(entry.fallback);
        assert_eq!(
            entry.error.as_deref(),
            Some(UiStyleErrorCode::UnknownVariant.as_str())
        );
    }

    #[test]
    fn overlapping_roles_apply_documented_atomic_to_dialog_priority() {
        let mut app = scoped_style_app();
        let target = app
            .world_mut()
            .spawn((
                UiStyleBinding::new()
                    .with_surface(UiSurfaceStyleRole::Panel)
                    .with_border(UiBorderStyleRole::Panel)
                    .with_card(UiCardStyleRole::Emphasis)
                    .with_dialog(UiDialogStyleRole::Destructive),
                BackgroundColor(Color::NONE),
                BorderColor::all(Color::NONE),
                Node::default(),
            ))
            .id();

        app.update();

        let theme = UiTheme::default();
        let entity = app.world().entity(target);
        assert_eq!(
            entity.get::<BackgroundColor>().unwrap().0,
            theme.colors.panel_background
        );
        assert_eq!(
            entity.get::<BorderColor>().unwrap(),
            &BorderColor::all(theme.colors.error)
        );
        assert_eq!(
            entity.get::<Node>().unwrap().padding,
            UiRect::all(px(UiMetrics::default().panel_padding))
        );
        let requests = entity
            .get::<UiResolvedStyleDebugSnapshot>()
            .unwrap()
            .entries
            .iter()
            .map(|entry| entry.request.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            requests,
            vec![
                "surface.panel",
                "border.panel",
                "card.emphasis",
                "dialog.destructive",
            ]
        );
    }
}
