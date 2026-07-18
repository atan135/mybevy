use serde_json::Value;
use std::{fs, path::PathBuf};
use tempfile::TempDir;
use ui_visual_audit::{
    ComparisonErrorCode, SemanticAuditRequest, SemanticAuditStatus, SemanticFindingCode,
    audit_semantics,
};

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .unwrap()
        .to_path_buf()
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/semantic")
        .join(name)
}

fn run(metadata: PathBuf, output: &TempDir) -> ui_visual_audit::SemanticAuditOutcome {
    audit_semantics(&SemanticAuditRequest {
        repository_root: repository_root(),
        allowed_input_roots: vec![
            PathBuf::from("tools/ui-visual-audit/fixtures/semantic"),
            output.path().to_path_buf(),
        ],
        allowed_output_root: output.path().to_path_buf(),
        metadata,
        config: fixture("ui-semantic-audit-v1.config.json"),
        output_directory: output.path().join("result"),
    })
    .unwrap()
}

#[test]
fn compact_and_expanded_profiles_pass_and_skip_nonsemantic_hidden_nodes() {
    for name in ["compact-pass.metadata.json", "expanded-pass.metadata.json"] {
        let output = TempDir::new_in(repository_root()).unwrap();
        let outcome = run(fixture(name), &output);
        assert_eq!(outcome.report.schema_version, 2);
        assert_eq!(outcome.report.status, SemanticAuditStatus::Passed);
        assert!(!outcome.report.separation.semantic_hard_failure);
        assert!(!outcome.report.separation.visual_similarity_consumed);
        assert!(
            !outcome
                .report
                .separation
                .can_visual_score_offset_hard_failure
        );
        if name.starts_with("compact") {
            assert_eq!(outcome.report.rules.skipped_invisible_nodes, 1);
            assert_eq!(outcome.report.rules.skipped_fully_clipped_nodes, 1);
            assert_eq!(outcome.report.input.device_profile, "compact");
        } else {
            assert_eq!(outcome.report.input.device_profile, "expanded");
        }
    }
}

