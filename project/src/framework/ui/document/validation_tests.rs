use super::*;
use serde_json::{Value, json};
use std::panic::{AssertUnwindSafe, catch_unwind};

const MINIMAL_DOCUMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/ui/documents/fixtures/minimal_page.v1.json"
));
const DUPLICATE_SLOT_DOCUMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/ui/documents/fixtures/invalid/duplicate_slot.v1.json"
));
const MULTI_ERROR_DOCUMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/ui/documents/fixtures/invalid/validation_multi_error.v1.json"
));
const STYLE_RESOURCE_DOCUMENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/ui/documents/fixtures/style_resources.v1.json"
));

#[test]
fn ui_document_validation_report_accepts_fixture_and_freezes_budget_usage() {
    let result = UiDocument::validate_json(MINIMAL_DOCUMENT);

    assert!(result.report.valid);
    assert!(!result.report.truncated);
    assert!(result.report.diagnostics.is_empty());
    assert_eq!(result.report.report_version, 1);
    assert_eq!(result.report.budget_profile, UI_DOCUMENT_BUDGET_PROFILE);
    assert_eq!(
        result.report.budget_usage.source_bytes,
        MINIMAL_DOCUMENT.len()
    );
    assert_eq!(result.report.budget_usage.nodes, 4);
    assert_eq!(result.report.budget_usage.max_tree_depth, 2);
    assert_eq!(result.report.budget_usage.max_children, 3);
    assert_eq!(result.report.budget_usage.assets, 1);
    assert_eq!(result.report.budget_usage.action_references, 1);
    assert_eq!(result.report.budget_usage.responsive_variants, 1);
    assert_eq!(result.report.budget_usage.state_responsive_overrides, 2);
    assert_eq!(result.report.budget_usage.animations, 0);
    assert_eq!(UI_DOCUMENT_MAX_ANIMATIONS, 32);
    assert_eq!(UI_DOCUMENT_MAX_EFFECT_COMPLEXITY, 256);
    assert!(result.validated().is_some());
}

#[test]
fn ui_document_validation_report_aggregates_stable_machine_diagnostics() {
    let first = UiDocument::validate_json(MULTI_ERROR_DOCUMENT).report;
    let second = UiDocument::validate_json(MULTI_ERROR_DOCUMENT).report;

    assert!(!first.valid);
    assert_eq!(first, second);
    for code in [
        "UI_LAYOUT_OBVIOUSLY_OUT_OF_BOUNDS",
        "UI_CONTROL_STATE_REQUIRED",
        "UI_STYLE_REFERENCE_CYCLE",
        "UI_ASSET_PATH_INVALID",
        "UI_ASSET_UNKNOWN",
        "UI_ACTION_PARAM_FORBIDDEN",
    ] {
        assert!(first.has_code(code), "missing {code}: {first:#?}");
    }
    assert!(first.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "UI_ACTION_PARAM_FORBIDDEN"
            && diagnostic.phase == UiValidationPhase::Capability
            && diagnostic.node_id.as_ref().map(UiNodeId::as_str) == Some("validation.action")
            && diagnostic.document_path == "$.root.children[1]"
            && diagnostic.field_path.ends_with(".on_click.params.payload")
            && !diagnostic.message.is_empty()
            && !diagnostic.suggestion.is_empty()
    }));
    assert!(first.diagnostics.windows(2).all(|pair| {
        (
            pair[0].phase,
            &pair[0].field_path,
            &pair[0].code,
            pair[0].node_id.as_ref(),
        ) <= (
            pair[1].phase,
            &pair[1].field_path,
            &pair[1].code,
            pair[1].node_id.as_ref(),
        )
    }));
    let machine_json = serde_json::to_value(&first).unwrap();
    assert_eq!(machine_json["report_version"], 1);
    assert!(machine_json["diagnostics"].is_array());
    assert!(!machine_json.to_string().contains("UiDocumentError"));
}

