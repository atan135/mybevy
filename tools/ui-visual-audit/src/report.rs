use crate::{
    ComparisonError, ComparisonErrorCode, PixelRect, ReferenceBinding, ReferenceEntry,
    baseline::{
        BASELINE_RECEIPT_SCHEMA_VERSION, BaselineApproval, BaselineArtifactIdentity,
        BaselineUpdatePlan, BaselineUpdateReceipt, validate_approval, validate_plan,
    },
    comparison::{
        create_output_directory, resolve_allowed_input_roots, resolve_allowed_root,
        resolve_input_file,
    },
    parse_and_validate_manifest,
};
use image::{ImageError, ImageFormat, ImageReader, Limits};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Cursor,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

pub const COMPARISON_BUNDLE_SCHEMA_VERSION: u32 = 1;
pub const COMPARISON_BUNDLE_ALGORITHM_VERSION: &str = "ui_comparison_bundle_v1";
pub const COMPARISON_RESULT_SCHEMA_VERSION: u32 = 1;
pub const COMPARISON_RESULT_FILENAME: &str = "comparison-result.json";
pub const REPORT_FILENAME: &str = "report.md";

const MAX_BUNDLE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_LINKED_ARTIFACT_BYTES: u64 = 32 * 1024 * 1024;
const MAX_TOTAL_LINKED_BYTES: u64 = 256 * 1024 * 1024;
const MAX_CAPTURES: usize = 256;
const MAX_ISSUES_PER_CAPTURE: usize = 1024;
const MAX_FIX_ITERATIONS: usize = 32;
const MAX_IMAGE_DIMENSION: u32 = 16_384;
const MAX_IMAGE_DECODE_ALLOC: u64 = 512 * 1024 * 1024;
const MAX_TOTAL_DECODED_PIXELS: u64 = 128 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct ReportBuildRequest {
    pub repository_root: PathBuf,
    pub allowed_input_roots: Vec<PathBuf>,
    pub allowed_output_root: PathBuf,
    pub bundle: PathBuf,
    pub output_directory: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactLink {
    pub path: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RootManifestLink {
    pub path: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FixIterationLink {
    pub iteration: u32,
    pub manifest: ArtifactLink,
    pub analysis: Option<ArtifactLink>,
    pub report: Option<ArtifactLink>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureArtifacts {
    pub reference: ArtifactLink,
    pub actual: ArtifactLink,
    pub overlay: ArtifactLink,
    pub heatmap: ArtifactLink,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MetricSummary {
    pub raw_changed_ratio_millionths: u32,
    pub alpha_changed_ratio_millionths: u32,
    pub tolerated_changed_ratio_millionths: u32,
    pub ssim_millionths: i32,
    pub geometry_changed_ratio_millionths: u32,
    pub large_area_ratio_millionths: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ThresholdSummary {
    pub profile: String,
    pub values: BTreeMap<String, i64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RegionSummary {
    pub region_id: String,
    pub level: String,
    pub bounds: PixelRect,
    pub status: String,
    pub metrics: MetricSummary,
    pub threshold: ThresholdSummary,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MaskSummary {
    pub mask_id: String,
    pub reason: String,
    pub bounds: PixelRect,
    pub artifact: Option<ArtifactLink>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AllowedDifferenceSummary {
    pub profile: String,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AiRunSummary {
    pub ran: bool,
    pub provider_id: Option<String>,
    pub model_id: Option<String>,
    pub issue_count: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueSeverity {
    Minor,
    Medium,
    Severe,
}

impl IssueSeverity {
    fn label(&self) -> &'static str {
        match self {
            Self::Minor => "minor",
            Self::Medium => "medium",
            Self::Severe => "severe",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceLocation {
    pub image_role: String,
    pub rect: Option<PixelRect>,
    pub description: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LocatedIssue {
    pub issue_id: String,
    pub source: String,
    pub region_id: Option<String>,
    pub severity: IssueSeverity,
    pub message: String,
    pub evidence: EvidenceLocation,
    pub node_id: Option<String>,
    pub source_path: Option<String>,
    pub likely_files: Vec<String>,
    pub likely_cause: Option<String>,
    pub suggested_change_scope: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BaselineGuard {
    pub reference_id: String,
    pub reference_manifest: ArtifactLink,
    pub expected: ReferenceBinding,
    pub observed: ReferenceBinding,
    pub approval_receipt: Option<ArtifactLink>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComparisonCapture {
    pub capture_id: String,
    pub screen: String,
    pub device: String,
    pub state: String,
    pub reference_binding: ReferenceBinding,
    pub artifacts: CaptureArtifacts,
    pub metrics: MetricSummary,
    pub regions: Vec<RegionSummary>,
    pub masks: Vec<MaskSummary>,
    pub allowed_differences: AllowedDifferenceSummary,
    pub algorithms: BTreeMap<String, String>,
    pub thresholds: Vec<ThresholdSummary>,
    pub ai: AiRunSummary,
    pub gate_state: String,
    pub issues: Vec<LocatedIssue>,
    pub baseline_guard: BaselineGuard,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComparisonBundle {
    pub schema_version: u32,
    pub algorithm_version: String,
    pub run_id: String,
    pub root_manifest: RootManifestLink,
    pub analysis: Option<ArtifactLink>,
    pub fix_iterations: Vec<FixIterationLink>,
    pub captures: Vec<ComparisonCapture>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerifiedArtifact {
    pub path: String,
    pub sha256: String,
    pub byte_length: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RootBacklink {
    pub run_id: String,
    pub root_manifest: VerifiedArtifact,
    pub comparison_input: VerifiedArtifact,
    pub root_to_comparison_verified: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComparisonResult {
    pub schema_version: u32,
    pub algorithm_version: String,
    pub status: String,
    pub root: RootBacklink,
    pub analysis: Option<VerifiedArtifact>,
    pub fix_iterations: Vec<FixIterationLink>,
    pub captures: Vec<ComparisonCapture>,
    pub summary: ComparisonSummary,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComparisonSummary {
    pub capture_count: usize,
    pub issue_count: usize,
    pub ai_ran_capture_count: usize,
    pub baseline_change_count: usize,
}

#[derive(Clone, Debug)]
struct ActiveReferenceEntry {
    binding: ReferenceBinding,
    screen: String,
    device: String,
    state: String,
}

type ActiveReferenceManifests = BTreeMap<(String, String), BTreeMap<String, ActiveReferenceEntry>>;

pub fn build_comparison_report(
    request: &ReportBuildRequest,
) -> Result<ComparisonResult, ComparisonError> {
    let repository_root = canonical_repository(&request.repository_root)?;
    let input_roots = resolve_allowed_input_roots(&repository_root, &request.allowed_input_roots)?;
    let output_root = resolve_allowed_root(
        &repository_root,
        &request.allowed_output_root,
        ComparisonErrorCode::AllowedOutputRootInvalid,
        "allowed output root",
    )?;
    let bundle_path = resolve_input_file(&repository_root, &input_roots, &request.bundle)?;
    let (bundle, bundle_bytes) = read_json::<ComparisonBundle>(&bundle_path, MAX_BUNDLE_BYTES)?;
    validate_bundle(&bundle)?;
    let bundle_identity = verified_identity(&repository_root, &bundle_path, &bundle_bytes);

    let root_path = resolve_input_file(
        &repository_root,
        &input_roots,
        Path::new(&bundle.root_manifest.path),
    )?;
    let (root_value, root_bytes) = read_json::<Value>(&root_path, MAX_LINKED_ARTIFACT_BYTES)?;
    validate_root_link(&root_value, &bundle, &bundle_identity)?;
    let root_identity = verified_identity(&repository_root, &root_path, &root_bytes);

    let mut total_bytes = root_identity.byte_length + bundle_identity.byte_length;
    let mut total_decoded_pixels = 0_u64;
    let mut active_reference_manifests = ActiveReferenceManifests::new();
    let capture_ids = bundle
        .captures
        .iter()
        .map(|capture| capture.capture_id.as_str())
        .collect::<BTreeSet<_>>();
    let analysis = bundle
        .analysis
        .as_ref()
        .map(|link| verify_link(&repository_root, &input_roots, link, &mut total_bytes))
        .transpose()?;
    if let Some(analysis) = &analysis {
        validate_child_backlink(
            &repository_root.join(&analysis.path),
            &bundle.run_id,
            &bundle.root_manifest.path,
            &capture_ids,
            true,
        )?;
    }
    for iteration in &bundle.fix_iterations {
        let manifest = verify_link(
            &repository_root,
            &input_roots,
            &iteration.manifest,
            &mut total_bytes,
        )?;
        validate_child_backlink(
            &repository_root.join(&manifest.path),
            &bundle.run_id,
            &bundle.root_manifest.path,
            &capture_ids,
            false,
        )?;
        if let Some(link) = &iteration.analysis {
            verify_link(&repository_root, &input_roots, link, &mut total_bytes)?;
        }
        if let Some(link) = &iteration.report {
            verify_link(&repository_root, &input_roots, link, &mut total_bytes)?;
        }
    }
    for capture in &bundle.captures {
        let mut capture_dimensions = None;
        let mut reference_artifact = None;
        for (role, link) in [
            ("reference", &capture.artifacts.reference),
            ("actual", &capture.artifacts.actual),
            ("overlay", &capture.artifacts.overlay),
            ("heatmap", &capture.artifacts.heatmap),
        ] {
            let (artifact, dimensions) = verify_image_link(
                &repository_root,
                &input_roots,
                link,
                &mut total_bytes,
                &mut total_decoded_pixels,
            )?;
            if capture_dimensions
                .replace(dimensions)
                .is_some_and(|prior| prior != dimensions)
            {
                return Err(link_error(format!(
                    "capture {} visual artifacts have mismatched dimensions",
                    capture.capture_id
                )));
            }
            if role == "reference" {
                reference_artifact = Some(artifact);
            }
        }
        for mask in &capture.masks {
            if let Some(link) = &mask.artifact {
                verify_link(&repository_root, &input_roots, link, &mut total_bytes)?;
            }
        }
        validate_capture_geometry(
            capture,
            capture_dimensions.ok_or_else(|| {
                report_error("capture visual artifacts did not provide dimensions")
            })?,
        )?;
        validate_baseline_guard(
            &repository_root,
            &input_roots,
            capture,
            reference_artifact.as_ref().ok_or_else(|| {
                report_error("capture did not produce a verified reference artifact")
            })?,
            &capture.baseline_guard,
            &mut total_bytes,
            &mut active_reference_manifests,
        )?;
    }
    if total_bytes > MAX_TOTAL_LINKED_BYTES {
        return Err(report_error(format!(
            "linked artifact bytes {total_bytes} exceed {MAX_TOTAL_LINKED_BYTES}"
        )));
    }

    let result = ComparisonResult {
        schema_version: COMPARISON_RESULT_SCHEMA_VERSION,
        algorithm_version: COMPARISON_BUNDLE_ALGORITHM_VERSION.to_owned(),
        status: comparison_status(&bundle.captures).to_owned(),
        root: RootBacklink {
            run_id: bundle.run_id.clone(),
            root_manifest: root_identity,
            comparison_input: bundle_identity,
            root_to_comparison_verified: true,
        },
        analysis,
        fix_iterations: bundle.fix_iterations.clone(),
        summary: comparison_summary(&bundle.captures),
        captures: bundle.captures,
    };

    let output_directory =
        create_output_directory(&repository_root, &output_root, &request.output_directory)?;
    persist_new_json(&output_directory.join(COMPARISON_RESULT_FILENAME), &result)?;
    let markdown = render_markdown(&result, &repository_root, &output_directory)?;
    persist_new_bytes(&output_directory.join(REPORT_FILENAME), markdown.as_bytes())?;
    Ok(result)
}

pub(crate) fn validate_comparison_result_provenance(
    repository_root: &Path,
    comparison: &ComparisonResult,
) -> Result<(), ComparisonError> {
    let repository_root = canonical_repository(repository_root)?;
    let input_roots =
        resolve_allowed_input_roots(&repository_root, std::slice::from_ref(&repository_root))?;
    if comparison.schema_version != COMPARISON_RESULT_SCHEMA_VERSION
        || comparison.algorithm_version != COMPARISON_BUNDLE_ALGORITHM_VERSION
        || !comparison.root.root_to_comparison_verified
    {
        return Err(link_error(
            "comparison result does not claim the required report provenance contract",
        ));
    }

    let mut total_bytes = 0_u64;
    let (root_identity, root_bytes) = verify_result_artifact(
        &repository_root,
        &input_roots,
        &comparison.root.root_manifest,
        MAX_LINKED_ARTIFACT_BYTES,
        &mut total_bytes,
    )?;
    let root_value: Value = serde_json::from_slice(&root_bytes)
        .map_err(|error| link_error(format!("root manifest is not valid JSON: {error}")))?;
    let (bundle_identity, bundle_bytes) = verify_result_artifact(
        &repository_root,
        &input_roots,
        &comparison.root.comparison_input,
        MAX_BUNDLE_BYTES,
        &mut total_bytes,
    )?;
    let bundle: ComparisonBundle = serde_json::from_slice(&bundle_bytes)
        .map_err(|error| link_error(format!("comparison bundle is not valid JSON: {error}")))?;
    validate_bundle(&bundle)?;
    validate_root_link(&root_value, &bundle, &bundle_identity)?;

    if comparison.root.run_id != bundle.run_id
        || comparison.root.root_manifest != root_identity
        || comparison.root.comparison_input != bundle_identity
        || comparison.captures != bundle.captures
        || comparison.fix_iterations != bundle.fix_iterations
        || comparison.status != comparison_status(&bundle.captures)
        || comparison.summary != comparison_summary(&bundle.captures)
    {
        return Err(link_error(
            "comparison result does not match the verified root and bundle evidence",
        ));
    }

    let capture_ids = bundle
        .captures
        .iter()
        .map(|capture| capture.capture_id.as_str())
        .collect::<BTreeSet<_>>();
    let analysis = bundle
        .analysis
        .as_ref()
        .map(|link| verify_link(&repository_root, &input_roots, link, &mut total_bytes))
        .transpose()?;
    if let Some(analysis) = &analysis {
        validate_child_backlink(
            &repository_root.join(&analysis.path),
            &bundle.run_id,
            &bundle.root_manifest.path,
            &capture_ids,
            true,
        )?;
    }
    if comparison.analysis != analysis {
        return Err(link_error(
            "comparison result analysis identity does not match the verified bundle link",
        ));
    }
    for iteration in &bundle.fix_iterations {
        let manifest = verify_link(
            &repository_root,
            &input_roots,
            &iteration.manifest,
            &mut total_bytes,
        )?;
        validate_child_backlink(
            &repository_root.join(&manifest.path),
            &bundle.run_id,
            &bundle.root_manifest.path,
            &capture_ids,
            false,
        )?;
        if let Some(link) = &iteration.analysis {
            verify_link(&repository_root, &input_roots, link, &mut total_bytes)?;
        }
        if let Some(link) = &iteration.report {
            verify_link(&repository_root, &input_roots, link, &mut total_bytes)?;
        }
    }
    if total_bytes > MAX_TOTAL_LINKED_BYTES {
        return Err(report_error(format!(
            "comparison provenance linked bytes {total_bytes} exceed {MAX_TOTAL_LINKED_BYTES}"
        )));
    }
    Ok(())
}

fn verify_result_artifact(
    repository_root: &Path,
    input_roots: &[PathBuf],
    expected: &VerifiedArtifact,
    maximum: u64,
    total_bytes: &mut u64,
) -> Result<(VerifiedArtifact, Vec<u8>), ComparisonError> {
    let link = ArtifactLink {
        path: expected.path.clone(),
        sha256: expected.sha256.clone(),
    };
    validate_artifact_link(&link)?;
    let path = resolve_input_file(repository_root, input_roots, Path::new(&link.path))?;
    let bytes = read_bounded(&path, maximum)?;
    if hash_bytes(&bytes) != link.sha256 {
        return Err(link_error(format!(
            "comparison result artifact hash mismatch for {}",
            link.path
        ))
        .at_path(&path));
    }
    *total_bytes = total_bytes
        .checked_add(bytes.len() as u64)
        .ok_or_else(|| report_error("comparison provenance byte accounting overflowed"))?;
    let identity = verified_identity(repository_root, &path, &bytes);
    if identity != *expected {
        return Err(link_error(
            "comparison result artifact byte length does not match its declared identity",
        )
        .at_path(&path));
    }
    Ok((identity, bytes))
}

fn comparison_summary(captures: &[ComparisonCapture]) -> ComparisonSummary {
    ComparisonSummary {
        capture_count: captures.len(),
        issue_count: captures.iter().map(|capture| capture.issues.len()).sum(),
        ai_ran_capture_count: captures.iter().filter(|capture| capture.ai.ran).count(),
        baseline_change_count: captures
            .iter()
            .filter(|capture| capture.baseline_guard.expected != capture.baseline_guard.observed)
            .count(),
    }
}

fn validate_bundle(bundle: &ComparisonBundle) -> Result<(), ComparisonError> {
    if bundle.schema_version != COMPARISON_BUNDLE_SCHEMA_VERSION
        || bundle.algorithm_version != COMPARISON_BUNDLE_ALGORITHM_VERSION
    {
        return Err(report_error(format!(
            "comparison bundle must use schema {COMPARISON_BUNDLE_SCHEMA_VERSION} and algorithm {COMPARISON_BUNDLE_ALGORITHM_VERSION}"
        )));
    }
    validate_text(&bundle.run_id, "run_id", 256)?;
    validate_relative_path(&bundle.root_manifest.path, "root manifest")?;
    if bundle.captures.is_empty() || bundle.captures.len() > MAX_CAPTURES {
        return Err(report_error(format!(
            "capture count must be between 1 and {MAX_CAPTURES}"
        )));
    }
    if bundle.fix_iterations.len() > MAX_FIX_ITERATIONS {
        return Err(report_error("fix iteration count exceeds the report limit"));
    }
    let mut iterations = BTreeSet::new();
    for fix in &bundle.fix_iterations {
        if fix.iteration == 0 || !iterations.insert(fix.iteration) {
            return Err(report_error(
                "fix iteration IDs must be positive and unique",
            ));
        }
        validate_artifact_link(&fix.manifest)?;
        if let Some(link) = &fix.analysis {
            validate_artifact_link(link)?;
        }
        if let Some(link) = &fix.report {
            validate_artifact_link(link)?;
        }
    }
    if let Some(link) = &bundle.analysis {
        validate_artifact_link(link)?;
    }
    let mut capture_ids = BTreeSet::new();
    for capture in &bundle.captures {
        for (label, value) in [
            ("screen", capture.screen.as_str()),
            ("device", capture.device.as_str()),
            ("state", capture.state.as_str()),
        ] {
            validate_text(value, label, 128)?;
        }
        let expected_id = format!("{}.{}.{}", capture.screen, capture.device, capture.state);
        if capture.capture_id != expected_id || !capture_ids.insert(&capture.capture_id) {
            return Err(report_error(format!(
                "capture_id must equal screen.device.state and be unique: {}",
                capture.capture_id
            )));
        }
        if capture.reference_binding.revision == 0 || !valid_hash(&capture.reference_binding.sha256)
        {
            return Err(report_error("capture reference binding is invalid"));
        }
        for link in [
            &capture.artifacts.reference,
            &capture.artifacts.actual,
            &capture.artifacts.overlay,
            &capture.artifacts.heatmap,
        ] {
            validate_artifact_link(link)?;
        }
        if capture.issues.len() > MAX_ISSUES_PER_CAPTURE {
            return Err(report_error("capture issue count exceeds the report limit"));
        }
        validate_text(
            &capture.allowed_differences.profile,
            "allowed differences profile",
            256,
        )?;
        validate_text(&capture.gate_state, "gate_state", 64)?;
        if !matches!(
            capture.gate_state.as_str(),
            "passed" | "needs_review" | "failed" | "invalid"
        ) {
            return Err(report_error(
                "gate_state is not one of the four public states",
            ));
        }
        if capture.algorithms.is_empty()
            || capture
                .algorithms
                .iter()
                .any(|(name, version)| name.trim().is_empty() || version.trim().is_empty())
        {
            return Err(report_error(
                "capture algorithms must contain non-empty name/version entries",
            ));
        }
        for region in &capture.regions {
            validate_text(&region.region_id, "region_id", 256)?;
            validate_text(&region.level, "region level", 64)?;
            validate_text(&region.status, "region status", 64)?;
            validate_threshold_summary(&region.threshold, "region")?;
        }
        if capture.thresholds.is_empty() || capture.thresholds.len() > 64 {
            return Err(report_error(
                "capture thresholds must contain 1 to 64 entries",
            ));
        }
        for threshold in &capture.thresholds {
            validate_threshold_summary(threshold, "capture")?;
        }
        for mask in &capture.masks {
            validate_text(&mask.mask_id, "mask_id", 256)?;
            validate_text(&mask.reason, "mask reason", 2048)?;
            if let Some(link) = &mask.artifact {
                validate_artifact_link(link)?;
            }
        }
        validate_ai_summary(&capture.ai)?;
        validate_issues(&capture.issues)?;
        if capture.baseline_guard.reference_id.trim().is_empty()
            || capture.baseline_guard.observed != capture.reference_binding
            || capture.baseline_guard.expected.revision == 0
            || capture.baseline_guard.observed.revision == 0
            || !valid_hash(&capture.baseline_guard.expected.sha256)
            || !valid_hash(&capture.baseline_guard.observed.sha256)
        {
            return Err(report_error(
                "baseline guard must identify the capture's observed reference binding",
            ));
        }
        validate_artifact_link(&capture.baseline_guard.reference_manifest)?;
    }
    Ok(())
}

fn validate_threshold_summary(
    threshold: &ThresholdSummary,
    scope: &str,
) -> Result<(), ComparisonError> {
    validate_text(
        &threshold.profile,
        &format!("{scope} threshold profile"),
        256,
    )?;
    if threshold.values.is_empty() || threshold.values.len() > 64 {
        return Err(report_error(format!(
            "{scope} threshold values must contain 1 to 64 entries"
        )));
    }
    for name in threshold.values.keys() {
        validate_text(name, &format!("{scope} threshold name"), 128)?;
    }
    Ok(())
}

fn validate_ai_summary(ai: &AiRunSummary) -> Result<(), ComparisonError> {
    if ai.ran
        && (ai.provider_id.as_deref().is_none_or(str::is_empty)
            || ai.model_id.as_deref().is_none_or(str::is_empty))
    {
        return Err(report_error(
            "AI ran=true requires non-empty provider_id and model_id",
        ));
    }
    if !ai.ran && (ai.provider_id.is_some() || ai.model_id.is_some() || ai.issue_count != 0) {
        return Err(report_error(
            "AI ran=false must not claim provider, model, or issues",
        ));
    }
    Ok(())
}

fn validate_issues(issues: &[LocatedIssue]) -> Result<(), ComparisonError> {
    let mut ids = BTreeSet::new();
    for issue in issues {
        validate_text(&issue.issue_id, "issue_id", 256)?;
        validate_text(&issue.source, "issue source", 128)?;
        validate_text(&issue.message, "issue message", 4096)?;
        validate_text(&issue.evidence.image_role, "evidence image role", 64)?;
        if !matches!(
            issue.evidence.image_role.as_str(),
            "reference" | "actual" | "overlay" | "heatmap" | "semantic_metadata"
        ) {
            return Err(report_error("issue evidence image role is unknown"));
        }
        validate_text(&issue.evidence.description, "evidence description", 4096)?;
        if !ids.insert(&issue.issue_id) {
            return Err(report_error("issue IDs must be unique within a capture"));
        }
        if issue.likely_files.len() > 32 {
            return Err(report_error("issue likely_files exceeds 32 entries"));
        }
        for path in &issue.likely_files {
            validate_text(path, "issue likely file", 1024)?;
        }
        if let Some(source_path) = &issue.source_path {
            validate_text(source_path, "issue source path", 1024)?;
        }
        if let Some(cause) = &issue.likely_cause {
            validate_text(cause, "issue likely cause", 4096)?;
        }
        if let Some(scope) = &issue.suggested_change_scope {
            validate_text(scope, "issue suggested change scope", 4096)?;
        }
    }
    Ok(())
}

fn validate_capture_geometry(
    capture: &ComparisonCapture,
    dimensions: (u32, u32),
) -> Result<(), ComparisonError> {
    let region_ids = capture
        .regions
        .iter()
        .map(|region| region.region_id.as_str())
        .collect::<BTreeSet<_>>();
    for region in &capture.regions {
        validate_rect_in_capture(
            region.bounds,
            dimensions,
            &format!("region {} bounds", region.region_id),
        )?;
    }
    for mask in &capture.masks {
        validate_rect_in_capture(
            mask.bounds,
            dimensions,
            &format!("mask {} bounds", mask.mask_id),
        )?;
    }
    for issue in &capture.issues {
        if let Some(region_id) = &issue.region_id
            && !region_ids.contains(region_id.as_str())
        {
            return Err(report_error(format!(
                "issue {} references an unknown region {}",
                issue.issue_id, region_id
            )));
        }
        if let Some(rect) = issue.evidence.rect {
            validate_rect_in_capture(
                rect,
                dimensions,
                &format!("issue {} evidence bounds", issue.issue_id),
            )?;
        }
    }
    Ok(())
}

fn validate_rect_in_capture(
    rect: PixelRect,
    dimensions: (u32, u32),
    label: &str,
) -> Result<(), ComparisonError> {
    if rect.width == 0 || rect.height == 0 || rect.x < 0 || rect.y < 0 {
        return Err(report_error(format!(
            "{label} must be non-empty and use non-negative coordinates"
        )));
    }
    let right = u64::try_from(rect.x)
        .ok()
        .and_then(|x| x.checked_add(u64::from(rect.width)));
    let bottom = u64::try_from(rect.y)
        .ok()
        .and_then(|y| y.checked_add(u64::from(rect.height)));
    if right.is_none_or(|value| value > u64::from(dimensions.0))
        || bottom.is_none_or(|value| value > u64::from(dimensions.1))
    {
        return Err(report_error(format!(
            "{label} is outside the {}x{} capture",
            dimensions.0, dimensions.1
        )));
    }
    Ok(())
}

fn validate_root_link(
    root: &Value,
    bundle: &ComparisonBundle,
    input: &VerifiedArtifact,
) -> Result<(), ComparisonError> {
    let run_id = root.pointer("/run_id").and_then(Value::as_str);
    let linked_path = root.pointer("/comparison/input").and_then(Value::as_str);
    let linked_hash = root
        .pointer("/comparison/input_sha256")
        .and_then(Value::as_str);
    if run_id != Some(bundle.run_id.as_str())
        || linked_path != Some(input.path.as_str())
        || linked_hash != Some(input.sha256.as_str())
    {
        return Err(link_error(
            "root manifest must bind the comparison input path and SHA-256 for the same run_id",
        ));
    }
    match (&bundle.analysis, root.pointer("/analysis")) {
        (Some(link), Some(value))
            if value.pointer("/path").and_then(Value::as_str) == Some(link.path.as_str())
                && value.pointer("/sha256").and_then(Value::as_str)
                    == Some(link.sha256.as_str()) => {}
        (None, Some(Value::Null) | None) => {}
        _ => {
            return Err(link_error(
                "root manifest analysis link must match the comparison bundle path and SHA-256",
            ));
        }
    }
    let root_iterations = root
        .pointer("/fix_iterations")
        .and_then(Value::as_array)
        .ok_or_else(|| link_error("root manifest must contain a fix_iterations array"))?;
    if root_iterations.len() != bundle.fix_iterations.len() {
        return Err(link_error(
            "root manifest fix iteration links do not match the comparison bundle",
        ));
    }
    for (expected, observed) in bundle.fix_iterations.iter().zip(root_iterations) {
        if observed.pointer("/iteration").and_then(Value::as_u64)
            != Some(u64::from(expected.iteration))
            || observed.pointer("/manifest/path").and_then(Value::as_str)
                != Some(expected.manifest.path.as_str())
            || observed.pointer("/manifest/sha256").and_then(Value::as_str)
                != Some(expected.manifest.sha256.as_str())
            || !optional_json_link_matches(
                observed.pointer("/analysis"),
                expected.analysis.as_ref(),
            )
            || !optional_json_link_matches(observed.pointer("/report"), expected.report.as_ref())
        {
            return Err(link_error(
                "root manifest fix iteration backlink is incomplete or mismatched",
            ));
        }
    }
    Ok(())
}

fn validate_child_backlink(
    path: &Path,
    run_id: &str,
    root_manifest_path: &str,
    known_capture_ids: &BTreeSet<&str>,
    require_all_captures: bool,
) -> Result<(), ComparisonError> {
    let (value, _) = read_json::<Value>(path, MAX_LINKED_ARTIFACT_BYTES)?;
    let backlink = value
        .pointer("/artifact_backlink")
        .ok_or_else(|| link_error("linked JSON artifact is missing artifact_backlink"))?;
    if backlink.pointer("/schema_version").and_then(Value::as_u64) != Some(1)
        || backlink.pointer("/root_run_id").and_then(Value::as_str) != Some(run_id)
        || backlink.pointer("/root_manifest").and_then(Value::as_str) != Some(root_manifest_path)
    {
        return Err(link_error(
            "linked JSON artifact backlink does not identify the root manifest",
        ));
    }
    let capture_values = backlink
        .pointer("/capture_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| link_error("artifact_backlink.capture_ids must be an array"))?;
    let mut observed = BTreeSet::new();
    for value in capture_values {
        let capture_id = value
            .as_str()
            .ok_or_else(|| link_error("artifact backlink capture IDs must be strings"))?;
        if !known_capture_ids.contains(capture_id) || !observed.insert(capture_id) {
            return Err(link_error(
                "artifact backlink contains an unknown or duplicate capture ID",
            ));
        }
    }
    if observed.is_empty() || (require_all_captures && observed != *known_capture_ids) {
        return Err(link_error(
            "artifact backlink does not cover the required root captures",
        ));
    }
    Ok(())
}

fn optional_json_link_matches(value: Option<&Value>, expected: Option<&ArtifactLink>) -> bool {
    match (value, expected) {
        (Some(value), Some(link)) => {
            value.pointer("/path").and_then(Value::as_str) == Some(link.path.as_str())
                && value.pointer("/sha256").and_then(Value::as_str) == Some(link.sha256.as_str())
        }
        (Some(Value::Null) | None, None) => true,
        _ => false,
    }
}

fn validate_baseline_guard(
    repository_root: &Path,
    input_roots: &[PathBuf],
    capture: &ComparisonCapture,
    reference_artifact: &VerifiedArtifact,
    guard: &BaselineGuard,
    total_bytes: &mut u64,
    active_reference_manifests: &mut ActiveReferenceManifests,
) -> Result<(), ComparisonError> {
    let active = resolve_active_reference(
        repository_root,
        input_roots,
        &guard.reference_manifest,
        &guard.reference_id,
        total_bytes,
        active_reference_manifests,
    )?;
    if active.binding != guard.observed
        || capture.reference_binding != guard.observed
        || reference_artifact.sha256 != guard.observed.sha256
    {
        return Err(baseline_conflict_error(
            "capture reference artifact or observed binding does not match the active reference manifest",
        ));
    }
    if active.screen != capture.screen
        || active.device != capture.device
        || active.state != capture.state
    {
        return Err(baseline_conflict_error(
            "capture screen, device, or state does not match the active reference manifest entry",
        ));
    }
    if guard.expected == guard.observed {
        if guard.approval_receipt.is_some() {
            return Err(report_error(
                "unchanged baseline must not claim an approval receipt",
            ));
        }
        return Ok(());
    }
    let link = guard.approval_receipt.as_ref().ok_or_else(|| {
        ComparisonError::input(
            ComparisonErrorCode::BaselineApprovalRequired,
            format!(
                "baseline {} changed without an approved receipt",
                guard.reference_id
            ),
        )
    })?;
    let verified = verify_link(repository_root, input_roots, link, total_bytes)?;
    let receipt_path = repository_root.join(&verified.path);
    let (receipt, _) =
        read_json::<BaselineUpdateReceipt>(&receipt_path, MAX_LINKED_ARTIFACT_BYTES)?;
    if receipt.schema_version != BASELINE_RECEIPT_SCHEMA_VERSION
        || receipt.reference_id != guard.reference_id
        || receipt.old_binding != guard.expected
        || receipt.new_binding != guard.observed
        || !receipt.human_approved
        || receipt.status != "applied_rerun_required"
        || !receipt.rerun_verification_required
        || receipt.acceptance_complete
    {
        return Err(ComparisonError::input(
            ComparisonErrorCode::BaselineApprovalRequired,
            "baseline receipt does not approve the observed transition",
        ));
    }
    let plan_verified =
        verify_baseline_identity(repository_root, input_roots, &receipt.plan, total_bytes)?;
    let plan_path = repository_root.join(&plan_verified.path);
    let (plan, plan_bytes) =
        read_json::<BaselineUpdatePlan>(&plan_path, MAX_LINKED_ARTIFACT_BYTES)?;
    validate_plan(&plan)?;
    if plan.reference_id != guard.reference_id
        || plan.old_binding != guard.expected
        || plan.new_binding != guard.observed
        || plan.reason != receipt.reason
        || receipt.metrics_before != plan.metrics_before
        || receipt.metrics_after != plan.metrics_after
        || receipt.old_image.sha256 != plan.old_image.sha256
        || receipt.new_image.sha256 != plan.new_image.sha256
        || receipt.old_image.format != plan.old_image.format
        || receipt.new_image.format != plan.new_image.format
        || (receipt.old_image.width, receipt.old_image.height)
            != (plan.old_image.width, plan.old_image.height)
        || (receipt.new_image.width, receipt.new_image.height)
            != (plan.new_image.width, plan.new_image.height)
    {
        return Err(ComparisonError::input(
            ComparisonErrorCode::BaselineApprovalRequired,
            "baseline receipt does not match its bound update plan",
        ));
    }
    let approval_verified =
        verify_baseline_identity(repository_root, input_roots, &receipt.approval, total_bytes)?;
    let approval_path = repository_root.join(&approval_verified.path);
    let (approval, _) = read_json::<BaselineApproval>(&approval_path, MAX_LINKED_ARTIFACT_BYTES)?;
    validate_approval(&approval, &hash_bytes(&plan_bytes))?;
    let old_archive = BaselineArtifactIdentity {
        path: receipt.old_image.path.clone(),
        sha256: receipt.old_image.sha256.clone(),
        byte_length: receipt.old_image.byte_length,
    };
    let new_archive = BaselineArtifactIdentity {
        path: receipt.new_image.path.clone(),
        sha256: receipt.new_image.sha256.clone(),
        byte_length: receipt.new_image.byte_length,
    };
    for identity in [
        &receipt.metrics_before,
        &receipt.metrics_after,
        &old_archive,
        &new_archive,
    ] {
        verify_baseline_identity(repository_root, input_roots, identity, total_bytes)?;
    }
    Ok(())
}

fn resolve_active_reference(
    repository_root: &Path,
    input_roots: &[PathBuf],
    manifest_link: &ArtifactLink,
    reference_id: &str,
    total_bytes: &mut u64,
    manifests: &mut ActiveReferenceManifests,
) -> Result<ActiveReferenceEntry, ComparisonError> {
    let key = (manifest_link.path.clone(), manifest_link.sha256.clone());
    if !manifests.contains_key(&key) {
        let verified = verify_link(repository_root, input_roots, manifest_link, total_bytes)?;
        let manifest_path = repository_root.join(&verified.path);
        let bytes = read_bounded(&manifest_path, MAX_LINKED_ARTIFACT_BYTES)?;
        if hash_bytes(&bytes) != manifest_link.sha256 {
            return Err(link_error(
                "active reference manifest changed while its linked evidence was being validated",
            ));
        }
        let validated = parse_and_validate_manifest(repository_root, &bytes).map_err(|error| {
            link_error(format!(
                "active reference manifest failed validation: {error}"
            ))
            .at_path(&manifest_path)
        })?;
        let mut entries = BTreeMap::new();
        for entry in validated.manifest.references {
            entries.insert(entry.reference_id.clone(), active_reference_entry(&entry));
        }
        manifests.insert(key.clone(), entries);
    }
    manifests
        .get(&key)
        .and_then(|entries| entries.get(reference_id))
        .cloned()
        .ok_or_else(|| {
            baseline_conflict_error(format!(
                "reference_id {reference_id} is not present in the active reference manifest"
            ))
        })
}

fn active_reference_entry(entry: &ReferenceEntry) -> ActiveReferenceEntry {
    ActiveReferenceEntry {
        binding: ReferenceBinding {
            sha256: entry.image.sha256.clone(),
            revision: entry.baseline.version,
        },
        screen: entry.key.screen.clone(),
        device: entry.key.device.clone(),
        state: entry.key.state.clone(),
    }
}

fn verify_baseline_identity(
    repository_root: &Path,
    input_roots: &[PathBuf],
    identity: &BaselineArtifactIdentity,
    total_bytes: &mut u64,
) -> Result<VerifiedArtifact, ComparisonError> {
    let verified = verify_link(
        repository_root,
        input_roots,
        &ArtifactLink {
            path: identity.path.clone(),
            sha256: identity.sha256.clone(),
        },
        total_bytes,
    )?;
    if verified.byte_length != identity.byte_length {
        return Err(link_error(
            "baseline receipt linked artifact byte length does not match",
        ));
    }
    Ok(verified)
}

fn verify_link(
    repository_root: &Path,
    input_roots: &[PathBuf],
    link: &ArtifactLink,
    total_bytes: &mut u64,
) -> Result<VerifiedArtifact, ComparisonError> {
    validate_artifact_link(link)?;
    let path = resolve_input_file(repository_root, input_roots, Path::new(&link.path))?;
    let bytes = read_bounded(&path, MAX_LINKED_ARTIFACT_BYTES)?;
    let observed = hash_bytes(&bytes);
    if observed != link.sha256 {
        return Err(
            link_error(format!("linked artifact hash mismatch for {}", link.path)).at_path(&path),
        );
    }
    *total_bytes = total_bytes
        .checked_add(bytes.len() as u64)
        .ok_or_else(|| report_error("linked artifact byte accounting overflowed"))?;
    Ok(verified_identity(repository_root, &path, &bytes))
}

fn verify_image_link(
    repository_root: &Path,
    input_roots: &[PathBuf],
    link: &ArtifactLink,
    total_bytes: &mut u64,
    total_decoded_pixels: &mut u64,
) -> Result<(VerifiedArtifact, (u32, u32)), ComparisonError> {
    validate_artifact_link(link)?;
    let path = resolve_input_file(repository_root, input_roots, Path::new(&link.path))?;
    let bytes = read_bounded(&path, MAX_LINKED_ARTIFACT_BYTES)?;
    if hash_bytes(&bytes) != link.sha256 {
        return Err(
            link_error(format!("linked image hash mismatch for {}", link.path)).at_path(&path),
        );
    }
    let reader = ImageReader::new(Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|error| link_error(format!("linked image format is invalid: {error}")))?;
    let format = match reader.format() {
        Some(ImageFormat::Png) => ImageFormat::Png,
        Some(ImageFormat::Jpeg) => ImageFormat::Jpeg,
        _ => {
            return Err(link_error("report visual artifacts must be PNG or JPEG").at_path(&path));
        }
    };
    let dimensions = reader.into_dimensions().map_err(|error| {
        link_error(format!("linked image header is invalid: {error}")).at_path(&path)
    })?;
    let pixels = u64::from(dimensions.0)
        .checked_mul(u64::from(dimensions.1))
        .ok_or_else(|| report_error("linked image pixel accounting overflowed"))?;
    *total_decoded_pixels = total_decoded_pixels
        .checked_add(pixels)
        .ok_or_else(|| report_error("linked image pixel accounting overflowed"))?;
    if *total_decoded_pixels > MAX_TOTAL_DECODED_PIXELS {
        return Err(report_error(format!(
            "linked images exceed {MAX_TOTAL_DECODED_PIXELS} decoded pixels"
        )));
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
    limits.max_alloc = Some(MAX_IMAGE_DECODE_ALLOC);
    let mut reader = ImageReader::with_format(Cursor::new(&bytes), format);
    reader.limits(limits);
    let decoded = reader.decode().map_err(|error| match error {
        ImageError::Limits(_) => link_error("linked image exceeded decoder limits").at_path(&path),
        _ => link_error(format!("linked image is truncated or corrupt: {error}")).at_path(&path),
    })?;
    if (decoded.width(), decoded.height()) != dimensions {
        return Err(link_error(
            "linked image dimensions changed between preflight and full decode",
        )
        .at_path(&path));
    }
    *total_bytes = total_bytes
        .checked_add(bytes.len() as u64)
        .ok_or_else(|| report_error("linked artifact byte accounting overflowed"))?;
    Ok((
        verified_identity(repository_root, &path, &bytes),
        dimensions,
    ))
}

fn validate_artifact_link(link: &ArtifactLink) -> Result<(), ComparisonError> {
    if link.path.trim().is_empty() || !valid_hash(&link.sha256) {
        return Err(link_error(
            "artifact links require a non-empty path and lowercase SHA-256",
        ));
    }
    validate_relative_path(&link.path, "artifact link")?;
    Ok(())
}

fn validate_relative_path(value: &str, label: &str) -> Result<(), ComparisonError> {
    let path = Path::new(value);
    if value.trim().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(link_error(format!(
            "{label} path must be repository-relative without parent traversal"
        )));
    }
    Ok(())
}

fn comparison_status(captures: &[ComparisonCapture]) -> &'static str {
    if captures
        .iter()
        .any(|capture| capture.gate_state == "invalid")
    {
        "invalid"
    } else if captures
        .iter()
        .any(|capture| capture.gate_state == "failed")
    {
        "failed"
    } else if captures
        .iter()
        .any(|capture| capture.gate_state == "needs_review")
    {
        "needs_review"
    } else {
        "passed"
    }
}

fn render_markdown(
    result: &ComparisonResult,
    repository_root: &Path,
    output_directory: &Path,
) -> Result<String, ComparisonError> {
    let mut lines = vec![
        "# UI Visual Comparison Report".to_owned(),
        String::new(),
        format!("- Run ID: `{}`", md(&result.root.run_id)),
        format!("- Status: `{}`", result.status),
        format!(
            "- Root manifest: {}",
            markdown_link(
                &result.root.root_manifest.path,
                repository_root,
                output_directory
            )?
        ),
        format!(
            "- Machine comparison: [{}]({})",
            COMPARISON_RESULT_FILENAME, COMPARISON_RESULT_FILENAME
        ),
        format!(
            "- Captures / issues / AI ran: {} / {} / {}",
            result.summary.capture_count,
            result.summary.issue_count,
            result.summary.ai_ran_capture_count
        ),
    ];
    if let Some(analysis) = &result.analysis {
        lines.push(format!(
            "- Analysis: {}",
            markdown_link(&analysis.path, repository_root, output_directory)?
        ));
    }
    lines.push(String::new());
    lines.push("## Captures".to_owned());
    for capture in &result.captures {
        lines.push(String::new());
        lines.push(format!(
            "### {} / {} / {}",
            md(&capture.screen),
            md(&capture.device),
            md(&capture.state)
        ));
        lines.push(String::new());
        lines.push(format!("- Gate: `{}`", md(&capture.gate_state)));
        lines.push(format!(
            "- Reference binding: `{}` revision `{}`",
            capture.reference_binding.sha256, capture.reference_binding.revision
        ));
        lines.push(format!(
            "- Reference / actual / overlay / heatmap: {} / {} / {} / {}",
            markdown_link(
                &capture.artifacts.reference.path,
                repository_root,
                output_directory
            )?,
            markdown_link(
                &capture.artifacts.actual.path,
                repository_root,
                output_directory
            )?,
            markdown_link(
                &capture.artifacts.overlay.path,
                repository_root,
                output_directory
            )?,
            markdown_link(
                &capture.artifacts.heatmap.path,
                repository_root,
                output_directory
            )?,
        ));
        lines.push(format!(
            "- Metrics (raw/alpha/tolerated/SSIM/geometry/large): `{}` / `{}` / `{}` / `{}` / `{}` / `{}` millionths",
            capture.metrics.raw_changed_ratio_millionths,
            capture.metrics.alpha_changed_ratio_millionths,
            capture.metrics.tolerated_changed_ratio_millionths,
            capture.metrics.ssim_millionths,
            capture.metrics.geometry_changed_ratio_millionths,
            capture.metrics.large_area_ratio_millionths,
        ));
        lines.push(format!(
            "- AI actually ran: `{}`{}",
            capture.ai.ran,
            capture
                .ai
                .model_id
                .as_ref()
                .map(|model| format!(
                    " (`{}` / `{}`)",
                    md(capture.ai.provider_id.as_deref().unwrap_or("unknown")),
                    md(model)
                ))
                .unwrap_or_default()
        ));
        lines.push(format!(
            "- Allowed differences: `{}`; {}",
            md(&capture.allowed_differences.profile),
            md(&capture.allowed_differences.notes.join("; "))
        ));
        lines.push(format!(
            "- Algorithms: {}",
            md(&capture
                .algorithms
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>()
                .join(", "))
        ));
        lines.push(format!(
            "- Capture thresholds: {}",
            md(&format_thresholds(&capture.thresholds))
        ));
        if capture.masks.is_empty() {
            lines.push("- Masks: none".to_owned());
        } else {
            lines.push(format!(
                "- Masks: {}",
                md(&capture
                    .masks
                    .iter()
                    .map(|mask| {
                        let artifact = mask
                            .artifact
                            .as_ref()
                            .map(|artifact| format!(" [{}]", artifact.path))
                            .unwrap_or_default();
                        format!(
                            "{} ({}) @ {},{},{},{}{}",
                            mask.mask_id,
                            mask.reason,
                            mask.bounds.x,
                            mask.bounds.y,
                            mask.bounds.width,
                            mask.bounds.height,
                            artifact
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("; "))
            ));
        }
        lines.push(String::new());
        lines.push("| Region | Level | Status | Bounds | Thresholds | Metrics |".to_owned());
        lines.push("| --- | --- | --- | --- | --- | --- |".to_owned());
        for region in &capture.regions {
            lines.push(format!(
                "| `{}` | `{}` | `{}` | `{},{},{},{}` | `{}` | raw={} tol={} ssim={} |",
                md(&region.region_id),
                md(&region.level),
                md(&region.status),
                region.bounds.x,
                region.bounds.y,
                region.bounds.width,
                region.bounds.height,
                md(&format_threshold_summary(&region.threshold)),
                region.metrics.raw_changed_ratio_millionths,
                region.metrics.tolerated_changed_ratio_millionths,
                region.metrics.ssim_millionths,
            ));
        }
        lines.push(String::new());
        lines.push("| Severity | Source | Region | Evidence | Node | Source path | Likely files | Problem | Likely cause | Suggested scope |".to_owned());
        lines.push("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |".to_owned());
        if capture.issues.is_empty() {
            lines.push("| - | - | - | - | - | - | - | No issues | - | - |".to_owned());
        } else {
            for issue in &capture.issues {
                let evidence = match issue.evidence.rect {
                    Some(rect) => format!(
                        "{} @ {},{},{},{}: {}",
                        issue.evidence.image_role,
                        rect.x,
                        rect.y,
                        rect.width,
                        rect.height,
                        issue.evidence.description
                    ),
                    None => format!(
                        "{}: {}",
                        issue.evidence.image_role, issue.evidence.description
                    ),
                };
                lines.push(format!(
                    "| `{}` | `{}` | {} | {} | {} | {} | {} | {} | {} | {} |",
                    issue.severity.label(),
                    md(&issue.source),
                    option_md(issue.region_id.as_deref()),
                    md(&evidence),
                    option_md(issue.node_id.as_deref()),
                    option_md(issue.source_path.as_deref()),
                    option_md(
                        (!issue.likely_files.is_empty())
                            .then(|| issue.likely_files.join("; "))
                            .as_deref(),
                    ),
                    md(&issue.message),
                    option_md(issue.likely_cause.as_deref()),
                    option_md(issue.suggested_change_scope.as_deref()),
                ));
            }
        }
    }
    if !result.fix_iterations.is_empty() {
        lines.push(String::new());
        lines.push("## Fix Iterations".to_owned());
        lines.push(String::new());
        lines.push("| Iteration | Manifest | Analysis | Report |".to_owned());
        lines.push("| --- | --- | --- | --- |".to_owned());
        for fix in &result.fix_iterations {
            lines.push(format!(
                "| {} | {} | {} | {} |",
                fix.iteration,
                markdown_link(&fix.manifest.path, repository_root, output_directory)?,
                optional_link(fix.analysis.as_ref(), repository_root, output_directory)?,
                optional_link(fix.report.as_ref(), repository_root, output_directory)?,
            ));
        }
    }
    lines.push(String::new());
    Ok(lines.join("\n"))
}

fn format_thresholds(thresholds: &[ThresholdSummary]) -> String {
    thresholds
        .iter()
        .map(format_threshold_summary)
        .collect::<Vec<_>>()
        .join("; ")
}

fn format_threshold_summary(threshold: &ThresholdSummary) -> String {
    let values = threshold
        .values
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{} ({values})", threshold.profile)
}

fn optional_link(
    link: Option<&ArtifactLink>,
    repository_root: &Path,
    output_directory: &Path,
) -> Result<String, ComparisonError> {
    link.map(|link| markdown_link(&link.path, repository_root, output_directory))
        .transpose()
        .map(|value| value.unwrap_or_else(|| "-".to_owned()))
}

fn markdown_link(
    path: &str,
    repository_root: &Path,
    output_directory: &Path,
) -> Result<String, ComparisonError> {
    let target = repository_root.join(path);
    if !target.is_file() {
        return Err(link_error(format!(
            "report refuses to render missing artifact {}",
            path
        )));
    }
    let relative = relative_path(output_directory, &target)
        .to_string_lossy()
        .replace('\\', "/")
        .replace(' ', "%20");
    Ok(format!("[{}]({relative})", md(path)))
}

fn relative_path(from: &Path, to: &Path) -> PathBuf {
    let from_components = from.components().collect::<Vec<_>>();
    let to_components = to.components().collect::<Vec<_>>();
    let common = from_components
        .iter()
        .zip(&to_components)
        .take_while(|(left, right)| left == right)
        .count();
    let mut result = PathBuf::new();
    for _ in common..from_components.len() {
        result.push("..");
    }
    for component in &to_components[common..] {
        result.push(component.as_os_str());
    }
    result
}

fn canonical_repository(path: &Path) -> Result<PathBuf, ComparisonError> {
    let root = fs::canonicalize(path).map_err(|error| {
        ComparisonError::input(
            ComparisonErrorCode::RepositoryRootInvalid,
            format!("repository root cannot be resolved: {error}"),
        )
        .at_path(path)
    })?;
    if !root.is_dir() {
        return Err(ComparisonError::input(
            ComparisonErrorCode::RepositoryRootInvalid,
            "repository root is not a directory",
        ));
    }
    Ok(root)
}

fn read_json<T: for<'de> Deserialize<'de>>(
    path: &Path,
    maximum: u64,
) -> Result<(T, Vec<u8>), ComparisonError> {
    let bytes = read_bounded(path, maximum)?;
    let value = serde_json::from_slice(&bytes).map_err(|error| {
        report_error(format!("strict JSON parse failed: {error}")).at_path(path)
    })?;
    Ok((value, bytes))
}

fn read_bounded(path: &Path, maximum: u64) -> Result<Vec<u8>, ComparisonError> {
    let file = fs::File::open(path).map_err(|error| {
        report_error(format!("linked artifact cannot be opened: {error}")).at_path(path)
    })?;
    let length = file
        .metadata()
        .map_err(|error| {
            report_error(format!("linked artifact metadata cannot be read: {error}")).at_path(path)
        })?
        .len();
    if length > maximum {
        return Err(report_error(format!("linked artifact exceeds {maximum} bytes")).at_path(path));
    }
    let mut bytes = Vec::with_capacity(length as usize);
    file.take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            report_error(format!("linked artifact cannot be read: {error}")).at_path(path)
        })?;
    if bytes.len() as u64 > maximum {
        return Err(report_error(format!("linked artifact exceeds {maximum} bytes")).at_path(path));
    }
    Ok(bytes)
}

fn persist_new_json(path: &Path, value: &impl Serialize) -> Result<(), ComparisonError> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        ComparisonError::internal_failure(format!(
            "comparison result serialization failed: {error}"
        ))
    })?;
    bytes.push(b'\n');
    persist_new_bytes(path, &bytes)
}

fn persist_new_bytes(path: &Path, bytes: &[u8]) -> Result<(), ComparisonError> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| {
            ComparisonError::internal(
                ComparisonErrorCode::ArtifactWriteFailed,
                format!("report artifact cannot be created: {error}"),
            )
            .at_path(path)
        })?;
    file.write_all(bytes).map_err(|error| {
        ComparisonError::internal(
            ComparisonErrorCode::ArtifactWriteFailed,
            format!("report artifact cannot be written: {error}"),
        )
        .at_path(path)
    })
}

fn verified_identity(repository_root: &Path, path: &Path, bytes: &[u8]) -> VerifiedArtifact {
    VerifiedArtifact {
        path: path
            .strip_prefix(repository_root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/"),
        sha256: hash_bytes(bytes),
        byte_length: bytes.len() as u64,
    }
}

fn validate_text(value: &str, label: &str, maximum: usize) -> Result<(), ComparisonError> {
    if value.trim().is_empty() || value.len() > maximum || value.chars().any(char::is_control) {
        return Err(report_error(format!(
            "{label} must be non-empty, control-free, and at most {maximum} bytes"
        )));
    }
    Ok(())
}

fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn report_error(message: impl Into<String>) -> ComparisonError {
    ComparisonError::input(ComparisonErrorCode::ReportInputInvalid, message)
}

fn link_error(message: impl Into<String>) -> ComparisonError {
    ComparisonError::input(ComparisonErrorCode::ReportLinkInvalid, message)
}

fn baseline_conflict_error(message: impl Into<String>) -> ComparisonError {
    ComparisonError::input(ComparisonErrorCode::BaselineConflict, message)
}

fn md(value: &str) -> String {
    value
        .replace('|', "\\|")
        .replace('\r', " ")
        .replace('\n', "<br>")
}

fn option_md(value: Option<&str>) -> String {
    value.map(md).unwrap_or_else(|| "unknown".to_owned())
}
