use bevy::{picking::Pickable, prelude::*};

use crate::framework::ui::{
    core::{
        UiLayer, UiLayerRoot, UiMetrics, UiOrientation, UiPanelKind, UiViewport,
        binding::{UiBindingValues, UiBoundText},
        focus::UiFocusState,
    },
    i18n::UiI18n,
    style::{
        UiButtonStyleRole, UiFontAssets, UiInputStyleRole, UiStyleBinding, UiStyleScope,
        UiTextStyleRole, UiTheme,
        theme::{
            UiThemeBackgroundRole, UiThemeBorderRole, UiThemePanelNodeRole, UiThemeRootNodeRole,
            UiThemeTextColorRole, UiThemeTextStyleRole,
        },
    },
    widgets::{
        DisabledButton, LoadingButton, disabled_primary_action_button_key,
        disabled_secondary_action_button_key, primary_action_button_key, screen_label,
        screen_label_key, screen_title_key, secondary_action_button_key, segment_option_key,
        segmented_control, selected_segment_option_key, text_input,
    },
};
use crate::game::{
    myserver::{
        AccountLoginState, CharacterSelectionState, MyServerEnvironment, MyServerProfiles,
        MyServerSession,
    },
    navigation::{AppUiMode, GameRouteCommand, game_panel_root, secondary_route_button_key},
    ui_ids::{OWNER_CHARACTER_SELECT, OWNER_LOGIN, PANEL_CHARACTER_SELECT, PANEL_LOGIN},
};

const LOGIN_SUBTITLE_BINDING_PATH: &str = "auth.login.subtitle";
const LOGIN_SUBTITLE_FALLBACK: &str = "Account Login";
const CHARACTER_SUBTITLE_BINDING_PATH: &str = "auth.character.subtitle";
const CHARACTER_SUBTITLE_FALLBACK: &str = "Character Select";
const DEFAULT_CHARACTER_NAME: &str = "";
const LOGIN_BACKGROUND_PATH: &str = "ui/images/login_stillwater_background.png";
const LOGIN_VISUAL_STYLE_VARIANT: &str = "login.stillwater";
const LOGIN_REFERENCE_WIDTH: f32 = 1376.0;
pub(super) const LOGIN_REFERENCE_PANEL_WIDTH: f32 = 344.0;
const LOGIN_REFERENCE_PANEL_HEIGHT: f32 = 496.0;

use super::{host::*, model::*};

pub(super) fn setup_login_screen(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    theme: Res<UiTheme>,
    metrics: Res<UiMetrics>,
    viewport: Res<UiViewport>,
    fonts: Res<UiFontAssets>,
    i18n: Res<UiI18n>,
    mut binding_values: ResMut<UiBindingValues>,
    session: Res<MyServerSession>,
    profiles: Res<MyServerProfiles>,
    mut clear_color: ResMut<ClearColor>,
) {
    let theme = theme.into_inner();
    let metrics = metrics.into_inner();
    let viewport = viewport.into_inner();
    let fonts = fonts.into_inner();
    let i18n = i18n.into_inner();
    clear_color.0 = Color::srgb_u8(4, 15, 18);
    let subtitle = i18n.tr(LOGIN_SUBTITLE_BINDING_PATH, LOGIN_SUBTITLE_FALLBACK);
    let background = asset_server.load(LOGIN_BACKGROUND_PATH);
    let panel_size = login_visual_panel_size(viewport, metrics);
    let panel_padding = login_visual_panel_padding(panel_size);
    binding_values.set_text(LOGIN_SUBTITLE_BINDING_PATH, subtitle.clone());
    commands
        .spawn((
            DespawnOnExit(AppUiMode::Login),
            game_panel_root(PANEL_LOGIN, UiPanelKind::Page, OWNER_LOGIN),
            UiLayerRoot {
                layer: UiLayer::Page,
            },
            Node {
                width: percent(100),
                height: percent(100),
                position_type: PositionType::Relative,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: viewport.safe_area_padding(metrics.page_padding),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgb_u8(4, 15, 18)),
            UiThemeRootNodeRole::Screen,
        ))
        .with_children(|root| {
            root.spawn((
                Name::new("Stillwater login background"),
                Node {
                    width: percent(100),
                    height: percent(100),
                    position_type: PositionType::Absolute,
                    ..default()
                },
                ImageNode::new(background).with_mode(NodeImageMode::Stretch),
                Pickable::IGNORE,
                ZIndex(0),
            ));
            root.spawn((
                Name::new("Stillwater account login"),
                UiStyleScope::new(LOGIN_VISUAL_STYLE_VARIANT),
                Node {
                    width: px(panel_size.x),
                    height: px(panel_size.y),
                    max_width: percent(100),
                    max_height: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(login_visual_panel_gap(viewport)),
                    padding: UiRect::all(px(panel_padding)),
                    border: UiRect::vertical(px(1)),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.01, 0.06, 0.08, 0.80)),
                BorderColor::all(Color::srgba(0.47, 0.79, 0.78, 0.30)),
                ZIndex(10),
            ))
            .with_children(|panel| {
                panel.spawn((
                    screen_title_key(
                        theme,
                        fonts,
                        i18n,
                        "app.name",
                        "MyBevy",
                        UiThemeTextStyleRole::TitleLarge,
                    ),
                    UiStyleBinding::new().with_text(UiTextStyleRole::Primary),
                ));
                panel.spawn((
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
                    UiStyleBinding::new().with_text(UiTextStyleRole::Muted),
                ));
                panel.spawn((
                    Name::new("Stillwater login header rule"),
                    Node {
                        width: percent(100),
                        height: px(1),
                        margin: UiRect::vertical(px(4)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.40, 0.74, 0.73, 0.38)),
                ));
                spawn_server_environment_section(panel, theme, viewport, fonts, i18n, &profiles);
                spawn_auth_form_section(panel, theme, metrics, viewport, fonts, i18n, &session);
                panel.spawn((
                    AuthDynamicRoot,
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(theme.layout.panel_gap),
                        ..default()
                    },
                ));
            });
        });
}