#[test]
fn ui_document_duplicate_slot_is_rejected_before_value_overwrite() {
    let report = UiDocument::validate_json(DUPLICATE_SLOT_DOCUMENT).report;

    assert!(!report.valid);
    assert_eq!(report.diagnostics.len(), 1);
    let duplicate = &report.diagnostics[0];
    assert_eq!(duplicate.code, "UI_DOCUMENT_DUPLICATE_OBJECT_KEY");
    assert_eq!(duplicate.phase, UiValidationPhase::Syntax);
    assert_eq!(duplicate.field_path, "$.root.component.slots.label");
    assert_eq!(
        UiDocument::parse_and_validate_json(DUPLICATE_SLOT_DOCUMENT)
            .unwrap_err()
            .code(),
        "UI_DOCUMENT_DUPLICATE_OBJECT_KEY"
    );
}

#[test]
fn ui_document_validation_applies_cheap_byte_gate_and_bounded_diagnostics() {
    let oversized = "{".repeat(UI_DOCUMENT_MAX_BYTES + 1);
    let report = UiDocument::validate_json(&oversized).report;
    assert_eq!(report.diagnostics.len(), 1);
    assert!(report.has_code("UI_DOCUMENT_BYTES_BUDGET_EXCEEDED"));
    assert_eq!(report.diagnostics[0].phase, UiValidationPhase::Budget);

    let mut duplicate_bomb = String::from("{");
    for index in 0..(UI_DOCUMENT_MAX_DIAGNOSTICS + 20) {
        if index > 0 {
            duplicate_bomb.push(',');
        }
        duplicate_bomb.push_str("\"same\":0");
    }
    duplicate_bomb.push('}');
    let report = UiDocument::validate_json(&duplicate_bomb).report;
    assert!(report.truncated);
    assert!(report.diagnostics.len() <= UI_DOCUMENT_MAX_DIAGNOSTICS);
    assert!(report.has_code("UI_DOCUMENT_DUPLICATE_OBJECT_KEY"));

    let overrides = (0..UI_DOCUMENT_MAX_DIAGNOSTICS)
        .map(|index| {
            json!({
                "node_id": "bounded.root",
                "set": { "layout": { "width": { "px": index + 1 } } }
            })
        })
        .collect::<Vec<_>>();
    let conflict_bomb = json!({
        "schema_version": 1,
        "document_id": "bounded.conflicts",
        "root": { "type": "container", "id": "bounded.root" },
        "states": [{ "id": "loading", "overrides": overrides }]
    });
    let report = UiDocument::validate_json(&conflict_bomb.to_string()).report;
    assert!(report.truncated);
    assert!(!report.diagnostics.is_empty());
    assert!(report.diagnostics.len() <= UI_DOCUMENT_MAX_DIAGNOSTICS);
}

#[test]
fn ui_document_validation_enforces_tree_string_and_patch_budgets() {
    let children = (0..=UI_DOCUMENT_MAX_CHILDREN)
        .map(|index| {
            json!({
                "type": "spacer",
                "id": format!("budget.node_{index}")
            })
        })
        .collect::<Vec<_>>();
    let document = json!({
        "schema_version": 1,
        "document_id": "budget.children",
        "root": {
            "type": "container",
            "id": "budget.root",
            "children": children
        }
    });
    let report = UiDocument::validate_json(&document.to_string()).report;
    assert!(report.has_code("UI_DOCUMENT_CHILDREN_BUDGET_EXCEEDED"));
    assert_eq!(
        report.budget_usage.max_children,
        UI_DOCUMENT_MAX_CHILDREN + 1
    );

    let fallback = "x".repeat(UI_DOCUMENT_MAX_STRING_BYTES + 1);
    let document = json!({
        "schema_version": 1,
        "document_id": "budget.string",
        "root": {
            "type": "text",
            "id": "budget.text",
            "content": { "i18n_key": "budget.title", "fallback": fallback }
        }
    });
    let report = UiDocument::validate_json(&document.to_string()).report;
    assert!(report.has_code("UI_DOCUMENT_STRING_BUDGET_EXCEEDED"));

    let literal = "x".repeat(UI_DOCUMENT_MAX_LITERAL_BYTES + 1);
    let document = json!({
        "schema_version": 1,
        "document_id": "budget.literal",
        "root": {
            "type": "text",
            "id": "budget.text",
            "content": { "literal": literal }
        }
    });
    let report = UiDocument::validate_json(&document.to_string()).report;
    assert!(report.has_code("UI_DOCUMENT_LITERAL_STRING_BUDGET_EXCEEDED"));

    let overrides = (0..=UI_DOCUMENT_MAX_OVERRIDES)
        .map(|_| {
            json!({
                "node_id": "budget.root",
                "set": { "layout": { "width": { "px": 10 } } }
            })
        })
        .collect::<Vec<_>>();
    let document = json!({
        "schema_version": 1,
        "document_id": "budget.overrides",
        "root": { "type": "container", "id": "budget.root" },
        "states": [{ "id": "loading", "overrides": overrides }]
    });
    let report = UiDocument::validate_json(&document.to_string()).report;
    assert!(report.has_code("UI_DOCUMENT_OVERRIDE_BUDGET_EXCEEDED"));
    assert_eq!(
        report.budget_usage.state_responsive_overrides,
        UI_DOCUMENT_MAX_OVERRIDES + 1
    );
}

