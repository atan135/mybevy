//! Deterministic repository-fixture orchestration for the complete reference-to-preview path.
//!
//! This mode is intentionally separate from online providers. It binds repository-authored
//! structured fixtures to real task, image, preprocess, viewport, and provider execution evidence.

use crate::{
    analysis::{
        AnalysisBoundingBox, AnalysisProviderProvenance, Axis, EvidenceSource,
        TrustedPreprocessEvidence, UiReferenceAnalysis, analysis_output_contract,
        parse_analysis_json, parse_provider_execution_analysis,
    },
    asset_strategy::{AssetCatalog, build_asset_strategy},
    contract::GenerationTask,
    generation::{GenerationConfiguration, GenerationParameters, prepare_generation_request},
    lifecycle::{CancellationToken, TaskFailure, TaskFailureKind},
    planning::plan_analysis,
    preprocess::{ArtifactKind, ReferencePreprocessManifest, preprocess_task},
    preview::{CommandPreviewExecutor, PreviewRunStatus, prepare_preview_command, run_preview},
    provider::{
        FixtureProvider, Provider, ProviderExecution, ProviderExecutionPolicy, ProviderId,
        ProviderImage, ProviderRegistry, ProviderRunner,
    },
    repair::{RepairConfiguration, RepairRunStatus, repair_generated_document},
    run_manifest::{ArtifactLink, StageEvidenceLinks, persist_run_bundle},
};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

pub const OFFLINE_FIXTURE_RUN_PROTOCOL_VERSION: u32 = 1;
const ANALYSIS_PROMPT_VERSION: &str = "analysis-fixture-v1";
const GENERATION_MODEL_ID: &str = "fixture-generation-v1";
const GENERATION_PROMPT_VERSION: &str = "ui-document-fixture-v1";
const MAX_FIXTURE_BYTES: u64 = 4 * 1024 * 1024;

/// Repository-authored fixture profiles. This keeps the regular and complex acceptance evidence
/// distinct without exposing any online-provider selection through the CLI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OfflineFixtureProfile {
    Regular,
    Complex,
}

impl OfflineFixtureProfile {
    pub fn parse(value: &str) -> Result<Self, TaskFailure> {
        match value {
            "regular" => Ok(Self::Regular),
            "complex" => Ok(Self::Complex),
            _ => Err(TaskFailure::invalid(
                "offline fixture profile must be `regular` or `complex`",
            )),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Regular => "regular",
            Self::Complex => "complex",
        }
    }

