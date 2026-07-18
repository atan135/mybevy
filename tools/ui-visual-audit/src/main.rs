use clap::{Parser, Subcommand};
use serde::Serialize;
use std::{io::Write, path::PathBuf};
use ui_visual_audit::{
    ComparisonError, ComparisonErrorResponse, ComparisonRequest, DiffAnalysisRequest,
    ManifestError, NormalizationRequest, analyze_aligned_diff, compare_images,
    load_and_validate_manifest, normalize_and_align,
};

#[derive(Debug, Parser)]
#[command(
    name = "ui-visual-audit",
    version,
    about = "Deterministic UI visual-audit tooling"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Parse and fully validate a reference manifest and every referenced image.
    ValidateManifest {
        #[arg(long)]
        repository_root: PathBuf,
        #[arg(long)]
        manifest: PathBuf,
    },
    /// Compare two PNG/JPEG files with an explicit config and isolated output directory.
    Compare {
        #[arg(long)]
        repository_root: PathBuf,
        #[arg(long, required = true)]
        allowed_input_root: Vec<PathBuf>,
        #[arg(long)]
        allowed_output_root: PathBuf,
        #[arg(long)]
        reference: PathBuf,
        #[arg(long)]
        actual: PathBuf,
        #[arg(long)]
        config: PathBuf,
        #[arg(long)]
        mask: Option<PathBuf>,
        #[arg(long)]
        output_directory: PathBuf,
    },
    /// Normalize, explicitly crop, and deterministically align two PNG/JPEG files.
    NormalizeAlign {
        #[arg(long)]
        repository_root: PathBuf,
        #[arg(long, required = true)]
        allowed_input_root: Vec<PathBuf>,
        #[arg(long)]
        allowed_output_root: PathBuf,
        #[arg(long)]
        reference: PathBuf,
        #[arg(long)]
        actual: PathBuf,
        #[arg(long)]
        normalization_manifest: PathBuf,
        #[arg(long)]
        output_directory: PathBuf,
    },
    /// Analyze two aligned RGBA8/sRGB PNGs and render deterministic diff artifacts.
    AnalyzeDiff {
        #[arg(long)]
        repository_root: PathBuf,
        #[arg(long, required = true)]
        allowed_input_root: Vec<PathBuf>,
        #[arg(long)]
        allowed_output_root: PathBuf,
        #[arg(long)]
        reference: PathBuf,
        #[arg(long)]
        actual: PathBuf,
        #[arg(long)]
        config: PathBuf,
        #[arg(long)]
        output_directory: PathBuf,
    },
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct ValidationSuccess {
    status: &'static str,
    schema_version: u32,
    reference_count: usize,
}

fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) if error.use_stderr() => {
            return exit_with_comparison_error(ComparisonError::cli_arguments_invalid(
                error.to_string(),
            ));
        }
        Err(error) => {
            let _ = error.print();
            return 0;
        }
    };

    match cli.command {
        Command::ValidateManifest {
            repository_root,
            manifest,
        } => match load_and_validate_manifest(&repository_root, &manifest) {
            Ok(validated) => write_stdout_json(&ValidationSuccess {
                status: "valid",
                schema_version: validated.manifest.schema_version,
                reference_count: validated.references.len(),
            }),
            Err(error) => exit_with_manifest_error(error),
        },
        Command::Compare {
            repository_root,
            allowed_input_root,
            allowed_output_root,
            reference,
            actual,
            config,
            mask,
            output_directory,
        } => match compare_images(&ComparisonRequest {
            repository_root,
            allowed_input_roots: allowed_input_root,
            allowed_output_root,
            reference,
            actual,
            config,
            mask,
            output_directory,
        }) {
            Ok(outcome) => match serde_json::to_vec_pretty(&outcome.report) {
                Ok(bytes) => match std::io::stdout().lock().write_all(&bytes) {
                    Ok(()) => outcome.exit_code.as_i32(),
                    Err(error) => exit_with_comparison_error(ComparisonError::internal_failure(
                        format!("comparison report cannot be written to stdout: {error}"),
                    )),
                },
                Err(error) => exit_with_comparison_error(ComparisonError::internal_failure(
                    format!("comparison report cannot be serialized for stdout: {error}"),
                )),
            },
            Err(error) => exit_with_comparison_error(error),
        },
        Command::NormalizeAlign {
            repository_root,
            allowed_input_root,
            allowed_output_root,
            reference,
            actual,
            normalization_manifest,
            output_directory,
        } => match normalize_and_align(&NormalizationRequest {
            repository_root,
            allowed_input_roots: allowed_input_root,
            allowed_output_root,
            reference,
            actual,
            normalization_manifest,
            output_directory,
        }) {
            Ok(outcome) => match serde_json::to_vec_pretty(&outcome.report) {
                Ok(bytes) => match std::io::stdout().lock().write_all(&bytes) {
                    Ok(()) => outcome.exit_code.as_i32(),
                    Err(error) => exit_with_comparison_error(ComparisonError::internal_failure(
                        format!("normalization report cannot be written to stdout: {error}"),
                    )),
                },
                Err(error) => exit_with_comparison_error(ComparisonError::internal_failure(
                    format!("normalization report cannot be serialized for stdout: {error}"),
                )),
            },
            Err(error) => exit_with_comparison_error(error),
        },
        Command::AnalyzeDiff {
            repository_root,
            allowed_input_root,
            allowed_output_root,
            reference,
            actual,
            config,
            output_directory,
        } => match analyze_aligned_diff(&DiffAnalysisRequest {
            repository_root,
            allowed_input_roots: allowed_input_root,
            allowed_output_root,
            reference,
            actual,
            config,
            output_directory,
        }) {
            Ok(outcome) => match serde_json::to_vec_pretty(&outcome.report) {
                Ok(bytes) => match std::io::stdout().lock().write_all(&bytes) {
                    Ok(()) => outcome.exit_code.as_i32(),
                    Err(error) => exit_with_comparison_error(ComparisonError::internal_failure(
                        format!("diff analysis report cannot be written to stdout: {error}"),
                    )),
                },
                Err(error) => exit_with_comparison_error(ComparisonError::internal_failure(
                    format!("diff analysis report cannot be serialized for stdout: {error}"),
                )),
            },
            Err(error) => exit_with_comparison_error(error),
        },
    }
}

fn write_stdout_json(value: &impl Serialize) -> i32 {
    match serde_json::to_vec_pretty(value) {
        Ok(bytes) if std::io::stdout().lock().write_all(&bytes).is_ok() => 0,
        _ => 5,
    }
}

fn exit_with_manifest_error(error: ManifestError) -> i32 {
    if let Ok(bytes) = serde_json::to_vec_pretty(&error) {
        let _ = std::io::stderr().lock().write_all(&bytes);
    }
    2
}

fn exit_with_comparison_error(error: ComparisonError) -> i32 {
    let response = ComparisonErrorResponse::from(&error);
    match serde_json::to_vec_pretty(&response) {
        Ok(bytes) => {
            let _ = std::io::stderr().lock().write_all(&bytes);
        }
        Err(_) => {
            let _ = std::io::stderr().lock().write_all(
                br#"{"schema_version":1,"status":"error","failure":{"failure_type":"internal","code":"internal_failure","message":"error response serialization failed"}}"#,
            );
        }
    }
    error.exit_code().as_i32()
}
