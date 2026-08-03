use std::{collections::BTreeMap, str::FromStr};

#[cfg(all(debug_assertions, not(target_os = "android")))]
use std::collections::HashMap;

use bevy::prelude::*;

#[cfg(all(debug_assertions, not(target_os = "android")))]
use crate::framework::ui::audit::UiAuditConfig;
use crate::framework::ui::{
    core::focus::UiFocusState,
    document::{
        UiActionDescriptor, UiActionId, UiActionParamSchema, UiActionParamType, UiActionRegistry,
        UiBindingDeclaration, UiBindingMissingBehavior, UiBindingPath, UiBindingScope,
        UiBindingType, UiDocumentId, UiNodeId, UiRegisteredActionKind,
    },
    widgets::{UiButtonEvent, UiButtonEventKind, UiTextInputValue},
};
#[cfg(all(debug_assertions, not(target_os = "android")))]
use crate::game::myserver::{AccountLoginState, CharacterSelectionState, CharacterSummary};
use crate::game::{
    myserver::{MyServerCommand, MyServerConfig, MyServerEvent, MyServerProfiles, MyServerSession},
    navigation::{AppUiMode, GameRouteCommand},
    ui_ids::{OWNER_CHARACTER_SELECT, OWNER_LOGIN, PANEL_CHARACTER_SELECT, PANEL_LOGIN},
};

use super::model::*;

pub(super) const LOGIN_DOCUMENT_ID: &str = "auth.login";
pub(super) const CHARACTER_SELECT_DOCUMENT_ID: &str = "auth.character_select";

pub(super) const ACTION_ACCOUNT_LOGIN: &str = "auth.account_login";
pub(super) const ACTION_GUEST_LOGIN: &str = "auth.guest_login";
pub(super) const ACTION_SWITCH_ENVIRONMENT: &str = "auth.switch_environment";
pub(super) const ACTION_LOAD_CHARACTERS: &str = "auth.load_characters";
pub(super) const ACTION_CREATE_CHARACTER: &str = "auth.create_character";
pub(super) const ACTION_SELECT_CHARACTER: &str = "auth.select_character";
pub(super) const ACTION_SWITCH_ACCOUNT: &str = "auth.switch_account";
pub(super) const ACTION_SWITCH_CHARACTER: &str = "auth.switch_character";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AuthActionSource {
    RustView,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct AuthPageBaseline {
    pub(super) mode: AppUiMode,
    pub(super) owner: crate::framework::ui::core::UiOwnerId,
    pub(super) panel: crate::framework::ui::core::UiPanelId,
    pub(super) action_source: AuthActionSource,
    pub(super) states: &'static [&'static str],
}

pub(super) const LOGIN_PAGE_BASELINE: AuthPageBaseline = AuthPageBaseline {
    mode: AppUiMode::Login,
    owner: OWNER_LOGIN,
    panel: PANEL_LOGIN,
    action_source: AuthActionSource::RustView,
    states: &[
        "not_logged_in",
        "logging_in",
        "logged_in",
        "login_failed",
        "blocked",
        "expired",
        "logged_out",
    ],
};

pub(super) const CHARACTER_SELECT_PAGE_BASELINE: AuthPageBaseline = AuthPageBaseline {
    mode: AppUiMode::CharacterSelect,
    owner: OWNER_CHARACTER_SELECT,
    panel: PANEL_CHARACTER_SELECT,
    action_source: AuthActionSource::RustView,
    states: &[
        "not_loaded",
        "loading",
        "no_characters",
        "creating",
        "awaiting_selection",
        "loading_profile",
        "selecting",
        "selected",
        "blocked",
        "selection_failed",
    ],
};

#[derive(Resource)]
pub(super) struct AuthHostContracts {
    pub(super) login_page: AuthPageBaseline,
    pub(super) character_select_page: AuthPageBaseline,
    pub(super) login_bindings: BTreeMap<UiBindingPath, UiBindingDeclaration>,
    pub(super) character_select_bindings: BTreeMap<UiBindingPath, UiBindingDeclaration>,
}