    fn analysis_fixture_file(self) -> &'static str {
        match self {
            Self::Regular => "regular_page.json",
            Self::Complex => "modal.json",
        }
    }

    fn generation_provider_fixture_file(self) -> &'static str {
        match self {
            Self::Regular => "generation.valid.json",
            Self::Complex => "generation.complex.valid.json",
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OfflineFixtureRunResult {
    pub protocol_version: u32,
    pub mode: String,
    pub fixture_profile: String,
    pub run_id: String,
    pub document_id: String,
    pub run_root: PathBuf,
    pub generated_document: PathBuf,
    pub source_map: PathBuf,
    pub validation_report: PathBuf,
    pub generation_trace: PathBuf,
    pub draft_assets_manifest: PathBuf,
    pub preview_screenshot: PathBuf,
    pub run_report: PathBuf,
    pub bundle_manifest: PathBuf,
    pub bundle_manifest_sha256: String,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct OfflineRunReport<'a> {
    protocol_version: u32,
    mode: &'static str,
    fixture_profile: &'static str,
    run_id: &'a str,
    task_sha256: String,
    target_viewport: crate::contract::TargetViewport,
    analysis: ProviderEvidence<'a>,
    generation: ProviderEvidence<'a>,
    generation_input_sha256: &'a str,
    canonical_document_sha256: &'a str,
    estimated_cost_micro_units: u64,
    sensitive_payloads_recorded_in_log: bool,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct ProviderEvidence<'a> {
    provider_id: &'a str,
    prompt_version: &'a str,
    schema_id: &'a str,
    schema_version: u32,
    server_request_id: &'a str,
    attempts: usize,
    input_units: Option<u64>,
    output_units: Option<u64>,
}

pub fn run_offline_fixture_generation(
    task_path: &Path,
    preprocess_options_path: Option<&Path>,
    repository_root: &Path,
    document_id: &str,
    fixture_profile: OfflineFixtureProfile,
    cancellation: &CancellationToken,
) -> Result<OfflineFixtureRunResult, TaskFailure> {
    cancellation.checkpoint()?;
    let repository_root = canonical_directory(repository_root, "repository root")?;
    let task = GenerationTask::load_json(task_path)?;
    let viewport = task.target_viewport.ok_or_else(|| {
        TaskFailure::new(
            TaskFailureKind::TargetViewportMissing,
            "offline fixture generation requires a target viewport",
            None,
        )
    })?;
    let preprocess = preprocess_task(
        task_path,
        preprocess_options_path,
        &repository_root,
        cancellation,
    )?;
    let run_root = canonical_directory(&preprocess.output_root, "preprocess run root")?;

    let task_artifact = run_root.join("input/generation-task.json");
    let task_bytes = pretty_json_bytes(&task)?;
    write_new_synced(&task_artifact, &task_bytes)?;

    let mut trusted_preprocess = Vec::with_capacity(preprocess.references.len());
    let mut reference_manifests = BTreeMap::new();
    for reference in &preprocess.references {
        let bytes = read_bounded(
            &reference.manifest,
            MAX_FIXTURE_BYTES,
            "preprocess manifest",
        )?;
        let manifest: ReferencePreprocessManifest = serde_json::from_slice(&bytes)
            .map_err(|_| TaskFailure::invalid("preprocess manifest is invalid JSON"))?;
        let trusted = TrustedPreprocessEvidence::from_manifest(&manifest, &bytes)?;
        reference_manifests.insert(reference.reference_id.clone(), manifest);
        trusted_preprocess.push(trusted);
    }

    let analysis_fixture = repository_root
        .join("tools/ui-generation/fixtures/analysis")
        .join(fixture_profile.analysis_fixture_file());
    let mut analysis = parse_analysis_json(&read_bounded(
        &analysis_fixture,
        MAX_FIXTURE_BYTES,
        "analysis fixture",
    )?)
    .map_err(analysis_report_failure)?;
    let analysis_provider_path =
        repository_root.join("tools/ui-generation/fixtures/providers/valid.json");
    let mut analysis_provider = FixtureProvider::load(&analysis_provider_path)?;
    let analysis_provider_id = analysis_provider.descriptor().id;
    let analysis_request_id = analysis_provider
        .success_server_request_id()
        .ok_or_else(|| TaskFailure::invalid("analysis fixture lacks a request ID"))?
        .to_owned();
    bind_analysis_fixture(
        &mut analysis,
        &task.run_id,
        analysis_provider_id.as_str(),
        &analysis_request_id,
        ANALYSIS_PROMPT_VERSION,
        &trusted_preprocess,
    )?;
    analysis_provider.bind_success_value(
        serde_json::to_value(&analysis)
            .map_err(|_| TaskFailure::invalid("bound analysis fixture is not serializable"))?,
    )?;
    let analysis_request = crate::provider::ProviderRequest::visual_analysis(
        task.run_id.clone(),
        ANALYSIS_PROMPT_VERSION,
        "Analyze the repository-authored acceptance fixture using the closed analysis schema.",
        provider_images(&preprocess.references, &reference_manifests)?,
        analysis_output_contract(),
    )?;
    let (analysis_runner, analysis_provider_id) = fixture_runner(analysis_provider)?;
    let analysis_execution = analysis_runner
        .execute(&analysis_provider_id, analysis_request, cancellation)
        .map_err(|failure| failure.failure)?;
    let analysis =
        parse_provider_execution_analysis(&analysis_execution, &task, &trusted_preprocess)
            .map_err(analysis_report_failure)?;
    let analysis_path = run_root.join("analysis/reference-analysis.json");
    write_json_new(&analysis_path, &analysis)?;

    let plan = plan_analysis(&analysis);
    let plan_path = run_root.join("analysis/generation-plan.json");
    write_json_new(&plan_path, &plan)?;
    let catalog = AssetCatalog::load_repository(&repository_root)?;
    let asset_strategy = build_asset_strategy(&analysis, &plan, &catalog, &[], &[])?;
    let asset_strategy_path = run_root.join("analysis/asset-strategy.json");
    write_json_new(&asset_strategy_path, &asset_strategy)?;

    let generation_parameters = GenerationParameters::new(0, 262_144, Some(7))?;
    let prepared = prepare_generation_request(
        &analysis,
        &plan,
        &asset_strategy,
        &catalog,
        GenerationConfiguration::new(
            document_id,
            GENERATION_MODEL_ID,
            GENERATION_PROMPT_VERSION,
            generation_parameters,
        )?,
    )?;
    let generation_provider_path = repository_root
        .join("tools/ui-generation/fixtures/providers")
        .join(fixture_profile.generation_provider_fixture_file());
    let mut generation_provider = FixtureProvider::load(&generation_provider_path)?;
    let generation_request_id = generation_provider
        .success_server_request_id()
        .ok_or_else(|| TaskFailure::invalid("generation fixture lacks a request ID"))?
        .to_owned();
    let mut generation_value = provider_success_value(&generation_provider_path)?;
    generation_value["document"]["document_id"] = Value::String(document_id.to_owned());
    let fixture_document = serde_json::to_string(&generation_value["document"])
        .map_err(|_| TaskFailure::invalid("generation fixture document is not serializable"))?;
    let canonical_fixture = project::framework::ui::document::tooling::canonicalize_json(
        &fixture_document,
    )
    .map_err(|error| {
        TaskFailure::invalid(format!(
            "generation fixture document failed formal canonicalization: {}",
            error.code()
        ))
    })?;
    generation_value["document"] = serde_json::from_str(&canonical_fixture)
        .expect("formal canonical JSON is always parseable");
    generation_provider.bind_success_value(generation_value)?;
    let (generation_runner, generation_provider_id) = fixture_runner(generation_provider)?;
    let generation_execution = generation_runner
        .execute(
            &generation_provider_id,
            prepared.request().clone(),
            cancellation,
        )
        .map_err(|failure| failure.failure)?;
    let repair = repair_generated_document(
        &generation_execution,
        &prepared,
        &generation_runner,
        &generation_provider_id,
        cancellation,
        RepairConfiguration::new(3)?,
    );
    if repair.status != RepairRunStatus::Passed {
        return Err(TaskFailure::new(
            TaskFailureKind::ProviderResponseMalformed,
            "offline fixture generation did not produce a valid document",
            repair
                .failure
                .as_ref()
                .map(|failure| failure.detail.clone()),
        ));
    }
    let generated = repair
        .final_document
        .as_ref()
        .expect("passed repair always contains a final document");
    let document_path = run_root.join("draft/generated-document.json");
    write_new_synced(&document_path, generated.canonical_document_json.as_bytes())?;
    let source_map_path = run_root.join("draft/source-map.json");
    write_json_new(&source_map_path, &generated.source_map)?;
    let validation_report_path = run_root.join("draft/validation-report.json");
    write_json_new(&validation_report_path, &generated.validation_report)?;
    // Keep the empty set explicit so every generated run has a stable draft-assets artifact.
    // A later asset-producing generator may append immutable asset links without changing the
    // closed-loop manifest shape.
    let draft_assets_manifest_path = run_root.join("draft/assets-manifest.json");
    write_json_new(&draft_assets_manifest_path, &Vec::<String>::new())?;
    let trace_path = run_root.join("logs/generation-trace.json");
    write_json_new(&trace_path, &generated.trace)?;

    let preview_width = viewport_dimension(viewport.logical_width, "logical_width")?;
    let preview_height = viewport_dimension(viewport.logical_height, "logical_height")?;
    let preview_plan = prepare_preview_command(
        &repository_root,
        &document_path,
        &run_root.join("preview/process"),
        preview_width,
        preview_height,
    )?;
    let preview = run_preview(preview_plan, &CommandPreviewExecutor, cancellation);

    let report_path = run_root.join("logs/offline-run.json");
    let analysis_contract = analysis_output_contract();
    let report = OfflineRunReport {
        protocol_version: OFFLINE_FIXTURE_RUN_PROTOCOL_VERSION,
        mode: "offline_fixture",
        fixture_profile: fixture_profile.label(),
        run_id: &task.run_id,
        task_sha256: hash_bytes(&task_bytes),
        target_viewport: viewport,
        analysis: provider_evidence(
            &analysis_execution,
            ANALYSIS_PROMPT_VERSION,
            analysis_contract.schema_id.as_str(),
            analysis_contract.schema_version,
            &analysis_request_id,
        ),
        generation: provider_evidence(
            &generation_execution,
            GENERATION_PROMPT_VERSION,
            generated.trace.output_schema.schema_id.as_str(),
            generated.trace.output_schema.schema_version,
            &generation_request_id,
        ),
        generation_input_sha256: prepared.input_sha256(),
        canonical_document_sha256: &generated.trace.canonical_document_sha256,
        estimated_cost_micro_units: 0,
        sensitive_payloads_recorded_in_log: false,
    };
    write_json_new(&report_path, &report)?;

    let stage_evidence = StageEvidenceLinks {
        input_preprocess_manifest: artifact_link(&run_root, &preprocess.manifest)?,
        input_references: preprocess
            .references
            .iter()
            .map(|reference| {
                let manifest = &reference_manifests[&reference.reference_id];
                let preview = manifest
                    .artifacts
                    .iter()
                    .find(|artifact| {
                        artifact.kind == ArtifactKind::StandardPreview && !artifact.auxiliary_only
                    })
                    .expect("trusted preprocess evidence requires a standard preview");
                artifact_link(
                    &run_root,
                    &reference.output_directory.join(&preview.file_name),
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
        reference_analysis: artifact_link(&run_root, &analysis_path)?,
        asset_strategy: artifact_link(&run_root, &asset_strategy_path)?,
        draft_assets: Vec::new(),
        generated_document: artifact_link(&run_root, &document_path)?,
        generation_trace: artifact_link(&run_root, &trace_path)?,
    };
    let bundle = persist_run_bundle(
        &repository_root,
        &task.run_id,
        stage_evidence,
        &repair,
        &preview,
    )?;
    if preview.status != PreviewRunStatus::Passed {
        let detail = preview
            .failure
            .as_ref()
            .map(|failure| format!("{}: {}", failure.code, failure.detail))
            .unwrap_or_else(|| "preview did not produce passed evidence".to_owned());
        return Err(TaskFailure::new(
            TaskFailureKind::PreviewFailed,
            format!("offline fixture generation preview failed: {detail}"),
            Some(bundle.manifest_path.display().to_string()),
        ));
    }

    Ok(OfflineFixtureRunResult {
        protocol_version: OFFLINE_FIXTURE_RUN_PROTOCOL_VERSION,
        mode: "offline_fixture".to_owned(),
        fixture_profile: fixture_profile.label().to_owned(),
        run_id: task.run_id,
        document_id: document_id.to_owned(),
        run_root,
        generated_document: document_path,
        source_map: source_map_path,
        validation_report: validation_report_path,
        generation_trace: trace_path,
        draft_assets_manifest: draft_assets_manifest_path,
        preview_screenshot: preview.command.screenshot_path,
        run_report: report_path,
        bundle_manifest: bundle.manifest_path,
        bundle_manifest_sha256: bundle.manifest_sha256,
    })
}

fn bind_analysis_fixture(
    analysis: &mut UiReferenceAnalysis,
    run_id: &str,
    provider_id: &str,
    request_id: &str,
    prompt_version: &str,
    trusted: &[TrustedPreprocessEvidence],
) -> Result<(), TaskFailure> {
    let trusted_by_id: BTreeMap<_, _> = trusted
        .iter()
        .map(|reference| (reference.reference_id.as_str(), reference))
        .collect();
    if trusted_by_id.len() != analysis.references.len()
        || analysis
            .references
            .iter()
            .any(|reference| !trusted_by_id.contains_key(reference.reference_id.as_str()))
    {
        return Err(TaskFailure::invalid(
            "analysis fixture reference IDs differ from the preprocessed task",
        ));
    }
    let old_dimensions: BTreeMap<_, _> = analysis
        .references
        .iter()
        .map(|reference| {
            (
                reference.reference_id.clone(),
                (f64::from(reference.width), f64::from(reference.height)),
            )
        })
        .collect();
    let scales: BTreeMap<_, _> = trusted
        .iter()
        .map(|reference| {
            let (old_width, old_height) = old_dimensions[&reference.reference_id];
            (
                reference.reference_id.clone(),
                (
                    f64::from(reference.width) / old_width,
                    f64::from(reference.height) / old_height,
                ),
            )
        })
        .collect();
    analysis.run_id = run_id.to_owned();
    analysis.provider = AnalysisProviderProvenance {
        provider_id: provider_id.to_owned(),
        server_request_id: request_id.to_owned(),
        prompt_version: prompt_version.to_owned(),
    };
    for reference in &mut analysis.references {
        let trusted = trusted_by_id[reference.reference_id.as_str()];
        reference.source_sha256 = trusted.source_sha256.clone();
        reference.preprocess_cache_key = trusted.preprocess_cache_key.clone();
        reference.preprocess_protocol_version = trusted.preprocess_protocol_version;
        reference.preprocess_implementation_version =
            trusted.preprocess_implementation_version.clone();
        reference.preprocess_manifest_sha256 = trusted.preprocess_manifest_sha256.clone();
        reference.standard_preview_sha256 = trusted.standard_preview_sha256.clone();
        reference.coordinate_space = trusted.coordinate_space;
        reference.coordinate_convention = trusted.coordinate_convention.clone();
        reference.width = trusted.width;
        reference.height = trusted.height;
    }
    for region in &mut analysis.regions {
        scale_box(&mut region.bounding_box, &scales)?;
    }
    for element in &mut analysis.elements {
        let reference_id = element.bounding_box.reference_id.clone();
        scale_box(&mut element.bounding_box, &scales)?;
        let scale = scales
            .get(&reference_id)
            .ok_or_else(|| TaskFailure::invalid("element uses an unknown reference"))?;
        for clue in &mut element.alignment_clues {
            clue.offset *= match clue.axis {
                Axis::Horizontal => scale.0,
                Axis::Vertical => scale.1,
            };
        }
    }
    for evidence in &mut analysis.evidence {
        if let EvidenceSource::ProviderResponse {
            provider_id: evidence_provider,
            server_request_id,
        } = &mut evidence.source
        {
            *evidence_provider = provider_id.to_owned();
            *server_request_id = request_id.to_owned();
        }
    }
    let report = analysis.validate_semantics();
    if report.valid {
        Ok(())
    } else {
        Err(analysis_report_failure(report))
    }
}

fn scale_box(
    bounding_box: &mut AnalysisBoundingBox,
    scales: &BTreeMap<String, (f64, f64)>,
) -> Result<(), TaskFailure> {
    let (scale_x, scale_y) = scales
        .get(&bounding_box.reference_id)
        .ok_or_else(|| TaskFailure::invalid("analysis bounding box uses an unknown reference"))?;
    bounding_box.x *= scale_x;
    bounding_box.y *= scale_y;
    bounding_box.width *= scale_x;
    bounding_box.height *= scale_y;
    Ok(())
}

fn provider_images(
    references: &[crate::preprocess::PreprocessedReferenceResult],
    manifests: &BTreeMap<String, ReferencePreprocessManifest>,
) -> Result<Vec<ProviderImage>, TaskFailure> {
    references
        .iter()
        .map(|reference| {
            let manifest = &manifests[&reference.reference_id];
            let preview = manifest
                .artifacts
                .iter()
                .find(|artifact| {
                    artifact.kind == ArtifactKind::StandardPreview && !artifact.auxiliary_only
                })
                .ok_or_else(|| {
                    TaskFailure::invalid("preprocess result lacks a standard preview")
                })?;
            let bytes = read_bounded(
                &reference.output_directory.join(&preview.file_name),
                64 * 1024 * 1024,
                "standard preview",
            )?;
            ProviderImage::new(
                reference.reference_id.clone(),
                "image/png",
                Arc::<[u8]>::from(bytes),
            )
        })
        .collect()
}

fn fixture_runner(provider: FixtureProvider) -> Result<(ProviderRunner, ProviderId), TaskFailure> {
    let provider_id = provider.descriptor().id;
    let mut registry = ProviderRegistry::default();
    registry.register(Arc::new(provider))?;
    ProviderRunner::new(registry, ProviderExecutionPolicy::default())
        .map(|runner| (runner, provider_id))
}

fn provider_success_value(path: &Path) -> Result<Value, TaskFailure> {
    let document: Value = serde_json::from_slice(&read_bounded(
        path,
        MAX_FIXTURE_BYTES,
        "generation provider fixture",
    )?)
    .map_err(|_| TaskFailure::invalid("generation provider fixture is invalid JSON"))?;
    document
        .get("outcome")
        .and_then(|outcome| outcome.get("value"))
        .cloned()
        .ok_or_else(|| TaskFailure::invalid("generation provider fixture lacks a success value"))
}

fn provider_evidence<'a>(
    execution: &'a ProviderExecution,
    prompt_version: &'a str,
    schema_id: &'a str,
    schema_version: u32,
    request_id: &'a str,
) -> ProviderEvidence<'a> {
    ProviderEvidence {
        provider_id: execution.trace.provider_id.as_str(),
        prompt_version,
        schema_id,
        schema_version,
        server_request_id: request_id,
        attempts: execution.trace.attempts.len(),
        input_units: execution.response.usage.input_units,
        output_units: execution.response.usage.output_units,
    }
}

fn artifact_link(run_root: &Path, path: &Path) -> Result<ArtifactLink, TaskFailure> {
    let bytes = read_bounded(path, 64 * 1024 * 1024, "run artifact")?;
    let relative = path.strip_prefix(run_root).map_err(|_| {
        TaskFailure::invalid("run artifact path is outside the controlled run root")
    })?;
    let relative = safe_relative_path(relative)?;
    ArtifactLink::new(relative, hash_bytes(&bytes), bytes.len() as u64)
}

fn safe_relative_path(path: &Path) -> Result<String, TaskFailure> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => {
                let part = part.to_str().ok_or_else(|| {
                    TaskFailure::invalid("run artifact path is not valid Unicode")
                })?;
                if part.is_empty() || !part.is_ascii() {
                    return Err(TaskFailure::invalid("run artifact path is not safe ASCII"));
                }
                parts.push(part);
            }
            _ => return Err(TaskFailure::invalid("run artifact path is not relative")),
        }
    }
    if parts.is_empty() {
        Err(TaskFailure::invalid("run artifact path is empty"))
    } else {
        Ok(parts.join("/"))
    }
}

