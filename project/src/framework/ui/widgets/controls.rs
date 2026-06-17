use std::collections::HashSet;

use bevy::{
    input::keyboard::{Key, KeyCode, KeyboardInput},
    prelude::*,
    ui::{FocusPolicy, RelativeCursorPosition},
};

use crate::framework::ui::{
    core::{UiFocusSystems, UiMetrics, focus::UiFocusState},
    i18n::{UiI18n, UiI18nText},
    style::{
        UiFontAssets,
        theme::{
            ButtonColors, UiTheme, UiThemeButtonNodeRole, UiThemeTextColorRole,
            UiThemeTextStyleRole,
        },
    },
    widgets::scroll::UiScrollPlugin,
};

const NUMERIC_CONTROL_LABEL_WIDTH: f32 = 132.0;
const TEXT_INPUT_FOCUS_SWITCH_LOG_TICKS: u64 = 12;
pub(crate) struct UiWidgetsPlugin;

impl Plugin for UiWidgetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(UiScrollPlugin)
            .init_resource::<UiTextInputClipboard>()
            .init_resource::<UiTextInputDiagnostics>()
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
                    sync_android_text_input
                        .after(update_text_input_cursor_from_pointer)
                        .before(sync_text_input_display),
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
struct UiSelectionLabel {
    base_text: String,
}

#[derive(Clone, Copy, Debug, Component)]
pub(crate) struct UiSlider {
    pub value: f32,
    pub min: f32,
    pub max: f32,
}

