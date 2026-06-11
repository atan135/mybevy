#![allow(dead_code)]

use bevy::prelude::*;

pub(in crate::game) struct UiAnimationPlugin;

impl Plugin for UiAnimationPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(Update, UiAnimationSystems::Tick)
            .add_systems(
                Update,
                tick_ui_alpha_animations.in_set(UiAnimationSystems::Tick),
            );
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, SystemSet)]
pub(in crate::game) enum UiAnimationSystems {
    Tick,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::game) enum UiAnimationCompletion {
    KeepComponent,
    RemoveComponent,
    DespawnEntity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::game) enum UiAnimationEasing {
    Linear,
    EaseOutCubic,
    EaseInOutCubic,
}

impl UiAnimationEasing {
    pub(in crate::game) fn sample(self, progress: f32) -> f32 {
        let progress = clamp_progress(progress);

        match self {
            UiAnimationEasing::Linear => progress,
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
pub(in crate::game) enum UiAnimationState {
    Running,
    Finished,
}

#[derive(Clone, Copy, Debug, Component, PartialEq)]
pub(in crate::game) struct UiAnimatedAlpha {
    pub from: f32,
    pub to: f32,
    pub duration_secs: f32,
    pub elapsed_secs: f32,
    pub easing: UiAnimationEasing,
    pub completion: UiAnimationCompletion,
    pub state: UiAnimationState,
}

impl UiAnimatedAlpha {
    pub(in crate::game) fn new(from: f32, to: f32, duration_secs: f32) -> Self {
        Self {
            from,
            to,
            duration_secs,
            elapsed_secs: 0.0,
            easing: UiAnimationEasing::Linear,
            completion: UiAnimationCompletion::RemoveComponent,
            state: UiAnimationState::Running,
        }
    }

    pub(in crate::game) fn fade_in(duration_secs: f32) -> Self {
        Self::new(0.0, 1.0, duration_secs)
    }

    pub(in crate::game) fn fade_out(duration_secs: f32) -> Self {
        Self::new(1.0, 0.0, duration_secs)
    }

    pub(in crate::game) fn with_easing(mut self, easing: UiAnimationEasing) -> Self {
        self.easing = easing;
        self
    }

    pub(in crate::game) fn with_completion(mut self, completion: UiAnimationCompletion) -> Self {
        self.completion = completion;
        self
    }

    pub(in crate::game) fn progress(self) -> f32 {
        animation_progress(self.elapsed_secs, self.duration_secs)
    }

    pub(in crate::game) fn eased_progress(self) -> f32 {
        self.easing.sample(self.progress())
    }

    pub(in crate::game) fn alpha(self) -> f32 {
        interpolate_alpha(self.from, self.to, self.eased_progress())
    }

    pub(in crate::game) fn is_finished(self) -> bool {
        self.state == UiAnimationState::Finished || self.progress() >= 1.0
    }

    pub(in crate::game) fn tick(&mut self, delta_secs: f32) -> UiAnimationState {
        if self.state == UiAnimationState::Finished {
            return UiAnimationState::Finished;
        }

        self.elapsed_secs = (self.elapsed_secs + delta_secs.max(0.0)).max(0.0);
        if self.progress() >= 1.0 {
            self.state = UiAnimationState::Finished;
        }

        self.state
    }
}

fn tick_ui_alpha_animations(
    mut commands: Commands,
    time: Res<Time>,
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

        let state = animation.tick(delta_secs);
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

fn apply_alpha(
    background: Option<Mut<BackgroundColor>>,
    text_color: Option<Mut<TextColor>>,
    has_text: bool,
    alpha: f32,
) {
    if let Some(mut background) = background
        && !has_text
    {
        let next_color = color_with_alpha(background.0, alpha);
        if background.0 != next_color {
            *background = BackgroundColor(next_color);
        }
    }

    if let Some(mut text_color) = text_color {
        let next_color = color_with_alpha(text_color.0, alpha);
        if text_color.0 != next_color {
            *text_color = TextColor(next_color);
        }
    }
}

fn color_with_alpha(color: Color, alpha: f32) -> Color {
    color.with_alpha(alpha.clamp(0.0, 1.0))
}

fn animation_progress(elapsed_secs: f32, duration_secs: f32) -> f32 {
    let elapsed_secs = elapsed_secs.max(0.0);
    let duration_secs = duration_secs.max(0.0);

    if duration_secs <= f32::EPSILON {
        1.0
    } else {
        (elapsed_secs / duration_secs).clamp(0.0, 1.0)
    }
}

fn clamp_progress(progress: f32) -> f32 {
    progress.clamp(0.0, 1.0)
}

fn interpolate_alpha(from: f32, to: f32, progress: f32) -> f32 {
    let progress = clamp_progress(progress);
    (from + (to - from) * progress).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 0.0001;

    fn assert_approx_eq(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= EPSILON,
            "expected {actual} to be approximately {expected}"
        );
    }

    fn alpha(color: Color) -> f32 {
        color.to_srgba().alpha
    }

    #[test]
    fn easing_samples_are_clamped() {
        assert_approx_eq(UiAnimationEasing::Linear.sample(-0.5), 0.0);
        assert_approx_eq(UiAnimationEasing::Linear.sample(1.5), 1.0);
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
        assert_approx_eq(animation_progress(0.0, -1.0), 1.0);
    }

    #[test]
    fn alpha_interpolation_clamps_to_visible_range() {
        assert_approx_eq(interpolate_alpha(0.2, 0.8, 0.5), 0.5);
        assert_approx_eq(interpolate_alpha(-1.0, 0.5, 0.0), 0.0);
        assert_approx_eq(interpolate_alpha(0.5, 2.0, 1.0), 1.0);
        assert_approx_eq(
            UiAnimatedAlpha::fade_in(2.0)
                .with_easing(UiAnimationEasing::EaseOutCubic)
                .alpha(),
            0.0,
        );
    }

    #[test]
    fn ticking_updates_state_and_clamps_progress() {
        let mut animation = UiAnimatedAlpha::new(0.0, 1.0, 1.0);

        assert_eq!(animation.tick(0.25), UiAnimationState::Running);
        assert_approx_eq(animation.progress(), 0.25);
        assert!(!animation.is_finished());

        assert_eq!(animation.tick(2.0), UiAnimationState::Finished);
        assert_approx_eq(animation.progress(), 1.0);
        assert!(animation.is_finished());

        assert_eq!(animation.tick(1.0), UiAnimationState::Finished);
        assert_approx_eq(animation.elapsed_secs, 2.25);
    }

    #[test]
    fn apply_alpha_updates_background_and_text_color() {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .add_plugins(UiAnimationPlugin);

        let entity = app
            .world_mut()
            .spawn((
                BackgroundColor(Color::srgba(0.1, 0.2, 0.3, 1.0)),
                TextColor(Color::srgba(0.4, 0.5, 0.6, 1.0)),
                UiAnimatedAlpha::new(0.2, 0.8, 1.0)
                    .with_completion(UiAnimationCompletion::KeepComponent),
            ))
            .id();

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(500));
        app.update();

        let background = app.world().get::<BackgroundColor>(entity).unwrap();
        let text_color = app.world().get::<TextColor>(entity).unwrap();

        assert_approx_eq(alpha(background.0), 0.5);
        assert_approx_eq(alpha(text_color.0), 0.5);
        assert!(app.world().get::<UiAnimatedAlpha>(entity).is_some());
    }

    #[test]
    fn text_alpha_does_not_reveal_required_background() {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .add_plugins(UiAnimationPlugin);

        let entity = app
            .world_mut()
            .spawn((
                Text::new("Confirm"),
                BackgroundColor(Color::NONE),
                TextColor(Color::srgba(0.4, 0.5, 0.6, 1.0)),
                UiAnimatedAlpha::new(0.2, 0.8, 1.0)
                    .with_completion(UiAnimationCompletion::KeepComponent),
            ))
            .id();

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(500));
        app.update();

        let background = app.world().get::<BackgroundColor>(entity).unwrap();
        let text_color = app.world().get::<TextColor>(entity).unwrap();

        assert_eq!(background.0, Color::NONE);
        assert_approx_eq(alpha(text_color.0), 0.5);
    }

    #[test]
    fn remove_component_completion_stops_animation() {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .add_plugins(UiAnimationPlugin);

        let entity = app
            .world_mut()
            .spawn((
                BackgroundColor(Color::WHITE),
                UiAnimatedAlpha::fade_out(0.1)
                    .with_completion(UiAnimationCompletion::RemoveComponent),
            ))
            .id();

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(100));
        app.update();

        assert!(app.world().get_entity(entity).is_ok());
        assert!(app.world().get::<UiAnimatedAlpha>(entity).is_none());
        assert_approx_eq(
            alpha(app.world().get::<BackgroundColor>(entity).unwrap().0),
            0.0,
        );
    }

    #[test]
    fn despawn_completion_removes_entity() {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .add_plugins(UiAnimationPlugin);

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
