use super::{host::*, model::*};
use crate::framework::network::RequestId;
use crate::framework::ui::{
    core::{UiMetrics, UiViewport, binding::UiBindingValues, focus::UiFocusState},
    document::{
        UiActionDispatch, UiActionId, UiActionValue, UiBindingPath, UiBindingScope, UiBindingType,
        UiBindingValue, UiBindingVisibility, UiDocument, UiDocumentAssetPreflightOverrides,
        UiDocumentAssetPreflightStatus, UiDocumentInputMode, UiDocumentLayer, UiDocumentNodeMarker,
        UiDocumentOpenRequest, UiDocumentOpenSource, UiDocumentPanel, UiDocumentPlatform,
        UiDocumentPreviewPlugin, UiDocumentReloadEvent, UiDocumentRequestId, UiDocumentRuntime,
        UiDocumentRuntimeCommand, UiDocumentRuntimePlugin, UiDocumentRuntimeSystems,
        UiDocumentSourceOrigin, UiNode, UiNodeId, UiPageState, UiRegisteredActionKind,
        UiSafeAreaClass, UiTargetProfile, UiTextInputSecurity,
        parse_approved_document_registration,
    },
    style::{UiFontAssets, UiTheme},
    widgets::{UiButtonEvent, UiButtonEventKind, UiSensitiveTextInput, UiTextInputValue},
};
use crate::game::{
    declarative_screen::DeclarativeScreenHostPlugin,
    myserver::{
        AccountLoginState, CharacterSelectionState, CharacterSummary, ElementValues,
        GameConnectionState, MyServerCommand, MyServerConfig, MyServerDisplayError,
        MyServerEnvironment, MyServerErrorKind, MyServerErrorSource, MyServerEvent,
        MyServerOperation, MyServerProfiles, MyServerSession,
    },
    navigation::{AppUiMode, GameRouteCommand},
    ui_ids::{OWNER_CHARACTER_SELECT, OWNER_LOGIN, PANEL_CHARACTER_SELECT, PANEL_LOGIN},
};
use bevy::{
    ecs::{message::MessageCursor, system::RunSystemOnce},
    prelude::*,
    state::app::StatesPlugin,
};
use std::time::SystemTime;
use std::{
    collections::{BTreeMap, HashMap},
    str::FromStr,
};

#[test]
fn login_document_is_valid_and_keeps_password_out_of_contract_surfaces() {
    let result = UiDocument::validate_json(LOGIN_DOCUMENT_SOURCE);
    assert!(result.report.valid, "{:#?}", result.report.diagnostics);
    let document = result.validated().unwrap().document();
    let account = find_document_node(&document.root, LOGIN_ACCOUNT_NODE).unwrap();
    let password = find_document_node(&document.root, LOGIN_PASSWORD_NODE).unwrap();

    let UiNode::TextInput {
        component,
        security,
        value,
        on_change,
        on_submit,
        ..
    } = password
    else {
        panic!("password must remain a text input");
    };
    assert_eq!(*security, UiTextInputSecurity::Sensitive);
    assert!(value.is_empty());
    assert!(component.bindings.value.is_none());
    assert!(on_change.is_none());
    assert!(on_submit.is_none());

    let UiNode::TextInput { component, .. } = account else {
        panic!("account must remain a text input");
    };
    assert_eq!(
        component
            .bindings
            .value
            .as_ref()
            .unwrap()
            .binding_path
            .as_str(),
        "auth.login.login_name"
    );
    assert!(!LOGIN_DOCUMENT_SOURCE.contains("auth.login.password"));
}

#[test]
fn login_document_resolves_reference_and_keyboard_height_layouts() {
    let validated = UiDocument::validate_json(LOGIN_DOCUMENT_SOURCE)
        .into_validated()
        .unwrap();
    let regular = UiTargetProfile::new(
        1376.0,
        768.0,
        UiSafeAreaClass::None,
        UiDocumentInputMode::MouseKeyboard,
        UiDocumentPlatform::Windows,
    )
    .unwrap();
    let keyboard = UiTargetProfile::new(
        1376.0,
        320.0,
        UiSafeAreaClass::Inset,
        UiDocumentInputMode::Touch,
        UiDocumentPlatform::Android,
    )
    .unwrap();
    let regular = validated
        .effective_document(&regular, &UiPageState::initial())
        .unwrap();
    let keyboard = validated
        .effective_document(&keyboard, &UiPageState::initial())
        .unwrap();
    let regular_panel = find_document_node(&regular.document.root, "login.panel").unwrap();
    let keyboard_panel = find_document_node(&keyboard.document.root, "login.panel").unwrap();

    assert_eq!(regular_panel.layout().max_width, px_length(384.0));
    assert_eq!(keyboard_panel.layout().max_width, px_length(760.0));
    assert!(
        keyboard
            .applied_overrides
            .iter()
            .any(|item| item.source_id == "short_landscape")
    );
}