impl UiSlider {
    pub(crate) fn new(value: f32, min: f32, max: f32) -> Self {
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
pub(crate) struct UiStepper {
    pub value: i32,
    pub min: i32,
    pub max: i32,
    pub step: i32,
}

impl UiStepper {
    pub(crate) fn new(value: i32, min: i32, max: i32, step: i32) -> Self {
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
pub(crate) struct UiTextInput;

#[derive(Clone, Debug, Default, Component)]
pub(crate) struct UiTextInputValue(pub String);

#[derive(Clone, Debug, Default, Component)]
pub(crate) struct UiTextInputCursor {
    position: usize,
    selection: Option<UiTextInputSelection>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct UiTextInputSelection {
    start: usize,
    end: usize,
}

#[derive(Clone, Copy, Debug, Component)]
pub(crate) struct UiTextInputMaxChars(pub usize);

#[derive(Component)]
pub(crate) struct ReadonlyTextInput;

#[derive(Component)]
pub(crate) struct DisabledTextInput;

#[derive(Clone, Debug, Component)]
pub(crate) struct UiTextInputRequired {
    message: String,
}

impl UiTextInputRequired {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, Component)]
pub(crate) struct UiTextInputAlphanumeric {
    min_chars: usize,
    max_chars: usize,
    message: String,
}

impl UiTextInputAlphanumeric {
    pub(crate) fn new(min_chars: usize, max_chars: usize, message: impl Into<String>) -> Self {
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
pub(crate) struct UiTextInputError;

#[derive(Clone, Debug, Default, Component)]
pub(crate) struct UiTextInputHelperText(pub String);

#[derive(Clone, Debug, Default, Component)]
pub(crate) struct UiTextInputValidationMessage(pub String);

#[derive(Clone, Debug, Default, Component)]
pub(crate) struct UiTextInputPlaceholder(pub String);

#[derive(Component)]
pub(crate) struct UiTextInputText;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Component)]
enum UiTextInputTextPart {
    Plain,
    Selected,
    Tail,
}

#[derive(Clone, Copy, Debug, Component)]
pub(crate) struct UiTextInputFormMessage {
    input: Entity,
}

#[derive(Debug, Default, Resource)]
struct UiTextInputClipboard {
    text: String,
}

#[derive(Debug, Default, Resource)]
struct UiTextInputDiagnostics {
    tick: u64,
    focused_entity: Option<Entity>,
    focus_changed_tick: u64,
    #[cfg(target_os = "android")]
    android_soft_keyboard_visible: bool,
    #[cfg(target_os = "android")]
    android_text_input_entity: Option<Entity>,
    #[cfg(target_os = "android")]
    android_text_input_snapshot: Option<UiTextInputNativeState>,
    #[cfg(target_os = "android")]
    android_text_input_skip_pull_until_tick: u64,
    #[cfg(target_os = "android")]
    android_text_input_pressed_entity: Option<Entity>,
    #[cfg(target_os = "android")]
    android_text_input_pressed_tick: u64,
    missing_pointer_position_logged: HashSet<Entity>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
struct UiTextInputNativeState {
    text: String,
    selection_start: usize,
    selection_end: usize,
}

#[derive(Clone, Debug, Message)]
pub(crate) struct UiTextInputSubmitted {
    pub entity: Entity,
    pub value: String,
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

pub(crate) fn slider_key(
    theme: &UiTheme,
    metrics: &UiMetrics,
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
        metrics,
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

pub(crate) fn disabled_slider_key(
    theme: &UiTheme,
    metrics: &UiMetrics,
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
        metrics,
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

pub(crate) fn stepper_key(
    theme: &UiTheme,
    metrics: &UiMetrics,
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
        metrics,
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

pub(crate) fn disabled_stepper_key(
    theme: &UiTheme,
    metrics: &UiMetrics,
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
        metrics,
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

pub(crate) fn text_input(
    theme: &UiTheme,
    metrics: &UiMetrics,
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
            min_height: px(metrics.input_height),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::FlexStart,
            padding: UiRect::axes(px(control_padding_x(metrics)), px(0)),
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

pub(crate) fn text_input_form_message(
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

fn action_button<T: Component>(
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

fn action_button_key_bundle<T: Component>(
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
fn disabled_action_button<T: Component>(
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

fn disabled_action_button_key_bundle<T: Component>(
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
enum IconButtonVisualState {
    Idle,
    Disabled,
    Loading,
}

fn icon_button_key_bundle<T: Bundle>(
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
    metrics: &UiMetrics,
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
        RelativeCursorPosition::default(),
        Node {
            width: percent(100),
            min_height: px(metrics.input_height),
            align_items: AlignItems::Center,
            column_gap: px(numeric_control_gap(metrics)),
            row_gap: px(metrics.control_gap),
            flex_wrap: FlexWrap::Wrap,
            padding: UiRect::axes(px(control_padding_x(metrics)), px(0)),
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
                slider_label_node(metrics),
                FocusPolicy::Pass,
                Text::new(label),
                TextFont {
                    font: fonts.regular.clone(),
                    font_size: theme.text.button,
                    ..default()
                },
                TextLayout::new_with_justify(Justify::Center),
                TextColor(value_color.color(theme)),
                value_color,
                UiThemeTextStyleRole::Button,
                label_i18n_text,
            ),
            (
                slider_track_node(metrics),
                UiSliderTrack,
                FocusPolicy::Pass,
                BackgroundColor(theme.colors.panel_border),
                children![(
                    UiSliderFill,
                    FocusPolicy::Pass,
                    Node {
                        width: percent(slider.ratio() * 100.0),
                        height: percent(100),
                        border_radius: BorderRadius::all(px(slider_track_height(metrics) * 0.5)),
                        ..default()
                    },
                    BackgroundColor(fill_color),
                )],
            ),
            (
                slider_value_node(metrics),
                FocusPolicy::Pass,
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
    metrics: &UiMetrics,
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
            column_gap: px(numeric_control_gap(metrics)),
            row_gap: px(metrics.control_gap),
            flex_wrap: FlexWrap::Wrap,
            ..default()
        },
        children![
            (
                stepper_label_node(metrics),
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
                stepper_button(theme, metrics, fonts, "-", stepper_button_colors, disabled),
                decrement_marker,
            ),
            (
                stepper_value_node(metrics),
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
                stepper_button(theme, metrics, fonts, "+", stepper_button_colors, disabled),
                increment_marker,
            ),
        ],
    )
}

fn stepper_button(
    theme: &UiTheme,
    metrics: &UiMetrics,
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
        square_button_node(theme, metrics),
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

fn button_node(theme: &UiTheme, metrics: &UiMetrics) -> Node {
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

fn square_button_node(theme: &UiTheme, metrics: &UiMetrics) -> Node {
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

fn icon_button_node(theme: &UiTheme, metrics: &UiMetrics) -> Node {
    Node {
        justify_self: JustifySelf::Center,
        ..square_button_node(theme, metrics)
    }
}

fn button_min_width(theme: &UiTheme, metrics: &UiMetrics) -> f32 {
    theme.button.min_width.max(metrics.button_height * 2.25)
}

fn square_button_size(metrics: &UiMetrics) -> f32 {
    metrics.button_height.max(metrics.touch_target_min)
}

fn control_padding_x(metrics: &UiMetrics) -> f32 {
    (metrics.control_gap * 2.0).clamp(12.0, 24.0)
}

fn numeric_control_gap(metrics: &UiMetrics) -> f32 {
    if numeric_control_is_compact(metrics) {
        return metrics.control_gap;
    }

    metrics.control_gap.max(10.0)
}

fn numeric_control_label_width(metrics: &UiMetrics) -> f32 {
    if numeric_control_is_compact(metrics) {
        return (metrics.dialog_max_width * 0.28).clamp(88.0, 104.0);
    }

    NUMERIC_CONTROL_LABEL_WIDTH
        .min(metrics.content_max_width * 0.34)
        .max(72.0)
}

fn slider_track_height(metrics: &UiMetrics) -> f32 {
    (metrics.icon_size * 0.36).clamp(8.0, 10.0)
}

fn stepper_value_width(metrics: &UiMetrics) -> f32 {
    if numeric_control_is_compact(metrics) {
        return (metrics.touch_target_min + metrics.control_gap).max(52.0);
    }

    (square_button_size(metrics) * 1.6).max(metrics.touch_target_min + metrics.control_gap * 2.0)
}

fn stepper_value_min_height(metrics: &UiMetrics) -> f32 {
    (metrics.button_height * 0.78).max(metrics.touch_target_min * 0.75)
}

fn slider_label_node(metrics: &UiMetrics) -> Node {
    Node {
        width: px(numeric_control_label_width(metrics)),
        flex_shrink: 0.0,
        ..default()
    }
}

fn slider_track_node(metrics: &UiMetrics) -> Node {
    let track_height = slider_track_height(metrics);
    Node {
        min_width: px(slider_track_min_width(metrics)),
        height: px(track_height),
        flex_grow: 1.0,
        flex_shrink: 1.0,
        overflow: Overflow::clip(),
        border_radius: BorderRadius::all(px(track_height * 0.5)),
        ..default()
    }
}

fn slider_value_node(metrics: &UiMetrics) -> Node {
    Node {
        width: px(stepper_value_width(metrics)),
        flex_shrink: 0.0,
        justify_content: JustifyContent::FlexEnd,
        ..default()
    }
}

fn stepper_label_node(metrics: &UiMetrics) -> Node {
    Node {
        width: px(numeric_control_label_width(metrics)),
        flex_shrink: 0.0,
        ..default()
    }
}

fn stepper_value_node(metrics: &UiMetrics) -> Node {
    Node {
        width: px(stepper_value_width(metrics)),
        min_height: px(stepper_value_min_height(metrics)),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        padding: UiRect::horizontal(px(metrics.control_gap)),
        border: UiRect::all(px(1)),
        border_radius: BorderRadius::all(px(4)),
        ..default()
    }
}

fn slider_track_min_width(metrics: &UiMetrics) -> f32 {
    if numeric_control_is_compact(metrics) {
        return (metrics.dialog_max_width * 0.24).clamp(80.0, 96.0);
    }

    (metrics.touch_target_min * 3.0).min(metrics.content_max_width * 0.42)
}

fn numeric_control_is_compact(metrics: &UiMetrics) -> bool {
    metrics.content_max_width <= 480.0
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
    mut sliders: Query<
        (
            Entity,
            &Interaction,
            &RelativeCursorPosition,
            &ComputedNode,
            &UiGlobalTransform,
            &mut UiSlider,
            Option<&InheritedVisibility>,
        ),
        (
            With<Button>,
            Without<DisabledButton>,
            Without<UiSliderTrack>,
        ),
    >,
    tracks: Query<
        (
            Entity,
            &ComputedNode,
            &UiGlobalTransform,
            Option<&InheritedVisibility>,
        ),
        With<UiSliderTrack>,
    >,
    parents: Query<&ChildOf>,
) {
    for (
        slider_entity,
        interaction,
        relative_cursor,
        slider_node,
        slider_transform,
        mut slider,
        slider_inherited_visibility,
    ) in &mut sliders
    {
        if *interaction != Interaction::Pressed
            || slider_inherited_visibility.is_some_and(|visibility| !visibility.get())
        {
            continue;
        }

        let Some(slider_normalized) = relative_cursor.normalized else {
            continue;
        };

        let slider_local_position = slider_normalized * slider_node.size;
        let slider_global_position = slider_transform
            .affine()
            .transform_point2(slider_local_position);

        let Some((_, track_node, track_transform, _)) =
            tracks
                .iter()
                .find(|(track_entity, _, _, track_inherited_visibility)| {
                    track_inherited_visibility.is_none_or(|visibility| visibility.get())
                        && parents
                            .iter_ancestors(*track_entity)
                            .any(|ancestor| ancestor == slider_entity)
                })
        else {
            continue;
        };

        let Some(normalized_track_position) =
            track_node.normalize_point(*track_transform, slider_global_position)
        else {
            continue;
        };
        let normalized_track_x = normalized_track_position.x;
        let next_value = slider_value_from_normalized_x(normalized_track_x, slider.min, slider.max);
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
    mut diagnostics: ResMut<UiTextInputDiagnostics>,
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
        (With<Button>, With<UiTextInput>),
    >,
    children: Query<&Children>,
    text_nodes: Query<&ComputedNode, With<UiTextInputText>>,
) {
    for (entity, interaction, relative_cursor, input_node, mut cursor, value, is_disabled) in
        &mut text_inputs
    {
        if *interaction != Interaction::Pressed || is_disabled {
            diagnostics.missing_pointer_position_logged.remove(&entity);
            continue;
        }

        #[cfg(target_os = "android")]
        {
            diagnostics.android_text_input_pressed_entity = Some(entity);
            diagnostics.android_text_input_pressed_tick = diagnostics.tick;
        }

        let Some(normalized) = relative_cursor.normalized else {
            if diagnostics.missing_pointer_position_logged.insert(entity) {
                debug!(
                    ?entity,
                    input_size = ?input_node.size,
                    content_size = ?input_node.content_size,
                    cursor_position = cursor.position,
                    value_len = value.0.len(),
                    "text input pressed without relative cursor position"
                );
            }
            continue;
        };
        diagnostics.missing_pointer_position_logged.remove(&entity);

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

#[cfg(target_os = "android")]
fn sync_android_text_input(
    focus_state: Res<UiFocusState>,
    mut diagnostics: ResMut<UiTextInputDiagnostics>,
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
) {
    let focused_text_input = focus_state.focused_entity.and_then(|entity| {
        text_inputs
            .get(entity)
            .ok()
            .and_then(|(_, _, _, _, is_disabled)| (!is_disabled).then_some(entity))
    });

    let Some(android_app) = bevy::android::ANDROID_APP.get() else {
        if focused_text_input.is_some() {
            warn!("cannot sync Android text input without AndroidApp");
        }
        return;
    };

    if diagnostics.android_text_input_entity != focused_text_input {
        if let Some(entity) = focused_text_input {
            let Ok((value, cursor, _, _, _)) = text_inputs.get(entity) else {
                return;
            };
            let state = ui_text_input_native_state_from_value(&value.0, cursor);

            android_app.set_ime_editor_info(
                bevy::android::android_activity::input::InputType::TYPE_CLASS_TEXT,
                bevy::android::android_activity::input::TextInputAction::Done,
                bevy::android::android_activity::input::ImeOptions::IME_FLAG_NO_FULLSCREEN,
            );
            android_app.set_text_input_state(state.to_android_text_input_state());
            android_app.show_soft_input(true);

            diagnostics.android_soft_keyboard_visible = true;
            diagnostics.android_text_input_entity = Some(entity);
            diagnostics.android_text_input_snapshot = Some(state.clone());
            diagnostics.android_text_input_skip_pull_until_tick =
                diagnostics.tick.saturating_add(1);
            debug!(
                ?entity,
                text = %state.text,
                selection_start = state.selection_start,
                selection_end = state.selection_end,
                "initialized Android text input state for focused field"
            );
        } else {
            if diagnostics.android_soft_keyboard_visible {
                android_app.hide_soft_input(false);
                debug!("requested Android soft keyboard hide after text input blur");
            }
            diagnostics.android_soft_keyboard_visible = false;
            diagnostics.android_text_input_entity = None;
            diagnostics.android_text_input_snapshot = None;
            diagnostics.android_text_input_skip_pull_until_tick = 0;
        }
        return;
    }

    let Some(entity) = focused_text_input else {
        return;
    };
    let text_input_pressed_this_tick = diagnostics.android_text_input_pressed_entity
        == Some(entity)
        && diagnostics.android_text_input_pressed_tick == diagnostics.tick;

    let Ok((mut value, mut cursor, max_chars, is_readonly, is_disabled)) =
        text_inputs.get_mut(entity)
    else {
        return;
    };
    if is_disabled {
        return;
    }

    let app_state = ui_text_input_native_state_from_value(&value.0, &cursor);
    if diagnostics.android_text_input_snapshot.as_ref() != Some(&app_state) {
        android_app.set_text_input_state(app_state.to_android_text_input_state());
        diagnostics.android_text_input_snapshot = Some(app_state.clone());
        diagnostics.android_text_input_skip_pull_until_tick = diagnostics.tick.saturating_add(1);
        if text_input_pressed_this_tick {
            android_app.show_soft_input(true);
            diagnostics.android_soft_keyboard_visible = true;
        }
        debug!(
            ?entity,
            text = %app_state.text,
            selection_start = app_state.selection_start,
            selection_end = app_state.selection_end,
            "pushed Bevy text input state to Android IME"
        );
        return;
    }

    if text_input_pressed_this_tick {
        android_app.show_soft_input(true);
        diagnostics.android_soft_keyboard_visible = true;
        debug!(
            ?entity,
            "requested Android soft keyboard show after focused text input press"
        );
    }

    if diagnostics.tick <= diagnostics.android_text_input_skip_pull_until_tick {
        return;
    }

    let native_state =
        UiTextInputNativeState::from_android_text_input_state(android_app.text_input_state());
    if diagnostics.android_text_input_snapshot.as_ref() == Some(&native_state) {
        return;
    }

    if is_readonly {
        android_app.set_text_input_state(app_state.to_android_text_input_state());
        diagnostics.android_text_input_snapshot = Some(app_state);
        diagnostics.android_text_input_skip_pull_until_tick = diagnostics.tick.saturating_add(1);
        debug!(
            ?entity,
            ime_text = %native_state.text,
            "rejected Android IME edit for readonly text input"
        );
        return;
    }

    let before_value = value.0.clone();
    let before_cursor = cursor.position;
    let before_selection = cursor.selection;
    apply_native_text_input_state(
        &mut value.0,
        &mut cursor,
        native_state,
        max_chars.map(|max_chars| max_chars.0),
    );
    let applied_state = ui_text_input_native_state_from_value(&value.0, &cursor);

    if value.0 != before_value
        || cursor.position != before_cursor
        || cursor.selection != before_selection
    {
        debug!(
            ?entity,
            before_value = %before_value,
            after_value = %value.0,
            before_cursor,
            after_cursor = cursor.position,
            before_selection = ?before_selection,
            after_selection = ?cursor.selection,
            "pulled Android IME text input state into Bevy"
        );
    }

    if diagnostics.android_text_input_snapshot.as_ref() != Some(&applied_state) {
        android_app.set_text_input_state(applied_state.to_android_text_input_state());
    }
    diagnostics.android_text_input_snapshot = Some(applied_state);
}

#[cfg(not(target_os = "android"))]
fn sync_android_text_input() {}

fn handle_text_input_keyboard(
    mut keyboard_inputs: MessageReader<KeyboardInput>,
    key_codes: Res<ButtonInput<KeyCode>>,
    focus_state: Res<UiFocusState>,
    mut diagnostics: ResMut<UiTextInputDiagnostics>,
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
    diagnostics.tick = diagnostics.tick.wrapping_add(1);
    let previous_focused = diagnostics.focused_entity;
    if previous_focused != focus_state.focused_entity {
        let previous_was_text_input =
            previous_focused.is_some_and(|entity| text_inputs.contains(entity));
        let focused_is_text_input = focus_state
            .focused_entity
            .is_some_and(|entity| text_inputs.contains(entity));
        if previous_was_text_input || focused_is_text_input {
            debug!(
                tick = diagnostics.tick,
                ?previous_focused,
                focused_entity = ?focus_state.focused_entity,
                "text input focus changed"
            );
        }
        diagnostics.focused_entity = focus_state.focused_entity;
        diagnostics.focus_changed_tick = diagnostics.tick;
    }
    let focus_ticks_ago = diagnostics
        .tick
        .saturating_sub(diagnostics.focus_changed_tick);

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

        let before_value = value.0.clone();
        let before_cursor = cursor.position;
        let before_selection = cursor.selection;
        let edit_event = ui_text_input_edit_event(keyboard_input, &key_codes);
        if should_skip_keyboard_text_edit_for_native_ime(&edit_event) {
            debug!(
                tick = diagnostics.tick,
                ?focused_entity,
                key_code = ?keyboard_input.key_code,
                logical_key = ?keyboard_input.logical_key,
                text = ?keyboard_input.text.as_deref(),
                "skipped keyboard text edit while Android IME state is authoritative"
            );
            continue;
        }

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

        if should_log_text_input_keyboard_event(
            keyboard_input,
            focus_ticks_ago,
            &before_value,
            &value.0,
            before_cursor,
            cursor.position,
            before_selection,
            cursor.selection,
        ) {
            debug!(
                tick = diagnostics.tick,
                ?focused_entity,
                focus_ticks_ago,
                key_code = ?keyboard_input.key_code,
                logical_key = ?keyboard_input.logical_key,
                text = ?keyboard_input.text.as_deref(),
                before_value = %before_value,
                after_value = %value.0,
                before_cursor,
                after_cursor = cursor.position,
                before_selection = ?before_selection,
                after_selection = ?cursor.selection,
                "text input keyboard event"
            );
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

    if is_focused {
        return theme.colors.primary_button.focused;
    }

    match interaction {
        Interaction::Pressed => theme.colors.primary_button.pressed,
        Interaction::Hovered => theme.colors.secondary_button.focused,
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

fn should_skip_keyboard_text_edit_for_native_ime(edit_event: &UiTextInputEditEvent<'_>) -> bool {
    cfg!(target_os = "android")
        && matches!(
            edit_event,
            UiTextInputEditEvent::Edit(
                UiTextInputEditAction::Insert(_)
                    | UiTextInputEditAction::Paste(_)
                    | UiTextInputEditAction::Backspace
                    | UiTextInputEditAction::Delete
            ) | UiTextInputEditEvent::Paste
        )
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

fn should_log_text_input_keyboard_event(
    keyboard_input: &KeyboardInput,
    focus_ticks_ago: u64,
    before_value: &str,
    after_value: &str,
    before_cursor: usize,
    after_cursor: usize,
    before_selection: Option<UiTextInputSelection>,
    after_selection: Option<UiTextInputSelection>,
) -> bool {
    if focus_ticks_ago > TEXT_INPUT_FOCUS_SWITCH_LOG_TICKS {
        return false;
    }

    keyboard_input.text.is_some()
        || before_value != after_value
        || before_cursor != after_cursor
        || before_selection != after_selection
}

#[cfg(target_os = "android")]
impl UiTextInputNativeState {
    fn from_android_text_input_state(
        state: bevy::android::android_activity::input::TextInputState,
    ) -> Self {
        Self {
            text: state.text,
            selection_start: state.selection.start,
            selection_end: state.selection.end,
        }
    }

    fn to_android_text_input_state(
        &self,
    ) -> bevy::android::android_activity::input::TextInputState {
        bevy::android::android_activity::input::TextInputState {
            text: self.text.clone(),
            selection: bevy::android::android_activity::input::TextSpan {
                start: self.selection_start,
                end: self.selection_end,
            },
            compose_region: None,
        }
    }
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn ui_text_input_native_state_from_value(
    value: &str,
    cursor: &UiTextInputCursor,
) -> UiTextInputNativeState {
    let mut cursor = cursor.clone();
    clamp_text_input_cursor(value, &mut cursor);
    let (selection_start, selection_end) = selection_range(&cursor)
        .map(|selection| (selection.start, selection.end))
        .unwrap_or((cursor.position, cursor.position));

    UiTextInputNativeState {
        text: value.to_string(),
        selection_start,
        selection_end,
    }
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn apply_native_text_input_state(
    value: &mut String,
    cursor: &mut UiTextInputCursor,
    state: UiTextInputNativeState,
    max_chars: Option<usize>,
) {
    let text = limit_text_input_text(state.text, max_chars);
    let selection_start = native_selection_to_char_boundary(&text, state.selection_start);
    let selection_end = native_selection_to_char_boundary(&text, state.selection_end);
    let (selection_start, selection_end) = if selection_start <= selection_end {
        (selection_start, selection_end)
    } else {
        (selection_end, selection_start)
    };

    *value = text;
    cursor.position = selection_end;
    cursor.selection = (selection_start < selection_end).then_some(UiTextInputSelection {
        start: selection_start,
        end: selection_end,
    });
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn limit_text_input_text(text: String, max_chars: Option<usize>) -> String {
    let Some(max_chars) = max_chars else {
        return text;
    };

    text.chars().take(max_chars).collect()
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

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn native_selection_to_char_boundary(value: &str, position: usize) -> usize {
    nearest_char_boundary(value, position.min(value.len()))
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
    fn native_state_from_value_uses_cursor_or_selection() {
        let value = "abcd";
        let selected_cursor = UiTextInputCursor {
            position: 3,
            selection: Some(UiTextInputSelection { start: 1, end: 3 }),
        };

        assert_eq!(
            ui_text_input_native_state_from_value(value, &selected_cursor),
            UiTextInputNativeState {
                text: "abcd".to_string(),
                selection_start: 1,
                selection_end: 3,
            }
        );

        assert_eq!(
            ui_text_input_native_state_from_value(value, &cursor(2)),
            UiTextInputNativeState {
                text: "abcd".to_string(),
                selection_start: 2,
                selection_end: 2,
            }
        );
    }

    #[test]
    fn apply_native_state_clamps_selection_and_max_chars() {
        let mut value = String::new();
        let mut cursor = cursor(0);

        apply_native_text_input_state(
            &mut value,
            &mut cursor,
            UiTextInputNativeState {
                text: "你好吗".to_string(),
                selection_start: "你好".len(),
                selection_end: usize::MAX,
            },
            Some(2),
        );

        assert_eq!(value, "你好");
        assert_eq!(cursor.position, value.len());
        assert_eq!(cursor.selection, None);
    }

    #[test]
    fn apply_native_state_normalizes_reversed_selection() {
        let mut value = String::new();
        let mut cursor = cursor(0);

        apply_native_text_input_state(
            &mut value,
            &mut cursor,
            UiTextInputNativeState {
                text: "abcd".to_string(),
                selection_start: 3,
                selection_end: 1,
            },
            None,
        );

        assert_eq!(value, "abcd");
        assert_eq!(cursor.position, 3);
        assert_eq!(
            cursor.selection,
            Some(UiTextInputSelection { start: 1, end: 3 })
        );
    }

    #[test]
    fn keyboard_diagnostics_only_log_near_focus_changes() {
        let keyboard_input = KeyboardInput {
            key_code: KeyCode::KeyX,
            logical_key: Key::Character("x".into()),
            state: bevy::input::ButtonState::Pressed,
            text: Some("x".into()),
            repeat: false,
            window: Entity::PLACEHOLDER,
        };

        assert!(should_log_text_input_keyboard_event(
            &keyboard_input,
            TEXT_INPUT_FOCUS_SWITCH_LOG_TICKS,
            "ab",
            "axb",
            1,
            2,
            None,
            None,
        ));
        assert!(!should_log_text_input_keyboard_event(
            &keyboard_input,
            TEXT_INPUT_FOCUS_SWITCH_LOG_TICKS + 1,
            "ab",
            "axb",
            1,
            2,
            None,
            None,
        ));
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
    fn focused_text_input_border_is_stable_while_interacting() {
        let theme = UiTheme::default();

        assert_eq!(
            text_input_border_color(&theme, Interaction::Pressed, true, false, false),
            theme.colors.primary_button.focused
        );
        assert_eq!(
            text_input_border_color(&theme, Interaction::Hovered, true, false, false),
            theme.colors.primary_button.focused
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
        let metrics = UiMetrics::default();
        let node = icon_button_node(&theme, &metrics);

        assert_eq!(node.min_width, px(square_button_size(&metrics)));
        assert_eq!(node.width, px(square_button_size(&metrics)));
        assert_eq!(node.height, px(square_button_size(&metrics)));
        assert_eq!(node.padding, UiRect::ZERO);
        assert_eq!(
            node.border_radius,
            BorderRadius::all(px(theme.button.radius))
        );
    }

    #[test]
    fn compact_metrics_keep_core_control_nodes_at_touch_target() {
        let theme = UiTheme::default();
        let metrics = UiMetrics::default();
        let button = button_node(&theme, &metrics);
        let text_input = Node {
            min_height: px(metrics.input_height),
            ..default()
        };
        let icon = icon_button_node(&theme, &metrics);

        assert_eq!(button.height, px(metrics.button_height));
        assert!(metrics.button_height >= metrics.touch_target_min);
        assert!(metrics.input_height >= metrics.touch_target_min);
        assert_eq!(text_input.min_height, px(metrics.input_height));
        assert_eq!(icon.width, px(square_button_size(&metrics)));
        assert!(square_button_size(&metrics) >= metrics.touch_target_min);
    }

    #[test]
    fn stepper_value_width_is_metrics_derived_and_stable() {
        let metrics = UiMetrics::default();
        let first = stepper_value_node(&metrics);
        let second = stepper_value_node(&metrics);

        assert_eq!(first.width, px(stepper_value_width(&metrics)));
        assert_eq!(first.width, second.width);
        assert_eq!(first.min_height, second.min_height);
    }

    #[test]
    fn compact_numeric_controls_fit_phone_panel_width() {
        let theme = UiTheme::default();
        let viewport = crate::framework::ui::core::UiViewport::from_device_logical_size(
            1080.0 / 3.0,
            2400.0 / 3.0,
            crate::framework::ui::core::UiInputMode::MouseTouch,
            crate::framework::ui::core::UiSafeArea::default(),
        );
        let metrics = UiMetrics::from_viewport_and_theme(&viewport, &theme);
        let panel_inner_width = viewport.logical_width
            - metrics.page_padding * 2.0
            - theme.layout.panel_gap * 2.0
            - theme.panel.border * 2.0;
        let slider_min_width = numeric_control_label_width(&metrics)
            + slider_track_min_width(&metrics)
            + stepper_value_width(&metrics)
            + numeric_control_gap(&metrics) * 2.0
            + control_padding_x(&metrics) * 2.0;
        let stepper_min_width = numeric_control_label_width(&metrics)
            + square_button_size(&metrics) * 2.0
            + stepper_value_width(&metrics)
            + numeric_control_gap(&metrics) * 3.0;

        assert!(slider_min_width <= panel_inner_width);
        assert!(stepper_min_width <= panel_inner_width);
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
