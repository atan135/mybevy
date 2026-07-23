//! Closed-loop generation entry point used by the audit runner.
//!
//! This module deliberately owns generation-mode selection and provider interaction. The
//! PowerShell runner only passes bounded file paths and records the returned JSON; it never
//! reconstructs prompts, model output envelopes, or provider credentials.

use crate::{
    contract::GenerationTask,
    credentials::{CredentialLocator, CredentialResolver},
    directory::RunId,
    lifecycle::{CancellationToken, TaskFailure, TaskFailureKind},
    offline::{OfflineFixtureProfile, OfflineFixtureRunResult, run_offline_fixture_generation},
    run_manifest::{
        ArtifactLink, ClosedLoopArtifactLinks, ClosedLoopBudgetConfiguration,
        ClosedLoopRunManifest, ClosedLoopRunProvenance, ClosedLoopRunState, ClosedLoopViewport,
    },
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub const CLOSED_LOOP_GENERATION_PROTOCOL_VERSION: u32 = 1;
pub const DEFAULT_PROVIDER_CREDENTIAL_ENVIRONMENT: &str = "UI_GENERATION_PROVIDER_TOKEN";

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationMode {
    #[default]
    Off,
    Fixture,
    Plan,
    Provider,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DraftAuditRegistration {
    pub runtime: String,
    pub screen: String,
    pub device: String,
    pub states: Vec<String>,
    pub document_path: PathBuf,
}

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClosedLoopGenerationResult {
    pub protocol_version: u32,
    pub mode: GenerationMode,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed_loop_manifest: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generated_document: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_map: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation_report: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview_screenshot: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audit_registration: Option<DraftAuditRegistration>,
}

/// Executes one bounded generation-mode action. `Provider` intentionally fails closed until an
/// approved provider adapter is registered in the tool; the credential lookup still happens here
/// so callers receive the shared, redacted failure taxonomy instead of a PowerShell error.
pub fn run_closed_loop_generation(
    mode: GenerationMode,
    task_path: &Path,
    preprocess_options_path: Option<&Path>,
    repository_root: &Path,
    document_id: &str,
    provider_credential_environment: Option<&str>,
    cancellation: &CancellationToken,
) -> Result<ClosedLoopGenerationResult, TaskFailure> {
    cancellation.checkpoint()?;
    match mode {
        GenerationMode::Off => Ok(ClosedLoopGenerationResult {
            protocol_version: CLOSED_LOOP_GENERATION_PROTOCOL_VERSION,
            mode,
            status: "disabled".to_owned(),
            run_id: None,
            document_id: None,
            closed_loop_manifest: None,
            generated_document: None,
            source_map: None,
            validation_report: None,
            preview_screenshot: None,
            audit_registration: None,
        }),
        GenerationMode::Plan => {
            // Planning verifies the immutable task and reference bytes but deliberately does not
            // reserve an output directory. This keeps a historical run inspectable even after
            // its artifact directory has been sealed.
            let task = GenerationTask::load_json(task_path)?;
            task.verify_reference_files(task_path, cancellation)?;
            let _ = repository_root;
            Ok(ClosedLoopGenerationResult {
                protocol_version: CLOSED_LOOP_GENERATION_PROTOCOL_VERSION,
                mode,
                status: "planned".to_owned(),
                run_id: Some(task.run_id),
                document_id: Some(document_id.to_owned()),
                closed_loop_manifest: None,
                generated_document: None,
                source_map: None,
                validation_report: None,
                preview_screenshot: None,
                audit_registration: None,
            })
        }
        GenerationMode::Provider => {
            let environment = provider_credential_environment
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(DEFAULT_PROVIDER_CREDENTIAL_ENVIRONMENT);
            let locator = CredentialLocator::new(Some(environment), None::<String>)?;
            let _credential = CredentialResolver::environment_only().resolve(&locator)?;
            Err(provider_adapter_unavailable())
        }
        GenerationMode::Fixture => {
            let fixture = run_offline_fixture_generation(
                task_path,
                preprocess_options_path,
                repository_root,
                document_id,
                OfflineFixtureProfile::Regular,
                cancellation,
            );
            match fixture {
                Ok(fixture) => {
                    fixture_result_to_closed_loop(mode, task_path, document_id, &fixture)
                }
                Err(failure) => {
                    if failure.kind() == TaskFailureKind::PreviewFailed {
                        write_fixture_preview_failure_manifest(
                            repository_root,
                            task_path,
                            &failure,
                        )?;
                    }
                    Err(failure)
                }
            }
        }
    }
}

fn provider_adapter_unavailable() -> TaskFailure {
    TaskFailure::new(
        TaskFailureKind::ProviderNotFound,
        "no approved online UI generation provider is registered; use Fixture or add a repository-owned provider adapter",
        None,
    )
}

fn fixture_result_to_closed_loop(
    mode: GenerationMode,
    task_path: &Path,
    document_id: &str,
    fixture: &OfflineFixtureRunResult,
) -> Result<ClosedLoopGenerationResult, TaskFailure> {
    let task = GenerationTask::load_json(task_path)?;
    let viewport = target_viewport(&task)?;
    let run_root = &fixture.run_root;
    let artifacts = fixture_artifacts(run_root, true)?;
    let mut manifest = new_fixture_manifest(&task, artifacts)?;
    advance_to_previewing(&mut manifest)?;
    manifest.transition(
        ClosedLoopRunState::Auditing,
        unix_millis()?,
        cache_key(&fixture.run_id, "auditing"),
    )?;
    let manifest_path = run_root.join("closed-loop-manifest.json");
    manifest.write_new(&manifest_path)?;

    Ok(ClosedLoopGenerationResult {
        protocol_version: CLOSED_LOOP_GENERATION_PROTOCOL_VERSION,
        mode,
        status: "ready_for_audit".to_owned(),
        run_id: Some(fixture.run_id.clone()),
        document_id: Some(document_id.to_owned()),
        closed_loop_manifest: Some(manifest_path),
        generated_document: Some(fixture.generated_document.clone()),
        source_map: Some(fixture.source_map.clone()),
        validation_report: Some(fixture.validation_report.clone()),
        preview_screenshot: Some(fixture.preview_screenshot.clone()),
        audit_registration: Some(DraftAuditRegistration {
            runtime: "standalone_ui_document".to_owned(),
            screen: "generated_draft".to_owned(),
            device: format!(
                "{}x{}@{}",
                viewport.logical_width, viewport.logical_height, viewport.device_scale
            ),
            states: vec!["initial".to_owned()],
            document_path: fixture.generated_document.clone(),
        }),
    })
}

/// The Stage 3 fixture runner seals its bundle before it returns a preview failure. Preserve that
/// verified generation evidence in a terminal closed-loop manifest so a timeout cannot look like
/// a completed audit-ready run.
fn write_fixture_preview_failure_manifest(
    repository_root: &Path,
    task_path: &Path,
    failure: &TaskFailure,
) -> Result<(), TaskFailure> {
    let task = GenerationTask::load_json(task_path)?;
    let run_id = RunId::parse(&task.run_id)?;
    let repository_root = fs::canonicalize(repository_root)
        .map_err(|_| TaskFailure::invalid("closed-loop repository root cannot be resolved"))?;
    let run_root = repository_root
        .join("summary/ui-generation")
        .join(run_id.as_str());
    let artifacts = fixture_artifacts(&run_root, false)?;
    let mut manifest = new_fixture_manifest(&task, artifacts)?;
    advance_to_previewing(&mut manifest)?;
    manifest.fail(unix_millis()?, failure.clone())?;
    manifest.write_new(&run_root.join("closed-loop-manifest.json"))
}

fn target_viewport(task: &GenerationTask) -> Result<crate::contract::TargetViewport, TaskFailure> {
    task.target_viewport.ok_or_else(|| {
        TaskFailure::new(
            TaskFailureKind::TargetViewportMissing,
            "closed-loop fixture generation requires a target viewport",
            None,
        )
    })
}

fn new_fixture_manifest(
    task: &GenerationTask,
    artifacts: ClosedLoopArtifactLinks,
) -> Result<ClosedLoopRunManifest, TaskFailure> {
    let viewport = target_viewport(task)?;
    let provenance = ClosedLoopRunProvenance {
        tool_version: env!("CARGO_PKG_VERSION").to_owned(),
        source_commit: source_commit(),
        model_id: "fixture-generation-v1".to_owned(),
        prompt_version: "ui-document-fixture-v1".to_owned(),
        schema_id: "ui-document-generation".to_owned(),
        schema_version: 1,
        algorithm_version: "offline-fixture-v1".to_owned(),
        viewport: ClosedLoopViewport {
            logical_width: viewport_dimension(f64::from(viewport.logical_width), "logical_width")?,
            logical_height: viewport_dimension(
                f64::from(viewport.logical_height),
                "logical_height",
            )?,
            device_scale_milli: scale_milli(f64::from(viewport.device_scale))?,
        },
        theme_id: "default".to_owned(),
        locale: "zh_cn".to_owned(),
        budget: ClosedLoopBudgetConfiguration {
            max_provider_calls: 6,
            max_elapsed_ms: 5 * 60 * 1000,
            max_images: 12,
            max_input_units: 1_000_000,
            max_output_units: 250_000,
            max_estimated_cost_microunits: 10_000_000,
        },
    };
    let created_at = unix_millis()?;
    ClosedLoopRunManifest::create(
        &task.run_id,
        created_at,
        provenance,
        artifacts,
        cache_key(&task.run_id, "created"),
    )
}

fn advance_to_previewing(manifest: &mut ClosedLoopRunManifest) -> Result<(), TaskFailure> {
    let run_id = manifest.run_id.clone();
    manifest.transition(
        ClosedLoopRunState::Preparing,
        unix_millis()?,
        cache_key(&run_id, "preparing"),
    )?;
    manifest.transition(
        ClosedLoopRunState::Generating,
        unix_millis()?,
        cache_key(&run_id, "generating"),
    )?;
    manifest.transition(
        ClosedLoopRunState::Validating,
        unix_millis()?,
        cache_key(&run_id, "validating"),
    )?;
    manifest.transition(
        ClosedLoopRunState::Previewing,
        unix_millis()?,
        cache_key(&run_id, "previewing"),
    )
}

fn fixture_artifacts(
    run_root: &Path,
    require_preview: bool,
) -> Result<ClosedLoopArtifactLinks, TaskFailure> {
    let preview_path = run_root.join("preview/process/preview.png");
    let preview = if preview_path.is_file() {
        Some(artifact_link(run_root, &preview_path)?)
    } else if require_preview {
        return Err(TaskFailure::new(
            TaskFailureKind::ArtifactMissing,
            "fixture generation completed without a preview screenshot",
            Some(preview_path.display().to_string()),
        ));
    } else {
        None
    };
    Ok(ClosedLoopArtifactLinks {
        generation_input: artifact_link(run_root, &run_root.join("input/generation-task.json"))?,
        reference_manifest: artifact_link(
            run_root,
            &run_root.join("input/preprocessed/manifest.json"),
        )?,
        generation_result: Some(artifact_link(
            run_root,
            &run_root.join("logs/offline-run.json"),
        )?),
        provider_metadata: Some(artifact_link(
            run_root,
            &run_root.join("logs/generation-trace.json"),
        )?),
        validation_report: Some(artifact_link(
            run_root,
            &run_root.join("draft/validation-report.json"),
        )?),
        source_map: Some(artifact_link(
            run_root,
            &run_root.join("draft/source-map.json"),
        )?),
        draft_assets_manifest: Some(artifact_link(
            run_root,
            &run_root.join("draft/assets-manifest.json"),
        )?),
        ui_document: Some(artifact_link(
            run_root,
            &run_root.join("draft/generated-document.json"),
        )?),
        assets: Vec::new(),
        preview,
        comparison: None,
        analysis: None,
        fix: None,
        approval: None,
    })
}

fn artifact_link(run_root: &Path, path: &Path) -> Result<ArtifactLink, TaskFailure> {
    let bytes = fs::read(path).map_err(|_| {
        TaskFailure::new(
            TaskFailureKind::ArtifactMissing,
            "closed-loop generation evidence artifact is missing",
            Some(path.display().to_string()),
        )
    })?;
    let relative = path.strip_prefix(run_root).map_err(|_| {
        TaskFailure::new(
            TaskFailureKind::UnsafeOutputPath,
            "closed-loop generation evidence escapes the run root",
            Some(path.display().to_string()),
        )
    })?;
    ArtifactLink::new(
        relative.to_string_lossy().replace('\\', "/"),
        format!("{:x}", Sha256::digest(&bytes)),
        u64::try_from(bytes.len()).map_err(|_| TaskFailure::invalid("artifact size overflow"))?,
    )
}

fn viewport_dimension(value: f64, field: &str) -> Result<u32, TaskFailure> {
    if !value.is_finite() || value.fract() != 0.0 || !(64.0..=4096.0).contains(&value) {
        return Err(TaskFailure::invalid(format!(
            "closed-loop {field} must be an integer in 64..=4096"
        )));
    }
    Ok(value as u32)
}

fn scale_milli(value: f64) -> Result<u32, TaskFailure> {
    let scaled = value * 1000.0;
    if !scaled.is_finite()
        || scaled.fract().abs() > f64::EPSILON
        || !(1.0..=32_000.0).contains(&scaled)
    {
        return Err(TaskFailure::invalid(
            "closed-loop device scale must convert to 1..=32000 milli-units",
        ));
    }
    Ok(scaled as u32)
}

fn unix_millis() -> Result<u64, TaskFailure> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| TaskFailure::invalid("system clock predates the Unix epoch"))
        .and_then(|duration| {
            u64::try_from(duration.as_millis())
                .map_err(|_| TaskFailure::invalid("system clock exceeds manifest timestamp range"))
        })
}