fn spawn_server_environment_section(
    parent: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    viewport: &UiViewport,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    profiles: &MyServerProfiles,
) {
    let compact_row = uses_landscape_login_control_grid(viewport);
    parent
        .spawn((Node {
            width: percent(100),
            flex_direction: if compact_row {
                FlexDirection::Row
            } else {
                FlexDirection::Column
            },
            align_items: if compact_row {
                AlignItems::Center
            } else {
                AlignItems::Stretch
            },
            column_gap: px(theme.layout.row_column_gap),
            row_gap: px(theme.layout.row_gap),
            ..default()
        },))
        .with_children(|parent| {
            parent.spawn((
                screen_label_key(
                    theme,
                    fonts,
                    i18n,
                    "auth.login.server_section",
                    "Server",
                    UiThemeTextStyleRole::SectionLabel,
                    UiThemeTextColorRole::Muted,
                ),
                UiStyleBinding::new().with_text(UiTextStyleRole::Muted),
                Node {
                    width: if compact_row { px(72) } else { auto() },
                    flex_shrink: 0.0,
                    ..default()
                },
            ));
            if compact_row {
                parent
                    .spawn((Node {
                        flex_grow: 1.0,
                        min_width: px(0),
                        ..default()
                    },))
                    .with_children(|container| {
                        spawn_server_environment_control(container, theme, fonts, i18n, profiles);
                    });
            } else {
                spawn_server_environment_control(parent, theme, fonts, i18n, profiles);
            }
        });
}

fn spawn_server_environment_control(
    parent: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    profiles: &MyServerProfiles,
) {
    parent
        .spawn(segmented_control(theme))
        .with_children(|segments| {
            if profiles.selected() == MyServerEnvironment::Local {
                segments.spawn((
                    selected_segment_option_key(
                        theme,
                        fonts,
                        i18n,
                        "local",
                        "auth.login.server.local",
                        "Local",
                    ),
                    ServerEnvironmentButton(MyServerEnvironment::Local),
                ));
            } else {
                segments.spawn((
                    segment_option_key(
                        theme,
                        fonts,
                        i18n,
                        "local",
                        "auth.login.server.local",
                        "Local",
                    ),
                    ServerEnvironmentButton(MyServerEnvironment::Local),
                ));
            }

            if profiles.selected() == MyServerEnvironment::Production {
                segments.spawn((
                    selected_segment_option_key(
                        theme,
                        fonts,
                        i18n,
                        "production",
                        "auth.login.server.production",
                        "Production",
                    ),
                    ServerEnvironmentButton(MyServerEnvironment::Production),
                ));
            } else {
                segments.spawn((
                    segment_option_key(
                        theme,
                        fonts,
                        i18n,
                        "production",
                        "auth.login.server.production",
                        "Production",
                    ),
                    ServerEnvironmentButton(MyServerEnvironment::Production),
                ));
            }
        });
}

