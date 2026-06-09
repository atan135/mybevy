use bevy::prelude::*;

use crate::game::{
    navigation::AppUiMode,
    ui::{
        core::{UiLayer, UiLayerRoot, UiPanelId, UiPanelKind, UiPanelRoot},
        i18n::UiI18n,
        style::{UiFontAssets, UiTheme},
        widgets::secondary_route_button_key,
    },
};

pub(super) fn setup_touch_ripple_overlay(
    mut commands: Commands,
    theme: Res<UiTheme>,
    fonts: Res<UiFontAssets>,
    i18n: Res<UiI18n>,
) {
    let theme = theme.into_inner();
    let fonts = fonts.into_inner();
    let i18n = i18n.into_inner();

    commands.spawn((
        DespawnOnExit(AppUiMode::WanfaTouchRipple),
        UiPanelRoot {
            id: UiPanelId::TouchRippleHud,
            kind: UiPanelKind::Hud,
            owner_mode: Some(AppUiMode::WanfaTouchRipple),
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
        children![secondary_route_button_key(
            theme,
            fonts,
            i18n,
            "nav.lobby",
            "Lobby",
            AppUiMode::Lobby
        )],
    ));
}
