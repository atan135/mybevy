//! Stable, runtime-free surface for repository development tools that author `UiDocument` JSON.
//!
//! Keep this facade intentionally narrow. Generation tools may validate and canonicalize
//! untrusted documents through it, but they do not gain access to game screens, actions, or the
//! runtime plugin.

pub use super::{
    CURRENT_SCHEMA_VERSION, MIN_SUPPORTED_SCHEMA_VERSION, UI_DOCUMENT_BUDGET_PROFILE,
    UI_DOCUMENT_MAX_BYTES, UiDocument, UiDocumentBudgetUsage, UiDocumentError,
    UiDocumentValidationResult, UiValidationDiagnostic, UiValidationPhase, UiValidationReport,
    UiValidationSeverity, ValidatedUiDocument,
};

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
}