pub(super) fn setup_character_select_screen(
    mut commands: Commands,
    theme: Res<UiTheme>,
    metrics: Res<UiMetrics>,
    viewport: Res<UiViewport>,
    fonts: Res<UiFontAssets>,
    i18n: Res<UiI18n>,
    mut binding_values: ResMut<UiBindingValues>,
    mut route_commands: MessageWriter<GameRouteCommand>,
    session: Res<MyServerSession>,
    mut clear_color: ResMut<ClearColor>,
) {
    if session.account_login_state != AccountLoginState::LoggedIn {
        route_commands.write(GameRouteCommand::ChangeMode(AppUiMode::Login));
        return;
    }

    let theme = theme.into_inner();
    let metrics = metrics.into_inner();
    let viewport = viewport.into_inner();
    let fonts = fonts.into_inner();
    let i18n = i18n.into_inner();
    clear_color.0 = theme.colors.screen_background;
    let subtitle = i18n.tr(CHARACTER_SUBTITLE_BINDING_PATH, CHARACTER_SUBTITLE_FALLBACK);
    let panel_width = auth_panel_width(viewport, theme, metrics);
    let panel_gap = auth_panel_gap(viewport, theme);
    binding_values.set_text(CHARACTER_SUBTITLE_BINDING_PATH, subtitle.clone());

    commands
        .spawn((
            DespawnOnExit(AppUiMode::CharacterSelect),
            game_panel_root(
                PANEL_CHARACTER_SELECT,
                UiPanelKind::Page,
                OWNER_CHARACTER_SELECT,
            ),
            UiLayerRoot {
                layer: UiLayer::Page,
            },
            Node {
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                padding: viewport.safe_area_padding(metrics.page_padding),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(theme.colors.screen_background),
            UiThemeBackgroundRole::Screen,
            UiThemeRootNodeRole::Screen,
        ))
        .with_children(|root| {
            root.spawn((
                UiThemePanelNodeRole::Standard,
                Node {
                    width: percent(100),
                    max_width: px(panel_width),
                    align_self: AlignSelf::FlexStart,
                    flex_direction: FlexDirection::Column,
                    row_gap: px(panel_gap),
                    padding: UiRect::all(px(auth_panel_padding(viewport, theme, metrics))),
                    border: UiRect::all(px(theme.panel.border)),
                    border_radius: BorderRadius::all(px(theme.panel.radius)),
                    ..default()
                },
                BackgroundColor(theme.colors.panel_background),
                BorderColor::all(theme.colors.panel_border),
                UiThemeBackgroundRole::Panel,
                UiThemeBorderRole::Panel,
            ))
            .with_children(|panel| {
                panel.spawn(screen_title_key(
                    theme,
                    fonts,
                    i18n,
                    "auth.character.title",
                    "Choose Character",
                    UiThemeTextStyleRole::TitleLarge,
                ));
                panel.spawn((
                    screen_label(
                        theme,
                        fonts,
                        subtitle,
                        UiThemeTextStyleRole::Subtitle,
                        UiThemeTextColorRole::Muted,
                    ),
                    UiBoundText::with_fallback(
                        CHARACTER_SUBTITLE_BINDING_PATH,
                        CHARACTER_SUBTITLE_FALLBACK,
                    )
                    .unwrap(),
                ));
                panel.spawn((
                    AuthDynamicRoot,
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(theme.layout.panel_gap),
                        ..default()
                    },
                ));
            });
        });
}

pub(super) fn cleanup_login_screen_state(
    mut ui_state: ResMut<LoginUiState>,
    mut focus_state: ResMut<UiFocusState>,
) {
    ui_state.clear_runtime_state();
    focus_state.focused_entity = None;
}

fn spawn_auth_form_section(
    parent: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    metrics: &UiMetrics,
    viewport: &UiViewport,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    session: &MyServerSession,
) {
    let login_pending = login_request_pending(session);
    let logged_in = session.account_login_state == AccountLoginState::LoggedIn;
    let use_landscape_control_grid = uses_landscape_login_control_grid(viewport);

    parent
        .spawn((Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: px(theme.layout.row_gap),
            ..default()
        },))
        .with_children(|parent| {
            parent.spawn((
                screen_label_key(
                    theme,
                    fonts,
                    i18n,
                    "auth.login.account_section",
                    "Account",
                    UiThemeTextStyleRole::SectionLabel,
                    UiThemeTextColorRole::Muted,
                ),
                UiStyleBinding::new().with_text(UiTextStyleRole::Muted),
            ));
            if use_landscape_control_grid {
                parent
                    .spawn((login_control_row(theme),))
                    .with_children(|parent| {
                        spawn_login_input_cell(
                            parent,
                            theme,
                            metrics,
                            fonts,
                            i18n.tr("auth.login.account_placeholder", "Account"),
                            session.login_name.clone().unwrap_or_default(),
                            LoginNameInput,
                        );
                        spawn_login_input_cell(
                            parent,
                            theme,
                            metrics,
                            fonts,
                            i18n.tr("auth.login.password_placeholder", "Password"),
                            "",
                            PasswordInput,
                        );
                    });
                parent
                    .spawn((login_control_row(theme),))
                    .with_children(|parent| {
                        spawn_login_button_cell(parent, |button| {
                            spawn_primary_button(
                                button,
                                theme,
                                metrics,
                                fonts,
                                i18n,
                                "auth.login.sign_in",
                                "Login",
                                login_pending || logged_in,
                                (
                                    AccountLoginButton,
                                    UiStyleBinding::new().with_button(UiButtonStyleRole::Primary),
                                ),
                            );
                        });
                        spawn_login_button_cell(parent, |button| {
                            spawn_secondary_button(
                                button,
                                theme,
                                metrics,
                                fonts,
                                i18n,
                                "auth.login.guest_login",
                                "Guest Login",
                                login_pending || logged_in,
                                (
                                    GuestLoginButton,
                                    UiStyleBinding::new().with_button(UiButtonStyleRole::Secondary),
                                ),
                            );
                        });
                    });
            } else {
                parent.spawn((
                    text_input(
                        theme,
                        metrics,
                        fonts,
                        i18n.tr("auth.login.account_placeholder", "Account"),
                        session.login_name.clone().unwrap_or_default(),
                    ),
                    LoginNameInput,
                    UiStyleBinding::new().with_input(UiInputStyleRole::Standard),
                ));
                parent.spawn((
                    text_input(
                        theme,
                        metrics,
                        fonts,
                        i18n.tr("auth.login.password_placeholder", "Password"),
                        "",
                    ),
                    PasswordInput,
                    UiStyleBinding::new().with_input(UiInputStyleRole::Standard),
                ));
                parent
                    .spawn((Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Column,
                        column_gap: px(theme.layout.row_column_gap),
                        row_gap: px(theme.layout.row_gap),
                        align_items: AlignItems::Stretch,
                        ..default()
                    },))
                    .with_children(|parent| {
                        spawn_primary_button(
                            parent,
                            theme,
                            metrics,
                            fonts,
                            i18n,
                            "auth.login.sign_in",
                            "Login",
                            login_pending || logged_in,
                            (
                                AccountLoginButton,
                                UiStyleBinding::new().with_button(UiButtonStyleRole::Primary),
                            ),
                        );
                        spawn_secondary_button(
                            parent,
                            theme,
                            metrics,
                            fonts,
                            i18n,
                            "auth.login.guest_login",
                            "Guest Login",
                            login_pending || logged_in,
                            (
                                GuestLoginButton,
                                UiStyleBinding::new().with_button(UiButtonStyleRole::Secondary),
                            ),
                        );
                    });
            }
        });
}

