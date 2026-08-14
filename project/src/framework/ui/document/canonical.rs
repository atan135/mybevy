use super::{UiDocument, UiDocumentError, ValidatedUiDocument};
use serde_json::Value;

impl UiDocument {
    pub fn to_canonical_json(&self) -> Result<String, serde_json::Error> {
        let mut value = serde_json::to_value(self)?;
        sort_json_objects(&mut value);
        serde_json::to_string(&value)
    }

    pub fn to_canonical_json_pretty(&self) -> Result<String, serde_json::Error> {
        let mut value = serde_json::to_value(self)?;
        sort_json_objects(&mut value);
        serde_json::to_string_pretty(&value).map(|mut output| {
            output.push('\n');
            output
        })
    }

    pub fn parse_and_validate_json(source: &str) -> Result<ValidatedUiDocument, UiDocumentError> {
        // Core owns the closed JSON schema and semantic validation contract. The Bevy runtime
        // keeps its local representation only as an adaptation target after this gate.
        let core_result = ui_document_core::ValidatedUiDocument::parse_json(source);
        let runtime_result = ValidatedUiDocument::parse_json(source);
        match (core_result, runtime_result) {
            (Ok(core), Ok(runtime)) => {
                debug_assert_eq!(
                    core.document().document_id.as_str(),
                    runtime.document().document_id.as_str(),
                    "core and runtime adapters must preserve the document identity"
                );
                debug_assert_eq!(
                    core.document().to_canonical_json().ok(),
                    runtime.document().to_canonical_json().ok(),
                    "core and runtime adapters must preserve canonical document content"
                );
                Ok(runtime)
            }
            (Err(core), Err(runtime)) if core.code() == runtime.code() => Err(runtime),
            (Err(core), _) => Err(UiDocumentError::CoreValidation {
                code: core.code().to_owned(),
                message: core.to_string(),
            }),
            (Ok(_), Err(runtime)) => Err(runtime),
        }
    }
}

fn sort_json_objects(value: &mut Value) {
    match value {
        Value::Object(object) => {
            let mut entries = std::mem::take(object).into_iter().collect::<Vec<_>>();
            entries.sort_unstable_by(|left, right| left.0.cmp(&right.0));
            for (_, child) in &mut entries {
                sort_json_objects(child);
            }
            object.extend(entries);
        }
        Value::Array(values) => values.iter_mut().for_each(sort_json_objects),
        Value::Number(number) if number.is_f64() && number.as_f64() == Some(0.0) => {
            *number = serde_json::Number::from_f64(0.0).expect("zero is finite");
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    use crate::framework::ui::document::{
        UiDocumentInputMode, UiDocumentPlatform, UiOrientation, UiSafeAreaClass, UiTargetProfile,
    };

    const MINIMAL_DOCUMENT: &str = r#"{
        "schema_version": 1,
        "document_id": "adapter.minimal",
        "root": { "type": "container", "id": "adapter.root" }
    }"#;

    #[test]
    fn core_gate_and_runtime_adapter_preserve_valid_document_identity_and_hash() {
        let core = ui_document_core::ValidatedUiDocument::parse_json(MINIMAL_DOCUMENT).unwrap();
        let runtime = UiDocument::parse_and_validate_json(MINIMAL_DOCUMENT).unwrap();

        assert_eq!(
            core.document().document_id.as_str(),
            runtime.document().document_id.as_str()
        );
        let core_canonical = core.document().to_canonical_json().unwrap();
        let runtime_canonical = runtime.document().to_canonical_json().unwrap();
        assert_eq!(core_canonical, runtime_canonical);
        assert_eq!(
            format!("{:x}", Sha256::digest(core_canonical.as_bytes())),
            format!("{:x}", Sha256::digest(runtime_canonical.as_bytes()))
        );
    }

    #[test]
    fn core_gate_rejects_unknown_schema_fields_with_matching_error_code() {
        let source = r#"{
            "schema_version": 1,
            "document_id": "adapter.unknown",
            "unknown": true,
            "root": { "type": "container", "id": "adapter.root" }
        }"#;

        let core = ui_document_core::ValidatedUiDocument::parse_json(source).unwrap_err();
        let runtime = UiDocument::parse_and_validate_json(source).unwrap_err();
        assert_eq!(core.code(), runtime.code());
    }

    #[test]
    fn core_gate_rejects_semantic_errors_with_matching_error_code() {
        let source = r#"{
            "schema_version": 1,
            "document_id": "adapter.semantic",
            "root": {
                "type": "container",
                "id": "adapter.root",
                "layout": { "width": { "px": -1 } }
            }
        }"#;

        let core = ui_document_core::ValidatedUiDocument::parse_json(source).unwrap_err();
        let runtime = UiDocument::parse_and_validate_json(source).unwrap_err();
        assert_eq!(core.code(), runtime.code());
    }

    #[test]
    fn core_and_runtime_adapters_agree_on_every_responsive_geometry_class_combination() {
        for width_class in ["compact", "medium", "expanded"] {
            for height_class in ["short", "regular", "tall"] {
                for orientation in ["portrait", "landscape"] {
                    let source = format!(
                        r#"{{
                          "schema_version": 1,
                          "document_id": "adapter.responsive",
                          "root": {{ "type": "container", "id": "adapter.root" }},
                          "responsive": [{{
                            "id": "geometry",
                            "when": {{
                              "width_class": "{width_class}",
                              "height_class": "{height_class}",
                              "orientation": "{orientation}"
                            }},
                            "overrides": [{{
                              "node_id": "adapter.root",
                              "set": {{ "layout": {{ "gap": {{ "px": 1 }} }} }}
                            }}]
                          }}]
                        }}"#
                    );
                    let core = ui_document_core::ValidatedUiDocument::parse_json(&source);
                    let runtime = ValidatedUiDocument::parse_json(&source);
                    assert_eq!(
                        core.is_ok(),
                        runtime.is_ok(),
                        "core/runtime validity diverged for {width_class}/{height_class}/{orientation}"
                    );
                    if let (Err(core), Err(runtime)) = (&core, &runtime) {
                        assert_eq!(
                            core.code(),
                            runtime.code(),
                            "core/runtime error code diverged for {width_class}/{height_class}/{orientation}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn square_target_profile_is_portrait_like_runtime_viewport() {
        let profile = UiTargetProfile::new(
            800.0,
            800.0,
            UiSafeAreaClass::None,
            UiDocumentInputMode::MouseKeyboard,
            UiDocumentPlatform::Windows,
        )
        .unwrap();
        assert_eq!(profile.orientation(), UiOrientation::Portrait);
    }
}
