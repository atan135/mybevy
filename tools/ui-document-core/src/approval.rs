//! Closed, read-only contract for a promoted approved document registration declaration.
//!
//! This adapter deliberately does not route a page or generate game code. A game screen must
//! explicitly choose when to register the resulting declarative page with its own route lifecycle.

use super::{
    UiActionId, UiActionTrigger, UiAssetId, UiBindingScope, UiBindingType, UiDocument,
    UiDocumentId, UiDocumentLayer, UiDocumentPanel, UiDocumentPreviewRegistration,
    UiDocumentSourcePath, UiDocumentSourceRoot, UiHostBindingKey, UiNode, UiNodeId, UiPageState,
    UiTargetProfile,
};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    str::FromStr,
};

pub const UI_APPROVED_DOCUMENT_REGISTRATION_PROTOCOL_VERSION: u32 = 2;
pub const UI_APPROVED_DOCUMENT_HOST_CONTRACT_VERSION: u32 = 1;
const LEGACY_REGISTRATION_PROTOCOL_VERSION: u32 = 1;
const REGISTRATION_KIND: &str = "ui_document_promotion_registration";
const LEGACY_REGISTRATION_TEMPLATE_VERSION: u32 = 1;
const REGISTRATION_TEMPLATE_VERSION: u32 = 2;
const REQUIRED_AUDIT_PROFILES: [&str; 4] = [
    "desktop",
    "phone-landscape",
    "phone-1080p-landscape",
    "tablet-landscape",
];

/// Closed, game-owned capabilities that an approved document may reference.
///
/// The contract intentionally contains document-facing names only. It cannot name a Rust type,
/// handler, system, message, URL, filesystem path, or executable command.
#[derive(Clone, Debug, PartialEq)]
pub struct UiApprovedDocumentHostContract {
    version: u32,
    document_id: UiDocumentId,
    owner: String,
    route: String,
    panel: UiDocumentPanel,
    layer: UiDocumentLayer,
    page_state: UiPageState,
    audit_profiles: Vec<String>,
    bindings: BTreeMap<UiHostBindingKey, UiBindingType>,
    actions: BTreeMap<UiActionId, BTreeSet<UiNodeId>>,
    resources: BTreeSet<UiAssetId>,
}