fn viewport_dimension(value: f32, field: &str) -> Result<u32, TaskFailure> {
    if !value.is_finite() || value.fract() != 0.0 || !(64.0..=4096.0).contains(&value) {
        return Err(TaskFailure::invalid(format!(
            "offline fixture {field} must be an integer in 64..=4096"
        )));
    }
    Ok(value as u32)
}

fn analysis_report_failure(report: crate::analysis::AnalysisValidationReport) -> TaskFailure {
    let detail = report
        .diagnostics
        .first()
        .map(|diagnostic| {
            format!(
                "{} at {}: {}",
                diagnostic.code, diagnostic.path, diagnostic.message
            )
        })
        .unwrap_or_else(|| "analysis fixture validation failed".to_owned());
    TaskFailure::invalid(detail)
}

fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf, TaskFailure> {
    let path = fs::canonicalize(path)
        .map_err(|error| TaskFailure::invalid(format!("{label} cannot be resolved: {error}")))?;
    if path.is_dir() {
        Ok(path)
    } else {
        Err(TaskFailure::invalid(format!("{label} is not a directory")))
    }
}

fn read_bounded(path: &Path, maximum: u64, label: &str) -> Result<Vec<u8>, TaskFailure> {
    let metadata = fs::metadata(path)
        .map_err(|error| TaskFailure::invalid(format!("{label} cannot be read: {error}")))?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > maximum {
        return Err(TaskFailure::invalid(format!(
            "{label} must be a nonempty regular file no larger than {maximum} bytes"
        )));
    }
    fs::read(path).map_err(|error| TaskFailure::invalid(format!("{label} read failed: {error}")))
}

fn write_json_new<T: Serialize>(path: &Path, value: &T) -> Result<(), TaskFailure> {
    write_new_synced(path, &pretty_json_bytes(value)?)
}

fn pretty_json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, TaskFailure> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| TaskFailure::invalid(format!("JSON serialization failed: {error}")))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn write_new_synced(path: &Path, bytes: &[u8]) -> Result<(), TaskFailure> {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|error| {
            TaskFailure::new(
                TaskFailureKind::OutputDirectoryConflict,
                format!("run artifact cannot be created: {error}"),
                Some(path.display().to_string()),
            )
        })?;
    file.write_all(bytes)
        .and_then(|_| file.flush())
        .and_then(|_| file.sync_all())
        .map_err(|error| {
            TaskFailure::invalid(format!(
                "run artifact write failed at {}: {error}",
                path.display()
            ))
        })
}

fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
