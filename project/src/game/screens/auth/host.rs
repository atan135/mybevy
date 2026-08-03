use std::{collections::BTreeMap, str::FromStr};

#[cfg(all(debug_assertions, not(target_os = "android")))]
use std::collections::HashMap;

use bevy::prelude::*;

#[cfg(all(debug_assertions, not(target_os = "android")))]
use crate::framework::ui::audit::UiAuditConfig;
use crate::framework::ui::{
    core::{binding::UiBindingValues, focus::UiFocusState},
    document::{
        UiActionDescriptor, UiActionDispatch, UiActionId, UiActionParamSchema, UiActionParamType,
        UiActionRegistry, UiActionValue, UiBindingDeclaration, UiBindingMissingBehavior,
        UiBindingPath, UiBindingScope, UiBindingType, UiBindingValue, UiBindingVisibility,
        UiDocumentId, UiDocumentLayer, UiDocumentNodeMarker, UiDocumentPanel, UiDocumentRuntime,
        UiHostBindingKey, UiNodeId, UiPageState, UiRegisteredActionKind,
    },
    widgets::{UiButtonEvent, UiButtonEventKind, UiSensitiveTextInput, UiTextInputValue},
};
#[cfg(all(debug_assertions, not(target_os = "android")))]
use crate::game::myserver::{AccountLoginState, CharacterSelectionState, CharacterSummary};
use crate::game::{
    declarative_screen::{
        DeclarativeScreenFailurePolicy, DeclarativeScreenHost, DeclarativeScreenRegistry,
        DeclarativeScreenSource,
    },
    myserver::{MyServerCommand, MyServerConfig, MyServerEvent, MyServerProfiles, MyServerSession},
    navigation::{AppUiMode, GameRouteCommand},
    ui_ids::{OWNER_CHARACTER_SELECT, OWNER_LOGIN, PANEL_CHARACTER_SELECT, PANEL_LOGIN},
};

use super::model::*;

pub(super) const LOGIN_DOCUMENT_ID: &str = "auth.login";
pub(super) const CHARACTER_SELECT_DOCUMENT_ID: &str = "auth.character_select";
pub(super) const LOGIN_DOCUMENT_SOURCE: &str =
    include_str!("../../../../assets/ui/documents/approved/auth/login.v1.json");
pub(super) const LOGIN_DOCUMENT_SOURCE_PATH: &str = "auth/login.v1.json";
pub(super) const LOGIN_ACCOUNT_NODE: &str = "login.account";
pub(super) const LOGIN_PASSWORD_NODE: &str = "login.password";

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
    UiDocument,
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
    action_source: AuthActionSource::UiDocument,
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
            "auth.login.error_title",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "auth.login.error_detail",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "auth.login.error_visibility",
            UiBindingScope::Owner,
            UiBindingType::Visibility,
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
            "auth.login.notice_visibility",
            UiBindingScope::Owner,
            UiBindingType::Visibility,
        ),
        (
            "auth.login.request_pending",
            UiBindingScope::Owner,
            UiBindingType::Bool,
        ),
        (
            "auth.login.disabled",
            UiBindingScope::Owner,
            UiBindingType::Bool,
        ),
        (
            "auth.login.environment_locked",
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
    mut screens: ResMut<DeclarativeScreenRegistry>,
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
    screens
        .register(login_declarative_screen_host(&contracts))
        .expect("Login declarative screen registration must be valid and unique");
}

