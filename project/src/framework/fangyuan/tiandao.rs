use serde::{Deserialize, Serialize};

pub const FANGYUAN_TIANDAO_DEFAULT_SOLIDIFY_THRESHOLD: f32 = 0.72;
pub const FANGYUAN_TIANDAO_DEFAULT_DECAY_THRESHOLD: f32 = 0.28;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FangyuanTiandaoManifestation {
    pub id: String,
    pub cause: FangyuanTiandaoCause,
    pub region: String,
    pub budget_cost: u32,
    pub lifecycle_state: FangyuanTiandaoLifecycleState,
    pub ttl: u32,
    pub solidify_score: f32,
}

impl FangyuanTiandaoManifestation {
    pub fn new(
        id: impl Into<String>,
        cause: FangyuanTiandaoCause,
        region: impl Into<String>,
        budget_cost: u32,
        ttl: u32,
        solidify_score: f32,
    ) -> Self {
        Self {
            id: id.into(),
            cause,
            region: region.into(),
            budget_cost,
            lifecycle_state: FangyuanTiandaoLifecycleState::Manifest,
            ttl,
            solidify_score,
        }
    }

    pub fn validate(&self) -> Result<(), FangyuanTiandaoValidationError> {
        if self.id.trim().is_empty() {
            return Err(FangyuanTiandaoValidationError::EmptyId);
        }
        if self.region.trim().is_empty() {
            return Err(FangyuanTiandaoValidationError::EmptyRegion);
        }
        if self.budget_cost == 0 {
            return Err(FangyuanTiandaoValidationError::InvalidBudgetCost {
                value: self.budget_cost,
            });
        }
        if !self.solidify_score.is_finite() || !(0.0..=1.0).contains(&self.solidify_score) {
            return Err(FangyuanTiandaoValidationError::InvalidSolidifyScore {
                value: self.solidify_score,
            });
        }
        Ok(())
    }

    pub fn tick(&mut self, rules: &FangyuanTiandaoLifecycleRules) -> FangyuanTiandaoTransition {
        fangyuan_tiandao_step_lifecycle(self, FangyuanTiandaoLifecycleInput::Tick, rules)
    }

    pub fn solidify(&mut self, rules: &FangyuanTiandaoLifecycleRules) -> FangyuanTiandaoTransition {
        fangyuan_tiandao_step_lifecycle(self, FangyuanTiandaoLifecycleInput::Solidify, rules)
    }

