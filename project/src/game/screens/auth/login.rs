use bevy::prelude::*;

use crate::framework::ui::{
    core::{
        UiLayer, UiLayerRoot, UiMetrics, UiPanelKind, UiViewport,
        binding::{UiBindingValues, UiBoundText},
        focus::UiFocusState,
    },
    i18n::UiI18n,
    style::{
        UiFontAssets, UiTheme,
        theme::{
            UiThemeBackgroundRole, UiThemeBorderRole, UiThemePanelNodeRole, UiThemeRootNodeRole,
            UiThemeTextColorRole, UiThemeTextStyleRole,
        },
    },
    widgets::{
        DisabledButton, LoadingButton, UiButtonEvent, UiButtonEventKind, UiTextInputValue,
        disabled_primary_action_button_key, disabled_secondary_action_button_key,
        primary_action_button_key, screen_label, screen_label_key, screen_title_key,
        secondary_action_button_key, text_input,
    },
};
use crate::game::{
    myserver::{
        AccountLoginState, CharacterSelectionState, CharacterSummary, ElementValues,
        GameConnectionState, MyServerCommand, MyServerDisplayError, MyServerErrorKind,
        MyServerErrorSource, MyServerEvent, MyServerOperation, MyServerSession,
    },
    navigation::{AppUiMode, GameRouteCommand, game_panel_root, secondary_route_button_key},
    ui_ids::{OWNER_LOGIN, PANEL_LOGIN},
};

const LOGIN_SUBTITLE_BINDING_PATH: &str = "auth.login.subtitle";
const LOGIN_SUBTITLE_FALLBACK: &str = "Account and Character";
const DEFAULT_CHARACTER_NAME: &str = "";

#[derive(Component)]
pub(super) struct LoginNameInput;

#[derive(Component)]
pub(super) struct PasswordInput;

#[derive(Component)]
pub(super) struct CharacterNameInput;

#[derive(Component)]
pub(super) struct AccountLoginButton;

#[derive(Component)]
pub(super) struct GuestLoginButton;

#[derive(Component)]
pub(super) struct LoadCharactersButton;

#[derive(Component)]
pub(super) struct CreateCharacterButton;

#[derive(Component)]
pub(super) struct SwitchAccountButton;

#[derive(Clone, Debug, Component)]
pub(super) struct SelectCharacterButton {
    character_id: String,
}

#[derive(Component)]
pub(super) struct AuthDynamicRoot;

#[derive(Clone, Debug, Default, Resource)]
pub(super) struct LoginUiState {
    rendered: Option<LoginUiSnapshot>,
    last_error: Option<MyServerDisplayError>,
    notice: Option<AuthStatusNotice>,
}