#[test]
fn auth_page_baselines_freeze_owner_panel_source_and_states() {
    assert_eq!(LOGIN_PAGE_BASELINE.mode, AppUiMode::Login);
    assert_eq!(LOGIN_PAGE_BASELINE.owner, OWNER_LOGIN);
    assert_eq!(LOGIN_PAGE_BASELINE.panel, PANEL_LOGIN);
    assert_eq!(
        LOGIN_PAGE_BASELINE.action_source,
        AuthActionSource::UiDocument
    );
    assert!(LOGIN_PAGE_BASELINE.states.contains(&"logging_in"));

    assert_eq!(
        CHARACTER_SELECT_PAGE_BASELINE.mode,
        AppUiMode::CharacterSelect
    );
    assert_eq!(CHARACTER_SELECT_PAGE_BASELINE.owner, OWNER_CHARACTER_SELECT);
    assert_eq!(CHARACTER_SELECT_PAGE_BASELINE.panel, PANEL_CHARACTER_SELECT);
    assert_eq!(
        CHARACTER_SELECT_PAGE_BASELINE.action_source,
        AuthActionSource::RustView
    );
    assert!(CHARACTER_SELECT_PAGE_BASELINE.states.contains(&"selecting"));
}

fn find_document_node<'a>(node: &'a UiNode, id: &str) -> Option<&'a UiNode> {
    if node.id().as_str() == id {
        return Some(node);
    }
    node.children()
        .iter()
        .find_map(|child| find_document_node(child, id))
}

fn px_length(value: f32) -> crate::framework::ui::document::UiLength {
    crate::framework::ui::document::UiLength::Px(value)
}

#[test]
fn auth_host_declares_all_closed_business_actions() {
    let descriptors = auth_action_descriptors();
    let actual = descriptors
        .iter()
        .map(|descriptor| descriptor.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let expected = [
        ACTION_ACCOUNT_LOGIN,
        ACTION_GUEST_LOGIN,
        ACTION_SWITCH_ENVIRONMENT,
        ACTION_LOAD_CHARACTERS,
        ACTION_CREATE_CHARACTER,
        ACTION_SELECT_CHARACTER,
        ACTION_SWITCH_ACCOUNT,
        ACTION_SWITCH_CHARACTER,
    ]
    .into_iter()
    .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(actual, expected);

    let select = descriptors
        .iter()
        .find(|descriptor| descriptor.id == UiActionId::from_str(ACTION_SELECT_CHARACTER).unwrap())
        .unwrap();
    assert_eq!(select.owner, OWNER_CHARACTER_SELECT.as_str());
    assert!(select.params.contains_key("character_id"));
    assert!(!select.params.contains_key("name"));

    let account_login = descriptors
        .iter()
        .find(|descriptor| descriptor.id.as_str() == ACTION_ACCOUNT_LOGIN)
        .unwrap();
    assert!(account_login.params.is_empty());
    let descriptor_debug = format!("{account_login:?}");
    assert!(!descriptor_debug.contains("password"));
    assert!(!descriptor_debug.contains("login_name"));
}

#[test]
fn login_approved_registration_matches_fixed_host_contract() {
    const REGISTRATION_SOURCE: &str =
        include_str!("../../../../assets/ui/documents/approved/auth/promotion.v1.json");
    let contracts = AuthHostContracts::default();
    let host = login_declarative_screen_host(&contracts);
    let registration = parse_approved_document_registration(REGISTRATION_SOURCE).unwrap();
    let audit = registration.audit_report(LOGIN_DOCUMENT_SOURCE).unwrap();

    assert_eq!(host.document_id.as_str(), LOGIN_DOCUMENT_ID);
    assert_eq!(host.mode, Some(AppUiMode::Login));
    assert_eq!(host.owner, OWNER_LOGIN);
    assert_eq!(host.binding_schema.len(), 12);
    assert_eq!(host.action_allowlist.len(), 3);
    assert_eq!(registration.owner(), OWNER_LOGIN.as_str());
    assert_eq!(audit.actions.len(), 3);
    assert!(!REGISTRATION_SOURCE.contains("password"));
}

#[test]
fn login_startup_registration_mounts_initial_mode_document() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin))
        .init_state::<AppUiMode>()
        .insert_resource(UiTheme::default())
        .insert_resource(UiMetrics::default())
        .insert_resource(UiFontAssets::test_registry())
        .init_resource::<UiFocusState>()
        .init_resource::<UiViewport>()
        .init_resource::<AuthHostContracts>()
        .add_plugins((
            UiDocumentRuntimePlugin,
            UiDocumentPreviewPlugin,
            DeclarativeScreenHostPlugin,
        ))
        .add_systems(Startup, register_auth_contracts);
    app.world_mut()
        .resource_mut::<UiDocumentAssetPreflightOverrides>()
        .set(
            crate::framework::ui::document::UiDocumentId::from_str(LOGIN_DOCUMENT_ID).unwrap(),
            crate::framework::ui::document::UiAssetId::from_str("login_background").unwrap(),
            UiDocumentAssetPreflightStatus::Failed {
                code: "UI_DOCUMENT_TEST_OPTIONAL_BACKGROUND_FAILED".to_owned(),
            },
        );

    for _ in 0..6 {
        app.update();
    }

    let document_id =
        crate::framework::ui::document::UiDocumentId::from_str(LOGIN_DOCUMENT_ID).unwrap();
    let runtime = app.world().resource::<UiDocumentRuntime>();
    if runtime
        .active_instance(OWNER_LOGIN.as_str(), &document_id)
        .is_none()
    {
        let messages = app.world().resource::<Messages<UiDocumentReloadEvent>>();
        let mut cursor = MessageCursor::<UiDocumentReloadEvent>::default();
        panic!(
            "initial Login document did not commit: {:#?}",
            cursor.read(messages).collect::<Vec<_>>()
        );
    }
}