impl Default for AuthHostContracts {
    fn default() -> Self {
        Self {
            login_page: LOGIN_PAGE_BASELINE,
            character_select_page: CHARACTER_SELECT_PAGE_BASELINE,
            login_bindings: login_binding_schema(),
            character_select_bindings: character_select_binding_schema(),
        }
    }
}

pub(super) fn login_binding_schema() -> BTreeMap<UiBindingPath, UiBindingDeclaration> {
    binding_schema([
        (
            "auth.login.login_name",
            UiBindingScope::Local,
            UiBindingType::String,
        ),
        (
            "auth.login.password",
            UiBindingScope::Local,
            UiBindingType::String,
        ),
        (
            "auth.account.player_id",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "auth.login.status",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "auth.login.notice_title",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "auth.login.notice_detail",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "auth.login.request_pending",
            UiBindingScope::Owner,
            UiBindingType::Bool,
        ),
        (
            "auth.login.environment",
            UiBindingScope::Owner,
            UiBindingType::Enum {
                values: vec!["local".to_owned(), "production".to_owned()],
            },
        ),
    ])
}

pub(super) fn character_select_binding_schema() -> BTreeMap<UiBindingPath, UiBindingDeclaration> {
    let character_item = UiBindingType::Record {
        fields: BTreeMap::from([
            ("character_id".to_owned(), UiBindingType::String),
            ("name".to_owned(), UiBindingType::String),
            ("detail".to_owned(), UiBindingType::String),
        ]),
    };
    binding_schema([
        (
            "auth.character.new_name",
            UiBindingScope::Local,
            UiBindingType::String,
        ),
        (
            "auth.account.player_id",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "auth.character.character_id",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "auth.character.pending_character_id",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "auth.character.items",
            UiBindingScope::Owner,
            UiBindingType::List {
                item: Box::new(character_item),
                max_items: 64,
            },
        ),
        (
            "auth.character.status",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "auth.character.connection_status",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "auth.character.request_pending",
            UiBindingScope::Owner,
            UiBindingType::Bool,
        ),
    ])
}

fn binding_schema<const N: usize>(
    specs: [(&str, UiBindingScope, UiBindingType); N],
) -> BTreeMap<UiBindingPath, UiBindingDeclaration> {
    specs
        .into_iter()
        .map(|(path, scope, value_type)| {
            (
                UiBindingPath::from_str(path).expect("Auth binding paths are static and valid"),
                UiBindingDeclaration {
                    scope,
                    value_type,
                    default: None,
                    missing: UiBindingMissingBehavior::UseConsumerFallback,
                },
            )
        })
        .collect()
}

pub(super) fn register_auth_contracts(
    contracts: Res<AuthHostContracts>,
    mut registry: ResMut<UiActionRegistry>,
) {
    debug_assert_eq!(contracts.login_page.owner, OWNER_LOGIN);
    debug_assert_eq!(
        contracts.character_select_page.owner,
        OWNER_CHARACTER_SELECT
    );
    debug_assert!(!contracts.login_bindings.is_empty());
    debug_assert!(!contracts.character_select_bindings.is_empty());
    for descriptor in auth_action_descriptors() {
        registry
            .register(descriptor)
            .expect("Auth action registration must be valid and unique");
    }
}

