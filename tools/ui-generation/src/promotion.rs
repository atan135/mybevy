//! Explicit, human-approved promotion from an immutable run bundle into approved game assets.
//!
//! The normal generation commands deliberately have no path to `project/assets` or `project/src`.
//! This module is the one exception. It verifies the sealed run evidence again, records a bounded
//! human decision document in that run, emits a deterministic plan, and only commits a new owned
//! approved-page directory after the caller repeats the plan hash.

use crate::{
    asset_strategy::{
        AlphaMode, AssetCatalog, AssetQualityVerdict, AssetSpecification, CatalogLicenseStatus,
        inspect_asset_file,
    },
    lifecycle::{TaskFailure, TaskFailureKind},
    run_manifest::{
        ArtifactLink, RUN_MANIFEST_PROTOCOL_VERSION, RunArtifactRecord, RunBundleStatus,
        UiGenerationRunManifest,
    },
};
use project::framework::ui::document::tooling::{
    CURRENT_SCHEMA_VERSION, canonicalize_json, parse_approved_document_registration, validate_json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub const PROMOTION_PROTOCOL_VERSION: u32 = 1;
pub const PROMOTION_DECISION_MANIFEST: &str = "approval/promotion-decisions.v1.json";
const PROMOTION_DECISION_MARKER: &str = "approval/PROMOTION_DECISIONS_COMMITTED";
const MAX_PROMOTION_FILE_BYTES: usize = 4 * 1024 * 1024;
const MAX_PROMOTION_RESOURCE_BYTES: usize = 16 * 1024 * 1024;
const MAX_DECISIONS: usize = 16;
const MAX_TEXT_BYTES: usize = 1_024;
const MAX_APPROVED_DOCUMENTS: usize = 512;
static STAGING_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PromotionDecisionKind {
    ReleaseApproval,
    AssetLicense,
    CoreLayout,
    BusinessAction,
    FrameworkCapability,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PromotionResolution {
    Accept,
    Reject,
    ReplaceAsset,
    ModifyText,
    ModifyConstraint,
    KeepPlaceholder,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionReferenceRegion {
    pub reference_id: Option<String>,
    pub region_id: Option<String>,
    pub element_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionDecisionCandidate {
    pub candidate_id: String,
    pub resolution: PromotionResolution,
    pub summary: String,
    pub impact: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionQuestion {
    pub question_id: String,
    pub kind: PromotionDecisionKind,
    pub reference_region: PromotionReferenceRegion,
    pub prompt: String,
    pub candidates: Vec<PromotionDecisionCandidate>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionDecision {
    pub question_id: String,
    pub candidate_id: String,
    pub resolution: PromotionResolution,
    pub rationale: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionDecisionSubmission {
    pub protocol_version: u32,
    pub run_id: String,
    pub run_manifest_sha256: String,
    pub canonical_document_sha256: String,
    pub input_sha256: String,
    pub approved_by: String,
    pub decisions: Vec<PromotionDecision>,
    #[serde(default)]
    pub resources: Vec<PromotionResourceSubmission>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PromotionResourceLicenseStatus {
    ProjectOwned,
    Redistributable,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionResourceSubmission {
    pub strategy_id: String,
    pub asset_id: String,
    pub source_relative_path: String,
    pub source_sha256: String,
    pub byte_length: u64,
    pub target_file_name: String,
    pub license_status: PromotionResourceLicenseStatus,
    pub license_reference: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionDecisionTemplate {
    pub protocol_version: u32,
    pub run_id: String,
    pub run_manifest_sha256: String,
    pub canonical_document_sha256: String,
    pub input_sha256: String,
    pub questions: Vec<PromotionQuestion>,
    pub submission: PromotionDecisionSubmission,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecordedPromotionDecisions {
    pub protocol_version: u32,
    pub run_id: String,
    pub run_manifest_sha256: String,
    pub canonical_document_sha256: String,
    pub input_sha256: String,
    pub approved_by: String,
    pub questions: Vec<PromotionQuestion>,
    pub decisions: Vec<PromotionDecision>,
    #[serde(default)]
    pub resources: Vec<PromotionResourceSubmission>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionDocumentChange {
    pub target_relative_path: String,
    pub sha256: String,
    pub byte_length: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionRegistrationChange {
    pub target_relative_path: String,
    pub template_version: u32,
    pub document_id: String,
    pub source_root: String,
    pub source_relative_path: String,
    pub owner: String,
    pub route: String,
    pub panel: String,
    pub layer: String,
    pub page_state: String,
    pub audit_profiles: Vec<String>,
    pub i18n_keys: Vec<String>,
    pub theme_tokens: Vec<String>,
    pub action_or_binding_registration: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionResourceChange {
    pub strategy_id: String,
    pub asset_id: String,
    pub source_relative_path: String,
    pub source_sha256: String,
    pub byte_length: u64,
    pub width: u32,
    pub height: u32,
    pub alpha: AlphaMode,
    pub target_asset_path: String,
    pub target_relative_path: String,
    pub catalog_relative_path: String,
    pub license_record_relative_path: String,
    pub license_status: PromotionResourceLicenseStatus,
    pub license_reference: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionPlan {
    pub protocol_version: u32,
    pub run_id: String,
    pub run_manifest_sha256: String,
    pub canonical_document_sha256: String,
    pub input_sha256: String,
    pub approved_by: String,
    pub document: PromotionDocumentChange,
    pub registration: PromotionRegistrationChange,
    pub resources: Vec<PromotionResourceChange>,
    pub plan_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PromotionResult {
    pub plan_sha256: String,
    pub approved_directory: PathBuf,
    pub document_path: PathBuf,
    pub registration_path: PathBuf,
    pub resource_paths: Vec<PathBuf>,
}

#[derive(Clone, Debug)]
struct TrustedRun {
    repository_root: PathBuf,
    run_root: PathBuf,
    manifest_sha256: String,
    document_json: String,
    document_id: String,
    canonical_document_sha256: String,
    input_sha256: String,
    analysis: Value,
    asset_strategy: Value,
    repair_run: Value,
    draft_assets: Vec<ArtifactLink>,
}

/// Creates the bounded decision request. It only reads a sealed bundle and never writes files.
pub fn create_promotion_decision_template(
    repository_root: &Path,
    run_id: &str,
) -> Result<PromotionDecisionTemplate, TaskFailure> {
    let trusted = load_trusted_run(repository_root, run_id)?;
    let questions = derive_questions(&trusted)?;
    Ok(PromotionDecisionTemplate {
        protocol_version: PROMOTION_PROTOCOL_VERSION,
        run_id: run_id.to_owned(),
        run_manifest_sha256: trusted.manifest_sha256.clone(),
        canonical_document_sha256: trusted.canonical_document_sha256.clone(),
        input_sha256: trusted.input_sha256.clone(),
        questions: questions.clone(),
        submission: PromotionDecisionSubmission {
            protocol_version: PROMOTION_PROTOCOL_VERSION,
            run_id: run_id.to_owned(),
            run_manifest_sha256: trusted.manifest_sha256,
            canonical_document_sha256: trusted.canonical_document_sha256,
            input_sha256: trusted.input_sha256,
            approved_by: String::new(),
            decisions: questions
                .iter()
                .map(|question| PromotionDecision {
                    question_id: question.question_id.clone(),
                    candidate_id: String::new(),
                    resolution: PromotionResolution::Reject,
                    rationale: String::new(),
                })
                .collect(),
            resources: Vec::new(),
        },
    })
}

/// Stores a fully explicit decision record beneath the already committed run. This does not write
/// formal game files and cannot replace a prior record.
pub fn record_promotion_decisions(
    repository_root: &Path,
    submission_path: &Path,
) -> Result<RecordedPromotionDecisions, TaskFailure> {
    let bytes = read_regular_file(
        submission_path,
        MAX_PROMOTION_FILE_BYTES,
        "decision submission",
    )?;
    let submission: PromotionDecisionSubmission =
        serde_json::from_slice(&bytes).map_err(|error| {
            invalid(format!(
                "promotion decision submission is invalid JSON: {error}"
            ))
        })?;
    let trusted = load_trusted_run(repository_root, &submission.run_id)?;
    let questions = derive_questions(&trusted)?;
    validate_submission(&submission, &trusted, &questions)?;

    let record = RecordedPromotionDecisions {
        protocol_version: PROMOTION_PROTOCOL_VERSION,
        run_id: submission.run_id,
        run_manifest_sha256: submission.run_manifest_sha256,
        canonical_document_sha256: submission.canonical_document_sha256,
        input_sha256: submission.input_sha256,
        approved_by: submission.approved_by,
        questions,
        decisions: submission.decisions,
        resources: submission.resources,
    };
    let bytes = pretty_json_bytes(&record)?;
    let decision_path = trusted.run_root.join(PROMOTION_DECISION_MANIFEST);
    let marker_path = trusted.run_root.join(PROMOTION_DECISION_MARKER);
    if decision_path.exists() || marker_path.exists() {
        return Err(conflict(
            "promotion decisions are append-only and already exist for this run",
        ));
    }
    write_new_synced(&decision_path, &bytes)?;
    let marker = format!("decision_sha256={}\n", hash_bytes(&bytes));
    if let Err(error) = write_new_synced(&marker_path, marker.as_bytes()) {
        let _ = fs::remove_file(&decision_path);
        return Err(error);
    }
    Ok(record)
}

/// Produces a no-write plan. `promote` rebuilds this exact value before committing anything.
pub fn create_promotion_plan(
    repository_root: &Path,
    run_id: &str,
    owner: &str,
    route: &str,
) -> Result<PromotionPlan, TaskFailure> {
    let trusted = load_trusted_run(repository_root, run_id)?;
    validate_owner_and_route(owner, route)?;
    let decisions = load_recorded_decisions(&trusted)?;
    validate_recorded_decisions(&decisions, &trusted)?;
    ensure_promotable_resolutions(&decisions)?;

    let folder = approved_folder_name(&trusted.document_id)?;
    let document_relative = format!("{folder}/document.v1.json");
    let registration_relative = format!("{folder}/promotion.v1.json");
    let approved_root = trusted
        .repository_root
        .join("project/assets/ui/documents/approved");
    ensure_owned_target_is_available(
        &trusted.repository_root,
        &approved_root,
        &folder,
        &trusted.document_id,
        owner,
        route,
    )?;
    let resources = validate_document_assets(&trusted, &decisions.resources, &folder)?;
    ensure_resource_target_is_available(&trusted.repository_root, &folder, &resources)?;

    let document = PromotionDocumentChange {
        target_relative_path: document_relative.clone(),
        sha256: trusted.canonical_document_sha256.clone(),
        byte_length: trusted.document_json.len() as u64,
    };
    let registration = PromotionRegistrationChange {
        target_relative_path: registration_relative,
        template_version: 1,
        document_id: trusted.document_id.clone(),
        source_root: "approved".to_owned(),
        source_relative_path: document_relative,
        owner: owner.to_owned(),
        route: route.to_owned(),
        panel: "page".to_owned(),
        layer: "page".to_owned(),
        page_state: "initial".to_owned(),
        audit_profiles: vec![
            "phone-small".to_owned(),
            "phone-portrait".to_owned(),
            "tablet-portrait".to_owned(),
            "tablet-landscape".to_owned(),
        ],
        // Stage 7 refuses generated i18n/action/binding fields. Page-local JSON tokens stay in
        // the document and are not silently promoted into the global theme.
        i18n_keys: Vec::new(),
        theme_tokens: Vec::new(),
        action_or_binding_registration: Vec::new(),
    };
    let material = PromotionPlanMaterial {
        protocol_version: PROMOTION_PROTOCOL_VERSION,
        run_id: run_id.to_owned(),
        run_manifest_sha256: trusted.manifest_sha256.clone(),
        canonical_document_sha256: trusted.canonical_document_sha256.clone(),
        input_sha256: trusted.input_sha256.clone(),
        approved_by: decisions.approved_by.clone(),
        document: document.clone(),
        registration: registration.clone(),
        resources: resources.clone(),
    };
    let plan_sha256 = hash_json(&material)?;
    Ok(PromotionPlan {
        protocol_version: material.protocol_version,
        run_id: material.run_id,
        run_manifest_sha256: material.run_manifest_sha256,
        canonical_document_sha256: material.canonical_document_sha256,
        input_sha256: material.input_sha256,
        approved_by: material.approved_by,
        document,
        registration,
        resources,
        plan_sha256,
    })
}

/// Commits exactly one new approved page directory. The caller must repeat the no-write plan hash.
pub fn promote(
    repository_root: &Path,
    run_id: &str,
    owner: &str,
    route: &str,
    confirm_plan: &str,
) -> Result<PromotionResult, TaskFailure> {
    let plan = create_promotion_plan(repository_root, run_id, owner, route)?;
    if !is_sha256(confirm_plan) || confirm_plan != plan.plan_sha256 {
        return Err(invalid(
            "promote requires the exact plan_sha256 from the current promotion-plan output",
        ));
    }
    let trusted = load_trusted_run(repository_root, run_id)?;
    let approved_root = trusted
        .repository_root
        .join("project/assets/ui/documents/approved");
    let folder = approved_folder_name(&trusted.document_id)?;
    ensure_owned_target_is_available(
        &trusted.repository_root,
        &approved_root,
        &folder,
        &trusted.document_id,
        owner,
        route,
    )?;
    ensure_resource_target_is_available(&trusted.repository_root, &folder, &plan.resources)?;

    let staging_root = approved_root.join(format!(
        ".promotion-staging-{}-{}",
        std::process::id(),
        STAGING_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let staging_page = staging_root.join(&folder);
    let final_page = approved_root.join(&folder);
    let result = (|| {
        fs::create_dir(&staging_root)
            .map_err(|error| write_failure(&staging_root, "create promotion staging", error))?;
        fs::create_dir(&staging_page)
            .map_err(|error| write_failure(&staging_page, "create staged page directory", error))?;
        let document_path = staging_page.join("document.v1.json");
        let registration_path = staging_page.join("promotion.v1.json");
        write_new_synced(&document_path, trusted.document_json.as_bytes())?;
        let registration = registration_json(&plan.registration)?;
        write_new_synced(&registration_path, &registration)?;
        if !plan.resources.is_empty() {
            let resource_directory = staging_page.join("assets");
            fs::create_dir(&resource_directory).map_err(|error| {
                write_failure(
                    &resource_directory,
                    "create staged resource directory",
                    error,
                )
            })?;
            for resource in &plan.resources {
                let source = trusted.run_root.join(&resource.source_relative_path);
                let source_bytes = read_regular_file(
                    &source,
                    MAX_PROMOTION_RESOURCE_BYTES,
                    "approved resource source",
                )?;
                if source_bytes.len() as u64 != resource.byte_length
                    || hash_bytes(&source_bytes) != resource.source_sha256
                {
                    return Err(invalid(
                        "approved resource source changed after promotion planning",
                    ));
                }
                let target = resource_directory.join(&resource.target_relative_path);
                write_new_synced_bounded(&target, &source_bytes, MAX_PROMOTION_RESOURCE_BYTES)?;
            }
            let catalog = resource_catalog_json(&folder, &plan.resources)?;
            write_new_synced(&staging_page.join("catalog.v1.json"), &catalog)?;
            let licenses = resource_license_records(&folder, &plan.resources)?;
            write_new_synced(&staging_page.join("LICENSES.md"), &licenses)?;
        }
        // Re-read staging evidence before a same-filesystem directory rename.
        if hash_bytes(&read_regular_file(
            &document_path,
            MAX_PROMOTION_FILE_BYTES,
            "staged approved document",
        )?) != plan.document.sha256
        {
            return Err(invalid(
                "staged approved document hash changed before commit",
            ));
        }
        let expected_registration = hash_bytes(&registration);
        if hash_bytes(&read_regular_file(
            &registration_path,
            MAX_PROMOTION_FILE_BYTES,
            "staged promotion registration",
        )?) != expected_registration
        {
            return Err(invalid(
                "staged promotion registration hash changed before commit",
            ));
        }
        if final_page.exists() {
            return Err(conflict(
                "approved target appeared while promotion was staging",
            ));
        }
        fs::rename(&staging_page, &final_page).map_err(|error| {
            write_failure(&final_page, "atomically commit approved page", error)
        })?;
        let _ = fs::remove_dir(&staging_root);
        Ok(PromotionResult {
            plan_sha256: plan.plan_sha256,
            approved_directory: final_page.clone(),
            document_path: final_page.join("document.v1.json"),
            registration_path: final_page.join("promotion.v1.json"),
            resource_paths: plan
                .resources
                .iter()
                .map(|resource| {
                    final_page
                        .join("assets")
                        .join(&resource.target_relative_path)
                })
                .collect(),
        })
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging_root);
    }
    result
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct PromotionPlanMaterial {
    protocol_version: u32,
    run_id: String,
    run_manifest_sha256: String,
    canonical_document_sha256: String,
    input_sha256: String,
    approved_by: String,
    document: PromotionDocumentChange,
    registration: PromotionRegistrationChange,
    resources: Vec<PromotionResourceChange>,
}

fn load_trusted_run(repository_root: &Path, run_id: &str) -> Result<TrustedRun, TaskFailure> {
    validate_run_id(run_id)?;
    let repository_root =
        canonical_regular_directory(repository_root, "promotion repository root")?;
    let generation_root = repository_root.join("summary/ui-generation");
    let generation_root =
        canonical_regular_directory(&generation_root, "promotion generation root")?;
    if !generation_root.starts_with(&repository_root) {
        return Err(invalid("promotion generation root escapes repository"));
    }
    let run_root = generation_root.join(run_id);
    let run_root = canonical_regular_directory(&run_root, "committed promotion run root")?;
    if run_root.parent() != Some(generation_root.as_path()) {
        return Err(invalid(
            "promotion run is not a direct child of summary/ui-generation",
        ));
    }
    let bundle_root =
        canonical_regular_directory(&run_root.join("bundle"), "committed run bundle")?;
    if !bundle_root.starts_with(&run_root) {
        return Err(invalid("committed run bundle escapes its run root"));
    }
    let manifest_path = bundle_root.join("manifest.json");
    let manifest_bytes =
        read_regular_file(&manifest_path, MAX_PROMOTION_FILE_BYTES, "run manifest")?;
    let manifest_sha256 = hash_bytes(&manifest_bytes);
    let marker = read_regular_file(&run_root.join("COMMITTED"), 256, "run committed marker")?;
    if marker != format!("manifest_sha256={manifest_sha256}\n").as_bytes() {
        return Err(invalid(
            "run committed marker does not bind the current manifest hash",
        ));
    }
    let manifest: UiGenerationRunManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| invalid(format!("committed run manifest is invalid: {error}")))?;
    if manifest.protocol_version != RUN_MANIFEST_PROTOCOL_VERSION
        || manifest.run_id != run_id
        || manifest.status != RunBundleStatus::Passed
        || !matches!(
            manifest.repair_status,
            crate::repair::RepairRunStatus::Passed
        )
        || !matches!(
            manifest.preview_status,
            crate::preview::PreviewRunStatus::Passed
        )
    {
        return Err(invalid(
            "promotion requires a passed committed bundle for the requested run",
        ));
    }
    verify_stage_links(&run_root, &manifest)?;
    verify_bundle_artifacts(&bundle_root, &manifest.artifacts)?;
    let final_document = required_bundle_artifact(&manifest.artifacts, "final_document")?;
    let trace = required_bundle_artifact(&manifest.artifacts, "generation_trace")?;
    let validation = required_bundle_artifact(&manifest.artifacts, "validation_report")?;
    let repair_run = required_bundle_artifact(&manifest.artifacts, "repair_run")?;
    let source_map = required_bundle_artifact(&manifest.artifacts, "source_map")?;
    let document_bytes = read_bundle_artifact(&bundle_root, final_document)?;
    let document_json = String::from_utf8(document_bytes)
        .map_err(|_| invalid("committed final document is not UTF-8"))?;
    let canonical = canonicalize_json(&document_json).map_err(|error| {
        invalid(format!(
            "committed final document no longer validates: {error}"
        ))
    })?;
    if canonical != document_json {
        return Err(invalid(
            "committed final document differs from the formal canonical UiDocument form",
        ));
    }
    let validation_result = validate_json(&document_json);
    let validated = validation_result.validated().ok_or_else(|| {
        invalid("committed final document fails formal UiDocument validation during promotion")
    })?;
    let document = validated.document();
    if document.schema_version != CURRENT_SCHEMA_VERSION {
        return Err(invalid(
            "promotion document schema version is not the current formal runtime version",
        ));
    }
    reject_business_fields(&serde_json::from_str::<Value>(&document_json).map_err(|_| {
        invalid("committed final document cannot be decoded for promotion policy validation")
    })?)?;
    let canonical_document_sha256 = hash_bytes(document_json.as_bytes());
    let trace_value: Value = serde_json::from_slice(&read_bundle_artifact(&bundle_root, trace)?)
        .map_err(|_| invalid("committed generation trace is not JSON"))?;
    let input_sha256 = required_hash_field(&trace_value, "input_sha256", "generation trace")?;
    if required_hash_field(
        &trace_value,
        "canonical_document_sha256",
        "generation trace",
    )? != canonical_document_sha256
    {
        return Err(invalid(
            "generation trace document hash differs from final document",
        ));
    }
    let validation_value: Value =
        serde_json::from_slice(&read_bundle_artifact(&bundle_root, validation)?)
            .map_err(|_| invalid("committed validation report is not JSON"))?;
    if validation_value.get("valid").and_then(Value::as_bool) != Some(true) {
        return Err(invalid(
            "promotion requires a passed formal validation report",
        ));
    }
    let source_map_value: Value =
        serde_json::from_slice(&read_bundle_artifact(&bundle_root, source_map)?)
            .map_err(|_| invalid("committed source map is not JSON"))?;
    if source_map_value.as_array().is_none_or(Vec::is_empty) {
        return Err(invalid("promotion requires non-empty source-map evidence"));
    }
    let repair_run_value: Value =
        serde_json::from_slice(&read_bundle_artifact(&bundle_root, repair_run)?)
            .map_err(|_| invalid("committed repair run is not JSON"))?;
    let repaired = repair_run_value
        .pointer("/final_document/canonical_document_json")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("passed repair run lacks final canonical document evidence"))?;
    if repaired != document_json {
        return Err(invalid(
            "repair run final document differs from bundle final document",
        ));
    }
    let analysis = read_stage_json(
        &run_root,
        &manifest.stage_evidence.reference_analysis,
        "analysis",
    )?;
    let asset_strategy = read_stage_json(
        &run_root,
        &manifest.stage_evidence.asset_strategy,
        "asset strategy",
    )?;
    Ok(TrustedRun {
        repository_root,
        run_root,
        manifest_sha256,
        document_json,
        document_id: document.document_id.as_str().to_owned(),
        canonical_document_sha256,
        input_sha256,
        analysis,
        asset_strategy,
        repair_run: repair_run_value,
        draft_assets: manifest.stage_evidence.draft_assets.clone(),
    })
}

fn derive_questions(trusted: &TrustedRun) -> Result<Vec<PromotionQuestion>, TaskFailure> {
    let mut questions = BTreeMap::new();
    insert_question(
        &mut questions,
        "release.approval",
        PromotionDecisionKind::ReleaseApproval,
        PromotionReferenceRegion {
            reference_id: None,
            region_id: None,
            element_id: None,
        },
        "Approve this exact validated document and promotion plan for formal review.",
    )?;
    if let Some(uncertainties) = trusted
        .analysis
        .get("uncertainties")
        .and_then(Value::as_array)
    {
        for uncertainty in uncertainties {
            let kind = uncertainty
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let question_kind = match kind {
                "ambiguous_layout" | "low_confidence" => Some(PromotionDecisionKind::CoreLayout),
                "hidden_interaction" => Some(PromotionDecisionKind::BusinessAction),
                _ => None,
            };
            let Some(question_kind) = question_kind else {
                continue;
            };
            let question_id = uncertainty
                .get("uncertainty_id")
                .and_then(Value::as_str)
                .map(|id| format!("analysis.{id}"))
                .ok_or_else(|| invalid("analysis uncertainty lacks a safe uncertainty_id"))?;
            let subject = uncertainty.get("subject").and_then(Value::as_object);
            let region = PromotionReferenceRegion {
                reference_id: subject
                    .and_then(|value| value.get("reference_id"))
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                region_id: subject
                    .and_then(|value| value.get("region_id"))
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                element_id: subject
                    .and_then(|value| value.get("element_id"))
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            };
            let prompt = uncertainty
                .get("follow_up_question")
                .and_then(Value::as_str)
                .unwrap_or("Human confirmation is required before promotion.");
            insert_question(&mut questions, &question_id, question_kind, region, prompt)?;
        }
    }
    if let Some(entries) = trusted
        .asset_strategy
        .get("entries")
        .and_then(Value::as_array)
    {
        for entry in entries {
            let disposition = entry
                .get("disposition")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let status = entry
                .get("approval_status")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if matches!(disposition, "authorized_crop" | "recreate" | "generate")
                && status != "approved"
            {
                let element = entry
                    .get("element_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| invalid("asset strategy entry lacks an element ID"))?;
                insert_question(
                    &mut questions,
                    &format!("asset.{element}"),
                    PromotionDecisionKind::AssetLicense,
                    PromotionReferenceRegion {
                        reference_id: None,
                        region_id: None,
                        element_id: Some(element.to_owned()),
                    },
                    "Confirm the asset license and whether this page must retain a placeholder or use a replacement.",
                )?;
            }
        }
    }
    for (field, kind) in [
        (
            "unimplemented_states",
            PromotionDecisionKind::BusinessAction,
        ),
        (
            "unsupported_capabilities",
            PromotionDecisionKind::FrameworkCapability,
        ),
        (
            "required_new_components",
            PromotionDecisionKind::FrameworkCapability,
        ),
    ] {
        if let Some(items) = trusted
            .repair_run
            .pointer(&format!("/final_document/disclosures/{field}"))
            .and_then(Value::as_array)
        {
            for item in items {
                let code = item
                    .get("code")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                let subject = item
                    .get("subject_id")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                insert_question(
                    &mut questions,
                    &format!("disclosure.{field}.{code}"),
                    kind,
                    PromotionReferenceRegion {
                        reference_id: None,
                        region_id: None,
                        element_id: subject,
                    },
                    "Resolve this generation disclosure without inventing runtime business behavior.",
                )?;
            }
        }
    }
    if questions.len() > MAX_DECISIONS {
        return Err(invalid(
            "promotion contains more than the closed high-impact decision budget",
        ));
    }
    Ok(questions.into_values().collect())
}

fn insert_question(
    questions: &mut BTreeMap<String, PromotionQuestion>,
    question_id: &str,
    kind: PromotionDecisionKind,
    reference_region: PromotionReferenceRegion,
    prompt: &str,
) -> Result<(), TaskFailure> {
    if !safe_label(question_id) || !bounded_text(prompt, MAX_TEXT_BYTES) {
        return Err(invalid("promotion question has an unsafe ID or prompt"));
    }
    let candidates = candidates_for(kind);
    if questions
        .insert(
            question_id.to_owned(),
            PromotionQuestion {
                question_id: question_id.to_owned(),
                kind,
                reference_region,
                prompt: prompt.to_owned(),
                candidates,
            },
        )
        .is_some()
    {
        return Err(invalid("promotion question IDs must be unique"));
    }
    Ok(())
}

fn candidates_for(kind: PromotionDecisionKind) -> Vec<PromotionDecisionCandidate> {
    let mut candidates = vec![candidate(
        "accept_current",
        PromotionResolution::Accept,
        "Accept the current validated result",
        "Promotes only this document hash; later changes require a new decision record.",
    )];
    match kind {
        PromotionDecisionKind::AssetLicense => candidates.extend([
            candidate(
                "replace_asset",
                PromotionResolution::ReplaceAsset,
                "Replace with an already licensed asset",
                "Requires regeneration or a new verified run before promotion.",
            ),
            candidate(
                "keep_placeholder",
                PromotionResolution::KeepPlaceholder,
                "Keep a placeholder",
                "Promotes no unlicensed pixels or binary assets.",
            ),
        ]),
        PromotionDecisionKind::CoreLayout => candidates.extend([
            candidate(
                "modify_constraint",
                PromotionResolution::ModifyConstraint,
                "Modify layout constraints",
                "Requires regeneration because the document hash changes.",
            ),
            candidate(
                "modify_text",
                PromotionResolution::ModifyText,
                "Modify visible text",
                "Requires regeneration because the document hash changes.",
            ),
        ]),
        PromotionDecisionKind::BusinessAction | PromotionDecisionKind::FrameworkCapability => {
            candidates.push(candidate(
                "keep_placeholder",
                PromotionResolution::KeepPlaceholder,
                "Keep the declarative placeholder",
                "Does not create an unknown action, binding, Rust system, or framework capability.",
            ));
        }
        PromotionDecisionKind::ReleaseApproval => {}
    }
    candidates.push(candidate(
        "reject_promotion",
        PromotionResolution::Reject,
        "Reject promotion",
        "Keeps the run isolated and writes no formal game files.",
    ));
    candidates
}

fn candidate(
    candidate_id: &str,
    resolution: PromotionResolution,
    summary: &str,
    impact: &str,
) -> PromotionDecisionCandidate {
    PromotionDecisionCandidate {
        candidate_id: candidate_id.to_owned(),
        resolution,
        summary: summary.to_owned(),
        impact: impact.to_owned(),
    }
}

fn validate_submission(
    submission: &PromotionDecisionSubmission,
    trusted: &TrustedRun,
    questions: &[PromotionQuestion],
) -> Result<(), TaskFailure> {
    if submission.protocol_version != PROMOTION_PROTOCOL_VERSION
        || submission.run_id
            != trusted
                .run_root
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
        || submission.run_manifest_sha256 != trusted.manifest_sha256
        || submission.canonical_document_sha256 != trusted.canonical_document_sha256
        || submission.input_sha256 != trusted.input_sha256
        || !safe_label(&submission.approved_by)
        || submission.decisions.len() != questions.len()
    {
        return Err(invalid(
            "promotion decision submission is not bound to this committed run/document/input or is incomplete",
        ));
    }
    let question_by_id: BTreeMap<_, _> = questions
        .iter()
        .map(|question| (question.question_id.as_str(), question))
        .collect();
    let mut seen = BTreeSet::new();
    for decision in &submission.decisions {
        let question = question_by_id
            .get(decision.question_id.as_str())
            .ok_or_else(|| invalid("promotion decision names an unknown question"))?;
        if !seen.insert(decision.question_id.as_str())
            || !bounded_text(&decision.rationale, MAX_TEXT_BYTES)
            || !question.candidates.iter().any(|candidate| {
                candidate.candidate_id == decision.candidate_id
                    && candidate.resolution == decision.resolution
            })
        {
            return Err(invalid(
                "promotion decision is duplicate, lacks a rationale, or selects an invalid candidate",
            ));
        }
    }
    Ok(())
}

fn load_recorded_decisions(
    trusted: &TrustedRun,
) -> Result<RecordedPromotionDecisions, TaskFailure> {
    let path = trusted.run_root.join(PROMOTION_DECISION_MANIFEST);
    let bytes = read_regular_file(
        &path,
        MAX_PROMOTION_FILE_BYTES,
        "recorded promotion decisions",
    )?;
    let marker = read_regular_file(
        &trusted.run_root.join(PROMOTION_DECISION_MARKER),
        256,
        "promotion decision marker",
    )?;
    if marker != format!("decision_sha256={}\n", hash_bytes(&bytes)).as_bytes() {
        return Err(invalid(
            "promotion decision marker does not bind decision manifest hash",
        ));
    }
    serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("recorded promotion decisions are invalid: {error}")))
}

fn validate_recorded_decisions(
    record: &RecordedPromotionDecisions,
    trusted: &TrustedRun,
) -> Result<(), TaskFailure> {
    let submission = PromotionDecisionSubmission {
        protocol_version: record.protocol_version,
        run_id: record.run_id.clone(),
        run_manifest_sha256: record.run_manifest_sha256.clone(),
        canonical_document_sha256: record.canonical_document_sha256.clone(),
        input_sha256: record.input_sha256.clone(),
        approved_by: record.approved_by.clone(),
        decisions: record.decisions.clone(),
        resources: record.resources.clone(),
    };
    let expected = derive_questions(trusted)?;
    if record.questions != expected {
        return Err(invalid(
            "recorded promotion questions differ from current trusted run evidence",
        ));
    }
    validate_submission(&submission, trusted, &expected)
}

fn ensure_promotable_resolutions(record: &RecordedPromotionDecisions) -> Result<(), TaskFailure> {
    for question in &record.questions {
        let decision = record
            .decisions
            .iter()
            .find(|decision| decision.question_id == question.question_id)
            .expect("validated decision record contains every question");
        if decision.resolution == PromotionResolution::Reject {
            return Err(invalid(
                "a human rejected promotion for a required decision",
            ));
        }
        if matches!(
            question.kind,
            PromotionDecisionKind::BusinessAction | PromotionDecisionKind::FrameworkCapability
        ) && decision.resolution != PromotionResolution::KeepPlaceholder
        {
            return Err(invalid(
                "unknown business actions/bindings or framework capabilities must remain placeholders before promotion",
            ));
        }
        if question.kind == PromotionDecisionKind::AssetLicense
            && !record.resources.is_empty()
            && decision.resolution != PromotionResolution::Accept
        {
            return Err(invalid(
                "a promoted binary asset requires explicit acceptance of its sealed license evidence",
            ));
        }
        if matches!(
            decision.resolution,
            PromotionResolution::ReplaceAsset
                | PromotionResolution::ModifyText
                | PromotionResolution::ModifyConstraint
        ) {
            return Err(invalid(
                "selected decision changes require a newly generated and separately approved document hash",
            ));
        }
    }
    Ok(())
}

fn validate_document_assets(
    trusted: &TrustedRun,
    submissions: &[PromotionResourceSubmission],
    folder: &str,
) -> Result<Vec<PromotionResourceChange>, TaskFailure> {
    let document: Value = serde_json::from_str(&trusted.document_json).map_err(|_| {
        invalid("validated document could not be decoded for asset promotion checks")
    })?;
    let paths = packaged_asset_paths(&document);
    let resources = validate_resource_submissions(trusted, submissions, folder, &paths)?;
    let promoted_prefix = format!("ui/documents/approved/{folder}/assets/");
    let catalog = if paths.iter().any(|path| !path.starts_with(&promoted_prefix)) {
        Some(AssetCatalog::load_repository(&trusted.repository_root)?)
    } else {
        None
    };
    for path in paths {
        if path.starts_with(&promoted_prefix) {
            if !resources
                .iter()
                .any(|resource| resource.target_asset_path == path)
            {
                return Err(invalid(
                    "document references a generated asset that is absent from the explicit promotion resource manifest",
                ));
            }
            continue;
        }
        let asset = catalog
            .as_ref()
            .and_then(|catalog| catalog.resolve_by_path(&path))
            .ok_or_else(|| {
            invalid(format!(
                "approved document references packaged asset `{path}` that is absent from the verified catalog"
            ))
        })?;
        if !matches!(
            asset.license.status,
            CatalogLicenseStatus::ProjectOwned | CatalogLicenseStatus::Redistributable
        ) {
            return Err(invalid(format!(
                "approved document references packaged asset `{path}` without a confirmed redistributable license"
            )));
        }
    }
    Ok(resources)
}

fn validate_resource_submissions(
    trusted: &TrustedRun,
    submissions: &[PromotionResourceSubmission],
    folder: &str,
    document_paths: &BTreeSet<String>,
) -> Result<Vec<PromotionResourceChange>, TaskFailure> {
    if submissions.len() > 32 {
        return Err(invalid(
            "promotion resource count exceeds the closed budget",
        ));
    }
    let linked: BTreeMap<_, _> = trusted
        .draft_assets
        .iter()
        .map(|link| (link.relative_path.as_str(), link))
        .collect();
    let entries = trusted
        .asset_strategy
        .get("entries")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("committed asset strategy lacks an entries array"))?;
    let mut seen_ids = BTreeSet::new();
    let mut seen_targets = BTreeSet::new();
    let mut changes = Vec::new();
    for submission in submissions {
        if !safe_asset_id(&submission.asset_id)
            || !safe_label(&submission.strategy_id)
            || !safe_relative_path(&submission.source_relative_path)
            || !is_sha256(&submission.source_sha256)
            || submission.byte_length == 0
            || !safe_resource_file_name(&submission.target_file_name)
            || !bounded_text(&submission.license_reference, 512)
            || !seen_ids.insert(submission.asset_id.as_str())
        {
            return Err(invalid(
                "promotion resource contains unsafe, duplicate, or incomplete metadata",
            ));
        }
        if !submission
            .asset_id
            .starts_with(&format!("ui.generated.{folder}."))
        {
            return Err(invalid(
                "promotion resource asset IDs must be namespaced to the promoted document folder",
            ));
        }
        let source_link = linked
            .get(submission.source_relative_path.as_str())
            .ok_or_else(|| invalid("promotion resource source is not a sealed draft asset link"))?;
        if source_link.sha256 != submission.source_sha256
            || source_link.byte_length != submission.byte_length
        {
            return Err(invalid(
                "promotion resource source hash differs from sealed draft evidence",
            ));
        }
        let source = trusted.run_root.join(&submission.source_relative_path);
        let bytes = read_regular_file(
            &source,
            MAX_PROMOTION_RESOURCE_BYTES,
            "promotion resource source",
        )?;
        if hash_bytes(&bytes) != submission.source_sha256
            || bytes.len() as u64 != submission.byte_length
        {
            return Err(invalid(
                "promotion resource source changed since the sealed run",
            ));
        }
        let entry = entries
            .iter()
            .find(|entry| {
                entry.get("strategy_id").and_then(Value::as_str) == Some(&submission.strategy_id)
            })
            .ok_or_else(|| {
                invalid("promotion resource strategy ID is absent from the sealed asset strategy")
            })?;
        let specification: AssetSpecification =
            serde_json::from_value(entry.get("specification").cloned().ok_or_else(|| {
                invalid("promotion resource strategy has no raster specification")
            })?)
            .map_err(|_| invalid("promotion resource strategy specification is invalid"))?;
        let quality = inspect_asset_file(&source, &specification)?;
        if quality.verdict == AssetQualityVerdict::Reject {
            return Err(invalid(
                "promotion resource failed Android asset quality checks",
            ));
        }
        validate_resource_license(entry, submission)?;
        let target_asset_path = format!(
            "ui/documents/approved/{folder}/assets/{}",
            submission.target_file_name
        );
        if !document_paths.contains(&target_asset_path)
            || !seen_targets.insert(target_asset_path.clone())
        {
            return Err(invalid(
                "each promotion resource must be referenced exactly once by the sealed document packaged path",
            ));
        }
        require_lfs_coverage(&trusted.repository_root, &submission.target_file_name)?;
        changes.push(PromotionResourceChange {
            strategy_id: submission.strategy_id.clone(),
            asset_id: submission.asset_id.clone(),
            source_relative_path: submission.source_relative_path.clone(),
            source_sha256: submission.source_sha256.clone(),
            byte_length: submission.byte_length,
            width: specification.width,
            height: specification.height,
            alpha: specification.alpha,
            target_asset_path: target_asset_path.clone(),
            target_relative_path: submission.target_file_name.clone(),
            catalog_relative_path: format!("documents/approved/{folder}/catalog.v1.json"),
            license_record_relative_path: format!("documents/approved/{folder}/LICENSES.md"),
            license_status: submission.license_status,
            license_reference: submission.license_reference.clone(),
        });
    }
    changes.sort_by(|left, right| left.asset_id.cmp(&right.asset_id));
    Ok(changes)
}

fn validate_resource_license(
    entry: &Value,
    submission: &PromotionResourceSubmission,
) -> Result<(), TaskFailure> {
    let disposition = entry
        .get("disposition")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let (recorded_status, recorded_reference) = match disposition {
        "authorized_crop" => {
            if entry.pointer("/crop/authorization").and_then(Value::as_str)
                != Some("derivatives_allowed")
            {
                return Err(invalid(
                    "authorized crop resource lacks derivative authorization",
                ));
            }
            (
                Some(PromotionResourceLicenseStatus::Redistributable),
                entry
                    .pointer("/crop/license_reference")
                    .and_then(Value::as_str),
            )
        }
        "generate" => {
            let status = match entry
                .pointer("/generation/license/status")
                .and_then(Value::as_str)
            {
                Some("project_owned") => PromotionResourceLicenseStatus::ProjectOwned,
                Some("redistributable") => PromotionResourceLicenseStatus::Redistributable,
                _ => {
                    return Err(invalid(
                        "generated resource lacks a confirmed redistributable license",
                    ));
                }
            };
            (
                Some(status),
                entry
                    .pointer("/generation/license/reference")
                    .and_then(Value::as_str),
            )
        }
        _ => {
            return Err(invalid(
                "only authorized crop or explicitly licensed generated assets may enter formal resources",
            ));
        }
    };
    if recorded_status != Some(submission.license_status)
        || recorded_reference != Some(submission.license_reference.as_str())
    {
        return Err(invalid(
            "promotion resource license differs from sealed strategy provenance",
        ));
    }
    Ok(())
}

fn ensure_resource_target_is_available(
    repository_root: &Path,
    folder: &str,
    resources: &[PromotionResourceChange],
) -> Result<(), TaskFailure> {
    if resources.is_empty() {
        return Ok(());
    }
    let target = repository_root
        .join("project/assets/ui/documents/approved")
        .join(folder);
    if fs::symlink_metadata(&target).is_ok() {
        return Err(conflict(
            "approved resource promotion directory already exists and may not be overwritten",
        ));
    }
    Ok(())
}

fn require_lfs_coverage(repository_root: &Path, file_name: &str) -> Result<(), TaskFailure> {
    let extension = Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())
        .ok_or_else(|| invalid("promotion resource has no file extension"))?;
    let attributes = read_regular_file(
        &repository_root.join(".gitattributes"),
        MAX_PROMOTION_FILE_BYTES,
        "Git LFS attributes",
    )?;
    let expected_prefix = format!("project/assets/**/*.{}", extension.to_ascii_lowercase());
    let covered = std::str::from_utf8(&attributes)
        .ok()
        .is_some_and(|content| {
            content.lines().any(|line| {
                let line = line.trim();
                line.starts_with(&expected_prefix)
                    && line.split_whitespace().any(|item| item == "filter=lfs")
            })
        });
    if !covered {
        return Err(invalid(
            "promotion resource extension is not covered by an explicit project/assets Git LFS rule",
        ));
    }
    Ok(())
}

fn resource_catalog_json(
    folder: &str,
    resources: &[PromotionResourceChange],
) -> Result<Vec<u8>, TaskFailure> {
    let assets: Vec<_> = resources
        .iter()
        .map(|resource| {
            serde_json::json!({
                "asset_id": resource.asset_id,
                "path": resource.target_asset_path,
                "kind": "raster",
                "sha256": resource.source_sha256,
                "byte_length": resource.byte_length,
                "width": resource.width,
                "height": resource.height,
                "alpha": match resource.alpha {
                    AlphaMode::Opaque => "opaque",
                    AlphaMode::Straight => "straight",
                    AlphaMode::NotApplicable => "not_applicable",
                },
                "license": {
                    "status": match resource.license_status {
                        PromotionResourceLicenseStatus::ProjectOwned => "project_owned",
                        PromotionResourceLicenseStatus::Redistributable => "redistributable",
                    },
                    "reference": format!("ui/documents/approved/{folder}/LICENSES.md"),
                },
                "tags": ["generated", folder],
            })
        })
        .collect();
    pretty_json_bytes(&serde_json::json!({
        "schema_version": 1,
        "assets": assets,
    }))
}

fn resource_license_records(
    folder: &str,
    resources: &[PromotionResourceChange],
) -> Result<Vec<u8>, TaskFailure> {
    let mut text = format!("# Promoted UI Asset Licenses: {folder}\n\n");
    for resource in resources {
        let status = match resource.license_status {
            PromotionResourceLicenseStatus::ProjectOwned => "project_owned",
            PromotionResourceLicenseStatus::Redistributable => "redistributable",
        };
        text.push_str(&format!(
            "- asset_id: {}\n  source_sha256: {}\n  license_status: {}\n  source_license_reference: {}\n",
            resource.asset_id, resource.source_sha256, status, resource.license_reference
        ));
    }
    if text.is_empty() || text.len() > MAX_PROMOTION_FILE_BYTES || text.contains('\0') {
        return Err(invalid("promotion resource license record is invalid"));
    }
    Ok(text.into_bytes())
}

fn packaged_asset_paths(value: &Value) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    collect_packaged_asset_paths(value, &mut paths);
    paths
}

fn collect_packaged_asset_paths(value: &Value, paths: &mut BTreeSet<String>) {
    match value {
        Value::Object(object) => {
            if object.get("kind").and_then(Value::as_str) == Some("packaged")
                && let Some(path) = object.get("path").and_then(Value::as_str)
            {
                paths.insert(path.to_owned());
            }
            for child in object.values() {
                collect_packaged_asset_paths(child, paths);
            }
        }
        Value::Array(values) => {
            for child in values {
                collect_packaged_asset_paths(child, paths);
            }
        }
        _ => {}
    }
}

fn ensure_owned_target_is_available(
    repository_root: &Path,
    approved_root: &Path,
    folder: &str,
    document_id: &str,
    owner: &str,
    route: &str,
) -> Result<(), TaskFailure> {
    let approved_root = canonical_regular_directory(approved_root, "approved document root")?;
    let target = approved_root.join(folder);
    if fs::symlink_metadata(&target).is_ok() {
        return Err(conflict(
            "approved page directory already exists and may not be overwritten",
        ));
    }
    let mut ownership = discover_approved_ownership(&approved_root)?;
    ownership
        .owners
        .extend(discover_existing_game_owners(repository_root)?);
    if ownership.document_ids.contains(document_id) {
        return Err(conflict("document ID is already owned by an approved page"));
    }
    if ownership.owners.contains(owner) {
        return Err(conflict(
            "approved page owner is already owned by another registration",
        ));
    }
    if ownership.routes.contains(route) {
        return Err(conflict(
            "approved page route is already owned by another registration",
        ));
    }
    Ok(())
}

fn discover_existing_game_owners(repository_root: &Path) -> Result<BTreeSet<String>, TaskFailure> {
    let path = repository_root.join("project/src/game/ui_ids.rs");
    let bytes = match read_regular_file(&path, 512 * 1024, "game UI owner declarations") {
        Ok(bytes) => bytes,
        Err(_error)
            if matches!(
                fs::symlink_metadata(&path),
                Err(source) if source.kind() == std::io::ErrorKind::NotFound
            ) =>
        {
            return Ok(BTreeSet::new());
        }
        Err(error) => return Err(error),
    };
    let source = std::str::from_utf8(&bytes)
        .map_err(|_| invalid("game UI owner declarations are not UTF-8"))?;
    let mut owners = BTreeSet::new();
    for remainder in source.split("UiOwnerId::new(\"").skip(1) {
        let Some(owner) = remainder.split('\"').next() else {
            return Err(invalid("game UI owner declaration is unterminated"));
        };
        if !safe_label(owner) {
            return Err(invalid("game UI owner declaration is unsafe"));
        }
        owners.insert(owner.to_owned());
    }
    Ok(owners)
}

#[derive(Default)]
struct ApprovedOwnership {
    document_ids: BTreeSet<String>,
    owners: BTreeSet<String>,
    routes: BTreeSet<String>,
}

fn discover_approved_ownership(root: &Path) -> Result<ApprovedOwnership, TaskFailure> {
    let mut ownership = ApprovedOwnership::default();
    let mut pending = vec![(root.to_path_buf(), 0_usize)];
    let mut entries = 0_usize;
    while let Some((directory, depth)) = pending.pop() {
        if depth > 16 || entries > MAX_APPROVED_DOCUMENTS * 8 {
            return Err(invalid(
                "approved document ownership scan exceeds its bounded budget",
            ));
        }
        for entry in fs::read_dir(&directory)
            .map_err(|error| invalid(format!("approved ownership scan failed: {error}")))?
        {
            let entry = entry
                .map_err(|error| invalid(format!("approved ownership entry failed: {error}")))?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| invalid(format!("approved ownership metadata failed: {error}")))?;
            if metadata.file_type().is_symlink() {
                return Err(invalid("approved ownership scan rejects symlinked paths"));
            }
            entries += 1;
            if metadata.is_dir() {
                pending.push((path, depth + 1));
                continue;
            }
            if !metadata.is_file()
                || path.extension().and_then(|value| value.to_str()) != Some("json")
            {
                continue;
            }
            let bytes =
                read_regular_file(&path, MAX_PROMOTION_FILE_BYTES, "approved ownership file")?;
            let value: Value = match serde_json::from_slice(&bytes) {
                Ok(value) => value,
                Err(_) => continue,
            };
            if let Some(document_id) = value.get("document_id").and_then(Value::as_str) {
                if safe_label(document_id) {
                    ownership.document_ids.insert(document_id.to_owned());
                }
            }
            if value.get("kind").and_then(Value::as_str)
                == Some("ui_document_promotion_registration")
            {
                for (field, values) in [
                    ("owner", &mut ownership.owners),
                    ("route", &mut ownership.routes),
                ] {
                    let value = value.get(field).and_then(Value::as_str).ok_or_else(|| {
                        invalid("approved promotion registration lacks owner or route")
                    })?;
                    if !safe_label(value) || !values.insert(value.to_owned()) {
                        return Err(invalid(
                            "approved promotion registration owner/route is invalid or duplicated",
                        ));
                    }
                }
            }
        }
    }
    if ownership.document_ids.len() > MAX_APPROVED_DOCUMENTS {
        return Err(invalid(
            "approved document count exceeds promotion ownership budget",
        ));
    }
    Ok(ownership)
}

fn registration_json(registration: &PromotionRegistrationChange) -> Result<Vec<u8>, TaskFailure> {
    let bytes = pretty_json_bytes(&serde_json::json!({
        "protocol_version": PROMOTION_PROTOCOL_VERSION,
        "kind": "ui_document_promotion_registration",
        "template_version": registration.template_version,
        "document_id": registration.document_id,
        "source": {
            "root": registration.source_root,
            "relative_path": registration.source_relative_path,
        },
        "owner": registration.owner,
        "route": registration.route,
        "panel": registration.panel,
        "layer": registration.layer,
        "page_state": registration.page_state,
        "audit_profiles": registration.audit_profiles,
        "i18n_keys": registration.i18n_keys,
        "theme_tokens": registration.theme_tokens,
        "action_or_binding_registration": registration.action_or_binding_registration,
    }))?;
    let source = std::str::from_utf8(&bytes)
        .map_err(|_| invalid("promotion registration JSON is not valid UTF-8"))?;
    parse_approved_document_registration(source).map_err(|error| {
        invalid(format!(
            "promotion registration is rejected by the formal project adapter: {}",
            error.code()
        ))
    })?;
    Ok(bytes)
}

fn verify_stage_links(
    run_root: &Path,
    manifest: &UiGenerationRunManifest,
) -> Result<(), TaskFailure> {
    let links = [
        &manifest.stage_evidence.input_preprocess_manifest,
        &manifest.stage_evidence.reference_analysis,
        &manifest.stage_evidence.asset_strategy,
        &manifest.stage_evidence.generated_document,
        &manifest.stage_evidence.generation_trace,
    ];
    for link in links
        .into_iter()
        .chain(manifest.stage_evidence.input_references.iter())
        .chain(manifest.stage_evidence.draft_assets.iter())
    {
        verify_link(run_root, link)?;
    }
    Ok(())
}

fn verify_bundle_artifacts(
    bundle_root: &Path,
    artifacts: &BTreeMap<String, RunArtifactRecord>,
) -> Result<(), TaskFailure> {
    if artifacts.is_empty() || artifacts.len() > 96 {
        return Err(invalid(
            "committed run artifact table is empty or over budget",
        ));
    }
    for (key, artifact) in artifacts {
        if !safe_label(key)
            || !safe_relative_path(&artifact.relative_path)
            || !is_sha256(&artifact.sha256)
        {
            return Err(invalid(
                "committed run artifact table contains unsafe metadata",
            ));
        }
        let bytes = read_bundle_artifact(bundle_root, artifact)?;
        if bytes.len() as u64 != artifact.byte_length || hash_bytes(&bytes) != artifact.sha256 {
            return Err(invalid(
                "committed run artifact bytes do not match their manifest record",
            ));
        }
    }
    Ok(())
}

fn required_bundle_artifact<'a>(
    artifacts: &'a BTreeMap<String, RunArtifactRecord>,
    name: &str,
) -> Result<&'a RunArtifactRecord, TaskFailure> {
    let artifact = artifacts
        .get(name)
        .ok_or_else(|| invalid(format!("committed run lacks required `{name}` artifact")))?;
    if artifact.kind != name {
        return Err(invalid(
            "committed run artifact kind differs from its required identity",
        ));
    }
    Ok(artifact)
}

fn read_bundle_artifact(root: &Path, artifact: &RunArtifactRecord) -> Result<Vec<u8>, TaskFailure> {
    read_relative_regular_file(
        root,
        &artifact.relative_path,
        MAX_PROMOTION_FILE_BYTES,
        "bundle artifact",
    )
}

fn read_stage_json(root: &Path, link: &ArtifactLink, label: &str) -> Result<Value, TaskFailure> {
    verify_link(root, link)?;
    serde_json::from_slice(&read_relative_regular_file(
        root,
        &link.relative_path,
        MAX_PROMOTION_FILE_BYTES,
        label,
    )?)
    .map_err(|error| invalid(format!("committed {label} is not JSON: {error}")))
}

fn verify_link(root: &Path, link: &ArtifactLink) -> Result<(), TaskFailure> {
    if !safe_relative_path(&link.relative_path) || !is_sha256(&link.sha256) || link.byte_length == 0
    {
        return Err(invalid("committed stage evidence link is invalid"));
    }
    let bytes = read_relative_regular_file(
        root,
        &link.relative_path,
        MAX_PROMOTION_FILE_BYTES,
        "stage evidence",
    )?;
    if bytes.len() as u64 != link.byte_length || hash_bytes(&bytes) != link.sha256 {
        return Err(invalid(
            "committed stage evidence differs from its recorded hash",
        ));
    }
    Ok(())
}

fn read_relative_regular_file(
    root: &Path,
    relative: &str,
    maximum: usize,
    label: &str,
) -> Result<Vec<u8>, TaskFailure> {
    if !safe_relative_path(relative) {
        return Err(invalid(format!("{label} has an unsafe relative path")));
    }
    let path = root.join(relative);
    let canonical = fs::canonicalize(&path)
        .map_err(|error| invalid(format!("{label} cannot be resolved: {error}")))?;
    if !canonical.starts_with(root) {
        return Err(invalid(format!("{label} escapes its controlled root")));
    }
    read_regular_file(&canonical, maximum, label)
}

fn reject_business_fields(value: &Value) -> Result<(), TaskFailure> {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                if matches!(
                    key.as_str(),
                    "action" | "on_click" | "binding_path" | "i18n_key"
                ) {
                    return Err(invalid(
                        "promotion refuses documents with action, binding, or i18n business fields",
                    ));
                }
                if key == "bindings"
                    && child
                        .as_object()
                        .is_some_and(|bindings| !bindings.is_empty())
                {
                    return Err(invalid(
                        "promotion refuses documents with binding declarations",
                    ));
                }
                reject_business_fields(child)?;
            }
        }
        Value::Array(values) => {
            for child in values {
                reject_business_fields(child)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_owner_and_route(owner: &str, route: &str) -> Result<(), TaskFailure> {
    if !safe_label(owner) || !safe_label(route) {
        return Err(invalid(
            "promotion owner and route must be bounded ASCII registration labels",
        ));
    }
    Ok(())
}

fn approved_folder_name(document_id: &str) -> Result<String, TaskFailure> {
    if !safe_label(document_id) {
        return Err(invalid(
            "formal UiDocument ID is not a safe promotion folder source",
        ));
    }
    let folder = document_id.replace('.', "_");
    if folder.len() > 128
        || !folder
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(invalid("approved promotion folder is unsafe"));
    }
    Ok(folder)
}

fn required_hash_field(value: &Value, field: &str, label: &str) -> Result<String, TaskFailure> {
    let value = value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("{label} lacks `{field}`")))?;
    if !is_sha256(value) {
        return Err(invalid(format!("{label} `{field}` is not a SHA-256")));
    }
    Ok(value.to_owned())
}

fn validate_run_id(value: &str) -> Result<(), TaskFailure> {
    if !safe_label(value) {
        return Err(invalid("promotion run ID is invalid"));
    }
    Ok(())
}

fn safe_relative_path(value: &str) -> bool {
    !value.is_empty()
        && value.is_ascii()
        && !Path::new(value).is_absolute()
        && !value.contains(['\\', ':', '\0'])
        && Path::new(value)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn safe_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_uppercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'.' | b'_' | b'-')
        })
}

fn safe_asset_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.split('.').all(|segment| {
            let mut bytes = segment.bytes();
            bytes.next().is_some_and(|first| first.is_ascii_lowercase())
                && bytes
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        })
}

fn safe_resource_file_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.ends_with(".png")
        && Path::new(value).components().count() == 1
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

fn bounded_text(value: &str, maximum: usize) -> bool {
    !value.is_empty() && value.len() <= maximum && !value.contains(['\0', '\r', '\n'])
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn canonical_regular_directory(path: &Path, label: &str) -> Result<PathBuf, TaskFailure> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| invalid(format!("{label} metadata is unavailable: {error}")))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(invalid(format!("{label} must be a real directory")));
    }
    fs::canonicalize(path)
        .map_err(|error| invalid(format!("{label} cannot be canonicalized: {error}")))
}

fn read_regular_file(path: &Path, maximum: usize, label: &str) -> Result<Vec<u8>, TaskFailure> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| invalid(format!("{label} metadata is unavailable: {error}")))?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() == 0
        || metadata.len() > maximum as u64
    {
        return Err(invalid(format!("{label} must be a bounded regular file")));
    }
    let before = (metadata.len(), metadata.modified().ok());
    let mut file = fs::File::open(path)
        .map_err(|error| invalid(format!("{label} cannot be opened: {error}")))?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.read_to_end(&mut bytes)
        .map_err(|error| invalid(format!("{label} cannot be read: {error}")))?;
    let after = fs::metadata(path)
        .map_err(|error| invalid(format!("{label} metadata changed during read: {error}")))?;
    if bytes.len() as u64 != before.0 || (after.len(), after.modified().ok()) != before {
        return Err(invalid(format!("{label} changed while it was being read")));
    }
    Ok(bytes)
}

