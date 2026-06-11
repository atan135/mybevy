use bevy::{ecs::hierarchy::ChildSpawnerCommands, prelude::*};

use crate::game::{
    navigation::AppUiMode,
    ui::{
        core::{
            UiAnimatedAlpha, UiAnimationCompletion, UiAnimationEasing, UiLayer, UiLayerRoot,
            UiMetrics, UiPanelCommand, UiPanelId, UiPanelKind, UiPanelRoot, UiViewport,
            UiWidthClass,
        },
        i18n::{UiI18n, UiI18nText},
        style::{
            UiFontAssets, UiTheme,
            theme::{
                ButtonColors, UiThemeBackgroundRole, UiThemeBorderRole, UiThemeButtonNodeRole,
                UiThemeRootNodeRole, UiThemeTextColorRole, UiThemeTextStyleRole,
            },
        },
        widgets::controls::{PrimaryButton, SecondaryButton},
        widgets::{
            DisabledButton, FocusableButton, FocusedButton, LoadingButton, SelectedButton,
            ui_scroll_column_with_max_height,
        },
    },
};

const CONFIRM_ENTRY_FADE_SECS: f32 = 0.16;

#[derive(Clone, Debug)]
pub(in crate::game) struct UiConfirmModal {
    pub id: UiModalId,
    pub title: String,
    pub body: String,
    pub detail: Option<String>,
    pub title_i18n_text: Option<UiI18nText>,
    pub body_i18n_text: Option<UiI18nText>,
    pub detail_i18n_text: Option<UiI18nText>,
    pub actions: Vec<UiModalActionSpec>,
}

