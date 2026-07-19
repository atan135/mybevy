//! Fail-closed execution of a Stage 5 fix plan.
//!
//! The repair plan is deliberately only an allow-list. This module accepts a separate, typed
//! patch set, previews every resulting file change, and requires a short-lived human approval
//! bound to both documents before it writes a draft. It never stages Git changes, commits, pushes,
//! or writes formal approved-page output.

use crate::{
    closed_loop_fix_plan::{
        CLOSED_LOOP_FIX_PLAN_PROTOCOL_VERSION, ClosedLoopFixPlan, FixModificationKind,
        FixPlanAction, FixPlanStatus,
    },
    lifecycle::{TaskFailure, TaskFailureKind},
};
use project::framework::ui::document::tooling::{canonicalize_json, validate_json};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

pub const CLOSED_LOOP_APPLY_PROTOCOL_VERSION: u32 = 1;
const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024;
const MAX_PATCHES: usize = 64;
const MAX_APPROVAL_SECONDS: u64 = 24 * 60 * 60;
static STAGING_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClosedLoopPatchSet {
    pub protocol_version: u32,
    pub run_id: String,
    pub plan_sha256: String,
    pub patches: Vec<ClosedLoopPatch>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClosedLoopPatch {
    UiDocument {
        action_id: String,
        target_file: String,
        node_id: String,
        field_path: String,
        value: Value,
    },
    DraftAssetVersion {
        action_id: String,
        target_file: String,
        replacement_source: String,
        source_sha256: String,
        source_provenance: String,
        license_reference: String,
    },
    RustUnifiedDiff {
        action_id: String,
        target_file: String,
        unified_diff: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClosedLoopApplyApproval {
    pub protocol_version: u32,
    pub approval_id: String,
    pub run_id: String,
    pub plan_sha256: String,
    pub patch_set_sha256: String,
    pub preview_sha256: String,
    pub approved_by: String,
    pub decision: String,
    pub issued_at_epoch_seconds: u64,
    pub expires_at_epoch_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClosedLoopApplyPreview {
    pub protocol_version: u32,
    pub run_id: String,
    pub plan_sha256: String,
    pub patch_set_sha256: String,
    pub preview_sha256: String,
    pub changes: Vec<ClosedLoopPreviewChange>,
    /// These fields are always present, including empty lists, so formal promotion can show the
    /// complete document/resource/i18n/theme/page-registration scope before a human approves it.
    pub promotion_scope: ClosedLoopPromotionScope,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClosedLoopPreviewChange {
    pub action_id: String,
    pub category: String,
    pub target_file: String,
    pub before_sha256: Option<String>,
    pub after_sha256: String,
    pub changed_lines: u32,
    pub full_unified_diff: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClosedLoopPromotionScope {
    pub documents: Vec<String>,
    pub resources: Vec<String>,
    pub i18n: Vec<String>,
    pub theme: Vec<String>,
    pub page_registration: Vec<String>,
    pub rust_sources: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClosedLoopApplyResult {
    pub plan_sha256: String,
    pub patch_set_sha256: String,
    pub preview_sha256: String,
    pub written_files: Vec<PathBuf>,
}

#[derive(Clone, Debug)]
struct PreparedWrite {
    action_id: String,
    category: String,
    target_file: String,
    target_path: PathBuf,
    before: Option<Vec<u8>>,
    after: Vec<u8>,
    changed_lines: u32,
}

/// Builds a complete, no-write diff. The JSON plan and patch set are hash-bound as raw files so
/// a decision cannot silently apply semantically equivalent but differently scoped input.
pub fn preview_closed_loop_apply(
    repository_root: &Path,
    plan_path: &Path,
    patch_set_path: &Path,
) -> Result<ClosedLoopApplyPreview, TaskFailure> {
    let prepared = prepare_apply(repository_root, plan_path, patch_set_path)?;
    Ok(preview_from_prepared(&prepared))
}

/// Applies an already-previewable patch set after validating a non-expired human approval.
/// This function has no Git operations and only changes the exact files represented by the
/// preview. A failed preflight writes nothing; a failed commit attempts to restore every file.
pub fn apply_closed_loop_patches(
    repository_root: &Path,
    plan_path: &Path,
    patch_set_path: &Path,
    approval_path: &Path,
) -> Result<ClosedLoopApplyResult, TaskFailure> {
    let prepared = prepare_apply(repository_root, plan_path, patch_set_path)?;
    let preview = preview_from_prepared(&prepared);
    let approval: ClosedLoopApplyApproval = read_json(approval_path, "closed-loop approval")?;
    validate_approval(&approval, &prepared, &preview, now_epoch_seconds()?)?;
    let written_files = commit_prepared_writes(&prepared.writes)?;
    Ok(ClosedLoopApplyResult {
        plan_sha256: prepared.plan_sha256,
        patch_set_sha256: prepared.patch_set_sha256,
        preview_sha256: preview.preview_sha256,
        written_files,
    })
}

struct PreparedApply {
    plan: ClosedLoopFixPlan,
    plan_sha256: String,
    patch_set_sha256: String,
    writes: Vec<PreparedWrite>,
}

fn prepare_apply(
    repository_root: &Path,
    plan_path: &Path,
    patch_set_path: &Path,
) -> Result<PreparedApply, TaskFailure> {
    let repository_root =
        canonical_regular_directory(repository_root, "closed-loop repository root")?;
    let plan_bytes = read_regular_file(plan_path, MAX_INPUT_BYTES, "closed-loop fix plan")?;
    let plan: ClosedLoopFixPlan = serde_json::from_slice(&plan_bytes)
        .map_err(|_| invalid("closed-loop fix plan is malformed or contains unknown fields"))?;
    validate_plan(&plan)?;
    let patch_bytes = read_regular_file(patch_set_path, MAX_INPUT_BYTES, "closed-loop patch set")?;
    let patch_set: ClosedLoopPatchSet = serde_json::from_slice(&patch_bytes)
        .map_err(|_| invalid("closed-loop patch set is malformed or contains unknown fields"))?;
    let plan_sha256 = hash_bytes(&plan_bytes);
    let patch_set_sha256 = hash_bytes(&patch_bytes);
    validate_patch_set(&patch_set, &plan, &plan_sha256)?;

    let actions: BTreeMap<_, _> = plan
        .actions
        .iter()
        .map(|action| (action.action_id.as_str(), action))
        .collect();
    let mut seen_actions = BTreeSet::new();
    let mut writes = Vec::new();
    for patch in &patch_set.patches {
        let action_id = patch_action_id(patch);
        if !seen_actions.insert(action_id) {
            return Err(conflict(
                "closed-loop patch set contains a duplicate action ID",
            ));
        }
        let action = actions
            .get(action_id)
            .copied()
            .ok_or_else(|| invalid("closed-loop patch names an action outside the fix plan"))?;
        writes.extend(prepare_patch(&repository_root, &plan, action, patch)?);
    }
    if seen_actions.len() != actions.len() {
        return Err(invalid(
            "closed-loop patch set must cover every action in the approved fix plan exactly once",
        ));
    }
    validate_prepared_writes(&writes, &plan)?;
    Ok(PreparedApply {
        plan,
        plan_sha256,
        patch_set_sha256,
        writes,
    })
}

fn validate_plan(plan: &ClosedLoopFixPlan) -> Result<(), TaskFailure> {
    if plan.protocol_version != CLOSED_LOOP_FIX_PLAN_PROTOCOL_VERSION
        || plan.run_id.is_empty()
        || !safe_label(&plan.run_id)
        || plan.actions.is_empty()
        || plan.actions.len() > MAX_PATCHES
        || plan.status == FixPlanStatus::NoAvailableFix
    {
        return Err(invalid(
            "closed-loop fix plan is not an applicable bounded Stage 5 plan",
        ));
    }
    if plan.policy.allowed_roots.is_empty()
        || plan.policy.max_files == 0
        || plan.policy.max_files > MAX_PATCHES
        || plan.policy.max_diff_lines == 0
        || plan.policy.max_asset_bytes == 0
        || plan
            .policy
            .allowed_roots
            .iter()
            .any(|root| normalize_root(root).is_none())
    {
        return Err(invalid(
            "closed-loop fix plan has an unsafe allowed-root policy",
        ));
    }
    let mut action_ids = BTreeSet::new();
    let mut total_diff_lines = 0u64;
    let mut total_asset_bytes = 0u64;
    for action in &plan.actions {
        if !safe_label(&action.action_id)
            || !action_ids.insert(&action.action_id)
            || !path_in_allowed_roots(&action.target.target_file, &plan)
            || protected_path(&action.target.target_file)
            || action.estimated_diff_lines > plan.policy.max_diff_lines
            || action
                .estimated_asset_bytes
                .is_some_and(|bytes| bytes > plan.policy.max_asset_bytes)
        {
            return Err(invalid(
                "closed-loop fix plan contains an unsafe action allow-list",
            ));
        }
        total_diff_lines = total_diff_lines.saturating_add(u64::from(action.estimated_diff_lines));
        total_asset_bytes =
            total_asset_bytes.saturating_add(action.estimated_asset_bytes.unwrap_or_default());
    }
    if action_ids.len() > plan.policy.max_files
        || total_diff_lines > u64::from(plan.policy.max_diff_lines)
        || total_asset_bytes > plan.policy.max_asset_bytes
    {
        return Err(invalid(
            "closed-loop fix plan exceeds its declared global file, diff, or asset budget",
        ));
    }
    Ok(())
}

fn validate_patch_set(
    patch_set: &ClosedLoopPatchSet,
    plan: &ClosedLoopFixPlan,
    plan_sha256: &str,
) -> Result<(), TaskFailure> {
    if patch_set.protocol_version != CLOSED_LOOP_APPLY_PROTOCOL_VERSION
        || patch_set.run_id != plan.run_id
        || patch_set.plan_sha256 != plan_sha256
        || !is_sha256(&patch_set.plan_sha256)
        || patch_set.patches.is_empty()
        || patch_set.patches.len() > MAX_PATCHES
    {
        return Err(invalid(
            "closed-loop patch set is not bound to the exact applicable fix plan",
        ));
    }
    Ok(())
}

fn prepare_patch(
    repository_root: &Path,
    plan: &ClosedLoopFixPlan,
    action: &FixPlanAction,
    patch: &ClosedLoopPatch,
) -> Result<Vec<PreparedWrite>, TaskFailure> {
    match patch {
        ClosedLoopPatch::UiDocument {
            action_id,
            target_file,
            node_id,
            field_path,
            value,
        } => prepare_document_patch(
            repository_root,
            plan,
            action,
            action_id,
            target_file,
            node_id,
            field_path,
            value,
        ),
        ClosedLoopPatch::DraftAssetVersion {
            action_id,
            target_file,
            replacement_source,
            source_sha256,
            source_provenance,
            license_reference,
        } => prepare_asset_patch(
            repository_root,
            plan,
            action,
            action_id,
            target_file,
            replacement_source,
            source_sha256,
            source_provenance,
            license_reference,
        ),
        ClosedLoopPatch::RustUnifiedDiff {
            action_id,
            target_file,
            unified_diff,
        } => prepare_rust_patch(
            repository_root,
            plan,
            action,
            action_id,
            target_file,
            unified_diff,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn prepare_document_patch(
    repository_root: &Path,
    plan: &ClosedLoopFixPlan,
    action: &FixPlanAction,
    action_id: &str,
    target_file: &str,
    node_id: &str,
    field_path: &str,
    value: &Value,
) -> Result<Vec<PreparedWrite>, TaskFailure> {
    if action.modification != FixModificationKind::UiDocumentLayout
        && action.modification != FixModificationKind::UiDocumentScopedToken
    {
        return Err(invalid(
            "a UiDocument patch does not match its planned modification kind",
        ));
    }
    validate_action_target(action, action_id, target_file, plan)?;
    if action.target.node_id.as_deref() != Some(node_id)
        || action.target.document_path.as_deref() != Some(field_path)
        || !safe_node_id(node_id)
        || !allowed_document_field(action.modification, field_path)
    {
        return Err(invalid(
            "UiDocument patch must use the plan-bound node ID and allowed field path",
        ));
    }
    let target_path = resolve_existing_regular_file(repository_root, target_file)?;
    let before = read_regular_file(&target_path, MAX_INPUT_BYTES, "UiDocument target")?;
    let source =
        std::str::from_utf8(&before).map_err(|_| invalid("UiDocument target is not UTF-8 JSON"))?;
    let mut document: Value =
        serde_json::from_str(source).map_err(|_| invalid("UiDocument patch target is not JSON"))?;
    let root = document
        .get("root")
        .ok_or_else(|| invalid("UiDocument patch target has no root node"))?;
    if count_nodes(root, node_id) != 1 {
        return Err(invalid(
            "UiDocument patch node ID must resolve to exactly one structural node",
        ));
    }
    let node = find_node_mut(
        document
            .get_mut("root")
            .ok_or_else(|| invalid("UiDocument patch target has no mutable root node"))?,
        node_id,
    )
    .ok_or_else(|| invalid("UiDocument patch node ID disappeared during structural traversal"))?;
    set_document_field(node, field_path, value.clone())?;
    let patched = serde_json::to_string(&document)
        .map_err(|_| invalid("UiDocument structured patch cannot be serialized"))?;
    let after = canonicalize_json(&patched)
        .map_err(|_| invalid("UiDocument structured patch violates the formal schema or policy"))?
        .into_bytes();
    if !validate_json(std::str::from_utf8(&after).expect("canonical JSON is UTF-8"))
        .report
        .valid
    {
        return Err(invalid(
            "UiDocument structured patch failed formal validation",
        ));
    }
    let changed_lines = changed_line_count(&before, &after)?;
    enforce_action_budget(action, changed_lines, after.len() as u64)?;
    Ok(vec![PreparedWrite {
        action_id: action_id.to_owned(),
        category: "document".to_owned(),
        target_file: target_file.to_owned(),
        target_path,
        before: Some(before),
        after,
        changed_lines,
    }])
}

#[allow(clippy::too_many_arguments)]
fn prepare_asset_patch(
    repository_root: &Path,
    plan: &ClosedLoopFixPlan,
    action: &FixPlanAction,
    action_id: &str,
    target_file: &str,
    replacement_source: &str,
    source_sha256: &str,
    source_provenance: &str,
    license_reference: &str,
) -> Result<Vec<PreparedWrite>, TaskFailure> {
    if action.modification != FixModificationKind::DraftAssetVersion {
        return Err(invalid(
            "a draft asset patch does not match its planned modification kind",
        ));
    }
    validate_action_target(action, action_id, target_file, plan)?;
    if !safe_relative_path(replacement_source)
        || protected_path(replacement_source)
        || !path_in_allowed_roots(replacement_source, plan)
        || !is_sha256(source_sha256)
        || !bounded_text(source_provenance)
        || !bounded_text(license_reference)
    {
        return Err(invalid(
            "draft asset version metadata is unsafe or outside the fix plan",
        ));
    }
    let prior_path = resolve_existing_regular_file(repository_root, target_file)?;
    let prior = read_regular_file(&prior_path, MAX_INPUT_BYTES, "previous draft asset")?;
    let source_path = resolve_existing_regular_file(repository_root, replacement_source)?;
    let replacement = read_regular_file(&source_path, MAX_INPUT_BYTES, "replacement draft asset")?;
    if hash_bytes(&replacement) != source_sha256 || prior == replacement {
        return Err(invalid(
            "draft asset replacement hash is not bound to new bytes or does not create a new version",
        ));
    }
    let versioned_target = versioned_asset_path(target_file, source_sha256)?;
    if versioned_target == target_file || versioned_target == replacement_source {
        return Err(invalid(
            "draft asset version would overwrite a protected source",
        ));
    }
    let versioned_path = resolve_new_regular_file(repository_root, &versioned_target)?;
    let record_target = format!("{versioned_target}.rollback.json");
    let record_path = resolve_new_regular_file(repository_root, &record_target)?;
    let record = serde_json::to_vec_pretty(&serde_json::json!({
        "protocol_version": CLOSED_LOOP_APPLY_PROTOCOL_VERSION,
        "kind": "draft_asset_version",
        "previous_relative_path": target_file,
        "previous_sha256": hash_bytes(&prior),
        "previous_source": target_file,
        "replacement_source": replacement_source,
        "replacement_sha256": source_sha256,
        "version_relative_path": versioned_target,
        "source_provenance": source_provenance,
        "license_reference": license_reference,
        "rollback_to_relative_path": target_file,
    }))
    .map_err(|_| invalid("draft asset rollback record cannot be serialized"))?;
    enforce_action_budget(action, 0, replacement.len() as u64)?;
    Ok(vec![
        PreparedWrite {
            action_id: action_id.to_owned(),
            category: "resource".to_owned(),
            target_file: versioned_target.clone(),
            target_path: versioned_path,
            before: None,
            after: replacement.clone(),
            changed_lines: 0,
        },
        PreparedWrite {
            action_id: action_id.to_owned(),
            category: "resource".to_owned(),
            target_file: record_target,
            target_path: record_path,
            before: None,
            after: record,
            changed_lines: 0,
        },
    ])
}

fn prepare_rust_patch(
    repository_root: &Path,
    plan: &ClosedLoopFixPlan,
    action: &FixPlanAction,
    action_id: &str,
    target_file: &str,
    unified_diff: &str,
) -> Result<Vec<PreparedWrite>, TaskFailure> {
    if !matches!(
        action.modification,
        FixModificationKind::CommonWidget
            | FixModificationKind::Theme
            | FixModificationKind::Framework
    ) || !action.requires_approval
    {
        return Err(invalid(
            "Rust patches are only allowed for explicitly approval-gated shared-scope actions",
        ));
    }
    validate_action_target(action, action_id, target_file, plan)?;
    if !target_file.ends_with(".rs") || unified_diff.len() > MAX_INPUT_BYTES {
        return Err(invalid(
            "Rust patch target or unified diff exceeds the closed policy",
        ));
    }
    let target_path = resolve_existing_regular_file(repository_root, target_file)?;
    let before = read_regular_file(&target_path, MAX_INPUT_BYTES, "Rust patch target")?;
    let source =
        std::str::from_utf8(&before).map_err(|_| invalid("Rust patch target is not UTF-8"))?;
    let after = apply_single_file_unified_diff(source, unified_diff, target_file)?.into_bytes();
    if after == before {
        return Err(invalid("Rust unified diff makes no actual source change"));
    }
    let changed_lines = changed_line_count(&before, &after)?;
    enforce_action_budget(action, changed_lines, 0)?;
    Ok(vec![PreparedWrite {
        action_id: action_id.to_owned(),
        category: match action.modification {
            FixModificationKind::Theme => "theme",
            FixModificationKind::CommonWidget => "common_widget",
            FixModificationKind::Framework => "framework",
            _ => unreachable!(),
        }
        .to_owned(),
        target_file: target_file.to_owned(),
        target_path,
        before: Some(before),
        after,
        changed_lines,
    }])
}

fn validate_action_target(
    action: &FixPlanAction,
    action_id: &str,
    target_file: &str,
    plan: &ClosedLoopFixPlan,
) -> Result<(), TaskFailure> {
    if action.action_id != action_id
        || action.target.target_file != target_file
        || !path_in_allowed_roots(target_file, plan)
        || protected_path(target_file)
    {
        return Err(invalid(
            "patch target is outside the exact fix plan allow-list",
        ));
    }
    Ok(())
}

fn validate_prepared_writes(
    writes: &[PreparedWrite],
    plan: &ClosedLoopFixPlan,
) -> Result<(), TaskFailure> {
    let asset_actions = plan
        .actions
        .iter()
        .filter(|action| action.modification == FixModificationKind::DraftAssetVersion)
        .count();
    if writes.is_empty()
        || writes.len() > MAX_PATCHES * 2
        || writes.len() > plan.policy.max_files.saturating_add(asset_actions)
    {
        return Err(invalid(
            "closed-loop patch set did not produce a bounded write set",
        ));
    }
    let mut targets = BTreeSet::new();
    for write in writes {
        if !targets.insert(&write.target_file)
            || protected_path(&write.target_file)
            || !path_in_allowed_roots(&write.target_file, plan)
        {
            return Err(conflict(
                "closed-loop patch set has conflicting writes or escapes the plan roots",
            ));
        }
        let metadata = fs::symlink_metadata(&write.target_path).ok();
        if metadata.as_ref().is_some_and(metadata_is_reparse) {
            return Err(invalid(
                "closed-loop write target is a symlink or reparse point",
            ));
        }
    }
    Ok(())
}

fn preview_from_prepared(prepared: &PreparedApply) -> ClosedLoopApplyPreview {
    let mut changes: Vec<_> = prepared
        .writes
        .iter()
        .map(|write| ClosedLoopPreviewChange {
            action_id: write.action_id.clone(),
            category: write.category.clone(),
            target_file: write.target_file.clone(),
            before_sha256: write.before.as_ref().map(|bytes| hash_bytes(bytes)),
            after_sha256: hash_bytes(&write.after),
            changed_lines: write.changed_lines,
            full_unified_diff: render_full_unified_diff(
                &write.target_file,
                write.before.as_deref(),
                &write.after,
            ),
        })
        .collect();
    changes.sort_by(|left, right| left.target_file.cmp(&right.target_file));
    let scope = promotion_scope(&changes);
    let material = serde_json::json!({
        "protocol_version": CLOSED_LOOP_APPLY_PROTOCOL_VERSION,
        "run_id": prepared.plan.run_id,
        "plan_sha256": prepared.plan_sha256,
        "patch_set_sha256": prepared.patch_set_sha256,
        "changes": changes,
        "promotion_scope": scope,
    });
    let preview_sha256 =
        hash_json(&material).expect("closed-loop preview material is serializable");
    ClosedLoopApplyPreview {
        protocol_version: CLOSED_LOOP_APPLY_PROTOCOL_VERSION,
        run_id: prepared.plan.run_id.clone(),
        plan_sha256: prepared.plan_sha256.clone(),
        patch_set_sha256: prepared.patch_set_sha256.clone(),
        preview_sha256,
        changes,
        promotion_scope: scope,
    }
}

fn promotion_scope(changes: &[ClosedLoopPreviewChange]) -> ClosedLoopPromotionScope {
    let mut scope = ClosedLoopPromotionScope {
        documents: Vec::new(),
        resources: Vec::new(),
        i18n: Vec::new(),
        theme: Vec::new(),
        page_registration: Vec::new(),
        rust_sources: Vec::new(),
    };
    for change in changes {
        match change.category.as_str() {
            "document" => scope.documents.push(change.target_file.clone()),
            "resource" => scope.resources.push(change.target_file.clone()),
            "theme" => scope.theme.push(change.target_file.clone()),
            "i18n" => scope.i18n.push(change.target_file.clone()),
            "page_registration" => scope.page_registration.push(change.target_file.clone()),
            _ => scope.rust_sources.push(change.target_file.clone()),
        }
    }
    scope
}

fn validate_approval(
    approval: &ClosedLoopApplyApproval,
    prepared: &PreparedApply,
    preview: &ClosedLoopApplyPreview,
    now: u64,
) -> Result<(), TaskFailure> {
    if approval.protocol_version != CLOSED_LOOP_APPLY_PROTOCOL_VERSION
        || !safe_label(&approval.approval_id)
        || !safe_label(&approval.approved_by)
        || approval.decision != "approved"
        || approval.run_id != prepared.plan.run_id
        || approval.plan_sha256 != prepared.plan_sha256
        || approval.patch_set_sha256 != prepared.patch_set_sha256
        || approval.preview_sha256 != preview.preview_sha256
        || !is_sha256(&approval.plan_sha256)
        || !is_sha256(&approval.patch_set_sha256)
        || !is_sha256(&approval.preview_sha256)
        || approval.issued_at_epoch_seconds > now
        || approval.expires_at_epoch_seconds <= now
        || approval.expires_at_epoch_seconds - approval.issued_at_epoch_seconds
            > MAX_APPROVAL_SECONDS
    {
        return Err(invalid(
            "closed-loop apply requires an exact, explicit, non-expired approval record",
        ));
    }
    Ok(())
}

fn commit_prepared_writes(writes: &[PreparedWrite]) -> Result<Vec<PathBuf>, TaskFailure> {
    let mut staged = Vec::new();
    for write in writes {
        let stage = temporary_sibling(&write.target_path, "stage")?;
        write_new_file(&stage, &write.after)?;
        staged.push((write, stage));
    }
    let mut committed: Vec<(&PreparedWrite, Option<PathBuf>)> = Vec::new();
    let result = (|| {
        for (write, stage) in &staged {
            match &write.before {
                Some(before) => {
                    let current = read_regular_file(
                        &write.target_path,
                        MAX_INPUT_BYTES,
                        "current patch target",
                    )?;
                    if &current != before {
                        return Err(conflict(
                            "patch target changed after preview and cannot be replaced",
                        ));
                    }
                }
                None if fs::symlink_metadata(&write.target_path).is_ok() => {
                    return Err(conflict(
                        "new patch target appeared after preview and will not be overwritten",
                    ));
                }
                None => {}
            }
            let backup = if write.before.is_some() {
                let backup = temporary_sibling(&write.target_path, "backup")?;
                fs::rename(&write.target_path, &backup).map_err(|error| {
                    write_failure(&write.target_path, "preserve prior file", error)
                })?;
                Some(backup)
            } else {
                None
            };
            if let Err(error) = fs::rename(stage, &write.target_path) {
                if let Some(backup) = &backup {
                    let _ = fs::rename(backup, &write.target_path);
                }
                return Err(write_failure(
                    &write.target_path,
                    "commit staged patch",
                    error,
                ));
            }
            committed.push((write, backup));
            let actual = read_regular_file(&write.target_path, MAX_INPUT_BYTES, "committed patch")?;
            if actual != write.after {
                return Err(invalid(
                    "committed patch bytes differ from the approved preview",
                ));
            }
        }
        Ok(())
    })();
    if result.is_err() {
        for (write, backup) in committed.iter().rev() {
            let _ = fs::remove_file(&write.target_path);
            if let Some(backup) = backup {
                let _ = fs::rename(backup, &write.target_path);
            }
        }
    }
    for (_, stage) in &staged {
        let _ = fs::remove_file(stage);
    }
    for (_, backup) in &committed {
        if let Some(backup) = backup {
            let _ = fs::remove_file(backup);
        }
    }
    result?;
    Ok(writes
        .iter()
        .map(|write| write.target_path.clone())
        .collect())
}

fn allowed_document_field(kind: FixModificationKind, field_path: &str) -> bool {
    let segments: Vec<_> = field_path.split('.').collect();
    if segments.len() != 2 || segments.iter().any(|segment| !safe_field_segment(segment)) {
        return false;
    }
    match kind {
        FixModificationKind::UiDocumentLayout => {
            segments[0] == "layout"
                && matches!(
                    segments[1],
                    "display"
                        | "position"
                        | "direction"
                        | "width"
                        | "height"
                        | "min_width"
                        | "min_height"
                        | "max_width"
                        | "max_height"
                        | "aspect_ratio"
                        | "margin"
                        | "padding"
                        | "border"
                        | "gap"
                        | "row_gap"
                        | "column_gap"
                        | "align_items"
                        | "justify_items"
                        | "align_self"
                        | "justify_self"
                        | "align_content"
                        | "justify_content"
                        | "wrap"
                        | "flex_grow"
                        | "flex_shrink"
                        | "flex_basis"
                        | "overflow"
                        | "scrollbar_width"
                        | "z_index"
                        | "grid_columns"
                        | "grid_rows"
                        | "grid_auto_columns"
                        | "grid_auto_rows"
                        | "grid_auto_flow"
                        | "grid_column"
                        | "grid_row"
                )
        }
        FixModificationKind::UiDocumentScopedToken => {
            segments[0] == "style"
                && matches!(segments[1], "component" | "role" | "text_role" | "inline")
        }
        _ => false,
    }
}

fn set_document_field(node: &mut Value, field_path: &str, value: Value) -> Result<(), TaskFailure> {
    let segments: Vec<_> = field_path.split('.').collect();
    let object = node
        .as_object_mut()
        .ok_or_else(|| invalid("UiDocument node is not an object"))?;
    let group = object
        .entry(segments[0].to_owned())
        .or_insert_with(|| Value::Object(Map::new()));
    let group = group
        .as_object_mut()
        .ok_or_else(|| invalid("UiDocument patch field group is not an object"))?;
    group.insert(segments[1].to_owned(), value);
    Ok(())
}

fn count_nodes(value: &Value, node_id: &str) -> usize {
    let here = value
        .get("id")
        .and_then(Value::as_str)
        .is_some_and(|id| id == node_id) as usize;
    here + value
        .get("children")
        .and_then(Value::as_array)
        .map(|children| {
            children
                .iter()
                .map(|child| count_nodes(child, node_id))
                .sum()
        })
        .unwrap_or(0)
}

fn find_node_mut<'a>(value: &'a mut Value, node_id: &str) -> Option<&'a mut Value> {
    if value
        .get("id")
        .and_then(Value::as_str)
        .is_some_and(|id| id == node_id)
    {
        return Some(value);
    }
    let children = value.get_mut("children")?.as_array_mut()?;
    for child in children {
        if let Some(found) = find_node_mut(child, node_id) {
            return Some(found);
        }
    }
    None
}

fn enforce_action_budget(
    action: &FixPlanAction,
    changed_lines: u32,
    asset_bytes: u64,
) -> Result<(), TaskFailure> {
    if changed_lines > action.estimated_diff_lines
        || action
            .estimated_asset_bytes
            .is_some_and(|limit| asset_bytes > limit)
    {
        return Err(invalid(
            "actual patch scope exceeds the exact diff or asset budget in the fix plan",
        ));
    }
    Ok(())
}

fn versioned_asset_path(target_file: &str, source_sha256: &str) -> Result<String, TaskFailure> {
    let path = Path::new(target_file);
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| invalid("draft asset target has no safe file name"))?;
    let (stem, extension) = file_name
        .rsplit_once('.')
        .ok_or_else(|| invalid("draft asset target needs a file extension for versioning"))?;
    if stem.is_empty()
        || extension.is_empty()
        || !safe_field_segment(stem)
        || !safe_field_segment(extension)
    {
        return Err(invalid("draft asset target name is unsafe"));
    }
    let parent = path.parent().and_then(Path::to_str).unwrap_or_default();
    let file_name = format!("{stem}.v{}.{}", &source_sha256[..12], extension);
    Ok(if parent.is_empty() {
        file_name
    } else {
        format!("{parent}/{file_name}")
    })
}

fn apply_single_file_unified_diff(
    source: &str,
    diff: &str,
    target_file: &str,
) -> Result<String, TaskFailure> {
    let mut lines = diff.lines();
    let old_header = lines
        .next()
        .ok_or_else(|| invalid("unified diff lacks old-file header"))?;
    let new_header = lines
        .next()
        .ok_or_else(|| invalid("unified diff lacks new-file header"))?;
    if old_header != format!("--- a/{target_file}") || new_header != format!("+++ b/{target_file}")
    {
        return Err(invalid(
            "unified diff headers must name the exact plan allow-listed Rust target",
        ));
    }
    let source_lines: Vec<_> = source.split('\n').map(str::to_owned).collect();
    let mut output = Vec::new();
    let mut source_index = 0usize;
    let remaining: Vec<_> = lines.collect();
    let mut position = 0usize;
    let mut hunk_count = 0usize;
    while position < remaining.len() {
        let header = remaining[position];
        let (old_start, _) = parse_hunk_header(header)?;
        hunk_count += 1;
        let target_index = old_start.saturating_sub(1);
        if target_index < source_index || target_index > source_lines.len() {
            return Err(conflict(
                "unified diff hunks are unordered or outside the Rust source",
            ));
        }
        output.extend_from_slice(&source_lines[source_index..target_index]);
        source_index = target_index;
        position += 1;
        while position < remaining.len() && !remaining[position].starts_with("@@ ") {
            let line = remaining[position];
            let (marker, content) = line.split_at(1);
            match marker {
                " " => {
                    if source_lines.get(source_index).map(String::as_str) != Some(content) {
                        return Err(conflict(
                            "unified diff context does not match the current Rust source",
                        ));
                    }
                    output.push(content.to_owned());
                    source_index += 1;
                }
                "-" => {
                    if source_lines.get(source_index).map(String::as_str) != Some(content) {
                        return Err(conflict(
                            "unified diff removal does not match the current Rust source",
                        ));
                    }
                    source_index += 1;
                }
                "+" => output.push(content.to_owned()),
                _ => return Err(invalid("unified diff contains an unsupported line marker")),
            }
            position += 1;
        }
    }
    if hunk_count == 0 {
        return Err(invalid("unified diff contains no hunks"));
    }
    output.extend_from_slice(&source_lines[source_index..]);
    Ok(output.join("\n"))
}

fn parse_hunk_header(header: &str) -> Result<(usize, usize), TaskFailure> {
    if !header.starts_with("@@ -") || !header.ends_with(" @@") {
        return Err(invalid("unified diff hunk header is malformed"));
    }
    let body = &header[4..header.len() - 3];
    let (old, new) = body
        .split_once(" +")
        .ok_or_else(|| invalid("unified diff hunk header is malformed"))?;
    let parse_range = |value: &str| -> Result<usize, TaskFailure> {
        value
            .split_once(',')
            .map(|(start, _)| start)
            .unwrap_or(value)
            .parse::<usize>()
            .map_err(|_| invalid("unified diff hunk range is invalid"))
    };
    Ok((parse_range(old)?, parse_range(new)?))
}

fn render_full_unified_diff(target_file: &str, before: Option<&[u8]>, after: &[u8]) -> String {
    let before = before
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .unwrap_or_default();
    let after = std::str::from_utf8(after).unwrap_or("<binary data>");
    let mut output = format!("--- a/{target_file}\n+++ b/{target_file}\n");
    for line in before.lines() {
        output.push('-');
        output.push_str(line);
        output.push('\n');
    }
    for line in after.lines() {
        output.push('+');
        output.push_str(line);
        output.push('\n');
    }
    output
}

fn changed_line_count(before: &[u8], after: &[u8]) -> Result<u32, TaskFailure> {
    let before = std::str::from_utf8(before).map_err(|_| invalid("changed source is not UTF-8"))?;
    let after = std::str::from_utf8(after).map_err(|_| invalid("changed source is not UTF-8"))?;
    let before: Vec<_> = before.lines().collect();
    let after: Vec<_> = after.lines().collect();
    let mut prefix = 0usize;
    while prefix < before.len() && prefix < after.len() && before[prefix] == after[prefix] {
        prefix += 1;
    }
    let mut suffix = 0usize;
    while suffix < before.len() - prefix
        && suffix < after.len() - prefix
        && before[before.len() - suffix - 1] == after[after.len() - suffix - 1]
    {
        suffix += 1;
    }
    u32::try_from((before.len() - prefix - suffix) + (after.len() - prefix - suffix))
        .map_err(|_| invalid("changed line count overflows the closed-loop budget"))
}

fn resolve_existing_regular_file(root: &Path, relative: &str) -> Result<PathBuf, TaskFailure> {
    let path = resolve_path(root, relative)?;
    let metadata = fs::symlink_metadata(&path)
        .map_err(|_| invalid("closed-loop patch target must already be a regular file"))?;
    if metadata_is_reparse(&metadata) || !metadata.is_file() {
        return Err(invalid("closed-loop patch target is not a regular file"));
    }
    Ok(path)
}

fn resolve_new_regular_file(root: &Path, relative: &str) -> Result<PathBuf, TaskFailure> {
    let path = resolve_path(root, relative)?;
    if fs::symlink_metadata(&path).is_ok() {
        return Err(conflict(
            "closed-loop versioned output already exists and will not be overwritten",
        ));
    }
    Ok(path)
}

fn resolve_path(root: &Path, relative: &str) -> Result<PathBuf, TaskFailure> {
    if !safe_relative_path(relative) || protected_path(relative) {
        return Err(invalid("closed-loop path is unsafe or protected"));
    }
    let mut current = root.to_path_buf();
    for component in Path::new(relative).components() {
        let Component::Normal(component) = component else {
            return Err(invalid("closed-loop path contains a non-normal component"));
        };
        current.push(component);
        if let Ok(metadata) = fs::symlink_metadata(&current) {
            if metadata_is_reparse(&metadata) {
                return Err(invalid(
                    "closed-loop path cannot traverse a symlink or reparse point",
                ));
            }
        }
    }
    let parent = current
        .parent()
        .ok_or_else(|| invalid("closed-loop path has no parent"))?;
    let parent = canonical_regular_directory(parent, "closed-loop target parent")?;
    if !parent.starts_with(root) {
        return Err(invalid("closed-loop target parent escapes repository root"));
    }
    Ok(current)
}

fn canonical_regular_directory(path: &Path, label: &str) -> Result<PathBuf, TaskFailure> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_| invalid(format!("{label} cannot be inspected")))?;
    if metadata_is_reparse(&metadata) || !metadata.is_dir() {
        return Err(invalid(format!(
            "{label} must be a regular directory without reparse points"
        )));
    }
    let canonical =
        fs::canonicalize(path).map_err(|_| invalid(format!("{label} cannot be resolved")))?;
    if !fs::metadata(&canonical).is_ok_and(|metadata| metadata.is_dir()) {
        return Err(invalid(format!("{label} is not a regular directory")));
    }
    Ok(canonical)
}

fn path_in_allowed_roots(path: &str, plan: &ClosedLoopFixPlan) -> bool {
    safe_relative_path(path)
        && plan
            .policy
            .allowed_roots
            .iter()
            .filter_map(|root| normalize_root(root))
            .any(|root| path.starts_with(&root))
}

fn normalize_root(value: &str) -> Option<String> {
    let value = value.trim().trim_end_matches('/');
    safe_relative_path(value).then(|| format!("{value}/"))
}

fn protected_path(path: &str) -> bool {
    let protected = [
        "reference",
        "baseline",
        "mask",
        "threshold",
        "credential",
        "secret",
        ".env",
        ".git",
        "run-ui-audit",
        "prompt",
        "cargo.toml",
        "cargo.lock",
        "security",
        "safety",
        "policy",
    ];
    path.to_ascii_lowercase().split('/').any(|part| {
        protected
            .iter()
            .any(|word| part == *word || part.starts_with(word))
    })
}

fn safe_relative_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 320
        && value.is_ascii()
        && !value.starts_with('/')
        && !value.contains(['\\', ':', '\0', '\n', '\r'])
        && value.split('/').all(safe_field_segment)
}

fn safe_field_segment(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'$'))
}

fn safe_label(value: &str) -> bool {
    value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn safe_node_id(value: &str) -> bool {
    value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn bounded_text(value: &str) -> bool {
    !value.is_empty() && value.len() <= 1_024 && !value.contains(['\0', '\n', '\r'])
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn metadata_is_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path, label: &str) -> Result<T, TaskFailure> {
    let bytes = read_regular_file(path, MAX_INPUT_BYTES, label)?;
    serde_json::from_slice(&bytes)
        .map_err(|_| invalid(format!("{label} is malformed or contains unknown fields")))
}

fn read_regular_file(path: &Path, maximum: usize, label: &str) -> Result<Vec<u8>, TaskFailure> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_| invalid(format!("{label} cannot be inspected")))?;
    if metadata_is_reparse(&metadata) || !metadata.is_file() || metadata.len() > maximum as u64 {
        return Err(invalid(format!("{label} is not a bounded regular file")));
    }
    let bytes = fs::read(path).map_err(|_| invalid(format!("{label} cannot be read")))?;
    if bytes.len() > maximum
        || fs::symlink_metadata(path)
            .ok()
            .is_none_or(|current| current.len() != metadata.len() || metadata_is_reparse(&current))
    {
        return Err(invalid(format!("{label} changed while it was read")));
    }
    Ok(bytes)
}

fn temporary_sibling(path: &Path, purpose: &str) -> Result<PathBuf, TaskFailure> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid("closed-loop write has no parent"))?;
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| invalid("closed-loop write has no safe name"))?;
    let unique = STAGING_COUNTER.fetch_add(1, Ordering::Relaxed);
    Ok(parent.join(format!(
        ".{name}.ui-generation-{purpose}-{}-{unique}",
        std::process::id()
    )))
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), TaskFailure> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| write_failure(path, "create staged output", error))?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|error| write_failure(path, "persist staged output", error))
}