impl LoginUiState {
    fn clear_runtime_state(&mut self) {
        self.rendered = None;
        self.last_error = None;
        self.notice = None;
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AuthStatusNotice {
    kind: AuthNoticeKind,
    title: String,
    detail: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AuthNoticeKind {
    Maintenance,
    Banned,
    PendingReview,
    VersionIncompatible,
    Kicked,
    Network,
    GenericFailure,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct LoginUiSnapshot {
    account_state: AccountLoginState,
    character_state: CharacterSelectionState,
    connection_state: GameConnectionState,
    player_id: Option<String>,
    login_name: Option<String>,
    guest_id: Option<String>,
    character_id: Option<String>,
    pending_character_id: Option<String>,
    characters: Vec<CharacterRowSnapshot>,
    selected_character_name: Option<String>,
    element_snapshot: Option<ElementSnapshot>,
    last_error: Option<AuthErrorSnapshot>,
    notice: Option<AuthStatusNotice>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CharacterRowSnapshot {
    character_id: String,
    name: String,
    detail: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ElementSnapshot {
    affinity: ElementValues,
    mastery: ElementValues,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AuthErrorSnapshot {
    kind: MyServerErrorKind,
    source: MyServerErrorSource,
    operation: Option<MyServerOperation>,
    message_key: &'static str,
    error_code: Option<String>,
    detail: Option<String>,
    retryable: bool,
    blocking: bool,
}

impl LoginUiSnapshot {
    fn from_session(session: &MyServerSession, ui_state: &LoginUiState) -> Self {
        Self {
            account_state: session.account_login_state,
            character_state: session.character_selection_state,
            connection_state: session.game_connection_state,
            player_id: session.player_id.clone(),
            login_name: session.login_name.clone(),
            guest_id: session.guest_id.clone(),
            character_id: session.character_id.clone(),
            pending_character_id: session.pending_character_id.clone(),
            characters: session
                .characters
                .iter()
                .map(CharacterRowSnapshot::from_character)
                .collect(),
            selected_character_name: session
                .current_character
                .as_ref()
                .map(|character| character.name.clone()),
            element_snapshot: element_snapshot_for_session(session),
            last_error: ui_state
                .last_error
                .as_ref()
                .map(AuthErrorSnapshot::from_display_error),
            notice: ui_state.notice.clone(),
        }
    }
}

impl CharacterRowSnapshot {
    fn from_character(character: &CharacterSummary) -> Self {
        Self {
            character_id: character.character_id.clone(),
            name: character.name.clone(),
            detail: character_display_detail(character),
        }
    }
}

impl AuthErrorSnapshot {
    fn from_display_error(error: &MyServerDisplayError) -> Self {
        Self {
            kind: error.kind,
            source: error.source,
            operation: error.operation,
            message_key: error.message_key,
            error_code: error.error_code.clone(),
            detail: error.detail.clone(),
            retryable: error.retryable,
            blocking: error.blocking,
        }
    }
}

pub(super) fn setup_login_screen(
    mut commands: Commands,
    theme: Res<UiTheme>,
    metrics: Res<UiMetrics>,
    viewport: Res<UiViewport>,
    fonts: Res<UiFontAssets>,
    i18n: Res<UiI18n>,
    mut binding_values: ResMut<UiBindingValues>,
    myserver_session: Res<MyServerSession>,
    mut clear_color: ResMut<ClearColor>,
) {
    let theme = theme.into_inner();
    let metrics = metrics.into_inner();
    let fonts = fonts.into_inner();
    let i18n = i18n.into_inner();
    clear_color.0 = theme.colors.screen_background;
    let subtitle = i18n.tr(LOGIN_SUBTITLE_BINDING_PATH, LOGIN_SUBTITLE_FALLBACK);
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
                    max_width: px(theme.layout.auth_panel_width),
                    align_self: AlignSelf::FlexStart,
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
            ))
            .with_children(|panel| {
                panel.spawn(screen_title_key(
                    theme,
                    fonts,
                    i18n,
                    "app.name",
                    "MyBevy",
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
                        LOGIN_SUBTITLE_BINDING_PATH,
                        LOGIN_SUBTITLE_FALLBACK,
                    )
                    .unwrap(),
                ));
                spawn_auth_form_section(panel, theme, metrics, fonts, i18n, &myserver_session);
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
    fonts: &UiFontAssets,
    i18n: &UiI18n,
    session: &MyServerSession,
) {
    let login_pending = login_request_pending(session);
    let logged_in = session.account_login_state == AccountLoginState::LoggedIn;

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
                "auth.login.account_section",
                "Account",
                UiThemeTextStyleRole::SectionLabel,
                UiThemeTextColorRole::Muted,
            ));
            parent.spawn((
                text_input(
                    theme,
                    metrics,
                    fonts,
                    i18n.tr("auth.login.account_placeholder", "Account"),
                    session.login_name.clone().unwrap_or_default(),
                ),
                LoginNameInput,
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
                    spawn_primary_button(
                        parent,
                        theme,
                        metrics,
                        fonts,
                        i18n,
                        "auth.login.sign_in",
                        "Login",
                        login_pending || logged_in,
                        AccountLoginButton,
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
                        GuestLoginButton,
                    );
                });
        });
}

fn spawn_dynamic_auth_children(
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
    parent.spawn((
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: px(theme.layout.row_gap),
            padding: UiRect::top(px(theme.layout.row_padding_y)),
            ..default()
        },
        children![
            screen_label_key(
                theme,
                fonts,
                i18n,
                "auth.login.dev_section",
                "Development",
                UiThemeTextStyleRole::SectionLabel,
                UiThemeTextColorRole::Muted,
            ),
            secondary_route_button_key(
                theme,
                metrics,
                fonts,
                i18n,
                "auth.login.dev_lobby",
                "Open Lobby",
                AppUiMode::Lobby,
            ),
        ],
    ));
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

pub(super) fn handle_login_buttons(
    mut myserver_commands: MessageWriter<MyServerCommand>,
    session: Res<MyServerSession>,
    mut ui_state: ResMut<LoginUiState>,
    mut input_values: ParamSet<(
        Query<&mut UiTextInputValue, With<LoginNameInput>>,
        Query<&mut UiTextInputValue, With<PasswordInput>>,
        Query<&mut UiTextInputValue, With<CharacterNameInput>>,
    )>,
    login_buttons: Query<(), With<AccountLoginButton>>,
    guest_buttons: Query<(), With<GuestLoginButton>>,
    load_buttons: Query<(), With<LoadCharactersButton>>,
    create_buttons: Query<(), With<CreateCharacterButton>>,
    switch_account_buttons: Query<(), With<SwitchAccountButton>>,
    select_buttons: Query<&SelectCharacterButton>,
    mut button_events: MessageReader<UiButtonEvent>,
) {
    let mut login_request_sent = false;
    let mut character_request_sent = false;

    for event in button_events.read() {
        if event.kind != UiButtonEventKind::Click {
            continue;
        }

        if login_buttons.contains(event.entity) {
            if login_request_sent || login_request_pending(&session) {
                continue;
            }
            let login_name = text_input_value(&input_values.p0());
            let password = text_input_value(&input_values.p1());
            if login_name.is_empty() || password.is_empty() {
                continue;
            }
            login_request_sent = true;
            ui_state.last_error = None;
            ui_state.notice = None;
            myserver_commands.write(MyServerCommand::Login {
                login_name,
                password,
                connect_game: false,
            });
        } else if guest_buttons.contains(event.entity) {
            if login_request_sent || login_request_pending(&session) {
                continue;
            }
            login_request_sent = true;
            ui_state.last_error = None;
            ui_state.notice = None;
            myserver_commands.write(MyServerCommand::GuestLogin {
                guest_id: None,
                connect_game: false,
            });
        } else if load_buttons.contains(event.entity) {
            if character_request_sent || !can_send_character_request(&session) {
                continue;
            }
            character_request_sent = true;
            ui_state.last_error = None;
            ui_state.notice = None;
            myserver_commands.write(MyServerCommand::LoadCharacterList);
        } else if create_buttons.contains(event.entity) {
            if character_request_sent || !can_send_character_request(&session) {
                continue;
            }
            let name = text_input_value(&input_values.p2());
            if name.is_empty() {
                continue;
            }
            character_request_sent = true;
            ui_state.last_error = None;
            ui_state.notice = None;
            myserver_commands.write(MyServerCommand::CreateCharacter {
                name,
                appearance_json: None,
            });
        } else if switch_account_buttons.contains(event.entity) {
            if login_request_pending(&session) {
                continue;
            }
            clear_text_input_values(&mut input_values.p0());
            clear_text_input_values(&mut input_values.p1());
            clear_text_input_values(&mut input_values.p2());
            ui_state.clear_runtime_state();
            myserver_commands.write(MyServerCommand::Logout);
        } else if let Ok(button) = select_buttons.get(event.entity) {
            if character_request_sent || !can_send_character_request(&session) {
                continue;
            }
            character_request_sent = true;
            ui_state.last_error = None;
            ui_state.notice = None;
            myserver_commands.write(MyServerCommand::SelectCharacter {
                character_id: button.character_id.clone(),
                connect_game: true,
            });
        }
    }
}

pub(super) fn follow_myserver_login_events(
    mut events: MessageReader<MyServerEvent>,
    mut commands: MessageWriter<MyServerCommand>,
    mut route_commands: MessageWriter<GameRouteCommand>,
    mut ui_state: ResMut<LoginUiState>,
    mut focus_state: ResMut<UiFocusState>,
    mut input_values: ParamSet<(
        Query<&mut UiTextInputValue, With<LoginNameInput>>,
        Query<&mut UiTextInputValue, With<PasswordInput>>,
        Query<&mut UiTextInputValue, With<CharacterNameInput>>,
    )>,
) {
    for event in events.read() {
        match event {
            MyServerEvent::DisplayError { error } => {
                ui_state.last_error = Some(error.clone());
                ui_state.notice = None;
            }
            MyServerEvent::LoginSucceeded(_) => {
                ui_state.last_error = None;
                ui_state.notice = None;
                commands.write(MyServerCommand::LoadCharacterList);
            }
            MyServerEvent::LoginFailed { error } => {
                ui_state.notice = Some(AuthStatusNotice::generic_failure(
                    "Login failed",
                    Some(error.clone()),
                ));
            }
            MyServerEvent::CharacterListFailed { error }
            | MyServerEvent::CharacterCreateFailed { error }
            | MyServerEvent::CharacterProfileFailed { error }
            | MyServerEvent::CharacterSelectFailed { error } => {
                ui_state.notice = Some(AuthStatusNotice::generic_failure(
                    "Character request failed",
                    Some(error.clone()),
                ));
            }
            MyServerEvent::CharacterCreated { character } => {
                ui_state.last_error = None;
                ui_state.notice = None;
                commands.write(MyServerCommand::SelectCharacter {
                    character_id: character.character_id.clone(),
                    connect_game: true,
                });
            }
            MyServerEvent::CharacterSelected { .. } | MyServerEvent::Authenticated { .. } => {
                clear_all_text_input_values(&mut input_values);
                focus_state.focused_entity = None;
                ui_state.clear_runtime_state();
                route_commands.write(GameRouteCommand::ChangeMode(AppUiMode::Lobby));
            }
            MyServerEvent::MaintenanceBlocked {
                message,
                retry_after_seconds,
            } => {
                ui_state.notice = Some(AuthStatusNotice {
                    kind: AuthNoticeKind::Maintenance,
                    title: "Server maintenance".to_string(),
                    detail: Some(match retry_after_seconds {
                        Some(seconds) => format!("{message} Retry after {seconds}s."),
                        None => message.clone(),
                    }),
                });
            }
            MyServerEvent::AccountStatusBlocked { code, message } => {
                ui_state.notice = Some(account_status_notice(code, message));
            }
            MyServerEvent::AccountBanned {
                message,
                banned_until,
            } => {
                ui_state.notice = Some(AuthStatusNotice {
                    kind: AuthNoticeKind::Banned,
                    title: "Account banned".to_string(),
                    detail: Some(match banned_until {
                        Some(until) => format!("{message} Until {until}."),
                        None => message.clone(),
                    }),
                });
            }
            MyServerEvent::VersionIncompatible {
                message,
                required_version,
                current_version,
            } => {
                ui_state.notice = Some(AuthStatusNotice {
                    kind: AuthNoticeKind::VersionIncompatible,
                    title: "Version incompatible".to_string(),
                    detail: Some(version_notice_detail(
                        message,
                        required_version.as_deref(),
                        current_version.as_deref(),
                    )),
                });
            }
            MyServerEvent::NetworkFailed { operation, error } => {
                ui_state.notice = Some(AuthStatusNotice {
                    kind: AuthNoticeKind::Network,
                    title: "Network unavailable".to_string(),
                    detail: Some(format!("{operation:?}: {error}")),
                });
            }
            MyServerEvent::SessionKicked {
                reason,
                category,
                timestamp,
            } => {
                ui_state.notice = Some(AuthStatusNotice {
                    kind: AuthNoticeKind::Kicked,
                    title: "Signed out elsewhere".to_string(),
                    detail: Some(format!("{category:?}: {reason} at {timestamp}")),
                });
            }
            _ => {}
        }
    }
}

pub(super) fn sync_login_screen_state(
    mut commands: Commands,
    theme: Res<UiTheme>,
    metrics: Res<UiMetrics>,
    fonts: Res<UiFontAssets>,
    i18n: Res<UiI18n>,
    session: Res<MyServerSession>,
    mut ui_state: ResMut<LoginUiState>,
    dynamic_roots: Query<Entity, With<AuthDynamicRoot>>,
) {
    let next_snapshot = LoginUiSnapshot::from_session(&session, &ui_state);
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
            spawn_dynamic_auth_children(parent, theme, metrics, fonts, i18n, &next_snapshot);
        });
    }
}

