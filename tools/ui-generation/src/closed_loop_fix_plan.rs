//! Bounded repair planning for the closed UI audit loop.
//!
//! A fix plan is evidence, not a patch. It turns the Stage 4 audit report into
//! a small set of typed, allow-listed proposals that Stage 6 can apply without
//! rediscovering ownership or accepting an analyzer-supplied command. Any
//! ambiguous, protected, or business-owned group remains an explicit manual
//! outcome instead of being guessed.

use crate::{
    directory::RunId,
    lifecycle::{TaskFailure, TaskFailureKind},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

pub const CLOSED_LOOP_FIX_PLAN_PROTOCOL_VERSION: u32 = 1;
const MAX_AUDIT_BYTES: usize = 2 * 1024 * 1024;
const MAX_ISSUES: usize = 2_048;
const MAX_GROUPS: usize = 512;
const MAX_ACTIONS: usize = 64;
const DEFAULT_MAX_FILES: usize = 8;
const DEFAULT_MAX_DIFF_LINES: u32 = 480;
const DEFAULT_MAX_ASSET_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FixPlanStatus {
    ReadyForApply,
    AwaitingApproval,
    NoAvailableFix,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FixAttribution {
    DocumentLayout,
    DocumentStyle,
    DraftAsset,
    BusinessContent,
    CommonWidget,
    Theme,
    Framework,
    ReferenceOrRule,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FixModificationKind {
    UiDocumentLayout,
    UiDocumentScopedToken,
    DraftAssetVersion,
    CommonWidget,
    Theme,
    Framework,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FixPlanRejectionCode {
    ManualReviewRequired,
    BusinessContentRequiresHumanReview,
    ProtectedTarget,
    MissingDocumentBinding,
    MissingDraftAssetTarget,
    AmbiguousTarget,
    ProtocolEscalationNotConfirmed,
    EscalationNotMultiPage,
    TargetOutsideAllowedRoots,
    ForbiddenTarget,
    FileLimitExceeded,
    DiffBudgetExceeded,
    AssetBudgetExceeded,
    DependencyChangeForbidden,
    DuplicateIneffectiveRepair,
    ConflictingRepair,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FixPlanRisk {
    Low,
    High,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClosedLoopAuditReport {
    pub schema_version: u32,
    pub run_id: String,
    pub status: String,
    pub priority_order: Vec<String>,
    #[serde(default)]
    pub hard_issues: Vec<AuditIssue>,
    #[serde(default)]
    pub visual_issues: Vec<AuditIssue>,
    #[serde(default)]
    pub ai_issues: Vec<AuditIssue>,
    pub issue_groups: Vec<AuditIssueGroup>,
    #[serde(default)]
    pub manual_review_group_ids: Vec<String>,
    pub source_map_bound: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditIssue {
    pub issue_id: String,
    pub source: String,
    pub priority: String,
    pub screen: String,
    pub device: String,
    pub state: String,
    pub region: AuditRegion,
    pub evidence: Vec<AuditEvidence>,
    #[serde(default)]
    pub document: Option<AuditDocumentBinding>,
    pub problem_type: String,
    pub message: String,
    pub severity: String,
    pub blocking: bool,
    #[serde(default)]
    pub likely_cause: Option<String>,
    #[serde(default)]
    pub likely_files: Vec<String>,
    pub attribution: FixAttribution,
    pub requires_manual_review: bool,
    pub automatic_fix_allowed: bool,
    #[serde(default)]
    pub manual_review_reason: Option<String>,
    #[serde(default)]
    pub protected_targets: Vec<String>,
    pub root_cause_key: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditRegion {
    pub region_id: String,
    #[serde(default)]
    pub bounds: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditEvidence {
    pub kind: String,
    pub artifact: AuditArtifact,
    pub description: String,
    #[serde(default)]
    pub image_role: Option<String>,
    #[serde(default)]
    pub image: Option<AuditArtifact>,
    #[serde(default)]
    pub image_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditArtifact {
    pub path: String,
    pub sha256: String,
    #[serde(default)]
    pub byte_length: Option<u64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditDocumentBinding {
    #[serde(default)]
    pub document_id: Option<String>,
    #[serde(default)]
    pub node_id: Option<String>,
    #[serde(default)]
    pub source_path: Option<String>,
    #[serde(default)]
    pub document_path: Option<String>,
    #[serde(default)]
    pub reference_element: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditIssueGroup {
    pub group_id: String,
    pub root_cause_key: String,
    pub attribution: FixAttribution,
    pub requires_manual_review: bool,
    pub automatic_fix_allowed: bool,
    pub priorities: Vec<String>,
    pub issue_ids: Vec<String>,
    pub captures: Vec<AuditCapture>,
    pub evidence: Vec<AuditEvidence>,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AuditCapture {
    pub screen: String,
    pub device: String,
    pub state: String,
}

#[derive(Clone, Debug)]
pub struct FixPlanPolicy {
    pub allowed_roots: Vec<String>,
    pub max_files: usize,
    pub max_diff_lines: u32,
    pub max_asset_bytes: u64,
    pub protocol_limitations: BTreeSet<String>,
}

impl Default for FixPlanPolicy {
    fn default() -> Self {
        Self {
            allowed_roots: vec!["draft/".to_owned(), "assets/".to_owned()],
            max_files: DEFAULT_MAX_FILES,
            max_diff_lines: DEFAULT_MAX_DIFF_LINES,
            max_asset_bytes: DEFAULT_MAX_ASSET_BYTES,
            protocol_limitations: BTreeSet::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FixPlanPolicySummary {
    pub allowed_roots: Vec<String>,
    pub forbidden_categories: Vec<String>,
    pub max_files: usize,
    pub max_diff_lines: u32,
    pub max_asset_bytes: u64,
    pub dependency_changes_allowed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FixTarget {
    pub target_file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FixVerificationMatrix {
    pub captures: Vec<AuditCapture>,
    pub rerun_group_captures_first: bool,
    pub rerun_all_related_device_states: bool,
    pub rerun_shared_scope_consumers: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FixPlanAction {
    pub action_id: String,
    pub group_id: String,
    pub attribution: FixAttribution,
    pub target: FixTarget,
    pub modification: FixModificationKind,
    pub expected_effect: String,
    pub verification: FixVerificationMatrix,
    pub estimated_diff_lines: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_asset_bytes: Option<u64>,
    pub risk: FixPlanRisk,
    pub requires_approval: bool,
    pub may_regress_other_device_states: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FixPlanRejection {
    pub group_id: String,
    pub code: FixPlanRejectionCode,
    pub detail: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FixPlanFinding {
    pub code: String,
    pub detail: String,
    pub group_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClosedLoopFixPlan {
    pub protocol_version: u32,
    pub run_id: String,
    pub audit_schema_version: u32,
    pub status: FixPlanStatus,
    pub requires_approval: bool,
    pub policy: FixPlanPolicySummary,
    pub actions: Vec<FixPlanAction>,
    pub rejected_groups: Vec<FixPlanRejection>,
    pub findings: Vec<FixPlanFinding>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WrittenFixPlan {
    pub plan_path: PathBuf,
    pub markdown_path: PathBuf,
    pub plan_sha256: String,
}

pub fn load_closed_loop_audit(path: &Path) -> Result<ClosedLoopAuditReport, TaskFailure> {
    let bytes = fs::read(path).map_err(|_| {
        TaskFailure::new(
            TaskFailureKind::ArtifactMissing,
            "closed-loop audit report cannot be read",
            Some(path.display().to_string()),
        )
    })?;
    if bytes.len() > MAX_AUDIT_BYTES {
        return Err(rejected("closed-loop audit report exceeds its byte budget"));
    }
    serde_json::from_slice(&bytes).map_err(|_| rejected("closed-loop audit report is malformed"))
}

pub fn create_closed_loop_fix_plan(
    report: &ClosedLoopAuditReport,
    policy: &FixPlanPolicy,
) -> Result<ClosedLoopFixPlan, TaskFailure> {
    validate_policy(policy)?;
    validate_audit_report(report)?;

    let issues = issues_by_id(report)?;
    let mut candidates = Vec::new();
    let mut rejections = Vec::new();
    let mut findings = Vec::new();

    for group in &report.issue_groups {
        let (group_issues, trusted_captures) = group_issues(group, &issues)?;
        match candidate_for_group(group, &group_issues, &trusted_captures, policy) {
            Ok(Some(action)) => {
                findings.push(FixPlanFinding {
                    code: "FIX_PLAN_DEVICE_STATE_REGRESSION_GUARD".to_owned(),
                    detail: format!(
                        "{} requires its affected capture matrix to be rerun before approval or application",
                        group.group_id
                    ),
                    group_id: group.group_id.clone(),
                });
                candidates.push(action);
            }
            Ok(None) => {}
            Err(rejection) => rejections.push(rejection),
        }
    }

    resolve_duplicate_and_conflicting_actions(&mut candidates, &mut rejections);
    enforce_plan_budgets(&mut candidates, &mut rejections, policy)?;
    candidates.sort_by(|left, right| left.action_id.cmp(&right.action_id));
    rejections.sort_by(|left, right| {
        left.group_id
            .cmp(&right.group_id)
            .then_with(|| left.code.cmp(&right.code))
    });
    findings.sort_by(|left, right| {
        left.group_id
            .cmp(&right.group_id)
            .then_with(|| left.code.cmp(&right.code))
    });

    let requires_approval = candidates.iter().any(|action| action.requires_approval);
    let status = if candidates.is_empty() {
        FixPlanStatus::NoAvailableFix
    } else if requires_approval {
        FixPlanStatus::AwaitingApproval
    } else {
        FixPlanStatus::ReadyForApply
    };
    Ok(ClosedLoopFixPlan {
        protocol_version: CLOSED_LOOP_FIX_PLAN_PROTOCOL_VERSION,
        run_id: report.run_id.clone(),
        audit_schema_version: report.schema_version,
        status,
        requires_approval,
        policy: policy_summary(policy),
        actions: candidates,
        rejected_groups: rejections,
        findings,
    })
}

pub fn write_closed_loop_fix_plan(
    plan: &ClosedLoopFixPlan,
    output_directory: &Path,
) -> Result<WrittenFixPlan, TaskFailure> {
    let output_directory = create_regular_output_directory(output_directory)?;
    let json = serde_json::to_vec_pretty(plan)
        .map_err(|_| rejected("closed-loop fix plan cannot be serialized"))?;
    let markdown = render_fix_plan_markdown(plan).into_bytes();
    let plan_path = output_directory.join("fix-plan.json");
    let markdown_path = output_directory.join("fix-plan.md");
    write_new_file(&plan_path, &json)?;
    if let Err(error) = write_new_file(&markdown_path, &markdown) {
        // The JSON is durable evidence even if the convenience rendering fails. Never replace it.
        return Err(error);
    }
    Ok(WrittenFixPlan {
        plan_path,
        markdown_path,
        plan_sha256: hash_bytes(&json),
    })
}

pub fn render_fix_plan_markdown(plan: &ClosedLoopFixPlan) -> String {
    let mut output = String::new();
    output.push_str("# Closed-loop UI Fix Plan\n\n");
    output.push_str(&format!("- Run: `{}`\n", plan.run_id));
    output.push_str(&format!("- Status: `{}`\n", plan_status_name(plan.status)));
    output.push_str(&format!(
        "- Requires approval: `{}`\n",
        plan.requires_approval
    ));
    output.push_str(&format!("- Actions: `{}`\n\n", plan.actions.len()));
    output.push_str("## Actions\n\n");
    if plan.actions.is_empty() {
        output.push_str("No safe automatic repair is available.\n\n");
    } else {
        for action in &plan.actions {
            output.push_str(&format!("### `{}`\n\n", action.action_id));
            output.push_str(&format!("- Group: `{}`\n", action.group_id));
            output.push_str(&format!("- Target: `{}`\n", action.target.target_file));
            if let Some(path) = &action.target.document_path {
                output.push_str(&format!("- Document path: `{path}`\n"));
            }
            if let Some(node_id) = &action.target.node_id {
                output.push_str(&format!("- Node ID: `{node_id}`\n"));
            }
            output.push_str(&format!(
                "- Modification: `{}`\n",
                modification_name(action.modification)
            ));
            output.push_str(&format!("- Expected effect: {}\n", action.expected_effect));
            output.push_str(&format!(
                "- Requires approval: `{}`\n",
                action.requires_approval
            ));
            output.push_str("- Verification captures:\n");
            for capture in &action.verification.captures {
                output.push_str(&format!(
                    "  - `{}/{}/{}`\n",
                    capture.screen, capture.device, capture.state
                ));
            }
            output.push('\n');
        }
    }
    output.push_str("## Rejected Groups\n\n");
    if plan.rejected_groups.is_empty() {
        output.push_str("None.\n");
    } else {
        for rejection in &plan.rejected_groups {
            output.push_str(&format!(
                "- `{}`: `{}` - {}\n",
                rejection.group_id,
                rejection_name(rejection.code),
                rejection.detail
            ));
        }
    }
    output
}

fn validate_policy(policy: &FixPlanPolicy) -> Result<(), TaskFailure> {
    if policy.allowed_roots.is_empty()
        || policy.allowed_roots.len() > 32
        || policy.max_files == 0
        || policy.max_files > MAX_ACTIONS
        || policy.max_diff_lines == 0
        || policy.max_asset_bytes == 0
    {
        return Err(rejected(
            "closed-loop fix plan policy is incomplete or exceeds hard limits",
        ));
    }
    let mut unique_roots = BTreeSet::new();
    for root in &policy.allowed_roots {
        let normalized = normalize_root(root)
            .ok_or_else(|| rejected("closed-loop fix plan policy has an unsafe allowed root"))?;
        if !unique_roots.insert(normalized) {
            return Err(rejected(
                "closed-loop fix plan policy has duplicate allowed roots",
            ));
        }
    }
    Ok(())
}

fn validate_audit_report(report: &ClosedLoopAuditReport) -> Result<(), TaskFailure> {
    if report.schema_version != 1
        || report.priority_order != ["hard", "visual", "ai"]
        || report.issue_groups.len() > MAX_GROUPS
    {
        return Err(rejected(
            "closed-loop audit report has an unsupported protocol",
        ));
    }
    RunId::parse(&report.run_id)?;
    let issue_count =
        report.hard_issues.len() + report.visual_issues.len() + report.ai_issues.len();
    if issue_count > MAX_ISSUES {
        return Err(rejected(
            "closed-loop audit report exceeds its issue budget",
        ));
    }
    for issue in report
        .hard_issues
        .iter()
        .chain(&report.visual_issues)
        .chain(&report.ai_issues)
    {
        if issue.issue_id.trim().is_empty()
            || issue.screen.trim().is_empty()
            || issue.device.trim().is_empty()
            || issue.state.trim().is_empty()
            || issue.region.region_id.trim().is_empty()
            || issue.evidence.is_empty()
            || issue.root_cause_key.trim().is_empty()
        {
            return Err(rejected(
                "closed-loop audit issue lacks required repair evidence",
            ));
        }
        for evidence in &issue.evidence {
            if !safe_relative_path(&evidence.artifact.path)
                || !is_sha256(&evidence.artifact.sha256)
                || evidence
                    .artifact
                    .byte_length
                    .is_some_and(|length| length == 0)
                || evidence.description.trim().is_empty()
            {
                return Err(rejected("closed-loop audit issue has invalid evidence"));
            }
        }
    }
    Ok(())
}

fn issues_by_id<'a>(
    report: &'a ClosedLoopAuditReport,
) -> Result<BTreeMap<&'a str, &'a AuditIssue>, TaskFailure> {
    let mut result = BTreeMap::new();
    for issue in report
        .hard_issues
        .iter()
        .chain(&report.visual_issues)
        .chain(&report.ai_issues)
    {
        if result.insert(issue.issue_id.as_str(), issue).is_some() {
            return Err(rejected("closed-loop audit report has duplicate issue IDs"));
        }
    }
    Ok(result)
}

fn group_issues<'a>(
    group: &AuditIssueGroup,
    all: &BTreeMap<&'a str, &'a AuditIssue>,
) -> Result<(Vec<&'a AuditIssue>, Vec<AuditCapture>), TaskFailure> {
    if group.group_id.trim().is_empty()
        || group.root_cause_key.trim().is_empty()
        || group.issue_ids.is_empty()
        || group.captures.is_empty()
        || group.evidence.is_empty()
    {
        return Err(rejected(
            "closed-loop audit issue group lacks required evidence",
        ));
    }
    let mut result = Vec::new();
    let mut ids = BTreeSet::new();
    for issue_id in &group.issue_ids {
        if !ids.insert(issue_id) {
            return Err(rejected(
                "closed-loop audit issue group repeats an issue ID",
            ));
        }
        let issue = all
            .get(issue_id.as_str())
            .ok_or_else(|| rejected("closed-loop audit issue group references an unknown issue"))?;
        if issue.attribution != group.attribution
            || issue.root_cause_key != group.root_cause_key
            || issue.requires_manual_review != group.requires_manual_review
            || issue.automatic_fix_allowed != group.automatic_fix_allowed
        {
            return Err(rejected(
                "closed-loop audit issue group conflicts with issue ownership",
            ));
        }
        result.push(*issue);
    }
    let trusted_captures = unique_captures_from_issues(&result);
    if unique_captures(&group.captures) != trusted_captures {
        return Err(rejected(
            "closed-loop audit issue group captures do not match its issue evidence",
        ));
    }
    Ok((result, trusted_captures))
}

fn candidate_for_group(
    group: &AuditIssueGroup,
    issues: &[&AuditIssue],
    trusted_captures: &[AuditCapture],
    policy: &FixPlanPolicy,
) -> Result<Option<FixPlanAction>, FixPlanRejection> {
    if group.requires_manual_review || !group.automatic_fix_allowed {
        let code = if group.attribution == FixAttribution::ReferenceOrRule {
            FixPlanRejectionCode::ProtectedTarget
        } else {
            FixPlanRejectionCode::ManualReviewRequired
        };
        return Err(group_rejection(
            group,
            code,
            "the audit group is explicitly manual-only and cannot become an automatic repair",
        ));
    }
    match group.attribution {
        FixAttribution::ReferenceOrRule => Err(group_rejection(
            group,
            FixPlanRejectionCode::ProtectedTarget,
            "reference images, baselines, masks, thresholds, and audit rules are forbidden targets",
        )),
        FixAttribution::BusinessContent => Err(group_rejection(
            group,
            FixPlanRejectionCode::BusinessContentRequiresHumanReview,
            "visual repair cannot infer product copy, route, binding, or action behavior",
        )),
        FixAttribution::DocumentLayout | FixAttribution::DocumentStyle => {
            let binding = unique_document_binding(issues).ok_or_else(|| {
                group_rejection(
                    group,
                    FixPlanRejectionCode::MissingDocumentBinding,
                    "a UiDocument repair requires one resolved node ID and document field path",
                )
            })?;
            let target_file = document_target_file(issues, policy).ok_or_else(|| {
                group_rejection(
                    group,
                    FixPlanRejectionCode::TargetOutsideAllowedRoots,
                    "the resolved UiDocument target is not inside an allowed staging root",
                )
            })?;
            let modification = match group.attribution {
                FixAttribution::DocumentLayout => FixModificationKind::UiDocumentLayout,
                FixAttribution::DocumentStyle => FixModificationKind::UiDocumentScopedToken,
                _ => unreachable!(),
            };
            Ok(Some(action_for(
                group,
                target_file,
                Some(binding.document_path),
                Some(binding.node_id),
                modification,
                false,
                trusted_captures,
            )))
        }
        FixAttribution::DraftAsset => {
            let target_file = unique_likely_file(issues).ok_or_else(|| {
                group_rejection(
                    group,
                    FixPlanRejectionCode::MissingDraftAssetTarget,
                    "a draft asset repair requires exactly one allow-listed draft asset target",
                )
            })?;
            if forbidden_target(&target_file) {
                return Err(group_rejection(
                    group,
                    forbidden_target_code(&target_file),
                    "draft asset target is a protected reference, rule, credential, prompt, or dependency file",
                ));
            }
            if !path_allowed(&target_file, policy) {
                return Err(group_rejection(
                    group,
                    FixPlanRejectionCode::TargetOutsideAllowedRoots,
                    "draft asset target is outside the allowed staging roots",
                ));
            }
            if !target_file.starts_with("assets/") {
                return Err(group_rejection(
                    group,
                    FixPlanRejectionCode::TargetOutsideAllowedRoots,
                    "draft asset repair targets must stay below the assets staging root",
                ));
            }
            Ok(Some(action_for(
                group,
                target_file,
                None,
                None,
                FixModificationKind::DraftAssetVersion,
                false,
                trusted_captures,
            )))
        }
        FixAttribution::CommonWidget | FixAttribution::Theme | FixAttribution::Framework => {
            let screens: BTreeSet<_> = trusted_captures
                .iter()
                .map(|capture| &capture.screen)
                .collect();
            if screens.len() < 2 {
                return Err(group_rejection(
                    group,
                    FixPlanRejectionCode::EscalationNotMultiPage,
                    "a shared component, theme, or framework escalation needs evidence from at least two screens",
                ));
            }
            if !policy.protocol_limitations.contains(&group.group_id) {
                return Err(group_rejection(
                    group,
                    FixPlanRejectionCode::ProtocolEscalationNotConfirmed,
                    "shared-scope escalation requires an explicit protocol limitation confirmation",
                ));
            }
            let target_file = unique_likely_file(issues).ok_or_else(|| {
                group_rejection(
                    group,
                    FixPlanRejectionCode::AmbiguousTarget,
                    "shared-scope escalation requires exactly one allow-listed source target",
                )
            })?;
            if forbidden_target(&target_file) {
                return Err(group_rejection(
                    group,
                    forbidden_target_code(&target_file),
                    "shared-scope repair target is protected by the closed-loop safety policy",
                ));
            }
            if !path_allowed(&target_file, policy) {
                return Err(group_rejection(
                    group,
                    FixPlanRejectionCode::TargetOutsideAllowedRoots,
                    "shared-scope repair target is outside the approved roots",
                ));
            }
            let modification = match group.attribution {
                FixAttribution::CommonWidget => FixModificationKind::CommonWidget,
                FixAttribution::Theme => FixModificationKind::Theme,
                FixAttribution::Framework => FixModificationKind::Framework,
                _ => unreachable!(),
            };
            Ok(Some(action_for(
                group,
                target_file,
                None,
                None,
                modification,
                true,
                trusted_captures,
            )))
        }
    }
}

fn action_for(
    group: &AuditIssueGroup,
    target_file: String,
    document_path: Option<String>,
    node_id: Option<String>,
    modification: FixModificationKind,
    requires_approval: bool,
    trusted_captures: &[AuditCapture],
) -> FixPlanAction {
    let estimated_diff_lines = match modification {
        FixModificationKind::UiDocumentLayout => 24,
        FixModificationKind::UiDocumentScopedToken => 16,
        FixModificationKind::DraftAssetVersion => 12,
        FixModificationKind::CommonWidget | FixModificationKind::Theme => 48,
        FixModificationKind::Framework => 64,
    };
    let expected_effect = match modification {
        FixModificationKind::UiDocumentLayout => {
            "adjust only the resolved UiDocument layout fields for the audited node".to_owned()
        }
        FixModificationKind::UiDocumentScopedToken => {
            "adjust only a page-scoped UiDocument token or style field for the audited node"
                .to_owned()
        }
        FixModificationKind::DraftAssetVersion => {
            "create a new draft asset version while preserving the previous source hash".to_owned()
        }
        FixModificationKind::CommonWidget => {
            "change the approved common widget behavior for the observed multi-page defect"
                .to_owned()
        }
        FixModificationKind::Theme => {
            "change the approved theme token for the observed multi-page defect".to_owned()
        }
        FixModificationKind::Framework => {
            "change the approved UI framework behavior for the confirmed protocol limitation"
                .to_owned()
        }
    };
    let identity = format!(
        "{}\n{}\n{}\n{}\n{}",
        group.group_id,
        target_file,
        document_path.as_deref().unwrap_or_default(),
        node_id.as_deref().unwrap_or_default(),
        modification_name(modification)
    );
    FixPlanAction {
        action_id: format!("fix-{}", &hash_bytes(identity.as_bytes())[..16]),
        group_id: group.group_id.clone(),
        attribution: group.attribution,
        target: FixTarget {
            target_file,
            document_path,
            node_id,
        },
        modification,
        expected_effect,
        verification: FixVerificationMatrix {
            captures: trusted_captures.to_vec(),
            rerun_group_captures_first: true,
            rerun_all_related_device_states: true,
            rerun_shared_scope_consumers: requires_approval,
        },
        estimated_diff_lines,
        estimated_asset_bytes: (modification == FixModificationKind::DraftAssetVersion)
            .then_some(1_048_576),
        risk: if requires_approval {
            FixPlanRisk::High
        } else {
            FixPlanRisk::Low
        },
        requires_approval,
        may_regress_other_device_states: true,
    }
}

fn unique_document_binding(issues: &[&AuditIssue]) -> Option<DocumentBinding> {
    let mut bindings = BTreeSet::new();
    for issue in issues {
        let binding = issue.document.as_ref()?;
        let node_id = nonempty(binding.node_id.as_deref())?;
        let document_path = nonempty(binding.document_path.as_deref())?;
        bindings.insert(DocumentBinding {
            node_id,
            document_path,
        });
    }
    (bindings.len() == 1)
        .then(|| bindings.into_iter().next())
        .flatten()
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct DocumentBinding {
    node_id: String,
    document_path: String,
}

fn document_target_file(issues: &[&AuditIssue], policy: &FixPlanPolicy) -> Option<String> {
    let mut sources = BTreeSet::new();
    for issue in issues {
        if let Some(source) = issue
            .document
            .as_ref()
            .and_then(|binding| binding.source_path.as_deref())
            && let Some(normalized) = normalize_candidate_path(source)
        {
            sources.insert(normalized);
        }
    }
    if sources.len() > 1 {
        return None;
    }
    let target = sources
        .into_iter()
        .next()
        .unwrap_or_else(|| "draft/generated-document.json".to_owned());
    path_allowed(&target, policy).then_some(target)
}

fn unique_likely_file(issues: &[&AuditIssue]) -> Option<String> {
    let candidates: BTreeSet<_> = issues
        .iter()
        .flat_map(|issue| issue.likely_files.iter())
        .filter_map(|path| normalize_candidate_path(path))
        .collect();
    (candidates.len() == 1)
        .then(|| candidates.into_iter().next())
        .flatten()
}

fn resolve_duplicate_and_conflicting_actions(
    candidates: &mut Vec<FixPlanAction>,
    rejections: &mut Vec<FixPlanRejection>,
) {
    let mut by_target: BTreeMap<(String, Option<String>, Option<String>), Vec<usize>> =
        BTreeMap::new();
    for (index, action) in candidates.iter().enumerate() {
        by_target
            .entry((
                action.target.target_file.clone(),
                action.target.document_path.clone(),
                action.target.node_id.clone(),
            ))
            .or_default()
            .push(index);
    }
    let mut discard = BTreeSet::new();
    for indexes in by_target.values() {
        if indexes.len() < 2 {
            continue;
        }
        let kinds: BTreeSet<_> = indexes
            .iter()
            .map(|index| candidates[*index].modification)
            .collect();
        if kinds.len() > 1 {
            for index in indexes {
                discard.insert(*index);
                rejections.push(FixPlanRejection {
                    group_id: candidates[*index].group_id.clone(),
                    code: FixPlanRejectionCode::ConflictingRepair,
                    detail: "several repair types target the same document field or file"
                        .to_owned(),
                });
            }
        } else {
            for index in indexes.iter().skip(1) {
                discard.insert(*index);
                rejections.push(FixPlanRejection {
                    group_id: candidates[*index].group_id.clone(),
                    code: FixPlanRejectionCode::DuplicateIneffectiveRepair,
                    detail: "an equivalent repair action already covers this target".to_owned(),
                });
            }
        }
    }
    *candidates = candidates
        .iter()
        .enumerate()
        .filter(|(index, _)| !discard.contains(index))
        .map(|(_, action)| action.clone())
        .collect();
}

fn enforce_plan_budgets(
    candidates: &mut Vec<FixPlanAction>,
    rejections: &mut Vec<FixPlanRejection>,
    policy: &FixPlanPolicy,
) -> Result<(), TaskFailure> {
    let total_diff_lines = candidates
        .iter()
        .map(|action| action.estimated_diff_lines)
        .sum::<u32>();
    let total_asset_bytes = candidates
        .iter()
        .filter_map(|action| action.estimated_asset_bytes)
        .sum::<u64>();
    let files: BTreeSet<_> = candidates
        .iter()
        .map(|action| action.target.target_file.as_str())
        .collect();
    let failure = if files.len() > policy.max_files {
        Some((
            FixPlanRejectionCode::FileLimitExceeded,
            "the plan exceeds the maximum target file count",
        ))
    } else if total_diff_lines > policy.max_diff_lines {
        Some((
            FixPlanRejectionCode::DiffBudgetExceeded,
            "the plan exceeds its estimated diff budget",
        ))
    } else if total_asset_bytes > policy.max_asset_bytes {
        Some((
            FixPlanRejectionCode::AssetBudgetExceeded,
            "the plan exceeds its draft asset byte budget",
        ))
    } else {
        None
    };
    if let Some((code, detail)) = failure {
        for action in candidates.drain(..) {
            rejections.push(FixPlanRejection {
                group_id: action.group_id,
                code,
                detail: detail.to_owned(),
            });
        }
    }
    Ok(())
}

fn policy_summary(policy: &FixPlanPolicy) -> FixPlanPolicySummary {
    let mut allowed_roots = policy
        .allowed_roots
        .iter()
        .filter_map(|root| normalize_root(root))
        .collect::<Vec<_>>();
    allowed_roots.sort();
    FixPlanPolicySummary {
        allowed_roots,
        forbidden_categories: vec![
            "reference".to_owned(),
            "baseline".to_owned(),
            "mask".to_owned(),
            "threshold".to_owned(),
            "credential".to_owned(),
            "git_configuration".to_owned(),
            "runner_command".to_owned(),
            "provider_prompt".to_owned(),
            "dependency_change".to_owned(),
        ],
        max_files: policy.max_files,
        max_diff_lines: policy.max_diff_lines,
        max_asset_bytes: policy.max_asset_bytes,
        dependency_changes_allowed: false,
    }
}

fn group_rejection(
    group: &AuditIssueGroup,
    code: FixPlanRejectionCode,
    detail: &str,
) -> FixPlanRejection {
    FixPlanRejection {
        group_id: group.group_id.clone(),
        code,
        detail: detail.to_owned(),
    }
}

fn unique_captures(captures: &[AuditCapture]) -> Vec<AuditCapture> {
    captures
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn unique_captures_from_issues(issues: &[&AuditIssue]) -> Vec<AuditCapture> {
    issues
        .iter()
        .map(|issue| AuditCapture {
            screen: issue.screen.clone(),
            device: issue.device.clone(),
            state: issue.state.clone(),
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn nonempty(value: Option<&str>) -> Option<String> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
}

fn normalize_root(value: &str) -> Option<String> {
    let normalized = normalize_candidate_path(value.trim().trim_end_matches('/'))?;
    Some(format!("{normalized}/"))
}

fn normalize_candidate_path(value: &str) -> Option<String> {
    let candidate = value.trim().replace('\\', "/");
    if candidate.is_empty() || candidate.starts_with('/') || candidate.contains(':') {
        return None;
    }
    let components: Vec<_> = candidate.split('/').collect();
    if components.is_empty()
        || components
            .iter()
            .any(|part| part.is_empty() || *part == "." || *part == "..")
    {
        return None;
    }
    let known_draft_offset = components
        .iter()
        .position(|part| *part == "draft" || *part == "assets");
    let result = known_draft_offset
        .map(|offset| components[offset..].join("/"))
        .unwrap_or_else(|| components.join("/"));
    safe_relative_path(&result).then_some(result)
}

fn path_allowed(path: &str, policy: &FixPlanPolicy) -> bool {
    if !safe_relative_path(path) || forbidden_target(path) {
        return false;
    }
    policy
        .allowed_roots
        .iter()
        .filter_map(|root| normalize_root(root))
        .any(|root| path.starts_with(&root))
}

fn forbidden_target(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let forbidden = [
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
    ];
    lower.split('/').any(|part| {
        forbidden
            .iter()
            .any(|word| part == *word || part.starts_with(word))
    })
}

fn forbidden_target_code(path: &str) -> FixPlanRejectionCode {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with("/cargo.toml") || lower.ends_with("/cargo.lock") {
        FixPlanRejectionCode::DependencyChangeForbidden
    } else {
        FixPlanRejectionCode::ForbiddenTarget
    }
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
                && segment.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'$')
                })
        })
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn create_regular_output_directory(path: &Path) -> Result<PathBuf, TaskFailure> {
    let raw = absolute_output_path(path)?;
    reject_reparse_chain(&raw)?;
    fs::create_dir_all(&raw).map_err(|_| {
        TaskFailure::new(
            TaskFailureKind::UnsafeOutputPath,
            "closed-loop fix plan output directory cannot be created",
            Some(path.display().to_string()),
        )
    })?;
    reject_reparse_chain(&raw)?;
    let raw_metadata = fs::symlink_metadata(&raw)
        .map_err(|_| rejected("closed-loop fix plan output directory cannot be inspected"))?;
    if metadata_is_reparse(&raw_metadata) || !raw_metadata.is_dir() {
        return Err(rejected(
            "closed-loop fix plan output directory is not a regular directory",
        ));
    }
    let canonical = fs::canonicalize(&raw)
        .map_err(|_| rejected("closed-loop fix plan output directory cannot be resolved"))?;
    if !fs::metadata(&canonical).is_ok_and(|metadata| metadata.is_dir()) {
        return Err(rejected(
            "closed-loop fix plan output directory is not a regular directory",
        ));
    }
    Ok(canonical)
}

fn absolute_output_path(path: &Path) -> Result<PathBuf, TaskFailure> {
    if path.as_os_str().is_empty() {
        return Err(rejected("closed-loop fix plan output directory is empty"));
    }
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::env::current_dir()
            .map(|current| current.join(path))
            .map_err(|_| rejected("closed-loop fix plan current directory cannot be resolved"))
    }
}

fn reject_reparse_chain(path: &Path) -> Result<(), TaskFailure> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata_is_reparse(&metadata) => {
                return Err(rejected(
                    "closed-loop fix plan output directory cannot traverse a symlink or reparse point",
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(_) => {
                return Err(rejected(
                    "closed-loop fix plan output directory path cannot be inspected",
                ));
            }
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

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), TaskFailure> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| {
            TaskFailure::new(
                TaskFailureKind::OutputDirectoryConflict,
                "closed-loop fix plan artifact already exists or cannot be created",
                Some(path.display().to_string()),
            )
        })?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| {
            TaskFailure::new(
                TaskFailureKind::UnsafeOutputPath,
                "closed-loop fix plan artifact cannot be written",
                Some(path.display().to_string()),
            )
        })
}

fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn rejected(message: impl Into<String>) -> TaskFailure {
    TaskFailure::new(TaskFailureKind::FixPlanRejected, message, None)
}

fn plan_status_name(status: FixPlanStatus) -> &'static str {
    match status {
        FixPlanStatus::ReadyForApply => "ready_for_apply",
        FixPlanStatus::AwaitingApproval => "awaiting_approval",
        FixPlanStatus::NoAvailableFix => "no_available_fix",
    }
}

fn modification_name(modification: FixModificationKind) -> &'static str {
    match modification {
        FixModificationKind::UiDocumentLayout => "ui_document_layout",
        FixModificationKind::UiDocumentScopedToken => "ui_document_scoped_token",
        FixModificationKind::DraftAssetVersion => "draft_asset_version",
        FixModificationKind::CommonWidget => "common_widget",
        FixModificationKind::Theme => "theme",
        FixModificationKind::Framework => "framework",
    }
}

fn rejection_name(rejection: FixPlanRejectionCode) -> &'static str {
    match rejection {
        FixPlanRejectionCode::ManualReviewRequired => "manual_review_required",
        FixPlanRejectionCode::BusinessContentRequiresHumanReview => {
            "business_content_requires_human_review"
        }
        FixPlanRejectionCode::ProtectedTarget => "protected_target",
        FixPlanRejectionCode::MissingDocumentBinding => "missing_document_binding",
        FixPlanRejectionCode::MissingDraftAssetTarget => "missing_draft_asset_target",
        FixPlanRejectionCode::AmbiguousTarget => "ambiguous_target",
        FixPlanRejectionCode::ProtocolEscalationNotConfirmed => "protocol_escalation_not_confirmed",
        FixPlanRejectionCode::EscalationNotMultiPage => "escalation_not_multi_page",
        FixPlanRejectionCode::TargetOutsideAllowedRoots => "target_outside_allowed_roots",
        FixPlanRejectionCode::ForbiddenTarget => "forbidden_target",
        FixPlanRejectionCode::FileLimitExceeded => "file_limit_exceeded",
        FixPlanRejectionCode::DiffBudgetExceeded => "diff_budget_exceeded",
        FixPlanRejectionCode::AssetBudgetExceeded => "asset_budget_exceeded",
        FixPlanRejectionCode::DependencyChangeForbidden => "dependency_change_forbidden",
        FixPlanRejectionCode::DuplicateIneffectiveRepair => "duplicate_ineffective_repair",
        FixPlanRejectionCode::ConflictingRepair => "conflicting_repair",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn audit() -> ClosedLoopAuditReport {
        serde_json::from_slice(include_bytes!("../fixtures/fix_plan/audit.valid.json")).unwrap()
    }

    #[test]
    fn document_plan_is_machine_readable_human_readable_and_bounded() {
        let plan = create_closed_loop_fix_plan(&audit(), &FixPlanPolicy::default()).unwrap();
        assert_eq!(plan.status, FixPlanStatus::ReadyForApply);
        assert_eq!(plan.actions.len(), 2);
        let layout = plan
            .actions
            .iter()
            .find(|action| action.attribution == FixAttribution::DocumentLayout)
            .unwrap();
        assert_eq!(layout.target.target_file, "draft/generated-document.json");
        assert_eq!(layout.target.node_id.as_deref(), Some("page.title"));
        assert!(layout.may_regress_other_device_states);
        let markdown = render_fix_plan_markdown(&plan);
        assert!(markdown.contains(&layout.action_id));
        assert!(serde_json::to_value(&plan).unwrap()["actions"].is_array());
    }

    #[test]
    fn stage_four_repository_artifact_links_do_not_require_byte_lengths() {
        let mut value: serde_json::Value =
            serde_json::from_slice(include_bytes!("../fixtures/fix_plan/audit.valid.json"))
                .unwrap();
        for issue_kind in ["hard_issues", "visual_issues", "ai_issues"] {
            for issue in value[issue_kind].as_array_mut().unwrap() {
                for evidence in issue["evidence"].as_array_mut().unwrap() {
                    evidence["artifact"]
                        .as_object_mut()
                        .unwrap()
                        .remove("byte_length");
                }
            }
        }
        for group in value["issue_groups"].as_array_mut().unwrap() {
            for evidence in group["evidence"].as_array_mut().unwrap() {
                evidence["artifact"]
                    .as_object_mut()
                    .unwrap()
                    .remove("byte_length");
            }
        }
        let report: ClosedLoopAuditReport = serde_json::from_value(value).unwrap();
        assert_eq!(
            create_closed_loop_fix_plan(&report, &FixPlanPolicy::default())
                .unwrap()
                .actions
                .len(),
            2
        );
    }

    #[test]
    fn each_manual_or_protected_attribution_is_rejected_without_guessing() {
        let mut report = audit();
        let cases = [
            (
                FixAttribution::BusinessContent,
                FixPlanRejectionCode::BusinessContentRequiresHumanReview,
            ),
            (
                FixAttribution::ReferenceOrRule,
                FixPlanRejectionCode::ProtectedTarget,
            ),
        ];
        for (attribution, expected) in cases {
            let mut current = report.clone();
            let group = current.issue_groups.first_mut().unwrap();
            group.attribution = attribution;
            group.requires_manual_review = attribution == FixAttribution::ReferenceOrRule;
            group.automatic_fix_allowed = attribution != FixAttribution::ReferenceOrRule;
            for issue in &mut current.hard_issues {
                issue.attribution = attribution;
                issue.requires_manual_review = attribution == FixAttribution::ReferenceOrRule;
                issue.automatic_fix_allowed = attribution != FixAttribution::ReferenceOrRule;
            }
            let plan = create_closed_loop_fix_plan(&current, &FixPlanPolicy::default()).unwrap();
            assert!(
                plan.rejected_groups
                    .iter()
                    .any(|rejection| rejection.code == expected)
            );
        }
        report.issue_groups[0].requires_manual_review = true;
        report.issue_groups[0].automatic_fix_allowed = false;
        report.hard_issues[0].requires_manual_review = true;
        report.hard_issues[0].automatic_fix_allowed = false;
        let plan = create_closed_loop_fix_plan(&report, &FixPlanPolicy::default()).unwrap();
        assert!(
            plan.rejected_groups
                .iter()
                .any(|rejection| rejection.code == FixPlanRejectionCode::ManualReviewRequired)
        );
    }

    #[test]
    fn every_audit_attribution_has_a_bounded_fixture_outcome() {
        let cases = [
            (FixAttribution::DocumentLayout, None, true),
            (FixAttribution::DocumentStyle, None, true),
            (FixAttribution::DraftAsset, None, true),
            (
                FixAttribution::BusinessContent,
                Some(FixPlanRejectionCode::BusinessContentRequiresHumanReview),
                false,
            ),
            (
                FixAttribution::CommonWidget,
                Some(FixPlanRejectionCode::EscalationNotMultiPage),
                false,
            ),
            (
                FixAttribution::Theme,
                Some(FixPlanRejectionCode::EscalationNotMultiPage),
                false,
            ),
            (
                FixAttribution::Framework,
                Some(FixPlanRejectionCode::EscalationNotMultiPage),
                false,
            ),
            (
                FixAttribution::ReferenceOrRule,
                Some(FixPlanRejectionCode::ProtectedTarget),
                false,
            ),
        ];
        for (attribution, rejection, expects_action) in cases {
            let mut report = audit();
            let group = &mut report.issue_groups[0];
            group.attribution = attribution;
            group.requires_manual_review = attribution == FixAttribution::ReferenceOrRule;
            group.automatic_fix_allowed = attribution != FixAttribution::ReferenceOrRule;
            let issue = &mut report.hard_issues[0];
            issue.attribution = attribution;
            issue.requires_manual_review = attribution == FixAttribution::ReferenceOrRule;
            issue.automatic_fix_allowed = attribution != FixAttribution::ReferenceOrRule;
            if attribution == FixAttribution::DraftAsset {
                issue.likely_files = vec!["assets/generated/title-v2.png".into()];
            }
            let plan = create_closed_loop_fix_plan(&report, &FixPlanPolicy::default()).unwrap();
            assert_eq!(
                plan.actions
                    .iter()
                    .any(|action| action.group_id == "group-title"),
                expects_action,
                "unexpected action result for {attribution:?}"
            );
            if let Some(expected) = rejection {
                assert!(
                    plan.rejected_groups
                        .iter()
                        .any(|actual| actual.group_id == "group-title" && actual.code == expected),
                    "missing rejection for {attribution:?}"
                );
            }
        }
    }

    #[test]
    fn shared_scope_escalation_needs_multi_page_protocol_confirmation_and_approval() {
        let mut report = audit();
        let group_id = {
            let group = report.issue_groups.first_mut().unwrap();
            group.attribution = FixAttribution::Theme;
            group.captures.push(AuditCapture {
                screen: "settings".into(),
                device: "phone".into(),
                state: "initial".into(),
            });
            group.issue_ids.push("hard-title-settings".into());
            group.group_id.clone()
        };
        for issue in &mut report.hard_issues {
            issue.attribution = FixAttribution::Theme;
            issue.likely_files = vec!["project/src/framework/ui/style/theme.rs".into()];
        }
        let mut second_page_issue = report.hard_issues[0].clone();
        second_page_issue.issue_id = "hard-title-settings".into();
        second_page_issue.screen = "settings".into();
        report.hard_issues.push(second_page_issue);
        let mut policy = FixPlanPolicy {
            allowed_roots: vec!["project/src/framework/ui/style/".into()],
            ..FixPlanPolicy::default()
        };
        let rejected = create_closed_loop_fix_plan(&report, &policy).unwrap();
        assert!(rejected.rejected_groups.iter().any(
            |rejection| rejection.code == FixPlanRejectionCode::ProtocolEscalationNotConfirmed
        ));
        policy.protocol_limitations.insert(group_id);
        let awaiting = create_closed_loop_fix_plan(&report, &policy).unwrap();
        assert_eq!(awaiting.status, FixPlanStatus::AwaitingApproval);
        assert!(awaiting.actions[0].requires_approval);
    }

    #[test]
    fn shared_scope_escalation_rejects_a_forged_group_capture() {
        let mut report = audit();
        let group_id = report.issue_groups[0].group_id.clone();
        report.issue_groups[0].attribution = FixAttribution::Theme;
        report.issue_groups[0].captures.push(AuditCapture {
            screen: "settings".into(),
            device: "phone".into(),
            state: "initial".into(),
        });
        report.hard_issues[0].attribution = FixAttribution::Theme;
        report.hard_issues[0].likely_files = vec!["project/src/framework/ui/style/theme.rs".into()];
        let mut policy = FixPlanPolicy {
            allowed_roots: vec!["project/src/framework/ui/style/".into()],
            ..FixPlanPolicy::default()
        };
        policy.protocol_limitations.insert(group_id);
        let failure = create_closed_loop_fix_plan(&report, &policy).unwrap_err();
        assert_eq!(failure.kind(), TaskFailureKind::FixPlanRejected);
        assert!(failure.message().contains("captures"));
    }

    #[test]
    fn unsafe_targets_duplicate_repairs_and_budget_overruns_are_rejected() {
        let mut report = audit();
        let mut duplicate = report.hard_issues[0].clone();
        duplicate.root_cause_key = "document_layout|node:page.title|visual|same".into();
        duplicate.issue_id = "visual-duplicate".into();
        report.visual_issues.push(duplicate);
        report.issue_groups.push(AuditIssueGroup {
            group_id: "group-duplicate".into(),
            root_cause_key: report.visual_issues[0].root_cause_key.clone(),
            attribution: FixAttribution::DocumentLayout,
            requires_manual_review: false,
            automatic_fix_allowed: true,
            priorities: vec!["visual".into()],
            issue_ids: vec!["visual-duplicate".into()],
            captures: vec![AuditCapture {
                screen: "home".into(),
                device: "phone".into(),
                state: "initial".into(),
            }],
            evidence: report.visual_issues[0].evidence.clone(),
        });
        let plan = create_closed_loop_fix_plan(&report, &FixPlanPolicy::default()).unwrap();
        assert!(plan.rejected_groups.iter().any(|rejection| rejection.code == FixPlanRejectionCode::DuplicateIneffectiveRepair));
        let mut policy = FixPlanPolicy {
            max_diff_lines: 1,
            ..FixPlanPolicy::default()
        };
        let limited = create_closed_loop_fix_plan(&audit(), &policy).unwrap();
        assert!(
            limited
                .rejected_groups
                .iter()
                .any(|rejection| rejection.code == FixPlanRejectionCode::DiffBudgetExceeded)
        );
        policy.max_diff_lines = DEFAULT_MAX_DIFF_LINES;
        let mut unsafe_report = audit();
        unsafe_report.ai_issues[0].attribution = FixAttribution::DraftAsset;
        unsafe_report.ai_issues[0].likely_files =
            vec!["project/assets/ui/references/protected.png".into()];
        unsafe_report.issue_groups[1].attribution = FixAttribution::DraftAsset;
        let unsafe_plan =
            create_closed_loop_fix_plan(&unsafe_report, &FixPlanPolicy::default()).unwrap();
        assert!(
            unsafe_plan
                .rejected_groups
                .iter()
                .any(|rejection| rejection.code == FixPlanRejectionCode::ForbiddenTarget)
        );

        let mut dependency_report = audit();
        let dependency_group_id = dependency_report.issue_groups[0].group_id.clone();
        dependency_report.issue_groups[0].attribution = FixAttribution::Framework;
        dependency_report.issue_groups[0]
            .captures
            .push(AuditCapture {
                screen: "settings".into(),
                device: "phone".into(),
                state: "initial".into(),
            });
        dependency_report.issue_groups[0]
            .issue_ids
            .push("hard-title-dependency-settings".into());
        dependency_report.hard_issues[0].attribution = FixAttribution::Framework;
        dependency_report.hard_issues[0].likely_files = vec!["project/Cargo.toml".into()];
        let mut second_dependency_issue = dependency_report.hard_issues[0].clone();
        second_dependency_issue.issue_id = "hard-title-dependency-settings".into();
        second_dependency_issue.screen = "settings".into();
        dependency_report.hard_issues.push(second_dependency_issue);
        let mut dependency_policy = FixPlanPolicy {
            allowed_roots: vec!["project/".into()],
            ..FixPlanPolicy::default()
        };
        dependency_policy
            .protocol_limitations
            .insert(dependency_group_id);
        let dependency_plan =
            create_closed_loop_fix_plan(&dependency_report, &dependency_policy).unwrap();
        assert!(dependency_plan.rejected_groups.iter().any(|rejection| {
            rejection.code == FixPlanRejectionCode::DependencyChangeForbidden
        }));
    }

    #[test]
    fn write_is_append_only_and_never_overwrites_plan_evidence() {
        let directory = tempfile::tempdir().unwrap();
        let plan = create_closed_loop_fix_plan(&audit(), &FixPlanPolicy::default()).unwrap();
        let written = write_closed_loop_fix_plan(&plan, directory.path()).unwrap();
        assert!(written.plan_path.is_file());
        assert!(written.markdown_path.is_file());
        assert!(write_closed_loop_fix_plan(&plan, directory.path()).is_err());
    }

    #[test]
    fn write_rejects_a_symlinked_output_directory_when_supported() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("target");
        let link = directory.path().join("linked-output");
        fs::create_dir(&target).unwrap();
        if let Err(error) = create_directory_link(&target, &link) {
            if error.kind() == std::io::ErrorKind::PermissionDenied
                || error.raw_os_error() == Some(1314)
            {
                return;
            }
            panic!("could not create test directory link: {error}");
        }
        let plan = create_closed_loop_fix_plan(&audit(), &FixPlanPolicy::default()).unwrap();
        assert!(write_closed_loop_fix_plan(&plan, &link).is_err());
        assert!(!target.join("fix-plan.json").exists());
    }

    #[cfg(unix)]
    fn create_directory_link(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    #[cfg(windows)]
    fn create_directory_link(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_dir(target, link)
    }
}