fn now_epoch_seconds() -> Result<u64, TaskFailure> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_| invalid("system clock is before the Unix epoch"))
}

fn patch_action_id(patch: &ClosedLoopPatch) -> &str {
    match patch {
        ClosedLoopPatch::UiDocument { action_id, .. }
        | ClosedLoopPatch::DraftAssetVersion { action_id, .. }
        | ClosedLoopPatch::RustUnifiedDiff { action_id, .. } => action_id,
    }
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn hash_json(value: &Value) -> Result<String, TaskFailure> {
    serde_json::to_vec(value)
        .map(|bytes| hash_bytes(&bytes))
        .map_err(|_| invalid("closed-loop preview cannot be serialized"))
}

fn invalid(message: impl Into<String>) -> TaskFailure {
    TaskFailure::new(TaskFailureKind::FixPlanRejected, message, None)
}

fn conflict(message: impl Into<String>) -> TaskFailure {
    TaskFailure::new(TaskFailureKind::FixPlanRejected, message, None)
}

fn write_failure(path: &Path, action: &str, error: std::io::Error) -> TaskFailure {
    TaskFailure::new(
        TaskFailureKind::FixPlanRejected,
        format!("closed-loop apply could not {action}: {error}"),
        Some(path.display().to_string()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::closed_loop_fix_plan::{
        FixAttribution, FixPlanPolicySummary, FixPlanRisk, FixTarget, FixVerificationMatrix,
    };
    use std::fs;
    use tempfile::TempDir;

    fn fixture_root() -> TempDir {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("draft/assets")).unwrap();
        fs::create_dir_all(root.path().join("project/src/framework/ui")).unwrap();
        let document = canonicalize_json(
            r#"{"schema_version":1,"document_id":"apply.fixture","assets":{},"tokens":{},"root":{"type":"container","id":"page.root","layout":{"width":{"percent":100}},"children":[]}}"#,
        )
        .unwrap();
        fs::write(root.path().join("draft/page.json"), document).unwrap();
        fs::write(root.path().join("draft/assets/old.png"), b"old asset").unwrap();
        fs::write(root.path().join("draft/assets/new.png"), b"new asset").unwrap();
        fs::write(
            root.path().join("project/src/framework/ui/widget.rs"),
            "pub fn paint() {\n    old();\n}\n",
        )
        .unwrap();
        root
    }

    fn action(id: &str, target: &str, modification: FixModificationKind) -> FixPlanAction {
        FixPlanAction {
            action_id: id.to_owned(),
            group_id: format!("group-{id}"),
            attribution: FixAttribution::DocumentLayout,
            target: FixTarget {
                target_file: target.to_owned(),
                document_path: (modification == FixModificationKind::UiDocumentLayout)
                    .then_some("layout.width".to_owned()),
                node_id: (modification == FixModificationKind::UiDocumentLayout)
                    .then_some("page.root".to_owned()),
            },
            modification,
            expected_effect: "fixture".to_owned(),
            verification: FixVerificationMatrix {
                captures: Vec::new(),
                rerun_group_captures_first: true,
                rerun_all_related_device_states: true,
                rerun_shared_scope_consumers: modification != FixModificationKind::UiDocumentLayout
                    && modification != FixModificationKind::DraftAssetVersion,
            },
            estimated_diff_lines: 64,
            estimated_asset_bytes: (modification == FixModificationKind::DraftAssetVersion)
                .then_some(1024),
            risk: FixPlanRisk::Low,
            requires_approval: matches!(
                modification,
                FixModificationKind::CommonWidget
                    | FixModificationKind::Theme
                    | FixModificationKind::Framework
            ),
            may_regress_other_device_states: true,
        }
    }

    fn plan(actions: Vec<FixPlanAction>) -> ClosedLoopFixPlan {
        ClosedLoopFixPlan {
            protocol_version: CLOSED_LOOP_FIX_PLAN_PROTOCOL_VERSION,
            run_id: "apply-fixture".to_owned(),
            audit_schema_version: 1,
            status: FixPlanStatus::ReadyForApply,
            requires_approval: false,
            policy: FixPlanPolicySummary {
                allowed_roots: vec!["draft".to_owned(), "project/src".to_owned()],
                forbidden_categories: Vec::new(),
                max_files: 8,
                max_diff_lines: 480,
                max_asset_bytes: 1024 * 1024,
                dependency_changes_allowed: false,
            },
            actions,
            rejected_groups: Vec::new(),
            findings: Vec::new(),
        }
    }

    fn write_plan_and_patches(
        root: &Path,
        plan: &ClosedLoopFixPlan,
        patches: Vec<ClosedLoopPatch>,
    ) -> (PathBuf, PathBuf) {
        let plan_path = root.join("plan.json");
        let plan_bytes = serde_json::to_vec_pretty(plan).unwrap();
        fs::write(&plan_path, &plan_bytes).unwrap();
        let patches_path = root.join("patches.json");
        fs::write(
            &patches_path,
            serde_json::to_vec_pretty(&ClosedLoopPatchSet {
                protocol_version: CLOSED_LOOP_APPLY_PROTOCOL_VERSION,
                run_id: plan.run_id.clone(),
                plan_sha256: hash_bytes(&plan_bytes),
                patches,
            })
            .unwrap(),
        )
        .unwrap();
        (plan_path, patches_path)
    }

    fn approved(preview: &ClosedLoopApplyPreview) -> ClosedLoopApplyApproval {
        ClosedLoopApplyApproval {
            protocol_version: CLOSED_LOOP_APPLY_PROTOCOL_VERSION,
            approval_id: "approval-1".to_owned(),
            run_id: preview.run_id.clone(),
            plan_sha256: preview.plan_sha256.clone(),
            patch_set_sha256: preview.patch_set_sha256.clone(),
            preview_sha256: preview.preview_sha256.clone(),
            approved_by: "reviewer".to_owned(),
            decision: "approved".to_owned(),
            issued_at_epoch_seconds: 10,
            expires_at_epoch_seconds: 100,
        }
    }

    #[test]
    fn structured_document_patch_updates_only_the_bound_node_and_field_after_approval() {
        let root = fixture_root();
        let plan = plan(vec![action(
            "layout",
            "draft/page.json",
            FixModificationKind::UiDocumentLayout,
        )]);
        let (plan_path, patches_path) = write_plan_and_patches(
            root.path(),
            &plan,
            vec![ClosedLoopPatch::UiDocument {
                action_id: "layout".to_owned(),
                target_file: "draft/page.json".to_owned(),
                node_id: "page.root".to_owned(),
                field_path: "layout.width".to_owned(),
                value: serde_json::json!({ "percent": 80 }),
            }],
        );
        let preview = preview_closed_loop_apply(root.path(), &plan_path, &patches_path).unwrap();
        assert_eq!(preview.promotion_scope.documents, vec!["draft/page.json"]);
        let approval_path = root.path().join("approval.json");
        fs::write(
            &approval_path,
            serde_json::to_vec(&approved(&preview)).unwrap(),
        )
        .unwrap();
        // Use a future clock-independent approval record by updating it around the actual now.
        let mut approval = approved(&preview);
        approval.issued_at_epoch_seconds = now_epoch_seconds().unwrap();
        approval.expires_at_epoch_seconds = approval.issued_at_epoch_seconds + 60;
        fs::write(&approval_path, serde_json::to_vec(&approval).unwrap()).unwrap();
        apply_closed_loop_patches(root.path(), &plan_path, &patches_path, &approval_path).unwrap();
        let result: Value =
            serde_json::from_slice(&fs::read(root.path().join("draft/page.json")).unwrap())
                .unwrap();
        assert_eq!(
            result.pointer("/root/layout/width"),
            Some(&serde_json::json!({ "percent": 80.0 }))
        );
    }

    #[test]
    fn patch_conflict_or_out_of_scope_target_leaves_draft_unchanged() {
        let root = fixture_root();
        let original = fs::read(root.path().join("draft/page.json")).unwrap();
        let plan = plan(vec![action(
            "layout",
            "draft/page.json",
            FixModificationKind::UiDocumentLayout,
        )]);
        let (plan_path, patches_path) = write_plan_and_patches(
            root.path(),
            &plan,
            vec![ClosedLoopPatch::UiDocument {
                action_id: "layout".to_owned(),
                target_file: "draft/other.json".to_owned(),
                node_id: "page.root".to_owned(),
                field_path: "layout.width".to_owned(),
                value: serde_json::json!({ "percent": 80 }),
            }],
        );
        assert!(preview_closed_loop_apply(root.path(), &plan_path, &patches_path).is_err());
        assert_eq!(
            fs::read(root.path().join("draft/page.json")).unwrap(),
            original
        );
    }

    #[test]
    fn asset_version_preserves_previous_hash_and_rejects_overwrite() {
        let root = fixture_root();
        let plan = plan(vec![action(
            "asset",
            "draft/assets/old.png",
            FixModificationKind::DraftAssetVersion,
        )]);
        let source_sha256 = hash_bytes(b"new asset");
        let (plan_path, patches_path) = write_plan_and_patches(
            root.path(),
            &plan,
            vec![ClosedLoopPatch::DraftAssetVersion {
                action_id: "asset".to_owned(),
                target_file: "draft/assets/old.png".to_owned(),
                replacement_source: "draft/assets/new.png".to_owned(),
                source_sha256: source_sha256.clone(),
                source_provenance: "fixture generator".to_owned(),
                license_reference: "project-owned".to_owned(),
            }],
        );
        let preview = preview_closed_loop_apply(root.path(), &plan_path, &patches_path).unwrap();
        assert_eq!(preview.promotion_scope.resources.len(), 2);
        let versioned = versioned_asset_path("draft/assets/old.png", &source_sha256).unwrap();
        assert!(!root.path().join(&versioned).exists());
        let mut approval = approved(&preview);
        approval.issued_at_epoch_seconds = now_epoch_seconds().unwrap();
        approval.expires_at_epoch_seconds = approval.issued_at_epoch_seconds + 60;
        let approval_path = root.path().join("approval.json");
        fs::write(&approval_path, serde_json::to_vec(&approval).unwrap()).unwrap();
        apply_closed_loop_patches(root.path(), &plan_path, &patches_path, &approval_path).unwrap();
        assert_eq!(
            fs::read(root.path().join("draft/assets/old.png")).unwrap(),
            b"old asset"
        );
        assert_eq!(
            fs::read(root.path().join(&versioned)).unwrap(),
            b"new asset"
        );
        assert!(preview_closed_loop_apply(root.path(), &plan_path, &patches_path).is_err());
    }

    #[test]
    fn rust_unified_diff_requires_exact_approved_patch_and_rejects_expired_approval() {
        let root = fixture_root();
        let mut shared = action(
            "rust",
            "project/src/framework/ui/widget.rs",
            FixModificationKind::CommonWidget,
        );
        shared.estimated_diff_lines = 4;
        let plan = plan(vec![shared]);
        let diff = "--- a/project/src/framework/ui/widget.rs\n+++ b/project/src/framework/ui/widget.rs\n@@ -1,3 +1,3 @@\n pub fn paint() {\n-    old();\n+    new();\n }";
        let (plan_path, patches_path) = write_plan_and_patches(
            root.path(),
            &plan,
            vec![ClosedLoopPatch::RustUnifiedDiff {
                action_id: "rust".to_owned(),
                target_file: "project/src/framework/ui/widget.rs".to_owned(),
                unified_diff: diff.to_owned(),
            }],
        );
        let preview = preview_closed_loop_apply(root.path(), &plan_path, &patches_path).unwrap();
        let approval_path = root.path().join("approval.json");
        let mut approval = approved(&preview);
        let now = now_epoch_seconds().unwrap();
        approval.issued_at_epoch_seconds = now - 10;
        approval.expires_at_epoch_seconds = now - 1;
        fs::write(&approval_path, serde_json::to_vec(&approval).unwrap()).unwrap();
        assert!(
            apply_closed_loop_patches(root.path(), &plan_path, &patches_path, &approval_path)
                .is_err()
        );
        assert!(
            fs::read_to_string(root.path().join("project/src/framework/ui/widget.rs"))
                .unwrap()
                .contains("old();")
        );
        let mut approval = approved(&preview);
        approval.issued_at_epoch_seconds = now_epoch_seconds().unwrap();
        approval.expires_at_epoch_seconds = approval.issued_at_epoch_seconds + 60;
        fs::write(&approval_path, serde_json::to_vec(&approval).unwrap()).unwrap();
        apply_closed_loop_patches(root.path(), &plan_path, &patches_path, &approval_path).unwrap();
        assert!(
            fs::read_to_string(root.path().join("project/src/framework/ui/widget.rs"))
                .unwrap()
                .contains("new();")
        );
    }

    #[test]
    fn partial_write_preflight_failure_does_not_write_any_target() {
        let root = fixture_root();
        let plan = plan(vec![
            action(
                "layout",
                "draft/page.json",
                FixModificationKind::UiDocumentLayout,
            ),
            action(
                "asset",
                "draft/assets/old.png",
                FixModificationKind::DraftAssetVersion,
            ),
        ]);
        let before_document = fs::read(root.path().join("draft/page.json")).unwrap();
        let before_asset = fs::read(root.path().join("draft/assets/old.png")).unwrap();
        let (plan_path, patches_path) = write_plan_and_patches(
            root.path(),
            &plan,
            vec![
                ClosedLoopPatch::UiDocument {
                    action_id: "layout".to_owned(),
                    target_file: "draft/page.json".to_owned(),
                    node_id: "page.root".to_owned(),
                    field_path: "layout.width".to_owned(),
                    value: serde_json::json!({ "percent": 80 }),
                },
                ClosedLoopPatch::DraftAssetVersion {
                    action_id: "asset".to_owned(),
                    target_file: "draft/assets/old.png".to_owned(),
                    replacement_source: "draft/assets/new.png".to_owned(),
                    source_sha256: "0".repeat(64),
                    source_provenance: "fixture".to_owned(),
                    license_reference: "project-owned".to_owned(),
                },
            ],
        );
        assert!(preview_closed_loop_apply(root.path(), &plan_path, &patches_path).is_err());
        assert_eq!(
            fs::read(root.path().join("draft/page.json")).unwrap(),
            before_document
        );
        assert_eq!(
            fs::read(root.path().join("draft/assets/old.png")).unwrap(),
            before_asset
        );
    }
}