pub(super) fn sync_login_button_flags(
    mut commands: Commands,
    session: Res<MyServerSession>,
    login_buttons: Query<Entity, With<AccountLoginButton>>,
    guest_buttons: Query<Entity, With<GuestLoginButton>>,
    load_buttons: Query<Entity, With<LoadCharactersButton>>,
    create_buttons: Query<Entity, With<CreateCharacterButton>>,
    select_buttons: Query<(Entity, &SelectCharacterButton)>,
    switch_account_buttons: Query<Entity, With<SwitchAccountButton>>,
) {
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

fn text_input_value<T: Component>(inputs: &Query<&mut UiTextInputValue, With<T>>) -> String {
    inputs
        .iter()
        .next()
        .map(|value| value.0.trim().to_string())
        .unwrap_or_default()
}

fn clear_text_input_values<T: Component>(inputs: &mut Query<&mut UiTextInputValue, With<T>>) {
    for mut value in inputs.iter_mut() {
        value.0.clear();
    }
}

fn clear_all_text_input_values(
    input_values: &mut ParamSet<(
        Query<&mut UiTextInputValue, With<LoginNameInput>>,
        Query<&mut UiTextInputValue, With<PasswordInput>>,
        Query<&mut UiTextInputValue, With<CharacterNameInput>>,
    )>,
) {
    clear_text_input_values(&mut input_values.p0());
    clear_text_input_values(&mut input_values.p1());
    clear_text_input_values(&mut input_values.p2());
}

fn login_request_pending(session: &MyServerSession) -> bool {
    matches!(session.account_login_state, AccountLoginState::LoggingIn)
        || session.login_request.is_some()
}

fn login_request_pending_snapshot(snapshot: &LoginUiSnapshot) -> bool {
    matches!(snapshot.account_state, AccountLoginState::LoggingIn)
}

fn character_request_pending(session: &MyServerSession) -> bool {
    matches!(
        session.character_selection_state,
        CharacterSelectionState::Loading
            | CharacterSelectionState::Creating
            | CharacterSelectionState::LoadingProfile
            | CharacterSelectionState::Selecting
    )
}

fn character_request_pending_snapshot(snapshot: &LoginUiSnapshot) -> bool {
    matches!(
        snapshot.character_state,
        CharacterSelectionState::Loading
            | CharacterSelectionState::Creating
            | CharacterSelectionState::LoadingProfile
            | CharacterSelectionState::Selecting
    )
}

fn can_send_character_request(session: &MyServerSession) -> bool {
    session.account_login_state == AccountLoginState::LoggedIn
        && !character_request_pending(session)
}

fn login_status_text(snapshot: &LoginUiSnapshot) -> String {
    match snapshot.account_state {
        AccountLoginState::NotLoggedIn => "Not logged in".to_string(),
        AccountLoginState::LoggingIn => "Logging in...".to_string(),
        AccountLoginState::LoggedIn => {
            if let Some(login_name) = snapshot.login_name.as_deref() {
                format!("Logged in as {login_name}")
            } else if let Some(guest_id) = snapshot.guest_id.as_deref() {
                format!("Guest {guest_id}")
            } else if let Some(player_id) = snapshot.player_id.as_deref() {
                format!("Player {player_id}")
            } else {
                "Logged in".to_string()
            }
        }
        AccountLoginState::LoginFailed => "Login failed".to_string(),
        AccountLoginState::Blocked => "Account blocked".to_string(),
        AccountLoginState::Expired => "Session expired".to_string(),
        AccountLoginState::LoggedOut => "Logged out".to_string(),
    }
}

fn character_status_text(snapshot: &LoginUiSnapshot) -> String {
    match snapshot.character_state {
        CharacterSelectionState::NotLoaded => "Characters not loaded".to_string(),
        CharacterSelectionState::Loading => "Loading characters...".to_string(),
        CharacterSelectionState::NoCharacters => "Create a character to continue".to_string(),
        CharacterSelectionState::Creating => "Creating character...".to_string(),
        CharacterSelectionState::AwaitingSelection => "Choose a character".to_string(),
        CharacterSelectionState::LoadingProfile => "Loading profile...".to_string(),
        CharacterSelectionState::Selecting => "Selecting character...".to_string(),
        CharacterSelectionState::Selected => snapshot
            .selected_character_name
            .as_ref()
            .map(|name| format!("Selected {name}"))
            .unwrap_or_else(|| "Character selected".to_string()),
        CharacterSelectionState::Blocked => "Character unavailable".to_string(),
        CharacterSelectionState::SelectionFailed => "Character request failed".to_string(),
    }
}

fn connection_status_text(snapshot: &LoginUiSnapshot) -> String {
    match snapshot.connection_state {
        GameConnectionState::NotConnected => "Game server not connected".to_string(),
        GameConnectionState::Connecting => "Connecting to game server...".to_string(),
        GameConnectionState::Connected => "Game server connected".to_string(),
        GameConnectionState::Authenticating => "Signing in to game server...".to_string(),
        GameConnectionState::Authenticated => "Entering Lobby...".to_string(),
        GameConnectionState::Disconnected => "Game server disconnected".to_string(),
        GameConnectionState::Reconnecting => "Refreshing ticket...".to_string(),
        GameConnectionState::ReconnectFailed => "Network or ticket request failed".to_string(),
    }
}

fn element_snapshot_for_session(session: &MyServerSession) -> Option<ElementSnapshot> {
    let character_id = session.character_id.as_deref()?;
    if session.character_elements.character_id.as_deref() != Some(character_id) {
        return None;
    }

    session
        .character_elements
        .snapshot_refreshed_at
        .map(|_| ElementSnapshot {
            affinity: session.character_elements.affinity,
            mastery: session.character_elements.mastery,
        })
}

fn format_element_values(values: ElementValues) -> String {
    format!(
        "earth {} / fire {} / water {} / wind {}",
        values.earth, values.fire, values.water, values.wind
    )
}

fn auth_error_title(error: &AuthErrorSnapshot) -> String {
    match error.kind {
        MyServerErrorKind::Maintenance => "Server maintenance".to_string(),
        MyServerErrorKind::AccountBlocked
        | MyServerErrorKind::IpBlocked
        | MyServerErrorKind::PlayerBlocked => "Account blocked".to_string(),
        MyServerErrorKind::AccountBanned => "Account banned".to_string(),
        MyServerErrorKind::PendingReview => "Account under review".to_string(),
        MyServerErrorKind::VersionIncompatible => "Version incompatible".to_string(),
        MyServerErrorKind::CharacterUnavailable => "Character unavailable".to_string(),
        MyServerErrorKind::TicketExpired => "Ticket expired".to_string(),
        MyServerErrorKind::GameAuthRejected => "Game authentication failed".to_string(),
        MyServerErrorKind::SessionKicked => "Signed out elsewhere".to_string(),
        MyServerErrorKind::ConnectionTimeout | MyServerErrorKind::TransportFailed => {
            "Network unavailable".to_string()
        }
        MyServerErrorKind::Unauthorized => "Session expired".to_string(),
        _ => operation_failure_title(error.operation),
    }
}

fn operation_failure_title(operation: Option<MyServerOperation>) -> String {
    match operation {
        Some(
            MyServerOperation::Login | MyServerOperation::GuestLogin | MyServerOperation::Register,
        ) => "Login failed".to_string(),
        Some(
            MyServerOperation::CharacterList
            | MyServerOperation::CharacterCreate
            | MyServerOperation::CharacterProfile
            | MyServerOperation::CharacterSelect,
        ) => "Character request failed".to_string(),
        Some(MyServerOperation::TicketRefresh) => "Ticket request failed".to_string(),
        Some(MyServerOperation::GameConnect | MyServerOperation::GameRequest) => {
            "Network unavailable".to_string()
        }
        _ => "Request failed".to_string(),
    }
}

fn auth_error_detail(error: &AuthErrorSnapshot) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(detail) = error
        .detail
        .as_deref()
        .filter(|detail| !detail.trim().is_empty())
    {
        parts.push(detail.to_string());
    } else {
        parts.push(error_message_fallback(error.kind).to_string());
    }
    if let Some(code) = error
        .error_code
        .as_deref()
        .filter(|code| !code.trim().is_empty())
    {
        parts.push(format!("Code {code}."));
    }
    if error.retryable {
        parts.push("You can retry this operation.".to_string());
    }
    Some(parts.join(" "))
}

