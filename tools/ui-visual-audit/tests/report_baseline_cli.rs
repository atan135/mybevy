use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use tempfile::TempDir;
use ui_visual_audit::{
    BaselineApplyRequest, BaselinePlanRequest, BaselineRerunVerificationRequest,
    ComparisonErrorCode, ReportBuildRequest, apply_baseline_update, build_comparison_report,
    plan_baseline_update, verify_baseline_rerun,
};

struct Fixture {
    _temporary: TempDir,
    root: PathBuf,
    inputs: PathBuf,
    outputs: PathBuf,
    manifest: PathBuf,
    old_hash: String,
    new_hash: String,
}

impl Fixture {
    fn new() -> Self {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().to_path_buf();
        let inputs = root.join("inputs");
        let outputs = root.join("outputs");
        let references = root.join("tools/ui-visual-audit/fixtures/references/gallery");
        fs::create_dir_all(&inputs).unwrap();
        fs::create_dir_all(&outputs).unwrap();
        fs::create_dir_all(&references).unwrap();
        let old = png_bytes([20, 40, 60, 255]);
        let new = png_bytes([80, 100, 120, 255]);
        let tablet = png_bytes([30, 50, 70, 255]);
        fs::write(references.join("phone.png"), &old).unwrap();
        fs::write(references.join("tablet.png"), &tablet).unwrap();
        fs::write(inputs.join("candidate.png"), &new).unwrap();
        fs::write(inputs.join("before.json"), b"{\"metric\":10}\n").unwrap();
        fs::write(inputs.join("after.json"), b"{\"metric\":2}\n").unwrap();
        let old_hash = hash(&old);
        let new_hash = hash(&new);
        let tablet_hash = hash(&tablet);
        let manifest = inputs.join("references.json");
        let reference = |id: &str, device: &str, locale: &str, image: &str, image_hash: &str| {
            json!({
                "reference_id": id,
                "key": {"screen":"gallery","device":device,"state":"default","locale":locale,"theme":"light"},
                "viewport": {
                    "logical_size":{"width":2.0,"height":2.0},
                    "physical_size":{"width":2,"height":2},
                    "device_scale":1.0,
                    "orientation":"square"
                },
                "image":{"storage":"committed_fixture","relative_path":format!("gallery/{image}"),"sha256":image_hash},
                "metadata":{"original_size":{"width":2,"height":2},"color_space":"srgb"},
                "provenance":{"source":"test","source_uri":null,"authorization_status":"repository_owned","license_id":"test-owned"},
                "baseline":{"version":1,"update_reason":"initial fixture","previous_sha256":null},
                "allowed_differences":{"profile":"strict","per_channel_tolerance":0,"max_changed_pixel_ratio":0.0,"notes":[]}
            })
        };
        write_json(
            &manifest,
            &json!({
                "schema_version":1,
                "references":[
                    reference("gallery_phone", "phone-small", "en_us", "phone.png", &old_hash),
                    reference("gallery_tablet", "tablet-portrait", "en_us", "tablet.png", &tablet_hash),
                    reference("gallery_tablet_alias", "tablet-portrait", "fr_fr", "tablet.png", &tablet_hash)
                ]
            }),
        );
        Self {
            _temporary: temporary,
            root,
            inputs,
            outputs,
            manifest,
            old_hash,
            new_hash,
        }
    }

    fn plan(
        &self,
        output: &str,
    ) -> Result<ui_visual_audit::baseline::BaselineUpdatePlan, ui_visual_audit::ComparisonError>
    {
        plan_baseline_update(&BaselinePlanRequest {
            repository_root: self.root.clone(),
            manifest: self.manifest.clone(),
            reference_id: "gallery_phone".to_owned(),
            new_image: self.inputs.join("candidate.png"),
            reason: "approved visual refresh".to_owned(),
            metrics_before: self.inputs.join("before.json"),
            metrics_after: self.inputs.join("after.json"),
            allowed_output_root: self.outputs.clone(),
            output_directory: self.outputs.join(output),
        })
    }

    fn write_approval(&self, plan_path: &Path, approved: bool, name: &str) -> PathBuf {
        let path = self.inputs.join(name);
        write_json(
            &path,
            &json!({
                "schema_version":1,
                "plan_sha256":hash(&fs::read(plan_path).unwrap()),
                "approved":approved,
                "approver":"fixture-reviewer",
                "approved_at":"2026-07-19T03:00:00+08:00",
                "rationale":"reviewed old and new images plus metric delta"
            }),
        );
        path
    }
}