    pub fn recycle(&mut self) -> FangyuanTiandaoTransition {
        fangyuan_tiandao_step_lifecycle(
            self,
            FangyuanTiandaoLifecycleInput::Recycle,
            &FangyuanTiandaoLifecycleRules::default(),
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum FangyuanTiandaoCause {
    PlayerAction { actor_id: String, action: String },
    RegionPressure { reason: String },
    Scripted { script_id: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FangyuanTiandaoLifecycleState {
    Manifest,
    Decay,
    Solidify,
    Recycle,
}

impl Default for FangyuanTiandaoLifecycleState {
    fn default() -> Self {
        Self::Manifest
    }
}

impl FangyuanTiandaoLifecycleState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Manifest => "manifest",
            Self::Decay => "decay",
            Self::Solidify => "solidify",
            Self::Recycle => "recycle",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FangyuanTiandaoLifecycleRules {
    pub solidify_threshold: f32,
    pub decay_threshold: f32,
    pub decay_ticks_before_recycle: u32,
}

impl Default for FangyuanTiandaoLifecycleRules {
    fn default() -> Self {
        Self {
            solidify_threshold: FANGYUAN_TIANDAO_DEFAULT_SOLIDIFY_THRESHOLD,
            decay_threshold: FANGYUAN_TIANDAO_DEFAULT_DECAY_THRESHOLD,
            decay_ticks_before_recycle: 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanTiandaoLifecycleInput {
    Tick,
    Solidify,
    Recycle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FangyuanTiandaoTransitionReason {
    ManifestTick,
    TtlExpired,
    ScoreBelowDecayThreshold,
    ScoreReachedSolidifyThreshold,
    ExplicitSolidify,
    DecayCompleted,
    ExplicitRecycle,
    AlreadyRecycled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FangyuanTiandaoTransition {
    pub from: FangyuanTiandaoLifecycleState,
    pub to: FangyuanTiandaoLifecycleState,
    pub reason: FangyuanTiandaoTransitionReason,
    pub ttl_before: u32,
    pub ttl_after: u32,
    pub released_budget: u32,
}

impl FangyuanTiandaoTransition {
    pub const fn changed(&self) -> bool {
        self.from as u8 != self.to as u8
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum FangyuanTiandaoValidationError {
    EmptyId,
    EmptyRegion,
    InvalidBudgetCost { value: u32 },
    InvalidSolidifyScore { value: f32 },
}

impl FangyuanTiandaoValidationError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::EmptyId => "tiandao_empty_id",
            Self::EmptyRegion => "tiandao_empty_region",
            Self::InvalidBudgetCost { .. } => "tiandao_invalid_budget_cost",
            Self::InvalidSolidifyScore { .. } => "tiandao_invalid_solidify_score",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FangyuanTiandaoBudgetRecycleResult {
    pub released_budget: u32,
    pub recycled: bool,
}

impl FangyuanTiandaoBudgetRecycleResult {
    pub const fn from_transition(transition: FangyuanTiandaoTransition) -> Self {
        Self {
            released_budget: transition.released_budget,
            recycled: matches!(transition.to, FangyuanTiandaoLifecycleState::Recycle),
        }
    }
}

pub fn fangyuan_tiandao_step_lifecycle(
    manifestation: &mut FangyuanTiandaoManifestation,
    input: FangyuanTiandaoLifecycleInput,
    rules: &FangyuanTiandaoLifecycleRules,
) -> FangyuanTiandaoTransition {
    let from = manifestation.lifecycle_state;
    let ttl_before = manifestation.ttl;

    let (to, ttl_after, reason) = match input {
        FangyuanTiandaoLifecycleInput::Recycle => (
            FangyuanTiandaoLifecycleState::Recycle,
            0,
            if matches!(from, FangyuanTiandaoLifecycleState::Recycle) {
                FangyuanTiandaoTransitionReason::AlreadyRecycled
            } else {
                FangyuanTiandaoTransitionReason::ExplicitRecycle
            },
        ),
        FangyuanTiandaoLifecycleInput::Solidify => {
            if manifestation.solidify_score >= rules.solidify_threshold {
                (
                    FangyuanTiandaoLifecycleState::Solidify,
                    ttl_before,
                    FangyuanTiandaoTransitionReason::ExplicitSolidify,
                )
            } else {
                (
                    FangyuanTiandaoLifecycleState::Decay,
                    ttl_before.saturating_sub(1),
                    FangyuanTiandaoTransitionReason::ScoreBelowDecayThreshold,
                )
            }
        }
        FangyuanTiandaoLifecycleInput::Tick => match from {
            FangyuanTiandaoLifecycleState::Manifest => step_manifest(manifestation, rules),
            FangyuanTiandaoLifecycleState::Decay => {
                let ttl_after = ttl_before.saturating_sub(1);
                if ttl_after <= rules.decay_ticks_before_recycle {
                    (
                        FangyuanTiandaoLifecycleState::Recycle,
                        0,
                        FangyuanTiandaoTransitionReason::DecayCompleted,
                    )
                } else {
                    (
                        FangyuanTiandaoLifecycleState::Decay,
                        ttl_after,
                        FangyuanTiandaoTransitionReason::ManifestTick,
                    )
                }
            }
            FangyuanTiandaoLifecycleState::Solidify => (
                FangyuanTiandaoLifecycleState::Solidify,
                ttl_before,
                FangyuanTiandaoTransitionReason::ScoreReachedSolidifyThreshold,
            ),
            FangyuanTiandaoLifecycleState::Recycle => (
                FangyuanTiandaoLifecycleState::Recycle,
                0,
                FangyuanTiandaoTransitionReason::AlreadyRecycled,
            ),
        },
    };

    manifestation.lifecycle_state = to;
    manifestation.ttl = ttl_after;

    let released_budget = if !matches!(from, FangyuanTiandaoLifecycleState::Recycle)
        && matches!(to, FangyuanTiandaoLifecycleState::Recycle)
    {
        manifestation.budget_cost
    } else {
        0
    };

    FangyuanTiandaoTransition {
        from,
        to,
        reason,
        ttl_before,
        ttl_after,
        released_budget,
    }
}

fn step_manifest(
    manifestation: &FangyuanTiandaoManifestation,
    rules: &FangyuanTiandaoLifecycleRules,
) -> (
    FangyuanTiandaoLifecycleState,
    u32,
    FangyuanTiandaoTransitionReason,
) {
    if manifestation.solidify_score >= rules.solidify_threshold {
        return (
            FangyuanTiandaoLifecycleState::Solidify,
            manifestation.ttl,
            FangyuanTiandaoTransitionReason::ScoreReachedSolidifyThreshold,
        );
    }
    if manifestation.solidify_score < rules.decay_threshold {
        return (
            FangyuanTiandaoLifecycleState::Decay,
            manifestation.ttl.saturating_sub(1),
            FangyuanTiandaoTransitionReason::ScoreBelowDecayThreshold,
        );
    }

    let ttl_after = manifestation.ttl.saturating_sub(1);
    if ttl_after == 0 {
        (
            FangyuanTiandaoLifecycleState::Decay,
            0,
            FangyuanTiandaoTransitionReason::TtlExpired,
        )
    } else {
        (
            FangyuanTiandaoLifecycleState::Manifest,
            ttl_after,
            FangyuanTiandaoTransitionReason::ManifestTick,
        )
    }
}

pub fn fangyuan_default_tiandao_manifestation() -> FangyuanTiandaoManifestation {
    FangyuanTiandaoManifestation::new(
        "tiandao.local_wind",
        FangyuanTiandaoCause::RegionPressure {
            reason: "local_budget_pressure".to_string(),
        },
        "home.default",
        8,
        30,
        0.42,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fangyuan_tiandao_manifestation_carries_cause_region_budget_ttl_and_score() {
        let manifestation = fangyuan_default_tiandao_manifestation();

        manifestation.validate().unwrap();
        assert_eq!(manifestation.id, "tiandao.local_wind");
        assert_eq!(manifestation.region, "home.default");
        assert_eq!(manifestation.budget_cost, 8);
        assert_eq!(
            manifestation.lifecycle_state,
            FangyuanTiandaoLifecycleState::Manifest
        );
        assert_eq!(manifestation.ttl, 30);
        assert_eq!(manifestation.solidify_score, 0.42);
        assert!(matches!(
            manifestation.cause,
            FangyuanTiandaoCause::RegionPressure { .. }
        ));
    }

    #[test]
    fn fangyuan_tiandao_manifest_tick_decrements_ttl_without_state_change() {
        let mut manifestation = fangyuan_default_tiandao_manifestation();
        let transition = manifestation.tick(&FangyuanTiandaoLifecycleRules::default());

        assert_eq!(transition.from, FangyuanTiandaoLifecycleState::Manifest);
        assert_eq!(transition.to, FangyuanTiandaoLifecycleState::Manifest);
        assert_eq!(
            transition.reason,
            FangyuanTiandaoTransitionReason::ManifestTick
        );
        assert_eq!(manifestation.ttl, 29);
        assert_eq!(transition.released_budget, 0);
    }

    #[test]
    fn fangyuan_tiandao_high_score_solidifies_and_keeps_budget() {
        let mut manifestation = fangyuan_default_tiandao_manifestation();
        manifestation.solidify_score = 0.9;

        let transition = manifestation.tick(&FangyuanTiandaoLifecycleRules::default());

        assert_eq!(transition.to, FangyuanTiandaoLifecycleState::Solidify);
        assert_eq!(
            transition.reason,
            FangyuanTiandaoTransitionReason::ScoreReachedSolidifyThreshold
        );
        assert_eq!(transition.released_budget, 0);
        assert_eq!(manifestation.ttl, 30);
    }

    #[test]
    fn fangyuan_tiandao_explicit_solidify_requires_threshold() {
        let mut manifestation = fangyuan_default_tiandao_manifestation();
        manifestation.solidify_score = 0.73;

        let transition = manifestation.solidify(&FangyuanTiandaoLifecycleRules::default());

        assert_eq!(transition.to, FangyuanTiandaoLifecycleState::Solidify);
        assert_eq!(
            transition.reason,
            FangyuanTiandaoTransitionReason::ExplicitSolidify
        );
    }

    #[test]
    fn fangyuan_tiandao_low_score_enters_decay_then_recycles_budget() {
        let mut manifestation = fangyuan_default_tiandao_manifestation();
        manifestation.solidify_score = 0.1;
        manifestation.ttl = 3;
        let rules = FangyuanTiandaoLifecycleRules::default();

        let decay = manifestation.tick(&rules);
        let recycle = manifestation.tick(&rules);
        let result = FangyuanTiandaoBudgetRecycleResult::from_transition(recycle);

        assert_eq!(decay.to, FangyuanTiandaoLifecycleState::Decay);
        assert_eq!(
            decay.reason,
            FangyuanTiandaoTransitionReason::ScoreBelowDecayThreshold
        );
        assert_eq!(recycle.to, FangyuanTiandaoLifecycleState::Recycle);
        assert_eq!(
            recycle.reason,
            FangyuanTiandaoTransitionReason::DecayCompleted
        );
        assert_eq!(result.released_budget, 8);
        assert!(result.recycled);
    }

    #[test]
    fn fangyuan_tiandao_ttl_expiry_decays_and_next_tick_recycles() {
        let mut manifestation = fangyuan_default_tiandao_manifestation();
        manifestation.ttl = 1;
        manifestation.solidify_score = 0.42;
        let rules = FangyuanTiandaoLifecycleRules::default();

        let decay = manifestation.tick(&rules);
        let recycle = manifestation.tick(&rules);

        assert_eq!(decay.to, FangyuanTiandaoLifecycleState::Decay);
        assert_eq!(decay.reason, FangyuanTiandaoTransitionReason::TtlExpired);
        assert_eq!(recycle.to, FangyuanTiandaoLifecycleState::Recycle);
        assert_eq!(recycle.released_budget, manifestation.budget_cost);
    }

    #[test]
    fn fangyuan_tiandao_explicit_recycle_releases_budget_once() {
        let mut manifestation = fangyuan_default_tiandao_manifestation();

        let first = manifestation.recycle();
        let second = manifestation.recycle();

        assert_eq!(first.to, FangyuanTiandaoLifecycleState::Recycle);
        assert_eq!(first.released_budget, 8);
        assert_eq!(second.released_budget, 0);
        assert_eq!(
            second.reason,
            FangyuanTiandaoTransitionReason::AlreadyRecycled
        );
    }

    #[test]
    fn fangyuan_tiandao_validation_rejects_invalid_score() {
        let mut manifestation = fangyuan_default_tiandao_manifestation();
        manifestation.solidify_score = 1.2;

        let error = manifestation.validate().unwrap_err();

        assert_eq!(error.code(), "tiandao_invalid_solidify_score");
    }
}
