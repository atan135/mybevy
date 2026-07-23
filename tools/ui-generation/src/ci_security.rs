//! Deterministic CI policy checks for the UI generation and visual-audit boundary.
//!
//! These checks intentionally validate policy names and workflow contracts only. They never
//! resolve credentials, send reference bytes, contact a provider, or discover remote devices.

use crate::{
    lifecycle::{TaskFailure, TaskFailureKind},
    observability::redact_report_value,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, path::Path};

pub const CI_SECURITY_POLICY_VERSION: u32 = 1;
const POLICY_PATH: &str = "tools/ui-generation/fixtures/ci/ui-ci-security-policy.v1.json";
const OFFLINE_WORKFLOW_PATH: &str = ".github/workflows/ui-visual-audit.yml";
const ONLINE_CONTRACT_WORKFLOW_PATH: &str = ".github/workflows/ui-online-audit-contract.yml";
pub const REFERENCE_BASELINE_APPROVAL_LABEL: &str = "ui-reference-baseline-approved";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CiRunMode {
    LocalDevelopment,
    PrFixture,
    PrDeterministicAudit,
    ManualOnlineGeneration,
    ScheduledOnlineAudit,
}

impl CiRunMode {
    const ALL: [Self; 5] = [
        Self::LocalDevelopment,
        Self::PrFixture,
        Self::PrDeterministicAudit,
        Self::ManualOnlineGeneration,
        Self::ScheduledOnlineAudit,
    ];

