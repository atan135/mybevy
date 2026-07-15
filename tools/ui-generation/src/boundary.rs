use crate::lifecycle::{TaskFailure, TaskFailureKind};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};
use toml::Value;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyBoundaryReport {
    pub project_manifest: PathBuf,
    pub tool_manifest: PathBuf,
    pub project_dependency_graph_excludes_tool: bool,
    pub tool_dependency_graph_reaches_project: bool,
    pub project_lock_excludes_tool_package: bool,
    pub tool_lock_contains_project_package: bool,
    pub crates_are_independent_workspaces: bool,
    pub standalone_preview_target_is_feature_gated: bool,
}

pub fn verify_dependency_boundary(
    repository_root: &Path,
) -> Result<DependencyBoundaryReport, TaskFailure> {
    let repository_root = fs::canonicalize(repository_root).map_err(|error| {
        boundary_failure(format!("repository root cannot be resolved: {error}"))
    })?;
    let project_manifest = canonical_manifest(&repository_root.join("project/Cargo.toml"))?;
    let tool_manifest =
        canonical_manifest(&repository_root.join("tools/ui-generation/Cargo.toml"))?;

    let project_dependency_graph_excludes_tool =
        !manifest_graph_reaches(&project_manifest, &tool_manifest)?;
    let tool_dependency_graph_reaches_project =
        manifest_graph_reaches(&tool_manifest, &project_manifest)?;
    let project_lock_excludes_tool_package = !lock_contains_local_package(
        &project_manifest.with_file_name("Cargo.lock"),
        "ui-generation",
    )?;
    let tool_lock_contains_project_package =
        lock_contains_local_package(&tool_manifest.with_file_name("Cargo.lock"), "project")?;
    let project_workspace = enclosing_workspace_root(&project_manifest)?;
    let tool_workspace = enclosing_workspace_root(&tool_manifest)?;
    let crates_are_independent_workspaces = project_workspace != tool_workspace;
    let standalone_preview_target_is_feature_gated =
        preview_target_is_feature_gated(&parse_toml_file(&project_manifest)?);

    validate_boundary_flags(
        project_dependency_graph_excludes_tool,
        tool_dependency_graph_reaches_project,
        project_lock_excludes_tool_package,
        tool_lock_contains_project_package,
        crates_are_independent_workspaces,
        standalone_preview_target_is_feature_gated,
    )?;

    Ok(DependencyBoundaryReport {
        project_manifest,
        tool_manifest,
        project_dependency_graph_excludes_tool,
        tool_dependency_graph_reaches_project,
        project_lock_excludes_tool_package,
        tool_lock_contains_project_package,
        crates_are_independent_workspaces,
        standalone_preview_target_is_feature_gated,
    })
}

fn manifest_graph_reaches(
    root_manifest: &Path,
    target_manifest: &Path,
) -> Result<bool, TaskFailure> {
    let root_manifest = canonical_manifest(root_manifest)?;
    let target_manifest = canonical_manifest(target_manifest)?;
    let mut pending = vec![root_manifest];
    let mut visited = BTreeSet::new();
    while let Some(manifest) = pending.pop() {
        if manifest == target_manifest {
            return Ok(true);
        }
        if !visited.insert(manifest.clone()) {
            continue;
        }
        let document = parse_toml_file(&manifest)?;
        let workspace_manifest = enclosing_workspace_manifest(&manifest)?;
        let workspace_document = workspace_manifest
            .as_ref()
            .map(|workspace| parse_toml_file(workspace))
            .transpose()?;
        let mut dependencies = dependency_manifest_paths(
            &document,
            &manifest,
            workspace_manifest.as_deref(),
            workspace_document.as_ref(),
        )?;
        if let (Some(workspace_manifest), Some(workspace_document)) =
            (workspace_manifest.as_ref(), workspace_document.as_ref())
        {
            dependencies.extend(patch_manifest_paths(
                workspace_document,
                workspace_manifest,
            )?);
        }
        pending.extend(dependencies);
    }
    Ok(false)
}

