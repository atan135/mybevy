use clap::{Parser, Subcommand};
use std::path::PathBuf;
use ui_generation::{
    audit::{AuditVisualExpectation, parse_page_states, run_document_audit_command},
    boundary::verify_dependency_boundary,
    inspect_task,
    lifecycle::CancellationToken,
    preprocess::preprocess_task,
    preview::{CommandPreviewExecutor, PreviewRunStatus, prepare_preview_command, run_preview},
};

#[derive(Debug, Parser)]
#[command(
    name = "ui-generation",
    about = "Repository-local UiDocument generation tool"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Validates task input, image bytes/hashes, and the run directory plan without writing it.
    InspectTask {
        #[arg(long)]
        task: PathBuf,
        #[arg(long)]
        repository_root: PathBuf,
    },
    /// Safely normalizes task reference images into an ignored run directory and cache.
    PreprocessTask {
        #[arg(long)]
        task: PathBuf,
        #[arg(long)]
        options: Option<PathBuf>,
        #[arg(long)]
        repository_root: PathBuf,
    },
    /// Verifies that Cargo metadata contains only the allowed tool -> project dependency.
    CheckBoundary {
        #[arg(long)]
        repository_root: PathBuf,
    },
    /// Runs the feature-gated standalone declarative preview process for one validated document.
    PreviewDocument {
        #[arg(long)]
        document: PathBuf,
        #[arg(long)]
        output_directory: PathBuf,
        #[arg(long)]
        repository_root: PathBuf,
        #[arg(long, default_value_t = 390)]
        width: u32,
        #[arg(long, default_value_t = 844)]
        height: u32,
    },
    /// Captures the standalone declarative screen for every requested state and audit device.
    AuditDocument {
        #[arg(long)]
        document: PathBuf,
        #[arg(long)]
        output_directory: PathBuf,
        #[arg(long)]
        repository_root: PathBuf,
        /// Comma-separated closed page-state IDs. Defaults to `initial`.
        #[arg(long)]
        states: Option<String>,
        /// Explicitly require these non-initial fixture states to differ visually from initial.
        #[arg(long)]
        require_distinct_from_initial: Option<String>,
    },
}

fn main() {
    if let Err(error) = run() {
        let serialized = serde_json::to_string_pretty(&error).unwrap_or_else(|_| {
            format!(
                r#"{{"code":"{}","message":"{}"}}"#,
                error.code(),
                error.message()
            )
        });
        eprintln!("{serialized}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), ui_generation::lifecycle::TaskFailure> {
    let output = match Cli::parse().command {
        Command::InspectTask {
            task,
            repository_root,
        } => serde_json::to_value(inspect_task(
            &task,
            &repository_root,
            &CancellationToken::default(),
        )?)
        .expect("task inspection is serializable"),
        Command::PreprocessTask {
            task,
            options,
            repository_root,
        } => serde_json::to_value(preprocess_task(
            &task,
            options.as_deref(),
            &repository_root,
            &CancellationToken::default(),
        )?)
        .expect("preprocess result is serializable"),
        Command::CheckBoundary { repository_root } => {
            serde_json::to_value(verify_dependency_boundary(&repository_root)?)
                .expect("dependency boundary report is serializable")
        }
        Command::PreviewDocument {
            document,
            output_directory,
            repository_root,
            width,
            height,
        } => {
            let plan = prepare_preview_command(
                &repository_root,
                &document,
                &output_directory,
                width,
                height,
            )?;
            let result = run_preview(plan, &CommandPreviewExecutor, &CancellationToken::default());
            if result.status == PreviewRunStatus::Failed {
                let failure = result.failure.as_ref();
                return Err(ui_generation::lifecycle::TaskFailure::new(
                    ui_generation::lifecycle::TaskFailureKind::InvalidInput,
                    failure.map_or("standalone preview failed", |failure| {
                        failure.detail.as_str()
                    }),
                    failure.map(|failure| failure.code.clone()),
                ));
            }
            serde_json::to_value(result).expect("preview result is serializable")
        }
        Command::AuditDocument {
            document,
            output_directory,
            repository_root,
            states,
            require_distinct_from_initial,
        } => {
            let states = parse_page_states(states.as_deref())?;
            let visual_expectation = require_distinct_from_initial.map_or_else(
                || Ok(AuditVisualExpectation::default()),
                |input| {
                    AuditVisualExpectation::distinct_from_initial(parse_page_states(Some(&input))?)
                },
            )?;
            let result = run_document_audit_command(
                &repository_root,
                &document,
                &output_directory,
                &states,
                &visual_expectation,
            )?;
            if matches!(
                result.status,
                ui_generation::audit::AuditMatrixStatus::Failed
            ) {
                return Err(ui_generation::lifecycle::TaskFailure::new(
                    ui_generation::lifecycle::TaskFailureKind::InvalidInput,
                    "one or more document audit captures failed",
                    Some(result.manifest_path.display().to_string()),
                ));
            }
            serde_json::to_value(result).expect("audit result is serializable")
        }
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&output).expect("CLI report is serializable")
    );
    Ok(())
}
