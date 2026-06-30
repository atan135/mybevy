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
    pub label: String,
    pub accessible_key: String,
    pub accessible_fallback: String,
    pub accessible_label: String,
}

impl UiIconButton {
    fn new(
        label: impl Into<String>,
        accessible_key: impl Into<String>,
        accessible_fallback: impl Into<String>,
        accessible_label: impl Into<String>,
    ) -> Self {
        Self {
            label: label.into(),
            accessible_key: accessible_key.into(),
            accessible_fallback: accessible_fallback.into(),
            accessible_label: accessible_label.into(),
        }
    }
}

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
    (
        Text::new(text),
        TextFont {
            font: fonts.regular.clone(),
            font_size: style_role.font_size(theme),
            ..default()
        },
        TextColor(theme.colors.text_primary),
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
    (
        Text::new(text),
        TextFont {
            font: fonts.regular.clone(),
            font_size: style_role.font_size(theme),
            ..default()
        },
        TextColor(color_role.color(theme)),
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
    i18n: &UiI18n,
    icon: impl Into<String>,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    icon_button_key_bundle(
        theme,
        metrics,
        fonts,
        icon,
        key,
        fallback,
        i18n.tr(key, fallback),
        theme.colors.secondary_button,
        SecondaryButton,
        IconButtonVisualState::Idle,
    )
}

pub(crate) fn disabled_icon_button_key(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    icon: impl Into<String>,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    icon_button_key_bundle(
        theme,
        metrics,
        fonts,
        icon,
        key,
        fallback,
        i18n.tr(key, fallback),
        theme.colors.secondary_button,
        (SecondaryButton, DisabledButton),
        IconButtonVisualState::Disabled,
    )
}

pub(crate) fn loading_icon_button_key(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    icon: impl Into<String>,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    icon_button_key_bundle(
        theme,
        metrics,
        fonts,
        icon,
        key,
        fallback,
        i18n.tr(key, fallback),
        theme.colors.primary_button,
        (PrimaryButton, LoadingButton),
        IconButtonVisualState::Loading,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IconButtonVisualState {
    Idle,
    Disabled,
    Loading,
}

pub(crate) fn icon_button_key_bundle<T: Bundle>(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    icon: impl Into<String>,
    accessible_key: impl Into<String>,
    accessible_fallback: impl Into<String>,
    accessible_label: impl Into<String>,
    colors: ButtonColors,
    marker: T,
    state: IconButtonVisualState,
) -> impl Bundle {
    let icon = icon.into();
    let accessible_label = accessible_label.into();
    let text_color_role = icon_button_text_color_role(state);

    (
        Button,
        FocusableButton,
        UiIconButton::new(
            icon.clone(),
            accessible_key,
            accessible_fallback,
            accessible_label,
        ),
        marker,
        icon_button_node(theme, metrics),
        BackgroundColor(icon_button_background_color(colors, state)),
        children![(
            Text::new(icon),
            TextFont {
                font: fonts.regular.clone(),
                font_size: theme.text.button,
                ..default()
            },
            TextColor(text_color_role.color(theme)),
            text_color_role,
            UiThemeTextStyleRole::Button,
        )],
    )
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
    state: IconButtonVisualState,
) -> Color {
    match state {
        IconButtonVisualState::Idle => colors.idle,
        IconButtonVisualState::Disabled => colors.disabled,
        IconButtonVisualState::Loading => colors.loading,
    }
}

pub(crate) fn icon_button_text_color_role(state: IconButtonVisualState) -> UiThemeTextColorRole {
    match state {
        IconButtonVisualState::Idle | IconButtonVisualState::Loading => {
            UiThemeTextColorRole::Primary
        }
        IconButtonVisualState::Disabled => UiThemeTextColorRole::Muted,
    }
}

pub(crate) fn sync_icon_button_accessible_labels(
    i18n: Res<UiI18n>,
    mut icon_buttons: Query<&mut UiIconButton>,
) {
    if !i18n.is_changed() {
        return;
    }

    for mut icon_button in &mut icon_buttons {
        let next_label = i18n.tr(
            &icon_button.accessible_key,
            icon_button.accessible_fallback.clone(),
        );
        if icon_button.accessible_label != next_label {
            icon_button.accessible_label = next_label;
        }
    }
}

pub(crate) fn sync_icon_button_nodes(
    theme: Res<UiTheme>,
    metrics: Res<UiMetrics>,
    mut icon_buttons: Query<&mut Node, With<UiIconButton>>,
) {
    if !theme.is_changed() && !metrics.is_changed() {
        return;
    }

    for mut node in &mut icon_buttons {
        let size = square_button_size(&metrics);
        node.min_width = px(size);
        node.width = px(size);
        node.height = px(size);
        node.padding = UiRect::ZERO;
        node.border_radius = BorderRadius::all(px(theme.button.radius));
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
