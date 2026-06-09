use bevy::{
    input::keyboard::{Key, KeyCode, KeyboardInput},
    prelude::*,
};

use crate::game::{
    navigation::{AppUiMode, RouteButton},
    ui::{
        core::{UiFocusSystems, focus::UiFocusState},
        i18n::{UiI18n, UiI18nText},
        style::{
            UiFontAssets,
            theme::{
                ButtonColors, UiTheme, UiThemeButtonNodeRole, UiThemeTextColorRole,
                UiThemeTextStyleRole,
            },
        },
        widgets::scroll::UiScrollPlugin,
    },
};

pub(in crate::game) struct UiWidgetsPlugin;

impl Plugin for UiWidgetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(UiScrollPlugin)
            .init_resource::<UiTextInputClipboard>()
            .add_message::<UiTextInputSubmitted>()
            .add_systems(
                Update,
                handle_text_input_keyboard
                    .after(UiFocusSystems::SyncFocusedMarkers)
                    .before(UiFocusSystems::Visuals),
            )
            .add_systems(
                Update,
                (
                    sync_text_input_display,
                    sync_text_input_form_messages,
                    update_button_visuals,
                    update_text_input_visuals,
                )
                    .in_set(UiFocusSystems::Visuals),
            );
    }
}

#[derive(Component)]
pub(in crate::game) struct PrimaryButton;

#[derive(Component)]
pub(in crate::game) struct SecondaryButton;

#[derive(Component)]
pub(in crate::game) struct DisabledButton;

#[derive(Component)]
pub(in crate::game) struct FocusableButton;

#[derive(Component)]
pub(in crate::game) struct FocusedButton;

#[derive(Component)]
pub(in crate::game) struct SelectedButton;

#[derive(Component)]
pub(in crate::game) struct LoadingButton;

#[derive(Component)]
pub(in crate::game) struct UiCheckbox;

#[derive(Component)]
pub(in crate::game) struct UiCheckboxChecked;

#[derive(Component)]
pub(in crate::game) struct UiToggle;

#[derive(Component)]
pub(in crate::game) struct UiToggleOn;

#[derive(Component)]
pub(in crate::game) struct UiSegmentedControl;

#[derive(Clone, Debug, Component)]
#[allow(dead_code)]
pub(in crate::game) struct UiSegmentOption {
    pub value: String,
}

#[derive(Component)]
pub(in crate::game) struct UiSegmentOptionSelected;

#[derive(Component)]
pub(in crate::game) struct UiTextInput;

#[derive(Clone, Debug, Default, Component)]
pub(in crate::game) struct UiTextInputValue(pub String);