#[test]
fn auth_binding_schemas_keep_account_and_character_identity_separate() {
    let login = login_binding_schema();
    let player_id = UiBindingPath::from_str("auth.account.player_id").unwrap();
    let character_id = UiBindingPath::from_str("auth.character.character_id").unwrap();
    assert_eq!(login[&player_id].scope, UiBindingScope::Owner);
    assert!(!login.contains_key(&character_id));

    let character = character_select_binding_schema();
    assert_eq!(character[&player_id].scope, UiBindingScope::Owner);
    assert_eq!(character[&character_id].scope, UiBindingScope::Owner);
    let items = UiBindingPath::from_str("auth.character.items").unwrap();
    let UiBindingType::List { item, .. } = &character[&items].value_type else {
        panic!("character items must be a typed list");
    };
    let UiBindingType::Record { fields } = item.as_ref() else {
        panic!("character items must use typed records");
    };
    assert!(fields.contains_key("character_id"));
    assert!(fields.contains_key("name"));
}

#[test]
fn pure_auth_snapshot_derives_without_bevy_world() {
    let session = MyServerSession {
        player_id: Some("plr_account".to_owned()),
        character_id: Some("chr_gameplay".to_owned()),
        current_character: Some(test_character("chr_gameplay", "DisplayOnly")),
        ..Default::default()
    };
    let snapshot = LoginUiSnapshot::from_session(&session, None, None);

    assert_eq!(snapshot.player_id.as_deref(), Some("plr_account"));
    assert_eq!(snapshot.character_id.as_deref(), Some("chr_gameplay"));
    assert_eq!(
        snapshot.selected_character_name.as_deref(),
        Some("DisplayOnly")
    );
}

#[cfg(all(debug_assertions, not(target_os = "android")))]
#[test]
fn character_select_audit_fixture_ignores_default_or_non_target_run() {
    let mut session = MyServerSession::default();
    maybe_seed_character_select_audit_session(false, &mut session);

    assert_eq!(session.account_login_state, AccountLoginState::NotLoggedIn);
    assert!(session.player_id.is_none());
    assert!(session.characters.is_empty());
}

#[cfg(all(debug_assertions, not(target_os = "android")))]
#[test]
fn character_select_audit_fixture_uses_distinct_business_ids() {
    let mut session = MyServerSession::default();
    maybe_seed_character_select_audit_session(true, &mut session);

    assert_eq!(session.account_login_state, AccountLoginState::LoggedIn);
    assert_eq!(
        session.character_selection_state,
        CharacterSelectionState::AwaitingSelection
    );
    assert_eq!(session.player_id.as_deref(), Some("plr_audit_account"));
    assert_eq!(session.characters.len(), 2);
    assert!(
        session
            .characters
            .iter()
            .all(|character| character.character_id.starts_with("chr_audit_"))
    );
    assert!(session.character_id.is_none());
}