fn error_message_fallback(kind: MyServerErrorKind) -> &'static str {
    match kind {
        MyServerErrorKind::Maintenance => "The server is temporarily closed for maintenance.",
        MyServerErrorKind::AccountBanned => "This account cannot enter the game.",
        MyServerErrorKind::PendingReview => "This account is waiting for review.",
        MyServerErrorKind::VersionIncompatible => "Update the client before entering.",
        MyServerErrorKind::SessionKicked => "This session was kicked by the server.",
        MyServerErrorKind::CharacterUnavailable => "Choose another character or contact support.",
        MyServerErrorKind::TicketExpired => "Issue a fresh character ticket before entering.",
        MyServerErrorKind::ConnectionTimeout | MyServerErrorKind::TransportFailed => {
            "Check the network connection and try again."
        }
        _ => "The request could not be completed.",
    }
}

fn version_notice_detail(
    message: &str,
    required_version: Option<&str>,
    current_version: Option<&str>,
) -> String {
    let mut detail = message.to_string();
    if let Some(required) = required_version.filter(|value| !value.trim().is_empty()) {
        detail.push_str(&format!(" Required {required}."));
    }
    if let Some(current) = current_version.filter(|value| !value.trim().is_empty()) {
        detail.push_str(&format!(" Current {current}."));
    }
    detail
}

