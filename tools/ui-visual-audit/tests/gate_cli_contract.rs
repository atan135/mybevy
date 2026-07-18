use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf, process::Command};
use tempfile::TempDir;
use ui_visual_audit::{
    AiAnalysisRequest, AiSeverity, ComparisonExitCode, DiffAnalysisReport, DiffAnalysisRequest,
    GateState, NormalizationRequest, RegionAuditReport, RegionAuditRequest, SemanticAuditReport,
    SemanticAuditRequest, VISUAL_GATE_REPORT_FILENAME, VisualGateReport, analyze_aligned_diff,
    analyze_with_ai, audit_regions, audit_semantics, normalize_and_align,
};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .unwrap()
        .to_path_buf()
}

struct PreparedGateRun {
    _workspace_temp: TempDir,
    root: PathBuf,
    bundle: PathBuf,
    critical_bundle: PathBuf,
    semantic_failed_bundle: PathBuf,
    pass_config: PathBuf,
    review_config: PathBuf,
}

fn prepare_gate_run() -> PreparedGateRun {
    let workspace_temp = TempDir::new_in(workspace_root()).unwrap();
    let root = workspace_temp.path().to_path_buf();
    let inputs = root.join("inputs");
    fs::create_dir_all(&inputs).unwrap();
    let width = 20;
    let height = 20;
    let mut reference = [230, 230, 230, 255].repeat((width * height) as usize);
    let mut actual = reference.clone();
    fill_rect(&mut reference, width, (8, 4, 4, 5), [20, 60, 120, 255]);
    fill_rect(&mut actual, width, (8, 4, 4, 5), [20, 60, 120, 255]);
    set_pixel(&mut actual, width, 6, 6, [220, 220, 220, 255]);
    let reference_path = write_png(&inputs, "reference.png", width, height, &reference);
    let actual_path = write_png(&inputs, "actual.png", width, height, &actual);
    let normalization_manifest = write_bytes(
        &inputs,
        "normalize.json",
        br#"{
          "schema_version":1,
          "algorithm_version":"normalize_align_v1",
          "orientation_policy":"apply_exif",
          "color_policy":"srgb_only",
          "alpha_policy":"straight_zero_transparent_rgb",
          "reference":{"crop":{"kind":"none"}},
          "actual":{"crop":{"kind":"none"}},
          "alignment":{"mode":"none","maximum_translation":{"x":0,"y":0}}
        }"#,
    );
    let normalized_dir = root.join("normalized");
    let normalized = normalize_and_align(&NormalizationRequest {
        repository_root: workspace_root(),
        allowed_input_roots: vec![inputs.clone()],
        allowed_output_root: root.clone(),
        reference: reference_path,
        actual: actual_path,
        normalization_manifest,
        output_directory: normalized_dir.clone(),
    })
    .unwrap();
    assert_eq!(normalized.exit_code, ComparisonExitCode::Success);

    let diff_config = write_bytes(
        &inputs,
        "metrics.json",
        include_bytes!("../fixtures/comparison/ui-diff-metrics-v1.config.json"),
    );
    let diff_dir = root.join("diff");
    let diff = analyze_aligned_diff(&DiffAnalysisRequest {
        repository_root: workspace_root(),
        allowed_input_roots: vec![normalized_dir.clone(), inputs.clone()],
        allowed_output_root: root.clone(),
        reference: normalized_dir.join("aligned-reference.png"),
        actual: normalized_dir.join("aligned-actual.png"),
        config: diff_config.clone(),
        output_directory: diff_dir.clone(),
    })
    .unwrap();
    assert_eq!(diff.exit_code, ComparisonExitCode::Success);

    let mut region_config: Value = serde_json::from_slice(include_bytes!(
        "../fixtures/comparison/ui-region-audit-v1.config.json"
    ))
    .unwrap();
    region_config["reference_binding"]["sha256"] =
        json!(normalized.report.reference.sha256.clone());
    region_config["audit_scope"] = json!("full_image");
    region_config["ignore_regions"] = json!([]);
    region_config["bounds_sources"] = json!([]);
    region_config["regions"] = json!([{
        "region_id": "frame",
        "label": "Full decorative capture",
        "semantic_role": "decoration",
        "level": "decorative",
        "clipping": "reject_out_of_bounds",
        "source": {"kind": "manual", "coordinate_space": "aligned", "shape": {
            "kind": "rectangle", "bounds": {"x": 0, "y": 0, "width": width, "height": height}
        }}
    }]);
    let region_config_path = write_bytes(
        &inputs,
        "regions.json",
        &serde_json::to_vec_pretty(&region_config).unwrap(),
    );
    let region_dir = root.join("regions");
    let region = audit_regions(&RegionAuditRequest {
        repository_root: workspace_root(),
        allowed_input_roots: vec![normalized_dir.clone(), inputs.clone(), root.clone()],
        allowed_output_root: root.clone(),
        reference: normalized_dir.join("aligned-reference.png"),
        actual: normalized_dir.join("aligned-actual.png"),
        diff_config: diff_config.clone(),
        region_config: region_config_path,
        normalization_report: normalized_dir.join("normalization-report.json"),
        output_directory: region_dir.clone(),
    })
    .unwrap();
    assert_eq!(region.exit_code, ComparisonExitCode::Success);
    assert!(
        region.report.region_results[0]
            .threshold_violations
            .is_empty()
    );

    let mut critical_region_config = region_config.clone();
    critical_region_config["regions"][0]["region_id"] = json!("primary_action");
    critical_region_config["regions"][0]["label"] = json!("Primary action");
    critical_region_config["regions"][0]["semantic_role"] = json!("key_button");
    critical_region_config["regions"][0]["level"] = json!("critical");
    let critical_region_config_path = write_bytes(
        &inputs,
        "critical-regions.json",
        &serde_json::to_vec_pretty(&critical_region_config).unwrap(),
    );
    let critical_region_dir = root.join("critical-regions");
    let critical_region = audit_regions(&RegionAuditRequest {
        repository_root: workspace_root(),
        allowed_input_roots: vec![normalized_dir.clone(), inputs.clone(), root.clone()],
        allowed_output_root: root.clone(),
        reference: normalized_dir.join("aligned-reference.png"),
        actual: normalized_dir.join("aligned-actual.png"),
        diff_config,
        region_config: critical_region_config_path,
        normalization_report: normalized_dir.join("normalization-report.json"),
        output_directory: critical_region_dir.clone(),
    })
    .unwrap();
    assert_eq!(critical_region.exit_code, ComparisonExitCode::Success);
    assert_eq!(
        critical_region.report.region_results[0].local_status,
        ui_visual_audit::RegionLocalStatus::Failed
    );

    let semantic_dir = root.join("semantic");
    let semantic = audit_semantics(&SemanticAuditRequest {
        repository_root: workspace_root(),
        allowed_input_roots: vec![
            PathBuf::from("tools/ui-visual-audit/fixtures/semantic"),
            root.clone(),
        ],
        allowed_output_root: root.clone(),
        metadata: PathBuf::from(
            "tools/ui-visual-audit/fixtures/semantic/compact-pass.metadata.json",
        ),
        config: PathBuf::from(
            "tools/ui-visual-audit/fixtures/semantic/ui-semantic-audit-v1.config.json",
        ),
        output_directory: semantic_dir.clone(),
    })
    .unwrap();
    assert_eq!(semantic.exit_code, ComparisonExitCode::Success);

    let mut failed_metadata: Value = serde_json::from_slice(include_bytes!(
        "../fixtures/semantic/compact-pass.metadata.json"
    ))
    .unwrap();
    let button = failed_metadata["semantic_tree"]["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|node| node["role"] == "button")
        .unwrap();
    let min_x = button["bounds"]["min_x"].as_f64().unwrap();
    let min_y = button["bounds"]["min_y"].as_f64().unwrap();
    button["bounds"]["max_x"] = json!(min_x + 4.0);
    button["bounds"]["max_y"] = json!(min_y + 4.0);
    let failed_metadata_path = root.join("semantic-failed.metadata.json");
    fs::write(
        &failed_metadata_path,
        serde_json::to_vec_pretty(&failed_metadata).unwrap(),
    )
    .unwrap();
    let semantic_failed_dir = root.join("semantic-failed");
    let semantic_failed = audit_semantics(&SemanticAuditRequest {
        repository_root: workspace_root(),
        allowed_input_roots: vec![
            PathBuf::from("tools/ui-visual-audit/fixtures/semantic"),
            root.clone(),
        ],
        allowed_output_root: root.clone(),
        metadata: failed_metadata_path,
        config: PathBuf::from(
            "tools/ui-visual-audit/fixtures/semantic/ui-semantic-audit-v1.config.json",
        ),
        output_directory: semantic_failed_dir.clone(),
    })
    .unwrap();
    assert_eq!(
        semantic_failed.exit_code,
        ComparisonExitCode::ThresholdFailure
    );
    assert!(!semantic_failed.report.findings.is_empty());

    let bundle = root.join("gate.bundle.json");
    let bundle_value = json!({
        "schema_version": 1,
        "run_id": "gate-cli-fixture",
        "captures": [{
            "capture_id": "gallery.compact.initial",
            "screen": "gallery",
            "device": "compact",
            "state": "initial",
            "reference_profile": "fixture-profile",
            "reference_binding": region.report.reference_binding,
            "diff_report": bound_report(diff_dir.join("diff-metrics-report.json")),
            "region_report": bound_report(region_dir.join("region-audit-report.json")),
            "semantic_report": bound_report(semantic_dir.join("semantic-audit-report.json"))
        }],
        "ai_report": null
    });
    fs::write(&bundle, serde_json::to_vec_pretty(&bundle_value).unwrap()).unwrap();
    let critical_bundle = root.join("gate-critical.bundle.json");
    let mut critical_bundle_value = bundle_value.clone();
    critical_bundle_value["captures"][0]["region_report"] =
        bound_report(critical_region_dir.join("region-audit-report.json"));
    fs::write(
        &critical_bundle,
        serde_json::to_vec_pretty(&critical_bundle_value).unwrap(),
    )
    .unwrap();
    let semantic_failed_bundle = root.join("gate-semantic-failed.bundle.json");
    let mut semantic_failed_bundle_value = bundle_value;
    semantic_failed_bundle_value["captures"][0]["semantic_report"] =
        bound_report(semantic_failed_dir.join("semantic-audit-report.json"));
    fs::write(
        &semantic_failed_bundle,
        serde_json::to_vec_pretty(&semantic_failed_bundle_value).unwrap(),
    )
    .unwrap();
    let pass_config = root.join("gate-pass.config.json");
    fs::write(
        &pass_config,
        serde_json::to_vec_pretty(&gate_config(
            1_000_000,
            -1_000_000,
            &region.report.reference_binding,
        ))
        .unwrap(),
    )
    .unwrap();
    let review_config = root.join("gate-review.config.json");
    fs::write(
        &review_config,
        serde_json::to_vec_pretty(&gate_config(0, 1_000_000, &region.report.reference_binding))
            .unwrap(),
    )
    .unwrap();
    PreparedGateRun {
        _workspace_temp: workspace_temp,
        root,
        bundle,
        critical_bundle,
        semantic_failed_bundle,
        pass_config,
        review_config,
    }
}