pub(super) fn login_declarative_screen_host(
    contracts: &AuthHostContracts,
) -> DeclarativeScreenHost {
    let binding_schema = contracts
        .login_bindings
        .iter()
        .filter(|(_, declaration)| {
            matches!(
                declaration.scope,
                UiBindingScope::Document | UiBindingScope::Owner
            )
        })
        .map(|(path, declaration)| {
            (
                UiHostBindingKey::new(declaration.scope, path.clone()),
                declaration.value_type.clone(),
            )
        })
        .collect();
    let source =
        DeclarativeScreenSource::approved(LOGIN_DOCUMENT_SOURCE_PATH, LOGIN_DOCUMENT_SOURCE);
    DeclarativeScreenHost {
        document_id: UiDocumentId::from_str(LOGIN_DOCUMENT_ID)
            .expect("Login document ID is static and valid"),
        route: "login",
        route_aliases: &["login"],
        mode: Some(AppUiMode::Login),
        owner: OWNER_LOGIN,
        panel: UiDocumentPanel::Page,
        layer: UiDocumentLayer::Page,
        initial_state: UiPageState::initial(),
        binding_schema,
        action_allowlist: [
            ACTION_ACCOUNT_LOGIN,
            ACTION_GUEST_LOGIN,
            ACTION_SWITCH_ENVIRONMENT,
        ]
        .into_iter()
        .map(|action| UiActionId::from_str(action).expect("Login action IDs are static and valid"))
        .collect(),
        audit_profiles: [
            "desktop",
            "phone-landscape",
            "phone-1080p-landscape",
            "tablet-landscape",
        ]
        .map(str::to_owned)
        .to_vec(),
        source: source.clone(),
        fallback_source: Some(source),
        failure_policy: DeclarativeScreenFailurePolicy::PackagedFallback,
    }
}