fn write_new_synced(path: &Path, bytes: &[u8]) -> Result<(), TaskFailure> {
    write_new_synced_bounded(path, bytes, MAX_PROMOTION_FILE_BYTES)
}

fn write_new_synced_bounded(path: &Path, bytes: &[u8], maximum: usize) -> Result<(), TaskFailure> {
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(invalid(
            "promotion output is empty or exceeds the bounded file budget",
        ));
    }
    let parent = path
        .parent()
        .ok_or_else(|| invalid("promotion output has no parent directory"))?;
    if !parent.is_dir() {
        fs::create_dir_all(parent)
            .map_err(|error| write_failure(parent, "create promotion output parent", error))?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| write_failure(path, "create promotion output", error))?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|error| write_failure(path, "write promotion output", error))
}

fn pretty_json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, TaskFailure> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|_| invalid("promotion record cannot be serialized"))?;
    bytes.push(b'\n');
    if bytes.len() > MAX_PROMOTION_FILE_BYTES {
        return Err(invalid("promotion JSON exceeds the bounded output budget"));
    }
    Ok(bytes)
}

fn hash_json<T: Serialize>(value: &T) -> Result<String, TaskFailure> {
    serde_json::to_vec(value)
        .map(|bytes| hash_bytes(&bytes))
        .map_err(|_| invalid("promotion plan cannot be hashed"))
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn invalid(message: impl Into<String>) -> TaskFailure {
    TaskFailure::new(TaskFailureKind::InvalidInput, message, None)
}

fn conflict(message: impl Into<String>) -> TaskFailure {
    TaskFailure::new(TaskFailureKind::OutputDirectoryConflict, message, None)
}

fn write_failure(path: &Path, action: &str, error: std::io::Error) -> TaskFailure {
    TaskFailure::new(
        TaskFailureKind::OutputDirectoryConflict,
        format!("promotion could not {action}: {error}"),
        Some(path.display().to_string()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};

    const RUN_ID: &str = "promotion-run";

    struct Fixture {
        repository: tempfile::TempDir,
        run_root: PathBuf,
    }

    impl Fixture {
        fn create() -> Self {
            Self::create_inner(false)
        }

        fn create_with_authorized_resource() -> Self {
            Self::create_inner(true)
        }

        fn create_inner(with_resource: bool) -> Self {
            let repository = tempfile::tempdir().unwrap();
            let root = repository.path();
            fs::create_dir_all(root.join("project/assets/ui/documents/approved")).unwrap();
            fs::create_dir_all(root.join("project/assets/ui")).unwrap();
            fs::write(
                root.join(".gitattributes"),
                b"project/assets/**/*.png filter=lfs diff=lfs merge=lfs -text\n",
            )
            .unwrap();
            let run_root = root.join("summary/ui-generation").join(RUN_ID);
            fs::create_dir_all(run_root.join("bundle/final")).unwrap();
            fs::create_dir_all(run_root.join("bundle/repair")).unwrap();
            fs::create_dir_all(run_root.join("bundle/logs")).unwrap();
            fs::create_dir_all(run_root.join("input/preprocessed")).unwrap();
            fs::create_dir_all(run_root.join("analysis")).unwrap();
            fs::create_dir_all(run_root.join("draft")).unwrap();

            let document_source = if with_resource {
                r#"{"schema_version":1,"document_id":"promotion.fixture","assets":{"promotion_image":{"kind":"image","source":{"kind":"packaged","path":"ui/documents/approved/promotion_fixture/assets/promotion.png"}}},"tokens":{},"root":{"type":"container","id":"page.root","children":[]}}"#
            } else {
                r#"{"schema_version":1,"document_id":"promotion.fixture","assets":{},"tokens":{},"root":{"type":"container","id":"page.root","children":[]}}"#
            };
            let document = canonicalize_json(document_source).unwrap();
            let input_hash = "1".repeat(64);
            let document_hash = hash_bytes(document.as_bytes());
            let bundle_files = BTreeMap::from([
                (
                    "final_document",
                    ("final/document.json", document.clone().into_bytes()),
                ),
                (
                    "generation_trace",
                    (
                        "final/generation-trace.json",
                        json_bytes(serde_json::json!({
                            "input_sha256": input_hash,
                            "canonical_document_sha256": document_hash,
                        })),
                    ),
                ),
                (
                    "validation_report",
                    (
                        "final/validation-report.json",
                        json_bytes(serde_json::json!({"valid":true})),
                    ),
                ),
                (
                    "source_map",
                    (
                        "final/source-map.json",
                        json_bytes(serde_json::json!([{"node_id":"page.root"}])),
                    ),
                ),
                (
                    "repair_run",
                    (
                        "repair/run.json",
                        json_bytes(serde_json::json!({
                            "final_document":{"canonical_document_json": document}
                        })),
                    ),
                ),
            ]);
            let mut artifacts = BTreeMap::new();
            for (kind, (relative, bytes)) in bundle_files {
                let path = run_root.join("bundle").join(relative);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(&path, &bytes).unwrap();
                artifacts.insert(
                    kind.to_owned(),
                    serde_json::json!({
                        "kind": kind,
                        "relative_path": relative,
                        "sha256": hash_bytes(&bytes),
                        "byte_length": bytes.len(),
                    }),
                );
            }
            let strategy = if with_resource {
                serde_json::json!({"entries":[{
                    "strategy_id":"promotion.crop",
                    "element_id":"promotion.image",
                    "disposition":"authorized_crop",
                    "specification":{"width":1,"height":1,"alpha":"straight","slice_insets":null,"color_space":"srgb","usage":"content_image"},
                    "crop":{"authorization":"derivatives_allowed","license_reference":"fixture-license"},
                    "approval_status":"pending_human_review"
                }]})
            } else {
                serde_json::json!({"entries":[]})
            };
            let mut stage = vec![
                (
                    "input/preprocessed/manifest.json",
                    json_bytes(serde_json::json!({"run_id":RUN_ID})),
                ),
                (
                    "analysis/reference-analysis.json",
                    json_bytes(serde_json::json!({"uncertainties":[]})),
                ),
                ("analysis/asset-strategy.json", json_bytes(strategy)),
                ("draft/generated-document.json", b"draft".to_vec()),
                ("logs/generation-trace.json", b"trace".to_vec()),
            ];
            if with_resource {
                stage.push(("assets/promotion.png", fixture_png()));
            }
            let mut links = BTreeMap::new();
            for (name, bytes) in stage {
                let path = run_root.join(name);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(&path, &bytes).unwrap();
                links.insert(
                    name,
                    serde_json::json!({
                        "relative_path": name,
                        "sha256": hash_bytes(&bytes),
                        "byte_length": bytes.len(),
                    }),
                );
            }
            let manifest = serde_json::json!({
                "protocol_version": RUN_MANIFEST_PROTOCOL_VERSION,
                "run_id": RUN_ID,
                "status": "passed",
                "stage_evidence": {
                    "input_preprocess_manifest": links["input/preprocessed/manifest.json"],
                    "input_references": [],
                    "reference_analysis": links["analysis/reference-analysis.json"],
                    "asset_strategy": links["analysis/asset-strategy.json"],
                    "draft_assets": if with_resource { vec![links["assets/promotion.png"].clone()] } else { Vec::new() },
                    "generated_document": links["draft/generated-document.json"],
                    "generation_trace": links["logs/generation-trace.json"],
                },
                "repair_status":"passed",
                "repair_round_count":0,
                "preview_status":"passed",
                "artifacts": artifacts,
            });
            let manifest_bytes = json_bytes(manifest);
            fs::write(run_root.join("bundle/manifest.json"), &manifest_bytes).unwrap();
            fs::write(
                run_root.join("COMMITTED"),
                format!("manifest_sha256={}\n", hash_bytes(&manifest_bytes)),
            )
            .unwrap();
            Self {
                repository,
                run_root,
            }
        }

        fn template(&self) -> PromotionDecisionTemplate {
            create_promotion_decision_template(self.repository.path(), RUN_ID).unwrap()
        }

        fn record(
            &self,
            resolution: PromotionResolution,
        ) -> Result<RecordedPromotionDecisions, TaskFailure> {
            self.record_with_resources(resolution, Vec::new())
        }

        fn record_authorized_resource(&self) -> Result<RecordedPromotionDecisions, TaskFailure> {
            let source = self.run_root.join("assets/promotion.png");
            let bytes = fs::read(&source).unwrap();
            self.record_with_resources(
                PromotionResolution::Accept,
                vec![PromotionResourceSubmission {
                    strategy_id: "promotion.crop".to_owned(),
                    asset_id: "ui.generated.promotion_fixture.image".to_owned(),
                    source_relative_path: "assets/promotion.png".to_owned(),
                    source_sha256: hash_bytes(&bytes),
                    byte_length: bytes.len() as u64,
                    target_file_name: "promotion.png".to_owned(),
                    license_status: PromotionResourceLicenseStatus::Redistributable,
                    license_reference: "fixture-license".to_owned(),
                }],
            )
        }

        fn record_with_resources(
            &self,
            resolution: PromotionResolution,
            resources: Vec<PromotionResourceSubmission>,
        ) -> Result<RecordedPromotionDecisions, TaskFailure> {
            let template = self.template();
            let decisions = template
                .questions
                .iter()
                .map(|question| {
                    let candidate = question
                        .candidates
                        .iter()
                        .find(|candidate| candidate.resolution == resolution)
                        .unwrap_or_else(|| question.candidates.last().unwrap());
                    PromotionDecision {
                        question_id: question.question_id.clone(),
                        candidate_id: candidate.candidate_id.clone(),
                        resolution: candidate.resolution,
                        rationale: "fixture explicit human decision".to_owned(),
                    }
                })
                .collect();
            let submission = PromotionDecisionSubmission {
                protocol_version: template.protocol_version,
                run_id: template.run_id,
                run_manifest_sha256: template.run_manifest_sha256,
                canonical_document_sha256: template.canonical_document_sha256,
                input_sha256: template.input_sha256,
                approved_by: "fixture_reviewer".to_owned(),
                decisions,
                resources,
            };
            let path = self.run_root.join("submission.json");
            fs::write(&path, json_bytes(serde_json::to_value(submission).unwrap())).unwrap();
            record_promotion_decisions(self.repository.path(), &path)
        }
    }

    fn json_bytes(value: Value) -> Vec<u8> {
        let mut bytes = serde_json::to_vec_pretty(&value).unwrap();
        bytes.push(b'\n');
        bytes
    }

    fn fixture_png() -> Vec<u8> {
        let mut output = Vec::new();
        PngEncoder::new(&mut output)
            .write_image(&[12, 34, 56, 200], 1, 1, ExtendedColorType::Rgba8)
            .unwrap();
        output
    }

    #[test]
    fn approved_decision_creates_a_new_atomic_owned_page_directory() {
        let fixture = Fixture::create();
        fixture.record(PromotionResolution::Accept).unwrap();
        let plan = create_promotion_plan(
            fixture.repository.path(),
            RUN_ID,
            "promotion_owner",
            "promotion_route",
        )
        .unwrap();
        assert!(plan.resources.is_empty());
        assert!(plan.registration.i18n_keys.is_empty());
        assert!(plan.registration.action_or_binding_registration.is_empty());
        let result = promote(
            fixture.repository.path(),
            RUN_ID,
            "promotion_owner",
            "promotion_route",
            &plan.plan_sha256,
        )
        .unwrap();
        assert!(result.document_path.is_file());
        assert!(result.registration_path.is_file());
        assert_eq!(
            hash_bytes(&fs::read(&result.document_path).unwrap()),
            plan.document.sha256
        );
        let registration: Value =
            serde_json::from_slice(&fs::read(&result.registration_path).unwrap()).unwrap();
        assert_eq!(registration["owner"], "promotion_owner");
        assert_eq!(registration["route"], "promotion_route");
        assert!(!fixture.repository.path().join("project/src").exists());
        assert!(!result.approved_directory.with_extension("partial").exists());
    }

    #[test]
    fn rejected_decision_blocks_planning_without_writing_formal_files() {
        let fixture = Fixture::create();
        fixture.record(PromotionResolution::Reject).unwrap();
        assert!(
            create_promotion_plan(
                fixture.repository.path(),
                RUN_ID,
                "promotion_owner",
                "promotion_route",
            )
            .is_err()
        );
        assert!(
            !fixture
                .repository
                .path()
                .join("project/assets/ui/documents/approved/promotion_fixture")
                .exists()
        );
    }

    #[test]
    fn conflict_after_plan_is_rechecked_without_partial_commit() {
        let fixture = Fixture::create();
        fixture.record(PromotionResolution::Accept).unwrap();
        let plan = create_promotion_plan(
            fixture.repository.path(),
            RUN_ID,
            "promotion_owner",
            "promotion_route",
        )
        .unwrap();
        let conflict = fixture
            .repository
            .path()
            .join("project/assets/ui/documents/approved/promotion_fixture");
        fs::create_dir(&conflict).unwrap();
        fs::write(conflict.join("sentinel.txt"), b"do not replace").unwrap();
        assert!(
            promote(
                fixture.repository.path(),
                RUN_ID,
                "promotion_owner",
                "promotion_route",
                &plan.plan_sha256,
            )
            .is_err()
        );
        assert_eq!(
            fs::read(conflict.join("sentinel.txt")).unwrap(),
            b"do not replace"
        );
        let staging: Vec<_> = fs::read_dir(conflict.parent().unwrap())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".promotion-staging-")
            })
            .collect();
        assert!(staging.is_empty());
    }

    #[test]
    fn existing_game_owner_cannot_be_reused_by_a_promoted_page() {
        let fixture = Fixture::create();
        fs::create_dir_all(fixture.repository.path().join("project/src/game")).unwrap();
        fs::write(
            fixture.repository.path().join("project/src/game/ui_ids.rs"),
            b"const OWNER: UiOwnerId = UiOwnerId::new(\"runtime_owner\");\n",
        )
        .unwrap();
        fixture.record(PromotionResolution::Accept).unwrap();
        assert!(
            create_promotion_plan(
                fixture.repository.path(),
                RUN_ID,
                "runtime_owner",
                "promotion_route",
            )
            .is_err()
        );
    }

    #[test]
    fn tampered_bundle_or_mismatched_submission_cannot_spoof_approval() {
        let fixture = Fixture::create();
        fs::write(
            fixture.run_root.join("bundle/final/document.json"),
            b"{\"schema_version\":1}",
        )
        .unwrap();
        assert!(create_promotion_decision_template(fixture.repository.path(), RUN_ID).is_err());

        let fixture = Fixture::create();
        let mut template = fixture.template();
        template.submission.input_sha256 = "f".repeat(64);
        let path = fixture.run_root.join("forged-submission.json");
        fs::write(
            &path,
            json_bytes(serde_json::to_value(template.submission).unwrap()),
        )
        .unwrap();
        assert!(record_promotion_decisions(fixture.repository.path(), &path).is_err());
        assert!(!fixture.run_root.join(PROMOTION_DECISION_MANIFEST).exists());
    }

    #[test]
    fn authorized_draft_resource_requires_lfs_and_commits_with_its_page_catalog() {
        let fixture = Fixture::create_with_authorized_resource();
        fixture.record_authorized_resource().unwrap();
        let plan = create_promotion_plan(
            fixture.repository.path(),
            RUN_ID,
            "resource_owner",
            "resource_route",
        )
        .unwrap();
        assert_eq!(plan.resources.len(), 1);
        assert_eq!(
            plan.resources[0].target_asset_path,
            "ui/documents/approved/promotion_fixture/assets/promotion.png"
        );
        let result = promote(
            fixture.repository.path(),
            RUN_ID,
            "resource_owner",
            "resource_route",
            &plan.plan_sha256,
        )
        .unwrap();
        assert_eq!(result.resource_paths.len(), 1);
        assert_eq!(fs::read(&result.resource_paths[0]).unwrap(), fixture_png(),);
        let catalog: Value =
            serde_json::from_slice(
                &fs::read(fixture.repository.path().join(
                    "project/assets/ui/documents/approved/promotion_fixture/catalog.v1.json",
                ))
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            catalog["assets"][0]["asset_id"],
            "ui.generated.promotion_fixture.image"
        );
    }
}
