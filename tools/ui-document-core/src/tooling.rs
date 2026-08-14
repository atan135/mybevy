//! Stable, runtime-free surface for repository development tools that author `UiDocument` JSON.
//!
//! Keep this facade intentionally narrow. Generation tools may validate and canonicalize
//! untrusted documents through it, but they do not gain access to game screens, actions, or the
//! runtime plugin.

pub use super::{
    CURRENT_SCHEMA_VERSION, MIN_SUPPORTED_SCHEMA_VERSION, UI_DOCUMENT_BUDGET_PROFILE,
    UI_DOCUMENT_MAX_BYTES, UiApprovedDocumentRegistration, UiApprovedDocumentRegistrationError,
    UiAssetSource, UiDocument, UiDocumentBudgetUsage, UiDocumentError, UiDocumentValidationResult,
    UiValidationDiagnostic, UiValidationPhase, UiValidationReport, UiValidationSeverity,
    ValidatedUiDocument, parse_approved_document_registration,
};
use super::{
    UiComponentVariant,
    control::{UiDocumentComponentKind, component_variant_supported},
};

/// Read-only design-system catalog for repository tools. This intentionally exposes values and
/// stable names, not runtime theme or widget implementation types.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiToolingToken {
    pub name: &'static str,
    pub kind: UiToolingTokenKind,
    pub value: UiToolingTokenValue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UiToolingTokenKind {
    Color,
    FontSize,
    Spacing,
    Radius,
    BorderWidth,
    RepeatedSize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UiToolingTokenValue {
    Scalar(f32),
    Srgba([f32; 4]),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UiToolingWidgetVariant {
    pub component: &'static str,
    pub variant: &'static str,
}

pub const BUILT_IN_TOKENS: &[UiToolingToken] = &[
    UiToolingToken {
        name: "color.screen_background",
        kind: UiToolingTokenKind::Color,
        value: UiToolingTokenValue::Srgba([0.05, 0.08, 0.11, 1.0]),
    },
    UiToolingToken {
        name: "color.panel_background",
        kind: UiToolingTokenKind::Color,
        value: UiToolingTokenValue::Srgba([0.10, 0.13, 0.16, 0.94]),
    },
    UiToolingToken {
        name: "color.panel_border",
        kind: UiToolingTokenKind::Color,
        value: UiToolingTokenValue::Srgba([0.22, 0.28, 0.31, 1.0]),
    },
    UiToolingToken {
        name: "color.text_primary",
        kind: UiToolingTokenKind::Color,
        value: UiToolingTokenValue::Srgba([0.92, 0.95, 0.95, 1.0]),
    },
    UiToolingToken {
        name: "font.title_large",
        kind: UiToolingTokenKind::FontSize,
        value: UiToolingTokenValue::Scalar(44.0),
    },
    UiToolingToken {
        name: "font.title",
        kind: UiToolingTokenKind::FontSize,
        value: UiToolingTokenValue::Scalar(34.0),
    },
    UiToolingToken {
        name: "font.body",
        kind: UiToolingTokenKind::FontSize,
        value: UiToolingTokenValue::Scalar(24.0),
    },
    UiToolingToken {
        name: "font.caption",
        kind: UiToolingTokenKind::FontSize,
        value: UiToolingTokenValue::Scalar(15.0),
    },
    UiToolingToken {
        name: "spacing.screen_padding",
        kind: UiToolingTokenKind::Spacing,
        value: UiToolingTokenValue::Scalar(24.0),
    },
    UiToolingToken {
        name: "spacing.page_gap",
        kind: UiToolingTokenKind::Spacing,
        value: UiToolingTokenValue::Scalar(18.0),
    },
    UiToolingToken {
        name: "spacing.panel_gap",
        kind: UiToolingTokenKind::Spacing,
        value: UiToolingTokenValue::Scalar(20.0),
    },
    UiToolingToken {
        name: "spacing.card_gap",
        kind: UiToolingTokenKind::Spacing,
        value: UiToolingTokenValue::Scalar(12.0),
    },
    UiToolingToken {
        name: "spacing.row_gap",
        kind: UiToolingTokenKind::Spacing,
        value: UiToolingTokenValue::Scalar(6.0),
    },
    UiToolingToken {
        name: "spacing.row_column_gap",
        kind: UiToolingTokenKind::Spacing,
        value: UiToolingTokenValue::Scalar(16.0),
    },
    UiToolingToken {
        name: "radius.button",
        kind: UiToolingTokenKind::Radius,
        value: UiToolingTokenValue::Scalar(6.0),
    },
    UiToolingToken {
        name: "radius.panel",
        kind: UiToolingTokenKind::Radius,
        value: UiToolingTokenValue::Scalar(8.0),
    },
    UiToolingToken {
        name: "border.panel",
        kind: UiToolingTokenKind::BorderWidth,
        value: UiToolingTokenValue::Scalar(1.0),
    },
    UiToolingToken {
        name: "size.button_height",
        kind: UiToolingTokenKind::RepeatedSize,
        value: UiToolingTokenValue::Scalar(46.0),
    },
    UiToolingToken {
        name: "size.button_min_width",
        kind: UiToolingTokenKind::RepeatedSize,
        value: UiToolingTokenValue::Scalar(112.0),
    },
];

pub const BUILT_IN_WIDGET_VARIANTS: &[UiToolingWidgetVariant] = &[
    UiToolingWidgetVariant {
        component: "button",
        variant: "default",
    },
    UiToolingWidgetVariant {
        component: "button",
        variant: "primary",
    },
    UiToolingWidgetVariant {
        component: "button",
        variant: "destructive",
    },
    UiToolingWidgetVariant {
        component: "button",
        variant: "secondary",
    },
    UiToolingWidgetVariant {
        component: "badge",
        variant: "default",
    },
    UiToolingWidgetVariant {
        component: "badge",
        variant: "error",
    },
    UiToolingWidgetVariant {
        component: "badge",
        variant: "info",
    },
    UiToolingWidgetVariant {
        component: "badge",
        variant: "success",
    },
    UiToolingWidgetVariant {
        component: "badge",
        variant: "warning",
    },
    UiToolingWidgetVariant {
        component: "progress",
        variant: "default",
    },
    UiToolingWidgetVariant {
        component: "progress",
        variant: "error",
    },
    UiToolingWidgetVariant {
        component: "progress",
        variant: "info",
    },
    UiToolingWidgetVariant {
        component: "progress",
        variant: "success",
    },
    UiToolingWidgetVariant {
        component: "progress",
        variant: "warning",
    },
];

/// Uses the same support matrix as `UiDocument` semantic validation.
pub fn widget_variant_is_supported(component: &str, variant: &str) -> bool {
    let kind = match component {
        "button" => UiDocumentComponentKind::Button,
        "badge" => UiDocumentComponentKind::Badge,
        "progress" => UiDocumentComponentKind::Progress,
        _ => return false,
    };
    let variant = match variant {
        "default" => UiComponentVariant::Default,
        "primary" => UiComponentVariant::Primary,
        "secondary" => UiComponentVariant::Secondary,
        "destructive" => UiComponentVariant::Destructive,
        "info" => UiComponentVariant::Info,
        "success" => UiComponentVariant::Success,
        "warning" => UiComponentVariant::Warning,
        "error" => UiComponentVariant::Error,
        _ => return false,
    };
    component_variant_supported(kind, variant)
}

/// Validates untrusted UTF-8 JSON with the same schema, semantic, capability, and budget checks
/// used by the game runtime.
pub fn validate_json(source: &str) -> UiDocumentValidationResult {
    UiDocument::validate_json(source)
}

/// Validates untrusted bytes and reports invalid UTF-8 without an intermediate lossy conversion.
pub fn validate_json_bytes(source: &[u8]) -> UiDocumentValidationResult {
    UiDocument::validate_json_bytes(source)
}

/// Emits the repository's deterministic JSON representation after full validation.
pub fn canonicalize_json(source: &str) -> Result<String, UiDocumentError> {
    let document = UiDocument::parse_and_validate_json(source)?;
    document
        .document()
        .to_canonical_json_pretty()
        .map_err(|error| UiDocumentError::Parse {
            message: format!("canonical JSON serialization failed: {error}"),
        })
}
