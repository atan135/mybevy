mod common;

use common::TestRepository;
use serde_json::Value;
use std::process::Command;

fn command(repository: &TestRepository, output: &str) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ui-visual-audit"));
    command.args([
        "analyze-diff",
        "--repository-root",
        repository.root.to_str().unwrap(),
        "--allowed-input-root",
        repository.inputs.to_str().unwrap(),
        "--allowed-output-root",
        repository.outputs.to_str().unwrap(),
        "--reference",
        repository.inputs.join("reference.png").to_str().unwrap(),
        "--actual",
        repository.inputs.join("actual.png").to_str().unwrap(),
        "--config",
        repository
            .inputs
            .join("metrics.config.json")
            .to_str()
            .unwrap(),
        "--output-directory",
        repository.outputs.join(output).to_str().unwrap(),
    ]);
    command
}

fn write_config(repository: &TestRepository) {
    repository.write_bytes(
        "metrics.config.json",
        include_bytes!("../fixtures/comparison/ui-diff-metrics-v1.config.json"),
    );
}

#[test]
fn cli_reports_differences_without_claiming_a_stage_nine_gate() {
    let repository = TestRepository::new();
    repository.write_png("reference.png", 8, 8, &[10, 20, 30, 255].repeat(64));
    repository.write_png("actual.png", 8, 8, &[40, 50, 60, 255].repeat(64));
    write_config(&repository);
    let output = command(&repository, "difference").output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["algorithm_version"], "ui_diff_metrics_v1");
    assert_eq!(report["status"], "analyzed");
    assert_eq!(report["metrics"]["raw"]["changed_pixels"], 64);
    assert_eq!(report["metrics"]["tolerated"]["changed_pixels"], 64);
    assert!(report["failure"].is_null());
    assert_eq!(report["artifacts"].as_array().unwrap().len(), 5);
    let persisted: Value = serde_json::from_slice(
        &std::fs::read(
            repository
                .outputs
                .join("difference/diff-metrics-report.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(persisted, report);
}

#[test]
fn cli_dimension_failure_keeps_machine_readable_comparison_exit() {
    let repository = TestRepository::new();
    repository.write_png("reference.png", 8, 8, &[10, 20, 30, 255].repeat(64));
    repository.write_png("actual.png", 7, 8, &[10, 20, 30, 255].repeat(56));
    write_config(&repository);
    let output = command(&repository, "dimensions").output().unwrap();
    assert_eq!(output.status.code(), Some(3));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["status"], "comparison_failed");
    assert_eq!(report["failure"]["failure_type"], "comparison");
    assert_eq!(report["failure"]["code"], "dimensions_mismatch");
    assert!(!repository.outputs.join("dimensions/heatmap.png").exists());
}

#[test]
fn cli_rejects_invalid_config_with_the_shared_input_error_contract() {
    let repository = TestRepository::new();
    repository.write_png("reference.png", 8, 8, &[10, 20, 30, 255].repeat(64));
    repository.write_png("actual.png", 8, 8, &[10, 20, 30, 255].repeat(64));
    repository.write_bytes(
        "metrics.config.json",
        br#"{"schema_version":1,"algorithm_version":"ui_diff_metrics_v1","over_threshold_channel_abs":8,"small_channel_tolerance":7,"edge_antialias_tolerance":12,"edge_luma_threshold":96,"ssim_window_size":8,"large_area_min_pixels":16,"large_area_min_ratio_millionths":1000}"#,
    );
    let output = command(&repository, "invalid").output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["failure"]["failure_type"], "input");
    assert_eq!(error["failure"]["code"], "config_invalid");
}
