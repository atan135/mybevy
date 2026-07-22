//! Lightweight provider-call budgets shared by generation and audit tools.

use crate::lifecycle::{TaskFailure, TaskFailureKind};
use serde::{Deserialize, Serialize};
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

const MAX_LIMIT_DURATION_MS: u64 = 24 * 60 * 60 * 1000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskExecutionLimits {
    pub max_provider_calls: u32,
    pub max_elapsed_ms: u64,
    pub max_images: usize,
    pub max_input_units: u64,
    pub max_output_units: u64,
    pub max_iterations: u32,
    pub max_estimated_cost_microunits: u64,
    pub input_cost_microunits_per_1k: u64,
    pub output_cost_microunits_per_1k: u64,
}

impl TaskExecutionLimits {
    pub fn validate(&self) -> Result<(), TaskFailure> {
        if self.max_provider_calls == 0
            || self.max_elapsed_ms == 0
            || self.max_elapsed_ms > MAX_LIMIT_DURATION_MS
            || self.max_images == 0
            || self.max_input_units == 0
            || self.max_output_units == 0
            || self.max_iterations == 0
            || self.max_estimated_cost_microunits == 0
        {
            return Err(TaskFailure::invalid(
                "task execution limits must use positive, bounded hard-stop values",
            ));
        }
        Ok(())
    }
}