#[cfg(all(debug_assertions, not(target_os = "android")))]
#[test]
fn character_select_audit_fixture_does_not_replace_real_login() {
    let mut session = MyServerSession {
        account_login_state: AccountLoginState::LoggedIn,
        player_id: Some("plr_real".to_owned()),
        ..Default::default()
    };
    maybe_seed_character_select_audit_session(true, &mut session);

    assert_eq!(session.player_id.as_deref(), Some("plr_real"));
    assert!(session.characters.is_empty());
}

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
    let snapshot = LoginUiSnapshot::from_session(
        &session,
        ui_state.last_error.as_ref(),
        ui_state.notice.as_ref(),
    );

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

    let snapshot = LoginUiSnapshot::from_session(
        &session,
        ui_state.last_error.as_ref(),
        ui_state.notice.as_ref(),
    );

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
fn auth_login_document_sends_account_login_only_from_sensitive_ecs_input() {
    const SENTINEL: &str = "stage11-unique-password-sentinel";
    let mut app = login_document_test_app(MyServerSession::default());
    set_login_document_inputs(&mut app, "alice", SENTINEL);
    app.world_mut().write_message(login_document_dispatch(
        ACTION_ACCOUNT_LOGIN,
        "login.submit",
        BTreeMap::new(),
    ));
    app.update();

    let commands = read_messages::<MyServerCommand>(&app);
    assert!(commands.iter().any(|command| matches!(
        command,
        MyServerCommand::Login {
            login_name,
            password,
            connect_game: false,
        } if login_name == "alice" && password == SENTINEL
    )));
    assert!(!format!("{commands:?}").contains(SENTINEL));
    assert_eq!(
        app.world()
            .resource::<MyServerSession>()
            .account_login_state,
        AccountLoginState::LoggingIn
    );
}

#[test]
fn forged_duplicate_login_markers_cannot_override_or_clear_active_inputs() {
    const REAL_PASSWORD: &str = "stage11-real-password-sentinel";
    let mut login_app = login_document_test_app(MyServerSession::default());
    set_login_document_inputs(&mut login_app, "real-account", REAL_PASSWORD);
    let instance_id = login_document_instance(&login_app);
    login_app.world_mut().spawn((
        UiDocumentNodeMarker {
            instance_id,
            node_id: UiNodeId::from_str(LOGIN_ACCOUNT_NODE).unwrap(),
        },
        UiTextInputValue("forged-account".to_owned()),
    ));
    login_app.world_mut().spawn((
        UiDocumentNodeMarker {
            instance_id,
            node_id: UiNodeId::from_str(LOGIN_PASSWORD_NODE).unwrap(),
        },
        UiTextInputValue("forged-password".to_owned()),
        UiSensitiveTextInput,
    ));
    login_app.world_mut().write_message(login_document_dispatch(
        ACTION_ACCOUNT_LOGIN,
        "login.submit",
        BTreeMap::new(),
    ));
    login_app
        .world_mut()
        .run_system_once(handle_login_document_actions)
        .expect("login document action handler should run");
    assert!(
        read_messages::<MyServerCommand>(&login_app)
            .iter()
            .any(|command| {
                matches!(
                    command,
                    MyServerCommand::Login {
                        login_name,
                        password,
                        connect_game: false,
                    } if login_name == "real-account" && password == REAL_PASSWORD
                )
            })
    );

    let mut clear_app = login_document_test_app(MyServerSession::default());
    set_login_document_inputs(&mut clear_app, "clear-account", "clear-password");
    let instance_id = login_document_instance(&clear_app);
    let forged_account = clear_app
        .world_mut()
        .spawn((
            UiDocumentNodeMarker {
                instance_id,
                node_id: UiNodeId::from_str(LOGIN_ACCOUNT_NODE).unwrap(),
            },
            UiTextInputValue("keep-forged-account".to_owned()),
        ))
        .id();
    let forged_password = clear_app
        .world_mut()
        .spawn((
            UiDocumentNodeMarker {
                instance_id,
                node_id: UiNodeId::from_str(LOGIN_PASSWORD_NODE).unwrap(),
            },
            UiTextInputValue("keep-forged-password".to_owned()),
            UiSensitiveTextInput,
        ))
        .id();
    let next_environment = match clear_app.world().resource::<MyServerProfiles>().selected() {
        MyServerEnvironment::Local => "production",
        MyServerEnvironment::Production => "local",
    };
    clear_app.world_mut().write_message(login_document_dispatch(
        ACTION_SWITCH_ENVIRONMENT,
        "login.environment",
        BTreeMap::from([(
            "environment".to_owned(),
            UiActionValue::Enum(next_environment.to_owned()),
        )]),
    ));
    clear_app
        .world_mut()
        .run_system_once(handle_login_document_actions)
        .expect("login document action handler should run");

    assert_eq!(active_login_input_value(&clear_app, LOGIN_ACCOUNT_NODE), "");
    assert_eq!(
        active_login_input_value(&clear_app, LOGIN_PASSWORD_NODE),
        ""
    );
    assert_eq!(
        clear_app
            .world()
            .get::<UiTextInputValue>(forged_account)
            .unwrap()
            .0,
        "keep-forged-account"
    );
    assert_eq!(
        clear_app
            .world()
            .get::<UiTextInputValue>(forged_password)
            .unwrap()
            .0,
        "keep-forged-password"
    );
}