pub(super) fn uses_landscape_login_control_grid(viewport: &UiViewport) -> bool {
    #[cfg(target_os = "android")]
    {
        viewport.orientation == UiOrientation::Landscape
    }

    #[cfg(not(target_os = "android"))]
    {
        viewport.orientation == UiOrientation::Landscape && viewport.device_scale >= 2.0
    }
}

fn login_control_row(theme: &UiTheme) -> Node {
    Node {
        width: percent(100),
        flex_direction: FlexDirection::Row,
        column_gap: px(theme.layout.row_column_gap),
        align_items: AlignItems::Stretch,
        ..default()
    }
}

pub(super) fn login_control_cell() -> Node {
    Node {
        flex_grow: 1.0,
        flex_basis: px(0),
        min_width: px(0),
        ..default()
    }
}

fn spawn_login_input_cell<T: Bundle>(
    parent: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    placeholder: impl Into<String>,
    value: impl Into<String>,
    marker: T,
) {
    parent.spawn((login_control_cell(),)).with_children(|cell| {
        cell.spawn((
            text_input(theme, metrics, fonts, placeholder, value),
            marker,
            UiStyleBinding::new().with_input(UiInputStyleRole::Standard),
        ));
    });
}

fn spawn_login_button_cell(
    parent: &mut ChildSpawnerCommands,
    spawn_button: impl FnOnce(&mut ChildSpawnerCommands),
) {
    parent
        .spawn((login_control_cell(),))
        .with_children(spawn_button);
}

fn auth_panel_width(viewport: &UiViewport, theme: &UiTheme, metrics: &UiMetrics) -> f32 {
    if viewport.orientation == UiOrientation::Landscape {
        theme.layout.content_width.min(metrics.content_max_width)
    } else {
        theme.layout.auth_panel_width
    }
}

fn auth_panel_gap(viewport: &UiViewport, theme: &UiTheme) -> f32 {
    if viewport.orientation == UiOrientation::Landscape && viewport.logical_height < 600.0 {
        theme.layout.row_gap
    } else {
        theme.layout.panel_gap
    }
}

fn auth_panel_padding(viewport: &UiViewport, theme: &UiTheme, metrics: &UiMetrics) -> f32 {
    if viewport.orientation == UiOrientation::Landscape && viewport.logical_height < 600.0 {
        metrics.panel_padding
    } else {
        theme.panel.padding
    }
}

pub(super) fn login_visual_panel_size(viewport: &UiViewport, metrics: &UiMetrics) -> Vec2 {
    let available_width = (viewport.logical_width - metrics.page_padding * 2.0).max(1.0);
    let available_height = (viewport.logical_height - metrics.page_padding * 2.0).max(1.0);
    let uses_reference_geometry = viewport.logical_width >= LOGIN_REFERENCE_WIDTH * 0.8;

    if uses_reference_geometry {
        return Vec2::new(
            LOGIN_REFERENCE_PANEL_WIDTH.min(available_width),
            LOGIN_REFERENCE_PANEL_HEIGHT.min(available_height),
        );
    }

    let width_ratio = if viewport.orientation == UiOrientation::Landscape {
        0.52
    } else {
        0.88
    };
    Vec2::new(
        (viewport.logical_width * width_ratio)
            .clamp(280.0, 420.0)
            .min(available_width),
        (viewport.logical_height * 0.82)
            .clamp(320.0, LOGIN_REFERENCE_PANEL_HEIGHT)
            .min(available_height),
    )
}