#[test]
fn report_success_is_deterministic_linked_and_no_clobber() {
    let fixture = Fixture::new();
    let bundle = write_report_bundle(
        &fixture,
        "report-input.json",
        &fixture.old_hash,
        None,
        false,
    );
    let request = ReportBuildRequest {
        repository_root: fixture.root.clone(),
        allowed_input_roots: vec![fixture.root.clone()],
        allowed_output_root: fixture.outputs.clone(),
        bundle,
        output_directory: fixture.outputs.join("report"),
    };
    let result = build_comparison_report(&request).unwrap();
    assert_eq!(result.status, "passed");
    assert!(result.root.root_to_comparison_verified);
    let report = fs::read_to_string(fixture.outputs.join("report/report.md")).unwrap();
    for text in [
        "gallery / phone-small / default",
        "Reference / actual / overlay / heatmap",
        "AI actually ran: `false`",
        "Allowed differences",
        "Algorithms",
        "Capture thresholds: strict (max_raw=1000, minimum_ssim=990000)",
        "clock (dynamic clock) @ 0,0,1,1",
        "strict (max_raw=1000)",
        "Source path",
    ] {
        assert!(report.contains(text), "missing {text}");
    }
    let first = fs::read(fixture.outputs.join("report/comparison-result.json")).unwrap();
    let error = build_comparison_report(&request).unwrap_err();
    assert_eq!(
        error.failure.code,
        ComparisonErrorCode::OutputDirectoryNotEmpty
    );

    let second_bundle = write_report_bundle(
        &fixture,
        "report-input-2.json",
        &fixture.old_hash,
        None,
        false,
    );
    let second = build_comparison_report(&ReportBuildRequest {
        bundle: second_bundle,
        output_directory: fixture.outputs.join("report-2"),
        ..request
    })
    .unwrap();
    assert_eq!(result.captures, second.captures);
    let second_bytes = fs::read(fixture.outputs.join("report-2/comparison-result.json")).unwrap();
    let first_value: Value = serde_json::from_slice(&first).unwrap();
    let second_value: Value = serde_json::from_slice(&second_bytes).unwrap();
    assert_eq!(first_value["captures"], second_value["captures"]);
}

#[test]
fn report_rejects_unlocated_regions_and_out_of_bounds_evidence() {
    let fixture = Fixture::new();
    let bundle = write_report_bundle(
        &fixture,
        "invalid-location.json",
        &fixture.old_hash,
        None,
        false,
    );
    let mut value: Value = serde_json::from_slice(&fs::read(&bundle).unwrap()).unwrap();
    value["captures"][0]["issues"][0]["region_id"] = json!("missing-region");
    write_json(&bundle, &value);
    rebind_root(&fixture, &bundle);
    let error = build_comparison_report(&ReportBuildRequest {
        repository_root: fixture.root.clone(),
        allowed_input_roots: vec![fixture.root.clone()],
        allowed_output_root: fixture.outputs.clone(),
        bundle: bundle.clone(),
        output_directory: fixture.outputs.join("unknown-region-report"),
    })
    .unwrap_err();
    assert_eq!(error.failure.code, ComparisonErrorCode::ReportInputInvalid);

    value["captures"][0]["issues"][0]["region_id"] = json!("content");
    value["captures"][0]["issues"][0]["evidence"]["rect"]["width"] = json!(3);
    write_json(&bundle, &value);
    rebind_root(&fixture, &bundle);
    let error = build_comparison_report(&ReportBuildRequest {
        repository_root: fixture.root.clone(),
        allowed_input_roots: vec![fixture.root.clone()],
        allowed_output_root: fixture.outputs.clone(),
        bundle,
        output_directory: fixture.outputs.join("out-of-bounds-report"),
    })
    .unwrap_err();
    assert_eq!(error.failure.code, ComparisonErrorCode::ReportInputInvalid);
}