#[test]
fn auth_guest_document_action_sends_guest_login_command() {
    let mut app = login_document_test_app(MyServerSession::default());
    app.world_mut().write_message(login_document_dispatch(
        ACTION_GUEST_LOGIN,
        "login.guest",
        BTreeMap::new(),
    ));
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
fn auth_login_document_deduplicates_account_actions_per_frame() {
    let mut app = login_document_test_app(MyServerSession::default());
    set_login_document_inputs(&mut app, "alice", "same-frame-secret");
    for _ in 0..2 {
        app.world_mut().write_message(login_document_dispatch(
            ACTION_ACCOUNT_LOGIN,
            "login.submit",
            BTreeMap::new(),
        ));
    }
    app.update();

    let commands = read_messages::<MyServerCommand>(&app);
    assert_eq!(
        commands
            .iter()
            .filter(|command| matches!(command, MyServerCommand::Login { .. }))
            .count(),
        1
    );
}

#[test]
fn auth_login_document_bindings_cover_pending_notice_and_error_states() {
    let pending_session = MyServerSession {
        account_login_state: AccountLoginState::LoggingIn,
        ..Default::default()
    };
    let mut pending = login_binding_test_app(pending_session, LoginUiState::default());
    pending.update();
    assert_eq!(
        login_binding_value(&pending, "auth.login.request_pending"),
        UiBindingValue::Bool(true)
    );
    assert_eq!(
        login_binding_value(&pending, "auth.login.disabled"),
        UiBindingValue::Bool(true)
    );
    assert_eq!(
        login_binding_value(&pending, "auth.login.environment_locked"),
        UiBindingValue::Bool(true)
    );

    for (kind, title) in [
        (AuthNoticeKind::Maintenance, "Server maintenance"),
        (AuthNoticeKind::Banned, "Account banned"),
        (AuthNoticeKind::PendingReview, "Account requires review"),
        (AuthNoticeKind::VersionIncompatible, "Version incompatible"),
        (AuthNoticeKind::Kicked, "Signed out elsewhere"),
        (AuthNoticeKind::Network, "Network unavailable"),
    ] {
        let state = LoginUiState {
            notice: Some(AuthStatusNotice {
                kind,
                title: title.to_owned(),
                detail: Some("stable detail".to_owned()),
            }),
            ..Default::default()
        };
        let mut app = login_binding_test_app(MyServerSession::default(), state);
        app.update();
        assert_eq!(
            login_binding_value(&app, "auth.login.notice_visibility"),
            UiBindingValue::Visibility(UiBindingVisibility::Visible)
        );
        assert_eq!(
            login_binding_value(&app, "auth.login.notice_title"),
            UiBindingValue::String(title.to_owned())
        );
        assert_eq!(
            login_binding_value(&app, "auth.login.notice_detail"),
            UiBindingValue::String("stable detail".to_owned())
        );
        assert_eq!(
            login_binding_value(&app, "auth.login.error_visibility"),
            UiBindingValue::Visibility(UiBindingVisibility::Hidden)
        );
    }

    let error = MyServerDisplayError {
        kind: MyServerErrorKind::TransportFailed,
        source: MyServerErrorSource::Client,
        operation: None,
        message_type: None,
        seq: None,
        http_status: None,
        error_code: Some("NETWORK_FAILURE".to_owned()),
        message_key: MyServerErrorKind::TransportFailed.message_key(),
        retryable: MyServerErrorKind::TransportFailed.retryable(),
        blocking: MyServerErrorKind::TransportFailed.blocking(),
        detail: Some("connection refused".to_owned()),
    };
    let state = LoginUiState {
        last_error: Some(error),
        ..Default::default()
    };
    let mut app = login_binding_test_app(MyServerSession::default(), state);
    app.update();
    assert_eq!(
        login_binding_value(&app, "auth.login.error_visibility"),
        UiBindingValue::Visibility(UiBindingVisibility::Visible)
    );
    assert_eq!(
        login_binding_value(&app, "auth.login.error_title"),
        UiBindingValue::String("Network unavailable".to_owned())
    );
    assert_eq!(
        login_binding_value(&app, "auth.login.error_detail"),
        UiBindingValue::String(
            "connection refused Code NETWORK_FAILURE. You can retry this operation.".to_owned()
        )
    );
    assert_eq!(
        login_binding_value(&app, "auth.login.notice_visibility"),
        UiBindingValue::Visibility(UiBindingVisibility::Hidden)
    );
}

#[test]
fn auth_server_environment_switch_updates_config_and_clears_identity_inputs() {
    let mut session = MyServerSession {
        account_login_state: AccountLoginState::LoginFailed,
        access_token: Some("stale-access".to_string()),
        player_id: Some("stale-player".to_string()),
        character_id: Some("stale-character".to_string()),
        ticket: Some("stale-ticket".to_string()),
        connected: true,
        authenticated: true,
        ..Default::default()
    };
    session.game_connection_state = GameConnectionState::Disconnected;
    let mut app = login_document_test_app(session);
    let selected = app.world().resource::<MyServerProfiles>().selected();
    let target = match selected {
        MyServerEnvironment::Local => MyServerEnvironment::Production,
        MyServerEnvironment::Production => MyServerEnvironment::Local,
    };
    let expected_base_url = app
        .world()
        .resource::<MyServerProfiles>()
        .config(target)
        .http_base_url
        .clone();
    set_login_document_inputs(&mut app, "alice", "stage11-switch-secret");
    app.world_mut().write_message(login_document_dispatch(
        ACTION_SWITCH_ENVIRONMENT,
        "login.environment",
        BTreeMap::from([(
            "environment".to_owned(),
            UiActionValue::Enum(environment_value(target).to_owned()),
        )]),
    ));
    app.update();

    assert_eq!(
        app.world().resource::<MyServerProfiles>().selected(),
        target
    );
    assert_eq!(
        app.world().resource::<MyServerConfig>().http_base_url,
        expected_base_url
    );
    let session = app.world().resource::<MyServerSession>();
    assert_eq!(session.account_login_state, AccountLoginState::NotLoggedIn);
    assert!(session.access_token.is_none());
    assert!(session.player_id.is_none());
    assert!(session.character_id.is_none());
    assert!(session.ticket.is_none());
    assert_eq!(
        session.game_connection_state,
        GameConnectionState::NotConnected
    );
    assert!(!session.connected);
    assert!(!session.authenticated);
    assert_login_document_inputs_empty(&mut app);
}

#[test]
fn auth_server_environment_switch_is_ignored_while_login_is_pending() {
    let session = MyServerSession {
        account_login_state: AccountLoginState::LoggingIn,
        ..Default::default()
    };
    let mut app = login_document_test_app(session);
    let selected = app.world().resource::<MyServerProfiles>().selected();
    let target = match selected {
        MyServerEnvironment::Local => MyServerEnvironment::Production,
        MyServerEnvironment::Production => MyServerEnvironment::Local,
    };
    let original_base_url = app
        .world()
        .resource::<MyServerConfig>()
        .http_base_url
        .clone();
    app.world_mut().write_message(login_document_dispatch(
        ACTION_SWITCH_ENVIRONMENT,
        "login.environment",
        BTreeMap::from([(
            "environment".to_owned(),
            UiActionValue::Enum(environment_value(target).to_owned()),
        )]),
    ));
    app.update();

    assert_eq!(
        app.world().resource::<MyServerProfiles>().selected(),
        selected
    );
    assert_eq!(
        app.world().resource::<MyServerConfig>().http_base_url,
        original_base_url
    );
}

#[test]
fn auth_server_environment_switch_is_ignored_with_an_in_flight_request() {
    let session = MyServerSession {
        login_request: Some(RequestId::from_raw(99)),
        ..Default::default()
    };
    let mut app = login_document_test_app(session);
    let selected = app.world().resource::<MyServerProfiles>().selected();
    let target = match selected {
        MyServerEnvironment::Local => MyServerEnvironment::Production,
        MyServerEnvironment::Production => MyServerEnvironment::Local,
    };
    app.world_mut().write_message(login_document_dispatch(
        ACTION_SWITCH_ENVIRONMENT,
        "login.environment",
        BTreeMap::from([(
            "environment".to_owned(),
            UiActionValue::Enum(environment_value(target).to_owned()),
        )]),
    ));
    app.update();

    assert_eq!(
        app.world().resource::<MyServerProfiles>().selected(),
        selected
    );
    assert_eq!(
        app.world().resource::<MyServerSession>().login_request,
        Some(RequestId::from_raw(99))
    );
}

#[test]
fn auth_create_button_sends_create_character_from_input_when_logged_in() {
    let mut app = character_button_test_app(logged_in_session());
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
    let mut app = character_button_test_app(logged_in_session());
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
    let mut app = character_button_test_app(logged_in_session());
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
    let mut app = character_button_test_app(session);
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
    let mut app = character_button_test_app(logged_in_session());
    let button = app.world_mut().spawn(SwitchAccountButton).id();
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
    assert_eq!(
        app.world()
            .get::<UiTextInputValue>(character_name)
            .unwrap()
            .0,
        ""
    );
}

#[test]
fn auth_change_character_keeps_account_and_sends_switch_character() {
    let mut session = logged_in_session();
    session.character_selection_state = CharacterSelectionState::Selected;
    session.character_id = Some("chr_selected".to_string());
    session.characters = vec![test_character("chr_selected", "WindRunner")];
    let mut app = character_button_test_app(session);
    let button = app.world_mut().spawn(ChangeCharacterButton).id();
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
            .any(|command| matches!(command, MyServerCommand::SwitchCharacter))
    );
    assert_eq!(
        app.world()
            .get::<UiTextInputValue>(character_name)
            .unwrap()
            .0,
        ""
    );
}

