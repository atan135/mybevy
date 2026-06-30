use super::*;

#[derive(Component)]
pub(crate) struct UiCheckbox;

#[derive(Component)]
pub(crate) struct UiCheckboxChecked;

#[derive(Component)]
pub(crate) struct UiToggle;

#[derive(Component)]
pub(crate) struct UiToggleOn;

#[derive(Component)]
pub(crate) struct UiSegmentedControl;

#[derive(Clone, Debug, Component)]
#[allow(dead_code)]
pub(crate) struct UiSegmentOption {
    pub value: String,
}

#[derive(Component)]
pub(crate) struct UiSegmentOptionSelected;

#[derive(Clone, Debug, Component)]
pub(crate) struct UiSelectionLabel {
    base_text: String,
}

pub(crate) fn checkbox(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
) -> impl Bundle {
    selection_button(
        theme,
        fonts,
        text,
        theme.colors.secondary_button,
        (SecondaryButton, UiCheckbox),
        SelectionVisualState::Idle,
    )
}

pub(crate) fn checkbox_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    selection_button_key_bundle(
        theme,
        fonts,
        i18n.tr(key, fallback),
        theme.colors.secondary_button,
        (SecondaryButton, UiCheckbox),
        SelectionVisualState::Idle,
        UiI18nText::new(key, fallback),
    )
}

#[allow(dead_code)]
pub(crate) fn checked_checkbox(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
) -> impl Bundle {
    selection_button(
        theme,
        fonts,
        text,
        theme.colors.secondary_button,
        (
            SecondaryButton,
            UiCheckbox,
            UiCheckboxChecked,
            SelectedButton,
        ),
        SelectionVisualState::Selected,
    )
}

pub(crate) fn checked_checkbox_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    selection_button_key_bundle(
        theme,
        fonts,
        i18n.tr(key, fallback),
        theme.colors.secondary_button,
        (
            SecondaryButton,
            UiCheckbox,
            UiCheckboxChecked,
            SelectedButton,
        ),
        SelectionVisualState::Selected,
        UiI18nText::new(key, fallback),
    )
}

pub(crate) fn disabled_checkbox_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    selection_button_key_bundle(
        theme,
        fonts,
        i18n.tr(key, fallback),
        theme.colors.secondary_button,
        (SecondaryButton, UiCheckbox, DisabledButton),
        SelectionVisualState::Disabled,
        UiI18nText::new(key, fallback),
    )
}

#[allow(dead_code)]
pub(crate) fn toggle(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
) -> impl Bundle {
    selection_button(
        theme,
        fonts,
        text,
        theme.colors.secondary_button,
        (SecondaryButton, UiToggle),
        SelectionVisualState::Idle,
    )
}

pub(crate) fn toggle_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    selection_button_key_bundle(
        theme,
        fonts,
        i18n.tr(key, fallback),
        theme.colors.secondary_button,
        (SecondaryButton, UiToggle),
        SelectionVisualState::Idle,
        UiI18nText::new(key, fallback),
    )
}

#[allow(dead_code)]
pub(crate) fn toggle_on(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
) -> impl Bundle {
    selection_button(
        theme,
        fonts,
        text,
        theme.colors.primary_button,
        (PrimaryButton, UiToggle, UiToggleOn, SelectedButton),
        SelectionVisualState::Selected,
    )
}

pub(crate) fn toggle_on_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    selection_button_key_bundle(
        theme,
        fonts,
        i18n.tr(key, fallback),
        theme.colors.primary_button,
        (PrimaryButton, UiToggle, UiToggleOn, SelectedButton),
        SelectionVisualState::Selected,
        UiI18nText::new(key, fallback),
    )
}

pub(crate) fn disabled_toggle_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    selection_button_key_bundle(
        theme,
        fonts,
        i18n.tr(key, fallback),
        theme.colors.secondary_button,
        (SecondaryButton, UiToggle, DisabledButton),
        SelectionVisualState::Disabled,
        UiI18nText::new(key, fallback),
    )
}

pub(crate) fn segmented_control(theme: &UiTheme) -> impl Bundle {
    (
        UiSegmentedControl,
        Node {
            width: percent(100),
            align_items: AlignItems::Center,
            column_gap: px(theme.layout.row_column_gap * 0.5),
            row_gap: px(theme.layout.row_gap),
            flex_wrap: FlexWrap::Wrap,
            ..default()
        },
    )
}