#[test]
fn report_rejects_missing_swapped_and_unapproved_baseline_evidence() {
    let fixture = Fixture::new();
    let bundle = write_report_bundle(&fixture, "missing.json", &fixture.old_hash, None, false);
    let mut value: Value = serde_json::from_slice(&fs::read(&bundle).unwrap()).unwrap();
    value["captures"][0]["artifacts"]["heatmap"]["path"] = json!("inputs/missing.png");
    write_json(&bundle, &value);
    rebind_root(&fixture, &bundle);
    let error = build_comparison_report(&ReportBuildRequest {
        repository_root: fixture.root.clone(),
        allowed_input_roots: vec![fixture.root.clone()],
        allowed_output_root: fixture.outputs.clone(),
        bundle: bundle.clone(),
        output_directory: fixture.outputs.join("missing-report"),
    })
    .unwrap_err();
    assert_eq!(error.failure.code, ComparisonErrorCode::InputMissing);

    value["captures"][0]["artifacts"]["heatmap"]["path"] = json!("inputs/heatmap.png");
    value["captures"][0]["artifacts"]["heatmap"]["sha256"] = json!("0".repeat(64));
    write_json(&bundle, &value);
    rebind_root(&fixture, &bundle);
    let error = build_comparison_report(&ReportBuildRequest {
        repository_root: fixture.root.clone(),
        allowed_input_roots: vec![fixture.root.clone()],
        allowed_output_root: fixture.outputs.clone(),
        bundle: bundle.clone(),
        output_directory: fixture.outputs.join("swapped-report"),
    })
    .unwrap_err();
    assert_eq!(error.failure.code, ComparisonErrorCode::ReportLinkInvalid);

    let linked = write_report_bundle(
        &fixture,
        "root-link-mismatch.json",
        &fixture.old_hash,
        None,
        false,
    );
    let root_path = fixture.inputs.join("manifest.json");
    let mut root: Value = serde_json::from_slice(&fs::read(&root_path).unwrap()).unwrap();
    root["analysis"]["sha256"] = json!("f".repeat(64));
    write_json(&root_path, &root);
    let error = build_comparison_report(&ReportBuildRequest {
        repository_root: fixture.root.clone(),
        allowed_input_roots: vec![fixture.root.clone()],
        allowed_output_root: fixture.outputs.clone(),
        bundle: linked,
        output_directory: fixture.outputs.join("root-link-mismatch-report"),
    })
    .unwrap_err();
    assert_eq!(error.failure.code, ComparisonErrorCode::ReportLinkInvalid);

    fixture.plan("changed-plan").unwrap();
    let plan_path = fixture
        .outputs
        .join("changed-plan/baseline-update-plan.json");
    let approval = fixture.write_approval(&plan_path, true, "changed-approval.json");
    apply_baseline_update(&BaselineApplyRequest {
        repository_root: fixture.root.clone(),
        plan: plan_path,
        approval,
        allowed_output_root: fixture.outputs.clone(),
        output_directory: fixture.outputs.join("changed-apply"),
    })
    .unwrap();
    let changed = write_report_bundle(&fixture, "changed.json", &fixture.new_hash, None, false);
    let error = build_comparison_report(&ReportBuildRequest {
        repository_root: fixture.root.clone(),
        allowed_input_roots: vec![fixture.root.clone()],
        allowed_output_root: fixture.outputs.clone(),
        bundle: changed,
        output_directory: fixture.outputs.join("unapproved-report"),
    })
    .unwrap_err();
    assert_eq!(
        error.failure.code,
        ComparisonErrorCode::BaselineApprovalRequired
    );
}

#[test]
fn report_rejects_reference_artifact_and_active_manifest_binding_forgery() {
    let fixture = Fixture::new();
    let forged_artifact = write_report_bundle(
        &fixture,
        "forged-reference-artifact.json",
        &fixture.old_hash,
        None,
        false,
    );
    let replacement = fs::read(fixture.inputs.join("candidate.png")).unwrap();
    fs::write(fixture.inputs.join("reference-phone.png"), &replacement).unwrap();
    let mut value: Value = serde_json::from_slice(&fs::read(&forged_artifact).unwrap()).unwrap();
    value["captures"][0]["artifacts"]["reference"]["sha256"] = json!(hash(&replacement));
    write_json(&forged_artifact, &value);
    rebind_root(&fixture, &forged_artifact);
    let error = build_comparison_report(&ReportBuildRequest {
        repository_root: fixture.root.clone(),
        allowed_input_roots: vec![fixture.root.clone()],
        allowed_output_root: fixture.outputs.clone(),
        bundle: forged_artifact,
        output_directory: fixture.outputs.join("forged-reference-artifact-report"),
    })
    .unwrap_err();
    assert_eq!(error.failure.code, ComparisonErrorCode::BaselineConflict);

    let unknown_reference = write_report_bundle(
        &fixture,
        "unknown-reference.json",
        &fixture.old_hash,
        None,
        false,
    );
    let mut value: Value = serde_json::from_slice(&fs::read(&unknown_reference).unwrap()).unwrap();
    value["captures"][0]["baseline_guard"]["reference_id"] = json!("unknown_reference");
    write_json(&unknown_reference, &value);
    rebind_root(&fixture, &unknown_reference);
    let error = build_comparison_report(&ReportBuildRequest {
        repository_root: fixture.root.clone(),
        allowed_input_roots: vec![fixture.root.clone()],
        allowed_output_root: fixture.outputs.clone(),
        bundle: unknown_reference,
        output_directory: fixture.outputs.join("unknown-reference-report"),
    })
    .unwrap_err();
    assert_eq!(error.failure.code, ComparisonErrorCode::BaselineConflict);

    let active_binding = write_report_bundle(
        &fixture,
        "active-binding-mismatch.json",
        &fixture.old_hash,
        None,
        false,
    );
    let mut value: Value = serde_json::from_slice(&fs::read(&active_binding).unwrap()).unwrap();
    value["captures"][0]["reference_binding"] = json!({"sha256":fixture.new_hash,"revision":2});
    value["captures"][0]["baseline_guard"]["observed"] =
        json!({"sha256":fixture.new_hash,"revision":2});
    write_json(&active_binding, &value);
    rebind_root(&fixture, &active_binding);
    let error = build_comparison_report(&ReportBuildRequest {
        repository_root: fixture.root.clone(),
        allowed_input_roots: vec![fixture.root.clone()],
        allowed_output_root: fixture.outputs.clone(),
        bundle: active_binding,
        output_directory: fixture.outputs.join("active-binding-mismatch-report"),
    })
    .unwrap_err();
    assert_eq!(error.failure.code, ComparisonErrorCode::BaselineConflict);
}

