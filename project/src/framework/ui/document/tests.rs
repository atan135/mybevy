use super::*;
use serde_json::{Value, json};
use std::{fs, path::PathBuf, str::FromStr};

const MINIMAL_DOCUMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/ui/documents/fixtures/minimal_page.v1.json"
));
const DUPLICATE_NODE_DOCUMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/ui/documents/fixtures/invalid/duplicate_node_id.v1.json"
));
const ILLEGAL_ID_DOCUMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/ui/documents/fixtures/invalid/illegal_id.v1.json"
));
const FUTURE_VERSION_DOCUMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/ui/documents/fixtures/invalid/future_version.v1.json"
));
const MISSING_ROOT_DOCUMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/ui/documents/fixtures/invalid/missing_root.v1.json"
));
const CANONICAL_DOCUMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/ui/documents/fixtures/minimal_page.v1.canonical.json"
));
const DOCUMENT_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/ui/documents/schema/ui_document.v1.schema.json"
));

#[test]
fn ui_document_parses_stage_one_fixture_and_indexes_nodes() {
    let validated = UiDocument::parse_and_validate_json(MINIMAL_DOCUMENT).unwrap();
    let document = validated.document();

    assert_eq!(document.schema_version, CURRENT_SCHEMA_VERSION);
    assert_eq!(document.document_id.as_str(), "example.minimal_page");
    assert_eq!(document.metadata.budget_profile, "mobile_baseline_v1");
    assert!(document.states.is_empty());
    assert_eq!(document.responsive.len(), 1);

    let hero_id = UiNodeId::from_str("page.hero").unwrap();
    assert_eq!(validated.node_path(&hero_id), Some("$.root.children[1]"));
    assert_eq!(validated.node_marker(&hero_id).unwrap().node_id, hero_id);
    assert_eq!(
        validated.audit_metadata(&hero_id).unwrap().document_path,
        "$.root.children[1]"
    );
}

#[test]
fn ui_document_rejects_duplicate_node_ids() {
    let error = UiDocument::parse_and_validate_json(DUPLICATE_NODE_DOCUMENT).unwrap_err();

    assert_eq!(error.code(), "UI_NODE_ID_DUPLICATE");
    assert!(matches!(
        error,
        UiDocumentError::DuplicateNodeId {
            first_path,
            duplicate_path,
            ..
        } if first_path == "$.root" && duplicate_path == "$.root.children[0]"
    ));
}

#[test]
fn ui_document_rejects_illegal_ids_during_parse() {
    let error = UiDocument::parse_and_validate_json(ILLEGAL_ID_DOCUMENT).unwrap_err();
    assert_eq!(error.code(), "UI_DOCUMENT_PARSE_FAILED");

    assert!(UiDocumentId::from_str("missing_namespace").is_err());
    assert!(UiNodeId::from_str("Page.root").is_err());
    assert!(UiAssetId::from_str("hero-image").is_err());
    assert!(UiStyleId::from_str("").is_err());
    assert!(UiActionId::from_str("example..continue").is_err());
}

#[test]
fn ui_document_rejects_unknown_versions() {
    let error = UiDocument::parse_and_validate_json(FUTURE_VERSION_DOCUMENT).unwrap_err();
    assert_eq!(error.code(), "UI_SCHEMA_FUTURE_VERSION");
    assert!(matches!(
        error,
        UiDocumentError::FutureSchemaVersion {
            found: 2,
            current: CURRENT_SCHEMA_VERSION
        }
    ));

    let invalid = UiDocument::parse_and_validate_json("{}").unwrap_err();
    assert_eq!(invalid.code(), "UI_SCHEMA_VERSION_INVALID");
}

#[test]
fn ui_document_rejects_missing_root_and_unknown_fields() {
    let missing_root = UiDocument::parse_and_validate_json(MISSING_ROOT_DOCUMENT).unwrap_err();
    assert_eq!(missing_root.code(), "UI_DOCUMENT_PARSE_FAILED");
    assert!(missing_root.to_string().contains("missing field `root`"));

    let source = MINIMAL_DOCUMENT.replacen(
        "\"schema_version\": 1,",
        "\"schema_version\": 1, \"unknown_capability\": true,",
        1,
    );
    let unknown = UiDocument::parse_and_validate_json(&source).unwrap_err();
    assert_eq!(unknown.code(), "UI_DOCUMENT_PARSE_FAILED");
    assert!(
        unknown
            .to_string()
            .contains("unknown field `unknown_capability`")
    );
}

#[test]
fn ui_document_canonical_json_matches_golden_and_round_trips() {
    let document = UiDocument::parse_and_validate_json(MINIMAL_DOCUMENT)
        .unwrap()
        .into_document();
    let canonical = document.to_canonical_json_pretty().unwrap();

    maybe_update_golden(
        "UPDATE_UI_DOCUMENT_GOLDENS",
        "assets/ui/documents/fixtures/minimal_page.v1.canonical.json",
        &canonical,
    );
    assert_eq!(canonical, CANONICAL_DOCUMENT);

    let reparsed = UiDocument::parse_and_validate_json(&canonical)
        .unwrap()
        .into_document();
    assert_eq!(reparsed, document);
    assert_eq!(
        document.to_canonical_json().unwrap(),
        reparsed.to_canonical_json().unwrap()
    );

    let canonical_value: Value = serde_json::from_str(&canonical).unwrap();
    assert_eq!(canonical_value["states"], json!([]));
    assert_eq!(
        canonical_value["root"]["children"][0]["layout"]["gap"],
        json!({ "px": 0 })
    );
    assert_eq!(
        canonical_value["root"]["children"][0]["style"]["role"],
        Value::Null
    );
}

#[test]
fn ui_document_schema_matches_rust_model() {
    let schema = generated_schema();
    maybe_update_golden(
        "UPDATE_UI_DOCUMENT_GOLDENS",
        "assets/ui/documents/schema/ui_document.v1.schema.json",
        &schema,
    );
    assert_eq!(schema, DOCUMENT_SCHEMA);

    let value: Value = serde_json::from_str(&schema).unwrap();
    assert_eq!(value["title"], "UiDocument");
    assert_eq!(value["additionalProperties"], false);
    assert!(
        value["required"]
            .as_array()
            .unwrap()
            .contains(&json!("root"))
    );
    assert_eq!(
        value["$defs"]["UiDocumentId"]["pattern"],
        UI_NAMESPACED_ID_PATTERN
    );
    assert_eq!(value["properties"]["schema_version"]["minimum"], 1);
    assert_eq!(value["properties"]["schema_version"]["maximum"], 1);
}

const UI_NAMESPACED_ID_PATTERN: &str = "^[a-z][a-z0-9_]*(\\.[a-z][a-z0-9_]*)+$";

fn generated_schema() -> String {
    let schema = serde_json::to_value(schemars::schema_for!(UiDocument)).unwrap();
    let mut output = serde_json::to_string_pretty(&schema).unwrap();
    output.push('\n');
    output
}

fn maybe_update_golden(variable: &str, relative_path: &str, contents: &str) {
    if std::env::var_os(variable).is_none() {
        return;
    }
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}
