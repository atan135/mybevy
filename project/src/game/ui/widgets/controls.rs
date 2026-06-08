use bevy::prelude::*;

use crate::game::{
    navigation::{AppUiMode, RouteButton},
    ui::style::theme::{ButtonColors, UiTheme},
};

pub(in crate::game) struct UiWidgetsPlugin;

impl Plugin for UiWidgetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_button_visuals);
    }
}

#[derive(Component)]
pub(in crate::game) struct PrimaryButton;

#[derive(Component)]
pub(in crate::game) struct SecondaryButton;

#[derive(Component)]
pub(in crate::game) struct DisabledButton;

#[derive(Component)]
pub(in crate::game) struct FocusedButton;

#[derive(Component)]
pub(in crate::game) struct SelectedButton;

#[derive(Component)]
pub(in crate::game) struct LoadingButton;

pub(in crate::game) fn screen_title(
    theme: &UiTheme,
    text: impl Into<String>,
    font_size: f32,
) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size,
            ..default()
        },
        TextColor(theme.colors.text_primary),
    )
}

pub(in crate::game) fn screen_label(
    text: impl Into<String>,
    font_size: f32,
    color: Color,
) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size,
            ..default()
        },
        TextColor(color),
    )
}

pub(in crate::game) fn primary_route_button(
    theme: &UiTheme,
    text: impl Into<String>,
    target: AppUiMode,
) -> impl Bundle {
    route_button(
        theme,
        text,
        target,
        theme.colors.primary_button,
        PrimaryButton,
    )
}

pub(in crate::game) fn secondary_route_button(
    theme: &UiTheme,
    text: impl Into<String>,
    target: AppUiMode,
) -> impl Bundle {
    route_button(
        theme,
        text,
        target,
        theme.colors.secondary_button,
        SecondaryButton,
    )
}

pub(in crate::game) fn primary_action_button(
    theme: &UiTheme,
    text: impl Into<String>,
) -> impl Bundle {
    action_button(theme, text, theme.colors.primary_button, PrimaryButton)
}

pub(in crate::game) fn secondary_action_button(
    theme: &UiTheme,
    text: impl Into<String>,
) -> impl Bundle {
    action_button(theme, text, theme.colors.secondary_button, SecondaryButton)
}

pub(in crate::game) fn disabled_primary_action_button(
    theme: &UiTheme,
    text: impl Into<String>,
) -> impl Bundle {
    disabled_action_button(theme, text, theme.colors.primary_button, PrimaryButton)
}

pub(in crate::game) fn disabled_secondary_action_button(
    theme: &UiTheme,
    text: impl Into<String>,
) -> impl Bundle {
    disabled_action_button(theme, text, theme.colors.secondary_button, SecondaryButton)
}

pub(in crate::game) fn loading_primary_action_button(
    theme: &UiTheme,
    text: impl Into<String>,
) -> impl Bundle {
    (
        action_button(theme, text, theme.colors.primary_button, PrimaryButton),
        LoadingButton,
    )
}

fn route_button<T: Component>(
    theme: &UiTheme,
    text: impl Into<String>,
    target: AppUiMode,
    colors: ButtonColors,
    marker: T,
) -> impl Bundle {
    (
        Button,
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
                font_size: theme.text.button,
                ..default()
            },
            TextColor(theme.colors.text_primary),
        )],
    )
}

fn action_button<T: Component>(
    theme: &UiTheme,
    text: impl Into<String>,
    colors: ButtonColors,
    marker: T,
) -> impl Bundle {
    (
        Button,
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
                font_size: theme.text.button,
                ..default()
            },
            TextColor(theme.colors.text_primary),
        )],
    )
}

fn disabled_action_button<T: Component>(
    theme: &UiTheme,
    text: impl Into<String>,
    colors: ButtonColors,
    marker: T,
) -> impl Bundle {
    (
        Button,
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
                font_size: theme.text.button,
                ..default()
            },
            TextColor(theme.colors.text_muted),
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
        With<Button>,
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
