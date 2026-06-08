use bevy::prelude::*;

use crate::game::{
    navigation::AppUiMode,
    ui::{
        core::{UiLayer, UiLayerRoot, UiScreenId, UiScreenRoot},
        style::UiTheme,
        widgets::secondary_route_button,
    },
};

pub(super) fn setup_touch_ripple_overlay(mut commands: Commands, theme: Res<UiTheme>) {
    let theme = theme.into_inner();

    commands.spawn((
        DespawnOnExit(AppUiMode::WanfaTouchRipple),
        UiScreenRoot {
            id: UiScreenId::TouchRippleHud,
        },
        UiLayerRoot {
            layer: UiLayer::Page,
        },
        Node {
            width: percent(100),
            height: percent(100),
            padding: UiRect::all(px(theme.layout.overlay_padding)),
            align_items: AlignItems::FlexStart,
            justify_content: JustifyContent::FlexEnd,
            ..default()
        },
        children![secondary_route_button(theme, "Lobby", AppUiMode::Lobby)],
    ));
}