#[test]
fn ui_document_validation_enforces_all_collection_and_payload_budgets() {
    let groups = (0..4)
        .map(|group| {
            json!({
                "type": "container",
                "id": format!("nodes.group_{group}"),
                "children": (0..128).map(|index| json!({
                    "type": "spacer",
                    "id": format!("nodes.item_{group}_{index}")
                })).collect::<Vec<_>>()
            })
        })
        .collect::<Vec<_>>();
    let document = json!({
        "schema_version": 1,
        "document_id": "budget.nodes",
        "root": { "type": "container", "id": "nodes.root", "children": groups }
    });
    assert!(
        UiDocument::validate_json(&document.to_string())
            .report
            .has_code("UI_DOCUMENT_NODE_COUNT_BUDGET_EXCEEDED")
    );

    let mut root = json!({ "type": "container", "id": "depth.node_24" });
    for depth in (0..24).rev() {
        root = json!({
            "type": "container",
            "id": format!("depth.node_{depth}"),
            "children": [root]
        });
    }
    let document = json!({
        "schema_version": 1,
        "document_id": "budget.depth",
        "root": root
    });
    assert!(
        UiDocument::validate_json(&document.to_string())
            .report
            .has_code("UI_DOCUMENT_TREE_DEPTH_BUDGET_EXCEEDED")
    );

    let base_asset = json!({
        "kind": "image",
        "source": { "kind": "packaged", "path": "ui/images/budget.png" }
    });
    let assets = (0..=UI_DOCUMENT_MAX_ASSETS)
        .map(|index| (format!("asset_{index}"), base_asset.clone()))
        .collect::<serde_json::Map<_, _>>();
    let document = json!({
        "schema_version": 1,
        "document_id": "budget.assets",
        "assets": assets,
        "root": { "type": "container", "id": "assets.root" }
    });
    assert!(
        UiDocument::validate_json(&document.to_string())
            .report
            .has_code("UI_DOCUMENT_ASSET_COUNT_BUDGET_EXCEEDED")
    );

    let styles = (0..=UI_DOCUMENT_MAX_STYLE_ENTRIES)
        .map(|index| (format!("style_{index}"), json!({})))
        .collect::<serde_json::Map<_, _>>();
    let document = json!({
        "schema_version": 1,
        "document_id": "budget.styles",
        "styles": styles,
        "root": { "type": "container", "id": "styles.root" }
    });
    assert!(
        UiDocument::validate_json(&document.to_string())
            .report
            .has_code("UI_DOCUMENT_STYLE_ENTRY_BUDGET_EXCEEDED")
    );

    let buttons = (0..=UI_DOCUMENT_MAX_ACTION_REFERENCES)
        .map(|index| {
            json!({
                "type": "button",
                "id": format!("actions.button_{index}"),
                "label": { "literal": "Action" },
                "on_click": { "action": format!("actions.invoke_{index}") }
            })
        })
        .collect::<Vec<_>>();
    let document = json!({
        "schema_version": 1,
        "document_id": "budget.actions",
        "root": { "type": "container", "id": "actions.root", "children": buttons }
    });
    assert!(
        UiDocument::validate_json(&document.to_string())
            .report
            .has_code("UI_DOCUMENT_ACTION_BUDGET_EXCEEDED")
    );

    let variants = (0..=UI_DOCUMENT_MAX_RESPONSIVE_VARIANTS)
        .map(|index| {
            json!({
                "id": format!("variant_{index}"),
                "when": { "platform": "windows" }
            })
        })
        .collect::<Vec<_>>();
    let document = json!({
        "schema_version": 1,
        "document_id": "budget.responsive",
        "root": { "type": "container", "id": "responsive.root" },
        "responsive": variants
    });
    assert!(
        UiDocument::validate_json(&document.to_string())
            .report
            .has_code("UI_DOCUMENT_RESPONSIVE_BUDGET_EXCEEDED")
    );

    let document = json!({
        "schema_version": 1,
        "document_id": "budget.metadata",
        "metadata": { "title": "x".repeat(UI_DOCUMENT_MAX_METADATA_BYTES + 1) },
        "root": { "type": "container", "id": "metadata.root" }
    });
    assert!(
        UiDocument::validate_json(&document.to_string())
            .report
            .has_code("UI_DOCUMENT_METADATA_BUDGET_EXCEEDED")
    );

    let document = json!({
        "schema_version": 1,
        "document_id": "budget.action_params",
        "root": {
            "type": "button",
            "id": "params.button",
            "label": { "literal": "Payload" },
            "on_click": {
                "action": "params.invoke",
                "params": {
                    "payload": {
                        "kind": "string",
                        "value": "x".repeat(UI_DOCUMENT_MAX_ACTION_PARAM_BYTES + 1)
                    }
                }
            }
        }
    });
    assert!(
        UiDocument::validate_json(&document.to_string())
            .report
            .has_code("UI_DOCUMENT_ACTION_PARAM_BUDGET_EXCEEDED")
    );
}

