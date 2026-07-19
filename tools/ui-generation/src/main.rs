use clap::{Parser, Subcommand};
use std::path::PathBuf;
use ui_generation::{
    audit::{AuditVisualExpectation, parse_page_states, run_document_audit_command},
    boundary::verify_dependency_boundary,
    closed_loop_fix_plan::{
        FixPlanPolicy, create_closed_loop_fix_plan, load_closed_loop_audit,
        write_closed_loop_fix_plan,
    },
    closed_loop_generation::{GenerationMode, run_closed_loop_generation},
    evaluation::run_fixture_evaluation,
    inspect_task,
    lifecycle::CancellationToken,
    offline::run_offline_fixture_generation,
    preprocess::preprocess_task,
    preview::{CommandPreviewExecutor, PreviewRunStatus, prepare_preview_command, run_preview},
    promotion::{
        create_promotion_decision_template, create_promotion_plan, promote,
        record_promotion_decisions,
    },
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
    /// Runs the repository-owned offline evaluation corpus and emits aggregate, redacted metrics.
    EvaluateFixtures {
        #[arg(long)]
        catalog: PathBuf,
        #[arg(long)]
        repository_root: PathBuf,
    },
    /// Runs the complete repository-owned offline fixture path and seals a previewed draft run.
    GenerateFixture {
        #[arg(long)]
        task: PathBuf,
        #[arg(long)]
        options: Option<PathBuf>,
        #[arg(long)]
        repository_root: PathBuf,
        #[arg(long)]
        document_id: String,
    },
    /// Runs a bounded closed-loop generation mode without exposing provider protocol to scripts.
    ClosedLoopGenerate {
        #[arg(long, value_parser = parse_generation_mode, default_value = "off")]
        mode: GenerationMode,
        #[arg(long)]
        task: PathBuf,
        #[arg(long)]
        options: Option<PathBuf>,
        #[arg(long)]
        repository_root: PathBuf,
        #[arg(long)]
        document_id: String,
        /// Environment variable name only. Its credential value is never accepted as an argument.
        #[arg(long)]
        provider_credential_environment: Option<String>,
    },
    /// Builds a bounded, non-applying repair plan from a Stage 4 closed-loop audit report.
    ClosedLoopPlan {
        #[arg(long)]
        audit: PathBuf,
        #[arg(long)]
        output_directory: PathBuf,
        /// Repeat to replace the draft/assets default modification roots.
        #[arg(long = "allowed-root")]
        allowed_roots: Vec<String>,
        /// A group ID whose multi-page issue is known to need an unsupported protocol capability.
        #[arg(long = "protocol-limitation")]
        protocol_limitations: Vec<String>,
    },
    /// Emits the small, high-impact decision template bound to a committed generation run.
    PromotionDecisions {
        #[arg(long)]
        run_id: String,
        #[arg(long)]
        repository_root: PathBuf,
    },
    /// Validates and append-only records explicit human decisions for a committed generation run.
    RecordPromotionDecisions {
        #[arg(long)]
        decisions: PathBuf,
        #[arg(long)]
        repository_root: PathBuf,
    },
    /// Emits the exact no-write promotion plan. Its hash is required by `promote`.
    PromotionPlan {
        #[arg(long)]
        run_id: String,
        #[arg(long)]
        owner: String,
        #[arg(long)]
        route: String,
        #[arg(long)]
        repository_root: PathBuf,
    },
    /// The only tool command allowed to write approved documents and explicitly promoted assets.
    Promote {
        #[arg(long)]
        run_id: String,
        #[arg(long)]
        owner: String,
        #[arg(long)]
        route: String,
        /// Must exactly equal the `plan_sha256` emitted by `promotion-plan`.
        #[arg(long)]
        confirm_plan: String,
        #[arg(long)]
        repository_root: PathBuf,
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
        Command::EvaluateFixtures {
            catalog,
            repository_root,
        } => serde_json::to_value(run_fixture_evaluation(&repository_root, &catalog)?)
            .expect("evaluation report is serializable"),
        Command::GenerateFixture {
            task,
            options,
            repository_root,
            document_id,
        } => serde_json::to_value(run_offline_fixture_generation(
            &task,
            options.as_deref(),
            &repository_root,
            &document_id,
            &CancellationToken::default(),
        )?)
        .expect("offline fixture run result is serializable"),
        Command::ClosedLoopGenerate {
            mode,
            task,
            options,
            repository_root,
            document_id,
            provider_credential_environment,
        } => serde_json::to_value(run_closed_loop_generation(
            mode,
            &task,
            options.as_deref(),
            &repository_root,
            &document_id,
            provider_credential_environment.as_deref(),
            &CancellationToken::default(),
        )?)
        .expect("closed-loop generation result is serializable"),
        Command::ClosedLoopPlan {
            audit,
            output_directory,
            allowed_roots,
            protocol_limitations,
        } => {
            let audit = load_closed_loop_audit(&audit)?;
            let mut policy = FixPlanPolicy::default();
            if !allowed_roots.is_empty() {
                policy.allowed_roots = allowed_roots;
            }
            policy.protocol_limitations = protocol_limitations.into_iter().collect();
            let plan = create_closed_loop_fix_plan(&audit, &policy)?;
            serde_json::to_value(write_closed_loop_fix_plan(&plan, &output_directory)?)
                .expect("closed-loop fix plan output is serializable")
        }
        Command::PromotionDecisions {
            run_id,
            repository_root,
        } => serde_json::to_value(create_promotion_decision_template(
            &repository_root,
            &run_id,
        )?)
        .expect("promotion decision template is serializable"),
        Command::RecordPromotionDecisions {
            decisions,
            repository_root,
        } => serde_json::to_value(record_promotion_decisions(&repository_root, &decisions)?)
            .expect("promotion decision record is serializable"),
        Command::PromotionPlan {
            run_id,
            owner,
            route,
            repository_root,
        } => serde_json::to_value(create_promotion_plan(
            &repository_root,
            &run_id,
            &owner,
            &route,
        )?)
        .expect("promotion plan is serializable"),
        Command::Promote {
            run_id,
            owner,
            route,
            confirm_plan,
            repository_root,
        } => serde_json::to_value(promote(
            &repository_root,
            &run_id,
            &owner,
            &route,
            &confirm_plan,
        )?)
        .expect("promotion result is serializable"),
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&output).expect("CLI report is serializable")
    );
    Ok(())
}

fn parse_generation_mode(value: &str) -> Result<GenerationMode, String> {
    match value.to_ascii_lowercase().as_str() {
        "off" => Ok(GenerationMode::Off),
        "fixture" => Ok(GenerationMode::Fixture),
        "plan" => Ok(GenerationMode::Plan),
        "provider" => Ok(GenerationMode::Provider),
        _ => Err("generation mode must be Off, Fixture, Plan, or Provider".to_owned()),
    }
}