#[test]
fn baseline_plan_requires_human_approval_and_rejects_stale_state() {
    let fixture = Fixture::new();
    let plan = fixture.plan("plan").unwrap();
    assert!(plan.human_approval_required);
    assert!(!plan.automatic_fix_may_apply);
    assert_eq!(plan.rerun_requirements.len(), 2);
    assert_eq!(plan.new_binding.sha256, fixture.new_hash);
    let error = fixture.plan("plan").unwrap_err();
    assert_eq!(error.failure.code, ComparisonErrorCode::BaselinePlanInvalid);
    let plan_path = fixture.outputs.join("plan/baseline-update-plan.json");
    let denied = fixture.write_approval(&plan_path, false, "denied.json");
    let error = apply_baseline_update(&BaselineApplyRequest {
        repository_root: fixture.root.clone(),
        plan: plan_path.clone(),
        approval: denied,
        allowed_output_root: fixture.outputs.clone(),
        output_directory: fixture.outputs.join("denied-apply"),
    })
    .unwrap_err();
    assert_eq!(
        error.failure.code,
        ComparisonErrorCode::BaselineApprovalRequired
    );
    assert_eq!(
        hash(
            &fs::read(
                fixture
                    .root
                    .join("tools/ui-visual-audit/fixtures/references/gallery/phone.png")
            )
            .unwrap()
        ),
        fixture.old_hash
    );

    let approved = fixture.write_approval(&plan_path, true, "approved.json");
    let mut manifest: Value =
        serde_json::from_slice(&fs::read(&fixture.manifest).unwrap()).unwrap();
    manifest["references"][0]["allowed_differences"]["notes"] = json!(["concurrent edit"]);
    write_json(&fixture.manifest, &manifest);
    let error = apply_baseline_update(&BaselineApplyRequest {
        repository_root: fixture.root.clone(),
        plan: plan_path,
        approval: approved,
        allowed_output_root: fixture.outputs.clone(),
        output_directory: fixture.outputs.join("stale-apply"),
    })
    .unwrap_err();
    assert_eq!(error.failure.code, ComparisonErrorCode::BaselineConflict);
    assert_eq!(
        hash(
            &fs::read(
                fixture
                    .root
                    .join("tools/ui-visual-audit/fixtures/references/gallery/phone.png")
            )
            .unwrap()
        ),
        fixture.old_hash
    );
}

