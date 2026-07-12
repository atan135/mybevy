use bevy::{picking::Pickable, prelude::*, ui::FocusPolicy};

use crate::framework::ui::{
    core::UiCurrentOwner,
    i18n::{UiI18n, UiI18nText},
    style::{
        UiFontAssets, UiTheme,
        theme::{ButtonColors, UiThemeButtonNodeRole, UiThemeTextColorRole, UiThemeTextStyleRole},
    },
};

use super::*;

const SELECTION_INDICATOR_SIZE: f32 = 22.0;
const TOGGLE_TRACK_WIDTH: f32 = 42.0;
const TOGGLE_TRACK_HEIGHT: f32 = 24.0;
const TOGGLE_THUMB_SIZE: f32 = 18.0;

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
pub(crate) struct UiSelectionLabel;

#[derive(Component)]
pub(crate) struct UiCheckboxBox;

#[derive(Component)]
pub(crate) struct UiCheckboxMark;

#[derive(Component)]
pub(crate) struct UiToggleTrack;

#[derive(Component)]
pub(crate) struct UiToggleThumb;

#[derive(Component)]
pub(crate) struct UiSegmentIndicator;

#[derive(Component)]
pub(crate) struct UiSelectionText;

#[allow(dead_code)]
pub(crate) fn checkbox(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
) -> impl Bundle {
    checkbox_bundle(theme, fonts, text, (), SelectionVisualState::Idle, (), ())
}

pub(crate) fn checkbox_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    checkbox_bundle(
        theme,
        fonts,
        i18n.tr(key, fallback),
        UiI18nText::new(key, fallback),
        SelectionVisualState::Idle,
        UiControlMeta::new(UiControlId::new(key), UiControlKind::Checkbox),
        (),
    )
}

#[allow(dead_code)]
pub(crate) fn checked_checkbox(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
) -> impl Bundle {
    checkbox_bundle(
        theme,
        fonts,
        text,
        (),
        SelectionVisualState::Selected,
        (),
        (UiCheckboxChecked, SelectedButton),
    )
}

pub(crate) fn checked_checkbox_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    checkbox_bundle(
        theme,
        fonts,
        i18n.tr(key, fallback),
        UiI18nText::new(key, fallback),
        SelectionVisualState::Selected,
        UiControlMeta::new(UiControlId::new(key), UiControlKind::Checkbox),
        (UiCheckboxChecked, SelectedButton),
    )
}

pub(crate) fn disabled_checkbox_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    checkbox_bundle(
        theme,
        fonts,
        i18n.tr(key, fallback),
        UiI18nText::new(key, fallback),
        SelectionVisualState::Disabled,
        UiControlMeta::new(UiControlId::new(key), UiControlKind::Checkbox),
        DisabledButton,
    )
}

#[allow(dead_code)]
pub(crate) fn toggle(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
) -> impl Bundle {
    toggle_bundle(theme, fonts, text, (), SelectionVisualState::Idle, (), ())
}

pub(crate) fn toggle_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    toggle_bundle(
        theme,
        fonts,
        i18n.tr(key, fallback),
        UiI18nText::new(key, fallback),
        SelectionVisualState::Idle,
        UiControlMeta::new(UiControlId::new(key), UiControlKind::Toggle),
        (),
    )
}

#[allow(dead_code)]
pub(crate) fn toggle_on(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
) -> impl Bundle {
    toggle_bundle(
        theme,
        fonts,
        text,
        (),
        SelectionVisualState::Selected,
        (),
        (UiToggleOn, SelectedButton),
    )
}

pub(crate) fn toggle_on_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    toggle_bundle(
        theme,
        fonts,
        i18n.tr(key, fallback),
        UiI18nText::new(key, fallback),
        SelectionVisualState::Selected,
        UiControlMeta::new(UiControlId::new(key), UiControlKind::Toggle),
        (UiToggleOn, SelectedButton),
    )
}