#[test]
fn auth_login_success_routes_to_character_select_and_loads_characters() {
    let mut app = auth_event_test_app();

    app.world_mut()
        .write_message(MyServerEvent::LoginSucceeded(test_login_session()));
    app.update();

    assert!(
        read_messages::<MyServerCommand>(&app)
            .iter()
            .any(|command| matches!(command, MyServerCommand::LoadCharacterList))
    );
    assert!(
        read_messages::<GameRouteCommand>(&app)
            .iter()
            .any(|command| matches!(
                command,
                GameRouteCommand::ChangeMode(AppUiMode::CharacterSelect)
            ))
    );
}

#[test]
fn auth_character_selected_routes_to_lobby() {
    let mut app = auth_event_test_app();

    app.world_mut()
        .write_message(MyServerEvent::CharacterSelected {
            player_id: "plr_1".to_string(),
            character_id: "chr_1".to_string(),
            world_id: Some(1),
        });
    app.update();

    assert!(
        read_messages::<GameRouteCommand>(&app)
            .iter()
            .any(|command| matches!(command, GameRouteCommand::ChangeMode(AppUiMode::Lobby)))
    );
}

#[test]
fn auth_logout_success_routes_to_login() {
    let mut app = auth_event_test_app();

    app.world_mut()
        .write_message(MyServerEvent::LogoutSucceeded);
    app.update();

    assert!(
        read_messages::<GameRouteCommand>(&app)
            .iter()
            .any(|command| matches!(command, GameRouteCommand::ChangeMode(AppUiMode::Login)))
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

fn character_button_test_app(session: MyServerSession) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_message::<UiButtonEvent>()
        .add_message::<MyServerCommand>()
        .insert_resource(session)
        .init_resource::<LoginUiState>()
        .add_systems(Update, handle_character_select_buttons);
    app
}

fn login_document_test_app(session: MyServerSession) -> App {
    const TEST_DOCUMENT: &str = r#"{
        "schema_version": 1,
        "document_id": "auth.login",
        "root": {
            "type": "container",
            "id": "login.test_root",
            "children": [
                {
                    "type": "text_input",
                    "id": "login.account",
                    "value": "",
                    "component": {
                        "slots": {
                            "label": {
                                "kind": "text",
                                "content": { "literal": "Account" }
                            }
                        }
                    }
                },
                {
                    "type": "text_input",
                    "id": "login.password",
                    "value": "",
                    "security": "sensitive",
                    "component": {
                        "slots": {
                            "label": {
                                "kind": "text",
                                "content": { "literal": "Password" }
                            }
                        }
                    }
                }
            ]
        }
    }"#;
    let validation = UiDocument::validate_json(TEST_DOCUMENT);
    assert!(
        validation.report.valid,
        "{:#?}",
        validation.report.diagnostics
    );
    let mut app = App::new();
    app.insert_resource(UiTheme::default())
        .insert_resource(UiMetrics::default())
        .insert_resource(UiFontAssets::test_registry())
        .init_resource::<UiFocusState>()
        .init_resource::<UiViewport>()
        .add_plugins(UiDocumentRuntimePlugin)
        .add_message::<MyServerCommand>()
        .insert_resource(session)
        .init_resource::<MyServerConfig>()
        .init_resource::<MyServerProfiles>()
        .init_resource::<LoginUiState>()
        .add_systems(
            Update,
            handle_login_document_actions.after(UiDocumentRuntimeSystems::Reconcile),
        );
    app.world_mut()
        .write_message(UiDocumentRuntimeCommand::Open(UiDocumentOpenRequest {
            request_id: UiDocumentRequestId(11),
            document_id: crate::framework::ui::document::UiDocumentId::from_str(LOGIN_DOCUMENT_ID)
                .unwrap(),
            owner: OWNER_LOGIN.as_str().to_owned(),
            source: UiDocumentOpenSource::Json(TEST_DOCUMENT.to_owned()),
            origin: UiDocumentSourceOrigin::Fixture {
                fixture_id: "auth_login_handler".to_owned(),
            },
            panel: UiDocumentPanel::Page,
            layer: UiDocumentLayer::Page,
            target_profile: UiTargetProfile::new(
                390.0,
                844.0,
                UiSafeAreaClass::None,
                UiDocumentInputMode::Touch,
                UiDocumentPlatform::Android,
            )
            .unwrap(),
            page_state: UiPageState::initial(),
            owner_alive: true,
            host_bindings: BTreeMap::new(),
        }));
    for _ in 0..3 {
        app.update();
    }
    let runtime = app.world().resource::<UiDocumentRuntime>();
    assert!(
        runtime
            .active_instance(
                OWNER_LOGIN.as_str(),
                &crate::framework::ui::document::UiDocumentId::from_str(LOGIN_DOCUMENT_ID).unwrap()
            )
            .is_some(),
        "{:#?}",
        runtime.record(UiDocumentRequestId(11))
    );
    app
}