pub(super) fn auth_action_descriptors() -> Vec<UiActionDescriptor> {
    vec![
        business_action(
            ACTION_ACCOUNT_LOGIN,
            LOGIN_DOCUMENT_ID,
            OWNER_LOGIN.as_str(),
            "login.submit",
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
pub(super) struct CharacterNameInput;

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

pub(super) fn handle_login_document_actions(
    mut actions: MessageReader<UiActionDispatch>,
    runtime: Res<UiDocumentRuntime>,
    mut input_values: Query<(
        &UiDocumentNodeMarker,
        &mut UiTextInputValue,
        Has<UiSensitiveTextInput>,
    )>,
    mut config: ResMut<MyServerConfig>,
    mut profiles: ResMut<MyServerProfiles>,
    mut session: ResMut<MyServerSession>,
    mut ui_state: ResMut<LoginUiState>,
    mut focus_state: ResMut<UiFocusState>,
    mut myserver_commands: MessageWriter<MyServerCommand>,
) {
    let Some(instance_id) = runtime.active_instance(
        OWNER_LOGIN.as_str(),
        &UiDocumentId::from_str(LOGIN_DOCUMENT_ID).expect("Login document ID is static and valid"),
    ) else {
        for _ in actions.read() {}
        return;
    };
    let mut login_request_sent = false;

    for action in actions.read() {
        if !is_login_business_action(action) {
            continue;
        }

        match action.action.as_str() {
            ACTION_ACCOUNT_LOGIN
                if action.source_node.as_str() == "login.submit" && action.params.is_empty() =>
            {
                if login_request_sent || login_request_pending(&session) {
                    continue;
                }
                let Some((login_name, password)) =
                    document_login_credentials(instance_id, &runtime, &mut input_values)
                else {
                    continue;
                };
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
            }
            ACTION_GUEST_LOGIN
                if action.source_node.as_str() == "login.guest" && action.params.is_empty() =>
            {
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
            }
            ACTION_SWITCH_ENVIRONMENT if action.source_node.as_str() == "login.environment" => {
                let Some(environment) = login_environment_param(action) else {
                    continue;
                };
                if environment == profiles.selected()
                    || !profiles.try_activate(environment, config.as_mut(), session.as_mut())
                {
                    continue;
                }
                clear_login_document_inputs(instance_id, &runtime, &mut input_values);
                focus_state.focused_entity = None;
                ui_state.clear_runtime_state();
                info!(?environment, "MyServer login environment selected");
            }
            _ => {}
        }
    }
}

fn is_login_business_action(action: &UiActionDispatch) -> bool {
    action.document_id.as_str() == LOGIN_DOCUMENT_ID
        && action.owner == OWNER_LOGIN.as_str()
        && matches!(
            &action.kind,
            UiRegisteredActionKind::BusinessCommand { target }
                if target == action.action.as_str()
        )
}

fn login_environment_param(
    action: &UiActionDispatch,
) -> Option<crate::game::myserver::MyServerEnvironment> {
    if action.params.len() != 1 {
        return None;
    }
    match action.params.get("environment") {
        Some(UiActionValue::Enum(value)) if value == "local" => {
            Some(crate::game::myserver::MyServerEnvironment::Local)
        }
        Some(UiActionValue::Enum(value)) if value == "production" => {
            Some(crate::game::myserver::MyServerEnvironment::Production)
        }
        _ => None,
    }
}

fn document_login_credentials(
    instance_id: crate::framework::ui::document::UiDocumentInstanceId,
    runtime: &UiDocumentRuntime,
    input_values: &mut Query<(
        &UiDocumentNodeMarker,
        &mut UiTextInputValue,
        Has<UiSensitiveTextInput>,
    )>,
) -> Option<(String, String)> {
    let account_id = UiNodeId::from_str(LOGIN_ACCOUNT_NODE).ok()?;
    let password_id = UiNodeId::from_str(LOGIN_PASSWORD_NODE).ok()?;
    let account_entity = runtime.node_entity(instance_id, &account_id)?;
    let password_entity = runtime.node_entity(instance_id, &password_id)?;

    let login_name = {
        let (marker, value, is_sensitive) = input_values.get_mut(account_entity).ok()?;
        if marker.instance_id != instance_id || marker.node_id != account_id || is_sensitive {
            return None;
        }
        value.0.trim().to_owned()
    };
    let password = {
        let (marker, value, is_sensitive) = input_values.get_mut(password_entity).ok()?;
        if marker.instance_id != instance_id || marker.node_id != password_id || !is_sensitive {
            return None;
        }
        value.0.trim().to_owned()
    };
    Some((login_name, password))
}

fn clear_login_document_inputs(
    instance_id: crate::framework::ui::document::UiDocumentInstanceId,
    runtime: &UiDocumentRuntime,
    input_values: &mut Query<(
        &UiDocumentNodeMarker,
        &mut UiTextInputValue,
        Has<UiSensitiveTextInput>,
    )>,
) {
    for (node, expected_sensitive) in [(LOGIN_ACCOUNT_NODE, false), (LOGIN_PASSWORD_NODE, true)] {
        let Ok(node_id) = UiNodeId::from_str(node) else {
            continue;
        };
        let Some(entity) = runtime.node_entity(instance_id, &node_id) else {
            continue;
        };
        let Ok((marker, mut value, is_sensitive)) = input_values.get_mut(entity) else {
            continue;
        };
        if marker.instance_id == instance_id
            && marker.node_id == node_id
            && is_sensitive == expected_sensitive
        {
            value.0.clear();
        }
    }
}

pub(super) fn sync_login_document_bindings(
    session: Res<MyServerSession>,
    profiles: Res<MyServerProfiles>,
    mut ui_state: ResMut<LoginUiState>,
    contracts: Res<AuthHostContracts>,
    mut binding_values: ResMut<UiBindingValues>,
) {
    let snapshot = LoginUiSnapshot::from_session(
        &session,
        ui_state.last_error.as_ref(),
        ui_state.notice.as_ref(),
    );
    let error_title = snapshot
        .last_error
        .as_ref()
        .map(auth_error_title)
        .unwrap_or_default();
    let error_detail = snapshot
        .last_error
        .as_ref()
        .and_then(auth_error_detail)
        .unwrap_or_default();
    let notice_title = snapshot
        .notice
        .as_ref()
        .map(|notice| notice.title.clone())
        .unwrap_or_default();
    let notice_detail = snapshot
        .notice
        .as_ref()
        .and_then(|notice| notice.detail.clone())
        .unwrap_or_default();
    let request_pending = login_request_pending(&session);
    let disabled = request_pending
        || session.account_login_state == crate::game::myserver::AccountLoginState::LoggedIn;

    for (path, value) in [
        (
            "auth.account.player_id",
            UiBindingValue::String(snapshot.player_id.clone().unwrap_or_default()),
        ),
        (
            "auth.login.status",
            UiBindingValue::String(login_status_text(&snapshot)),
        ),
        (
            "auth.login.error_title",
            UiBindingValue::String(error_title),
        ),
        (
            "auth.login.error_detail",
            UiBindingValue::String(error_detail),
        ),
        (
            "auth.login.error_visibility",
            UiBindingValue::Visibility(if snapshot.last_error.is_some() {
                UiBindingVisibility::Visible
            } else {
                UiBindingVisibility::Hidden
            }),
        ),
        (
            "auth.login.notice_title",
            UiBindingValue::String(notice_title),
        ),
        (
            "auth.login.notice_detail",
            UiBindingValue::String(notice_detail),
        ),
        (
            "auth.login.notice_visibility",
            UiBindingValue::Visibility(if snapshot.notice.is_some() {
                UiBindingVisibility::Visible
            } else {
                UiBindingVisibility::Hidden
            }),
        ),
        (
            "auth.login.request_pending",
            UiBindingValue::Bool(request_pending),
        ),
        ("auth.login.disabled", UiBindingValue::Bool(disabled)),
        (
            "auth.login.environment_locked",
            UiBindingValue::Bool(MyServerProfiles::selection_locked(&session)),
        ),
        (
            "auth.login.environment",
            UiBindingValue::Enum(
                match profiles.selected() {
                    crate::game::myserver::MyServerEnvironment::Local => "local",
                    crate::game::myserver::MyServerEnvironment::Production => "production",
                }
                .to_owned(),
            ),
        ),
    ] {
        let path = UiBindingPath::from_str(path).expect("Login binding paths are static and valid");
        let declaration = contracts
            .login_bindings
            .get(&path)
            .expect("Login binding schema contains every synchronized value");
        binding_values.set_scoped(
            LOGIN_DOCUMENT_ID,
            OWNER_LOGIN.as_str(),
            &path,
            declaration,
            value,
        );
    }
    ui_state.rendered = Some(snapshot);
}

pub(super) fn handle_character_select_buttons(
    mut myserver_commands: MessageWriter<MyServerCommand>,
    session: Res<MyServerSession>,
    mut ui_state: ResMut<LoginUiState>,
    mut input_values: Query<&mut UiTextInputValue, With<CharacterNameInput>>,
    load_buttons: Query<(), With<LoadCharactersButton>>,
    create_buttons: Query<(), With<CreateCharacterButton>>,
    switch_account_buttons: Query<(), With<SwitchAccountButton>>,
    change_character_buttons: Query<(), With<ChangeCharacterButton>>,
    select_buttons: Query<&SelectCharacterButton>,
    mut button_events: MessageReader<UiButtonEvent>,
) {
    let mut character_request_sent = false;

    for event in button_events.read() {
        if event.kind != UiButtonEventKind::Click {
            continue;
        }

        if load_buttons.contains(event.entity) {
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
            let name = text_input_value(&input_values);
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
            clear_text_input_values(&mut input_values);
            ui_state.clear_runtime_state();
            myserver_commands.write(MyServerCommand::Logout);
        } else if change_character_buttons.contains(event.entity) {
            if character_request_sent || !can_change_character(&session) {
                continue;
            }
            character_request_sent = true;
            clear_text_input_values(&mut input_values);
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

pub(super) fn follow_myserver_login_events(
    mut events: MessageReader<MyServerEvent>,
    mut commands: MessageWriter<MyServerCommand>,
    mut route_commands: MessageWriter<GameRouteCommand>,
    mut ui_state: ResMut<LoginUiState>,
    mut focus_state: ResMut<UiFocusState>,
    mut input_values: Query<&mut UiTextInputValue, With<CharacterNameInput>>,
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
                clear_text_input_values(&mut input_values);
                focus_state.focused_entity = None;
                ui_state.clear_runtime_state();
                route_commands.write(GameRouteCommand::ChangeMode(AppUiMode::Lobby));
            }
            MyServerEvent::LogoutSucceeded => {
                clear_text_input_values(&mut input_values);
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
