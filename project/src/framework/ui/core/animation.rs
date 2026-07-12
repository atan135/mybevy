#![allow(dead_code)]

use std::fmt;

use bevy::prelude::*;
use serde::Serialize;

use crate::framework::ui::{
    core::UiFocusSystems,
    style::{UiTheme, theme::UiThemeSystems},
};

pub(crate) struct UiAnimationPlugin;

impl Plugin for UiAnimationPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<UiAnimationCommand>()
            .add_message::<UiAnimationEvent>()
            .init_resource::<UiMotionPolicy>()
            .init_resource::<UiAnimationThemeChangeState>()
            .configure_sets(
                Update,
                UiAnimationSystems::Tick
                    .after(UiThemeSystems::Refresh)
                    .after(UiFocusSystems::Visuals),
            )
            .add_systems(
                Update,
                (
                    cancel_ui_animations_on_theme_change,
                    handle_ui_animation_commands,
                    tick_ui_property_animations,
                    tick_ui_alpha_animations,
                )
                    .chain()
                    .in_set(UiAnimationSystems::Tick),
            );
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
pub(crate) enum UiAnimationSystems {
    Tick,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Resource, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UiMotionPolicy {
    #[default]
    Full,
    Reduced,
    Disabled,
}

impl UiMotionPolicy {
    const REDUCED_SPEED_MULTIPLIER: f32 = 4.0;

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Reduced => "reduced",
            Self::Disabled => "disabled",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiAnimationCompletion {
    KeepComponent,
    RemoveComponent,
    DespawnEntity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiAnimationEasing {
    Linear,
    EaseInCubic,
    EaseOutCubic,
    EaseInOutCubic,
}

impl UiAnimationEasing {
    pub(crate) fn sample(self, progress: f32) -> f32 {
        let progress = clamp_progress(progress);

        match self {
            UiAnimationEasing::Linear => progress,
            UiAnimationEasing::EaseInCubic => progress.powi(3),
            UiAnimationEasing::EaseOutCubic => 1.0 - (1.0 - progress).powi(3),
            UiAnimationEasing::EaseInOutCubic => {
                if progress < 0.5 {
                    4.0 * progress.powi(3)
                } else {
                    1.0 - (-2.0 * progress + 2.0).powi(3) / 2.0
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiAnimationState {
    Running,
    Finished,
}

#[derive(Clone, Copy, Debug, Component, PartialEq)]
pub(crate) struct UiAnimatedAlpha {
    pub from: f32,
    pub to: f32,
    pub duration_secs: f32,
    pub elapsed_secs: f32,
    pub easing: UiAnimationEasing,
    pub completion: UiAnimationCompletion,
    pub state: UiAnimationState,
}

impl UiAnimatedAlpha {
    pub(crate) fn new(from: f32, to: f32, duration_secs: f32) -> Self {
        Self {
            from: sanitize_alpha(from, 0.0),
            to: sanitize_alpha(to, 1.0),
            duration_secs: sanitize_non_negative(duration_secs),
            elapsed_secs: 0.0,
            easing: UiAnimationEasing::Linear,
            completion: UiAnimationCompletion::RemoveComponent,
            state: UiAnimationState::Running,
        }
    }

    pub(crate) fn fade_in(duration_secs: f32) -> Self {
        Self::new(0.0, 1.0, duration_secs)
    }

    pub(crate) fn fade_out(duration_secs: f32) -> Self {
        Self::new(1.0, 0.0, duration_secs)
    }

    pub(crate) fn with_easing(mut self, easing: UiAnimationEasing) -> Self {
        self.easing = easing;
        self
    }

    pub(crate) fn with_completion(mut self, completion: UiAnimationCompletion) -> Self {
        self.completion = completion;
        self
    }

    pub(crate) fn progress(self) -> f32 {
        animation_progress(self.elapsed_secs, self.duration_secs)
    }

    pub(crate) fn eased_progress(self) -> f32 {
        self.easing.sample(self.progress())
    }

    pub(crate) fn alpha(self) -> f32 {
        interpolate_alpha(self.from, self.to, self.eased_progress())
    }

    pub(crate) fn is_finished(self) -> bool {
        self.state == UiAnimationState::Finished || self.progress() >= 1.0
    }

    pub(crate) fn tick(&mut self, delta_secs: f32) -> UiAnimationState {
        if self.state == UiAnimationState::Finished {
            return UiAnimationState::Finished;
        }

        let delta_secs = if delta_secs.is_finite() {
            delta_secs.max(0.0)
        } else {
            0.0
        };
        self.elapsed_secs = (self.elapsed_secs + delta_secs).max(0.0);
        if self.progress() >= 1.0 {
            self.finish();
        }

        self.state
    }

    fn finish(&mut self) {
        self.elapsed_secs = self.elapsed_secs.max(self.duration_secs);
        self.state = UiAnimationState::Finished;
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct UiAnimationId(&'static str);

impl UiAnimationId {
    pub(crate) const fn new(value: &'static str) -> Self {
        Self(value)
    }

    pub(crate) const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for UiAnimationId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UiAnimationTarget {
    Alpha,
    TransformTranslation,
    LayoutPosition,
    LayoutSize,
    TransformScale,
    BackgroundColor,
    TextColor,
}

impl UiAnimationTarget {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Alpha => "alpha",
            Self::TransformTranslation => "transform_translation",
            Self::LayoutPosition => "layout_position",
            Self::LayoutSize => "layout_size",
            Self::TransformScale => "transform_scale",
            Self::BackgroundColor => "background_color",
            Self::TextColor => "text_color",
        }
    }

    const fn value_kind(self) -> UiAnimationValueKind {
        match self {
            Self::Alpha => UiAnimationValueKind::Scalar,
            Self::TransformTranslation
            | Self::LayoutPosition
            | Self::LayoutSize
            | Self::TransformScale => UiAnimationValueKind::Vector,
            Self::BackgroundColor | Self::TextColor => UiAnimationValueKind::Color,
        }
    }

    pub(crate) const fn causes_layout_reflow(self) -> bool {
        matches!(self, Self::LayoutPosition | Self::LayoutSize)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum UiAnimationValue {
    Scalar(f32),
    Vector(Vec2),
    Color(Color),
}

impl UiAnimationValue {
    const fn kind(self) -> UiAnimationValueKind {
        match self {
            Self::Scalar(_) => UiAnimationValueKind::Scalar,
            Self::Vector(_) => UiAnimationValueKind::Vector,
            Self::Color(_) => UiAnimationValueKind::Color,
        }
    }

    fn is_finite(self) -> bool {
        match self {
            Self::Scalar(value) => value.is_finite(),
            Self::Vector(value) => value.is_finite(),
            Self::Color(color) => {
                let linear = color.to_linear();
                linear.red.is_finite()
                    && linear.green.is_finite()
                    && linear.blue.is_finite()
                    && linear.alpha.is_finite()
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UiAnimationValueKind {
    Scalar,
    Vector,
    Color,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiAnimationDirection {
    Normal,
    Reverse,
    Alternate,
    AlternateReverse,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiAnimationRepeat {
    Once,
    Count(u32),
    Infinite,
}

impl UiAnimationRepeat {
    const fn iterations(self) -> Option<u32> {
        match self {
            Self::Once => Some(1),
            Self::Count(count) => Some(count),
            Self::Infinite => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiAnimationInterruption {
    UseDeclaredFrom,
    ContinueFromCurrent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiAnimationCancelBehavior {
    KeepCurrent,
    SnapToStart,
    SnapToEnd,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct UiAnimationSpec {
    pub id: UiAnimationId,
    pub target: UiAnimationTarget,
    pub from: UiAnimationValue,
    pub to: UiAnimationValue,
    pub duration_secs: f32,
    pub delay_secs: f32,
    pub easing: UiAnimationEasing,
    pub direction: UiAnimationDirection,
    pub repeat: UiAnimationRepeat,
    pub completion: UiAnimationCompletion,
}

impl UiAnimationSpec {
    pub(crate) const fn new(
        id: UiAnimationId,
        target: UiAnimationTarget,
        from: UiAnimationValue,
        to: UiAnimationValue,
        duration_secs: f32,
    ) -> Self {
        Self {
            id,
            target,
            from,
            to,
            duration_secs,
            delay_secs: 0.0,
            easing: UiAnimationEasing::Linear,
            direction: UiAnimationDirection::Normal,
            repeat: UiAnimationRepeat::Once,
            completion: UiAnimationCompletion::RemoveComponent,
        }
    }

    pub(crate) const fn alpha(id: UiAnimationId, from: f32, to: f32, duration_secs: f32) -> Self {
        Self::new(
            id,
            UiAnimationTarget::Alpha,
            UiAnimationValue::Scalar(from),
            UiAnimationValue::Scalar(to),
            duration_secs,
        )
    }

    pub(crate) const fn transform_translation(
        id: UiAnimationId,
        from: Vec2,
        to: Vec2,
        duration_secs: f32,
    ) -> Self {
        Self::new(
            id,
            UiAnimationTarget::TransformTranslation,
            UiAnimationValue::Vector(from),
            UiAnimationValue::Vector(to),
            duration_secs,
        )
    }

    pub(crate) const fn layout_position(
        id: UiAnimationId,
        from: Vec2,
        to: Vec2,
        duration_secs: f32,
    ) -> Self {
        Self::new(
            id,
            UiAnimationTarget::LayoutPosition,
            UiAnimationValue::Vector(from),
            UiAnimationValue::Vector(to),
            duration_secs,
        )
    }

    pub(crate) const fn layout_size(
        id: UiAnimationId,
        from: Vec2,
        to: Vec2,
        duration_secs: f32,
    ) -> Self {
        Self::new(
            id,
            UiAnimationTarget::LayoutSize,
            UiAnimationValue::Vector(from),
            UiAnimationValue::Vector(to),
            duration_secs,
        )
    }

    pub(crate) const fn transform_scale(
        id: UiAnimationId,
        from: Vec2,
        to: Vec2,
        duration_secs: f32,
    ) -> Self {
        Self::new(
            id,
            UiAnimationTarget::TransformScale,
            UiAnimationValue::Vector(from),
            UiAnimationValue::Vector(to),
            duration_secs,
        )
    }

    pub(crate) const fn background_color(
        id: UiAnimationId,
        from: Color,
        to: Color,
        duration_secs: f32,
    ) -> Self {
        Self::new(
            id,
            UiAnimationTarget::BackgroundColor,
            UiAnimationValue::Color(from),
            UiAnimationValue::Color(to),
            duration_secs,
        )
    }

    pub(crate) const fn text_color(
        id: UiAnimationId,
        from: Color,
        to: Color,
        duration_secs: f32,
    ) -> Self {
        Self::new(
            id,
            UiAnimationTarget::TextColor,
            UiAnimationValue::Color(from),
            UiAnimationValue::Color(to),
            duration_secs,
        )
    }

    pub(crate) const fn with_delay(mut self, delay_secs: f32) -> Self {
        self.delay_secs = delay_secs;
        self
    }

    pub(crate) const fn with_easing(mut self, easing: UiAnimationEasing) -> Self {
        self.easing = easing;
        self
    }

    pub(crate) const fn with_direction(mut self, direction: UiAnimationDirection) -> Self {
        self.direction = direction;
        self
    }

    pub(crate) const fn with_repeat(mut self, repeat: UiAnimationRepeat) -> Self {
        self.repeat = repeat;
        self
    }

    pub(crate) const fn with_completion(mut self, completion: UiAnimationCompletion) -> Self {
        self.completion = completion;
        self
    }

    pub(crate) fn validate(self) -> Result<(), UiAnimationError> {
        if self.id.as_str().trim().is_empty() {
            return Err(UiAnimationError::EmptyId);
        }
        if self.from.kind() != self.target.value_kind()
            || self.to.kind() != self.target.value_kind()
        {
            return Err(UiAnimationError::ValueTypeMismatch);
        }
        if !self.from.is_finite() || !self.to.is_finite() {
            return Err(UiAnimationError::NonFiniteValue);
        }
        if !self.duration_secs.is_finite() {
            return Err(UiAnimationError::NonFiniteDuration);
        }
        if self.duration_secs < 0.0 {
            return Err(UiAnimationError::NegativeDuration);
        }
        if !self.delay_secs.is_finite() {
            return Err(UiAnimationError::NonFiniteDelay);
        }
        if self.delay_secs < 0.0 {
            return Err(UiAnimationError::NegativeDelay);
        }
        if self.repeat == UiAnimationRepeat::Count(0) {
            return Err(UiAnimationError::ZeroRepeatCount);
        }
        if self.repeat == UiAnimationRepeat::Infinite && self.duration_secs <= f32::EPSILON {
            return Err(UiAnimationError::ZeroDurationInfiniteRepeat);
        }
        if let (
            UiAnimationTarget::Alpha,
            UiAnimationValue::Scalar(from),
            UiAnimationValue::Scalar(to),
        ) = (self.target, self.from, self.to)
            && (!(0.0..=1.0).contains(&from) || !(0.0..=1.0).contains(&to))
        {
            return Err(UiAnimationError::AlphaOutOfRange);
        }
        if self.target == UiAnimationTarget::LayoutSize
            && let (UiAnimationValue::Vector(from), UiAnimationValue::Vector(to)) =
                (self.from, self.to)
            && (from.min_element() < 0.0 || to.min_element() < 0.0)
        {
            return Err(UiAnimationError::NegativeLayoutSize);
        }
        if matches!(
            self.target,
            UiAnimationTarget::BackgroundColor | UiAnimationTarget::TextColor
        ) {
            let UiAnimationValue::Color(from) = self.from else {
                return Err(UiAnimationError::ValueTypeMismatch);
            };
            let UiAnimationValue::Color(to) = self.to else {
                return Err(UiAnimationError::ValueTypeMismatch);
            };
            if !(0.0..=1.0).contains(&from.to_linear().alpha)
                || !(0.0..=1.0).contains(&to.to_linear().alpha)
            {
                return Err(UiAnimationError::ColorAlphaOutOfRange);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiAnimationError {
    EmptyId,
    ValueTypeMismatch,
    NonFiniteValue,
    NonFiniteDuration,
    NegativeDuration,
    NonFiniteDelay,
    NegativeDelay,
    ZeroRepeatCount,
    ZeroDurationInfiniteRepeat,
    AlphaOutOfRange,
    ColorAlphaOutOfRange,
    NegativeLayoutSize,
    TargetEntityMissing,
    TargetComponentMissing,
    ConflictingTarget,
    ConflictingLegacyAlpha,
    CurrentValueUnavailable,
    NonFiniteSeekProgress,
}

impl UiAnimationError {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::EmptyId => "empty_id",
            Self::ValueTypeMismatch => "value_type_mismatch",
            Self::NonFiniteValue => "non_finite_value",
            Self::NonFiniteDuration => "non_finite_duration",
            Self::NegativeDuration => "negative_duration",
            Self::NonFiniteDelay => "non_finite_delay",
            Self::NegativeDelay => "negative_delay",
            Self::ZeroRepeatCount => "zero_repeat_count",
            Self::ZeroDurationInfiniteRepeat => "zero_duration_infinite_repeat",
            Self::AlphaOutOfRange => "alpha_out_of_range",
            Self::ColorAlphaOutOfRange => "color_alpha_out_of_range",
            Self::NegativeLayoutSize => "negative_layout_size",
            Self::TargetEntityMissing => "target_entity_missing",
            Self::TargetComponentMissing => "target_component_missing",
            Self::ConflictingTarget => "conflicting_target",
            Self::ConflictingLegacyAlpha => "conflicting_legacy_alpha",
            Self::CurrentValueUnavailable => "current_value_unavailable",
            Self::NonFiniteSeekProgress => "non_finite_seek_progress",
        }
    }
}

#[derive(Clone, Debug, Message)]
pub(crate) enum UiAnimationCommand {
    Start {
        entity: Entity,
        animation: UiAnimationSpec,
        interruption: UiAnimationInterruption,
    },
    Cancel {
        entity: Entity,
        target: Option<UiAnimationTarget>,
        behavior: UiAnimationCancelBehavior,
    },
    Seek {
        entity: Entity,
        target: Option<UiAnimationTarget>,
        progress: f32,
        pause: bool,
    },
    SetPaused {
        entity: Entity,
        target: Option<UiAnimationTarget>,
        paused: bool,
    },
}

impl UiAnimationCommand {
    pub(crate) const fn start(entity: Entity, animation: UiAnimationSpec) -> Self {
        Self::Start {
            entity,
            animation,
            interruption: UiAnimationInterruption::UseDeclaredFrom,
        }
    }

    pub(crate) const fn continue_from_current(entity: Entity, animation: UiAnimationSpec) -> Self {
        Self::Start {
            entity,
            animation,
            interruption: UiAnimationInterruption::ContinueFromCurrent,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiAnimationEventKind {
    Completed,
    Cancelled,
    Replaced,
    Rejected,
}

#[derive(Clone, Copy, Debug, Message, PartialEq)]
pub(crate) struct UiAnimationEvent {
    pub entity: Entity,
    pub id: UiAnimationId,
    pub target: UiAnimationTarget,
    pub kind: UiAnimationEventKind,
    pub error: Option<UiAnimationError>,
}

#[derive(Clone, Debug, Component, Default)]
pub(crate) struct UiAnimations {
    tracks: Vec<UiAnimationTrack>,
}

impl UiAnimations {
    pub(crate) fn try_from_specs(
        specs: impl IntoIterator<Item = UiAnimationSpec>,
    ) -> Result<Self, UiAnimationError> {
        let mut animations = Self::default();
        for spec in specs {
            spec.validate()?;
            if animations.tracks.iter().any(|track| {
                track.spec.target != spec.target
                    && animation_targets_conflict(track.spec.target, spec.target)
            }) {
                return Err(UiAnimationError::ConflictingTarget);
            }
            if let Some(index) = animations
                .tracks
                .iter()
                .position(|track| track.spec.target == spec.target)
            {
                animations.tracks[index] = UiAnimationTrack::new(spec);
            } else {
                animations.tracks.push(UiAnimationTrack::new(spec));
            }
        }
        Ok(animations)
    }

    pub(crate) fn try_from_spec(spec: UiAnimationSpec) -> Result<Self, UiAnimationError> {
        Self::try_from_specs([spec])
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }
}

#[derive(Clone, Debug)]
struct UiAnimationTrack {
    spec: UiAnimationSpec,
    elapsed_secs: f32,
    state: UiAnimationState,
    paused: bool,
    seek_progress: Option<f32>,
    pending_cancel: Option<UiAnimationCancelBehavior>,
    raw_progress: f32,
    eased_progress: f32,
    dirty: bool,
}

impl UiAnimationTrack {
    fn new(spec: UiAnimationSpec) -> Self {
        Self {
            spec,
            elapsed_secs: 0.0,
            state: UiAnimationState::Running,
            paused: false,
            seek_progress: None,
            pending_cancel: None,
            raw_progress: directed_progress(spec.direction, 0, 0.0),
            eased_progress: spec
                .easing
                .sample(directed_progress(spec.direction, 0, 0.0)),
            dirty: true,
        }
    }

    fn advance(&mut self, delta_secs: f32, policy: UiMotionPolicy) -> UiAnimationFrame {
        if self.state == UiAnimationState::Finished {
            return UiAnimationFrame {
                raw_progress: self.raw_progress,
                eased_progress: self.eased_progress,
                finished: true,
            };
        }

        if policy == UiMotionPolicy::Disabled {
            let iterations = self.spec.repeat.iterations().unwrap_or(1);
            let raw_progress = final_directed_progress(self.spec, iterations);
            self.state = UiAnimationState::Finished;
            return self.record_frame(raw_progress, true);
        }

        if policy == UiMotionPolicy::Reduced && self.spec.repeat == UiAnimationRepeat::Infinite {
            let raw_progress = final_directed_progress(self.spec, 1);
            self.state = UiAnimationState::Finished;
            return self.record_frame(raw_progress, true);
        }

        if let Some(progress) = self.seek_progress {
            let raw_progress = directed_progress(self.spec.direction, 0, progress);
            return self.record_frame(raw_progress, false);
        }

        if !self.paused {
            let delta_secs = if delta_secs.is_finite() {
                delta_secs.max(0.0)
            } else {
                0.0
            };
            let speed = if policy == UiMotionPolicy::Reduced {
                UiMotionPolicy::REDUCED_SPEED_MULTIPLIER
            } else {
                1.0
            };
            self.elapsed_secs = (self.elapsed_secs + delta_secs * speed).max(0.0);
        }

        let delay_secs = if policy == UiMotionPolicy::Reduced {
            0.0
        } else {
            self.spec.delay_secs
        };
        if self.elapsed_secs < delay_secs {
            return self.record_frame(directed_progress(self.spec.direction, 0, 0.0), false);
        }

        let active_elapsed = (self.elapsed_secs - delay_secs).max(0.0);
        let Some(iterations) = self.spec.repeat.iterations() else {
            let cycle = (active_elapsed / self.spec.duration_secs).floor() as u32;
            let cycle_progress =
                (active_elapsed % self.spec.duration_secs) / self.spec.duration_secs;
            return self.record_frame(
                directed_progress(self.spec.direction, cycle, cycle_progress),
                false,
            );
        };

        if self.spec.duration_secs <= f32::EPSILON {
            let raw_progress = final_directed_progress(self.spec, iterations);
            self.state = UiAnimationState::Finished;
            return self.record_frame(raw_progress, true);
        }

        let total_duration = self.spec.duration_secs * iterations as f32;
        if active_elapsed >= total_duration {
            let raw_progress = final_directed_progress(self.spec, iterations);
            self.state = UiAnimationState::Finished;
            return self.record_frame(raw_progress, true);
        }

        let cycle = (active_elapsed / self.spec.duration_secs).floor() as u32;
        let cycle_progress = (active_elapsed % self.spec.duration_secs) / self.spec.duration_secs;
        self.record_frame(
            directed_progress(self.spec.direction, cycle, cycle_progress),
            false,
        )
    }

    fn record_frame(&mut self, raw_progress: f32, finished: bool) -> UiAnimationFrame {
        self.raw_progress = clamp_progress(raw_progress);
        self.eased_progress = self.spec.easing.sample(self.raw_progress);
        UiAnimationFrame {
            raw_progress: self.raw_progress,
            eased_progress: self.eased_progress,
            finished,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct UiAnimationFrame {
    raw_progress: f32,
    eased_progress: f32,
    finished: bool,
}

#[derive(Clone, Debug, Component, PartialEq, Serialize)]
pub(crate) struct UiAnimationDebugSnapshot {
    pub policy: String,
    pub tracks: Vec<UiAnimationTrackDebugSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct UiAnimationTrackDebugSnapshot {
    pub id: String,
    pub target: String,
    pub state: String,
    pub raw_progress: f32,
    pub eased_progress: f32,
    pub paused: bool,
    pub causes_layout_reflow: bool,
}

#[derive(Default, Resource)]
struct UiAnimationThemeChangeState {
    observed_initial_theme: bool,
}

fn cancel_ui_animations_on_theme_change(
    mut commands: Commands,
    theme: Option<Res<UiTheme>>,
    mut state: ResMut<UiAnimationThemeChangeState>,
    mut animations: Query<(Entity, &mut UiAnimations)>,
    legacy_animations: Query<Entity, With<UiAnimatedAlpha>>,
) {
    let Some(theme) = theme else {
        return;
    };
    if !state.observed_initial_theme {
        state.observed_initial_theme = true;
        return;
    }
    if !theme.is_changed() {
        return;
    }

    for (entity, mut animations) in &mut animations {
        for track in &mut animations.tracks {
            if track.state == UiAnimationState::Running {
                track.pending_cancel = Some(UiAnimationCancelBehavior::KeepCurrent);
            }
        }
        if animations.tracks.is_empty() {
            commands.entity(entity).remove::<UiAnimations>();
        }
    }
    for entity in &legacy_animations {
        commands.entity(entity).remove::<UiAnimatedAlpha>();
    }
}

type UiAnimationTargetQuery<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static UiAnimations>,
        Has<Text>,
        Option<&'static BackgroundColor>,
        Option<&'static TextColor>,
        Option<&'static Node>,
        Option<&'static UiTransform>,
        Has<UiAnimatedAlpha>,
    ),
>;

fn handle_ui_animation_commands(
    mut commands: Commands,
    mut animation_commands: MessageReader<UiAnimationCommand>,
    mut animation_events: MessageWriter<UiAnimationEvent>,
    targets: UiAnimationTargetQuery,
) {
    let mut pending = Vec::<(Entity, UiAnimations)>::new();

    for command in animation_commands.read() {
        let entity = match command {
            UiAnimationCommand::Start { entity, .. }
            | UiAnimationCommand::Cancel { entity, .. }
            | UiAnimationCommand::Seek { entity, .. }
            | UiAnimationCommand::SetPaused { entity, .. } => *entity,
        };
        let Ok((existing, has_text, background, text_color, node, transform, has_legacy_alpha)) =
            targets.get(entity)
        else {
            if let UiAnimationCommand::Start { animation, .. } = command {
                write_rejected_event(
                    &mut animation_events,
                    entity,
                    *animation,
                    UiAnimationError::TargetEntityMissing,
                );
            }
            continue;
        };

        let pending_index = pending
            .iter()
            .position(|(pending_entity, _)| *pending_entity == entity)
            .unwrap_or_else(|| {
                pending.push((entity, existing.cloned().unwrap_or_default()));
                pending.len() - 1
            });
        let animations = &mut pending[pending_index].1;

        match command {
            UiAnimationCommand::Start {
                animation,
                interruption,
                ..
            } => {
                let mut animation = *animation;
                if let Err(error) = animation.validate() {
                    write_rejected_event(&mut animation_events, entity, animation, error);
                    continue;
                }
                if !target_is_available(
                    animation.target,
                    has_text,
                    background.is_some(),
                    text_color.is_some(),
                    node.is_some(),
                    transform.is_some(),
                ) {
                    write_rejected_event(
                        &mut animation_events,
                        entity,
                        animation,
                        UiAnimationError::TargetComponentMissing,
                    );
                    continue;
                }
                if has_legacy_alpha
                    && matches!(
                        animation.target,
                        UiAnimationTarget::Alpha
                            | UiAnimationTarget::BackgroundColor
                            | UiAnimationTarget::TextColor
                    )
                {
                    write_rejected_event(
                        &mut animation_events,
                        entity,
                        animation,
                        UiAnimationError::ConflictingLegacyAlpha,
                    );
                    continue;
                }
                if animations.tracks.iter().any(|track| {
                    track.spec.target != animation.target
                        && animation_targets_conflict(track.spec.target, animation.target)
                }) {
                    write_rejected_event(
                        &mut animation_events,
                        entity,
                        animation,
                        UiAnimationError::ConflictingTarget,
                    );
                    continue;
                }
                if *interruption == UiAnimationInterruption::ContinueFromCurrent {
                    let Some(current) = current_animation_value(
                        animation.target,
                        has_text,
                        background,
                        text_color,
                        node,
                        transform,
                    ) else {
                        write_rejected_event(
                            &mut animation_events,
                            entity,
                            animation,
                            UiAnimationError::CurrentValueUnavailable,
                        );
                        continue;
                    };
                    animation.from = current;
                }

                if let Some(index) = animations
                    .tracks
                    .iter()
                    .position(|track| track.spec.target == animation.target)
                {
                    let replaced = animations.tracks.remove(index);
                    animation_events.write(UiAnimationEvent {
                        entity,
                        id: replaced.spec.id,
                        target: replaced.spec.target,
                        kind: UiAnimationEventKind::Replaced,
                        error: None,
                    });
                }
                animations.tracks.push(UiAnimationTrack::new(animation));
            }
            UiAnimationCommand::Cancel {
                target, behavior, ..
            } => {
                for track in &mut animations.tracks {
                    if target.is_none_or(|target| target == track.spec.target) {
                        track.pending_cancel = Some(*behavior);
                    }
                }
            }
            UiAnimationCommand::Seek {
                target,
                progress,
                pause,
                ..
            } => {
                if !progress.is_finite() {
                    for track in &animations.tracks {
                        if target.is_none_or(|target| target == track.spec.target) {
                            write_rejected_event(
                                &mut animation_events,
                                entity,
                                track.spec,
                                UiAnimationError::NonFiniteSeekProgress,
                            );
                        }
                    }
                    continue;
                }
                for track in &mut animations.tracks {
                    if target.is_none_or(|target| target == track.spec.target) {
                        let progress = clamp_progress(*progress);
                        track.elapsed_secs =
                            track.spec.delay_secs + track.spec.duration_secs * progress;
                        track.seek_progress = pause.then_some(progress);
                        track.paused = *pause;
                        track.state = UiAnimationState::Running;
                        track.dirty = true;
                    }
                }
            }
            UiAnimationCommand::SetPaused { target, paused, .. } => {
                for track in &mut animations.tracks {
                    if target.is_none_or(|target| target == track.spec.target) {
                        if !paused && let Some(progress) = track.seek_progress {
                            track.elapsed_secs =
                                track.spec.delay_secs + track.spec.duration_secs * progress;
                        }
                        track.paused = *paused;
                        if !paused {
                            track.seek_progress = None;
                        }
                        track.dirty = true;
                    }
                }
            }
        }
    }

    for (entity, animations) in pending {
        if animations.is_empty() {
            commands.entity(entity).remove::<UiAnimations>();
        } else {
            commands.entity(entity).insert(animations);
        }
    }
}

fn tick_ui_property_animations(
    mut commands: Commands,
    time: Res<Time>,
    policy: Res<UiMotionPolicy>,
    mut animation_events: MessageWriter<UiAnimationEvent>,
    mut animations: Query<(
        Entity,
        &mut UiAnimations,
        Has<Text>,
        Option<&mut BackgroundColor>,
        Option<&mut TextColor>,
        Option<&mut Node>,
        Option<&mut UiTransform>,
        Option<&mut UiAnimationDebugSnapshot>,
        Has<UiAnimatedAlpha>,
    )>,
) {
    let delta_secs = time.delta_secs();

    for (
        entity,
        mut animations,
        has_text,
        mut background,
        mut text_color,
        mut node,
        mut transform,
        mut snapshot,
        has_legacy_alpha,
    ) in &mut animations
    {
        let snapshot_policy_changed = snapshot
            .as_deref()
            .is_none_or(|snapshot| snapshot.policy != policy.as_str());
        let has_track_work = animations.tracks.iter().any(|track| {
            track.pending_cancel.is_some()
                || (track.state == UiAnimationState::Running
                    && (track.dirty
                        || (!track.paused && track.seek_progress.is_none())
                        || *policy == UiMotionPolicy::Disabled
                        || (*policy == UiMotionPolicy::Reduced
                            && track.spec.repeat == UiAnimationRepeat::Infinite)))
        });
        if !has_track_work {
            if snapshot_policy_changed {
                let next_snapshot = animation_debug_snapshot(*policy, &animations.tracks);
                if let Some(snapshot) = snapshot.as_deref_mut() {
                    if *snapshot != next_snapshot {
                        *snapshot = next_snapshot;
                    }
                } else {
                    commands.entity(entity).insert(next_snapshot);
                }
            }
            continue;
        }

        let mut retained = Vec::with_capacity(animations.tracks.len());
        let mut despawn = false;

        for mut track in std::mem::take(&mut animations.tracks) {
            if has_legacy_alpha
                && matches!(
                    track.spec.target,
                    UiAnimationTarget::Alpha
                        | UiAnimationTarget::BackgroundColor
                        | UiAnimationTarget::TextColor
                )
            {
                write_rejected_event(
                    &mut animation_events,
                    entity,
                    track.spec,
                    UiAnimationError::ConflictingLegacyAlpha,
                );
                continue;
            }
            if !target_is_available(
                track.spec.target,
                has_text,
                background.is_some(),
                text_color.is_some(),
                node.is_some(),
                transform.is_some(),
            ) {
                write_rejected_event(
                    &mut animation_events,
                    entity,
                    track.spec,
                    UiAnimationError::TargetComponentMissing,
                );
                continue;
            }

            if let Some(cancel) = track.pending_cancel.take() {
                match cancel {
                    UiAnimationCancelBehavior::KeepCurrent => {}
                    UiAnimationCancelBehavior::SnapToStart => apply_animation_value(
                        track.spec.target,
                        track.spec.from,
                        has_text,
                        background.as_deref_mut(),
                        text_color.as_deref_mut(),
                        node.as_deref_mut(),
                        transform.as_deref_mut(),
                    ),
                    UiAnimationCancelBehavior::SnapToEnd => {
                        let iterations = track.spec.repeat.iterations().unwrap_or(1);
                        let progress = final_directed_progress(track.spec, iterations);
                        apply_animation_value(
                            track.spec.target,
                            interpolate_value(track.spec.from, track.spec.to, progress),
                            has_text,
                            background.as_deref_mut(),
                            text_color.as_deref_mut(),
                            node.as_deref_mut(),
                            transform.as_deref_mut(),
                        );
                    }
                }
                animation_events.write(UiAnimationEvent {
                    entity,
                    id: track.spec.id,
                    target: track.spec.target,
                    kind: UiAnimationEventKind::Cancelled,
                    error: None,
                });
                continue;
            }

            if track.state == UiAnimationState::Finished {
                retained.push(track);
                continue;
            }

            let frame = track.advance(delta_secs, *policy);
            apply_animation_value(
                track.spec.target,
                interpolate_value(track.spec.from, track.spec.to, frame.eased_progress),
                has_text,
                background.as_deref_mut(),
                text_color.as_deref_mut(),
                node.as_deref_mut(),
                transform.as_deref_mut(),
            );
            track.dirty = false;

            if frame.finished {
                animation_events.write(UiAnimationEvent {
                    entity,
                    id: track.spec.id,
                    target: track.spec.target,
                    kind: UiAnimationEventKind::Completed,
                    error: None,
                });
                match track.spec.completion {
                    UiAnimationCompletion::KeepComponent => retained.push(track),
                    UiAnimationCompletion::RemoveComponent => {}
                    UiAnimationCompletion::DespawnEntity => {
                        despawn = true;
                        break;
                    }
                }
            } else {
                retained.push(track);
            }
        }

        if despawn {
            commands.entity(entity).try_despawn();
            continue;
        }

        animations.tracks = retained;
        if animations.tracks.is_empty() {
            commands
                .entity(entity)
                .remove::<(UiAnimations, UiAnimationDebugSnapshot)>();
        } else {
            let next_snapshot = animation_debug_snapshot(*policy, &animations.tracks);
            if let Some(snapshot) = snapshot.as_deref_mut() {
                if *snapshot != next_snapshot {
                    *snapshot = next_snapshot;
                }
            } else {
                commands.entity(entity).insert(next_snapshot);
            }
        }
    }
}

fn tick_ui_alpha_animations(
    mut commands: Commands,
    time: Res<Time>,
    policy: Res<UiMotionPolicy>,
    mut animations: Query<(
        Entity,
        &mut UiAnimatedAlpha,
        Has<Text>,
        Option<&mut BackgroundColor>,
        Option<&mut TextColor>,
    )>,
) {
    let delta_secs = time.delta_secs();

    for (entity, mut animation, has_text, background, text_color) in &mut animations {
        if animation.state == UiAnimationState::Finished {
            continue;
        }

        let state = match *policy {
            UiMotionPolicy::Full => animation.tick(delta_secs),
            UiMotionPolicy::Reduced => {
                animation.tick(delta_secs * UiMotionPolicy::REDUCED_SPEED_MULTIPLIER)
            }
            UiMotionPolicy::Disabled => {
                animation.finish();
                UiAnimationState::Finished
            }
        };
        let alpha = animation.alpha();
        apply_alpha(background, text_color, has_text, alpha);

        if state == UiAnimationState::Finished {
            match animation.completion {
                UiAnimationCompletion::KeepComponent => {}
                UiAnimationCompletion::RemoveComponent => {
                    commands.entity(entity).remove::<UiAnimatedAlpha>();
                }
                UiAnimationCompletion::DespawnEntity => {
                    commands.entity(entity).try_despawn();
                }
            }
        }
    }
}

fn write_rejected_event(
    events: &mut MessageWriter<UiAnimationEvent>,
    entity: Entity,
    spec: UiAnimationSpec,
    error: UiAnimationError,
) {
    events.write(UiAnimationEvent {
        entity,
        id: spec.id,
        target: spec.target,
        kind: UiAnimationEventKind::Rejected,
        error: Some(error),
    });
}

fn target_is_available(
    target: UiAnimationTarget,
    has_text: bool,
    has_background: bool,
    has_text_color: bool,
    has_node: bool,
    has_transform: bool,
) -> bool {
    match target {
        UiAnimationTarget::Alpha => {
            (has_text && has_text_color) || (!has_text && has_background) || has_text_color
        }
        UiAnimationTarget::TransformTranslation | UiAnimationTarget::TransformScale => {
            has_transform
        }
        UiAnimationTarget::LayoutPosition | UiAnimationTarget::LayoutSize => has_node,
        UiAnimationTarget::BackgroundColor => has_background,
        UiAnimationTarget::TextColor => has_text_color,
    }
}

fn animation_targets_conflict(left: UiAnimationTarget, right: UiAnimationTarget) -> bool {
    if left == right {
        return true;
    }
    matches!(
        (left, right),
        (
            UiAnimationTarget::Alpha,
            UiAnimationTarget::BackgroundColor | UiAnimationTarget::TextColor
        ) | (
            UiAnimationTarget::BackgroundColor | UiAnimationTarget::TextColor,
            UiAnimationTarget::Alpha
        )
    )
}

fn current_animation_value(
    target: UiAnimationTarget,
    has_text: bool,
    background: Option<&BackgroundColor>,
    text_color: Option<&TextColor>,
    node: Option<&Node>,
    transform: Option<&UiTransform>,
) -> Option<UiAnimationValue> {
    match target {
        UiAnimationTarget::Alpha if has_text => {
            text_color.map(|color| UiAnimationValue::Scalar(color.0.to_linear().alpha))
        }
        UiAnimationTarget::Alpha => background
            .map(|color| UiAnimationValue::Scalar(color.0.to_linear().alpha))
            .or_else(|| {
                text_color.map(|color| UiAnimationValue::Scalar(color.0.to_linear().alpha))
            }),
        UiAnimationTarget::TransformTranslation => transform.and_then(|transform| {
            match (transform.translation.x, transform.translation.y) {
                (Val::Px(x), Val::Px(y)) => Some(UiAnimationValue::Vector(Vec2::new(x, y))),
                _ => None,
            }
        }),
        UiAnimationTarget::LayoutPosition => node.and_then(|node| match (node.left, node.top) {
            (Val::Px(x), Val::Px(y)) => Some(UiAnimationValue::Vector(Vec2::new(x, y))),
            _ => None,
        }),
        UiAnimationTarget::LayoutSize => node.and_then(|node| match (node.width, node.height) {
            (Val::Px(width), Val::Px(height)) => {
                Some(UiAnimationValue::Vector(Vec2::new(width, height)))
            }
            _ => None,
        }),
        UiAnimationTarget::TransformScale => {
            transform.map(|transform| UiAnimationValue::Vector(transform.scale))
        }
        UiAnimationTarget::BackgroundColor => {
            background.map(|color| UiAnimationValue::Color(color.0))
        }
        UiAnimationTarget::TextColor => text_color.map(|color| UiAnimationValue::Color(color.0)),
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_animation_value(
    target: UiAnimationTarget,
    value: UiAnimationValue,
    has_text: bool,
    background: Option<&mut BackgroundColor>,
    text_color: Option<&mut TextColor>,
    node: Option<&mut Node>,
    transform: Option<&mut UiTransform>,
) {
    match (target, value) {
        (UiAnimationTarget::Alpha, UiAnimationValue::Scalar(alpha)) => {
            apply_alpha_refs(background, text_color, has_text, alpha);
        }
        (UiAnimationTarget::TransformTranslation, UiAnimationValue::Vector(translation)) => {
            if let Some(transform) = transform {
                let next = Val2::px(translation.x, translation.y);
                if transform.translation != next {
                    transform.translation = next;
                }
            }
        }
        (UiAnimationTarget::LayoutPosition, UiAnimationValue::Vector(position)) => {
            if let Some(node) = node {
                set_if_changed(&mut node.left, px(position.x));
                set_if_changed(&mut node.top, px(position.y));
            }
        }
        (UiAnimationTarget::LayoutSize, UiAnimationValue::Vector(size)) => {
            if let Some(node) = node {
                set_if_changed(&mut node.width, px(size.x));
                set_if_changed(&mut node.height, px(size.y));
            }
        }
        (UiAnimationTarget::TransformScale, UiAnimationValue::Vector(scale)) => {
            if let Some(transform) = transform
                && transform.scale != scale
            {
                transform.scale = scale;
            }
        }
        (UiAnimationTarget::BackgroundColor, UiAnimationValue::Color(color)) => {
            if let Some(background) = background {
                let next = BackgroundColor(color);
                if *background != next {
                    *background = next;
                }
            }
        }
        (UiAnimationTarget::TextColor, UiAnimationValue::Color(color)) => {
            if let Some(text_color) = text_color {
                let next = TextColor(color);
                if *text_color != next {
                    *text_color = next;
                }
            }
        }
        _ => {}
    }
}

fn set_if_changed<T: PartialEq>(current: &mut T, next: T) {
    if *current != next {
        *current = next;
    }
}

fn interpolate_value(
    from: UiAnimationValue,
    to: UiAnimationValue,
    progress: f32,
) -> UiAnimationValue {
    let progress = clamp_progress(progress);
    match (from, to) {
        (UiAnimationValue::Scalar(from), UiAnimationValue::Scalar(to)) => {
            UiAnimationValue::Scalar(from + (to - from) * progress)
        }
        (UiAnimationValue::Vector(from), UiAnimationValue::Vector(to)) => {
            UiAnimationValue::Vector(from.lerp(to, progress))
        }
        (UiAnimationValue::Color(from), UiAnimationValue::Color(to)) => {
            let from = from.to_linear();
            let to = to.to_linear();
            UiAnimationValue::Color(Color::linear_rgba(
                from.red + (to.red - from.red) * progress,
                from.green + (to.green - from.green) * progress,
                from.blue + (to.blue - from.blue) * progress,
                from.alpha + (to.alpha - from.alpha) * progress,
            ))
        }
        _ => from,
    }
}

fn directed_progress(
    direction: UiAnimationDirection,
    zero_based_iteration: u32,
    progress: f32,
) -> f32 {
    let progress = clamp_progress(progress);
    let reverse = match direction {
        UiAnimationDirection::Normal => false,
        UiAnimationDirection::Reverse => true,
        UiAnimationDirection::Alternate => zero_based_iteration % 2 == 1,
        UiAnimationDirection::AlternateReverse => zero_based_iteration % 2 == 0,
    };
    if reverse { 1.0 - progress } else { progress }
}

fn final_directed_progress(spec: UiAnimationSpec, iterations: u32) -> f32 {
    directed_progress(spec.direction, iterations.saturating_sub(1), 1.0)
}

fn animation_debug_snapshot(
    policy: UiMotionPolicy,
    tracks: &[UiAnimationTrack],
) -> UiAnimationDebugSnapshot {
    let mut tracks = tracks
        .iter()
        .map(|track| UiAnimationTrackDebugSnapshot {
            id: track.spec.id.as_str().to_owned(),
            target: track.spec.target.as_str().to_owned(),
            state: match track.state {
                UiAnimationState::Running => "running",
                UiAnimationState::Finished => "finished",
            }
            .to_owned(),
            raw_progress: track.raw_progress,
            eased_progress: track.eased_progress,
            paused: track.paused || track.seek_progress.is_some(),
            causes_layout_reflow: track.spec.target.causes_layout_reflow(),
        })
        .collect::<Vec<_>>();
    tracks.sort_by(|left, right| {
        left.target
            .cmp(&right.target)
            .then_with(|| left.id.cmp(&right.id))
    });
    UiAnimationDebugSnapshot {
        policy: policy.as_str().to_owned(),
        tracks,
    }
}

fn apply_alpha(
    background: Option<Mut<BackgroundColor>>,
    text_color: Option<Mut<TextColor>>,
    has_text: bool,
    alpha: f32,
) {
    apply_alpha_refs(
        background.map(Mut::into_inner),
        text_color.map(Mut::into_inner),
        has_text,
        alpha,
    );
}

fn apply_alpha_refs(
    background: Option<&mut BackgroundColor>,
    text_color: Option<&mut TextColor>,
    has_text: bool,
    alpha: f32,
) {
    let alpha = sanitize_alpha(alpha, 0.0);
    if let Some(background) = background
        && !has_text
    {
        let next_color = color_with_alpha(background.0, alpha);
        if background.0 != next_color {
            *background = BackgroundColor(next_color);
        }
    }

    if let Some(text_color) = text_color {
        let next_color = color_with_alpha(text_color.0, alpha);
        if text_color.0 != next_color {
            *text_color = TextColor(next_color);
        }
    }
}

fn color_with_alpha(color: Color, alpha: f32) -> Color {
    color.with_alpha(sanitize_alpha(alpha, 0.0))
}

fn animation_progress(elapsed_secs: f32, duration_secs: f32) -> f32 {
    let elapsed_secs = sanitize_non_negative(elapsed_secs);
    let duration_secs = sanitize_non_negative(duration_secs);

    if duration_secs <= f32::EPSILON {
        1.0
    } else {
        (elapsed_secs / duration_secs).clamp(0.0, 1.0)
    }
}

fn clamp_progress(progress: f32) -> f32 {
    if progress.is_finite() {
        progress.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn interpolate_alpha(from: f32, to: f32, progress: f32) -> f32 {
    let progress = clamp_progress(progress);
    (sanitize_alpha(from, 0.0) + (sanitize_alpha(to, 1.0) - sanitize_alpha(from, 0.0)) * progress)
        .clamp(0.0, 1.0)
}

fn sanitize_alpha(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        fallback.clamp(0.0, 1.0)
    }
}

fn sanitize_non_negative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::message::MessageCursor;

    const EPSILON: f32 = 0.0001;
    const TEST_ANIMATION: UiAnimationId = UiAnimationId::new("test.animation");

    fn animation_test_app() -> App {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .add_plugins(UiAnimationPlugin);
        app
    }

    fn assert_approx_eq(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= EPSILON,
            "expected {actual} to be approximately {expected}"
        );
    }

    fn alpha(color: Color) -> f32 {
        color.to_linear().alpha
    }

    fn events(app: &App) -> Vec<UiAnimationEvent> {
        let messages = app.world().resource::<Messages<UiAnimationEvent>>();
        let mut cursor = MessageCursor::default();
        cursor.read(messages).copied().collect()
    }

    fn send(app: &mut App, command: UiAnimationCommand) {
        app.world_mut()
            .resource_mut::<Messages<UiAnimationCommand>>()
            .write(command);
    }

    fn advance(app: &mut App, seconds: f32) {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(seconds));
        app.update();
    }

    #[test]
    fn easing_samples_are_clamped_and_stable() {
        assert_approx_eq(UiAnimationEasing::Linear.sample(-0.5), 0.0);
        assert_approx_eq(UiAnimationEasing::Linear.sample(f32::NAN), 0.0);
        assert_approx_eq(UiAnimationEasing::EaseInCubic.sample(0.5), 0.125);
        assert_approx_eq(UiAnimationEasing::EaseOutCubic.sample(0.5), 0.875);
        assert_approx_eq(UiAnimationEasing::EaseInOutCubic.sample(0.25), 0.0625);
        assert_approx_eq(UiAnimationEasing::EaseInOutCubic.sample(0.75), 0.9375);
    }

    #[test]
    fn progress_clamps_elapsed_and_zero_duration() {
        assert_approx_eq(animation_progress(-1.0, 2.0), 0.0);
        assert_approx_eq(animation_progress(1.0, 2.0), 0.5);
        assert_approx_eq(animation_progress(3.0, 2.0), 1.0);
        assert_approx_eq(animation_progress(0.0, 0.0), 1.0);
        assert_approx_eq(animation_progress(f32::NAN, f32::NAN), 1.0);
    }

    #[test]
    fn legacy_alpha_sanitizes_non_finite_values_and_keeps_semitransparent_target() {
        let invalid = UiAnimatedAlpha::new(f32::NAN, f32::INFINITY, f32::NAN);
        assert_eq!(invalid.from, 0.0);
        assert_eq!(invalid.to, 1.0);
        assert_eq!(invalid.duration_secs, 0.0);

        let mut overlay = UiAnimatedAlpha::new(0.0, 0.56, 1.0);
        overlay.tick(1.0);
        assert_approx_eq(overlay.alpha(), 0.56);
    }

    #[test]
    fn spec_validation_returns_stable_reasons() {
        let mismatched = UiAnimationSpec::new(
            TEST_ANIMATION,
            UiAnimationTarget::TransformScale,
            UiAnimationValue::Scalar(0.0),
            UiAnimationValue::Scalar(1.0),
            1.0,
        );
        assert_eq!(
            mismatched.validate(),
            Err(UiAnimationError::ValueTypeMismatch)
        );
        assert_eq!(
            UiAnimationSpec::transform_scale(
                TEST_ANIMATION,
                Vec2::ZERO,
                Vec2::splat(f32::NAN),
                1.0
            )
            .validate(),
            Err(UiAnimationError::NonFiniteValue)
        );
        assert_eq!(
            UiAnimationSpec::alpha(TEST_ANIMATION, 0.0, 1.0, f32::NAN).validate(),
            Err(UiAnimationError::NonFiniteDuration)
        );
        assert_eq!(
            UiAnimationSpec::alpha(TEST_ANIMATION, 0.0, 1.0, 1.0)
                .with_delay(f32::INFINITY)
                .validate(),
            Err(UiAnimationError::NonFiniteDelay)
        );
        assert_eq!(
            UiAnimationSpec::alpha(TEST_ANIMATION, 0.0, 1.0, 1.0)
                .with_repeat(UiAnimationRepeat::Count(0))
                .validate(),
            Err(UiAnimationError::ZeroRepeatCount)
        );
        assert_eq!(
            UiAnimationSpec::alpha(TEST_ANIMATION, 0.0, 1.0, 0.0)
                .with_repeat(UiAnimationRepeat::Infinite)
                .validate(),
            Err(UiAnimationError::ZeroDurationInfiniteRepeat)
        );
        assert_eq!(
            UiAnimationSpec::layout_size(TEST_ANIMATION, Vec2::ZERO, Vec2::new(-1.0, 2.0), 1.0)
                .validate(),
            Err(UiAnimationError::NegativeLayoutSize)
        );
        assert_eq!(
            UiAnimationError::NonFiniteValue.as_str(),
            "non_finite_value"
        );
    }

    #[test]
    fn overlapping_visual_channels_are_rejected_instead_of_using_write_order() {
        let conflict = UiAnimations::try_from_specs([
            UiAnimationSpec::alpha(UiAnimationId::new("alpha"), 0.0, 1.0, 1.0),
            UiAnimationSpec::background_color(
                UiAnimationId::new("color"),
                Color::BLACK,
                Color::WHITE,
                1.0,
            ),
        ]);
        assert_eq!(conflict.unwrap_err(), UiAnimationError::ConflictingTarget);

        let mut app = animation_test_app();
        let entity = app
            .world_mut()
            .spawn((BackgroundColor(Color::BLACK), UiAnimatedAlpha::fade_in(1.0)))
            .id();
        send(
            &mut app,
            UiAnimationCommand::start(
                entity,
                UiAnimationSpec::background_color(TEST_ANIMATION, Color::BLACK, Color::WHITE, 1.0),
            ),
        );
        app.update();
        assert!(events(&app).iter().any(|event| {
            event.kind == UiAnimationEventKind::Rejected
                && event.error == Some(UiAnimationError::ConflictingLegacyAlpha)
        }));
        assert!(app.world().get::<UiAnimations>(entity).is_none());
    }

    #[test]
    fn direction_and_repeat_choose_actual_final_endpoint() {
        let alternate_two = UiAnimationSpec::alpha(TEST_ANIMATION, 0.2, 0.8, 1.0)
            .with_direction(UiAnimationDirection::Alternate)
            .with_repeat(UiAnimationRepeat::Count(2));
        let alternate_three = alternate_two.with_repeat(UiAnimationRepeat::Count(3));
        let reverse = alternate_two
            .with_direction(UiAnimationDirection::Reverse)
            .with_repeat(UiAnimationRepeat::Once);
        let alternate_reverse_two = alternate_two
            .with_direction(UiAnimationDirection::AlternateReverse)
            .with_repeat(UiAnimationRepeat::Count(2));

        assert_approx_eq(final_directed_progress(alternate_two, 2), 0.0);
        assert_approx_eq(final_directed_progress(alternate_three, 3), 1.0);
        assert_approx_eq(final_directed_progress(reverse, 1), 0.0);
        assert_approx_eq(final_directed_progress(alternate_reverse_two, 2), 1.0);
    }

    #[test]
    fn one_entity_runs_different_targets_in_parallel_and_replaces_same_target() {
        let mut app = animation_test_app();
        let entity = app
            .world_mut()
            .spawn((BackgroundColor(Color::BLACK), UiTransform::default()))
            .id();
        send(
            &mut app,
            UiAnimationCommand::start(
                entity,
                UiAnimationSpec::transform_scale(
                    UiAnimationId::new("scale.first"),
                    Vec2::ONE,
                    Vec2::splat(2.0),
                    1.0,
                ),
            ),
        );
        send(
            &mut app,
            UiAnimationCommand::start(
                entity,
                UiAnimationSpec::background_color(
                    UiAnimationId::new("color"),
                    Color::BLACK,
                    Color::WHITE,
                    1.0,
                ),
            ),
        );
        send(
            &mut app,
            UiAnimationCommand::start(
                entity,
                UiAnimationSpec::transform_scale(
                    UiAnimationId::new("scale.second"),
                    Vec2::splat(0.5),
                    Vec2::splat(1.5),
                    1.0,
                ),
            ),
        );

        app.update();
        let animations = app.world().get::<UiAnimations>(entity).unwrap();
        assert_eq!(animations.tracks.len(), 2);
        assert!(
            animations
                .tracks
                .iter()
                .any(|track| track.spec.id == UiAnimationId::new("scale.second"))
        );
        assert!(events(&app).iter().any(|event| {
            event.id == UiAnimationId::new("scale.first")
                && event.kind == UiAnimationEventKind::Replaced
        }));
    }

    #[test]
    fn property_animation_applies_alpha_transform_layout_size_and_colors() {
        let mut app = animation_test_app();
        let alpha_entity = app
            .world_mut()
            .spawn(BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.2)))
            .id();
        let entity = app
            .world_mut()
            .spawn((
                Node {
                    width: px(10.0),
                    height: px(20.0),
                    left: px(0.0),
                    top: px(0.0),
                    ..default()
                },
                UiTransform::default(),
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.2)),
            ))
            .id();
        let specs = [
            UiAnimationSpec::transform_translation(
                UiAnimationId::new("translation"),
                Vec2::ZERO,
                Vec2::new(20.0, 40.0),
                1.0,
            ),
            UiAnimationSpec::layout_size(
                UiAnimationId::new("size"),
                Vec2::new(10.0, 20.0),
                Vec2::new(30.0, 60.0),
                1.0,
            ),
            UiAnimationSpec::background_color(
                UiAnimationId::new("color"),
                Color::linear_rgb(0.0, 0.0, 0.0),
                Color::linear_rgb(1.0, 0.0, 0.0),
                1.0,
            ),
        ];
        for spec in specs {
            send(&mut app, UiAnimationCommand::start(entity, spec));
        }
        send(
            &mut app,
            UiAnimationCommand::start(
                alpha_entity,
                UiAnimationSpec::alpha(UiAnimationId::new("alpha"), 0.2, 0.6, 1.0),
            ),
        );
        advance(&mut app, 0.5);

        let node = app.world().get::<Node>(entity).unwrap();
        assert_eq!(node.width, px(20.0));
        assert_eq!(node.height, px(40.0));
        let transform = app.world().get::<UiTransform>(entity).unwrap();
        assert_eq!(transform.translation, Val2::px(10.0, 20.0));
        let background = app.world().get::<BackgroundColor>(entity).unwrap();
        assert_approx_eq(background.0.to_linear().red, 0.5);
        assert_approx_eq(alpha(background.0), 1.0);
        assert_approx_eq(
            alpha(app.world().get::<BackgroundColor>(alpha_entity).unwrap().0),
            0.4,
        );
    }

    #[test]
    fn delay_zero_duration_and_completion_event_are_deterministic() {
        let mut app = animation_test_app();
        let entity = app.world_mut().spawn(BackgroundColor(Color::BLACK)).id();
        send(
            &mut app,
            UiAnimationCommand::start(
                entity,
                UiAnimationSpec::alpha(TEST_ANIMATION, 0.0, 0.42, 0.0).with_delay(0.5),
            ),
        );
        advance(&mut app, 0.25);
        assert_approx_eq(
            alpha(app.world().get::<BackgroundColor>(entity).unwrap().0),
            0.0,
        );
        advance(&mut app, 0.25);
        assert_approx_eq(
            alpha(app.world().get::<BackgroundColor>(entity).unwrap().0),
            0.42,
        );
        assert!(app.world().get::<UiAnimations>(entity).is_none());
        assert!(events(&app).iter().any(|event| {
            event.id == TEST_ANIMATION && event.kind == UiAnimationEventKind::Completed
        }));
    }

    #[test]
    fn continue_from_current_avoids_interruption_jump_and_declared_mode_is_explicit() {
        let mut app = animation_test_app();
        let entity = app
            .world_mut()
            .spawn(UiTransform::from_scale(Vec2::splat(1.4)))
            .id();
        send(
            &mut app,
            UiAnimationCommand::continue_from_current(
                entity,
                UiAnimationSpec::transform_scale(TEST_ANIMATION, Vec2::ONE, Vec2::splat(2.0), 1.0),
            ),
        );
        app.update();
        assert_eq!(
            app.world().get::<UiTransform>(entity).unwrap().scale,
            Vec2::splat(1.4)
        );
        let track = &app.world().get::<UiAnimations>(entity).unwrap().tracks[0];
        assert_eq!(track.spec.from, UiAnimationValue::Vector(Vec2::splat(1.4)));
    }

    #[test]
    fn continue_from_current_rejects_non_pixel_layout_values() {
        let mut app = animation_test_app();
        let entity = app
            .world_mut()
            .spawn(Node {
                left: percent(10.0),
                top: Val::Auto,
                ..default()
            })
            .id();
        send(
            &mut app,
            UiAnimationCommand::continue_from_current(
                entity,
                UiAnimationSpec::layout_position(
                    TEST_ANIMATION,
                    Vec2::ZERO,
                    Vec2::splat(20.0),
                    1.0,
                ),
            ),
        );
        app.update();
        assert!(events(&app).iter().any(|event| {
            event.kind == UiAnimationEventKind::Rejected
                && event.error == Some(UiAnimationError::CurrentValueUnavailable)
        }));
        assert!(app.world().get::<UiAnimations>(entity).is_none());
        assert_eq!(app.world().get::<Node>(entity).unwrap().left, percent(10.0));
    }

    #[test]
    fn cancel_keep_current_and_snap_to_end_emit_cancelled() {
        let mut app = animation_test_app();
        let keep = app
            .world_mut()
            .spawn(UiTransform::from_scale(Vec2::ONE))
            .id();
        send(
            &mut app,
            UiAnimationCommand::start(
                keep,
                UiAnimationSpec::transform_scale(
                    UiAnimationId::new("keep"),
                    Vec2::ONE,
                    Vec2::splat(2.0),
                    1.0,
                ),
            ),
        );
        advance(&mut app, 0.5);
        send(
            &mut app,
            UiAnimationCommand::Cancel {
                entity: keep,
                target: None,
                behavior: UiAnimationCancelBehavior::KeepCurrent,
            },
        );
        app.update();
        assert_eq!(
            app.world().get::<UiTransform>(keep).unwrap().scale,
            Vec2::splat(1.5)
        );
        assert!(events(&app).iter().any(|event| {
            event.id == UiAnimationId::new("keep") && event.kind == UiAnimationEventKind::Cancelled
        }));

        let snap = app
            .world_mut()
            .spawn(UiTransform::from_scale(Vec2::ONE))
            .id();
        let reverse = UiAnimationSpec::transform_scale(
            UiAnimationId::new("snap"),
            Vec2::ONE,
            Vec2::splat(2.0),
            1.0,
        )
        .with_direction(UiAnimationDirection::Alternate)
        .with_repeat(UiAnimationRepeat::Count(2));
        send(&mut app, UiAnimationCommand::start(snap, reverse));
        app.update();
        send(
            &mut app,
            UiAnimationCommand::Cancel {
                entity: snap,
                target: None,
                behavior: UiAnimationCancelBehavior::SnapToEnd,
            },
        );
        app.update();
        assert_eq!(
            app.world().get::<UiTransform>(snap).unwrap().scale,
            Vec2::ONE
        );
        assert!(events(&app).iter().any(|event| {
            event.id == UiAnimationId::new("snap") && event.kind == UiAnimationEventKind::Cancelled
        }));
    }

    #[test]
    fn disabled_and_reduced_motion_use_directed_endpoints_and_clean_up() {
        let mut app = animation_test_app();
        *app.world_mut().resource_mut::<UiMotionPolicy>() = UiMotionPolicy::Disabled;
        let reverse = app.world_mut().spawn(BackgroundColor(Color::WHITE)).id();
        send(
            &mut app,
            UiAnimationCommand::start(
                reverse,
                UiAnimationSpec::alpha(TEST_ANIMATION, 0.2, 0.8, 10.0)
                    .with_delay(5.0)
                    .with_direction(UiAnimationDirection::Reverse),
            ),
        );
        app.update();
        assert_approx_eq(
            alpha(app.world().get::<BackgroundColor>(reverse).unwrap().0),
            0.2,
        );
        assert!(app.world().get::<UiAnimations>(reverse).is_none());

        *app.world_mut().resource_mut::<UiMotionPolicy>() = UiMotionPolicy::Reduced;
        let looping = app
            .world_mut()
            .spawn(UiTransform::from_scale(Vec2::ONE))
            .id();
        send(
            &mut app,
            UiAnimationCommand::start(
                looping,
                UiAnimationSpec::transform_scale(TEST_ANIMATION, Vec2::ONE, Vec2::splat(1.25), 1.0)
                    .with_direction(UiAnimationDirection::Reverse)
                    .with_repeat(UiAnimationRepeat::Infinite),
            ),
        );
        app.update();
        assert_eq!(
            app.world().get::<UiTransform>(looping).unwrap().scale,
            Vec2::ONE
        );
        assert!(app.world().get::<UiAnimations>(looping).is_none());

        *app.world_mut().resource_mut::<UiMotionPolicy>() = UiMotionPolicy::Full;
        let sought = app
            .world_mut()
            .spawn(UiTransform::from_scale(Vec2::ONE))
            .id();
        send(
            &mut app,
            UiAnimationCommand::start(
                sought,
                UiAnimationSpec::transform_scale(
                    UiAnimationId::new("sought"),
                    Vec2::ONE,
                    Vec2::splat(2.0),
                    1.0,
                ),
            ),
        );
        send(
            &mut app,
            UiAnimationCommand::Seek {
                entity: sought,
                target: None,
                progress: 0.25,
                pause: true,
            },
        );
        app.update();
        *app.world_mut().resource_mut::<UiMotionPolicy>() = UiMotionPolicy::Disabled;
        app.update();
        assert_eq!(
            app.world().get::<UiTransform>(sought).unwrap().scale,
            Vec2::splat(2.0)
        );
        assert!(app.world().get::<UiAnimations>(sought).is_none());
    }

    #[test]
    fn disabled_motion_uses_final_repeat_direction_despite_delay_and_paused_seek() {
        let mut app = animation_test_app();
        *app.world_mut().resource_mut::<UiMotionPolicy>() = UiMotionPolicy::Disabled;
        let cases = [
            (
                UiAnimationId::new("alternate.even"),
                UiAnimationDirection::Alternate,
                UiAnimationRepeat::Count(2),
                0.2,
            ),
            (
                UiAnimationId::new("alternate.odd"),
                UiAnimationDirection::Alternate,
                UiAnimationRepeat::Count(3),
                0.8,
            ),
            (
                UiAnimationId::new("alternate_reverse.even"),
                UiAnimationDirection::AlternateReverse,
                UiAnimationRepeat::Count(2),
                0.8,
            ),
            (
                UiAnimationId::new("alternate_reverse.odd"),
                UiAnimationDirection::AlternateReverse,
                UiAnimationRepeat::Count(3),
                0.2,
            ),
        ];
        let mut entities = Vec::new();
        for (id, direction, repeat, expected) in cases {
            let entity = app.world_mut().spawn(BackgroundColor(Color::WHITE)).id();
            send(
                &mut app,
                UiAnimationCommand::start(
                    entity,
                    UiAnimationSpec::alpha(id, 0.2, 0.8, 1.0)
                        .with_delay(99.0)
                        .with_direction(direction)
                        .with_repeat(repeat),
                ),
            );
            send(
                &mut app,
                UiAnimationCommand::Seek {
                    entity,
                    target: None,
                    progress: 0.37,
                    pause: true,
                },
            );
            entities.push((entity, expected));
        }

        app.update();

        for (entity, expected) in entities {
            assert_approx_eq(
                alpha(app.world().get::<BackgroundColor>(entity).unwrap().0),
                expected,
            );
            assert!(app.world().get::<UiAnimations>(entity).is_none());
        }
    }

    #[test]
    fn seek_and_pause_hold_a_deterministic_frame_without_rewriting_visuals() {
        #[derive(Default, Resource)]
        struct SnapshotChangeCount(usize);

        fn count_snapshot_changes(
            snapshots: Query<(), Changed<UiAnimationDebugSnapshot>>,
            mut count: ResMut<SnapshotChangeCount>,
        ) {
            count.0 += snapshots.iter().count();
        }

        let mut app = animation_test_app();
        app.init_resource::<SnapshotChangeCount>().add_systems(
            Update,
            count_snapshot_changes.after(UiAnimationSystems::Tick),
        );
        let entity = app
            .world_mut()
            .spawn(UiTransform::from_scale(Vec2::ONE))
            .id();
        send(
            &mut app,
            UiAnimationCommand::start(
                entity,
                UiAnimationSpec::transform_scale(TEST_ANIMATION, Vec2::ONE, Vec2::splat(2.0), 2.0)
                    .with_repeat(UiAnimationRepeat::Count(2)),
            ),
        );
        send(
            &mut app,
            UiAnimationCommand::Seek {
                entity,
                target: None,
                progress: 0.25,
                pause: true,
            },
        );
        app.update();
        assert_eq!(
            app.world().get::<UiTransform>(entity).unwrap().scale,
            Vec2::splat(1.25)
        );
        app.world_mut().clear_trackers();
        app.update();
        let entity_ref = app.world().entity(entity);
        let transform = entity_ref.get_ref::<UiTransform>().unwrap();
        assert!(!transform.is_changed());
        let animations = entity_ref.get_ref::<UiAnimations>().unwrap();
        let snapshot = entity_ref.get_ref::<UiAnimationDebugSnapshot>().unwrap();
        assert!(!animations.is_changed());
        assert!(!snapshot.is_changed());
        assert!(snapshot.tracks[0].paused);
        assert_approx_eq(snapshot.tracks[0].raw_progress, 0.25);

        app.world_mut().resource_mut::<SnapshotChangeCount>().0 = 0;
        *app.world_mut().resource_mut::<UiMotionPolicy>() = UiMotionPolicy::Reduced;
        app.world_mut().clear_trackers();
        app.update();
        let entity_ref = app.world().entity(entity);
        assert!(!entity_ref.get_ref::<UiAnimations>().unwrap().is_changed());
        let snapshot = entity_ref.get_ref::<UiAnimationDebugSnapshot>().unwrap();
        assert_eq!(snapshot.policy, "reduced");
        assert_eq!(app.world().resource::<SnapshotChangeCount>().0, 1);

        app.world_mut().resource_mut::<SnapshotChangeCount>().0 = 0;
        app.world_mut().clear_trackers();
        app.update();
        let snapshot = app
            .world()
            .entity(entity)
            .get_ref::<UiAnimationDebugSnapshot>()
            .unwrap();
        assert!(!snapshot.is_changed());
        assert_eq!(app.world().resource::<SnapshotChangeCount>().0, 0);
    }

    #[test]
    fn direct_legacy_alpha_insertion_rejects_conflicting_generic_track_at_tick() {
        let mut app = animation_test_app();
        let generic = UiAnimations::try_from_spec(UiAnimationSpec::background_color(
            TEST_ANIMATION,
            Color::BLACK,
            Color::WHITE,
            1.0,
        ))
        .unwrap();
        let entity = app
            .world_mut()
            .spawn((
                BackgroundColor(Color::BLACK),
                generic,
                UiAnimatedAlpha::new(0.0, 0.4, 1.0),
            ))
            .id();
        advance(&mut app, 0.5);

        assert!(events(&app).iter().any(|event| {
            event.kind == UiAnimationEventKind::Rejected
                && event.error == Some(UiAnimationError::ConflictingLegacyAlpha)
        }));
        assert!(app.world().get::<UiAnimations>(entity).is_none());
        assert_approx_eq(
            alpha(app.world().get::<BackgroundColor>(entity).unwrap().0),
            0.2,
        );
    }

    #[test]
    fn destroyed_or_missing_target_has_stable_cancellation_semantics() {
        let mut app = animation_test_app();
        let entity = app
            .world_mut()
            .spawn(UiTransform::from_scale(Vec2::ONE))
            .id();
        app.world_mut().entity_mut(entity).despawn();
        send(
            &mut app,
            UiAnimationCommand::start(
                entity,
                UiAnimationSpec::transform_scale(TEST_ANIMATION, Vec2::ONE, Vec2::splat(2.0), 1.0),
            ),
        );
        app.update();
        assert!(events(&app).iter().any(|event| {
            event.kind == UiAnimationEventKind::Rejected
                && event.error == Some(UiAnimationError::TargetEntityMissing)
        }));

        let missing_component = app.world_mut().spawn_empty().id();
        send(
            &mut app,
            UiAnimationCommand::start(
                missing_component,
                UiAnimationSpec::transform_scale(TEST_ANIMATION, Vec2::ONE, Vec2::splat(2.0), 1.0),
            ),
        );
        app.update();
        assert!(events(&app).iter().any(|event| {
            event.kind == UiAnimationEventKind::Rejected
                && event.error == Some(UiAnimationError::TargetComponentMissing)
        }));
    }

    #[test]
    fn generic_completion_can_keep_player_or_despawn_target() {
        let mut app = animation_test_app();
        let keep = app
            .world_mut()
            .spawn((
                UiTransform::default(),
                UiAnimations::try_from_spec(
                    UiAnimationSpec::transform_scale(
                        UiAnimationId::new("keep.finished"),
                        Vec2::ONE,
                        Vec2::splat(1.2),
                        0.0,
                    )
                    .with_completion(UiAnimationCompletion::KeepComponent),
                )
                .unwrap(),
            ))
            .id();
        let despawn = app
            .world_mut()
            .spawn((
                UiTransform::default(),
                UiAnimations::try_from_spec(
                    UiAnimationSpec::transform_scale(
                        UiAnimationId::new("despawn.finished"),
                        Vec2::ONE,
                        Vec2::splat(1.2),
                        0.0,
                    )
                    .with_completion(UiAnimationCompletion::DespawnEntity),
                )
                .unwrap(),
            ))
            .id();
        app.update();

        let animations = app.world().get::<UiAnimations>(keep).unwrap();
        assert_eq!(animations.tracks[0].state, UiAnimationState::Finished);
        assert_eq!(
            app.world().get::<UiTransform>(keep).unwrap().scale,
            Vec2::splat(1.2)
        );
        assert!(app.world().get_entity(despawn).is_err());
    }

    #[test]
    fn theme_change_cancels_active_tracks_without_restoring_old_values() {
        let mut app = animation_test_app();
        app.insert_resource(UiTheme::default());
        app.update();
        let entity = app.world_mut().spawn(BackgroundColor(Color::BLACK)).id();
        send(
            &mut app,
            UiAnimationCommand::start(
                entity,
                UiAnimationSpec::background_color(TEST_ANIMATION, Color::BLACK, Color::WHITE, 2.0),
            ),
        );
        advance(&mut app, 0.5);
        app.world_mut()
            .entity_mut(entity)
            .insert(BackgroundColor(Color::srgb(0.1, 0.8, 0.3)));
        app.world_mut()
            .resource_mut::<UiTheme>()
            .colors
            .panel_background = Color::srgb(0.1, 0.8, 0.3);
        app.update();

        assert_eq!(
            app.world().get::<BackgroundColor>(entity).unwrap().0,
            Color::srgb(0.1, 0.8, 0.3)
        );
        assert!(app.world().get::<UiAnimations>(entity).is_none());
    }

    #[test]
    fn legacy_alpha_applies_background_and_text_and_disabled_finishes_same_frame() {
        let mut app = animation_test_app();
        *app.world_mut().resource_mut::<UiMotionPolicy>() = UiMotionPolicy::Disabled;
        let background = app
            .world_mut()
            .spawn((
                BackgroundColor(Color::WHITE),
                UiAnimatedAlpha::new(0.2, 0.56, 10.0),
            ))
            .id();
        let text = app
            .world_mut()
            .spawn((
                Text::new("Confirm"),
                BackgroundColor(Color::NONE),
                TextColor(Color::WHITE),
                UiAnimatedAlpha::new(0.0, 0.72, 10.0),
            ))
            .id();
        app.update();
        assert_approx_eq(
            alpha(app.world().get::<BackgroundColor>(background).unwrap().0),
            0.56,
        );
        assert_approx_eq(alpha(app.world().get::<TextColor>(text).unwrap().0), 0.72);
        assert_eq!(
            app.world().get::<BackgroundColor>(text).unwrap().0,
            Color::NONE
        );
        assert!(app.world().get::<UiAnimatedAlpha>(background).is_none());
    }

    #[test]
    fn despawn_completion_removes_entity() {
        let mut app = animation_test_app();
        let entity = app
            .world_mut()
            .spawn((
                BackgroundColor(Color::WHITE),
                UiAnimatedAlpha::fade_out(0.0)
                    .with_completion(UiAnimationCompletion::DespawnEntity),
            ))
            .id();
        app.update();
        assert!(app.world().get_entity(entity).is_err());
    }
}
