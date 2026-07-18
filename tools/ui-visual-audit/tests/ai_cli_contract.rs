use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};
use serde_json::{Value, json};
use std::{fs, path::PathBuf, process::Command};
use tempfile::TempDir;
use ui_visual_audit::{
    AI_ANALYSIS_REPORT_FILENAME, AiAnalysisReport, ComparisonExitCode, DiffAnalysisRequest,
    NormalizationRequest, RegionAuditRequest, SemanticAuditRequest, analyze_aligned_diff,
    audit_regions, audit_semantics, normalize_and_align,
};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .unwrap()
        .to_path_buf()
}

struct PreparedAiRun {
    _workspace_temp: TempDir,
    inputs: PathBuf,
    bundle: PathBuf,
    config: PathBuf,
    output: PathBuf,
}

fn prepare_ai_run() -> PreparedAiRun {
    let workspace_temp = TempDir::new_in(workspace_root()).unwrap();
    let inputs = workspace_temp.path().join("inputs");
    fs::create_dir_all(&inputs).unwrap();
    let width = 20;
    let height = 20;
    let mut reference = [230, 230, 230, 255].repeat((width * height) as usize);
    let mut actual = reference.clone();
    fill_rect(&mut reference, width, 8, 4, 4, 5, [20, 60, 120, 255]);
    fill_rect(&mut actual, width, 8, 4, 4, 5, [20, 60, 120, 255]);
    set_pixel(&mut actual, width, 6, 6, [0, 0, 0, 255]);
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
    let normalized_dir = workspace_temp.path().join("normalized");
    let normalized = normalize_and_align(&NormalizationRequest {
        repository_root: workspace_root(),
        allowed_input_roots: vec![inputs.clone()],
        allowed_output_root: workspace_temp.path().to_path_buf(),
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
    let diff_dir = workspace_temp.path().join("diff");
    let diff = analyze_aligned_diff(&DiffAnalysisRequest {
        repository_root: workspace_root(),
        allowed_input_roots: vec![normalized_dir.clone(), inputs.clone()],
        allowed_output_root: workspace_temp.path().to_path_buf(),
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
    region_config["regions"] = json!([{
        "region_id": "full",
        "label": "Full capture",
        "semantic_role": "content",
        "level": "normal",
        "clipping": "reject_out_of_bounds",
        "source": {"kind": "manual", "coordinate_space": "aligned", "shape": {
            "kind": "rectangle", "bounds": {"x": 0, "y": 0, "width": width, "height": height}
        }}
    }]);
    region_config["bounds_sources"] = json!([]);
    let region_config_path = write_bytes(
        &inputs,
        "regions.json",
        &serde_json::to_vec_pretty(&region_config).unwrap(),
    );
    let region_dir = workspace_temp.path().join("regions");
    let region = audit_regions(&RegionAuditRequest {
        repository_root: workspace_root(),
        allowed_input_roots: vec![
            normalized_dir.clone(),
            inputs.clone(),
            workspace_temp.path().to_path_buf(),
        ],
        allowed_output_root: workspace_temp.path().to_path_buf(),
        reference: normalized_dir.join("aligned-reference.png"),
        actual: normalized_dir.join("aligned-actual.png"),
        diff_config,
        region_config: region_config_path,
        normalization_report: normalized_dir.join("normalization-report.json"),
        output_directory: region_dir.clone(),
    })
    .unwrap();
    assert_eq!(region.exit_code, ComparisonExitCode::Success);

    let semantic_metadata = workspace_temp.path().join("capture.metadata.json");
    let mut metadata: Value = serde_json::from_slice(include_bytes!(
        "../fixtures/semantic/compact-pass.metadata.json"
    ))
    .unwrap();
    let button = metadata["semantic_tree"]["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|node| node["role"] == "button")
        .unwrap();
    let min_x = button["bounds"]["min_x"].as_f64().unwrap();
    let min_y = button["bounds"]["min_y"].as_f64().unwrap();
    button["bounds"]["max_x"] = json!(min_x + 4.0);
    button["bounds"]["max_y"] = json!(min_y + 4.0);
    fs::write(
        &semantic_metadata,
        serde_json::to_vec_pretty(&metadata).unwrap(),
    )
    .unwrap();
    let semantic_dir = workspace_temp.path().join("semantic");
    let semantic = audit_semantics(&SemanticAuditRequest {
        repository_root: workspace_root(),
        allowed_input_roots: vec![
            workspace_temp.path().to_path_buf(),
            PathBuf::from("tools/ui-visual-audit/fixtures/semantic"),
        ],
        allowed_output_root: workspace_temp.path().to_path_buf(),
        metadata: semantic_metadata.clone(),
        config: PathBuf::from(
            "tools/ui-visual-audit/fixtures/semantic/ui-semantic-audit-v1.config.json",
        ),
        output_directory: semantic_dir.clone(),
    })
    .unwrap();
    assert!(!semantic.report.findings.is_empty());

    let response = workspace_temp.path().join("fixture-response.json");
    fs::write(&response, br#"{"schema_version":1,"issues":[]}"#).unwrap();
    let config = workspace_temp.path().join("fixture.config.json");
    fs::write(
        &config,
        serde_json::to_vec_pretty(&json!({
            "schema_version": 1,
            "algorithm_version": "ui_ai_visual_analysis_v1",
            "provider": {
                "mode": "fixture",
                "provider_id": "fixture-ai",
                "audit_model_id": "fixture-audit-v1",
                "generation_model_id": "fixture-generation-v1",
                "response": response
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
    let bundle = workspace_temp.path().join("bundle.json");
    fs::write(
        &bundle,
        serde_json::to_vec_pretty(&json!({
            "schema_version": 1,
            "run_id": "fixture-ai-run",
            "captures": [{
                "capture_id": "login.compact.initial",
                "screen": "login",
                "device": "compact",
                "state": "initial",
                "images": {
                    "reference": normalized_dir.join("aligned-reference.png"),
                    "actual": normalized_dir.join("aligned-actual.png"),
                    "overlay": diff_dir.join("overlay.png"),
                    "heatmap": diff_dir.join("heatmap.png")
                },
                "diff_metrics": diff_dir.join("diff-metrics-report.json"),
                "region_metrics": region_dir.join("region-audit-report.json"),
                "semantic_report": semantic_dir.join("semantic-audit-report.json"),
                "ui_metadata": semantic_metadata,
                "allowed_differences": {"profile": "fixture", "notes": ["No live data"]},
                "likely_files": ["project/src/game/screens/auth/login.rs"],
                "privacy": {"redact_semantic_text": true, "redaction_rects": []}
            }]
        }))
        .unwrap(),
    )
    .unwrap();
    let output = workspace_temp.path().join("ai-output");
    PreparedAiRun {
        _workspace_temp: workspace_temp,
        inputs,
        bundle,
        config,
        output,
    }
}

#[test]
fn fixture_cli_consumes_the_complete_bundle_and_preserves_hard_failures() {
    let run = prepare_ai_run();
    let result = run_cli(&run);
    assert_eq!(
        result.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: AiAnalysisReport =
        serde_json::from_slice(&fs::read(run.output.join(AI_ANALYSIS_REPORT_FILENAME)).unwrap())
            .unwrap();
    assert_eq!(report.input.image_count, 4);
    assert_eq!(report.input.capture_count, 1);
    assert!(report.issues.is_empty());
    assert!(!report.deterministic_hard_failures.is_empty());
    assert!(report.deterministic_hard_failures_preserved);
    assert!(!report.visual_similarity_is_sole_conclusion);
    assert!(!report.provider.self_review_is_sole_conclusion);
    assert!(!report.privacy.credentials_persisted);
    assert!(!report.privacy.raw_provider_response_persisted);
}

#[test]
fn cli_rejects_region_metrics_forged_against_the_supplied_images() {
    let run = prepare_ai_run();
    let bundle: Value = serde_json::from_slice(&fs::read(&run.bundle).unwrap()).unwrap();
    let region_path = PathBuf::from(bundle["captures"][0]["region_metrics"].as_str().unwrap());
    let mut region: Value = serde_json::from_slice(&fs::read(&region_path).unwrap()).unwrap();
    region["inputs"]["aligned_actual_sha256"] =
        json!("0000000000000000000000000000000000000000000000000000000000000000");
    fs::write(&region_path, serde_json::to_vec_pretty(&region).unwrap()).unwrap();
    let result = run_cli(&run);
    assert_eq!(result.status.code(), Some(2));
    let failure: Value = serde_json::from_slice(&result.stderr).unwrap();
    assert_eq!(failure["failure"]["code"], "ai_input_invalid");
    assert!(!run.output.exists());
}

#[test]
fn cli_rejects_swapped_or_forged_overlay_and_heatmap_artifacts() {
    for role in ["overlay", "heatmap"] {
        let run = prepare_ai_run();
        let mut bundle: Value = serde_json::from_slice(&fs::read(&run.bundle).unwrap()).unwrap();
        if role == "overlay" {
            let overlay = bundle["captures"][0]["images"]["overlay"].clone();
            bundle["captures"][0]["images"]["overlay"] =
                bundle["captures"][0]["images"]["heatmap"].clone();
            bundle["captures"][0]["images"]["heatmap"] = overlay;
        } else {
            let path = PathBuf::from(bundle["captures"][0]["images"]["heatmap"].as_str().unwrap());
            let mut image = image::open(&path).unwrap().into_rgba8();
            image.put_pixel(0, 0, image::Rgba([17, 23, 31, 255]));
            image.save(&path).unwrap();
        }
        fs::write(&run.bundle, serde_json::to_vec_pretty(&bundle).unwrap()).unwrap();
        let result = run_cli(&run);
        assert_eq!(result.status.code(), Some(2), "role={role}");
        let failure: Value = serde_json::from_slice(&result.stderr).unwrap();
        assert_eq!(failure["failure"]["code"], "ai_input_invalid");
    }
}

#[test]
fn cli_rejects_semantic_report_with_forged_metadata_hash() {
    let run = prepare_ai_run();
    let bundle: Value = serde_json::from_slice(&fs::read(&run.bundle).unwrap()).unwrap();
    let semantic_path = PathBuf::from(bundle["captures"][0]["semantic_report"].as_str().unwrap());
    let mut semantic: Value = serde_json::from_slice(&fs::read(&semantic_path).unwrap()).unwrap();
    semantic["input"]["metadata_sha256"] =
        json!("0000000000000000000000000000000000000000000000000000000000000000");
    fs::write(
        &semantic_path,
        serde_json::to_vec_pretty(&semantic).unwrap(),
    )
    .unwrap();
    let result = run_cli(&run);
    assert_eq!(result.status.code(), Some(2));
    let failure: Value = serde_json::from_slice(&result.stderr).unwrap();
    assert_eq!(failure["failure"]["code"], "ai_input_invalid");
}

fn run_cli(run: &PreparedAiRun) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ui-visual-audit"))
        .args([
            "analyze-ai",
            "--repository-root",
            workspace_root().to_str().unwrap(),
            "--allowed-input-root",
            run._workspace_temp.path().to_str().unwrap(),
            "--allowed-input-root",
            run.inputs.to_str().unwrap(),
            "--allowed-output-root",
            run._workspace_temp.path().to_str().unwrap(),
            "--bundle",
            run.bundle.to_str().unwrap(),
            "--config",
            run.config.to_str().unwrap(),
            "--output-directory",
            run.output.to_str().unwrap(),
        ])
        .output()
        .unwrap()
}

fn set_pixel(bytes: &mut [u8], width: u32, x: u32, y: u32, color: [u8; 4]) {
    let offset = ((y * width + x) * 4) as usize;
    bytes[offset..offset + 4].copy_from_slice(&color);
}

fn fill_rect(
    bytes: &mut [u8],
    image_width: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    color: [u8; 4],
) {
    for row in y..y + height {
        for column in x..x + width {
            set_pixel(bytes, image_width, column, row, color);
        }
    }
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
