mod common;

use common::TestRepository;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf, process::Command};
use ui_visual_audit::{
    ComparisonErrorCode, ComparisonExitCode, NormalizationRequest, RegionAuditRequest,
    RegionLocalStatus, audit_regions, normalize_and_align,
};

struct PreparedRun {
    repository: TestRepository,
    aligned_reference: PathBuf,
    aligned_actual: PathBuf,
    normalization_report: PathBuf,
    diff_config: PathBuf,
    region_config: PathBuf,
}

impl PreparedRun {
    fn request(&self, output: &str) -> RegionAuditRequest {
        RegionAuditRequest {
            repository_root: self.repository.root.clone(),
            allowed_input_roots: vec![
                self.repository.inputs.clone(),
                self.repository.outputs.clone(),
            ],
            allowed_output_root: self.repository.outputs.clone(),
            reference: self.aligned_reference.clone(),
            actual: self.aligned_actual.clone(),
            diff_config: self.diff_config.clone(),
            region_config: self.region_config.clone(),
            normalization_report: self.normalization_report.clone(),
            output_directory: self.repository.outputs.join(output),
        }
    }

    fn rewrite_config(&mut self, mutator: impl FnOnce(&mut Value)) {
        let mut value: Value =
            serde_json::from_slice(&fs::read(&self.region_config).unwrap()).unwrap();
        mutator(&mut value);
        fs::write(
            &self.region_config,
            serde_json::to_vec_pretty(&value).unwrap(),
        )
        .unwrap();
    }
}