fn login_binding_test_app(session: MyServerSession, ui_state: LoginUiState) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<UiBindingValues>()
        .init_resource::<MyServerProfiles>()
        .init_resource::<AuthHostContracts>()
        .insert_resource(session)
        .insert_resource(ui_state)
        .add_systems(Update, sync_login_document_bindings);
    app
}

fn login_binding_value(app: &App, path: &str) -> UiBindingValue {
    let path = UiBindingPath::from_str(path).unwrap();
    let declaration = app
        .world()
        .resource::<AuthHostContracts>()
        .login_bindings
        .get(&path)
        .unwrap();
    app.world()
        .resource::<UiBindingValues>()
        .scoped_value(LOGIN_DOCUMENT_ID, OWNER_LOGIN.as_str(), &path, declaration)
        .unwrap()
}

fn set_login_document_inputs(app: &mut App, account: &str, password: &str) {
    let mut query = app.world_mut().query::<(
        &UiDocumentNodeMarker,
        &mut UiTextInputValue,
        Has<UiSensitiveTextInput>,
    )>();
    let mut account_found = false;
    let mut password_found = false;
    for (marker, mut value, is_sensitive) in query.iter_mut(app.world_mut()) {
        match marker.node_id.as_str() {
            LOGIN_ACCOUNT_NODE => {
                assert!(!is_sensitive);
                value.0 = account.to_owned();
                account_found = true;
            }
            LOGIN_PASSWORD_NODE => {
                assert!(is_sensitive);
                value.0 = password.to_owned();
                password_found = true;
            }
            _ => {}
        }
    }
    assert!(account_found && password_found);
}

