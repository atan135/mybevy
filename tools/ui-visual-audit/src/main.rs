use clap::{Parser, Subcommand};
use serde::Serialize;
use std::{io::Write, path::PathBuf};
use ui_visual_audit::{
    AiAnalysisRequest, BaselineApplyRequest, BaselinePlanRequest, BaselineRerunVerificationRequest,
    ComparisonError, ComparisonErrorResponse, ComparisonRequest, DiffAnalysisRequest, GateRequest,
    ManifestError, NormalizationRequest, RegionAuditRequest, ReportBuildRequest,
    SemanticAuditRequest, analyze_aligned_diff, analyze_with_ai, apply_baseline_update,
    audit_regions, audit_semantics, build_comparison_report, compare_images, evaluate_visual_gate,
    load_and_validate_manifest, normalize_and_align, plan_baseline_update, verify_baseline_rerun,
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
    /// Apply explicit include/exclude regions and local weighted rules to aligned inputs.
    AuditRegions {
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
        diff_config: PathBuf,
        #[arg(long)]
        region_config: PathBuf,
        #[arg(long)]
        normalization_report: PathBuf,
        #[arg(long)]
        output_directory: PathBuf,
    },
    /// Audit a captured runtime semantic tree without consuming visual similarity scores.
    AuditSemantics {
        #[arg(long)]
        repository_root: PathBuf,
        #[arg(long, required = true)]
        allowed_input_root: Vec<PathBuf>,
        #[arg(long)]
        allowed_output_root: PathBuf,
        #[arg(long)]
        metadata: PathBuf,
        #[arg(long)]
        config: PathBuf,
        #[arg(long)]
        output_directory: PathBuf,
    },
    /// Run an explicitly configured fixture, mock, or online AI visual-analysis provider.
    AnalyzeAi {
        #[arg(long)]
        repository_root: PathBuf,
        #[arg(long, required = true)]
        allowed_input_root: Vec<PathBuf>,
        #[arg(long)]
        allowed_output_root: PathBuf,
        #[arg(long)]
        bundle: PathBuf,
        #[arg(long)]
        config: PathBuf,
        #[arg(long)]
        output_directory: PathBuf,
    },
    /// Merge deterministic reports and optional AI issues into the strict four-state gate.
    EvaluateGate {
        #[arg(long)]
        repository_root: PathBuf,
        #[arg(long, required = true)]
        allowed_input_root: Vec<PathBuf>,
        #[arg(long)]
        allowed_output_root: PathBuf,
        #[arg(long)]
        bundle: PathBuf,
        #[arg(long)]
        config: PathBuf,
        #[arg(long)]
        output_directory: PathBuf,
    },
    /// Validate a strict comparison bundle and render machine/human reports.
    BuildReport {
        #[arg(long)]
        repository_root: PathBuf,
        #[arg(long, required = true)]
        allowed_input_root: Vec<PathBuf>,
        #[arg(long)]
        allowed_output_root: PathBuf,
        #[arg(long)]
        bundle: PathBuf,
        #[arg(long)]
        output_directory: PathBuf,
    },
    /// Create an immutable baseline update plan; never changes the baseline.
    PlanBaselineUpdate {
        #[arg(long)]
        repository_root: PathBuf,
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        reference_id: String,
        #[arg(long)]
        new_image: PathBuf,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        metrics_before: PathBuf,
        #[arg(long)]
        metrics_after: PathBuf,
        #[arg(long)]
        allowed_output_root: PathBuf,
        #[arg(long)]
        output_directory: PathBuf,
    },
    /// Apply a non-stale baseline plan with a separate explicit human approval record.
    ApplyBaselineUpdate {
        #[arg(long)]
        repository_root: PathBuf,
        #[arg(long)]
        plan: PathBuf,
        #[arg(long)]
        approval: PathBuf,
        #[arg(long)]
        allowed_output_root: PathBuf,
        #[arg(long)]
        output_directory: PathBuf,
    },
    /// Prove every device/state related to an updated baseline was rerun.
    VerifyBaselineRerun {
        #[arg(long)]
        repository_root: PathBuf,
        #[arg(long)]
        receipt: PathBuf,
        #[arg(long)]
        comparison_result: PathBuf,
        #[arg(long)]
        allowed_output_root: PathBuf,
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
        Command::AuditRegions {
            repository_root,
            allowed_input_root,
            allowed_output_root,
            reference,
            actual,
            diff_config,
            region_config,
            normalization_report,
            output_directory,
        } => match audit_regions(&RegionAuditRequest {
            repository_root,
            allowed_input_roots: allowed_input_root,
            allowed_output_root,
            reference,
            actual,
            diff_config,
            region_config,
            normalization_report,
            output_directory,
        }) {
            Ok(outcome) => match serde_json::to_vec_pretty(&outcome.report) {
                Ok(bytes) => match std::io::stdout().lock().write_all(&bytes) {
                    Ok(()) => outcome.exit_code.as_i32(),
                    Err(error) => exit_with_comparison_error(ComparisonError::internal_failure(
                        format!("region audit report cannot be written to stdout: {error}"),
                    )),
                },
                Err(error) => exit_with_comparison_error(ComparisonError::internal_failure(
                    format!("region audit report cannot be serialized for stdout: {error}"),
                )),
            },
            Err(error) => exit_with_comparison_error(error),
        },
        Command::AuditSemantics {
            repository_root,
            allowed_input_root,
            allowed_output_root,
            metadata,
            config,
            output_directory,
        } => match audit_semantics(&SemanticAuditRequest {
            repository_root,
            allowed_input_roots: allowed_input_root,
            allowed_output_root,
            metadata,
            config,
            output_directory,
        }) {
            Ok(outcome) => match serde_json::to_vec_pretty(&outcome.report) {
                Ok(bytes) => match std::io::stdout().lock().write_all(&bytes) {
                    Ok(()) => outcome.exit_code.as_i32(),
                    Err(error) => exit_with_comparison_error(ComparisonError::internal_failure(
                        format!("semantic audit report cannot be written to stdout: {error}"),
                    )),
                },
                Err(error) => exit_with_comparison_error(ComparisonError::internal_failure(
                    format!("semantic audit report cannot be serialized for stdout: {error}"),
                )),
            },
            Err(error) => exit_with_comparison_error(error),
        },
        Command::AnalyzeAi {
            repository_root,
            allowed_input_root,
            allowed_output_root,
            bundle,
            config,
            output_directory,
        } => match analyze_with_ai(&AiAnalysisRequest {
            repository_root,
            allowed_input_roots: allowed_input_root,
            allowed_output_root,
            bundle,
            config,
            output_directory,
        }) {
            Ok(outcome) => match serde_json::to_vec_pretty(&outcome.report) {
                Ok(bytes) => match std::io::stdout().lock().write_all(&bytes) {
                    Ok(()) => outcome.exit_code.as_i32(),
                    Err(error) => exit_with_comparison_error(ComparisonError::internal_failure(
                        format!("AI analysis report cannot be written to stdout: {error}"),
                    )),
                },
                Err(error) => exit_with_comparison_error(ComparisonError::internal_failure(
                    format!("AI analysis report cannot be serialized for stdout: {error}"),
                )),
            },
            Err(error) => exit_with_comparison_error(error),
        },
        Command::EvaluateGate {
            repository_root,
            allowed_input_root,
            allowed_output_root,
            bundle,
            config,
            output_directory,
        } => match evaluate_visual_gate(&GateRequest {
            repository_root,
            allowed_input_roots: allowed_input_root,
            allowed_output_root,
            bundle,
            config,
            output_directory,
        }) {
            Ok(outcome) => match serde_json::to_vec_pretty(&outcome.report) {
                Ok(bytes) => match std::io::stdout().lock().write_all(&bytes) {
                    Ok(()) => outcome.exit_code.as_i32(),
                    Err(error) => exit_with_comparison_error(ComparisonError::internal_failure(
                        format!("visual gate report cannot be written to stdout: {error}"),
                    )),
                },
                Err(error) => exit_with_comparison_error(ComparisonError::internal_failure(
                    format!("visual gate report cannot be serialized for stdout: {error}"),
                )),
            },
            Err(error) => exit_with_comparison_error(error),
        },
        Command::BuildReport {
            repository_root,
            allowed_input_root,
            allowed_output_root,
            bundle,
            output_directory,
        } => match build_comparison_report(&ReportBuildRequest {
            repository_root,
            allowed_input_roots: allowed_input_root,
            allowed_output_root,
            bundle,
            output_directory,
        }) {
            Ok(result) => write_stdout_json(&result),
            Err(error) => exit_with_comparison_error(error),
        },
        Command::PlanBaselineUpdate {
            repository_root,
            manifest,
            reference_id,
            new_image,
            reason,
            metrics_before,
            metrics_after,
            allowed_output_root,
            output_directory,
        } => match plan_baseline_update(&BaselinePlanRequest {
            repository_root,
            manifest,
            reference_id,
            new_image,
            reason,
            metrics_before,
            metrics_after,
            allowed_output_root,
            output_directory,
        }) {
            Ok(plan) => write_stdout_json(&plan),
            Err(error) => exit_with_comparison_error(error),
        },
        Command::ApplyBaselineUpdate {
            repository_root,
            plan,
            approval,
            allowed_output_root,
            output_directory,
        } => match apply_baseline_update(&BaselineApplyRequest {
            repository_root,
            plan,
            approval,
            allowed_output_root,
            output_directory,
        }) {
            Ok(receipt) => write_stdout_json(&receipt),
            Err(error) => exit_with_comparison_error(error),
        },
        Command::VerifyBaselineRerun {
            repository_root,
            receipt,
            comparison_result,
            allowed_output_root,
            output_directory,
        } => match verify_baseline_rerun(&BaselineRerunVerificationRequest {
            repository_root,
            receipt,
            comparison_result,
            allowed_output_root,
            output_directory,
        }) {
            Ok(verification) => write_stdout_json(&verification),
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