#[test]
fn public_audit_rejects_more_than_the_fixed_finding_limit() {
    let output = TempDir::new_in(repository_root()).unwrap();
    let metadata_path = output.path().join("overlap-limit.metadata.json");
    let mut value: Value =
        serde_json::from_slice(&fs::read(fixture("compact-pass.metadata.json")).unwrap()).unwrap();
    let nodes = value["semantic_tree"]["nodes"].as_array_mut().unwrap();
    let template = nodes[1].clone();
    for index in 0..46 {
        let mut overlap = template.clone();
        overlap["stable_id"] =
            format!("document:login:page/example.login/node:page.overlap_{index:02}").into();
        overlap["capture_entity"] = format!("{}v1#limit", 500 + index).into();
        overlap["entity_name"] = format!("OverlapText{index:02}").into();
        overlap["stack_index"] = (500 + index).into();
        overlap["node_id"] = format!("page.overlap_{index:02}").into();
        nodes.push(overlap);
    }
    fs::write(&metadata_path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    let error = audit_semantics(&SemanticAuditRequest {
        repository_root: repository_root(),
        allowed_input_roots: vec![
            PathBuf::from("tools/ui-visual-audit/fixtures/semantic"),
            output.path().to_path_buf(),
        ],
        allowed_output_root: output.path().to_path_buf(),
        metadata: metadata_path,
        config: fixture("ui-semantic-audit-v1.config.json"),
        output_directory: output.path().join("result"),
    })
    .unwrap_err();
    assert_eq!(
        error.failure.code,
        ComparisonErrorCode::SemanticFindingsLimitExceeded
    );
    assert_eq!(error.exit_code().as_i32(), 2);
}

#[test]
fn zero_sized_actionable_and_image_nodes_fail_before_clipped_nonzero_nodes_skip() {
    let output = TempDir::new_in(repository_root()).unwrap();
    let metadata_path = output.path().join("zero-size.metadata.json");
    let mut value: Value =
        serde_json::from_slice(&fs::read(fixture("compact-pass.metadata.json")).unwrap()).unwrap();
    let nodes = value["semantic_tree"]["nodes"].as_array_mut().unwrap();
    nodes[4]["visible"] = true.into();
    nodes[4]["fully_clipped"] = true.into();
    nodes[4]["has_visible_label"] = true.into();
    let mut image = nodes[4].clone();
    image["stable_id"] = "panel:login/root/image[0]".into();
    image["capture_entity"] = "zero-image".into();
    image["role"] = "image".into();
    nodes.push(image);
    fs::write(&metadata_path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    let outcome = run(metadata_path, &output);
    let zero_ids = outcome
        .report
        .findings
        .iter()
        .filter(|finding| finding.code == SemanticFindingCode::SemanticNodeZeroSize)
        .map(|finding| finding.primary.stable_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert!(zero_ids.contains("panel:login/root/button[1]"));
    assert!(zero_ids.contains("panel:login/root/image[0]"));
    assert!(!zero_ids.contains("panel:login/root/button[2]"));
}

#[test]
fn clipped_runtime_label_evidence_produces_a_missing_label_hard_failure() {
    let output = TempDir::new_in(repository_root()).unwrap();
    let metadata_path = output.path().join("clipped-label.metadata.json");
    let mut value: Value =
        serde_json::from_slice(&fs::read(fixture("compact-pass.metadata.json")).unwrap()).unwrap();
    value["semantic_tree"]["nodes"][2]["has_visible_label"] = false.into();
    fs::write(&metadata_path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    let outcome = run(metadata_path, &output);
    let finding = outcome
        .report
        .findings
        .iter()
        .find(|finding| finding.code == SemanticFindingCode::VisibleLabelMissing)
        .unwrap();
    assert_eq!(
        finding.primary.stable_id,
        "document:login:page/example.login/node:page.continue"
    );
    assert!(outcome.report.separation.semantic_hard_failure);
}

#[test]
fn all_layout_control_scroll_and_overlay_failures_are_structured_and_located() {
    let output = TempDir::new_in(repository_root()).unwrap();
    let metadata_path = output.path().join("failures.metadata.json");
    let mut value: Value =
        serde_json::from_slice(&fs::read(fixture("compact-pass.metadata.json")).unwrap()).unwrap();
    let nodes = value["semantic_tree"]["nodes"].as_array_mut().unwrap();

    let mut overlap = nodes[1].clone();
    overlap["stable_id"] = "document:login:page/example.login/node:page.subtitle".into();
    overlap["node_id"] = "page.subtitle".into();
    overlap["capture_entity"] = "77v1#x".into();
    overlap["measured_text_bounds"] =
        serde_json::json!({ "min_x": 100.0, "min_y": 60.0, "max_x": 220.0, "max_y": 82.0 });
    nodes.push(overlap);
    nodes[1]["clip_bounds"] =
        serde_json::json!({ "min_x": 24.0, "min_y": 48.0, "max_x": 120.0, "max_y": 70.0 });
    nodes[1]["bounds"] =
        serde_json::json!({ "min_x": -2.0, "min_y": 48.0, "max_x": 366.0, "max_y": 80.0 });
    nodes[2]["bounds"] =
        serde_json::json!({ "min_x": 24.0, "min_y": 100.0, "max_x": 50.0, "max_y": 120.0 });
    nodes[2]["has_visible_label"] = false.into();
    nodes[2]["disabled"] = true.into();
    nodes[2]["focused"] = true.into();
    nodes[2]["interaction"] = "hovered".into();
    nodes[3]["scroll"]["content_reachable"] = false.into();

    let mut zero = nodes[2].clone();
    zero["stable_id"] = "panel:login/root/icon_button[9]".into();
    zero["identity_source"] = "hierarchy_fallback".into();
    zero["document_id"] = Value::Null;
    zero["node_id"] = Value::Null;
    zero["source_path"] = Value::Null;
    zero["capture_entity"] = "99v1#x".into();
    zero["entity_name"] = "LoginZeroIconButton".into();
    zero["stack_index"] = 99.into();
    zero["role"] = "icon_button".into();
    zero["bounds"] =
        serde_json::json!({ "min_x": 100.0, "min_y": 200.0, "max_x": 100.0, "max_y": 200.0 });
    zero["clip_bounds"] = zero["bounds"].clone();
    zero["has_visible_label"] = true.into();
    zero["disabled"] = false.into();
    zero["focused"] = false.into();
    zero["interaction"] = "none".into();
    zero["likely_files"] = serde_json::json!(["project/src/game/screens/login.rs"]);
    nodes.push(zero);

    let mut loading = nodes[2].clone();
    loading["stable_id"] = "document:login:page/example.login/node:page.loading".into();
    loading["node_id"] = "page.loading".into();
    loading["capture_entity"] = "100v1#x".into();
    loading["bounds"] =
        serde_json::json!({ "min_x": 24.0, "min_y": 730.0, "max_x": 120.0, "max_y": 780.0 });
    loading["clip_bounds"] = loading["bounds"].clone();
    loading["loading"] = true.into();
    loading["disabled"] = false.into();
    loading["focused"] = false.into();
    loading["interaction"] = "pressed".into();
    loading["has_visible_label"] = true.into();
    nodes.push(loading);

    let panels = value["semantic_tree"]["panels"].as_array_mut().unwrap();
    panels.push(serde_json::json!({
        "stable_id": "confirm_modal", "capture_entity": "modal-test-entity",
        "entity_name": "UiConfirmModalRoot",
        "likely_files": ["project/src/framework/ui/overlays/modal.rs"],
        "kind": "modal", "layer_policy": "modal", "visible": true,
        "z_index": -1, "has_focusable_descendants": true,
        "focused_descendant": false, "focused_stable_id": null, "active_focus_scope": true,
        "focus_scope_enforced": false, "focus_suppressed": false,
        "blocks_lower_input": false,
        "pickable_blocks_lower": false, "input_block_reason": "none"
    }));
    panels.push(serde_json::json!({
        "stable_id": "toast:0", "capture_entity": "toast-test-entity",
        "entity_name": "UiToastRoot",
        "likely_files": ["project/src/framework/ui/overlays/toast.rs"],
        "kind": "toast", "layer_policy": "toast", "visible": true,
        "z_index": -2, "has_focusable_descendants": false,
        "focused_descendant": false, "focused_stable_id": null, "active_focus_scope": false,
        "focus_scope_enforced": true, "focus_suppressed": true,
        "blocks_lower_input": true,
        "pickable_blocks_lower": true, "input_block_reason": "toast"
    }));
    fs::write(&metadata_path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();

    let outcome = run(metadata_path, &output);
    assert_eq!(outcome.report.status, SemanticAuditStatus::SemanticFailed);
    assert_eq!(outcome.exit_code.as_i32(), 4);
    assert!(outcome.report.separation.semantic_hard_failure);
    assert!(
        !outcome
            .report
            .separation
            .can_visual_score_offset_hard_failure
    );
    let codes = outcome
        .report
        .findings
        .iter()
        .map(|finding| finding.code)
        .collect::<std::collections::BTreeSet<_>>();
    for expected in [
        SemanticFindingCode::TextOverlap,
        SemanticFindingCode::CriticalTextClipped,
        SemanticFindingCode::SafeAreaOverflow,
        SemanticFindingCode::ScrollContentUnreachable,
        SemanticFindingCode::SemanticNodeZeroSize,
        SemanticFindingCode::TouchTargetTooSmall,
        SemanticFindingCode::VisibleLabelMissing,
        SemanticFindingCode::DisabledStateInconsistent,
        SemanticFindingCode::LoadingStateInconsistent,
        SemanticFindingCode::OverlayZOrderInvalid,
        SemanticFindingCode::OverlayFocusScopeInvalid,
        SemanticFindingCode::OverlayInputBlockingInvalid,
    ] {
        assert!(codes.contains(&expected), "missing {expected:?}");
    }
    let declarative = outcome
        .report
        .findings
        .iter()
        .find(|finding| {
            finding.primary.stable_id == "document:login:page/example.login/node:page.title"
        })
        .unwrap();
    assert_eq!(
        declarative.primary.document_id.as_deref(),
        Some("example.login")
    );
    assert_eq!(declarative.primary.node_id.as_deref(), Some("page.title"));
    assert_eq!(
        declarative.primary.source_path.as_deref(),
        Some("ui/documents/approved/login/document.v1.json")
    );
    let traditional = outcome
        .report
        .findings
        .iter()
        .find(|finding| finding.primary.stable_id == "panel:login/root/icon_button[9]")
        .unwrap();
    assert_eq!(traditional.primary.panel_id.as_deref(), Some("login"));
    assert_eq!(
        traditional.primary.entity_name.as_deref(),
        Some("LoginZeroIconButton")
    );
    assert_eq!(
        traditional.primary.likely_files,
        ["project/src/game/screens/login.rs"]
    );
    let modal = outcome
        .report
        .findings
        .iter()
        .find(|finding| finding.primary.stable_id == "confirm_modal")
        .unwrap();
    assert_eq!(modal.primary.capture_entity, "modal-test-entity");
    assert_eq!(
        modal.primary.entity_name.as_deref(),
        Some("UiConfirmModalRoot")
    );
    assert_eq!(modal.primary.panel_id.as_deref(), Some("confirm_modal"));
    assert_eq!(
        modal.primary.likely_files,
        ["project/src/framework/ui/overlays/modal.rs"]
    );
}

#[test]
fn output_is_deterministic_across_capture_entity_changes() {
    let first = TempDir::new_in(repository_root()).unwrap();
    let second = TempDir::new_in(repository_root()).unwrap();
    let first_outcome = run(fixture("compact-pass.metadata.json"), &first);
    let mut value: Value =
        serde_json::from_slice(&fs::read(fixture("compact-pass.metadata.json")).unwrap()).unwrap();
    for (index, node) in value["semantic_tree"]["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        node["capture_entity"] = format!("{}v9#different", 900 + index).into();
    }
    let changed = second.path().join("changed.metadata.json");
    fs::write(&changed, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    let second_outcome = run(changed, &second);
    let first_ids = first_outcome
        .report
        .findings
        .iter()
        .map(|finding| &finding.primary.stable_id)
        .collect::<Vec<_>>();
    let second_ids = second_outcome
        .report
        .findings
        .iter()
        .map(|finding| &finding.primary.stable_id)
        .collect::<Vec<_>>();
    assert_eq!(first_ids, second_ids);
    assert_eq!(first_outcome.report.status, second_outcome.report.status);
}

#[test]
fn runtime_semantic_tree_fixture_matches_the_tool_protocol_exactly() {
    for name in ["compact-pass.metadata.json", "expanded-pass.metadata.json"] {
        let value: Value = serde_json::from_slice(&fs::read(fixture(name)).unwrap()).unwrap();
        serde_json::from_value::<ui_visual_audit::SemanticTree>(
            value["semantic_tree"].clone(),
        )
        .expect("runtime semantic_tree serialization must match the deny-unknown-fields tool schema");
    }
}

#[test]
fn strict_schema_rejects_a_node_that_references_a_missing_panel() {
    let output = TempDir::new_in(repository_root()).unwrap();
    let metadata_path = output.path().join("missing-panel.metadata.json");
    let mut value: Value =
        serde_json::from_slice(&fs::read(fixture("compact-pass.metadata.json")).unwrap()).unwrap();
    value["semantic_tree"]["nodes"][1]["panel_id"] = "missing_panel".into();
    fs::write(&metadata_path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    let error = audit_semantics(&SemanticAuditRequest {
        repository_root: repository_root(),
        allowed_input_roots: vec![
            PathBuf::from("tools/ui-visual-audit/fixtures/semantic"),
            output.path().to_path_buf(),
        ],
        allowed_output_root: output.path().to_path_buf(),
        metadata: metadata_path,
        config: fixture("ui-semantic-audit-v1.config.json"),
        output_directory: output.path().join("result"),
    })
    .unwrap_err();
    assert_eq!(
        error.failure.code,
        ComparisonErrorCode::SemanticIdentityInvalid
    );
    assert!(error.failure.message.contains("references missing panel"));
}

#[test]
fn text_overlap_uses_visible_clip_and_never_compares_different_panels() {
    let output = TempDir::new_in(repository_root()).unwrap();
    let metadata_path = output.path().join("layered.metadata.json");
    let mut value: Value =
        serde_json::from_slice(&fs::read(fixture("compact-pass.metadata.json")).unwrap()).unwrap();
    let nodes = value["semantic_tree"]["nodes"].as_array_mut().unwrap();
    let mut clipped = nodes[1].clone();
    clipped["stable_id"] = "document:login:page/example.login/node:page.clipped_sibling".into();
    clipped["node_id"] = "page.clipped_sibling".into();
    clipped["capture_entity"] = "201v1#x".into();
    clipped["role"] = "text".into();
    clipped["measured_text_bounds"] =
        serde_json::json!({ "min_x": 180.0, "min_y": 48.0, "max_x": 260.0, "max_y": 78.0 });
    clipped["clip_bounds"] =
        serde_json::json!({ "min_x": 250.0, "min_y": 48.0, "max_x": 260.0, "max_y": 78.0 });
    nodes.push(clipped);
    let mut modal = nodes[1].clone();
    modal["stable_id"] = "panel:modal/root/text[0]".into();
    modal["identity_source"] = "hierarchy_fallback".into();
    modal["capture_entity"] = "202v1#x".into();
    modal["document_id"] = Value::Null;
    modal["node_id"] = Value::Null;
    modal["source_path"] = Value::Null;
    modal["panel_id"] = "confirm_modal".into();
    modal["likely_files"] = serde_json::json!(["project/src/framework/ui/overlays/modal.rs"]);
    nodes.push(modal);
    value["semantic_tree"]["panels"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "stable_id": "confirm_modal", "capture_entity": "modal-layer-entity",
            "entity_name": "UiConfirmModalRoot",
            "likely_files": ["project/src/framework/ui/overlays/modal.rs"],
            "kind": "modal", "layer_policy": "modal", "visible": true, "z_index": 100,
            "has_focusable_descendants": false, "focused_descendant": false,
            "focused_stable_id": null, "active_focus_scope": false,
            "focus_scope_enforced": false, "focus_suppressed": true,
            "blocks_lower_input": true, "pickable_blocks_lower": true,
            "input_block_reason": "confirm_modal Modal"
        }));
    fs::write(&metadata_path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    let outcome = run(metadata_path, &output);
    assert_eq!(outcome.report.status, SemanticAuditStatus::Passed);
    assert!(
        outcome
            .report
            .findings
            .iter()
            .all(|finding| finding.code != SemanticFindingCode::TextOverlap)
    );
}

#[test]
fn modal_with_focused_dropdown_above_it_is_a_legal_active_scope() {
    let output = TempDir::new_in(repository_root()).unwrap();
    let metadata_path = output.path().join("modal-dropdown.metadata.json");
    let mut value: Value =
        serde_json::from_slice(&fs::read(fixture("expanded-pass.metadata.json")).unwrap()).unwrap();
    let panels = value["semantic_tree"]["panels"].as_array_mut().unwrap();
    panels[1]["active_focus_scope"] = false.into();
    panels[1]["focused_descendant"] = false.into();
    panels[1]["focused_stable_id"] = Value::Null;
    panels[1]["focus_scope_enforced"] = false.into();
    panels.push(serde_json::json!({
        "stable_id": "dropdown", "capture_entity": "dropdown-pass-entity",
        "entity_name": "UiDropdownRoot",
        "likely_files": ["project/src/framework/ui/overlays/popover.rs"], "kind": "floating",
        "layer_policy": "transient_above_modal", "visible": true, "z_index": 120,
        "has_focusable_descendants": true, "focused_descendant": true,
        "focused_stable_id": "panel:settings/root/button[0]", "active_focus_scope": true,
        "focus_scope_enforced": true, "focus_suppressed": false,
        "blocks_lower_input": false, "pickable_blocks_lower": true,
        "input_block_reason": "none"
    }));
    fs::write(&metadata_path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    assert_eq!(
        run(metadata_path, &output).report.status,
        SemanticAuditStatus::Passed
    );
}

#[test]
fn transient_above_modal_policy_requires_a_strictly_higher_z_index() {
    let output = TempDir::new_in(repository_root()).unwrap();
    let metadata_path = output.path().join("low-dropdown.metadata.json");
    let mut value: Value =
        serde_json::from_slice(&fs::read(fixture("expanded-pass.metadata.json")).unwrap()).unwrap();
    let panels = value["semantic_tree"]["panels"].as_array_mut().unwrap();
    panels[1]["active_focus_scope"] = false.into();
    panels[1]["focused_descendant"] = false.into();
    panels[1]["focused_stable_id"] = Value::Null;
    panels.push(serde_json::json!({
        "stable_id": "dropdown", "capture_entity": "dropdown-low-entity",
        "entity_name": "UiDropdownRoot",
        "likely_files": ["project/src/framework/ui/overlays/popover.rs"], "kind": "floating",
        "layer_policy": "transient_above_modal", "visible": true, "z_index": 90,
        "has_focusable_descendants": true, "focused_descendant": true,
        "focused_stable_id": "panel:settings/root/button[0]", "active_focus_scope": true,
        "focus_scope_enforced": true, "focus_suppressed": false,
        "blocks_lower_input": false, "pickable_blocks_lower": true,
        "input_block_reason": "none"
    }));
    fs::write(&metadata_path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    let outcome = run(metadata_path, &output);
    assert!(outcome.report.findings.iter().any(|finding| {
        finding.code == SemanticFindingCode::OverlayZOrderInvalid
            && finding.primary.stable_id == "dropdown"
    }));
}

#[test]
fn tooltip_requires_complete_subtree_pick_through_while_dropdown_may_block() {
    let pass_output = TempDir::new_in(repository_root()).unwrap();
    let pass_metadata = pass_output.path().join("tooltip-pass.metadata.json");
    let mut value: Value =
        serde_json::from_slice(&fs::read(fixture("compact-pass.metadata.json")).unwrap()).unwrap();
    value["semantic_tree"]["panels"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "stable_id": "tooltip", "capture_entity": "tooltip-test-entity",
            "entity_name": "UiTooltipRoot",
            "likely_files": ["project/src/framework/ui/overlays/popover.rs"], "kind": "floating",
            "layer_policy": "transient_above_modal", "visible": true, "z_index": 120,
            "has_focusable_descendants": false, "focused_descendant": false,
            "focused_stable_id": null, "active_focus_scope": false,
            "focus_scope_enforced": false, "focus_suppressed": true,
            "blocks_lower_input": false, "pickable_blocks_lower": false,
            "input_block_reason": "none"
        }));
    fs::write(&pass_metadata, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    assert_eq!(
        run(pass_metadata, &pass_output).report.status,
        SemanticAuditStatus::Passed
    );

    let fail_output = TempDir::new_in(repository_root()).unwrap();
    value["semantic_tree"]["panels"][1]["pickable_blocks_lower"] = true.into();
    let fail_metadata = fail_output.path().join("tooltip-fail.metadata.json");
    fs::write(&fail_metadata, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    let outcome = run(fail_metadata, &fail_output);
    assert!(outcome.report.findings.iter().any(|finding| {
        finding.code == SemanticFindingCode::OverlayInputBlockingInvalid
            && finding.primary.stable_id == "tooltip"
    }));
}

#[test]
fn modal_focus_on_lower_page_and_unapproved_floating_over_modal_fail() {
    let output = TempDir::new_in(repository_root()).unwrap();
    let metadata_path = output.path().join("invalid-overlay.metadata.json");
    let mut value: Value =
        serde_json::from_slice(&fs::read(fixture("expanded-pass.metadata.json")).unwrap()).unwrap();
    let panels = value["semantic_tree"]["panels"].as_array_mut().unwrap();
    panels[1]["focused_descendant"] = false.into();
    panels[1]["focused_stable_id"] = Value::Null;
    panels.push(serde_json::json!({
        "stable_id": "inspector", "capture_entity": "inspector-test-entity",
        "entity_name": "GalleryInspector",
        "likely_files": ["project/src/game/screens/dev/ui_gallery.rs"],
        "kind": "floating", "layer_policy": "floating",
        "visible": true, "z_index": 120, "has_focusable_descendants": false,
        "focused_descendant": false, "focused_stable_id": null, "active_focus_scope": false,
        "focus_scope_enforced": false, "focus_suppressed": false,
        "blocks_lower_input": false, "pickable_blocks_lower": true,
        "input_block_reason": "none"
    }));
    fs::write(&metadata_path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    let outcome = run(metadata_path, &output);
    assert!(
        outcome
            .report
            .findings
            .iter()
            .any(|finding| finding.code == SemanticFindingCode::OverlayFocusScopeInvalid)
    );
    assert!(
        outcome
            .report
            .findings
            .iter()
            .any(|finding| finding.code == SemanticFindingCode::OverlayZOrderInvalid)
    );
}

#[test]
fn loading_without_focusables_passes_when_focus_is_explicitly_suppressed() {
    let output = TempDir::new_in(repository_root()).unwrap();
    let metadata_path = output.path().join("loading.metadata.json");
    let mut value: Value =
        serde_json::from_slice(&fs::read(fixture("expanded-pass.metadata.json")).unwrap()).unwrap();
    let panel = &mut value["semantic_tree"]["panels"][1];
    panel["stable_id"] = "global_loading".into();
    panel["kind"] = "blocking_overlay".into();
    panel["layer_policy"] = "blocking".into();
    panel["z_index"] = 150.into();
    panel["has_focusable_descendants"] = false.into();
    panel["focused_descendant"] = false.into();
    panel["focused_stable_id"] = Value::Null;
    panel["active_focus_scope"] = true.into();
    panel["focus_scope_enforced"] = true.into();
    panel["focus_suppressed"] = true.into();
    panel["input_block_reason"] = "global_loading BlockingOverlay".into();
    value["semantic_tree"]["nodes"][1]["panel_id"] = "global_loading".into();
    fs::write(&metadata_path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    assert_eq!(
        run(metadata_path, &output).report.status,
        SemanticAuditStatus::Passed
    );
}