fn login_document_instance(app: &App) -> crate::framework::ui::document::UiDocumentInstanceId {
    app.world()
        .resource::<UiDocumentRuntime>()
        .active_instance(
            OWNER_LOGIN.as_str(),
            &crate::framework::ui::document::UiDocumentId::from_str(LOGIN_DOCUMENT_ID).unwrap(),
        )
        .unwrap()
}

fn active_login_input_entity(app: &App, node: &str) -> Entity {
    app.world()
        .resource::<UiDocumentRuntime>()
        .node_entity(
            login_document_instance(app),
            &UiNodeId::from_str(node).unwrap(),
        )
        .unwrap()
}

fn active_login_input_value<'a>(app: &'a App, node: &str) -> &'a str {
    &app.world()
        .get::<UiTextInputValue>(active_login_input_entity(app, node))
        .unwrap()
        .0
}

fn assert_login_document_inputs_empty(app: &mut App) {
    let mut query = app
        .world_mut()
        .query::<(&UiDocumentNodeMarker, &UiTextInputValue)>();
    let values = query
        .iter(app.world())
        .filter(|(marker, _)| {
            matches!(
                marker.node_id.as_str(),
                LOGIN_ACCOUNT_NODE | LOGIN_PASSWORD_NODE
            )
        })
        .map(|(_, value)| value.0.as_str())
        .collect::<Vec<_>>();
    assert_eq!(values.len(), 2);
    assert!(values.into_iter().all(str::is_empty));
}

fn login_document_dispatch(
    action: &str,
    source: &str,
    params: BTreeMap<String, UiActionValue>,
) -> UiActionDispatch {
    UiActionDispatch {
        action: UiActionId::from_str(action).unwrap(),
        document_id: crate::framework::ui::document::UiDocumentId::from_str(LOGIN_DOCUMENT_ID)
            .unwrap(),
        owner: OWNER_LOGIN.as_str().to_owned(),
        source_node: crate::framework::ui::document::UiNodeId::from_str(source).unwrap(),
        kind: UiRegisteredActionKind::BusinessCommand {
            target: action.to_owned(),
        },
        params,
    }
}

fn environment_value(environment: MyServerEnvironment) -> &'static str {
    match environment {
        MyServerEnvironment::Local => "local",
        MyServerEnvironment::Production => "production",
    }
}

fn auth_event_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_message::<MyServerEvent>()
        .add_message::<MyServerCommand>()
        .add_message::<GameRouteCommand>()
        .insert_resource(MyServerSession::default())
        .init_resource::<LoginUiState>()
        .init_resource::<UiFocusState>()
        .add_systems(Update, follow_myserver_login_events);
    app
}

fn logged_in_session() -> MyServerSession {
    MyServerSession {
        account_login_state: AccountLoginState::LoggedIn,
        access_token: Some("access-token".to_string()),
        ..Default::default()
    }
}

fn test_login_session() -> crate::game::myserver::LoginSession {
    crate::game::myserver::LoginSession {
        player_id: "plr_1".to_string(),
        access_token: "access-token".to_string(),
        ticket: None,
        ticket_expires_at: None,
        game_host: None,
        game_port: None,
        game_transport: None,
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
