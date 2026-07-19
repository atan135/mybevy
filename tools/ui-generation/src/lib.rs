#[cfg(feature = "full")]
pub mod analysis;
#[cfg(feature = "full")]
pub mod asset_strategy;
#[cfg(feature = "full")]
pub mod audit;
#[cfg(feature = "full")]
pub mod boundary;
#[cfg(feature = "full")]
pub mod contract;
pub mod credentials;
#[cfg(feature = "full")]
pub mod directory;
#[cfg(feature = "full")]
pub mod evaluation;
#[cfg(feature = "full")]
pub mod generation;
pub mod lifecycle;
#[cfg(feature = "full")]
pub mod observability;
#[cfg(feature = "full")]
pub mod offline;
#[cfg(feature = "full")]
pub mod planning;
#[cfg(feature = "full")]
pub mod preprocess;
#[cfg(feature = "full")]
pub mod preview;
#[cfg(feature = "full")]
pub mod promotion;
pub mod provider;
pub mod provider_budget;
#[cfg(feature = "full")]
pub mod repair;
#[cfg(feature = "full")]
pub mod run_manifest;
#[cfg(feature = "full")]
pub mod series;
#[cfg(feature = "full")]
pub mod workspace;

#[cfg(feature = "full")]
use contract::{GenerationTask, TaskAssessment, VerifiedReferenceImage};
#[cfg(feature = "full")]
use directory::RunDirectoryPlan;
#[cfg(feature = "full")]
use lifecycle::{CancellationToken, TaskFailure};
#[cfg(feature = "full")]
use serde::Serialize;
#[cfg(feature = "full")]
use std::path::Path;

#[cfg(feature = "full")]
pub const TASK_CONTRACT_VERSION: u32 = 1;

#[cfg(feature = "full")]
#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskInspection {
    pub task: GenerationTask,
    pub assessment: TaskAssessment,
    pub verified_references: Vec<VerifiedReferenceImage>,
    pub directory_plan: RunDirectoryPlan,
    pub ui_document_schema_version: u32,
}

/// Performs the Stage 1 checks without creating a run directory or writing any run artifacts.
#[cfg(feature = "full")]
pub fn inspect_task(
    task_path: &Path,
    repository_root: &Path,
    cancellation: &CancellationToken,
) -> Result<TaskInspection, TaskFailure> {
    cancellation.checkpoint()?;
    let task = GenerationTask::load_json(task_path)?;
    let assessment = task.assess();
    let verified_references = task.verify_reference_files(task_path, cancellation)?;
    let directory_plan = RunDirectoryPlan::plan(repository_root, &task.run_id)?;
    cancellation.checkpoint()?;

    Ok(TaskInspection {
        task,
        assessment,
        verified_references,
        directory_plan,
        ui_document_schema_version:
            project::framework::ui::document::tooling::CURRENT_SCHEMA_VERSION,
    })
}

#[cfg(all(test, feature = "full"))]
mod tests {
    use project::framework::ui::document::tooling;

    #[test]
    fn project_tooling_facade_is_the_document_entry_point() {
        let source = r#"{
            "schema_version": 1,
            "document_id": "tooling.facade",
            "root": { "type": "container", "id": "root.container" }
        }"#;
        let canonical = tooling::canonicalize_json(source).unwrap();
        assert!(tooling::validate_json(&canonical).report.valid);
    }
}
