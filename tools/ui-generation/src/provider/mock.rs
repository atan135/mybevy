use super::{
    Provider, ProviderCallContext, ProviderDescriptor, ProviderError, ProviderErrorKind,
    ProviderRequest, ProviderResponse, ProviderUsage, ServerRequestId, StructuredProviderOutput,
};
use std::{
    collections::VecDeque,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::Duration,
};

#[derive(Clone, Debug)]
pub enum MockScenario {
    Success {
        output: StructuredProviderOutput,
        request_id: Option<ServerRequestId>,
    },
    Timeout,
    RateLimited {
        retry_after: Duration,
        request_id: Option<ServerRequestId>,
    },
    AuthenticationFailure {
        request_id: Option<ServerRequestId>,
    },
    ServiceUnavailable {
        request_id: Option<ServerRequestId>,
    },
    MalformedResponse {
        request_id: Option<ServerRequestId>,
    },
}

pub struct MockProvider {
    descriptor: ProviderDescriptor,
    scenarios: Mutex<VecDeque<MockScenario>>,
    call_count: AtomicUsize,
}

impl MockProvider {
    pub fn new(
        descriptor: ProviderDescriptor,
        scenarios: impl IntoIterator<Item = MockScenario>,
    ) -> Self {
        Self {
            descriptor,
            scenarios: Mutex::new(scenarios.into_iter().collect()),
            call_count: AtomicUsize::new(0),
        }
    }

    pub fn call_count(&self) -> usize {
        self.call_count.load(Ordering::Acquire)
    }
}

impl Provider for MockProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        self.descriptor.clone()
    }

    fn invoke(
        &self,
        _request: ProviderRequest,
        context: ProviderCallContext,
    ) -> Result<ProviderResponse, ProviderError> {
        context.checkpoint()?;
        self.call_count.fetch_add(1, Ordering::AcqRel);
        let scenario = self
            .scenarios
            .lock()
            .expect("mock scenario mutex poisoned")
            .pop_front()
            .unwrap_or(MockScenario::ServiceUnavailable { request_id: None });
        match scenario {
            MockScenario::Success { output, request_id } => Ok(ProviderResponse {
                output,
                server_request_id: request_id,
                usage: ProviderUsage::default(),
            }),
            MockScenario::Timeout => loop {
                let remaining = context.remaining();
                if remaining.is_zero() {
                    return Err(ProviderError::new(ProviderErrorKind::Timeout));
                }
                context.checkpoint()?;
                thread::sleep(remaining.min(Duration::from_millis(1)));
            },
            MockScenario::RateLimited {
                retry_after,
                request_id,
            } => Err(with_request_id(
                ProviderError::new(ProviderErrorKind::RateLimited).with_retry_after(retry_after),
                request_id,
            )),
            MockScenario::AuthenticationFailure { request_id } => Err(with_request_id(
                ProviderError::new(ProviderErrorKind::Authentication),
                request_id,
            )),
            MockScenario::ServiceUnavailable { request_id } => Err(with_request_id(
                ProviderError::new(ProviderErrorKind::ServiceUnavailable),
                request_id,
            )),
            MockScenario::MalformedResponse { request_id } => Err(with_request_id(
                ProviderError::new(ProviderErrorKind::MalformedResponse),
                request_id,
            )),
        }
    }
}

fn with_request_id(error: ProviderError, request_id: Option<ServerRequestId>) -> ProviderError {
    request_id
        .map(|request_id| error.clone().with_request_id(request_id))
        .unwrap_or(error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        lifecycle::CancellationToken,
        provider::{ProviderCapabilities, ProviderId, ProviderOperation, StructuredOutputContract},
    };
    use std::{collections::BTreeSet, sync::Arc};

    fn descriptor() -> ProviderDescriptor {
        ProviderDescriptor {
            id: ProviderId::new("mock-errors").unwrap(),
            capabilities: ProviderCapabilities {
                image_input: true,
                structured_output: true,
                max_image_count: 4,
                operations: BTreeSet::from([ProviderOperation::VisualAnalysis]),
            },
        }
    }

    fn request() -> ProviderRequest {
        ProviderRequest::visual_analysis(
            "mock-errors",
            "prompt-v1",
            "fixture prompt",
            vec![
                super::super::ProviderImage::new(
                    "primary",
                    "image/png",
                    Arc::<[u8]>::from(b"fixture".as_slice()),
                )
                .unwrap(),
            ],
            StructuredOutputContract::new("ui-reference-analysis", 1).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn mock_covers_required_provider_failures() {
        let scenarios = [
            MockScenario::RateLimited {
                retry_after: Duration::from_millis(10),
                request_id: None,
            },
            MockScenario::AuthenticationFailure { request_id: None },
            MockScenario::ServiceUnavailable { request_id: None },
            MockScenario::MalformedResponse { request_id: None },
        ];
        let expected = [
            ProviderErrorKind::RateLimited,
            ProviderErrorKind::Authentication,
            ProviderErrorKind::ServiceUnavailable,
            ProviderErrorKind::MalformedResponse,
        ];
        let provider = MockProvider::new(descriptor(), scenarios);
        for expected_kind in expected {
            let error = provider
                .invoke(
                    request(),
                    ProviderCallContext::new(
                        1,
                        Duration::from_secs(1),
                        CancellationToken::default(),
                    ),
                )
                .unwrap_err();
            assert_eq!(error.kind, expected_kind);
        }
    }
}