    fn key(&self) -> &'static str {
        match self {
            Self::LocalDevelopment => "local_development",
            Self::PrFixture => "pr_fixture",
            Self::PrDeterministicAudit => "pr_deterministic_audit",
            Self::ManualOnlineGeneration => "manual_online_generation",
            Self::ScheduledOnlineAudit => "scheduled_online_audit",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CiModePolicy {
    pub triggers: Vec<String>,
    pub execution: String,
    pub reads_online_credentials: bool,
    pub permits_network: bool,
    pub permits_remote_device: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_environment_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_environment: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OnlineContractPolicy {
    pub execution: String,
    pub credential_scope: String,
    pub approved_provider_domains: Vec<String>,
    pub reference_upload: String,
    pub user_reference_upload_enabled: bool,
    pub timeout_seconds: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactPolicy {
    pub failure_reports_downloadable: bool,
    pub redaction_required: bool,
    pub include_original_credentials: bool,
    pub include_unapproved_reference_images: bool,
    pub max_artifact_bytes: u64,
    pub retention_days: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SupplyChainPolicy {
    pub require_locked_cargo: bool,
    pub reject_git_dependencies: bool,
    pub generated_resources_require_license: bool,
    pub model_output_requires_human_approval: bool,
    pub untrusted_shader_execution: String,
    pub approved_shader_license_required: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CiQuotaPolicy {
    pub offline_timeout_minutes: u32,
    pub online_contract_timeout_minutes: u32,
    pub cache_max_bytes: u64,
    pub artifact_max_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CiSecurityPolicy {
    pub schema_version: u32,
    pub approval_label: String,
    pub protected_path_prefixes: Vec<String>,
    pub modes: BTreeMap<String, CiModePolicy>,
    pub online_contract: OnlineContractPolicy,
    pub artifacts: ArtifactPolicy,
    pub supply_chain: SupplyChainPolicy,
    pub quotas: CiQuotaPolicy,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CiSecurityContractReport {
    pub schema_version: u32,
    pub policy_path: String,
    pub validated_modes: Vec<String>,
    pub rejected_workflow_capabilities: Vec<String>,
    pub approval_label: String,
    pub offline_timeout_minutes: u32,
    pub online_contract_timeout_minutes: u32,
    pub cache_max_bytes: u64,
    pub artifact_max_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CiSecurityFixtureReport {
    pub contract: CiSecurityContractReport,
    pub rejected_scenarios: Vec<String>,
}

pub fn check_ci_security_contract(
    repository_root: &Path,
) -> Result<CiSecurityContractReport, TaskFailure> {
    let policy_path = repository_root.join(POLICY_PATH);
    let policy = load_policy(&policy_path)?;
    validate_policy(&policy)?;
    validate_workflow_contracts(repository_root, &policy)?;

    Ok(CiSecurityContractReport {
        schema_version: CI_SECURITY_POLICY_VERSION,
        policy_path: POLICY_PATH.to_owned(),
        validated_modes: CiRunMode::ALL
            .iter()
            .map(|mode| mode.key().to_owned())
            .collect(),
        rejected_workflow_capabilities: vec![
            "ordinary_pr_online_credentials".to_owned(),
            "external_or_untrusted_remote_device".to_owned(),
            "automatic_commit_push_release_or_branch_protection".to_owned(),
            "unapproved_provider_reference_upload".to_owned(),
        ],
        approval_label: policy.approval_label,
        offline_timeout_minutes: policy.quotas.offline_timeout_minutes,
        online_contract_timeout_minutes: policy.quotas.online_contract_timeout_minutes,
        cache_max_bytes: policy.quotas.cache_max_bytes,
        artifact_max_bytes: policy.quotas.artifact_max_bytes,
    })
}

pub fn run_ci_security_fixture(
    repository_root: &Path,
) -> Result<CiSecurityFixtureReport, TaskFailure> {
    let contract = check_ci_security_contract(repository_root)?;
    let policy = load_policy(&repository_root.join(POLICY_PATH))?;

    let mut missing_credential = policy.clone();
    missing_credential
        .modes
        .get_mut(CiRunMode::ManualOnlineGeneration.key())
        .expect("validated policy contains manual online generation")
        .credential_environment_name = None;
    assert_rejected(&missing_credential, "missing_online_credential_contract")?;

    let mut external_branch = policy.clone();
    external_branch
        .modes
        .get_mut(CiRunMode::PrFixture.key())
        .expect("validated policy contains PR fixture mode")
        .permits_remote_device = true;
    assert_rejected(&external_branch, "external_branch_remote_device")?;

    let mut missing_environment = policy.clone();
    missing_environment
        .modes
        .get_mut(CiRunMode::ScheduledOnlineAudit.key())
        .expect("validated policy contains scheduled online audit")
        .required_environment = None;
    assert_rejected(
        &missing_environment,
        "online_mode_without_protected_environment",
    )?;

    let mut unapproved_provider = policy.clone();
    unapproved_provider
        .online_contract
        .approved_provider_domains = vec!["unapproved-provider.example.test".to_owned()];
    assert_rejected(&unapproved_provider, "unapproved_provider_domain")?;

    if reference_or_baseline_change_requires_approval(
        &policy,
        &["tools/ui-visual-audit/fixtures/references/phone.png".to_owned()],
        &[],
    )? {
        // Expected: the helper signals that a protected change needs the approval label.
    } else {
        return Err(policy_rejected(
            "reference/baseline fixture did not require its approval label",
        ));
    }
    if reference_or_baseline_change_requires_approval(
        &policy,
        &["tools/ui-visual-audit/fixtures/references/phone.png".to_owned()],
        &[REFERENCE_BASELINE_APPROVAL_LABEL.to_owned()],
    )? {
        return Err(policy_rejected(
            "approved reference/baseline fixture remained blocked",
        ));
    }

    let redacted = redact_failure_report(&json!({
        "api_token": "fixture-secret-not-a-real-key",
        "account_email": "fixture.account@example.test",
        "reference_image": "not-an-authorized-image",
        "reference_image_bytes": "not-image-bytes",
        "status": "failed"
    }))?;
    if redacted["api_token"] != "[REDACTED]"
        || redacted["account_email"] != "[REDACTED]"
        || redacted["reference_image"] != "[REDACTED]"
        || redacted["reference_image_bytes"] != "[REDACTED]"
    {
        return Err(policy_rejected(
            "downloadable failure report did not redact sensitive fields",
        ));
    }

    Ok(CiSecurityFixtureReport {
        contract,
        rejected_scenarios: vec![
            "missing_online_credential_contract".to_owned(),
            "external_branch_remote_device".to_owned(),
            "online_mode_without_protected_environment".to_owned(),
            "reference_baseline_change_without_approval_label".to_owned(),
            "unapproved_provider_domain".to_owned(),
            "failure_report_secret_account_or_reference_image".to_owned(),
        ],
    })
}

pub fn reference_or_baseline_change_requires_approval(
    policy: &CiSecurityPolicy,
    changed_files: &[String],
    labels: &[String],
) -> Result<bool, TaskFailure> {
    validate_policy(policy)?;
    let protected_change = changed_files.iter().any(|path| {
        let path = path.replace('\\', "/");
        policy
            .protected_path_prefixes
            .iter()
            .any(|prefix| path.starts_with(prefix))
    });
    Ok(protected_change && !labels.iter().any(|label| label == &policy.approval_label))
}

pub fn redact_failure_report(value: &Value) -> Result<Value, TaskFailure> {
    let redacted = redact_report_value(value);
    if contains_unredacted_sensitive_value(&redacted, None) {
        return Err(policy_rejected(
            "failure report contains a credential, account field, or raw reference image",
        ));
    }
    Ok(redacted)
}

fn load_policy(path: &Path) -> Result<CiSecurityPolicy, TaskFailure> {
    let bytes = fs::read(path).map_err(|_| {
        policy_rejected("CI security policy fixture cannot be read from its repository path")
    })?;
    serde_json::from_slice(&bytes)
        .map_err(|_| policy_rejected("CI security policy fixture is not valid JSON"))
}

fn validate_policy(policy: &CiSecurityPolicy) -> Result<(), TaskFailure> {
    if policy.schema_version != CI_SECURITY_POLICY_VERSION
        || policy.approval_label != REFERENCE_BASELINE_APPROVAL_LABEL
        || policy.protected_path_prefixes.is_empty()
        || policy
            .protected_path_prefixes
            .iter()
            .any(|path| path.is_empty() || path.starts_with('/') || path.contains(".."))
    {
        return Err(policy_rejected(
            "CI security policy has an incompatible approval or protected-path contract",
        ));
    }

    for mode in CiRunMode::ALL {
        let Some(mode_policy) = policy.modes.get(mode.key()) else {
            return Err(policy_rejected(
                "CI security policy omitted a required run mode",
            ));
        };
        validate_mode_policy(&mode, mode_policy)?;
    }
    if policy.modes.len() != CiRunMode::ALL.len() {
        return Err(policy_rejected(
            "CI security policy declares an unknown run mode",
        ));
    }

    let online = &policy.online_contract;
    if online.execution != "contract_only"
        || online.credential_scope != "provider_api_only"
        || online.approved_provider_domains != ["provider.example.invalid"]
        || online.reference_upload != "approved_provider_only"
        || online.user_reference_upload_enabled
        || !(60..=900).contains(&online.timeout_seconds)
    {
        return Err(policy_rejected(
            "online provider policy is not a bounded deny-by-default contract",
        ));
    }

    let artifacts = &policy.artifacts;
    if !artifacts.failure_reports_downloadable
        || !artifacts.redaction_required
        || artifacts.include_original_credentials
        || artifacts.include_unapproved_reference_images
        || artifacts.max_artifact_bytes == 0
        || artifacts.max_artifact_bytes > 64 * 1024 * 1024
        || !(1..=30).contains(&artifacts.retention_days)
    {
        return Err(policy_rejected(
            "artifact policy permits unsafe reports or an unbounded retention quota",
        ));
    }

    let supply_chain = &policy.supply_chain;
    if !supply_chain.require_locked_cargo
        || !supply_chain.reject_git_dependencies
        || !supply_chain.generated_resources_require_license
        || !supply_chain.model_output_requires_human_approval
        || supply_chain.untrusted_shader_execution != "forbidden"
        || !supply_chain.approved_shader_license_required
    {
        return Err(policy_rejected(
            "supply-chain, generated-resource, or shader policy is not fail-closed",
        ));
    }

    let quotas = &policy.quotas;
    if !(1..=30).contains(&quotas.offline_timeout_minutes)
        || !(1..=30).contains(&quotas.online_contract_timeout_minutes)
        || quotas.cache_max_bytes == 0
        || quotas.cache_max_bytes > 4 * 1024 * 1024 * 1024
        || quotas.artifact_max_bytes != artifacts.max_artifact_bytes
    {
        return Err(policy_rejected(
            "CI timeout, cache, or artifact quota is unsafe",
        ));
    }
    Ok(())
}

fn validate_mode_policy(mode: &CiRunMode, policy: &CiModePolicy) -> Result<(), TaskFailure> {
    let offline = |triggers: &[&str]| {
        policy.execution == "offline"
            && policy.triggers == triggers
            && !policy.reads_online_credentials
            && !policy.permits_network
            && !policy.permits_remote_device
            && policy.credential_environment_name.is_none()
            && policy.required_environment.is_none()
    };
    let online_contract = |trigger: &str| {
        policy.execution == "contract_only"
            && policy.triggers == [trigger]
            && !policy.reads_online_credentials
            && !policy.permits_network
            && !policy.permits_remote_device
            && policy.credential_environment_name.as_deref() == Some("UI_GENERATION_PROVIDER_TOKEN")
            && policy.required_environment.as_deref() == Some("ui-audit-online")
    };
    let valid = match mode {
        CiRunMode::LocalDevelopment => offline(&["local"]),
        CiRunMode::PrFixture => offline(&["pull_request"]),
        CiRunMode::PrDeterministicAudit => offline(&["pull_request", "push", "workflow_dispatch"]),
        CiRunMode::ManualOnlineGeneration => online_contract("workflow_dispatch"),
        CiRunMode::ScheduledOnlineAudit => online_contract("schedule"),
    };
    if valid {
        Ok(())
    } else {
        Err(policy_rejected(
            "CI mode permits credentials, network, or remote devices outside its contract",
        ))
    }
}

fn validate_workflow_contracts(
    repository_root: &Path,
    policy: &CiSecurityPolicy,
) -> Result<(), TaskFailure> {
    let offline = read_workflow(&repository_root.join(OFFLINE_WORKFLOW_PATH))?;
    require_text(&offline, "pull_request:")?;
    require_text(&offline, "push:")?;
    require_text(&offline, "permissions:")?;
    require_text(&offline, "contents: read")?;
    require_text(&offline, "ci-security-fixture")?;
    require_text(
        &offline,
        "test-ui-reference-baseline-approval.ps1 -SelfTest",
    )?;
    require_text(&offline, "test-ui-supply-chain.ps1")?;
    require_text(&offline, "reference-baseline-approval")?;
    require_text(&offline, "write-ui-ci-failure-report.ps1")?;
    require_text(&offline, "actions/upload-artifact@v4")?;
    require_text(&offline, "retention-days: 14")?;
    require_text(&offline, "--locked")?;
    reject_text(
        &offline,
        &[
            "secrets.",
            "pull_request_target",
            "-onlineai",
            "mybevy_ui_audit_ai_config",
            "remote-device",
            "git push",
            "gh release",
            "branches/",
        ],
    )?;

    let approval_script =
        read_workflow(&repository_root.join("scripts/test-ui-reference-baseline-approval.ps1"))?;
    require_text(&approval_script, &policy.approval_label)?;
    for protected_path in &policy.protected_path_prefixes {
        require_text(&approval_script, protected_path)?;
    }

    let online = read_workflow(&repository_root.join(ONLINE_CONTRACT_WORKFLOW_PATH))?;
    require_text(&online, "workflow_dispatch:")?;
    require_text(&online, "schedule:")?;
    require_text(&online, "environment: ui-audit-online")?;
    require_text(&online, "timeout-minutes: 15")?;
    require_text(&online, "contents: read")?;
    require_text(&online, "MYBEVY_UI_AUDIT_ONLINE_EXECUTION: contract_only")?;
    require_text(&online, "ci-security-contract")?;
    require_text(&online, "write-ui-ci-failure-report.ps1")?;
    require_text(&online, "actions/upload-artifact@v4")?;
    require_text(&online, "retention-days: 14")?;
    require_text(&online, "--locked")?;
    reject_text(
        &online,
        &[
            "secrets.",
            "-onlineai",
            "mybevy_ui_audit_ai_config",
            "remote-device",
            "git push",
            "gh release",
            "branches/",
        ],
    )
}

fn read_workflow(path: &Path) -> Result<String, TaskFailure> {
    fs::read_to_string(path)
        .map(|value| value.replace("\r\n", "\n").to_ascii_lowercase())
        .map_err(|_| policy_rejected("required CI workflow cannot be read"))
}

fn require_text(workflow: &str, value: &str) -> Result<(), TaskFailure> {
    if workflow.contains(&value.to_ascii_lowercase()) {
        Ok(())
    } else {
        Err(policy_rejected(
            "CI workflow omitted a required security contract",
        ))
    }
}

fn reject_text(workflow: &str, prohibited: &[&str]) -> Result<(), TaskFailure> {
    if prohibited
        .iter()
        .any(|value| workflow.contains(&value.to_ascii_lowercase()))
    {
        return Err(policy_rejected(
            "CI workflow contains a prohibited privileged capability",
        ));
    }
    Ok(())
}

fn contains_unredacted_sensitive_value(value: &Value, field_name: Option<&str>) -> bool {
    let sensitive = field_name.is_some_and(|name| {
        let name = name.to_ascii_lowercase();
        [
            "credential",
            "secret",
            "token",
            "account",
            "email",
            "reference_image",
            "reference_image_bytes",
        ]
        .iter()
        .any(|needle| name.contains(needle))
    });
    if sensitive && value != "[REDACTED]" {
        return true;
    }
    match value {
        Value::Array(values) => values
            .iter()
            .any(|value| contains_unredacted_sensitive_value(value, field_name)),
        Value::Object(values) => values
            .iter()
            .any(|(name, value)| contains_unredacted_sensitive_value(value, Some(name))),
        _ => false,
    }
}

fn assert_rejected(policy: &CiSecurityPolicy, scenario: &str) -> Result<(), TaskFailure> {
    if validate_policy(policy).is_ok() {
        Err(policy_rejected(format!(
            "CI security fixture unexpectedly accepted `{scenario}`"
        )))
    } else {
        Ok(())
    }
}

fn policy_rejected(message: impl Into<String>) -> TaskFailure {
    TaskFailure::new(TaskFailureKind::SafetyPolicyRejected, message, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn repository_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap()
    }

    #[test]
    fn repository_ci_contract_is_fail_closed_and_has_all_five_modes() {
        let report = check_ci_security_contract(&repository_root()).unwrap();
        assert_eq!(report.validated_modes.len(), 5);
        assert_eq!(report.approval_label, REFERENCE_BASELINE_APPROVAL_LABEL);
        assert!(report.cache_max_bytes > report.artifact_max_bytes);
    }

    #[test]
    fn fixture_exercises_credentials_permissions_external_branch_baseline_and_provider_rejections()
    {
        let report = run_ci_security_fixture(&repository_root()).unwrap();
        assert_eq!(report.rejected_scenarios.len(), 6);
    }

    #[test]
    fn protected_reference_changes_require_the_exact_label() {
        let policy = load_policy(&repository_root().join(POLICY_PATH)).unwrap();
        assert!(
            reference_or_baseline_change_requires_approval(
                &policy,
                &["tools/ui-visual-audit/fixtures/references/test.png".to_owned()],
                &["unrelated".to_owned()],
            )
            .unwrap()
        );
        assert!(
            !reference_or_baseline_change_requires_approval(
                &policy,
                &["tools/ui-visual-audit/fixtures/references/test.png".to_owned()],
                &[REFERENCE_BASELINE_APPROVAL_LABEL.to_owned()],
            )
            .unwrap()
        );
    }
}