fn dependency_manifest_paths(
    document: &Value,
    manifest: &Path,
    workspace_manifest: Option<&Path>,
    workspace_document: Option<&Value>,
) -> Result<Vec<PathBuf>, TaskFailure> {
    let mut dependencies = Vec::new();
    for table_name in ["dependencies", "dev-dependencies", "build-dependencies"] {
        collect_dependency_table(
            document.get(table_name),
            manifest,
            workspace_manifest,
            workspace_document,
            &mut dependencies,
        )?;
    }
    if let Some(targets) = document.get("target").and_then(Value::as_table) {
        for target in targets.values() {
            for table_name in ["dependencies", "dev-dependencies", "build-dependencies"] {
                collect_dependency_table(
                    target.get(table_name),
                    manifest,
                    workspace_manifest,
                    workspace_document,
                    &mut dependencies,
                )?;
            }
        }
    }
    dependencies.extend(patch_manifest_paths(document, manifest)?);
    Ok(dependencies)
}

fn collect_dependency_table(
    dependency_table: Option<&Value>,
    manifest: &Path,
    workspace_manifest: Option<&Path>,
    workspace_document: Option<&Value>,
    output: &mut Vec<PathBuf>,
) -> Result<(), TaskFailure> {
    let Some(dependencies) = dependency_table.and_then(Value::as_table) else {
        return Ok(());
    };
    for (name, specification) in dependencies {
        if let Some(path) = specification.get("path").and_then(Value::as_str) {
            output.push(resolve_dependency_manifest(manifest, path)?);
            continue;
        }
        if specification
            .get("workspace")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            let workspace_manifest = workspace_manifest.ok_or_else(|| {
                boundary_failure(format!(
                    "dependency `{name}` in {} inherits a workspace dependency without an enclosing workspace",
                    manifest.display()
                ))
            })?;
            let workspace_specification = workspace_document
                .and_then(|document| document.get("workspace"))
                .and_then(|workspace| workspace.get("dependencies"))
                .and_then(|dependencies| dependencies.get(name))
                .ok_or_else(|| {
                    boundary_failure(format!(
                        "workspace dependency `{name}` inherited by {} is not declared",
                        manifest.display()
                    ))
                })?;
            if let Some(path) = workspace_specification.get("path").and_then(Value::as_str) {
                output.push(resolve_dependency_manifest(workspace_manifest, path)?);
            }
        }
    }
    Ok(())
}

fn patch_manifest_paths(document: &Value, manifest: &Path) -> Result<Vec<PathBuf>, TaskFailure> {
    let mut paths = Vec::new();
    if let Some(registries) = document.get("patch").and_then(Value::as_table) {
        for registry in registries.values().filter_map(Value::as_table) {
            for specification in registry.values() {
                if let Some(path) = specification.get("path").and_then(Value::as_str) {
                    paths.push(resolve_dependency_manifest(manifest, path)?);
                }
            }
        }
    }
    if let Some(replacements) = document.get("replace").and_then(Value::as_table) {
        for specification in replacements.values() {
            if let Some(path) = specification.get("path").and_then(Value::as_str) {
                paths.push(resolve_dependency_manifest(manifest, path)?);
            }
        }
    }
    Ok(paths)
}

fn resolve_dependency_manifest(
    owner_manifest: &Path,
    dependency_path: &str,
) -> Result<PathBuf, TaskFailure> {
    let directory = owner_manifest.parent().ok_or_else(|| {
        boundary_failure(format!(
            "manifest has no parent directory: {}",
            owner_manifest.display()
        ))
    })?;
    canonical_manifest(&directory.join(dependency_path).join("Cargo.toml"))
}