fn account_status_notice(code: &str, message: &str) -> AuthStatusNotice {
    let normalized_code = normalize_status_code(code);
    let detail = Some(format!("{message} ({code})"));

    if is_pending_review_status(&normalized_code) {
        return AuthStatusNotice {
            kind: AuthNoticeKind::PendingReview,
            title: "Account requires review".to_string(),
            detail,
        };
    }

    if is_kicked_status(&normalized_code) {
        return AuthStatusNotice {
            kind: AuthNoticeKind::Kicked,
            title: "Signed out elsewhere".to_string(),
            detail,
        };
    }

    AuthStatusNotice {
        kind: AuthNoticeKind::GenericFailure,
        title: "Account blocked".to_string(),
        detail,
    }
}

fn is_pending_review_status(normalized_code: &str) -> bool {
    normalized_code.starts_with("REGISTER_") || normalized_code.contains("PENDING_REVIEW")
}

fn is_kicked_status(normalized_code: &str) -> bool {
    normalized_code.contains("KICK") || normalized_code.contains("CONCURRENT")
}

fn normalize_status_code(value: &str) -> String {
    let mut code = String::new();
    let mut last_was_underscore = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            code.push(ch.to_ascii_uppercase());
            last_was_underscore = false;
        } else if !last_was_underscore && !code.is_empty() {
            code.push('_');
            last_was_underscore = true;
        }
    }
    while code.ends_with('_') {
        code.pop();
    }
    code
}

