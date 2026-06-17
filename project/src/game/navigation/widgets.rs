use bevy::prelude::*;

use crate::framework::ui::{
    core::{UiMetrics, UiOwnerId, UiPanelId, UiPanelKind, UiPanelRoot},
    i18n::UiI18n,
    style::{UiFontAssets, theme::UiTheme},
    widgets::{
        primary_action_button, primary_action_button_key, secondary_action_button,
        secondary_action_button_key,
    },
};
use crate::game::navigation::{AppUiMode, RouteButton};

pub(in crate::game) fn game_panel_root(
    id: UiPanelId,
    kind: UiPanelKind,
    owner: UiOwnerId,
) -> UiPanelRoot {
    UiPanelRoot {
        id,
        kind,
        owner: Some(owner),
    }
}

#[allow(dead_code)]
pub(in crate::game) fn primary_route_button(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    text: impl Into<String>,
    target: AppUiMode,
) -> impl Bundle {
    (
        primary_action_button(theme, metrics, fonts, text),
        RouteButton { target },
    )
}

pub(in crate::game) fn primary_route_button_key(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
    target: AppUiMode,
) -> impl Bundle {
    (
        primary_action_button_key(theme, metrics, fonts, i18n, key, fallback),
        RouteButton { target },
    )
}

#[allow(dead_code)]
pub(in crate::game) fn secondary_route_button(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    text: impl Into<String>,
    target: AppUiMode,
) -> impl Bundle {
    (
        secondary_action_button(theme, metrics, fonts, text),
        RouteButton { target },
    )
}

pub(in crate::game) fn secondary_route_button_key(
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
    target: AppUiMode,
) -> impl Bundle {
    (
        secondary_action_button_key(theme, metrics, fonts, i18n, key, fallback),
        RouteButton { target },
    )
}