#[derive(Clone, Debug, Default, Component)]
pub(in crate::game) struct UiTextInputCursor {
    position: usize,
    selection: Option<UiTextInputSelection>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct UiTextInputSelection {
    start: usize,
    end: usize,
}

#[derive(Clone, Copy, Debug, Component)]
pub(in crate::game) struct UiTextInputMaxChars(pub usize);

#[derive(Component)]
pub(in crate::game) struct ReadonlyTextInput;

#[derive(Component)]
pub(in crate::game) struct DisabledTextInput;

#[derive(Clone, Debug, Component)]
pub(in crate::game) struct UiTextInputRequired {
    message: String,
}

impl UiTextInputRequired {
    pub(in crate::game) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[derive(Component)]
pub(in crate::game) struct UiTextInputError;

#[derive(Clone, Debug, Default, Component)]
pub(in crate::game) struct UiTextInputHelperText(pub String);

#[derive(Clone, Debug, Default, Component)]
pub(in crate::game) struct UiTextInputValidationMessage(pub String);

#[derive(Clone, Debug, Default, Component)]
pub(in crate::game) struct UiTextInputPlaceholder(pub String);

#[derive(Component)]
pub(in crate::game) struct UiTextInputText;

#[derive(Clone, Copy, Debug, Component)]
pub(in crate::game) struct UiTextInputFormMessage {
    input: Entity,
}

#[derive(Debug, Default, Resource)]
struct UiTextInputClipboard {
    text: String,
}

#[derive(Clone, Debug, Message)]
pub(in crate::game) struct UiTextInputSubmitted {
    pub entity: Entity,
    pub value: String,
}

pub(in crate::game) fn screen_title(
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

pub(in crate::game) fn screen_title_key(
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

pub(in crate::game) fn screen_label(
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

pub(in crate::game) fn screen_label_key(
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
pub(in crate::game) fn primary_route_button(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
    target: AppUiMode,
) -> impl Bundle {
    route_button(
        theme,
        fonts,
        text,
        target,
        theme.colors.primary_button,
        PrimaryButton,
    )
}

pub(in crate::game) fn primary_route_button_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
    target: AppUiMode,
) -> impl Bundle {
    route_button_key_bundle(
        theme,
        fonts,
        i18n.tr(key, fallback),
        target,
        theme.colors.primary_button,
        PrimaryButton,
        UiI18nText::new(key, fallback),
    )
}

#[allow(dead_code)]
pub(in crate::game) fn secondary_route_button(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
    target: AppUiMode,
) -> impl Bundle {
    route_button(
        theme,
        fonts,
        text,
        target,
        theme.colors.secondary_button,
        SecondaryButton,
    )
}

pub(in crate::game) fn secondary_route_button_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
    target: AppUiMode,
) -> impl Bundle {
    route_button_key_bundle(
        theme,
        fonts,
        i18n.tr(key, fallback),
        target,
        theme.colors.secondary_button,
        SecondaryButton,
        UiI18nText::new(key, fallback),
    )
}

pub(in crate::game) fn primary_action_button(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
) -> impl Bundle {
    action_button(
        theme,
        fonts,
        text,
        theme.colors.primary_button,
        PrimaryButton,
    )
}

pub(in crate::game) fn primary_action_button_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    action_button_key_bundle(
        theme,
        fonts,
        i18n.tr(key, fallback),
        theme.colors.primary_button,
        PrimaryButton,
        UiI18nText::new(key, fallback),
    )
}

pub(in crate::game) fn primary_action_button_with_i18n_text(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
    i18n_text: UiI18nText,
) -> impl Bundle {
    action_button_key_bundle(
        theme,
        fonts,
        text,
        theme.colors.primary_button,
        PrimaryButton,
        i18n_text,
    )
}

pub(in crate::game) fn secondary_action_button(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
) -> impl Bundle {
    action_button(
        theme,
        fonts,
        text,
        theme.colors.secondary_button,
        SecondaryButton,
    )
}

pub(in crate::game) fn secondary_action_button_with_i18n_text(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
    i18n_text: UiI18nText,
) -> impl Bundle {
    action_button_key_bundle(
        theme,
        fonts,
        text,
        theme.colors.secondary_button,
        SecondaryButton,
        i18n_text,
    )
}

pub(in crate::game) fn secondary_action_button_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    action_button_key_bundle(
        theme,
        fonts,
        i18n.tr(key, fallback),
        theme.colors.secondary_button,
        SecondaryButton,
        UiI18nText::new(key, fallback),
    )
}

#[allow(dead_code)]
pub(in crate::game) fn disabled_primary_action_button(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
) -> impl Bundle {
    disabled_action_button(
        theme,
        fonts,
        text,
        theme.colors.primary_button,
        PrimaryButton,
    )
}

pub(in crate::game) fn disabled_primary_action_button_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    disabled_action_button_key_bundle(
        theme,
        fonts,
        i18n.tr(key, fallback),
        theme.colors.primary_button,
        PrimaryButton,
        UiI18nText::new(key, fallback),
    )
}

#[allow(dead_code)]
pub(in crate::game) fn disabled_secondary_action_button(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
) -> impl Bundle {
    disabled_action_button(
        theme,
        fonts,
        text,
        theme.colors.secondary_button,
        SecondaryButton,
    )
}

pub(in crate::game) fn disabled_secondary_action_button_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    disabled_action_button_key_bundle(
        theme,
        fonts,
        i18n.tr(key, fallback),
        theme.colors.secondary_button,
        SecondaryButton,
        UiI18nText::new(key, fallback),
    )
}

#[allow(dead_code)]
pub(in crate::game) fn loading_primary_action_button(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
) -> impl Bundle {
    (
        action_button(
            theme,
            fonts,
            text,
            theme.colors.primary_button,
            PrimaryButton,
        ),
        LoadingButton,
    )
}

pub(in crate::game) fn loading_primary_action_button_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    (
        action_button_key_bundle(
            theme,
            fonts,
            i18n.tr(key, fallback),
            theme.colors.primary_button,
            PrimaryButton,
            UiI18nText::new(key, fallback),
        ),
        LoadingButton,
    )
}