impl AuthStatusNotice {
    fn generic_failure(title: impl Into<String>, detail: Option<String>) -> Self {
        Self {
            kind: AuthNoticeKind::GenericFailure,
            title: title.into(),
            detail,
        }
    }

    fn is_blocking(&self) -> bool {
        matches!(
            self.kind,
            AuthNoticeKind::Maintenance
                | AuthNoticeKind::Banned
                | AuthNoticeKind::PendingReview
                | AuthNoticeKind::VersionIncompatible
                | AuthNoticeKind::Kicked
        )
    }
}

fn character_display_detail(character: &CharacterSummary) -> String {
    let discriminator = character
        .display_discriminator
        .as_deref()
        .or(character.character_id_short.as_deref())
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!("#{value}"))
        .unwrap_or_else(|| short_character_id(&character.character_id));

    let world = character
        .world_id
        .map(|world_id| format!("World {world_id}"))
        .unwrap_or_else(|| "World unknown".to_string());
    let status = character.status.as_deref().unwrap_or("active");
    format!("{discriminator} · {world} · {status}")
}

fn short_character_id(character_id: &str) -> String {
    let suffix: String = character_id
        .chars()
        .rev()
        .take(8)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    if suffix.is_empty() {
        "#unknown".to_string()
    } else {
        format!("#{suffix}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        ecs::message::MessageCursor,
        prelude::{App, Messages, MinimalPlugins},
    };
    use std::collections::HashMap;
    use std::time::SystemTime;

    #[test]
    fn character_detail_prefers_display_discriminator() {
        let character = test_character("chr_0000000000001", "WindRunner")
            .with_discriminator("0001")
            .with_short("short");

        assert_eq!(
            character_display_detail(&character),
            "#0001 · World 1 · active"
        );
    }

    #[test]
    fn character_detail_falls_back_to_short_id() {
        let character = test_character("chr_0000000000001", "WindRunner").with_short("00000001");

        assert_eq!(
            character_display_detail(&character),
            "#00000001 · World 1 · active"
        );
    }

    #[test]
    fn character_request_gate_blocks_pending_operations() {
        let mut session = MyServerSession {
            account_login_state: AccountLoginState::LoggedIn,
            ..Default::default()
        };
        assert!(can_send_character_request(&session));

        session.character_selection_state = CharacterSelectionState::Creating;
        assert!(!can_send_character_request(&session));

        session.character_selection_state = CharacterSelectionState::Selecting;
        assert!(!can_send_character_request(&session));
    }

    #[test]
    fn snapshot_uses_character_id_not_name_as_identity() {
        let mut first = test_character("chr_first", "SameName");
        first.display_discriminator = Some("1111".to_string());
        let mut second = test_character("chr_second", "SameName");
        second.display_discriminator = Some("2222".to_string());
        let session = MyServerSession {
            characters: vec![first, second],
            ..Default::default()
        };

        let ui_state = LoginUiState::default();
        let snapshot = LoginUiSnapshot::from_session(&session, &ui_state);

        assert_eq!(snapshot.characters[0].character_id, "chr_first");
        assert_eq!(snapshot.characters[1].character_id, "chr_second");
        assert_ne!(snapshot.characters[0].detail, snapshot.characters[1].detail);
    }

    #[test]
    fn snapshot_includes_connection_error_and_server_element_state() {
        let mut ui_state = LoginUiState::default();
        ui_state.last_error = Some(MyServerDisplayError::from_error_code(
            MyServerErrorSource::Http,
            Some(MyServerOperation::Login),
            None,
            None,
            None,
            "MAINTENANCE",
            Some("closed for patch".to_string()),
        ));
        let mut session = MyServerSession {
            account_login_state: AccountLoginState::LoggedIn,
            character_selection_state: CharacterSelectionState::Selected,
            game_connection_state: GameConnectionState::Authenticating,
            character_id: Some("chr_1".to_string()),
            current_character: Some(test_character("chr_1", "WindRunner")),
            ..Default::default()
        };
        session.apply_character_elements_snapshot(
            "chr_1".to_string(),
            crate::game::myserver::CharacterElements {
                affinity: ElementValues {
                    earth: 2500,
                    fire: 2500,
                    water: 2500,
                    wind: 2500,
                },
                mastery: ElementValues {
                    fire: 7,
                    ..Default::default()
                },
            },
            SystemTime::UNIX_EPOCH,
        );

        let snapshot = LoginUiSnapshot::from_session(&session, &ui_state);

        assert_eq!(
            connection_status_text(&snapshot),
            "Signing in to game server..."
        );
        assert_eq!(
            auth_error_title(snapshot.last_error.as_ref().unwrap()),
            "Server maintenance"
        );
        assert_eq!(
            snapshot.selected_character_name.as_deref(),
            Some("WindRunner")
        );
        let elements = snapshot.element_snapshot.unwrap();
        assert_eq!(elements.affinity.earth, 2500);
        assert_eq!(elements.mastery.fire, 7);
    }

    #[test]
    fn element_snapshot_only_uses_current_server_cache() {
        let mut matching = MyServerSession {
            character_id: Some("chr_current".to_string()),
            ..Default::default()
        };
        matching.apply_character_elements_snapshot(
            "chr_current".to_string(),
            crate::game::myserver::CharacterElements {
                affinity: ElementValues {
                    wind: 11,
                    ..Default::default()
                },
                ..Default::default()
            },
            SystemTime::UNIX_EPOCH,
        );
        assert_eq!(
            element_snapshot_for_session(&matching)
                .unwrap()
                .affinity
                .wind,
            11
        );

        let mut stale = MyServerSession {
            character_id: Some("chr_current".to_string()),
            ..Default::default()
        };
        stale.apply_character_elements_snapshot(
            "chr_old".to_string(),
            crate::game::myserver::CharacterElements {
                affinity: ElementValues {
                    wind: 11,
                    ..Default::default()
                },
                ..Default::default()
            },
            SystemTime::UNIX_EPOCH,
        );
        assert!(element_snapshot_for_session(&stale).is_none());
    }

    #[test]
    fn auth_error_titles_cover_blocking_and_network_states() {
        for (kind, expected) in [
            (MyServerErrorKind::AccountBanned, "Account banned"),
            (MyServerErrorKind::PendingReview, "Account under review"),
            (
                MyServerErrorKind::VersionIncompatible,
                "Version incompatible",
            ),
            (MyServerErrorKind::SessionKicked, "Signed out elsewhere"),
            (MyServerErrorKind::TransportFailed, "Network unavailable"),
        ] {
            let error = AuthErrorSnapshot {
                kind,
                source: MyServerErrorSource::Client,
                operation: None,
                message_key: kind.message_key(),
                error_code: None,
                detail: None,
                retryable: kind.retryable(),
                blocking: kind.blocking(),
            };

            assert_eq!(auth_error_title(&error), expected);
        }
    }

    #[test]
    fn auth_account_status_notice_classifies_pending_review_kick_and_blocked() {
        let pending = account_status_notice("REGISTER_PENDING_REVIEW", "pending review");
        assert_eq!(pending.kind, AuthNoticeKind::PendingReview);
        assert_eq!(pending.title, "Account requires review");

        let kicked = account_status_notice("SESSION_KICK_CONCURRENT_LOGIN", "login elsewhere");
        assert_eq!(kicked.kind, AuthNoticeKind::Kicked);
        assert_eq!(kicked.title, "Signed out elsewhere");

        let blocked = account_status_notice("ACCOUNT_BLOCKED", "blocked");
        assert_eq!(blocked.kind, AuthNoticeKind::GenericFailure);
        assert_eq!(blocked.title, "Account blocked");
    }

    #[test]
    fn auth_login_button_sends_account_login_command_from_inputs() {
        let mut app = login_button_test_app(MyServerSession::default());
        let button = app.world_mut().spawn(AccountLoginButton).id();
        app.world_mut()
            .spawn((LoginNameInput, UiTextInputValue("alice".to_string())));
        app.world_mut()
            .spawn((PasswordInput, UiTextInputValue("secret".to_string())));

        click(&mut app, button);
        app.update();

        let commands = read_messages::<MyServerCommand>(&app);
        assert!(commands.iter().any(|command| matches!(
            command,
            MyServerCommand::Login {
                login_name,
                password,
                connect_game: false,
            } if login_name == "alice" && password == "secret"
        )));
    }

    #[test]
    fn auth_guest_button_sends_guest_login_command() {
        let mut app = login_button_test_app(MyServerSession::default());
        let button = app.world_mut().spawn(GuestLoginButton).id();

        click(&mut app, button);
        app.update();

        let commands = read_messages::<MyServerCommand>(&app);
        assert!(commands.iter().any(|command| matches!(
            command,
            MyServerCommand::GuestLogin {
                guest_id: None,
                connect_game: false,
            }
        )));
    }

    #[test]
    fn auth_create_button_sends_create_character_from_input_when_logged_in() {
        let mut app = login_button_test_app(logged_in_session());
        let button = app.world_mut().spawn(CreateCharacterButton).id();
        app.world_mut().spawn((
            CharacterNameInput,
            UiTextInputValue("WindRunner".to_string()),
        ));

        click(&mut app, button);
        app.update();

        let commands = read_messages::<MyServerCommand>(&app);
        assert!(commands.iter().any(|command| matches!(
            command,
            MyServerCommand::CreateCharacter {
                name,
                appearance_json: None,
            } if name == "WindRunner"
        )));
    }

    #[test]
    fn auth_select_button_sends_character_id_not_name() {
        let mut app = login_button_test_app(logged_in_session());
        let button = app
            .world_mut()
            .spawn(SelectCharacterButton {
                character_id: "chr_selected".to_string(),
            })
            .id();

        click(&mut app, button);
        app.update();

        let commands = read_messages::<MyServerCommand>(&app);
        assert!(commands.iter().any(|command| matches!(
            command,
            MyServerCommand::SelectCharacter {
                character_id,
                connect_game: true,
            } if character_id == "chr_selected"
        )));
    }

    #[test]
    fn auth_character_request_clicks_are_deduplicated_per_frame() {
        let mut app = login_button_test_app(logged_in_session());
        let create = app.world_mut().spawn(CreateCharacterButton).id();
        let select = app
            .world_mut()
            .spawn(SelectCharacterButton {
                character_id: "chr_selected".to_string(),
            })
            .id();
        app.world_mut().spawn((
            CharacterNameInput,
            UiTextInputValue("WindRunner".to_string()),
        ));

        click(&mut app, create);
        click(&mut app, select);
        app.update();

        let role_commands = read_messages::<MyServerCommand>(&app)
            .into_iter()
            .filter(|command| {
                matches!(
                    command,
                    MyServerCommand::LoadCharacterList
                        | MyServerCommand::CreateCharacter { .. }
                        | MyServerCommand::SelectCharacter { .. }
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(role_commands.len(), 1);
        assert!(matches!(
            &role_commands[0],
            MyServerCommand::CreateCharacter { name, .. } if name == "WindRunner"
        ));
    }

    #[test]
    fn auth_pending_character_state_blocks_character_requests() {
        let mut session = logged_in_session();
        session.character_selection_state = CharacterSelectionState::Creating;
        let mut app = login_button_test_app(session);
        let button = app.world_mut().spawn(CreateCharacterButton).id();
        app.world_mut().spawn((
            CharacterNameInput,
            UiTextInputValue("WindRunner".to_string()),
        ));

        click(&mut app, button);
        app.update();

        assert!(read_messages::<MyServerCommand>(&app).is_empty());
    }

    #[test]
    fn auth_switch_account_clears_inputs_and_sends_logout() {
        let mut app = login_button_test_app(logged_in_session());
        let button = app.world_mut().spawn(SwitchAccountButton).id();
        let login = app
            .world_mut()
            .spawn((LoginNameInput, UiTextInputValue("alice".to_string())))
            .id();
        let password = app
            .world_mut()
            .spawn((PasswordInput, UiTextInputValue("secret".to_string())))
            .id();
        let character_name = app
            .world_mut()
            .spawn((
                CharacterNameInput,
                UiTextInputValue("WindRunner".to_string()),
            ))
            .id();

        click(&mut app, button);
        app.update();

        let commands = read_messages::<MyServerCommand>(&app);
        assert!(
            commands
                .iter()
                .any(|command| matches!(command, MyServerCommand::Logout))
        );
        assert_eq!(app.world().get::<UiTextInputValue>(login).unwrap().0, "");
        assert_eq!(app.world().get::<UiTextInputValue>(password).unwrap().0, "");
        assert_eq!(
            app.world()
                .get::<UiTextInputValue>(character_name)
                .unwrap()
                .0,
            ""
        );
    }

    trait CharacterTestExt {
        fn with_discriminator(self, value: &str) -> Self;
        fn with_short(self, value: &str) -> Self;
    }

    impl CharacterTestExt for CharacterSummary {
        fn with_discriminator(mut self, value: &str) -> Self {
            self.display_discriminator = Some(value.to_string());
            self
        }

        fn with_short(mut self, value: &str) -> Self {
            self.character_id_short = Some(value.to_string());
            self
        }
    }

    fn login_button_test_app(session: MyServerSession) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<UiButtonEvent>()
            .add_message::<MyServerCommand>()
            .insert_resource(session)
            .init_resource::<LoginUiState>()
            .add_systems(Update, handle_login_buttons);
        app
    }

    fn logged_in_session() -> MyServerSession {
        MyServerSession {
            account_login_state: AccountLoginState::LoggedIn,
            access_token: Some("access-token".to_string()),
            ..Default::default()
        }
    }

    fn click(app: &mut App, entity: Entity) {
        app.world_mut().write_message(UiButtonEvent {
            entity,
            kind: UiButtonEventKind::Click,
            button: None,
        });
    }

    fn read_messages<M>(app: &App) -> Vec<M>
    where
        M: Message + Clone,
    {
        let messages = app.world().resource::<Messages<M>>();
        let mut cursor = MessageCursor::default();
        cursor.read(messages).cloned().collect()
    }

    fn test_character(character_id: &str, name: &str) -> CharacterSummary {
        CharacterSummary {
            character_id: character_id.to_string(),
            character_id_short: None,
            display_discriminator: None,
            same_name_hint: None,
            name: name.to_string(),
            world_id: Some(1),
            status: Some("active".to_string()),
            appearance_json: None,
            created_at: None,
            last_login_at: None,
            deleted_at: None,
            position: None,
            attributes: None,
            lifecycle: None,
            extra: HashMap::new(),
        }
    }
}