#[test]
fn ui_document_validation_counts_effects_from_style_references_and_patches() {
    let mut value: Value = serde_json::from_str(STYLE_RESOURCE_DOCUMENT).unwrap();
    let style = value["styles"]["panel_base"].clone();
    let shadow = style["properties"]["shadows"][0].clone();
    let mut three_shadow_style = style;
    three_shadow_style["properties"]["shadows"] =
        Value::Array(vec![shadow.clone(), shadow.clone(), shadow]);
    value["styles"] = Value::Object(
        (0..86)
            .map(|index| (format!("effect_{index}"), three_shadow_style.clone()))
            .collect(),
    );
    value["root"]["style"] = json!({ "component": "effect_0" });
    value["root"]["children"] = json!([]);
    value["assets"]
        .as_object_mut()
        .unwrap()
        .remove("frosted_panel");

    let report = UiDocument::validate_json(&value.to_string()).report;
    assert!(report.has_code("UI_DOCUMENT_EFFECT_COMPLEXITY_BUDGET_EXCEEDED"));
    assert!(report.budget_usage.effect_complexity > UI_DOCUMENT_MAX_EFFECT_COMPLEXITY);
}

#[test]
fn ui_document_typed_validation_reuses_semantic_budget_checks() {
    let mut document = UiDocument::parse_and_validate_json(MINIMAL_DOCUMENT)
        .unwrap()
        .into_document();
    document.metadata.budget_profile = "unregistered_profile".to_owned();

    let error = ValidatedUiDocument::new(document).unwrap_err();
    let UiDocumentError::ValidationReport { report } = error else {
        panic!("typed validation should return a machine report")
    };
    assert!(report.has_code("UI_BUDGET_PROFILE_UNKNOWN"));
    assert_eq!(
        report.diagnostics[0].field_path,
        "$.metadata.budget_profile"
    );
    assert_eq!(
        report.budget_usage.source_bytes,
        UI_DOCUMENT_SOURCE_BYTES_UNKNOWN
    );
}

