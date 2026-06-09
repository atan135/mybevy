use bevy::prelude::*;

use crate::game::ui::{
    core::{UiLayer, UiLayerRoot},
    i18n::{UiI18n, UiI18nText},
    style::{
        UiFontAssets, UiTheme,
        theme::{
            UiThemeBackgroundRole, UiThemeBorderRole, UiThemePanelNodeRole, UiThemeRootNodeRole,
            UiThemeTextColorRole, UiThemeTextStyleRole,
        },
    },
    widgets::screen_label,
};

const DEFAULT_TOAST_DURATION_SECS: f32 = 2.4;

#[derive(Clone, Debug)]
pub(in crate::game) struct UiToast {
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

pub(in crate::game) fn spawn_toast(
    commands: &mut Commands,
    theme: &UiTheme,
    fonts: &UiFontAssets,
    toast: &UiToast,
) {
    commands
        .spawn((
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
            UiThemeRootNodeRole::Toast,
        ))
        .with_children(|root| {
            root.spawn((
                UiThemePanelNodeRole::Toast,
                Node {
                    max_width: px(420),
                    padding: UiRect::axes(px(18), px(12)),
                    border: UiRect::all(px(theme.panel.border)),
                    border_radius: BorderRadius::all(px(theme.button.radius)),
                    ..default()
                },
                BackgroundColor(theme.colors.panel_background),
                BorderColor::all(theme.colors.panel_border),
                UiThemeBackgroundRole::Panel,
                UiThemeBorderRole::Panel,
            ))
            .with_children(|panel| {
                if let Some(i18n_text) = toast.i18n_text.clone() {
                    panel.spawn((
                        screen_label(
                            theme,
                            fonts,
                            toast.text.clone(),
                            UiThemeTextStyleRole::Caption,
                            UiThemeTextColorRole::Primary,
                        ),
                        i18n_text,
                    ));
                } else {
                    panel.spawn(screen_label(
                        theme,
                        fonts,
                        toast.text.clone(),
                        UiThemeTextStyleRole::Caption,
                        UiThemeTextColorRole::Primary,
                    ));
                }
            });
        });
}

pub(in crate::game) fn close_toasts(
    commands: &mut Commands,
    toast_roots: &Query<Entity, With<UiToastRoot>>,
) {
    for entity in toast_roots {
        commands.entity(entity).try_despawn();
    }
}
