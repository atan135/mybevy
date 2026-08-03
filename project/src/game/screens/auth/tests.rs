use super::{host::*, model::*, view::*};
use crate::framework::network::RequestId;
use crate::framework::ui::{
    core::{UiMetrics, UiOrientation, UiViewport, focus::UiFocusState},
    document::{UiActionId, UiBindingPath, UiBindingScope, UiBindingType},
    widgets::{UiButtonEvent, UiButtonEventKind, UiTextInputValue},
};
use crate::game::{
    myserver::{
        AccountLoginState, CharacterSelectionState, CharacterSummary, ElementValues,
        GameConnectionState, MyServerCommand, MyServerConfig, MyServerDisplayError,
        MyServerEnvironment, MyServerErrorKind, MyServerErrorSource, MyServerEvent,
        MyServerOperation, MyServerProfiles, MyServerSession,
    },
    navigation::{AppUiMode, GameRouteCommand},
    ui_ids::{OWNER_CHARACTER_SELECT, OWNER_LOGIN, PANEL_CHARACTER_SELECT, PANEL_LOGIN},
};
use bevy::{ecs::message::MessageCursor, prelude::*};
use std::time::SystemTime;
use std::{collections::HashMap, str::FromStr};

#[test]
fn landscape_login_control_grid_is_limited_to_high_density_devices() {
    let mut viewport = UiViewport::default();

    assert!(!uses_landscape_login_control_grid(&viewport));

    viewport.device_scale = 2.0;
    assert!(uses_landscape_login_control_grid(&viewport));

    viewport.orientation = UiOrientation::Portrait;
    assert!(!uses_landscape_login_control_grid(&viewport));
}

#[test]
fn login_reference_panel_width_survives_keyboard_viewport_resize() {
    let metrics = UiMetrics::default();
    let full_viewport = UiViewport::default();
    let mut keyboard_viewport = full_viewport;
    keyboard_viewport.logical_height = 320.0;

    let full_size = login_visual_panel_size(&full_viewport, &metrics);
    let keyboard_size = login_visual_panel_size(&keyboard_viewport, &metrics);

    assert_eq!(full_size.x, LOGIN_REFERENCE_PANEL_WIDTH);
    assert_eq!(keyboard_size.x, LOGIN_REFERENCE_PANEL_WIDTH);
    assert!(keyboard_size.y < full_size.y);
}

#[test]
fn login_control_cells_have_content_independent_flex_bases() {
    let cell = login_control_cell();

    assert_eq!(cell.flex_grow, 1.0);
    assert_eq!(cell.flex_basis, px(0));
    assert_eq!(cell.min_width, px(0));
}

#[test]
fn auth_page_baselines_freeze_owner_panel_source_and_states() {
    assert_eq!(LOGIN_PAGE_BASELINE.mode, AppUiMode::Login);
    assert_eq!(LOGIN_PAGE_BASELINE.owner, OWNER_LOGIN);
    assert_eq!(LOGIN_PAGE_BASELINE.panel, PANEL_LOGIN);
    assert_eq!(
        LOGIN_PAGE_BASELINE.action_source,
        AuthActionSource::RustView
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
    assert_eq!(
        app.world()
            .resource::<MyServerSession>()
            .account_login_state,
        AccountLoginState::LoggingIn
    );
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
    let mut app = server_environment_test_app(session);
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
    let button = app.world_mut().spawn(ServerEnvironmentButton(target)).id();
    let login = app
        .world_mut()
        .spawn((LoginNameInput, UiTextInputValue("alice".to_string())))
        .id();
    let password = app
        .world_mut()
        .spawn((PasswordInput, UiTextInputValue("secret".to_string())))
        .id();

    click(&mut app, button);
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
    assert_eq!(app.world().get::<UiTextInputValue>(login).unwrap().0, "");
    assert_eq!(app.world().get::<UiTextInputValue>(password).unwrap().0, "");
}

#[test]
fn auth_server_environment_switch_is_ignored_while_login_is_pending() {
    let session = MyServerSession {
        account_login_state: AccountLoginState::LoggingIn,
        ..Default::default()
    };
    let mut app = server_environment_test_app(session);
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
    let button = app.world_mut().spawn(ServerEnvironmentButton(target)).id();

    click(&mut app, button);
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
    let mut app = server_environment_test_app(session);
    let selected = app.world().resource::<MyServerProfiles>().selected();
    let target = match selected {
        MyServerEnvironment::Local => MyServerEnvironment::Production,
        MyServerEnvironment::Production => MyServerEnvironment::Local,
    };
    let button = app.world_mut().spawn(ServerEnvironmentButton(target)).id();

    click(&mut app, button);
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

#[test]
fn auth_change_character_keeps_account_and_sends_switch_character() {
    let mut session = logged_in_session();
    session.character_selection_state = CharacterSelectionState::Selected;
    session.character_id = Some("chr_selected".to_string());
    session.characters = vec![test_character("chr_selected", "WindRunner")];
    let mut app = login_button_test_app(session);
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

fn server_environment_test_app(session: MyServerSession) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_message::<UiButtonEvent>()
        .insert_resource(session)
        .init_resource::<MyServerConfig>()
        .init_resource::<MyServerProfiles>()
        .init_resource::<LoginUiState>()
        .init_resource::<UiFocusState>()
        .add_systems(Update, handle_server_environment_buttons);
    app
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