fn login_visual_panel_padding(panel_size: Vec2) -> f32 {
    (panel_size.x * 0.085)
        .min(panel_size.y * 0.06)
        .clamp(18.0, 30.0)
}

fn login_visual_panel_gap(viewport: &UiViewport) -> f32 {
    if viewport.logical_height < 540.0 {
        10.0
    } else {
        16.0
    }
}

fn spawn_dynamic_auth_children(
    parent: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    fonts: &UiFontAssets,
    snapshot: &LoginUiSnapshot,
) {
    spawn_status_notice(parent, theme, fonts, snapshot);
}

fn spawn_dynamic_character_select_children(
    parent: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    snapshot: &LoginUiSnapshot,
) {
    spawn_status_notice(parent, theme, fonts, snapshot);
    spawn_session_summary_row(parent, theme, metrics, fonts, i18n, snapshot);
    spawn_character_section(parent, theme, metrics, fonts, i18n, snapshot);
    spawn_selected_profile_section(parent, theme, fonts, snapshot);
    spawn_development_section(parent, theme, metrics, fonts, i18n);
}

fn spawn_session_summary_row(
    parent: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    snapshot: &LoginUiSnapshot,
) {
    let account_text = login_status_text(snapshot);
    let character_text = character_status_text(snapshot);
    let connection_text = connection_status_text(snapshot);
    let can_switch_account = snapshot.account_state == AccountLoginState::LoggedIn
        || snapshot.account_state == AccountLoginState::LoginFailed
        || snapshot.account_state == AccountLoginState::LoggedOut;
    let can_change_character = snapshot.account_state == AccountLoginState::LoggedIn
        && snapshot.character_state == CharacterSelectionState::Selected;

    parent
        .spawn((
            Node {
                width: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                column_gap: px(theme.layout.row_column_gap),
                row_gap: px(theme.layout.row_gap),
                flex_wrap: FlexWrap::Wrap,
                padding: UiRect::axes(px(0), px(theme.layout.row_padding_y)),
                ..default()
            },
            children![(
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(theme.layout.row_gap),
                    flex_grow: 1.0,
                    ..default()
                },
                children![
                    screen_label(
                        theme,
                        fonts,
                        account_text,
                        UiThemeTextStyleRole::Body,
                        UiThemeTextColorRole::Primary,
                    ),
                    screen_label(
                        theme,
                        fonts,
                        character_text,
                        UiThemeTextStyleRole::Caption,
                        UiThemeTextColorRole::Muted,
                    ),
                    screen_label(
                        theme,
                        fonts,
                        connection_text,
                        UiThemeTextStyleRole::Caption,
                        UiThemeTextColorRole::Muted,
                    ),
                ],
            ),],
        ))
        .with_children(|row| {
            spawn_secondary_button(
                row,
                theme,
                metrics,
                fonts,
                i18n,
                "auth.login.switch_account",
                "Switch Account",
                !can_switch_account || login_request_pending_snapshot(snapshot),
                SwitchAccountButton,
            );
            if can_change_character {
                spawn_secondary_button(
                    row,
                    theme,
                    metrics,
                    fonts,
                    i18n,
                    "auth.login.change_character",
                    "Change Character",
                    character_request_pending_snapshot(snapshot),
                    ChangeCharacterButton,
                );
            }
        });
}

