use bevy::{picking::Pickable, prelude::*};

use crate::framework::ui::{
    core::{
        UiAnimatedAlpha, UiAnimationCompletion, UiAnimationEasing, UiLayer, UiLayerRoot, UiMetrics,
        UiViewport,
    },
    i18n::{UiI18n, UiI18nText},
    style::{
        UiFontAssets, UiTheme,
        theme::{
            UiThemeBackgroundRole, UiThemeBorderRole, UiThemeRootNodeRole, UiThemeTextColorRole,
            UiThemeTextStyleRole,
        },
    },
};

const DEFAULT_TOAST_DURATION_SECS: f32 = 2.4;
const MIN_TOAST_DURATION_SECS: f32 = 0.1;
const TOAST_FADE_IN_SECS: f32 = 0.14;
const TOAST_FADE_OUT_SECS: f32 = 0.2;

#[derive(Clone, Debug)]
pub(crate) struct UiToast {
    pub text: String,
    pub duration_secs: f32,
    pub i18n_text: Option<UiI18nText>,
}

impl UiToast {
    #[allow(dead_code)]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            duration_secs: DEFAULT_TOAST_DURATION_SECS,
            i18n_text: None,
        }
    }

    pub fn new_key(i18n: &UiI18n, key: &'static str, fallback: &'static str) -> Self {
        Self {
            text: i18n.tr(key, fallback),
            duration_secs: DEFAULT_TOAST_DURATION_SECS,
            i18n_text: Some(UiI18nText::new(key, fallback)),
        }
    }
}

#[derive(Component)]
pub(crate) struct UiToastRoot {
    timer: Timer,
    fade_out_started: bool,
    fade_out_secs: f32,
}

#[cfg(test)]
impl UiToastRoot {
    pub(crate) fn for_audit_test() -> Self {
        Self {
            timer: Timer::from_seconds(1.0, TimerMode::Once),
            fade_out_started: false,
            fade_out_secs: TOAST_FADE_OUT_SECS,
        }
    }
}

#[derive(Component)]
pub(crate) struct UiToastAnimatedVisual;

pub(crate) fn tick_toasts(
    mut commands: Commands,
    time: Res<Time>,
    mut toasts: Query<(Entity, &mut UiToastRoot)>,
    children: Query<&Children>,
    visuals: Query<Entity, With<UiToastAnimatedVisual>>,
) {
    for (entity, mut toast) in &mut toasts {
        toast.timer.tick(time.delta());

        if should_start_toast_fade_out(
            toast.timer.elapsed_secs(),
            toast.timer.duration().as_secs_f32(),
            toast.fade_out_secs,
            toast.fade_out_started,
        ) {
            toast.fade_out_started = true;
            start_toast_fade_out(
                &mut commands,
                entity,
                toast.fade_out_secs,
                &children,
                &visuals,
            );
        }

        if toast.timer.is_finished() {
            commands.entity(entity).try_despawn();
        }
    }
}