#[test]
fn approved_apply_stays_incomplete_until_every_related_capture_is_verified() {
    let fixture = Fixture::new();
    fixture.plan("plan").unwrap();
    let plan_path = fixture.outputs.join("plan/baseline-update-plan.json");
    let approval = fixture.write_approval(&plan_path, true, "approved.json");
    let receipt = apply_baseline_update(&BaselineApplyRequest {
        repository_root: fixture.root.clone(),
        plan: plan_path,
        approval,
        allowed_output_root: fixture.outputs.clone(),
        output_directory: fixture.outputs.join("apply"),
    })
    .unwrap();
    assert_eq!(receipt.status, "applied_rerun_required");
    assert!(!receipt.acceptance_complete);
    assert!(receipt.rerun_verification_required);
    assert_eq!(receipt.rerun_requirements.len(), 2);

    let receipt_path = fixture.outputs.join("apply/baseline-update-receipt.json");
    let incomplete_bundle = write_report_bundle(
        &fixture,
        "incomplete.json",
        &fixture.new_hash,
        Some(&receipt_path),
        false,
    );
    let incomplete_result = build_comparison_report(&ReportBuildRequest {
        repository_root: fixture.root.clone(),
        allowed_input_roots: vec![fixture.root.clone()],
        allowed_output_root: fixture.outputs.clone(),
        bundle: incomplete_bundle,
        output_directory: fixture.outputs.join("incomplete-report"),
    })
    .unwrap();
    assert_eq!(incomplete_result.captures.len(), 1);
    let error = verify_baseline_rerun(&BaselineRerunVerificationRequest {
        repository_root: fixture.root.clone(),
        receipt: receipt_path.clone(),
        comparison_result: fixture
            .outputs
            .join("incomplete-report/comparison-result.json"),
        allowed_output_root: fixture.outputs.clone(),
        output_directory: fixture.outputs.join("incomplete-verification"),
    })
    .unwrap_err();
    assert_eq!(
        error.failure.code,
        ComparisonErrorCode::BaselineRerunIncomplete
    );

    let identity_substitution = write_report_bundle(
        &fixture,
        "identity-substitution.json",
        &fixture.new_hash,
        Some(&receipt_path),
        true,
    );
    let mut substitution: Value =
        serde_json::from_slice(&fs::read(&identity_substitution).unwrap()).unwrap();
    substitution["captures"][1]["baseline_guard"]["reference_id"] = json!("gallery_tablet_alias");
    write_json(&identity_substitution, &substitution);
    rebind_root(&fixture, &identity_substitution);
    build_comparison_report(&ReportBuildRequest {
        repository_root: fixture.root.clone(),
        allowed_input_roots: vec![fixture.root.clone()],
        allowed_output_root: fixture.outputs.clone(),
        bundle: identity_substitution,
        output_directory: fixture.outputs.join("identity-substitution-report"),
    })
    .unwrap();
    let error = verify_baseline_rerun(&BaselineRerunVerificationRequest {
        repository_root: fixture.root.clone(),
        receipt: receipt_path.clone(),
        comparison_result: fixture
            .outputs
            .join("identity-substitution-report/comparison-result.json"),
        allowed_output_root: fixture.outputs.clone(),
        output_directory: fixture.outputs.join("identity-substitution-verification"),
    })
    .unwrap_err();
    assert_eq!(
        error.failure.code,
        ComparisonErrorCode::BaselineRerunIncomplete
    );

    let missing_root_binding = write_report_bundle(
        &fixture,
        "missing-root-comparison-binding.json",
        &fixture.new_hash,
        Some(&receipt_path),
        true,
    );
    build_comparison_report(&ReportBuildRequest {
        repository_root: fixture.root.clone(),
        allowed_input_roots: vec![fixture.root.clone()],
        allowed_output_root: fixture.outputs.clone(),
        bundle: missing_root_binding,
        output_directory: fixture
            .outputs
            .join("missing-root-comparison-binding-report"),
    })
    .unwrap();
    let root_path = fixture.inputs.join("manifest.json");
    let mut root: Value = serde_json::from_slice(&fs::read(&root_path).unwrap()).unwrap();
    root.as_object_mut().unwrap().remove("comparison");
    write_json(&root_path, &root);
    let root_bytes = fs::read(&root_path).unwrap();
    let missing_root_result = fixture
        .outputs
        .join("missing-root-comparison-binding-report/comparison-result.json");
    let mut result: Value =
        serde_json::from_slice(&fs::read(&missing_root_result).unwrap()).unwrap();
    result["root"]["root_manifest"]["sha256"] = json!(hash(&root_bytes));
    result["root"]["root_manifest"]["byte_length"] = json!(root_bytes.len());
    write_json(&missing_root_result, &result);
    let error = verify_baseline_rerun(&BaselineRerunVerificationRequest {
        repository_root: fixture.root.clone(),
        receipt: receipt_path.clone(),
        comparison_result: missing_root_result,
        allowed_output_root: fixture.outputs.clone(),
        output_directory: fixture
            .outputs
            .join("missing-root-comparison-binding-verification"),
    })
    .unwrap_err();
    assert_eq!(
        error.failure.code,
        ComparisonErrorCode::BaselineRerunIncomplete
    );

    let replaced_input_identity = write_report_bundle(
        &fixture,
        "replaced-input-identity.json",
        &fixture.new_hash,
        Some(&receipt_path),
        true,
    );
    build_comparison_report(&ReportBuildRequest {
        repository_root: fixture.root.clone(),
        allowed_input_roots: vec![fixture.root.clone()],
        allowed_output_root: fixture.outputs.clone(),
        bundle: replaced_input_identity,
        output_directory: fixture.outputs.join("replaced-input-identity-report"),
    })
    .unwrap();
    let replaced_input_result = fixture
        .outputs
        .join("replaced-input-identity-report/comparison-result.json");
    let mut result: Value =
        serde_json::from_slice(&fs::read(&replaced_input_result).unwrap()).unwrap();
    result["root"]["comparison_input"]["sha256"] = json!("0".repeat(64));
    write_json(&replaced_input_result, &result);
    let error = verify_baseline_rerun(&BaselineRerunVerificationRequest {
        repository_root: fixture.root.clone(),
        receipt: receipt_path.clone(),
        comparison_result: replaced_input_result,
        allowed_output_root: fixture.outputs.clone(),
        output_directory: fixture.outputs.join("replaced-input-identity-verification"),
    })
    .unwrap_err();
    assert_eq!(
        error.failure.code,
        ComparisonErrorCode::BaselineRerunIncomplete
    );

    let full_bundle = write_report_bundle(
        &fixture,
        "full.json",
        &fixture.new_hash,
        Some(&receipt_path),
        true,
    );
    build_comparison_report(&ReportBuildRequest {
        repository_root: fixture.root.clone(),
        allowed_input_roots: vec![fixture.root.clone()],
        allowed_output_root: fixture.outputs.clone(),
        bundle: full_bundle,
        output_directory: fixture.outputs.join("full-report"),
    })
    .unwrap();
    let verification = verify_baseline_rerun(&BaselineRerunVerificationRequest {
        repository_root: fixture.root.clone(),
        receipt: receipt_path,
        comparison_result: fixture.outputs.join("full-report/comparison-result.json"),
        allowed_output_root: fixture.outputs.clone(),
        output_directory: fixture.outputs.join("verification"),
    })
    .unwrap();
    assert!(verification.acceptance_complete);
    assert_eq!(verification.verified_capture_ids.len(), 2);
}