fn canonical_manifest(manifest: &Path) -> Result<PathBuf, TaskFailure> {
    let manifest = fs::canonicalize(manifest).map_err(|error| {
        boundary_failure(format!(
            "Cargo manifest cannot be resolved at {}: {error}",
            manifest.display()
        ))
    })?;
    if !manifest.is_file() {
        return Err(boundary_failure(format!(
            "Cargo manifest is not a file: {}",
            manifest.display()
        )));
    }
    Ok(manifest)
}

fn parse_toml_file(path: &Path) -> Result<Value, TaskFailure> {
    let source = fs::read_to_string(path).map_err(|error| {
        boundary_failure(format!("cannot read TOML file {}: {error}", path.display()))
    })?;
    toml::from_str(&source).map_err(|error| {
        boundary_failure(format!(
            "cannot parse TOML file {}: {error}",
            path.display()
        ))
    })
}

fn lock_contains_local_package(lock_path: &Path, package_name: &str) -> Result<bool, TaskFailure> {
    let document = parse_toml_file(lock_path)?;
    let packages = document
        .get("package")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            boundary_failure(format!(
                "Cargo lockfile has no package array: {}",
                lock_path.display()
            ))
        })?;
    Ok(packages.iter().any(|package| {
        package.get("name").and_then(Value::as_str) == Some(package_name)
            && package.get("source").is_none()
    }))
}

fn enclosing_workspace_root(manifest: &Path) -> Result<PathBuf, TaskFailure> {
    Ok(enclosing_workspace_manifest(manifest)?
        .and_then(|workspace| workspace.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| manifest.parent().unwrap_or(manifest).to_path_buf()))
}

fn enclosing_workspace_manifest(manifest: &Path) -> Result<Option<PathBuf>, TaskFailure> {
    let Some(mut directory) = manifest.parent() else {
        return Ok(None);
    };
    loop {
        let candidate = directory.join("Cargo.toml");
        if candidate.exists() {
            let document = parse_toml_file(&candidate)?;
            if document.get("workspace").is_some() {
                return canonical_manifest(&candidate).map(Some);
            }
        }
        let Some(parent) = directory.parent() else {
            break;
        };
        directory = parent;
    }
    Ok(None)
}

fn validate_boundary_flags(
    project_dependency_graph_excludes_tool: bool,
    tool_dependency_graph_reaches_project: bool,
    project_lock_excludes_tool_package: bool,
    tool_lock_contains_project_package: bool,
    crates_are_independent_workspaces: bool,
    standalone_preview_target_is_feature_gated: bool,
) -> Result<(), TaskFailure> {
    if project_dependency_graph_excludes_tool
        && tool_dependency_graph_reaches_project
        && project_lock_excludes_tool_package
        && tool_lock_contains_project_package
        && crates_are_independent_workspaces
        && standalone_preview_target_is_feature_gated
    {
        Ok(())
    } else {
        Err(boundary_failure(format!(
            "dependency direction must be ui-generation -> project with independent Cargo roots and a feature-gated preview target (project_graph_excludes_tool={project_dependency_graph_excludes_tool}, tool_graph_reaches_project={tool_dependency_graph_reaches_project}, project_lock_excludes_tool={project_lock_excludes_tool_package}, tool_lock_contains_project={tool_lock_contains_project_package}, independent={crates_are_independent_workspaces}, preview_feature_gated={standalone_preview_target_is_feature_gated})"
        )))
    }
}

fn preview_target_is_feature_gated(document: &Value) -> bool {
    let feature = "ui-document-preview-tool";
    let feature_declared = document
        .get("features")
        .and_then(Value::as_table)
        .is_some_and(|features| features.contains_key(feature));
    let excluded_from_default = document
        .get("features")
        .and_then(|features| features.get("default"))
        .and_then(Value::as_array)
        .is_none_or(|defaults| !defaults.iter().any(|value| value.as_str() == Some(feature)));
    let target_gated = document
        .get("bin")
        .and_then(Value::as_array)
        .is_some_and(|bins| {
            bins.iter().any(|bin| {
                bin.get("name").and_then(Value::as_str) == Some("ui-document-preview")
                    && bin
                        .get("required-features")
                        .and_then(Value::as_array)
                        .is_some_and(|features| {
                            features.len() == 1 && features[0].as_str() == Some(feature)
                        })
            })
        });
    feature_declared && excluded_from_default && target_gated
}

