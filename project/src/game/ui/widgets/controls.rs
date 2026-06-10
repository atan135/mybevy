use bevy::{
    input::keyboard::{Key, KeyCode, KeyboardInput},
    prelude::*,
    ui::RelativeCursorPosition,
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

const NUMERIC_CONTROL_LABEL_WIDTH: f32 = 132.0;
const SLIDER_TRACK_HEIGHT: f32 = 8.0;
const STEPPER_VALUE_WIDTH: f32 = 72.0;
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
                    update_text_input_cursor_from_pointer,
                    update_selection_control_interactions,
                    update_slider_interactions,
                    update_stepper_interactions,
                    sync_selection_control_visuals,
                    sync_text_input_display,
                    sync_text_input_form_messages,
                    sync_numeric_control_display,
                    sync_icon_button_accessible_labels,
                    sync_icon_button_nodes,
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

#[derive(Clone, Debug, Component)]
#[allow(dead_code)]
pub(in crate::game) struct UiIconButton {
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

#[derive(Clone, Debug, Component)]
struct UiSelectionLabel {
    base_text: String,
}

#[derive(Clone, Copy, Debug, Component)]
pub(in crate::game) struct UiSlider {
    pub value: f32,
    pub min: f32,
    pub max: f32,
}

impl UiSlider {
    pub(in crate::game) fn new(value: f32, min: f32, max: f32) -> Self {
        let (min, max) = ordered_slider_bounds(min, max);
        Self {
            value: clamp_slider_value(value, min, max),
            min,
            max,
        }
    }

    fn ratio(self) -> f32 {
        slider_ratio(self.value, self.min, self.max)
    }
}

#[derive(Component)]
struct UiSliderFill;

#[derive(Component)]
struct UiSliderTrack;

#[derive(Component)]
struct UiSliderValueText;

#[derive(Clone, Copy, Debug, Component)]
pub(in crate::game) struct UiStepper {
    pub value: i32,
    pub min: i32,
    pub max: i32,
    pub step: i32,
}

impl UiStepper {
    pub(in crate::game) fn new(value: i32, min: i32, max: i32, step: i32) -> Self {
        let (min, max) = ordered_stepper_bounds(min, max);
        let step = stepper_step(step);
        Self {
            value: clamp_stepper_value(value, min, max),
            min,
            max,
            step,
        }
    }
}

#[derive(Component)]
struct UiStepperDecrementButton;

#[derive(Component)]
struct UiStepperIncrementButton;

#[derive(Component)]
struct UiStepperValueText;

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

#[derive(Clone, Debug, Component)]
pub(in crate::game) struct UiTextInputAlphanumeric {
    min_chars: usize,
    max_chars: usize,
    message: String,
}

impl UiTextInputAlphanumeric {
    pub(in crate::game) fn new(
        min_chars: usize,
        max_chars: usize,
        message: impl Into<String>,
    ) -> Self {
        let min_chars = min_chars.min(max_chars);
        Self {
            min_chars,
            max_chars,
            message: message.into(),
        }
    }

    fn validate<'a>(&'a self, value: &str) -> Option<&'a str> {
        let char_count = value.chars().count();
        let valid = (self.min_chars..=self.max_chars).contains(&char_count)
            && value.chars().all(|chr| chr.is_ascii_alphanumeric());

        (!valid).then_some(self.message.as_str())
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Component)]
enum UiTextInputTextPart {
    Plain,
    Selected,
    Tail,
}

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

#[allow(dead_code)]
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

#[allow(dead_code)]
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

#[allow(dead_code)]
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

#[allow(dead_code)]
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

pub(in crate::game) fn icon_button_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    icon: impl Into<String>,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    icon_button_key_bundle(
        theme,
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

pub(in crate::game) fn disabled_icon_button_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    icon: impl Into<String>,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    icon_button_key_bundle(
        theme,
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

pub(in crate::game) fn loading_icon_button_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    icon: impl Into<String>,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    icon_button_key_bundle(
        theme,
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

pub(in crate::game) fn slider_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
    value: f32,
    min: f32,
    max: f32,
) -> impl Bundle {
    slider_bundle(
        theme,
        fonts,
        i18n.tr(key, fallback),
        value,
        min,
        max,
        UiI18nText::new(key, fallback),
        (),
        false,
    )
}

pub(in crate::game) fn disabled_slider_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
    value: f32,
    min: f32,
    max: f32,
) -> impl Bundle {
    slider_bundle(
        theme,
        fonts,
        i18n.tr(key, fallback),
        value,
        min,
        max,
        UiI18nText::new(key, fallback),
        DisabledButton,
        true,
    )
}

pub(in crate::game) fn stepper_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
    value: i32,
    min: i32,
    max: i32,
    step: i32,
) -> impl Bundle {
    stepper_bundle(
        theme,
        fonts,
        i18n.tr(key, fallback),
        value,
        min,
        max,
        step,
        UiI18nText::new(key, fallback),
        (),
        UiStepperDecrementButton,
        UiStepperIncrementButton,
        false,
    )
}