#[derive(Clone, Debug)]
pub(in crate::game) struct UiModalActionSpec {
    pub label: String,
    pub action: UiModalAction,
    pub style: UiModalActionStyle,
    pub i18n_text: Option<UiI18nText>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(in crate::game) enum UiModalId {
    TouchRippleLaunch,
    GalleryConfirm,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(in crate::game) enum UiModalAction {
    Cancel,
    Confirm,
    TouchRippleSinglePlayer,
    TouchRippleNetworked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(in crate::game) enum UiModalActionStyle {
    Primary,
    Secondary,
}

#[derive(Clone, Copy, Debug, Message)]
pub(in crate::game) struct UiModalResult {
    pub id: UiModalId,
    pub action: UiModalAction,
}

#[derive(Component)]
pub(in crate::game) struct UiModalActionButton {
    id: UiModalId,
    action: UiModalAction,
}

#[derive(Clone, Debug)]
pub(in crate::game) struct UiI18nTextSpec {
    pub text: String,
    pub i18n_text: UiI18nText,
}

impl UiI18nTextSpec {
    pub(in crate::game) fn new(i18n: &UiI18n, key: &'static str, fallback: &'static str) -> Self {
        Self {
            text: i18n.tr(key, fallback),
            i18n_text: UiI18nText::new(key, fallback),
        }
    }
}

#[derive(Component)]
pub(in crate::game) struct UiConfirmAnimatedPanel;

#[derive(Clone, Copy, Component)]
pub(in crate::game) struct UiConfirmAnimatedButton {
    style: UiModalActionStyle,
}

pub(in crate::game) fn handle_modal_action_buttons(
    mut modal_results: MessageWriter<UiModalResult>,
    mut panel_commands: MessageWriter<UiPanelCommand>,
    buttons: Query<
        (&Interaction, &UiModalActionButton),
        (
            Changed<Interaction>,
            With<Button>,
            Without<DisabledButton>,
            Without<LoadingButton>,
        ),
    >,
) {
    for (interaction, action_button) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }

        modal_results.write(UiModalResult {
            id: action_button.id,
            action: action_button.action,
        });
        panel_commands.write(UiPanelCommand::Close(UiPanelId::ConfirmModal));
    }
}

pub(in crate::game) fn spawn_confirm_modal(
    commands: &mut Commands,
    theme: &UiTheme,
    metrics: &UiMetrics,
    viewport: &UiViewport,
    fonts: &UiFontAssets,
    modal: &UiConfirmModal,
    owner_mode: Option<AppUiMode>,
) {
    commands
        .spawn((
            UiPanelRoot {
                id: UiPanelId::ConfirmModal,
                kind: UiPanelKind::Modal,
                owner_mode,
            },
            UiLayerRoot {
                layer: UiLayer::Modal,
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
                padding: viewport.safe_area_padding(metrics.page_padding),
                ..default()
            },
            ZIndex(100),
            BackgroundColor(theme.colors.modal_overlay_background.with_alpha(0.0)),
            UiThemeBackgroundRole::ModalOverlay,
            UiThemeRootNodeRole::BlockingOverlay,
            confirm_entry_fade_animation(theme.colors.modal_overlay_background),
        ))
        .with_children(|root| {
            root.spawn((
                confirm_panel_node(theme, metrics),
                BackgroundColor(theme.colors.panel_background.with_alpha(0.0)),
                BorderColor::all(theme.colors.panel_border.with_alpha(0.0)),
                UiThemeBackgroundRole::Panel,
                UiThemeBorderRole::Panel,
                UiConfirmAnimatedPanel,
                confirm_entry_fade_animation(theme.colors.panel_background),
            ))
            .with_children(|panel| {
                if let Some(i18n_text) = modal.title_i18n_text.clone() {
                    panel.spawn((
                        confirm_text(
                            theme,
                            fonts,
                            modal.title.clone(),
                            UiThemeTextStyleRole::Subtitle,
                            UiThemeTextColorRole::Primary,
                        ),
                        i18n_text,
                    ));
                } else {
                    panel.spawn(confirm_text(
                        theme,
                        fonts,
                        modal.title.clone(),
                        UiThemeTextStyleRole::Subtitle,
                        UiThemeTextColorRole::Primary,
                    ));
                }

                panel
                    .spawn(confirm_body_scroll_node(metrics))
                    .with_children(|body| {
                        if let Some(i18n_text) = modal.body_i18n_text.clone() {
                            body.spawn((
                                confirm_text(
                                    theme,
                                    fonts,
                                    modal.body.clone(),
                                    UiThemeTextStyleRole::Body,
                                    UiThemeTextColorRole::Primary,
                                ),
                                i18n_text,
                            ));
                        } else {
                            body.spawn(confirm_text(
                                theme,
                                fonts,
                                modal.body.clone(),
                                UiThemeTextStyleRole::Body,
                                UiThemeTextColorRole::Primary,
                            ));
                        }

                        if let Some(detail) = &modal.detail {
                            if let Some(i18n_text) = modal.detail_i18n_text.clone() {
                                body.spawn((
                                    confirm_text(
                                        theme,
                                        fonts,
                                        detail.clone(),
                                        UiThemeTextStyleRole::Caption,
                                        UiThemeTextColorRole::Muted,
                                    ),
                                    i18n_text,
                                ));
                            } else {
                                body.spawn(confirm_text(
                                    theme,
                                    fonts,
                                    detail.clone(),
                                    UiThemeTextStyleRole::Caption,
                                    UiThemeTextColorRole::Muted,
                                ));
                            }
                        }
                    });

                panel
                    .spawn(confirm_action_row_node(metrics, viewport.width_class))
                    .with_children(|actions| {
                        for action in &modal.actions {
                            let action_marker = UiModalActionButton {
                                id: modal.id,
                                action: action.action,
                            };
                            spawn_confirm_action_button(
                                actions,
                                theme,
                                metrics,
                                fonts,
                                action,
                                action_marker,
                            );
                        }
                    });
            });
        });
}

pub(in crate::game) fn sync_confirm_entry_visual_alpha(
    theme: Res<UiTheme>,
    mut panels: Query<(&mut BorderColor, Option<&UiAnimatedAlpha>), With<UiConfirmAnimatedPanel>>,
    mut buttons: Query<(
        &Interaction,
        &mut BackgroundColor,
        &UiConfirmAnimatedButton,
        Option<&UiAnimatedAlpha>,
        Has<DisabledButton>,
        Has<FocusedButton>,
        Has<SelectedButton>,
        Has<LoadingButton>,
    )>,
) {
    let border_target_alpha = color_alpha(theme.colors.panel_border);

    for (mut border, animation) in &mut panels {
        let next_border =
            border_with_alpha(*border, entry_border_alpha(animation, border_target_alpha));
        if *border != next_border {
            *border = next_border;
        }
    }

    for (
        interaction,
        mut background,
        animated_button,
        animation,
        is_disabled,
        is_focused,
        is_selected,
        is_loading,
    ) in &mut buttons
    {
        let colors = confirm_button_colors(&theme, animated_button.style);
        let alpha = animation
            .map(|animation| animation.alpha())
            .unwrap_or_else(|| color_alpha(background.0));
        let next_background = BackgroundColor(confirm_button_background_color(
            colors,
            *interaction,
            is_disabled,
            is_focused,
            is_selected,
            is_loading,
            alpha,
        ));
        if *background != next_background {
            *background = next_background;
        }
    }
}

fn spawn_confirm_action_button(
    actions: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    action: &UiModalActionSpec,
    action_marker: UiModalActionButton,
) {
    let colors = confirm_button_colors(theme, action.style);
    let background = colors.idle;

    let mut button = match action.style {
        UiModalActionStyle::Primary => actions.spawn((
            confirm_button_base(theme, metrics, background),
            PrimaryButton,
            UiConfirmAnimatedButton {
                style: action.style,
            },
            confirm_entry_fade_animation(background),
            action_marker,
        )),
        UiModalActionStyle::Secondary => actions.spawn((
            confirm_button_base(theme, metrics, background),
            SecondaryButton,
            UiConfirmAnimatedButton {
                style: action.style,
            },
            confirm_entry_fade_animation(background),
            action_marker,
        )),
    };

    button.with_children(|button| {
        if let Some(i18n_text) = action.i18n_text.clone() {
            button.spawn((
                confirm_button_label(theme, fonts, action.label.clone()),
                i18n_text,
            ));
        } else {
            button.spawn(confirm_button_label(theme, fonts, action.label.clone()));
        }
    });
}

fn confirm_panel_node(theme: &UiTheme, metrics: &UiMetrics) -> Node {
    Node {
        width: percent(100),
        max_width: px(metrics.dialog_max_width),
        max_height: percent(100),
        flex_direction: FlexDirection::Column,
        row_gap: px(metrics.section_gap),
        padding: UiRect::all(px(metrics.panel_padding)),
        border: UiRect::all(px(theme.panel.border)),
        border_radius: BorderRadius::all(px(theme.panel.radius)),
        ..default()
    }
}

fn confirm_body_scroll_node(metrics: &UiMetrics) -> impl Bundle {
    ui_scroll_column_with_max_height(metrics.control_gap, confirm_body_max_height(metrics))
}

fn confirm_action_row_node(metrics: &UiMetrics, width_class: UiWidthClass) -> Node {
    let is_compact = width_class == UiWidthClass::Compact;

    Node {
        width: percent(100),
        align_items: AlignItems::Center,
        justify_content: if is_compact {
            JustifyContent::FlexStart
        } else {
            JustifyContent::FlexEnd
        },
        justify_items: if is_compact {
            JustifyItems::Stretch
        } else {
            JustifyItems::End
        },
        column_gap: px(metrics.control_gap),
        row_gap: px(metrics.control_gap),
        flex_wrap: FlexWrap::Wrap,
        margin: UiRect::top(px(metrics.control_gap)),
        ..default()
    }
}

fn confirm_body_max_height(metrics: &UiMetrics) -> f32 {
    (metrics.dialog_max_width * 0.9).clamp(160.0, 420.0)
}

fn confirm_button_base(theme: &UiTheme, metrics: &UiMetrics, background: Color) -> impl Bundle {
    (
        Button,
        FocusableButton,
        UiThemeButtonNodeRole::Button,
        Node {
            min_width: px(theme.button.min_width.max(metrics.button_height * 2.25)),
            height: px(metrics.button_height),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::axes(px((metrics.control_gap * 2.0).clamp(12.0, 24.0)), px(0)),
            border_radius: BorderRadius::all(px(theme.button.radius)),
            ..default()
        },
        BackgroundColor(background.with_alpha(0.0)),
    )
}

fn confirm_button_label(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
) -> impl Bundle {
    confirm_text(
        theme,
        fonts,
        text,
        UiThemeTextStyleRole::Button,
        UiThemeTextColorRole::Primary,
    )
}

fn confirm_text(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
    style_role: UiThemeTextStyleRole,
    color_role: UiThemeTextColorRole,
) -> impl Bundle {
    let color = color_role.color(theme);

    (
        Text::new(text),
        TextFont {
            font: fonts.regular.clone(),
            font_size: style_role.font_size(theme),
            ..default()
        },
        TextColor(color.with_alpha(0.0)),
        color_role,
        style_role,
        confirm_entry_fade_animation(color),
    )
}

fn confirm_entry_fade_animation(color: Color) -> UiAnimatedAlpha {
    UiAnimatedAlpha::new(0.0, color_alpha(color), CONFIRM_ENTRY_FADE_SECS)
        .with_easing(UiAnimationEasing::EaseOutCubic)
        .with_completion(UiAnimationCompletion::RemoveComponent)
}

fn confirm_button_colors(theme: &UiTheme, style: UiModalActionStyle) -> ButtonColors {
    match style {
        UiModalActionStyle::Primary => theme.colors.primary_button,
        UiModalActionStyle::Secondary => theme.colors.secondary_button,
    }
}

fn confirm_button_background_color(
    colors: ButtonColors,
    interaction: Interaction,
    is_disabled: bool,
    is_focused: bool,
    is_selected: bool,
    is_loading: bool,
    alpha: f32,
) -> Color {
    confirm_button_visual_color(
        colors,
        interaction,
        is_disabled,
        is_focused,
        is_selected,
        is_loading,
    )
    .with_alpha(alpha.clamp(0.0, 1.0))
}

fn confirm_button_visual_color(
    colors: ButtonColors,
    interaction: Interaction,
    is_disabled: bool,
    is_focused: bool,
    is_selected: bool,
    is_loading: bool,
) -> Color {
    if is_disabled {
        return colors.disabled;
    }

    if is_loading {
        return colors.loading;
    }

    match interaction {
        Interaction::Pressed => colors.pressed,
        Interaction::Hovered => colors.hovered,
        Interaction::None if is_selected => colors.selected,
        Interaction::None if is_focused => colors.focused,
        Interaction::None => colors.idle,
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
    fn confirm_entry_fade_uses_modal_overlay_alpha_target() {
        let animation = confirm_entry_fade_animation(Color::srgba(0.1, 0.2, 0.3, 0.72));

        assert_approx_eq(animation.from, 0.0);
        assert_approx_eq(animation.to, 0.72);
        assert_approx_eq(animation.duration_secs, CONFIRM_ENTRY_FADE_SECS);
        assert_eq!(animation.easing, UiAnimationEasing::EaseOutCubic);
        assert_eq!(animation.completion, UiAnimationCompletion::RemoveComponent);
    }

    #[test]
    fn confirm_button_background_keeps_visual_state_and_fade_alpha() {
        let colors = UiTheme::default().colors.primary_button;

        assert_eq!(
            confirm_button_visual_color(colors, Interaction::Pressed, true, true, true, true),
            colors.disabled
        );
        assert_eq!(
            confirm_button_visual_color(colors, Interaction::Pressed, false, true, true, true),
            colors.loading
        );
        assert_eq!(
            confirm_button_visual_color(colors, Interaction::Hovered, false, false, false, false),
            colors.hovered
        );

        let faded = confirm_button_background_color(
            colors,
            Interaction::Hovered,
            false,
            false,
            false,
            false,
            0.35,
        );

        assert_eq!(faded.with_alpha(1.0), colors.hovered);
        assert_approx_eq(color_alpha(faded), 0.35);
    }

    #[test]
    fn confirm_border_alpha_follows_panel_background() {
        let border = BorderColor::all(Color::srgba(0.2, 0.3, 0.4, 1.0));
        let synced = border_with_alpha(border, 0.58);

        assert_approx_eq(color_alpha(synced.top), 0.58);
        assert_approx_eq(color_alpha(synced.right), 0.58);
        assert_approx_eq(color_alpha(synced.bottom), 0.58);
        assert_approx_eq(color_alpha(synced.left), 0.58);
    }

    #[test]
    fn confirm_border_alpha_restores_theme_target_after_entry() {
        assert_approx_eq(entry_border_alpha(None, 0.9), 0.9);

        let mut animation = confirm_entry_fade_animation(Color::srgba(0.1, 0.2, 0.3, 0.94));
        animation.tick(CONFIRM_ENTRY_FADE_SECS * 0.5);

        assert_approx_eq(
            entry_border_alpha(Some(&animation), 0.9),
            UiAnimationEasing::EaseOutCubic.sample(0.5) * 0.9,
        );
    }

    #[test]
    fn confirm_panel_width_uses_metrics_dialog_max_width() {
        let theme = UiTheme::default();
        let metrics = UiMetrics::default();
        let node = confirm_panel_node(&theme, &metrics);

        assert_eq!(node.width, percent(100));
        assert_eq!(node.max_width, px(metrics.dialog_max_width));
    }

    #[test]
    fn compact_confirm_action_row_wraps_from_start() {
        let metrics = UiMetrics::default();
        let node = confirm_action_row_node(&metrics, UiWidthClass::Compact);

        assert_eq!(node.justify_content, JustifyContent::FlexStart);
        assert_eq!(node.justify_items, JustifyItems::Stretch);
        assert_eq!(node.flex_wrap, FlexWrap::Wrap);
        assert_eq!(node.column_gap, px(metrics.control_gap));
    }

    #[test]
    fn expanded_confirm_action_row_aligns_to_end() {
        let metrics = UiMetrics::default();
        let node = confirm_action_row_node(&metrics, UiWidthClass::Expanded);

        assert_eq!(node.justify_content, JustifyContent::FlexEnd);
        assert_eq!(node.justify_items, JustifyItems::End);
        assert_eq!(node.flex_wrap, FlexWrap::Wrap);
    }
}
