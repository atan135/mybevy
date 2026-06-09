use bevy::prelude::*;

use crate::game::{
    navigation::AppUiMode,
    ui::{
        core::{UiLayer, UiLayerRoot, UiPanelId, UiPanelKind, UiPanelRoot},
        i18n::UiI18n,
        style::{
            UiFontAssets, UiTheme,
            theme::{UiThemeBackgroundRole, UiThemeBorderRole, UiThemeTextColorRole},
        },
        widgets::{primary_route_button_key, screen_label_key, screen_title_key},
    },
};

pub(super) fn setup_login_screen(
    mut commands: Commands,
    theme: Res<UiTheme>,
    fonts: Res<UiFontAssets>,
    i18n: Res<UiI18n>,
    mut clear_color: ResMut<ClearColor>,
) {
    let theme = theme.into_inner();
    let fonts = fonts.into_inner();
    let i18n = i18n.into_inner();
    clear_color.0 = theme.colors.screen_background;

    commands.spawn((
        DespawnOnExit(AppUiMode::Login),
        UiPanelRoot {
            id: UiPanelId::LoginPage,
            kind: UiPanelKind::Page,
            owner_mode: Some(AppUiMode::Login),
        },
        UiLayerRoot {
            layer: UiLayer::Page,
        },
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::all(px(theme.layout.screen_padding)),
            ..default()
        },
        BackgroundColor(theme.colors.screen_background),
        UiThemeBackgroundRole::Screen,
        children![(
            Node {
                width: percent(100),
                max_width: px(theme.layout.auth_panel_width),
                flex_direction: FlexDirection::Column,
                row_gap: px(theme.layout.panel_gap),
                padding: UiRect::all(px(theme.panel.padding)),
                border: UiRect::all(px(theme.panel.border)),
                border_radius: BorderRadius::all(px(theme.panel.radius)),
                ..default()
            },
            BackgroundColor(theme.colors.panel_background),
            BorderColor::all(theme.colors.panel_border),
            UiThemeBackgroundRole::Panel,
            UiThemeBorderRole::Panel,
            children![
                screen_title_key(
                    theme,
                    fonts,
                    i18n,
                    "app.name",
                    "MyBevy",
                    theme.text.title_large,
                ),
                screen_label_key(
                    theme,
                    fonts,
                    i18n,
                    "auth.login.subtitle",
                    "Player Login",
                    theme.text.subtitle,
                    UiThemeTextColorRole::Muted,
                ),
                primary_route_button_key(
                    theme,
                    fonts,
                    i18n,
                    "auth.login.guest_login",
                    "Guest Login",
                    AppUiMode::Lobby,
                ),
            ],
        )],
    ));
}
