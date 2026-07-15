pub mod analysis;
pub mod boundary;
pub mod contract;
pub mod credentials;
pub mod directory;
pub mod lifecycle;
pub mod planning;
pub mod preprocess;
pub mod provider;

use contract::{GenerationTask, TaskAssessment, VerifiedReferenceImage};
use directory::RunDirectoryPlan;
use lifecycle::{CancellationToken, TaskFailure};
use serde::Serialize;
use std::path::Path;

pub const TASK_CONTRACT_VERSION: u32 = 1;

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

#[cfg(test)]
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
