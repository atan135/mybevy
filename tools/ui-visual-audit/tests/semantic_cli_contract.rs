use serde_json::Value;
use std::{path::PathBuf, process::Command};
use tempfile::TempDir;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .unwrap()
        .to_path_buf()
}

fn command(output: &TempDir, metadata: &str) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ui-visual-audit"));
    command.args([
        "audit-semantics",
        "--repository-root",
        root().to_str().unwrap(),
        "--allowed-input-root",
        "tools/ui-visual-audit/fixtures/semantic",
        "--allowed-output-root",
        output.path().to_str().unwrap(),
        "--metadata",
        metadata,
        "--config",
        "tools/ui-visual-audit/fixtures/semantic/ui-semantic-audit-v1.config.json",
        "--output-directory",
        output.path().join("result").to_str().unwrap(),
    ]);
    command
}

#[test]
fn cli_passes_compact_and_writes_report() {
    let output = TempDir::new_in(root()).unwrap();
    let result = command(
        &output,
        "tools/ui-visual-audit/fixtures/semantic/compact-pass.metadata.json",
    )
    .output()
    .unwrap();
    assert_eq!(
        result.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let stdout: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(stdout["status"], "passed");
    assert!(
        output
            .path()
            .join("result/semantic-audit-report.json")
            .is_file()
    );
}

#[test]
fn cli_rejects_unsafe_metadata_path_with_stable_snake_case_code() {
    let output = TempDir::new_in(root()).unwrap();
    let result = command(&output, "../../outside.json").output().unwrap();
    assert_eq!(result.status.code(), Some(2));
    let stderr: Value = serde_json::from_slice(&result.stderr).unwrap();
    assert_eq!(stderr["failure"]["code"], "input_path_unsafe");
}