fn prepare() -> PreparedRun {
    let repository = TestRepository::new();
    let width = 20;
    let height = 20;
    let mut reference = [230, 230, 230, 255].repeat((width * height) as usize);
    let mut actual = reference.clone();
    fill_rect(&mut reference, width, 8, 4, 4, 5, [20, 60, 120, 255]);
    fill_rect(&mut actual, width, 8, 4, 4, 5, [20, 60, 120, 255]);
    fill_rect(&mut actual, width, 0, 0, 4, 4, [80, 90, 100, 255]);
    set_pixel(&mut actual, width, 6, 6, [0, 0, 0, 255]);
    let reference_path = repository.write_png("reference.png", width, height, &reference);
    let actual_path = repository.write_png("actual.png", width, height, &actual);
    let normalization_manifest = repository.write_bytes(
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
    let normalized_directory = repository.outputs.join("normalized");
    let normalized = normalize_and_align(&NormalizationRequest {
        repository_root: repository.root.clone(),
        allowed_input_roots: vec![repository.inputs.clone()],
        allowed_output_root: repository.outputs.clone(),
        reference: reference_path,
        actual: actual_path,
        normalization_manifest,
        output_directory: normalized_directory.clone(),
    })
    .unwrap();
    assert_eq!(normalized.exit_code, ComparisonExitCode::Success);

    let mut mask = [0, 0, 0, 0].repeat((width * height) as usize);
    set_pixel(&mut mask, width, 2, 2, [255, 255, 255, 255]);
    let mask_path = repository.write_png("dynamic-mask.png", width, height, &mask);
    let mask_hash = format!("{:x}", Sha256::digest(fs::read(mask_path).unwrap()));
    let reference_hash = normalized.report.reference.sha256.clone();
    let mut config: Value = serde_json::from_slice(include_bytes!(
        "../fixtures/comparison/ui-region-audit-v1.config.json"
    ))
    .unwrap();
    config["reference_binding"]["sha256"] = json!(reference_hash);
    let active_reference_hash = config["reference_binding"]["sha256"].clone();
    for ignore in config["ignore_regions"].as_array_mut().unwrap() {
        ignore["reference_binding"]["sha256"] = active_reference_hash.clone();
        if ignore["ignore_id"] == "avatar_animation" {
            ignore["shape"]["sha256"] = json!(mask_hash);
        }
    }
    let region_config =
        repository.write_bytes("regions.json", &serde_json::to_vec_pretty(&config).unwrap());
    let diff_config = repository.write_bytes(
        "metrics.json",
        include_bytes!("../fixtures/comparison/ui-diff-metrics-v1.config.json"),
    );
    PreparedRun {
        aligned_reference: normalized_directory.join("aligned-reference.png"),
        aligned_actual: normalized_directory.join("aligned-actual.png"),
        normalization_report: normalized_directory.join("normalization-report.json"),
        repository,
        diff_config,
        region_config,
    }
}

#[test]
fn fixture_maps_sources_masks_overlaps_and_local_weighted_results() {
    let run = prepare();
    let outcome = audit_regions(&run.request("region-audit")).unwrap();
    assert_eq!(outcome.exit_code, ComparisonExitCode::Success);
    assert_eq!(outcome.report.algorithm_version, "ui_region_audit_v1");
    assert!(outcome.report.scope_boundary.contains("no_global"));
    assert_eq!(outcome.report.coverage.ignored_union_pixels, 16);
    assert_eq!(outcome.report.coverage.ignored_ratio_millionths, 40_000);
    assert_eq!(outcome.report.ignore_regions[0].newly_ignored_pixels, 16);
    assert_eq!(outcome.report.ignore_regions[1].selected_pixels, 1);
    assert_eq!(
        outcome.report.ignore_regions[1].overlap_with_prior_ignores_pixels,
        1
    );
    assert_eq!(outcome.report.ignore_regions[1].newly_ignored_pixels, 0);

    let score = &outcome.report.region_results[0];
    assert_eq!(score.selected_pixels_before_exclusions, 64);
    assert_eq!(score.excluded_pixels, 16);
    assert_eq!(score.evaluated_pixels, 48);
    assert_eq!(score.metrics.raw.changed_pixels, 1);
    assert_eq!(score.local_status, RegionLocalStatus::Failed);
    assert_eq!(score.primary_difference_locations[0].aligned.x, 6);
    assert_eq!(
        score.primary_difference_locations[0].reference_original.x,
        6
    );

    let action = &outcome.report.region_results[1];
    assert_eq!(action.overlaps_prior_regions_pixels, 16);
    assert_eq!(action.metrics.raw.changed_pixels, 1);
    assert_eq!(outcome.report.weight_summary.total_declared_weight, 250);
    assert_eq!(outcome.report.weight_summary.failed_weight, 200);
    assert_eq!(outcome.report.weight_summary.passed_weight, 50);

    let ignored = image::open(
        run.repository
            .outputs
            .join("region-audit/ignored-regions.png"),
    )
    .unwrap()
    .into_rgba8();
    assert_eq!(ignored.dimensions(), (20, 20));
    assert_eq!(ignored.get_pixel(0, 0).0, [255, 0, 255, 255]);
    assert_ne!(ignored.get_pixel(6, 6).0, [255, 0, 255, 255]);
    assert_eq!(outcome.report.artifacts.len(), 3);
    let persisted: Value = serde_json::from_slice(
        &fs::read(
            run.repository
                .outputs
                .join("region-audit/region-audit-report.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(persisted["status"], "analyzed");
}

#[test]
fn stale_reference_and_per_mask_bindings_are_rejected() {
    let mut stale = prepare();
    stale.rewrite_config(|config| {
        config["reference_binding"]["sha256"] = json!("11".repeat(32));
    });
    let error = audit_regions(&stale.request("stale")).unwrap_err();
    assert_eq!(
        error.failure.code,
        ComparisonErrorCode::ReferenceBindingMismatch
    );

    let mut mask_stale = prepare();
    mask_stale.rewrite_config(|config| {
        config["ignore_regions"][0]["reference_binding"]["revision"] = json!(2);
    });
    let error = audit_regions(&mask_stale.request("mask-stale")).unwrap_err();
    assert_eq!(error.failure.code, ComparisonErrorCode::MaskBindingMismatch);
}

#[test]
fn missing_reason_and_excessive_ignore_ratio_have_stable_codes() {
    let mut missing_reason = prepare();
    missing_reason.rewrite_config(|config| {
        config["ignore_regions"][0]["reason"] = json!("  ");
    });
    let error = audit_regions(&missing_reason.request("reason")).unwrap_err();
    assert_eq!(error.failure.code, ComparisonErrorCode::IgnoreReasonMissing);

    let mut excessive = prepare();
    excessive.rewrite_config(|config| {
        config["maximum_ignored_ratio_millionths"] = json!(39_999);
    });
    let error = audit_regions(&excessive.request("ratio")).unwrap_err();
    assert_eq!(error.failure.code, ComparisonErrorCode::IgnoreRatioExceeded);
}

#[test]
fn empty_after_exclusion_and_mask_size_mismatch_are_rejected() {
    let mut empty = prepare();
    empty.rewrite_config(|config| {
        config["regions"] = json!([{
            "region_id":"clock_only",
            "label":"Clock only",
            "semantic_role":"dynamic",
            "level":"normal",
            "clipping":"reject_out_of_bounds",
            "source":{
                "kind":"manual",
                "coordinate_space":"aligned",
                "shape":{"kind":"rectangle","bounds":{"x":0,"y":0,"width":4,"height":4}}
            }
        }]);
    });
    let error = audit_regions(&empty.request("empty")).unwrap_err();
    assert_eq!(error.failure.code, ComparisonErrorCode::RegionEmpty);

    let mut wrong_size = prepare();
    let small =
        wrong_size
            .repository
            .write_png("small-mask.png", 2, 2, &[255, 255, 255, 255].repeat(4));
    let hash = format!("{:x}", Sha256::digest(fs::read(&small).unwrap()));
    wrong_size.rewrite_config(|config| {
        config["ignore_regions"][1]["shape"]["path"] = json!("inputs/small-mask.png");
        config["ignore_regions"][1]["shape"]["sha256"] = json!(hash);
    });
    let error = audit_regions(&wrong_size.request("size")).unwrap_err();
    assert_eq!(
        error.failure.code,
        ComparisonErrorCode::MaskDimensionsMismatch
    );
}

#[test]
fn strict_schema_and_weight_order_do_not_silently_fallback() {
    let mut unknown = prepare();
    unknown.rewrite_config(|config| {
        config["unexpected"] = json!(true);
    });
    let error = audit_regions(&unknown.request("unknown")).unwrap_err();
    assert_eq!(error.failure.code, ComparisonErrorCode::ConfigParseFailed);

    let mut weights = prepare();
    weights.rewrite_config(|config| {
        config["threshold_profiles"]["critical"]["weight"] = json!(1);
    });
    let error = audit_regions(&weights.request("weights")).unwrap_err();
    assert_eq!(error.failure.code, ComparisonErrorCode::RegionConfigInvalid);
}

#[test]
fn full_image_scope_rejects_gaps_and_accepts_explicit_complete_coverage() {
    let mut gap = prepare();
    gap.rewrite_config(|config| {
        config["audit_scope"] = json!("full_image");
    });
    let error = audit_regions(&gap.request("full-gap")).unwrap_err();
    assert_eq!(
        error.failure.code,
        ComparisonErrorCode::AuditScopeIncomplete
    );

    let mut complete = prepare();
    complete.rewrite_config(|config| {
        config["audit_scope"] = json!("full_image");
        config["regions"] = json!([{
            "region_id":"explicit_full_image",
            "label":"Explicit full image coverage",
            "semantic_role":"content",
            "level":"normal",
            "clipping":"reject_out_of_bounds",
            "source":{
                "kind":"manual",
                "coordinate_space":"aligned",
                "shape":{"kind":"rectangle","bounds":{"x":0,"y":0,"width":20,"height":20}}
            }
        }]);
    });
    let outcome = audit_regions(&complete.request("full-complete")).unwrap();
    assert_eq!(outcome.exit_code, ComparisonExitCode::Success);
    assert_eq!(outcome.report.coverage.declared_include_union_pixels, 400);
    assert_eq!(outcome.report.coverage.uncovered_pixels, 0);
    assert_eq!(outcome.report.coverage.effective_audited_union_pixels, 384);
    assert_eq!(outcome.report.region_results.len(), 1);
    assert_eq!(outcome.report.region_results[0].evaluated_pixels, 384);
    assert_eq!(
        outcome.report.region_results[0]
            .metrics
            .raw
            .evaluated_pixels,
        outcome.report.coverage.effective_audited_union_pixels
    );
}

#[test]
fn cli_prints_the_persisted_report_and_keeps_stage_nine_gate_out_of_scope() {
    let run = prepare();
    let output_directory = run.repository.outputs.join("cli");
    let output = Command::new(env!("CARGO_BIN_EXE_ui-visual-audit"))
        .args([
            "audit-regions",
            "--repository-root",
            run.repository.root.to_str().unwrap(),
            "--allowed-input-root",
            run.repository.inputs.to_str().unwrap(),
            "--allowed-input-root",
            run.repository.outputs.to_str().unwrap(),
            "--allowed-output-root",
            run.repository.outputs.to_str().unwrap(),
            "--reference",
            run.aligned_reference.to_str().unwrap(),
            "--actual",
            run.aligned_actual.to_str().unwrap(),
            "--diff-config",
            run.diff_config.to_str().unwrap(),
            "--region-config",
            run.region_config.to_str().unwrap(),
            "--normalization-report",
            run.normalization_report.to_str().unwrap(),
            "--output-directory",
            output_directory.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let stdout: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(stdout["status"], "analyzed");
    assert_eq!(stdout["region_results"][0]["local_status"], "failed");
    assert!(
        stdout["scope_boundary"]
            .as_str()
            .unwrap()
            .contains("no_global")
    );
    let persisted: Value = serde_json::from_slice(
        &fs::read(output_directory.join("region-audit-report.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(persisted, stdout);
}

fn fill_rect(
    rgba: &mut [u8],
    width: u32,
    x: u32,
    y: u32,
    rect_width: u32,
    rect_height: u32,
    color: [u8; 4],
) {
    for target_y in y..y + rect_height {
        for target_x in x..x + rect_width {
            set_pixel(rgba, width, target_x, target_y, color);
        }
    }
}

fn set_pixel(rgba: &mut [u8], width: u32, x: u32, y: u32, color: [u8; 4]) {
    let offset = ((y * width + x) * 4) as usize;
    rgba[offset..offset + 4].copy_from_slice(&color);
}