#[test]
fn cli_pass_is_deterministic_and_output_is_no_clobber() {
    let run = prepare_gate_run();
    let output_a = run.root.join("gate-output-a");
    let output_b = run.root.join("gate-output-b");
    let first = run_cli(&run, &run.pass_config, &run.bundle, &output_a);
    assert_eq!(
        first.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let second = run_cli(&run, &run.pass_config, &run.bundle, &output_b);
    assert_eq!(
        second.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    let first_bytes = fs::read(output_a.join(VISUAL_GATE_REPORT_FILENAME)).unwrap();
    let second_bytes = fs::read(output_b.join(VISUAL_GATE_REPORT_FILENAME)).unwrap();
    assert_eq!(first_bytes, second_bytes);
    let report: VisualGateReport = serde_json::from_slice(&first_bytes).unwrap();
    assert_eq!(report.status, GateState::Passed);
    assert!(!report.summary.global_numeric_score_emitted);
    assert!(report.summary.failed_regions_remain_individually_visible);

    let before = first_bytes;
    let no_clobber = run_cli(&run, &run.pass_config, &run.bundle, &output_a);
    assert_eq!(no_clobber.status.code(), Some(2));
    assert_eq!(
        fs::read(output_a.join(VISUAL_GATE_REPORT_FILENAME)).unwrap(),
        before
    );
}

#[test]
fn decorative_profile_failure_returns_needs_review_and_preserves_region() {
    let run = prepare_gate_run();
    let output = run.root.join("gate-review-output");
    let result = run_cli(&run, &run.review_config, &run.bundle, &output);
    assert_eq!(
        result.status.code(),
        Some(3),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: VisualGateReport =
        serde_json::from_slice(&fs::read(output.join(VISUAL_GATE_REPORT_FILENAME)).unwrap())
            .unwrap();
    assert_eq!(report.status, GateState::NeedsReview);
    let regions = report.captures[0].regions.as_ref().unwrap();
    assert_eq!(
        regions.results[0].upstream_local_status,
        ui_visual_audit::RegionLocalStatus::Passed
    );
    assert_eq!(regions.decorative.failed, 1);
    assert!(!regions.averaging_used_for_gate);
    assert_eq!(regions.results[0].gate_state, GateState::NeedsReview);
    assert!(!regions.results[0].profile_threshold_violations.is_empty());
}

#[test]
fn selected_profile_can_relax_upstream_thresholds_but_critical_still_cannot_average_away() {
    let run = prepare_gate_run();
    let relaxed_output = run.root.join("gate-relaxed-upstream-output");
    let relaxed = run_cli(
        &run,
        &run.pass_config,
        &run.critical_bundle,
        &relaxed_output,
    );
    assert_eq!(
        relaxed.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&relaxed.stderr)
    );
    let relaxed_report: VisualGateReport = serde_json::from_slice(
        &fs::read(relaxed_output.join(VISUAL_GATE_REPORT_FILENAME)).unwrap(),
    )
    .unwrap();
    let relaxed_region = &relaxed_report.captures[0].regions.as_ref().unwrap().results[0];
    assert!(
        relaxed_report.captures[0]
            .threshold_source
            .starts_with("reference_profile:fixture-profile")
    );
    assert_eq!(
        relaxed_region.upstream_local_status,
        ui_visual_audit::RegionLocalStatus::Failed
    );
    assert!(!relaxed_region.upstream_threshold_violations.is_empty());
    assert!(relaxed_region.profile_threshold_violations.is_empty());
    assert_eq!(relaxed_region.gate_state, GateState::Passed);

    let strict_output = run.root.join("gate-strict-critical-output");
    let strict = run_cli(
        &run,
        &run.review_config,
        &run.critical_bundle,
        &strict_output,
    );
    assert_eq!(strict.status.code(), Some(4));
    let strict_report: VisualGateReport =
        serde_json::from_slice(&fs::read(strict_output.join(VISUAL_GATE_REPORT_FILENAME)).unwrap())
            .unwrap();
    assert_eq!(strict_report.status, GateState::Failed);
    assert_eq!(
        strict_report.primary_failure_type,
        ui_visual_audit::GateFailureType::CriticalRegionFailure
    );
    assert!(
        !strict_report.captures[0]
            .regions
            .as_ref()
            .unwrap()
            .averaging_used_for_gate
    );
}

#[test]
fn bound_report_hash_mismatch_is_an_invalid_terminal_state() {
    let run = prepare_gate_run();
    let mut bundle: Value = serde_json::from_slice(&fs::read(&run.bundle).unwrap()).unwrap();
    bundle["captures"][0]["diff_report"]["sha256"] =
        json!("0000000000000000000000000000000000000000000000000000000000000000");
    let forged_bundle = run.root.join("gate-forged.bundle.json");
    fs::write(&forged_bundle, serde_json::to_vec_pretty(&bundle).unwrap()).unwrap();
    let output = run.root.join("gate-invalid-output");
    let result = run_cli(&run, &run.pass_config, &forged_bundle, &output);
    assert_eq!(
        result.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: VisualGateReport =
        serde_json::from_slice(&fs::read(output.join(VISUAL_GATE_REPORT_FILENAME)).unwrap())
            .unwrap();
    assert_eq!(report.status, GateState::Invalid);
    assert_eq!(
        report.primary_failure_type,
        ui_visual_audit::GateFailureType::InvalidEvidence
    );
    assert_eq!(report.validation_errors[0].code, "report_hash_mismatch");
    assert!(report.captures.is_empty());
}

#[test]
fn explicit_dimension_mismatch_is_a_failed_hard_gate() {
    let run = prepare_gate_run();
    let mut bundle: Value = serde_json::from_slice(&fs::read(&run.bundle).unwrap()).unwrap();
    let diff_path = PathBuf::from(
        bundle["captures"][0]["diff_report"]["path"]
            .as_str()
            .unwrap(),
    );
    let mut diff: Value = serde_json::from_slice(&fs::read(&diff_path).unwrap()).unwrap();
    diff["status"] = json!("comparison_failed");
    diff["dimensions"]["actual"]["width"] = json!(21);
    diff["metrics"] = Value::Null;
    diff["performance"] = Value::Null;
    diff["failure"] = json!({
        "failure_type": "comparison",
        "code": "dimensions_mismatch",
        "message": "fixture dimension mismatch"
    });
    fs::write(&diff_path, serde_json::to_vec_pretty(&diff).unwrap()).unwrap();
    bundle["captures"][0]["diff_report"] = bound_report(diff_path);
    bundle["captures"][0]["region_report"] = Value::Null;
    let mismatch_bundle = run.root.join("gate-dimension.bundle.json");
    fs::write(
        &mismatch_bundle,
        serde_json::to_vec_pretty(&bundle).unwrap(),
    )
    .unwrap();
    let output = run.root.join("gate-dimension-output");
    let result = run_cli(&run, &run.pass_config, &mismatch_bundle, &output);
    assert_eq!(
        result.status.code(),
        Some(4),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: VisualGateReport =
        serde_json::from_slice(&fs::read(output.join(VISUAL_GATE_REPORT_FILENAME)).unwrap())
            .unwrap();
    assert_eq!(report.status, GateState::Failed);
    assert_eq!(
        report.primary_failure_type,
        ui_visual_audit::GateFailureType::DimensionMismatch
    );
    assert!(report.captures[0].dimensions.hard_failure);
    assert!(report.captures[0].regions.is_none());
}

#[test]
fn actual_ai_reports_drive_severe_medium_and_minor_gate_states() {
    let run = prepare_gate_run();
    for (severity, expected_exit, expected_state, expected_failure) in [
        (
            AiSeverity::Severe,
            4,
            GateState::Failed,
            ui_visual_audit::GateFailureType::AiSevereIssue,
        ),
        (
            AiSeverity::Medium,
            4,
            GateState::Failed,
            ui_visual_audit::GateFailureType::AiMediumIssue,
        ),
        (
            AiSeverity::Minor,
            0,
            GateState::Passed,
            ui_visual_audit::GateFailureType::None,
        ),
    ] {
        let suffix = ai_severity_label(severity);
        let prepared = prepare_stage_eight_ai_bundle(
            &run,
            &run.bundle,
            severity,
            &format!("severity-{suffix}"),
        );
        let output = run.root.join(format!("gate-ai-{suffix}-output"));
        let result = run_cli(&run, &run.pass_config, &prepared.gate_bundle, &output);
        assert_eq!(
            result.status.code(),
            Some(expected_exit),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let report: VisualGateReport =
            serde_json::from_slice(&fs::read(output.join(VISUAL_GATE_REPORT_FILENAME)).unwrap())
                .unwrap();
        assert_eq!(report.status, expected_state);
        assert_eq!(report.primary_failure_type, expected_failure);
        assert_eq!(report.captures[0].ai.issues.len(), 1);
        assert_eq!(
            report.captures[0].ai.issues[0].blocking,
            severity != AiSeverity::Minor
        );
        if severity == AiSeverity::Minor {
            assert_eq!(report.captures[0].ai.minor_count, 1);
            assert!(report.captures[0].ai.minor_issues_are_report_only);
        }
    }
}

#[test]
fn ai_cannot_override_dimension_semantic_or_critical_priority() {
    let run = prepare_gate_run();

    let critical_ai = prepare_stage_eight_ai_bundle(
        &run,
        &run.critical_bundle,
        AiSeverity::Severe,
        "priority-critical",
    );
    let critical_output = run.root.join("gate-ai-priority-critical-output");
    let critical = run_cli(
        &run,
        &run.review_config,
        &critical_ai.gate_bundle,
        &critical_output,
    );
    assert_eq!(critical.status.code(), Some(4));
    let critical_report = read_gate_report(&critical_output);
    assert_eq!(
        critical_report.primary_failure_type,
        ui_visual_audit::GateFailureType::CriticalRegionFailure
    );
    assert_eq!(critical_report.captures[0].ai.severe_count, 1);

    let semantic_ai = prepare_stage_eight_ai_bundle(
        &run,
        &run.semantic_failed_bundle,
        AiSeverity::Severe,
        "priority-semantic",
    );
    let semantic_output = run.root.join("gate-ai-priority-semantic-output");
    let semantic = run_cli(
        &run,
        &run.pass_config,
        &semantic_ai.gate_bundle,
        &semantic_output,
    );
    assert_eq!(semantic.status.code(), Some(4));
    let semantic_report = read_gate_report(&semantic_output);
    assert_eq!(
        semantic_report.primary_failure_type,
        ui_visual_audit::GateFailureType::SemanticHardFailure
    );
    assert!(semantic_report.captures[0].semantic.hard_failure_count > 0);
    assert_eq!(semantic_report.captures[0].ai.severe_count, 1);

    let dimension_ai =
        prepare_stage_eight_ai_bundle(&run, &run.bundle, AiSeverity::Severe, "priority-dimension");
    let dimension_bundle = convert_to_dimension_bundle(&run, &dimension_ai, "priority-dimension");
    let dimension_output = run.root.join("gate-ai-priority-dimension-output");
    let dimension = run_cli(&run, &run.pass_config, &dimension_bundle, &dimension_output);
    assert_eq!(dimension.status.code(), Some(4));
    let dimension_report = read_gate_report(&dimension_output);
    assert_eq!(
        dimension_report.primary_failure_type,
        ui_visual_audit::GateFailureType::DimensionMismatch
    );
    assert_eq!(dimension_report.captures[0].ai.severe_count, 1);
}

#[test]
fn forged_ai_image_or_dropped_semantic_copy_is_invalid() {
    let run = prepare_gate_run();
    let forged =
        prepare_stage_eight_ai_bundle(&run, &run.bundle, AiSeverity::Minor, "invalid-image-hash");
    mutate_ai_and_rebind(&forged, |report| {
        report["input"]["provider_images"][0]["source_sha256"] = json!("0".repeat(64));
    });
    let forged_output = run.root.join("gate-ai-forged-image-output");
    let forged_result = run_cli(&run, &run.pass_config, &forged.gate_bundle, &forged_output);
    assert_eq!(forged_result.status.code(), Some(2));
    let forged_report = read_gate_report(&forged_output);
    assert_eq!(forged_report.status, GateState::Invalid);
    assert!(
        forged_report
            .validation_errors
            .iter()
            .any(|error| error.code == "ai_image_provenance_mismatch")
    );

    let dropped = prepare_stage_eight_ai_bundle(
        &run,
        &run.semantic_failed_bundle,
        AiSeverity::Minor,
        "invalid-hard-failure-copy",
    );
    mutate_ai_and_rebind(&dropped, |report| {
        report["deterministic_hard_failures"] = json!([]);
    });
    let dropped_output = run.root.join("gate-ai-dropped-hard-failure-output");
    let dropped_result = run_cli(
        &run,
        &run.pass_config,
        &dropped.gate_bundle,
        &dropped_output,
    );
    assert_eq!(dropped_result.status.code(), Some(2));
    let dropped_report = read_gate_report(&dropped_output);
    assert_eq!(dropped_report.status, GateState::Invalid);
    assert!(
        dropped_report
            .validation_errors
            .iter()
            .any(|error| { error.code == "deterministic_hard_failures_not_preserved" })
    );
}

struct PreparedAiGateBundle {
    gate_bundle: PathBuf,
    ai_report: PathBuf,
}

fn prepare_stage_eight_ai_bundle(
    run: &PreparedGateRun,
    source_gate_bundle: &PathBuf,
    severity: AiSeverity,
    suffix: &str,
) -> PreparedAiGateBundle {
    let mut gate_bundle: Value =
        serde_json::from_slice(&fs::read(source_gate_bundle).unwrap()).unwrap();
    let capture = &gate_bundle["captures"][0];
    let capture_id = capture["capture_id"].as_str().unwrap().to_owned();
    let screen = capture["screen"].clone();
    let device = capture["device"].clone();
    let state = capture["state"].clone();
    let diff_path = PathBuf::from(capture["diff_report"]["path"].as_str().unwrap());
    let region_path = PathBuf::from(
        capture["region_report"]["path"]
            .as_str()
            .expect("Stage 8 fixture requires a region report"),
    );
    let semantic_path = PathBuf::from(capture["semantic_report"]["path"].as_str().unwrap());
    let diff: DiffAnalysisReport = serde_json::from_slice(&fs::read(&diff_path).unwrap()).unwrap();
    let region: RegionAuditReport =
        serde_json::from_slice(&fs::read(&region_path).unwrap()).unwrap();
    let semantic: SemanticAuditReport =
        serde_json::from_slice(&fs::read(&semantic_path).unwrap()).unwrap();
    let reference_image = diff.inputs.reference.path.clone();
    let actual_image = diff.inputs.actual.path.clone();
    let metadata_path = semantic.input.path.clone();
    let artifact_path = |artifact_type: &str| {
        PathBuf::from(
            diff.artifacts
                .iter()
                .find(|artifact| artifact.artifact_type == artifact_type)
                .unwrap()
                .path
                .clone(),
        )
    };
    let response_path = run.root.join(format!("ai-{suffix}.response.json"));
    let region_id = region.region_results[0].region_id.clone();
    fs::write(
        &response_path,
        serde_json::to_vec_pretty(&json!({
            "schema_version": 1,
            "issues": [{
                "capture_id": capture_id,
                "problem_type": "layout",
                "severity": ai_severity_label(severity),
                "problem": "Fixture visual issue",
                "evidence": [{
                    "image_id": format!("{capture_id}.actual"),
                    "description": "Actual capture evidence"
                }],
                "region": {"region_id": region_id, "bounds": null},
                "reference_element": null,
                "node_id": null,
                "likely_cause": "Fixture cause",
                "suggested_files": ["project/src/game/screens/dev/ui_gallery.rs"]
            }]
        }))
        .unwrap(),
    )
    .unwrap();
    let config_path = run.root.join(format!("ai-{suffix}.config.json"));
    fs::write(
        &config_path,
        serde_json::to_vec_pretty(&json!({
            "schema_version": 1,
            "algorithm_version": "ui_ai_visual_analysis_v1",
            "provider": {
                "mode": "fixture",
                "provider_id": "stage09-fixture-ai",
                "audit_model_id": "stage09-audit-v1",
                "generation_model_id": "stage09-generation-v1",
                "response": response_path
            },
            "policy": {
                "attempt_timeout_ms": 1000,
                "minimum_request_interval_ms": 0,
                "max_attempts": 1,
                "initial_backoff_ms": 0,
                "max_backoff_ms": 0,
                "max_output_tokens": 1024
            }
        }))
        .unwrap(),
    )
    .unwrap();
    let ai_bundle_path = run.root.join(format!("ai-{suffix}.bundle.json"));
    fs::write(
        &ai_bundle_path,
        serde_json::to_vec_pretty(&json!({
            "schema_version": 1,
            "run_id": format!("stage09-ai-{suffix}"),
            "captures": [{
                "capture_id": capture_id,
                "screen": screen,
                "device": device,
                "state": state,
                "images": {
                    "reference": reference_image,
                    "actual": actual_image,
                    "overlay": artifact_path("overlay"),
                    "heatmap": artifact_path("heatmap")
                },
                "diff_metrics": diff_path,
                "region_metrics": region_path,
                "semantic_report": semantic_path,
                "ui_metadata": metadata_path,
                "allowed_differences": {
                    "profile": "stage09-fixture",
                    "notes": ["Synthetic repository fixture"]
                },
                "likely_files": ["project/src/game/screens/dev/ui_gallery.rs"],
                "privacy": {"redact_semantic_text": true, "redaction_rects": []}
            }]
        }))
        .unwrap(),
    )
    .unwrap();
    let ai_output = run.root.join(format!("ai-{suffix}-output"));
    let outcome = analyze_with_ai(&AiAnalysisRequest {
        repository_root: workspace_root(),
        allowed_input_roots: vec![
            run.root.clone(),
            PathBuf::from("tools/ui-visual-audit/fixtures/semantic"),
        ],
        allowed_output_root: run.root.clone(),
        bundle: ai_bundle_path,
        config: config_path,
        output_directory: ai_output.clone(),
    })
    .unwrap();
    assert_eq!(outcome.exit_code, ComparisonExitCode::Success);
    assert_eq!(outcome.report.issues[0].severity, severity);
    let ai_report = ai_output.join("ai-analysis-report.json");
    gate_bundle["ai_report"] = bound_report(ai_report.clone());
    let gate_bundle_path = run.root.join(format!("gate-ai-{suffix}.bundle.json"));
    fs::write(
        &gate_bundle_path,
        serde_json::to_vec_pretty(&gate_bundle).unwrap(),
    )
    .unwrap();
    PreparedAiGateBundle {
        gate_bundle: gate_bundle_path,
        ai_report,
    }
}

fn convert_to_dimension_bundle(
    run: &PreparedGateRun,
    prepared: &PreparedAiGateBundle,
    suffix: &str,
) -> PathBuf {
    let mut bundle: Value =
        serde_json::from_slice(&fs::read(&prepared.gate_bundle).unwrap()).unwrap();
    let original_diff = PathBuf::from(
        bundle["captures"][0]["diff_report"]["path"]
            .as_str()
            .unwrap(),
    );
    let mut diff: Value = serde_json::from_slice(&fs::read(&original_diff).unwrap()).unwrap();
    diff["status"] = json!("comparison_failed");
    diff["dimensions"]["actual"]["width"] = json!(21);
    diff["metrics"] = Value::Null;
    diff["performance"] = Value::Null;
    diff["failure"] = json!({
        "failure_type": "comparison",
        "code": "dimensions_mismatch",
        "message": "fixture dimension mismatch"
    });
    let diff_path = run.root.join(format!("{suffix}.diff-report.json"));
    fs::write(&diff_path, serde_json::to_vec_pretty(&diff).unwrap()).unwrap();
    bundle["captures"][0]["diff_report"] = bound_report(diff_path);
    bundle["captures"][0]["region_report"] = Value::Null;
    mutate_ai_and_rebind(prepared, |report| {
        report["input"]["region_metric_count"] = json!(0);
        report["issues"][0]["region"] = json!({"region_id": null, "bounds": null});
    });
    bundle["ai_report"] = bound_report(prepared.ai_report.clone());
    let path = run.root.join(format!("gate-{suffix}.bundle.json"));
    fs::write(&path, serde_json::to_vec_pretty(&bundle).unwrap()).unwrap();
    path
}

fn mutate_ai_and_rebind(prepared: &PreparedAiGateBundle, mutate: impl FnOnce(&mut Value)) {
    let mut report: Value =
        serde_json::from_slice(&fs::read(&prepared.ai_report).unwrap()).unwrap();
    mutate(&mut report);
    fs::write(
        &prepared.ai_report,
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    let mut bundle: Value =
        serde_json::from_slice(&fs::read(&prepared.gate_bundle).unwrap()).unwrap();
    bundle["ai_report"] = bound_report(prepared.ai_report.clone());
    fs::write(
        &prepared.gate_bundle,
        serde_json::to_vec_pretty(&bundle).unwrap(),
    )
    .unwrap();
}

fn read_gate_report(output: &std::path::Path) -> VisualGateReport {
    serde_json::from_slice(&fs::read(output.join(VISUAL_GATE_REPORT_FILENAME)).unwrap()).unwrap()
}

fn ai_severity_label(severity: AiSeverity) -> &'static str {
    match severity {
        AiSeverity::Severe => "severe",
        AiSeverity::Medium => "medium",
        AiSeverity::Minor => "minor",
    }
}

fn run_cli(
    run: &PreparedGateRun,
    config: &PathBuf,
    bundle: &PathBuf,
    output: &PathBuf,
) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ui-visual-audit"))
        .arg("evaluate-gate")
        .arg("--repository-root")
        .arg(workspace_root())
        .arg("--allowed-input-root")
        .arg(&run.root)
        .arg("--allowed-output-root")
        .arg(&run.root)
        .arg("--bundle")
        .arg(bundle)
        .arg("--config")
        .arg(config)
        .arg("--output-directory")
        .arg(output)
        .output()
        .unwrap()
}

fn gate_config(
    maximum: u32,
    minimum_ssim: i32,
    reference_binding: &ui_visual_audit::ReferenceBinding,
) -> Value {
    let threshold = json!({
        "max_raw_changed_ratio_millionths": maximum,
        "max_alpha_changed_ratio_millionths": maximum,
        "max_tolerated_changed_ratio_millionths": maximum,
        "minimum_ssim_millionths": minimum_ssim,
        "max_geometry_changed_ratio_millionths": maximum,
        "max_large_area_ratio_millionths": maximum
    });
    json!({
        "schema_version": 1,
        "algorithm_version": "ui_visual_gate_v1",
        "conservative_default": {
            "critical": strict_gate_threshold(),
            "normal": strict_gate_threshold(),
            "decorative": strict_gate_threshold()
        },
        "reference_profiles": [{
            "profile_id": "fixture-profile",
            "reference_binding": reference_binding,
            "thresholds": {
                "critical": threshold,
                "normal": threshold,
                "decorative": threshold
            },
            "calibration_fixture_id": "stage09-integration-fixture",
            "adjustment_rationale": "Integration test profile"
        }]
    })
}

fn strict_gate_threshold() -> Value {
    json!({
        "max_raw_changed_ratio_millionths": 0,
        "max_alpha_changed_ratio_millionths": 0,
        "max_tolerated_changed_ratio_millionths": 0,
        "minimum_ssim_millionths": 1000000,
        "max_geometry_changed_ratio_millionths": 0,
        "max_large_area_ratio_millionths": 0
    })
}

fn bound_report(path: PathBuf) -> Value {
    let bytes = fs::read(&path).unwrap();
    json!({
        "path": path,
        "sha256": format!("{:x}", Sha256::digest(bytes))
    })
}

fn write_png(
    directory: &std::path::Path,
    name: &str,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> PathBuf {
    let path = directory.join(name);
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(rgba, width, height, ExtendedColorType::Rgba8)
        .unwrap();
    fs::write(&path, bytes).unwrap();
    path
}

fn write_bytes(directory: &std::path::Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = directory.join(name);
    fs::write(&path, bytes).unwrap();
    path
}

fn set_pixel(rgba: &mut [u8], width: u32, x: u32, y: u32, color: [u8; 4]) {
    let offset = ((y * width + x) * 4) as usize;
    rgba[offset..offset + 4].copy_from_slice(&color);
}

fn fill_rect(
    rgba: &mut [u8],
    width: u32,
    (x, y, rect_width, rect_height): (u32, u32, u32, u32),
    color: [u8; 4],
) {
    for row in y..y + rect_height {
        for column in x..x + rect_width {
            set_pixel(rgba, width, column, row, color);
        }
    }
}
