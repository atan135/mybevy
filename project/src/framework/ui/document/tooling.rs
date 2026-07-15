//! Stable, runtime-free surface for repository development tools that author `UiDocument` JSON.
//!
//! Keep this facade intentionally narrow. Generation tools may validate and canonicalize
//! untrusted documents through it, but they do not gain access to game screens, actions, or the
//! runtime plugin.

pub use super::{
    CURRENT_SCHEMA_VERSION, MIN_SUPPORTED_SCHEMA_VERSION, UI_DOCUMENT_BUDGET_PROFILE,
    UI_DOCUMENT_MAX_BYTES, UiAssetSource, UiDocument, UiDocumentBudgetUsage, UiDocumentError,
    UiDocumentValidationResult, UiValidationDiagnostic, UiValidationPhase, UiValidationReport,
    UiValidationSeverity, ValidatedUiDocument,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::ui::style::UiTheme;
    use bevy::prelude::Color;
    use std::collections::BTreeSet;

    const MINIMAL_DOCUMENT: &str =
        include_str!("../../../../assets/ui/documents/fixtures/minimal_page.v1.json");

    #[test]
    fn ui_document_tooling_facade_validates_and_canonicalizes() {
        let result = validate_json(MINIMAL_DOCUMENT);
        assert!(result.report.valid);
        assert!(result.validated().is_some());

        let canonical = canonicalize_json(MINIMAL_DOCUMENT).unwrap();
        assert!(canonical.ends_with('\n'));
        assert!(validate_json_bytes(canonical.as_bytes()).report.valid);
    }

    #[test]
    fn ui_document_tooling_catalog_has_stable_unique_names() {
        let token_names: BTreeSet<_> = BUILT_IN_TOKENS.iter().map(|token| token.name).collect();
        assert_eq!(token_names.len(), BUILT_IN_TOKENS.len());
        assert!(BUILT_IN_TOKENS.iter().all(|token| {
            match token.value {
                UiToolingTokenValue::Scalar(value) => value.is_finite() && value >= 0.0,
                UiToolingTokenValue::Srgba(value) => value
                    .iter()
                    .all(|channel| channel.is_finite() && (0.0..=1.0).contains(channel)),
            }
        }));

        let variants: BTreeSet<_> = BUILT_IN_WIDGET_VARIANTS
            .iter()
            .map(|variant| (variant.component, variant.variant))
            .collect();
        assert_eq!(variants.len(), BUILT_IN_WIDGET_VARIANTS.len());
        assert!(
            BUILT_IN_WIDGET_VARIANTS
                .iter()
                .all(|entry| { widget_variant_is_supported(entry.component, entry.variant) })
        );
        assert!(!widget_variant_is_supported("list_item", "standard"));
        assert!(!widget_variant_is_supported("label", "body"));
    }

    #[test]
    fn ui_document_tooling_catalog_matches_the_runtime_default_theme() {
        let theme = UiTheme::default();
        let expected = [
            (
                "color.screen_background",
                color(theme.colors.screen_background),
            ),
            (
                "color.panel_background",
                color(theme.colors.panel_background),
            ),
            ("color.panel_border", color(theme.colors.panel_border)),
            ("color.text_primary", color(theme.colors.text_primary)),
            ("font.title_large", scalar(theme.text.title_large)),
            ("font.title", scalar(theme.text.title)),
            ("font.body", scalar(theme.text.body)),
            ("font.caption", scalar(theme.text.caption)),
            (
                "spacing.screen_padding",
                scalar(theme.layout.screen_padding),
            ),
            ("spacing.page_gap", scalar(theme.layout.page_gap)),
            ("spacing.panel_gap", scalar(theme.layout.panel_gap)),
            ("spacing.card_gap", scalar(theme.layout.card_gap)),
            ("spacing.row_gap", scalar(theme.layout.row_gap)),
            (
                "spacing.row_column_gap",
                scalar(theme.layout.row_column_gap),
            ),
            ("radius.button", scalar(theme.button.radius)),
            ("radius.panel", scalar(theme.panel.radius)),
            ("border.panel", scalar(theme.panel.border)),
            ("size.button_height", scalar(theme.button.height)),
            ("size.button_min_width", scalar(theme.button.min_width)),
        ];
        assert_eq!(expected.len(), BUILT_IN_TOKENS.len());
        for (name, value) in expected {
            assert_eq!(
                BUILT_IN_TOKENS
                    .iter()
                    .find(|token| token.name == name)
                    .map(|token| token.value),
                Some(value),
                "tooling token {name} drifted from UiTheme::default()"
            );
        }
    }

    fn scalar(value: f32) -> UiToolingTokenValue {
        UiToolingTokenValue::Scalar(value)
    }

    fn color(value: Color) -> UiToolingTokenValue {
        let value = value.to_srgba();
        UiToolingTokenValue::Srgba([value.red, value.green, value.blue, value.alpha])
    }
}
