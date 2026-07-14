use crate::lifecycle::{TaskFailure, TaskFailureKind};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

const MAX_RUN_ID_LEN: usize = 64;

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct RunId(String);

impl RunId {
    pub fn parse(value: &str) -> Result<Self, TaskFailure> {
        let valid_length = !value.is_empty() && value.len() <= MAX_RUN_ID_LEN;
        let mut chars = value.chars();
        let valid_first = chars
            .next()
            .is_some_and(|character| character.is_ascii_lowercase() || character.is_ascii_digit());
        let valid_rest = chars.all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || character == '-'
                || character == '_'
        });
        let reserved = is_windows_reserved_name(value);
        if !valid_length
            || !valid_first
            || !valid_rest
            || reserved
            || value == "."
            || value == ".."
            || value.contains("..")
        {
            return Err(TaskFailure::invalid(
                "run_id must be 1-64 lowercase ASCII letters, digits, '-' or '_', start with a letter or digit, and not be a reserved path name",
            ));
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_windows_reserved_name(value: &str) -> bool {
    let upper = value.to_ascii_uppercase();
    matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (upper.len() == 4
            && (upper.starts_with("COM") || upper.starts_with("LPT"))
            && upper.as_bytes()[3].is_ascii_digit()
            && upper.as_bytes()[3] != b'0')
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunDirectoryPlan {
    pub run_id: RunId,
    pub root: PathBuf,
    pub input: PathBuf,
    pub analysis: PathBuf,
    pub draft: PathBuf,
    pub assets: PathBuf,
    pub preview: PathBuf,
    pub logs: PathBuf,
    pub manifest: PathBuf,
}

impl RunDirectoryPlan {
    /// Plans the complete run layout. This function deliberately performs no directory creation.
    pub fn plan(repository_root: &Path, run_id: &str) -> Result<Self, TaskFailure> {
        let run_id = RunId::parse(run_id)?;
        let repository_root = fs::canonicalize(repository_root).map_err(|error| {
            TaskFailure::new(
                TaskFailureKind::UnsafeOutputPath,
                format!("repository root cannot be resolved: {error}"),
                Some(repository_root.display().to_string()),
            )
        })?;
        if !repository_root.is_dir() {
            return Err(TaskFailure::new(
                TaskFailureKind::UnsafeOutputPath,
                "repository root is not a directory",
                Some(repository_root.display().to_string()),
            ));
        }

        let generation_root = repository_root.join("summary").join("ui-generation");
        ensure_existing_ancestor_is_contained(&repository_root, &generation_root)?;
        let root = generation_root.join(run_id.as_str());
        if fs::symlink_metadata(&root).is_ok() {
            return Err(TaskFailure::new(
                TaskFailureKind::OutputDirectoryConflict,
                "the requested run output path already exists",
                Some(root.display().to_string()),
            ));
        }
        ensure_lexically_contained(&generation_root, &root)?;

        let plan = Self {
            run_id,
            input: root.join("input"),
            analysis: root.join("analysis"),
            draft: root.join("draft"),
            assets: root.join("assets"),
            preview: root.join("preview"),
            logs: root.join("logs"),
            manifest: root.join("manifest.json"),
            root,
        };
        for path in plan.all_paths() {
            ensure_lexically_contained(&plan.root, path)?;
        }
        Ok(plan)
    }

    pub fn all_paths(&self) -> [&Path; 8] {
        [
            &self.root,
            &self.input,
            &self.analysis,
            &self.draft,
            &self.assets,
            &self.preview,
            &self.logs,
            &self.manifest,
        ]
    }
}

fn ensure_existing_ancestor_is_contained(
    repository_root: &Path,
    candidate: &Path,
) -> Result<(), TaskFailure> {
    let mut ancestor = candidate;
    while !ancestor.exists() {
        ancestor = ancestor.parent().ok_or_else(|| {
            TaskFailure::new(
                TaskFailureKind::UnsafeOutputPath,
                "output path has no existing ancestor",
                Some(candidate.display().to_string()),
            )
        })?;
    }
    let canonical_ancestor = fs::canonicalize(ancestor).map_err(|error| {
        TaskFailure::new(
            TaskFailureKind::UnsafeOutputPath,
            format!("output ancestor cannot be resolved: {error}"),
            Some(ancestor.display().to_string()),
        )
    })?;
    if !canonical_ancestor.starts_with(repository_root) {
        return Err(TaskFailure::new(
            TaskFailureKind::UnsafeOutputPath,
            "output path resolves outside the repository root",
            Some(candidate.display().to_string()),
        ));
    }
    Ok(())
}

fn ensure_lexically_contained(root: &Path, candidate: &Path) -> Result<(), TaskFailure> {
    if candidate.starts_with(root) {
        Ok(())
    } else {
        Err(TaskFailure::new(
            TaskFailureKind::UnsafeOutputPath,
            "planned output escapes the generation root",
            Some(candidate.display().to_string()),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_id_rejects_escape_absolute_separator_and_reserved_names() {
        for invalid in [
            "",
            "../escape",
            "run..escape",
            "a/b",
            "a\\b",
            ".",
            "..",
            "/absolute",
            "C:\\run",
            "UPPER",
            "con",
            "LPT1",
        ] {
            assert!(RunId::parse(invalid).is_err(), "accepted {invalid:?}");
        }
        assert_eq!(
            RunId::parse("gallery-2026_07").unwrap().as_str(),
            "gallery-2026_07"
        );
    }

    #[test]
    fn planning_lists_all_required_paths_without_creating_them() {
        let repository = tempfile::tempdir().unwrap();
        let plan = RunDirectoryPlan::plan(repository.path(), "fixture-run").unwrap();
        assert_eq!(plan.input, plan.root.join("input"));
        assert_eq!(plan.analysis, plan.root.join("analysis"));
        assert_eq!(plan.draft, plan.root.join("draft"));
        assert_eq!(plan.assets, plan.root.join("assets"));
        assert_eq!(plan.preview, plan.root.join("preview"));
        assert_eq!(plan.logs, plan.root.join("logs"));
        assert_eq!(plan.manifest, plan.root.join("manifest.json"));
        assert!(!plan.root.exists());
        assert!(
            plan.all_paths()
                .iter()
                .all(|path| path.starts_with(&plan.root))
        );
    }

    #[test]
    fn existing_run_path_is_a_stable_conflict() {
        let repository = tempfile::tempdir().unwrap();
        let conflict = repository.path().join("summary/ui-generation/existing");
        fs::create_dir_all(&conflict).unwrap();
        let failure = RunDirectoryPlan::plan(repository.path(), "existing").unwrap_err();
        assert_eq!(failure.kind(), TaskFailureKind::OutputDirectoryConflict);
    }
}