impl Default for TaskExecutionLimits {
    fn default() -> Self {
        Self {
            max_provider_calls: 6,
            max_elapsed_ms: 5 * 60 * 1000,
            max_images: 12,
            max_input_units: 1_000_000,
            max_output_units: 250_000,
            max_iterations: 4,
            max_estimated_cost_microunits: 10_000_000,
            input_cost_microunits_per_1k: 1_000,
            output_cost_microunits_per_1k: 2_000,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskUsageSnapshot {
    pub provider_calls: u32,
    pub images: usize,
    pub input_units: u64,
    pub output_units: u64,
    pub iterations: u32,
    pub estimated_cost_microunits: u64,
    pub elapsed_ms: u64,
}

#[derive(Clone)]
pub struct TaskBudget {
    limits: TaskExecutionLimits,
    started: Instant,
    usage: Arc<Mutex<TaskUsageSnapshot>>,
}

/// An uncommitted local provider-attempt reservation. Dropping it restores exactly the call and
/// image capacity it reserved, which lets a higher-level runtime compose local and shared quota
/// checks without charging a run that never reaches an external provider.
pub struct TaskAttemptReservation {
    usage: Arc<Mutex<TaskUsageSnapshot>>,
    image_count: usize,
    committed: bool,
}

impl TaskAttemptReservation {
    pub fn commit(mut self) {
        self.committed = true;
    }
}

impl Drop for TaskAttemptReservation {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        let mut usage = self.usage.lock().expect("task budget mutex poisoned");
        usage.provider_calls = usage.provider_calls.saturating_sub(1);
        usage.images = usage.images.saturating_sub(self.image_count);
    }
}

impl TaskBudget {
    pub fn new(limits: TaskExecutionLimits) -> Result<Self, TaskFailure> {
        limits.validate()?;
        Ok(Self {
            limits,
            started: Instant::now(),
            usage: Arc::new(Mutex::new(TaskUsageSnapshot::default())),
        })
    }

    pub fn limits(&self) -> &TaskExecutionLimits {
        &self.limits
    }

    pub fn snapshot(&self) -> TaskUsageSnapshot {
        let mut snapshot = self
            .usage
            .lock()
            .expect("task budget mutex poisoned")
            .clone();
        snapshot.elapsed_ms = elapsed_ms(self.started);
        snapshot
    }

    pub fn reserve_provider_attempt(&self, image_count: usize) -> Result<(), TaskFailure> {
        self.reserve_provider_attempt_reservation(image_count)?
            .commit();
        Ok(())
    }

    pub fn reserve_provider_attempt_reservation(
        &self,
        image_count: usize,
    ) -> Result<TaskAttemptReservation, TaskFailure> {
        let elapsed = elapsed_ms(self.started);
        let mut usage = self.usage.lock().expect("task budget mutex poisoned");
        if elapsed > self.limits.max_elapsed_ms {
            return Err(limit_failure(
                "UI_GENERATION_LIMIT_ELAPSED",
                "task elapsed-time limit reached",
            ));
        }
        if usage.provider_calls >= self.limits.max_provider_calls {
            return Err(limit_failure(
                "UI_GENERATION_LIMIT_PROVIDER_CALLS",
                "task provider-call limit reached",
            ));
        }
        let next_images = usage.images.checked_add(image_count).ok_or_else(|| {
            limit_failure("UI_GENERATION_LIMIT_IMAGES", "task image limit overflowed")
        })?;
        if next_images > self.limits.max_images {
            return Err(limit_failure(
                "UI_GENERATION_LIMIT_IMAGES",
                "task image limit reached",
            ));
        }
        usage.provider_calls += 1;
        usage.images = next_images;
        usage.elapsed_ms = elapsed;
        Ok(TaskAttemptReservation {
            usage: Arc::clone(&self.usage),
            image_count,
            committed: false,
        })
    }

    pub fn record_provider_usage(
        &self,
        input_units: Option<u64>,
        output_units: Option<u64>,
    ) -> Result<(), TaskFailure> {
        let input_units = input_units.unwrap_or(0);
        let output_units = output_units.unwrap_or(0);
        let input_cost = units_cost(input_units, self.limits.input_cost_microunits_per_1k)?;
        let output_cost = units_cost(output_units, self.limits.output_cost_microunits_per_1k)?;
        let cost = input_cost.checked_add(output_cost).ok_or_else(|| {
            limit_failure("UI_GENERATION_LIMIT_COST", "task estimated cost overflowed")
        })?;

        let elapsed = elapsed_ms(self.started);
        let mut usage = self.usage.lock().expect("task budget mutex poisoned");
        let mut next = usage.clone();
        next.elapsed_ms = elapsed;
        next.input_units = next.input_units.checked_add(input_units).ok_or_else(|| {
            limit_failure(
                "UI_GENERATION_LIMIT_INPUT_UNITS",
                "task input-unit limit overflowed",
            )
        })?;
        next.output_units = next.output_units.checked_add(output_units).ok_or_else(|| {
            limit_failure(
                "UI_GENERATION_LIMIT_OUTPUT_UNITS",
                "task output-unit limit overflowed",
            )
        })?;
        next.estimated_cost_microunits = next
            .estimated_cost_microunits
            .checked_add(cost)
            .ok_or_else(|| {
                limit_failure("UI_GENERATION_LIMIT_COST", "task estimated cost overflowed")
            })?;
        if elapsed > self.limits.max_elapsed_ms {
            return Err(limit_failure(
                "UI_GENERATION_LIMIT_ELAPSED",
                "task elapsed-time limit reached",
            ));
        }
        if next.input_units > self.limits.max_input_units {
            return Err(limit_failure(
                "UI_GENERATION_LIMIT_INPUT_UNITS",
                "task input-unit limit reached",
            ));
        }
        if next.output_units > self.limits.max_output_units {
            return Err(limit_failure(
                "UI_GENERATION_LIMIT_OUTPUT_UNITS",
                "task output-unit limit reached",
            ));
        }
        if next.estimated_cost_microunits > self.limits.max_estimated_cost_microunits {
            return Err(limit_failure(
                "UI_GENERATION_LIMIT_COST",
                "task estimated-cost limit reached",
            ));
        }
        *usage = next;
        Ok(())
    }

    /// Reserves a repair or audit iteration before performing any externally visible work.
    pub fn reserve_iteration(&self) -> Result<(), TaskFailure> {
        let elapsed = elapsed_ms(self.started);
        let mut usage = self.usage.lock().expect("task budget mutex poisoned");
        if elapsed > self.limits.max_elapsed_ms {
            return Err(limit_failure(
                "UI_GENERATION_LIMIT_ELAPSED",
                "task elapsed-time limit reached",
            ));
        }
        if usage.iterations >= self.limits.max_iterations {
            return Err(limit_failure(
                "UI_GENERATION_LIMIT_ITERATIONS",
                "task iteration limit reached",
            ));
        }
        usage.iterations += 1;
        usage.elapsed_ms = elapsed;
        Ok(())
    }
}

fn units_cost(units: u64, rate_per_1k: u64) -> Result<u64, TaskFailure> {
    units
        .checked_mul(rate_per_1k)
        .and_then(|value| value.checked_add(999))
        .map(|value| value / 1000)
        .ok_or_else(|| limit_failure("UI_GENERATION_LIMIT_COST", "task estimated cost overflowed"))
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

fn limit_failure(code: &str, message: &str) -> TaskFailure {
    TaskFailure::new(
        TaskFailureKind::InvalidInput,
        message,
        Some(code.to_owned()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_attempt_and_usage_limits_are_shared_hard_stops() {
        let budget = TaskBudget::new(TaskExecutionLimits {
            max_provider_calls: 1,
            max_images: 2,
            max_input_units: 10,
            max_output_units: 10,
            ..TaskExecutionLimits::default()
        })
        .unwrap();
        budget.reserve_provider_attempt(2).unwrap();
        assert!(budget.reserve_provider_attempt(1).is_err());
        budget.record_provider_usage(Some(10), Some(10)).unwrap();
        assert_eq!(budget.snapshot().images, 2);
    }

    #[test]
    fn iterations_are_a_hard_stop_and_rejected_usage_is_not_partially_recorded() {
        let budget = TaskBudget::new(TaskExecutionLimits {
            max_provider_calls: 2,
            max_images: 2,
            max_input_units: 1,
            max_output_units: 1,
            max_iterations: 1,
            ..TaskExecutionLimits::default()
        })
        .unwrap();
        budget.reserve_iteration().unwrap();
        assert_eq!(
            budget.reserve_iteration().unwrap_err().subject(),
            Some("UI_GENERATION_LIMIT_ITERATIONS")
        );
        assert!(budget.record_provider_usage(Some(2), None).is_err());
        assert_eq!(budget.snapshot().input_units, 0);
    }
}