fn cache_key(run_id: &str, stage: &str) -> String {
    format!(
        "{:x}",
        Sha256::digest(format!("{run_id}:{stage}").as_bytes())
    )
}

fn source_commit() -> String {
    // The CLI intentionally avoids invoking a shell. The audit runner already records the real
    // commit; this non-empty value keeps the closed manifest valid when the tool is used alone.
    std::env::var("MYBEVY_UI_AUDIT_GIT_COMMIT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "unknown".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::Path};

    #[test]
    fn off_mode_has_no_side_effects_or_provider_requirements() {
        let result = run_closed_loop_generation(
            GenerationMode::Off,
            Path::new("missing-task.json"),
            None,
            Path::new("missing-root"),
            "unused.document",
            None,
            &CancellationToken::default(),
        )
        .unwrap();
        assert_eq!(result.status, "disabled");
        assert!(result.closed_loop_manifest.is_none());
    }

    #[test]
    fn provider_mode_reports_missing_credentials_without_echoing_locator() {
        let failure = run_closed_loop_generation(
            GenerationMode::Provider,
            Path::new("unused-task.json"),
            None,
            Path::new("unused-root"),
            "unused.document",
            Some("UI_GENERATION_TEST_PROVIDER_TOKEN_THAT_MUST_NOT_EXIST"),
            &CancellationToken::default(),
        )
        .unwrap_err();
        assert_eq!(failure.kind(), TaskFailureKind::CredentialUnavailable);
        assert!(!failure.message().contains("TEST_PROVIDER_TOKEN"));
    }

    #[test]
    fn viewport_and_scale_conversion_fail_closed() {
        assert!(viewport_dimension(63.0, "width").is_err());
        assert!(viewport_dimension(390.5, "width").is_err());
        assert_eq!(scale_milli(3.25).unwrap(), 3_250);
        assert!(scale_milli(3.3333).is_err());
    }

    #[test]
    fn fixture_result_binds_generation_evidence_before_the_audit_state() {
        let temporary = tempfile::tempdir().unwrap();
        let fixture = fixture_result(temporary.path());
        let task = acceptance_task_path();
        let result = fixture_result_to_closed_loop(
            GenerationMode::Fixture,
            &task,
            "generated.audit_draft",
            &fixture,
        )
        .unwrap();

        assert_eq!(result.status, "ready_for_audit");
        assert_eq!(
            result.audit_registration.as_ref().unwrap().runtime,
            "standalone_ui_document"
        );
        let manifest =
            ClosedLoopRunManifest::load(result.closed_loop_manifest.as_ref().unwrap()).unwrap();
        assert_eq!(manifest.state, ClosedLoopRunState::Auditing);
        assert!(manifest.artifacts.generation_result.is_some());
        assert!(manifest.artifacts.provider_metadata.is_some());
        assert!(manifest.artifacts.validation_report.is_some());
        assert!(manifest.artifacts.source_map.is_some());
        assert!(manifest.artifacts.draft_assets_manifest.is_some());
    }

    #[test]
    fn missing_fixture_draft_resource_fails_before_audit_registration() {
        let temporary = tempfile::tempdir().unwrap();
        let fixture = fixture_result(temporary.path());
        fs::remove_file(&fixture.generated_document).unwrap();

        let failure = fixture_result_to_closed_loop(
            GenerationMode::Fixture,
            &acceptance_task_path(),
            "generated.audit_draft",
            &fixture,
        )
        .unwrap_err();
        assert_eq!(failure.kind(), TaskFailureKind::ArtifactMissing);
    }

    #[test]
    fn preview_timeout_is_persisted_as_a_terminal_failed_manifest() {
        let temporary = tempfile::tempdir().unwrap();
        let fixture = fixture_result(temporary.path());
        fs::remove_file(&fixture.preview_screenshot).unwrap();
        let task = GenerationTask::load_json(&acceptance_task_path()).unwrap();
        let artifacts = fixture_artifacts(&fixture.run_root, false).unwrap();
        let mut manifest = new_fixture_manifest(&task, artifacts).unwrap();
        advance_to_previewing(&mut manifest).unwrap();
        let preview_failure = TaskFailure::new(
            TaskFailureKind::PreviewFailed,
            "preview executor timed out",
            None,
        );
        manifest
            .fail(unix_millis().unwrap(), preview_failure)
            .unwrap();
        let path = fixture.run_root.join("closed-loop-manifest.json");
        manifest.write_new(&path).unwrap();

        let persisted = ClosedLoopRunManifest::load(&path).unwrap();
        assert_eq!(persisted.state, ClosedLoopRunState::Failed);
        assert!(persisted.artifacts.preview.is_none());
        assert_eq!(
            persisted.failure.as_ref().unwrap().kind(),
            TaskFailureKind::PreviewFailed
        );
    }

    #[test]
    fn provider_adapter_stays_explicitly_unavailable_without_an_approved_implementation() {
        assert_eq!(
            provider_adapter_unavailable().kind(),
            TaskFailureKind::ProviderNotFound
        );
    }

    fn acceptance_task_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/acceptance/task.valid.json")
    }

    fn fixture_result(root: &Path) -> OfflineFixtureRunResult {
        let run_root = root.join("run");
        let generated_document = run_root.join("draft/generated-document.json");
        let source_map = run_root.join("draft/source-map.json");
        let validation_report = run_root.join("draft/validation-report.json");
        let generation_trace = run_root.join("logs/generation-trace.json");
        let draft_assets_manifest = run_root.join("draft/assets-manifest.json");
        let preview_screenshot = run_root.join("preview/process/preview.png");
        let run_report = run_root.join("logs/offline-run.json");
        let bundle_manifest = run_root.join("bundle/manifest.json");
        for path in [
            run_root.join("input/generation-task.json"),
            run_root.join("input/preprocessed/manifest.json"),
            generated_document.clone(),
            source_map.clone(),
            validation_report.clone(),
            generation_trace.clone(),
            draft_assets_manifest.clone(),
            preview_screenshot.clone(),
            run_report.clone(),
            bundle_manifest.clone(),
        ] {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"fixture evidence").unwrap();
        }
        OfflineFixtureRunResult {
            protocol_version: 1,
            mode: "offline_fixture".to_owned(),
            fixture_profile: "regular".to_owned(),
            run_id: "acceptance-03-final-20260718-04".to_owned(),
            document_id: "generated.audit_draft".to_owned(),
            run_root,
            generated_document,
            source_map,
            validation_report,
            generation_trace,
            draft_assets_manifest,
            preview_screenshot,
            run_report,
            bundle_manifest,
            bundle_manifest_sha256: "a".repeat(64),
        }
    }
}