#[test]
fn ui_document_typed_validation_does_not_treat_expansion_as_source_bytes() {
    let groups = (0..4)
        .map(|group| {
            json!({
                "type": "container",
                "id": format!("expansion.group_{group}"),
                "children": (0..124).map(|index| json!({
                    "type": "spacer",
                    "id": format!("expansion.item_{group}_{index}")
                })).collect::<Vec<_>>()
            })
        })
        .collect::<Vec<_>>();
    let compact = json!({
        "schema_version": 1,
        "document_id": "expansion.equivalent_inputs",
        "root": {
            "type": "container",
            "id": "expansion.root",
            "children": groups
        },
        "responsive": [{
            "id": "compact",
            "when": { "width_class": "compact" },
            "overrides": [{
                "node_id": "expansion.root",
                "set": { "layout": { "gap": { "px": 1 } } }
            }]
        }]
    })
    .to_string();

    assert!(compact.len() < UI_DOCUMENT_MAX_BYTES);
    let json_result = UiDocument::validate_json(&compact);
    assert!(json_result.report.valid, "{:#?}", json_result.report);
    assert_eq!(json_result.report.budget_usage.nodes, 501);
    assert_eq!(json_result.report.budget_usage.max_tree_depth, 3);
    assert_eq!(json_result.report.budget_usage.max_children, 124);
    let document = json_result.into_validated().unwrap().into_document();
    let expanded_bytes = serde_json::to_vec(&document).unwrap().len();
    eprintln!(
        "typed expansion regression: compact_bytes={}, expanded_bytes={expanded_bytes}",
        compact.len()
    );
    assert!(expanded_bytes > UI_DOCUMENT_MAX_BYTES);

    let typed = ValidatedUiDocument::new(document).unwrap();
    let profile = UiTargetProfile::new(
        360.0,
        700.0,
        UiSafeAreaClass::None,
        UiDocumentInputMode::MouseKeyboard,
        UiDocumentPlatform::Windows,
    )
    .unwrap();
    let effective = typed
        .effective_document(&profile, &UiPageState::initial())
        .unwrap();
    assert_eq!(effective.applied_overrides.len(), 1);
    assert_eq!(effective.document.root.children().len(), 4);
}

#[test]
fn ui_document_validation_never_panics_for_deterministic_malformed_inputs() {
    let corpus = [
        "",
        "null",
        "[]",
        "{",
        "{\"schema_version\":1e999}",
        "{\"schema_version\":1,\"document_id\":\"测试.页面\"}",
        "{\"path\":\"C:\\\\secret\\\\file\"}",
        &format!("{}0{}", "[".repeat(140), "]".repeat(140)),
    ];
    for source in corpus {
        assert!(
            catch_unwind(AssertUnwindSafe(|| UiDocument::validate_json(source))).is_ok(),
            "validator panicked for {source:?}"
        );
    }

    let mut state = 0x9e37_79b9_7f4a_7c15u64;
    for length in 0..512usize {
        let mut bytes = Vec::with_capacity(length);
        for _ in 0..length {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            bytes.push((state >> 24) as u8);
        }
        let result = catch_unwind(AssertUnwindSafe(|| UiDocument::validate_json_bytes(&bytes)));
        assert!(
            result.is_ok(),
            "validator panicked for seed length {length}"
        );
        let report = result.unwrap().report;
        assert!(report.diagnostics.len() <= UI_DOCUMENT_MAX_DIAGNOSTICS);
    }
    assert!(
        UiDocument::validate_json_bytes(&[0xff, 0xfe, 0xfd])
            .report
            .has_code("UI_DOCUMENT_UTF8_INVALID")
    );
}

#[test]
fn ui_document_tree_and_asset_models_are_reachable_and_resource_acyclic_by_construction() {
    let result = UiDocument::validate_json(MINIMAL_DOCUMENT);
    let validated = result.validated().unwrap();
    let mut reachable = 0usize;
    let mut pending = vec![&validated.document().root];
    while let Some(node) = pending.pop() {
        reachable += 1;
        pending.extend(node.children());
    }
    assert_eq!(reachable, result.report.budget_usage.nodes);

    // Asset entries contain sources and atlas frames, but no edge to another asset entry.
    for entry in validated.document().assets.values() {
        assert!(matches!(
            entry.source,
            UiAssetSource::Packaged { .. }
                | UiAssetSource::ContentCache { .. }
                | UiAssetSource::BuiltInMaterial { .. }
        ));
    }
}
