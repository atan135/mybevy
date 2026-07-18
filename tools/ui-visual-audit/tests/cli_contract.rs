mod common;

use common::TestRepository;
use serde_json::Value;
use std::{fs, process::Command};

fn compare_command(
    repository: &TestRepository,
    reference: &std::path::Path,
    actual: &std::path::Path,
    config: &std::path::Path,
    output_name: &str,
) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ui-visual-audit"));
    command
        .arg("compare")
        .arg("--repository-root")
        .arg(&repository.root)
        .arg("--allowed-input-root")
        .arg(&repository.inputs)
        .arg("--allowed-output-root")
        .arg(&repository.outputs)
        .arg("--reference")
        .arg(reference)
        .arg("--actual")
        .arg(actual)
        .arg("--config")
        .arg(config)
        .arg("--output-directory")
        .arg(repository.outputs.join(output_name));
    command
}

#[test]
fn help_is_human_readable_and_successful() {
    let output = Command::new(env!("CARGO_BIN_EXE_ui-visual-audit"))
        .arg("compare")
        .arg("--help")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("--reference"));
    assert!(stdout.contains("--allowed-input-root"));
    assert!(stdout.contains("--output-directory"));
}

#[test]
fn success_stdout_and_persisted_report_follow_the_json_contract() {
    let repository = TestRepository::new();
    let reference = repository.write_png("reference.png", 1, 1, &[1, 2, 3, 255]);
    let actual = repository.write_png("actual.png", 1, 1, &[1, 2, 3, 255]);
    let mask = repository.write_png("mask.png", 1, 1, &[255, 255, 255, 255]);
    let config = repository.write_config(0.0);
    let mut command = compare_command(&repository, &reference, &actual, &config, "success");
    command.arg("--mask").arg(mask);
    let output = command.output().unwrap();

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["algorithm_version"], "exact_rgba_v1");
    assert_eq!(report["status"], "passed");
    assert_eq!(report["dimensions"]["reference"]["width"], 1);
    assert_eq!(report["metrics"]["evaluated_pixels"], 1);
    assert_eq!(report["region_results"][0]["region_id"], "full_image");
    assert_eq!(report["inputs"]["mask"]["dimensions"]["width"], 1);
    assert_eq!(report["failure"], Value::Null);
    assert_eq!(report["artifacts"][0]["artifact_type"], "comparison_report");

    let persisted = fs::read(repository.outputs.join("success/comparison-report.json")).unwrap();
    let persisted: Value = serde_json::from_slice(&persisted).unwrap();
    assert_eq!(persisted, report);
    let reported_reference =
        std::path::PathBuf::from(report["inputs"]["reference"]["path"].as_str().unwrap());
    assert!(reported_reference.is_absolute());
    assert!(reported_reference.starts_with(fs::canonicalize(&repository.root).unwrap()));
}

#[test]
fn binary_exit_codes_distinguish_input_comparison_and_threshold_failures() {
    let repository = TestRepository::new();
    let exact = repository.write_png("exact.png", 1, 1, &[0, 0, 0, 255]);
    let changed = repository.write_png("changed.png", 1, 1, &[255, 255, 255, 255]);
    let wide = repository.write_png("wide.png", 2, 1, &[0, 0, 0, 255, 0, 0, 0, 255]);
    let corrupt = repository.write_bytes("corrupt.png", b"not an image");
    let config = repository.write_config(0.0);

    let input = compare_command(&repository, &corrupt, &exact, &config, "input")
        .output()
        .unwrap();
    assert_eq!(input.status.code(), Some(2));
    let input_error: Value = serde_json::from_slice(&input.stderr).unwrap();
    assert_eq!(input_error["failure"]["failure_type"], "input");
    assert_eq!(input_error["failure"]["code"], "image_corrupt");

    let comparison = compare_command(&repository, &exact, &wide, &config, "comparison")
        .output()
        .unwrap();
    assert_eq!(comparison.status.code(), Some(3));
    let comparison_report: Value = serde_json::from_slice(&comparison.stdout).unwrap();
    assert_eq!(comparison_report["status"], "comparison_failed");
    assert_eq!(comparison_report["failure"]["failure_type"], "comparison");
    assert_eq!(comparison_report["failure"]["code"], "dimensions_mismatch");

    let threshold = compare_command(&repository, &exact, &changed, &config, "threshold")
        .output()
        .unwrap();
    assert_eq!(threshold.status.code(), Some(4));
    let threshold_report: Value = serde_json::from_slice(&threshold.stdout).unwrap();
    assert_eq!(threshold_report["status"], "threshold_failed");
    assert_eq!(threshold_report["failure"]["failure_type"], "threshold");
    assert_eq!(threshold_report["failure"]["code"], "threshold_exceeded");
}

#[test]
fn invalid_cli_and_reserved_artifact_conflicts_are_machine_readable_input_errors() {
    let invalid = Command::new(env!("CARGO_BIN_EXE_ui-visual-audit"))
        .arg("compare")
        .arg("--unknown")
        .output()
        .unwrap();
    assert_eq!(invalid.status.code(), Some(2));
    let invalid: Value = serde_json::from_slice(&invalid.stderr).unwrap();
    assert_eq!(invalid["failure"]["code"], "cli_arguments_invalid");

    let repository = TestRepository::new();
    let reference = repository.write_png("reference.png", 1, 1, &[1, 2, 3, 255]);
    let actual = repository.write_png("actual.png", 1, 1, &[1, 2, 3, 255]);
    let config = repository.write_config(0.0);
    let output_directory = repository.outputs.join("conflict");
    fs::create_dir(&output_directory).unwrap();
    fs::write(output_directory.join("comparison-report.json"), b"existing").unwrap();

    let conflict = compare_command(&repository, &reference, &actual, &config, "conflict")
        .output()
        .unwrap();
    assert_eq!(conflict.status.code(), Some(2));
    let conflict: Value = serde_json::from_slice(&conflict.stderr).unwrap();
    assert_eq!(conflict["failure"]["code"], "artifact_name_conflict");
}
