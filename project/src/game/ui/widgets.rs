use bevy::prelude::*;

use crate::game::{
    navigation::{AppScreen, RouteButton},
    ui::theme::{
        PRIMARY_BUTTON, PRIMARY_BUTTON_HOVERED, PRIMARY_BUTTON_PRESSED, SECONDARY_BUTTON,
        SECONDARY_BUTTON_HOVERED, SECONDARY_BUTTON_PRESSED, TEXT_PRIMARY,
    },
};

pub(in crate::game) struct UiWidgetsPlugin;

impl Plugin for UiWidgetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_button_visuals);
    }
}

#[derive(Component)]
struct PrimaryButton;

#[derive(Component)]
struct SecondaryButton;

pub(in crate::game) fn screen_title(text: impl Into<String>, font_size: f32) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size,
            ..default()
        },
        TextColor(TEXT_PRIMARY),
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
    text: impl Into<String>,
    target: AppScreen,
) -> impl Bundle {
    route_button(text, target, PRIMARY_BUTTON, PrimaryButton)
}

pub(in crate::game) fn secondary_route_button(
    text: impl Into<String>,
    target: AppScreen,
) -> impl Bundle {
    route_button(text, target, SECONDARY_BUTTON, SecondaryButton)
}

fn route_button<T: Component>(
    text: impl Into<String>,
    target: AppScreen,
    background: Color,
    marker: T,
) -> impl Bundle {
    (
        Button,
        RouteButton { target },
        marker,
        Node {
            min_width: px(112),
            height: px(46),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::axes(px(18), px(0)),
            border_radius: BorderRadius::all(px(6)),
            ..default()
        },
        BackgroundColor(background),
        children![(
            Text::new(text),
            TextFont {
                font_size: 18.0,
                ..default()
            },
            TextColor(TEXT_PRIMARY),
        )],
    )
}

fn update_button_visuals(
    mut buttons: Query<
        (
            &Interaction,
            &mut BackgroundColor,
            Has<PrimaryButton>,
            Has<SecondaryButton>,
        ),
        (Changed<Interaction>, With<Button>),
    >,
) {
    for (interaction, mut background, is_primary, is_secondary) in &mut buttons {
        if !is_primary && !is_secondary {
            continue;
        }

        *background = match (*interaction, is_primary) {
            (Interaction::Pressed, true) => PRIMARY_BUTTON_PRESSED.into(),
            (Interaction::Hovered, true) => PRIMARY_BUTTON_HOVERED.into(),
            (Interaction::None, true) => PRIMARY_BUTTON.into(),
            (Interaction::Pressed, false) => SECONDARY_BUTTON_PRESSED.into(),
            (Interaction::Hovered, false) => SECONDARY_BUTTON_HOVERED.into(),
            (Interaction::None, false) => SECONDARY_BUTTON.into(),
        };
    }
}
