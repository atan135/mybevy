use bevy::prelude::*;

use crate::game::{
    navigation::AppUiMode,
    ui::{
        core::{UiLayer, UiLayerRoot, UiPanelId, UiPanelKind, UiPanelRoot},
        style::UiTheme,
        widgets::{primary_route_button, screen_label, screen_title},
    },
};

pub(super) fn setup_login_screen(
    mut commands: Commands,
    theme: Res<UiTheme>,
    mut clear_color: ResMut<ClearColor>,
) {
    let theme = theme.into_inner();
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
            children![
                screen_title(theme, "MyBevy", theme.text.title_large),
                screen_label("Player Login", theme.text.subtitle, theme.colors.text_muted),
                primary_route_button(theme, "Guest Login", AppUiMode::Lobby),
            ],
        )],
    ));
}
