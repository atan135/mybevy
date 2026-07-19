use crate::{
    ComparisonError, ComparisonErrorCode, ReferenceBinding, ReferenceEntry, ReferenceManifest,
    ReferenceStorage, load_and_validate_manifest, parse_and_validate_manifest,
    reference_manifest::{COMMITTED_REFERENCE_ROOT, TEMPORARY_REFERENCE_ROOT},
    report::{
        COMPARISON_BUNDLE_ALGORITHM_VERSION, COMPARISON_RESULT_SCHEMA_VERSION, ComparisonResult,
        validate_comparison_result_provenance,
    },
    validate_baseline_update,
};
use image::{ImageError, ImageFormat, ImageReader, Limits};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
};

pub const BASELINE_PLAN_SCHEMA_VERSION: u32 = 1;
pub const BASELINE_PLAN_ALGORITHM_VERSION: &str = "ui_baseline_update_v1";
pub const BASELINE_PLAN_FILENAME: &str = "baseline-update-plan.json";
pub const BASELINE_APPROVAL_SCHEMA_VERSION: u32 = 1;
pub const BASELINE_RECEIPT_SCHEMA_VERSION: u32 = 1;
pub const BASELINE_RECEIPT_FILENAME: &str = "baseline-update-receipt.json";
pub const BASELINE_RERUN_VERIFICATION_SCHEMA_VERSION: u32 = 1;
pub const BASELINE_RERUN_VERIFICATION_FILENAME: &str = "baseline-rerun-verification.json";

const MAX_JSON_BYTES: u64 = 4 * 1024 * 1024;
const MAX_IMAGE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_IMAGE_DIMENSION: u32 = 16_384;
const MAX_DECODE_ALLOC: u64 = 512 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct BaselinePlanRequest {
    pub repository_root: PathBuf,
    pub manifest: PathBuf,
    pub reference_id: String,
    pub new_image: PathBuf,
    pub reason: String,
    pub metrics_before: PathBuf,
    pub metrics_after: PathBuf,
    pub allowed_output_root: PathBuf,
    pub output_directory: PathBuf,
}

#[derive(Clone, Debug)]
pub struct BaselineApplyRequest {
    pub repository_root: PathBuf,
    pub plan: PathBuf,
    pub approval: PathBuf,
    pub allowed_output_root: PathBuf,
    pub output_directory: PathBuf,
}