pub(crate) fn spawn_toast(
    commands: &mut Commands,
    theme: &UiTheme,
    metrics: &UiMetrics,
    viewport: &UiViewport,
    fonts: &UiFontAssets,
    toast: &UiToast,
) {
    let duration_secs = toast_duration_secs(toast.duration_secs);
    let fade_in_secs = toast_fade_in_secs(duration_secs);
    let fade_out_secs = toast_fade_out_secs(duration_secs);

    commands
        .spawn((
            Name::new("UiToastRoot"),
            UiToastRoot {
                timer: Timer::from_seconds(duration_secs, TimerMode::Once),
                fade_out_started: false,
                fade_out_secs,
            },
            UiLayerRoot {
                layer: UiLayer::Toast,
            },
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(metrics.page_padding + viewport.safe_area.top),
                justify_content: JustifyContent::Center,
                padding: UiRect {
                    left: px(metrics.page_padding + viewport.safe_area.left),
                    right: px(metrics.page_padding + viewport.safe_area.right),
                    top: px(0),
                    bottom: px(0),
                },
                ..default()
            },
            ZIndex(200),
            Pickable::IGNORE,
            UiThemeRootNodeRole::Toast,
        ))
        .with_children(|root| {
            root.spawn((
                toast_panel_node(theme, metrics),
                BackgroundColor(theme.colors.panel_background.with_alpha(0.0)),
                BorderColor::all(theme.colors.panel_border.with_alpha(0.0)),
                UiThemeBackgroundRole::Panel,
                UiThemeBorderRole::Panel,
                UiToastAnimatedVisual,
                Pickable::IGNORE,
                toast_fade_in_animation(fade_in_secs),
            ))
            .with_children(|panel| {
                if let Some(i18n_text) = toast.i18n_text.clone() {
                    panel.spawn((
                        toast_label(theme, fonts, toast.text.clone(), fade_in_secs),
                        i18n_text,
                    ));
                } else {
                    panel.spawn(toast_label(theme, fonts, toast.text.clone(), fade_in_secs));
                }
            });
        });
}

fn toast_panel_node(theme: &UiTheme, metrics: &UiMetrics) -> Node {
    Node {
        max_width: px(toast_panel_max_width(metrics)),
        padding: UiRect::axes(px(metrics.panel_padding), px(metrics.control_gap * 1.5)),
        border: UiRect::all(px(theme.panel.border)),
        border_radius: BorderRadius::all(px(theme.button.radius)),
        ..default()
    }
}

fn toast_panel_max_width(metrics: &UiMetrics) -> f32 {
    metrics.dialog_max_width.min(metrics.content_max_width)
}

pub(crate) fn close_toasts(
    commands: &mut Commands,
    toast_roots: &Query<Entity, With<UiToastRoot>>,
) {
    for entity in toast_roots {
        commands.entity(entity).try_despawn();
    }
}

pub(crate) fn sync_toast_border_alpha(
    mut panels: Query<(&BackgroundColor, &mut BorderColor), With<UiToastAnimatedVisual>>,
) {
    for (background, mut border) in &mut panels {
        let next_border = border_with_alpha(*border, background.0.to_srgba().alpha);
        if *border != next_border {
            *border = next_border;
        }
    }
}

fn start_toast_fade_out(
    commands: &mut Commands,
    root: Entity,
    fade_out_secs: f32,
    children: &Query<&Children>,
    visuals: &Query<Entity, With<UiToastAnimatedVisual>>,
) {
    for entity in children.iter_descendants(root) {
        if visuals.get(entity).is_ok() {
            commands
                .entity(entity)
                .insert(toast_fade_out_animation(fade_out_secs));
        }
    }
}

fn border_with_alpha(border: BorderColor, alpha: f32) -> BorderColor {
    BorderColor {
        top: border.top.with_alpha(alpha),
        right: border.right.with_alpha(alpha),
        bottom: border.bottom.with_alpha(alpha),
        left: border.left.with_alpha(alpha),
    }
}

fn toast_fade_in_animation(duration_secs: f32) -> UiAnimatedAlpha {
    UiAnimatedAlpha::fade_in(duration_secs)
        .with_easing(UiAnimationEasing::EaseOutCubic)
        .with_completion(UiAnimationCompletion::RemoveComponent)
}

fn toast_fade_out_animation(duration_secs: f32) -> UiAnimatedAlpha {
    UiAnimatedAlpha::fade_out(duration_secs)
        .with_easing(UiAnimationEasing::EaseOutCubic)
        .with_completion(UiAnimationCompletion::KeepComponent)
}

fn toast_label(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
    fade_in_secs: f32,
) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font: fonts.regular.clone(),
            font_size: UiThemeTextStyleRole::Caption.font_size(theme),
            ..default()
        },
        TextColor(UiThemeTextColorRole::Primary.color(theme).with_alpha(0.0)),
        UiThemeTextColorRole::Primary,
        UiThemeTextStyleRole::Caption,
        UiToastAnimatedVisual,
        Pickable::IGNORE,
        toast_fade_in_animation(fade_in_secs),
    )
}

