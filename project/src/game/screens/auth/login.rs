use bevy::prelude::*;

use crate::game::{
    navigation::AppUiMode,
    ui::{
        core::{
            UiLayer, UiLayerRoot, UiMetrics, UiPanelKind, UiPanelRoot, UiViewport,
            binding::{UiBindingValues, UiBoundText},
        },
        i18n::UiI18n,
        style::{
            UiFontAssets, UiTheme,
            theme::{
                UiThemeBackgroundRole, UiThemeBorderRole, UiThemePanelNodeRole,
                UiThemeRootNodeRole, UiThemeTextColorRole, UiThemeTextStyleRole,
            },
        },
        widgets::{primary_route_button_key, screen_label, screen_title_key},
    },
    ui_ids::{OWNER_LOGIN, PANEL_LOGIN_PAGE},
};

const LOGIN_SUBTITLE_BINDING_PATH: &str = "auth.login.subtitle";
const LOGIN_SUBTITLE_FALLBACK: &str = "Player Login";

pub(super) fn setup_login_screen(
    mut commands: Commands,
    theme: Res<UiTheme>,
    metrics: Res<UiMetrics>,
    viewport: Res<UiViewport>,
    fonts: Res<UiFontAssets>,
    i18n: Res<UiI18n>,
    mut binding_values: ResMut<UiBindingValues>,
    mut clear_color: ResMut<ClearColor>,
) {
    let theme = theme.into_inner();
    let metrics = metrics.into_inner();
    let fonts = fonts.into_inner();
    let i18n = i18n.into_inner();
    clear_color.0 = theme.colors.screen_background;
    let subtitle = i18n.tr(LOGIN_SUBTITLE_BINDING_PATH, LOGIN_SUBTITLE_FALLBACK);
    binding_values.set_text(LOGIN_SUBTITLE_BINDING_PATH, subtitle.clone());

    commands.spawn((
        DespawnOnExit(AppUiMode::Login),
        UiPanelRoot {
            id: PANEL_LOGIN_PAGE,
            kind: UiPanelKind::Page,
            owner: Some(OWNER_LOGIN),
        },
        UiLayerRoot {
            layer: UiLayer::Page,
        },
        Node {
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: viewport.safe_area_padding(metrics.page_padding),
            ..default()
        },
        BackgroundColor(theme.colors.screen_background),
        UiThemeBackgroundRole::Screen,
        UiThemeRootNodeRole::Screen,
        children![(
            UiThemePanelNodeRole::Standard,
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
                    UiThemeTextStyleRole::TitleLarge,
                ),
                (
                    screen_label(
                        theme,
                        fonts,
                        subtitle,
                        UiThemeTextStyleRole::Subtitle,
                        UiThemeTextColorRole::Muted,
                    ),
                    UiBoundText::with_fallback(
                        LOGIN_SUBTITLE_BINDING_PATH,
                        LOGIN_SUBTITLE_FALLBACK,
                    )
                    .unwrap(),
                ),
                primary_route_button_key(
                    theme,
                    metrics,
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

pub(super) fn sync_login_binding_values(
    i18n: Res<UiI18n>,
    mut binding_values: ResMut<UiBindingValues>,
) {
    if !i18n.is_changed() {
        return;
    }

    binding_values.set_text(
        LOGIN_SUBTITLE_BINDING_PATH,
        i18n.tr(LOGIN_SUBTITLE_BINDING_PATH, LOGIN_SUBTITLE_FALLBACK),
    );
}
