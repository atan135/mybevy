use std::{collections::BTreeMap, str::FromStr};

#[cfg(all(debug_assertions, not(target_os = "android")))]
use std::collections::HashMap;

use bevy::prelude::*;

#[cfg(all(debug_assertions, not(target_os = "android")))]
use crate::framework::ui::audit::UiAuditConfig;
#[cfg(all(debug_assertions, not(target_os = "android")))]
use crate::framework::ui::core::UiViewport;
use crate::framework::ui::{
    core::{binding::UiBindingValues, focus::UiFocusState},
    document::{
        UiActionDescriptor, UiActionDispatch, UiActionId, UiActionParamSchema, UiActionParamType,
        UiActionRegistry, UiActionValue, UiBindingDeclaration, UiBindingMissingBehavior,
        UiBindingPath, UiBindingScope, UiBindingType, UiBindingValue, UiDocumentId,
        UiDocumentLayer, UiDocumentNodeMarker, UiDocumentPanel, UiDocumentRuntime,
        UiHostBindingKey, UiNodeId, UiPageState, UiRegisteredActionKind,
    },
    i18n::UiI18n,
    widgets::{UiSensitiveTextInput, UiTextInputValue},
};
#[cfg(all(debug_assertions, not(target_os = "android")))]
use crate::game::myserver::{CharacterSelectionState, CharacterSummary};
use crate::game::{
    declarative_screen::{
        DeclarativeScreenFailurePolicy, DeclarativeScreenHost, DeclarativeScreenRegistry,
        DeclarativeScreenSource,
    },
    myserver::{
        AccountLoginState, MyServerCommand, MyServerConfig, MyServerEvent, MyServerProfiles,
        MyServerSession, RegistrationServerError, RegistrationState, RegistrationValidationError,
        validate_registration_request,
    },
    navigation::{AppUiMode, GameRouteCommand},
    ui_ids::{OWNER_CHARACTER_SELECT, OWNER_LOGIN, PANEL_CHARACTER_SELECT, PANEL_LOGIN},
};

use super::model::*;

pub(super) const LOGIN_DOCUMENT_ID: &str = "auth.login";
pub(super) const CHARACTER_SELECT_DOCUMENT_ID: &str = "auth.character_select";
pub(super) const LOGIN_DOCUMENT_SOURCE: &str =
    include_str!("../../../../assets/ui/documents/approved/auth/login.v1.json");
pub(super) const LOGIN_DOCUMENT_SOURCE_PATH: &str = "auth/login.v1.json";
pub(super) const CHARACTER_SELECT_DOCUMENT_SOURCE: &str =
    include_str!("../../../../assets/ui/documents/approved/auth/character_select.v1.json");
pub(super) const CHARACTER_SELECT_DOCUMENT_SOURCE_PATH: &str = "auth/character_select.v1.json";
pub(super) const LOGIN_ACCOUNT_NODE: &str = "login.account";
pub(super) const LOGIN_PASSWORD_NODE: &str = "login.password";
pub(super) const REGISTRATION_ACCOUNT_NODE: &str = "login.register.account";
pub(super) const REGISTRATION_PASSWORD_NODE: &str = "login.register.password";
pub(super) const REGISTRATION_PASSWORD_CONFIRMATION_NODE: &str =
    "login.register.password_confirmation";
pub(super) const CHARACTER_CREATE_NAME_NODE: &str = "character.create_name";
const CHARACTER_NAME_MAX_BYTES: usize = 256;
const CHARACTER_ID_MAX_BYTES: usize =
    crate::framework::ui::document::UI_REPEAT_MAX_ITEM_STRING_BYTES;

pub(super) const ACTION_ACCOUNT_LOGIN: &str = "auth.account_login";
pub(super) const ACTION_GUEST_LOGIN: &str = "auth.guest_login";
pub(super) const ACTION_SHOW_REGISTRATION: &str = "auth.show_registration";
pub(super) const ACTION_SHOW_LOGIN: &str = "auth.show_login";
pub(super) const ACTION_REGISTER: &str = "auth.register";
pub(super) const ACTION_DISMISS_REGISTRATION_REVIEW: &str = "auth.dismiss_registration_review";
pub(super) const ACTION_SWITCH_ENVIRONMENT: &str = "auth.switch_environment";
pub(super) const ACTION_LOAD_CHARACTERS: &str = "auth.load_characters";
pub(super) const ACTION_CREATE_CHARACTER: &str = "auth.create_character";
pub(super) const ACTION_SELECT_CHARACTER: &str = "auth.select_character";
pub(super) const ACTION_SWITCH_ACCOUNT: &str = "auth.switch_account";
pub(super) const ACTION_SWITCH_CHARACTER: &str = "auth.switch_character";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AuthActionSource {
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
        "login",
        "register",
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
    action_source: AuthActionSource::UiDocument,
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
            "auth.register.login_name",
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
            "auth.login.error_display",
            UiBindingScope::Owner,
            UiBindingType::Enum {
                values: vec!["flex".to_owned(), "none".to_owned()],
            },
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
            "auth.login.notice_display",
            UiBindingScope::Owner,
            UiBindingType::Enum {
                values: vec!["flex".to_owned(), "none".to_owned()],
            },
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
        (
            "auth.login.mode",
            UiBindingScope::Owner,
            UiBindingType::Enum {
                values: vec!["login".to_owned(), "register".to_owned()],
            },
        ),
        (
            "auth.login.login_display",
            UiBindingScope::Owner,
            UiBindingType::Enum {
                values: vec!["flex".to_owned(), "none".to_owned()],
            },
        ),
        (
            "auth.login.register_display",
            UiBindingScope::Owner,
            UiBindingType::Enum {
                values: vec!["flex".to_owned(), "none".to_owned()],
            },
        ),
        (
            "auth.register.state",
            UiBindingScope::Owner,
            UiBindingType::Enum {
                values: [
                    "idle",
                    "registering",
                    "failed",
                    "pending_review",
                    "succeeded",
                ]
                .map(str::to_owned)
                .to_vec(),
            },
        ),
        (
            "auth.register.request_pending",
            UiBindingScope::Owner,
            UiBindingType::Bool,
        ),
        (
            "auth.register.disabled",
            UiBindingScope::Owner,
            UiBindingType::Bool,
        ),
        (
            "auth.register.error_title",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "auth.register.error_detail",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "auth.register.error_display",
            UiBindingScope::Owner,
            UiBindingType::Enum {
                values: vec!["flex".to_owned(), "none".to_owned()],
            },
        ),
        (
            "auth.register.review_display",
            UiBindingScope::Owner,
            UiBindingType::Enum {
                values: vec!["flex".to_owned(), "none".to_owned()],
            },
        ),
        (
            "auth.register.success_display",
            UiBindingScope::Owner,
            UiBindingType::Enum {
                values: vec!["flex".to_owned(), "none".to_owned()],
            },
        ),
    ])
}

