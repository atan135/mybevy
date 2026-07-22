//! Closed-loop operational controls for cache reuse, bounded scheduling, budgets, and artifacts.
//!
//! The types in this module are deliberately provider-agnostic. They are safe to exercise with
//! fixture work and make no network calls, read no credentials, or serialize provider requests.

use crate::{
    lifecycle::{CancellationToken, TaskFailure, TaskFailureKind},
    provider_budget::{TaskBudget, TaskExecutionLimits, TaskUsageSnapshot},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

pub const OPERATIONS_PROTOCOL_VERSION: u32 = 1;
const ARTIFACT_ROOT_MARKER: &str = ".ui-generation-artifact-root";
const ARTIFACT_ROOT_MARKER_CONTENT: &[u8] = b"ui-generation-artifact-root-v1\n";
const MAX_QUEUE_CAPACITY: usize = 1_024;
const MAX_ARTIFACT_RETENTION_MS: u64 = 365 * 24 * 60 * 60 * 1_000;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheStage {
    Preprocess,
    VisualAnalysis,
    UiDocumentGeneration,
    Screenshot,
    Comparison,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CacheViewport {
    pub width: u32,
    pub height: u32,
    pub device_scale_milli: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CacheDimensions {
    pub input_sha256: String,
    pub schema_id: String,
    pub schema_version: u32,
    pub prompt_revision: String,
    pub model_revision: String,
    pub theme_revision: String,
    pub font_revision: String,
    pub viewport: CacheViewport,
    pub algorithm_revision: String,
}

impl CacheDimensions {
    pub fn validate(&self) -> Result<(), TaskFailure> {
        if !is_sha256(&self.input_sha256)
            || !safe_label(&self.schema_id, 128)
            || self.schema_version == 0
            || !safe_label(&self.prompt_revision, 128)
            || !safe_label(&self.model_revision, 128)
            || !safe_label(&self.theme_revision, 128)
            || !safe_label(&self.font_revision, 128)
            || !safe_label(&self.algorithm_revision, 128)
            || self.viewport.width == 0
            || self.viewport.height == 0
            || self.viewport.width > 16_384
            || self.viewport.height > 16_384
            || self.viewport.device_scale_milli == 0
            || self.viewport.device_scale_milli > 16_000
        {
            return Err(TaskFailure::new(
                TaskFailureKind::CacheIncompatible,
                "cache dimensions require every schema, prompt, model, theme, font, viewport, algorithm, and input-hash dimension",
                None,
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StageCacheKey {
    pub protocol_version: u32,
    pub stage: CacheStage,
    pub dimensions: CacheDimensions,
}

impl StageCacheKey {
    pub fn new(stage: CacheStage, dimensions: CacheDimensions) -> Result<Self, TaskFailure> {
        dimensions.validate()?;
        Ok(Self {
            protocol_version: OPERATIONS_PROTOCOL_VERSION,
            stage,
            dimensions,
        })
    }

    pub fn digest(&self) -> Result<String, TaskFailure> {
        self.validate()?;
        let bytes = serde_json::to_vec(self)
            .map_err(|_| TaskFailure::invalid("cache key cannot be serialized"))?;
        Ok(hex_sha256(&bytes))
    }

    pub fn reuse_decision(&self, candidate: &Self) -> Result<CacheReuseDecision, TaskFailure> {
        self.validate()?;
        candidate.validate()?;
        let mut invalidation = Vec::new();
        if self.protocol_version != candidate.protocol_version {
            invalidation.push(CacheInvalidation::Protocol);
        }
        if self.stage != candidate.stage {
            invalidation.push(CacheInvalidation::Stage);
        }
        let expected = &self.dimensions;
        let actual = &candidate.dimensions;
        if expected.input_sha256 != actual.input_sha256 {
            invalidation.push(CacheInvalidation::InputHash);
        }
        if expected.schema_id != actual.schema_id
            || expected.schema_version != actual.schema_version
        {
            invalidation.push(CacheInvalidation::Schema);
        }
        if expected.prompt_revision != actual.prompt_revision {
            invalidation.push(CacheInvalidation::Prompt);
        }
        if expected.model_revision != actual.model_revision {
            invalidation.push(CacheInvalidation::Model);
        }
        if expected.theme_revision != actual.theme_revision {
            invalidation.push(CacheInvalidation::Theme);
        }
        if expected.font_revision != actual.font_revision {
            invalidation.push(CacheInvalidation::Font);
        }
        if expected.viewport != actual.viewport {
            invalidation.push(CacheInvalidation::Viewport);
        }
        if expected.algorithm_revision != actual.algorithm_revision {
            invalidation.push(CacheInvalidation::Algorithm);
        }
        if invalidation.is_empty() {
            Ok(CacheReuseDecision::Hit {
                cache_key: self.digest()?,
            })
        } else {
            Ok(CacheReuseDecision::Miss { invalidation })
        }
    }

    fn validate(&self) -> Result<(), TaskFailure> {
        if self.protocol_version != OPERATIONS_PROTOCOL_VERSION {
            return Err(TaskFailure::new(
                TaskFailureKind::CacheIncompatible,
                "cache key protocol is unsupported",
                None,
            ));
        }
        self.dimensions.validate()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheInvalidation {
    Protocol,
    Stage,
    InputHash,
    Schema,
    Prompt,
    Model,
    Theme,
    Font,
    Viewport,
    Algorithm,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "result")]
pub enum CacheReuseDecision {
    Hit {
        cache_key: String,
    },
    Miss {
        invalidation: Vec<CacheInvalidation>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskQueuePolicy {
    pub max_queued_or_running: usize,
    pub default_provider_concurrency: u32,
    pub provider_concurrency: BTreeMap<String, u32>,
}

impl Default for TaskQueuePolicy {
    fn default() -> Self {
        Self {
            max_queued_or_running: 16,
            default_provider_concurrency: 1,
            provider_concurrency: BTreeMap::new(),
        }
    }
}

impl TaskQueuePolicy {
    pub fn validate(&self) -> Result<(), TaskFailure> {
        if self.max_queued_or_running == 0
            || self.max_queued_or_running > MAX_QUEUE_CAPACITY
            || self.default_provider_concurrency == 0
        {
            return Err(TaskFailure::invalid(
                "task queue requires a bounded positive capacity and provider concurrency",
            ));
        }
        if self
            .provider_concurrency
            .iter()
            .any(|(provider, limit)| !safe_label(provider, 128) || *limit == 0)
        {
            return Err(TaskFailure::invalid(
                "provider concurrency keys and limits are invalid",
            ));
        }
        Ok(())
    }

    fn provider_limit(&self, provider_id: &str) -> u32 {
        self.provider_concurrency
            .get(provider_id)
            .copied()
            .unwrap_or(self.default_provider_concurrency)
    }
}

#[derive(Clone, Debug)]
pub struct QueuedTask {
    pub run_id: String,
    pub iteration: u32,
    pub task_id: String,
    pub provider_id: String,
    pub cancellation: CancellationToken,
}

impl QueuedTask {
    pub fn validate(&self) -> Result<(), TaskFailure> {
        if !safe_label(&self.run_id, 128)
            || self.iteration == 0
            || !safe_label(&self.task_id, 128)
            || !safe_label(&self.provider_id, 128)
        {
            return Err(TaskFailure::invalid(
                "queued task needs safe run, iteration, task, and provider correlation IDs",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QueueTicket {
    pub queue_id: u64,
    pub task_id: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueTaskStatus {
    Queued,
    Running,
    Cancelling,
    Cancelled,
    Finished,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QueueSnapshot {
    pub queued: usize,
    pub running: usize,
    pub cancelled: usize,
    pub finished: usize,
    pub provider_active: BTreeMap<String, u32>,
    pub peak_running: usize,
}

struct ManagedTask {
    task: QueuedTask,
    status: QueueTaskStatus,
}

struct QueueState {
    next_queue_id: u64,
    tasks: BTreeMap<u64, ManagedTask>,
    provider_active: BTreeMap<String, u32>,
    peak_running: usize,
}

impl Default for QueueState {
    fn default() -> Self {
        Self {
            next_queue_id: 1,
            tasks: BTreeMap::new(),
            provider_active: BTreeMap::new(),
            peak_running: 0,
        }
    }
}

#[derive(Clone)]
pub struct BoundedTaskQueue {
    policy: TaskQueuePolicy,
    state: Arc<Mutex<QueueState>>,
}

impl BoundedTaskQueue {
    pub fn new(policy: TaskQueuePolicy) -> Result<Self, TaskFailure> {
        policy.validate()?;
        Ok(Self {
            policy,
            state: Arc::new(Mutex::new(QueueState::default())),
        })
    }

    pub fn enqueue(&self, task: QueuedTask) -> Result<QueueTicket, TaskFailure> {
        task.validate()?;
        let mut state = self.state.lock().expect("task queue mutex poisoned");
        // Tickets are only useful while work is live. Pruning terminal records here keeps the
        // scheduler itself bounded during long-lived processes without releasing active slots.
        state.tasks.retain(|_, managed| {
            !matches!(
                managed.status,
                QueueTaskStatus::Cancelled | QueueTaskStatus::Finished
            )
        });
        let live_tasks = state
            .tasks
            .values()
            .filter(|managed| {
                matches!(
                    managed.status,
                    QueueTaskStatus::Queued
                        | QueueTaskStatus::Running
                        | QueueTaskStatus::Cancelling
                )
            })
            .count();
        if live_tasks >= self.policy.max_queued_or_running {
            return Err(TaskFailure::new(
                TaskFailureKind::ProviderRateLimited,
                "bounded task queue is full",
                Some("UI_GENERATION_QUEUE_FULL".to_owned()),
            ));
        }
        if state
            .tasks
            .values()
            .any(|managed| managed.task.task_id == task.task_id)
        {
            return Err(TaskFailure::invalid(
                "task queue rejects a duplicate task ID until its history is pruned",
            ));
        }
        let queue_id = state.next_queue_id;
        state.next_queue_id = state
            .next_queue_id
            .checked_add(1)
            .ok_or_else(|| TaskFailure::invalid("task queue identifier overflowed"))?;
        let task_id = task.task_id.clone();
        state.tasks.insert(
            queue_id,
            ManagedTask {
                task,
                status: QueueTaskStatus::Queued,
            },
        );
        Ok(QueueTicket { queue_id, task_id })
    }

    /// Claims the next runnable task. A provider at its concurrency limit leaves its task queued
    /// while work for another provider can proceed.
    pub fn try_start_next(&self) -> Option<QueueLease> {
        let mut state = self.state.lock().expect("task queue mutex poisoned");
        let candidate = state.tasks.iter().find_map(|(queue_id, managed)| {
            if managed.status != QueueTaskStatus::Queued {
                return None;
            }
            if managed.task.cancellation.is_requested() {
                return Some((*queue_id, false));
            }
            let active = state
                .provider_active
                .get(&managed.task.provider_id)
                .copied()
                .unwrap_or(0);
            let allowed = self.policy.provider_limit(&managed.task.provider_id);
            (active < allowed).then_some((*queue_id, true))
        });
        let (queue_id, runnable) = candidate?;
        if !runnable {
            if let Some(managed) = state.tasks.get_mut(&queue_id) {
                managed.status = QueueTaskStatus::Cancelled;
            }
            drop(state);
            return self.try_start_next();
        }
        let ticket = QueueTicket {
            queue_id,
            task_id: state.tasks.get(&queue_id)?.task.task_id.clone(),
        };
        self.start_ticket_locked(&mut state, &ticket)
    }

    /// Claims only the specified task. Runtime callers use this after enqueueing so a competing
    /// caller cannot accidentally receive another run's queued work.
    pub fn try_start_ticket(&self, ticket: &QueueTicket) -> Option<QueueLease> {
        let mut state = self.state.lock().expect("task queue mutex poisoned");
        self.start_ticket_locked(&mut state, ticket)
    }

    fn start_ticket_locked(
        &self,
        state: &mut QueueState,
        ticket: &QueueTicket,
    ) -> Option<QueueLease> {
        let managed = state.tasks.get(&ticket.queue_id)?;
        if managed.task.task_id != ticket.task_id || managed.status != QueueTaskStatus::Queued {
            return None;
        }
        if managed.task.cancellation.is_requested() {
            state.tasks.get_mut(&ticket.queue_id)?.status = QueueTaskStatus::Cancelled;
            return None;
        }
        let active = state
            .provider_active
            .get(&managed.task.provider_id)
            .copied()
            .unwrap_or(0);
        if active >= self.policy.provider_limit(&managed.task.provider_id) {
            return None;
        }
        let managed = state.tasks.get_mut(&ticket.queue_id)?;
        managed.status = QueueTaskStatus::Running;
        let provider_id = managed.task.provider_id.clone();
        let task_id = managed.task.task_id.clone();
        let run_id = managed.task.run_id.clone();
        let iteration = managed.task.iteration;
        let cancellation = managed.task.cancellation.clone();
        let active = state
            .provider_active
            .entry(provider_id.clone())
            .or_default();
        *active += 1;
        let running = state
            .tasks
            .values()
            .filter(|managed| managed.status == QueueTaskStatus::Running)
            .count();
        state.peak_running = state.peak_running.max(running);
        Some(QueueLease {
            queue_id: ticket.queue_id,
            run_id,
            iteration,
            task_id,
            provider_id,
            cancellation,
            state: Arc::clone(&self.state),
            released: false,
        })
    }

    pub fn cancel(&self, ticket: &QueueTicket) -> bool {
        let mut state = self.state.lock().expect("task queue mutex poisoned");
        let Some(managed) = state.tasks.get_mut(&ticket.queue_id) else {
            return false;
        };
        if managed.task.task_id != ticket.task_id
            || matches!(
                managed.status,
                QueueTaskStatus::Cancelled | QueueTaskStatus::Finished
            )
        {
            return false;
        }
        managed.task.cancellation.request();
        managed.status = if managed.status == QueueTaskStatus::Queued {
            QueueTaskStatus::Cancelled
        } else {
            QueueTaskStatus::Cancelling
        };
        true
    }

    pub fn snapshot(&self) -> QueueSnapshot {
        let state = self.state.lock().expect("task queue mutex poisoned");
        let mut snapshot = QueueSnapshot {
            queued: 0,
            running: 0,
            cancelled: 0,
            finished: 0,
            provider_active: state.provider_active.clone(),
            peak_running: state.peak_running,
        };
        for managed in state.tasks.values() {
            match managed.status {
                QueueTaskStatus::Queued => snapshot.queued += 1,
                QueueTaskStatus::Running | QueueTaskStatus::Cancelling => snapshot.running += 1,
                QueueTaskStatus::Cancelled => snapshot.cancelled += 1,
                QueueTaskStatus::Finished => snapshot.finished += 1,
            }
        }
        snapshot
    }
}

pub struct QueueLease {
    queue_id: u64,
    pub run_id: String,
    pub iteration: u32,
    pub task_id: String,
    pub provider_id: String,
    pub cancellation: CancellationToken,
    state: Arc<Mutex<QueueState>>,
    released: bool,
}

impl QueueLease {
    pub fn finish(mut self) {
        self.release(QueueTaskStatus::Finished);
    }

    fn release(&mut self, requested_status: QueueTaskStatus) {
        if self.released {
            return;
        }
        let mut state = self.state.lock().expect("task queue mutex poisoned");
        let cancellation_requested = state
            .tasks
            .get(&self.queue_id)
            .map(|managed| managed.task.cancellation.is_requested());
        if let Some(cancellation_requested) = cancellation_requested {
            let provider_is_idle = state
                .provider_active
                .get_mut(&self.provider_id)
                .is_some_and(|provider_active| {
                    *provider_active = provider_active.saturating_sub(1);
                    *provider_active == 0
                });
            if provider_is_idle {
                state.provider_active.remove(&self.provider_id);
            }
            if let Some(managed) = state.tasks.get_mut(&self.queue_id) {
                managed.status = if cancellation_requested {
                    QueueTaskStatus::Cancelled
                } else {
                    requested_status
                };
            }
        }
        self.released = true;
    }
}

impl Drop for QueueLease {
    fn drop(&mut self) {
        self.release(QueueTaskStatus::Finished);
    }
}

/// An in-memory, restart-serializable daily quota. Hosts persist `snapshot` between process
/// launches and restore it with `from_snapshot`; counters are checked before each reservation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DailyBudgetSnapshot {
    pub day: String,
    pub usage: TaskUsageSnapshot,
}

#[derive(Clone)]
pub struct DailyBudget {
    day: String,
    limits: TaskExecutionLimits,
    usage: Arc<Mutex<TaskUsageSnapshot>>,
}

/// An uncommitted daily provider-attempt reservation. Calls, images, and iterations are reserved
/// together under one mutex and are all restored if a later local or queue preflight fails.
pub struct DailyAttemptReservation {
    usage: Arc<Mutex<TaskUsageSnapshot>>,
    image_count: usize,
    committed: bool,
}

impl DailyAttemptReservation {
    pub fn commit(mut self) {
        self.committed = true;
    }
}

impl Drop for DailyAttemptReservation {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        let mut usage = self.usage.lock().expect("daily budget mutex poisoned");
        usage.provider_calls = usage.provider_calls.saturating_sub(1);
        usage.images = usage.images.saturating_sub(self.image_count);
        usage.iterations = usage.iterations.saturating_sub(1);
    }
}

impl DailyBudget {
    pub fn new(day: impl Into<String>, limits: TaskExecutionLimits) -> Result<Self, TaskFailure> {
        limits.validate()?;
        let day = day.into();
        if !is_day(&day) {
            return Err(TaskFailure::invalid(
                "daily budget day must use the YYYY-MM-DD UTC format",
            ));
        }
        Ok(Self {
            day,
            limits,
            usage: Arc::new(Mutex::new(TaskUsageSnapshot::default())),
        })
    }

    pub fn from_snapshot(
        limits: TaskExecutionLimits,
        snapshot: DailyBudgetSnapshot,
    ) -> Result<Self, TaskFailure> {
        let budget = Self::new(snapshot.day, limits)?;
        validate_usage(&budget.limits, &snapshot.usage)?;
        *budget.usage.lock().expect("daily budget mutex poisoned") = snapshot.usage;
        Ok(budget)
    }

    pub fn reserve_provider_attempt(&self, image_count: usize) -> Result<(), TaskFailure> {
        self.mutate(|next| {
            next.provider_calls = next.provider_calls.checked_add(1).ok_or_else(|| {
                budget_failure(
                    "UI_GENERATION_DAILY_PROVIDER_CALLS",
                    "daily provider calls overflowed",
                )
            })?;
            next.images = next.images.checked_add(image_count).ok_or_else(|| {
                budget_failure("UI_GENERATION_DAILY_IMAGES", "daily image count overflowed")
            })?;
            Ok(())
        })
    }

    /// Reserves the daily iteration, model-call, and image budget as one transaction. The caller
    /// must commit the returned guard only once all remaining preflight checks can start I/O.
    pub fn reserve_external_attempt(
        &self,
        image_count: usize,
    ) -> Result<DailyAttemptReservation, TaskFailure> {
        let mut usage = self.usage.lock().expect("daily budget mutex poisoned");
        let mut next = usage.clone();
        next.iterations = next.iterations.checked_add(1).ok_or_else(|| {
            budget_failure(
                "UI_GENERATION_DAILY_ITERATIONS",
                "daily iteration count overflowed",
            )
        })?;
        next.provider_calls = next.provider_calls.checked_add(1).ok_or_else(|| {
            budget_failure(
                "UI_GENERATION_DAILY_PROVIDER_CALLS",
                "daily provider calls overflowed",
            )
        })?;
        next.images = next.images.checked_add(image_count).ok_or_else(|| {
            budget_failure("UI_GENERATION_DAILY_IMAGES", "daily image count overflowed")
        })?;
        validate_usage(&self.limits, &next)?;
        *usage = next;
        Ok(DailyAttemptReservation {
            usage: Arc::clone(&self.usage),
            image_count,
            committed: false,
        })
    }

    pub fn record_provider_usage(
        &self,
        input_units: u64,
        output_units: u64,
    ) -> Result<(), TaskFailure> {
        let input_cost = units_cost(input_units, self.limits.input_cost_microunits_per_1k)?;
        let output_cost = units_cost(output_units, self.limits.output_cost_microunits_per_1k)?;
        let cost = input_cost.checked_add(output_cost).ok_or_else(|| {
            budget_failure(
                "UI_GENERATION_DAILY_COST",
                "daily estimated cost overflowed",
            )
        })?;
        self.mutate(|next| {
            next.input_units = next.input_units.checked_add(input_units).ok_or_else(|| {
                budget_failure(
                    "UI_GENERATION_DAILY_INPUT_UNITS",
                    "daily input units overflowed",
                )
            })?;
            next.output_units = next.output_units.checked_add(output_units).ok_or_else(|| {
                budget_failure(
                    "UI_GENERATION_DAILY_OUTPUT_UNITS",
                    "daily output units overflowed",
                )
            })?;
            next.estimated_cost_microunits = next
                .estimated_cost_microunits
                .checked_add(cost)
                .ok_or_else(|| {
                    budget_failure(
                        "UI_GENERATION_DAILY_COST",
                        "daily estimated cost overflowed",
                    )
                })?;
            Ok(())
        })
    }

    pub fn reserve_iteration(&self) -> Result<(), TaskFailure> {
        self.mutate(|next| {
            next.iterations = next.iterations.checked_add(1).ok_or_else(|| {
                budget_failure(
                    "UI_GENERATION_DAILY_ITERATIONS",
                    "daily iteration count overflowed",
                )
            })?;
            Ok(())
        })
    }

    pub fn record_elapsed(&self, elapsed_ms: u64) -> Result<(), TaskFailure> {
        self.mutate(|next| {
            next.elapsed_ms = next.elapsed_ms.checked_add(elapsed_ms).ok_or_else(|| {
                budget_failure(
                    "UI_GENERATION_DAILY_ELAPSED",
                    "daily elapsed time overflowed",
                )
            })?;
            Ok(())
        })
    }

    pub fn snapshot(&self) -> DailyBudgetSnapshot {
        DailyBudgetSnapshot {
            day: self.day.clone(),
            usage: self
                .usage
                .lock()
                .expect("daily budget mutex poisoned")
                .clone(),
        }
    }

    fn mutate(
        &self,
        mutate: impl FnOnce(&mut TaskUsageSnapshot) -> Result<(), TaskFailure>,
    ) -> Result<(), TaskFailure> {
        let mut usage = self.usage.lock().expect("daily budget mutex poisoned");
        let mut next = usage.clone();
        mutate(&mut next)?;
        validate_usage(&self.limits, &next)?;
        *usage = next;
        Ok(())
    }
}

impl DailyBudgetSnapshot {
    pub fn to_json_bytes(&self) -> Result<Vec<u8>, TaskFailure> {
        serde_json::to_vec_pretty(self)
            .map_err(|_| TaskFailure::invalid("daily budget snapshot cannot be serialized"))
    }

    pub fn parse_json(bytes: &[u8]) -> Result<Self, TaskFailure> {
        if bytes.len() > 64 * 1024 {
            return Err(TaskFailure::invalid(
                "daily budget snapshot exceeds its byte budget",
            ));
        }
        let snapshot: Self = serde_json::from_slice(bytes)
            .map_err(|_| TaskFailure::invalid("daily budget snapshot is malformed"))?;
        if !is_day(&snapshot.day) {
            return Err(TaskFailure::invalid("daily budget snapshot day is invalid"));
        }
        Ok(snapshot)
    }
}

/// Shared, explicit runtime governance for real provider attempts. Hosts construct one instance
/// and inject the same `Arc` into every runner that must share daily quota and provider slots.
#[derive(Clone)]
pub struct ProviderRuntimeGovernor {
    queue: BoundedTaskQueue,
    daily_budget: DailyBudget,
    next_task_id: Arc<AtomicU64>,
}

impl ProviderRuntimeGovernor {
    pub fn new(
        queue_policy: TaskQueuePolicy,
        daily_budget: DailyBudget,
    ) -> Result<Self, TaskFailure> {
        Ok(Self {
            queue: BoundedTaskQueue::new(queue_policy)?,
            daily_budget,
            next_task_id: Arc::new(AtomicU64::new(1)),
        })
    }

    pub fn queue_snapshot(&self) -> QueueSnapshot {
        self.queue.snapshot()
    }

    pub fn daily_snapshot(&self) -> DailyBudgetSnapshot {
        self.daily_budget.snapshot()
    }

    /// Acquires a provider slot and reserves daily attempt/iteration limits immediately before
    /// an external call. It never waits behind another run: saturation is a stable fail-closed
    /// `ProviderRateLimited` result so callers do not accumulate stale queued requests.
    pub fn acquire_provider_attempt(
        &self,
        run_id: &str,
        iteration: u32,
        provider_id: &str,
        image_count: usize,
        cancellation: &CancellationToken,
        task_budget: &TaskBudget,
    ) -> Result<ProviderRuntimeLease, TaskFailure> {
        cancellation.checkpoint()?;
        let sequence = self
            .next_task_id
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                value.checked_add(1)
            })
            .map_err(|_| TaskFailure::invalid("provider runtime task ID overflowed"))?;
        let ticket = self.queue.enqueue(QueuedTask {
            run_id: run_id.to_owned(),
            iteration,
            task_id: format!("provider-task-{sequence}"),
            provider_id: provider_id.to_owned(),
            cancellation: cancellation.clone(),
        })?;
        let queue_lease = match self.queue.try_start_ticket(&ticket) {
            Some(lease) => lease,
            None if cancellation.is_requested() => {
                let _ = self.queue.cancel(&ticket);
                return Err(TaskFailure::new(
                    TaskFailureKind::Cancelled,
                    "provider runtime queue was cancelled",
                    None,
                ));
            }
            None => {
                let _ = self.queue.cancel(&ticket);
                return Err(TaskFailure::new(
                    TaskFailureKind::ProviderRateLimited,
                    "provider runtime queue or provider concurrency limit is reached",
                    Some("UI_GENERATION_QUEUE_PROVIDER_CONCURRENCY".to_owned()),
                ));
            }
        };
        cancellation.checkpoint()?;
        let daily_reservation = self.daily_budget.reserve_external_attempt(image_count)?;
        let task_reservation = task_budget.reserve_provider_attempt_reservation(image_count)?;
        cancellation.checkpoint()?;
        daily_reservation.commit();
        task_reservation.commit();
        Ok(ProviderRuntimeLease {
            _queue_lease: queue_lease,
            daily_budget: self.daily_budget.clone(),
        })
    }
}

/// Drops its queue lease on every completion, failure, timeout, and cancellation path.
pub struct ProviderRuntimeLease {
    _queue_lease: QueueLease,
    daily_budget: DailyBudget,
}

impl ProviderRuntimeLease {
    pub fn record_elapsed(&self, elapsed_ms: u64) -> Result<(), TaskFailure> {
        self.daily_budget.record_elapsed(elapsed_ms)
    }

    pub fn record_success(
        &self,
        elapsed_ms: u64,
        input_units: Option<u64>,
        output_units: Option<u64>,
    ) -> Result<(), TaskFailure> {
        self.record_elapsed(elapsed_ms)?;
        self.daily_budget
            .record_provider_usage(input_units.unwrap_or(0), output_units.unwrap_or(0))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunCorrelation {
    pub run_id: String,
    pub iteration: u32,
    pub task_id: String,
}

impl RunCorrelation {
    pub fn new(
        run_id: impl Into<String>,
        iteration: u32,
        task_id: impl Into<String>,
    ) -> Result<Self, TaskFailure> {
        let correlation = Self {
            run_id: run_id.into(),
            iteration,
            task_id: task_id.into(),
        };
        if !safe_label(&correlation.run_id, 128)
            || correlation.iteration == 0
            || !safe_label(&correlation.task_id, 128)
        {
            return Err(TaskFailure::invalid(
                "log correlation requires safe run ID, nonzero iteration, and task ID",
            ));
        }
        Ok(correlation)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationStage {
    Preprocess,
    VisualAnalysis,
    UiDocumentGeneration,
    Screenshot,
    Comparison,
    Validation,
    Fix,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StageTelemetry {
    pub stage: OperationStage,
    pub elapsed_ms: u64,
    pub cache_hit: bool,
    pub retry_count: u32,
    pub provider_calls: u32,
    pub artifact_bytes: u64,
    pub node_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunFinalStatus {
    Passed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunTelemetryReport {
    pub protocol_version: u32,
    pub correlation: RunCorrelation,
    pub stages: Vec<StageTelemetry>,
    pub cache_hits: u32,
    pub cache_misses: u32,
    pub total_elapsed_ms: u64,
    pub total_retries: u32,
    pub provider_calls: u32,
    pub artifact_bytes: u64,
    pub node_count: u32,
    pub final_status: RunFinalStatus,
}

pub struct RunTelemetry {
    correlation: RunCorrelation,
    stages: Vec<StageTelemetry>,
}

impl RunTelemetry {
    pub fn new(correlation: RunCorrelation) -> Self {
        Self {
            correlation,
            stages: Vec::new(),
        }
    }

    pub fn record(&mut self, stage: StageTelemetry) -> Result<(), TaskFailure> {
        if stage.elapsed_ms > 24 * 60 * 60 * 1_000 || stage.artifact_bytes > 1024 * 1024 * 1024 {
            return Err(TaskFailure::invalid(
                "stage telemetry exceeds the operational duration or artifact-size boundary",
            ));
        }
        self.stages.push(stage);
        Ok(())
    }

    pub fn finish(self, final_status: RunFinalStatus) -> RunTelemetryReport {
        let cache_hits = u32::try_from(self.stages.iter().filter(|stage| stage.cache_hit).count())
            .unwrap_or(u32::MAX);
        let cache_misses = u32::try_from(self.stages.len())
            .unwrap_or(u32::MAX)
            .saturating_sub(cache_hits);
        RunTelemetryReport {
            protocol_version: OPERATIONS_PROTOCOL_VERSION,
            correlation: self.correlation,
            total_elapsed_ms: self.stages.iter().map(|stage| stage.elapsed_ms).sum(),
            total_retries: self.stages.iter().map(|stage| stage.retry_count).sum(),
            provider_calls: self.stages.iter().map(|stage| stage.provider_calls).sum(),
            artifact_bytes: self.stages.iter().map(|stage| stage.artifact_bytes).sum(),
            node_count: self.stages.iter().map(|stage| stage.node_count).sum(),
            stages: self.stages,
            cache_hits,
            cache_misses,
            final_status,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RedactedLogEvent {
    pub correlation: RunCorrelation,
    pub event: String,
    pub fields: Value,
}

impl RedactedLogEvent {
    pub fn new(
        correlation: RunCorrelation,
        event: impl Into<String>,
        fields: Value,
    ) -> Result<Self, TaskFailure> {
        let event = event.into();
        if !safe_label(&event, 128) {
            return Err(TaskFailure::invalid("log event name must be a safe label"));
        }
        Ok(Self {
            correlation,
            event,
            fields: crate::observability::redact_report_value(&fields),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactStoragePolicy {
    pub max_total_bytes: u64,
    pub max_single_artifact_bytes: u64,
    pub minimum_free_disk_bytes: u64,
}

impl ArtifactStoragePolicy {
    pub fn validate(&self) -> Result<(), TaskFailure> {
        if self.max_total_bytes == 0
            || self.max_single_artifact_bytes == 0
            || self.max_single_artifact_bytes > self.max_total_bytes
        {
            return Err(TaskFailure::invalid("artifact storage policy is invalid"));
        }
        Ok(())
    }

    /// Checks an injected free-space observation so callers can test safely without filling disk.
    pub fn reserve(
        &self,
        current_total_bytes: u64,
        new_artifact_bytes: u64,
        observed_free_disk_bytes: u64,
    ) -> Result<(), TaskFailure> {
        self.validate()?;
        let next_total = current_total_bytes
            .checked_add(new_artifact_bytes)
            .ok_or_else(|| storage_failure("artifact storage total overflowed"))?;
        let required_free = self
            .minimum_free_disk_bytes
            .checked_add(new_artifact_bytes)
            .ok_or_else(|| storage_failure("artifact storage free-space calculation overflowed"))?;
        if new_artifact_bytes > self.max_single_artifact_bytes
            || next_total > self.max_total_bytes
            || observed_free_disk_bytes < required_free
        {
            return Err(storage_failure(
                "artifact write exceeds quota or would violate the free-disk reserve",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactRetentionPolicy {
    pub passed_ttl_ms: u64,
    pub failed_ttl_ms: u64,
    pub cancelled_ttl_ms: u64,
}

impl ArtifactRetentionPolicy {
    pub fn validate(&self) -> Result<(), TaskFailure> {
        if self.passed_ttl_ms == 0
            || self.failed_ttl_ms == 0
            || self.cancelled_ttl_ms == 0
            || self.passed_ttl_ms > MAX_ARTIFACT_RETENTION_MS
            || self.failed_ttl_ms > MAX_ARTIFACT_RETENTION_MS
            || self.cancelled_ttl_ms > MAX_ARTIFACT_RETENTION_MS
        {
            return Err(TaskFailure::invalid(
                "artifact retention periods must be positive and bounded",
            ));
        }
        Ok(())
    }

    fn ttl(&self, status: ArtifactRunStatus) -> Option<u64> {
        match status {
            ArtifactRunStatus::Passed => Some(self.passed_ttl_ms),
            ArtifactRunStatus::Failed => Some(self.failed_ttl_ms),
            ArtifactRunStatus::Cancelled => Some(self.cancelled_ttl_ms),
            ArtifactRunStatus::Running => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactRunStatus {
    Running,
    Passed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactRetentionRecord {
    pub run_id: String,
    pub status: ArtifactRunStatus,
    pub terminal_at_unix_ms: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactRetentionInventory {
    pub protocol_version: u32,
    pub policy: ArtifactRetentionPolicy,
    pub runs: Vec<ArtifactRetentionRecord>,
}

impl ArtifactRetentionInventory {
    pub fn load(path: &Path) -> Result<Self, TaskFailure> {
        let inventory: Self = serde_json::from_slice(
            &fs::read(path)
                .map_err(|_| TaskFailure::invalid("artifact inventory cannot be read"))?,
        )
        .map_err(|_| TaskFailure::invalid("artifact inventory is malformed"))?;
        inventory.validate()?;
        Ok(inventory)
    }

    pub fn validate(&self) -> Result<(), TaskFailure> {
        if self.protocol_version != OPERATIONS_PROTOCOL_VERSION {
            return Err(TaskFailure::protocol_incompatible(
                "artifact inventory protocol is unsupported",
            ));
        }
        self.policy.validate()?;
        let mut run_ids = BTreeSet::new();
        for record in &self.runs {
            if !safe_artifact_run_id(&record.run_id) || !run_ids.insert(record.run_id.clone()) {
                return Err(TaskFailure::invalid(
                    "artifact inventory run IDs must be unique safe labels",
                ));
            }
            if (record.status == ArtifactRunStatus::Running) != record.terminal_at_unix_ms.is_none()
            {
                return Err(TaskFailure::invalid(
                    "running artifacts omit terminal time and terminal artifacts require it",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactCleanupPlan {
    pub protocol_version: u32,
    pub artifact_root_digest: String,
    pub delete_run_ids: Vec<String>,
    pub retained_run_ids: Vec<String>,
}

pub struct ArtifactCleaner {
    root: PathBuf,
}

impl ArtifactCleaner {
    /// Creates only a new, explicitly named artifact root and marker. Cleanup refuses roots that
    /// lack this marker, so it cannot be pointed at an arbitrary workspace directory.
    pub fn initialize(root: &Path) -> Result<Self, TaskFailure> {
        if root.file_name().and_then(|name| name.to_str()) != Some("ui-generation-artifacts") {
            return Err(TaskFailure::new(
                TaskFailureKind::UnsafeOutputPath,
                "artifact root must be explicitly named ui-generation-artifacts",
                None,
            ));
        }
        let parent = root.parent().ok_or_else(|| {
            TaskFailure::new(
                TaskFailureKind::UnsafeOutputPath,
                "artifact root must have an existing parent directory",
                None,
            )
        })?;
        reject_reparse_directory(parent, "artifact root parent")?;
        fs::create_dir(root).map_err(|_| {
            TaskFailure::new(
                TaskFailureKind::UnsafeOutputPath,
                "artifact root must be newly created rather than reused",
                None,
            )
        })?;
        fs::create_dir(root.join("runs"))
            .map_err(|_| TaskFailure::invalid("artifact runs directory cannot be created"))?;
        fs::write(
            root.join(ARTIFACT_ROOT_MARKER),
            ARTIFACT_ROOT_MARKER_CONTENT,
        )
        .map_err(|_| TaskFailure::invalid("artifact root marker cannot be written"))?;
        Self::open(root)
    }

    pub fn open(root: &Path) -> Result<Self, TaskFailure> {
        reject_reparse_directory(root, "artifact root")?;
        let marker = root.join(ARTIFACT_ROOT_MARKER);
        if !regular_file_contents(&marker, ARTIFACT_ROOT_MARKER_CONTENT)? {
            return Err(TaskFailure::new(
                TaskFailureKind::UnsafeOutputPath,
                "artifact cleanup requires an initialized artifact root marker",
                None,
            ));
        }
        reject_reparse_directory(&root.join("runs"), "artifact runs root")?;
        Ok(Self {
            root: fs::canonicalize(root)
                .map_err(|_| TaskFailure::invalid("artifact root cannot be canonicalized"))?,
        })
    }

    pub fn plan(
        &self,
        inventory: &ArtifactRetentionInventory,
        now_unix_ms: u64,
    ) -> Result<ArtifactCleanupPlan, TaskFailure> {
        inventory.validate()?;
        let mut delete_run_ids = Vec::new();
        let mut retained_run_ids = Vec::new();
        for record in &inventory.runs {
            let expired = record
                .terminal_at_unix_ms
                .zip(inventory.policy.ttl(record.status))
                .is_some_and(|(terminal_at, ttl)| {
                    now_unix_ms >= terminal_at && now_unix_ms - terminal_at >= ttl
                });
            if expired {
                self.run_path(&record.run_id)?;
                delete_run_ids.push(record.run_id.clone());
            } else {
                retained_run_ids.push(record.run_id.clone());
            }
        }
        Ok(ArtifactCleanupPlan {
            protocol_version: OPERATIONS_PROTOCOL_VERSION,
            artifact_root_digest: self.root_digest(),
            delete_run_ids,
            retained_run_ids,
        })
    }

    /// Applies only a plan produced for this marker-protected root. Every descendant is checked
    /// for links/reparse points before deletion and absent runs are harmlessly skipped.
    pub fn apply(&self, plan: &ArtifactCleanupPlan) -> Result<Vec<String>, TaskFailure> {
        if plan.protocol_version != OPERATIONS_PROTOCOL_VERSION
            || !is_sha256(&plan.artifact_root_digest)
            || plan.artifact_root_digest != self.root_digest()
        {
            return Err(TaskFailure::protocol_incompatible(
                "artifact cleanup plan protocol or root binding is unsupported",
            ));
        }
        reject_reparse_directory(&self.root, "artifact cleanup root")?;
        reject_reparse_directory(&self.root.join("runs"), "artifact cleanup runs root")?;
        let mut deleted = Vec::new();
        for run_id in &plan.delete_run_ids {
            let run = self.run_path(run_id)?;
            if !run.exists() {
                continue;
            }
            reject_reparse_tree(&run)?;
            fs::remove_dir_all(&run).map_err(|_| {
                TaskFailure::new(
                    TaskFailureKind::UnsafeOutputPath,
                    "controlled artifact cleanup could not delete a preflighted run",
                    Some(run_id.clone()),
                )
            })?;
            deleted.push(run_id.clone());
        }
        Ok(deleted)
    }

    fn run_path(&self, run_id: &str) -> Result<PathBuf, TaskFailure> {
        if !safe_artifact_run_id(run_id) {
            return Err(TaskFailure::new(
                TaskFailureKind::UnsafeOutputPath,
                "artifact cleanup run ID is unsafe",
                None,
            ));
        }
        let runs = self.root.join("runs");
        let path = runs.join(run_id);
        if path.parent() != Some(runs.as_path()) {
            return Err(TaskFailure::new(
                TaskFailureKind::UnsafeOutputPath,
                "artifact cleanup run path escaped its runs root",
                None,
            ));
        }
        Ok(path)
    }

    fn root_digest(&self) -> String {
        hex_sha256(self.root.to_string_lossy().as_bytes())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OfflineOperationsStressReport {
    pub protocol_version: u32,
    pub submitted_tasks: u32,
    pub provider_concurrency_limit: u32,
    pub peak_running_tasks: usize,
    pub cancelled_tasks: usize,
    pub exact_cache_hit: bool,
    pub cross_dimension_reuse_blocked: bool,
    pub daily_budget_exhausted: bool,
    pub disk_reserve_rejected: bool,
    pub log_redacted: bool,
    pub final_status: RunFinalStatus,
}

/// A deterministic, no-network pressure rehearsal for scheduler, budget, cache, disk, and log
/// guards. The returned report is suitable for a CI artifact but contains no user input.
pub fn run_offline_operations_stress_fixture() -> Result<OfflineOperationsStressReport, TaskFailure>
{
    let queue = BoundedTaskQueue::new(TaskQueuePolicy {
        max_queued_or_running: 4,
        default_provider_concurrency: 1,
        provider_concurrency: BTreeMap::from([("fixture-provider".to_owned(), 2)]),
    })?;
    let mut tickets = Vec::new();
    for index in 1..=4 {
        tickets.push(queue.enqueue(QueuedTask {
            run_id: format!("stress-run-{index}"),
            iteration: 1,
            task_id: format!("stress-task-{index}"),
            provider_id: "fixture-provider".to_owned(),
            cancellation: CancellationToken::default(),
        })?);
    }
    let first = queue
        .try_start_next()
        .ok_or_else(|| TaskFailure::invalid("stress fixture did not start first task"))?;
    let second = queue
        .try_start_next()
        .ok_or_else(|| TaskFailure::invalid("stress fixture did not start second task"))?;
    if queue.try_start_next().is_some() {
        return Err(TaskFailure::invalid(
            "stress fixture exceeded its provider concurrency limit",
        ));
    }
    if !queue.cancel(&tickets[2]) {
        return Err(TaskFailure::invalid(
            "stress fixture could not cancel queued task",
        ));
    }
    first.finish();
    let fourth = queue
        .try_start_next()
        .ok_or_else(|| TaskFailure::invalid("stress fixture did not schedule remaining task"))?;
    second.finish();
    fourth.finish();
    let snapshot = queue.snapshot();

    let daily = DailyBudget::new(
        "2026-07-22",
        TaskExecutionLimits {
            max_provider_calls: 2,
            max_images: 2,
            max_iterations: 2,
            ..TaskExecutionLimits::default()
        },
    )?;
    daily.reserve_provider_attempt(1)?;
    daily.reserve_provider_attempt(1)?;
    let daily_budget_exhausted = daily.reserve_provider_attempt(1).is_err();

    let dimensions = stress_cache_dimensions();
    let preprocess = StageCacheKey::new(CacheStage::Preprocess, dimensions.clone())?;
    let exact_cache_hit = matches!(
        preprocess.reuse_decision(&preprocess)?,
        CacheReuseDecision::Hit { .. }
    );
    let changed = StageCacheKey::new(
        CacheStage::Preprocess,
        CacheDimensions {
            font_revision: "font-v2".to_owned(),
            ..dimensions
        },
    )?;
    let cross_dimension_reuse_blocked = matches!(
        preprocess.reuse_decision(&changed)?,
        CacheReuseDecision::Miss { .. }
    );

    let disk_reserve_rejected = ArtifactStoragePolicy {
        max_total_bytes: 100,
        max_single_artifact_bytes: 80,
        minimum_free_disk_bytes: 50,
    }
    .reserve(0, 60, 100)
    .is_err();
    let event = RedactedLogEvent::new(
        RunCorrelation::new("stress-run-1", 1, "stress-task-1")?,
        "fixture_completed",
        serde_json::json!({"api_token": "sk-private", "account_text": "person@example.test"}),
    )?;
    let serialized = serde_json::to_string(&event)
        .map_err(|_| TaskFailure::invalid("stress log event cannot be serialized"))?;
    let log_redacted =
        !serialized.contains("sk-private") && !serialized.contains("person@example.test");

    Ok(OfflineOperationsStressReport {
        protocol_version: OPERATIONS_PROTOCOL_VERSION,
        submitted_tasks: u32::try_from(tickets.len()).unwrap_or(u32::MAX),
        provider_concurrency_limit: 2,
        peak_running_tasks: snapshot.peak_running,
        cancelled_tasks: snapshot.cancelled,
        exact_cache_hit,
        cross_dimension_reuse_blocked,
        daily_budget_exhausted,
        disk_reserve_rejected,
        log_redacted,
        final_status: RunFinalStatus::Passed,
    })
}

fn validate_usage(
    limits: &TaskExecutionLimits,
    usage: &TaskUsageSnapshot,
) -> Result<(), TaskFailure> {
    if usage.provider_calls > limits.max_provider_calls {
        return Err(budget_failure(
            "UI_GENERATION_DAILY_PROVIDER_CALLS",
            "daily provider-call limit reached",
        ));
    }
    if usage.images > limits.max_images {
        return Err(budget_failure(
            "UI_GENERATION_DAILY_IMAGES",
            "daily image limit reached",
        ));
    }
    if usage.input_units > limits.max_input_units {
        return Err(budget_failure(
            "UI_GENERATION_DAILY_INPUT_UNITS",
            "daily input-unit limit reached",
        ));
    }
    if usage.output_units > limits.max_output_units {
        return Err(budget_failure(
            "UI_GENERATION_DAILY_OUTPUT_UNITS",
            "daily output-unit limit reached",
        ));
    }
    if usage.iterations > limits.max_iterations {
        return Err(budget_failure(
            "UI_GENERATION_DAILY_ITERATIONS",
            "daily iteration limit reached",
        ));
    }
    if usage.elapsed_ms > limits.max_elapsed_ms {
        return Err(budget_failure(
            "UI_GENERATION_DAILY_ELAPSED",
            "daily elapsed-time limit reached",
        ));
    }
    if usage.estimated_cost_microunits > limits.max_estimated_cost_microunits {
        return Err(budget_failure(
            "UI_GENERATION_DAILY_COST",
            "daily estimated-cost limit reached",
        ));
    }
    Ok(())
}

fn units_cost(units: u64, rate_per_1k: u64) -> Result<u64, TaskFailure> {
    units
        .checked_mul(rate_per_1k)
        .and_then(|value| value.checked_add(999))
        .map(|value| value / 1_000)
        .ok_or_else(|| budget_failure("UI_GENERATION_DAILY_COST", "daily cost overflowed"))
}

fn budget_failure(code: &str, message: &str) -> TaskFailure {
    TaskFailure::new(
        TaskFailureKind::ProviderRateLimited,
        message,
        Some(code.to_owned()),
    )
}

fn storage_failure(message: &str) -> TaskFailure {
    TaskFailure::new(
        TaskFailureKind::ArtifactMissing,
        message,
        Some("UI_GENERATION_ARTIFACT_STORAGE_LIMIT".to_owned()),
    )
}

fn is_day(value: &str) -> bool {
    value.len() == 10
        && value.as_bytes()[4] == b'-'
        && value.as_bytes()[7] == b'-'
        && value
            .bytes()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
}

fn safe_label(value: &str, maximum_length: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum_length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn safe_artifact_run_id(value: &str) -> bool {
    safe_label(value, 128) && !matches!(value, "." | "..")
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn hex_sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn reject_reparse_directory(path: &Path, subject: &str) -> Result<(), TaskFailure> {
    let metadata = fs::symlink_metadata(path).map_err(|_| {
        TaskFailure::new(
            TaskFailureKind::UnsafeOutputPath,
            "artifact directory cannot be inspected",
            Some(subject.to_owned()),
        )
    })?;
    if !metadata.is_dir() || is_reparse(&metadata) {
        return Err(TaskFailure::new(
            TaskFailureKind::UnsafeOutputPath,
            "artifact directory must be a regular non-reparse directory",
            Some(subject.to_owned()),
        ));
    }
    Ok(())
}

fn regular_file_contents(path: &Path, expected: &[u8]) -> Result<bool, TaskFailure> {
    let metadata = fs::symlink_metadata(path).map_err(|_| {
        TaskFailure::new(
            TaskFailureKind::UnsafeOutputPath,
            "artifact marker cannot be inspected",
            None,
        )
    })?;
    if !metadata.is_file()
        || metadata.len() != u64::try_from(expected.len()).unwrap_or(u64::MAX)
        || is_reparse(&metadata)
    {
        return Ok(false);
    }
    Ok(fs::read(path).ok().as_deref() == Some(expected))
}

fn reject_reparse_tree(path: &Path) -> Result<(), TaskFailure> {
    let metadata = fs::symlink_metadata(path).map_err(|_| {
        TaskFailure::new(
            TaskFailureKind::UnsafeOutputPath,
            "artifact cleanup target cannot be inspected",
            None,
        )
    })?;
    if is_reparse(&metadata) || (!metadata.is_dir() && !metadata.is_file()) {
        return Err(TaskFailure::new(
            TaskFailureKind::UnsafeOutputPath,
            "artifact cleanup rejects reparse points and non-regular filesystem entries",
            None,
        ));
    }
    if metadata.is_dir() {
        for entry in fs::read_dir(path).map_err(|_| {
            TaskFailure::new(
                TaskFailureKind::UnsafeOutputPath,
                "artifact cleanup target cannot be enumerated",
                None,
            )
        })? {
            let entry = entry
                .map_err(|_| TaskFailure::invalid("artifact directory entry is unreadable"))?;
            reject_reparse_tree(&entry.path())?;
        }
    }
    Ok(())
}

fn is_reparse(metadata: &fs::Metadata) -> bool {
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

fn stress_cache_dimensions() -> CacheDimensions {
    CacheDimensions {
        input_sha256: "a".repeat(64),
        schema_id: "ui-reference-analysis".to_owned(),
        schema_version: 1,
        prompt_revision: "fixture-prompt-v1".to_owned(),
        model_revision: "fixture-model-v1".to_owned(),
        theme_revision: "default-v1".to_owned(),
        font_revision: "font-v1".to_owned(),
        viewport: CacheViewport {
            width: 390,
            height: 844,
            device_scale_milli: 3_000,
        },
        algorithm_revision: "fixture-algorithm-v1".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_cache_stage_requires_all_reuse_dimensions_and_invalidates_each_one() {
        let dimensions = stress_cache_dimensions();
        let expected_invalidations = [
            (
                CacheInvalidation::InputHash,
                CacheDimensions {
                    input_sha256: "b".repeat(64),
                    ..dimensions.clone()
                },
            ),
            (
                CacheInvalidation::Schema,
                CacheDimensions {
                    schema_id: "other-schema".to_owned(),
                    ..dimensions.clone()
                },
            ),
            (
                CacheInvalidation::Prompt,
                CacheDimensions {
                    prompt_revision: "fixture-prompt-v2".to_owned(),
                    ..dimensions.clone()
                },
            ),
            (
                CacheInvalidation::Model,
                CacheDimensions {
                    model_revision: "fixture-model-v2".to_owned(),
                    ..dimensions.clone()
                },
            ),
            (
                CacheInvalidation::Theme,
                CacheDimensions {
                    theme_revision: "theme-v2".to_owned(),
                    ..dimensions.clone()
                },
            ),
            (
                CacheInvalidation::Font,
                CacheDimensions {
                    font_revision: "font-v2".to_owned(),
                    ..dimensions.clone()
                },
            ),
            (
                CacheInvalidation::Viewport,
                CacheDimensions {
                    viewport: CacheViewport {
                        width: 844,
                        height: 390,
                        device_scale_milli: 3_000,
                    },
                    ..dimensions.clone()
                },
            ),
            (
                CacheInvalidation::Algorithm,
                CacheDimensions {
                    algorithm_revision: "fixture-algorithm-v2".to_owned(),
                    ..dimensions.clone()
                },
            ),
        ];
        for stage in [
            CacheStage::Preprocess,
            CacheStage::VisualAnalysis,
            CacheStage::UiDocumentGeneration,
            CacheStage::Screenshot,
            CacheStage::Comparison,
        ] {
            let key = StageCacheKey::new(stage, dimensions.clone()).unwrap();
            assert!(matches!(
                key.reuse_decision(&key).unwrap(),
                CacheReuseDecision::Hit { .. }
            ));
            for (expected, changed_dimensions) in &expected_invalidations {
                let changed = StageCacheKey::new(stage, changed_dimensions.clone()).unwrap();
                assert!(matches!(
                    key.reuse_decision(&changed).unwrap(),
                    CacheReuseDecision::Miss { invalidation }
                        if invalidation == vec![*expected]
                ));
            }
        }
        let missing = CacheDimensions {
            prompt_revision: String::new(),
            ..dimensions
        };
        assert!(StageCacheKey::new(CacheStage::Preprocess, missing).is_err());
    }

    #[test]
    fn bounded_queue_enforces_provider_capacity_and_cancellation() {
        let capacity_one = BoundedTaskQueue::new(TaskQueuePolicy {
            max_queued_or_running: 1,
            ..TaskQueuePolicy::default()
        })
        .unwrap();
        capacity_one
            .enqueue(QueuedTask {
                run_id: "queue-run-1".to_owned(),
                iteration: 1,
                task_id: "queue-task-1".to_owned(),
                provider_id: "fixture".to_owned(),
                cancellation: CancellationToken::default(),
            })
            .unwrap();
        assert_eq!(
            capacity_one
                .enqueue(QueuedTask {
                    run_id: "queue-run-2".to_owned(),
                    iteration: 1,
                    task_id: "queue-task-2".to_owned(),
                    provider_id: "fixture".to_owned(),
                    cancellation: CancellationToken::default(),
                })
                .unwrap_err()
                .subject(),
            Some("UI_GENERATION_QUEUE_FULL")
        );

        let queue = BoundedTaskQueue::new(TaskQueuePolicy {
            max_queued_or_running: 3,
            default_provider_concurrency: 1,
            provider_concurrency: BTreeMap::from([("fixture".to_owned(), 1)]),
        })
        .unwrap();
        let first = queue
            .enqueue(QueuedTask {
                run_id: "run-1".to_owned(),
                iteration: 1,
                task_id: "task-1".to_owned(),
                provider_id: "fixture".to_owned(),
                cancellation: CancellationToken::default(),
            })
            .unwrap();
        let second = queue
            .enqueue(QueuedTask {
                run_id: "run-2".to_owned(),
                iteration: 1,
                task_id: "task-2".to_owned(),
                provider_id: "fixture".to_owned(),
                cancellation: CancellationToken::default(),
            })
            .unwrap();
        let active = queue.try_start_next().unwrap();
        assert!(queue.try_start_next().is_none());
        assert!(queue.cancel(&second));
        active.finish();
        assert!(queue.try_start_next().is_none());
        let snapshot = queue.snapshot();
        assert_eq!(snapshot.peak_running, 1);
        assert_eq!(snapshot.cancelled, 1);
        assert_eq!(first.task_id, "task-1");
    }

    #[test]
    fn queue_prunes_idle_provider_counts_after_completion_and_cancellation() {
        let queue = BoundedTaskQueue::new(TaskQueuePolicy {
            max_queued_or_running: 4,
            default_provider_concurrency: 1,
            provider_concurrency: BTreeMap::from([
                ("provider-a".to_owned(), 1),
                ("provider-b".to_owned(), 1),
            ]),
        })
        .unwrap();
        let _provider_a = queue
            .enqueue(QueuedTask {
                run_id: "provider-run-a".to_owned(),
                iteration: 1,
                task_id: "provider-task-a".to_owned(),
                provider_id: "provider-a".to_owned(),
                cancellation: CancellationToken::default(),
            })
            .unwrap();
        let provider_b = queue
            .enqueue(QueuedTask {
                run_id: "provider-run-b".to_owned(),
                iteration: 1,
                task_id: "provider-task-b".to_owned(),
                provider_id: "provider-b".to_owned(),
                cancellation: CancellationToken::default(),
            })
            .unwrap();
        let active_a = queue.try_start_next().unwrap();
        let active_b = queue.try_start_next().unwrap();
        assert_eq!(queue.snapshot().provider_active.len(), 2);
        active_a.finish();
        assert_eq!(
            queue.snapshot().provider_active,
            BTreeMap::from([("provider-b".to_owned(), 1)])
        );
        assert!(queue.cancel(&provider_b));
        active_b.finish();
        assert!(queue.snapshot().provider_active.is_empty());

        queue
            .enqueue(QueuedTask {
                run_id: "provider-run-a-next".to_owned(),
                iteration: 1,
                task_id: "provider-task-a-next".to_owned(),
                provider_id: "provider-a".to_owned(),
                cancellation: CancellationToken::default(),
            })
            .unwrap();
        let next_a = queue.try_start_next().unwrap();
        assert_eq!(next_a.provider_id, "provider-a");
        next_a.finish();
        assert!(queue.snapshot().provider_active.is_empty());
    }

    #[test]
    fn daily_budget_is_atomic_and_persistable() {
        let limits = TaskExecutionLimits {
            max_provider_calls: 1,
            max_images: 1,
            max_input_units: 4,
            max_output_units: 4,
            max_iterations: 1,
            max_estimated_cost_microunits: 100,
            ..TaskExecutionLimits::default()
        };
        let daily = DailyBudget::new("2026-07-22", limits.clone()).unwrap();
        daily.reserve_provider_attempt(1).unwrap();
        assert!(daily.reserve_provider_attempt(1).is_err());
        daily.record_provider_usage(4, 4).unwrap();
        daily.reserve_iteration().unwrap();
        daily.record_elapsed(12).unwrap();
        let serialized = daily.snapshot().to_json_bytes().unwrap();
        let restored = DailyBudget::from_snapshot(
            limits,
            DailyBudgetSnapshot::parse_json(&serialized).unwrap(),
        )
        .unwrap();
        assert_eq!(restored.snapshot().usage.provider_calls, 1);
        assert_eq!(restored.snapshot().usage.iterations, 1);
    }

    #[test]
    fn telemetry_and_logs_record_only_correlated_redacted_evidence() {
        let correlation = RunCorrelation::new("run-1", 2, "task-1").unwrap();
        let mut telemetry = RunTelemetry::new(correlation.clone());
        telemetry
            .record(StageTelemetry {
                stage: OperationStage::VisualAnalysis,
                elapsed_ms: 10,
                cache_hit: false,
                retry_count: 1,
                provider_calls: 1,
                artifact_bytes: 20,
                node_count: 3,
            })
            .unwrap();
        let report = telemetry.finish(RunFinalStatus::Passed);
        assert_eq!(report.cache_misses, 1);
        assert_eq!(report.total_retries, 1);
        let log = RedactedLogEvent::new(
            correlation,
            "provider_finished",
            serde_json::json!({"authorization": "Bearer private", "message": "user@example.test"}),
        )
        .unwrap();
        let output = serde_json::to_string(&log).unwrap();
        assert!(!output.contains("private"));
        assert!(!output.contains("user@example.test"));
    }

    #[test]
    fn cleanup_rejects_unknown_roots_preserves_failures_until_their_ttl_and_checks_links() {
        let parent = tempfile::tempdir().unwrap();
        let unknown = parent.path().join("unknown");
        fs::create_dir(&unknown).unwrap();
        assert!(ArtifactCleaner::open(&unknown).is_err());

        let root = parent.path().join("ui-generation-artifacts");
        let cleaner = ArtifactCleaner::initialize(&root).unwrap();
        let failed = root.join("runs").join("failed-run");
        fs::create_dir(&failed).unwrap();
        fs::write(failed.join("evidence.json"), b"{}").unwrap();
        let inventory = ArtifactRetentionInventory {
            protocol_version: OPERATIONS_PROTOCOL_VERSION,
            policy: ArtifactRetentionPolicy {
                passed_ttl_ms: 10,
                failed_ttl_ms: 100,
                cancelled_ttl_ms: 10,
            },
            runs: vec![ArtifactRetentionRecord {
                run_id: "failed-run".to_owned(),
                status: ArtifactRunStatus::Failed,
                terminal_at_unix_ms: Some(100),
            }],
        };
        let retained = cleaner.plan(&inventory, 150).unwrap();
        assert!(retained.delete_run_ids.is_empty());
        let expired = cleaner.plan(&inventory, 200).unwrap();
        let mut forged_root_plan = expired.clone();
        forged_root_plan.artifact_root_digest = "b".repeat(64);
        assert!(cleaner.apply(&forged_root_plan).is_err());
        assert_eq!(cleaner.apply(&expired).unwrap(), vec!["failed-run"]);
        assert!(!failed.exists());
        let unsafe_inventory = ArtifactRetentionInventory {
            runs: vec![ArtifactRetentionRecord {
                run_id: "..".to_owned(),
                status: ArtifactRunStatus::Passed,
                terminal_at_unix_ms: Some(0),
            }],
            ..inventory
        };
        assert!(unsafe_inventory.validate().is_err());
    }

    #[test]
    fn disk_shortage_and_offline_pressure_rehearsal_are_reported() {
        assert!(
            ArtifactStoragePolicy {
                max_total_bytes: 10,
                max_single_artifact_bytes: 10,
                minimum_free_disk_bytes: 5,
            }
            .reserve(0, 6, 10)
            .is_err()
        );
        let report = run_offline_operations_stress_fixture().unwrap();
        assert_eq!(report.peak_running_tasks, 2);
        assert_eq!(report.cancelled_tasks, 1);
        assert!(report.daily_budget_exhausted);
        assert!(report.disk_reserve_rejected);
        assert!(report.log_redacted);
    }
}