#[derive(Clone, Debug)]
pub struct BaselineRerunVerificationRequest {
    pub repository_root: PathBuf,
    pub receipt: PathBuf,
    pub comparison_result: PathBuf,
    pub allowed_output_root: PathBuf,
    pub output_directory: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BaselineArtifactIdentity {
    pub path: String,
    pub sha256: String,
    pub byte_length: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BaselineImageIdentity {
    pub path: String,
    pub sha256: String,
    pub byte_length: u64,
    pub format: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BaselineCaptureRequirement {
    pub capture_id: String,
    pub reference_id: String,
    pub screen: String,
    pub device: String,
    pub state: String,
    pub expected_binding: ReferenceBinding,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BaselineUpdatePlan {
    pub schema_version: u32,
    pub algorithm_version: String,
    pub plan_id: String,
    pub manifest: BaselineArtifactIdentity,
    pub reference_id: String,
    pub target_image_path: String,
    pub reason: String,
    pub old_image: BaselineImageIdentity,
    pub new_image: BaselineImageIdentity,
    pub old_binding: ReferenceBinding,
    pub new_binding: ReferenceBinding,
    pub metrics_before: BaselineArtifactIdentity,
    pub metrics_after: BaselineArtifactIdentity,
    pub human_approval_required: bool,
    pub automatic_fix_may_apply: bool,
    pub rerun_requirements: Vec<BaselineCaptureRequirement>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BaselineApproval {
    pub schema_version: u32,
    pub plan_sha256: String,
    pub approved: bool,
    pub approver: String,
    pub approved_at: String,
    pub rationale: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BaselineUpdateReceipt {
    pub schema_version: u32,
    pub algorithm_version: String,
    pub status: String,
    pub reference_id: String,
    pub plan: BaselineArtifactIdentity,
    pub approval: BaselineArtifactIdentity,
    pub manifest_before_sha256: String,
    pub manifest_after_sha256: String,
    pub old_binding: ReferenceBinding,
    pub new_binding: ReferenceBinding,
    pub human_approved: bool,
    pub reason: String,
    pub old_image: BaselineImageIdentity,
    pub new_image: BaselineImageIdentity,
    pub metrics_before: BaselineArtifactIdentity,
    pub metrics_after: BaselineArtifactIdentity,
    pub rerun_requirements: Vec<BaselineCaptureRequirement>,
    pub rerun_verification_required: bool,
    pub acceptance_complete: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BaselineRerunVerification {
    pub schema_version: u32,
    pub algorithm_version: String,
    pub status: String,
    pub receipt: BaselineArtifactIdentity,
    pub comparison_result: BaselineArtifactIdentity,
    pub verified_capture_ids: Vec<String>,
    pub acceptance_complete: bool,
}

pub fn plan_baseline_update(
    request: &BaselinePlanRequest,
) -> Result<BaselineUpdatePlan, ComparisonError> {
    let root = canonical_repository(&request.repository_root)?;
    validate_reason(&request.reason)?;
    let manifest_path = resolve_repo_file(&root, &request.manifest)?;
    let manifest_bytes = read_bounded(&manifest_path, MAX_JSON_BYTES)?;
    let validated = parse_and_validate_manifest(&root, &manifest_bytes).map_err(|error| {
        baseline_plan_error(format!("reference manifest is invalid: {error}"))
            .at_path(&manifest_path)
    })?;
    let target = validated
        .manifest
        .references
        .iter()
        .find(|entry| entry.reference_id == request.reference_id)
        .ok_or_else(|| baseline_plan_error("reference_id does not exist in the manifest"))?;
    if validated
        .manifest
        .references
        .iter()
        .filter(|entry| {
            entry.image.storage == target.image.storage
                && entry.image.relative_path == target.image.relative_path
        })
        .count()
        != 1
    {
        return Err(baseline_plan_error(
            "a baseline update target must own a unique physical image path",
        ));
    }
    let resolved = validated
        .references
        .iter()
        .find(|entry| entry.reference_id == request.reference_id)
        .ok_or_else(|| baseline_plan_error("validated reference image is missing"))?;
    let new_image_path = resolve_repo_file(&root, &request.new_image)?;
    if new_image_path == resolved.path {
        return Err(baseline_plan_error(
            "new image must be a separate candidate; planning never overwrites a baseline",
        ));
    }
    let old_image = image_identity(&root, &resolved.path)?;
    let new_image = image_identity(&root, &new_image_path)?;
    if old_image.sha256 == new_image.sha256 {
        return Err(baseline_plan_error(
            "candidate image hash must differ from the active baseline",
        ));
    }
    if old_image.format != new_image.format
        || (old_image.width, old_image.height) != (new_image.width, new_image.height)
    {
        return Err(baseline_plan_error(
            "candidate format and dimensions must equal the active baseline",
        ));
    }
    let manifest_identity = identity_from_bytes(&root, &manifest_path, &manifest_bytes);
    let metrics_before =
        metric_identity(&root, &resolve_repo_file(&root, &request.metrics_before)?)?;
    let metrics_after = metric_identity(&root, &resolve_repo_file(&root, &request.metrics_after)?)?;
    let old_binding = binding(target);
    let new_binding = ReferenceBinding {
        sha256: new_image.sha256.clone(),
        revision: target
            .baseline
            .version
            .checked_add(1)
            .ok_or_else(|| baseline_plan_error("baseline revision overflowed"))?,
    };
    let rerun_requirements = related_capture_requirements(
        &validated.manifest,
        target,
        &request.reference_id,
        &new_binding,
    );
    let plan_seed = format!(
        "{}:{}:{}:{}:{}",
        manifest_identity.sha256,
        request.reference_id,
        old_image.sha256,
        new_image.sha256,
        request.reason.trim()
    );
    let plan = BaselineUpdatePlan {
        schema_version: BASELINE_PLAN_SCHEMA_VERSION,
        algorithm_version: BASELINE_PLAN_ALGORITHM_VERSION.to_owned(),
        plan_id: format!("baseline-{}", &hash_bytes(plan_seed.as_bytes())[..16]),
        manifest: manifest_identity,
        reference_id: request.reference_id.clone(),
        target_image_path: old_image.path.clone(),
        reason: request.reason.trim().to_owned(),
        old_image,
        new_image,
        old_binding,
        new_binding,
        metrics_before,
        metrics_after,
        human_approval_required: true,
        automatic_fix_may_apply: false,
        rerun_requirements,
    };
    validate_plan(&plan)?;
    let output = create_new_output_directory(
        &root,
        &request.allowed_output_root,
        &request.output_directory,
    )?;
    write_new_json(&output.join(BASELINE_PLAN_FILENAME), &plan)?;
    Ok(plan)
}

pub fn apply_baseline_update(
    request: &BaselineApplyRequest,
) -> Result<BaselineUpdateReceipt, ComparisonError> {
    let root = canonical_repository(&request.repository_root)?;
    let plan_path = resolve_repo_file(&root, &request.plan)?;
    let approval_path = resolve_repo_file(&root, &request.approval)?;
    let (plan, plan_bytes) = read_json::<BaselineUpdatePlan>(&plan_path, MAX_JSON_BYTES)?;
    let (approval, approval_bytes) = read_json::<BaselineApproval>(&approval_path, MAX_JSON_BYTES)?;
    validate_plan(&plan)?;
    validate_approval(&approval, &hash_bytes(&plan_bytes))?;

    let manifest_path = resolve_repo_file(&root, Path::new(&plan.manifest.path))?;
    let current_manifest_bytes = read_bounded(&manifest_path, MAX_JSON_BYTES)?;
    let current_manifest_identity =
        identity_from_bytes(&root, &manifest_path, &current_manifest_bytes);
    if current_manifest_identity.sha256 != plan.manifest.sha256 {
        return Err(baseline_conflict(
            "reference manifest changed after the plan was created",
        ));
    }
    let new_image_path = resolve_repo_file(&root, Path::new(&plan.new_image.path))?;
    let candidate_image_bytes = read_bounded(&new_image_path, MAX_IMAGE_BYTES)?;
    if image_identity_from_bytes(&root, &new_image_path, &candidate_image_bytes)? != plan.new_image
    {
        return Err(baseline_conflict(
            "candidate image changed after the plan was created",
        ));
    }
    for identity in [&plan.metrics_before, &plan.metrics_after] {
        let path = resolve_repo_file(&root, Path::new(&identity.path))?;
        if file_identity(&root, &path, MAX_JSON_BYTES)? != *identity {
            return Err(baseline_conflict(
                "metric evidence changed after the plan was created",
            ));
        }
    }

    let validated =
        parse_and_validate_manifest(&root, &current_manifest_bytes).map_err(|error| {
            baseline_conflict(format!("active manifest no longer validates: {error}"))
        })?;
    let mut candidate = validated.manifest.clone();
    let entry = candidate
        .references
        .iter_mut()
        .find(|entry| entry.reference_id == plan.reference_id)
        .ok_or_else(|| baseline_conflict("planned reference_id no longer exists"))?;
    if binding(entry) != plan.old_binding
        || resolve_reference_path(&root, entry)? != root.join(&plan.target_image_path)
    {
        return Err(baseline_conflict(
            "active reference identity or target path differs from the plan",
        ));
    }
    let previous = entry.clone();
    entry.image.sha256 = plan.new_binding.sha256.clone();
    entry.baseline.version = plan.new_binding.revision;
    entry.baseline.previous_sha256 = Some(plan.old_binding.sha256.clone());
    entry.baseline.update_reason = plan.reason.clone();
    validate_baseline_update(&previous, entry)
        .map_err(|error| baseline_conflict(format!("planned transition is invalid: {error}")))?;
    let target_image_path = resolve_reference_path(&root, entry)?;
    if target_image_path == new_image_path {
        return Err(baseline_conflict(
            "candidate image must remain separate from the destination",
        ));
    }
    let output = create_new_output_directory(
        &root,
        &request.allowed_output_root,
        &request.output_directory,
    )?;
    let candidate_manifest_bytes = pretty_json_bytes(&candidate)?;
    let old_image_bytes = read_bounded(&target_image_path, MAX_IMAGE_BYTES)?;
    let old_archive_path = output.join(format!("old-reference.{}", plan.old_image.format));
    let new_archive_path = output.join(format!("new-reference.{}", plan.new_image.format));
    if let Err(error) = write_create_new(&old_archive_path, &old_image_bytes)
        .and_then(|()| write_create_new(&new_archive_path, &candidate_image_bytes))
    {
        let _ = fs::remove_dir_all(&output);
        return Err(error);
    }
    let archived_old_image = image_identity_from_bytes(&root, &old_archive_path, &old_image_bytes)?;
    let archived_new_image =
        image_identity_from_bytes(&root, &new_archive_path, &candidate_image_bytes)?;
    let manifest_after_sha256 = hash_bytes(&candidate_manifest_bytes);
    let receipt = BaselineUpdateReceipt {
        schema_version: BASELINE_RECEIPT_SCHEMA_VERSION,
        algorithm_version: BASELINE_PLAN_ALGORITHM_VERSION.to_owned(),
        status: "applied_rerun_required".to_owned(),
        reference_id: plan.reference_id.clone(),
        plan: identity_from_bytes(&root, &plan_path, &plan_bytes),
        approval: identity_from_bytes(&root, &approval_path, &approval_bytes),
        manifest_before_sha256: current_manifest_identity.sha256,
        manifest_after_sha256: manifest_after_sha256.clone(),
        old_binding: plan.old_binding.clone(),
        new_binding: plan.new_binding.clone(),
        human_approved: true,
        reason: plan.reason.clone(),
        old_image: archived_old_image,
        new_image: archived_new_image,
        metrics_before: plan.metrics_before.clone(),
        metrics_after: plan.metrics_after.clone(),
        rerun_requirements: plan.rerun_requirements.clone(),
        rerun_verification_required: true,
        acceptance_complete: false,
    };
    let receipt_path = output.join(BASELINE_RECEIPT_FILENAME);
    let staged_receipt_path = sibling_temp(&receipt_path, std::process::id(), "receipt-staged")?;
    if let Err(error) =
        pretty_json_bytes(&receipt).and_then(|bytes| write_create_new(&staged_receipt_path, &bytes))
    {
        let _ = fs::remove_dir_all(&output);
        return Err(error);
    }
    if let Err(error) = apply_two_file_transaction(
        &manifest_path,
        &candidate_manifest_bytes,
        &plan.manifest.sha256,
        &target_image_path,
        &candidate_image_bytes,
        &plan.old_image.sha256,
        || {
            let applied_identity = file_identity(&root, &manifest_path, MAX_JSON_BYTES)?;
            if applied_identity.sha256 != manifest_after_sha256 {
                return Err(ComparisonError::internal_failure(
                    "baseline manifest does not match the staged post-update receipt",
                ));
            }
            load_and_validate_manifest(&root, &manifest_path).map_err(|error| {
                ComparisonError::internal_failure(format!(
                    "updated manifest failed post-transaction validation: {error}"
                ))
            })?;
            fs::rename(&staged_receipt_path, &receipt_path).map_err(transaction_error)
        },
    ) {
        let _ = fs::remove_dir_all(&output);
        return Err(error);
    }
    Ok(receipt)
}

pub fn verify_baseline_rerun(
    request: &BaselineRerunVerificationRequest,
) -> Result<BaselineRerunVerification, ComparisonError> {
    let root = canonical_repository(&request.repository_root)?;
    let receipt_path = resolve_repo_file(&root, &request.receipt)?;
    let comparison_path = resolve_repo_file(&root, &request.comparison_result)?;
    let (receipt, receipt_bytes) =
        read_json::<BaselineUpdateReceipt>(&receipt_path, MAX_JSON_BYTES)?;
    let (comparison, comparison_bytes) =
        read_json::<ComparisonResult>(&comparison_path, MAX_JSON_BYTES)?;
    if receipt.schema_version != BASELINE_RECEIPT_SCHEMA_VERSION
        || receipt.algorithm_version != BASELINE_PLAN_ALGORITHM_VERSION
        || receipt.status != "applied_rerun_required"
        || !receipt.human_approved
        || receipt.acceptance_complete
        || !receipt.rerun_verification_required
    {
        return Err(baseline_rerun_error(
            "receipt is not an applied update awaiting rerun verification",
        ));
    }
    let active_manifest = validate_receipt_provenance(&root, &receipt)?;
    let approved_plan_path = resolve_repo_file(&root, Path::new(&receipt.plan.path))?;
    let (approved_plan, _) = read_json::<BaselineUpdatePlan>(&approved_plan_path, MAX_JSON_BYTES)?;
    let active_manifest_path = resolve_repo_file(&root, Path::new(&approved_plan.manifest.path))?;
    if comparison.schema_version != COMPARISON_RESULT_SCHEMA_VERSION
        || comparison.algorithm_version != COMPARISON_BUNDLE_ALGORITHM_VERSION
        || comparison.status != "passed"
    {
        return Err(baseline_rerun_error(
            "comparison result must have passed before baseline rerun acceptance",
        ));
    }
    validate_comparison_provenance(&root, &comparison)?;
    let mut verified = BTreeSet::new();
    for requirement in &receipt.rerun_requirements {
        let active_entry = active_manifest
            .references
            .iter()
            .find(|entry| entry.reference_id == requirement.reference_id)
            .ok_or_else(|| {
                baseline_rerun_error(format!(
                    "required reference {} no longer exists in the active manifest",
                    requirement.reference_id
                ))
            })?;
        if binding(active_entry) != requirement.expected_binding
            || active_entry.key.screen != requirement.screen
            || active_entry.key.device != requirement.device
            || active_entry.key.state != requirement.state
        {
            return Err(baseline_rerun_error(format!(
                "required reference {} no longer matches its active device/state binding",
                requirement.reference_id
            )));
        }
        let capture = comparison
            .captures
            .iter()
            .find(|capture| capture.capture_id == requirement.capture_id)
            .ok_or_else(|| {
                baseline_rerun_error(format!(
                    "required capture {} was not rerun",
                    requirement.capture_id
                ))
            })?;
        if capture.screen != requirement.screen
            || capture.device != requirement.device
            || capture.state != requirement.state
            || capture.reference_binding != requirement.expected_binding
            || capture.baseline_guard.reference_id != requirement.reference_id
            || capture.baseline_guard.observed != requirement.expected_binding
            || capture.artifacts.reference.sha256 != requirement.expected_binding.sha256
            || capture.baseline_guard.reference_manifest.sha256 != receipt.manifest_after_sha256
            || resolve_repo_file(
                &root,
                Path::new(&capture.baseline_guard.reference_manifest.path),
            )? != active_manifest_path
            || capture.gate_state != "passed"
        {
            return Err(baseline_rerun_error(format!(
                "required capture {} does not prove the expected baseline and passed gate state",
                requirement.capture_id
            )));
        }
        verified.insert(capture.capture_id.clone());
    }
    let output = create_new_output_directory(
        &root,
        &request.allowed_output_root,
        &request.output_directory,
    )?;
    let verification = BaselineRerunVerification {
        schema_version: BASELINE_RERUN_VERIFICATION_SCHEMA_VERSION,
        algorithm_version: BASELINE_PLAN_ALGORITHM_VERSION.to_owned(),
        status: "complete".to_owned(),
        receipt: identity_from_bytes(&root, &receipt_path, &receipt_bytes),
        comparison_result: identity_from_bytes(&root, &comparison_path, &comparison_bytes),
        verified_capture_ids: verified.into_iter().collect(),
        acceptance_complete: true,
    };
    write_new_json(
        &output.join(BASELINE_RERUN_VERIFICATION_FILENAME),
        &verification,
    )?;
    Ok(verification)
}

fn validate_receipt_provenance(
    root: &Path,
    receipt: &BaselineUpdateReceipt,
) -> Result<ReferenceManifest, ComparisonError> {
    let plan_path = resolve_repo_file(root, Path::new(&receipt.plan.path))?;
    let (plan, plan_bytes) = read_json::<BaselineUpdatePlan>(&plan_path, MAX_JSON_BYTES)?;
    if identity_from_bytes(root, &plan_path, &plan_bytes) != receipt.plan {
        return Err(baseline_rerun_error(
            "receipt plan identity no longer matches its bound file",
        ));
    }
    validate_plan(&plan)?;
    let approval_path = resolve_repo_file(root, Path::new(&receipt.approval.path))?;
    let (approval, approval_bytes) = read_json::<BaselineApproval>(&approval_path, MAX_JSON_BYTES)?;
    if identity_from_bytes(root, &approval_path, &approval_bytes) != receipt.approval {
        return Err(baseline_rerun_error(
            "receipt approval identity no longer matches its bound file",
        ));
    }
    validate_approval(&approval, &hash_bytes(&plan_bytes))?;
    if plan.reference_id != receipt.reference_id
        || plan.old_binding != receipt.old_binding
        || plan.new_binding != receipt.new_binding
        || plan.reason != receipt.reason
        || plan.rerun_requirements != receipt.rerun_requirements
        || plan.old_image.sha256 != receipt.old_image.sha256
        || plan.new_image.sha256 != receipt.new_image.sha256
        || plan.old_image.format != receipt.old_image.format
        || plan.new_image.format != receipt.new_image.format
        || (plan.old_image.width, plan.old_image.height)
            != (receipt.old_image.width, receipt.old_image.height)
        || (plan.new_image.width, plan.new_image.height)
            != (receipt.new_image.width, receipt.new_image.height)
    {
        return Err(baseline_rerun_error(
            "receipt contents do not match the approved plan",
        ));
    }
    for image in [&receipt.old_image, &receipt.new_image] {
        let path = resolve_repo_file(root, Path::new(&image.path))?;
        if image_identity(root, &path)? != *image {
            return Err(baseline_rerun_error(
                "receipt image evidence no longer matches its archived file",
            ));
        }
    }
    for metrics in [&plan.metrics_before, &plan.metrics_after] {
        let path = resolve_repo_file(root, Path::new(&metrics.path))?;
        if metric_identity(root, &path)? != *metrics {
            return Err(baseline_rerun_error(
                "baseline metric evidence changed after planning",
            ));
        }
    }
    let manifest_path = resolve_repo_file(root, Path::new(&plan.manifest.path))?;
    let active_manifest = file_identity(root, &manifest_path, MAX_JSON_BYTES)?;
    if active_manifest.sha256 != receipt.manifest_after_sha256 {
        return Err(baseline_rerun_error(
            "active reference manifest changed after baseline apply",
        ));
    }
    load_and_validate_manifest(root, &manifest_path)
        .map(|validated| validated.manifest)
        .map_err(|error| {
            baseline_rerun_error(format!(
                "active reference manifest no longer validates: {error}"
            ))
        })
}

fn validate_comparison_provenance(
    root: &Path,
    comparison: &ComparisonResult,
) -> Result<(), ComparisonError> {
    validate_comparison_result_provenance(root, comparison).map_err(|error| {
        baseline_rerun_error(format!(
            "comparison result provenance is not trusted: {}",
            error.failure.message
        ))
    })
}

pub(crate) fn validate_plan(plan: &BaselineUpdatePlan) -> Result<(), ComparisonError> {
    if plan.schema_version != BASELINE_PLAN_SCHEMA_VERSION
        || plan.algorithm_version != BASELINE_PLAN_ALGORITHM_VERSION
        || !plan.human_approval_required
        || plan.automatic_fix_may_apply
        || plan.reference_id.trim().is_empty()
        || plan.reason.trim().is_empty()
        || plan.old_binding.sha256 != plan.old_image.sha256
        || plan.new_binding.sha256 != plan.new_image.sha256
        || plan.new_binding.revision != plan.old_binding.revision.checked_add(1).unwrap_or(0)
        || plan.rerun_requirements.is_empty()
    {
        return Err(baseline_plan_error(
            "baseline update plan contract is invalid",
        ));
    }
    validate_reason(&plan.reason)?;
    for hash in [
        &plan.manifest.sha256,
        &plan.old_image.sha256,
        &plan.new_image.sha256,
        &plan.metrics_before.sha256,
        &plan.metrics_after.sha256,
    ] {
        if !valid_hash(hash) {
            return Err(baseline_plan_error("plan contains an invalid SHA-256"));
        }
    }
    let mut captures = BTreeSet::new();
    if plan
        .rerun_requirements
        .iter()
        .any(|requirement| !captures.insert(&requirement.capture_id))
    {
        return Err(baseline_plan_error(
            "rerun requirement capture IDs must be unique",
        ));
    }
    Ok(())
}

pub(crate) fn validate_approval(
    approval: &BaselineApproval,
    plan_hash: &str,
) -> Result<(), ComparisonError> {
    if approval.schema_version != BASELINE_APPROVAL_SCHEMA_VERSION
        || approval.plan_sha256 != plan_hash
        || !approval.approved
        || !valid_record_text(&approval.approver, 256)
        || !looks_like_rfc3339(&approval.approved_at)
        || !valid_record_text(&approval.rationale, 4096)
    {
        return Err(ComparisonError::input(
            ComparisonErrorCode::BaselineApprovalRequired,
            "a matching explicit human approval record is required",
        ));
    }
    Ok(())
}

fn valid_record_text(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty() && value.len() <= maximum && !value.chars().any(char::is_control)
}

fn looks_like_rfc3339(value: &str) -> bool {
    let bytes = value.as_bytes();
    if !(20..=64).contains(&bytes.len())
        || bytes.get(4) != Some(&b'-')
        || bytes.get(7) != Some(&b'-')
        || bytes.get(10) != Some(&b'T')
        || bytes.get(13) != Some(&b':')
        || bytes.get(16) != Some(&b':')
    {
        return false;
    }
    value.ends_with('Z')
        || bytes.len().checked_sub(6).is_some_and(|offset| {
            matches!(bytes.get(offset), Some(b'+') | Some(b'-'))
                && bytes.get(offset + 3) == Some(&b':')
        })
}

fn related_capture_requirements(
    manifest: &ReferenceManifest,
    target: &ReferenceEntry,
    target_reference_id: &str,
    new_binding: &ReferenceBinding,
) -> Vec<BaselineCaptureRequirement> {
    let mut requirements = manifest
        .references
        .iter()
        .filter(|entry| {
            entry.key.screen == target.key.screen
                && entry.key.locale == target.key.locale
                && entry.key.theme == target.key.theme
        })
        .map(|entry| BaselineCaptureRequirement {
            capture_id: format!(
                "{}.{}.{}",
                entry.key.screen, entry.key.device, entry.key.state
            ),
            reference_id: entry.reference_id.clone(),
            screen: entry.key.screen.clone(),
            device: entry.key.device.clone(),
            state: entry.key.state.clone(),
            expected_binding: if entry.reference_id == target_reference_id {
                new_binding.clone()
            } else {
                binding(entry)
            },
        })
        .collect::<Vec<_>>();
    requirements.sort_by(|left, right| left.capture_id.cmp(&right.capture_id));
    requirements
}

fn apply_two_file_transaction<F>(
    manifest_path: &Path,
    manifest_bytes: &[u8],
    expected_manifest_sha256: &str,
    image_path: &Path,
    image_bytes: &[u8],
    expected_image_sha256: &str,
    finalize: F,
) -> Result<(), ComparisonError>
where
    F: FnOnce() -> Result<(), ComparisonError>,
{
    let nonce = std::process::id();
    let manifest_temp = sibling_temp(manifest_path, nonce, "manifest-new")?;
    let manifest_backup = sibling_temp(manifest_path, nonce, "manifest-old")?;
    let image_temp = sibling_temp(image_path, nonce, "image-new")?;
    let image_backup = sibling_temp(image_path, nonce, "image-old")?;
    write_create_new(&manifest_temp, manifest_bytes)?;
    if let Err(error) = write_create_new(&image_temp, image_bytes) {
        let _ = fs::remove_file(&manifest_temp);
        return Err(error);
    }
    let (current_manifest, current_image) = match (
        read_bounded(manifest_path, MAX_JSON_BYTES),
        read_bounded(image_path, MAX_IMAGE_BYTES),
    ) {
        (Ok(manifest), Ok(image)) => (manifest, image),
        (Err(error), _) | (_, Err(error)) => {
            let _ = fs::remove_file(&manifest_temp);
            let _ = fs::remove_file(&image_temp);
            return Err(error);
        }
    };
    if hash_bytes(&current_manifest) != expected_manifest_sha256
        || hash_bytes(&current_image) != expected_image_sha256
    {
        let _ = fs::remove_file(&manifest_temp);
        let _ = fs::remove_file(&image_temp);
        return Err(baseline_conflict(
            "baseline manifest or image changed immediately before transaction apply",
        ));
    }
    let result = (|| {
        fs::rename(image_path, &image_backup).map_err(transaction_error)?;
        fs::rename(&image_temp, image_path).map_err(transaction_error)?;
        fs::rename(manifest_path, &manifest_backup).map_err(transaction_error)?;
        fs::rename(&manifest_temp, manifest_path).map_err(transaction_error)?;
        finalize()?;
        Ok(())
    })();
    if let Err(error) = result {
        if manifest_backup.exists() {
            let _ = fs::remove_file(manifest_path);
            let _ = fs::rename(&manifest_backup, manifest_path);
        }
        if image_backup.exists() {
            let _ = fs::remove_file(image_path);
            let _ = fs::rename(&image_backup, image_path);
        }
        let _ = fs::remove_file(&manifest_temp);
        let _ = fs::remove_file(&image_temp);
        return Err(error);
    }
    let _ = fs::remove_file(&manifest_backup);
    let _ = fs::remove_file(&image_backup);
    Ok(())
}

fn sibling_temp(path: &Path, nonce: u32, suffix: &str) -> Result<PathBuf, ComparisonError> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| baseline_plan_error("baseline target filename is invalid"))?;
    Ok(path.with_file_name(format!(".{name}.{nonce}.{suffix}")))
}

fn transaction_error(error: std::io::Error) -> ComparisonError {
    ComparisonError::internal_failure(format!("baseline transaction failed: {error}"))
}

fn create_new_output_directory(
    root: &Path,
    allowed_output_root: &Path,
    output_directory: &Path,
) -> Result<PathBuf, ComparisonError> {
    let allowed = resolve_repo_directory(root, allowed_output_root)?;
    let output = if output_directory.is_absolute() {
        output_directory.to_path_buf()
    } else {
        root.join(output_directory)
    };
    let parent = output
        .parent()
        .ok_or_else(|| baseline_plan_error("output directory requires a parent"))?;
    let canonical_parent = fs::canonicalize(parent).map_err(|error| {
        baseline_plan_error(format!("output parent cannot be resolved: {error}"))
    })?;
    if !canonical_parent.starts_with(&allowed) {
        return Err(baseline_plan_error(
            "output directory is outside the allowed output root",
        ));
    }
    fs::create_dir(&output).map_err(|error| {
        baseline_plan_error(format!(
            "output directory must be new and cannot be created: {error}"
        ))
    })?;
    fs::canonicalize(&output).map_err(|error| {
        baseline_plan_error(format!("new output directory cannot be resolved: {error}"))
    })
}

fn resolve_repo_directory(root: &Path, path: &Path) -> Result<PathBuf, ComparisonError> {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let canonical = fs::canonicalize(&joined).map_err(|error| {
        baseline_plan_error(format!("directory cannot be resolved: {error}")).at_path(&joined)
    })?;
    if !canonical.starts_with(root) || !canonical.is_dir() {
        return Err(baseline_plan_error(
            "directory must be an existing directory inside the repository",
        ));
    }
    Ok(canonical)
}

fn resolve_repo_file(root: &Path, path: &Path) -> Result<PathBuf, ComparisonError> {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let canonical = fs::canonicalize(&joined).map_err(|error| {
        baseline_plan_error(format!("file cannot be resolved: {error}")).at_path(&joined)
    })?;
    if !canonical.starts_with(root) || !canonical.is_file() {
        return Err(baseline_plan_error(
            "file must be an existing regular file inside the repository",
        ));
    }
    Ok(canonical)
}

fn resolve_reference_path(root: &Path, entry: &ReferenceEntry) -> Result<PathBuf, ComparisonError> {
    let base = match entry.image.storage {
        ReferenceStorage::CommittedFixture => COMMITTED_REFERENCE_ROOT,
        ReferenceStorage::TemporaryLocal => TEMPORARY_REFERENCE_ROOT,
    };
    let path = root.join(base).join(&entry.image.relative_path);
    let parent = path
        .parent()
        .and_then(|parent| fs::canonicalize(parent).ok())
        .ok_or_else(|| baseline_plan_error("reference image parent cannot be resolved"))?;
    let allowed = fs::canonicalize(root.join(base)).map_err(|error| {
        baseline_plan_error(format!("reference root cannot be resolved: {error}"))
    })?;
    if !parent.starts_with(&allowed) {
        return Err(baseline_plan_error(
            "reference image path escaped its storage root",
        ));
    }
    Ok(path)
}

fn image_identity(root: &Path, path: &Path) -> Result<BaselineImageIdentity, ComparisonError> {
    let bytes = read_bounded(path, MAX_IMAGE_BYTES)?;
    image_identity_from_bytes(root, path, &bytes)
}

fn image_identity_from_bytes(
    root: &Path,
    path: &Path,
    bytes: &[u8],
) -> Result<BaselineImageIdentity, ComparisonError> {
    let mut reader = ImageReader::new(Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|error| {
            baseline_plan_error(format!("image format cannot be detected: {error}"))
        })?;
    let format_label = match reader.format() {
        Some(ImageFormat::Png) => "png",
        Some(ImageFormat::Jpeg) => "jpeg",
        _ => return Err(baseline_plan_error("baseline images must be PNG or JPEG")),
    };
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_ALLOC);
    reader.limits(limits);
    let decoded = reader.decode().map_err(|error| match error {
        ImageError::Limits(_) => baseline_plan_error("image exceeded decoder limits"),
        _ => baseline_plan_error(format!("image is truncated or corrupt: {error}")),
    })?;
    let (width, height) = (decoded.width(), decoded.height());
    if width == 0 || height == 0 || width > MAX_IMAGE_DIMENSION || height > MAX_IMAGE_DIMENSION {
        return Err(baseline_plan_error(
            "candidate image dimensions are outside limits",
        ));
    }
    Ok(BaselineImageIdentity {
        path: repo_relative(root, path),
        sha256: hash_bytes(bytes),
        byte_length: bytes.len() as u64,
        format: format_label.to_owned(),
        width,
        height,
    })
}

fn file_identity(
    root: &Path,
    path: &Path,
    maximum: u64,
) -> Result<BaselineArtifactIdentity, ComparisonError> {
    let bytes = read_bounded(path, maximum)?;
    Ok(identity_from_bytes(root, path, &bytes))
}

fn metric_identity(root: &Path, path: &Path) -> Result<BaselineArtifactIdentity, ComparisonError> {
    let (_, bytes) = read_json::<serde_json::Value>(path, MAX_JSON_BYTES)?;
    Ok(identity_from_bytes(root, path, &bytes))
}

fn identity_from_bytes(root: &Path, path: &Path, bytes: &[u8]) -> BaselineArtifactIdentity {
    BaselineArtifactIdentity {
        path: repo_relative(root, path),
        sha256: hash_bytes(bytes),
        byte_length: bytes.len() as u64,
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(
    path: &Path,
    maximum: u64,
) -> Result<(T, Vec<u8>), ComparisonError> {
    let bytes = read_bounded(path, maximum)?;
    let value = serde_json::from_slice(&bytes).map_err(|error| {
        baseline_plan_error(format!("strict JSON parse failed: {error}")).at_path(path)
    })?;
    Ok((value, bytes))
}

fn read_bounded(path: &Path, maximum: u64) -> Result<Vec<u8>, ComparisonError> {
    let file = fs::File::open(path).map_err(|error| {
        baseline_plan_error(format!("file cannot be opened: {error}")).at_path(path)
    })?;
    let length = file
        .metadata()
        .map_err(|error| {
            baseline_plan_error(format!("file metadata cannot be read: {error}")).at_path(path)
        })?
        .len();
    if length > maximum {
        return Err(baseline_plan_error(format!("file exceeds {maximum} bytes")).at_path(path));
    }
    let mut bytes = Vec::with_capacity(length as usize);
    file.take(maximum + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            baseline_plan_error(format!("file cannot be read: {error}")).at_path(path)
        })?;
    if bytes.len() as u64 > maximum {
        return Err(baseline_plan_error(format!("file exceeds {maximum} bytes")).at_path(path));
    }
    Ok(bytes)
}

fn write_new_json(path: &Path, value: &impl Serialize) -> Result<(), ComparisonError> {
    let bytes = pretty_json_bytes(value)?;
    write_create_new(path, &bytes)
}

fn pretty_json_bytes(value: &impl Serialize) -> Result<Vec<u8>, ComparisonError> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        ComparisonError::internal_failure(format!("baseline JSON serialization failed: {error}"))
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn write_create_new(path: &Path, bytes: &[u8]) -> Result<(), ComparisonError> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| {
            ComparisonError::internal_failure(format!("artifact cannot be created: {error}"))
                .at_path(path)
        })?;
    file.write_all(bytes).map_err(|error| {
        ComparisonError::internal_failure(format!("artifact cannot be written: {error}"))
            .at_path(path)
    })
}

fn binding(entry: &ReferenceEntry) -> ReferenceBinding {
    ReferenceBinding {
        sha256: entry.image.sha256.clone(),
        revision: entry.baseline.version,
    }
}

fn validate_reason(reason: &str) -> Result<(), ComparisonError> {
    if reason.trim().is_empty() || reason.len() > 4096 || reason.chars().any(char::is_control) {
        return Err(baseline_plan_error(
            "update reason must be non-empty, control-free, and at most 4096 bytes",
        ));
    }
    Ok(())
}

fn repo_relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
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

fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn baseline_plan_error(message: impl Into<String>) -> ComparisonError {
    ComparisonError::input(ComparisonErrorCode::BaselinePlanInvalid, message)
}

fn baseline_conflict(message: impl Into<String>) -> ComparisonError {
    ComparisonError::input(ComparisonErrorCode::BaselineConflict, message)
}

fn baseline_rerun_error(message: impl Into<String>) -> ComparisonError {
    ComparisonError::input(ComparisonErrorCode::BaselineRerunIncomplete, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transaction_rolls_back_when_receipt_finalization_fails() {
        let temporary = tempfile::tempdir().unwrap();
        let manifest_path = temporary.path().join("references.json");
        let image_path = temporary.path().join("reference.png");
        let old_manifest = b"old-manifest";
        let old_image = b"old-image";
        fs::write(&manifest_path, old_manifest).unwrap();
        fs::write(&image_path, old_image).unwrap();

        let error = apply_two_file_transaction(
            &manifest_path,
            b"new-manifest",
            &hash_bytes(old_manifest),
            &image_path,
            b"new-image",
            &hash_bytes(old_image),
            || {
                Err(ComparisonError::internal_failure(
                    "receipt finalization failed",
                ))
            },
        )
        .unwrap_err();

        assert_eq!(error.failure.code, ComparisonErrorCode::InternalFailure);
        assert_eq!(fs::read(&manifest_path).unwrap(), old_manifest);
        assert_eq!(fs::read(&image_path).unwrap(), old_image);
        let leftovers = fs::read_dir(temporary.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .filter(|name| name.to_string_lossy().starts_with('.'))
            .collect::<Vec<_>>();
        assert!(
            leftovers.is_empty(),
            "transaction left temporary files behind"
        );
    }
}
