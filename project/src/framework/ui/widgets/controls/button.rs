use super::*;

#[derive(Component)]
pub(crate) struct PrimaryButton;

#[derive(Component)]
pub(crate) struct SecondaryButton;

#[derive(Component)]
pub(crate) struct DisabledButton;

#[derive(Component)]
pub(crate) struct FocusableButton;

#[derive(Component)]
pub(crate) struct FocusedButton;

#[derive(Component)]
pub(crate) struct SelectedButton;

#[derive(Component)]
pub(crate) struct LoadingButton;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiButtonEventKind {
    Down,
    Up,
    Click,
    Cancel,
}

#[derive(Clone, Copy, Debug, Message)]
#[allow(dead_code)]
pub(crate) struct UiButtonEvent {
    pub entity: Entity,
    pub kind: UiButtonEventKind,
    pub button: Option<PointerButton>,
}

#[derive(Clone, Debug, Component)]
#[allow(dead_code)]
pub(crate) struct UiIconButton {
    pub icon: UiIconId,
    pub accessible_key: String,
    pub accessible_fallback: String,
    pub accessible_label: String,
    pub layout: UiIconButtonLayout,
    pub visuals: UiIconButtonVisuals,
    pub visual_state: UiButtonVisualState,
}

impl UiIconButton {
    fn new(
        icon: UiIconId,
        accessible_key: impl Into<String>,
        accessible_fallback: impl Into<String>,
        accessible_label: impl Into<String>,
        layout: UiIconButtonLayout,
        visuals: UiIconButtonVisuals,
        visual_state: UiButtonVisualState,
    ) -> Self {
        Self {
            icon,
            accessible_key: accessible_key.into(),
            accessible_fallback: accessible_fallback.into(),
            accessible_label: accessible_label.into(),
            layout,
            visuals,
            visual_state,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiIconLabelPlacement {
    Leading,
    Trailing,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum UiIconButtonLayout {
    IconOnly,
    Labeled(UiIconLabelPlacement),
    FixedImage {
        width: f32,
        height: f32,
        visual_size: f32,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiButtonVisualState {
    Idle,
    Hovered,
    Pressed,
    Focused,
    Selected,
    Disabled,
    Loading,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct UiIconStateOverride {
    pub icon: Option<UiIconId>,
    pub tint: Option<Color>,
    pub background: Option<Color>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct UiIconButtonVisuals {
    pub base_icon: UiIconId,
    pub idle: UiIconStateOverride,
    pub hovered: UiIconStateOverride,
    pub pressed: UiIconStateOverride,
    pub focused: UiIconStateOverride,
    pub selected: UiIconStateOverride,
    pub disabled: UiIconStateOverride,
    pub loading: UiIconStateOverride,
}

impl UiIconButtonVisuals {
    pub(crate) const fn new(base_icon: UiIconId) -> Self {
        Self {
            base_icon,
            idle: UiIconStateOverride {
                icon: None,
                tint: None,
                background: None,
            },
            hovered: UiIconStateOverride {
                icon: None,
                tint: None,
                background: None,
            },
            pressed: UiIconStateOverride {
                icon: None,
                tint: None,
                background: None,
            },
            focused: UiIconStateOverride {
                icon: None,
                tint: None,
                background: None,
            },
            selected: UiIconStateOverride {
                icon: None,
                tint: None,
                background: None,
            },
            disabled: UiIconStateOverride {
                icon: None,
                tint: None,
                background: None,
            },
            loading: UiIconStateOverride {
                icon: Some(UiIconId::LOADING),
                tint: None,
                background: None,
            },
        }
    }

    pub(crate) const fn override_for(self, state: UiButtonVisualState) -> UiIconStateOverride {
        match state {
            UiButtonVisualState::Idle => self.idle,
            UiButtonVisualState::Hovered => self.hovered,
            UiButtonVisualState::Pressed => self.pressed,
            UiButtonVisualState::Focused => self.focused,
            UiButtonVisualState::Selected => self.selected,
            UiButtonVisualState::Disabled => self.disabled,
            UiButtonVisualState::Loading => self.loading,
        }
    }
}

#[derive(Component)]
pub(super) struct UiIconAccessibilityLabel;

pub(crate) fn emit_ui_button_events(
    mut button_events: MessageWriter<UiButtonEvent>,
    mut presses: MessageReader<Pointer<Press>>,
    mut releases: MessageReader<Pointer<Release>>,
    mut clicks: MessageReader<Pointer<Click>>,
    mut cancels: MessageReader<Pointer<Cancel>>,
    buttons: Query<
        (),
        (
            With<Button>,
            Without<DisabledButton>,
            Without<LoadingButton>,
        ),
    >,
    parents: Query<&ChildOf>,
) {
    for press in presses.read() {
        if let Some(button_entity) = ui_button_event_target(press.entity, &buttons, &parents) {
            button_events.write(UiButtonEvent {
                entity: button_entity,
                kind: UiButtonEventKind::Down,
                button: Some(press.button),
            });
        }
    }

    for click in clicks.read() {
        if let Some(button_entity) = ui_button_event_target(click.entity, &buttons, &parents) {
            button_events.write(UiButtonEvent {
                entity: button_entity,
                kind: UiButtonEventKind::Click,
                button: Some(click.button),
            });
        }
    }

    for release in releases.read() {
        if let Some(button_entity) = ui_button_event_target(release.entity, &buttons, &parents) {
            button_events.write(UiButtonEvent {
                entity: button_entity,
                kind: UiButtonEventKind::Up,
                button: Some(release.button),
            });
        }
    }

    for cancel in cancels.read() {
        if let Some(button_entity) = ui_button_event_target(cancel.entity, &buttons, &parents) {
            button_events.write(UiButtonEvent {
                entity: button_entity,
                kind: UiButtonEventKind::Cancel,
                button: None,
            });
        }
    }
}

pub(crate) fn ui_button_event_target(
    entity: Entity,
    buttons: &Query<
        (),
        (
            With<Button>,
            Without<DisabledButton>,
            Without<LoadingButton>,
        ),
    >,
    parents: &Query<&ChildOf>,
) -> Option<Entity> {
    if buttons.contains(entity) {
        return Some(entity);
    }

    parents
        .iter_ancestors(entity)
        .find(|ancestor| buttons.contains(*ancestor))
}

pub(crate) fn screen_title(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
    style_role: UiThemeTextStyleRole,
) -> impl Bundle {
    let style = UiTextStyleToken::for_theme_role(theme, style_role);
    (
        try_ui_styled_text(fonts, text, style, theme.colors.text_primary)
            .expect("built-in title text style must be valid"),
        UiThemeTextColorRole::Primary,
        style_role,
    )
}

pub(crate) fn screen_title_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
    style_role: UiThemeTextStyleRole,
) -> impl Bundle {
    (
        screen_title(theme, fonts, i18n.tr(key, fallback), style_role),
        UiI18nText::new(key, fallback),
    )
}

pub(crate) fn screen_label(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
    style_role: UiThemeTextStyleRole,
    color_role: UiThemeTextColorRole,
) -> impl Bundle {
    let style = UiTextStyleToken::for_theme_role(theme, style_role);
    (
        try_ui_styled_text(fonts, text, style, color_role.color(theme))
            .expect("built-in label text style must be valid"),
        color_role,
        style_role,
    )
}

pub(crate) fn screen_label_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
    style_role: UiThemeTextStyleRole,
    color_role: UiThemeTextColorRole,
) -> impl Bundle {
    (
        screen_label(theme, fonts, i18n.tr(key, fallback), style_role, color_role),
        UiI18nText::new(key, fallback),
    )
}

#[allow(dead_code)]
pub(crate) fn primary_action_button(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    text: impl Into<String>,
) -> impl Bundle {
    action_button(
        theme,
        metrics,
        fonts,
        text,
        theme.colors.primary_button,
        PrimaryButton,
    )
}

pub(crate) fn primary_action_button_key(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    action_button_key_bundle(
        theme,
        metrics,
        fonts,
        i18n.tr(key, fallback),
        theme.colors.primary_button,
        PrimaryButton,
        UiI18nText::new(key, fallback),
    )
}

#[allow(dead_code)]
pub(crate) fn primary_action_button_with_i18n_text(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    text: impl Into<String>,
    i18n_text: UiI18nText,
) -> impl Bundle {
    action_button_key_bundle(
        theme,
        metrics,
        fonts,
        text,
        theme.colors.primary_button,
        PrimaryButton,
        i18n_text,
    )
}

#[allow(dead_code)]
pub(crate) fn secondary_action_button(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    text: impl Into<String>,
) -> impl Bundle {
    action_button(
        theme,
        metrics,
        fonts,
        text,
        theme.colors.secondary_button,
        SecondaryButton,
    )
}

#[allow(dead_code)]
pub(crate) fn secondary_action_button_with_i18n_text(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    text: impl Into<String>,
    i18n_text: UiI18nText,
) -> impl Bundle {
    action_button_key_bundle(
        theme,
        metrics,
        fonts,
        text,
        theme.colors.secondary_button,
        SecondaryButton,
        i18n_text,
    )
}

pub(crate) fn secondary_action_button_key(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    action_button_key_bundle(
        theme,
        metrics,
        fonts,
        i18n.tr(key, fallback),
        theme.colors.secondary_button,
        SecondaryButton,
        UiI18nText::new(key, fallback),
    )
}

#[allow(dead_code)]
pub(crate) fn disabled_primary_action_button(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    text: impl Into<String>,
) -> impl Bundle {
    disabled_action_button(
        theme,
        metrics,
        fonts,
        text,
        theme.colors.primary_button,
        PrimaryButton,
    )
}

pub(crate) fn disabled_primary_action_button_key(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    disabled_action_button_key_bundle(
        theme,
        metrics,
        fonts,
        i18n.tr(key, fallback),
        theme.colors.primary_button,
        PrimaryButton,
        UiI18nText::new(key, fallback),
    )
}

#[allow(dead_code)]
pub(crate) fn disabled_secondary_action_button(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    text: impl Into<String>,
) -> impl Bundle {
    disabled_action_button(
        theme,
        metrics,
        fonts,
        text,
        theme.colors.secondary_button,
        SecondaryButton,
    )
}

pub(crate) fn disabled_secondary_action_button_key(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    disabled_action_button_key_bundle(
        theme,
        metrics,
        fonts,
        i18n.tr(key, fallback),
        theme.colors.secondary_button,
        SecondaryButton,
        UiI18nText::new(key, fallback),
    )
}

#[allow(dead_code)]
pub(crate) fn loading_primary_action_button(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    text: impl Into<String>,
) -> impl Bundle {
    (
        action_button(
            theme,
            metrics,
            fonts,
            text,
            theme.colors.primary_button,
            PrimaryButton,
        ),
        LoadingButton,
    )
}

pub(crate) fn loading_primary_action_button_key(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    (
        action_button_key_bundle(
            theme,
            metrics,
            fonts,
            i18n.tr(key, fallback),
            theme.colors.primary_button,
            PrimaryButton,
            UiI18nText::new(key, fallback),
        ),
        LoadingButton,
    )
}

pub(crate) fn icon_button_key(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    asset_server: &AssetServer,
    i18n: &UiI18n,
    icon: UiIconId,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    icon_only_button_key_bundle(
        theme,
        metrics,
        fonts,
        asset_server,
        icon,
        key,
        fallback,
        i18n.tr(key, fallback),
        theme.colors.secondary_button,
        SecondaryButton,
        UiIconButtonLayout::IconOnly,
        UiButtonVisualState::Idle,
        UiIconButtonVisuals::new(icon),
    )
}

pub(crate) fn disabled_icon_button_key(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    asset_server: &AssetServer,
    i18n: &UiI18n,
    icon: UiIconId,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    icon_only_button_key_bundle(
        theme,
        metrics,
        fonts,
        asset_server,
        icon,
        key,
        fallback,
        i18n.tr(key, fallback),
        theme.colors.secondary_button,
        (SecondaryButton, DisabledButton),
        UiIconButtonLayout::IconOnly,
        UiButtonVisualState::Disabled,
        UiIconButtonVisuals::new(icon),
    )
}

pub(crate) fn loading_icon_button_key(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    asset_server: &AssetServer,
    i18n: &UiI18n,
    icon: UiIconId,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    icon_only_button_key_bundle(
        theme,
        metrics,
        fonts,
        asset_server,
        icon,
        key,
        fallback,
        i18n.tr(key, fallback),
        theme.colors.primary_button,
        (PrimaryButton, LoadingButton),
        UiIconButtonLayout::IconOnly,
        UiButtonVisualState::Loading,
        UiIconButtonVisuals::new(icon),
    )
}

pub(crate) fn icon_label_button_key(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    asset_server: &AssetServer,
    i18n: &UiI18n,
    icon: UiIconId,
    placement: UiIconLabelPlacement,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    icon_label_button_key_bundle(
        theme,
        metrics,
        fonts,
        asset_server,
        icon,
        placement,
        key,
        fallback,
        i18n.tr(key, fallback),
        theme.colors.secondary_button,
        SecondaryButton,
        UiButtonVisualState::Idle,
        UiIconButtonVisuals::new(icon),
    )
}

pub(crate) fn image_button_key(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    asset_server: &AssetServer,
    i18n: &UiI18n,
    image: UiIconId,
    width: f32,
    height: f32,
    visual_size: f32,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    icon_only_button_key_bundle(
        theme,
        metrics,
        fonts,
        asset_server,
        image,
        key,
        fallback,
        i18n.tr(key, fallback),
        theme.colors.secondary_button,
        SecondaryButton,
        UiIconButtonLayout::FixedImage {
            width,
            height,
            visual_size,
        },
        UiButtonVisualState::Idle,
        UiIconButtonVisuals::new(image),
    )
}

#[allow(dead_code)]
pub(crate) fn action_button<T: Component>(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    text: impl Into<String>,
    colors: ButtonColors,
    marker: T,
) -> impl Bundle {
    (
        Button,
        FocusableButton,
        marker,
        UiThemeButtonNodeRole::Button,
        button_node(theme, metrics),
        BackgroundColor(colors.idle),
        children![(
            Text::new(text),
            TextFont {
                font: fonts.regular.clone(),
                font_size: theme.text.button,
                ..default()
            },
            TextColor(theme.colors.text_primary),
            UiThemeTextColorRole::Primary,
            UiThemeTextStyleRole::Button,
        )],
    )
}

pub(crate) fn action_button_key_bundle<T: Component>(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    text: impl Into<String>,
    colors: ButtonColors,
    marker: T,
    i18n_text: UiI18nText,
) -> impl Bundle {
    (
        Button,
        FocusableButton,
        marker,
        UiThemeButtonNodeRole::Button,
        button_node(theme, metrics),
        BackgroundColor(colors.idle),
        children![(
            Text::new(text),
            TextFont {
                font: fonts.regular.clone(),
                font_size: theme.text.button,
                ..default()
            },
            TextColor(theme.colors.text_primary),
            UiThemeTextColorRole::Primary,
            UiThemeTextStyleRole::Button,
            i18n_text,
        )],
    )
}

#[allow(dead_code)]
pub(crate) fn disabled_action_button<T: Component>(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    text: impl Into<String>,
    colors: ButtonColors,
    marker: T,
) -> impl Bundle {
    (
        Button,
        FocusableButton,
        marker,
        DisabledButton,
        UiThemeButtonNodeRole::Button,
        button_node(theme, metrics),
        BackgroundColor(colors.disabled),
        children![(
            Text::new(text),
            TextFont {
                font: fonts.regular.clone(),
                font_size: theme.text.button,
                ..default()
            },
            TextColor(theme.colors.text_muted),
            UiThemeTextColorRole::Muted,
            UiThemeTextStyleRole::Button,
        )],
    )
}

pub(crate) fn disabled_action_button_key_bundle<T: Component>(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    text: impl Into<String>,
    colors: ButtonColors,
    marker: T,
    i18n_text: UiI18nText,
) -> impl Bundle {
    (
        Button,
        FocusableButton,
        marker,
        DisabledButton,
        UiThemeButtonNodeRole::Button,
        button_node(theme, metrics),
        BackgroundColor(colors.disabled),
        children![(
            Text::new(text),
            TextFont {
                font: fonts.regular.clone(),
                font_size: theme.text.button,
                ..default()
            },
            TextColor(theme.colors.text_muted),
            UiThemeTextColorRole::Muted,
            UiThemeTextStyleRole::Button,
            i18n_text,
        )],
    )
}

pub(crate) fn icon_only_button_key_bundle<T: Bundle>(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    asset_server: &AssetServer,
    icon: UiIconId,
    accessible_key: impl Into<String>,
    accessible_fallback: impl Into<String>,
    accessible_label: impl Into<String>,
    colors: ButtonColors,
    marker: T,
    layout: UiIconButtonLayout,
    state: UiButtonVisualState,
    visuals: UiIconButtonVisuals,
) -> impl Bundle {
    let accessible_key = accessible_key.into();
    let accessible_fallback = accessible_fallback.into();
    let accessible_label = accessible_label.into();
    let style = resolve_icon_button_style(theme, colors, visuals, state);
    let visual_size = match layout {
        UiIconButtonLayout::FixedImage { visual_size, .. } => visual_size,
        UiIconButtonLayout::IconOnly | UiIconButtonLayout::Labeled(_) => ui_icon_default_size(icon),
    };

    (
        Button,
        icon_button_accessibility_node(&accessible_label),
        FocusableButton,
        UiIconButton::new(
            icon,
            accessible_key.clone(),
            accessible_fallback.clone(),
            accessible_label.clone(),
            layout,
            visuals,
            state,
        ),
        marker,
        icon_button_layout_node(theme, metrics, layout),
        BackgroundColor(style.background),
        children![
            ui_icon(asset_server, style.icon, visual_size, style.tint),
            (
                Text::new(accessible_label),
                TextFont {
                    font: fonts.regular.clone(),
                    font_size: 1.0,
                    ..default()
                },
                Node {
                    position_type: PositionType::Absolute,
                    width: px(0),
                    height: px(0),
                    overflow: Overflow::clip(),
                    ..default()
                },
                Visibility::Hidden,
                UiI18nText::new(accessible_key, accessible_fallback),
                UiIconAccessibilityLabel,
            )
        ],
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn icon_label_button_key_bundle<T: Bundle>(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    asset_server: &AssetServer,
    icon: UiIconId,
    placement: UiIconLabelPlacement,
    accessible_key: impl Into<String>,
    accessible_fallback: impl Into<String>,
    accessible_label: impl Into<String>,
    colors: ButtonColors,
    marker: T,
    state: UiButtonVisualState,
    visuals: UiIconButtonVisuals,
) -> impl Bundle {
    let accessible_key = accessible_key.into();
    let accessible_fallback = accessible_fallback.into();
    let accessible_label = accessible_label.into();
    let layout = UiIconButtonLayout::Labeled(placement);
    let style = resolve_icon_button_style(theme, colors, visuals, state);

    (
        Button,
        icon_button_accessibility_node(&accessible_label),
        FocusableButton,
        UiIconButton::new(
            icon,
            accessible_key.clone(),
            accessible_fallback.clone(),
            accessible_label.clone(),
            layout,
            visuals,
            state,
        ),
        marker,
        icon_button_layout_node(theme, metrics, layout),
        BackgroundColor(style.background),
        children![
            ui_icon(
                asset_server,
                style.icon,
                ui_icon_default_size(icon),
                style.tint,
            ),
            (
                Text::new(accessible_label),
                TextFont {
                    font: fonts.regular.clone(),
                    font_size: theme.text.button,
                    ..default()
                },
                TextColor(theme.colors.text_primary),
                UiThemeTextColorRole::Primary,
                UiThemeTextStyleRole::Button,
                UiI18nText::new(accessible_key, accessible_fallback),
            )
        ],
    )
}

fn icon_button_accessibility_node(label: &str) -> AccessibilityNode {
    let mut node = AccessKitNode::new(AccessKitRole::Button);
    node.set_label(label);
    AccessibilityNode::from(node)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct UiResolvedIconButtonStyle {
    pub icon: UiIconId,
    pub tint: Color,
    pub background: Color,
}

pub(crate) fn resolve_icon_button_style(
    theme: &UiTheme,
    colors: ButtonColors,
    visuals: UiIconButtonVisuals,
    state: UiButtonVisualState,
) -> UiResolvedIconButtonStyle {
    let state_override = visuals.override_for(state);
    UiResolvedIconButtonStyle {
        icon: state_override.icon.unwrap_or(visuals.base_icon),
        tint: state_override
            .tint
            .unwrap_or_else(|| icon_button_state_tint(theme, state)),
        background: state_override
            .background
            .unwrap_or_else(|| icon_button_background_color(colors, state)),
    }
}

pub(crate) fn button_node(theme: &UiTheme, metrics: &UiMetrics) -> Node {
    Node {
        min_width: px(button_min_width(theme, metrics)),
        height: px(metrics.button_height),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        padding: UiRect::axes(px(control_padding_x(metrics)), px(0)),
        border_radius: BorderRadius::all(px(theme.button.radius)),
        ..default()
    }
}

pub(crate) fn square_button_node(theme: &UiTheme, metrics: &UiMetrics) -> Node {
    let size = square_button_size(metrics);
    Node {
        min_width: px(size),
        width: px(size),
        height: px(size),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        padding: UiRect::ZERO,
        border_radius: BorderRadius::all(px(theme.button.radius)),
        ..default()
    }
}

pub(crate) fn icon_button_node(theme: &UiTheme, metrics: &UiMetrics) -> Node {
    Node {
        justify_self: JustifySelf::Center,
        ..square_button_node(theme, metrics)
    }
}

pub(crate) fn icon_label_button_node(
    theme: &UiTheme,
    metrics: &UiMetrics,
    placement: UiIconLabelPlacement,
) -> Node {
    Node {
        flex_direction: match placement {
            UiIconLabelPlacement::Leading => FlexDirection::Row,
            UiIconLabelPlacement::Trailing => FlexDirection::RowReverse,
        },
        column_gap: px(metrics.control_gap),
        ..button_node(theme, metrics)
    }
}

pub(crate) fn fixed_image_button_node(
    theme: &UiTheme,
    metrics: &UiMetrics,
    width: f32,
    height: f32,
) -> Node {
    let width = if width.is_finite() && width > 0.0 {
        width.max(metrics.touch_target_min)
    } else {
        square_button_size(metrics)
    };
    let height = if height.is_finite() && height > 0.0 {
        height.max(metrics.touch_target_min)
    } else {
        square_button_size(metrics)
    };
    Node {
        min_width: px(width),
        width: px(width),
        min_height: px(height),
        height: px(height),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        padding: UiRect::ZERO,
        border_radius: BorderRadius::all(px(theme.button.radius)),
        ..default()
    }
}

pub(crate) fn icon_button_layout_node(
    theme: &UiTheme,
    metrics: &UiMetrics,
    layout: UiIconButtonLayout,
) -> Node {
    match layout {
        UiIconButtonLayout::IconOnly => icon_button_node(theme, metrics),
        UiIconButtonLayout::Labeled(placement) => icon_label_button_node(theme, metrics, placement),
        UiIconButtonLayout::FixedImage { width, height, .. } => {
            fixed_image_button_node(theme, metrics, width, height)
        }
    }
}

pub(crate) fn button_min_width(theme: &UiTheme, metrics: &UiMetrics) -> f32 {
    theme.button.min_width.max(metrics.button_height * 2.25)
}

pub(crate) fn square_button_size(metrics: &UiMetrics) -> f32 {
    metrics.button_height.max(metrics.touch_target_min)
}

pub(crate) fn control_padding_x(metrics: &UiMetrics) -> f32 {
    (metrics.control_gap * 2.0).clamp(12.0, 24.0)
}
pub(crate) fn icon_button_background_color(
    colors: ButtonColors,
    state: UiButtonVisualState,
) -> Color {
    match state {
        UiButtonVisualState::Idle => colors.idle,
        UiButtonVisualState::Hovered => colors.hovered,
        UiButtonVisualState::Pressed => colors.pressed,
        UiButtonVisualState::Focused => colors.focused,
        UiButtonVisualState::Selected => colors.selected,
        UiButtonVisualState::Disabled => colors.disabled,
        UiButtonVisualState::Loading => colors.loading,
    }
}

pub(crate) fn icon_button_state_tint(theme: &UiTheme, state: UiButtonVisualState) -> Color {
    let colors = theme.colors.icon_tint;
    match state {
        UiButtonVisualState::Idle => colors.idle,
        UiButtonVisualState::Hovered => colors.hovered,
        UiButtonVisualState::Pressed => colors.pressed,
        UiButtonVisualState::Focused => colors.focused,
        UiButtonVisualState::Selected => colors.selected,
        UiButtonVisualState::Disabled => colors.disabled,
        UiButtonVisualState::Loading => colors.loading,
    }
}

pub(crate) fn icon_button_visual_state(
    interaction: Interaction,
    is_disabled: bool,
    is_focused: bool,
    is_selected: bool,
    is_loading: bool,
) -> UiButtonVisualState {
    if is_disabled {
        return UiButtonVisualState::Disabled;
    }
    if is_loading {
        return UiButtonVisualState::Loading;
    }
    match interaction {
        Interaction::Pressed => UiButtonVisualState::Pressed,
        Interaction::Hovered => UiButtonVisualState::Hovered,
        Interaction::None if is_selected => UiButtonVisualState::Selected,
        Interaction::None if is_focused => UiButtonVisualState::Focused,
        Interaction::None => UiButtonVisualState::Idle,
    }
}

pub(crate) fn sync_icon_button_accessible_labels(
    i18n: Res<UiI18n>,
    mut icon_buttons: Query<(&mut UiIconButton, &mut Button, &mut AccessibilityNode)>,
) {
    if !i18n.is_changed() {
        return;
    }

    for (mut icon_button, mut button, mut accessibility) in &mut icon_buttons {
        let next_label = i18n.tr(
            &icon_button.accessible_key,
            icon_button.accessible_fallback.clone(),
        );
        if icon_button.accessible_label != next_label {
            icon_button.accessible_label.clone_from(&next_label);
            button.set_changed();
        }
        if accessibility.role() != AccessKitRole::Button {
            accessibility.set_role(AccessKitRole::Button);
        }
        if accessibility.label() != Some(next_label.as_str()) {
            accessibility.set_label(next_label);
        }
    }
}

pub(crate) fn sync_icon_button_nodes(
    theme: Res<UiTheme>,
    metrics: Res<UiMetrics>,
    mut icon_buttons: Query<(&UiIconButton, &mut Node)>,
) {
    if !theme.is_changed() && !metrics.is_changed() {
        return;
    }

    for (icon_button, mut node) in &mut icon_buttons {
        let next = icon_button_layout_node(&theme, &metrics, icon_button.layout);
        if icon_button_layout_owned_fields_differ(&node, &next) {
            apply_icon_button_layout_owned_fields(&mut node, &next);
        }
    }
}

fn icon_button_layout_owned_fields_differ(current: &Node, next: &Node) -> bool {
    current.width != next.width
        || current.height != next.height
        || current.min_width != next.min_width
        || current.min_height != next.min_height
        || current.align_items != next.align_items
        || current.justify_content != next.justify_content
        || current.padding != next.padding
        || current.border_radius != next.border_radius
        || current.flex_direction != next.flex_direction
        || current.column_gap != next.column_gap
}

fn apply_icon_button_layout_owned_fields(node: &mut Node, next: &Node) {
    node.width = next.width;
    node.height = next.height;
    node.min_width = next.min_width;
    node.min_height = next.min_height;
    node.align_items = next.align_items;
    node.justify_content = next.justify_content;
    node.padding = next.padding;
    node.border_radius = next.border_radius;
    node.flex_direction = next.flex_direction;
    node.column_gap = next.column_gap;
}

pub(crate) fn update_icon_button_visuals(
    theme: Res<UiTheme>,
    asset_server: Res<AssetServer>,
    mut buttons: Query<(
        &Interaction,
        &mut BackgroundColor,
        &Children,
        &mut UiIconButton,
        Has<PrimaryButton>,
        Has<DisabledButton>,
        Has<FocusedButton>,
        Has<SelectedButton>,
        Has<LoadingButton>,
    )>,
    mut icons: Query<(
        &mut ImageNode,
        &mut UiIconVisual,
        &mut UiIconResolutionStatus,
        &mut UiIconAssetStatus,
    )>,
) {
    for (
        interaction,
        mut background,
        children,
        mut icon_button,
        is_primary,
        is_disabled,
        is_focused,
        is_selected,
        is_loading,
    ) in &mut buttons
    {
        let state = icon_button_visual_state(
            *interaction,
            is_disabled,
            is_focused,
            is_selected,
            is_loading,
        );
        let colors = if is_primary {
            theme.colors.primary_button
        } else {
            theme.colors.secondary_button
        };
        let style = resolve_icon_button_style(&theme, colors, icon_button.visuals, state);
        let next_background = BackgroundColor(style.background);
        if *background != next_background {
            *background = next_background;
        }

        for child in children.iter() {
            let Ok((mut image, mut visual, mut resolution, mut asset_status)) =
                icons.get_mut(child)
            else {
                continue;
            };
            apply_ui_icon_request(
                &asset_server,
                style.icon,
                style.tint,
                &mut image,
                &mut visual,
                &mut resolution,
                &mut asset_status,
            );
        }

        if icon_button.visual_state != state {
            icon_button.visual_state = state;
        }
    }
}
pub(crate) fn update_button_visuals(
    theme: Res<UiTheme>,
    mut buttons: Query<
        (
            &Interaction,
            &mut BackgroundColor,
            Has<PrimaryButton>,
            Has<SecondaryButton>,
            Has<DisabledButton>,
            Has<FocusedButton>,
            Has<SelectedButton>,
            Has<LoadingButton>,
        ),
        (
            With<Button>,
            Without<UiIconButton>,
            Without<UiTextInput>,
            Without<UiSelectionLabel>,
        ),
    >,
) {
    for (
        interaction,
        mut background,
        is_primary,
        is_secondary,
        is_disabled,
        is_focused,
        is_selected,
        is_loading,
    ) in &mut buttons
    {
        if !is_primary && !is_secondary {
            continue;
        }

        let colors = if is_primary {
            theme.colors.primary_button
        } else {
            theme.colors.secondary_button
        };

        let next_background = BackgroundColor(button_background_color(
            colors,
            *interaction,
            is_disabled,
            is_focused,
            is_selected,
            is_loading,
        ));
        if *background != next_background {
            *background = next_background;
        }
    }
}

pub(crate) fn button_background_color(
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