pub(super) fn character_select_binding_schema() -> BTreeMap<UiBindingPath, UiBindingDeclaration> {
    let character_item = UiBindingType::Record {
        fields: BTreeMap::from([
            ("character_id".to_owned(), UiBindingType::String),
            ("display_name".to_owned(), UiBindingType::String),
            ("detail".to_owned(), UiBindingType::String),
            ("selected".to_owned(), UiBindingType::Bool),
            ("pending".to_owned(), UiBindingType::Bool),
            ("disabled".to_owned(), UiBindingType::Bool),
            ("loading".to_owned(), UiBindingType::Bool),
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
            "auth.account.summary",
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
            "auth.character.collection_state",
            UiBindingScope::Owner,
            UiBindingType::Enum {
                values: vec!["loading".to_owned(), "ready".to_owned(), "error".to_owned()],
            },
        ),
        (
            "auth.character.view_state",
            UiBindingScope::Owner,
            UiBindingType::Enum {
                values: [
                    "loading",
                    "empty",
                    "ready",
                    "error",
                    "creating",
                    "selecting",
                    "current",
                ]
                .map(str::to_owned)
                .to_vec(),
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
        (
            "auth.character.load_disabled",
            UiBindingScope::Owner,
            UiBindingType::Bool,
        ),
        (
            "auth.character.load_loading",
            UiBindingScope::Owner,
            UiBindingType::Bool,
        ),
        (
            "auth.character.create_disabled",
            UiBindingScope::Owner,
            UiBindingType::Bool,
        ),
        (
            "auth.character.create_loading",
            UiBindingScope::Owner,
            UiBindingType::Bool,
        ),
        (
            "auth.character.switch_account_disabled",
            UiBindingScope::Owner,
            UiBindingType::Bool,
        ),
        (
            "auth.character.switch_character_disabled",
            UiBindingScope::Owner,
            UiBindingType::Bool,
        ),
        (
            "auth.character.switch_character_visibility",
            UiBindingScope::Owner,
            UiBindingType::Enum {
                values: vec!["flex".to_owned(), "none".to_owned()],
            },
        ),
        (
            "auth.character.error_title",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "auth.character.error_detail",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "auth.character.error_visibility",
            UiBindingScope::Owner,
            UiBindingType::Enum {
                values: vec!["flex".to_owned(), "none".to_owned()],
            },
        ),
        (
            "auth.character.notice_title",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "auth.character.notice_detail",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "auth.character.notice_visibility",
            UiBindingScope::Owner,
            UiBindingType::Enum {
                values: vec!["flex".to_owned(), "none".to_owned()],
            },
        ),
        (
            "auth.character.profile_visibility",
            UiBindingScope::Owner,
            UiBindingType::Enum {
                values: vec!["flex".to_owned(), "none".to_owned()],
            },
        ),
        (
            "auth.character.profile_title",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "auth.character.affinity",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "auth.character.mastery",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "auth.character.item.character_id",
            UiBindingScope::Item,
            UiBindingType::String,
        ),
        (
            "auth.character.item.display_name",
            UiBindingScope::Item,
            UiBindingType::String,
        ),
        (
            "auth.character.item.detail",
            UiBindingScope::Item,
            UiBindingType::String,
        ),
        (
            "auth.character.item.selected",
            UiBindingScope::Item,
            UiBindingType::Bool,
        ),
        (
            "auth.character.item.pending",
            UiBindingScope::Item,
            UiBindingType::Bool,
        ),
        (
            "auth.character.item.disabled",
            UiBindingScope::Item,
            UiBindingType::Bool,
        ),
        (
            "auth.character.item.loading",
            UiBindingScope::Item,
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
    screens
        .register(character_select_declarative_screen_host(&contracts))
        .expect("CharacterSelect declarative screen registration must be valid and unique");
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
            ACTION_SHOW_REGISTRATION,
            ACTION_SHOW_LOGIN,
            ACTION_REGISTER,
            ACTION_DISMISS_REGISTRATION_REVIEW,
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

pub(super) fn character_select_declarative_screen_host(
    contracts: &AuthHostContracts,
) -> DeclarativeScreenHost {
    let binding_schema = contracts
        .character_select_bindings
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
    let source = DeclarativeScreenSource::approved(
        CHARACTER_SELECT_DOCUMENT_SOURCE_PATH,
        CHARACTER_SELECT_DOCUMENT_SOURCE,
    );
    DeclarativeScreenHost {
        document_id: UiDocumentId::from_str(CHARACTER_SELECT_DOCUMENT_ID)
            .expect("CharacterSelect document ID is static and valid"),
        route: "character_select",
        route_aliases: &["character_select"],
        mode: Some(AppUiMode::CharacterSelect),
        owner: OWNER_CHARACTER_SELECT,
        panel: UiDocumentPanel::Page,
        layer: UiDocumentLayer::Page,
        initial_state: UiPageState::initial(),
        binding_schema,
        action_allowlist: [
            ACTION_LOAD_CHARACTERS,
            ACTION_CREATE_CHARACTER,
            ACTION_SELECT_CHARACTER,
            ACTION_SWITCH_ACCOUNT,
            ACTION_SWITCH_CHARACTER,
        ]
        .into_iter()
        .map(|action| {
            UiActionId::from_str(action).expect("CharacterSelect action IDs are static and valid")
        })
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
            ACTION_SHOW_REGISTRATION,
            LOGIN_DOCUMENT_ID,
            OWNER_LOGIN.as_str(),
            "login.mode.register",
        ),
        business_action(
            ACTION_SHOW_LOGIN,
            LOGIN_DOCUMENT_ID,
            OWNER_LOGIN.as_str(),
            "login.mode.login",
        ),
        business_action(
            ACTION_REGISTER,
            LOGIN_DOCUMENT_ID,
            OWNER_LOGIN.as_str(),
            "login.register.submit",
        ),
        business_action(
            ACTION_DISMISS_REGISTRATION_REVIEW,
            LOGIN_DOCUMENT_ID,
            OWNER_LOGIN.as_str(),
            "login.registration.back",
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
        ),
        business_action(
            ACTION_SELECT_CHARACTER,
            CHARACTER_SELECT_DOCUMENT_ID,
            OWNER_CHARACTER_SELECT.as_str(),
            "character.row.select",
        )
        .with_param(
            "character_id",
            UiActionParamSchema::required(UiActionParamType::OpaqueId {
                max_bytes: CHARACTER_ID_MAX_BYTES,
            }),
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
    viewport: Res<UiViewport>,
    mut session: ResMut<MyServerSession>,
    mut ui_state: ResMut<LoginUiState>,
) {
    if !audit_config.targets_screen("character_select") {
        return;
    }
    if session.account_login_state == AccountLoginState::LoggedIn {
        return;
    }
    maybe_seed_character_select_audit_session(true, &mut session);
    ui_state.clear_runtime_state();

    if viewport.logical_width >= 1100.0 && viewport.logical_height <= 740.0 {
        session.characters.clear();
        session.character_selection_state = CharacterSelectionState::NoCharacters;
    } else if viewport.logical_width < 900.0 && viewport.device_scale < 2.5 {
        session.characters = vec![audit_character(
            "chr:audit/long-unicode-角色-0000000000000001",
            "WindRunnerWithAnIntentionallyLongDisplayName",
            "1042",
            1,
        )];
    } else if viewport.logical_width < 900.0 {
        session.characters = (0..6)
            .map(|index| {
                audit_character(
                    &format!("chr:audit:multi:{index:02}"),
                    if index % 2 == 0 {
                        "WindRunner"
                    } else {
                        "StoneSong"
                    },
                    &format!("{:04}", 1042 + index),
                    i64::from(index + 1),
                )
            })
            .collect();
    } else {
        session.character_selection_state = CharacterSelectionState::SelectionFailed;
        ui_state.notice = Some(AuthStatusNotice::generic_failure(
            "Character request failed",
            Some("Audit fixture: character service unavailable.".to_owned()),
        ));
    }
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum AuthEntryMode {
    #[default]
    Login,
    Register,
}

impl AuthEntryMode {
    const fn binding_value(self) -> &'static str {
        match self {
            Self::Login => "login",
            Self::Register => "register",
        }
    }
}

#[derive(Clone, Debug, Default, Resource)]
pub(super) struct LoginUiState {
    pub(super) rendered: Option<LoginUiSnapshot>,
    pub(super) last_error: Option<crate::game::myserver::MyServerDisplayError>,
    pub(super) notice: Option<AuthStatusNotice>,
    pub(super) entry_mode: AuthEntryMode,
    pub(super) registration_validation_error: Option<RegistrationValidationError>,
    pub(super) registration_succeeded: bool,
}

pub(super) fn guard_character_select_session(
    session: Res<MyServerSession>,
    mut route_commands: MessageWriter<GameRouteCommand>,
) {
    if character_select_requires_login(&session) {
        route_commands.write(GameRouteCommand::ChangeMode(AppUiMode::Login));
    }
}

pub(super) fn character_select_requires_login(session: &MyServerSession) -> bool {
    session.account_login_state != AccountLoginState::LoggedIn
}

pub(super) fn cleanup_login_screen_state(
    mut ui_state: ResMut<LoginUiState>,
    mut focus_state: ResMut<UiFocusState>,
    runtime: Res<UiDocumentRuntime>,
    mut input_values: Query<(
        &UiDocumentNodeMarker,
        &mut UiTextInputValue,
        Has<UiSensitiveTextInput>,
    )>,
) {
    clear_active_login_document_inputs(&runtime, &mut input_values);
    ui_state.clear_runtime_state();
    focus_state.focused_entity = None;
}

impl LoginUiState {
    pub(super) fn clear_runtime_state(&mut self) {
        self.rendered = None;
        self.last_error = None;
        self.notice = None;
        self.entry_mode = AuthEntryMode::Login;
        self.registration_validation_error = None;
        self.registration_succeeded = false;
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
    let mut auth_request_sent = false;

    for action in actions.read() {
        if auth_request_sent || !is_login_business_action(action) {
            continue;
        }

        match action.action.as_str() {
            ACTION_ACCOUNT_LOGIN
                if action.source_node.as_str() == "login.submit" && action.params.is_empty() =>
            {
                if login_request_pending(&session) {
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
                auth_request_sent = true;
                session.begin_login();
                ui_state.last_error = None;
                ui_state.notice = None;
                ui_state.registration_validation_error = None;
                ui_state.registration_succeeded = false;
                myserver_commands.write(MyServerCommand::Login {
                    login_name,
                    password,
                    connect_game: false,
                });
            }
            ACTION_GUEST_LOGIN
                if action.source_node.as_str() == "login.guest" && action.params.is_empty() =>
            {
                if login_request_pending(&session) {
                    continue;
                }
                auth_request_sent = true;
                session.begin_login();
                ui_state.last_error = None;
                ui_state.notice = None;
                ui_state.registration_validation_error = None;
                ui_state.registration_succeeded = false;
                myserver_commands.write(MyServerCommand::GuestLogin {
                    guest_id: None,
                    connect_game: false,
                });
            }
            ACTION_SHOW_REGISTRATION
                if action.source_node.as_str() == "login.mode.register"
                    && action.params.is_empty()
                    && !login_request_pending(&session) =>
            {
                ui_state.entry_mode = AuthEntryMode::Register;
                clear_registration_feedback(&mut ui_state);
            }
            ACTION_SHOW_LOGIN
                if action.source_node.as_str() == "login.mode.login"
                    && action.params.is_empty()
                    && !login_request_pending(&session) =>
            {
                ui_state.entry_mode = AuthEntryMode::Login;
                clear_registration_feedback(&mut ui_state);
            }
            ACTION_REGISTER
                if action.source_node.as_str() == "login.register.submit"
                    && action.params.is_empty()
                    && ui_state.entry_mode == AuthEntryMode::Register
                    && !login_request_pending(&session) =>
            {
                let Some((login_name, password, password_confirmation)) =
                    document_registration_credentials(instance_id, &runtime, &mut input_values)
                else {
                    continue;
                };
                let registration = match validate_registration_request(
                    &login_name,
                    &password,
                    &password_confirmation,
                ) {
                    Ok(registration) => registration,
                    Err(error) => {
                        ui_state.registration_validation_error = Some(error);
                        ui_state.last_error = None;
                        ui_state.notice = None;
                        continue;
                    }
                };
                auth_request_sent = true;
                session.begin_registration();
                ui_state.last_error = None;
                ui_state.notice = None;
                ui_state.registration_validation_error = None;
                ui_state.registration_succeeded = false;
                myserver_commands.write(MyServerCommand::Register {
                    login_name: registration.login_name,
                    password: registration.password,
                    connect_game: false,
                });
            }
            ACTION_DISMISS_REGISTRATION_REVIEW
                if action.source_node.as_str() == "login.registration.back"
                    && action.params.is_empty()
                    && session.registration_state == RegistrationState::PendingReview =>
            {
                clear_auth_document_inputs(instance_id, &runtime, &mut input_values);
                focus_state.focused_entity = None;
                ui_state.entry_mode = AuthEntryMode::Login;
                clear_registration_feedback(&mut ui_state);
                myserver_commands.write(MyServerCommand::DismissRegistrationReview);
            }
            ACTION_SWITCH_ENVIRONMENT if action.source_node.as_str() == "login.environment" => {
                let Some(environment) = login_environment_param(action) else {
                    continue;
                };
                if login_request_pending(&session)
                    || environment == profiles.selected()
                    || !profiles.try_activate(environment, config.as_mut(), session.as_mut())
                {
                    continue;
                }
                clear_auth_document_inputs(instance_id, &runtime, &mut input_values);
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

fn document_registration_credentials(
    instance_id: crate::framework::ui::document::UiDocumentInstanceId,
    runtime: &UiDocumentRuntime,
    input_values: &mut Query<(
        &UiDocumentNodeMarker,
        &mut UiTextInputValue,
        Has<UiSensitiveTextInput>,
    )>,
) -> Option<(String, String, String)> {
    let account_id = UiNodeId::from_str(REGISTRATION_ACCOUNT_NODE).ok()?;
    let password_id = UiNodeId::from_str(REGISTRATION_PASSWORD_NODE).ok()?;
    let confirmation_id = UiNodeId::from_str(REGISTRATION_PASSWORD_CONFIRMATION_NODE).ok()?;
    let account_entity = runtime.node_entity(instance_id, &account_id)?;
    let password_entity = runtime.node_entity(instance_id, &password_id)?;
    let confirmation_entity = runtime.node_entity(instance_id, &confirmation_id)?;

    let login_name = read_document_input(
        instance_id,
        account_id,
        account_entity,
        false,
        input_values,
        true,
    )?;
    let password = read_document_input(
        instance_id,
        password_id,
        password_entity,
        true,
        input_values,
        false,
    )?;
    let password_confirmation = read_document_input(
        instance_id,
        confirmation_id,
        confirmation_entity,
        true,
        input_values,
        false,
    )?;
    Some((login_name, password, password_confirmation))
}

fn read_document_input(
    instance_id: crate::framework::ui::document::UiDocumentInstanceId,
    node_id: UiNodeId,
    entity: Entity,
    expected_sensitive: bool,
    input_values: &mut Query<(
        &UiDocumentNodeMarker,
        &mut UiTextInputValue,
        Has<UiSensitiveTextInput>,
    )>,
    trim: bool,
) -> Option<String> {
    let (marker, value, is_sensitive) = input_values.get_mut(entity).ok()?;
    if marker.instance_id != instance_id
        || marker.node_id != node_id
        || is_sensitive != expected_sensitive
    {
        return None;
    }
    Some(if trim {
        value.0.trim().to_owned()
    } else {
        value.0.clone()
    })
}

fn clear_auth_document_inputs(
    instance_id: crate::framework::ui::document::UiDocumentInstanceId,
    runtime: &UiDocumentRuntime,
    input_values: &mut Query<(
        &UiDocumentNodeMarker,
        &mut UiTextInputValue,
        Has<UiSensitiveTextInput>,
    )>,
) {
    for (node, expected_sensitive) in [
        (LOGIN_ACCOUNT_NODE, false),
        (LOGIN_PASSWORD_NODE, true),
        (REGISTRATION_ACCOUNT_NODE, false),
        (REGISTRATION_PASSWORD_NODE, true),
        (REGISTRATION_PASSWORD_CONFIRMATION_NODE, true),
    ] {
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

fn clear_registration_feedback(ui_state: &mut LoginUiState) {
    ui_state.last_error = None;
    ui_state.notice = None;
    ui_state.registration_validation_error = None;
    ui_state.registration_succeeded = false;
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
    let (registration_error_title, registration_error_detail, registration_error_visible) =
        registration_error_bindings(&ui_state);
    let registration_state = registration_state_binding(&session, &ui_state);

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
            "auth.login.error_display",
            UiBindingValue::Enum(
                if snapshot.last_error.is_some() {
                    "flex"
                } else {
                    "none"
                }
                .to_owned(),
            ),
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
            "auth.login.notice_display",
            UiBindingValue::Enum(
                if snapshot.notice.is_some() {
                    "flex"
                } else {
                    "none"
                }
                .to_owned(),
            ),
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
        (
            "auth.login.mode",
            UiBindingValue::Enum(ui_state.entry_mode.binding_value().to_owned()),
        ),
        (
            "auth.login.login_display",
            UiBindingValue::Enum(
                if ui_state.entry_mode == AuthEntryMode::Login {
                    "flex"
                } else {
                    "none"
                }
                .to_owned(),
            ),
        ),
        (
            "auth.login.register_display",
            UiBindingValue::Enum(
                if ui_state.entry_mode == AuthEntryMode::Register {
                    "flex"
                } else {
                    "none"
                }
                .to_owned(),
            ),
        ),
        (
            "auth.register.state",
            UiBindingValue::Enum(registration_state.to_owned()),
        ),
        (
            "auth.register.request_pending",
            UiBindingValue::Bool(request_pending),
        ),
        ("auth.register.disabled", UiBindingValue::Bool(disabled)),
        (
            "auth.register.error_title",
            UiBindingValue::String(registration_error_title),
        ),
        (
            "auth.register.error_detail",
            UiBindingValue::String(registration_error_detail),
        ),
        (
            "auth.register.error_display",
            UiBindingValue::Enum(
                if registration_error_visible {
                    "flex"
                } else {
                    "none"
                }
                .to_owned(),
            ),
        ),
        (
            "auth.register.review_display",
            UiBindingValue::Enum(
                if session.registration_state == RegistrationState::PendingReview {
                    "flex"
                } else {
                    "none"
                }
                .to_owned(),
            ),
        ),
        (
            "auth.register.success_display",
            UiBindingValue::Enum(
                if ui_state.registration_succeeded {
                    "flex"
                } else {
                    "none"
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

fn registration_state_binding(session: &MyServerSession, ui_state: &LoginUiState) -> &'static str {
    match session.registration_state {
        RegistrationState::Registering => "registering",
        RegistrationState::Failed => "failed",
        RegistrationState::PendingReview => "pending_review",
        RegistrationState::Idle if ui_state.registration_validation_error.is_some() => "failed",
        RegistrationState::Idle if ui_state.registration_succeeded => "succeeded",
        RegistrationState::Idle => "idle",
    }
}

fn registration_error_bindings(ui_state: &LoginUiState) -> (String, String, bool) {
    if let Some(error) = ui_state.registration_validation_error.as_ref() {
        return (
            "Registration failed".to_owned(),
            error.message_key().to_owned(),
            true,
        );
    }

    let Some(error) = ui_state.last_error.as_ref().filter(|error| {
        error.operation == Some(crate::game::myserver::MyServerOperation::Register)
    }) else {
        return (String::new(), String::new(), false);
    };
    let detail = error
        .error_code
        .as_deref()
        .and_then(RegistrationServerError::from_error_code)
        .map(|error| error.message_key().to_owned())
        .unwrap_or_else(|| error.message_key.to_owned());
    ("Registration failed".to_owned(), detail, true)
}

pub(super) fn handle_character_select_document_actions(
    mut actions: MessageReader<UiActionDispatch>,
    runtime: Res<UiDocumentRuntime>,
    mut input_values: Query<(
        &UiDocumentNodeMarker,
        &mut UiTextInputValue,
        Has<UiSensitiveTextInput>,
    )>,
    session: Res<MyServerSession>,
    mut ui_state: ResMut<LoginUiState>,
    mut focus_state: ResMut<UiFocusState>,
    mut myserver_commands: MessageWriter<MyServerCommand>,
) {
    let Some(instance_id) = runtime.active_instance(
        OWNER_CHARACTER_SELECT.as_str(),
        &UiDocumentId::from_str(CHARACTER_SELECT_DOCUMENT_ID)
            .expect("CharacterSelect document ID is static and valid"),
    ) else {
        for _ in actions.read() {}
        return;
    };
    let mut request_sent = false;

    for action in actions.read() {
        if request_sent || !is_character_select_business_action(action) {
            continue;
        }

        match action.action.as_str() {
            ACTION_LOAD_CHARACTERS
                if action.source_node.as_str() == "character.reload"
                    && action.params.is_empty()
                    && can_send_character_request(&session) =>
            {
                request_sent = true;
                clear_character_feedback(&mut ui_state);
                myserver_commands.write(MyServerCommand::LoadCharacterList);
            }
            ACTION_CREATE_CHARACTER
                if action.source_node.as_str() == "character.create"
                    && action.params.is_empty()
                    && can_send_character_request(&session) =>
            {
                let Some(name) = document_character_name(instance_id, &runtime, &mut input_values)
                else {
                    continue;
                };
                if name.is_empty() || name.len() > CHARACTER_NAME_MAX_BYTES {
                    continue;
                }
                request_sent = true;
                clear_character_feedback(&mut ui_state);
                myserver_commands.write(MyServerCommand::CreateCharacter {
                    name,
                    appearance_json: None,
                });
            }
            ACTION_SELECT_CHARACTER
                if action.source_node.as_str() == "character.row.select"
                    && can_send_character_request(&session) =>
            {
                let Some(character_id) = selected_character_id(action) else {
                    continue;
                };
                if !session
                    .characters
                    .iter()
                    .any(|character| character.character_id == character_id)
                {
                    continue;
                }
                request_sent = true;
                clear_character_feedback(&mut ui_state);
                myserver_commands.write(MyServerCommand::SelectCharacter {
                    character_id,
                    connect_game: true,
                });
            }
            ACTION_SWITCH_ACCOUNT
                if action.source_node.as_str() == "character.switch_account"
                    && action.params.is_empty()
                    && !login_request_pending(&session) =>
            {
                request_sent = true;
                clear_character_document_input(instance_id, &runtime, &mut input_values);
                focus_state.focused_entity = None;
                ui_state.clear_runtime_state();
                myserver_commands.write(MyServerCommand::Logout);
            }
            ACTION_SWITCH_CHARACTER
                if action.source_node.as_str() == "character.switch_character"
                    && action.params.is_empty()
                    && can_change_character(&session) =>
            {
                request_sent = true;
                clear_character_document_input(instance_id, &runtime, &mut input_values);
                clear_character_feedback(&mut ui_state);
                myserver_commands.write(MyServerCommand::SwitchCharacter);
            }
            _ => {}
        }
    }
}

fn is_character_select_business_action(action: &UiActionDispatch) -> bool {
    action.document_id.as_str() == CHARACTER_SELECT_DOCUMENT_ID
        && action.owner == OWNER_CHARACTER_SELECT.as_str()
        && matches!(
            &action.kind,
            UiRegisteredActionKind::BusinessCommand { target }
                if target == action.action.as_str()
        )
}

fn selected_character_id(action: &UiActionDispatch) -> Option<String> {
    if action.params.len() != 1 {
        return None;
    }
    match action.params.get("character_id") {
        Some(UiActionValue::String(character_id))
            if !character_id.is_empty() && character_id.len() <= CHARACTER_ID_MAX_BYTES =>
        {
            Some(character_id.clone())
        }
        _ => None,
    }
}

fn document_character_name(
    instance_id: crate::framework::ui::document::UiDocumentInstanceId,
    runtime: &UiDocumentRuntime,
    input_values: &mut Query<(
        &UiDocumentNodeMarker,
        &mut UiTextInputValue,
        Has<UiSensitiveTextInput>,
    )>,
) -> Option<String> {
    let node_id = UiNodeId::from_str(CHARACTER_CREATE_NAME_NODE).ok()?;
    let entity = runtime.node_entity(instance_id, &node_id)?;
    let (marker, value, is_sensitive) = input_values.get_mut(entity).ok()?;
    if marker.instance_id != instance_id || marker.node_id != node_id || is_sensitive {
        return None;
    }
    Some(value.0.trim().to_owned())
}

fn clear_character_document_input(
    instance_id: crate::framework::ui::document::UiDocumentInstanceId,
    runtime: &UiDocumentRuntime,
    input_values: &mut Query<(
        &UiDocumentNodeMarker,
        &mut UiTextInputValue,
        Has<UiSensitiveTextInput>,
    )>,
) {
    let Ok(node_id) = UiNodeId::from_str(CHARACTER_CREATE_NAME_NODE) else {
        return;
    };
    let Some(entity) = runtime.node_entity(instance_id, &node_id) else {
        return;
    };
    let Ok((marker, mut value, is_sensitive)) = input_values.get_mut(entity) else {
        return;
    };
    if marker.instance_id == instance_id && marker.node_id == node_id && !is_sensitive {
        value.0.clear();
    }
}

fn clear_character_feedback(ui_state: &mut LoginUiState) {
    ui_state.last_error = None;
    ui_state.notice = None;
}

pub(super) fn sync_character_select_document_bindings(
    session: Res<MyServerSession>,
    i18n: Res<UiI18n>,
    mut ui_state: ResMut<LoginUiState>,
    contracts: Res<AuthHostContracts>,
    mut binding_values: ResMut<UiBindingValues>,
) {
    let snapshot = LoginUiSnapshot::from_session(
        &session,
        ui_state.last_error.as_ref(),
        ui_state.notice.as_ref(),
    );
    let request_pending = character_request_pending(&session);
    let selecting = session.character_selection_state
        == crate::game::myserver::CharacterSelectionState::Selecting;
    let items = snapshot
        .characters
        .iter()
        .map(|character| {
            let selected =
                snapshot.character_id.as_deref() == Some(character.character_id.as_str());
            let pending = selecting
                && snapshot.pending_character_id.as_deref()
                    == Some(character.character_id.as_str());
            UiBindingValue::Record(BTreeMap::from([
                (
                    "character_id".to_owned(),
                    UiBindingValue::String(character.character_id.clone()),
                ),
                (
                    "display_name".to_owned(),
                    UiBindingValue::String(character.name.clone()),
                ),
                (
                    "detail".to_owned(),
                    UiBindingValue::String(localized_character_detail(character, &i18n)),
                ),
                ("selected".to_owned(), UiBindingValue::Bool(selected)),
                ("pending".to_owned(), UiBindingValue::Bool(pending)),
                (
                    "disabled".to_owned(),
                    UiBindingValue::Bool(request_pending || selected),
                ),
                ("loading".to_owned(), UiBindingValue::Bool(pending)),
            ]))
        })
        .collect();
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
    let element_snapshot = snapshot.element_snapshot;
    let profile_visible = session.character_selection_state
        == crate::game::myserver::CharacterSelectionState::Selected;

    for (path, value) in [
        (
            "auth.account.player_id",
            UiBindingValue::String(snapshot.player_id.clone().unwrap_or_default()),
        ),
        (
            "auth.account.summary",
            UiBindingValue::String(account_summary_text(&snapshot, &i18n)),
        ),
        (
            "auth.character.character_id",
            UiBindingValue::String(snapshot.character_id.clone().unwrap_or_default()),
        ),
        (
            "auth.character.pending_character_id",
            UiBindingValue::String(snapshot.pending_character_id.clone().unwrap_or_default()),
        ),
        ("auth.character.items", UiBindingValue::List(items)),
        (
            "auth.character.collection_state",
            UiBindingValue::Enum(character_collection_state(&snapshot).to_owned()),
        ),
        (
            "auth.character.view_state",
            UiBindingValue::Enum(character_view_state(&snapshot).to_owned()),
        ),
        (
            "auth.character.status",
            UiBindingValue::String(localized_character_status_text(&snapshot, &i18n)),
        ),
        (
            "auth.character.connection_status",
            UiBindingValue::String(localized_connection_status_text(&snapshot, &i18n)),
        ),
        (
            "auth.character.request_pending",
            UiBindingValue::Bool(request_pending),
        ),
        (
            "auth.character.load_disabled",
            UiBindingValue::Bool(!can_send_character_request(&session)),
        ),
        (
            "auth.character.load_loading",
            UiBindingValue::Bool(
                session.character_selection_state
                    == crate::game::myserver::CharacterSelectionState::Loading,
            ),
        ),
        (
            "auth.character.create_disabled",
            UiBindingValue::Bool(!can_send_character_request(&session)),
        ),
        (
            "auth.character.create_loading",
            UiBindingValue::Bool(
                session.character_selection_state
                    == crate::game::myserver::CharacterSelectionState::Creating,
            ),
        ),
        (
            "auth.character.switch_account_disabled",
            UiBindingValue::Bool(login_request_pending(&session)),
        ),
        (
            "auth.character.switch_character_disabled",
            UiBindingValue::Bool(!can_change_character(&session)),
        ),
        (
            "auth.character.switch_character_visibility",
            UiBindingValue::Enum(if profile_visible { "flex" } else { "none" }.to_owned()),
        ),
        (
            "auth.character.error_title",
            UiBindingValue::String(error_title),
        ),
        (
            "auth.character.error_detail",
            UiBindingValue::String(error_detail),
        ),
        (
            "auth.character.error_visibility",
            UiBindingValue::Enum(
                if snapshot.last_error.is_some() {
                    "flex"
                } else {
                    "none"
                }
                .to_owned(),
            ),
        ),
        (
            "auth.character.notice_title",
            UiBindingValue::String(notice_title),
        ),
        (
            "auth.character.notice_detail",
            UiBindingValue::String(notice_detail),
        ),
        (
            "auth.character.notice_visibility",
            UiBindingValue::Enum(
                if snapshot.notice.is_some() {
                    "flex"
                } else {
                    "none"
                }
                .to_owned(),
            ),
        ),
        (
            "auth.character.profile_visibility",
            UiBindingValue::Enum(if profile_visible { "flex" } else { "none" }.to_owned()),
        ),
        (
            "auth.character.profile_title",
            UiBindingValue::String(
                snapshot
                    .selected_character_name
                    .as_ref()
                    .map(|name| {
                        format!(
                            "{name} · {}",
                            i18n.tr("auth.character.profile", "Character profile")
                        )
                    })
                    .unwrap_or_else(|| i18n.tr("auth.character.profile", "Character profile")),
            ),
        ),
        (
            "auth.character.affinity",
            UiBindingValue::String(
                element_snapshot
                    .map(|elements| {
                        format!(
                            "{}: {}",
                            i18n.tr("auth.character.affinity", "Affinity"),
                            localized_element_values(elements.affinity, &i18n)
                        )
                    })
                    .unwrap_or_else(|| {
                        i18n.tr(
                            "auth.character.affinity_unavailable",
                            "Affinity data unavailable",
                        )
                    }),
            ),
        ),
        (
            "auth.character.mastery",
            UiBindingValue::String(
                element_snapshot
                    .map(|elements| {
                        format!(
                            "{}: {}",
                            i18n.tr("auth.character.mastery", "Mastery"),
                            localized_element_values(elements.mastery, &i18n)
                        )
                    })
                    .unwrap_or_else(|| {
                        i18n.tr(
                            "auth.character.mastery_unavailable",
                            "Mastery data unavailable",
                        )
                    }),
            ),
        ),
    ] {
        let path = UiBindingPath::from_str(path)
            .expect("CharacterSelect binding paths are static and valid");
        let declaration = contracts
            .character_select_bindings
            .get(&path)
            .expect("CharacterSelect binding schema contains every synchronized value");
        binding_values.set_scoped(
            CHARACTER_SELECT_DOCUMENT_ID,
            OWNER_CHARACTER_SELECT.as_str(),
            &path,
            declaration,
            value,
        );
    }
    ui_state.rendered = Some(snapshot);
}

pub(super) fn account_summary_text(snapshot: &LoginUiSnapshot, i18n: &UiI18n) -> String {
    if let Some(login_name) = snapshot.login_name.as_deref() {
        i18n.tr_args(
            "auth.character.account_summary",
            "Account {account_name}",
            [("account_name", login_name)],
        )
    } else if let Some(guest_id) = snapshot.guest_id.as_deref() {
        i18n.tr_args(
            "auth.character.guest_summary",
            "Guest {account_name}",
            [("account_name", guest_id)],
        )
    } else if let Some(player_id) = snapshot.player_id.as_deref() {
        i18n.tr_args(
            "auth.character.player_summary",
            "Player {account_name}",
            [("account_name", player_id)],
        )
    } else {
        i18n.tr("auth.character.account_unavailable", "Account unavailable")
    }
}

pub(super) fn localized_character_detail(
    character: &CharacterRowSnapshot,
    i18n: &UiI18n,
) -> String {
    let world = character
        .world_id
        .map(|world_id| format!("{} {world_id}", i18n.tr("auth.character.world", "World")))
        .unwrap_or_else(|| i18n.tr("auth.character.world_unknown", "World unknown"));
    let status = if character.status.eq_ignore_ascii_case("active") {
        i18n.tr("auth.character.status.active", "active")
    } else {
        character.status.clone()
    };
    format!("{} · {world} · {status}", character.discriminator)
}

pub(super) fn localized_character_status_text(snapshot: &LoginUiSnapshot, i18n: &UiI18n) -> String {
    use crate::game::myserver::CharacterSelectionState;
    let (key, fallback) = match snapshot.character_state {
        CharacterSelectionState::NotLoaded => ("not_loaded", "Characters not loaded"),
        CharacterSelectionState::Loading => ("loading", "Loading characters..."),
        CharacterSelectionState::NoCharacters => ("empty", "Create a character to continue"),
        CharacterSelectionState::Creating => ("creating", "Creating character..."),
        CharacterSelectionState::AwaitingSelection => ("choose", "Choose a character"),
        CharacterSelectionState::LoadingProfile => ("loading_profile", "Loading profile..."),
        CharacterSelectionState::Selecting => ("selecting", "Selecting character..."),
        CharacterSelectionState::Selected => {
            return snapshot
                .selected_character_name
                .as_deref()
                .map(|name| {
                    format!(
                        "{} {name}",
                        i18n.tr("auth.character.status.selected", "Selected")
                    )
                })
                .unwrap_or_else(|| {
                    i18n.tr(
                        "auth.character.status.selected_default",
                        "Character selected",
                    )
                });
        }
        CharacterSelectionState::Blocked => ("unavailable", "Character unavailable"),
        CharacterSelectionState::SelectionFailed => ("failed", "Character request failed"),
    };
    i18n.tr(&format!("auth.character.status.{key}"), fallback)
}

pub(super) fn localized_connection_status_text(
    snapshot: &LoginUiSnapshot,
    i18n: &UiI18n,
) -> String {
    use crate::game::myserver::GameConnectionState;
    let (key, fallback) = match snapshot.connection_state {
        GameConnectionState::NotConnected => ("not_connected", "Game server not connected"),
        GameConnectionState::Connecting => ("connecting", "Connecting to game server..."),
        GameConnectionState::Connected => ("connected", "Game server connected"),
        GameConnectionState::Authenticating => ("authenticating", "Signing in to game server..."),
        GameConnectionState::Authenticated => ("authenticated", "Game server authenticated"),
        GameConnectionState::Disconnected => ("disconnected", "Game server disconnected"),
        GameConnectionState::Reconnecting => ("reconnecting", "Refreshing ticket..."),
        GameConnectionState::ReconnectFailed => ("failed", "Network or ticket request failed"),
    };
    i18n.tr(&format!("auth.character.connection.{key}"), fallback)
}

fn localized_element_values(values: crate::game::myserver::ElementValues, i18n: &UiI18n) -> String {
    format!(
        "{} {} / {} {} / {} {} / {} {}",
        i18n.tr("auth.character.element.earth", "earth"),
        values.earth,
        i18n.tr("auth.character.element.fire", "fire"),
        values.fire,
        i18n.tr("auth.character.element.water", "water"),
        values.water,
        i18n.tr("auth.character.element.wind", "wind"),
        values.wind
    )
}

fn character_collection_state(snapshot: &LoginUiSnapshot) -> &'static str {
    use crate::game::myserver::CharacterSelectionState;
    match snapshot.character_state {
        CharacterSelectionState::NotLoaded | CharacterSelectionState::Loading => "loading",
        CharacterSelectionState::Blocked | CharacterSelectionState::SelectionFailed => "error",
        _ => "ready",
    }
}

fn character_view_state(snapshot: &LoginUiSnapshot) -> &'static str {
    use crate::game::myserver::CharacterSelectionState;
    match snapshot.character_state {
        CharacterSelectionState::NotLoaded
        | CharacterSelectionState::Loading
        | CharacterSelectionState::LoadingProfile => "loading",
        CharacterSelectionState::NoCharacters => "empty",
        CharacterSelectionState::Creating => "creating",
        CharacterSelectionState::Selecting => "selecting",
        CharacterSelectionState::Selected => "current",
        CharacterSelectionState::Blocked | CharacterSelectionState::SelectionFailed => "error",
        CharacterSelectionState::AwaitingSelection if snapshot.characters.is_empty() => "empty",
        CharacterSelectionState::AwaitingSelection => "ready",
    }
}

pub(super) fn follow_myserver_login_events(
    mut events: MessageReader<MyServerEvent>,
    mut commands: MessageWriter<MyServerCommand>,
    mut route_commands: MessageWriter<GameRouteCommand>,
    runtime: Res<UiDocumentRuntime>,
    mut ui_state: ResMut<LoginUiState>,
    mut focus_state: ResMut<UiFocusState>,
    mut input_values: Query<(
        &UiDocumentNodeMarker,
        &mut UiTextInputValue,
        Has<UiSensitiveTextInput>,
    )>,
) {
    for event in events.read() {
        match event {
            MyServerEvent::DisplayError { error } => {
                if error.operation == Some(crate::game::myserver::MyServerOperation::Register) {
                    ui_state.registration_succeeded = false;
                }
                ui_state.last_error = Some(error.clone());
                ui_state.notice = None;
            }
            MyServerEvent::LoginSucceeded(_) => {
                ui_state.registration_succeeded = ui_state.entry_mode == AuthEntryMode::Register;
                ui_state.last_error = None;
                ui_state.notice = None;
                ui_state.registration_validation_error = None;
                clear_active_login_document_inputs(&runtime, &mut input_values);
                commands.write(MyServerCommand::LoadCharacterList);
                route_commands.write(GameRouteCommand::ChangeMode(AppUiMode::CharacterSelect));
            }
            MyServerEvent::LoginFailed { error } => {
                ui_state.notice = Some(AuthStatusNotice::generic_failure(
                    "Login failed",
                    Some(error.clone()),
                ));
            }
            MyServerEvent::RegistrationPendingReview { message, .. } => {
                ui_state.entry_mode = AuthEntryMode::Register;
                ui_state.registration_succeeded = false;
                ui_state.notice = Some(AuthStatusNotice {
                    kind: AuthNoticeKind::PendingReview,
                    title: "Registration requires review".to_string(),
                    detail: Some(message.clone()),
                });
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
                clear_active_character_document_input(&runtime, &mut input_values);
                focus_state.focused_entity = None;
                ui_state.clear_runtime_state();
                route_commands.write(GameRouteCommand::ChangeMode(AppUiMode::Lobby));
            }
            MyServerEvent::LogoutSucceeded => {
                clear_active_login_document_inputs(&runtime, &mut input_values);
                clear_active_character_document_input(&runtime, &mut input_values);
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

fn clear_active_character_document_input(
    runtime: &UiDocumentRuntime,
    input_values: &mut Query<(
        &UiDocumentNodeMarker,
        &mut UiTextInputValue,
        Has<UiSensitiveTextInput>,
    )>,
) {
    let Ok(document_id) = UiDocumentId::from_str(CHARACTER_SELECT_DOCUMENT_ID) else {
        return;
    };
    let Some(instance_id) = runtime.active_instance(OWNER_CHARACTER_SELECT.as_str(), &document_id)
    else {
        return;
    };
    clear_character_document_input(instance_id, runtime, input_values);
}

fn clear_active_login_document_inputs(
    runtime: &UiDocumentRuntime,
    input_values: &mut Query<(
        &UiDocumentNodeMarker,
        &mut UiTextInputValue,
        Has<UiSensitiveTextInput>,
    )>,
) {
    let Some(instance_id) = runtime.active_instance(
        OWNER_LOGIN.as_str(),
        &UiDocumentId::from_str(LOGIN_DOCUMENT_ID).expect("Login document ID is static and valid"),
    ) else {
        return;
    };
    clear_auth_document_inputs(instance_id, runtime, input_values);
}
