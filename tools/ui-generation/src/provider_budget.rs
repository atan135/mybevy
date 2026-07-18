//! Lightweight provider-call budgets shared by generation and audit tools.

use crate::lifecycle::{TaskFailure, TaskFailureKind};
use serde::Serialize;
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

const MAX_LIMIT_DURATION_MS: u64 = 60 * 60 * 1000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskExecutionLimits {
    pub max_provider_calls: u32,
    pub max_elapsed_ms: u64,
    pub max_images: usize,
    pub max_input_units: u64,
    pub max_output_units: u64,
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
            max_estimated_cost_microunits: 10_000_000,
            input_cost_microunits_per_1k: 1_000,
            output_cost_microunits_per_1k: 2_000,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskUsageSnapshot {
    pub provider_calls: u32,
    pub images: usize,
    pub input_units: u64,
    pub output_units: u64,
    pub estimated_cost_microunits: u64,
    pub elapsed_ms: u64,
}

#[derive(Clone)]
pub struct TaskBudget {
    limits: TaskExecutionLimits,
    started: Instant,
    usage: Arc<Mutex<TaskUsageSnapshot>>,
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
        let elapsed = elapsed_ms(self.started);
        let mut usage = self.usage.lock().expect("task budget mutex poisoned");
        usage.elapsed_ms = elapsed;
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
        Ok(())
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
        usage.elapsed_ms = elapsed;
        usage.input_units = usage.input_units.checked_add(input_units).ok_or_else(|| {
            limit_failure(
                "UI_GENERATION_LIMIT_INPUT_UNITS",
                "task input-unit limit overflowed",
            )
        })?;
        usage.output_units = usage
            .output_units
            .checked_add(output_units)
            .ok_or_else(|| {
                limit_failure(
                    "UI_GENERATION_LIMIT_OUTPUT_UNITS",
                    "task output-unit limit overflowed",
                )
            })?;
        usage.estimated_cost_microunits = usage
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
        if usage.input_units > self.limits.max_input_units {
            return Err(limit_failure(
                "UI_GENERATION_LIMIT_INPUT_UNITS",
                "task input-unit limit reached",
            ));
        }
        if usage.output_units > self.limits.max_output_units {
            return Err(limit_failure(
                "UI_GENERATION_LIMIT_OUTPUT_UNITS",
                "task output-unit limit reached",
            ));
        }
        if usage.estimated_cost_microunits > self.limits.max_estimated_cost_microunits {
            return Err(limit_failure(
                "UI_GENERATION_LIMIT_COST",
                "task estimated-cost limit reached",
            ));
        }
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
}