fn spawn_character_section(
    parent: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    snapshot: &LoginUiSnapshot,
) {
    let logged_in = snapshot.account_state == AccountLoginState::LoggedIn;
    let list_pending = snapshot.character_state == CharacterSelectionState::Loading;
    let create_pending = snapshot.character_state == CharacterSelectionState::Creating;
    let select_pending = snapshot.character_state == CharacterSelectionState::Selecting;
    let character_pending = character_request_pending_snapshot(snapshot);

    parent
        .spawn((Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: px(theme.layout.row_gap),
            ..default()
        },))
        .with_children(|parent| {
            parent.spawn(screen_label_key(
                theme,
                fonts,
                i18n,
                "auth.login.characters_section",
                "Characters",
                UiThemeTextStyleRole::SectionLabel,
                UiThemeTextColorRole::Muted,
            ));
            parent
                .spawn((Node {
                    width: percent(100),
                    column_gap: px(theme.layout.row_column_gap),
                    row_gap: px(theme.layout.row_gap),
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                },))
                .with_children(|parent| {
                    spawn_secondary_button(
                        parent,
                        theme,
                        metrics,
                        fonts,
                        i18n,
                        "auth.login.load_characters",
                        "Load Characters",
                        !logged_in || list_pending || create_pending || select_pending,
                        LoadCharactersButton,
                    );
                });

            if list_pending {
                parent.spawn(loading_label(
                    theme,
                    fonts,
                    i18n.tr("auth.login.loading_characters", "Loading characters..."),
                ));
            } else if snapshot.characters.is_empty() {
                parent.spawn(screen_label(
                    theme,
                    fonts,
                    if logged_in {
                        i18n.tr("auth.login.no_characters", "No characters yet.")
                    } else {
                        i18n.tr("auth.login.characters_locked", "Login to load characters.")
                    },
                    UiThemeTextStyleRole::Caption,
                    UiThemeTextColorRole::Muted,
                ));
            } else {
                for character in &snapshot.characters {
                    spawn_character_row(
                        parent,
                        theme,
                        metrics,
                        fonts,
                        i18n,
                        character,
                        snapshot,
                        select_pending,
                    );
                }
            }

            parent
                .spawn((Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(theme.layout.row_gap),
                    padding: UiRect::top(px(theme.layout.row_padding_y)),
                    ..default()
                },))
                .with_children(|parent| {
                    parent.spawn((
                        text_input(
                            theme,
                            metrics,
                            fonts,
                            i18n.tr("auth.login.character_name_placeholder", "Character name"),
                            DEFAULT_CHARACTER_NAME,
                        ),
                        CharacterNameInput,
                    ));
                    spawn_primary_button(
                        parent,
                        theme,
                        metrics,
                        fonts,
                        i18n,
                        "auth.login.create_character",
                        "Create Character",
                        !logged_in || character_pending,
                        CreateCharacterButton,
                    );
                });
        });
}

fn spawn_character_row(
    parent: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    character: &CharacterRowSnapshot,
    snapshot: &LoginUiSnapshot,
    select_pending: bool,
) {
    let is_selected = snapshot.character_id.as_deref() == Some(character.character_id.as_str());
    let is_pending = select_pending
        && snapshot.pending_character_id.as_deref() == Some(character.character_id.as_str());
    let disabled = select_pending || is_selected;

    parent
        .spawn((
            Node {
                width: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                column_gap: px(theme.layout.row_column_gap),
                row_gap: px(theme.layout.row_gap),
                flex_wrap: FlexWrap::Wrap,
                padding: UiRect::axes(px(0), px(theme.layout.row_padding_y)),
                ..default()
            },
            children![(
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(theme.layout.row_gap),
                    flex_grow: 1.0,
                    ..default()
                },
                children![
                    screen_label(
                        theme,
                        fonts,
                        character.name.clone(),
                        UiThemeTextStyleRole::Body,
                        UiThemeTextColorRole::Primary,
                    ),
                    screen_label(
                        theme,
                        fonts,
                        character.detail.clone(),
                        UiThemeTextStyleRole::Caption,
                        UiThemeTextColorRole::Muted,
                    ),
                ],
            ),],
        ))
        .with_children(|row| {
            spawn_primary_button(
                row,
                theme,
                metrics,
                fonts,
                i18n,
                if is_selected {
                    "auth.login.character_selected"
                } else if is_pending {
                    "auth.login.selecting_character"
                } else {
                    "auth.login.select_character"
                },
                if is_selected {
                    "Selected"
                } else if is_pending {
                    "Selecting..."
                } else {
                    "Select"
                },
                disabled,
                SelectCharacterButton {
                    character_id: character.character_id.clone(),
                },
            );
        });
}

fn spawn_status_notice(
    parent: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    fonts: &UiFontAssets,
    snapshot: &LoginUiSnapshot,
) {
    if let Some(error) = snapshot.last_error.as_ref() {
        spawn_notice_panel(
            parent,
            theme,
            fonts,
            auth_error_title(error),
            auth_error_detail(error),
            true,
        );
    }

    if let Some(notice) = snapshot.notice.as_ref() {
        spawn_notice_panel(
            parent,
            theme,
            fonts,
            notice.title.clone(),
            notice.detail.clone(),
            notice.is_blocking(),
        );
    }
}

fn spawn_notice_panel(
    parent: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    fonts: &UiFontAssets,
    title: String,
    detail: Option<String>,
    prominent: bool,
) {
    let border_color = if prominent {
        theme.colors.error
    } else {
        theme.colors.panel_border
    };
    let title_color = if prominent {
        theme.colors.text_error
    } else {
        theme.colors.text_primary
    };

    parent
        .spawn((
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: px(theme.layout.row_gap),
                padding: UiRect::all(px(theme.layout.row_padding_y * 1.5)),
                border: UiRect::all(px(theme.panel.border)),
                border_radius: BorderRadius::all(px(theme.panel.radius)),
                ..default()
            },
            BackgroundColor(theme.colors.panel_background),
            BorderColor::all(border_color),
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new(title),
                TextFont {
                    font: fonts.regular.clone(),
                    font_size: UiThemeTextStyleRole::Body.font_size(theme),
                    ..default()
                },
                TextColor(title_color),
                UiThemeTextStyleRole::Body,
            ));
            if let Some(detail) = detail.filter(|detail| !detail.trim().is_empty()) {
                panel.spawn(screen_label(
                    theme,
                    fonts,
                    detail,
                    UiThemeTextStyleRole::Caption,
                    UiThemeTextColorRole::Muted,
                ));
            }
        });
}

