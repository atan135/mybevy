use bevy::prelude::*;

use crate::game::ui::{
    core::{UiLayer, UiLayerRoot},
    style::UiTheme,
    widgets::screen_label,
};

const DEFAULT_TOAST_DURATION_SECS: f32 = 2.4;

#[derive(Clone, Debug)]
pub(in crate::game) struct UiToast {
    pub text: String,
    pub duration_secs: f32,
}

impl UiToast {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            duration_secs: DEFAULT_TOAST_DURATION_SECS,
        }
    }
}

#[derive(Component)]
pub(in crate::game) struct UiToastRoot {
    timer: Timer,
}

pub(in crate::game) fn tick_toasts(
    mut commands: Commands,
    time: Res<Time>,
    mut toasts: Query<(Entity, &mut UiToastRoot)>,
) {
    for (entity, mut toast) in &mut toasts {
        toast.timer.tick(time.delta());
        if toast.timer.is_finished() {
            commands.entity(entity).try_despawn();
        }
    }
}

pub(in crate::game) fn spawn_toast(commands: &mut Commands, theme: &UiTheme, toast: &UiToast) {
    commands.spawn((
        UiToastRoot {
            timer: Timer::from_seconds(toast.duration_secs.max(0.1), TimerMode::Once),
        },
        UiLayerRoot {
            layer: UiLayer::Toast,
        },
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(theme.layout.overlay_padding),
            justify_content: JustifyContent::Center,
            padding: UiRect::horizontal(px(theme.layout.overlay_padding)),
            ..default()
        },
        ZIndex(200),
        children![(
            Node {
                max_width: px(420),
                padding: UiRect::axes(px(18), px(12)),
                border: UiRect::all(px(theme.panel.border)),
                border_radius: BorderRadius::all(px(theme.button.radius)),
                ..default()
            },
            BackgroundColor(theme.colors.panel_background),
            BorderColor::all(theme.colors.panel_border),
            children![screen_label(
                toast.text.clone(),
                theme.text.caption,
                theme.colors.text_primary,
            )],
        )],
    ));
}

pub(in crate::game) fn close_toasts(
    commands: &mut Commands,
    toast_roots: &Query<Entity, With<UiToastRoot>>,
) {
    for entity in toast_roots {
        commands.entity(entity).try_despawn();
    }
}