fn boundary_failure(message: impl Into<String>) -> TaskFailure {
    TaskFailure::new(TaskFailureKind::DependencyBoundaryViolation, message, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repository_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap()
    }

    fn write_manifest(directory: &Path, source: &str) -> PathBuf {
        fs::create_dir_all(directory).unwrap();
        let manifest = directory.join("Cargo.toml");
        fs::write(&manifest, source).unwrap();
        manifest
    }

    #[test]
    fn manifest_graph_accepts_only_tool_to_project_direction() {
        let root = tempfile::tempdir().unwrap();
        let project = write_manifest(
            &root.path().join("project"),
            "[package]\nname='project'\nversion='0.1.0'\n",
        );
        let tool = write_manifest(
            &root.path().join("tool"),
            "[package]\nname='ui-generation'\nversion='0.1.0'\n[dependencies]\nproject={path='../project'}\n",
        );
        assert!(manifest_graph_reaches(&tool, &project).unwrap());
        assert!(!manifest_graph_reaches(&project, &tool).unwrap());
    }

    #[test]
    fn indirect_project_dependency_on_tool_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        let project = write_manifest(
            &root.path().join("project"),
            "[package]\nname='project'\nversion='0.1.0'\n[dependencies]\nmiddle={path='../middle'}\n",
        );
        write_manifest(
            &root.path().join("middle"),
            "[package]\nname='middle'\nversion='0.1.0'\n[dependencies]\nui-generation={path='../tool'}\n",
        );
        let tool = write_manifest(
            &root.path().join("tool"),
            "[package]\nname='ui-generation'\nversion='0.1.0'\n",
        );
        let project_dependency_graph_excludes_tool =
            !manifest_graph_reaches(&project, &tool).unwrap();
        assert!(!project_dependency_graph_excludes_tool);
        let failure = validate_boundary_flags(
            project_dependency_graph_excludes_tool,
            true,
            true,
            true,
            true,
            true,
        )
        .unwrap_err();
        assert_eq!(failure.kind(), TaskFailureKind::DependencyBoundaryViolation);
        assert!(
            failure
                .message()
                .contains("project_graph_excludes_tool=false")
        );
    }

    #[test]
    fn lockfile_check_distinguishes_local_and_registry_packages() {
        let root = tempfile::tempdir().unwrap();
        let lock = root.path().join("Cargo.lock");
        fs::write(
            &lock,
            "version = 4\n[[package]]\nname='project'\nversion='0.1.0'\n[[package]]\nname='ui-generation'\nversion='9.0.0'\nsource='registry+https://example.invalid/index'\n",
        )
        .unwrap();
        assert!(lock_contains_local_package(&lock, "project").unwrap());
        assert!(!lock_contains_local_package(&lock, "ui-generation").unwrap());
    }

    #[test]
    fn standalone_preview_target_requires_non_default_feature() {
        let gated: Value = toml::from_str(
            r#"
            [features]
            default = []
            ui-document-preview-tool = []

            [[bin]]
            name = "ui-document-preview"
            required-features = ["ui-document-preview-tool"]
            "#,
        )
        .unwrap();
        assert!(preview_target_is_feature_gated(&gated));

        let mut ungated = gated;
        ungated["bin"][0]
            .as_table_mut()
            .unwrap()
            .remove("required-features");
        assert!(!preview_target_is_feature_gated(&ungated));
    }

    #[test]
    fn summary_generation_outputs_are_git_ignored() {
        let root = repository_root();
        let output = std::process::Command::new("git")
            .args([
                "check-ignore",
                "--quiet",
                "summary/ui-generation/test/input/reference.png",
            ])
            .current_dir(root)
            .status()
            .unwrap();
        assert!(output.success());
    }
}