#[allow(dead_code)]
pub(in crate::game) fn checkbox(
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

pub(in crate::game) fn checkbox_key(
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
pub(in crate::game) fn checked_checkbox(
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

pub(in crate::game) fn checked_checkbox_key(
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

pub(in crate::game) fn disabled_checkbox_key(
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
pub(in crate::game) fn toggle(
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

pub(in crate::game) fn toggle_key(
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
pub(in crate::game) fn toggle_on(
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

pub(in crate::game) fn toggle_on_key(
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

pub(in crate::game) fn disabled_toggle_key(
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

pub(in crate::game) fn segmented_control(theme: &UiTheme) -> impl Bundle {
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

pub(in crate::game) fn segment_option_key(
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

pub(in crate::game) fn selected_segment_option_key(
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

pub(in crate::game) fn disabled_segment_option_key(
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

pub(in crate::game) fn text_input(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    placeholder: impl Into<String>,
    value: impl Into<String>,
) -> impl Bundle {
    let value = value.into();
    let placeholder = placeholder.into();
    let initial_cursor_position = value.len();
    let display_text = if value.is_empty() {
        placeholder.clone()
    } else {
        value.clone()
    };
    let display_color = if value.is_empty() {
        theme.colors.text_muted
    } else {
        theme.colors.text_primary
    };

    (
        Button,
        FocusableButton,
        UiTextInput,
        UiTextInputValue(value),
        UiTextInputCursor {
            position: initial_cursor_position,
            selection: None,
        },
        UiTextInputPlaceholder(placeholder),
        UiThemeButtonNodeRole::TextInput,
        Node {
            width: percent(100),
            min_height: px(theme.button.height),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::FlexStart,
            padding: UiRect::axes(px(theme.button.padding_x), px(0)),
            border: UiRect::all(px(theme.panel.border)),
            border_radius: BorderRadius::all(px(theme.button.radius)),
            ..default()
        },
        BackgroundColor(text_input_background_color(
            theme,
            Interaction::None,
            false,
            false,
        )),
        BorderColor::all(text_input_border_color(
            theme,
            Interaction::None,
            false,
            false,
            false,
        )),
        children![(
            Text::new(display_text),
            TextFont {
                font: fonts.regular.clone(),
                font_size: theme.text.button,
                ..default()
            },
            TextColor(display_color),
            UiTextInputText,
            UiThemeTextStyleRole::Button,
        )],
    )
}

pub(in crate::game) fn text_input_form_message(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    input: Entity,
) -> impl Bundle {
    (
        Text::new(""),
        TextFont {
            font: fonts.regular.clone(),
            font_size: theme.text.caption,
            ..default()
        },
        TextColor(theme.colors.text_muted),
        UiTextInputFormMessage { input },
        UiThemeTextStyleRole::Caption,
    )
}

fn route_button<T: Component>(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
    target: AppUiMode,
    colors: ButtonColors,
    marker: T,
) -> impl Bundle {
    (
        Button,
        FocusableButton,
        RouteButton { target },
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

fn route_button_key_bundle<T: Component>(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
    target: AppUiMode,
    colors: ButtonColors,
    marker: T,
    i18n_text: UiI18nText,
) -> impl Bundle {
    (
        Button,
        FocusableButton,
        RouteButton { target },
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

fn action_button<T: Component>(
    theme: &UiTheme,
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
        Node {
            min_width: px(theme.button.min_width),
            height: px(theme.button.height),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::axes(px(theme.button.padding_x), px(0)),
            border_radius: BorderRadius::all(px(theme.button.radius)),
            ..default()
        },
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

fn action_button_key_bundle<T: Component>(
    theme: &UiTheme,
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
        Node {
            min_width: px(theme.button.min_width),
            height: px(theme.button.height),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::axes(px(theme.button.padding_x), px(0)),
            border_radius: BorderRadius::all(px(theme.button.radius)),
            ..default()
        },
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
fn disabled_action_button<T: Component>(
    theme: &UiTheme,
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
        Node {
            min_width: px(theme.button.min_width),
            height: px(theme.button.height),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::axes(px(theme.button.padding_x), px(0)),
            border_radius: BorderRadius::all(px(theme.button.radius)),
            ..default()
        },
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

fn disabled_action_button_key_bundle<T: Component>(
    theme: &UiTheme,
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
        Node {
            min_width: px(theme.button.min_width),
            height: px(theme.button.height),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::axes(px(theme.button.padding_x), px(0)),
            border_radius: BorderRadius::all(px(theme.button.radius)),
            ..default()
        },
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
enum SelectionVisualState {
    Idle,
    Selected,
    Disabled,
}

fn selection_button<T: Bundle>(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
    colors: ButtonColors,
    marker: T,
    state: SelectionVisualState,
) -> impl Bundle {
    (
        Button,
        FocusableButton,
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
            Text::new(text),
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

fn selection_button_key_bundle<T: Bundle>(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
    colors: ButtonColors,
    marker: T,
    state: SelectionVisualState,
    i18n_text: UiI18nText,
) -> impl Bundle {
    (
        Button,
        FocusableButton,
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
            Text::new(text),
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

fn segment_option_key_bundle(
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

fn selection_button_background_color(
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

fn selection_button_text_color(theme: &UiTheme, state: SelectionVisualState) -> Color {
    selection_button_text_color_role(state).color(theme)
}

fn selection_button_text_color_role(state: SelectionVisualState) -> UiThemeTextColorRole {
    match state {
        SelectionVisualState::Disabled => UiThemeTextColorRole::Muted,
        SelectionVisualState::Idle | SelectionVisualState::Selected => {
            UiThemeTextColorRole::Primary
        }
    }
}

fn update_button_visuals(
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
        (With<Button>, Without<UiTextInput>),
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

        *background = button_background_color(
            colors,
            *interaction,
            is_disabled,
            is_focused,
            is_selected,
            is_loading,
        )
        .into();
    }
}

fn button_background_color(
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

fn handle_text_input_keyboard(
    mut keyboard_inputs: MessageReader<KeyboardInput>,
    key_codes: Res<ButtonInput<KeyCode>>,
    focus_state: Res<UiFocusState>,
    mut text_inputs: Query<
        (
            &mut UiTextInputValue,
            &mut UiTextInputCursor,
            Option<&UiTextInputMaxChars>,
            Has<ReadonlyTextInput>,
            Has<DisabledTextInput>,
        ),
        With<UiTextInput>,
    >,
    mut clipboard: ResMut<UiTextInputClipboard>,
    mut submissions: MessageWriter<UiTextInputSubmitted>,
) {
    let Some(focused_entity) = focus_state.focused_entity else {
        for _ in keyboard_inputs.read() {}
        return;
    };

    let Ok((mut value, mut cursor, max_chars, is_readonly, is_disabled)) =
        text_inputs.get_mut(focused_entity)
    else {
        for _ in keyboard_inputs.read() {}
        return;
    };

    let mode = UiTextInputEditMode {
        readonly: is_readonly,
        disabled: is_disabled,
        max_chars: max_chars.map(|max_chars| max_chars.0),
    };

    for keyboard_input in keyboard_inputs.read() {
        if !keyboard_input.state.is_pressed() {
            continue;
        }

        let edit_event = ui_text_input_edit_event(keyboard_input, &key_codes);
        match edit_event {
            UiTextInputEditEvent::Submit => {
                if is_readonly || is_disabled {
                    continue;
                }

                submissions.write(UiTextInputSubmitted {
                    entity: focused_entity,
                    value: value.0.clone(),
                });
            }
            UiTextInputEditEvent::Copy => {
                if is_disabled {
                    continue;
                }

                clipboard.text =
                    selected_text(&value.0, &cursor).unwrap_or_else(|| value.0.clone());
            }
            UiTextInputEditEvent::Paste => {
                let clipboard_text = clipboard.text.clone();
                apply_text_input_edit(
                    &mut value.0,
                    &mut cursor,
                    UiTextInputEditAction::Paste(&clipboard_text),
                    mode,
                );
            }
            UiTextInputEditEvent::Edit(action) => {
                apply_text_input_edit(&mut value.0, &mut cursor, action, mode);
            }
            UiTextInputEditEvent::None => {}
        }
    }
}

fn sync_text_input_display(
    theme: Res<UiTheme>,
    focus_state: Res<UiFocusState>,
    parents: Query<&ChildOf>,
    text_inputs: Query<
        (
            &UiTextInputValue,
            &UiTextInputPlaceholder,
            &UiTextInputCursor,
            Has<DisabledTextInput>,
        ),
        With<UiTextInput>,
    >,
    mut texts: Query<(Entity, &mut Text, &mut TextColor), With<UiTextInputText>>,
) {
    for (text_entity, mut text, mut text_color) in &mut texts {
        let Some(input_entity) = parents
            .iter_ancestors(text_entity)
            .find(|ancestor| text_inputs.get(*ancestor).is_ok())
        else {
            continue;
        };

        let Ok((value, placeholder, cursor, is_disabled)) = text_inputs.get(input_entity) else {
            continue;
        };

        let is_focused = focus_state.focused_entity == Some(input_entity);
        let display = if value.0.is_empty() && !is_focused {
            placeholder.0.clone()
        } else if is_focused && !is_disabled {
            text_input_display_with_cursor(&value.0, cursor)
        } else {
            value.0.clone()
        };
        let color = if is_disabled || value.0.is_empty() && !is_focused {
            theme.colors.text_muted
        } else {
            theme.colors.text_primary
        };

        if text.0 != display {
            text.0 = display;
        }
        if text_color.0 != color {
            text_color.0 = color;
        }
    }
}

fn sync_text_input_form_messages(
    theme: Res<UiTheme>,
    text_inputs: Query<(
        &UiTextInputValue,
        Option<&UiTextInputHelperText>,
        Option<&UiTextInputValidationMessage>,
        Option<&UiTextInputRequired>,
        Has<UiTextInputError>,
        Has<DisabledTextInput>,
    )>,
    mut messages: Query<(&UiTextInputFormMessage, &mut Text, &mut TextColor)>,
) {
    for (message, mut text, mut text_color) in &mut messages {
        let Ok((value, helper_text, validation_message, required, has_error, is_disabled)) =
            text_inputs.get(message.input)
        else {
            continue;
        };

        let state = text_input_form_state(
            &value.0,
            helper_text.map(|helper| helper.0.as_str()),
            validation_message.map(|validation| validation.0.as_str()),
            required,
            has_error,
        );
        let display = state.message.unwrap_or_default();
        let color = if is_disabled {
            theme.colors.text_muted
        } else if state.is_error {
            theme.colors.text_error
        } else {
            theme.colors.text_muted
        };

        if text.0 != display {
            text.0 = display;
        }
        if text_color.0 != color {
            text_color.0 = color;
        }
    }
}

fn update_text_input_visuals(
    theme: Res<UiTheme>,
    mut text_inputs: Query<
        (
            &Interaction,
            &mut BackgroundColor,
            &mut BorderColor,
            Has<FocusedButton>,
            Has<DisabledTextInput>,
            Has<UiTextInputError>,
            &UiTextInputValue,
            Option<&UiTextInputValidationMessage>,
            Option<&UiTextInputRequired>,
        ),
        (With<Button>, With<UiTextInput>),
    >,
) {
    for (
        interaction,
        mut background,
        mut border,
        is_focused,
        is_disabled,
        has_error,
        value,
        validation_message,
        required,
    ) in &mut text_inputs
    {
        let is_error = text_input_has_error(
            &value.0,
            validation_message.map(|message| message.0.as_str()),
            required,
            has_error,
        );
        let background_color =
            text_input_background_color(&theme, *interaction, is_focused, is_disabled);
        if background.0 != background_color {
            *background = BackgroundColor(background_color);
        }

        *border = BorderColor::all(text_input_border_color(
            &theme,
            *interaction,
            is_focused,
            is_disabled,
            is_error,
        ));
    }
}

fn text_input_background_color(
    theme: &UiTheme,
    interaction: Interaction,
    is_focused: bool,
    is_disabled: bool,
) -> Color {
    if is_disabled {
        return theme.colors.secondary_button.disabled;
    }

    match interaction {
        Interaction::Pressed => theme.colors.secondary_button.pressed,
        Interaction::Hovered => theme.colors.secondary_button.hovered,
        Interaction::None if is_focused => theme.colors.secondary_button.focused,
        Interaction::None => theme.colors.secondary_button.idle,
    }
}

fn text_input_border_color(
    theme: &UiTheme,
    interaction: Interaction,
    is_focused: bool,
    is_disabled: bool,
    is_error: bool,
) -> Color {
    if is_disabled {
        return theme.colors.secondary_button.disabled;
    }

    if is_error {
        return theme.colors.error;
    }

    match interaction {
        Interaction::Pressed => theme.colors.primary_button.pressed,
        Interaction::Hovered if is_focused => theme.colors.primary_button.focused,
        Interaction::Hovered => theme.colors.secondary_button.focused,
        Interaction::None if is_focused => theme.colors.primary_button.focused,
        Interaction::None => theme.colors.panel_border,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct UiTextInputFormState {
    message: Option<String>,
    is_error: bool,
}

fn text_input_form_state(
    value: &str,
    helper_text: Option<&str>,
    validation_message: Option<&str>,
    required: Option<&UiTextInputRequired>,
    has_error: bool,
) -> UiTextInputFormState {
    if let Some(message) = validation_message.filter(|message| !message.is_empty()) {
        return UiTextInputFormState {
            message: Some(message.to_string()),
            is_error: true,
        };
    }

    if has_error {
        return UiTextInputFormState {
            message: None,
            is_error: true,
        };
    }

    if let Some(required) = required
        && value.is_empty()
    {
        return UiTextInputFormState {
            message: (!required.message.is_empty()).then(|| required.message.clone()),
            is_error: true,
        };
    }

    UiTextInputFormState {
        message: helper_text
            .filter(|message| !message.is_empty())
            .map(str::to_string),
        is_error: false,
    }
}

fn text_input_has_error(
    value: &str,
    validation_message: Option<&str>,
    required: Option<&UiTextInputRequired>,
    has_error: bool,
) -> bool {
    text_input_form_state(value, None, validation_message, required, has_error).is_error
}

#[derive(Clone, Copy)]
struct UiTextInputEditMode {
    readonly: bool,
    disabled: bool,
    max_chars: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UiTextInputEditAction<'a> {
    Insert(&'a str),
    Paste(&'a str),
    Backspace,
    Delete,
    MoveLeft,
    MoveRight,
    MoveHome,
    MoveEnd,
    SelectAll,
}

enum UiTextInputEditEvent<'a> {
    Edit(UiTextInputEditAction<'a>),
    Copy,
    Paste,
    Submit,
    None,
}

fn ui_text_input_edit_event<'a>(
    keyboard_input: &'a KeyboardInput,
    key_codes: &ButtonInput<KeyCode>,
) -> UiTextInputEditEvent<'a> {
    let is_control_pressed = key_codes.any_pressed([
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
    ]);

    if is_control_pressed {
        match keyboard_input.key_code {
            KeyCode::KeyA => return UiTextInputEditEvent::Edit(UiTextInputEditAction::SelectAll),
            KeyCode::KeyC => return UiTextInputEditEvent::Copy,
            KeyCode::KeyV => return UiTextInputEditEvent::Paste,
            _ => {}
        }
    }

    match &keyboard_input.logical_key {
        Key::Enter => UiTextInputEditEvent::Submit,
        Key::Backspace => UiTextInputEditEvent::Edit(UiTextInputEditAction::Backspace),
        Key::Delete => UiTextInputEditEvent::Edit(UiTextInputEditAction::Delete),
        Key::ArrowLeft => UiTextInputEditEvent::Edit(UiTextInputEditAction::MoveLeft),
        Key::ArrowRight => UiTextInputEditEvent::Edit(UiTextInputEditAction::MoveRight),
        Key::Home => UiTextInputEditEvent::Edit(UiTextInputEditAction::MoveHome),
        Key::End => UiTextInputEditEvent::Edit(UiTextInputEditAction::MoveEnd),
        Key::Space => {
            if is_control_pressed {
                UiTextInputEditEvent::None
            } else {
                UiTextInputEditEvent::Edit(UiTextInputEditAction::Insert(
                    keyboard_input.text.as_deref().unwrap_or(" "),
                ))
            }
        }
        _ => {
            if is_control_pressed {
                return UiTextInputEditEvent::None;
            }

            if let Some(inserted_text) = keyboard_input
                .text
                .as_deref()
                .filter(|text| text.chars().all(is_printable_char))
            {
                UiTextInputEditEvent::Edit(UiTextInputEditAction::Insert(inserted_text))
            } else {
                UiTextInputEditEvent::None
            }
        }
    }
}

fn apply_text_input_edit(
    value: &mut String,
    cursor: &mut UiTextInputCursor,
    action: UiTextInputEditAction,
    mode: UiTextInputEditMode,
) {
    clamp_text_input_cursor(value, cursor);

    if mode.disabled {
        return;
    }

    match action {
        UiTextInputEditAction::MoveLeft => {
            cursor.selection = None;
            cursor.position = previous_char_boundary(value, cursor.position);
        }
        UiTextInputEditAction::MoveRight => {
            cursor.selection = None;
            cursor.position = next_char_boundary(value, cursor.position);
        }
        UiTextInputEditAction::MoveHome => {
            cursor.selection = None;
            cursor.position = 0;
        }
        UiTextInputEditAction::MoveEnd => {
            cursor.selection = None;
            cursor.position = value.len();
        }
        UiTextInputEditAction::SelectAll => {
            cursor.position = value.len();
            cursor.selection = (!value.is_empty()).then_some(UiTextInputSelection {
                start: 0,
                end: value.len(),
            });
        }
        UiTextInputEditAction::Insert(text) | UiTextInputEditAction::Paste(text) => {
            if mode.readonly {
                return;
            }

            replace_selection_or_insert(value, cursor, text, mode.max_chars);
        }
        UiTextInputEditAction::Backspace => {
            if mode.readonly {
                return;
            }

            if delete_selection(value, cursor) {
                return;
            }

            let delete_from = previous_char_boundary(value, cursor.position);
            if delete_from != cursor.position {
                value.replace_range(delete_from..cursor.position, "");
                cursor.position = delete_from;
            }
        }
        UiTextInputEditAction::Delete => {
            if mode.readonly {
                return;
            }

            if delete_selection(value, cursor) {
                return;
            }

            let delete_to = next_char_boundary(value, cursor.position);
            if delete_to != cursor.position {
                value.replace_range(cursor.position..delete_to, "");
            }
        }
    }
}

fn replace_selection_or_insert(
    value: &mut String,
    cursor: &mut UiTextInputCursor,
    text: &str,
    max_chars: Option<usize>,
) {
    let (selection_start, selection_end) = selection_range(cursor)
        .map(|selection| (selection.start, selection.end))
        .unwrap_or((cursor.position, cursor.position));
    let selected_chars = value[selection_start..selection_end].chars().count();
    let current_chars = value.chars().count();
    let available_chars = max_chars
        .map(|max_chars| max_chars.saturating_sub(current_chars.saturating_sub(selected_chars)))
        .unwrap_or(usize::MAX);
    let inserted_text = text
        .chars()
        .filter(|chr| is_printable_char(*chr))
        .take(available_chars)
        .collect::<String>();

    value.replace_range(selection_start..selection_end, &inserted_text);
    cursor.position = selection_start + inserted_text.len();
    cursor.selection = None;
}

fn delete_selection(value: &mut String, cursor: &mut UiTextInputCursor) -> bool {
    let Some(selection) = selection_range(cursor) else {
        cursor.selection = None;
        return false;
    };

    value.replace_range(selection.start..selection.end, "");
    cursor.position = selection.start;
    cursor.selection = None;
    true
}

fn selected_text(value: &str, cursor: &UiTextInputCursor) -> Option<String> {
    let selection = selection_range(cursor)?;
    Some(value[selection.start..selection.end].to_string())
}

fn selection_range(cursor: &UiTextInputCursor) -> Option<UiTextInputSelection> {
    cursor
        .selection
        .filter(|selection| selection.start < selection.end)
}

fn clamp_text_input_cursor(value: &str, cursor: &mut UiTextInputCursor) {
    cursor.position = nearest_char_boundary(value, cursor.position.min(value.len()));

    cursor.selection = cursor.selection.and_then(|selection| {
        let start = nearest_char_boundary(value, selection.start.min(value.len()));
        let end = nearest_char_boundary(value, selection.end.min(value.len()));
        (start < end).then_some(UiTextInputSelection { start, end })
    });
}

fn previous_char_boundary(value: &str, position: usize) -> usize {
    if position == 0 {
        return 0;
    }

    value[..position]
        .char_indices()
        .last()
        .map(|(index, _)| index)
        .unwrap_or(0)
}

fn next_char_boundary(value: &str, position: usize) -> usize {
    value[position..]
        .char_indices()
        .nth(1)
        .map(|(offset, _)| position + offset)
        .unwrap_or(value.len())
}

fn nearest_char_boundary(value: &str, position: usize) -> usize {
    let mut position = position.min(value.len());
    while position > 0 && !value.is_char_boundary(position) {
        position -= 1;
    }
    position
}

fn text_input_display_with_cursor(value: &str, cursor: &UiTextInputCursor) -> String {
    let cursor_position = nearest_char_boundary(value, cursor.position.min(value.len()));
    let mut display = String::with_capacity(value.len() + 1);
    display.push_str(&value[..cursor_position]);
    display.push('|');
    display.push_str(&value[cursor_position..]);
    display
}

fn is_printable_char(chr: char) -> bool {
    let is_in_private_use_area = ('\u{e000}'..='\u{f8ff}').contains(&chr)
        || ('\u{f0000}'..='\u{ffffd}').contains(&chr)
        || ('\u{100000}'..='\u{10fffd}').contains(&chr);

    !is_in_private_use_area && !chr.is_ascii_control()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editable(max_chars: Option<usize>) -> UiTextInputEditMode {
        UiTextInputEditMode {
            readonly: false,
            disabled: false,
            max_chars,
        }
    }

    fn readonly() -> UiTextInputEditMode {
        UiTextInputEditMode {
            readonly: true,
            disabled: false,
            max_chars: None,
        }
    }

    fn disabled() -> UiTextInputEditMode {
        UiTextInputEditMode {
            readonly: false,
            disabled: true,
            max_chars: None,
        }
    }

    fn cursor(position: usize) -> UiTextInputCursor {
        UiTextInputCursor {
            position,
            selection: None,
        }
    }

    fn required(message: &str) -> UiTextInputRequired {
        UiTextInputRequired::new(message)
    }

    #[test]
    fn insert_adds_text_at_cursor() {
        let mut value = "ab".to_string();
        let mut cursor = cursor(1);

        apply_text_input_edit(
            &mut value,
            &mut cursor,
            UiTextInputEditAction::Insert("X"),
            editable(None),
        );

        assert_eq!(value, "aXb");
        assert_eq!(cursor.position, 2);
    }

    #[test]
    fn cursor_moves_left_right_and_home_end() {
        let mut value = "abc".to_string();
        let mut cursor = cursor(value.len());

        apply_text_input_edit(
            &mut value,
            &mut cursor,
            UiTextInputEditAction::MoveLeft,
            editable(None),
        );
        assert_eq!(cursor.position, 2);

        apply_text_input_edit(
            &mut value,
            &mut cursor,
            UiTextInputEditAction::MoveRight,
            editable(None),
        );
        assert_eq!(cursor.position, 3);

        apply_text_input_edit(
            &mut value,
            &mut cursor,
            UiTextInputEditAction::MoveHome,
            editable(None),
        );
        assert_eq!(cursor.position, 0);

        apply_text_input_edit(
            &mut value,
            &mut cursor,
            UiTextInputEditAction::MoveEnd,
            editable(None),
        );
        assert_eq!(cursor.position, value.len());
    }

    #[test]
    fn backspace_deletes_before_cursor() {
        let mut value = "abc".to_string();
        let mut cursor = cursor(2);

        apply_text_input_edit(
            &mut value,
            &mut cursor,
            UiTextInputEditAction::Backspace,
            editable(None),
        );

        assert_eq!(value, "ac");
        assert_eq!(cursor.position, 1);
    }

    #[test]
    fn delete_removes_after_cursor() {
        let mut value = "abc".to_string();
        let mut cursor = cursor(1);

        apply_text_input_edit(
            &mut value,
            &mut cursor,
            UiTextInputEditAction::Delete,
            editable(None),
        );

        assert_eq!(value, "ac");
        assert_eq!(cursor.position, 1);
    }

    #[test]
    fn max_chars_limits_inserted_text() {
        let mut value = "ab".to_string();
        let mut cursor = cursor(value.len());

        apply_text_input_edit(
            &mut value,
            &mut cursor,
            UiTextInputEditAction::Insert("cde"),
            editable(Some(4)),
        );

        assert_eq!(value, "abcd");
        assert_eq!(cursor.position, value.len());
    }

    #[test]
    fn selected_text_is_replaced_and_counts_against_max_chars() {
        let mut value = "abcd".to_string();
        let mut cursor = UiTextInputCursor {
            position: 3,
            selection: Some(UiTextInputSelection { start: 1, end: 3 }),
        };

        apply_text_input_edit(
            &mut value,
            &mut cursor,
            UiTextInputEditAction::Insert("XYZ"),
            editable(Some(5)),
        );

        assert_eq!(value, "aXYZd");
        assert_eq!(cursor.position, 4);
    }

    #[test]
    fn readonly_does_not_edit_but_allows_cursor_movement() {
        let mut value = "abc".to_string();
        let mut cursor = cursor(2);

        apply_text_input_edit(
            &mut value,
            &mut cursor,
            UiTextInputEditAction::Insert("X"),
            readonly(),
        );
        apply_text_input_edit(
            &mut value,
            &mut cursor,
            UiTextInputEditAction::Backspace,
            readonly(),
        );

        assert_eq!(value, "abc");
        assert_eq!(cursor.position, 2);

        apply_text_input_edit(
            &mut value,
            &mut cursor,
            UiTextInputEditAction::MoveLeft,
            readonly(),
        );

        assert_eq!(value, "abc");
        assert_eq!(cursor.position, 1);
    }

    #[test]
    fn disabled_does_not_edit_or_move_cursor() {
        let mut value = "abc".to_string();
        let mut cursor = cursor(2);

        apply_text_input_edit(
            &mut value,
            &mut cursor,
            UiTextInputEditAction::Insert("X"),
            disabled(),
        );
        apply_text_input_edit(
            &mut value,
            &mut cursor,
            UiTextInputEditAction::MoveLeft,
            disabled(),
        );
        apply_text_input_edit(
            &mut value,
            &mut cursor,
            UiTextInputEditAction::Delete,
            disabled(),
        );

        assert_eq!(value, "abc");
        assert_eq!(cursor.position, 2);
    }

    #[test]
    fn utf8_cursor_uses_char_boundaries() {
        let mut value = "你a".to_string();
        let mut cursor = cursor(value.len());

        apply_text_input_edit(
            &mut value,
            &mut cursor,
            UiTextInputEditAction::MoveLeft,
            editable(None),
        );
        assert_eq!(cursor.position, "你".len());

        apply_text_input_edit(
            &mut value,
            &mut cursor,
            UiTextInputEditAction::Backspace,
            editable(None),
        );

        assert_eq!(value, "a");
        assert_eq!(cursor.position, 0);
    }

    #[test]
    fn helper_text_displays_when_input_has_no_error() {
        assert_eq!(
            text_input_form_state("Pilot", Some("Visible helper"), None, None, false),
            UiTextInputFormState {
                message: Some("Visible helper".to_string()),
                is_error: false,
            }
        );
    }

    #[test]
    fn validation_message_overrides_helper_and_required() {
        let required = required("Required");

        assert_eq!(
            text_input_form_state(
                "",
                Some("Helper"),
                Some("Validation failed"),
                Some(&required),
                false,
            ),
            UiTextInputFormState {
                message: Some("Validation failed".to_string()),
                is_error: true,
            }
        );
    }

    #[test]
    fn required_empty_value_generates_error_state() {
        let required = required("Required");

        assert_eq!(
            text_input_form_state("", Some("Helper"), None, Some(&required), false),
            UiTextInputFormState {
                message: Some("Required".to_string()),
                is_error: true,
            }
        );
        assert_eq!(
            text_input_form_state("Pilot", Some("Helper"), None, Some(&required), false),
            UiTextInputFormState {
                message: Some("Helper".to_string()),
                is_error: false,
            }
        );
    }

    #[test]
    fn disabled_border_color_overrides_error_state() {
        let theme = UiTheme::default();

        assert_eq!(
            text_input_border_color(&theme, Interaction::None, true, true, true),
            theme.colors.secondary_button.disabled
        );
        assert_eq!(
            text_input_border_color(&theme, Interaction::None, true, false, true),
            theme.colors.error
        );
    }

    #[test]
    fn selection_visual_state_prioritizes_disabled_and_selected_colors() {
        let colors = UiTheme::default().colors.secondary_button;

        assert_eq!(
            selection_button_background_color(
                colors,
                Interaction::Hovered,
                true,
                SelectionVisualState::Disabled,
            ),
            colors.disabled
        );
        assert_eq!(
            selection_button_background_color(
                colors,
                Interaction::None,
                false,
                SelectionVisualState::Selected,
            ),
            colors.selected
        );
        assert_eq!(
            selection_button_background_color(
                colors,
                Interaction::None,
                true,
                SelectionVisualState::Idle,
            ),
            colors.focused
        );
    }
}