pub(crate) fn disabled_toggle_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
) -> impl Bundle {
    toggle_bundle(
        theme,
        fonts,
        i18n.tr(key, fallback),
        UiI18nText::new(key, fallback),
        SelectionVisualState::Disabled,
        UiControlMeta::new(UiControlId::new(key), UiControlKind::Toggle),
        DisabledButton,
    )
}

pub(crate) fn segmented_control(theme: &UiTheme) -> impl Bundle {
    (
        UiSegmentedControl,
        Node {
            width: percent(100),
            min_height: px(theme.button.height),
            align_items: AlignItems::Stretch,
            column_gap: px(3),
            padding: UiRect::all(px(3)),
            border: UiRect::all(px(theme.panel.border)),
            border_radius: BorderRadius::all(px(theme.button.radius)),
            ..default()
        },
        BackgroundColor(theme.colors.secondary_button.idle),
        BorderColor::all(theme.colors.panel_border),
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
        UiControlMeta::new(UiControlId::new(key), UiControlKind::Segmented),
        (),
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
    segment_option_key_bundle(
        theme,
        fonts,
        i18n.tr(key, fallback),
        value,
        SelectionVisualState::Selected,
        UiI18nText::new(key, fallback),
        UiControlMeta::new(UiControlId::new(key), UiControlKind::Segmented),
        (UiSegmentOptionSelected, SelectedButton),
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
    segment_option_key_bundle(
        theme,
        fonts,
        i18n.tr(key, fallback),
        value,
        SelectionVisualState::Disabled,
        UiI18nText::new(key, fallback),
        UiControlMeta::new(UiControlId::new(key), UiControlKind::Segmented),
        DisabledButton,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SelectionVisualState {
    Idle,
    Selected,
    Disabled,
    Loading,
    Error,
}

fn checkbox_bundle<I: Bundle, M: Bundle, S: Bundle>(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
    i18n: I,
    state: SelectionVisualState,
    meta: M,
    state_marker: S,
) -> impl Bundle {
    let selected = state == SelectionVisualState::Selected;
    (
        selection_root(theme, state),
        UiCheckbox,
        UiSelectionLabel,
        UiControlFlags {
            selected,
            disabled: state == SelectionVisualState::Disabled,
            ..default()
        },
        meta,
        state_marker,
        children![
            (
                UiCheckboxBox,
                FocusPolicy::Pass,
                Node {
                    width: px(SELECTION_INDICATOR_SIZE),
                    height: px(SELECTION_INDICATOR_SIZE),
                    flex_shrink: 0.0,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border: UiRect::all(px(theme.panel.border.max(1.0))),
                    border_radius: BorderRadius::all(px(4)),
                    ..default()
                },
                BackgroundColor(theme.colors.secondary_button.idle),
                BorderColor::all(selection_indicator_border(theme, state)),
                children![(
                    UiCheckboxMark,
                    Pickable::IGNORE,
                    Node {
                        width: px(12),
                        height: px(12),
                        border_radius: BorderRadius::all(px(2)),
                        ..default()
                    },
                    BackgroundColor(theme.colors.primary_button.hovered),
                    if selected {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    },
                )],
            ),
            selection_text(theme, fonts, text, i18n, state),
        ],
    )
}

fn toggle_bundle<I: Bundle, M: Bundle, S: Bundle>(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
    i18n: I,
    state: SelectionVisualState,
    meta: M,
    state_marker: S,
) -> impl Bundle {
    let selected = state == SelectionVisualState::Selected;
    (
        selection_root(theme, state),
        UiToggle,
        UiSelectionLabel,
        UiControlFlags {
            selected,
            disabled: state == SelectionVisualState::Disabled,
            ..default()
        },
        meta,
        state_marker,
        children![
            (
                UiToggleTrack,
                FocusPolicy::Pass,
                Node {
                    width: px(TOGGLE_TRACK_WIDTH),
                    height: px(TOGGLE_TRACK_HEIGHT),
                    flex_shrink: 0.0,
                    border: UiRect::all(px(theme.panel.border.max(1.0))),
                    border_radius: BorderRadius::all(px(TOGGLE_TRACK_HEIGHT * 0.5)),
                    ..default()
                },
                BackgroundColor(if selected {
                    theme.colors.primary_button.selected
                } else {
                    theme.colors.secondary_button.idle
                }),
                BorderColor::all(selection_indicator_border(theme, state)),
                children![(
                    UiToggleThumb,
                    Pickable::IGNORE,
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(if selected { 20 } else { 2 }),
                        top: px(2),
                        width: px(TOGGLE_THUMB_SIZE),
                        height: px(TOGGLE_THUMB_SIZE),
                        border_radius: BorderRadius::all(px(TOGGLE_THUMB_SIZE * 0.5)),
                        ..default()
                    },
                    BackgroundColor(if state == SelectionVisualState::Disabled {
                        theme.colors.text_muted
                    } else {
                        theme.colors.text_primary
                    }),
                )],
            ),
            selection_text(theme, fonts, text, i18n, state),
        ],
    )
}

fn segment_option_key_bundle<I: Bundle, M: Bundle, S: Bundle>(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
    value: impl Into<String>,
    state: SelectionVisualState,
    i18n: I,
    meta: M,
    state_marker: S,
) -> impl Bundle {
    let selected = state == SelectionVisualState::Selected;
    (
        Button,
        FocusableButton,
        UiSegmentOption {
            value: value.into(),
        },
        UiSelectionLabel,
        UiControlFlags {
            selected,
            disabled: state == SelectionVisualState::Disabled,
            ..default()
        },
        meta,
        state_marker,
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
        BackgroundColor(selection_button_background_color(
            theme.colors.secondary_button,
            Interaction::None,
            false,
            state,
        )),
        children![
            selection_text(theme, fonts, text, i18n, state),
            (
                UiSegmentIndicator,
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
                if selected {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                },
            ),
        ],
    )
}

fn selection_root(theme: &UiTheme, state: SelectionVisualState) -> impl Bundle {
    (
        Button,
        FocusableButton,
        UiThemeButtonNodeRole::Button,
        Node {
            min_width: px(theme.button.min_width),
            height: px(theme.button.height),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::FlexStart,
            column_gap: px(theme.layout.row_gap.max(8.0)),
            padding: UiRect::axes(px(theme.button.padding_x), px(0)),
            border_radius: BorderRadius::all(px(theme.button.radius)),
            ..default()
        },
        BackgroundColor(selection_button_background_color(
            theme.colors.secondary_button,
            Interaction::None,
            false,
            state,
        )),
    )
}

fn selection_text<I: Bundle>(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
    i18n: I,
    state: SelectionVisualState,
) -> impl Bundle {
    (
        UiSelectionText,
        FocusPolicy::Pass,
        Node {
            min_width: px(0),
            flex_grow: 1.0,
            ..default()
        },
        Text::new(text),
        TextFont {
            font: fonts.regular.clone(),
            font_size: theme.text.button,
            ..default()
        },
        TextColor(selection_button_text_color(theme, state)),
        selection_button_text_color_role(state),
        UiThemeTextStyleRole::Button,
        i18n,
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
        state == SelectionVisualState::Loading,
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
        SelectionVisualState::Idle
        | SelectionVisualState::Selected
        | SelectionVisualState::Loading
        | SelectionVisualState::Error => UiThemeTextColorRole::Primary,
    }
}

#[cfg(test)]
pub(crate) fn selection_display_text(base_text: &str, _state: SelectionVisualState) -> String {
    base_text.to_string()
}

fn selection_indicator_border(theme: &UiTheme, state: SelectionVisualState) -> Color {
    match state {
        SelectionVisualState::Selected => theme.colors.primary_button.hovered,
        SelectionVisualState::Disabled => theme.colors.secondary_button.disabled,
        SelectionVisualState::Loading => theme.colors.icon_tint.loading,
        SelectionVisualState::Error => theme.colors.error,
        SelectionVisualState::Idle => theme.colors.panel_border,
    }
}

pub(crate) fn update_selection_control_interactions(
    mut commands: Commands,
    current_owner: Res<UiCurrentOwner>,
    parents: Query<&ChildOf>,
    segmented_roots: Query<(), With<UiSegmentedControl>>,
    segment_options: Query<(Entity, Option<&UiControlFlags>), With<UiSegmentOption>>,
    buttons: Query<
        (
            Has<UiCheckbox>,
            Has<UiCheckboxChecked>,
            Has<UiToggle>,
            Has<UiToggleOn>,
            Option<&UiSegmentOption>,
            Option<&UiControlMeta>,
            Option<&UiControlOwner>,
            Option<&UiControlFlags>,
        ),
        (
            With<Button>,
            Without<DisabledButton>,
            Without<LoadingButton>,
            Without<UiStepper>,
        ),
    >,
    mut button_events: MessageReader<UiButtonEvent>,
    mut control_events: MessageWriter<UiControlEvent>,
) {
    for event in button_events.read() {
        if event.kind != UiButtonEventKind::Click {
            continue;
        }

        let Ok((is_checkbox, is_checked, is_toggle, is_toggle_on, segment, meta, owner, flags)) =
            buttons.get(event.entity)
        else {
            continue;
        };

        if flags.is_some_and(|flags| flags.disabled || flags.loading) {
            continue;
        }

        let mut event_value = UiControlValue::None;
        if is_checkbox {
            let selected = !(is_checked || flags.is_some_and(|flags| flags.selected));
            set_selection_markers::<UiCheckboxChecked>(
                &mut commands,
                event.entity,
                selected,
                flags.copied(),
            );
            event_value = UiControlValue::Bool(selected);
        } else if is_toggle {
            let selected = !(is_toggle_on || flags.is_some_and(|flags| flags.selected));
            set_selection_markers::<UiToggleOn>(
                &mut commands,
                event.entity,
                selected,
                flags.copied(),
            );
            event_value = UiControlValue::Bool(selected);
        } else if let Some(segment) = segment {
            let root = parents
                .iter_ancestors(event.entity)
                .find(|ancestor| segmented_roots.contains(*ancestor));

            for (selected_entity, selected_flags) in &segment_options {
                if selected_entity == event.entity {
                    continue;
                }
                let same_root = root.is_some_and(|root| {
                    parents
                        .iter_ancestors(selected_entity)
                        .any(|ancestor| ancestor == root)
                });
                if same_root {
                    set_selection_markers::<UiSegmentOptionSelected>(
                        &mut commands,
                        selected_entity,
                        false,
                        selected_flags.copied(),
                    );
                }
            }

            set_selection_markers::<UiSegmentOptionSelected>(
                &mut commands,
                event.entity,
                true,
                flags.copied(),
            );
            event_value = UiControlValue::Text(segment.value.clone());
        }

        if !matches!(event_value, UiControlValue::None)
            && let Some(meta) = meta
        {
            control_events.write(UiControlEvent {
                entity: event.entity,
                owner: event_owner(owner, &current_owner),
                control_id: meta.id,
                control_kind: meta.kind,
                kind: UiControlEventKind::ValueChanged,
                value: event_value,
                reason: control_event_reason(event),
            });
        }
    }
}

fn set_selection_markers<T: Component + Default>(
    commands: &mut Commands,
    entity: Entity,
    selected: bool,
    flags: Option<UiControlFlags>,
) {
    let mut next_flags = flags.unwrap_or_default();
    next_flags.selected = selected;
    if selected {
        commands
            .entity(entity)
            .insert((T::default(), SelectedButton, next_flags));
    } else {
        commands
            .entity(entity)
            .remove::<T>()
            .remove::<SelectedButton>()
            .insert(next_flags);
    }
}

pub(crate) fn sync_selection_control_visuals(
    theme: Res<UiTheme>,
    mut controls: Query<
        (
            Entity,
            &Interaction,
            &mut BackgroundColor,
            Has<FocusedButton>,
            Has<DisabledButton>,
            Has<UiCheckboxChecked>,
            Has<UiToggleOn>,
            Has<UiSegmentOptionSelected>,
            Has<UiCheckbox>,
            Has<UiToggle>,
            Has<UiSegmentOption>,
            Option<&UiControlFlags>,
        ),
        (
            With<Button>,
            With<UiSelectionLabel>,
            Without<UiCheckboxBox>,
            Without<UiToggleTrack>,
            Without<UiToggleThumb>,
        ),
    >,
    children: Query<&Children>,
    mut checkbox_boxes: Query<
        (&mut BackgroundColor, &mut BorderColor),
        (
            With<UiCheckboxBox>,
            Without<UiToggleTrack>,
            Without<UiToggleThumb>,
        ),
    >,
    mut checkbox_marks: Query<&mut Visibility, (With<UiCheckboxMark>, Without<UiSegmentIndicator>)>,
    mut toggle_tracks: Query<
        (&mut BackgroundColor, &mut BorderColor),
        (
            With<UiToggleTrack>,
            Without<UiCheckboxBox>,
            Without<UiToggleThumb>,
        ),
    >,
    mut toggle_thumbs: Query<(&mut Node, &mut BackgroundColor), With<UiToggleThumb>>,
    mut segment_indicators: Query<
        &mut Visibility,
        (With<UiSegmentIndicator>, Without<UiCheckboxMark>),
    >,
    mut text_colors: Query<&mut TextColor, With<UiSelectionText>>,
) {
    for (
        entity,
        interaction,
        mut background,
        focused,
        disabled_marker,
        checked,
        toggle_on,
        segment_selected,
        is_checkbox,
        is_toggle,
        is_segment,
        flags,
    ) in &mut controls
    {
        if !is_checkbox && !is_toggle && !is_segment {
            continue;
        }
        let flags = flags.copied().unwrap_or_default();
        let disabled = disabled_marker || flags.disabled;
        let selected = checked || toggle_on || segment_selected || flags.selected;
        let state = if disabled {
            SelectionVisualState::Disabled
        } else if flags.loading {
            SelectionVisualState::Loading
        } else if flags.error {
            SelectionVisualState::Error
        } else if selected {
            SelectionVisualState::Selected
        } else {
            SelectionVisualState::Idle
        };
        let colors = if toggle_on || (is_toggle && selected) {
            theme.colors.primary_button
        } else {
            theme.colors.secondary_button
        };
        let next = if state == SelectionVisualState::Error {
            theme.colors.error.with_alpha(0.28)
        } else {
            selection_button_background_color(colors, *interaction, focused, state)
        };
        if background.0 != next {
            *background = BackgroundColor(next);
        }

        for child in children.iter_descendants(entity) {
            if let Ok((mut box_background, mut border)) = checkbox_boxes.get_mut(child) {
                let next_background = BackgroundColor(theme.colors.secondary_button.idle);
                if *box_background != next_background {
                    *box_background = next_background;
                }
                let next_border = BorderColor::all(selection_indicator_border(&theme, state));
                if *border != next_border {
                    *border = next_border;
                }
            }
            if let Ok(mut visibility) = checkbox_marks.get_mut(child) {
                let next = if selected {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                };
                if *visibility != next {
                    *visibility = next;
                }
            }
            if let Ok((mut track_background, mut border)) = toggle_tracks.get_mut(child) {
                let next_background = BackgroundColor(if selected {
                    theme.colors.primary_button.selected
                } else {
                    theme.colors.secondary_button.idle
                });
                if *track_background != next_background {
                    *track_background = next_background;
                }
                let next_border = BorderColor::all(selection_indicator_border(&theme, state));
                if *border != next_border {
                    *border = next_border;
                }
            }
            if let Ok((mut node, mut thumb_background)) = toggle_thumbs.get_mut(child) {
                let next_left = px(if selected { 20 } else { 2 });
                if node.left != next_left {
                    node.left = next_left;
                }
                let next_background = BackgroundColor(if disabled {
                    theme.colors.text_muted
                } else {
                    theme.colors.text_primary
                });
                if *thumb_background != next_background {
                    *thumb_background = next_background;
                }
            }
            if let Ok(mut visibility) = segment_indicators.get_mut(child) {
                let next = if selected {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                };
                if *visibility != next {
                    *visibility = next;
                }
            }
            if let Ok(mut color) = text_colors.get_mut(child) {
                let next = TextColor(if state == SelectionVisualState::Error {
                    theme.colors.text_error
                } else {
                    selection_button_text_color(&theme, state)
                });
                if *color != next {
                    *color = next;
                }
            }
        }
    }
}

impl Default for UiCheckboxChecked {
    fn default() -> Self {
        Self
    }
}

impl Default for UiToggleOn {
    fn default() -> Self {
        Self
    }
}

impl Default for UiSegmentOptionSelected {
    fn default() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_no_longer_encode_visual_state_with_symbols() {
        assert_eq!(
            selection_display_text("Medium", SelectionVisualState::Selected),
            "Medium"
        );
        assert_eq!(
            selection_display_text("Medium", SelectionVisualState::Idle),
            "Medium"
        );
        assert_eq!(
            selection_display_text("Medium", SelectionVisualState::Disabled),
            "Medium"
        );
    }

    #[test]
    fn selection_indicator_geometry_is_state_invariant() {
        assert_eq!(SELECTION_INDICATOR_SIZE, 22.0);
        assert_eq!(TOGGLE_TRACK_WIDTH, 42.0);
        assert_eq!(TOGGLE_THUMB_SIZE, 18.0);
        assert_eq!(20.0 + TOGGLE_THUMB_SIZE, TOGGLE_TRACK_WIDTH - 4.0);
    }

    #[test]
    fn flags_only_disabled_and_loading_selection_controls_ignore_clicks() {
        let mut app = App::new();
        app.init_resource::<UiCurrentOwner>()
            .add_message::<UiButtonEvent>()
            .add_message::<UiControlEvent>()
            .add_systems(Update, update_selection_control_interactions);
        let checkbox = app
            .world_mut()
            .spawn((
                Button,
                UiCheckbox,
                UiControlMeta::new(
                    UiControlId::new("test.checkbox.disabled"),
                    UiControlKind::Checkbox,
                ),
                UiControlFlags {
                    disabled: true,
                    ..default()
                },
            ))
            .id();
        let toggle = app
            .world_mut()
            .spawn((
                Button,
                UiToggle,
                UiControlMeta::new(
                    UiControlId::new("test.toggle.loading"),
                    UiControlKind::Toggle,
                ),
                UiControlFlags {
                    loading: true,
                    ..default()
                },
            ))
            .id();
        let segmented = app.world_mut().spawn(UiSegmentedControl).id();
        let segment = app
            .world_mut()
            .spawn((
                Button,
                UiSegmentOption {
                    value: "blocked".to_owned(),
                },
                UiControlMeta::new(
                    UiControlId::new("test.segment.disabled"),
                    UiControlKind::Segmented,
                ),
                UiControlFlags {
                    disabled: true,
                    ..default()
                },
            ))
            .id();
        app.world_mut().entity_mut(segmented).add_child(segment);
        for entity in [checkbox, toggle, segment] {
            app.world_mut().write_message(UiButtonEvent {
                entity,
                kind: UiButtonEventKind::Click,
                button: None,
            });
        }

        app.update();

        assert!(!app.world().entity(checkbox).contains::<UiCheckboxChecked>());
        assert!(!app.world().entity(toggle).contains::<UiToggleOn>());
        assert!(
            !app.world()
                .entity(segment)
                .contains::<UiSegmentOptionSelected>()
        );
        for entity in [checkbox, toggle, segment] {
            assert!(!app.world().get::<UiControlFlags>(entity).unwrap().selected);
            assert!(!app.world().entity(entity).contains::<SelectedButton>());
        }
        let messages = app.world().resource::<Messages<UiControlEvent>>();
        let mut cursor = bevy::ecs::message::MessageCursor::default();
        assert_eq!(cursor.read(messages).count(), 0);
    }

    #[test]
    fn selection_visual_sync_is_change_stable_after_first_frame() {
        let mut app = App::new();
        app.insert_resource(UiTheme::default())
            .add_systems(Update, sync_selection_control_visuals);
        let theme = UiTheme::default();
        let fonts = UiFontAssets::test_registry();
        let i18n = UiI18n::test_with_texts(
            "zh_cn",
            &[
                ("test.checkbox", "复选"),
                ("test.toggle", "开关"),
                ("test.segment", "分段"),
            ],
        );
        let checkbox = app
            .world_mut()
            .spawn(checked_checkbox_key(
                &theme,
                &fonts,
                &i18n,
                "test.checkbox",
                "Checkbox",
            ))
            .id();
        let toggle = app
            .world_mut()
            .spawn(toggle_on_key(
                &theme,
                &fonts,
                &i18n,
                "test.toggle",
                "Toggle",
            ))
            .id();
        let segmented = app.world_mut().spawn(segmented_control(&theme)).id();
        let segment = app
            .world_mut()
            .spawn(selected_segment_option_key(
                &theme,
                &fonts,
                &i18n,
                "selected",
                "test.segment",
                "Segment",
            ))
            .id();
        app.world_mut().entity_mut(segmented).add_child(segment);

        app.update();
        let checkbox_box = app
            .world()
            .get::<Children>(checkbox)
            .unwrap()
            .iter()
            .find(|entity| app.world().get::<UiCheckboxBox>(*entity).is_some())
            .unwrap();
        let checkbox_mark = app
            .world()
            .get::<Children>(checkbox_box)
            .unwrap()
            .iter()
            .find(|entity| app.world().get::<UiCheckboxMark>(*entity).is_some())
            .unwrap();
        let track = app
            .world()
            .get::<Children>(toggle)
            .unwrap()
            .iter()
            .find(|entity| app.world().get::<UiToggleTrack>(*entity).is_some())
            .unwrap();
        let thumb = app
            .world()
            .get::<Children>(track)
            .unwrap()
            .iter()
            .find(|entity| app.world().get::<UiToggleThumb>(*entity).is_some())
            .unwrap();
        let indicator = app
            .world()
            .get::<Children>(segment)
            .unwrap()
            .iter()
            .find(|entity| app.world().get::<UiSegmentIndicator>(*entity).is_some())
            .unwrap();

        app.world_mut().clear_trackers();
        app.update();

        for root in [checkbox, toggle, segment] {
            assert!(
                !app.world()
                    .entity(root)
                    .get_ref::<BackgroundColor>()
                    .unwrap()
                    .is_changed()
            );
        }
        assert!(
            !app.world()
                .entity(checkbox_box)
                .get_ref::<BackgroundColor>()
                .unwrap()
                .is_changed()
        );
        assert!(
            !app.world()
                .entity(checkbox_box)
                .get_ref::<BorderColor>()
                .unwrap()
                .is_changed()
        );
        assert!(
            !app.world()
                .entity(checkbox_mark)
                .get_ref::<Visibility>()
                .unwrap()
                .is_changed()
        );
        assert!(
            !app.world()
                .entity(thumb)
                .get_ref::<Node>()
                .unwrap()
                .is_changed()
        );
        assert!(
            !app.world()
                .entity(track)
                .get_ref::<BackgroundColor>()
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
    }
}