fn spawn_selected_profile_section(
    parent: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    fonts: &UiFontAssets,
    snapshot: &LoginUiSnapshot,
) {
    let Some(elements) = snapshot.element_snapshot else {
        return;
    };
    let title = snapshot
        .selected_character_name
        .as_deref()
        .map(|name| format!("{name} profile"))
        .unwrap_or_else(|| "Character profile".to_string());

    parent
        .spawn((Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: px(theme.layout.row_gap),
            padding: UiRect::top(px(theme.layout.row_padding_y)),
            ..default()
        },))
        .with_children(|parent| {
            parent.spawn(screen_label(
                theme,
                fonts,
                title,
                UiThemeTextStyleRole::SectionLabel,
                UiThemeTextColorRole::Muted,
            ));
            parent.spawn(screen_label(
                theme,
                fonts,
                "affinity and mastery are long-term server state",
                UiThemeTextStyleRole::Caption,
                UiThemeTextColorRole::Muted,
            ));
            parent.spawn(screen_label(
                theme,
                fonts,
                format!("affinity {}", format_element_values(elements.affinity)),
                UiThemeTextStyleRole::Caption,
                UiThemeTextColorRole::Primary,
            ));
            parent.spawn(screen_label(
                theme,
                fonts,
                format!("mastery {}", format_element_values(elements.mastery)),
                UiThemeTextStyleRole::Caption,
                UiThemeTextColorRole::Primary,
            ));
        });
}

fn spawn_development_section(
    parent: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
) {
    parent
        .spawn((Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: px(theme.layout.row_gap),
            padding: UiRect::top(px(theme.layout.row_padding_y)),
            ..default()
        },))
        .with_children(|section| {
            section.spawn((
                screen_label_key(
                    theme,
                    fonts,
                    i18n,
                    "auth.login.dev_section",
                    "Development",
                    UiThemeTextStyleRole::SectionLabel,
                    UiThemeTextColorRole::Muted,
                ),
                UiStyleBinding::new().with_text(UiTextStyleRole::Muted),
            ));
            section.spawn((
                secondary_route_button_key(
                    theme,
                    metrics,
                    fonts,
                    i18n,
                    "auth.login.dev_lobby",
                    "Open Lobby",
                    AppUiMode::Lobby,
                ),
                UiStyleBinding::new().with_button(UiButtonStyleRole::Secondary),
            ));
        });
}

fn spawn_primary_button<T: Bundle>(
    parent: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
    disabled: bool,
    marker: T,
) {
    if disabled {
        parent.spawn((
            disabled_primary_action_button_key(theme, metrics, fonts, i18n, key, fallback),
            marker,
        ));
    } else {
        parent.spawn((
            primary_action_button_key(theme, metrics, fonts, i18n, key, fallback),
            marker,
        ));
    }
}

fn spawn_secondary_button<T: Bundle>(
    parent: &mut ChildSpawnerCommands,
    theme: &UiTheme,
    metrics: &UiMetrics,
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    key: &'static str,
    fallback: &'static str,
    disabled: bool,
    marker: T,
) {
    if disabled {
        parent.spawn((
            disabled_secondary_action_button_key(theme, metrics, fonts, i18n, key, fallback),
            marker,
        ));
    } else {
        parent.spawn((
            secondary_action_button_key(theme, metrics, fonts, i18n, key, fallback),
            marker,
        ));
    }
}

fn loading_label(theme: &UiTheme, fonts: &UiFontAssets, text: String) -> impl Bundle {
    screen_label(
        theme,
        fonts,
        text,
        UiThemeTextStyleRole::Caption,
        UiThemeTextColorRole::Muted,
    )
}

pub(super) fn sync_login_screen_state(
    mut commands: Commands,
    theme: Res<UiTheme>,
    fonts: Res<UiFontAssets>,
    i18n: Res<UiI18n>,
    session: Res<MyServerSession>,
    mut ui_state: ResMut<LoginUiState>,
    dynamic_roots: Query<Entity, With<AuthDynamicRoot>>,
) {
    let next_snapshot = LoginUiSnapshot::from_session(
        &session,
        ui_state.last_error.as_ref(),
        ui_state.notice.as_ref(),
    );
    if ui_state.rendered.as_ref() == Some(&next_snapshot) && !i18n.is_changed() {
        return;
    }
    ui_state.rendered = Some(next_snapshot.clone());

    let theme = theme.into_inner();
    let fonts = fonts.into_inner();
    for root in &dynamic_roots {
        commands.entity(root).despawn_related::<Children>();
        commands.entity(root).with_children(|parent| {
            spawn_dynamic_auth_children(parent, theme, fonts, &next_snapshot);
        });
    }
}

