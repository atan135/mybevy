use clap::{Parser, Subcommand};
use serde::Serialize;
use std::path::PathBuf;
use ui_visual_audit::{ManifestError, load_and_validate_manifest};

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
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct ValidationSuccess {
    status: &'static str,
    schema_version: u32,
    reference_count: usize,
}

fn main() {
    let result = match Cli::parse().command {
        Command::ValidateManifest {
            repository_root,
            manifest,
        } => load_and_validate_manifest(&repository_root, &manifest).map(|validated| {
            ValidationSuccess {
                status: "valid",
                schema_version: validated.manifest.schema_version,
                reference_count: validated.references.len(),
            }
        }),
    };

    match result {
        Ok(report) => println!("{}", serde_json::to_string_pretty(&report).unwrap()),
        Err(error) => exit_with_error(error),
    }
}

fn exit_with_error(error: ManifestError) -> ! {
    eprintln!("{}", serde_json::to_string_pretty(&error).unwrap());
    std::process::exit(2);
}