pub(super) fn auth_action_descriptors() -> Vec<UiActionDescriptor> {
    vec![
        business_action(
            ACTION_ACCOUNT_LOGIN,
            LOGIN_DOCUMENT_ID,
            OWNER_LOGIN.as_str(),
            "login.submit",
        )
        .with_param(
            "login_name",
            UiActionParamSchema::required(UiActionParamType::String { max_bytes: 256 }),
        )
        .with_param(
            "password",
            UiActionParamSchema::required(UiActionParamType::String { max_bytes: 4096 }),
        ),
        business_action(
            ACTION_GUEST_LOGIN,
            LOGIN_DOCUMENT_ID,
            OWNER_LOGIN.as_str(),
            "login.guest",
        ),
        business_action(
            ACTION_SWITCH_ENVIRONMENT,
            LOGIN_DOCUMENT_ID,
            OWNER_LOGIN.as_str(),
            "login.environment",
        )
        .with_param(
            "environment",
            UiActionParamSchema::required(UiActionParamType::Enum {
                values: ["local".to_owned(), "production".to_owned()]
                    .into_iter()
                    .collect(),
            }),
        ),
        business_action(
            ACTION_LOAD_CHARACTERS,
            CHARACTER_SELECT_DOCUMENT_ID,
            OWNER_CHARACTER_SELECT.as_str(),
            "character.reload",
        ),
        business_action(
            ACTION_CREATE_CHARACTER,
            CHARACTER_SELECT_DOCUMENT_ID,
            OWNER_CHARACTER_SELECT.as_str(),
            "character.create",
        )
        .with_param(
            "name",
            UiActionParamSchema::required(UiActionParamType::String { max_bytes: 256 }),
        ),
        business_action(
            ACTION_SELECT_CHARACTER,
            CHARACTER_SELECT_DOCUMENT_ID,
            OWNER_CHARACTER_SELECT.as_str(),
            "character.row.select",
        )
        .with_param(
            "character_id",
            UiActionParamSchema::required(UiActionParamType::String { max_bytes: 256 }),
        ),
        business_action(
            ACTION_SWITCH_ACCOUNT,
            CHARACTER_SELECT_DOCUMENT_ID,
            OWNER_CHARACTER_SELECT.as_str(),
            "character.switch_account",
        ),
        business_action(
            ACTION_SWITCH_CHARACTER,
            CHARACTER_SELECT_DOCUMENT_ID,
            OWNER_CHARACTER_SELECT.as_str(),
            "character.switch_character",
        ),
    ]
}

fn business_action(action: &str, document: &str, owner: &str, source: &str) -> UiActionDescriptor {
    UiActionDescriptor::new(
        UiActionId::from_str(action).expect("Auth action IDs are static and valid"),
        UiDocumentId::from_str(document).expect("Auth document IDs are static and valid"),
        owner,
        UiRegisteredActionKind::BusinessCommand {
            target: action.to_owned(),
        },
    )
    .with_source(UiNodeId::from_str(source).expect("Auth source node IDs are static and valid"))
}

#[cfg(all(debug_assertions, not(target_os = "android")))]
pub(super) fn prepare_character_select_audit_fixture(
    audit_config: Res<UiAuditConfig>,
    mut session: ResMut<MyServerSession>,
) {
    maybe_seed_character_select_audit_session(
        audit_config.targets_screen("character_select"),
        &mut session,
    );
}

#[cfg(all(debug_assertions, not(target_os = "android")))]
pub(super) fn maybe_seed_character_select_audit_session(
    targets_character_select: bool,
    session: &mut MyServerSession,
) {
    if targets_character_select && session.account_login_state != AccountLoginState::LoggedIn {
        seed_character_select_audit_session(session);
    }
}

#[cfg(all(debug_assertions, not(target_os = "android")))]
pub(super) fn seed_character_select_audit_session(session: &mut MyServerSession) {
    session.account_login_state = AccountLoginState::LoggedIn;
    session.character_selection_state = CharacterSelectionState::AwaitingSelection;
    session.player_id = Some("plr_audit_account".to_owned());
    session.login_name = Some("audit_player".to_owned());
    session.characters = vec![
        audit_character("chr_audit_alpha", "WindRunner", "1042", 1),
        audit_character("chr_audit_beta", "WindRunner", "7815", 2),
    ];
}

