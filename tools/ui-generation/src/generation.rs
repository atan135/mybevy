use crate::{
    analysis::{
        MAX_ANALYSIS_ELEMENTS, MAX_ANALYSIS_UNCERTAINTIES, TextAdoption, UiReferenceAnalysis,
        UncertaintyKind, VisualElementKind,
    },
    asset_strategy::{
        ASSET_STRATEGY_PROTOCOL_VERSION, AssetCatalog, AssetDisposition, AssetStrategyManifest,
        CatalogAssetKind, MAX_ASSET_ENTRIES,
    },
    lifecycle::{TaskFailure, TaskFailureKind},
    planning::{
        MAX_PLAN_COMPONENTS, MAX_PLAN_TOKENS, PLANNING_PROTOCOL_VERSION, RecommendationScope,
        TokenOrigin, UiGenerationPlan, plan_analysis,
    },
    provider::{
        ProviderAttemptOutcome, ProviderExecution, ProviderOperation, ProviderRequest,
        StructuredOutputContract, is_safe_metadata_label,
    },
};
use project::framework::ui::document::tooling::{
    CURRENT_SCHEMA_VERSION, MIN_SUPPORTED_SCHEMA_VERSION, UI_DOCUMENT_MAX_BYTES,
    UiValidationReport, canonicalize_json, validate_json_bytes,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const GENERATION_OUTPUT_SCHEMA_ID: &str = "ui-document-generation";
pub const GENERATION_OUTPUT_SCHEMA_VERSION: u32 = 1;
pub const MAX_GENERATION_INPUT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_GENERATION_RESPONSE_BYTES: usize = 512 * 1024;
pub const MAX_GENERATION_DISCLOSURES: usize = 512;
pub const MAX_MERGED_GENERATION_DISCLOSURES: usize = MAX_GENERATION_DISCLOSURES
    + (MAX_ANALYSIS_UNCERTAINTIES * 2)
    + MAX_PLAN_TOKENS
    + MAX_PLAN_COMPONENTS
    + MAX_ASSET_ENTRIES
    + (MAX_ANALYSIS_ELEMENTS * 2);
const MAX_DISCLOSURE_MESSAGE_BYTES: usize = 512;
const MAX_GENERATION_JSON_NODES: usize = 250_000;
const MAX_GENERATION_JSON_DEPTH: usize = 64;
const MAX_GENERATION_CONTAINER_ITEMS: usize = 20_000;
const MAX_GENERATION_STRING_BYTES: usize = 16 * 1024;
const MAX_GENERATION_TOTAL_STRING_BYTES: usize = 8 * 1024 * 1024;
const GENERATION_INSTRUCTION: &str = "Generate the requested UiDocument as the exact structured output contract. Use only supplied source-map node IDs, literal text decisions, registered assets, and protocol-supported fields. Do not invent actions, bindings, i18n keys, states, or business behavior.";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationParameters {
    pub temperature_milli: u16,
    pub max_output_bytes: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
}

impl GenerationParameters {
    pub fn new(
        temperature_milli: u16,
        max_output_bytes: u32,
        seed: Option<u64>,
    ) -> Result<Self, TaskFailure> {
        if temperature_milli > 2_000
            || max_output_bytes == 0
            || max_output_bytes as usize > MAX_GENERATION_RESPONSE_BYTES
        {
            return Err(TaskFailure::invalid(
                "generation parameters exceed the bounded temperature or output-size policy",
            ));
        }
        Ok(Self {
            temperature_milli,
            max_output_bytes,
            seed,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationConfiguration {
    document_id: String,
    ui_document_schema_version: u32,
    model_id: String,
    prompt_version: String,
    parameters: GenerationParameters,
}

impl GenerationConfiguration {
    pub fn new(
        document_id: impl Into<String>,
        model_id: impl Into<String>,
        prompt_version: impl Into<String>,
        parameters: GenerationParameters,
    ) -> Result<Self, TaskFailure> {
        Self::new_for_schema(
            document_id,
            CURRENT_SCHEMA_VERSION,
            model_id,
            prompt_version,
            parameters,
        )
    }

    pub fn new_for_schema(
        document_id: impl Into<String>,
        ui_document_schema_version: u32,
        model_id: impl Into<String>,
        prompt_version: impl Into<String>,
        parameters: GenerationParameters,
    ) -> Result<Self, TaskFailure> {
        let document_id = document_id.into();
        let model_id = model_id.into();
        let prompt_version = prompt_version.into();
        if !is_document_id(&document_id)
            || !(MIN_SUPPORTED_SCHEMA_VERSION..=CURRENT_SCHEMA_VERSION)
                .contains(&ui_document_schema_version)
            || !is_safe_metadata_label(&model_id, 128)
            || !is_safe_metadata_label(&prompt_version, 128)
        {
            return Err(TaskFailure::invalid(
                "generation document, model, and prompt identifiers must be bounded safe labels",
            ));
        }
        Ok(Self {
            document_id,
            ui_document_schema_version,
            model_id,
            prompt_version,
            parameters,
        })
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceMapEntry {
    pub reference_element_id: String,
    pub node_id: String,
    pub reference_id: String,
    pub evidence_ids: Vec<String>,
    pub document_path: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TextSourceStrategy {
    Literal,
    Unresolved,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TextSourceDecision {
    pub reference_element_id: String,
    pub node_id: String,
    pub strategy: TextSourceStrategy,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationDisclosure {
    pub code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_id: Option<String>,
    pub message: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationDisclosures {
    pub assumptions: Vec<GenerationDisclosure>,
    pub unimplemented_states: Vec<GenerationDisclosure>,
    pub required_new_components: Vec<GenerationDisclosure>,
    pub unsupported_capabilities: Vec<GenerationDisclosure>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderGenerationEnvelope {
    document: Value,
    #[serde(default)]
    assumptions: Vec<GenerationDisclosure>,
    #[serde(default)]
    unimplemented_states: Vec<GenerationDisclosure>,
    #[serde(default)]
    required_new_components: Vec<GenerationDisclosure>,
    #[serde(default)]
    unsupported_capabilities: Vec<GenerationDisclosure>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationTrace {
    pub provider_id: String,
    pub model_id: String,
    pub prompt_version: String,
    pub output_schema: StructuredOutputContract,
    pub ui_document_schema_version: u32,
    pub input_sha256: String,
    pub parameters: GenerationParameters,
    pub server_request_id: String,
    pub canonical_document_sha256: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedUiDocument {
    pub canonical_document_json: String,
    pub source_map: Vec<SourceMapEntry>,
    pub text_decisions: Vec<TextSourceDecision>,
    pub disclosures: GenerationDisclosures,
    pub trace: GenerationTrace,
    pub validation_report: UiValidationReport,
}

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
struct AllowedAsset {
    asset_id: String,
    path: String,
    kind: CatalogAssetKind,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct GenerationPolicy<'a> {
    document_id: &'a str,
    ui_document_schema_version: u32,
    source_map: &'a [SourceMapEntry],
    text_decisions: &'a [TextSourceDecision],
    allowed_assets: &'a [AllowedAsset],
    allow_actions: bool,
    allow_bindings: bool,
    allow_i18n_keys: bool,
    protocol_fields_only: bool,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct GenerationStructuredInputs<'a> {
    model_id: &'a str,
    prompt_version: &'a str,
    analysis: &'a UiReferenceAnalysis,
    plan: &'a UiGenerationPlan,
    asset_strategy: &'a AssetStrategyManifest,
    policy: GenerationPolicy<'a>,
    parameters: &'a GenerationParameters,
}

#[derive(Clone, Debug)]
pub struct PreparedGenerationRequest {
    request: ProviderRequest,
    analysis: UiReferenceAnalysis,
    plan: UiGenerationPlan,
    asset_strategy: AssetStrategyManifest,
    configuration: GenerationConfiguration,
    input_sha256: String,
    source_map: Vec<SourceMapEntry>,
    text_decisions: Vec<TextSourceDecision>,
    allowed_assets: Vec<AllowedAsset>,
}

impl PreparedGenerationRequest {
    pub fn request(&self) -> &ProviderRequest {
        &self.request
    }

    pub fn input_sha256(&self) -> &str {
        &self.input_sha256
    }
}

pub fn generation_output_contract() -> StructuredOutputContract {
    StructuredOutputContract::new(
        GENERATION_OUTPUT_SCHEMA_ID,
        GENERATION_OUTPUT_SCHEMA_VERSION,
    )
    .expect("generation output contract constants are valid")
}

pub fn prepare_generation_request(
    analysis: &UiReferenceAnalysis,
    plan: &UiGenerationPlan,
    asset_strategy: &AssetStrategyManifest,
    catalog: &AssetCatalog,
    configuration: GenerationConfiguration,
) -> Result<PreparedGenerationRequest, TaskFailure> {
    validate_trusted_inputs(analysis, plan, asset_strategy)?;
    let source_map = derive_source_map(analysis);
    let text_decisions = derive_text_decisions(analysis);
    let allowed_assets = collect_allowed_assets(asset_strategy, catalog)?;
    let structured = GenerationStructuredInputs {
        model_id: &configuration.model_id,
        prompt_version: &configuration.prompt_version,
        analysis,
        plan,
        asset_strategy,
        policy: GenerationPolicy {
            document_id: &configuration.document_id,
            ui_document_schema_version: configuration.ui_document_schema_version,
            source_map: &source_map,
            text_decisions: &text_decisions,
            allowed_assets: &allowed_assets,
            allow_actions: false,
            allow_bindings: false,
            allow_i18n_keys: false,
            protocol_fields_only: true,
        },
        parameters: &configuration.parameters,
    };
    let structured_value = serde_json::to_value(structured)
        .map_err(|error| TaskFailure::invalid(format!("generation inputs are invalid: {error}")))?;
    validate_value_budget(&structured_value, MAX_GENERATION_INPUT_BYTES)?;
    let input_bytes = serde_json::to_vec(&structured_value).map_err(|error| {
        TaskFailure::invalid(format!("generation inputs cannot be hashed: {error}"))
    })?;
    let input_sha256 = sha256(&input_bytes);
    let request = ProviderRequest::structured_generation(
        analysis.run_id.clone(),
        configuration.prompt_version.clone(),
        GENERATION_INSTRUCTION,
        structured_value,
        Vec::new(),
        generation_output_contract(),
    )?;
    Ok(PreparedGenerationRequest {
        request,
        analysis: analysis.clone(),
        plan: plan.clone(),
        asset_strategy: asset_strategy.clone(),
        configuration,
        input_sha256,
        source_map,
        text_decisions,
        allowed_assets,
    })
}

pub fn validate_generation_execution(
    execution: &ProviderExecution,
    prepared: &PreparedGenerationRequest,
) -> Result<GeneratedUiDocument, TaskFailure> {
    let request_id = execution.response.server_request_id.as_ref();
    let result = validate_generation_execution_inner(execution, prepared);
    result.map_err(|failure| {
        request_id.map_or(failure.clone(), |request_id| {
            failure.with_server_request_id(request_id.as_str())
        })
    })
}

fn validate_generation_execution_inner(
    execution: &ProviderExecution,
    prepared: &PreparedGenerationRequest,
) -> Result<GeneratedUiDocument, TaskFailure> {
    validate_execution_provenance(execution, prepared)?;
    validate_value_budget(
        &execution.response.output.value,
        prepared.configuration.parameters.max_output_bytes as usize,
    )?;
    let envelope: ProviderGenerationEnvelope =
        serde_json::from_value(execution.response.output.value.clone()).map_err(|_| {
            TaskFailure::new(
                TaskFailureKind::ProviderResponseMalformed,
                "provider generation output does not match the strict response envelope",
                None,
            )
        })?;
    validate_provider_disclosures(&envelope, &prepared.analysis)?;

    let document_bytes = serde_json::to_vec(&envelope.document).map_err(|_| {
        TaskFailure::new(
            TaskFailureKind::ProviderResponseMalformed,
            "provider document cannot be serialized as JSON",
            None,
        )
    })?;
    if document_bytes.len() > UI_DOCUMENT_MAX_BYTES {
        return Err(TaskFailure::new(
            TaskFailureKind::ProviderResponseMalformed,
            "provider document exceeds the formal UiDocument byte budget",
            None,
        ));
    }
    validate_value_budget(&envelope.document, UI_DOCUMENT_MAX_BYTES)?;

    let formal = validate_json_bytes(&document_bytes);
    if !formal.report.valid {
        let codes = formal
            .report
            .diagnostics
            .iter()
            .take(8)
            .map(|diagnostic| diagnostic.code.as_str())
            .collect::<Vec<_>>()
            .join(",");
        return Err(TaskFailure::new(
            TaskFailureKind::ProviderResponseMalformed,
            format!("formal UiDocument facade rejected provider output: {codes}"),
            None,
        ));
    }
    let source = std::str::from_utf8(&document_bytes).map_err(|_| {
        TaskFailure::new(
            TaskFailureKind::ProviderResponseMalformed,
            "provider document is not valid UTF-8 JSON",
            None,
        )
    })?;
    let canonical_document_json = canonicalize_json(source).map_err(|error| {
        TaskFailure::new(
            TaskFailureKind::ProviderResponseMalformed,
            format!(
                "formal UiDocument canonicalization failed: {}",
                error.code()
            ),
            None,
        )
    })?;
    let canonical_value: Value = serde_json::from_str(&canonical_document_json)
        .expect("formal canonical JSON is always parseable");
    let document_paths = validate_document_policy(&canonical_value, prepared)?;
    let source_map = prepared
        .source_map
        .iter()
        .map(|entry| SourceMapEntry {
            document_path: document_paths[&entry.node_id].clone(),
            ..entry.clone()
        })
        .collect();
    let disclosures = merge_disclosures(envelope, prepared)?;
    let canonical_document_sha256 = sha256(canonical_document_json.as_bytes());
    let server_request_id = execution
        .response
        .server_request_id
        .as_ref()
        .expect("provenance validation requires a request ID")
        .as_str()
        .to_owned();
    Ok(GeneratedUiDocument {
        canonical_document_json,
        source_map,
        text_decisions: prepared.text_decisions.clone(),
        disclosures,
        trace: GenerationTrace {
            provider_id: execution.trace.provider_id.as_str().to_owned(),
            model_id: prepared.configuration.model_id.clone(),
            prompt_version: prepared.configuration.prompt_version.clone(),
            output_schema: generation_output_contract(),
            ui_document_schema_version: prepared.configuration.ui_document_schema_version,
            input_sha256: prepared.input_sha256.clone(),
            parameters: prepared.configuration.parameters.clone(),
            server_request_id,
            canonical_document_sha256,
        },
        validation_report: formal.report,
    })
}

fn validate_execution_provenance(
    execution: &ProviderExecution,
    prepared: &PreparedGenerationRequest,
) -> Result<(), TaskFailure> {
    let expected = prepared.request.log_metadata();
    if execution.response.output.operation != ProviderOperation::StructuredGeneration
        || execution.response.output.schema != generation_output_contract()
        || execution.trace.request != expected
        || execution.trace.request.run_id != prepared.analysis.run_id
        || execution.trace.request.prompt_version != prepared.configuration.prompt_version
    {
        return Err(malformed(
            "provider execution provenance differs from the prepared generation request",
        ));
    }
    if execution.response.server_request_id.is_none() {
        return Err(malformed(
            "traceable generation requires a provider server request ID",
        ));
    }
    let response_request_id = execution.response.server_request_id.as_ref();
    if !execution.trace.attempts.last().is_some_and(|attempt| {
        attempt.outcome == ProviderAttemptOutcome::Succeeded
            && attempt.server_request_id.as_ref() == response_request_id
    }) {
        return Err(malformed(
            "provider execution trace does not end with the response request ID",
        ));
    }
    Ok(())
}

fn validate_trusted_inputs(
    analysis: &UiReferenceAnalysis,
    plan: &UiGenerationPlan,
    asset_strategy: &AssetStrategyManifest,
) -> Result<(), TaskFailure> {
    if !analysis.validate_semantics().valid {
        return Err(TaskFailure::invalid(
            "generation requires a semantically valid UiReferenceAnalysis",
        ));
    }
    if plan.protocol_version != PLANNING_PROTOCOL_VERSION
        || plan.analysis_id != analysis.analysis_id
        || *plan != plan_analysis(analysis)
    {
        return Err(TaskFailure::invalid(
            "generation plan must be the deterministic plan for the supplied analysis",
        ));
    }
    if asset_strategy.protocol_version != ASSET_STRATEGY_PROTOCOL_VERSION
        || asset_strategy.analysis_id != analysis.analysis_id
        || asset_strategy.planning_protocol_version != plan.protocol_version
    {
        return Err(TaskFailure::invalid(
            "asset strategy is not bound to the supplied analysis and planning protocol",
        ));
    }
    let analysis_ids: BTreeSet<_> = analysis
        .elements
        .iter()
        .map(|element| element.element_id.as_str())
        .collect();
    let strategy_ids: BTreeSet<_> = asset_strategy
        .entries
        .iter()
        .map(|entry| entry.element_id())
        .collect();
    if analysis_ids != strategy_ids || strategy_ids.len() != asset_strategy.entries.len() {
        return Err(TaskFailure::invalid(
            "asset strategy must cover every analysis element exactly once",
        ));
    }
    Ok(())
}

fn derive_source_map(analysis: &UiReferenceAnalysis) -> Vec<SourceMapEntry> {
    let mut entries = analysis
        .elements
        .iter()
        .map(|element| SourceMapEntry {
            reference_element_id: element.element_id.clone(),
            node_id: element.element_id.clone(),
            reference_id: element.bounding_box.reference_id.clone(),
            evidence_ids: element.evidence_ids.clone(),
            document_path: String::new(),
        })
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

fn derive_text_decisions(analysis: &UiReferenceAnalysis) -> Vec<TextSourceDecision> {
    let mut decisions = analysis
        .elements
        .iter()
        .filter(|element| element.kind == VisualElementKind::Text)
        .map(|element| TextSourceDecision {
            reference_element_id: element.element_id.clone(),
            node_id: element.element_id.clone(),
            strategy: if adopted_text(element).is_some() {
                TextSourceStrategy::Literal
            } else {
                TextSourceStrategy::Unresolved
            },
        })
        .collect::<Vec<_>>();
    decisions.sort_by(|left, right| left.reference_element_id.cmp(&right.reference_element_id));
    decisions
}

fn collect_allowed_assets(
    asset_strategy: &AssetStrategyManifest,
    catalog: &AssetCatalog,
) -> Result<Vec<AllowedAsset>, TaskFailure> {
    let mut assets = BTreeMap::new();
    for entry in &asset_strategy.entries {
        if entry.disposition() != AssetDisposition::ExistingAsset {
            continue;
        }
        let asset_id = entry.existing_asset_id().ok_or_else(|| {
            TaskFailure::invalid("existing-asset strategy entry is missing its stable asset ID")
        })?;
        let asset = catalog.resolve(asset_id).ok_or_else(|| {
            TaskFailure::invalid("asset strategy refers to an ID absent from the trusted catalog")
        })?;
        assets.insert(
            asset.asset_id.clone(),
            AllowedAsset {
                asset_id: asset.asset_id.clone(),
                path: asset.path.clone(),
                kind: asset.kind,
            },
        );
    }
    Ok(assets.into_values().collect())
}

fn validate_provider_disclosures(
    envelope: &ProviderGenerationEnvelope,
    analysis: &UiReferenceAnalysis,
) -> Result<(), TaskFailure> {
    let ids: BTreeSet<_> = analysis
        .elements
        .iter()
        .map(|element| element.element_id.as_str())
        .collect();
    let groups = [
        &envelope.assumptions,
        &envelope.unimplemented_states,
        &envelope.required_new_components,
        &envelope.unsupported_capabilities,
    ];
    if groups.iter().map(|group| group.len()).sum::<usize>() > MAX_GENERATION_DISCLOSURES {
        return Err(malformed(
            "provider disclosures exceed the bounded entry count",
        ));
    }
    for disclosure in groups.into_iter().flatten() {
        if !is_safe_metadata_label(&disclosure.code, 128)
            || disclosure.message.is_empty()
            || disclosure.message.len() > MAX_DISCLOSURE_MESSAGE_BYTES
            || disclosure
                .subject_id
                .as_deref()
                .is_some_and(|subject| !ids.contains(subject))
        {
            return Err(malformed(
                "provider disclosure has an unsafe code, subject, or message",
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct DocumentNode<'a> {
    value: &'a Map<String, Value>,
    parent_id: Option<String>,
    path: String,
}

fn validate_document_policy(
    document: &Value,
    prepared: &PreparedGenerationRequest,
) -> Result<BTreeMap<String, String>, TaskFailure> {
    let object = document
        .as_object()
        .ok_or_else(|| malformed("formal document is not an object"))?;
    if object.get("document_id").and_then(Value::as_str)
        != Some(prepared.configuration.document_id.as_str())
    {
        return Err(malformed(
            "provider document_id differs from the requested document ID",
        ));
    }
    if object.get("schema_version").and_then(Value::as_u64)
        != Some(u64::from(prepared.configuration.ui_document_schema_version))
    {
        return Err(malformed(
            "provider schema_version differs from the requested UiDocument version",
        ));
    }
    for field in ["states", "responsive"] {
        if object
            .get(field)
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty())
        {
            return Err(malformed(
                "generation cannot infer hidden states or responsive variants from one visible state",
            ));
        }
    }
    reject_business_fields(document)?;
    validate_assets(object, prepared)?;
    let root = object
        .get("root")
        .ok_or_else(|| malformed("provider document has no root node"))?;
    let mut nodes = BTreeMap::new();
    collect_nodes(root, None, "$.root", &mut nodes)?;
    let expected_ids: BTreeSet<_> = prepared
        .source_map
        .iter()
        .map(|entry| entry.node_id.as_str())
        .collect();
    let actual_ids: BTreeSet<_> = nodes.keys().map(String::as_str).collect();
    if expected_ids != actual_ids {
        return Err(malformed(
            "document node IDs must exactly match the deterministic source map",
        ));
    }
    let expected_parents: BTreeMap<_, _> = prepared
        .analysis
        .elements
        .iter()
        .map(|element| (element.element_id.as_str(), element.parent_id.as_deref()))
        .collect();
    for (id, node) in &nodes {
        if node.parent_id.as_deref() != expected_parents[id.as_str()] {
            return Err(malformed(
                "document node hierarchy differs from the reference element hierarchy",
            ));
        }
    }
    validate_literal_text(root, &nodes, &prepared.analysis)?;
    validate_mapped_assets(&nodes, prepared)?;
    Ok(nodes
        .into_iter()
        .map(|(id, node)| (id, node.path))
        .collect())
}

fn collect_nodes<'a>(
    node: &'a Value,
    parent_id: Option<&str>,
    path: &str,
    output: &mut BTreeMap<String, DocumentNode<'a>>,
) -> Result<(), TaskFailure> {
    let object = node
        .as_object()
        .ok_or_else(|| malformed("UiDocument node is not an object"))?;
    let id = object
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| malformed("UiDocument node has no string ID"))?;
    if output
        .insert(
            id.to_owned(),
            DocumentNode {
                value: object,
                parent_id: parent_id.map(str::to_owned),
                path: path.to_owned(),
            },
        )
        .is_some()
    {
        return Err(malformed("UiDocument source-map node ID is duplicated"));
    }
    let is_container = object.get("type").and_then(Value::as_str) == Some("container");
    let children = if is_container {
        object.get("children").and_then(Value::as_array)
    } else {
        object
            .get("component")
            .and_then(Value::as_object)
            .and_then(|component| component.get("children"))
            .and_then(Value::as_array)
    };
    for (index, child) in children.into_iter().flatten().enumerate() {
        let child_path = if is_container {
            format!("{path}.children[{index}]")
        } else {
            format!("{path}.component.children[{index}]")
        };
        collect_nodes(child, Some(id), &child_path, output)?;
    }
    Ok(())
}

fn reject_business_fields(value: &Value) -> Result<(), TaskFailure> {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                if matches!(
                    key.as_str(),
                    "action" | "on_click" | "binding_path" | "i18n_key"
                ) {
                    return Err(malformed(
                        "generation cannot introduce actions, bindings, or i18n keys without a trusted registration",
                    ));
                }
                if key == "bindings"
                    && child
                        .as_object()
                        .is_some_and(|bindings| !bindings.is_empty())
                {
                    return Err(malformed(
                        "generation cannot introduce binding declarations without a trusted registration",
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

fn validate_literal_text(
    root: &Value,
    nodes: &BTreeMap<String, DocumentNode<'_>>,
    analysis: &UiReferenceAnalysis,
) -> Result<(), TaskFailure> {
    let mut expected = Vec::new();
    for element in analysis
        .elements
        .iter()
        .filter(|element| element.kind == VisualElementKind::Text)
    {
        match adopted_text(element) {
            Some(text) => {
                expected.push(text.to_owned());
                let mut local = Vec::new();
                collect_literals(
                    &Value::Object(nodes[&element.element_id].value.clone()),
                    false,
                    &mut local,
                );
                if local != [text.to_owned()] {
                    return Err(malformed(
                        "mapped text nodes must use exactly their adopted literal text",
                    ));
                }
            }
            None => {
                let mut local = Vec::new();
                collect_literals(
                    &Value::Object(nodes[&element.element_id].value.clone()),
                    false,
                    &mut local,
                );
                if !local.is_empty() {
                    return Err(malformed(
                        "unresolved reference text cannot become invented document text",
                    ));
                }
            }
        }
    }
    let mut actual = Vec::new();
    collect_literals(root, true, &mut actual);
    expected.sort();
    actual.sort();
    if actual != expected {
        return Err(malformed(
            "document literal text must exactly match adopted reference text",
        ));
    }
    Ok(())
}

fn collect_literals(value: &Value, include_children: bool, output: &mut Vec<String>) {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                if key == "literal" {
                    if let Some(literal) = child.as_str() {
                        output.push(literal.to_owned());
                    }
                } else if include_children || (key != "children" && key != "component") {
                    collect_literals(child, include_children, output);
                } else if key == "component" {
                    if let Some(component) = child.as_object() {
                        for (component_key, component_value) in component {
                            if component_key != "children" {
                                collect_literals(component_value, false, output);
                            }
                        }
                    }
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_literals(value, include_children, output);
            }
        }
        _ => {}
    }
}

fn validate_assets(
    document: &Map<String, Value>,
    prepared: &PreparedGenerationRequest,
) -> Result<(), TaskFailure> {
    let assets = document
        .get("assets")
        .and_then(Value::as_object)
        .ok_or_else(|| malformed("canonical UiDocument assets map is missing"))?;
    let allowed: BTreeMap<_, _> = prepared
        .allowed_assets
        .iter()
        .map(|asset| (asset.asset_id.as_str(), asset))
        .collect();
    let actual: BTreeSet<_> = assets.keys().map(String::as_str).collect();
    let expected: BTreeSet<_> = allowed.keys().copied().collect();
    if actual != expected {
        return Err(malformed(
            "document assets must exactly match existing assets selected by the strategy",
        ));
    }
    for (id, value) in assets {
        let allowed = allowed[id.as_str()];
        let object = value
            .as_object()
            .ok_or_else(|| malformed("document asset entry is not an object"))?;
        let kind = object.get("kind").and_then(Value::as_str);
        let source = object
            .get("source")
            .and_then(Value::as_object)
            .ok_or_else(|| malformed("document asset source is missing"))?;
        if source.get("kind").and_then(Value::as_str) != Some("packaged")
            || source.get("path").and_then(Value::as_str) != Some(allowed.path.as_str())
            || match allowed.kind {
                CatalogAssetKind::Raster => !matches!(kind, Some("image" | "icon")),
                CatalogAssetKind::Font => kind != Some("font"),
            }
        {
            return Err(malformed(
                "document asset kind or packaged path differs from the trusted stable-ID catalog",
            ));
        }
    }
    Ok(())
}

fn validate_mapped_assets(
    nodes: &BTreeMap<String, DocumentNode<'_>>,
    prepared: &PreparedGenerationRequest,
) -> Result<(), TaskFailure> {
    for entry in &prepared.asset_strategy.entries {
        let mapped_asset = nodes[entry.element_id()]
            .value
            .get("asset")
            .and_then(Value::as_str);
        match entry.disposition() {
            AssetDisposition::ExistingAsset => {
                if mapped_asset != entry.existing_asset_id() {
                    return Err(malformed(
                        "existing asset must be used by its mapped reference element",
                    ));
                }
            }
            _ if mapped_asset.is_some() => {
                return Err(malformed(
                    "non-existing asset strategies cannot reference a packaged asset",
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

fn merge_disclosures(
    envelope: ProviderGenerationEnvelope,
    prepared: &PreparedGenerationRequest,
) -> Result<GenerationDisclosures, TaskFailure> {
    let mut output = GenerationDisclosures {
        assumptions: envelope.assumptions,
        unimplemented_states: envelope.unimplemented_states,
        required_new_components: envelope.required_new_components,
        unsupported_capabilities: envelope.unsupported_capabilities,
    };
    for uncertainty in &prepared.analysis.uncertainties {
        let subject_id = uncertainty.subject.element_id.clone();
        let disclosure = GenerationDisclosure {
            code: "GENERATION_REFERENCE_UNCERTAINTY".to_owned(),
            subject_id,
            message: "reference evidence contains an unresolved uncertainty".to_owned(),
        };
        if uncertainty.kind == UncertaintyKind::HiddenInteraction {
            output.unimplemented_states.push(GenerationDisclosure {
                code: "GENERATION_HIDDEN_STATE_UNIMPLEMENTED".to_owned(),
                subject_id: disclosure.subject_id.clone(),
                message: "hidden interaction or state is not implemented from visual evidence"
                    .to_owned(),
            });
        }
        output.assumptions.push(disclosure);
    }
    for token in prepared
        .plan
        .tokens
        .iter()
        .filter(|token| token.origin == TokenOrigin::HeuristicAssumption)
    {
        output.assumptions.push(GenerationDisclosure {
            code: "GENERATION_HEURISTIC_TOKEN".to_owned(),
            subject_id: token.source_element_ids.first().cloned(),
            message: "a visual token is a heuristic assumption rather than measured evidence"
                .to_owned(),
        });
    }
    for component in prepared.plan.components.iter().filter(|component| {
        component.scope != RecommendationScope::ExistingGlobal
            && !matches!(component.component.as_str(), "container" | "label")
    }) {
        output.required_new_components.push(GenerationDisclosure {
            code: "GENERATION_COMPONENT_NOT_REGISTERED".to_owned(),
            subject_id: component.source_element_ids.first().cloned(),
            message: "the planned component has no registered reusable variant".to_owned(),
        });
    }
    for entry in prepared.asset_strategy.entries.iter().filter(|entry| {
        !matches!(
            entry.disposition(),
            AssetDisposition::ExistingAsset | AssetDisposition::Programmatic
        )
    }) {
        output.unsupported_capabilities.push(GenerationDisclosure {
            code: "GENERATION_ASSET_NOT_AVAILABLE".to_owned(),
            subject_id: Some(entry.element_id().to_owned()),
            message: "the asset strategy has no packaged asset available to the draft".to_owned(),
        });
    }
    for element in &prepared.analysis.elements {
        if element.kind == VisualElementKind::Text && adopted_text(element).is_none() {
            output.unsupported_capabilities.push(GenerationDisclosure {
                code: "GENERATION_TEXT_UNRESOLVED".to_owned(),
                subject_id: Some(element.element_id.clone()),
                message: "reference text has no trusted literal and remains unresolved".to_owned(),
            });
        }
        if element
            .component_candidates
            .iter()
            .any(|candidate| candidate.kind == crate::analysis::ComponentCandidateKind::Button)
        {
            output.unimplemented_states.push(GenerationDisclosure {
                code: "GENERATION_ACTION_UNREGISTERED".to_owned(),
                subject_id: Some(element.element_id.clone()),
                message: "interactive appearance has no trusted registered action".to_owned(),
            });
        }
    }
    normalize_disclosures(&mut output.assumptions);
    normalize_disclosures(&mut output.unimplemented_states);
    normalize_disclosures(&mut output.required_new_components);
    normalize_disclosures(&mut output.unsupported_capabilities);
    let merged_count = output.assumptions.len()
        + output.unimplemented_states.len()
        + output.required_new_components.len()
        + output.unsupported_capabilities.len();
    if merged_count > MAX_MERGED_GENERATION_DISCLOSURES {
        return Err(malformed(format!(
            "merged generation disclosures exceed the {MAX_MERGED_GENERATION_DISCLOSURES}-entry upstream budget"
        )));
    }
    Ok(output)
}

fn normalize_disclosures(values: &mut Vec<GenerationDisclosure>) {
    values.sort();
    values.dedup();
}

fn adopted_text(element: &crate::analysis::AnalysisElement) -> Option<&str> {
    let text = element.text.as_ref()?;
    match text.adopted {
        TextAdoption::HumanProvided => text.human_provided_text.as_deref(),
        TextAdoption::Candidate { candidate_index } => text
            .original_candidates
            .get(candidate_index)
            .map(|candidate| candidate.raw_text.as_str()),
        TextAdoption::Unresolved => None,
    }
}

fn validate_value_budget(value: &Value, maximum_bytes: usize) -> Result<(), TaskFailure> {
    let bytes = serde_json::to_vec(value)
        .map_err(|_| malformed("structured generation value cannot be serialized"))?;
    if bytes.len() > maximum_bytes {
        return Err(malformed(
            "structured generation value exceeds its byte budget",
        ));
    }
    let mut nodes = 0usize;
    let mut strings = 0usize;
    let mut stack = vec![(value, 1usize)];
    while let Some((value, depth)) = stack.pop() {
        nodes = nodes.saturating_add(1);
        if nodes > MAX_GENERATION_JSON_NODES || depth > MAX_GENERATION_JSON_DEPTH {
            return Err(malformed(
                "structured generation value exceeds its node or depth budget",
            ));
        }
        match value {
            Value::Object(object) => {
                if object.len() > MAX_GENERATION_CONTAINER_ITEMS {
                    return Err(malformed(
                        "structured generation object exceeds its entry budget",
                    ));
                }
                for (key, child) in object {
                    if key.len() > MAX_GENERATION_STRING_BYTES {
                        return Err(malformed(
                            "structured generation key exceeds its string budget",
                        ));
                    }
                    strings = strings.saturating_add(key.len());
                    stack.push((child, depth + 1));
                }
            }
            Value::Array(values) => {
                if values.len() > MAX_GENERATION_CONTAINER_ITEMS {
                    return Err(malformed(
                        "structured generation array exceeds its item budget",
                    ));
                }
                stack.extend(values.iter().map(|child| (child, depth + 1)));
            }
            Value::String(value) => {
                if value.len() > MAX_GENERATION_STRING_BYTES {
                    return Err(malformed(
                        "structured generation string exceeds its per-string budget",
                    ));
                }
                strings = strings.saturating_add(value.len());
            }
            _ => {}
        }
        if strings > MAX_GENERATION_TOTAL_STRING_BYTES {
            return Err(malformed(
                "structured generation value exceeds its total string budget",
            ));
        }
    }
    Ok(())
}

fn is_document_id(value: &str) -> bool {
    value.len() <= 128
        && value.split('.').count() >= 2
        && value.split('.').all(|segment| {
            let mut bytes = segment.bytes();
            bytes.next().is_some_and(|byte| byte.is_ascii_lowercase())
                && bytes
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        })
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn malformed(message: impl Into<String>) -> TaskFailure {
    TaskFailure::new(TaskFailureKind::ProviderResponseMalformed, message, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        analysis::{ComponentCandidateKind, parse_analysis_json},
        asset_strategy::{AssetCatalog, AssetDecision, AssetDecisionRequest, build_asset_strategy},
        planning::plan_analysis,
        provider::{
            ProviderAttemptTrace, ProviderExecutionTrace, ProviderId, ProviderResponse,
            ProviderUsage, ServerRequestId, StructuredProviderOutput,
        },
    };
    use std::path::{Path, PathBuf};

    fn repository_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap()
    }

    fn prepared(analysis_fixture: &str, document_id: &str) -> PreparedGenerationRequest {
        let analysis = parse_analysis_json(&fixture_analysis(analysis_fixture)).unwrap();
        let plan = plan_analysis(&analysis);
        let catalog = AssetCatalog::load_repository(&repository_root()).unwrap();
        let strategy = build_asset_strategy(&analysis, &plan, &catalog, &[], &[]).unwrap();
        prepare_generation_request(
            &analysis,
            &plan,
            &strategy,
            &catalog,
            GenerationConfiguration::new(
                document_id,
                "fixture-model-v1",
                "ui-document-v1",
                GenerationParameters::new(0, 262_144, Some(7)).unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn fixture_analysis(name: &str) -> Vec<u8> {
        std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("fixtures/analysis")
                .join(name),
        )
        .unwrap()
    }

    fn fixture_generation(name: &str) -> Value {
        serde_json::from_slice(
            &std::fs::read(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("fixtures/generation")
                    .join(name),
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn execution(prepared: &PreparedGenerationRequest, value: Value) -> ProviderExecution {
        let request_id = ServerRequestId::new("fixture-generation-001").unwrap();
        ProviderExecution {
            response: ProviderResponse {
                output: StructuredProviderOutput {
                    operation: ProviderOperation::StructuredGeneration,
                    schema: generation_output_contract(),
                    value,
                },
                server_request_id: Some(request_id.clone()),
                usage: ProviderUsage::default(),
            },
            trace: ProviderExecutionTrace {
                provider_id: ProviderId::new("fixture").unwrap(),
                request: prepared.request().log_metadata(),
                attempts: vec![ProviderAttemptTrace {
                    attempt: 1,
                    outcome: ProviderAttemptOutcome::Succeeded,
                    server_request_id: Some(request_id),
                    elapsed_ms: 0,
                }],
            },
        }
    }

    #[test]
    fn minimal_fixture_is_canonical_and_source_mapped() {
        let prepared = prepared("regular_page.json", "generated.minimal_fixture");
        let generated = validate_generation_execution(
            &execution(&prepared, fixture_generation("minimal.valid.json")),
            &prepared,
        )
        .unwrap();
        assert!(generated.validation_report.valid);
        assert_eq!(generated.source_map.len(), 2);
        assert_eq!(generated.source_map[0].reference_element_id, "page.root");
        assert_eq!(generated.source_map[0].node_id, "page.root");
        assert_eq!(generated.source_map[0].document_path, "$.root");
        assert_eq!(
            generated.text_decisions[0].strategy,
            TextSourceStrategy::Literal
        );
        assert_eq!(generated.trace.model_id, "fixture-model-v1");
        assert_eq!(generated.trace.server_request_id, "fixture-generation-001");
        assert_eq!(generated.trace.input_sha256, prepared.input_sha256());
        assert!(generated.canonical_document_json.ends_with('\n'));
    }

    #[test]
    fn complex_fixture_preserves_hierarchy_and_reports_capability_gaps() {
        let prepared = prepared("modal.json", "generated.complex_fixture");
        let generated = validate_generation_execution(
            &execution(&prepared, fixture_generation("complex.valid.json")),
            &prepared,
        )
        .unwrap();
        assert_eq!(generated.source_map.len(), 4);
        assert_eq!(
            generated
                .source_map
                .iter()
                .find(|entry| entry.node_id == "modal.title")
                .unwrap()
                .document_path,
            "$.root.children[0].children[1]"
        );
        assert!(!generated.disclosures.assumptions.is_empty());
        assert!(
            generated
                .disclosures
                .unsupported_capabilities
                .iter()
                .any(|finding| finding.code == "GENERATION_ASSET_NOT_AVAILABLE")
        );
        assert!(
            generated
                .disclosures
                .required_new_components
                .iter()
                .any(|finding| finding.code == "GENERATION_COMPONENT_NOT_REGISTERED")
        );
    }

    #[test]
    fn invalid_fixture_is_rejected_by_the_formal_facade() {
        let prepared = prepared("regular_page.json", "generated.invalid_fixture");
        let failure = validate_generation_execution(
            &execution(&prepared, fixture_generation("invalid.document.json")),
            &prepared,
        )
        .unwrap_err();
        assert_eq!(failure.kind(), TaskFailureKind::ProviderResponseMalformed);
        assert!(
            failure
                .message()
                .contains("formal UiDocument facade rejected")
        );
        assert_eq!(failure.server_request_id(), Some("fixture-generation-001"));
    }

    #[test]
    fn unsupported_fixture_remains_valid_but_cannot_invent_actions_or_assets() {
        let prepared = prepared("hud.json", "generated.unsupported_fixture");
        let generated = validate_generation_execution(
            &execution(&prepared, fixture_generation("unsupported.valid.json")),
            &prepared,
        )
        .unwrap();
        assert!(
            generated
                .disclosures
                .unsupported_capabilities
                .iter()
                .any(|finding| finding.subject_id.as_deref() == Some("hud.ability_icon"))
        );
        assert!(
            generated
                .disclosures
                .unimplemented_states
                .iter()
                .any(|finding| finding.code == "GENERATION_ACTION_UNREGISTERED")
        );

        let mut invented = fixture_generation("unsupported.valid.json");
        invented["document"]["root"]["children"][1] = serde_json::json!({
            "type": "button",
            "id": "hud.ability_icon",
            "label": {"literal": "Cast"},
            "on_click": {"action": "game.cast"}
        });
        assert!(validate_generation_execution(&execution(&prepared, invented), &prepared).is_err());

        let mut invented_state = fixture_generation("unsupported.valid.json");
        invented_state["document"]["states"] = serde_json::json!([{
            "id": "loading",
            "overrides": [{
                "node_id": "hud.health",
                "set": {"layout": {"width": {"px": 361}}}
            }]
        }]);
        let failure =
            validate_generation_execution(&execution(&prepared, invented_state), &prepared)
                .unwrap_err();
        assert!(
            failure.message().contains("cannot infer hidden states"),
            "{}",
            failure.message()
        );
    }

    #[test]
    fn structured_contract_does_not_extract_markdown_json() {
        let prepared = prepared("regular_page.json", "generated.markdown_fixture");
        let fenced =
            Value::String("```json\n{\"document\":{\"schema_version\":1}}\n```".to_owned());
        let failure =
            validate_generation_execution(&execution(&prepared, fenced), &prepared).unwrap_err();
        assert_eq!(failure.kind(), TaskFailureKind::ProviderResponseMalformed);
        assert!(failure.message().contains("strict response envelope"));
    }

    #[test]
    fn source_map_and_input_hash_are_deterministic_and_collision_free() {
        let first = prepared("modal.json", "generated.hash_fixture");
        let second = prepared("modal.json", "generated.hash_fixture");
        assert_eq!(first.input_sha256(), second.input_sha256());
        assert_eq!(first.source_map, second.source_map);
        assert_eq!(
            first
                .source_map
                .iter()
                .map(|entry| entry.node_id.as_str())
                .collect::<BTreeSet<_>>()
                .len(),
            first.source_map.len()
        );
    }

    #[test]
    fn registered_component_variant_is_carried_in_the_plan_and_validated() {
        let mut analysis = parse_analysis_json(&fixture_analysis("regular_page.json")).unwrap();
        analysis.elements[1].component_candidates[0].kind = ComponentCandidateKind::Badge;
        let plan = plan_analysis(&analysis);
        let catalog = AssetCatalog::load_repository(&repository_root()).unwrap();
        let strategy = build_asset_strategy(&analysis, &plan, &catalog, &[], &[]).unwrap();
        let prepared = prepare_generation_request(
            &analysis,
            &plan,
            &strategy,
            &catalog,
            GenerationConfiguration::new(
                "generated.component_fixture",
                "fixture-model-v1",
                "ui-document-v1",
                GenerationParameters::new(0, 262_144, Some(7)).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(prepared.request().structured_inputs().is_some_and(|input| {
            input["plan"]["components"]
                .as_array()
                .unwrap()
                .iter()
                .any(|component| {
                    component["component"] == "badge" && component["variant"] == "default"
                })
        }));
        let mut value = fixture_generation("minimal.valid.json");
        value["document"]["document_id"] = Value::String("generated.component_fixture".into());
        value["document"]["root"]["children"][0] = serde_json::json!({
            "type": "badge",
            "id": "page.title",
            "component": {
                "variant": "default",
                "slots": {
                    "label": {
                        "kind": "text",
                        "content": {"literal": "Start game"}
                    }
                }
            }
        });
        let generated =
            validate_generation_execution(&execution(&prepared, value), &prepared).unwrap();
        assert!(generated.validation_report.valid);
        assert!(generated.disclosures.required_new_components.is_empty());
    }

    #[test]
    fn stable_asset_id_resolves_only_to_the_catalog_packaged_path() {
        let analysis = parse_analysis_json(&fixture_analysis("hud.json")).unwrap();
        let plan = plan_analysis(&analysis);
        let catalog = AssetCatalog::load_repository(&repository_root()).unwrap();
        let strategy = build_asset_strategy(
            &analysis,
            &plan,
            &catalog,
            &[],
            &[AssetDecisionRequest {
                element_id: "hud.ability_icon".to_owned(),
                decision: AssetDecision::ExistingAsset {
                    asset_id: "ui.icon.help".to_owned(),
                },
            }],
        )
        .unwrap();
        let prepared = prepare_generation_request(
            &analysis,
            &plan,
            &strategy,
            &catalog,
            GenerationConfiguration::new(
                "generated.asset_fixture",
                "fixture-model-v1",
                "ui-document-v1",
                GenerationParameters::new(0, 262_144, Some(7)).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let mut value = fixture_generation("unsupported.valid.json");
        value["document"]["document_id"] = Value::String("generated.asset_fixture".into());
        value["document"]["assets"] = serde_json::json!({
            "ui.icon.help": {
                "kind": "icon",
                "source": {"kind": "packaged", "path": "ui/icons/help.png"}
            }
        });
        value["document"]["root"]["children"][1] = serde_json::json!({
            "type": "icon",
            "id": "hud.ability_icon",
            "asset": "ui.icon.help",
            "layout": {"width": {"px": 88}, "height": {"px": 88}}
        });
        let generated =
            validate_generation_execution(&execution(&prepared, value.clone()), &prepared).unwrap();
        assert!(
            generated
                .canonical_document_json
                .contains("ui/icons/help.png")
        );

        value["document"]["assets"]["ui.icon.help"]["source"]["path"] =
            Value::String("ui/icons/close.png".into());
        assert!(validate_generation_execution(&execution(&prepared, value), &prepared).is_err());
    }

    #[test]
    fn merged_disclosures_preserve_provider_tail_and_local_assumptions() {
        let prepared = prepared("modal.json", "generated.complex_fixture");
        let mut value = fixture_generation("complex.valid.json");
        value["unsupported_capabilities"] = Value::Array(Vec::new());
        value["assumptions"] = Value::Array(
            (0..MAX_GENERATION_DISCLOSURES)
                .map(|index| {
                    serde_json::json!({
                        "code": format!("PROVIDER_ASSUMPTION_{index:04}"),
                        "message": "bounded provider assumption"
                    })
                })
                .collect(),
        );

        let generated =
            validate_generation_execution(&execution(&prepared, value), &prepared).unwrap();
        assert!(generated.disclosures.assumptions.len() > MAX_GENERATION_DISCLOSURES);
        assert_eq!(
            generated
                .disclosures
                .assumptions
                .iter()
                .filter(|item| item.code.starts_with("PROVIDER_ASSUMPTION_"))
                .count(),
            MAX_GENERATION_DISCLOSURES
        );
        assert!(
            generated
                .disclosures
                .assumptions
                .iter()
                .any(|item| item.code == "PROVIDER_ASSUMPTION_0511")
        );
        assert!(
            generated
                .disclosures
                .assumptions
                .iter()
                .any(|item| item.code == "GENERATION_HEURISTIC_TOKEN")
        );
        assert!(
            generated
                .disclosures
                .assumptions
                .iter()
                .any(|item| item.code == "GENERATION_REFERENCE_UNCERTAINTY")
        );
        assert!(
            generated
                .disclosures
                .unimplemented_states
                .iter()
                .any(|item| item.code == "GENERATION_HIDDEN_STATE_UNIMPLEMENTED")
        );
        assert!(
            generated
                .disclosures
                .required_new_components
                .iter()
                .any(|item| item.code == "GENERATION_COMPONENT_NOT_REGISTERED")
        );
        assert!(
            generated
                .disclosures
                .unsupported_capabilities
                .iter()
                .any(|item| item.code == "GENERATION_ASSET_NOT_AVAILABLE")
        );
        let merged_count = generated.disclosures.assumptions.len()
            + generated.disclosures.unimplemented_states.len()
            + generated.disclosures.required_new_components.len()
            + generated.disclosures.unsupported_capabilities.len();
        assert!(merged_count <= MAX_MERGED_GENERATION_DISCLOSURES);
    }

    #[test]
    fn trace_labels_and_response_budgets_fail_closed() {
        assert!(
            GenerationConfiguration::new(
                "generated.safe",
                "model\nsecret",
                "prompt-v1",
                GenerationParameters::new(0, 1024, None).unwrap(),
            )
            .is_err()
        );
        assert!(
            GenerationConfiguration::new_for_schema(
                "generated.safe",
                CURRENT_SCHEMA_VERSION + 1,
                "model-v1",
                "prompt-v1",
                GenerationParameters::new(0, 1024, None).unwrap(),
            )
            .is_err()
        );
        let prepared = prepared("regular_page.json", "generated.budget_fixture");
        let mut oversized = fixture_generation("minimal.valid.json");
        oversized["assumptions"] = Value::Array(vec![
            serde_json::json!({
                "code": "A",
                "message": "x"
            });
            MAX_GENERATION_DISCLOSURES + 1
        ]);
        assert!(
            validate_generation_execution(&execution(&prepared, oversized), &prepared).is_err()
        );
        let mut mismatched_trace = execution(&prepared, fixture_generation("minimal.valid.json"));
        mismatched_trace.trace.attempts[0].server_request_id =
            Some(ServerRequestId::new("different-request-002").unwrap());
        assert!(
            validate_generation_execution(&mismatched_trace, &prepared)
                .unwrap_err()
                .message()
                .contains("does not end with the response request ID")
        );
    }
}
