use super::{
    Provider, ProviderCallContext, ProviderDescriptor, ProviderError, ProviderErrorKind,
    ProviderOperation, ProviderRequest, ProviderResponse, ProviderUsage, ServerRequestId,
    StructuredOutputContract, StructuredProviderOutput,
};
use crate::lifecycle::{TaskFailure, TaskFailureKind};
use serde::Deserialize;
use serde_json::Value;
use std::{fs, path::Path};

const FIXTURE_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FixtureCase {
    Valid,
    Invalid,
    OverBudget,
    Interrupted,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureDocument {
    fixture_version: u32,
    case: FixtureCase,
    source: FixtureSource,
    provider: ProviderDescriptor,
    expected_operation: ProviderOperation,
    outcome: FixtureOutcome,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureSource {
    description: String,
    authored_for_tests: bool,
    contains_sensitive_data: bool,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum FixtureOutcome {
    Success {
        schema: StructuredOutputContract,
        value: Value,
        server_request_id: Option<String>,
        input_units: Option<u64>,
        output_units: Option<u64>,
    },
    MalformedResponse {
        server_request_id: Option<String>,
    },
    Interrupted,
}

pub struct FixtureProvider {
    fixture: FixtureDocument,
}

impl FixtureProvider {
    pub fn load(path: &Path) -> Result<Self, TaskFailure> {
        let source = fs::read_to_string(path).map_err(|_| fixture_failure())?;
        let fixture: FixtureDocument =
            serde_json::from_str(&source).map_err(|_| fixture_failure())?;
        if fixture.fixture_version != FIXTURE_VERSION
            || fixture.source.description.trim().is_empty()
            || !fixture.source.authored_for_tests
            || fixture.source.contains_sensitive_data
        {
            return Err(fixture_failure());
        }
        validate_fixture_case(&fixture)?;
        Ok(Self { fixture })
    }

    pub fn case(&self) -> FixtureCase {
        self.fixture.case
    }

    /// Rebinds only the structured value of a repository-authored success fixture.
    /// Operation, schema, provider metadata, usage, and request identity remain fixture-owned.
    pub fn bind_success_value(&mut self, value: Value) -> Result<(), TaskFailure> {
        match (&self.fixture.case, &mut self.fixture.outcome) {
            (FixtureCase::Valid, FixtureOutcome::Success { value: output, .. }) => {
                *output = value;
                Ok(())
            }
            _ => Err(fixture_failure()),
        }
    }

    pub fn success_server_request_id(&self) -> Option<&str> {
        match &self.fixture.outcome {
            FixtureOutcome::Success {
                server_request_id, ..
            } => server_request_id.as_deref(),
            _ => None,
        }
    }
}

impl Provider for FixtureProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        self.fixture.provider.clone()
    }

    fn invoke(
        &self,
        request: ProviderRequest,
        context: ProviderCallContext,
    ) -> Result<ProviderResponse, ProviderError> {
        context.checkpoint()?;
        if request.operation() != self.fixture.expected_operation {
            return Err(ProviderError::new(ProviderErrorKind::MalformedResponse));
        }
        match &self.fixture.outcome {
            FixtureOutcome::Success {
                schema,
                value,
                server_request_id,
                input_units,
                output_units,
            } => Ok(ProviderResponse {
                output: StructuredProviderOutput {
                    operation: self.fixture.expected_operation.clone(),
                    schema: schema.clone(),
                    value: value.clone(),
                },
                server_request_id: parse_request_id(server_request_id.as_deref())?,
                usage: ProviderUsage {
                    input_units: *input_units,
                    output_units: *output_units,
                },
            }),
            FixtureOutcome::MalformedResponse { server_request_id } => {
                let mut error = ProviderError::new(ProviderErrorKind::MalformedResponse);
                if let Some(request_id) = parse_request_id(server_request_id.as_deref())? {
                    error = error.with_request_id(request_id);
                }
                Err(error)
            }
            FixtureOutcome::Interrupted => Err(ProviderError::new(ProviderErrorKind::Cancelled)),
        }
    }
}

fn parse_request_id(value: Option<&str>) -> Result<Option<ServerRequestId>, ProviderError> {
    value
        .map(|value| {
            ServerRequestId::new(value)
                .map_err(|_| ProviderError::new(ProviderErrorKind::MalformedResponse))
        })
        .transpose()
}

fn validate_fixture_case(fixture: &FixtureDocument) -> Result<(), TaskFailure> {
    let valid_pair = matches!(
        (fixture.case, &fixture.outcome),
        (FixtureCase::Valid, FixtureOutcome::Success { .. })
            | (FixtureCase::OverBudget, FixtureOutcome::Success { .. })
            | (
                FixtureCase::Invalid,
                FixtureOutcome::MalformedResponse { .. }
            )
            | (FixtureCase::Interrupted, FixtureOutcome::Interrupted)
    );
    if valid_pair {
        Ok(())
    } else {
        Err(fixture_failure())
    }
}

fn fixture_failure() -> TaskFailure {
    TaskFailure::new(
        TaskFailureKind::ProviderResponseMalformed,
        "provider fixture is unreadable, sensitive, or violates the fixture contract",
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        lifecycle::CancellationToken,
        provider::{
            ProviderCallContext, ProviderErrorKind, ProviderImage, ProviderRequest,
            StructuredOutputContract,
        },
    };
    use std::{path::PathBuf, sync::Arc, time::Duration};

    fn fixture_path(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/providers")
            .join(name)
    }

    fn request() -> ProviderRequest {
        ProviderRequest::visual_analysis(
            "fixture-provider-run",
            "prompt-v1",
            "repository-authored fixture instruction",
            vec![
                ProviderImage::new(
                    "primary",
                    "image/png",
                    Arc::<[u8]>::from(b"fixture-image".as_slice()),
                )
                .unwrap(),
            ],
            StructuredOutputContract::new("ui-reference-analysis", 1).unwrap(),
        )
        .unwrap()
    }

    fn invoke(name: &str) -> Result<ProviderResponse, ProviderError> {
        FixtureProvider::load(&fixture_path(name)).unwrap().invoke(
            request(),
            ProviderCallContext::new(1, Duration::from_secs(1), CancellationToken::default()),
        )
    }

    #[test]
    fn fixture_provider_reads_valid_and_over_budget_structured_outputs() {
        let valid = invoke("valid.json").unwrap();
        assert_eq!(valid.output.value["regions"], serde_json::json!([]));
        assert_eq!(
            valid
                .server_request_id
                .as_ref()
                .map(ServerRequestId::as_str),
            Some("fixture-valid-001")
        );

        let over_budget = invoke("over_budget.json").unwrap();
        assert_eq!(over_budget.output.value["simulated_node_count"], 10001);
    }

    #[test]
    fn fixture_provider_reads_invalid_and_interrupted_responses() {
        assert_eq!(
            invoke("invalid.json").unwrap_err().kind,
            ProviderErrorKind::MalformedResponse
        );
        assert_eq!(
            invoke("interrupted.json").unwrap_err().kind,
            ProviderErrorKind::Cancelled
        );
    }

    #[test]
    fn valid_fixture_can_bind_dynamic_structured_evidence_without_changing_identity() {
        let mut provider = FixtureProvider::load(&fixture_path("valid.json")).unwrap();
        provider
            .bind_success_value(serde_json::json!({"bound": true}))
            .unwrap();

        let response = provider
            .invoke(
                request(),
                ProviderCallContext::new(1, Duration::from_secs(1), CancellationToken::default()),
            )
            .unwrap();
        assert_eq!(response.output.value, serde_json::json!({"bound": true}));
        assert_eq!(
            response
                .server_request_id
                .as_ref()
                .map(ServerRequestId::as_str),
            Some("fixture-valid-001")
        );
    }
}
