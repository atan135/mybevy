use bevy::prelude::*;

use crate::game::navigation::AppUiMode;
use crate::game::ui::{
    core::{
        UiAnimatedAlpha, UiAnimationCompletion, UiAnimationEasing, UiBlockingOverlay, UiLayer,
        UiLayerRoot, UiPanelId, UiPanelKind, UiPanelRoot,
    },
    i18n::{UiI18n, UiI18nText},
    style::{
        UiFontAssets, UiTheme,
        theme::{
            UiThemeBackgroundRole, UiThemeBorderRole, UiThemePanelNodeRole, UiThemeRootNodeRole,
            UiThemeTextColorRole, UiThemeTextStyleRole,
        },
    },
};

const LOADING_ENTRY_FADE_SECS: f32 = 0.16;

#[derive(Clone, Debug)]
pub(in crate::game) struct UiLoading {
    pub text: String,
    pub cancellable: bool,
    pub i18n_text: Option<UiI18nText>,
}

impl UiLoading {
    #[allow(dead_code)]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            cancellable: false,
            i18n_text: None,
        }
    }

    pub fn new_key(i18n: &UiI18n, key: &'static str, fallback: &'static str) -> Self {
        Self {
            text: i18n.tr(key, fallback),
            cancellable: false,
            i18n_text: Some(UiI18nText::new(key, fallback)),
        }
    }

    #[allow(dead_code)]
    pub fn cancellable(mut self) -> Self {
        self.cancellable = true;
        self
    }
}

#[derive(Component)]
pub(in crate::game) struct UiLoadingAnimatedPanel;

pub(in crate::game) fn spawn_loading(
    commands: &mut Commands,
    theme: &UiTheme,
    fonts: &UiFontAssets,
    loading: &UiLoading,
    owner_mode: Option<AppUiMode>,
) {
    commands
        .spawn((
            UiPanelRoot {
                id: UiPanelId::GlobalLoading,
                kind: UiPanelKind::BlockingOverlay,
                owner_mode,
            },
            UiBlockingOverlay {
                cancellable: loading.cancellable,
            },
            UiLayerRoot {
                layer: UiLayer::Loading,
            },
            Button,
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::all(px(theme.layout.screen_padding)),
                ..default()
            },
            ZIndex(150),
            BackgroundColor(theme.colors.loading_overlay_background.with_alpha(0.0)),
            UiThemeBackgroundRole::LoadingOverlay,
            UiThemeRootNodeRole::BlockingOverlay,
            loading_entry_fade_animation(theme.colors.loading_overlay_background),
        ))
        .with_children(|root| {
            root.spawn((
                UiThemePanelNodeRole::Loading,
                Node {
                    min_width: px(260),
                    max_width: px(420),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    padding: UiRect::axes(px(22), px(16)),
                    border: UiRect::all(px(theme.panel.border)),
                    border_radius: BorderRadius::all(px(theme.panel.radius)),
                    ..default()
                },
                BackgroundColor(theme.colors.panel_background.with_alpha(0.0)),
                BorderColor::all(theme.colors.panel_border.with_alpha(0.0)),
                UiThemeBackgroundRole::Panel,
                UiThemeBorderRole::Panel,
                UiLoadingAnimatedPanel,
                loading_entry_fade_animation(theme.colors.panel_background),
            ))
            .with_children(|panel| {
                if let Some(i18n_text) = loading.i18n_text.clone() {
                    panel.spawn((loading_label(theme, fonts, loading.text.clone()), i18n_text));
                } else {
                    panel.spawn(loading_label(theme, fonts, loading.text.clone()));
                }
            });
        });
}

pub(in crate::game) fn sync_loading_entry_border_alpha(
    theme: Res<UiTheme>,
    mut panels: Query<(&mut BorderColor, Option<&UiAnimatedAlpha>), With<UiLoadingAnimatedPanel>>,
) {
    let target_alpha = color_alpha(theme.colors.panel_border);

    for (mut border, animation) in &mut panels {
        let next_border = border_with_alpha(*border, entry_border_alpha(animation, target_alpha));
        if *border != next_border {
            *border = next_border;
        }
    }
}

fn loading_label(theme: &UiTheme, fonts: &UiFontAssets, text: impl Into<String>) -> impl Bundle {
    let color = UiThemeTextColorRole::Primary.color(theme);

    (
        Text::new(text),
        TextFont {
            font: fonts.regular.clone(),
            font_size: UiThemeTextStyleRole::Body.font_size(theme),
            ..default()
        },
        TextColor(color.with_alpha(0.0)),
        UiThemeTextColorRole::Primary,
        UiThemeTextStyleRole::Body,
        loading_entry_fade_animation(color),
    )
}

fn loading_entry_fade_animation(color: Color) -> UiAnimatedAlpha {
    UiAnimatedAlpha::new(0.0, color_alpha(color), LOADING_ENTRY_FADE_SECS)
        .with_easing(UiAnimationEasing::EaseOutCubic)
        .with_completion(UiAnimationCompletion::RemoveComponent)
}

fn border_with_alpha(border: BorderColor, alpha: f32) -> BorderColor {
    BorderColor {
        top: border.top.with_alpha(alpha),
        right: border.right.with_alpha(alpha),
        bottom: border.bottom.with_alpha(alpha),
        left: border.left.with_alpha(alpha),
    }
}

fn color_alpha(color: Color) -> f32 {
    color.to_srgba().alpha
}

fn entry_border_alpha(animation: Option<&UiAnimatedAlpha>, target_alpha: f32) -> f32 {
    animation
        .map(|animation| animation.eased_progress() * target_alpha)
        .unwrap_or(target_alpha)
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

    #[test]
    fn loading_entry_fade_uses_overlay_alpha_target() {
        let animation = loading_entry_fade_animation(Color::srgba(0.1, 0.2, 0.3, 0.56));

        assert_approx_eq(animation.from, 0.0);
        assert_approx_eq(animation.to, 0.56);
        assert_approx_eq(animation.duration_secs, LOADING_ENTRY_FADE_SECS);
        assert_eq!(animation.easing, UiAnimationEasing::EaseOutCubic);
        assert_eq!(animation.completion, UiAnimationCompletion::RemoveComponent);
    }

    #[test]
    fn loading_border_alpha_follows_panel_background() {
        let border = BorderColor::all(Color::srgba(0.2, 0.3, 0.4, 1.0));
        let synced = border_with_alpha(border, 0.42);

        assert_approx_eq(color_alpha(synced.top), 0.42);
        assert_approx_eq(color_alpha(synced.right), 0.42);
        assert_approx_eq(color_alpha(synced.bottom), 0.42);
        assert_approx_eq(color_alpha(synced.left), 0.42);
    }

    #[test]
    fn loading_border_alpha_restores_theme_target_after_entry() {
        assert_approx_eq(entry_border_alpha(None, 0.8), 0.8);

        let mut animation = loading_entry_fade_animation(Color::srgba(0.1, 0.2, 0.3, 0.94));
        animation.tick(LOADING_ENTRY_FADE_SECS * 0.5);

        assert_approx_eq(
            entry_border_alpha(Some(&animation), 0.8),
            UiAnimationEasing::EaseOutCubic.sample(0.5) * 0.8,
        );
    }
}
