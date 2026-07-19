use crate::{
    lifecycle::{TaskFailure, TaskFailureKind},
    preview::{
        PreviewRunResult, PreviewRunStatus, validate_passed_preview_evidence,
        validate_passed_preview_evidence_at,
    },
    repair::{MAX_REPAIR_ROUNDS, RepairRunResult, RepairRunStatus},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    fs::OpenOptions,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::SystemTime,
};

pub const RUN_MANIFEST_PROTOCOL_VERSION: u32 = 1;
const MAX_ARTIFACTS: usize = 96;
const MAX_MANIFEST_BYTES: usize = 2 * 1024 * 1024;
const MAX_JSON_ARTIFACT_BYTES: usize = 4 * 1024 * 1024;
const MAX_SCREENSHOT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;
const MAX_STAGE_RESOURCE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_STAGE_EVIDENCE_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactLink {
    pub relative_path: String,
    pub sha256: String,
    pub byte_length: u64,
}

impl ArtifactLink {
    pub fn new(
        relative_path: impl Into<String>,
        sha256: impl Into<String>,
        byte_length: u64,
    ) -> Result<Self, TaskFailure> {
        let result = Self {
            relative_path: relative_path.into(),
            sha256: sha256.into(),
            byte_length,
        };
        if !safe_relative_path(&result.relative_path)
            || !is_sha256(&result.sha256)
            || result.byte_length == 0
        {
            return Err(TaskFailure::invalid(
                "artifact links require a safe relative path, SHA-256, and nonzero byte length",
            ));
        }
        Ok(result)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StageEvidenceLinks {
    pub input_preprocess_manifest: ArtifactLink,
    pub input_references: Vec<ArtifactLink>,
    pub reference_analysis: ArtifactLink,
    pub asset_strategy: ArtifactLink,
    pub draft_assets: Vec<ArtifactLink>,
    pub generated_document: ArtifactLink,
    pub generation_trace: ArtifactLink,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunBundleStatus {
    Passed,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunArtifactRecord {
    pub kind: String,
    pub relative_path: String,
    pub sha256: String,
    pub byte_length: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UiGenerationRunManifest {
    pub protocol_version: u32,
    pub run_id: String,
    pub status: RunBundleStatus,
    pub stage_evidence: StageEvidenceLinks,
    pub repair_status: RepairRunStatus,
    pub repair_round_count: usize,
    pub preview_status: PreviewRunStatus,
    pub artifacts: BTreeMap<String, RunArtifactRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PersistedRunBundle {
    pub run_root: PathBuf,
    pub bundle_directory: PathBuf,
    pub manifest_path: PathBuf,
    pub committed_marker: PathBuf,
    pub manifest_sha256: String,
}

pub fn persist_run_bundle(
    repository_root: &Path,
    run_id: &str,
    stage_evidence: StageEvidenceLinks,
    repair: &RepairRunResult,
    preview: &PreviewRunResult,
) -> Result<PersistedRunBundle, TaskFailure> {
    crate::directory::RunId::parse(run_id)?;
    if repair.rounds.len() > usize::from(MAX_REPAIR_ROUNDS) {
        return Err(TaskFailure::invalid(
            "run bundle repair round count exceeds the closed policy",
        ));
    }
    let repository_root =
        canonical_regular_directory(repository_root, "run bundle repository root")?;
    let generation_root_path = repository_root.join("summary/ui-generation");
    reject_reparse_components(&repository_root, Path::new("summary/ui-generation"))?;
    let generation_root =
        canonical_regular_directory(&generation_root_path, "run bundle generation root")?;
    if !generation_root.starts_with(&repository_root) {
        return Err(TaskFailure::invalid(
            "run bundle generation root escapes the repository",
        ));
    }
    let run_root_path = generation_root_path.join(run_id);
    reject_reparse_components(
        &repository_root,
        &PathBuf::from("summary/ui-generation").join(run_id),
    )?;
    let run_root = canonical_regular_directory(&run_root_path, "existing Stage 3 run root")?;
    if !run_root.starts_with(&generation_root)
        || run_root.parent() != Some(generation_root.as_path())
    {
        return Err(TaskFailure::invalid(
            "run bundle root is not a direct child of summary/ui-generation",
        ));
    }
    validate_stage3_run_layout(&run_root)?;
    reject_existing_bundle_targets(&run_root)?;
    validate_run_result_consistency(repair, preview)?;
    validate_stage_links(&run_root, run_id, &stage_evidence, repair, preview)?;

    let partial = run_root.join(".bundle-partial");
    fs::create_dir(&partial).map_err(|error| bundle_write_failure(&partial, error))?;

    let result = write_bundle(&partial, run_id, stage_evidence, repair, preview);
    let (manifest_sha256, manifest_relative_path) = match result {
        Ok(result) => result,
        Err(error) => return Err(error),
    };
    let bundle_directory = run_root.join("bundle");
    fs::rename(&partial, &bundle_directory)
        .map_err(|error| bundle_write_failure(&bundle_directory, error))?;
    let committed_marker = run_root.join("COMMITTED");
    write_new_synced(
        &committed_marker,
        format!("manifest_sha256={manifest_sha256}\n").as_bytes(),
    )?;
    Ok(PersistedRunBundle {
        run_root,
        manifest_path: bundle_directory.join(manifest_relative_path),
        bundle_directory,
        committed_marker,
        manifest_sha256,
    })
}

fn write_bundle(
    partial: &Path,
    run_id: &str,
    stage_evidence: StageEvidenceLinks,
    repair: &RepairRunResult,
    preview: &PreviewRunResult,
) -> Result<(String, PathBuf), TaskFailure> {
    let mut artifacts = BTreeMap::new();
    write_json_artifact(
        partial,
        "repair_run",
        "repair/run.json",
        repair,
        &mut artifacts,
    )?;
    write_json_artifact(
        partial,
        "repair_initial_document",
        "repair/initial-document.json",
        &repair.initial_document,
        &mut artifacts,
    )?;
    for (index, round) in repair.rounds.iter().enumerate() {
        write_json_artifact(
            partial,
            &format!("repair_round_{:02}", index + 1),
            &format!("repair/round-{:02}.json", index + 1),
            round,
            &mut artifacts,
        )?;
    }
    if let Some(document) = &repair.final_document {
        write_new_artifact(
            partial,
            "final_document",
            "final/document.json",
            document.canonical_document_json.as_bytes(),
            &mut artifacts,
        )?;
        write_json_artifact(
            partial,
            "generation_trace",
            "final/generation-trace.json",
            &document.trace,
            &mut artifacts,
        )?;
        write_json_artifact(
            partial,
            "source_map",
            "final/source-map.json",
            &document.source_map,
            &mut artifacts,
        )?;
        write_json_artifact(
            partial,
            "validation_report",
            "final/validation-report.json",
            &document.validation_report,
            &mut artifacts,
        )?;
    }
    if let Some(summary) = &repair.node_tree_summary {
        write_json_artifact(
            partial,
            "node_tree_summary",
            "final/node-tree-summary.json",
            summary,
            &mut artifacts,
        )?;
    }
    write_json_artifact(
        partial,
        "preview_run",
        "preview/run.json",
        preview,
        &mut artifacts,
    )?;
    copy_optional_artifact(
        partial,
        "preview_process_result",
        "preview/process-result.json",
        &preview.command.result_path,
        MAX_JSON_ARTIFACT_BYTES as u64,
        &mut artifacts,
    )?;
    copy_optional_artifact(
        partial,
        "preview_screenshot",
        "preview/preview.png",
        &preview.command.screenshot_path,
        MAX_SCREENSHOT_BYTES,
        &mut artifacts,
    )?;
    copy_optional_artifact(
        partial,
        "preview_log",
        "preview/preview.log",
        &preview.command.log_path,
        MAX_LOG_BYTES,
        &mut artifacts,
    )?;
    if preview.status == PreviewRunStatus::Passed {
        let process_result = artifacts.get("preview_process_result");
        let screenshot = artifacts.get("preview_screenshot");
        let log = artifacts.get("preview_log");
        if process_result.is_none() || screenshot.is_none() || log.is_none() {
            return Err(TaskFailure::invalid(
                "passed preview evidence requires result, screenshot, and log artifacts",
            ));
        }
        let screenshot = screenshot.expect("checked above");
        if preview.screenshot_sha256.as_deref() != Some(screenshot.sha256.as_str())
            || preview.screenshot_bytes != Some(screenshot.byte_length)
        {
            return Err(TaskFailure::invalid(
                "passed preview screenshot evidence hash or byte length differs",
            ));
        }
        validate_passed_preview_evidence_at(
            preview,
            &partial.join("preview/process-result.json"),
            &partial.join("preview/preview.png"),
            &partial.join("preview/preview.log"),
        )
        .map_err(|failure| {
            TaskFailure::invalid(format!(
                "persisted passed preview evidence failed strict revalidation: {} ({})",
                failure.code, failure.detail
            ))
        })?;
    }
    if artifacts.len() > MAX_ARTIFACTS {
        return Err(TaskFailure::invalid(
            "run bundle exceeds the artifact count budget",
        ));
    }

    let status =
        if repair.status == RepairRunStatus::Passed && preview.status == PreviewRunStatus::Passed {
            RunBundleStatus::Passed
        } else {
            RunBundleStatus::Failed
        };
    let manifest = UiGenerationRunManifest {
        protocol_version: RUN_MANIFEST_PROTOCOL_VERSION,
        run_id: run_id.to_owned(),
        status,
        stage_evidence,
        repair_status: repair.status,
        repair_round_count: repair.rounds.len(),
        preview_status: preview.status.clone(),
        artifacts,
    };
    let manifest_bytes = pretty_json_bytes(&manifest)?;
    if manifest_bytes.len() > MAX_MANIFEST_BYTES {
        return Err(TaskFailure::invalid("run manifest exceeds its byte budget"));
    }
    let manifest_relative_path = PathBuf::from("manifest.json");
    write_new_synced(&partial.join(&manifest_relative_path), &manifest_bytes)?;
    Ok((hash_bytes(&manifest_bytes), manifest_relative_path))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PreprocessRunIdentity {
    protocol_version: u32,
    implementation_version: String,
    run_id: String,
    references: Vec<PreprocessReferenceIdentity>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PreprocessReferenceIdentity {
    reference_id: String,
    source_path: PathBuf,
    source_sha256: String,
    cache_key: String,
    artifact_directory: PathBuf,
}

#[derive(Deserialize)]
struct AnalysisIdentity {
    schema_id: String,
    schema_version: u32,
    analysis_id: String,
    run_id: String,
    #[serde(default)]
    references: Vec<AnalysisReferenceIdentity>,
}

#[derive(Deserialize)]
struct AnalysisReferenceIdentity {
    reference_id: String,
    source_sha256: String,
    preprocess_cache_key: String,
    preprocess_protocol_version: u32,
    preprocess_implementation_version: String,
    preprocess_manifest_sha256: String,
    standard_preview_sha256: String,
}

#[derive(Deserialize)]
struct AssetStrategyIdentity {
    protocol_version: u32,
    analysis_id: String,
}

#[derive(Deserialize)]
struct GenerationTraceIdentity {
    canonical_document_sha256: String,
}

struct VerifiedStageEvidence {
    preprocess: PreprocessRunIdentity,
    analysis: AnalysisIdentity,
    asset_strategy: AssetStrategyIdentity,
    generated_document: serde_json::Value,
    generation_trace: GenerationTraceIdentity,
}

fn validate_run_result_consistency(
    repair: &RepairRunResult,
    preview: &PreviewRunResult,
) -> Result<(), TaskFailure> {
    match repair.status {
        RepairRunStatus::Passed => {
            let Some(final_document) = &repair.final_document else {
                return Err(TaskFailure::invalid(
                    "passed repair result requires a final document",
                ));
            };
            let expected_summary =
                crate::repair::node_tree_summary(&final_document.canonical_document_json)?;
            if repair.failure.is_some()
                || repair.node_tree_summary.as_ref() != Some(&expected_summary)
            {
                return Err(TaskFailure::invalid(
                    "passed repair result has inconsistent failure or node summary evidence",
                ));
            }
        }
        RepairRunStatus::Failed => {
            if repair.failure.is_none()
                || repair.final_document.is_some()
                || repair.node_tree_summary.is_some()
            {
                return Err(TaskFailure::invalid(
                    "failed repair result has inconsistent final evidence",
                ));
            }
        }
    }
    match preview.status {
        PreviewRunStatus::Passed => {
            if preview.failure.is_some()
                || !is_sha256(preview.screenshot_sha256.as_deref().unwrap_or_default())
                || preview.screenshot_bytes.is_none_or(|length| length == 0)
                || preview.process.exit_code != Some(0)
                || preview.process.timed_out
                || preview.process.cancelled
            {
                return Err(TaskFailure::invalid(
                    "passed preview result has inconsistent process or screenshot evidence",
                ));
            }
            validate_passed_preview_evidence(preview).map_err(|failure| {
                TaskFailure::invalid(format!(
                    "passed preview evidence failed strict revalidation: {} ({})",
                    failure.code, failure.detail
                ))
            })?;
        }
        PreviewRunStatus::Failed => {
            if preview.failure.is_none()
                || preview.screenshot_sha256.is_some()
                || preview.screenshot_bytes.is_some()
            {
                return Err(TaskFailure::invalid(
                    "failed preview result has inconsistent success evidence",
                ));
            }
        }
    }
    Ok(())
}

fn validate_stage_links(
    run_root: &Path,
    run_id: &str,
    links: &StageEvidenceLinks,
    repair: &RepairRunResult,
    preview: &PreviewRunResult,
) -> Result<(), TaskFailure> {
    if links.input_references.is_empty()
        || links.input_references.len() + links.draft_assets.len() > 64
    {
        return Err(TaskFailure::invalid(
            "run stage evidence must link 1..=64 input and draft resource artifacts",
        ));
    }
    if links.input_preprocess_manifest.relative_path != "input/preprocessed/manifest.json" {
        return Err(TaskFailure::invalid(
            "run stage evidence must use the Stage 3 preprocess manifest",
        ));
    }
    let mut seen_paths = BTreeSet::new();
    let mut total_bytes = 0_u64;
    let preprocess_bytes = verify_stage_artifact(
        run_root,
        &links.input_preprocess_manifest,
        MAX_JSON_ARTIFACT_BYTES as u64,
        &mut total_bytes,
        &mut seen_paths,
    )?;
    let analysis_bytes = verify_stage_artifact(
        run_root,
        &links.reference_analysis,
        MAX_JSON_ARTIFACT_BYTES as u64,
        &mut total_bytes,
        &mut seen_paths,
    )?;
    let asset_strategy_bytes = verify_stage_artifact(
        run_root,
        &links.asset_strategy,
        MAX_JSON_ARTIFACT_BYTES as u64,
        &mut total_bytes,
        &mut seen_paths,
    )?;
    let generated_document_bytes = verify_stage_artifact(
        run_root,
        &links.generated_document,
        MAX_JSON_ARTIFACT_BYTES as u64,
        &mut total_bytes,
        &mut seen_paths,
    )?;
    let generation_trace_bytes = verify_stage_artifact(
        run_root,
        &links.generation_trace,
        MAX_JSON_ARTIFACT_BYTES as u64,
        &mut total_bytes,
        &mut seen_paths,
    )?;
    let input_reference_hashes = links
        .input_references
        .iter()
        .map(|link| {
            verify_stage_artifact(
                run_root,
                link,
                MAX_STAGE_RESOURCE_BYTES,
                &mut total_bytes,
                &mut seen_paths,
            )
            .map(|_| link.sha256.as_str())
        })
        .collect::<Result<Vec<_>, _>>()?;
    for link in &links.draft_assets {
        verify_stage_artifact(
            run_root,
            link,
            MAX_STAGE_RESOURCE_BYTES,
            &mut total_bytes,
            &mut seen_paths,
        )?;
    }

    let verified = VerifiedStageEvidence {
        preprocess: parse_identity(&preprocess_bytes, "preprocess run manifest")?,
        analysis: parse_identity(&analysis_bytes, "reference analysis")?,
        asset_strategy: parse_identity(&asset_strategy_bytes, "asset strategy")?,
        generated_document: parse_identity(&generated_document_bytes, "generated document")?,
        generation_trace: parse_identity(&generation_trace_bytes, "generation trace")?,
    };
    validate_stage_identities(
        run_id,
        &verified,
        &input_reference_hashes,
        &generated_document_bytes,
        repair,
        preview,
    )
}

fn verify_stage_artifact(
    run_root: &Path,
    link: &ArtifactLink,
    maximum_bytes: u64,
    total_bytes: &mut u64,
    seen_paths: &mut BTreeSet<String>,
) -> Result<Vec<u8>, TaskFailure> {
    if !safe_relative_path(&link.relative_path) || !is_sha256(&link.sha256) || link.byte_length == 0
    {
        return Err(TaskFailure::invalid("run stage evidence link is invalid"));
    }
    if !seen_paths.insert(link.relative_path.to_ascii_lowercase()) {
        return Err(TaskFailure::invalid(
            "run stage evidence contains a duplicate artifact path",
        ));
    }
    let next_total = total_bytes
        .checked_add(link.byte_length)
        .ok_or_else(|| TaskFailure::invalid("run stage evidence byte count overflowed"))?;
    if next_total > MAX_STAGE_EVIDENCE_BYTES {
        return Err(TaskFailure::invalid(
            "run stage evidence exceeds its aggregate byte budget",
        ));
    }
    let relative_path = Path::new(&link.relative_path);
    reject_reparse_components(run_root, relative_path)?;
    let joined = run_root.join(relative_path);
    let canonical = fs::canonicalize(&joined).map_err(|_| {
        TaskFailure::invalid(format!(
            "run stage evidence is missing: {}",
            link.relative_path
        ))
    })?;
    if !canonical.starts_with(run_root) || canonical == run_root {
        return Err(TaskFailure::invalid(
            "run stage evidence escapes the current run root",
        ));
    }
    let bytes = read_bounded_stable_file(&canonical, maximum_bytes, "run stage evidence")?;
    reject_reparse_components(run_root, relative_path)?;
    let canonical_after = fs::canonicalize(&joined)
        .map_err(|_| TaskFailure::invalid("run stage evidence changed while it was read"))?;
    if canonical_after != canonical {
        return Err(TaskFailure::invalid(
            "run stage evidence path changed while it was read",
        ));
    }
    if bytes.len() as u64 != link.byte_length || hash_bytes(&bytes) != link.sha256 {
        return Err(TaskFailure::invalid(format!(
            "run stage evidence hash or byte length changed: {}",
            link.relative_path
        )));
    }
    *total_bytes = next_total;
    Ok(bytes)
}

fn parse_identity<T: for<'de> Deserialize<'de>>(
    bytes: &[u8],
    label: &str,
) -> Result<T, TaskFailure> {
    serde_json::from_slice(bytes)
        .map_err(|_| TaskFailure::invalid(format!("{label} identity JSON is malformed")))
}

fn validate_stage_identities(
    run_id: &str,
    verified: &VerifiedStageEvidence,
    input_reference_hashes: &[&str],
    generated_document_bytes: &[u8],
    repair: &RepairRunResult,
    preview: &PreviewRunResult,
) -> Result<(), TaskFailure> {
    if verified.preprocess.run_id != run_id || verified.analysis.run_id != run_id {
        return Err(TaskFailure::invalid(
            "run stage evidence belongs to a different run ID",
        ));
    }
    if verified.preprocess.protocol_version == 0
        || verified.preprocess.implementation_version.trim().is_empty()
        || verified.analysis.schema_id != "ui-reference-analysis"
        || verified.analysis.schema_version == 0
        || verified.asset_strategy.protocol_version == 0
    {
        return Err(TaskFailure::invalid(
            "run stage evidence has an unsupported protocol identity",
        ));
    }
    if verified.analysis.analysis_id != verified.asset_strategy.analysis_id {
        return Err(TaskFailure::invalid(
            "asset strategy analysis ID differs from the reference analysis",
        ));
    }
    validate_reference_identities(verified, input_reference_hashes)?;
    if verified.generated_document != repair.initial_document
        || hash_json_value(&repair.initial_document) != repair.initial_document_sha256
    {
        return Err(TaskFailure::invalid(
            "generated document evidence differs from the repair input document",
        ));
    }
    let document_id = verified
        .generated_document
        .get("document_id")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| TaskFailure::invalid("generated document has no document ID"))?;
    if verified.generation_trace.canonical_document_sha256 != hash_bytes(generated_document_bytes) {
        return Err(TaskFailure::invalid(
            "generation trace canonical document hash differs from the linked document",
        ));
    }
    if let Some(final_document) = &repair.final_document {
        let final_value: serde_json::Value =
            serde_json::from_str(&final_document.canonical_document_json)
                .map_err(|_| TaskFailure::invalid("final repair document JSON is malformed"))?;
        if final_value
            .get("document_id")
            .and_then(serde_json::Value::as_str)
            != Some(document_id)
            || final_document.trace.canonical_document_sha256
                != hash_bytes(final_document.canonical_document_json.as_bytes())
            || preview.command.canonical_document_sha256
                != final_document.trace.canonical_document_sha256
        {
            return Err(TaskFailure::invalid(
                "repair, generation, and preview document identities disagree",
            ));
        }
    }
    Ok(())
}

fn validate_reference_identities(
    verified: &VerifiedStageEvidence,
    input_reference_hashes: &[&str],
) -> Result<(), TaskFailure> {
    if verified.preprocess.references.is_empty()
        || verified.preprocess.references.len() != verified.analysis.references.len()
    {
        return Err(TaskFailure::invalid(
            "preprocess and analysis reference counts disagree",
        ));
    }
    let preprocess_by_id = verified
        .preprocess
        .references
        .iter()
        .map(|reference| (reference.reference_id.as_str(), reference))
        .collect::<BTreeMap<_, _>>();
    if preprocess_by_id.len() != verified.preprocess.references.len() {
        return Err(TaskFailure::invalid(
            "preprocess evidence contains duplicate reference IDs",
        ));
    }
    let analysis_reference_ids = verified
        .analysis
        .references
        .iter()
        .map(|reference| reference.reference_id.as_str())
        .collect::<BTreeSet<_>>();
    if analysis_reference_ids.len() != verified.analysis.references.len()
        || analysis_reference_ids != preprocess_by_id.keys().copied().collect()
    {
        return Err(TaskFailure::invalid(
            "preprocess and analysis reference ID sets disagree",
        ));
    }
    let mut claimed_hashes = BTreeSet::new();
    for reference in &verified.analysis.references {
        let preprocess = preprocess_by_id
            .get(reference.reference_id.as_str())
            .ok_or_else(|| {
                TaskFailure::invalid("analysis references an image absent from preprocess evidence")
            })?;
        if preprocess.source_sha256 != reference.source_sha256
            || preprocess.cache_key != reference.preprocess_cache_key
            || !is_sha256(&preprocess.source_sha256)
            || !is_sha256(&preprocess.cache_key)
            || reference.preprocess_protocol_version != verified.preprocess.protocol_version
            || reference.preprocess_implementation_version
                != verified.preprocess.implementation_version
            || !is_sha256(&reference.preprocess_manifest_sha256)
            || !is_sha256(&reference.standard_preview_sha256)
            || preprocess.source_path.as_os_str().is_empty()
            || preprocess.artifact_directory != PathBuf::from(&reference.reference_id)
        {
            return Err(TaskFailure::invalid(
                "preprocess and analysis reference identities disagree",
            ));
        }
        claimed_hashes.extend([
            reference.source_sha256.as_str(),
            reference.preprocess_manifest_sha256.as_str(),
            reference.standard_preview_sha256.as_str(),
        ]);
        if !input_reference_hashes.iter().any(|hash| {
            *hash == reference.source_sha256
                || *hash == reference.preprocess_manifest_sha256
                || *hash == reference.standard_preview_sha256
        }) {
            return Err(TaskFailure::invalid(
                "analysis reference has no linked input evidence artifact",
            ));
        }
    }
    if input_reference_hashes
        .iter()
        .any(|hash| !claimed_hashes.contains(hash))
    {
        return Err(TaskFailure::invalid(
            "linked input evidence is not claimed by the analysis",
        ));
    }
    Ok(())
}

fn hash_json_value(value: &serde_json::Value) -> String {
    hash_bytes(&serde_json::to_vec(value).expect("serde_json::Value is serializable"))
}

fn canonical_regular_directory(path: &Path, label: &str) -> Result<PathBuf, TaskFailure> {
    let link_metadata = fs::symlink_metadata(path)
        .map_err(|_| TaskFailure::invalid(format!("{label} cannot be resolved")))?;
    if metadata_is_reparse(&link_metadata) {
        return Err(TaskFailure::invalid(format!(
            "{label} cannot be a symlink or reparse point"
        )));
    }
    let canonical = fs::canonicalize(path)
        .map_err(|_| TaskFailure::invalid(format!("{label} cannot be resolved")))?;
    let metadata = fs::metadata(&canonical)
        .map_err(|_| TaskFailure::invalid(format!("{label} cannot be inspected")))?;
    if !metadata.is_dir() {
        return Err(TaskFailure::invalid(format!(
            "{label} must be a regular directory"
        )));
    }
    Ok(canonical)
}

fn validate_stage3_run_layout(run_root: &Path) -> Result<(), TaskFailure> {
    for relative in [
        "input/preprocessed",
        "analysis",
        "draft",
        "assets",
        "preview",
        "logs",
    ] {
        reject_reparse_components(run_root, Path::new(relative))?;
        let directory = run_root.join(relative);
        let metadata = fs::metadata(&directory).map_err(|_| {
            TaskFailure::invalid("run bundle requires an existing Stage 3 run directory layout")
        })?;
        if !metadata.is_dir() {
            return Err(TaskFailure::invalid(
                "run bundle Stage 3 layout contains a non-directory entry",
            ));
        }
    }
    reject_reparse_components(run_root, Path::new("input/preprocessed/manifest.json"))?;
    let manifest = run_root.join("input/preprocessed/manifest.json");
    if !fs::metadata(&manifest).is_ok_and(|metadata| metadata.is_file()) {
        return Err(TaskFailure::invalid(
            "run bundle requires the existing Stage 3 preprocess manifest",
        ));
    }
    Ok(())
}

fn reject_existing_bundle_targets(run_root: &Path) -> Result<(), TaskFailure> {
    for name in [".bundle-partial", "bundle", "COMMITTED"] {
        let path = run_root.join(name);
        match fs::symlink_metadata(&path) {
            Ok(_) => {
                return Err(TaskFailure::new(
                    TaskFailureKind::OutputDirectoryConflict,
                    "run bundle target already exists and cannot be overwritten",
                    Some(path.display().to_string()),
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(bundle_write_failure(&path, error)),
        }
    }
    Ok(())
}

fn reject_reparse_components(root: &Path, relative: &Path) -> Result<(), TaskFailure> {
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(TaskFailure::invalid(
            "run evidence path must contain only normal relative components",
        ));
    }
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let std::path::Component::Normal(segment) = component else {
            unreachable!("relative path components were checked")
        };
        current.push(segment);
        let metadata = fs::symlink_metadata(&current).map_err(|_| {
            TaskFailure::invalid(format!(
                "run evidence path component cannot be resolved: {}",
                current.display()
            ))
        })?;
        if metadata_is_reparse(&metadata) {
            return Err(TaskFailure::invalid(
                "run evidence path cannot contain symlinks or reparse points",
            ));
        }
    }
    Ok(())
}

fn metadata_is_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        return metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    }
    #[cfg(not(windows))]
    false
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FileMetadataSnapshot {
    byte_length: u64,
    modified: Option<SystemTime>,
    created: Option<SystemTime>,
    readonly: bool,
    regular_file: bool,
}

impl FileMetadataSnapshot {
    fn capture(metadata: &fs::Metadata) -> Self {
        Self {
            byte_length: metadata.len(),
            modified: metadata.modified().ok(),
            created: metadata.created().ok(),
            readonly: metadata.permissions().readonly(),
            regular_file: metadata.is_file(),
        }
    }
}

fn read_bounded_stable_file(
    path: &Path,
    maximum_bytes: u64,
    label: &str,
) -> Result<Vec<u8>, TaskFailure> {
    read_bounded_stable_file_with_hook(path, maximum_bytes, label, || {})
}

fn read_bounded_stable_file_with_hook<F>(
    path: &Path,
    maximum_bytes: u64,
    label: &str,
    after_read: F,
) -> Result<Vec<u8>, TaskFailure>
where
    F: FnOnce(),
{
    let link_metadata = fs::symlink_metadata(path)
        .map_err(|_| TaskFailure::invalid(format!("{label} cannot be inspected")))?;
    if metadata_is_reparse(&link_metadata) {
        return Err(TaskFailure::invalid(format!(
            "{label} cannot be a symlink or reparse point"
        )));
    }
    let mut file = fs::File::open(path).map_err(|error| bundle_write_failure(path, error))?;
    let before_metadata = file
        .metadata()
        .map_err(|error| bundle_write_failure(path, error))?;
    let before = FileMetadataSnapshot::capture(&before_metadata);
    if !before.regular_file || before.byte_length == 0 || before.byte_length > maximum_bytes {
        return Err(TaskFailure::invalid(format!(
            "{label} must be a nonempty regular file within its byte budget"
        )));
    }
    let mut bytes = Vec::with_capacity(before.byte_length.min(maximum_bytes) as usize);
    Read::by_ref(&mut file)
        .take(maximum_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| bundle_write_failure(path, error))?;
    after_read();
    let after_handle = FileMetadataSnapshot::capture(
        &file
            .metadata()
            .map_err(|error| bundle_write_failure(path, error))?,
    );
    let after_path = FileMetadataSnapshot::capture(
        &fs::metadata(path).map_err(|error| bundle_write_failure(path, error))?,
    );
    let after_link_metadata =
        fs::symlink_metadata(path).map_err(|error| bundle_write_failure(path, error))?;
    if before != after_handle
        || before != after_path
        || metadata_is_reparse(&after_link_metadata)
        || bytes.len() as u64 != before.byte_length
        || bytes.len() as u64 > maximum_bytes
    {
        return Err(TaskFailure::invalid(format!(
            "{label} changed while it was read"
        )));
    }
    Ok(bytes)
}

fn write_json_artifact<T: Serialize>(
    root: &Path,
    kind: &str,
    relative_path: &str,
    value: &T,
    artifacts: &mut BTreeMap<String, RunArtifactRecord>,
) -> Result<(), TaskFailure> {
    let bytes = pretty_json_bytes(value)?;
    if bytes.len() > MAX_JSON_ARTIFACT_BYTES {
        return Err(TaskFailure::invalid(
            "run JSON artifact exceeds its byte budget",
        ));
    }
    write_new_artifact(root, kind, relative_path, &bytes, artifacts)
}

fn write_new_artifact(
    root: &Path,
    kind: &str,
    relative_path: &str,
    bytes: &[u8],
    artifacts: &mut BTreeMap<String, RunArtifactRecord>,
) -> Result<(), TaskFailure> {
    if !safe_artifact_key(kind)
        || !safe_relative_path(relative_path)
        || artifacts.contains_key(kind)
        || bytes.is_empty()
    {
        return Err(TaskFailure::invalid("run artifact identity is invalid"));
    }
    let path = root.join(relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| bundle_write_failure(parent, error))?;
    }
    write_new_synced(&path, bytes)?;
    artifacts.insert(
        kind.to_owned(),
        RunArtifactRecord {
            kind: kind.to_owned(),
            relative_path: relative_path.to_owned(),
            sha256: hash_bytes(bytes),
            byte_length: bytes.len() as u64,
        },
    );
    Ok(())
}

fn copy_optional_artifact(
    root: &Path,
    kind: &str,
    relative_path: &str,
    source: &Path,
    maximum_bytes: u64,
    artifacts: &mut BTreeMap<String, RunArtifactRecord>,
) -> Result<(), TaskFailure> {
    match fs::symlink_metadata(source) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(bundle_write_failure(source, error)),
    }
    let bytes = read_bounded_stable_file(source, maximum_bytes, "preview evidence artifact")?;
    write_new_artifact(root, kind, relative_path, &bytes, artifacts)
}

fn pretty_json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, TaskFailure> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|_| TaskFailure::invalid("run artifact cannot be serialized"))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn write_new_synced(path: &Path, bytes: &[u8]) -> Result<(), TaskFailure> {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|error| bundle_write_failure(path, error))?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|error| bundle_write_failure(path, error))
}

fn bundle_write_failure(path: &Path, error: std::io::Error) -> TaskFailure {
    TaskFailure::new(
        TaskFailureKind::OutputDirectoryConflict,
        format!("run bundle evidence write failed: {error}"),
        Some(path.display().to_string()),
    )
}

fn safe_artifact_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn safe_relative_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 320
        && value.is_ascii()
        && !value.starts_with('/')
        && !value.contains(['\\', ':', '\0', '\n', '\r'])
        && value.split('/').all(|segment| {
            !segment.is_empty()
                && segment != "."
                && segment != ".."
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        })
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Versioned manifest for the complete generate, audit, repair, and approval
/// lifecycle. This is intentionally separate from `UiGenerationRunManifest`:
/// that type is the sealed Stage 3 bundle consumed by promotion and must remain
/// backward compatible while the closed-loop runner is introduced incrementally.
pub const CLOSED_LOOP_RUN_MANIFEST_PROTOCOL_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClosedLoopRunState {
    Created,
    Preparing,
    Generating,
    Validating,
    Previewing,
    Auditing,
    PlanningFix,
    ApplyingFix,
    Verifying,
    AwaitingApproval,
    Passed,
    Failed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClosedLoopStatePolicy {
    pub allowed_from: &'static [ClosedLoopRunState],
    pub timeout_ms: u64,
    pub retryable: bool,
    pub terminal: bool,
    pub performs_external_call: bool,
}

impl ClosedLoopRunState {
    pub const fn policy(self) -> ClosedLoopStatePolicy {
        use ClosedLoopRunState::*;
        match self {
            Created => ClosedLoopStatePolicy {
                allowed_from: &[],
                timeout_ms: 0,
                retryable: false,
                terminal: false,
                performs_external_call: false,
            },
            Preparing => ClosedLoopStatePolicy {
                allowed_from: &[Created],
                timeout_ms: 60_000,
                retryable: true,
                terminal: false,
                performs_external_call: false,
            },
            Generating => ClosedLoopStatePolicy {
                allowed_from: &[Preparing, Verifying],
                timeout_ms: 300_000,
                retryable: true,
                terminal: false,
                performs_external_call: true,
            },
            Validating => ClosedLoopStatePolicy {
                allowed_from: &[Generating, ApplyingFix],
                timeout_ms: 60_000,
                retryable: true,
                terminal: false,
                performs_external_call: false,
            },
            Previewing => ClosedLoopStatePolicy {
                allowed_from: &[Validating],
                timeout_ms: 300_000,
                retryable: true,
                terminal: false,
                performs_external_call: true,
            },
            Auditing => ClosedLoopStatePolicy {
                allowed_from: &[Previewing, Verifying],
                timeout_ms: 300_000,
                retryable: true,
                terminal: false,
                performs_external_call: true,
            },
            PlanningFix => ClosedLoopStatePolicy {
                allowed_from: &[Auditing],
                timeout_ms: 60_000,
                retryable: true,
                terminal: false,
                performs_external_call: false,
            },
            ApplyingFix => ClosedLoopStatePolicy {
                allowed_from: &[PlanningFix],
                timeout_ms: 120_000,
                retryable: true,
                terminal: false,
                performs_external_call: true,
            },
            Verifying => ClosedLoopStatePolicy {
                allowed_from: &[ApplyingFix],
                timeout_ms: 300_000,
                retryable: true,
                terminal: false,
                performs_external_call: false,
            },
            AwaitingApproval => ClosedLoopStatePolicy {
                allowed_from: &[Auditing, PlanningFix, Verifying],
                timeout_ms: 7 * 24 * 60 * 60 * 1000,
                retryable: false,
                terminal: false,
                performs_external_call: false,
            },
            Passed => ClosedLoopStatePolicy {
                allowed_from: &[AwaitingApproval],
                timeout_ms: 0,
                retryable: false,
                terminal: true,
                performs_external_call: false,
            },
            Failed | Cancelled => ClosedLoopStatePolicy {
                allowed_from: &[],
                timeout_ms: 0,
                retryable: false,
                terminal: true,
                performs_external_call: false,
            },
        }
    }

    pub const fn is_terminal(self) -> bool {
        self.policy().terminal
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClosedLoopArtifactKind {
    GenerationInput,
    ReferenceManifest,
    UiDocument,
    Asset,
    Preview,
    Comparison,
    Analysis,
    Fix,
    Approval,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClosedLoopArtifactLinks {
    pub generation_input: ArtifactLink,
    pub reference_manifest: ArtifactLink,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ui_document: Option<ArtifactLink>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assets: Vec<ArtifactLink>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<ArtifactLink>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comparison: Option<ArtifactLink>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub analysis: Option<ArtifactLink>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<ArtifactLink>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval: Option<ArtifactLink>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClosedLoopViewport {
    pub logical_width: u32,
    pub logical_height: u32,
    pub device_scale_milli: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClosedLoopBudgetConfiguration {
    pub max_provider_calls: u32,
    pub max_elapsed_ms: u64,
    pub max_images: u32,
    pub max_input_units: u64,
    pub max_output_units: u64,
    pub max_estimated_cost_microunits: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClosedLoopRunProvenance {
    pub tool_version: String,
    pub source_commit: String,
    pub model_id: String,
    pub prompt_version: String,
    pub schema_id: String,
    pub schema_version: u32,
    pub algorithm_version: String,
    pub viewport: ClosedLoopViewport,
    pub theme_id: String,
    pub locale: String,
    pub budget: ClosedLoopBudgetConfiguration,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClosedLoopStateCheckpoint {
    pub state: ClosedLoopRunState,
    pub entered_at_unix_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at_unix_ms: Option<u64>,
    pub cache_key: String,
    pub attempt: u32,
}

impl ClosedLoopStateCheckpoint {
    pub const fn identity(&self, checkpoint_index: usize) -> ClosedLoopCheckpointIdentity {
        ClosedLoopCheckpointIdentity {
            checkpoint_index,
            state: self.state,
            attempt: self.attempt,
        }
    }
}

/// Identifies one persisted attempt, including repeated states in a repair loop.
/// State alone is deliberately not a recovery key.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClosedLoopCheckpointIdentity {
    pub checkpoint_index: usize,
    pub state: ClosedLoopRunState,
    pub attempt: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClosedLoopCancellation {
    pub reason: String,
    pub requested_at_unix_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClosedLoopRunManifest {
    pub protocol_version: u32,
    pub run_id: String,
    pub state: ClosedLoopRunState,
    pub created_at_unix_ms: u64,
    pub updated_at_unix_ms: u64,
    pub provenance: ClosedLoopRunProvenance,
    pub artifacts: ClosedLoopArtifactLinks,
    pub checkpoints: Vec<ClosedLoopStateCheckpoint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<TaskFailure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancellation: Option<ClosedLoopCancellation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClosedLoopRecoveryPlan {
    pub last_complete_checkpoint: Option<ClosedLoopCheckpointIdentity>,
    pub restart_checkpoint: ClosedLoopCheckpointIdentity,
    pub restart_cache_key: String,
    pub reusable_external_call_checkpoints: Vec<ClosedLoopCheckpointIdentity>,
}

impl ClosedLoopRunManifest {
    pub fn create(
        run_id: impl Into<String>,
        created_at_unix_ms: u64,
        provenance: ClosedLoopRunProvenance,
        artifacts: ClosedLoopArtifactLinks,
        created_cache_key: impl Into<String>,
    ) -> Result<Self, TaskFailure> {
        let manifest = Self {
            protocol_version: CLOSED_LOOP_RUN_MANIFEST_PROTOCOL_VERSION,
            run_id: run_id.into(),
            state: ClosedLoopRunState::Created,
            created_at_unix_ms,
            updated_at_unix_ms: created_at_unix_ms,
            provenance,
            artifacts,
            checkpoints: vec![ClosedLoopStateCheckpoint {
                state: ClosedLoopRunState::Created,
                entered_at_unix_ms: created_at_unix_ms,
                completed_at_unix_ms: None,
                cache_key: created_cache_key.into(),
                attempt: 1,
            }],
            failure: None,
            cancellation: None,
        };
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn transition(
        &mut self,
        next: ClosedLoopRunState,
        entered_at_unix_ms: u64,
        cache_key: impl Into<String>,
    ) -> Result<(), TaskFailure> {
        self.validate()?;
        let cache_key = cache_key.into();
        if !is_sha256(&cache_key) {
            return Err(TaskFailure::invalid(
                "closed-loop checkpoint cache keys must be SHA-256 values",
            ));
        }
        if self.state.is_terminal() {
            return Err(TaskFailure::invalid_state_transition(
                "a terminal closed-loop run cannot transition to another state",
            ));
        }
        if next.is_terminal() {
            return Err(TaskFailure::invalid_state_transition(
                "use fail, cancel, or approve instead of transitioning to a terminal state",
            ));
        }
        if !next.policy().allowed_from.contains(&self.state) {
            return Err(TaskFailure::invalid_state_transition(format!(
                "invalid closed-loop run transition from {:?} to {:?}",
                self.state, next
            )));
        }
        validate_state_artifacts(next, &self.artifacts)?;
        self.complete_current(entered_at_unix_ms)?;
        self.state = next;
        self.updated_at_unix_ms = entered_at_unix_ms;
        self.checkpoints.push(ClosedLoopStateCheckpoint {
            state: next,
            entered_at_unix_ms,
            completed_at_unix_ms: None,
            cache_key,
            attempt: 1,
        });
        self.validate()
    }

    pub fn approve(&mut self, approved_at_unix_ms: u64) -> Result<(), TaskFailure> {
        self.validate()?;
        if self.state != ClosedLoopRunState::AwaitingApproval {
            return Err(TaskFailure::invalid_state_transition(
                "only a run awaiting approval can be passed",
            ));
        }
        validate_state_artifacts(ClosedLoopRunState::Passed, &self.artifacts)?;
        self.complete_current(approved_at_unix_ms)?;
        self.state = ClosedLoopRunState::Passed;
        self.updated_at_unix_ms = approved_at_unix_ms;
        self.checkpoints.push(ClosedLoopStateCheckpoint {
            state: ClosedLoopRunState::Passed,
            entered_at_unix_ms: approved_at_unix_ms,
            completed_at_unix_ms: Some(approved_at_unix_ms),
            cache_key: self.current_checkpoint()?.cache_key.clone(),
            attempt: 1,
        });
        self.validate()
    }

    pub fn fail(
        &mut self,
        failed_at_unix_ms: u64,
        failure: TaskFailure,
    ) -> Result<(), TaskFailure> {
        self.validate()?;
        if self.state.is_terminal() {
            return Err(TaskFailure::invalid_state_transition(
                "a terminal closed-loop run cannot be failed again",
            ));
        }
        self.complete_current(failed_at_unix_ms)?;
        self.state = ClosedLoopRunState::Failed;
        self.updated_at_unix_ms = failed_at_unix_ms;
        self.failure = Some(failure);
        self.checkpoints.push(ClosedLoopStateCheckpoint {
            state: ClosedLoopRunState::Failed,
            entered_at_unix_ms: failed_at_unix_ms,
            completed_at_unix_ms: Some(failed_at_unix_ms),
            cache_key: self.current_checkpoint()?.cache_key.clone(),
            attempt: 1,
        });
        self.validate()
    }

    /// Cancellation wins only while the run is non-terminal. A concurrent caller
    /// that observes a completed approval receives `false` and cannot overwrite it.
    pub fn cancel(&mut self, cancelled_at_unix_ms: u64, reason: impl Into<String>) -> bool {
        if self.validate().is_err() || self.state.is_terminal() {
            return false;
        }
        let reason = reason.into();
        if reason.trim().is_empty() || self.complete_current(cancelled_at_unix_ms).is_err() {
            return false;
        }
        let cache_key = match self.current_checkpoint() {
            Ok(checkpoint) => checkpoint.cache_key.clone(),
            Err(_) => return false,
        };
        self.state = ClosedLoopRunState::Cancelled;
        self.updated_at_unix_ms = cancelled_at_unix_ms;
        self.cancellation = Some(ClosedLoopCancellation {
            reason,
            requested_at_unix_ms: cancelled_at_unix_ms,
        });
        self.checkpoints.push(ClosedLoopStateCheckpoint {
            state: ClosedLoopRunState::Cancelled,
            entered_at_unix_ms: cancelled_at_unix_ms,
            completed_at_unix_ms: Some(cancelled_at_unix_ms),
            cache_key,
            attempt: 1,
        });
        self.validate().is_ok()
    }

    pub fn recovery_plan(
        &self,
        expected_cache_keys: &BTreeMap<ClosedLoopCheckpointIdentity, String>,
    ) -> Result<ClosedLoopRecoveryPlan, TaskFailure> {
        self.validate()?;
        if self.state.is_terminal() {
            return Err(TaskFailure::invalid_state_transition(
                "a terminal closed-loop run cannot be resumed",
            ));
        }
        for (identity, key) in expected_cache_keys {
            if !is_sha256(key) {
                return Err(TaskFailure::invalid(
                    "closed-loop recovery cache keys must be SHA-256 values",
                ));
            }
            let checkpoint = self
                .checkpoints
                .get(identity.checkpoint_index)
                .ok_or_else(|| {
                    TaskFailure::new(
                        TaskFailureKind::CacheIncompatible,
                        "closed-loop recovery cache key refers to a missing checkpoint",
                        None,
                    )
                })?;
            if checkpoint.state != identity.state || checkpoint.attempt != identity.attempt {
                return Err(TaskFailure::new(
                    TaskFailureKind::CacheIncompatible,
                    "closed-loop recovery cache key does not match checkpoint identity",
                    None,
                ));
            }
        }

        let mut last_complete_index = None;
        let mut reusable_external_call_checkpoints = Vec::new();
        for (index, checkpoint) in self.checkpoints.iter().enumerate() {
            let identity = checkpoint.identity(index);
            if checkpoint.completed_at_unix_ms.is_none()
                || expected_cache_keys.get(&identity) != Some(&checkpoint.cache_key)
            {
                break;
            }
            last_complete_index = Some(index);
            if checkpoint.state.policy().performs_external_call {
                reusable_external_call_checkpoints.push(identity);
            }
        }

        let restart_index = last_complete_index.map_or(0, |index| index + 1);
        let restart_checkpoint = self.checkpoints.get(restart_index).ok_or_else(|| {
            TaskFailure::manifest_corrupt(
                "closed-loop manifest has no incomplete state after its last complete checkpoint",
            )
        })?;
        let restart_checkpoint_identity = restart_checkpoint.identity(restart_index);
        let restart_cache_key = expected_cache_keys
            .get(&restart_checkpoint_identity)
            .cloned()
            .ok_or_else(|| {
                TaskFailure::new(
                    TaskFailureKind::CacheIncompatible,
                    "closed-loop recovery requires a cache key for the exact restart checkpoint",
                    None,
                )
            })?;
        Ok(ClosedLoopRecoveryPlan {
            last_complete_checkpoint: last_complete_index
                .map(|index| self.checkpoints[index].identity(index)),
            restart_checkpoint: restart_checkpoint_identity,
            restart_cache_key,
            reusable_external_call_checkpoints,
        })
    }

    /// Keeps complete checkpoints through the recovery point and starts a new
    /// attempt at the first stale or incomplete state. Callers persist this
    /// manifest before any new external work begins.
    pub fn restart_from(
        &mut self,
        plan: &ClosedLoopRecoveryPlan,
        restarted_at_unix_ms: u64,
    ) -> Result<(), TaskFailure> {
        self.validate()?;
        if self.state.is_terminal() || plan.restart_checkpoint.state.is_terminal() {
            return Err(TaskFailure::invalid_state_transition(
                "a terminal closed-loop state cannot be restarted",
            ));
        }
        let restart_index = plan.restart_checkpoint.checkpoint_index;
        let restart_checkpoint = self.checkpoints.get(restart_index).ok_or_else(|| {
            TaskFailure::manifest_corrupt("recovery state is absent from manifest")
        })?;
        if restart_checkpoint.identity(restart_index) != plan.restart_checkpoint {
            return Err(TaskFailure::manifest_corrupt(
                "recovery checkpoint identity does not match the current manifest",
            ));
        }
        match &plan.last_complete_checkpoint {
            None if restart_index != 0 => {
                return Err(TaskFailure::manifest_corrupt(
                    "recovery plan omits the checkpoint before its restart boundary",
                ));
            }
            Some(identity) => {
                let expected_index = identity.checkpoint_index.checked_add(1).ok_or_else(|| {
                    TaskFailure::manifest_corrupt("recovery checkpoint index overflowed")
                })?;
                let checkpoint =
                    self.checkpoints
                        .get(identity.checkpoint_index)
                        .ok_or_else(|| {
                            TaskFailure::manifest_corrupt(
                                "recovery completion checkpoint is absent",
                            )
                        })?;
                if expected_index != restart_index
                    || checkpoint.identity(identity.checkpoint_index) != *identity
                    || checkpoint.completed_at_unix_ms.is_none()
                {
                    return Err(TaskFailure::manifest_corrupt(
                        "recovery plan does not use an exact complete checkpoint boundary",
                    ));
                }
            }
            None => {}
        }
        if !is_sha256(&plan.restart_cache_key)
            || self.checkpoints[..restart_index]
                .iter()
                .any(|checkpoint| checkpoint.completed_at_unix_ms.is_none())
            || (restart_index + 1 < self.checkpoints.len()
                && self.checkpoints[restart_index]
                    .completed_at_unix_ms
                    .is_none())
        {
            return Err(TaskFailure::manifest_corrupt(
                "closed-loop recovery plan does not start at a complete boundary",
            ));
        }
        let attempt = restart_checkpoint.attempt.checked_add(1).ok_or_else(|| {
            TaskFailure::manifest_corrupt("closed-loop attempt counter overflowed")
        })?;
        self.checkpoints.truncate(restart_index);
        self.state = plan.restart_checkpoint.state;
        self.updated_at_unix_ms = restarted_at_unix_ms;
        self.failure = None;
        self.cancellation = None;
        self.checkpoints.push(ClosedLoopStateCheckpoint {
            state: plan.restart_checkpoint.state,
            entered_at_unix_ms: restarted_at_unix_ms,
            completed_at_unix_ms: None,
            cache_key: plan.restart_cache_key.clone(),
            attempt,
        });
        self.validate()
    }

    pub fn to_json_bytes(&self) -> Result<Vec<u8>, TaskFailure> {
        self.validate()?;
        let bytes = pretty_json_bytes(self)?;
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err(TaskFailure::invalid(
                "closed-loop run manifest exceeds its byte budget",
            ));
        }
        Ok(bytes)
    }

    pub fn parse_json(bytes: &[u8]) -> Result<Self, TaskFailure> {
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err(TaskFailure::manifest_corrupt(
                "closed-loop run manifest exceeds its byte budget",
            ));
        }
        let manifest: Self = serde_json::from_slice(bytes).map_err(|_| {
            TaskFailure::manifest_corrupt("closed-loop run manifest is malformed or incomplete")
        })?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn load(path: &Path) -> Result<Self, TaskFailure> {
        let bytes =
            read_bounded_stable_file(path, MAX_MANIFEST_BYTES as u64, "closed-loop manifest")
                .map_err(|_| {
                    TaskFailure::manifest_corrupt("closed-loop run manifest cannot be read")
                })?;
        Self::parse_json(&bytes)
    }

    pub fn write_new(&self, path: &Path) -> Result<(), TaskFailure> {
        let bytes = self.to_json_bytes()?;
        write_new_synced(path, &bytes)
    }

    fn complete_current(&mut self, completed_at_unix_ms: u64) -> Result<(), TaskFailure> {
        let checkpoint = self.current_checkpoint_mut()?;
        if completed_at_unix_ms < checkpoint.entered_at_unix_ms {
            return Err(TaskFailure::manifest_corrupt(
                "closed-loop checkpoint completion predates its entry",
            ));
        }
        checkpoint.completed_at_unix_ms = Some(completed_at_unix_ms);
        Ok(())
    }

    fn current_checkpoint(&self) -> Result<&ClosedLoopStateCheckpoint, TaskFailure> {
        self.checkpoints.last().ok_or_else(|| {
            TaskFailure::manifest_corrupt("closed-loop manifest has no current checkpoint")
        })
    }

    fn current_checkpoint_mut(&mut self) -> Result<&mut ClosedLoopStateCheckpoint, TaskFailure> {
        self.checkpoints.last_mut().ok_or_else(|| {
            TaskFailure::manifest_corrupt("closed-loop manifest has no current checkpoint")
        })
    }

    fn validate(&self) -> Result<(), TaskFailure> {
        if self.protocol_version != CLOSED_LOOP_RUN_MANIFEST_PROTOCOL_VERSION {
            return Err(TaskFailure::protocol_incompatible(format!(
                "closed-loop run manifest protocol {} is unsupported",
                self.protocol_version
            )));
        }
        crate::directory::RunId::parse(&self.run_id).map_err(|_| {
            TaskFailure::manifest_corrupt("closed-loop run manifest has an unsafe run ID")
        })?;
        validate_closed_loop_provenance(&self.provenance)?;
        validate_closed_loop_artifacts(&self.artifacts)?;
        if self.checkpoints.is_empty()
            || self.checkpoints.first().map(|checkpoint| checkpoint.state)
                != Some(ClosedLoopRunState::Created)
            || self.checkpoints.last().map(|checkpoint| checkpoint.state) != Some(self.state)
        {
            return Err(TaskFailure::manifest_corrupt(
                "closed-loop checkpoints do not have a valid created/current boundary",
            ));
        }
        let mut previous: Option<&ClosedLoopStateCheckpoint> = None;
        for checkpoint in &self.checkpoints {
            if !is_sha256(&checkpoint.cache_key) || checkpoint.attempt == 0 {
                return Err(TaskFailure::manifest_corrupt(
                    "closed-loop checkpoint has an invalid cache key or attempt",
                ));
            }
            if checkpoint
                .completed_at_unix_ms
                .is_some_and(|completed| completed < checkpoint.entered_at_unix_ms)
            {
                return Err(TaskFailure::manifest_corrupt(
                    "closed-loop checkpoint completion predates its entry",
                ));
            }
            if let Some(previous) = previous {
                if previous.state.is_terminal()
                    || previous.completed_at_unix_ms.is_none()
                    || checkpoint.entered_at_unix_ms < previous.entered_at_unix_ms
                    || (!checkpoint.state.is_terminal()
                        && !checkpoint
                            .state
                            .policy()
                            .allowed_from
                            .contains(&previous.state))
                {
                    return Err(TaskFailure::manifest_corrupt(
                        "closed-loop checkpoint sequence has an illegal transition",
                    ));
                }
            }
            previous = Some(checkpoint);
        }
        let current = self.current_checkpoint()?;
        if self.state.is_terminal() {
            if current.completed_at_unix_ms.is_none() {
                return Err(TaskFailure::manifest_corrupt(
                    "closed-loop terminal checkpoint is incomplete",
                ));
            }
        } else if current.completed_at_unix_ms.is_some() {
            return Err(TaskFailure::manifest_corrupt(
                "closed-loop non-terminal checkpoint is already complete",
            ));
        }
        if self.updated_at_unix_ms < self.created_at_unix_ms
            || self.updated_at_unix_ms < current.entered_at_unix_ms
        {
            return Err(TaskFailure::manifest_corrupt(
                "closed-loop manifest timestamps are inconsistent",
            ));
        }
        match self.state {
            ClosedLoopRunState::Failed
                if self.failure.is_none()
                    || self.cancellation.is_some()
                    || self
                        .failure
                        .as_ref()
                        .is_some_and(|failure| failure.code() != failure.kind().code()) =>
            {
                return Err(TaskFailure::manifest_corrupt(
                    "failed closed-loop run has inconsistent failure data",
                ));
            }
            ClosedLoopRunState::Cancelled
                if self.cancellation.as_ref().is_none_or(|cancellation| {
                    cancellation.reason.trim().is_empty()
                        || cancellation.requested_at_unix_ms < self.created_at_unix_ms
                }) || self.failure.is_some() =>
            {
                return Err(TaskFailure::manifest_corrupt(
                    "cancelled closed-loop run has inconsistent cancellation data",
                ));
            }
            ClosedLoopRunState::Failed | ClosedLoopRunState::Cancelled => {}
            _ if self.failure.is_some() || self.cancellation.is_some() => {
                return Err(TaskFailure::manifest_corrupt(
                    "non-terminal closed-loop run contains terminal failure data",
                ));
            }
            _ => {}
        }
        validate_state_artifacts(self.state, &self.artifacts)
    }
}

fn validate_closed_loop_provenance(
    provenance: &ClosedLoopRunProvenance,
) -> Result<(), TaskFailure> {
    let required = [
        &provenance.tool_version,
        &provenance.source_commit,
        &provenance.model_id,
        &provenance.prompt_version,
        &provenance.schema_id,
        &provenance.algorithm_version,
        &provenance.theme_id,
        &provenance.locale,
    ];
    if required.iter().any(|value| value.trim().is_empty())
        || provenance.schema_version == 0
        || provenance.viewport.logical_width == 0
        || provenance.viewport.logical_height == 0
        || provenance.viewport.device_scale_milli == 0
        || provenance.budget.max_provider_calls == 0
        || provenance.budget.max_elapsed_ms == 0
        || provenance.budget.max_images == 0
        || provenance.budget.max_input_units == 0
        || provenance.budget.max_output_units == 0
        || provenance.budget.max_estimated_cost_microunits == 0
    {
        return Err(TaskFailure::manifest_corrupt(
            "closed-loop manifest has incomplete provenance or budget configuration",
        ));
    }
    Ok(())
}

fn validate_closed_loop_artifacts(artifacts: &ClosedLoopArtifactLinks) -> Result<(), TaskFailure> {
    let mut paths = BTreeSet::new();
    let links = std::iter::once(&artifacts.generation_input)
        .chain(std::iter::once(&artifacts.reference_manifest))
        .chain(artifacts.ui_document.iter())
        .chain(artifacts.assets.iter())
        .chain(artifacts.preview.iter())
        .chain(artifacts.comparison.iter())
        .chain(artifacts.analysis.iter())
        .chain(artifacts.fix.iter())
        .chain(artifacts.approval.iter());
    for link in links {
        ArtifactLink::new(&link.relative_path, &link.sha256, link.byte_length).map_err(|_| {
            TaskFailure::manifest_corrupt("closed-loop manifest has an invalid artifact link")
        })?;
        if !paths.insert(link.relative_path.to_ascii_lowercase()) {
            return Err(TaskFailure::manifest_corrupt(
                "closed-loop manifest contains duplicate artifact paths",
            ));
        }
    }
    Ok(())
}

fn validate_state_artifacts(
    state: ClosedLoopRunState,
    artifacts: &ClosedLoopArtifactLinks,
) -> Result<(), TaskFailure> {
    let required = |artifact: ClosedLoopArtifactKind, present: bool| {
        if present {
            Ok(())
        } else {
            Err(TaskFailure::manifest_corrupt(format!(
                "closed-loop state {:?} requires {:?} evidence",
                state, artifact
            )))
        }
    };
    match state {
        ClosedLoopRunState::Created
        | ClosedLoopRunState::Preparing
        | ClosedLoopRunState::Generating
        | ClosedLoopRunState::Failed
        | ClosedLoopRunState::Cancelled => Ok(()),
        ClosedLoopRunState::Validating | ClosedLoopRunState::Previewing => required(
            ClosedLoopArtifactKind::UiDocument,
            artifacts.ui_document.is_some(),
        ),
        ClosedLoopRunState::Auditing => {
            required(
                ClosedLoopArtifactKind::UiDocument,
                artifacts.ui_document.is_some(),
            )?;
            required(ClosedLoopArtifactKind::Preview, artifacts.preview.is_some())
        }
        ClosedLoopRunState::PlanningFix => {
            required(
                ClosedLoopArtifactKind::Comparison,
                artifacts.comparison.is_some(),
            )?;
            required(
                ClosedLoopArtifactKind::Analysis,
                artifacts.analysis.is_some(),
            )
        }
        ClosedLoopRunState::ApplyingFix | ClosedLoopRunState::Verifying => {
            required(ClosedLoopArtifactKind::Fix, artifacts.fix.is_some())
        }
        ClosedLoopRunState::AwaitingApproval => {
            required(
                ClosedLoopArtifactKind::UiDocument,
                artifacts.ui_document.is_some(),
            )?;
            required(ClosedLoopArtifactKind::Preview, artifacts.preview.is_some())?;
            required(
                ClosedLoopArtifactKind::Comparison,
                artifacts.comparison.is_some(),
            )?;
            required(
                ClosedLoopArtifactKind::Analysis,
                artifacts.analysis.is_some(),
            )
        }
        ClosedLoopRunState::Passed => {
            required(
                ClosedLoopArtifactKind::Approval,
                artifacts.approval.is_some(),
            )?;
            required(
                ClosedLoopArtifactKind::UiDocument,
                artifacts.ui_document.is_some(),
            )?;
            required(ClosedLoopArtifactKind::Preview, artifacts.preview.is_some())?;
            required(
                ClosedLoopArtifactKind::Comparison,
                artifacts.comparison.is_some(),
            )?;
            required(
                ClosedLoopArtifactKind::Analysis,
                artifacts.analysis.is_some(),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        preview::{PreviewCommandPlan, PreviewFailure, PreviewFailureKind, PreviewProcessRecord},
        repair::{RepairFailure, RepairFailureKind},
    };
    use serde_json::Value;

    const RUN_ID: &str = "test-run";

    struct RepositoryFixture {
        repository: tempfile::TempDir,
        run_root: PathBuf,
        links: StageEvidenceLinks,
        initial_document: Value,
        stage_bytes: BTreeMap<String, Vec<u8>>,
    }

    impl RepositoryFixture {
        fn create() -> Self {
            let repository = tempfile::tempdir().unwrap();
            let run_root = repository.path().join("summary/ui-generation").join(RUN_ID);
            for relative in [
                "input/preprocessed/primary",
                "analysis",
                "draft",
                "assets",
                "preview",
                "logs",
            ] {
                fs::create_dir_all(run_root.join(relative)).unwrap();
            }

            let source_sha256 = hash_bytes(b"original reference fixture");
            let preview_bytes = b"standard preview fixture".to_vec();
            let preview_sha256 = hash_bytes(&preview_bytes);
            let reference_manifest_bytes = json_bytes(serde_json::json!({
                "protocol_version": 1,
                "implementation_version": "ui-reference-preprocess-1",
                "reference_id": "primary",
                "source_sha256": source_sha256,
                "cache_key": "2".repeat(64),
                "artifacts": [{
                    "kind": "standard_preview",
                    "file_name": "preview.png",
                    "sha256": preview_sha256,
                    "byte_length": preview_bytes.len()
                }]
            }));
            let reference_manifest_sha256 = hash_bytes(&reference_manifest_bytes);
            let preprocess_bytes = json_bytes(serde_json::json!({
                "protocol_version": 1,
                "implementation_version": "ui-reference-preprocess-1",
                "run_id": RUN_ID,
                "references": [{
                    "reference_id": "primary",
                    "source_path": "C:/fixture/reference.png",
                    "source_sha256": source_sha256,
                    "cache_key": "2".repeat(64),
                    "artifact_directory": "primary"
                }]
            }));
            let analysis_bytes = json_bytes(serde_json::json!({
                "schema_id": "ui-reference-analysis",
                "schema_version": 1,
                "analysis_id": "analysis.stage8_fixture",
                "run_id": RUN_ID,
                "references": [{
                    "reference_id": "primary",
                    "source_sha256": source_sha256,
                    "preprocess_cache_key": "2".repeat(64),
                    "preprocess_protocol_version": 1,
                    "preprocess_implementation_version": "ui-reference-preprocess-1",
                    "preprocess_manifest_sha256": reference_manifest_sha256,
                    "standard_preview_sha256": preview_sha256
                }]
            }));
            let asset_strategy_bytes = json_bytes(serde_json::json!({
                "protocol_version": 1,
                "analysis_id": "analysis.stage8_fixture"
            }));
            let initial_document = serde_json::json!({
                "schema_version": 1,
                "document_id": "generated.stage8_fixture",
                "assets": {},
                "tokens": {},
                "root": {
                    "type": "container",
                    "id": "page.root",
                    "children": []
                }
            });
            let generated_document_bytes = json_bytes(initial_document.clone());
            let generation_trace_bytes = json_bytes(serde_json::json!({
                "canonical_document_sha256": hash_bytes(&generated_document_bytes)
            }));
            let draft_asset_bytes = b"draft asset fixture".to_vec();

            let files = [
                ("input/preprocessed/manifest.json", preprocess_bytes),
                (
                    "input/preprocessed/primary/manifest.json",
                    reference_manifest_bytes,
                ),
                ("input/preprocessed/primary/preview.png", preview_bytes),
                ("analysis/reference-analysis.json", analysis_bytes),
                ("analysis/asset-strategy.json", asset_strategy_bytes),
                ("draft/generated-document.json", generated_document_bytes),
                ("logs/generation-trace.json", generation_trace_bytes),
                ("assets/draft.bin", draft_asset_bytes),
            ];
            let mut stage_bytes = BTreeMap::new();
            for (relative, bytes) in files {
                fs::write(run_root.join(relative), &bytes).unwrap();
                stage_bytes.insert(relative.to_owned(), bytes);
            }
            fs::write(run_root.join("input/stage3-sentinel.txt"), b"preserve me").unwrap();

            let links = StageEvidenceLinks {
                input_preprocess_manifest: artifact_link(
                    &run_root,
                    "input/preprocessed/manifest.json",
                ),
                input_references: vec![artifact_link(
                    &run_root,
                    "input/preprocessed/primary/preview.png",
                )],
                reference_analysis: artifact_link(&run_root, "analysis/reference-analysis.json"),
                asset_strategy: artifact_link(&run_root, "analysis/asset-strategy.json"),
                draft_assets: vec![artifact_link(&run_root, "assets/draft.bin")],
                generated_document: artifact_link(&run_root, "draft/generated-document.json"),
                generation_trace: artifact_link(&run_root, "logs/generation-trace.json"),
            };
            Self {
                repository,
                run_root,
                links,
                initial_document,
                stage_bytes,
            }
        }

        fn repair(&self) -> RepairRunResult {
            failed_repair(self.initial_document.clone())
        }
    }

    fn json_bytes(value: Value) -> Vec<u8> {
        let mut bytes = serde_json::to_vec_pretty(&value).unwrap();
        bytes.push(b'\n');
        bytes
    }

    fn artifact_link(run_root: &Path, relative: &str) -> ArtifactLink {
        let bytes = fs::read(run_root.join(relative)).unwrap();
        ArtifactLink::new(relative, hash_bytes(&bytes), bytes.len() as u64).unwrap()
    }

    fn closed_loop_link(relative_path: &str) -> ArtifactLink {
        ArtifactLink::new(relative_path, "a".repeat(64), 1).unwrap()
    }

    fn closed_loop_provenance() -> ClosedLoopRunProvenance {
        ClosedLoopRunProvenance {
            tool_version: "ui-generation-0.1.0".to_owned(),
            source_commit: "0123456789abcdef".to_owned(),
            model_id: "offline-fixture".to_owned(),
            prompt_version: "ui-generation-v1".to_owned(),
            schema_id: "ui-document".to_owned(),
            schema_version: 1,
            algorithm_version: "reference-analysis-v1".to_owned(),
            viewport: ClosedLoopViewport {
                logical_width: 390,
                logical_height: 844,
                device_scale_milli: 3_000,
            },
            theme_id: "default".to_owned(),
            locale: "zh_cn".to_owned(),
            budget: ClosedLoopBudgetConfiguration {
                max_provider_calls: 6,
                max_elapsed_ms: 300_000,
                max_images: 12,
                max_input_units: 1_000_000,
                max_output_units: 250_000,
                max_estimated_cost_microunits: 10_000_000,
            },
        }
    }

    fn closed_loop_artifacts() -> ClosedLoopArtifactLinks {
        ClosedLoopArtifactLinks {
            generation_input: closed_loop_link("input/task.json"),
            reference_manifest: closed_loop_link("input/reference-manifest.json"),
            ui_document: None,
            assets: Vec::new(),
            preview: None,
            comparison: None,
            analysis: None,
            fix: None,
            approval: None,
        }
    }

    fn cache_key(letter: char) -> String {
        letter.to_string().repeat(64)
    }

    fn closed_loop_manifest() -> ClosedLoopRunManifest {
        ClosedLoopRunManifest::create(
            "closed-loop-fixture",
            1,
            closed_loop_provenance(),
            closed_loop_artifacts(),
            cache_key('a'),
        )
        .unwrap()
    }

    fn advance_to_auditing() -> ClosedLoopRunManifest {
        let mut manifest = closed_loop_manifest();
        manifest
            .transition(ClosedLoopRunState::Preparing, 2, cache_key('b'))
            .unwrap();
        manifest
            .transition(ClosedLoopRunState::Generating, 3, cache_key('c'))
            .unwrap();
        manifest.artifacts.ui_document = Some(closed_loop_link("draft/document.json"));
        manifest
            .transition(ClosedLoopRunState::Validating, 4, cache_key('d'))
            .unwrap();
        manifest
            .transition(ClosedLoopRunState::Previewing, 5, cache_key('e'))
            .unwrap();
        manifest.artifacts.preview = Some(closed_loop_link("preview/phone-portrait.png"));
        manifest
            .transition(ClosedLoopRunState::Auditing, 6, cache_key('f'))
            .unwrap();
        manifest
    }

    fn expected_checkpoint_cache_keys(
        manifest: &ClosedLoopRunManifest,
    ) -> BTreeMap<ClosedLoopCheckpointIdentity, String> {
        manifest
            .checkpoints
            .iter()
            .enumerate()
            .map(|(index, checkpoint)| (checkpoint.identity(index), checkpoint.cache_key.clone()))
            .collect()
    }

    fn advance_through_fix_cycle_to_auditing() -> ClosedLoopRunManifest {
        let mut manifest = advance_to_auditing();
        manifest.artifacts.comparison = Some(closed_loop_link("audit/comparison-first.json"));
        manifest.artifacts.analysis = Some(closed_loop_link("audit/analysis-first.json"));
        manifest
            .transition(ClosedLoopRunState::PlanningFix, 7, cache_key('1'))
            .unwrap();
        manifest.artifacts.fix = Some(closed_loop_link("fix/plan-first.json"));
        manifest
            .transition(ClosedLoopRunState::ApplyingFix, 8, cache_key('2'))
            .unwrap();
        manifest
            .transition(ClosedLoopRunState::Validating, 9, cache_key('3'))
            .unwrap();
        manifest
            .transition(ClosedLoopRunState::Previewing, 10, cache_key('4'))
            .unwrap();
        manifest.artifacts.preview = Some(closed_loop_link("preview/phone-portrait-after-fix.png"));
        manifest
            .transition(ClosedLoopRunState::Auditing, 11, cache_key('5'))
            .unwrap();
        manifest
    }

    fn failed_repair(initial_document: Value) -> RepairRunResult {
        RepairRunResult {
            status: RepairRunStatus::Failed,
            initial_document_sha256: hash_json_value(&initial_document),
            initial_document,
            rounds: Vec::new(),
            final_document: None,
            node_tree_summary: None,
            failure: Some(RepairFailure {
                kind: RepairFailureKind::MaximumRoundsReached,
                code: "UI_GENERATION_REPAIR_MAXIMUM_ROUNDS_REACHED".to_owned(),
                detail: "fixture failure".to_owned(),
            }),
        }
    }

    fn failed_preview(directory: &Path) -> PreviewRunResult {
        PreviewRunResult {
            status: PreviewRunStatus::Failed,
            command: PreviewCommandPlan {
                program: "cargo".to_owned(),
                arguments: Vec::new(),
                working_directory: directory.to_path_buf(),
                document_path: directory.join("document.json"),
                screenshot_path: directory.join("preview.png"),
                result_path: directory.join("preview-result.json"),
                log_path: directory.join("preview.log"),
                width: 390,
                height: 844,
                page_state: "initial".to_owned(),
                timeout_frames: 1200,
                stable_frames: 30,
                process_timeout_ms: 120_000,
                canonical_document_sha256: "c".repeat(64),
            },
            process: PreviewProcessRecord {
                exit_code: Some(0),
                timed_out: false,
                cancelled: false,
                elapsed_ms: 10,
            },
            screenshot_sha256: None,
            screenshot_bytes: None,
            failure: Some(PreviewFailure {
                kind: PreviewFailureKind::ProcessFailed,
                code: "UI_GENERATION_PREVIEW_PROCESS_FAILED".to_owned(),
                detail: "fixture failure".to_owned(),
            }),
        }
    }

    fn forged_passed_preview(directory: &Path, screenshot: &[u8]) -> PreviewRunResult {
        fs::create_dir_all(directory).unwrap();
        let screenshot_path = directory.join("preview.png");
        let result_path = directory.join("preview-result.json");
        let log_path = directory.join("preview.log");
        fs::write(&screenshot_path, screenshot).unwrap();
        fs::write(&log_path, b"fixture preview log").unwrap();
        let canonical_document_sha256 = "c".repeat(64);
        fs::write(
            &result_path,
            serde_json::to_vec(&serde_json::json!({
                "protocol_version": 1,
                "status": "passed",
                "document_id": "generated.stage8_fixture",
                "canonical_document_sha256": canonical_document_sha256,
                "width": 390,
                "height": 844,
                "elapsed_frames": 60,
                "stable_frames": 30,
                "screenshot_path": screenshot_path.to_string_lossy(),
                "captured_size": [390, 844]
            }))
            .unwrap(),
        )
        .unwrap();
        PreviewRunResult {
            status: PreviewRunStatus::Passed,
            command: PreviewCommandPlan {
                program: "cargo".to_owned(),
                arguments: Vec::new(),
                working_directory: directory.to_path_buf(),
                document_path: directory.join("document.json"),
                screenshot_path,
                result_path,
                log_path,
                width: 390,
                height: 844,
                page_state: "initial".to_owned(),
                timeout_frames: 1200,
                stable_frames: 30,
                process_timeout_ms: 120_000,
                canonical_document_sha256,
            },
            process: PreviewProcessRecord {
                exit_code: Some(0),
                timed_out: false,
                cancelled: false,
                elapsed_ms: 10,
            },
            screenshot_sha256: Some(hash_bytes(screenshot)),
            screenshot_bytes: Some(screenshot.len() as u64),
            failure: None,
        }
    }

    #[test]
    fn existing_stage3_run_links_real_evidence_and_commits_last_without_clobber() {
        let fixture = RepositoryFixture::create();
        let preview_dir = tempfile::tempdir().unwrap();
        fs::write(preview_dir.path().join("preview.log"), b"fixture log").unwrap();
        fs::write(
            preview_dir.path().join("preview-result.json"),
            b"{\"status\":\"failed\"}",
        )
        .unwrap();
        let repair = fixture.repair();
        let preview = failed_preview(preview_dir.path());
        let persisted = persist_run_bundle(
            fixture.repository.path(),
            RUN_ID,
            fixture.links.clone(),
            &repair,
            &preview,
        )
        .unwrap();
        assert!(persisted.committed_marker.is_file());
        assert!(persisted.manifest_path.is_file());
        assert!(!persisted.run_root.join(".bundle-partial").exists());
        assert_eq!(
            fs::read(persisted.run_root.join("input/stage3-sentinel.txt")).unwrap(),
            b"preserve me"
        );
        let manifest: Value =
            serde_json::from_slice(&fs::read(&persisted.manifest_path).unwrap()).unwrap();
        assert_eq!(
            manifest["stage_evidence"]["reference_analysis"]["relative_path"],
            "analysis/reference-analysis.json"
        );
        assert_eq!(
            manifest["stage_evidence"]["reference_analysis"]["sha256"],
            fixture.links.reference_analysis.sha256
        );
        assert_eq!(
            manifest["stage_evidence"]["reference_analysis"]["byte_length"],
            fixture.links.reference_analysis.byte_length
        );
        assert_eq!(manifest["repair_round_count"], 0);
        assert!(manifest["artifacts"]["repair_run"].is_object());
        assert!(manifest["artifacts"]["preview_process_result"].is_object());
        assert!(manifest["artifacts"]["preview_log"].is_object());
        for (relative, before) in &fixture.stage_bytes {
            assert_eq!(&fs::read(fixture.run_root.join(relative)).unwrap(), before);
        }

        let conflict = persist_run_bundle(
            fixture.repository.path(),
            RUN_ID,
            fixture.links.clone(),
            &repair,
            &preview,
        )
        .unwrap_err();
        assert_eq!(conflict.kind(), TaskFailureKind::OutputDirectoryConflict);
        assert!(persisted.committed_marker.is_file());
    }

    #[test]
    fn missing_and_forged_stage_artifacts_fail_without_committed_marker() {
        assert!(ArtifactLink::new("../escape.json", "a".repeat(64), 1).is_err());
        let fixture = RepositoryFixture::create();
        fs::remove_file(fixture.run_root.join("analysis/reference-analysis.json")).unwrap();
        let preview_dir = tempfile::tempdir().unwrap();
        let result = persist_run_bundle(
            fixture.repository.path(),
            RUN_ID,
            fixture.links.clone(),
            &fixture.repair(),
            &failed_preview(preview_dir.path()),
        );
        assert!(result.is_err());
        assert!(!fixture.run_root.join("COMMITTED").exists());
        assert!(!fixture.run_root.join(".bundle-partial").exists());

        let fixture = RepositoryFixture::create();
        let mut forged_hash = fixture.links.clone();
        forged_hash.reference_analysis.sha256 = "f".repeat(64);
        assert!(
            persist_run_bundle(
                fixture.repository.path(),
                RUN_ID,
                forged_hash,
                &fixture.repair(),
                &failed_preview(preview_dir.path()),
            )
            .is_err()
        );
        assert!(!fixture.run_root.join("COMMITTED").exists());

        let mut forged_length = fixture.links.clone();
        forged_length.reference_analysis.byte_length += 1;
        assert!(
            persist_run_bundle(
                fixture.repository.path(),
                RUN_ID,
                forged_length,
                &fixture.repair(),
                &failed_preview(preview_dir.path()),
            )
            .is_err()
        );
        assert!(!fixture.run_root.join("COMMITTED").exists());
    }

    #[test]
    fn cross_run_identity_and_duplicate_links_are_rejected() {
        let fixture = RepositoryFixture::create();
        assert!(
            ArtifactLink::new(
                "../another-run/analysis/reference-analysis.json",
                "a".repeat(64),
                1,
            )
            .is_err()
        );

        let mut analysis: Value = serde_json::from_slice(
            &fs::read(fixture.run_root.join("analysis/reference-analysis.json")).unwrap(),
        )
        .unwrap();
        analysis["run_id"] = Value::String("another-run".to_owned());
        fs::write(
            fixture.run_root.join("analysis/reference-analysis.json"),
            json_bytes(analysis),
        )
        .unwrap();
        let mut links = fixture.links.clone();
        links.reference_analysis =
            artifact_link(&fixture.run_root, "analysis/reference-analysis.json");
        let preview_dir = tempfile::tempdir().unwrap();
        assert!(
            persist_run_bundle(
                fixture.repository.path(),
                RUN_ID,
                links,
                &fixture.repair(),
                &failed_preview(preview_dir.path()),
            )
            .is_err()
        );
        assert!(!fixture.run_root.join("COMMITTED").exists());

        let fixture = RepositoryFixture::create();
        let mut duplicate = fixture.links.clone();
        duplicate.asset_strategy = duplicate.reference_analysis.clone();
        assert!(
            persist_run_bundle(
                fixture.repository.path(),
                RUN_ID,
                duplicate,
                &fixture.repair(),
                &failed_preview(preview_dir.path()),
            )
            .is_err()
        );
        assert!(!fixture.run_root.join("COMMITTED").exists());
    }

    #[test]
    fn existing_publish_targets_are_never_overwritten() {
        for target in [".bundle-partial", "bundle", "COMMITTED"] {
            let fixture = RepositoryFixture::create();
            if target == "COMMITTED" {
                fs::write(fixture.run_root.join(target), b"existing marker").unwrap();
            } else {
                fs::create_dir(fixture.run_root.join(target)).unwrap();
            }
            let preview_dir = tempfile::tempdir().unwrap();
            let error = persist_run_bundle(
                fixture.repository.path(),
                RUN_ID,
                fixture.links.clone(),
                &fixture.repair(),
                &failed_preview(preview_dir.path()),
            )
            .unwrap_err();
            assert_eq!(error.kind(), TaskFailureKind::OutputDirectoryConflict);
            assert!(fixture.run_root.join(target).exists());
        }
    }

    #[test]
    fn oversized_preview_failure_never_creates_committed_marker() {
        let fixture = RepositoryFixture::create();
        let preview_dir = tempfile::tempdir().unwrap();
        fs::write(
            preview_dir.path().join("preview.log"),
            vec![b'x'; MAX_LOG_BYTES as usize + 1],
        )
        .unwrap();
        let result = persist_run_bundle(
            fixture.repository.path(),
            RUN_ID,
            fixture.links.clone(),
            &fixture.repair(),
            &failed_preview(preview_dir.path()),
        );
        assert!(result.is_err());
        assert!(!fixture.run_root.join("COMMITTED").exists());
        assert!(fixture.run_root.join(".bundle-partial").is_dir());
    }

    #[test]
    fn stable_reader_rejects_a_file_changed_between_read_and_metadata_check() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("changing.bin");
        fs::write(&path, b"before").unwrap();
        let result = read_bounded_stable_file_with_hook(&path, 1024, "changing fixture", || {
            fs::write(&path, b"after with a different length").unwrap();
        });
        assert!(result.unwrap_err().message().contains("changed while"));
    }

    #[test]
    fn missing_stage3_run_and_inconsistent_results_are_not_published() {
        let repository = tempfile::tempdir().unwrap();
        fs::create_dir_all(repository.path().join("summary/ui-generation")).unwrap();
        let evidence_fixture = RepositoryFixture::create();
        let preview_dir = tempfile::tempdir().unwrap();
        assert!(
            persist_run_bundle(
                repository.path(),
                RUN_ID,
                evidence_fixture.links.clone(),
                &evidence_fixture.repair(),
                &failed_preview(preview_dir.path()),
            )
            .is_err()
        );
        assert!(
            !repository
                .path()
                .join("summary/ui-generation")
                .join(RUN_ID)
                .exists()
        );

        let fixture = RepositoryFixture::create();
        let mut forged_repair = fixture.repair();
        forged_repair.status = RepairRunStatus::Passed;
        forged_repair.failure = None;
        assert!(
            persist_run_bundle(
                fixture.repository.path(),
                RUN_ID,
                fixture.links.clone(),
                &forged_repair,
                &failed_preview(preview_dir.path()),
            )
            .is_err()
        );
        assert!(!fixture.run_root.join(".bundle-partial").exists());
        assert!(!fixture.run_root.join("COMMITTED").exists());

        let mut forged_preview = failed_preview(preview_dir.path());
        forged_preview.status = PreviewRunStatus::Passed;
        forged_preview.failure = None;
        forged_preview.screenshot_sha256 = Some("a".repeat(64));
        forged_preview.screenshot_bytes = Some(1);
        assert!(
            persist_run_bundle(
                fixture.repository.path(),
                RUN_ID,
                fixture.links.clone(),
                &fixture.repair(),
                &forged_preview,
            )
            .is_err()
        );
        assert!(!fixture.run_root.join("COMMITTED").exists());
    }

    #[test]
    fn forged_passed_preview_with_matching_hash_but_invalid_png_is_not_committed() {
        let fixture = RepositoryFixture::create();
        let preview_parent = tempfile::tempdir().unwrap();
        let invalid_png = [b"\x89PNG\r\n\x1a\n".as_slice(), b"truncated"].concat();
        let preview = forged_passed_preview(&preview_parent.path().join("passed"), &invalid_png);
        let error = persist_run_bundle(
            fixture.repository.path(),
            RUN_ID,
            fixture.links.clone(),
            &fixture.repair(),
            &preview,
        )
        .unwrap_err();
        assert!(error.message().contains("strict revalidation"));
        assert!(!fixture.run_root.join(".bundle-partial").exists());
        assert!(!fixture.run_root.join("COMMITTED").exists());
    }

    #[test]
    fn symlinked_artifact_and_run_root_are_rejected_when_supported() {
        let fixture = RepositoryFixture::create();
        let outside = fixture.repository.path().join("outside-analysis.json");
        fs::write(&outside, b"{}").unwrap();
        let link_path = fixture.run_root.join("analysis/symlink-analysis.json");
        if !create_file_symlink(&outside, &link_path) {
            return;
        }
        let bytes = fs::read(&outside).unwrap();
        let mut links = fixture.links.clone();
        links.reference_analysis = ArtifactLink::new(
            "analysis/symlink-analysis.json",
            hash_bytes(&bytes),
            bytes.len() as u64,
        )
        .unwrap();
        let preview_dir = tempfile::tempdir().unwrap();
        assert!(
            persist_run_bundle(
                fixture.repository.path(),
                RUN_ID,
                links,
                &fixture.repair(),
                &failed_preview(preview_dir.path()),
            )
            .is_err()
        );
        assert!(!fixture.run_root.join("COMMITTED").exists());

        let repository = tempfile::tempdir().unwrap();
        let generation_root = repository.path().join("summary/ui-generation");
        fs::create_dir_all(&generation_root).unwrap();
        let linked_root = repository.path().join("linked-stage3-run");
        fs::create_dir(&linked_root).unwrap();
        let run_link = generation_root.join(RUN_ID);
        if create_directory_symlink(&linked_root, &run_link) {
            let error = persist_run_bundle(
                repository.path(),
                RUN_ID,
                fixture.links.clone(),
                &fixture.repair(),
                &failed_preview(preview_dir.path()),
            );
            assert!(error.is_err());
            assert!(!linked_root.join("COMMITTED").exists());
        }
    }

    #[test]
    fn closed_loop_manifest_covers_the_full_approval_lifecycle() {
        let mut manifest = advance_to_auditing();
        manifest.artifacts.comparison = Some(closed_loop_link("audit/comparison.json"));
        manifest.artifacts.analysis = Some(closed_loop_link("audit/analysis.json"));
        manifest
            .transition(ClosedLoopRunState::AwaitingApproval, 7, cache_key('1'))
            .unwrap();
        manifest.artifacts.approval = Some(closed_loop_link("approval/release.json"));
        manifest.approve(8).unwrap();

        assert_eq!(manifest.state, ClosedLoopRunState::Passed);
        assert!(manifest.state.is_terminal());
        assert_eq!(manifest.checkpoints.len(), 8);
        assert!(
            manifest
                .checkpoints
                .iter()
                .all(|checkpoint| checkpoint.completed_at_unix_ms.is_some())
        );
    }

    #[test]
    fn closed_loop_manifest_rejects_illegal_state_transitions() {
        let mut manifest = closed_loop_manifest();
        let error = manifest
            .transition(ClosedLoopRunState::Generating, 2, cache_key('b'))
            .unwrap_err();
        assert_eq!(error.kind(), TaskFailureKind::InvalidStateTransition);
        assert_eq!(manifest.state, ClosedLoopRunState::Created);
    }

    #[test]
    fn closed_loop_manifest_detects_corruption_and_protocol_incompatibility() {
        let corruption = ClosedLoopRunManifest::parse_json(b"not JSON").unwrap_err();
        assert_eq!(corruption.kind(), TaskFailureKind::ManifestCorrupt);

        let mut incompatible = serde_json::to_value(closed_loop_manifest()).unwrap();
        incompatible["protocol_version"] = serde_json::json!(99);
        let incompatibility =
            ClosedLoopRunManifest::parse_json(&serde_json::to_vec(&incompatible).unwrap())
                .unwrap_err();
        assert_eq!(
            incompatibility.kind(),
            TaskFailureKind::ProtocolIncompatible
        );
    }

    #[test]
    fn closed_loop_manifest_reuses_completed_external_calls_with_matching_cache_keys() {
        let mut manifest = advance_to_auditing();
        let expected = expected_checkpoint_cache_keys(&manifest);
        let plan = manifest.recovery_plan(&expected).unwrap();
        assert_eq!(
            plan.last_complete_checkpoint,
            Some(manifest.checkpoints[4].identity(4))
        );
        assert_eq!(plan.restart_checkpoint, manifest.checkpoints[5].identity(5));
        assert_eq!(
            plan.reusable_external_call_checkpoints,
            [
                manifest.checkpoints[2].identity(2),
                manifest.checkpoints[4].identity(4)
            ]
        );

        let mut stale_generation = expected.clone();
        stale_generation.insert(manifest.checkpoints[2].identity(2), cache_key('9'));
        let stale_plan = manifest.recovery_plan(&stale_generation).unwrap();
        assert_eq!(
            stale_plan.last_complete_checkpoint,
            Some(manifest.checkpoints[1].identity(1))
        );
        assert_eq!(
            stale_plan.restart_checkpoint,
            manifest.checkpoints[2].identity(2)
        );
        assert!(stale_plan.reusable_external_call_checkpoints.is_empty());

        manifest.restart_from(&plan, 7).unwrap();
        assert_eq!(manifest.state, ClosedLoopRunState::Auditing);
        assert_eq!(manifest.checkpoints.last().unwrap().attempt, 2);
    }

    #[test]
    fn closed_loop_recovery_uses_the_latest_repeated_checkpoint_in_a_fix_cycle() {
        let mut manifest = advance_through_fix_cycle_to_auditing();
        let mut expected = expected_checkpoint_cache_keys(&manifest);
        let latest_preview = manifest.checkpoints[9].identity(9);
        let first_preview = manifest.checkpoints[4].identity(4);
        assert_eq!(latest_preview.state, ClosedLoopRunState::Previewing);
        assert_eq!(first_preview.state, ClosedLoopRunState::Previewing);
        assert_ne!(latest_preview, first_preview);

        let plan = manifest.recovery_plan(&expected).unwrap();
        assert_eq!(plan.last_complete_checkpoint, Some(latest_preview.clone()));
        assert_eq!(
            plan.restart_checkpoint,
            manifest.checkpoints[10].identity(10)
        );
        assert!(
            plan.reusable_external_call_checkpoints
                .contains(&latest_preview)
        );

        expected.insert(latest_preview.clone(), cache_key('6'));
        let plan = manifest.recovery_plan(&expected).unwrap();
        assert_eq!(
            plan.last_complete_checkpoint,
            Some(manifest.checkpoints[8].identity(8))
        );
        assert_eq!(plan.restart_checkpoint, latest_preview);
        manifest.restart_from(&plan, 12).unwrap();
        assert_eq!(manifest.checkpoints.len(), 10);
        assert_eq!(manifest.checkpoints[4].identity(4), first_preview);
        assert_eq!(
            manifest.checkpoints[9].state,
            ClosedLoopRunState::Previewing
        );
        assert_eq!(manifest.checkpoints[9].attempt, 2);
        assert_eq!(manifest.checkpoints[9].cache_key, cache_key('6'));
    }

    #[test]
    fn closed_loop_manifest_persists_and_cancellation_cannot_replace_passed_state() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("closed-loop-manifest.json");
        let mut manifest = advance_to_auditing();
        manifest.artifacts.comparison = Some(closed_loop_link("audit/comparison.json"));
        manifest.artifacts.analysis = Some(closed_loop_link("audit/analysis.json"));
        manifest
            .transition(ClosedLoopRunState::AwaitingApproval, 7, cache_key('1'))
            .unwrap();
        manifest.artifacts.approval = Some(closed_loop_link("approval/release.json"));
        manifest.approve(8).unwrap();
        manifest.write_new(&path).unwrap();
        assert_eq!(ClosedLoopRunManifest::load(&path).unwrap(), manifest);
        assert!(!manifest.cancel(9, "too late"));
        assert_eq!(manifest.state, ClosedLoopRunState::Passed);
    }

    #[cfg(unix)]
    fn create_file_symlink(target: &Path, link: &Path) -> bool {
        std::os::unix::fs::symlink(target, link).is_ok()
    }

    #[cfg(windows)]
    fn create_file_symlink(target: &Path, link: &Path) -> bool {
        std::os::windows::fs::symlink_file(target, link).is_ok()
    }

    #[cfg(unix)]
    fn create_directory_symlink(target: &Path, link: &Path) -> bool {
        std::os::unix::fs::symlink(target, link).is_ok()
    }

    #[cfg(windows)]
    fn create_directory_symlink(target: &Path, link: &Path) -> bool {
        std::os::windows::fs::symlink_dir(target, link).is_ok()
    }
}
