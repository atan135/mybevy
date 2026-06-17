use bevy::prelude::*;

use crate::game::{
    navigation::{AppUiMode, secondary_route_button_key},
    ui::{
        core::{UiLayer, UiLayerRoot, UiMetrics, UiPanelKind, UiPanelRoot, UiViewport},
        i18n::UiI18n,
        style::{UiFontAssets, UiTheme, theme::UiThemeRootNodeRole},
    },
    ui_ids::{OWNER_TOUCH_RIPPLE, PANEL_TOUCH_RIPPLE_HUD},
};

pub(super) fn setup_touch_ripple_overlay(
    mut commands: Commands,
    theme: Res<UiTheme>,
    metrics: Res<UiMetrics>,
    viewport: Res<UiViewport>,
    fonts: Res<UiFontAssets>,
    i18n: Res<UiI18n>,
) {
    let theme = theme.into_inner();
    let metrics = metrics.into_inner();
    let fonts = fonts.into_inner();
    let i18n = i18n.into_inner();

    commands.spawn((
        DespawnOnExit(AppUiMode::WanfaTouchRipple),
        UiPanelRoot {
            id: PANEL_TOUCH_RIPPLE_HUD,
            kind: UiPanelKind::Hud,
            owner: Some(OWNER_TOUCH_RIPPLE),
        },
        UiLayerRoot {
            layer: UiLayer::Page,
        },
        Node {
            width: percent(100),
            height: percent(100),
            padding: viewport.safe_area_padding(metrics.page_padding),
            align_items: AlignItems::FlexStart,
            justify_content: JustifyContent::FlexEnd,
            ..default()
        },
        UiThemeRootNodeRole::Overlay,
        children![secondary_route_button_key(
            theme,
            metrics,
            fonts,
            i18n,
            "nav.lobby",
            "Lobby",
            AppUiMode::Lobby
        )],
    ));
}
