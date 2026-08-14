//! Read-only bridge from a reviewable generation task to the game-owned approved host catalog.
//!
//! The task repeats the UI-facing allowlist for auditability. This module never trusts that copy:
//! it resolves the exact owner/route/document tuple from the production catalog and delegates the
//! final document check to the project's approved-registration parser.

use crate::{
    contract::{GenerationHostBindingRequest, GenerationHostContractRequest},
    lifecycle::TaskFailure,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeSet, fs, path::Path};
use ui_document_core::parse_approved_document_registration;

pub const HOST_CONTRACT_CATALOG_PATH: &str = "project/assets/ui/documents/host_contracts.v1.json";
pub const TRUSTED_RESOURCE_CATALOG_PATH: &str =
    "tools/ui-generation/assets/ui_asset_catalog.v1.json";
pub const REQUIRED_AUDIT_PROFILES: [&str; 4] = [
    "desktop",
    "phone-landscape",
    "phone-1080p-landscape",
    "tablet-landscape",
];
const MAX_HOST_CONTRACT_CATALOG_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedGenerationHostContract {
    pub document_id: String,
    pub owner: String,
    pub route: String,
    pub version: u32,
    pub allowed_actions: Vec<String>,
    pub allowed_bindings: Vec<GenerationHostBindingRequest>,
    pub target_profiles: Vec<String>,
    pub resource_catalog: String,
    pub forbidden_capabilities: Vec<crate::contract::GenerationForbiddenCapability>,
    pub host_contract: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    schema_version: u32,
    contracts: Vec<CatalogEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogEntry {
    document_id: String,
    owner: String,
    route: String,
    host_contract: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogHostContract {
    version: u32,
    bindings: Vec<GenerationHostBindingRequest>,
    actions: Vec<CatalogAction>,
    resources: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogAction {
    id: String,
    sources: Vec<String>,
}

pub fn resolve_repository_host_contract(
    repository_root: &Path,
    request: &GenerationHostContractRequest,
) -> Result<ResolvedGenerationHostContract, TaskFailure> {
    let repository_root = fs::canonicalize(repository_root).map_err(|_| {
        TaskFailure::invalid("generation host contract repository root cannot be resolved")
    })?;
    let catalog_path = repository_root.join(HOST_CONTRACT_CATALOG_PATH);
    let metadata = fs::symlink_metadata(&catalog_path)
        .map_err(|_| TaskFailure::invalid("generation host contract catalog is unavailable"))?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() == 0
        || metadata.len() > MAX_HOST_CONTRACT_CATALOG_BYTES
    {
        return Err(TaskFailure::invalid(
            "generation host contract catalog must be a bounded regular file",
        ));
    }
    let bytes = fs::read(&catalog_path)
        .map_err(|_| TaskFailure::invalid("generation host contract catalog cannot be read"))?;
    let catalog: Catalog = serde_json::from_slice(&bytes)
        .map_err(|_| TaskFailure::invalid("generation host contract catalog is invalid"))?;
    if catalog.schema_version != 1 || catalog.contracts.is_empty() || catalog.contracts.len() > 256
    {
        return Err(TaskFailure::invalid(
            "generation host contract catalog schema version or size is invalid",
        ));
    }
    let entries = catalog
        .contracts
        .iter()
        .filter(|entry| {
            entry.document_id == request.document_id
                && entry.owner == request.owner
                && entry.route == request.route
        })
        .collect::<Vec<_>>();
    if entries.len() != 1 {
        return Err(TaskFailure::invalid(
            "generation task must select exactly one pre-registered production host contract",
        ));
    }
    let entry = entries[0];
    let contract: CatalogHostContract = serde_json::from_value(entry.host_contract.clone())
        .map_err(|_| {
            TaskFailure::invalid("generation host contract capability schema is invalid")
        })?;
    verify_catalog_entry(entry, &contract)?;
    verify_task_copy(request, &contract)?;

    Ok(ResolvedGenerationHostContract {
        document_id: request.document_id.clone(),
        owner: request.owner.clone(),
        route: request.route.clone(),
        version: request.version,
        allowed_actions: sorted_actions(&contract.actions),
        allowed_bindings: sorted_bindings(&contract.bindings),
        target_profiles: REQUIRED_AUDIT_PROFILES.map(str::to_owned).to_vec(),
        resource_catalog: TRUSTED_RESOURCE_CATALOG_PATH.to_owned(),
        forbidden_capabilities: request.forbidden_capabilities.clone(),
        host_contract: entry.host_contract.clone(),
    })
}

impl ResolvedGenerationHostContract {
    pub fn preview_registration_json(&self) -> Result<String, TaskFailure> {
        serde_json::to_string(&self.registration_value()).map_err(|_| {
            TaskFailure::invalid("generation host preview registration cannot be serialized")
        })
    }

    /// Uses the same closed parser and source-contract validation that approved runtime pages use.
    pub fn validate_document_source(&self, document_json: &str) -> Result<(), TaskFailure> {
        let registration = self.registration_value();
        let source = serde_json::to_string(&registration).map_err(|_| {
            TaskFailure::invalid("generation host registration cannot be serialized")
        })?;
        let registration = parse_approved_document_registration(&source).map_err(|error| {
            TaskFailure::invalid(format!(
                "generation host contract is rejected by the production registration parser: {}",
                error.code()
            ))
        })?;
        registration
            .validate_document_source_contract(document_json)
            .map_err(|error| {
                TaskFailure::invalid(format!(
                    "generation document violates the production host contract: {}",
                    error.code()
                ))
            })
    }

    fn registration_value(&self) -> Value {
        serde_json::json!({
            "protocol_version": 2,
            "kind": "ui_document_promotion_registration",
            "template_version": 2,
            "document_id": self.document_id,
            "source": {
                "root": "approved",
                "relative_path": "generation_host_contract_probe/document.v1.json",
            },
            "owner": self.owner,
            "route": self.route,
            "panel": "page",
            "layer": "page",
            "page_state": "initial",
            "audit_profiles": REQUIRED_AUDIT_PROFILES,
            "i18n_keys": [],
            "theme_tokens": [],
            "action_or_binding_registration": [],
            "host_contract": self.host_contract,
        })
    }
}

fn verify_catalog_entry(
    entry: &CatalogEntry,
    contract: &CatalogHostContract,
) -> Result<(), TaskFailure> {
    if contract.version != 1
        || contract.actions.len() > 256
        || contract.bindings.len() > 256
        || contract.resources.len() > 256
        || !entry.host_contract.is_object()
    {
        return Err(TaskFailure::invalid(
            "generation host contract version or capability budget is invalid",
        ));
    }
    let actions = sorted_actions(&contract.actions);
    if actions.len() != contract.actions.len()
        || contract
            .actions
            .iter()
            .any(|action| action.sources.is_empty())
    {
        return Err(TaskFailure::invalid(
            "generation host contract actions must be unique and have declared node sources",
        ));
    }
    let bindings = sorted_bindings(&contract.bindings);
    if bindings.len() != contract.bindings.len() {
        return Err(TaskFailure::invalid(
            "generation host contract bindings must be unique",
        ));
    }
    let resources = contract.resources.iter().collect::<BTreeSet<_>>();
    if resources.len() != contract.resources.len() {
        return Err(TaskFailure::invalid(
            "generation host contract resources must be unique",
        ));
    }
    Ok(())
}

fn verify_task_copy(
    request: &GenerationHostContractRequest,
    contract: &CatalogHostContract,
) -> Result<(), TaskFailure> {
    if request.version != contract.version
        || request.resource_catalog != TRUSTED_RESOURCE_CATALOG_PATH
        || request.target_profiles != REQUIRED_AUDIT_PROFILES.map(str::to_owned)
        || request.allowed_actions != sorted_actions(&contract.actions)
        || request.allowed_bindings != sorted_bindings(&contract.bindings)
    {
        return Err(TaskFailure::invalid(
            "generation task host allowlist, profile matrix, version, or resource catalog differs from the trusted production catalog",
        ));
    }
    Ok(())
}

fn sorted_actions(actions: &[CatalogAction]) -> Vec<String> {
    let mut actions = actions
        .iter()
        .map(|action| action.id.clone())
        .collect::<Vec<_>>();
    actions.sort();
    actions
}

fn sorted_bindings(bindings: &[GenerationHostBindingRequest]) -> Vec<GenerationHostBindingRequest> {
    let mut bindings = bindings.to_vec();
    bindings.sort_by(|left, right| {
        (left.scope.as_str(), left.path.as_str()).cmp(&(right.scope.as_str(), right.path.as_str()))
    });
    bindings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{GenerationForbiddenCapability, GenerationTask};
    use serde_json::Value;
    use std::{fs, path::PathBuf};
    use ui_document_core::UiDocument;

    fn request() -> GenerationHostContractRequest {
        GenerationHostContractRequest {
            document_id: "approved.business_acceptance".to_owned(),
            owner: "approved_business_acceptance".to_owned(),
            route: "ui_approved_business_acceptance".to_owned(),
            version: 1,
            allowed_actions: vec!["approved.acceptance_continue".to_owned()],
            allowed_bindings: vec![GenerationHostBindingRequest {
                scope: "owner".to_owned(),
                path: "acceptance.status".to_owned(),
                value_type: serde_json::json!({"kind": "string"}),
            }],
            target_profiles: REQUIRED_AUDIT_PROFILES.map(str::to_owned).to_vec(),
            resource_catalog: TRUSTED_RESOURCE_CATALOG_PATH.to_owned(),
            forbidden_capabilities: vec![
                GenerationForbiddenCapability::RustCode,
                GenerationForbiddenCapability::Scripts,
                GenerationForbiddenCapability::ArbitraryActions,
                GenerationForbiddenCapability::ArbitraryBindings,
                GenerationForbiddenCapability::ArbitraryResources,
                GenerationForbiddenCapability::BusinessProtocol,
                GenerationForbiddenCapability::CargoConfiguration,
                GenerationForbiddenCapability::AndroidConfiguration,
            ],
        }
    }

    fn repository_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap()
    }

    #[test]
    fn task_copy_must_exactly_match_the_game_owned_host_catalog() {
        let resolved = resolve_repository_host_contract(&repository_root(), &request()).unwrap();
        assert_eq!(resolved.version, 1);
        assert_eq!(resolved.allowed_actions, ["approved.acceptance_continue"]);

        let mut unknown = request();
        unknown.allowed_actions.push("unknown.action".to_owned());
        assert!(resolve_repository_host_contract(&repository_root(), &unknown).is_err());
    }

    #[test]
    fn stage9_task_fixtures_accept_only_the_exact_catalog_copy() {
        let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/stage9");
        let valid =
            GenerationTask::load_json(&fixture_root.join("host_contract.task.valid.json")).unwrap();
        resolve_repository_host_contract(&repository_root(), valid.host_contract.as_ref().unwrap())
            .unwrap();

        for fixture in [
            "failure.unknown_action.task.json",
            "failure.unknown_binding.task.json",
            "failure.unauthorized_resource.task.json",
        ] {
            let rejected = GenerationTask::load_json(&fixture_root.join(fixture)).unwrap();
            assert!(
                resolve_repository_host_contract(
                    &repository_root(),
                    rejected.host_contract.as_ref().unwrap()
                )
                .is_err()
            );
        }
    }

    #[test]
    fn production_host_parser_rejects_unknown_actions_bindings_and_resources() {
        let contract = resolve_repository_host_contract(&repository_root(), &request()).unwrap();
        let path = repository_root().join(
            "project/assets/ui/documents/approved/business_acceptance_fixture/document.v1.json",
        );
        let source = fs::read_to_string(path).unwrap();
        contract.validate_document_source(&source).unwrap();

        let mut unknown_action: Value = serde_json::from_str(&source).unwrap();
        unknown_action["root"]["children"][1]["on_click"]["action"] =
            serde_json::json!("unknown.action");
        assert!(
            contract
                .validate_document_source(&unknown_action.to_string())
                .is_err()
        );

        let mut unknown_binding: Value = serde_json::from_str(&source).unwrap();
        unknown_binding["bindings"]["unknown.value"] = serde_json::json!({
            "scope": "owner",
            "value_type": { "kind": "string" },
            "default": { "kind": "string", "value": "unexpected" },
            "missing": "use_default"
        });
        assert!(
            contract
                .validate_document_source(&unknown_binding.to_string())
                .is_err()
        );

        let mut unknown_resource: Value = serde_json::from_str(&source).unwrap();
        unknown_resource["assets"]["ui.icon.help"] = serde_json::json!({
            "kind": "icon",
            "source": { "kind": "packaged", "path": "ui/icons/help.png" }
        });
        assert!(
            contract
                .validate_document_source(&unknown_resource.to_string())
                .is_err()
        );

        let over_budget_list: Value = serde_json::from_str(include_str!(
            "../fixtures/stage9/failure.over_budget_list.document.json"
        ))
        .unwrap();
        assert!(
            !UiDocument::validate_json(&over_budget_list.to_string())
                .report
                .valid
        );
    }
}
