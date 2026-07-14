use clap::{Parser, Subcommand};
use std::path::PathBuf;
use ui_generation::{
    boundary::verify_dependency_boundary, inspect_task, lifecycle::CancellationToken,
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
    /// Verifies that Cargo metadata contains only the allowed tool -> project dependency.
    CheckBoundary {
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
        Command::CheckBoundary { repository_root } => {
            serde_json::to_value(verify_dependency_boundary(&repository_root)?)
                .expect("dependency boundary report is serializable")
        }
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&output).expect("CLI report is serializable")
    );
    Ok(())
}