pub(in crate::game) fn disabled_stepper_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
    value: i32,
    min: i32,
    max: i32,
    step: i32,
) -> impl Bundle {
    stepper_bundle(
        theme,
        fonts,
        i18n.tr(key, fallback),
        value,
        min,
        max,
        step,
        UiI18nText::new(key, fallback),
        DisabledButton,
        (UiStepperDecrementButton, DisabledButton),
        (UiStepperIncrementButton, DisabledButton),
        true,
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
        RelativeCursorPosition::default(),
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
            Text::new(""),
            TextFont {
                font: fonts.regular.clone(),
                font_size: theme.text.button,
                ..default()
            },
            TextColor(display_color),
            UiTextInputText,
            UiTextInputTextPart::Plain,
            UiThemeTextStyleRole::Button,
            children![
                (
                    TextSpan::new(display_text),
                    TextFont {
                        font: fonts.regular.clone(),
                        font_size: theme.text.button,
                        ..default()
                    },
                    TextColor(display_color),
                    TextBackgroundColor(Color::NONE),
                    UiTextInputTextPart::Plain,
                    UiThemeTextStyleRole::Button,
                ),
                (
                    TextSpan::new(""),
                    TextFont {
                        font: fonts.regular.clone(),
                        font_size: theme.text.button,
                        ..default()
                    },
                    TextColor(theme.colors.text_primary),
                    TextBackgroundColor(Color::NONE),
                    UiTextInputTextPart::Selected,
                    UiThemeTextStyleRole::Button,
                ),
                (
                    TextSpan::new(""),
                    TextFont {
                        font: fonts.regular.clone(),
                        font_size: theme.text.button,
                        ..default()
                    },
                    TextColor(display_color),
                    TextBackgroundColor(Color::NONE),
                    UiTextInputTextPart::Tail,
                    UiThemeTextStyleRole::Button,
                ),
            ],
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
enum IconButtonVisualState {
    Idle,
    Disabled,
    Loading,
}

fn icon_button_key_bundle<T: Bundle>(
    theme: &UiTheme,
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
        icon_button_node(theme),
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

fn selection_button_key_bundle<T: Bundle>(
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

fn slider_bundle<T: Bundle>(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    label: impl Into<String>,
    value: f32,
    min: f32,
    max: f32,
    label_i18n_text: UiI18nText,
    marker: T,
    disabled: bool,
) -> impl Bundle {
    let slider = UiSlider::new(value, min, max);
    let fill_color = if disabled {
        theme.colors.secondary_button.disabled
    } else {
        theme.colors.primary_button.idle
    };
    let value_color = if disabled {
        UiThemeTextColorRole::Muted
    } else {
        UiThemeTextColorRole::Primary
    };

    (
        Button,
        FocusableButton,
        UiThemeButtonNodeRole::TextInput,
        marker,
        slider,
        Node {
            width: percent(100),
            min_height: px(theme.button.height),
            align_items: AlignItems::Center,
            column_gap: px(theme.layout.row_column_gap),
            padding: UiRect::axes(px(theme.button.padding_x), px(0)),
            border: UiRect::all(px(theme.panel.border)),
            border_radius: BorderRadius::all(px(theme.button.radius)),
            ..default()
        },
        BackgroundColor(text_input_background_color(
            theme,
            Interaction::None,
            false,
            disabled,
        )),
        BorderColor::all(text_input_border_color(
            theme,
            Interaction::None,
            false,
            disabled,
            false,
        )),
        children![
            (
                slider_label_node(),
                Text::new(label),
                TextFont {
                    font: fonts.regular.clone(),
                    font_size: theme.text.button,
                    ..default()
                },
                TextColor(value_color.color(theme)),
                value_color,
                UiThemeTextStyleRole::Button,
                label_i18n_text,
            ),
            (
                slider_track_node(),
                UiSliderTrack,
                RelativeCursorPosition::default(),
                BackgroundColor(theme.colors.panel_border),
                children![(
                    UiSliderFill,
                    Node {
                        width: percent(slider.ratio() * 100.0),
                        height: percent(100),
                        border_radius: BorderRadius::all(px(SLIDER_TRACK_HEIGHT * 0.5)),
                        ..default()
                    },
                    BackgroundColor(fill_color),
                )],
            ),
            (
                slider_value_node(),
                Text::new(format_slider_value(slider.value)),
                TextFont {
                    font: fonts.regular.clone(),
                    font_size: theme.text.button,
                    ..default()
                },
                TextColor(value_color.color(theme)),
                value_color,
                UiThemeTextStyleRole::Button,
                UiSliderValueText,
            ),
        ],
    )
}

fn stepper_bundle<T: Bundle, D: Bundle, I: Bundle>(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    label: impl Into<String>,
    value: i32,
    min: i32,
    max: i32,
    step: i32,
    label_i18n_text: UiI18nText,
    marker: T,
    decrement_marker: D,
    increment_marker: I,
    disabled: bool,
) -> impl Bundle {
    let stepper = UiStepper::new(value, min, max, step);
    let value_color = if disabled {
        UiThemeTextColorRole::Muted
    } else {
        UiThemeTextColorRole::Primary
    };
    let stepper_button_colors = theme.colors.secondary_button;

    (
        marker,
        stepper,
        Node {
            width: percent(100),
            align_items: AlignItems::Center,
            column_gap: px(theme.layout.row_column_gap),
            row_gap: px(theme.layout.row_gap),
            flex_wrap: FlexWrap::Wrap,
            ..default()
        },
        children![
            (
                stepper_label_node(),
                Text::new(label),
                TextFont {
                    font: fonts.regular.clone(),
                    font_size: theme.text.button,
                    ..default()
                },
                TextColor(value_color.color(theme)),
                value_color,
                UiThemeTextStyleRole::Button,
                label_i18n_text,
            ),
            (
                stepper_button(theme, fonts, "-", stepper_button_colors, disabled),
                decrement_marker,
            ),
            (
                stepper_value_node(),
                Text::new(stepper.value.to_string()),
                TextFont {
                    font: fonts.regular.clone(),
                    font_size: theme.text.button,
                    ..default()
                },
                TextColor(value_color.color(theme)),
                value_color,
                UiThemeTextStyleRole::Button,
                UiStepperValueText,
            ),
            (
                stepper_button(theme, fonts, "+", stepper_button_colors, disabled),
                increment_marker,
            ),
        ],
    )
}

fn stepper_button(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
    colors: ButtonColors,
    disabled: bool,
) -> impl Bundle {
    (
        Button,
        FocusableButton,
        SecondaryButton,
        UiThemeButtonNodeRole::Button,
        Node {
            min_width: px(theme.button.height),
            width: px(theme.button.height),
            height: px(theme.button.height),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::axes(px(0), px(0)),
            border_radius: BorderRadius::all(px(theme.button.radius)),
            ..default()
        },
        BackgroundColor(button_background_color(
            colors,
            Interaction::None,
            disabled,
            false,
            false,
            false,
        )),
        children![(
            Text::new(text),
            TextFont {
                font: fonts.regular.clone(),
                font_size: theme.text.button,
                ..default()
            },
            TextColor(if disabled {
                theme.colors.text_muted
            } else {
                theme.colors.text_primary
            }),
            if disabled {
                UiThemeTextColorRole::Muted
            } else {
                UiThemeTextColorRole::Primary
            },
            UiThemeTextStyleRole::Button,
        )],
    )
}

fn icon_button_node(theme: &UiTheme) -> Node {
    Node {
        min_width: px(theme.button.height),
        width: px(theme.button.height),
        height: px(theme.button.height),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        justify_self: JustifySelf::Center,
        padding: UiRect::ZERO,
        border_radius: BorderRadius::all(px(theme.button.radius)),
        ..default()
    }
}

fn slider_label_node() -> Node {
    Node {
        width: px(NUMERIC_CONTROL_LABEL_WIDTH),
        ..default()
    }
}

fn slider_track_node() -> Node {
    Node {
        height: px(SLIDER_TRACK_HEIGHT),
        flex_grow: 1.0,
        overflow: Overflow::clip(),
        border_radius: BorderRadius::all(px(SLIDER_TRACK_HEIGHT * 0.5)),
        ..default()
    }
}

fn slider_value_node() -> Node {
    Node {
        width: px(STEPPER_VALUE_WIDTH),
        justify_content: JustifyContent::FlexEnd,
        ..default()
    }
}

fn stepper_label_node() -> Node {
    Node {
        width: px(NUMERIC_CONTROL_LABEL_WIDTH),
        ..default()
    }
}

fn stepper_value_node() -> Node {
    Node {
        width: px(STEPPER_VALUE_WIDTH),
        min_height: px(36),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        padding: UiRect::horizontal(px(8)),
        border: UiRect::all(px(1)),
        border_radius: BorderRadius::all(px(4)),
        ..default()
    }
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

fn selection_display_text(base_text: &str, state: SelectionVisualState) -> String {
    match state {
        SelectionVisualState::Selected => format!("[x] {base_text}"),
        SelectionVisualState::Idle => format!("[ ] {base_text}"),
        SelectionVisualState::Disabled => format!("[-] {base_text}"),
    }
}

fn icon_button_background_color(colors: ButtonColors, state: IconButtonVisualState) -> Color {
    match state {
        IconButtonVisualState::Idle => colors.idle,
        IconButtonVisualState::Disabled => colors.disabled,
        IconButtonVisualState::Loading => colors.loading,
    }
}

fn icon_button_text_color_role(state: IconButtonVisualState) -> UiThemeTextColorRole {
    match state {
        IconButtonVisualState::Idle | IconButtonVisualState::Loading => {
            UiThemeTextColorRole::Primary
        }
        IconButtonVisualState::Disabled => UiThemeTextColorRole::Muted,
    }
}

fn sync_icon_button_accessible_labels(
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

fn sync_icon_button_nodes(
    theme: Res<UiTheme>,
    mut icon_buttons: Query<&mut Node, With<UiIconButton>>,
) {
    if !theme.is_changed() {
        return;
    }

    for mut node in &mut icon_buttons {
        node.min_width = px(theme.button.height);
        node.width = px(theme.button.height);
        node.height = px(theme.button.height);
        node.padding = UiRect::ZERO;
        node.border_radius = BorderRadius::all(px(theme.button.radius));
    }
}

fn update_selection_control_interactions(
    mut commands: Commands,
    parents: Query<&ChildOf>,
    segmented_roots: Query<(), With<UiSegmentedControl>>,
    segment_options: Query<Entity, (With<UiSegmentOption>, With<UiSegmentOptionSelected>)>,
    buttons: Query<
        (
            Entity,
            &Interaction,
            Has<UiCheckbox>,
            Has<UiCheckboxChecked>,
            Has<UiToggle>,
            Has<UiToggleOn>,
            Has<UiSegmentOption>,
        ),
        (
            Changed<Interaction>,
            With<Button>,
            Without<DisabledButton>,
            Without<LoadingButton>,
            Without<UiStepper>,
        ),
    >,
) {
    for (
        entity,
        interaction,
        is_checkbox,
        is_checked,
        is_toggle,
        is_toggle_on,
        is_segment_option,
    ) in &buttons
    {
        if *interaction != Interaction::Pressed {
            continue;
        }

        if is_checkbox {
            if is_checked {
                commands
                    .entity(entity)
                    .remove::<UiCheckboxChecked>()
                    .remove::<SelectedButton>();
            } else {
                commands
                    .entity(entity)
                    .insert((UiCheckboxChecked, SelectedButton));
            }
        } else if is_toggle {
            if is_toggle_on {
                commands
                    .entity(entity)
                    .remove::<UiToggleOn>()
                    .remove::<SelectedButton>();
            } else {
                commands.entity(entity).insert((UiToggleOn, SelectedButton));
            }
        } else if is_segment_option {
            let root = parents
                .iter_ancestors(entity)
                .find(|ancestor| segmented_roots.contains(*ancestor));

            for selected_entity in &segment_options {
                if selected_entity == entity {
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
                .entity(entity)
                .insert((UiSegmentOptionSelected, SelectedButton));
        }
    }
}

fn update_slider_interactions(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    parents: Query<&ChildOf>,
    mut sliders: Query<&mut UiSlider, (Without<DisabledButton>, Without<UiSliderTrack>)>,
    tracks: Query<
        (
            Entity,
            &RelativeCursorPosition,
            Option<&InheritedVisibility>,
        ),
        With<UiSliderTrack>,
    >,
) {
    if !mouse_buttons.pressed(MouseButton::Left) {
        return;
    }

    for (track_entity, relative_cursor, inherited_visibility) in &tracks {
        if !relative_cursor.cursor_over()
            || inherited_visibility.is_some_and(|visibility| !visibility.get())
        {
            continue;
        }

        let Some(normalized) = relative_cursor.normalized else {
            continue;
        };

        let Some(slider_entity) = parents
            .iter_ancestors(track_entity)
            .find(|ancestor| sliders.get(*ancestor).is_ok())
        else {
            continue;
        };

        let Ok(mut slider) = sliders.get_mut(slider_entity) else {
            continue;
        };

        let next_value = slider_value_from_normalized_x(normalized.x, slider.min, slider.max);
        if slider.value != next_value {
            slider.value = next_value;
        }
    }
}

fn update_stepper_interactions(
    parents: Query<&ChildOf>,
    mut steppers: Query<&mut UiStepper>,
    buttons: Query<
        (
            Entity,
            &Interaction,
            Has<UiStepperDecrementButton>,
            Has<UiStepperIncrementButton>,
        ),
        (
            Changed<Interaction>,
            With<Button>,
            Without<DisabledButton>,
            Without<LoadingButton>,
        ),
    >,
) {
    for (button_entity, interaction, is_decrement, is_increment) in &buttons {
        if *interaction != Interaction::Pressed || !is_decrement && !is_increment {
            continue;
        }

        let Some(stepper_entity) = parents
            .iter_ancestors(button_entity)
            .find(|ancestor| steppers.get(*ancestor).is_ok())
        else {
            continue;
        };

        let Ok(mut stepper) = steppers.get_mut(stepper_entity) else {
            continue;
        };

        let next_value = if is_increment {
            stepper_increment_value(stepper.value, stepper.min, stepper.max, stepper.step)
        } else {
            stepper_decrement_value(stepper.value, stepper.min, stepper.max, stepper.step)
        };
        if stepper.value != next_value {
            stepper.value = next_value;
        }
    }
}

fn sync_selection_control_visuals(
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

fn update_text_input_cursor_from_pointer(
    mut text_inputs: Query<
        (
            Entity,
            &Interaction,
            &RelativeCursorPosition,
            &ComputedNode,
            &mut UiTextInputCursor,
            &UiTextInputValue,
            Has<DisabledTextInput>,
        ),
        (Changed<Interaction>, With<Button>, With<UiTextInput>),
    >,
    children: Query<&Children>,
    text_nodes: Query<&ComputedNode, With<UiTextInputText>>,
) {
    for (entity, interaction, relative_cursor, input_node, mut cursor, value, is_disabled) in
        &mut text_inputs
    {
        if *interaction != Interaction::Pressed || is_disabled {
            continue;
        }

        let Some(normalized) = relative_cursor.normalized else {
            continue;
        };

        let text_width = children
            .get(entity)
            .ok()
            .and_then(|children| {
                children
                    .iter()
                    .filter_map(|child| text_nodes.get(child).ok())
                    .map(|node| node.size.x)
                    .find(|width| *width > 0.0)
            })
            .unwrap_or(input_node.content_size.x);
        let local_x = (normalized.x + 0.5) * input_node.size.x;
        let text_x = (local_x - input_node.padding.min_inset.x).clamp(0.0, text_width);
        let text_ratio = text_x / text_width.max(f32::EPSILON);
        cursor.position = text_input_cursor_position_from_ratio(&value.0, text_ratio);
        cursor.selection = None;
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
    children: Query<&Children>,
    text_inputs: Query<
        (
            &UiTextInputValue,
            &UiTextInputPlaceholder,
            &UiTextInputCursor,
            Has<DisabledTextInput>,
        ),
        With<UiTextInput>,
    >,
    mut roots: Query<(Entity, &mut Text, &mut TextColor), With<UiTextInputText>>,
    mut spans: Query<
        (
            &mut TextSpan,
            &UiTextInputTextPart,
            &mut TextColor,
            Option<&mut TextBackgroundColor>,
        ),
        Without<UiTextInputText>,
    >,
) {
    for (root_entity, mut root_text, mut root_text_color) in &mut roots {
        let Some(input_entity) = parents
            .iter_ancestors(root_entity)
            .find(|ancestor| text_inputs.get(*ancestor).is_ok())
        else {
            continue;
        };

        let Ok((value, placeholder, cursor, is_disabled)) = text_inputs.get(input_entity) else {
            continue;
        };

        let is_focused = focus_state.focused_entity == Some(input_entity);
        let display = if value.0.is_empty() && !is_focused {
            UiTextInputDisplay::placeholder(placeholder.0.clone())
        } else if is_focused && !is_disabled {
            text_input_display_parts(&value.0, cursor)
        } else {
            UiTextInputDisplay::plain(value.0.clone())
        };
        let color = if is_disabled || value.0.is_empty() && !is_focused {
            theme.colors.text_muted
        } else {
            theme.colors.text_primary
        };
        let selected_text_color = theme.colors.screen_background;
        let selected_background = theme.colors.primary_button.focused;

        if !root_text.0.is_empty() {
            root_text.0.clear();
        }
        if root_text_color.0 != color {
            root_text_color.0 = color;
        }

        let Ok(children) = children.get(root_entity) else {
            continue;
        };

        for child in children {
            let Ok((mut span, part, mut span_color, background)) = spans.get_mut(*child) else {
                continue;
            };

            let next_text = match part {
                UiTextInputTextPart::Plain => display.plain.as_str(),
                UiTextInputTextPart::Selected => display.selected.as_str(),
                UiTextInputTextPart::Tail => display.tail.as_str(),
            };
            if span.as_str() != next_text {
                span.0 = next_text.to_string();
            }

            let next_color = match part {
                UiTextInputTextPart::Selected if !display.selected.is_empty() => {
                    selected_text_color
                }
                _ => color,
            };
            if span_color.0 != next_color {
                span_color.0 = next_color;
            }

            if let Some(mut background) = background {
                let next_background = match part {
                    UiTextInputTextPart::Selected if !display.selected.is_empty() => {
                        selected_background
                    }
                    _ => Color::NONE,
                };
                if background.0 != next_background {
                    background.0 = next_background;
                }
            }
        }
    }
}

fn sync_text_input_form_messages(
    theme: Res<UiTheme>,
    text_inputs: Query<(
        &UiTextInputValue,
        Option<&UiTextInputHelperText>,
        Option<&UiTextInputValidationMessage>,
        Option<&UiTextInputAlphanumeric>,
        Option<&UiTextInputRequired>,
        Has<UiTextInputError>,
        Has<DisabledTextInput>,
    )>,
    mut messages: Query<(&UiTextInputFormMessage, &mut Text, &mut TextColor)>,
) {
    for (message, mut text, mut text_color) in &mut messages {
        let Ok((
            value,
            helper_text,
            validation_message,
            alphanumeric,
            required,
            has_error,
            is_disabled,
        )) = text_inputs.get(message.input)
        else {
            continue;
        };

        let state = text_input_form_state(
            &value.0,
            helper_text.map(|helper| helper.0.as_str()),
            text_input_validation_message(&value.0, validation_message, alphanumeric),
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

fn sync_numeric_control_display(
    sliders: Query<(Entity, &UiSlider), Changed<UiSlider>>,
    steppers: Query<(Entity, &UiStepper), Changed<UiStepper>>,
    children: Query<&Children>,
    mut slider_fills: Query<&mut Node, With<UiSliderFill>>,
    mut value_texts: ParamSet<(
        Query<&mut Text, With<UiSliderValueText>>,
        Query<&mut Text, With<UiStepperValueText>>,
    )>,
) {
    {
        let mut slider_value_texts = value_texts.p0();
        for (slider_entity, slider) in &sliders {
            let width = percent(slider.ratio() * 100.0);
            let display = format_slider_value(slider.value);
            for child in children.iter_descendants(slider_entity) {
                if let Ok(mut fill_node) = slider_fills.get_mut(child)
                    && fill_node.width != width
                {
                    fill_node.width = width;
                }

                if let Ok(mut text) = slider_value_texts.get_mut(child)
                    && text.0 != display
                {
                    text.0 = display.clone();
                }
            }
        }
    }

    {
        let mut stepper_value_texts = value_texts.p1();
        for (stepper_entity, stepper) in &steppers {
            let display = stepper.value.to_string();
            for child in children.iter_descendants(stepper_entity) {
                if let Ok(mut text) = stepper_value_texts.get_mut(child)
                    && text.0 != display
                {
                    text.0 = display.clone();
                }
            }
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
            Option<&UiTextInputAlphanumeric>,
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
        alphanumeric,
        required,
    ) in &mut text_inputs
    {
        let is_error = text_input_has_error(
            &value.0,
            text_input_validation_message(&value.0, validation_message, alphanumeric),
            required,
            has_error,
        );
        let background_color =
            text_input_background_color(&theme, *interaction, is_focused, is_disabled);
        if background.0 != background_color {
            *background = BackgroundColor(background_color);
        }

        let next_border = BorderColor::all(text_input_border_color(
            &theme,
            *interaction,
            is_focused,
            is_disabled,
            is_error,
        ));
        if *border != next_border {
            *border = next_border;
        }
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

fn text_input_validation_message<'a>(
    value: &str,
    validation_message: Option<&'a UiTextInputValidationMessage>,
    alphanumeric: Option<&'a UiTextInputAlphanumeric>,
) -> Option<&'a str> {
    validation_message
        .map(|validation| validation.0.as_str())
        .filter(|message| !message.is_empty())
        .or_else(|| alphanumeric.and_then(|rule| rule.validate(value)))
}

fn ordered_slider_bounds(min: f32, max: f32) -> (f32, f32) {
    if min <= max { (min, max) } else { (max, min) }
}

fn clamp_slider_value(value: f32, min: f32, max: f32) -> f32 {
    if value.is_nan() {
        return min;
    }

    value.clamp(min, max)
}

fn slider_ratio(value: f32, min: f32, max: f32) -> f32 {
    let (min, max) = ordered_slider_bounds(min, max);
    let range = max - min;
    if range <= f32::EPSILON {
        return 0.0;
    }

    (clamp_slider_value(value, min, max) - min) / range
}

fn slider_value_from_normalized_x(normalized_x: f32, min: f32, max: f32) -> f32 {
    let (min, max) = ordered_slider_bounds(min, max);
    let ratio = (normalized_x + 0.5).clamp(0.0, 1.0);
    min + (max - min) * ratio
}

fn format_slider_value(value: f32) -> String {
    if value.fract().abs() < 0.05 {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    }
}

fn ordered_stepper_bounds(min: i32, max: i32) -> (i32, i32) {
    if min <= max { (min, max) } else { (max, min) }
}

fn stepper_step(step: i32) -> i32 {
    step.abs().max(1)
}

fn clamp_stepper_value(value: i32, min: i32, max: i32) -> i32 {
    value.clamp(min, max)
}

fn stepper_increment_value(value: i32, min: i32, max: i32, step: i32) -> i32 {
    let (min, max) = ordered_stepper_bounds(min, max);
    clamp_stepper_value(value.saturating_add(stepper_step(step)), min, max)
}

fn stepper_decrement_value(value: i32, min: i32, max: i32, step: i32) -> i32 {
    let (min, max) = ordered_stepper_bounds(min, max);
    clamp_stepper_value(value.saturating_sub(stepper_step(step)), min, max)
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

fn text_input_cursor_position_from_ratio(value: &str, ratio: f32) -> usize {
    if value.is_empty() {
        return 0;
    }

    let char_count = value.chars().count();
    let char_index = (ratio.clamp(0.0, 1.0) * char_count as f32).round() as usize;
    value
        .char_indices()
        .map(|(index, _)| index)
        .nth(char_index)
        .unwrap_or(value.len())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct UiTextInputDisplay {
    plain: String,
    selected: String,
    tail: String,
}

impl UiTextInputDisplay {
    fn plain(text: String) -> Self {
        Self {
            plain: text,
            selected: String::new(),
            tail: String::new(),
        }
    }

    fn placeholder(text: String) -> Self {
        Self::plain(text)
    }
}

fn text_input_display_parts(value: &str, cursor: &UiTextInputCursor) -> UiTextInputDisplay {
    let cursor_position = nearest_char_boundary(value, cursor.position.min(value.len()));
    if let Some(selection) = selection_range(cursor) {
        let start = nearest_char_boundary(value, selection.start.min(value.len()));
        let end = nearest_char_boundary(value, selection.end.min(value.len()));
        let cursor_at_start = cursor_position <= start;
        return UiTextInputDisplay {
            plain: if cursor_at_start {
                format!("{}|", &value[..start])
            } else {
                value[..start].to_string()
            },
            selected: value[start..end].to_string(),
            tail: if cursor_at_start {
                value[end..].to_string()
            } else {
                format!("|{}", &value[end..])
            },
        };
    }

    UiTextInputDisplay {
        plain: format!("{}|", &value[..cursor_position]),
        selected: String::new(),
        tail: value[cursor_position..].to_string(),
    }
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
    fn text_input_display_splits_selected_range() {
        let cursor = UiTextInputCursor {
            position: 3,
            selection: Some(UiTextInputSelection { start: 1, end: 3 }),
        };

        assert_eq!(
            text_input_display_parts("abcd", &cursor),
            UiTextInputDisplay {
                plain: "a".to_string(),
                selected: "bc".to_string(),
                tail: "|d".to_string(),
            }
        );
    }

    #[test]
    fn text_input_cursor_position_maps_ratio_to_char_boundary() {
        assert_eq!(text_input_cursor_position_from_ratio("abcd", 0.0), 0);
        assert_eq!(text_input_cursor_position_from_ratio("abcd", 0.5), 2);
        assert_eq!(text_input_cursor_position_from_ratio("abcd", 1.0), 4);
        assert_eq!(
            text_input_cursor_position_from_ratio("你好吗", 0.5),
            "你好".len()
        );
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
    fn alphanumeric_validation_clears_for_matching_value() {
        let rule = UiTextInputAlphanumeric::new(4, 8, "Use 4-8 letters or numbers.");

        assert_eq!(rule.validate("33333311"), None);
        assert_eq!(rule.validate("AB12"), None);
        assert_eq!(
            rule.validate("bad-code"),
            Some("Use 4-8 letters or numbers.")
        );
        assert_eq!(rule.validate("abc"), Some("Use 4-8 letters or numbers."));
        assert_eq!(
            rule.validate("abcdefghi"),
            Some("Use 4-8 letters or numbers.")
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
    fn button_background_color_uses_documented_visual_priority() {
        let colors = UiTheme::default().colors.primary_button;

        assert_eq!(
            button_background_color(colors, Interaction::Pressed, true, true, true, true),
            colors.disabled
        );
        assert_eq!(
            button_background_color(colors, Interaction::Pressed, false, true, true, true),
            colors.loading
        );
        assert_eq!(
            button_background_color(colors, Interaction::Pressed, false, true, true, false),
            colors.pressed
        );
        assert_eq!(
            button_background_color(colors, Interaction::Hovered, false, true, true, false),
            colors.hovered
        );
        assert_eq!(
            button_background_color(colors, Interaction::None, false, true, true, false),
            colors.selected
        );
        assert_eq!(
            button_background_color(colors, Interaction::None, false, true, false, false),
            colors.focused
        );
        assert_eq!(
            button_background_color(colors, Interaction::None, false, false, false, false),
            colors.idle
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

    #[test]
    fn selection_text_color_role_matches_disabled_state() {
        assert!(matches!(
            selection_button_text_color_role(SelectionVisualState::Disabled),
            UiThemeTextColorRole::Muted
        ));
        assert!(matches!(
            selection_button_text_color_role(SelectionVisualState::Selected),
            UiThemeTextColorRole::Primary
        ));
        assert!(matches!(
            selection_button_text_color_role(SelectionVisualState::Idle),
            UiThemeTextColorRole::Primary
        ));
    }

    #[test]
    fn selection_display_text_marks_state() {
        assert_eq!(
            selection_display_text("Medium", SelectionVisualState::Selected),
            "[x] Medium"
        );
        assert_eq!(
            selection_display_text("Medium", SelectionVisualState::Idle),
            "[ ] Medium"
        );
        assert_eq!(
            selection_display_text("Medium", SelectionVisualState::Disabled),
            "[-] Medium"
        );
    }

    #[test]
    fn icon_button_background_and_text_roles_match_visual_state() {
        let colors = UiTheme::default().colors.secondary_button;

        assert_eq!(
            icon_button_background_color(colors, IconButtonVisualState::Idle),
            colors.idle
        );
        assert_eq!(
            icon_button_background_color(colors, IconButtonVisualState::Disabled),
            colors.disabled
        );
        assert_eq!(
            icon_button_background_color(colors, IconButtonVisualState::Loading),
            colors.loading
        );
        assert!(matches!(
            icon_button_text_color_role(IconButtonVisualState::Idle),
            UiThemeTextColorRole::Primary
        ));
        assert!(matches!(
            icon_button_text_color_role(IconButtonVisualState::Loading),
            UiThemeTextColorRole::Primary
        ));
        assert!(matches!(
            icon_button_text_color_role(IconButtonVisualState::Disabled),
            UiThemeTextColorRole::Muted
        ));
    }

    #[test]
    fn icon_button_node_uses_stable_square_button_size() {
        let theme = UiTheme::default();
        let node = icon_button_node(&theme);

        assert_eq!(node.min_width, px(theme.button.height));
        assert_eq!(node.width, px(theme.button.height));
        assert_eq!(node.height, px(theme.button.height));
        assert_eq!(node.padding, UiRect::ZERO);
        assert_eq!(
            node.border_radius,
            BorderRadius::all(px(theme.button.radius))
        );
    }

    #[test]
    fn slider_ratio_orders_bounds_and_clamps_value() {
        assert_eq!(slider_ratio(50.0, 0.0, 100.0), 0.5);
        assert_eq!(slider_ratio(150.0, 0.0, 100.0), 1.0);
        assert_eq!(slider_ratio(-10.0, 0.0, 100.0), 0.0);
        assert_eq!(slider_ratio(25.0, 100.0, 0.0), 0.25);
        assert_eq!(slider_ratio(10.0, 10.0, 10.0), 0.0);
    }

    #[test]
    fn slider_model_orders_bounds_clamps_nan_and_formats_values() {
        let slider = UiSlider::new(f32::NAN, 100.0, 0.0);

        assert_eq!(slider.min, 0.0);
        assert_eq!(slider.max, 100.0);
        assert_eq!(slider.value, 0.0);
        assert_eq!(slider.ratio(), 0.0);
        assert_eq!(format_slider_value(42.02), "42");
        assert_eq!(format_slider_value(42.06), "42.1");
        assert_eq!(format_slider_value(42.16), "42.2");
    }

    #[test]
    fn slider_value_from_normalized_x_maps_track_position_to_value() {
        assert_eq!(slider_value_from_normalized_x(-0.5, 0.0, 100.0), 0.0);
        assert_eq!(slider_value_from_normalized_x(0.0, 0.0, 100.0), 50.0);
        assert_eq!(slider_value_from_normalized_x(0.5, 0.0, 100.0), 100.0);
        assert_eq!(slider_value_from_normalized_x(0.75, 0.0, 100.0), 100.0);
        assert_eq!(slider_value_from_normalized_x(-0.75, 0.0, 100.0), 0.0);
    }

    #[test]
    fn stepper_increment_and_decrement_clamp_to_bounds() {
        assert_eq!(stepper_increment_value(4, 1, 8, 2), 6);
        assert_eq!(stepper_increment_value(7, 1, 8, 2), 8);
        assert_eq!(stepper_decrement_value(4, 1, 8, 2), 2);
        assert_eq!(stepper_decrement_value(2, 1, 8, 2), 1);
        assert_eq!(stepper_increment_value(4, 8, 1, -2), 6);
        assert_eq!(stepper_decrement_value(4, 8, 1, 0), 3);
    }

    #[test]
    fn stepper_model_orders_bounds_clamps_value_and_normalizes_step() {
        let stepper = UiStepper::new(20, 10, 1, -3);

        assert_eq!(stepper.min, 1);
        assert_eq!(stepper.max, 10);
        assert_eq!(stepper.value, 10);
        assert_eq!(stepper.step, 3);

        let zero_stepper = UiStepper::new(5, 1, 10, 0);
        assert_eq!(zero_stepper.step, 1);
    }
}
