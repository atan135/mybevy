use bevy::{
    input::keyboard::{Key, KeyboardInput},
    prelude::*,
};

use crate::game::{
    navigation::{AppUiMode, RouteButton},
    ui::{
        core::{UiFocusSystems, focus::UiFocusState},
        i18n::{UiI18n, UiI18nText},
        style::{
            UiFontAssets,
            theme::{ButtonColors, UiTheme, UiThemeTextColorRole},
        },
        widgets::scroll::UiScrollPlugin,
    },
};

pub(in crate::game) struct UiWidgetsPlugin;

impl Plugin for UiWidgetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(UiScrollPlugin)
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
pub(in crate::game) struct UiTextInput;

#[derive(Clone, Debug, Default, Component)]
pub(in crate::game) struct UiTextInputValue(pub String);

#[derive(Clone, Debug, Default, Component)]
pub(in crate::game) struct UiTextInputPlaceholder(pub String);

#[derive(Component)]
pub(in crate::game) struct UiTextInputText;

#[derive(Clone, Debug, Message)]
pub(in crate::game) struct UiTextInputSubmitted {
    pub entity: Entity,
    pub value: String,
}

pub(in crate::game) fn screen_title(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
    font_size: f32,
) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font: fonts.regular.clone(),
            font_size,
            ..default()
        },
        TextColor(theme.colors.text_primary),
        UiThemeTextColorRole::Primary,
    )
}

pub(in crate::game) fn screen_title_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
    font_size: f32,
) -> impl Bundle {
    (
        screen_title(theme, fonts, i18n.tr(key, fallback), font_size),
        UiI18nText::new(key, fallback),
    )
}

pub(in crate::game) fn screen_label(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    text: impl Into<String>,
    font_size: f32,
    color_role: UiThemeTextColorRole,
) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font: fonts.regular.clone(),
            font_size,
            ..default()
        },
        TextColor(color_role.color(theme)),
        color_role,
    )
}

pub(in crate::game) fn screen_label_key(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
    font_size: f32,
    color_role: UiThemeTextColorRole,
) -> impl Bundle {
    (
        screen_label(theme, fonts, i18n.tr(key, fallback), font_size, color_role),
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

pub(in crate::game) fn text_input(
    theme: &UiTheme,
    fonts: &UiFontAssets,
    placeholder: impl Into<String>,
    value: impl Into<String>,
) -> impl Bundle {
    let value = value.into();
    let placeholder = placeholder.into();
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
        UiTextInputPlaceholder(placeholder),
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
        BackgroundColor(text_input_background_color(theme, Interaction::None, false)),
        BorderColor::all(text_input_border_color(theme, Interaction::None, false)),
        children![(
            Text::new(display_text),
            TextFont {
                font: fonts.regular.clone(),
                font_size: theme.text.button,
                ..default()
            },
            TextColor(display_color),
            UiTextInputText,
        )],
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
            i18n_text,
        )],
    )
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

        *background = if is_disabled {
            colors.disabled.into()
        } else if is_loading {
            colors.loading.into()
        } else {
            match *interaction {
                Interaction::Pressed => colors.pressed.into(),
                Interaction::Hovered => colors.hovered.into(),
                Interaction::None if is_selected => colors.selected.into(),
                Interaction::None if is_focused => colors.focused.into(),
                Interaction::None => colors.idle.into(),
            }
        };
    }
}

fn handle_text_input_keyboard(
    mut keyboard_inputs: MessageReader<KeyboardInput>,
    focus_state: Res<UiFocusState>,
    mut text_inputs: Query<&mut UiTextInputValue, With<UiTextInput>>,
    mut submissions: MessageWriter<UiTextInputSubmitted>,
) {
    let Some(focused_entity) = focus_state.focused_entity else {
        for _ in keyboard_inputs.read() {}
        return;
    };

    let Ok(mut value) = text_inputs.get_mut(focused_entity) else {
        for _ in keyboard_inputs.read() {}
        return;
    };

    for keyboard_input in keyboard_inputs.read() {
        if !keyboard_input.state.is_pressed() {
            continue;
        }

        match (&keyboard_input.logical_key, &keyboard_input.text) {
            (Key::Enter, _) => {
                submissions.write(UiTextInputSubmitted {
                    entity: focused_entity,
                    value: value.0.clone(),
                });
            }
            (Key::Backspace, _) => {
                value.0.pop();
            }
            (_, Some(inserted_text)) if inserted_text.chars().all(is_printable_char) => {
                value.0.push_str(inserted_text);
            }
            _ => {}
        }
    }
}

fn sync_text_input_display(
    theme: Res<UiTheme>,
    parents: Query<&ChildOf>,
    text_inputs: Query<(&UiTextInputValue, &UiTextInputPlaceholder), With<UiTextInput>>,
    mut texts: Query<(Entity, &mut Text, &mut TextColor), With<UiTextInputText>>,
) {
    for (text_entity, mut text, mut text_color) in &mut texts {
        let Some(input_entity) = parents
            .iter_ancestors(text_entity)
            .find(|ancestor| text_inputs.get(*ancestor).is_ok())
        else {
            continue;
        };

        let Ok((value, placeholder)) = text_inputs.get(input_entity) else {
            continue;
        };

        let (display, color) = if value.0.is_empty() {
            (placeholder.0.as_str(), theme.colors.text_muted)
        } else {
            (value.0.as_str(), theme.colors.text_primary)
        };

        if text.0 != display {
            text.0 = display.to_string();
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
        ),
        (With<Button>, With<UiTextInput>),
    >,
) {
    for (interaction, mut background, mut border, is_focused) in &mut text_inputs {
        let background_color = text_input_background_color(&theme, *interaction, is_focused);
        if background.0 != background_color {
            *background = BackgroundColor(background_color);
        }

        *border = BorderColor::all(text_input_border_color(&theme, *interaction, is_focused));
    }
}

fn text_input_background_color(
    theme: &UiTheme,
    interaction: Interaction,
    is_focused: bool,
) -> Color {
    match interaction {
        Interaction::Pressed => theme.colors.secondary_button.pressed,
        Interaction::Hovered => theme.colors.secondary_button.hovered,
        Interaction::None if is_focused => theme.colors.secondary_button.focused,
        Interaction::None => theme.colors.secondary_button.idle,
    }
}

fn text_input_border_color(theme: &UiTheme, interaction: Interaction, is_focused: bool) -> Color {
    match interaction {
        Interaction::Pressed => theme.colors.primary_button.pressed,
        Interaction::Hovered if is_focused => theme.colors.primary_button.focused,
        Interaction::Hovered => theme.colors.secondary_button.focused,
        Interaction::None if is_focused => theme.colors.primary_button.focused,
        Interaction::None => theme.colors.panel_border,
    }
}

fn is_printable_char(chr: char) -> bool {
    let is_in_private_use_area = ('\u{e000}'..='\u{f8ff}').contains(&chr)
        || ('\u{f0000}'..='\u{ffffd}').contains(&chr)
        || ('\u{100000}'..='\u{10fffd}').contains(&chr);

    !is_in_private_use_area && !chr.is_ascii_control()
}
