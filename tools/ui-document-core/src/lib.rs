mod approval;
mod asset;
mod binding_action;
mod budget;
mod canonical;
mod content;
mod control;
mod id;
mod layout;
mod model;
mod report;
mod responsive;
mod style;
mod tooling;
mod validation;

pub use approval::*;
pub use asset::*;
pub use binding_action::*;
pub use budget::*;
pub use content::*;
pub use control::*;
pub use id::*;
pub use layout::*;
pub use model::*;
pub use report::*;
pub use responsive::*;
pub use style::*;
pub use tooling::*;
pub use validation::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiDocumentPanel {
    Page,
    Hud,
    Floating,
    Modal,
    BlockingOverlay,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiDocumentLayer {
    Page,
    Floating,
    Modal,
    Loading,
    Toast,
    Debug,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiDocumentSourceRoot {
    Approved,
    Fixture,
    Authoring,
    ContentCache,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize)]
pub struct UiDocumentSourcePath {
    logical: String,
}
impl UiDocumentSourcePath {
    pub fn new(
        root: UiDocumentSourceRoot,
        relative: impl AsRef<str>,
    ) -> Result<Self, UiDocumentSourcePathError> {
        let relative = relative.as_ref();
        if relative.is_empty()
            || !relative.ends_with(".json")
            || relative.contains(['\\', ':', '\0', '\n', '\r'])
            || relative
                .split('/')
                .any(|s| s.is_empty() || s == "." || s == "..")
        {
            return Err(UiDocumentSourcePathError);
        }
        let prefix = match root {
            UiDocumentSourceRoot::Approved => "ui/documents/approved",
            UiDocumentSourceRoot::Fixture => "ui/documents/fixtures",
            UiDocumentSourceRoot::Authoring => "ui-documents/source",
            UiDocumentSourceRoot::ContentCache => "ui-documents/cache",
        };
        Ok(Self {
            logical: format!("{prefix}/{relative}"),
        })
    }
    pub fn as_str(&self) -> &str {
        &self.logical
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UiDocumentSourcePathError;
impl std::fmt::Display for UiDocumentSourcePathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("invalid document source path")
    }
}
impl std::error::Error for UiDocumentSourcePathError {}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct UiDocumentApprovalAuditMetadata {
    pub host_contract_version: Option<u32>,
    pub actions: Vec<String>,
    pub bindings: Vec<String>,
    pub canonical_document_sha256: String,
}

#[derive(Clone, Debug)]
pub struct UiDocumentPreviewRegistration {
    pub document_id: UiDocumentId,
    pub owner: String,
    pub source_path: UiDocumentSourcePath,
    pub source_json: String,
    pub panel: UiDocumentPanel,
    pub layer: UiDocumentLayer,
    pub target_profile: UiTargetProfile,
    pub page_state: UiPageState,
    pub owner_alive: bool,
    pub host_bindings: std::collections::BTreeMap<UiHostBindingKey, UiBindingType>,
    pub watch: bool,
    pub open_on_register: bool,
    pub audit_profiles: Vec<String>,
    pub approval_audit: Option<UiDocumentApprovalAuditMetadata>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"{
        "schema_version": 1,
        "document_id": "core.smoke",
        "root": { "type": "container", "id": "root.container" }
    }"#;

    #[test]
    fn validates_and_canonicalizes_minimal_document() {
        let result = UiDocument::validate_json(MINIMAL);
        assert!(result.report.valid);
        let canonical = canonicalize_json(MINIMAL).expect("canonical document");
        assert!(canonical.ends_with('\n'));
        assert!(validate_json_bytes(canonical.as_bytes()).report.valid);
    }

    #[test]
    fn rejects_oversized_document_bytes() {
        let oversized = format!(
            "{}{}",
            MINIMAL.trim_end_matches('}'),
            "x".repeat(UI_DOCUMENT_MAX_BYTES)
        );
        let result = validate_json_bytes(oversized.as_bytes());
        assert!(!result.report.valid);
    }
}