#[cfg(all(debug_assertions, not(target_os = "android")))]
fn audit_character(
    character_id: &str,
    name: &str,
    discriminator: &str,
    world_id: i64,
) -> CharacterSummary {
    CharacterSummary {
        character_id: character_id.to_owned(),
        character_id_short: None,
        display_discriminator: Some(discriminator.to_owned()),
        same_name_hint: None,
        name: name.to_owned(),
        world_id: Some(world_id),
        status: Some("active".to_owned()),
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

#[derive(Clone, Copy, Debug, Component)]
pub(super) struct ServerEnvironmentButton(pub(super) crate::game::myserver::MyServerEnvironment);

#[derive(Component)]
pub(super) struct LoadCharactersButton;

#[derive(Component)]
pub(super) struct CreateCharacterButton;

#[derive(Component)]
pub(super) struct SwitchAccountButton;

#[derive(Component)]
pub(super) struct ChangeCharacterButton;

#[derive(Clone, Debug, Component)]
pub(super) struct SelectCharacterButton {
    pub(super) character_id: String,
}

#[derive(Component)]
pub(super) struct AuthDynamicRoot;

#[derive(Clone, Debug, Default, Resource)]
pub(super) struct LoginUiState {
    pub(super) rendered: Option<LoginUiSnapshot>,
    pub(super) last_error: Option<crate::game::myserver::MyServerDisplayError>,
    pub(super) notice: Option<AuthStatusNotice>,
}

impl LoginUiState {
    pub(super) fn clear_runtime_state(&mut self) {
        self.rendered = None;
        self.last_error = None;
        self.notice = None;
    }
}

pub(super) fn handle_login_buttons(
    mut myserver_commands: MessageWriter<MyServerCommand>,
    mut session: ResMut<MyServerSession>,
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
    change_character_buttons: Query<(), With<ChangeCharacterButton>>,
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
            session.begin_login();
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
            session.begin_login();
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
        } else if change_character_buttons.contains(event.entity) {
            if character_request_sent || !can_change_character(&session) {
                continue;
            }
            character_request_sent = true;
            clear_text_input_values(&mut input_values.p2());
            ui_state.last_error = None;
            ui_state.notice = None;
            myserver_commands.write(MyServerCommand::SwitchCharacter);
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

pub(super) fn handle_server_environment_buttons(
    mut config: ResMut<MyServerConfig>,
    mut profiles: ResMut<MyServerProfiles>,
    mut session: ResMut<MyServerSession>,
    mut ui_state: ResMut<LoginUiState>,
    mut focus_state: ResMut<UiFocusState>,
    mut input_values: ParamSet<(
        Query<&mut UiTextInputValue, With<LoginNameInput>>,
        Query<&mut UiTextInputValue, With<PasswordInput>>,
        Query<&mut UiTextInputValue, With<CharacterNameInput>>,
    )>,
    environment_buttons: Query<&ServerEnvironmentButton>,
    mut button_events: MessageReader<UiButtonEvent>,
) {
    for event in button_events.read() {
        if event.kind != UiButtonEventKind::Click {
            continue;
        }
        let Ok(button) = environment_buttons.get(event.entity) else {
            continue;
        };
        if button.0 == profiles.selected() {
            continue;
        }
        if !profiles.try_activate(button.0, config.as_mut(), session.as_mut()) {
            continue;
        }

        clear_all_text_input_values(&mut input_values);
        focus_state.focused_entity = None;
        ui_state.clear_runtime_state();
        info!(environment = ?button.0, "MyServer login environment selected");
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
                route_commands.write(GameRouteCommand::ChangeMode(AppUiMode::CharacterSelect));
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
            MyServerEvent::LogoutSucceeded => {
                clear_all_text_input_values(&mut input_values);
                focus_state.focused_entity = None;
                ui_state.clear_runtime_state();
                route_commands.write(GameRouteCommand::ChangeMode(AppUiMode::Login));
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
