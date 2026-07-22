use crate::{
    generation::{
        GeneratedUiDocument, GenerationPolicyDiagnostic, PreparedGenerationRequest,
        StagedGeneration, StagingDocumentValidation, extract_generation_staging,
        finalize_staging_generation, validate_staging_document,
    },
    lifecycle::{CancellationToken, TaskFailure, TaskFailureKind},
    observability::TaskBudget,
    provider::{
        ProviderExecutionFailure, ProviderExecutionTrace, ProviderId, ProviderRequest,
        ProviderRunner, RequestLogMetadata, StructuredOutputContract,
    },
};
use project::framework::ui::document::tooling::{
    UI_DOCUMENT_MAX_BYTES, UiValidationDiagnostic, UiValidationPhase, UiValidationSeverity,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const REPAIR_OUTPUT_SCHEMA_ID: &str = "ui-document-repair";
pub const REPAIR_OUTPUT_SCHEMA_VERSION: u32 = 1;
pub const MAX_REPAIR_ROUNDS: u8 = 3;
pub const MAX_REPAIR_DIAGNOSTICS: usize = 64;
const REPAIR_PROMPT_VERSION: &str = "ui-document-repair-v1";
const REPAIR_INSTRUCTION: &str = "Return exactly the structured repair contract. Modify only the supplied staging UiDocument to resolve the supplied diagnostics while preserving every frozen guardrail.";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairFailureKind {
    InitialGenerationInvalid,
    DocumentOverBudget,
    NoProgress,
    RepeatedDiagnostics,
    MaximumRoundsReached,
    ProviderUnavailable,
    ProviderTimeout,
    ProviderCancelled,
    ProviderRejected,
    ProviderResponseMalformed,
    FinalPolicyRejected,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RepairFailure {
    pub kind: RepairFailureKind,
    pub code: String,
    pub detail: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairRunStatus {
    Passed,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepairConfiguration {
    maximum_rounds: u8,
}

impl RepairConfiguration {
    pub fn new(maximum_rounds: u8) -> Result<Self, TaskFailure> {
        if maximum_rounds > MAX_REPAIR_ROUNDS {
            return Err(TaskFailure::invalid(format!(
                "repair rounds must be in 0..={MAX_REPAIR_ROUNDS}"
            )));
        }
        Ok(Self { maximum_rounds })
    }

    pub fn maximum_rounds(&self) -> u8 {
        self.maximum_rounds
    }
}

impl Default for RepairConfiguration {
    fn default() -> Self {
        Self {
            maximum_rounds: MAX_REPAIR_ROUNDS,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RepairDiagnostic {
    pub code: String,
    pub phase: String,
    pub severity: String,
    pub document_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    pub field_path: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RepairValidationEvidence {
    pub valid: bool,
    pub fingerprint_sha256: String,
    pub diagnostics: Vec<RepairDiagnostic>,
    pub report: StagingDocumentValidation,
}

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RepairRoundEvidence {
    pub round: u8,
    pub input_document_sha256: String,
    pub validation: RepairValidationEvidence,
    pub request: RequestLogMetadata,
    pub structured_request: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_trace: Option<ProviderExecutionTrace>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_document_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_document: Option<Value>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NodeTreeSummary {
    pub node_count: usize,
    pub sha256: String,
    pub lines: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RepairRunResult {
    pub status: RepairRunStatus,
    pub initial_document_sha256: String,
    pub initial_document: Value,
    pub rounds: Vec<RepairRoundEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_document: Option<GeneratedUiDocument>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_tree_summary: Option<NodeTreeSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<RepairFailure>,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct RepairStructuredInput<'a> {
    round: u8,
    maximum_rounds: u8,
    current_staging_document: &'a Value,
    validation: &'a [RepairDiagnostic],
    validation_fingerprint_sha256: &'a str,
    guardrails: crate::generation::GenerationRepairPolicySnapshot,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RepairProviderEnvelope {
    document: Value,
}

pub fn repair_output_contract() -> StructuredOutputContract {
    StructuredOutputContract::new(REPAIR_OUTPUT_SCHEMA_ID, REPAIR_OUTPUT_SCHEMA_VERSION)
        .expect("repair output contract constants are valid")
}

pub fn repair_generated_document(
    initial_execution: &crate::provider::ProviderExecution,
    prepared: &PreparedGenerationRequest,
    runner: &ProviderRunner,
    provider_id: &ProviderId,
    cancellation: &CancellationToken,
    configuration: RepairConfiguration,
) -> RepairRunResult {
    let staged = match extract_generation_staging(initial_execution, prepared) {
        Ok(staged) => staged,
        Err(failure) => {
            let initial = initial_execution
                .response
                .output
                .value
                .get("document")
                .cloned()
                .unwrap_or(Value::Null);
            let kind = if failure.message().contains("exceeds") {
                RepairFailureKind::DocumentOverBudget
            } else {
                RepairFailureKind::InitialGenerationInvalid
            };
            return failed_run(initial, Vec::new(), failure_for_task(kind, &failure));
        }
    };
    let budget = TaskBudget::new(runner.task_limits())
        .expect("a constructed provider runner always has valid task limits");
    repair_staged_document(
        staged,
        prepared,
        runner,
        provider_id,
        cancellation,
        configuration,
        &budget,
    )
}

fn repair_staged_document(
    staged: StagedGeneration,
    prepared: &PreparedGenerationRequest,
    runner: &ProviderRunner,
    provider_id: &ProviderId,
    cancellation: &CancellationToken,
    configuration: RepairConfiguration,
    budget: &TaskBudget,
) -> RepairRunResult {
    let initial = staged.document().clone();
    let mut current = initial.clone();
    let mut rounds = Vec::new();
    let mut previous_document_sha256: Option<String> = None;
    let mut previous_validation_sha256: Option<String> = None;

    loop {
        let input_document_sha256 = hash_json(&current);
        let validation = match repair_validation(&current, prepared) {
            Ok(validation) => validation,
            Err(failure) => {
                let kind = if failure.message().contains("budget")
                    || failure.message().contains("exceeds")
                {
                    RepairFailureKind::DocumentOverBudget
                } else {
                    RepairFailureKind::FinalPolicyRejected
                };
                return failed_run(initial, rounds, failure_for_task(kind, &failure));
            }
        };

        if validation.valid {
            return match finalize_staging_generation(&staged, current, prepared) {
                Ok(final_document) => {
                    let summary = node_tree_summary(&final_document.canonical_document_json)
                        .expect("validated canonical document has a node tree");
                    RepairRunResult {
                        status: RepairRunStatus::Passed,
                        initial_document_sha256: hash_json(&initial),
                        initial_document: initial,
                        rounds,
                        final_document: Some(final_document),
                        node_tree_summary: Some(summary),
                        failure: None,
                    }
                }
                Err(failure) => failed_run(
                    initial,
                    rounds,
                    failure_for_task(RepairFailureKind::FinalPolicyRejected, &failure),
                ),
            };
        }

        if rounds.len() >= usize::from(configuration.maximum_rounds) {
            return failed_run(
                initial,
                rounds,
                failure(
                    RepairFailureKind::MaximumRoundsReached,
                    "UI_GENERATION_REPAIR_MAXIMUM_ROUNDS_REACHED",
                    "repair stopped at the configured hard round limit",
                ),
            );
        }
        if previous_document_sha256.as_deref() == Some(&input_document_sha256) {
            return failed_run(
                initial,
                rounds,
                failure(
                    RepairFailureKind::NoProgress,
                    "UI_GENERATION_REPAIR_NO_PROGRESS",
                    "repair provider returned an unchanged staging document",
                ),
            );
        }
        if previous_validation_sha256.as_deref() == Some(&validation.fingerprint_sha256) {
            return failed_run(
                initial,
                rounds,
                failure(
                    RepairFailureKind::RepeatedDiagnostics,
                    "UI_GENERATION_REPAIR_DIAGNOSTICS_REPEATED",
                    "repair produced the same bounded validation diagnostics consecutively",
                ),
            );
        }
        if let Err(failure) = budget.reserve_iteration() {
            return failed_run(
                initial,
                rounds,
                failure_for_task(RepairFailureKind::MaximumRoundsReached, &failure),
            );
        }
        let round = rounds.len() as u8 + 1;
        let structured_request = serde_json::to_value(RepairStructuredInput {
            round,
            maximum_rounds: configuration.maximum_rounds,
            current_staging_document: &current,
            validation: &validation.diagnostics,
            validation_fingerprint_sha256: &validation.fingerprint_sha256,
            guardrails: prepared.repair_policy_snapshot(),
        })
        .expect("bounded repair input is serializable");
        let request = ProviderRequest::structured_generation(
            prepared.run_id(),
            REPAIR_PROMPT_VERSION,
            REPAIR_INSTRUCTION,
            structured_request.clone(),
            Vec::new(),
            repair_output_contract(),
        )
        .expect("trusted repair request labels and contract are valid");
        let request_metadata = request.log_metadata();
        let execution = match runner.execute_with_budget(provider_id, request, cancellation, budget)
        {
            Ok(execution) => execution,
            Err(execution_failure) => {
                rounds.push(RepairRoundEvidence {
                    round,
                    input_document_sha256,
                    validation,
                    request: request_metadata,
                    structured_request,
                    provider_trace: Some(execution_failure.trace.clone()),
                    output_document_sha256: None,
                    output_document: None,
                });
                return failed_run(initial, rounds, provider_failure(&execution_failure));
            }
        };
        let provider_trace = execution.trace.clone();
        let envelope =
            match serde_json::from_value::<RepairProviderEnvelope>(execution.response.output.value)
            {
                Ok(envelope) => envelope,
                Err(_) => {
                    rounds.push(RepairRoundEvidence {
                        round,
                        input_document_sha256,
                        validation,
                        request: request_metadata,
                        structured_request,
                        provider_trace: Some(provider_trace),
                        output_document_sha256: None,
                        output_document: None,
                    });
                    return failed_run(
                        initial,
                        rounds,
                        failure(
                            RepairFailureKind::ProviderResponseMalformed,
                            "UI_GENERATION_REPAIR_RESPONSE_MALFORMED",
                            "repair provider output did not match the strict document envelope",
                        ),
                    );
                }
            };
        let output_document_sha256 = hash_json(&envelope.document);
        let output_bytes = serde_json::to_vec(&envelope.document)
            .expect("serde_json::Value is always serializable")
            .len();
        rounds.push(RepairRoundEvidence {
            round,
            input_document_sha256: input_document_sha256.clone(),
            validation,
            request: request_metadata,
            structured_request,
            provider_trace: Some(provider_trace),
            output_document_sha256: Some(output_document_sha256),
            output_document: Some(envelope.document.clone()),
        });
        if output_bytes > UI_DOCUMENT_MAX_BYTES {
            return failed_run(
                initial,
                rounds,
                failure(
                    RepairFailureKind::DocumentOverBudget,
                    "UI_GENERATION_REPAIR_DOCUMENT_OVER_BUDGET",
                    "repair provider document exceeds the frozen formal byte budget",
                ),
            );
        }
        previous_document_sha256 = Some(input_document_sha256);
        previous_validation_sha256 = Some(
            rounds
                .last()
                .expect("round was just recorded")
                .validation
                .fingerprint_sha256
                .clone(),
        );
        current = envelope.document;
    }
}

fn repair_validation(
    document: &Value,
    prepared: &PreparedGenerationRequest,
) -> Result<RepairValidationEvidence, TaskFailure> {
    let report = validate_staging_document(document, prepared)?;
    let mut diagnostics = report
        .formal_report
        .diagnostics
        .iter()
        .map(formal_diagnostic)
        .chain(report.policy_diagnostics.iter().map(policy_diagnostic))
        .take(MAX_REPAIR_DIAGNOSTICS)
        .collect::<Vec<_>>();
    let node_paths = staging_node_paths(document);
    for diagnostic in &mut diagnostics {
        if diagnostic.node_id.is_none() {
            diagnostic.node_id = nearest_node_id(
                &node_paths,
                &diagnostic.document_path,
                &diagnostic.field_path,
            );
        }
    }
    diagnostics.sort_by(|left, right| {
        (
            &left.phase,
            &left.field_path,
            &left.code,
            &left.node_id,
            &left.document_path,
        )
            .cmp(&(
                &right.phase,
                &right.field_path,
                &right.code,
                &right.node_id,
                &right.document_path,
            ))
    });
    let fingerprint_sha256 =
        hash_bytes(&serde_json::to_vec(&diagnostics).expect("repair diagnostics are serializable"));
    Ok(RepairValidationEvidence {
        valid: report.valid,
        fingerprint_sha256,
        diagnostics,
        report,
    })
}

fn staging_node_paths(document: &Value) -> Vec<(String, String)> {
    fn collect(node: &Value, path: &str, output: &mut Vec<(String, String)>) {
        let Some(object) = node.as_object() else {
            return;
        };
        let Some(id) = object.get("id").and_then(Value::as_str) else {
            return;
        };
        output.push((path.to_owned(), id.to_owned()));
        let kind = object.get("type").and_then(Value::as_str);
        let children = if kind == Some("container") {
            object.get("children").and_then(Value::as_array)
        } else {
            object
                .get("component")
                .and_then(Value::as_object)
                .and_then(|component| component.get("children"))
                .and_then(Value::as_array)
        };
        for (index, child) in children.into_iter().flatten().enumerate() {
            let child_path = if kind == Some("container") {
                format!("{path}.children[{index}]")
            } else {
                format!("{path}.component.children[{index}]")
            };
            collect(child, &child_path, output);
        }
    }

    let mut output = Vec::new();
    if let Some(root) = document.get("root") {
        collect(root, "$.root", &mut output);
    }
    output.sort_by(|left, right| right.0.len().cmp(&left.0.len()).then(left.cmp(right)));
    output
}

fn nearest_node_id(
    node_paths: &[(String, String)],
    document_path: &str,
    field_path: &str,
) -> Option<String> {
    node_paths
        .iter()
        .find_map(|(path, node_id)| {
            [document_path, field_path]
                .iter()
                .any(|candidate| {
                    *candidate == path
                        || candidate.strip_prefix(path).is_some_and(|suffix| {
                            suffix.starts_with('.') || suffix.starts_with('[')
                        })
                })
                .then(|| node_id.clone())
        })
        .or_else(|| {
            if document_path == "$" || field_path == "$" {
                node_paths
                    .iter()
                    .find(|(path, _)| path == "$.root")
                    .map(|(_, node_id)| node_id.clone())
            } else {
                None
            }
        })
}

fn formal_diagnostic(diagnostic: &UiValidationDiagnostic) -> RepairDiagnostic {
    RepairDiagnostic {
        code: diagnostic.code.clone(),
        phase: validation_phase(diagnostic.phase).to_owned(),
        severity: validation_severity(diagnostic.severity).to_owned(),
        document_path: diagnostic.document_path.clone(),
        node_id: diagnostic.node_id.as_ref().map(ToString::to_string),
        field_path: diagnostic.field_path.clone(),
    }
}

fn policy_diagnostic(diagnostic: &GenerationPolicyDiagnostic) -> RepairDiagnostic {
    RepairDiagnostic {
        code: diagnostic.code.clone(),
        phase: "generation_policy".to_owned(),
        severity: "error".to_owned(),
        document_path: diagnostic.document_path.clone(),
        node_id: diagnostic.node_id.clone(),
        field_path: diagnostic.document_path.clone(),
    }
}

fn validation_phase(phase: UiValidationPhase) -> &'static str {
    match phase {
        UiValidationPhase::Syntax => "syntax",
        UiValidationPhase::Structure => "structure",
        UiValidationPhase::Reference => "reference",
        UiValidationPhase::Capability => "capability",
        UiValidationPhase::Budget => "budget",
    }
}

fn validation_severity(severity: UiValidationSeverity) -> &'static str {
    match severity {
        UiValidationSeverity::Error => "error",
        UiValidationSeverity::Warning => "warning",
    }
}

fn provider_failure(execution: &ProviderExecutionFailure) -> RepairFailure {
    let kind = match execution.failure.kind() {
        TaskFailureKind::ProviderNotFound | TaskFailureKind::ProviderServiceUnavailable => {
            RepairFailureKind::ProviderUnavailable
        }
        TaskFailureKind::ProviderTimeout => RepairFailureKind::ProviderTimeout,
        TaskFailureKind::Cancelled => RepairFailureKind::ProviderCancelled,
        TaskFailureKind::ProviderResponseMalformed => RepairFailureKind::ProviderResponseMalformed,
        _ => RepairFailureKind::ProviderRejected,
    };
    failure_for_task(kind, &execution.failure)
}

fn failed_run(
    initial_document: Value,
    rounds: Vec<RepairRoundEvidence>,
    failure: RepairFailure,
) -> RepairRunResult {
    RepairRunResult {
        status: RepairRunStatus::Failed,
        initial_document_sha256: hash_json(&initial_document),
        initial_document,
        rounds,
        final_document: None,
        node_tree_summary: None,
        failure: Some(failure),
    }
}

fn failure_for_task(kind: RepairFailureKind, failure: &TaskFailure) -> RepairFailure {
    RepairFailure {
        kind,
        code: failure.code().to_owned(),
        detail: failure.message().to_owned(),
    }
}

fn failure(kind: RepairFailureKind, code: &str, detail: &str) -> RepairFailure {
    RepairFailure {
        kind,
        code: code.to_owned(),
        detail: detail.to_owned(),
    }
}

pub fn node_tree_summary(canonical_document_json: &str) -> Result<NodeTreeSummary, TaskFailure> {
    let value: Value = serde_json::from_str(canonical_document_json)
        .map_err(|_| TaskFailure::invalid("canonical UiDocument JSON is malformed"))?;
    let root = value
        .get("root")
        .ok_or_else(|| TaskFailure::invalid("canonical UiDocument has no root"))?;
    let mut lines = Vec::new();
    collect_node_summary(root, "$.root", None, &mut lines)?;
    let bytes = lines.join("\n");
    Ok(NodeTreeSummary {
        node_count: lines.len(),
        sha256: hash_bytes(bytes.as_bytes()),
        lines,
    })
}

fn collect_node_summary(
    node: &Value,
    path: &str,
    parent: Option<&str>,
    lines: &mut Vec<String>,
) -> Result<(), TaskFailure> {
    let object = node
        .as_object()
        .ok_or_else(|| TaskFailure::invalid("canonical UiDocument node is not an object"))?;
    let id = object
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| TaskFailure::invalid("canonical UiDocument node has no ID"))?;
    let kind = object
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| TaskFailure::invalid("canonical UiDocument node has no type"))?;
    lines.push(format!("{path}|{id}|{kind}|{}", parent.unwrap_or("-")));
    let children = if kind == "container" {
        object.get("children").and_then(Value::as_array)
    } else {
        object
            .get("component")
            .and_then(Value::as_object)
            .and_then(|component| component.get("children"))
            .and_then(Value::as_array)
    };
    for (index, child) in children.into_iter().flatten().enumerate() {
        let child_path = if kind == "container" {
            format!("{path}.children[{index}]")
        } else {
            format!("{path}.component.children[{index}]")
        };
        collect_node_summary(child, &child_path, Some(id), lines)?;
    }
    Ok(())
}

fn hash_json(value: &Value) -> String {
    hash_bytes(&serde_json::to_vec(value).expect("serde_json::Value is serializable"))
}

fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        analysis::parse_analysis_json,
        asset_strategy::{AssetCatalog, build_asset_strategy},
        generation::{
            GenerationConfiguration, GenerationParameters, generation_output_contract,
            prepare_generation_request,
        },
        planning::plan_analysis,
        provider::{
            MockProvider, MockScenario, ProviderAttemptOutcome, ProviderAttemptTrace,
            ProviderCapabilities, ProviderDescriptor, ProviderExecution, ProviderExecutionPolicy,
            ProviderExecutionTrace, ProviderOperation, ProviderRegistry, ProviderResponse,
            ProviderUsage, RetryPolicy, ServerRequestId, StructuredProviderOutput,
        },
    };
    use serde_json::json;
    use std::{collections::BTreeSet, path::Path, sync::Arc, time::Duration};

    fn repository_root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap()
    }

    fn prepared() -> PreparedGenerationRequest {
        let bytes = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/analysis/regular_page.json"),
        )
        .unwrap();
        let analysis = parse_analysis_json(&bytes).unwrap();
        let plan = plan_analysis(&analysis);
        let catalog = AssetCatalog::load_repository(&repository_root()).unwrap();
        let strategy = build_asset_strategy(&analysis, &plan, &catalog, &[], &[]).unwrap();
        prepare_generation_request(
            &analysis,
            &plan,
            &strategy,
            &catalog,
            GenerationConfiguration::new(
                "generated.minimal_fixture",
                "fixture-model-v1",
                "ui-document-v1",
                GenerationParameters::new(0, 500_000, Some(7)).unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn valid_document() -> Value {
        serde_json::from_slice::<Value>(include_bytes!("../fixtures/generation/minimal.valid.json"))
            .unwrap()["document"]
            .clone()
    }

    fn invalid_document(field: &str, value: Value) -> Value {
        let mut document = valid_document();
        document["root"][field] = value;
        document
    }

    fn initial_execution(
        prepared: &PreparedGenerationRequest,
        document: Value,
    ) -> ProviderExecution {
        let request_id = ServerRequestId::new("initial-generation-001").unwrap();
        ProviderExecution {
            response: ProviderResponse {
                output: StructuredProviderOutput {
                    operation: ProviderOperation::StructuredGeneration,
                    schema: generation_output_contract(),
                    value: json!({
                        "document": document,
                        "assumptions": [],
                        "unimplemented_states": [],
                        "required_new_components": [],
                        "unsupported_capabilities": []
                    }),
                },
                server_request_id: Some(request_id.clone()),
                usage: ProviderUsage::default(),
            },
            trace: ProviderExecutionTrace {
                provider_id: ProviderId::new("initial-fixture").unwrap(),
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

    fn repair_output(document: Value, request_id: &str) -> MockScenario {
        MockScenario::Success {
            output: StructuredProviderOutput {
                operation: ProviderOperation::StructuredGeneration,
                schema: repair_output_contract(),
                value: json!({"document": document}),
            },
            request_id: Some(ServerRequestId::new(request_id).unwrap()),
        }
    }

    fn runner_with_attempt_timeout(
        scenarios: Vec<MockScenario>,
        attempt_timeout: Duration,
    ) -> (ProviderRunner, ProviderId) {
        let provider_id = ProviderId::new("repair-fixture").unwrap();
        let provider = MockProvider::new(
            ProviderDescriptor {
                id: provider_id.clone(),
                capabilities: ProviderCapabilities {
                    image_input: false,
                    structured_output: true,
                    max_image_count: 0,
                    operations: BTreeSet::from([ProviderOperation::StructuredGeneration]),
                },
            },
            scenarios,
        );
        let mut registry = ProviderRegistry::default();
        registry.register(Arc::new(provider)).unwrap();
        let runner = ProviderRunner::new(
            registry,
            ProviderExecutionPolicy {
                attempt_timeout,
                minimum_request_interval: Duration::ZERO,
                retry: RetryPolicy {
                    max_attempts: 1,
                    initial_backoff: Duration::ZERO,
                    max_backoff: Duration::ZERO,
                },
                task_limits: crate::observability::TaskExecutionLimits::default(),
            },
        )
        .unwrap();
        (runner, provider_id)
    }

    fn runner(scenarios: Vec<MockScenario>) -> (ProviderRunner, ProviderId) {
        // These tests exercise repair semantics. Keep scheduler contention from turning normal
        // mock responses into accidental timeout tests.
        runner_with_attempt_timeout(scenarios, Duration::from_secs(10))
    }

    fn run(initial: Value, scenarios: Vec<MockScenario>, maximum_rounds: u8) -> RepairRunResult {
        run_with_attempt_timeout(initial, scenarios, maximum_rounds, Duration::from_secs(10))
    }

    fn run_with_attempt_timeout(
        initial: Value,
        scenarios: Vec<MockScenario>,
        maximum_rounds: u8,
        attempt_timeout: Duration,
    ) -> RepairRunResult {
        let prepared = prepared();
        let initial = initial_execution(&prepared, initial);
        let (runner, provider_id) = runner_with_attempt_timeout(scenarios, attempt_timeout);
        repair_generated_document(
            &initial,
            &prepared,
            &runner,
            &provider_id,
            &CancellationToken::default(),
            RepairConfiguration::new(maximum_rounds).unwrap(),
        )
    }

    #[test]
    fn one_structured_round_repairs_and_is_deterministic() {
        let initial = invalid_document("unknown_provider_field", json!(true));
        let first = run(
            initial.clone(),
            vec![repair_output(valid_document(), "repair-001")],
            3,
        );
        let second = run(
            initial,
            vec![repair_output(valid_document(), "repair-001")],
            3,
        );
        assert_eq!(first.status, RepairRunStatus::Passed);
        assert_eq!(first.rounds.len(), 1);
        assert!(first.rounds[0].structured_request["current_staging_document"].is_object());
        assert!(first.rounds[0].structured_request["validation"].is_array());
        assert!(
            first.rounds[0].structured_request["validation"]
                .as_array()
                .unwrap()
                .iter()
                .any(|diagnostic| {
                    diagnostic["document_path"].as_str().is_some()
                        && diagnostic["node_id"] == "page.root"
                })
        );
        assert_eq!(
            first
                .final_document
                .as_ref()
                .unwrap()
                .canonical_document_json,
            second
                .final_document
                .as_ref()
                .unwrap()
                .canonical_document_json
        );
        assert_eq!(first.node_tree_summary, second.node_tree_summary);
    }

    #[test]
    fn unchanged_document_and_repeated_diagnostics_stop_early() {
        let initial = invalid_document("unknown_provider_field", json!(true));
        let unchanged = run(
            initial.clone(),
            vec![repair_output(initial.clone(), "repair-unchanged")],
            3,
        );
        assert_eq!(
            unchanged.failure.unwrap().kind,
            RepairFailureKind::NoProgress
        );

        let changed_same_error = invalid_document("unknown_provider_field", json!(false));
        let repeated = run(
            initial,
            vec![repair_output(changed_same_error, "repair-repeated")],
            3,
        );
        assert_eq!(
            repeated.failure.unwrap().kind,
            RepairFailureKind::RepeatedDiagnostics
        );
    }

    #[test]
    fn maximum_rounds_is_a_hard_limit() {
        let initial = invalid_document("unknown_a", json!(true));
        let round_one = invalid_document("unknown_b", json!(true));
        let result = run(initial, vec![repair_output(round_one, "repair-max-001")], 1);
        assert_eq!(result.rounds.len(), 1);
        assert_eq!(
            result.failure.unwrap().kind,
            RepairFailureKind::MaximumRoundsReached
        );
    }

    #[test]
    fn provider_unavailable_timeout_and_cancel_are_stable() {
        let initial = invalid_document("unknown_provider_field", json!(true));
        let unavailable = run(
            initial.clone(),
            vec![MockScenario::ServiceUnavailable { request_id: None }],
            3,
        );
        assert_eq!(
            unavailable.failure.unwrap().kind,
            RepairFailureKind::ProviderUnavailable
        );
        let timeout = run_with_attempt_timeout(
            initial.clone(),
            vec![MockScenario::Timeout],
            3,
            Duration::from_millis(20),
        );
        assert_eq!(
            timeout.failure.unwrap().kind,
            RepairFailureKind::ProviderTimeout
        );

        let prepared = prepared();
        let initial_execution = initial_execution(&prepared, initial);
        let (runner, provider_id) = runner(vec![repair_output(valid_document(), "unused")]);
        let cancellation = CancellationToken::default();
        cancellation.request();
        let cancelled = repair_generated_document(
            &initial_execution,
            &prepared,
            &runner,
            &provider_id,
            &cancellation,
            RepairConfiguration::default(),
        );
        assert_eq!(
            cancelled.failure.unwrap().kind,
            RepairFailureKind::ProviderCancelled
        );
    }

    #[test]
    fn policy_and_budget_cannot_be_relaxed_by_repair() {
        let initial = invalid_document("unknown_provider_field", json!(true));
        let mut invented_action = valid_document();
        invented_action["root"]["children"][0]["on_click"] = json!({"action": "game.unsafe"});
        let action = run(
            initial.clone(),
            vec![repair_output(invented_action, "repair-policy")],
            1,
        );
        assert_eq!(
            action.failure.unwrap().kind,
            RepairFailureKind::MaximumRoundsReached
        );
        assert_eq!(
            action.rounds[0].structured_request["guardrails"]["allow_actions"],
            json!(false)
        );

        let oversized = Value::String("x".repeat(UI_DOCUMENT_MAX_BYTES + 1));
        let over_budget = run(initial, vec![repair_output(oversized, "repair-budget")], 1);
        assert_eq!(
            over_budget.failure.unwrap().kind,
            RepairFailureKind::DocumentOverBudget
        );
    }

    #[test]
    fn node_tree_summary_is_ordered_by_document_path() {
        let canonical = project::framework::ui::document::tooling::canonicalize_json(
            &serde_json::to_string(&valid_document()).unwrap(),
        )
        .unwrap();
        let first = node_tree_summary(&canonical).unwrap();
        let second = node_tree_summary(&canonical).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.node_count, 2);
        assert_eq!(first.lines[0], "$.root|page.root|container|-");
        assert!(first.lines[1].contains("|page.title|text|page.root"));
    }
}