#[test]
fn report_and_baseline_plan_cli_emit_machine_readable_contracts() {
    let fixture = Fixture::new();
    let bundle = write_report_bundle(&fixture, "cli-report.json", &fixture.old_hash, None, false);
    let output = Command::new(env!("CARGO_BIN_EXE_ui-visual-audit"))
        .args([
            "build-report",
            "--repository-root",
            fixture.root.to_str().unwrap(),
            "--allowed-input-root",
            fixture.root.to_str().unwrap(),
            "--allowed-output-root",
            fixture.outputs.to_str().unwrap(),
            "--bundle",
            bundle.to_str().unwrap(),
            "--output-directory",
            fixture.outputs.join("cli-report-output").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["schema_version"], 1);
    assert_eq!(result["status"], "passed");

    let failure = Command::new(env!("CARGO_BIN_EXE_ui-visual-audit"))
        .args([
            "build-report",
            "--repository-root",
            fixture.root.to_str().unwrap(),
            "--allowed-input-root",
            fixture.root.to_str().unwrap(),
            "--allowed-output-root",
            fixture.outputs.to_str().unwrap(),
            "--bundle",
            bundle.to_str().unwrap(),
            "--output-directory",
            fixture.outputs.join("cli-report-output").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!failure.status.success());
    let failure_result: Value = serde_json::from_slice(&failure.stderr).unwrap();
    assert_eq!(
        failure_result["failure"]["code"],
        "output_directory_not_empty"
    );

    let output = Command::new(env!("CARGO_BIN_EXE_ui-visual-audit"))
        .args([
            "plan-baseline-update",
            "--repository-root",
            fixture.root.to_str().unwrap(),
            "--manifest",
            fixture.manifest.to_str().unwrap(),
            "--reference-id",
            "gallery_phone",
            "--new-image",
            fixture.inputs.join("candidate.png").to_str().unwrap(),
            "--reason",
            "CLI fixture approval request",
            "--metrics-before",
            fixture.inputs.join("before.json").to_str().unwrap(),
            "--metrics-after",
            fixture.inputs.join("after.json").to_str().unwrap(),
            "--allowed-output-root",
            fixture.outputs.to_str().unwrap(),
            "--output-directory",
            fixture.outputs.join("cli-plan").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let plan: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(plan["human_approval_required"], true);
    assert_eq!(plan["automatic_fix_may_apply"], false);
}

#[test]
fn local_success_and_failure_report_demo_cleans_all_temporary_artifacts() {
    let root;
    {
        let fixture = Fixture::new();
        root = fixture.root.clone();
        let success = write_report_bundle(
            &fixture,
            "success-demo.json",
            &fixture.old_hash,
            None,
            false,
        );
        let success_result = build_comparison_report(&ReportBuildRequest {
            repository_root: fixture.root.clone(),
            allowed_input_roots: vec![fixture.root.clone()],
            allowed_output_root: fixture.outputs.clone(),
            bundle: success,
            output_directory: fixture.outputs.join("success-demo"),
        })
        .unwrap();
        assert_eq!(success_result.status, "passed");

        let failure = write_report_bundle(
            &fixture,
            "failure-demo.json",
            &fixture.old_hash,
            None,
            false,
        );
        let mut value: Value = serde_json::from_slice(&fs::read(&failure).unwrap()).unwrap();
        value["captures"][0]["gate_state"] = json!("failed");
        write_json(&failure, &value);
        rebind_root(&fixture, &failure);
        let failure_result = build_comparison_report(&ReportBuildRequest {
            repository_root: fixture.root.clone(),
            allowed_input_roots: vec![fixture.root.clone()],
            allowed_output_root: fixture.outputs.clone(),
            bundle: failure,
            output_directory: fixture.outputs.join("failure-demo"),
        })
        .unwrap();
        assert_eq!(failure_result.status, "failed");
        let report = fs::read_to_string(fixture.outputs.join("failure-demo/report.md")).unwrap();
        assert!(report.contains("- Status: `failed`"));
    }
    assert!(
        !root.exists(),
        "TempDir drop must remove success/failure demo artifacts"
    );
}

fn write_report_bundle(
    fixture: &Fixture,
    name: &str,
    observed_hash: &str,
    receipt: Option<&Path>,
    include_tablet: bool,
) -> PathBuf {
    for (name, bytes) in [
        (
            "reference-phone.png",
            fs::read(
                fixture
                    .root
                    .join("tools/ui-visual-audit/fixtures/references/gallery/phone.png"),
            )
            .unwrap(),
        ),
        (
            "reference-tablet.png",
            fs::read(
                fixture
                    .root
                    .join("tools/ui-visual-audit/fixtures/references/gallery/tablet.png"),
            )
            .unwrap(),
        ),
        ("actual.png", png_bytes([22, 42, 62, 255])),
        ("overlay.png", png_bytes([21, 41, 61, 255])),
        ("heatmap.png", png_bytes([255, 0, 0, 255])),
    ] {
        fs::write(fixture.inputs.join(name), bytes).unwrap();
    }
    write_json(
        &fixture.inputs.join("analysis.json"),
        &json!({
            "schema_version":1,
            "artifact_backlink":{
                "schema_version":1,
                "root_run_id":"fixture-run",
                "root_manifest":"inputs/manifest.json",
                "capture_ids":if include_tablet {
                    vec!["gallery.phone-small.default", "gallery.tablet-portrait.default"]
                } else {
                    vec!["gallery.phone-small.default"]
                }
            }
        }),
    );
    write_json(
        &fixture.inputs.join("fix-manifest.json"),
        &json!({
            "schema_version":1,
            "artifact_backlink":{
                "schema_version":1,
                "root_run_id":"fixture-run",
                "root_manifest":"inputs/manifest.json",
                "capture_ids":if include_tablet {
                    vec!["gallery.phone-small.default", "gallery.tablet-portrait.default"]
                } else {
                    vec!["gallery.phone-small.default"]
                }
            }
        }),
    );
    let link = |name: &str| {
        let path = fixture.inputs.join(name);
        json!({"path":relative(&fixture.root, &path),"sha256":hash(&fs::read(path).unwrap())})
    };
    let mut captures = vec![capture_json(
        fixture,
        "phone-small",
        "gallery_phone",
        observed_hash,
        &fixture.old_hash,
        receipt,
        &link,
    )];
    if include_tablet {
        let manifest: Value =
            serde_json::from_slice(&fs::read(&fixture.manifest).unwrap()).unwrap();
        let tablet_hash = manifest["references"][1]["image"]["sha256"]
            .as_str()
            .unwrap();
        captures.push(capture_json(
            fixture,
            "tablet-portrait",
            "gallery_tablet",
            tablet_hash,
            tablet_hash,
            None,
            &link,
        ));
    }
    let bundle = fixture.inputs.join(name);
    write_json(
        &bundle,
        &json!({
            "schema_version":1,
            "algorithm_version":"ui_comparison_bundle_v1",
            "run_id":"fixture-run",
            "root_manifest":{"path":"inputs/manifest.json"},
            "analysis":link("analysis.json"),
        "fix_iterations":[{"iteration":1,"manifest":link("fix-manifest.json"),"analysis":null,"report":null}],
            "captures":captures
        }),
    );
    rebind_root(fixture, &bundle);
    bundle
}

fn capture_json(
    fixture: &Fixture,
    device: &str,
    reference_id: &str,
    observed_hash: &str,
    expected_hash: &str,
    receipt: Option<&Path>,
    link: &impl Fn(&str) -> Value,
) -> Value {
    let reference_artifact = match device {
        "phone-small" => "reference-phone.png",
        "tablet-portrait" => "reference-tablet.png",
        _ => panic!("fixture does not define a reference artifact for {device}"),
    };
    let approval_receipt = receipt.map(|path| {
        json!({
            "path":relative(&fixture.root, path),
            "sha256":hash(&fs::read(path).unwrap())
        })
    });
    json!({
        "capture_id":format!("gallery.{device}.default"),
        "screen":"gallery","device":device,"state":"default",
        "reference_binding":{"sha256":observed_hash,"revision":if observed_hash == fixture.new_hash {2} else {1}},
        "artifacts":{"reference":link(reference_artifact),"actual":link("actual.png"),"overlay":link("overlay.png"),"heatmap":link("heatmap.png")},
        "metrics":{"raw_changed_ratio_millionths":100,"alpha_changed_ratio_millionths":0,"tolerated_changed_ratio_millionths":50,"ssim_millionths":999000,"geometry_changed_ratio_millionths":0,"large_area_ratio_millionths":0},
        "regions":[{"region_id":"content","level":"normal","bounds":{"x":0,"y":0,"width":2,"height":2},"status":"passed","metrics":{"raw_changed_ratio_millionths":100,"alpha_changed_ratio_millionths":0,"tolerated_changed_ratio_millionths":50,"ssim_millionths":999000,"geometry_changed_ratio_millionths":0,"large_area_ratio_millionths":0},"threshold":{"profile":"strict","values":{"max_raw":1000}}}],
        "masks":[{"mask_id":"clock","reason":"dynamic clock","bounds":{"x":0,"y":0,"width":1,"height":1},"artifact":null}],
        "allowed_differences":{"profile":"strict","notes":["clock is masked"]},
        "algorithms":{"diff":"ui_diff_metrics_v1","semantic":"ui_semantic_audit_v1","gate":"ui_visual_gate_v1"},
        "thresholds":[{"profile":"strict","values":{"max_raw":1000,"minimum_ssim":990000}}],
        "ai":{"ran":false,"provider_id":null,"model_id":null,"issue_count":0},
        "gate_state":"passed",
        "issues":[{"issue_id":"semantic-1","source":"semantic","region_id":"content","severity":"minor","message":"fixture issue","evidence":{"image_role":"overlay","rect":{"x":0,"y":0,"width":1,"height":1},"description":"visible at title"},"node_id":"title","source_path":"project/src/game/screens/page.rs","likely_files":["project/src/game/screens/page.rs"],"likely_cause":"spacing token","suggested_change_scope":"page-local title spacing"}],
        "baseline_guard":{"reference_id":reference_id,"reference_manifest":{"path":relative(&fixture.root, &fixture.manifest),"sha256":hash(&fs::read(&fixture.manifest).unwrap())},"expected":{"sha256":expected_hash,"revision":1},"observed":{"sha256":observed_hash,"revision":if observed_hash == fixture.new_hash {2} else {1}},"approval_receipt":approval_receipt}
    })
}

fn rebind_root(fixture: &Fixture, bundle: &Path) {
    let bundle_value: Value = serde_json::from_slice(&fs::read(bundle).unwrap()).unwrap();
    write_json(
        &fixture.inputs.join("manifest.json"),
        &json!({
            "run_id":"fixture-run",
            "comparison":{"input":relative(&fixture.root, bundle),"input_sha256":hash(&fs::read(bundle).unwrap())},
            "analysis":{"path":"inputs/analysis.json","sha256":hash(&fs::read(fixture.inputs.join("analysis.json")).unwrap())},
            "fix_iterations":bundle_value["fix_iterations"]
        }),
    );
}

fn png_bytes(color: [u8; 4]) -> Vec<u8> {
    let mut bytes = Vec::new();
    let pixels = color.repeat(4);
    PngEncoder::new(&mut bytes)
        .write_image(&pixels, 2, 2, ExtendedColorType::Rgba8)
        .unwrap();
    bytes
}

fn write_json(path: &Path, value: &Value) {
    let mut bytes = serde_json::to_vec_pretty(value).unwrap();
    bytes.push(b'\n');
    fs::write(path, bytes).unwrap();
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/")
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