pub(super) fn sync_character_select_screen_state(
    mut commands: Commands,
    theme: Res<UiTheme>,
    metrics: Res<UiMetrics>,
    fonts: Res<UiFontAssets>,
    i18n: Res<UiI18n>,
    session: Res<MyServerSession>,
    mut ui_state: ResMut<LoginUiState>,
    dynamic_roots: Query<Entity, With<AuthDynamicRoot>>,
) {
    let next_snapshot = LoginUiSnapshot::from_session(
        &session,
        ui_state.last_error.as_ref(),
        ui_state.notice.as_ref(),
    );
    if ui_state.rendered.as_ref() == Some(&next_snapshot) && !i18n.is_changed() {
        return;
    }
    ui_state.rendered = Some(next_snapshot.clone());

    let theme = theme.into_inner();
    let metrics = metrics.into_inner();
    let fonts = fonts.into_inner();
    let i18n = i18n.into_inner();
    for root in &dynamic_roots {
        commands.entity(root).despawn_related::<Children>();
        commands.entity(root).with_children(|parent| {
            spawn_dynamic_character_select_children(
                parent,
                theme,
                metrics,
                fonts,
                i18n,
                &next_snapshot,
            );
        });
    }
}

pub(super) fn sync_login_button_flags(
    mut commands: Commands,
    session: Res<MyServerSession>,
    environment_buttons: Query<Entity, With<ServerEnvironmentButton>>,
    login_buttons: Query<Entity, With<AccountLoginButton>>,
    guest_buttons: Query<Entity, With<GuestLoginButton>>,
    load_buttons: Query<Entity, With<LoadCharactersButton>>,
    create_buttons: Query<Entity, With<CreateCharacterButton>>,
    select_buttons: Query<(Entity, &SelectCharacterButton)>,
    switch_account_buttons: Query<Entity, With<SwitchAccountButton>>,
    change_character_buttons: Query<Entity, With<ChangeCharacterButton>>,
) {
    let environment_locked = MyServerProfiles::selection_locked(&session);
    let login_disabled = login_request_pending(&session)
        || session.account_login_state == AccountLoginState::LoggedIn;
    let load_disabled = !can_send_character_request(&session)
        || matches!(
            session.character_selection_state,
            CharacterSelectionState::Loading
        );
    let create_disabled = !can_send_character_request(&session)
        || matches!(
            session.character_selection_state,
            CharacterSelectionState::Creating
        );
    let switch_disabled = login_request_pending(&session);

    for entity in &environment_buttons {
        set_button_disabled(&mut commands, entity, environment_locked);
    }

    for entity in &login_buttons {
        set_button_disabled(&mut commands, entity, login_disabled);
        set_button_loading(&mut commands, entity, login_request_pending(&session));
    }
    for entity in &guest_buttons {
        set_button_disabled(&mut commands, entity, login_disabled);
        set_button_loading(&mut commands, entity, login_request_pending(&session));
    }
    for entity in &load_buttons {
        set_button_disabled(&mut commands, entity, load_disabled);
        set_button_loading(
            &mut commands,
            entity,
            matches!(
                session.character_selection_state,
                CharacterSelectionState::Loading
            ),
        );
    }
    for entity in &create_buttons {
        set_button_disabled(&mut commands, entity, create_disabled);
        set_button_loading(
            &mut commands,
            entity,
            matches!(
                session.character_selection_state,
                CharacterSelectionState::Creating
            ),
        );
    }
    for (entity, button) in &select_buttons {
        let selecting = matches!(
            session.character_selection_state,
            CharacterSelectionState::Selecting
        );
        let is_selected = session.character_id.as_deref() == Some(button.character_id.as_str());
        let is_pending = selecting
            && session.pending_character_id.as_deref() == Some(button.character_id.as_str());
        set_button_disabled(&mut commands, entity, selecting || is_selected);
        set_button_loading(&mut commands, entity, is_pending);
    }
    for entity in &switch_account_buttons {
        set_button_disabled(&mut commands, entity, switch_disabled);
        set_button_loading(&mut commands, entity, false);
    }
    for entity in &change_character_buttons {
        set_button_disabled(&mut commands, entity, !can_change_character(&session));
        set_button_loading(&mut commands, entity, false);
    }
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
    binding_values.set_text(
        CHARACTER_SUBTITLE_BINDING_PATH,
        i18n.tr(CHARACTER_SUBTITLE_BINDING_PATH, CHARACTER_SUBTITLE_FALLBACK),
    );
}

fn set_button_disabled(commands: &mut Commands, entity: Entity, disabled: bool) {
    if disabled {
        commands.entity(entity).try_insert(DisabledButton);
    } else {
        commands.entity(entity).try_remove::<DisabledButton>();
    }
}

fn set_button_loading(commands: &mut Commands, entity: Entity, loading: bool) {
    if loading {
        commands.entity(entity).try_insert(LoadingButton);
    } else {
        commands.entity(entity).try_remove::<LoadingButton>();
    }
}