pub(crate) fn segment_option_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    value: impl Into<String>,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    segment_option_key_bundle(
        theme,
        fonts,
        i18n.tr(key, fallback),
        value,
        SelectionVisualState::Idle,
        UiI18nText::new(key, fallback),
    )
}

pub(crate) fn selected_segment_option_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    value: impl Into<String>,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    (
        segment_option_key_bundle(
            theme,
            fonts,
            i18n.tr(key, fallback),
            value,
            SelectionVisualState::Selected,
            UiI18nText::new(key, fallback),
        ),
        UiSegmentOptionSelected,
        SelectedButton,
    )
}

pub(crate) fn disabled_segment_option_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    value: impl Into<String>,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    (
        segment_option_key_bundle(
            theme,
            fonts,
            i18n.tr(key, fallback),
            value,
            SelectionVisualState::Disabled,
            UiI18nText::new(key, fallback),
        ),
        DisabledButton,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SelectionVisualState {
    Idle,
    Selected,
    Disabled,
}

pub(crate) fn selection_button<T: Bundle>(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
    colors: ButtonColors,
    marker: T,
    state: SelectionVisualState,
) -> impl Bundle {
    let text = text.into();

    (
        Button,
        FocusableButton,
        UiSelectionLabel {
            base_text: text.clone(),
        },
        marker,
        UiThemeButtonNodeRole::Button,
        Node {
            min_width: px(theme.button.min_width),
            height: px(theme.button.height),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::axes(px(theme.button.padding_x), px(0)),
            border_radius: BorderRadius::all(px(theme.button.radius)),
            ..default()
        },
        BackgroundColor(selection_button_background_color(
            colors,
            Interaction::None,
            false,
            state,
        )),
        children![(
            Text::new(selection_display_text(&text, state)),
            TextFont {
                font: fonts.regular.clone(),
                font_size: theme.text.button,
                ..default()
            },
            TextColor(selection_button_text_color(theme, state)),
            selection_button_text_color_role(state),
            UiThemeTextStyleRole::Button,
        )],
    )
}

pub(crate) fn selection_button_key_bundle<T: Bundle>(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
    colors: ButtonColors,
    marker: T,
    state: SelectionVisualState,
    i18n_text: UiI18nText,
) -> impl Bundle {
    let text = text.into();

    (
        Button,
        FocusableButton,
        UiSelectionLabel {
            base_text: text.clone(),
        },
        marker,
        UiThemeButtonNodeRole::Button,
        Node {
            min_width: px(theme.button.min_width),
            height: px(theme.button.height),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::axes(px(theme.button.padding_x), px(0)),
            border_radius: BorderRadius::all(px(theme.button.radius)),
            ..default()
        },
        BackgroundColor(selection_button_background_color(
            colors,
            Interaction::None,
            false,
            state,
        )),
        children![(
            Text::new(selection_display_text(&text, state)),
            TextFont {
                font: fonts.regular.clone(),
                font_size: theme.text.button,
                ..default()
            },
            TextColor(selection_button_text_color(theme, state)),
            selection_button_text_color_role(state),
            UiThemeTextStyleRole::Button,
            i18n_text,
        )],
    )
}

pub(crate) fn segment_option_key_bundle(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
    value: impl Into<String>,
    state: SelectionVisualState,
    i18n_text: UiI18nText,
) -> impl Bundle {
    selection_button_key_bundle(
        theme,
        fonts,
        text,
        theme.colors.secondary_button,
        (
            SecondaryButton,
            UiSegmentOption {
                value: value.into(),
            },
        ),
        state,
        i18n_text,
    )
}

pub(crate) fn selection_button_background_color(
    colors: ButtonColors,
    interaction: Interaction,
    is_focused: bool,
    state: SelectionVisualState,
) -> Color {
    button_background_color(
        colors,
        interaction,
        state == SelectionVisualState::Disabled,
        is_focused,
        state == SelectionVisualState::Selected,
        false,
    )
}

pub(crate) fn selection_button_text_color(theme: &UiTheme, state: SelectionVisualState) -> Color {
    selection_button_text_color_role(state).color(theme)
}

pub(crate) fn selection_button_text_color_role(
    state: SelectionVisualState,
) -> UiThemeTextColorRole {
    match state {
        SelectionVisualState::Disabled => UiThemeTextColorRole::Muted,
        SelectionVisualState::Idle | SelectionVisualState::Selected => {
            UiThemeTextColorRole::Primary
        }
    }
}

pub(crate) fn selection_display_text(base_text: &str, state: SelectionVisualState) -> String {
    match state {
        SelectionVisualState::Selected => format!("[x] {base_text}"),
        SelectionVisualState::Idle => format!("[ ] {base_text}"),
        SelectionVisualState::Disabled => format!("[-] {base_text}"),
    }
}

pub(crate) fn update_selection_control_interactions(
    mut commands: Commands,
    parents: Query<&ChildOf>,
    segmented_roots: Query<(), With<UiSegmentedControl>>,
    segment_options: Query<Entity, (With<UiSegmentOption>, With<UiSegmentOptionSelected>)>,
    buttons: Query<
        (
            Has<UiCheckbox>,
            Has<UiCheckboxChecked>,
            Has<UiToggle>,
            Has<UiToggleOn>,
            Has<UiSegmentOption>,
        ),
        (
            With<Button>,
            Without<DisabledButton>,
            Without<LoadingButton>,
            Without<UiStepper>,
        ),
    >,
    mut button_events: MessageReader<UiButtonEvent>,
) {
    for event in button_events.read() {
        if event.kind != UiButtonEventKind::Click {
            continue;
        }

        let Ok((is_checkbox, is_checked, is_toggle, is_toggle_on, is_segment_option)) =
            buttons.get(event.entity)
        else {
            continue;
        };

        if is_checkbox {
            if is_checked {
                commands
                    .entity(event.entity)
                    .remove::<UiCheckboxChecked>()
                    .remove::<SelectedButton>();
            } else {
                commands
                    .entity(event.entity)
                    .insert((UiCheckboxChecked, SelectedButton));
            }
        } else if is_toggle {
            if is_toggle_on {
                commands
                    .entity(event.entity)
                    .remove::<UiToggleOn>()
                    .remove::<SelectedButton>();
            } else {
                commands
                    .entity(event.entity)
                    .insert((UiToggleOn, SelectedButton));
            }
        } else if is_segment_option {
            let root = parents
                .iter_ancestors(event.entity)
                .find(|ancestor| segmented_roots.contains(*ancestor));

            for selected_entity in &segment_options {
                if selected_entity == event.entity {
                    continue;
                }

                let same_root = root.is_some_and(|root| {
                    parents
                        .iter_ancestors(selected_entity)
                        .any(|ancestor| ancestor == root)
                });
                if same_root {
                    commands
                        .entity(selected_entity)
                        .remove::<UiSegmentOptionSelected>()
                        .remove::<SelectedButton>();
                }
            }

            commands
                .entity(event.entity)
                .insert((UiSegmentOptionSelected, SelectedButton));
        }
    }
}
pub(crate) fn sync_selection_control_visuals(
    theme: Res<UiTheme>,
    mut controls: Query<
        (
            Entity,
            &Interaction,
            &UiSelectionLabel,
            &mut BackgroundColor,
            Has<FocusedButton>,
            Has<DisabledButton>,
            Has<UiCheckboxChecked>,
            Has<UiToggleOn>,
            Has<UiSegmentOptionSelected>,
            Has<UiCheckbox>,
            Has<UiToggle>,
            Has<UiSegmentOption>,
        ),
        With<Button>,
    >,
    children: Query<&Children>,
    mut texts: Query<&mut Text>,
) {
    for (
        entity,
        interaction,
        label,
        mut background,
        is_focused,
        is_disabled,
        is_checked,
        is_toggle_on,
        is_segment_selected,
        is_checkbox,
        is_toggle,
        is_segment_option,
    ) in &mut controls
    {
        if !is_checkbox && !is_toggle && !is_segment_option {
            continue;
        }

        let state = if is_disabled {
            SelectionVisualState::Disabled
        } else if is_checked || is_toggle_on || is_segment_selected {
            SelectionVisualState::Selected
        } else {
            SelectionVisualState::Idle
        };

        let colors = if is_toggle_on {
            theme.colors.primary_button
        } else {
            theme.colors.secondary_button
        };
        let next_background =
            selection_button_background_color(colors, *interaction, is_focused, state);
        if background.0 != next_background {
            *background = BackgroundColor(next_background);
        }

        let display = selection_display_text(&label.base_text, state);
        for child in children.iter_descendants(entity) {
            let Ok(mut text) = texts.get_mut(child) else {
                continue;
            };
            if text.0 != display {
                text.0 = display.clone();
            }
        }
    }
}
