#[cfg(feature = "full")]
use super::{FixtureProvider, ProviderImage, StructuredOutputContract};
use super::{
    Provider, ProviderCallContext, ProviderError, ProviderErrorKind, ProviderId, ProviderRequest,
    ProviderResponse, RequestLogMetadata, ServerRequestId, duration_millis, validate_response,
};
#[cfg(feature = "full")]
use crate::operations::ProviderRuntimeGovernor;
use crate::{
    lifecycle::{CancellationToken, TaskFailure, TaskFailureKind},
    provider_budget::{TaskBudget, TaskExecutionLimits},
};
use serde::Serialize;
#[cfg(feature = "full")]
use std::path::Path;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

const CANCELLATION_POLL_INTERVAL: Duration = Duration::from_millis(2);
const MAX_POLICY_DURATION: Duration = Duration::from_secs(60 * 60);

#[derive(Clone, Debug)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
}

impl RetryPolicy {
    pub fn validate(&self) -> Result<(), TaskFailure> {
        if self.max_attempts == 0 || self.max_attempts > 10 {
            return Err(TaskFailure::invalid(
                "provider max_attempts must be between 1 and 10",
            ));
        }
        if self.initial_backoff > self.max_backoff {
            return Err(TaskFailure::invalid(
                "provider initial retry backoff cannot exceed maximum backoff",
            ));
        }
        if self.max_backoff > MAX_POLICY_DURATION {
            return Err(TaskFailure::invalid(
                "provider retry backoff cannot exceed one hour",
            ));
        }
        Ok(())
    }