impl UiApprovedDocumentHostContract {
    pub fn new(
        version: u32,
        document_id: UiDocumentId,
        owner: impl Into<String>,
        route: impl Into<String>,
        panel: UiDocumentPanel,
        layer: UiDocumentLayer,
        page_state: UiPageState,
        audit_profiles: Vec<String>,
        bindings: BTreeMap<UiHostBindingKey, UiBindingType>,
        actions: BTreeMap<UiActionId, BTreeSet<UiNodeId>>,
        resources: BTreeSet<UiAssetId>,
    ) -> Result<Self, UiApprovedDocumentRegistrationError> {
        let owner = owner.into();
        let route = route.into();
        if version != UI_APPROVED_DOCUMENT_HOST_CONTRACT_VERSION
            || !safe_registration_label(&owner)
            || !safe_registration_label(&route)
            || !matches!(
                (panel, layer),
                (
                    UiDocumentPanel::Page | UiDocumentPanel::Hud,
                    UiDocumentLayer::Page
                ) | (UiDocumentPanel::Floating, UiDocumentLayer::Floating)
            )
            || page_state != UiPageState::initial()
            || normalized_profiles(&audit_profiles).is_none()
            || actions.values().any(BTreeSet::is_empty)
        {
            return Err(UiApprovedDocumentRegistrationError::new(
                "UI_APPROVED_REGISTRATION_HOST_CONTRACT_INVALID",
                "approved host contract identity, version, audit profiles, or action sources are invalid",
            ));
        }
        Ok(Self {
            version,
            document_id,
            owner,
            route,
            panel,
            layer,
            page_state,
            audit_profiles: REQUIRED_AUDIT_PROFILES.map(str::to_owned).to_vec(),
            bindings,
            actions,
            resources,
        })
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn bindings(&self) -> &BTreeMap<UiHostBindingKey, UiBindingType> {
        &self.bindings
    }

    pub fn actions(&self) -> &BTreeMap<UiActionId, BTreeSet<UiNodeId>> {
        &self.actions
    }

    pub fn resources(&self) -> &BTreeSet<UiAssetId> {
        &self.resources
    }

    fn matches_registration(&self, registration: &UiApprovedDocumentRegistration) -> bool {
        self.document_id == registration.document_id
            && self.owner == registration.owner
            && self.route == registration.route
            && self.panel == registration.panel
            && self.layer == registration.layer
            && self.page_state == registration.page_state
            && self.audit_profiles == registration.audit_profiles
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiApprovedDocumentAuditReport {
    pub host_contract_version: Option<u32>,
    pub actions: Vec<String>,
    pub bindings: Vec<String>,
    pub canonical_document_sha256: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiApprovedDocumentRegistration {
    document_id: UiDocumentId,
    source_path: UiDocumentSourcePath,
    owner: String,
    route: String,
    panel: UiDocumentPanel,
    layer: UiDocumentLayer,
    page_state: UiPageState,
    audit_profiles: Vec<String>,
    host_contract: Option<UiApprovedDocumentHostContract>,
}

impl UiApprovedDocumentRegistration {
    pub fn document_id(&self) -> &UiDocumentId {
        &self.document_id
    }

    pub fn source_path(&self) -> &UiDocumentSourcePath {
        &self.source_path
    }

    pub fn owner(&self) -> &str {
        &self.owner
    }

    /// A review-only route label. The adapter never dispatches a game route from this string.
    pub fn route(&self) -> &str {
        &self.route
    }

    pub fn panel(&self) -> UiDocumentPanel {
        self.panel
    }

    pub fn layer(&self) -> UiDocumentLayer {
        self.layer
    }

    pub fn page_state(&self) -> &UiPageState {
        &self.page_state
    }

    pub fn audit_profiles(&self) -> &[String] {
        &self.audit_profiles
    }

    pub fn host_contract(&self) -> Option<&UiApprovedDocumentHostContract> {
        self.host_contract.as_ref()
    }

    /// Converts a reviewed declaration into the existing formal preview/runtime registration.
    /// A game-owned route adapter must call this explicitly during its own lifecycle.
    pub fn to_preview_registration(
        &self,
        source_json: String,
        target_profile: UiTargetProfile,
    ) -> Result<UiDocumentPreviewRegistration, UiApprovedDocumentRegistrationError> {
        self.to_preview_registration_with_contract(source_json, target_profile, None)
    }

    /// Converts a reviewed declaration only after the game passes the exact contract it owns.
    /// Older registrations have no contract and therefore retain their zero-business-capability
    /// behavior even when a newer host exists.
    pub fn to_preview_registration_with_contract(
        &self,
        source_json: String,
        target_profile: UiTargetProfile,
        game_contract: Option<&UiApprovedDocumentHostContract>,
    ) -> Result<UiDocumentPreviewRegistration, UiApprovedDocumentRegistrationError> {
        self.validate_document_source_contract(&source_json)?;
        if let Some(registration_contract) = &self.host_contract {
            let Some(game_contract) = game_contract else {
                return Err(UiApprovedDocumentRegistrationError::new(
                    "UI_APPROVED_REGISTRATION_HOST_CONTRACT_REQUIRED",
                    "approved registration with business capabilities requires an explicit game host contract",
                ));
            };
            if registration_contract != game_contract || !game_contract.matches_registration(self) {
                return Err(UiApprovedDocumentRegistrationError::new(
                    "UI_APPROVED_REGISTRATION_HOST_CONTRACT_MISMATCH",
                    "approved registration contract does not exactly match the game host",
                ));
            }
        }
        let audit = self.audit_report(&source_json)?;
        Ok(UiDocumentPreviewRegistration {
            document_id: self.document_id.clone(),
            owner: self.owner.clone(),
            source_path: self.source_path.clone(),
            source_json,
            panel: self.panel,
            layer: self.layer,
            target_profile,
            page_state: self.page_state.clone(),
            owner_alive: true,
            host_bindings: self
                .host_contract
                .as_ref()
                .map(|contract| contract.bindings.clone())
                .unwrap_or_default(),
            watch: false,
            open_on_register: true,
            audit_profiles: self.audit_profiles.clone(),
            approval_audit: Some(super::UiDocumentApprovalAuditMetadata {
                host_contract_version: audit.host_contract_version,
                actions: audit.actions,
                bindings: audit.bindings,
                canonical_document_sha256: audit.canonical_document_sha256,
            }),
        })
    }

    /// Validates the document-facing part of the registration contract without supplying a game
    /// host. Development tooling uses this before it writes a registration; the game still must
    /// pass its independently registered contract before runtime registration is allowed.
    pub fn validate_document_source_contract(
        &self,
        source_json: &str,
    ) -> Result<(), UiApprovedDocumentRegistrationError> {
        let validation = UiDocument::validate_json(&source_json);
        let document = validation.validated().ok_or_else(|| {
            UiApprovedDocumentRegistrationError::new(
                "UI_APPROVED_REGISTRATION_DOCUMENT_INVALID",
                "approved registration source does not pass formal UiDocument validation",
            )
        })?;
        if document.document().document_id != self.document_id {
            return Err(UiApprovedDocumentRegistrationError::new(
                "UI_APPROVED_REGISTRATION_DOCUMENT_ID_MISMATCH",
                "approved registration document_id differs from its source document",
            ));
        }
        match &self.host_contract {
            Some(contract) => validate_document_contract(document.document(), contract)?,
            None => reject_business_fields(&serde_json::from_str::<Value>(&source_json).map_err(
                |_| {
                    UiApprovedDocumentRegistrationError::new(
                        "UI_APPROVED_REGISTRATION_DOCUMENT_INVALID",
                        "approved registration source cannot be decoded as JSON",
                    )
                },
            )?)?,
        }
        Ok(())
    }

    pub fn audit_report(
        &self,
        source_json: &str,
    ) -> Result<UiApprovedDocumentAuditReport, UiApprovedDocumentRegistrationError> {
        self.validate_document_source_contract(source_json)?;
        let validation = UiDocument::validate_json(source_json);
        let document = validation.validated().ok_or_else(|| {
            UiApprovedDocumentRegistrationError::new(
                "UI_APPROVED_REGISTRATION_DOCUMENT_INVALID",
                "approved registration source does not pass formal UiDocument validation",
            )
        })?;
        if document.document().document_id != self.document_id {
            return Err(UiApprovedDocumentRegistrationError::new(
                "UI_APPROVED_REGISTRATION_DOCUMENT_ID_MISMATCH",
                "approved registration document_id differs from its source document",
            ));
        }
        let canonical = document
            .document()
            .to_canonical_json_pretty()
            .map_err(|_| {
                UiApprovedDocumentRegistrationError::new(
                    "UI_APPROVED_REGISTRATION_DOCUMENT_INVALID",
                    "approved registration source cannot be canonicalized",
                )
            })?;
        let (actions, bindings) = if let Some(contract) = &self.host_contract {
            (
                contract
                    .actions
                    .keys()
                    .map(|id| id.as_str().to_owned())
                    .collect(),
                contract.bindings.keys().map(binding_key_label).collect(),
            )
        } else {
            (Vec::new(), Vec::new())
        };
        Ok(UiApprovedDocumentAuditReport {
            host_contract_version: self.host_contract.as_ref().map(|contract| contract.version),
            actions,
            bindings,
            canonical_document_sha256: format!("{:x}", Sha256::digest(canonical.as_bytes())),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiApprovedDocumentRegistrationError {
    code: &'static str,
    message: String,
}

impl UiApprovedDocumentRegistrationError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }
}

impl fmt::Display for UiApprovedDocumentRegistrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for UiApprovedDocumentRegistrationError {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistrationFile {
    protocol_version: u32,
    kind: String,
    template_version: u32,
    document_id: String,
    source: RegistrationSource,
    owner: String,
    route: String,
    panel: String,
    layer: String,
    page_state: String,
    audit_profiles: Vec<String>,
    i18n_keys: Vec<String>,
    theme_tokens: Vec<String>,
    #[serde(default)]
    action_or_binding_registration: Vec<String>,
    #[serde(default)]
    host_contract: Option<RegistrationHostContract>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistrationSource {
    root: String,
    relative_path: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistrationHostContract {
    version: u32,
    bindings: Vec<RegistrationBinding>,
    actions: Vec<RegistrationAction>,
    resources: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistrationBinding {
    scope: UiBindingScope,
    path: String,
    value_type: UiBindingType,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistrationAction {
    id: String,
    sources: Vec<String>,
}

/// Parses only the closed promotion declaration emitted by the development tool.
pub fn parse_approved_document_registration(
    source: &str,
) -> Result<UiApprovedDocumentRegistration, UiApprovedDocumentRegistrationError> {
    let file: RegistrationFile = serde_json::from_str(source).map_err(|_| {
        UiApprovedDocumentRegistrationError::new(
            "UI_APPROVED_REGISTRATION_SCHEMA_INVALID",
            "approved registration must match the closed JSON schema",
        )
    })?;
    let registration_contract = match (
        file.protocol_version,
        file.template_version,
        file.host_contract,
    ) {
        (LEGACY_REGISTRATION_PROTOCOL_VERSION, LEGACY_REGISTRATION_TEMPLATE_VERSION, None) => None,
        (
            UI_APPROVED_DOCUMENT_REGISTRATION_PROTOCOL_VERSION,
            REGISTRATION_TEMPLATE_VERSION,
            Some(contract),
        ) => Some(contract),
        _ => {
            return Err(UiApprovedDocumentRegistrationError::new(
                "UI_APPROVED_REGISTRATION_VERSION_UNSUPPORTED",
                "approved registration protocol/template version is unsupported",
            ));
        }
    };
    if file.kind != REGISTRATION_KIND
        || file.source.root != "approved"
        || !file.i18n_keys.is_empty()
        || !file.theme_tokens.is_empty()
        || !file.action_or_binding_registration.is_empty()
    {
        return Err(UiApprovedDocumentRegistrationError::new(
            "UI_APPROVED_REGISTRATION_CLOSED_FIELD_REJECTED",
            "approved registration contains an unsupported protocol field or business registration",
        ));
    }
    let document_id = UiDocumentId::from_str(&file.document_id).map_err(|_| {
        UiApprovedDocumentRegistrationError::new(
            "UI_APPROVED_REGISTRATION_DOCUMENT_ID_INVALID",
            "approved registration document_id is invalid",
        )
    })?;
    let panel = match file.panel.as_str() {
        "page" => UiDocumentPanel::Page,
        "hud" => UiDocumentPanel::Hud,
        "floating" => UiDocumentPanel::Floating,
        _ => {
            return Err(UiApprovedDocumentRegistrationError::new(
                "UI_APPROVED_REGISTRATION_CLOSED_FIELD_REJECTED",
                "approved registration panel is outside the closed page/HUD/floating set",
            ));
        }
    };
    let layer = match file.layer.as_str() {
        "page" => UiDocumentLayer::Page,
        "floating" => UiDocumentLayer::Floating,
        _ => {
            return Err(UiApprovedDocumentRegistrationError::new(
                "UI_APPROVED_REGISTRATION_CLOSED_FIELD_REJECTED",
                "approved registration layer is outside the closed page/floating set",
            ));
        }
    };
    let source_path =
        UiDocumentSourcePath::new(UiDocumentSourceRoot::Approved, file.source.relative_path)
            .map_err(|_| {
                UiApprovedDocumentRegistrationError::new(
                    "UI_APPROVED_REGISTRATION_SOURCE_INVALID",
                    "approved registration source path is invalid",
                )
            })?;
    if !safe_registration_label(&file.owner) || !safe_registration_label(&file.route) {
        return Err(UiApprovedDocumentRegistrationError::new(
            "UI_APPROVED_REGISTRATION_OWNER_ROUTE_INVALID",
            "approved registration owner or route is invalid",
        ));
    }
    let page_state = UiPageState::from_str(&file.page_state).map_err(|_| {
        UiApprovedDocumentRegistrationError::new(
            "UI_APPROVED_REGISTRATION_PAGE_STATE_INVALID",
            "approved registration page state is invalid",
        )
    })?;
    if page_state != UiPageState::initial() || normalized_profiles(&file.audit_profiles).is_none() {
        return Err(UiApprovedDocumentRegistrationError::new(
            "UI_APPROVED_REGISTRATION_AUDIT_INVALID",
            "approved registration page state or audit profiles differ from the closed template",
        ));
    }
    let audit_profiles = REQUIRED_AUDIT_PROFILES.map(str::to_owned).to_vec();
    let host_contract = registration_contract
        .map(|contract| {
            parse_host_contract(
                contract,
                document_id.clone(),
                file.owner.clone(),
                file.route.clone(),
                panel,
                layer,
                page_state.clone(),
                audit_profiles.clone(),
            )
        })
        .transpose()?;
    Ok(UiApprovedDocumentRegistration {
        document_id,
        source_path,
        owner: file.owner,
        route: file.route,
        panel,
        layer,
        page_state,
        audit_profiles,
        host_contract,
    })
}

fn parse_host_contract(
    contract: RegistrationHostContract,
    document_id: UiDocumentId,
    owner: String,
    route: String,
    panel: UiDocumentPanel,
    layer: UiDocumentLayer,
    page_state: UiPageState,
    audit_profiles: Vec<String>,
) -> Result<UiApprovedDocumentHostContract, UiApprovedDocumentRegistrationError> {
    let mut bindings = BTreeMap::new();
    for binding in contract.bindings {
        if matches!(binding.scope, UiBindingScope::Local | UiBindingScope::Item) {
            return Err(UiApprovedDocumentRegistrationError::new(
                "UI_APPROVED_REGISTRATION_HOST_CONTRACT_INVALID",
                "approved host contract may only declare document or owner bindings",
            ));
        }
        let path = super::UiBindingPath::from_str(&binding.path).map_err(|_| {
            UiApprovedDocumentRegistrationError::new(
                "UI_APPROVED_REGISTRATION_HOST_CONTRACT_INVALID",
                "approved host contract binding path is invalid",
            )
        })?;
        if bindings
            .insert(
                UiHostBindingKey::new(binding.scope, path),
                binding.value_type,
            )
            .is_some()
        {
            return Err(UiApprovedDocumentRegistrationError::new(
                "UI_APPROVED_REGISTRATION_HOST_CONTRACT_INVALID",
                "approved host contract has duplicate bindings",
            ));
        }
    }
    let mut actions = BTreeMap::new();
    for action in contract.actions {
        let id = UiActionId::from_str(&action.id).map_err(|_| {
            UiApprovedDocumentRegistrationError::new(
                "UI_APPROVED_REGISTRATION_HOST_CONTRACT_INVALID",
                "approved host contract action ID is invalid",
            )
        })?;
        let mut sources = BTreeSet::new();
        for source in action.sources {
            let source = UiNodeId::from_str(&source).map_err(|_| {
                UiApprovedDocumentRegistrationError::new(
                    "UI_APPROVED_REGISTRATION_HOST_CONTRACT_INVALID",
                    "approved host contract action source is invalid",
                )
            })?;
            if !sources.insert(source) {
                return Err(UiApprovedDocumentRegistrationError::new(
                    "UI_APPROVED_REGISTRATION_HOST_CONTRACT_INVALID",
                    "approved host contract action has duplicate sources",
                ));
            }
        }
        if actions.insert(id, sources).is_some() {
            return Err(UiApprovedDocumentRegistrationError::new(
                "UI_APPROVED_REGISTRATION_HOST_CONTRACT_INVALID",
                "approved host contract has duplicate action IDs",
            ));
        }
    }
    let mut resources = BTreeSet::new();
    for resource in contract.resources {
        let resource = UiAssetId::from_str(&resource).map_err(|_| {
            UiApprovedDocumentRegistrationError::new(
                "UI_APPROVED_REGISTRATION_HOST_CONTRACT_INVALID",
                "approved host contract resource ID is invalid",
            )
        })?;
        if !resources.insert(resource) {
            return Err(UiApprovedDocumentRegistrationError::new(
                "UI_APPROVED_REGISTRATION_HOST_CONTRACT_INVALID",
                "approved host contract has duplicate resource IDs",
            ));
        }
    }
    UiApprovedDocumentHostContract::new(
        contract.version,
        document_id,
        owner,
        route,
        panel,
        layer,
        page_state,
        audit_profiles,
        bindings,
        actions,
        resources,
    )
}

fn normalized_profiles(values: &[String]) -> Option<Vec<String>> {
    if values.len() != REQUIRED_AUDIT_PROFILES.len() {
        return None;
    }
    let mut values = values.to_vec();
    values.sort();
    values.dedup();
    let mut expected = REQUIRED_AUDIT_PROFILES.map(str::to_owned).to_vec();
    expected.sort();
    (values == expected).then_some(expected)
}

fn safe_registration_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_uppercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'.' | b'_' | b'-')
        })
}

fn validate_document_contract(
    document: &UiDocument,
    contract: &UiApprovedDocumentHostContract,
) -> Result<(), UiApprovedDocumentRegistrationError> {
    let mut bindings = BTreeMap::new();
    for (path, declaration) in &document.bindings {
        if matches!(
            declaration.scope,
            UiBindingScope::Document | UiBindingScope::Owner
        ) {
            bindings.insert(
                UiHostBindingKey::new(declaration.scope, path.clone()),
                declaration.value_type.clone(),
            );
        }
    }
    if bindings != contract.bindings {
        return Err(UiApprovedDocumentRegistrationError::new(
            "UI_APPROVED_REGISTRATION_BINDING_CONTRACT_MISMATCH",
            "approved document bindings differ from the registered game host contract",
        ));
    }
    let mut actions = BTreeMap::<UiActionId, BTreeSet<UiNodeId>>::new();
    collect_document_actions(&document.root, &mut actions);
    if actions != contract.actions {
        return Err(UiApprovedDocumentRegistrationError::new(
            "UI_APPROVED_REGISTRATION_ACTION_CONTRACT_MISMATCH",
            "approved document actions or source nodes differ from the registered game host contract",
        ));
    }
    let resources: BTreeSet<UiAssetId> = document.assets.keys().cloned().collect();
    if resources != contract.resources {
        return Err(UiApprovedDocumentRegistrationError::new(
            "UI_APPROVED_REGISTRATION_RESOURCE_CONTRACT_MISMATCH",
            "approved document resources differ from the registration resource contract",
        ));
    }
    Ok(())
}

fn collect_document_actions(node: &UiNode, actions: &mut BTreeMap<UiActionId, BTreeSet<UiNodeId>>) {
    for trigger in [
        UiActionTrigger::Click,
        UiActionTrigger::Change,
        UiActionTrigger::Submit,
    ] {
        if let Some(action) = super::node_action(node, trigger) {
            actions
                .entry(action.action.clone())
                .or_default()
                .insert(node.id().clone());
        }
    }
    for child in node.children() {
        collect_document_actions(child, actions);
    }
}

fn binding_key_label(key: &UiHostBindingKey) -> String {
    let scope = match key.scope {
        UiBindingScope::Document => "document",
        UiBindingScope::Owner => "owner",
        UiBindingScope::Local => "local",
        UiBindingScope::Item => "item",
    };
    format!("{scope}:{}", key.path)
}

fn reject_business_fields(value: &Value) -> Result<(), UiApprovedDocumentRegistrationError> {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                if matches!(
                    key.as_str(),
                    "action" | "on_click" | "on_change" | "on_submit" | "binding_path" | "i18n_key"
                ) || (key == "bindings"
                    && child.as_object().is_some_and(|value| !value.is_empty()))
                {
                    return Err(UiApprovedDocumentRegistrationError::new(
                        "UI_APPROVED_REGISTRATION_BUSINESS_FIELD_REJECTED",
                        "approved registration source contains an action, binding, or i18n field",
                    ));
                }
                reject_business_fields(child)?;
            }
        }
        Value::Array(values) => {
            for value in values {
                reject_business_fields(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}
