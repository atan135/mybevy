use std::fmt;

use bevy::{input::keyboard::Key, picking::Pickable, prelude::*, ui::FocusPolicy};

use crate::framework::ui::widgets::icon::{
    UiIconId, UiIconResolutionStatus, UiIconVisual, apply_ui_icon_tint, ui_icon,
};
use crate::framework::ui::{
    core::{UiCurrentOwner, UiOwnerId, UiPanelCommand, UiPanelRequest, focus::UiFocusState},
    i18n::{UiI18n, UiI18nText},
    overlays::{UiDropdownPanel, UiTooltipPanel},
    style::{
        UiFontAssets, UiTheme,
        fonts::truncate_with_ellipsis,
        theme::{UiThemeButtonNodeRole, UiThemeTextStyleRole},
    },
};

use super::{
    DisabledButton, FocusableButton, FocusedButton, LoadingButton, SelectedButton, UiButtonEvent,
    UiButtonEventKind,
};

const DROPDOWN_LABEL_MAX_ASCII_GRAPHEMES: usize = 18;
const DROPDOWN_LABEL_MAX_WIDE_GRAPHEMES: usize = 10;
const DROPDOWN_CHEVRON_WIDTH: f32 = 14.0;
const DROPDOWN_CHEVRON_HEIGHT: f32 = 14.0;
const DROPDOWN_CHEVRON_RENDER_SIZE: f32 = 24.0;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct UiControlId(&'static str);

impl UiControlId {
    pub(crate) const fn new(value: &'static str) -> Self {
        Self(value)
    }

    pub(crate) const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for UiControlId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum UiControlKind {
    Button,
    ImageButton,
    TextInput,
    Badge,
    Progress,
    Tab,
    Tooltip,
    Dropdown,
    Checkbox,
    Toggle,
    Segmented,
    Slider,
    Stepper,
    Scroll,
    Modal,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub(crate) enum UiControlState {
    #[default]
    Normal,
    Hovered,
    Pressed,
    Focused,
    Selected,
    Disabled,
    Loading,
    Empty,
    Error,
}

impl UiControlKind {
    pub(crate) const fn supports_state(self, state: UiControlState) -> bool {
        use UiControlKind as Kind;
        use UiControlState as State;

        match self {
            Kind::Button | Kind::ImageButton | Kind::Tab => matches!(
                state,
                State::Normal
                    | State::Hovered
                    | State::Pressed
                    | State::Focused
                    | State::Selected
                    | State::Disabled
                    | State::Loading
            ),
            Kind::TextInput => matches!(
                state,
                State::Normal
                    | State::Hovered
                    | State::Pressed
                    | State::Focused
                    | State::Disabled
                    | State::Empty
                    | State::Error
            ),
            Kind::Badge => matches!(
                state,
                State::Normal
                    | State::Selected
                    | State::Disabled
                    | State::Loading
                    | State::Empty
                    | State::Error
            ),
            Kind::Progress => matches!(
                state,
                State::Normal | State::Disabled | State::Loading | State::Empty | State::Error
            ),
            Kind::Tooltip => matches!(state, State::Normal | State::Disabled | State::Error),
            Kind::Dropdown => true,
            Kind::Checkbox | Kind::Toggle | Kind::Segmented => !matches!(state, State::Empty),
            Kind::Slider | Kind::Stepper => matches!(
                state,
                State::Normal
                    | State::Hovered
                    | State::Pressed
                    | State::Focused
                    | State::Disabled
                    | State::Error
            ),
            Kind::Scroll | Kind::Modal => matches!(
                state,
                State::Normal | State::Loading | State::Empty | State::Error
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Component)]
pub(crate) struct UiControlFlags {
    pub selected: bool,
    pub disabled: bool,
    pub loading: bool,
    pub empty: bool,
    pub error: bool,
}

impl UiControlFlags {
    pub(crate) const fn from_state(state: UiControlState) -> Self {
        Self {
            selected: matches!(state, UiControlState::Selected),
            disabled: matches!(state, UiControlState::Disabled),
            loading: matches!(state, UiControlState::Loading),
            empty: matches!(state, UiControlState::Empty),
            error: matches!(state, UiControlState::Error),
        }
    }
}

#[derive(Clone, Copy, Debug, Component)]
pub(crate) struct UiControlMeta {
    pub id: UiControlId,
    pub kind: UiControlKind,
}

impl UiControlMeta {
    pub(crate) const fn new(id: UiControlId, kind: UiControlKind) -> Self {
        Self { id, kind }
    }
}

#[derive(Clone, Copy, Debug, Component)]
pub(crate) struct UiControlOwner(pub UiOwnerId);

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum UiControlValue {
    None,
    Bool(bool),
    Text(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiControlEventKind {
    ValueChanged,
    Opened,
    Closed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiControlEventReason {
    Pointer,
    Keyboard,
    ClickAway,
    Escape,
    OwnerRemoved,
}

#[derive(Clone, Debug, Message, PartialEq)]
pub(crate) struct UiControlEvent {
    pub entity: Entity,
    pub owner: Option<UiOwnerId>,
    pub control_id: UiControlId,
    pub control_kind: UiControlKind,
    pub kind: UiControlEventKind,
    pub value: UiControlValue,
    pub reason: UiControlEventReason,
}

#[derive(Clone, Debug, Component)]
pub(crate) struct UiBadge {
    pub state: UiControlState,
}

#[derive(Component)]
pub(crate) struct UiBadgeLabel;

#[derive(Clone, Copy, Debug, Component)]
pub(crate) struct UiProgress {
    pub value: f32,
    pub state: UiControlState,
}

impl UiProgress {
    pub(crate) fn new(value: f32, state: UiControlState) -> Self {
        Self {
            value: if value.is_finite() {
                value.clamp(0.0, 1.0)
            } else {
                0.0
            },
            state,
        }
    }
}

#[derive(Component)]
pub(crate) struct UiProgressFill;

#[derive(Component)]
pub(crate) struct UiProgressLabel {
    key: Option<&'static str>,
    fallback: String,
}

impl UiProgressLabel {
    pub(crate) fn set_dynamic_fallback(&mut self, fallback: String) {
        self.key = None;
        self.fallback = fallback;
    }
}

#[derive(Component)]
pub(crate) struct UiTabList;

#[derive(Clone, Debug, Component)]
pub(crate) struct UiTab {
    pub value: String,
}

#[derive(Component)]
pub(crate) struct UiTabIndicator;

#[derive(Component)]
pub(crate) struct UiTabLabel;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct UiDropdownOption {
    pub value: String,
    pub label: String,
    pub disabled: bool,
}

impl UiDropdownOption {
    pub(crate) fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            disabled: false,
        }
    }

    pub(crate) fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }
}

#[derive(Clone, Debug, Component)]
pub(crate) struct UiDropdown {
    pub placeholder: String,
    pub options: Vec<UiDropdownOption>,
    pub selected: Option<usize>,
    loading_text: String,
    empty_text: String,
    error_text: String,
}

impl UiDropdown {
    pub(crate) fn new(
        placeholder: impl Into<String>,
        options: Vec<UiDropdownOption>,
        selected: Option<usize>,
    ) -> Self {
        Self {
            placeholder: placeholder.into(),
            selected: selected.filter(|index| *index < options.len()),
            options,
            loading_text: "Loading...".to_string(),
            empty_text: "No options".to_string(),
            error_text: "Unable to load options".to_string(),
        }
    }

    pub(crate) fn with_status_text(
        mut self,
        loading: impl Into<String>,
        empty: impl Into<String>,
        error: impl Into<String>,
    ) -> Self {
        self.loading_text = loading.into();
        self.empty_text = empty.into();
        self.error_text = error.into();
        self
    }

    pub(crate) fn selected_option(&self) -> Option<&UiDropdownOption> {
        self.selected.and_then(|index| self.options.get(index))
    }

    pub(crate) fn set_document_status_text(
        &mut self,
        empty: Option<String>,
        error: Option<String>,
    ) {
        if let Some(empty) = empty {
            self.empty_text = empty;
        }
        if let Some(error) = error {
            self.error_text = error;
        }
    }

    pub(crate) fn display_text(&self, flags: UiControlFlags) -> String {
        if flags.loading {
            return self.loading_text.clone();
        }
        if flags.error {
            return self.error_text.clone();
        }
        if flags.empty || self.options.is_empty() {
            return self.empty_text.clone();
        }

        self.selected_option()
            .map(|option| option.label.clone())
            .unwrap_or_else(|| self.placeholder.clone())
    }
}

#[derive(Component)]
pub(crate) struct UiDropdownLabel;

#[derive(Component)]
pub(crate) struct UiDropdownLabelFrame;

#[derive(Component)]
pub(crate) struct UiDropdownChevron;

#[derive(Component)]
pub(crate) struct UiDropdownChevronIcon;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UiTooltipTone {
    Standard,
    Error,
}

#[derive(Clone, Debug, Component)]
pub(crate) struct UiTooltip {
    pub text: String,
    pub tone: UiTooltipTone,
}

#[derive(Component)]
pub(crate) struct UiTooltipPinned;

pub(crate) fn resolve_control_state(
    interaction: Interaction,
    focused: bool,
    flags: UiControlFlags,
) -> UiControlState {
    if flags.disabled {
        UiControlState::Disabled
    } else if flags.loading {
        UiControlState::Loading
    } else if flags.error {
        UiControlState::Error
    } else {
        match interaction {
            Interaction::Pressed => UiControlState::Pressed,
            Interaction::Hovered => UiControlState::Hovered,
            Interaction::None if flags.selected => UiControlState::Selected,
            Interaction::None if focused => UiControlState::Focused,
            Interaction::None if flags.empty => UiControlState::Empty,
            Interaction::None => UiControlState::Normal,
        }
    }
}

pub(crate) fn badge_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
    state: UiControlState,
) -> impl Bundle {
    debug_assert!(UiControlKind::Badge.supports_state(state));
    (
        UiBadge { state },
        UiControlMeta::new(UiControlId::new(key), UiControlKind::Badge),
        badge_node(theme),
        BackgroundColor(control_state_color(theme, state)),
        BorderColor::all(control_state_border_color(theme, state)),
        children![(
            UiBadgeLabel,
            Text::new(i18n.tr(key, fallback)),
            TextFont {
                font: fonts.regular.clone(),
                font_size: theme.text.caption,
                ..default()
            },
            TextColor(control_state_text_color(theme, state)),
            UiThemeTextStyleRole::Caption,
            UiI18nText::new(key, fallback),
        )],
    )
}

pub(crate) fn badge(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
    state: UiControlState,
) -> impl Bundle {
    debug_assert!(UiControlKind::Badge.supports_state(state));
    (
        UiBadge { state },
        UiControlMeta::new(UiControlId::new("document.badge"), UiControlKind::Badge),
        badge_node(theme),
        BackgroundColor(control_state_color(theme, state)),
        BorderColor::all(control_state_border_color(theme, state)),
        children![(
            UiBadgeLabel,
            Text::new(text),
            TextFont {
                font: fonts.regular.clone(),
                font_size: theme.text.caption,
                ..default()
            },
            TextColor(control_state_text_color(theme, state)),
            UiThemeTextStyleRole::Caption,
        )],
    )
}

pub(crate) fn progress_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
    value: f32,
    state: UiControlState,
) -> impl Bundle {
    debug_assert!(UiControlKind::Progress.supports_state(state));
    let progress = UiProgress::new(value, state);
    let fill_width = progress_fill_width(progress);
    (
        progress,
        UiControlMeta::new(UiControlId::new(key), UiControlKind::Progress),
        Node {
            width: percent(100),
            min_height: px(36),
            align_items: AlignItems::Center,
            column_gap: px(theme.layout.row_gap.max(8.0)),
            ..default()
        },
        children![
            (
                Node {
                    min_width: px(92),
                    flex_grow: 1.0,
                    height: px(10),
                    border: UiRect::all(px(theme.panel.border)),
                    border_radius: BorderRadius::all(px(5)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(theme.colors.secondary_button.idle),
                BorderColor::all(control_state_border_color(theme, state)),
                children![(
                    UiProgressFill,
                    Node {
                        width: percent(fill_width),
                        height: percent(100),
                        border_radius: BorderRadius::all(px(5)),
                        ..default()
                    },
                    BackgroundColor(progress_fill_color(theme, state)),
                )],
            ),
            (
                UiProgressLabel {
                    key: Some(key),
                    fallback: fallback.to_owned(),
                },
                Node {
                    width: px(54),
                    ..default()
                },
                Text::new(progress_display_text(progress, i18n.tr(key, fallback))),
                TextFont {
                    font: fonts.regular.clone(),
                    font_size: theme.text.caption,
                    ..default()
                },
                TextColor(control_state_text_color(theme, state)),
                UiThemeTextStyleRole::Caption,
            ),
        ],
    )
}

pub(crate) fn progress(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
    value: f32,
    state: UiControlState,
) -> impl Bundle {
    debug_assert!(UiControlKind::Progress.supports_state(state));
    let fallback = text.into();
    let progress = UiProgress::new(value, state);
    let fill_width = progress_fill_width(progress);
    (
        progress,
        UiControlMeta::new(
            UiControlId::new("document.progress"),
            UiControlKind::Progress,
        ),
        Node {
            width: percent(100),
            min_height: px(36),
            align_items: AlignItems::Center,
            column_gap: px(theme.layout.row_gap.max(8.0)),
            ..default()
        },
        children![
            (
                Node {
                    min_width: px(92),
                    flex_grow: 1.0,
                    height: px(10),
                    border: UiRect::all(px(theme.panel.border)),
                    border_radius: BorderRadius::all(px(5)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(theme.colors.secondary_button.idle),
                BorderColor::all(control_state_border_color(theme, state)),
                children![(
                    UiProgressFill,
                    Node {
                        width: percent(fill_width),
                        height: percent(100),
                        border_radius: BorderRadius::all(px(5)),
                        ..default()
                    },
                    BackgroundColor(progress_fill_color(theme, state)),
                )],
            ),
            (
                UiProgressLabel {
                    key: None,
                    fallback: fallback.clone(),
                },
                Node {
                    width: px(54),
                    ..default()
                },
                Text::new(progress_display_text(progress, fallback)),
                TextFont {
                    font: fonts.regular.clone(),
                    font_size: theme.text.caption,
                    ..default()
                },
                TextColor(control_state_text_color(theme, state)),
                UiThemeTextStyleRole::Caption,
            ),
        ],
    )
}

pub(crate) fn tab_list(theme: &UiTheme) -> impl Bundle {
    (
        UiTabList,
        Node {
            width: percent(100),
            min_height: px(theme.button.height),
            align_items: AlignItems::Stretch,
            column_gap: px(theme.layout.row_gap.max(4.0)),
            flex_wrap: FlexWrap::Wrap,
            padding: UiRect::all(px(3)),
            border: UiRect::all(px(theme.panel.border)),
            border_radius: BorderRadius::all(px(theme.button.radius)),
            ..default()
        },
        BackgroundColor(theme.colors.secondary_button.idle),
        BorderColor::all(theme.colors.panel_border),
    )
}

pub(crate) fn tab_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    value: impl Into<String>,
    key: &'static str,
    fallback: &'static str,
    state: UiControlState,
) -> impl Bundle {
    debug_assert!(UiControlKind::Tab.supports_state(state));
    let flags = UiControlFlags::from_state(state);
    (
        Button,
        FocusableButton,
        UiTab {
            value: value.into(),
        },
        UiControlMeta::new(UiControlId::new(key), UiControlKind::Tab),
        flags,
        UiThemeButtonNodeRole::Button,
        Node {
            min_width: px(theme.button.min_width),
            height: px(theme.button.height),
            flex_grow: 1.0,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::axes(px(theme.button.padding_x), px(0)),
            border_radius: BorderRadius::all(px((theme.button.radius - 2.0).max(0.0))),
            ..default()
        },
        BackgroundColor(control_state_color(theme, state)),
        children![
            (
                UiTabLabel,
                Text::new(i18n.tr(key, fallback)),
                TextFont {
                    font: fonts.regular.clone(),
                    font_size: theme.text.button,
                    ..default()
                },
                TextColor(control_state_text_color(theme, state)),
                UiThemeTextStyleRole::Button,
                UiI18nText::new(key, fallback),
            ),
            (
                UiTabIndicator,
                Pickable::IGNORE,
                Node {
                    position_type: PositionType::Absolute,
                    left: px(theme.button.padding_x),
                    right: px(theme.button.padding_x),
                    bottom: px(3),
                    height: px(2),
                    border_radius: BorderRadius::all(px(1)),
                    ..default()
                },
                BackgroundColor(theme.colors.primary_button.focused),
                if flags.selected {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                },
            ),
        ],
    )
}

pub(crate) fn tab(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    value: impl Into<String>,
    text: impl Into<String>,
    state: UiControlState,
) -> impl Bundle {
    debug_assert!(UiControlKind::Tab.supports_state(state));
    let flags = UiControlFlags::from_state(state);
    (
        Button,
        FocusableButton,
        UiTab {
            value: value.into(),
        },
        UiControlMeta::new(UiControlId::new("document.tab"), UiControlKind::Tab),
        flags,
        UiThemeButtonNodeRole::Button,
        Node {
            min_width: px(theme.button.min_width),
            height: px(theme.button.height),
            flex_grow: 1.0,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::axes(px(theme.button.padding_x), px(0)),
            border_radius: BorderRadius::all(px((theme.button.radius - 2.0).max(0.0))),
            ..default()
        },
        BackgroundColor(control_state_color(theme, state)),
        children![
            (
                UiTabLabel,
                Text::new(text),
                TextFont {
                    font: fonts.regular.clone(),
                    font_size: theme.text.button,
                    ..default()
                },
                TextColor(control_state_text_color(theme, state)),
                UiThemeTextStyleRole::Button,
            ),
            (
                UiTabIndicator,
                Pickable::IGNORE,
                Node {
                    position_type: PositionType::Absolute,
                    left: px(theme.button.padding_x),
                    right: px(theme.button.padding_x),
                    bottom: px(3),
                    height: px(2),
                    border_radius: BorderRadius::all(px(1)),
                    ..default()
                },
                BackgroundColor(theme.colors.primary_button.focused),
                if flags.selected {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                },
            ),
        ],
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn dropdown_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    asset_server: &AssetServer,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
    options: Vec<UiDropdownOption>,
    selected: Option<usize>,
    state: UiControlState,
) -> impl Bundle {
    debug_assert!(UiControlKind::Dropdown.supports_state(state));
    let dropdown = UiDropdown::new(i18n.tr(key, fallback), options, selected).with_status_text(
        i18n.tr("ui.controls.loading", "Loading..."),
        i18n.tr("ui.controls.empty", "No options"),
        i18n.tr("ui.controls.error", "Unable to load options"),
    );
    let flags = UiControlFlags::from_state(state);
    let display = dropdown_display_text(&dropdown, flags);
    (
        Button,
        FocusableButton,
        UiControlMeta::new(UiControlId::new(key), UiControlKind::Dropdown),
        flags,
        dropdown,
        UiThemeButtonNodeRole::TextInput,
        Node {
            width: percent(100),
            height: px(theme.button.height),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            column_gap: px(theme.layout.row_gap.max(8.0)),
            padding: UiRect::axes(px(theme.button.padding_x), px(0)),
            border: UiRect::all(px(theme.panel.border)),
            border_radius: BorderRadius::all(px(theme.button.radius)),
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(control_state_color(theme, state)),
        BorderColor::all(control_state_border_color(theme, state)),
        children![
            (
                UiDropdownLabelFrame,
                FocusPolicy::Pass,
                Node {
                    min_width: px(0),
                    height: percent(100),
                    flex_grow: 1.0,
                    align_items: AlignItems::Center,
                    overflow: Overflow::clip(),
                    ..default()
                },
                children![(
                    UiDropdownLabel,
                    FocusPolicy::Pass,
                    Text::new(display),
                    TextFont {
                        font: fonts.regular.clone(),
                        font_size: theme.text.button,
                        ..default()
                    },
                    TextColor(control_state_text_color(theme, state)),
                    TextLayout::new(Justify::Left, bevy::text::LineBreak::NoWrap),
                    UiThemeTextStyleRole::Button,
                )],
            ),
            (
                UiDropdownChevron,
                FocusPolicy::Pass,
                Node {
                    width: px(DROPDOWN_CHEVRON_WIDTH),
                    min_width: px(DROPDOWN_CHEVRON_WIDTH),
                    height: px(DROPDOWN_CHEVRON_HEIGHT),
                    min_height: px(DROPDOWN_CHEVRON_HEIGHT),
                    flex_shrink: 0.0,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    overflow: Overflow::clip(),
                    ..default()
                },
                children![(
                    UiDropdownChevronIcon,
                    Pickable::IGNORE,
                    ui_icon(
                        asset_server,
                        UiIconId::CHEVRON_DOWN,
                        DROPDOWN_CHEVRON_RENDER_SIZE,
                        control_state_text_color(theme, state),
                    ),
                )],
            ),
        ],
    )
}

pub(crate) fn tooltip_target(
    control_id: UiControlId,
    text: impl Into<String>,
    tone: UiTooltipTone,
) -> impl Bundle {
    (
        UiControlMeta::new(control_id, UiControlKind::Tooltip),
        UiTooltip {
            text: text.into(),
            tone,
        },
    )
}

pub(crate) fn control_event_reason(event: &UiButtonEvent) -> UiControlEventReason {
    if event.button.is_some() {
        UiControlEventReason::Pointer
    } else {
        UiControlEventReason::Keyboard
    }
}

pub(crate) fn event_owner(
    owner: Option<&UiControlOwner>,
    current_owner: &UiCurrentOwner,
) -> Option<UiOwnerId> {
    owner.map(|owner| owner.0).or(current_owner.owner)
}

fn badge_node(theme: &UiTheme) -> Node {
    Node {
        min_width: px(24),
        min_height: px(24),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        padding: UiRect::axes(px(theme.layout.row_gap.max(8.0)), px(2)),
        border: UiRect::all(px(theme.panel.border)),
        border_radius: BorderRadius::all(px(theme.button.radius.min(12.0))),
        ..default()
    }
}

fn control_state_color(theme: &UiTheme, state: UiControlState) -> Color {
    match state {
        UiControlState::Normal | UiControlState::Empty => theme.colors.secondary_button.idle,
        UiControlState::Hovered => theme.colors.secondary_button.hovered,
        UiControlState::Pressed => theme.colors.secondary_button.pressed,
        UiControlState::Focused => theme.colors.secondary_button.focused,
        UiControlState::Selected => theme.colors.primary_button.selected,
        UiControlState::Disabled => theme.colors.secondary_button.disabled,
        UiControlState::Loading => theme.colors.secondary_button.loading,
        UiControlState::Error => theme.colors.error.with_alpha(0.28),
    }
}

fn control_state_border_color(theme: &UiTheme, state: UiControlState) -> Color {
    match state {
        UiControlState::Focused => theme.colors.primary_button.focused,
        UiControlState::Selected => theme.colors.primary_button.hovered,
        UiControlState::Error => theme.colors.error,
        UiControlState::Disabled => theme.colors.secondary_button.disabled,
        UiControlState::Normal
        | UiControlState::Hovered
        | UiControlState::Pressed
        | UiControlState::Loading
        | UiControlState::Empty => theme.colors.panel_border,
    }
}

fn control_state_text_color(theme: &UiTheme, state: UiControlState) -> Color {
    match state {
        UiControlState::Disabled | UiControlState::Empty => theme.colors.text_muted,
        UiControlState::Error => theme.colors.text_error,
        _ => theme.colors.text_primary,
    }
}

fn progress_fill_color(theme: &UiTheme, state: UiControlState) -> Color {
    match state {
        UiControlState::Error => theme.colors.error,
        UiControlState::Disabled | UiControlState::Empty => theme.colors.secondary_button.disabled,
        UiControlState::Loading => theme.colors.icon_tint.loading,
        _ => theme.colors.primary_button.idle,
    }
}

fn progress_fill_width(progress: UiProgress) -> f32 {
    match progress.state {
        UiControlState::Empty | UiControlState::Error => 0.0,
        UiControlState::Loading => 62.0,
        _ => progress.value * 100.0,
    }
}

pub(crate) fn progress_display_text(progress: UiProgress, fallback: String) -> String {
    match progress.state {
        UiControlState::Loading | UiControlState::Empty | UiControlState::Error => fallback,
        _ => format!("{:.0}%", progress.value * 100.0),
    }
}

fn dropdown_display_text(dropdown: &UiDropdown, flags: UiControlFlags) -> String {
    let display = dropdown.display_text(flags);
    let max_graphemes = if display.is_ascii() {
        DROPDOWN_LABEL_MAX_ASCII_GRAPHEMES
    } else {
        DROPDOWN_LABEL_MAX_WIDE_GRAPHEMES
    };
    truncate_with_ellipsis(&display, max_graphemes)
}

pub(crate) fn sync_control_gate_markers(
    mut commands: Commands,
    controls: Query<(
        Entity,
        &UiControlFlags,
        Has<DisabledButton>,
        Has<LoadingButton>,
    )>,
) {
    for (entity, flags, has_disabled, has_loading) in &controls {
        if flags.disabled != has_disabled {
            if flags.disabled {
                commands.entity(entity).insert(DisabledButton);
            } else {
                commands.entity(entity).remove::<DisabledButton>();
            }
        }
        if flags.loading != has_loading {
            if flags.loading {
                commands.entity(entity).insert(LoadingButton);
            } else {
                commands.entity(entity).remove::<LoadingButton>();
            }
        }
    }
}

pub(crate) fn update_component_control_interactions(
    mut commands: Commands,
    current_owner: Res<UiCurrentOwner>,
    parents: Query<&ChildOf>,
    tab_lists: Query<(), With<UiTabList>>,
    mut tab_queries: ParamSet<(
        Query<
            (
                &UiTab,
                &UiControlMeta,
                &UiControlFlags,
                Has<DisabledButton>,
                Has<LoadingButton>,
                Option<&UiControlOwner>,
            ),
            Without<UiDropdown>,
        >,
        Query<(Entity, &mut UiControlFlags), (With<UiTab>, Without<UiDropdown>)>,
    )>,
    dropdowns: Query<
        (
            &UiDropdown,
            &UiControlMeta,
            &UiControlFlags,
            Option<&UiControlOwner>,
        ),
        Without<UiTab>,
    >,
    mut button_events: MessageReader<UiButtonEvent>,
    mut panel_commands: MessageWriter<UiPanelCommand>,
    mut control_events: MessageWriter<UiControlEvent>,
) {
    for event in button_events.read() {
        if event.kind != UiButtonEventKind::Click {
            continue;
        }

        let tab = tab_queries.p0().get(event.entity).ok().map(
            |(tab, meta, flags, disabled, loading, owner)| {
                (
                    tab.value.clone(),
                    *meta,
                    *flags,
                    disabled,
                    loading,
                    owner.copied(),
                )
            },
        );
        if let Some((value, meta, flags, disabled, loading, owner)) = tab {
            if flags.disabled || flags.loading || disabled || loading {
                continue;
            }
            let root = parents
                .iter_ancestors(event.entity)
                .find(|ancestor| tab_lists.contains(*ancestor));
            for (candidate, mut candidate_flags) in &mut tab_queries.p1() {
                let same_root = root.is_some_and(|root| {
                    parents
                        .iter_ancestors(candidate)
                        .any(|ancestor| ancestor == root)
                });
                if same_root {
                    let selected = candidate == event.entity;
                    if candidate_flags.selected != selected {
                        candidate_flags.selected = selected;
                    }
                    if selected {
                        commands.entity(candidate).insert(SelectedButton);
                    } else {
                        commands.entity(candidate).remove::<SelectedButton>();
                    }
                }
            }
            control_events.write(UiControlEvent {
                entity: event.entity,
                owner: event_owner(owner.as_ref(), &current_owner),
                control_id: meta.id,
                control_kind: meta.kind,
                kind: UiControlEventKind::ValueChanged,
                value: UiControlValue::Text(value),
                reason: control_event_reason(event),
            });
            continue;
        }

        let Ok((dropdown, meta, flags, owner)) = dropdowns.get(event.entity) else {
            continue;
        };
        if flags.disabled
            || flags.loading
            || flags.empty
            || flags.error
            || dropdown.options.is_empty()
        {
            continue;
        }
        panel_commands.write(UiPanelCommand::Open(UiPanelRequest::Dropdown(
            UiDropdownPanel {
                anchor: event.entity,
                meta: *meta,
                owner: event_owner(owner, &current_owner),
                dropdown: dropdown.clone(),
            },
        )));
        control_events.write(UiControlEvent {
            entity: event.entity,
            owner: event_owner(owner, &current_owner),
            control_id: meta.id,
            control_kind: meta.kind,
            kind: UiControlEventKind::Opened,
            value: dropdown
                .selected_option()
                .map(|option| UiControlValue::Text(option.value.clone()))
                .unwrap_or(UiControlValue::None),
            reason: control_event_reason(event),
        });
    }
}

pub(crate) fn sync_component_control_visuals(
    theme: Res<UiTheme>,
    mut interactive: Query<
        (
            Entity,
            &Interaction,
            &UiControlMeta,
            &UiControlFlags,
            Has<FocusedButton>,
            &mut BackgroundColor,
            Option<&mut BorderColor>,
        ),
        Or<(With<UiTab>, With<UiDropdown>)>,
    >,
    children: Query<&Children>,
    mut text_colors: Query<&mut TextColor>,
    mut indicators: Query<&mut Visibility, With<UiTabIndicator>>,
    mut chevrons: Query<
        (&mut ImageNode, &mut UiIconVisual, &UiIconResolutionStatus),
        With<UiDropdownChevronIcon>,
    >,
    dropdowns: Query<&UiDropdown>,
    mut dropdown_labels: Query<&mut Text, With<UiDropdownLabel>>,
) {
    for (entity, interaction, meta, flags, focused, mut background, border) in &mut interactive {
        let state = resolve_control_state(*interaction, focused, *flags);
        let next_background = BackgroundColor(control_state_color(&theme, state));
        if *background != next_background {
            *background = next_background;
        }
        if let Some(mut border) = border {
            let next = BorderColor::all(control_state_border_color(&theme, state));
            if *border != next {
                *border = next;
            }
        }
        for child in children.iter_descendants(entity) {
            if let Ok(mut color) = text_colors.get_mut(child) {
                let next = TextColor(control_state_text_color(&theme, state));
                if *color != next {
                    *color = next;
                }
            }
            if let Ok(mut visibility) = indicators.get_mut(child) {
                let next = if flags.selected {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                };
                if *visibility != next {
                    *visibility = next;
                }
            }
            if let Ok((mut image, mut visual, resolution)) = chevrons.get_mut(child) {
                apply_ui_icon_tint(
                    control_state_text_color(&theme, state),
                    &mut image,
                    &mut visual,
                    resolution,
                );
            }
        }
        if meta.kind == UiControlKind::Dropdown
            && let Ok(dropdown) = dropdowns.get(entity)
        {
            let display = dropdown_display_text(dropdown, *flags);
            for child in children.iter_descendants(entity) {
                if let Ok(mut text) = dropdown_labels.get_mut(child)
                    && text.0 != display
                {
                    text.0 = display.clone();
                }
            }
        }
    }
}

pub(crate) fn sync_static_component_visuals(
    theme: Res<UiTheme>,
    i18n: Res<UiI18n>,
    mut badges: Query<
        (Entity, &UiBadge, &mut BackgroundColor, &mut BorderColor),
        Without<UiProgress>,
    >,
    progresses: Query<(Entity, &UiProgress), Without<UiBadge>>,
    children: Query<&Children>,
    mut badge_labels: Query<&mut TextColor, (With<UiBadgeLabel>, Without<UiProgressLabel>)>,
    mut progress_fills: Query<
        (&mut Node, &mut BackgroundColor),
        (With<UiProgressFill>, Without<UiBadge>),
    >,
    mut progress_labels: Query<
        (&UiProgressLabel, &mut Text, &mut TextColor),
        (With<UiProgressLabel>, Without<UiBadgeLabel>),
    >,
) {
    for (entity, badge, mut background, mut border) in &mut badges {
        let next_background = BackgroundColor(control_state_color(&theme, badge.state));
        if *background != next_background {
            *background = next_background;
        }
        let next_border = BorderColor::all(control_state_border_color(&theme, badge.state));
        if *border != next_border {
            *border = next_border;
        }
        for child in children.iter_descendants(entity) {
            if let Ok(mut color) = badge_labels.get_mut(child) {
                let next = TextColor(control_state_text_color(&theme, badge.state));
                if *color != next {
                    *color = next;
                }
            }
        }
    }

    for (entity, progress) in &progresses {
        for child in children.iter_descendants(entity) {
            if let Ok((mut node, mut background)) = progress_fills.get_mut(child) {
                let next_width = percent(progress_fill_width(*progress));
                if node.width != next_width {
                    node.width = next_width;
                }
                let next_background = BackgroundColor(progress_fill_color(&theme, progress.state));
                if *background != next_background {
                    *background = next_background;
                }
            }
            if let Ok((label, mut text, mut color)) = progress_labels.get_mut(child) {
                let fallback = label.key.map_or_else(
                    || label.fallback.clone(),
                    |key| i18n.tr(key, label.fallback.clone()),
                );
                let next_text = progress_display_text(*progress, fallback);
                if text.0 != next_text {
                    text.0 = next_text;
                }
                let next_color = TextColor(control_state_text_color(&theme, progress.state));
                if *color != next_color {
                    *color = next_color;
                }
            }
        }
    }
}

pub(crate) fn sync_tooltip_visibility(
    current_owner: Res<UiCurrentOwner>,
    controls: Query<(
        Entity,
        &Interaction,
        &UiTooltip,
        &UiControlMeta,
        Option<&UiControlOwner>,
        Has<FocusedButton>,
        Has<UiTooltipPinned>,
    )>,
    open_tooltips: Query<&crate::framework::ui::overlays::UiPopoverAnchor>,
    mut panel_commands: MessageWriter<UiPanelCommand>,
) {
    let desired = controls
        .iter()
        .filter(|(_, interaction, _, _, _, focused, pinned)| {
            tooltip_visibility_requested(**interaction, *focused, *pinned)
        })
        .min_by_key(|(entity, ..)| entity.to_bits());
    let open = open_tooltips
        .iter()
        .find(|popover| popover.kind == UiControlKind::Tooltip);

    match (desired, open) {
        (Some((entity, _, _, _, _, _, _)), Some(open)) if open.anchor == entity => {}
        (Some((entity, _, tooltip, meta, owner, _, _)), _) => {
            panel_commands.write(UiPanelCommand::Open(UiPanelRequest::Tooltip(
                UiTooltipPanel {
                    anchor: entity,
                    meta: *meta,
                    owner: event_owner(owner, &current_owner),
                    tooltip: tooltip.clone(),
                },
            )));
        }
        (None, Some(_)) => {
            panel_commands.write(UiPanelCommand::Close(
                crate::framework::ui::core::UI_PANEL_TOOLTIP,
            ));
        }
        (None, None) => {}
    }
}

fn tooltip_visibility_requested(interaction: Interaction, focused: bool, pinned: bool) -> bool {
    matches!(interaction, Interaction::Hovered | Interaction::Pressed) || focused || pinned
}

pub(crate) fn handle_dropdown_keyboard(
    key_codes: Res<ButtonInput<KeyCode>>,
    keys: Res<ButtonInput<Key>>,
    mut focus_state: ResMut<UiFocusState>,
    dropdown_roots: Query<&crate::framework::ui::overlays::UiPopoverAnchor>,
    options: Query<(
        Entity,
        &crate::framework::ui::overlays::UiDropdownOptionButton,
        Has<DisabledButton>,
    )>,
) {
    let Some(popover) = dropdown_roots
        .iter()
        .find(|popover| popover.kind == UiControlKind::Dropdown)
    else {
        return;
    };

    if key_codes.just_pressed(KeyCode::Escape) || keys.just_pressed(Key::BrowserBack) {
        return;
    }

    let mut candidates = options
        .iter()
        .filter(|(_, option, disabled)| option.control == popover.anchor && !disabled)
        .map(|(entity, option, _)| (option.index, entity))
        .collect::<Vec<_>>();
    candidates.sort_by_key(|(index, _)| *index);
    if candidates.is_empty() {
        return;
    }

    let direction = if key_codes.just_pressed(KeyCode::ArrowDown) {
        1_i32
    } else if key_codes.just_pressed(KeyCode::ArrowUp) {
        -1_i32
    } else if key_codes.just_pressed(KeyCode::Home) {
        focus_state.focused_entity = candidates.first().map(|(_, entity)| *entity);
        return;
    } else if key_codes.just_pressed(KeyCode::End) {
        focus_state.focused_entity = candidates.last().map(|(_, entity)| *entity);
        return;
    } else {
        return;
    };

    let current = focus_state
        .focused_entity
        .and_then(|focused| candidates.iter().position(|(_, entity)| *entity == focused));
    let next = match (current, direction) {
        (Some(index), 1) => (index + 1) % candidates.len(),
        (Some(index), -1) => index.checked_sub(1).unwrap_or(candidates.len() - 1),
        (None, 1) => 0,
        (None, -1) => candidates.len() - 1,
        _ => 0,
    };
    focus_state.focused_entity = Some(candidates[next].1);
}

pub(crate) fn focus_opened_dropdown(
    mut focus_state: ResMut<UiFocusState>,
    roots: Query<
        &crate::framework::ui::overlays::UiPopoverAnchor,
        With<crate::framework::ui::overlays::UiDropdownOverlay>,
    >,
    options: Query<(
        Entity,
        &crate::framework::ui::overlays::UiDropdownOptionButton,
        Has<DisabledButton>,
    )>,
) {
    let Ok(popover) = roots.single() else {
        return;
    };
    let mut candidates = options
        .iter()
        .filter(|(_, option, disabled)| option.control == popover.anchor && !disabled)
        .map(|(entity, option, _)| (option.index, option.selected, entity))
        .collect::<Vec<_>>();
    candidates.sort_by_key(|(index, _, _)| *index);
    if candidates.is_empty() {
        return;
    }
    if focus_state.focused_entity.is_some_and(|focused| {
        candidates
            .iter()
            .any(|(_, _, candidate)| *candidate == focused)
    }) {
        return;
    }
    focus_state.focused_entity = candidates
        .iter()
        .find(|(_, selected, _)| *selected)
        .or_else(|| candidates.first())
        .map(|(_, _, entity)| *entity);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_i18n(entries: &[(&str, &str)]) -> UiI18n {
        UiI18n::test_with_texts("zh_cn", entries)
    }

    #[test]
    fn support_matrix_rejects_meaningless_states() {
        assert!(!UiControlKind::Progress.supports_state(UiControlState::Pressed));
        assert!(!UiControlKind::Tooltip.supports_state(UiControlState::Selected));
        assert!(!UiControlKind::Checkbox.supports_state(UiControlState::Empty));
        assert!(UiControlKind::Dropdown.supports_state(UiControlState::Error));
        assert!(UiControlKind::Tab.supports_state(UiControlState::Focused));
    }

    #[test]
    fn pinned_tooltip_requests_visibility_without_pointer_or_focus() {
        assert!(tooltip_visibility_requested(Interaction::None, false, true));
        assert!(tooltip_visibility_requested(
            Interaction::Hovered,
            false,
            false
        ));
        assert!(tooltip_visibility_requested(Interaction::None, true, false));
        assert!(!tooltip_visibility_requested(
            Interaction::None,
            false,
            false
        ));
    }

    #[test]
    fn visual_state_priority_is_stable() {
        let all = UiControlFlags {
            selected: true,
            disabled: true,
            loading: true,
            empty: true,
            error: true,
        };
        assert_eq!(
            resolve_control_state(Interaction::Pressed, true, all),
            UiControlState::Disabled
        );
        assert_eq!(
            resolve_control_state(
                Interaction::Pressed,
                true,
                UiControlFlags {
                    selected: true,
                    loading: true,
                    error: true,
                    ..default()
                },
            ),
            UiControlState::Loading
        );
        assert_eq!(
            resolve_control_state(
                Interaction::Pressed,
                true,
                UiControlFlags {
                    selected: true,
                    error: true,
                    ..default()
                },
            ),
            UiControlState::Error
        );
        assert_eq!(
            resolve_control_state(
                Interaction::None,
                true,
                UiControlFlags {
                    selected: true,
                    ..default()
                },
            ),
            UiControlState::Selected
        );
    }

    #[test]
    fn dropdown_clamps_selection_and_formats_terminal_states() {
        let options = vec![UiDropdownOption::new("one", "One")];
        let dropdown = UiDropdown::new("Choose", options, Some(8));
        assert_eq!(dropdown.selected, None);
        assert_eq!(dropdown.display_text(UiControlFlags::default()), "Choose");
        assert_eq!(
            dropdown.display_text(UiControlFlags {
                empty: true,
                ..default()
            }),
            "No options"
        );
        assert_eq!(
            dropdown.display_text(UiControlFlags {
                error: true,
                ..default()
            }),
            "Unable to load options"
        );
    }

    #[test]
    fn progress_normalizes_nan_and_terminal_fill_width() {
        let progress = UiProgress::new(f32::NAN, UiControlState::Normal);
        assert_eq!(progress.value, 0.0);
        assert_eq!(progress_fill_width(progress), 0.0);
        assert_eq!(
            progress_fill_width(UiProgress::new(0.8, UiControlState::Loading)),
            62.0
        );
        assert_eq!(
            progress_fill_width(UiProgress::new(0.8, UiControlState::Error)),
            0.0
        );
    }

    #[test]
    fn progress_terminal_text_uses_localized_fallback() {
        let theme = UiTheme::default();
        let fonts = UiFontAssets::test_registry();
        let i18n = test_i18n(&[("test.progress.error", "加载失败")]);
        let mut world = World::new();
        let root = world
            .spawn(progress_key(
                &theme,
                &fonts,
                &i18n,
                "test.progress.error",
                "Error",
                0.8,
                UiControlState::Error,
            ))
            .id();
        let label = world
            .get::<Children>(root)
            .unwrap()
            .iter()
            .find(|entity| world.get::<UiProgressLabel>(*entity).is_some())
            .unwrap();

        assert_eq!(world.get::<Text>(label).unwrap().0, "加载失败");
    }

    #[test]
    fn dropdown_long_text_keeps_fixed_root_geometry_and_single_line_clip() {
        let theme = UiTheme::default();
        let fonts = UiFontAssets::test_registry();
        let i18n = test_i18n(&[("test.dropdown", "选择区域")]);
        let options = vec![UiDropdownOption::new(
            "long",
            "这是一个用于验证长文本换行且不会改变控件点击区域尺寸的区域名称",
        )];
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Image>();
        let asset_server = app.world().resource::<AssetServer>().clone();
        let world = app.world_mut();
        let normal = world
            .spawn(dropdown_key(
                &theme,
                &fonts,
                &asset_server,
                &i18n,
                "test.dropdown",
                "Choose",
                options.clone(),
                None,
                UiControlState::Normal,
            ))
            .id();
        let selected = world
            .spawn(dropdown_key(
                &theme,
                &fonts,
                &asset_server,
                &i18n,
                "test.dropdown",
                "Choose",
                options,
                Some(0),
                UiControlState::Selected,
            ))
            .id();

        let normal_node = world.get::<Node>(normal).unwrap();
        let selected_node = world.get::<Node>(selected).unwrap();
        assert_eq!(normal_node.height, px(theme.button.height));
        assert_eq!(selected_node.height, normal_node.height);
        assert_eq!(selected_node.overflow, Overflow::clip());

        let frame = world
            .get::<Children>(selected)
            .unwrap()
            .iter()
            .find(|entity| world.get::<UiDropdownLabelFrame>(*entity).is_some())
            .unwrap();
        assert_eq!(world.get::<Node>(frame).unwrap().overflow, Overflow::clip());
        let chevron = world
            .get::<Children>(selected)
            .unwrap()
            .iter()
            .find(|entity| world.get::<UiDropdownChevron>(*entity).is_some())
            .unwrap();
        let chevron_node = world.get::<Node>(chevron).unwrap();
        assert_eq!(chevron_node.width, px(DROPDOWN_CHEVRON_WIDTH));
        assert_eq!(chevron_node.min_width, px(DROPDOWN_CHEVRON_WIDTH));
        assert_eq!(chevron_node.height, px(DROPDOWN_CHEVRON_HEIGHT));
        assert_eq!(chevron_node.min_height, px(DROPDOWN_CHEVRON_HEIGHT));
        assert_eq!(chevron_node.flex_shrink, 0.0);
        assert_eq!(chevron_node.overflow, Overflow::clip());
        let chevron_icon = world
            .get::<Children>(chevron)
            .unwrap()
            .iter()
            .find(|entity| world.get::<UiDropdownChevronIcon>(*entity).is_some())
            .unwrap();
        let chevron_icon_node = world.get::<Node>(chevron_icon).unwrap();
        assert_eq!(chevron_icon_node.width, px(DROPDOWN_CHEVRON_RENDER_SIZE));
        assert_eq!(chevron_icon_node.height, px(DROPDOWN_CHEVRON_RENDER_SIZE));
        let resolution = world.get::<UiIconResolutionStatus>(chevron_icon).unwrap();
        assert_eq!(resolution.requested, UiIconId::CHEVRON_DOWN);
        assert_eq!(resolution.rendered, UiIconId::CHEVRON_DOWN);
        assert_eq!(resolution.path, "ui/icons/chevron-down.png");
        assert_eq!(
            world.get::<ImageNode>(chevron_icon).unwrap().color,
            control_state_text_color(&theme, UiControlState::Selected)
        );
        assert!(world.get::<Text>(chevron_icon).is_none());
        let label = world
            .get::<Children>(frame)
            .unwrap()
            .iter()
            .find(|entity| world.get::<UiDropdownLabel>(*entity).is_some())
            .unwrap();
        assert_eq!(
            world.get::<TextLayout>(label).unwrap().linebreak,
            bevy::text::LineBreak::NoWrap
        );
        let rendered = &world.get::<Text>(label).unwrap().0;
        assert!(rendered.ends_with('…'));
        assert_eq!(
            unicode_segmentation::UnicodeSegmentation::graphemes(rendered.as_str(), true).count(),
            DROPDOWN_LABEL_MAX_WIDE_GRAPHEMES
        );
    }

    #[test]
    fn dropdown_chevron_icon_keeps_geometry_and_tints_across_terminal_states() {
        let theme = UiTheme::default();
        let fonts = UiFontAssets::test_registry();
        let i18n = test_i18n(&[("test.dropdown", "选择区域")]);
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Image>();
        let asset_server = app.world().resource::<AssetServer>().clone();
        let world = app.world_mut();
        let mut image_id = None;

        for state in [
            UiControlState::Normal,
            UiControlState::Selected,
            UiControlState::Disabled,
            UiControlState::Loading,
            UiControlState::Error,
        ] {
            let root = world
                .spawn(dropdown_key(
                    &theme,
                    &fonts,
                    &asset_server,
                    &i18n,
                    "test.dropdown",
                    "Choose",
                    vec![UiDropdownOption::new("one", "One")],
                    (state == UiControlState::Selected).then_some(0),
                    state,
                ))
                .id();
            let frame = world
                .get::<Children>(root)
                .unwrap()
                .iter()
                .find(|entity| world.get::<UiDropdownChevron>(*entity).is_some())
                .unwrap();
            let icon = world
                .get::<Children>(frame)
                .unwrap()
                .iter()
                .find(|entity| world.get::<UiDropdownChevronIcon>(*entity).is_some())
                .unwrap();
            let frame_node = world.get::<Node>(frame).unwrap();
            assert_eq!(frame_node.width, px(DROPDOWN_CHEVRON_WIDTH));
            assert_eq!(frame_node.height, px(DROPDOWN_CHEVRON_HEIGHT));
            let image = world.get::<ImageNode>(icon).unwrap();
            assert_eq!(image.color, control_state_text_color(&theme, state));
            if let Some(expected) = image_id {
                assert_eq!(image.image.id(), expected);
            } else {
                image_id = Some(image.image.id());
            }
        }
    }

    #[test]
    fn dropdown_truncation_keeps_short_ascii_labels_and_reserves_wide_ellipsis() {
        let short_ascii = UiDropdown::new(
            "Choose a region",
            vec![UiDropdownOption::new("one", "One")],
            None,
        );
        assert_eq!(
            dropdown_display_text(&short_ascii, UiControlFlags::default()),
            "Choose a region"
        );

        let long_ascii = UiDropdown::new(
            "A deliberately long dropdown label",
            vec![UiDropdownOption::new("one", "One")],
            None,
        );
        let rendered = dropdown_display_text(&long_ascii, UiControlFlags::default());
        assert!(rendered.ends_with('…'));
        assert_eq!(
            unicode_segmentation::UnicodeSegmentation::graphemes(rendered.as_str(), true).count(),
            DROPDOWN_LABEL_MAX_ASCII_GRAPHEMES
        );

        let long_wide = UiDropdown::new(
            "\u{8fd9}\u{662f}\u{4e00}\u{4e2a}\u{7528}\u{4e8e}\u{9a8c}\u{8bc1}\u{957f}\u{6587}\
             \u{672c}\u{6362}\u{884c}\u{7684}\u{533a}\u{57df}\u{540d}\u{79f0}",
            vec![UiDropdownOption::new("one", "One")],
            None,
        );
        let rendered = dropdown_display_text(&long_wide, UiControlFlags::default());
        assert!(rendered.ends_with('…'));
        assert_eq!(
            unicode_segmentation::UnicodeSegmentation::graphemes(rendered.as_str(), true).count(),
            DROPDOWN_LABEL_MAX_WIDE_GRAPHEMES
        );
    }

    #[test]
    fn flags_only_disabled_and_loading_tabs_ignore_clicks() {
        let mut app = App::new();
        app.init_resource::<UiCurrentOwner>()
            .add_message::<UiButtonEvent>()
            .add_message::<UiControlEvent>()
            .add_message::<UiPanelCommand>()
            .add_systems(Update, update_component_control_interactions);
        let tab_list_entity = app.world_mut().spawn(UiTabList).id();
        let disabled = app
            .world_mut()
            .spawn((
                Button,
                UiTab {
                    value: "disabled".to_owned(),
                },
                UiControlMeta::new(UiControlId::new("test.tab.disabled"), UiControlKind::Tab),
                UiControlFlags {
                    disabled: true,
                    ..default()
                },
            ))
            .id();
        let loading = app
            .world_mut()
            .spawn((
                Button,
                UiTab {
                    value: "loading".to_owned(),
                },
                UiControlMeta::new(UiControlId::new("test.tab.loading"), UiControlKind::Tab),
                UiControlFlags {
                    loading: true,
                    ..default()
                },
            ))
            .id();
        app.world_mut()
            .entity_mut(tab_list_entity)
            .add_children(&[disabled, loading]);
        for entity in [disabled, loading] {
            app.world_mut().write_message(UiButtonEvent {
                entity,
                kind: UiButtonEventKind::Click,
                button: None,
            });
        }

        app.update();

        assert!(
            !app.world()
                .get::<UiControlFlags>(disabled)
                .unwrap()
                .selected
        );
        assert!(!app.world().get::<UiControlFlags>(loading).unwrap().selected);
        assert!(!app.world().entity(disabled).contains::<SelectedButton>());
        assert!(!app.world().entity(loading).contains::<SelectedButton>());
        let messages = app.world().resource::<Messages<UiControlEvent>>();
        let mut cursor = bevy::ecs::message::MessageCursor::default();
        assert_eq!(cursor.read(messages).count(), 0);
    }

    #[test]
    fn component_visual_sync_is_change_stable_after_first_frame() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        app.init_asset::<Image>()
            .insert_resource(UiTheme::default())
            .insert_resource(test_i18n(&[("test.progress", "加载中")]))
            .add_systems(
                Update,
                (
                    sync_component_control_visuals,
                    sync_static_component_visuals,
                ),
            );

        let badge = app
            .world_mut()
            .spawn((
                UiBadge {
                    state: UiControlState::Selected,
                },
                BackgroundColor(Color::NONE),
                BorderColor::all(Color::NONE),
            ))
            .id();
        let badge_label = app
            .world_mut()
            .spawn((UiBadgeLabel, TextColor(Color::NONE)))
            .id();
        app.world_mut().entity_mut(badge).add_child(badge_label);

        let progress = app
            .world_mut()
            .spawn(UiProgress::new(0.72, UiControlState::Loading))
            .id();
        let fill = app
            .world_mut()
            .spawn((
                UiProgressFill,
                Node::default(),
                BackgroundColor(Color::NONE),
            ))
            .id();
        let progress_label = app
            .world_mut()
            .spawn((
                UiProgressLabel {
                    key: Some("test.progress"),
                    fallback: "Loading...".to_owned(),
                },
                Text::new("stale"),
                TextColor(Color::NONE),
            ))
            .id();
        app.world_mut()
            .entity_mut(progress)
            .add_children(&[fill, progress_label]);

        let tab = app
            .world_mut()
            .spawn((
                Button,
                Interaction::None,
                UiTab {
                    value: "tab".to_owned(),
                },
                UiControlMeta::new(UiControlId::new("test.tab"), UiControlKind::Tab),
                UiControlFlags {
                    selected: true,
                    ..default()
                },
                BackgroundColor(Color::NONE),
                BorderColor::all(Color::NONE),
            ))
            .id();
        let tab_text = app.world_mut().spawn(TextColor(Color::NONE)).id();
        let indicator = app
            .world_mut()
            .spawn((UiTabIndicator, Visibility::Hidden))
            .id();
        app.world_mut()
            .entity_mut(tab)
            .add_children(&[tab_text, indicator]);

        let dropdown = app
            .world_mut()
            .spawn((
                Button,
                Interaction::None,
                UiDropdown::new("Choose", vec![UiDropdownOption::new("one", "One")], Some(0)),
                UiControlMeta::new(UiControlId::new("test.dropdown"), UiControlKind::Dropdown),
                UiControlFlags {
                    selected: true,
                    ..default()
                },
                BackgroundColor(Color::NONE),
                BorderColor::all(Color::NONE),
            ))
            .id();
        let dropdown_label = app
            .world_mut()
            .spawn((UiDropdownLabel, Text::new("stale"), TextColor(Color::NONE)))
            .id();
        let asset_server = app.world().resource::<AssetServer>().clone();
        let chevron = app
            .world_mut()
            .spawn((
                UiDropdownChevronIcon,
                ui_icon(
                    &asset_server,
                    UiIconId::CHEVRON_DOWN,
                    DROPDOWN_CHEVRON_RENDER_SIZE,
                    Color::NONE,
                ),
            ))
            .id();
        app.world_mut()
            .entity_mut(dropdown)
            .add_children(&[dropdown_label, chevron]);

        app.update();
        app.world_mut().clear_trackers();
        app.update();

        for entity in [badge, tab, dropdown] {
            assert!(
                !app.world()
                    .entity(entity)
                    .get_ref::<BackgroundColor>()
                    .unwrap()
                    .is_changed()
            );
        }
        assert!(
            !app.world()
                .entity(badge)
                .get_ref::<BorderColor>()
                .unwrap()
                .is_changed()
        );
        assert!(
            !app.world()
                .entity(badge_label)
                .get_ref::<TextColor>()
                .unwrap()
                .is_changed()
        );
        assert!(
            !app.world()
                .entity(fill)
                .get_ref::<Node>()
                .unwrap()
                .is_changed()
        );
        assert!(
            !app.world()
                .entity(fill)
                .get_ref::<BackgroundColor>()
                .unwrap()
                .is_changed()
        );
        assert!(
            !app.world()
                .entity(progress_label)
                .get_ref::<Text>()
                .unwrap()
                .is_changed()
        );
        assert!(
            !app.world()
                .entity(indicator)
                .get_ref::<Visibility>()
                .unwrap()
                .is_changed()
        );
        assert!(
            !app.world()
                .entity(chevron)
                .get_ref::<ImageNode>()
                .unwrap()
                .is_changed()
        );
        assert!(
            !app.world()
                .entity(chevron)
                .get_ref::<UiIconVisual>()
                .unwrap()
                .is_changed()
        );
    }

    #[test]
    fn theme_and_i18n_resource_changes_preserve_dropdown_selection_state() {
        let mut app = App::new();
        app.insert_resource(UiTheme::default())
            .insert_resource(test_i18n(&[("test.dropdown", "选择区域")]))
            .add_systems(Update, sync_component_control_visuals);
        let dropdown = app
            .world_mut()
            .spawn((
                Button,
                Interaction::None,
                UiDropdown::new(
                    "选择区域",
                    vec![UiDropdownOption::new("north", "北境")],
                    Some(0),
                ),
                UiControlMeta::new(UiControlId::new("test.dropdown"), UiControlKind::Dropdown),
                UiControlFlags {
                    selected: true,
                    ..default()
                },
                SelectedButton,
                BackgroundColor(Color::NONE),
                BorderColor::all(Color::NONE),
            ))
            .id();
        let label = app
            .world_mut()
            .spawn((UiDropdownLabel, Text::new("stale"), TextColor(Color::NONE)))
            .id();
        app.world_mut().entity_mut(dropdown).add_child(label);
        app.update();

        let mut next_theme = UiTheme::default();
        next_theme.colors.text_primary = Color::srgb(0.2, 0.8, 0.4);
        app.insert_resource(next_theme);
        app.insert_resource(UiI18n::test_with_texts(
            "en_us",
            &[("test.dropdown", "Choose a region")],
        ));
        app.update();

        assert_eq!(
            app.world().get::<UiDropdown>(dropdown).unwrap().selected,
            Some(0)
        );
        assert!(
            app.world()
                .get::<UiControlFlags>(dropdown)
                .unwrap()
                .selected
        );
        assert!(app.world().entity(dropdown).contains::<SelectedButton>());
        assert_eq!(app.world().get::<Text>(label).unwrap().0, "北境");
    }
}