    fn delay_for(&self, completed_attempt: u32, error: &ProviderError) -> Duration {
        if let Some(retry_after) = error.retry_after() {
            return retry_after.min(self.max_backoff);
        }
        let exponent = completed_attempt.saturating_sub(1).min(31);
        self.initial_backoff
            .saturating_mul(1_u32 << exponent)
            .min(self.max_backoff)
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(250),
            max_backoff: Duration::from_secs(2),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ProviderExecutionPolicy {
    pub attempt_timeout: Duration,
    pub minimum_request_interval: Duration,
    pub retry: RetryPolicy,
    /// Limits are checked before every provider attempt and after each reported usage record.
    pub task_limits: TaskExecutionLimits,
}

impl ProviderExecutionPolicy {
    pub fn validate(&self) -> Result<(), TaskFailure> {
        if self.attempt_timeout.is_zero() {
            return Err(TaskFailure::invalid(
                "provider attempt timeout must be greater than zero",
            ));
        }
        if self.attempt_timeout > MAX_POLICY_DURATION
            || self.minimum_request_interval > MAX_POLICY_DURATION
        {
            return Err(TaskFailure::invalid(
                "provider timeout and local rate interval cannot exceed one hour",
            ));
        }
        self.retry.validate()?;
        self.task_limits.validate()
    }
}

impl Default for ProviderExecutionPolicy {
    fn default() -> Self {
        Self {
            attempt_timeout: Duration::from_secs(60),
            minimum_request_interval: Duration::ZERO,
            retry: RetryPolicy::default(),
            task_limits: TaskExecutionLimits::default(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderAttemptOutcome {
    Succeeded,
    Failed { kind: ProviderErrorKind },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderAttemptTrace {
    pub attempt: u32,
    pub outcome: ProviderAttemptOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_request_id: Option<ServerRequestId>,
    pub elapsed_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderExecutionTrace {
    pub provider_id: ProviderId,
    pub request: RequestLogMetadata,
    pub attempts: Vec<ProviderAttemptTrace>,
}

#[derive(Clone, Debug)]
pub struct ProviderExecution {
    pub response: ProviderResponse,
    pub trace: ProviderExecutionTrace,
}

#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderExecutionFailure {
    pub failure: TaskFailure,
    pub trace: ProviderExecutionTrace,
}

/// Deterministic, repository-owned evidence that the optional shared governor is wired into the
/// real provider execution path. It uses only the local fixture provider and never reads a secret
/// or opens a network connection.
#[cfg(feature = "full")]
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OfflineRuntimeGovernanceStressReport {
    pub first_provider_call_succeeded: bool,
    pub daily_budget_blocked_second_call: bool,
    pub provider_calls_recorded: u32,
    pub final_status: String,
}

#[cfg(feature = "full")]
pub fn run_offline_runtime_governance_stress_fixture()
-> Result<OfflineRuntimeGovernanceStressReport, TaskFailure> {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/providers/valid.json");
    let provider = Arc::new(FixtureProvider::load(&fixture_path)?);
    let provider_id = provider.descriptor().id.clone();
    let mut registry = ProviderRegistry::default();
    registry.register(provider)?;
    let limits = TaskExecutionLimits {
        max_provider_calls: 1,
        max_images: 2,
        max_iterations: 2,
        ..TaskExecutionLimits::default()
    };
    let governor = Arc::new(ProviderRuntimeGovernor::new(
        crate::operations::TaskQueuePolicy {
            max_queued_or_running: 2,
            default_provider_concurrency: 1,
            provider_concurrency: BTreeMap::new(),
        },
        crate::operations::DailyBudget::new("2026-07-22", limits)?,
    )?);
    let runner = ProviderRunner::new_with_runtime_governor(
        registry,
        ProviderExecutionPolicy {
            retry: RetryPolicy {
                max_attempts: 1,
                initial_backoff: Duration::ZERO,
                max_backoff: Duration::ZERO,
            },
            ..ProviderExecutionPolicy::default()
        },
        governor.clone(),
    )?;
    let request = || {
        ProviderRequest::visual_analysis(
            "runtime-governance-stress",
            "fixture-prompt-v1",
            "repository-owned offline fixture instruction",
            vec![ProviderImage::new(
                "fixture-reference",
                "image/png",
                Arc::<[u8]>::from(b"offline-runtime-governance".as_slice()),
            )?],
            StructuredOutputContract::new("ui-reference-analysis", 1)?,
        )
    };
    runner
        .execute(&provider_id, request()?, &CancellationToken::default())
        .map_err(|failure| failure.failure)?;
    let blocked = runner
        .execute(&provider_id, request()?, &CancellationToken::default())
        .unwrap_err();
    let daily_budget_blocked_second_call = blocked.failure.kind()
        == TaskFailureKind::ProviderRateLimited
        && blocked.failure.subject() == Some("UI_GENERATION_DAILY_PROVIDER_CALLS");
    if !daily_budget_blocked_second_call {
        return Err(TaskFailure::new(
            TaskFailureKind::ProviderRateLimited,
            "runtime governance stress fixture did not block the second provider call",
            None,
        ));
    }
    let snapshot = governor.daily_snapshot();
    Ok(OfflineRuntimeGovernanceStressReport {
        first_provider_call_succeeded: true,
        daily_budget_blocked_second_call,
        provider_calls_recorded: snapshot.usage.provider_calls,
        final_status: "passed".to_owned(),
    })
}

#[derive(Default)]
pub struct ProviderRegistry {
    providers: BTreeMap<ProviderId, Arc<dyn Provider>>,
}

impl ProviderRegistry {
    pub fn register(&mut self, provider: Arc<dyn Provider>) -> Result<(), TaskFailure> {
        let descriptor = provider.descriptor();
        if self.providers.contains_key(&descriptor.id) {
            return Err(TaskFailure::invalid(format!(
                "provider ID is already registered: {}",
                descriptor.id.as_str()
            )));
        }
        self.providers.insert(descriptor.id, provider);
        Ok(())
    }

    pub fn select(&self, provider_id: &ProviderId) -> Result<Arc<dyn Provider>, TaskFailure> {
        self.providers.get(provider_id).cloned().ok_or_else(|| {
            TaskFailure::new(
                TaskFailureKind::ProviderNotFound,
                "requested provider is not registered",
                Some(provider_id.as_str().to_owned()),
            )
        })
    }
}

pub struct ProviderRunner {
    registry: ProviderRegistry,
    policy: ProviderExecutionPolicy,
    rate_slots: Mutex<BTreeMap<ProviderId, Instant>>,
    rate_limit_clock: Arc<dyn RateLimitClock>,
    #[cfg(feature = "full")]
    runtime_governor: Option<Arc<ProviderRuntimeGovernor>>,
}

impl ProviderRunner {
    pub fn new(
        registry: ProviderRegistry,
        policy: ProviderExecutionPolicy,
    ) -> Result<Self, TaskFailure> {
        policy.validate()?;
        Ok(Self {
            registry,
            policy,
            rate_slots: Mutex::new(BTreeMap::new()),
            rate_limit_clock: Arc::new(SystemRateLimitClock),
            #[cfg(feature = "full")]
            runtime_governor: None,
        })
    }

    /// Installs explicit shared governance for this runner. Multiple runners receive the same
    /// `Arc` when their calls must share queue slots and a daily provider budget.
    #[cfg(feature = "full")]
    pub fn new_with_runtime_governor(
        registry: ProviderRegistry,
        policy: ProviderExecutionPolicy,
        runtime_governor: Arc<ProviderRuntimeGovernor>,
    ) -> Result<Self, TaskFailure> {
        let mut runner = Self::new(registry, policy)?;
        runner.runtime_governor = Some(runtime_governor);
        Ok(runner)
    }

    #[cfg(test)]
    fn with_rate_limit_clock(
        registry: ProviderRegistry,
        policy: ProviderExecutionPolicy,
        rate_limit_clock: Arc<dyn RateLimitClock>,
    ) -> Result<Self, TaskFailure> {
        policy.validate()?;
        Ok(Self {
            registry,
            policy,
            rate_slots: Mutex::new(BTreeMap::new()),
            rate_limit_clock,
            #[cfg(feature = "full")]
            runtime_governor: None,
        })
    }

    #[allow(clippy::result_large_err)]
    pub fn execute(
        &self,
        provider_id: &ProviderId,
        request: ProviderRequest,
        cancellation: &CancellationToken,
    ) -> Result<ProviderExecution, ProviderExecutionFailure> {
        let budget = TaskBudget::new(self.policy.task_limits.clone()).map_err(|failure| {
            ProviderExecutionFailure {
                failure,
                trace: ProviderExecutionTrace {
                    provider_id: provider_id.clone(),
                    request: request.log_metadata(),
                    attempts: Vec::new(),
                },
            }
        })?;
        self.execute_with_budget(provider_id, request, cancellation, &budget)
    }

    /// Uses a caller-owned task budget so analysis, generation, and bounded repair can share
    /// one hard stop instead of each phase receiving an independent retry allowance.
    #[allow(clippy::result_large_err)]
    pub fn execute_with_budget(
        &self,
        provider_id: &ProviderId,
        request: ProviderRequest,
        cancellation: &CancellationToken,
        budget: &TaskBudget,
    ) -> Result<ProviderExecution, ProviderExecutionFailure> {
        let metadata = request.log_metadata();
        let mut trace = ProviderExecutionTrace {
            provider_id: provider_id.clone(),
            request: metadata,
            attempts: Vec::new(),
        };
        let provider =
            self.registry
                .select(provider_id)
                .map_err(|failure| ProviderExecutionFailure {
                    failure,
                    trace: trace.clone(),
                })?;
        provider
            .descriptor()
            .capabilities
            .validate_request(&request)
            .map_err(|failure| ProviderExecutionFailure {
                failure,
                trace: trace.clone(),
            })?;

        let mut attempt = 1;
        loop {
            if cancellation.is_requested() {
                return Err(cancelled_failure(trace));
            }
            let rate_wait = self.reserve_rate_slot(provider_id);
            if !self.rate_limit_clock.wait(rate_wait, cancellation) {
                return Err(cancelled_failure(trace));
            }
            #[cfg(feature = "full")]
            let runtime_lease = match self.runtime_governor.as_ref() {
                Some(governor) => Some(
                    governor
                        .acquire_provider_attempt(
                            &trace.request.run_id,
                            attempt,
                            provider_id.as_str(),
                            request.image_count(),
                            cancellation,
                            budget,
                        )
                        .map_err(|failure| ProviderExecutionFailure {
                            failure,
                            trace: trace.clone(),
                        })?,
                ),
                None => {
                    // Rate waiting belongs to the task wall clock. Reserve only after it has
                    // elapsed so an overdue task cannot begin one more provider attempt.
                    budget
                        .reserve_provider_attempt(request.image_count())
                        .map_err(|failure| ProviderExecutionFailure {
                            failure,
                            trace: trace.clone(),
                        })?;
                    None
                }
            };

            #[cfg(not(feature = "full"))]
            budget
                .reserve_provider_attempt(request.image_count())
                .map_err(|failure| ProviderExecutionFailure {
                    failure,
                    trace: trace.clone(),
                })?;

            let started = Instant::now();
            let result = invoke_with_timeout(
                Arc::clone(&provider),
                request.clone(),
                attempt,
                self.policy.attempt_timeout,
                cancellation,
            );
            let elapsed_ms = duration_millis(started.elapsed());
            match result.and_then(|response| {
                validate_response(&request, &response)?;
                Ok(response)
            }) {
                Ok(response) => {
                    #[cfg(feature = "full")]
                    if let Some(runtime_lease) = &runtime_lease
                        && let Err(failure) = runtime_lease.record_success(
                            elapsed_ms,
                            response.usage.input_units,
                            response.usage.output_units,
                        )
                    {
                        trace.attempts.push(ProviderAttemptTrace {
                            attempt,
                            outcome: ProviderAttemptOutcome::Succeeded,
                            server_request_id: response.server_request_id.clone(),
                            elapsed_ms,
                        });
                        return Err(ProviderExecutionFailure { failure, trace });
                    }
                    if let Err(failure) = budget.record_provider_usage(
                        response.usage.input_units,
                        response.usage.output_units,
                    ) {
                        trace.attempts.push(ProviderAttemptTrace {
                            attempt,
                            outcome: ProviderAttemptOutcome::Succeeded,
                            server_request_id: response.server_request_id.clone(),
                            elapsed_ms,
                        });
                        return Err(ProviderExecutionFailure { failure, trace });
                    }
                    trace.attempts.push(ProviderAttemptTrace {
                        attempt,
                        outcome: ProviderAttemptOutcome::Succeeded,
                        server_request_id: response.server_request_id.clone(),
                        elapsed_ms,
                    });
                    return Ok(ProviderExecution { response, trace });
                }
                Err(error) => {
                    trace.attempts.push(ProviderAttemptTrace {
                        attempt,
                        outcome: ProviderAttemptOutcome::Failed { kind: error.kind },
                        server_request_id: error.server_request_id.clone(),
                        elapsed_ms,
                    });
                    #[cfg(feature = "full")]
                    if let Some(runtime_lease) = &runtime_lease
                        && let Err(failure) = runtime_lease.record_elapsed(elapsed_ms)
                    {
                        return Err(ProviderExecutionFailure { failure, trace });
                    }
                    if error.kind == ProviderErrorKind::Cancelled || cancellation.is_requested() {
                        return Err(cancelled_failure(trace));
                    }
                    if !error.kind.retryable() || attempt >= self.policy.retry.max_attempts {
                        return Err(ProviderExecutionFailure {
                            failure: error.to_task_failure(),
                            trace,
                        });
                    }
                    #[cfg(feature = "full")]
                    drop(runtime_lease);
                    let delay = self.policy.retry.delay_for(attempt, &error);
                    if !sleep_with_cancellation(delay, cancellation) {
                        return Err(cancelled_failure(trace));
                    }
                    attempt += 1;
                }
            }
        }
    }

    pub fn task_limits(&self) -> TaskExecutionLimits {
        self.policy.task_limits.clone()
    }

    fn reserve_rate_slot(&self, provider_id: &ProviderId) -> Duration {
        if self.policy.minimum_request_interval.is_zero() {
            return Duration::ZERO;
        }
        let now = self.rate_limit_clock.now();
        let mut slots = self.rate_slots.lock().expect("rate limiter mutex poisoned");
        let reserved = slots
            .get(provider_id)
            .copied()
            .map(|next| next.max(now))
            .unwrap_or(now);
        slots.insert(
            provider_id.clone(),
            reserved + self.policy.minimum_request_interval,
        );
        reserved.saturating_duration_since(now)
    }
}

trait RateLimitClock: Send + Sync {
    fn now(&self) -> Instant;
    fn wait(&self, duration: Duration, cancellation: &CancellationToken) -> bool;
}

struct SystemRateLimitClock;

impl RateLimitClock for SystemRateLimitClock {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn wait(&self, duration: Duration, cancellation: &CancellationToken) -> bool {
        sleep_with_cancellation(duration, cancellation)
    }
}

fn invoke_with_timeout(
    provider: Arc<dyn Provider>,
    request: ProviderRequest,
    attempt: u32,
    timeout: Duration,
    outer_cancellation: &CancellationToken,
) -> Result<ProviderResponse, ProviderError> {
    let attempt_cancellation = CancellationToken::default();
    let worker_cancellation = attempt_cancellation.clone();
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let context = ProviderCallContext::new(attempt, timeout, worker_cancellation);
        let _ = sender.send(provider.invoke(request, context));
    });

    let started = Instant::now();
    loop {
        if outer_cancellation.is_requested() {
            attempt_cancellation.request();
            return Err(ProviderError::new(ProviderErrorKind::Cancelled));
        }
        let remaining = timeout.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            attempt_cancellation.request();
            return Err(ProviderError::new(ProviderErrorKind::Timeout));
        }
        match receiver.recv_timeout(remaining.min(CANCELLATION_POLL_INTERVAL)) {
            Ok(result) => return result,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(ProviderError::new(ProviderErrorKind::ServiceUnavailable));
            }
        }
    }
}

fn sleep_with_cancellation(duration: Duration, cancellation: &CancellationToken) -> bool {
    let started = Instant::now();
    while started.elapsed() < duration {
        if cancellation.is_requested() {
            return false;
        }
        thread::sleep(
            duration
                .saturating_sub(started.elapsed())
                .min(CANCELLATION_POLL_INTERVAL),
        );
    }
    !cancellation.is_requested()
}

fn cancelled_failure(trace: ProviderExecutionTrace) -> ProviderExecutionFailure {
    ProviderExecutionFailure {
        failure: TaskFailure::new(
            TaskFailureKind::Cancelled,
            "provider request was cancelled",
            None,
        ),
        trace,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "full")]
    use crate::operations::{
        DailyBudget, DailyBudgetSnapshot, ProviderRuntimeGovernor, TaskQueuePolicy,
    };
    use crate::provider::{
        FixtureProvider, MockProvider, MockScenario, ProviderCapabilities, ProviderDescriptor,
        ProviderOperation, StructuredProviderOutput,
        tests::{test_contract, test_request},
    };
    use crate::provider_budget::{TaskBudget, TaskExecutionLimits, TaskUsageSnapshot};
    use serde_json::json;
    use std::{collections::BTreeSet, path::Path};

    struct DeterministicRateLimitClock {
        now: Mutex<Instant>,
        waits: Mutex<Vec<Duration>>,
    }

    impl DeterministicRateLimitClock {
        fn new() -> Self {
            Self {
                now: Mutex::new(Instant::now()),
                waits: Mutex::new(Vec::new()),
            }
        }

        fn waits(&self) -> Vec<Duration> {
            self.waits.lock().expect("waits mutex poisoned").clone()
        }
    }

    impl RateLimitClock for DeterministicRateLimitClock {
        fn now(&self) -> Instant {
            *self.now.lock().expect("clock mutex poisoned")
        }

        fn wait(&self, duration: Duration, cancellation: &CancellationToken) -> bool {
            if cancellation.is_requested() {
                return false;
            }
            if !duration.is_zero() {
                self.waits
                    .lock()
                    .expect("waits mutex poisoned")
                    .push(duration);
                let mut now = self.now.lock().expect("clock mutex poisoned");
                *now += duration;
            }
            true
        }
    }

    fn descriptor(id: &str) -> ProviderDescriptor {
        ProviderDescriptor {
            id: ProviderId::new(id).unwrap(),
            capabilities: ProviderCapabilities {
                image_input: true,
                structured_output: true,
                max_image_count: 4,
                operations: BTreeSet::from([
                    ProviderOperation::VisualAnalysis,
                    ProviderOperation::StructuredGeneration,
                ]),
            },
        }
    }

    fn success(request_id: &str) -> MockScenario {
        MockScenario::Success {
            output: StructuredProviderOutput {
                operation: ProviderOperation::VisualAnalysis,
                schema: test_contract(),
                value: json!({"regions": []}),
            },
            request_id: Some(ServerRequestId::new(request_id).unwrap()),
        }
    }

    fn test_policy(max_attempts: u32) -> ProviderExecutionPolicy {
        ProviderExecutionPolicy {
            // These tests exercise retry, contract, and rate-limit semantics. Keep scheduling
            // contention from turning them into accidental timeout tests.
            attempt_timeout: Duration::from_secs(10),
            minimum_request_interval: Duration::ZERO,
            retry: RetryPolicy {
                max_attempts,
                initial_backoff: Duration::from_millis(1),
                max_backoff: Duration::from_millis(3),
            },
            task_limits: TaskExecutionLimits::default(),
        }
    }

    fn timeout_test_policy(max_attempts: u32) -> ProviderExecutionPolicy {
        ProviderExecutionPolicy {
            attempt_timeout: Duration::from_millis(20),
            ..test_policy(max_attempts)
        }
    }

    #[cfg(feature = "full")]
    fn shared_runtime_governor(
        limits: TaskExecutionLimits,
        provider_concurrency: u32,
    ) -> Arc<ProviderRuntimeGovernor> {
        Arc::new(
            ProviderRuntimeGovernor::new(
                TaskQueuePolicy {
                    max_queued_or_running: 4,
                    default_provider_concurrency: provider_concurrency,
                    provider_concurrency: BTreeMap::new(),
                },
                DailyBudget::new("2026-07-22", limits).unwrap(),
            )
            .unwrap(),
        )
    }

    #[cfg(feature = "full")]
    fn runtime_governor_from_snapshot(
        limits: TaskExecutionLimits,
        usage: TaskUsageSnapshot,
    ) -> Arc<ProviderRuntimeGovernor> {
        let daily_budget = DailyBudget::from_snapshot(
            limits,
            DailyBudgetSnapshot {
                day: "2026-07-22".to_owned(),
                usage,
            },
        )
        .unwrap();
        Arc::new(ProviderRuntimeGovernor::new(TaskQueuePolicy::default(), daily_budget).unwrap())
    }

    #[cfg(feature = "full")]
    fn fixture_provider_path(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/providers")
            .join(name)
    }

    #[test]
    fn execution_policy_rejects_unbounded_attempts_and_durations() {
        let mut policy = test_policy(11);
        assert!(policy.validate().is_err());
        policy = test_policy(1);
        policy.attempt_timeout = MAX_POLICY_DURATION + Duration::from_millis(1);
        assert!(policy.validate().is_err());
    }

    #[test]
    fn registry_selects_by_stable_provider_id() {
        let mut registry = ProviderRegistry::default();
        registry
            .register(Arc::new(MockProvider::new(
                descriptor("fixture-a"),
                [success("request-a")],
            )))
            .unwrap();
        assert_eq!(
            registry
                .select(&ProviderId::new("fixture-a").unwrap())
                .unwrap()
                .descriptor()
                .id
                .as_str(),
            "fixture-a"
        );
        let failure = match registry.select(&ProviderId::new("missing").unwrap()) {
            Ok(_) => panic!("missing provider must not be selected"),
            Err(failure) => failure,
        };
        assert_eq!(failure.kind(), TaskFailureKind::ProviderNotFound);
    }

    #[test]
    fn retry_is_limited_and_records_each_server_request_id() {
        let provider = Arc::new(MockProvider::new(
            descriptor("retry-fixture"),
            [
                MockScenario::RateLimited {
                    retry_after: Duration::from_millis(1),
                    request_id: Some(ServerRequestId::new("rate-limit-001").unwrap()),
                },
                MockScenario::ServiceUnavailable {
                    request_id: Some(ServerRequestId::new("service-002").unwrap()),
                },
                success("success-003"),
            ],
        ));
        let mut registry = ProviderRegistry::default();
        registry.register(provider.clone()).unwrap();
        let runner = ProviderRunner::new(registry, test_policy(3)).unwrap();
        let execution = runner
            .execute(
                &ProviderId::new("retry-fixture").unwrap(),
                test_request(),
                &CancellationToken::default(),
            )
            .unwrap();
        assert_eq!(provider.call_count(), 3);
        assert_eq!(execution.trace.attempts.len(), 3);
        assert_eq!(
            execution.trace.attempts[0]
                .server_request_id
                .as_ref()
                .map(ServerRequestId::as_str),
            Some("rate-limit-001")
        );
        assert_eq!(
            execution
                .response
                .server_request_id
                .as_ref()
                .map(ServerRequestId::as_str),
            Some("success-003")
        );
    }

    #[test]
    fn retry_stops_at_configured_attempt_limit() {
        let provider = Arc::new(MockProvider::new(
            descriptor("limited-retry"),
            [
                MockScenario::ServiceUnavailable { request_id: None },
                MockScenario::ServiceUnavailable { request_id: None },
                success("must-not-run"),
            ],
        ));
        let mut registry = ProviderRegistry::default();
        registry.register(provider.clone()).unwrap();
        let runner = ProviderRunner::new(registry, test_policy(2)).unwrap();
        let failure = runner
            .execute(
                &ProviderId::new("limited-retry").unwrap(),
                test_request(),
                &CancellationToken::default(),
            )
            .unwrap_err();
        assert_eq!(provider.call_count(), 2);
        assert_eq!(failure.trace.attempts.len(), 2);
        assert_eq!(
            failure.failure.kind(),
            TaskFailureKind::ProviderServiceUnavailable
        );
    }

    #[test]
    fn response_contract_mismatch_is_not_retried_and_keeps_request_id() {
        let provider = Arc::new(MockProvider::new(
            descriptor("mismatched-schema"),
            [MockScenario::Success {
                output: StructuredProviderOutput {
                    operation: ProviderOperation::VisualAnalysis,
                    schema: super::super::StructuredOutputContract::new("wrong-schema", 1).unwrap(),
                    value: json!({}),
                },
                request_id: Some(ServerRequestId::new("mismatch-001").unwrap()),
            }],
        ));
        let mut registry = ProviderRegistry::default();
        registry.register(provider.clone()).unwrap();
        let runner = ProviderRunner::new(registry, test_policy(3)).unwrap();
        let failure = runner
            .execute(
                &ProviderId::new("mismatched-schema").unwrap(),
                test_request(),
                &CancellationToken::default(),
            )
            .unwrap_err();
        assert_eq!(provider.call_count(), 1);
        assert_eq!(
            failure.failure.kind(),
            TaskFailureKind::ProviderResponseMalformed
        );
        assert_eq!(failure.failure.server_request_id(), Some("mismatch-001"));
        assert_eq!(
            failure.trace.attempts[0]
                .server_request_id
                .as_ref()
                .map(ServerRequestId::as_str),
            Some("mismatch-001")
        );
    }

    #[test]
    fn runner_enforces_timeout_and_observes_cancellation() {
        let provider = Arc::new(MockProvider::new(
            descriptor("timeout-fixture"),
            [MockScenario::Timeout, MockScenario::Timeout],
        ));
        let mut registry = ProviderRegistry::default();
        registry.register(provider).unwrap();
        let runner = ProviderRunner::new(registry, timeout_test_policy(1)).unwrap();
        let started = Instant::now();
        let failure = runner
            .execute(
                &ProviderId::new("timeout-fixture").unwrap(),
                test_request(),
                &CancellationToken::default(),
            )
            .unwrap_err();
        assert_eq!(failure.failure.kind(), TaskFailureKind::ProviderTimeout);
        assert!(started.elapsed() < Duration::from_millis(200));

        let cancellation = CancellationToken::default();
        cancellation.request();
        let cancelled = runner
            .execute(
                &ProviderId::new("timeout-fixture").unwrap(),
                test_request(),
                &cancellation,
            )
            .unwrap_err();
        assert_eq!(cancelled.failure.kind(), TaskFailureKind::Cancelled);
    }

    #[test]
    fn local_rate_limit_spaces_provider_calls() {
        let provider = Arc::new(MockProvider::new(
            descriptor("locally-limited"),
            [success("first"), success("second")],
        ));
        let mut registry = ProviderRegistry::default();
        registry.register(provider.clone()).unwrap();
        let mut policy = test_policy(1);
        policy.minimum_request_interval = Duration::from_millis(12);
        let clock = Arc::new(DeterministicRateLimitClock::new());
        let runner =
            ProviderRunner::with_rate_limit_clock(registry, policy, clock.clone()).unwrap();
        let id = ProviderId::new("locally-limited").unwrap();
        runner
            .execute(&id, test_request(), &CancellationToken::default())
            .unwrap();
        runner
            .execute(&id, test_request(), &CancellationToken::default())
            .unwrap();
        assert_eq!(provider.call_count(), 2);
        assert_eq!(clock.waits(), vec![Duration::from_millis(12)]);
    }

    #[test]
    fn caller_owned_task_budget_stops_the_next_provider_attempt_before_invocation() {
        let provider = Arc::new(MockProvider::new(
            descriptor("budgeted-fixture"),
            [success("first"), success("second")],
        ));
        let mut registry = ProviderRegistry::default();
        registry.register(provider.clone()).unwrap();
        let runner = ProviderRunner::new(registry, test_policy(1)).unwrap();
        let budget = TaskBudget::new(TaskExecutionLimits {
            max_provider_calls: 1,
            max_images: 2,
            ..TaskExecutionLimits::default()
        })
        .unwrap();
        let provider_id = ProviderId::new("budgeted-fixture").unwrap();
        runner
            .execute_with_budget(
                &provider_id,
                test_request(),
                &CancellationToken::default(),
                &budget,
            )
            .unwrap();
        let failure = runner
            .execute_with_budget(
                &provider_id,
                test_request(),
                &CancellationToken::default(),
                &budget,
            )
            .unwrap_err();
        assert_eq!(
            failure.failure.subject(),
            Some("UI_GENERATION_LIMIT_PROVIDER_CALLS")
        );
        assert_eq!(provider.call_count(), 1);
    }

    #[test]
    fn rate_wait_is_charged_before_the_next_task_attempt_starts() {
        let provider = Arc::new(MockProvider::new(
            descriptor("rate-budget-fixture"),
            [success("first"), success("second")],
        ));
        let mut registry = ProviderRegistry::default();
        registry.register(provider.clone()).unwrap();
        let mut policy = test_policy(1);
        policy.minimum_request_interval = Duration::from_millis(150);
        let runner = ProviderRunner::new(registry, policy).unwrap();
        let budget = TaskBudget::new(TaskExecutionLimits {
            max_provider_calls: 2,
            max_elapsed_ms: 100,
            max_images: 2,
            ..TaskExecutionLimits::default()
        })
        .unwrap();
        let provider_id = ProviderId::new("rate-budget-fixture").unwrap();
        runner
            .execute_with_budget(
                &provider_id,
                test_request(),
                &CancellationToken::default(),
                &budget,
            )
            .unwrap();
        let failure = runner
            .execute_with_budget(
                &provider_id,
                test_request(),
                &CancellationToken::default(),
                &budget,
            )
            .unwrap_err();
        assert_eq!(
            failure.failure.subject(),
            Some("UI_GENERATION_LIMIT_ELAPSED")
        );
        assert_eq!(provider.call_count(), 1);
    }

    #[cfg(feature = "full")]
    #[test]
    fn shared_runtime_governor_blocks_real_concurrent_provider_calls_and_releases_on_cancel() {
        let governor = shared_runtime_governor(TaskExecutionLimits::default(), 1);
        let provider_id = ProviderId::new("governed-concurrency").unwrap();
        let blocking_provider = Arc::new(MockProvider::new(
            descriptor("governed-concurrency"),
            [MockScenario::Timeout],
        ));
        let mut blocking_registry = ProviderRegistry::default();
        blocking_registry
            .register(blocking_provider.clone())
            .unwrap();
        let mut blocking_policy = test_policy(1);
        blocking_policy.attempt_timeout = Duration::from_secs(1);
        let blocking_runner = ProviderRunner::new_with_runtime_governor(
            blocking_registry,
            blocking_policy,
            governor.clone(),
        )
        .unwrap();
        let cancellation = CancellationToken::default();
        let thread_cancellation = cancellation.clone();
        let thread_provider_id = provider_id.clone();
        let blocking_thread = thread::spawn(move || {
            blocking_runner.execute(&thread_provider_id, test_request(), &thread_cancellation)
        });
        let deadline = Instant::now() + Duration::from_millis(300);
        while blocking_provider.call_count() == 0 {
            assert!(Instant::now() < deadline, "blocking provider did not start");
            thread::sleep(Duration::from_millis(1));
        }

        let succeeding_provider = Arc::new(MockProvider::new(
            descriptor("governed-concurrency"),
            [success("after-cancel")],
        ));
        let mut succeeding_registry = ProviderRegistry::default();
        succeeding_registry
            .register(succeeding_provider.clone())
            .unwrap();
        let succeeding_runner = ProviderRunner::new_with_runtime_governor(
            succeeding_registry,
            test_policy(1),
            governor.clone(),
        )
        .unwrap();
        let saturated_budget = TaskBudget::new(TaskExecutionLimits {
            max_provider_calls: 2,
            max_images: 2,
            ..TaskExecutionLimits::default()
        })
        .unwrap();
        let daily_before_saturation = governor.daily_snapshot();
        let saturated = succeeding_runner
            .execute_with_budget(
                &provider_id,
                test_request(),
                &CancellationToken::default(),
                &saturated_budget,
            )
            .unwrap_err();
        assert_eq!(
            saturated.failure.kind(),
            TaskFailureKind::ProviderRateLimited
        );
        assert_eq!(
            saturated.failure.subject(),
            Some("UI_GENERATION_QUEUE_PROVIDER_CONCURRENCY")
        );
        assert_eq!(succeeding_provider.call_count(), 0);
        assert_eq!(saturated_budget.snapshot().provider_calls, 0);
        assert_eq!(saturated_budget.snapshot().images, 0);
        assert_eq!(governor.daily_snapshot(), daily_before_saturation);

        cancellation.request();
        assert_eq!(
            blocking_thread.join().unwrap().unwrap_err().failure.kind(),
            TaskFailureKind::Cancelled
        );
        succeeding_runner
            .execute(&provider_id, test_request(), &CancellationToken::default())
            .unwrap();
        assert_eq!(succeeding_provider.call_count(), 1);
        assert!(governor.queue_snapshot().provider_active.is_empty());
    }

    #[cfg(feature = "full")]
    #[test]
    fn shared_runtime_governor_releases_provider_slot_after_failure() {
        let governor = shared_runtime_governor(TaskExecutionLimits::default(), 1);
        let provider_id = ProviderId::new("governed-failure").unwrap();
        let failing_provider = Arc::new(MockProvider::new(
            descriptor("governed-failure"),
            [MockScenario::ServiceUnavailable { request_id: None }],
        ));
        let mut failing_registry = ProviderRegistry::default();
        failing_registry.register(failing_provider).unwrap();
        let failing_runner = ProviderRunner::new_with_runtime_governor(
            failing_registry,
            test_policy(1),
            governor.clone(),
        )
        .unwrap();
        let failing_budget = TaskBudget::new(TaskExecutionLimits {
            max_provider_calls: 2,
            max_images: 2,
            ..TaskExecutionLimits::default()
        })
        .unwrap();
        assert_eq!(
            failing_runner
                .execute_with_budget(
                    &provider_id,
                    test_request(),
                    &CancellationToken::default(),
                    &failing_budget,
                )
                .unwrap_err()
                .failure
                .kind(),
            TaskFailureKind::ProviderServiceUnavailable
        );
        assert_eq!(failing_budget.snapshot().provider_calls, 1);
        assert_eq!(failing_budget.snapshot().images, 1);
        let failure_usage = governor.daily_snapshot().usage;
        assert_eq!(failure_usage.provider_calls, 1);
        assert_eq!(failure_usage.images, 1);
        assert_eq!(failure_usage.iterations, 1);
        assert!(governor.queue_snapshot().provider_active.is_empty());

        let succeeding_provider = Arc::new(MockProvider::new(
            descriptor("governed-failure"),
            [success("after-failure")],
        ));
        let mut succeeding_registry = ProviderRegistry::default();
        succeeding_registry
            .register(succeeding_provider.clone())
            .unwrap();
        ProviderRunner::new_with_runtime_governor(
            succeeding_registry,
            test_policy(1),
            governor.clone(),
        )
        .unwrap()
        .execute(&provider_id, test_request(), &CancellationToken::default())
        .unwrap();
        assert_eq!(succeeding_provider.call_count(), 1);
    }

    #[cfg(feature = "full")]
    #[test]
    fn timed_out_provider_attempt_commits_budgets_and_releases_its_slot() {
        let governor = shared_runtime_governor(TaskExecutionLimits::default(), 1);
        let provider = Arc::new(MockProvider::new(
            descriptor("governed-timeout-accounting"),
            [MockScenario::Timeout],
        ));
        let mut registry = ProviderRegistry::default();
        registry.register(provider.clone()).unwrap();
        let timeout_budget = TaskBudget::new(TaskExecutionLimits {
            max_provider_calls: 2,
            max_images: 2,
            ..TaskExecutionLimits::default()
        })
        .unwrap();
        let failure = ProviderRunner::new_with_runtime_governor(
            registry,
            timeout_test_policy(1),
            governor.clone(),
        )
        .unwrap()
        .execute_with_budget(
            &ProviderId::new("governed-timeout-accounting").unwrap(),
            test_request(),
            &CancellationToken::default(),
            &timeout_budget,
        )
        .unwrap_err();
        assert_eq!(failure.failure.kind(), TaskFailureKind::ProviderTimeout);
        assert_eq!(provider.call_count(), 1);
        assert_eq!(timeout_budget.snapshot().provider_calls, 1);
        assert_eq!(timeout_budget.snapshot().images, 1);
        let timeout_usage = governor.daily_snapshot().usage;
        assert_eq!(timeout_usage.provider_calls, 1);
        assert_eq!(timeout_usage.images, 1);
        assert_eq!(timeout_usage.iterations, 1);
        assert!(timeout_usage.elapsed_ms > 0);
        assert!(governor.queue_snapshot().provider_active.is_empty());
    }

    #[cfg(feature = "full")]
    #[test]
    fn daily_call_and_restored_snapshot_limits_stop_real_provider_execution() {
        let limits = TaskExecutionLimits {
            max_provider_calls: 1,
            max_images: 2,
            max_iterations: 4,
            ..TaskExecutionLimits::default()
        };
        let governor = shared_runtime_governor(limits.clone(), 1);
        let provider_id = ProviderId::new("governed-daily-calls").unwrap();
        let first_provider = Arc::new(MockProvider::new(
            descriptor("governed-daily-calls"),
            [success("daily-first")],
        ));
        let mut first_registry = ProviderRegistry::default();
        first_registry.register(first_provider.clone()).unwrap();
        ProviderRunner::new_with_runtime_governor(first_registry, test_policy(1), governor.clone())
            .unwrap()
            .execute(&provider_id, test_request(), &CancellationToken::default())
            .unwrap();
        assert_eq!(first_provider.call_count(), 1);

        let blocked_provider = Arc::new(MockProvider::new(
            descriptor("governed-daily-calls"),
            [success("daily-blocked")],
        ));
        let mut blocked_registry = ProviderRegistry::default();
        blocked_registry.register(blocked_provider.clone()).unwrap();
        let blocked = ProviderRunner::new_with_runtime_governor(
            blocked_registry,
            test_policy(1),
            governor.clone(),
        )
        .unwrap()
        .execute(&provider_id, test_request(), &CancellationToken::default())
        .unwrap_err();
        assert_eq!(blocked.failure.kind(), TaskFailureKind::ProviderRateLimited);
        assert_eq!(
            blocked.failure.subject(),
            Some("UI_GENERATION_DAILY_PROVIDER_CALLS")
        );
        assert_eq!(blocked_provider.call_count(), 0);

        let restored_daily = DailyBudget::from_snapshot(limits, governor.daily_snapshot()).unwrap();
        let restored_governor = Arc::new(
            ProviderRuntimeGovernor::new(TaskQueuePolicy::default(), restored_daily).unwrap(),
        );
        let restored_provider = Arc::new(MockProvider::new(
            descriptor("governed-daily-calls"),
            [success("restored-blocked")],
        ));
        let mut restored_registry = ProviderRegistry::default();
        restored_registry
            .register(restored_provider.clone())
            .unwrap();
        let restored = ProviderRunner::new_with_runtime_governor(
            restored_registry,
            test_policy(1),
            restored_governor,
        )
        .unwrap()
        .execute(&provider_id, test_request(), &CancellationToken::default())
        .unwrap_err();
        assert_eq!(
            restored.failure.subject(),
            Some("UI_GENERATION_DAILY_PROVIDER_CALLS")
        );
        assert_eq!(restored_provider.call_count(), 0);
    }

    #[cfg(feature = "full")]
    #[test]
    fn daily_call_and_image_preflight_failures_do_not_consume_iterations() {
        let call_limits = TaskExecutionLimits {
            max_provider_calls: 1,
            max_images: 4,
            max_iterations: 4,
            ..TaskExecutionLimits::default()
        };
        let call_governor = runtime_governor_from_snapshot(
            call_limits.clone(),
            TaskUsageSnapshot {
                provider_calls: 1,
                images: 1,
                ..TaskUsageSnapshot::default()
            },
        );
        let call_provider = Arc::new(MockProvider::new(
            descriptor("governed-daily-preflight-call"),
            [success("must-not-call")],
        ));
        let mut call_registry = ProviderRegistry::default();
        call_registry.register(call_provider.clone()).unwrap();
        let call_budget = TaskBudget::new(TaskExecutionLimits {
            max_provider_calls: 2,
            max_images: 2,
            ..TaskExecutionLimits::default()
        })
        .unwrap();
        let call_failure = ProviderRunner::new_with_runtime_governor(
            call_registry,
            test_policy(1),
            call_governor.clone(),
        )
        .unwrap()
        .execute_with_budget(
            &ProviderId::new("governed-daily-preflight-call").unwrap(),
            test_request(),
            &CancellationToken::default(),
            &call_budget,
        )
        .unwrap_err();
        assert_eq!(
            call_failure.failure.subject(),
            Some("UI_GENERATION_DAILY_PROVIDER_CALLS")
        );
        assert_eq!(call_provider.call_count(), 0);
        assert_eq!(call_budget.snapshot().provider_calls, 0);
        assert_eq!(call_budget.snapshot().images, 0);
        assert_eq!(call_governor.daily_snapshot().usage.iterations, 0);
        assert_eq!(call_governor.daily_snapshot().usage.provider_calls, 1);
        assert!(call_governor.queue_snapshot().provider_active.is_empty());

        let image_limits = TaskExecutionLimits {
            max_provider_calls: 4,
            max_images: 1,
            max_iterations: 4,
            ..TaskExecutionLimits::default()
        };
        let image_governor = runtime_governor_from_snapshot(
            image_limits.clone(),
            TaskUsageSnapshot {
                images: 1,
                ..TaskUsageSnapshot::default()
            },
        );
        let image_provider = Arc::new(MockProvider::new(
            descriptor("governed-daily-preflight-image"),
            [success("must-not-call")],
        ));
        let mut image_registry = ProviderRegistry::default();
        image_registry.register(image_provider.clone()).unwrap();
        let image_budget = TaskBudget::new(TaskExecutionLimits {
            max_provider_calls: 2,
            max_images: 2,
            ..TaskExecutionLimits::default()
        })
        .unwrap();
        let image_failure = ProviderRunner::new_with_runtime_governor(
            image_registry,
            test_policy(1),
            image_governor.clone(),
        )
        .unwrap()
        .execute_with_budget(
            &ProviderId::new("governed-daily-preflight-image").unwrap(),
            test_request(),
            &CancellationToken::default(),
            &image_budget,
        )
        .unwrap_err();
        assert_eq!(
            image_failure.failure.subject(),
            Some("UI_GENERATION_DAILY_IMAGES")
        );
        assert_eq!(image_provider.call_count(), 0);
        assert_eq!(image_budget.snapshot().provider_calls, 0);
        assert_eq!(image_budget.snapshot().images, 0);
        assert_eq!(image_governor.daily_snapshot().usage.iterations, 0);
        assert_eq!(image_governor.daily_snapshot().usage.images, 1);
        assert!(image_governor.queue_snapshot().provider_active.is_empty());
    }

    #[cfg(feature = "full")]
    #[test]
    fn local_budget_preflight_failure_does_not_consume_daily_quota_or_slot() {
        let governor = shared_runtime_governor(TaskExecutionLimits::default(), 1);
        let provider = Arc::new(MockProvider::new(
            descriptor("governed-local-preflight"),
            [success("must-not-call")],
        ));
        let mut registry = ProviderRegistry::default();
        registry.register(provider.clone()).unwrap();
        let local_budget = TaskBudget::new(TaskExecutionLimits {
            max_provider_calls: 1,
            max_images: 2,
            ..TaskExecutionLimits::default()
        })
        .unwrap();
        local_budget.reserve_provider_attempt(1).unwrap();
        let local_before = local_budget.snapshot();
        let failure =
            ProviderRunner::new_with_runtime_governor(registry, test_policy(1), governor.clone())
                .unwrap()
                .execute_with_budget(
                    &ProviderId::new("governed-local-preflight").unwrap(),
                    test_request(),
                    &CancellationToken::default(),
                    &local_budget,
                )
                .unwrap_err();
        assert_eq!(
            failure.failure.subject(),
            Some("UI_GENERATION_LIMIT_PROVIDER_CALLS")
        );
        assert_eq!(provider.call_count(), 0);
        assert_eq!(
            local_budget.snapshot().provider_calls,
            local_before.provider_calls
        );
        assert_eq!(local_budget.snapshot().images, local_before.images);
        assert_eq!(
            governor.daily_snapshot().usage,
            TaskUsageSnapshot::default()
        );
        assert!(governor.queue_snapshot().provider_active.is_empty());
    }

    #[cfg(feature = "full")]
    #[test]
    fn cancelled_preflight_does_not_consume_local_or_daily_quota() {
        let governor = shared_runtime_governor(TaskExecutionLimits::default(), 1);
        let provider = Arc::new(MockProvider::new(
            descriptor("governed-cancelled-preflight"),
            [success("must-not-call")],
        ));
        let mut registry = ProviderRegistry::default();
        registry.register(provider.clone()).unwrap();
        let local_budget = TaskBudget::new(TaskExecutionLimits {
            max_provider_calls: 2,
            max_images: 2,
            ..TaskExecutionLimits::default()
        })
        .unwrap();
        let cancellation = CancellationToken::default();
        cancellation.request();
        let failure =
            ProviderRunner::new_with_runtime_governor(registry, test_policy(1), governor.clone())
                .unwrap()
                .execute_with_budget(
                    &ProviderId::new("governed-cancelled-preflight").unwrap(),
                    test_request(),
                    &cancellation,
                    &local_budget,
                )
                .unwrap_err();
        assert_eq!(failure.failure.kind(), TaskFailureKind::Cancelled);
        assert_eq!(provider.call_count(), 0);
        assert_eq!(local_budget.snapshot().provider_calls, 0);
        assert_eq!(local_budget.snapshot().images, 0);
        assert_eq!(
            governor.daily_snapshot().usage,
            TaskUsageSnapshot::default()
        );
        assert!(governor.queue_snapshot().provider_active.is_empty());
    }

    #[cfg(feature = "full")]
    #[test]
    fn daily_usage_and_iteration_limits_stop_real_provider_execution() {
        let usage_limits = TaskExecutionLimits {
            max_provider_calls: 4,
            max_images: 4,
            max_iterations: 4,
            max_input_units: 16,
            max_output_units: 4,
            ..TaskExecutionLimits::default()
        };
        let usage_governor = shared_runtime_governor(usage_limits, 1);
        let fixture_provider =
            Arc::new(FixtureProvider::load(&fixture_provider_path("valid.json")).unwrap());
        let fixture_id = fixture_provider.descriptor().id.clone();
        let mut fixture_registry = ProviderRegistry::default();
        fixture_registry.register(fixture_provider).unwrap();
        let usage_failure = ProviderRunner::new_with_runtime_governor(
            fixture_registry,
            test_policy(1),
            usage_governor,
        )
        .unwrap()
        .execute(&fixture_id, test_request(), &CancellationToken::default())
        .unwrap_err();
        assert_eq!(usage_failure.trace.attempts.len(), 1);
        assert_eq!(
            usage_failure.failure.subject(),
            Some("UI_GENERATION_DAILY_OUTPUT_UNITS")
        );

        let iteration_governor = shared_runtime_governor(
            TaskExecutionLimits {
                max_provider_calls: 4,
                max_images: 4,
                max_iterations: 1,
                ..TaskExecutionLimits::default()
            },
            1,
        );
        let iteration_provider = Arc::new(MockProvider::new(
            descriptor("governed-daily-iterations"),
            [success("iteration-first"), success("iteration-blocked")],
        ));
        let mut iteration_registry = ProviderRegistry::default();
        iteration_registry
            .register(iteration_provider.clone())
            .unwrap();
        let iteration_runner = ProviderRunner::new_with_runtime_governor(
            iteration_registry,
            test_policy(1),
            iteration_governor,
        )
        .unwrap();
        let iteration_id = ProviderId::new("governed-daily-iterations").unwrap();
        iteration_runner
            .execute(&iteration_id, test_request(), &CancellationToken::default())
            .unwrap();
        let iteration_failure = iteration_runner
            .execute(&iteration_id, test_request(), &CancellationToken::default())
            .unwrap_err();
        assert_eq!(
            iteration_failure.failure.subject(),
            Some("UI_GENERATION_DAILY_ITERATIONS")
        );
        assert_eq!(iteration_provider.call_count(), 1);
    }

    #[cfg(feature = "full")]
    #[test]
    fn offline_runtime_governance_stress_fixture_uses_the_real_execute_path() {
        let report = run_offline_runtime_governance_stress_fixture().unwrap();
        assert!(report.first_provider_call_succeeded);
        assert!(report.daily_budget_blocked_second_call);
        assert_eq!(report.provider_calls_recorded, 1);
        assert_eq!(report.final_status, "passed");
    }
}