fn toast_duration_secs(duration_secs: f32) -> f32 {
    duration_secs.max(MIN_TOAST_DURATION_SECS)
}

fn toast_fade_in_secs(duration_secs: f32) -> f32 {
    TOAST_FADE_IN_SECS.min(duration_secs * 0.5)
}

fn toast_fade_out_secs(duration_secs: f32) -> f32 {
    TOAST_FADE_OUT_SECS.min(duration_secs * 0.5)
}

fn toast_fade_out_starts_at(duration_secs: f32, fade_out_secs: f32) -> f32 {
    (duration_secs - fade_out_secs).max(0.0)
}

fn should_start_toast_fade_out(
    elapsed_secs: f32,
    duration_secs: f32,
    fade_out_secs: f32,
    fade_out_started: bool,
) -> bool {
    !fade_out_started && elapsed_secs >= toast_fade_out_starts_at(duration_secs, fade_out_secs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::world::CommandQueue;

    const EPSILON: f32 = 0.0001;

    fn assert_approx_eq(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= EPSILON,
            "expected {actual} to be approximately {expected}"
        );
    }

    #[test]
    fn toast_duration_is_clamped() {
        assert_approx_eq(toast_duration_secs(2.4), 2.4);
        assert_approx_eq(toast_duration_secs(0.02), MIN_TOAST_DURATION_SECS);
    }

    #[test]
    fn fade_lengths_fit_short_durations() {
        let duration_secs = toast_duration_secs(0.1);

        assert_approx_eq(toast_fade_in_secs(duration_secs), 0.05);
        assert_approx_eq(toast_fade_out_secs(duration_secs), 0.05);
        assert_approx_eq(
            toast_fade_out_starts_at(duration_secs, toast_fade_out_secs(duration_secs)),
            0.05,
        );
    }

    #[test]
    fn fade_out_starts_near_lifecycle_end() {
        let duration_secs = toast_duration_secs(2.4);
        let fade_out_secs = toast_fade_out_secs(duration_secs);

        assert_approx_eq(fade_out_secs, TOAST_FADE_OUT_SECS);
        assert_approx_eq(toast_fade_out_starts_at(duration_secs, fade_out_secs), 2.2);
        assert!(!should_start_toast_fade_out(
            2.19,
            duration_secs,
            fade_out_secs,
            false,
        ));
        assert!(should_start_toast_fade_out(
            2.2,
            duration_secs,
            fade_out_secs,
            false,
        ));
    }

    #[test]
    fn fade_out_does_not_repeat_once_started() {
        assert!(!should_start_toast_fade_out(2.4, 2.4, 0.2, true));
    }

    #[test]
    fn toast_panel_max_width_uses_metrics_bounds() {
        let theme = UiTheme::default();
        let metrics = UiMetrics::default();
        let node = toast_panel_node(&theme, &metrics);

        assert_eq!(node.max_width, px(toast_panel_max_width(&metrics)));
        assert_eq!(
            node.max_width,
            px(metrics.dialog_max_width.min(metrics.content_max_width))
        );
    }

    #[test]
    fn complete_toast_subtree_is_pick_through() {
        let mut world = World::new();
        let mut queue = CommandQueue::default();
        let mut commands = Commands::new(&mut queue, &world);
        spawn_toast(
            &mut commands,
            &UiTheme::default(),
            &UiMetrics::default(),
            &UiViewport::default(),
            &UiFontAssets::test_registry(),
            &UiToast::new("toast"),
        );
        queue.apply(&mut world);

        let mut query = world.query::<&Pickable>();
        let values = query.iter(&world).collect::<Vec<_>>();
        assert_eq!(values.len(), 3);
        assert!(values.iter().all(|pickable| **pickable == Pickable::IGNORE));
    }
}
