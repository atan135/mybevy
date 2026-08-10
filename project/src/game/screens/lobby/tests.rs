use super::{host::*, model::*, *};

use std::{collections::BTreeMap, str::FromStr};

use bevy::{ecs::message::MessageCursor, prelude::*, state::app::StatesPlugin};

use crate::framework::{
    scene::prelude::{
        SceneCommand, SceneEntered, SceneEvent, SceneExited, SceneFailure, SceneFailureKind,
        SceneLifecycleState,
    },
    ui::{
        core::{
            UI_PANEL_CONFIRM_MODAL, UI_PANEL_GLOBAL_LOADING, UiMetrics, UiPanelCommand,
            UiPanelRequest, UiViewport, binding::UiBindingValues, focus::UiFocusState,
        },
        document::{
            UiActionDispatch, UiActionId, UiActionValue, UiDocumentAssetPreflightOverrides,
            UiDocumentDiff, UiDocumentDiffKind, UiDocumentId, UiDocumentInstanceId,
            UiDocumentPreviewPlugin, UiDocumentReloadError, UiDocumentReloadEvent,
            UiDocumentReloadId, UiDocumentReloadReport, UiDocumentReloadStage,
            UiDocumentReloadStatus, UiDocumentRuntime, UiDocumentRuntimePlugin,
            UiDocumentStateDecision, UiNode, UiNodeId, UiRegisteredActionKind,
            parse_approved_document_registration,
        },
        i18n::UiI18nPlugin,
        overlays::{UiModalResult, UiOverlayCommand},
        style::{UiFontAssets, UiTheme},
    },
};
use crate::game::{
    declarative_screen::DeclarativeScreenHostPlugin,
    features::touch_ripple::TouchLaunchMode,
    myserver::{GameConnectionState, MyServerCommand, MyServerSession},
    scenes::main_world_entry::{MainWorldEntryIntent, MainWorldEntryState},
    ui_ids::{ACTION_TOUCH_RIPPLE_SINGLE_PLAYER, MODAL_TOUCH_RIPPLE_LAUNCH, OWNER_LOBBY},
};

#[test]
fn lobby_document_is_valid_and_uses_keyed_typed_collection() {
    let parsed: Result<crate::framework::ui::document::UiDocument, _> =
        serde_json::from_str(LOBBY_DOCUMENT_SOURCE);
    assert!(parsed.is_ok(), "{:#?}", parsed.unwrap_err());

    let validation =
        crate::framework::ui::document::UiDocument::validate_json(LOBBY_DOCUMENT_SOURCE);
    assert!(
        validation.report.valid,
        "{:#?}",
        validation.report.diagnostics
    );
    let document = validation.validated().unwrap().document();
    let source: serde_json::Value = serde_json::from_str(LOBBY_DOCUMENT_SOURCE).unwrap();
    assert_eq!(
        source.pointer("/assets/lobby_background/source/path"),
        Some(&serde_json::Value::String(
            "ui/images/login_stillwater_background.png".to_owned()
        ))
    );
    assert_eq!(
        source.pointer("/root/children/0/id"),
        Some(&serde_json::Value::String("lobby.background".to_owned()))
    );
    assert_eq!(
        source.pointer("/root/children/0/failure/kind"),
        Some(&serde_json::Value::String("error_color".to_owned()))
    );
    assert_eq!(
        source.pointer("/root/children/0/failure/color"),
        Some(&serde_json::Value::String("#7a2930ff".to_owned()))
    );
    let repeat = find_document_node(&document.root, "lobby.entries")
        .unwrap()
        .repeat()
        .unwrap();
    assert_eq!(repeat.source.as_str(), "lobby.games.items");
    assert_eq!(repeat.key, "entry_id");
    assert_eq!(
        repeat
            .item_bindings
            .get(
                &crate::framework::ui::document::UiBindingPath::from_str(
                    "lobby.games.item.entry_id",
                )
                .unwrap()
            )
            .map(String::as_str),
        Some("entry_id")
    );
    assert!(find_document_node(&document.root, "lobby.entry_artwork").is_none());
    assert!(!LOBBY_DOCUMENT_SOURCE.contains("artwork_status"));
    for (node_id, display_binding) in [
        ("lobby.error", "lobby.games.error_display"),
        ("lobby.resource_notice", "lobby.resources.notice_display"),
        ("lobby.selection", "lobby.games.selected_display"),
    ] {
        let node = find_document_node(&document.root, node_id).unwrap();
        assert_eq!(
            node.style()
                .bindings
                .display
                .as_ref()
                .map(|path| path.as_str()),
            Some(display_binding)
        );
        assert!(node.style().bindings.visibility.is_none());
    }
}

#[test]
fn lobby_promotion_registration_matches_fixed_host_contract() {
    const REGISTRATION_SOURCE: &str =
        include_str!("../../../../assets/ui/documents/approved/lobby/lobby.promotion.v1.json");
    let contract = LobbyHostContract::default();
    let host = lobby_declarative_screen_host(&contract);
    let registration = parse_approved_document_registration(REGISTRATION_SOURCE).unwrap();
    let audit = registration.audit_report(LOBBY_DOCUMENT_SOURCE).unwrap();

    assert_eq!(host.document_id.as_str(), LOBBY_DOCUMENT_ID);
    assert_eq!(host.mode, Some(AppUiMode::Lobby));
    assert_eq!(host.owner, OWNER_LOBBY);
    assert_eq!(host.route, "lobby");
    assert_eq!(host.action_allowlist.len(), 6);
    assert_eq!(registration.owner(), OWNER_LOBBY.as_str());
    assert_eq!(registration.route(), "lobby");
    assert_eq!(audit.actions.len(), 6);
    assert!(
        !host
            .binding_schema
            .keys()
            .any(|key| key.scope == crate::framework::ui::document::UiBindingScope::Item)
    );
}

#[test]
fn lobby_startup_registration_mounts_fixed_document() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin))
        .init_state::<AppUiMode>()
        .insert_resource(UiTheme::default())
        .insert_resource(UiMetrics::default())
        .insert_resource(UiFontAssets::test_registry())
        .init_resource::<UiFocusState>()
        .init_resource::<UiViewport>()
        .init_resource::<LobbyHostContract>()
        .add_plugins((
            UiDocumentRuntimePlugin,
            UiDocumentPreviewPlugin,
            DeclarativeScreenHostPlugin,
        ))
        .add_systems(Startup, register_lobby_contract);
    set_lobby_audit_image_failure(
        &mut app
            .world_mut()
            .resource_mut::<UiDocumentAssetPreflightOverrides>(),
        true,
    );
    app.world_mut()
        .resource_mut::<NextState<AppUiMode>>()
        .set(AppUiMode::Lobby);

    for _ in 0..8 {
        app.update();
    }

    let document_id = UiDocumentId::from_str(LOBBY_DOCUMENT_ID).unwrap();
    let instance_id = app
        .world()
        .resource::<UiDocumentRuntime>()
        .active_instance(OWNER_LOBBY.as_str(), &document_id)
        .unwrap_or_else(|| panic!("{:#?}", read_messages::<UiDocumentReloadEvent>(&app)));
    let background_entity = app
        .world()
        .resource::<UiDocumentRuntime>()
        .node_entity(
            instance_id,
            &UiNodeId::from_str("lobby.background").unwrap(),
        )
        .expect("Lobby background node must be committed");
    assert!(app.world().get::<ImageNode>(background_entity).is_none());
    let fallback = app
        .world()
        .get::<BackgroundColor>(background_entity)
        .expect("failed Image node must expose the declared solid fallback")
        .0
        .to_srgba();
    assert!((fallback.red - 122.0 / 255.0).abs() < 0.001);
    assert!((fallback.green - 41.0 / 255.0).abs() < 0.001);
    assert!((fallback.blue - 48.0 / 255.0).abs() < 0.001);
    assert!((fallback.alpha - 1.0).abs() < 0.001);
}

#[test]
fn lobby_runtime_removes_hidden_sections_from_layout() {
    let mut state = LobbyUiState::default();
    state.selected_entry_id = None;
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin))
        .init_state::<AppUiMode>()
        .insert_resource(UiTheme::default())
        .insert_resource(UiMetrics::default())
        .insert_resource(UiFontAssets::test_registry())
        .init_resource::<UiFocusState>()
        .init_resource::<UiViewport>()
        .init_resource::<UiBindingValues>()
        .init_resource::<LobbyHostContract>()
        .init_resource::<MainWorldEntryState>()
        .insert_resource(MyServerSession {
            game_connection_state: GameConnectionState::Authenticated,
            ..Default::default()
        })
        .insert_resource(state)
        .add_plugins((
            UiDocumentRuntimePlugin,
            UiDocumentPreviewPlugin,
            DeclarativeScreenHostPlugin,
        ))
        .add_systems(Startup, register_lobby_contract)
        .add_systems(Update, sync_lobby_document_bindings);
    set_lobby_audit_image_failure(
        &mut app
            .world_mut()
            .resource_mut::<UiDocumentAssetPreflightOverrides>(),
        true,
    );
    app.world_mut()
        .resource_mut::<NextState<AppUiMode>>()
        .set(AppUiMode::Lobby);

    for _ in 0..8 {
        app.update();
    }

    let document_id = UiDocumentId::from_str(LOBBY_DOCUMENT_ID).unwrap();
    let instance_id = app
        .world()
        .resource::<UiDocumentRuntime>()
        .active_instance(OWNER_LOBBY.as_str(), &document_id)
        .unwrap();
    for node_id in ["lobby.error", "lobby.resource_notice", "lobby.selection"] {
        let entity = app
            .world()
            .resource::<UiDocumentRuntime>()
            .node_entity(instance_id, &UiNodeId::from_str(node_id).unwrap())
            .unwrap();
        assert_eq!(
            app.world().get::<Node>(entity).unwrap().display,
            Display::None,
            "{node_id} must not reserve layout space while hidden"
        );
    }
    let scroll = app
        .world()
        .resource::<UiDocumentRuntime>()
        .node_entity(instance_id, &UiNodeId::from_str("lobby.scroll").unwrap())
        .unwrap();
    let scroll_node = app.world().get::<Node>(scroll).unwrap();
    assert_eq!(scroll_node.position_type, PositionType::Absolute);
    assert_eq!(scroll_node.width, percent(100));
    assert_eq!(scroll_node.height, percent(100));
    assert_eq!(scroll_node.max_height, px(2400));
    assert_eq!(scroll_node.overflow, Overflow::scroll_y());
}

#[test]
fn lobby_content_audit_injects_image_failure_only_for_phone_landscape() {
    let mut phone_landscape = UiViewport::default();
    phone_landscape.logical_width = 800.0;
    phone_landscape.logical_height = 360.0;
    phone_landscape.device_scale = 2.0;
    assert!(lobby_content_audit_uses_image_failure(&phone_landscape));

    let mut phone_1080p = phone_landscape;
    phone_1080p.device_scale = 3.0;
    assert!(!lobby_content_audit_uses_image_failure(&phone_1080p));

    let desktop = UiViewport::default();
    assert!(!lobby_content_audit_uses_image_failure(&desktop));

    let mut tablet = desktop;
    tablet.logical_height = 800.0;
    tablet.device_scale = 2.0;
    assert!(!lobby_content_audit_uses_image_failure(&tablet));
}

#[test]
fn lobby_actions_are_closed_and_entry_ids_are_revalidated() {
    let descriptors = lobby_action_descriptors();
    let actual = descriptors
        .iter()
        .map(|descriptor| descriptor.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let expected = [
        ACTION_SELECT_GAME,
        ACTION_ENTER_GAME,
        ACTION_RELOAD_GAMES,
        ACTION_NAVIGATE,
        ACTION_CHANGE_CHARACTER,
        ACTION_LOGOUT,
    ]
    .into_iter()
    .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(actual, expected);

    let enter = descriptors
        .iter()
        .find(|descriptor| descriptor.id.as_str() == ACTION_ENTER_GAME)
        .unwrap();
    assert!(enter.params.contains_key("entry_id"));

    let mut app = lobby_action_test_app();
    let original = app
        .world()
        .resource::<LobbyUiState>()
        .selected_entry_id
        .clone();
    app.world_mut().write_message(lobby_dispatch(
        ACTION_SELECT_GAME,
        "lobby.entry.select",
        [("entry_id", UiActionValue::String("forged:entry".to_owned()))]
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    ));
    app.update();

    assert_eq!(
        app.world().resource::<LobbyUiState>().selected_entry_id,
        original
    );
}

#[test]
fn lobby_enter_deduplicates_same_frame_and_opens_public_loading() {
    let mut app = lobby_action_test_app();
    for entry_id in [ENTRY_SAMPLE_DUNGEON, ENTRY_ROBOT_SYNC] {
        app.world_mut().write_message(lobby_dispatch(
            ACTION_ENTER_GAME,
            "lobby.entry.enter",
            [("entry_id", UiActionValue::String(entry_id.to_owned()))]
                .into_iter()
                .map(|(key, value)| (key.to_owned(), value))
                .collect(),
        ));
    }
    app.update();

    let scene_commands = read_messages::<SceneCommand>(&app);
    assert_eq!(scene_commands.len(), 1);
    assert!(matches!(
        &scene_commands[0],
        SceneCommand::Switch(request)
            if request.enter.scene_id.as_str() == SAMPLE_DUNGEON_ROOM_SCENE_ID
    ));
    assert_eq!(
        app.world()
            .resource::<LobbyUiState>()
            .pending_entry_id
            .as_deref(),
        Some(ENTRY_SAMPLE_DUNGEON)
    );
    assert!(
        read_messages::<UiPanelCommand>(&app)
            .iter()
            .any(|command| { matches!(command, UiPanelCommand::Open(UiPanelRequest::Loading(_))) })
    );
}

#[test]
fn lobby_navigation_rejects_source_destination_forgery() {
    let mut app = lobby_action_test_app();
    app.world_mut().write_message(lobby_dispatch(
        ACTION_NAVIGATE,
        "lobby.nav.audio_settings",
        [("destination", UiActionValue::Enum("ui_gallery".to_owned()))]
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    ));
    app.update();
    assert!(read_messages::<GameRouteCommand>(&app).is_empty());

    app.world_mut().write_message(lobby_dispatch(
        ACTION_NAVIGATE,
        "lobby.nav.ui_gallery",
        [("destination", UiActionValue::Enum("ui_gallery".to_owned()))]
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    ));
    app.update();
    assert!(
        read_messages::<GameRouteCommand>(&app)
            .iter()
            .any(|command| {
                matches!(command, GameRouteCommand::ChangeMode(AppUiMode::UiGallery))
            })
    );
}

#[test]
fn touch_ripple_entry_uses_public_confirm_and_modal_result() {
    let mut app = lobby_action_test_app();
    app.world_mut().write_message(lobby_dispatch(
        ACTION_ENTER_GAME,
        "lobby.entry.enter",
        [(
            "entry_id",
            UiActionValue::String(ENTRY_TOUCH_RIPPLE.to_owned()),
        )]
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value))
        .collect(),
    ));
    app.update();
    assert!(read_messages::<UiPanelCommand>(&app).iter().any(|command| {
        matches!(command, UiPanelCommand::Open(UiPanelRequest::Confirm(confirm))
            if confirm.id == MODAL_TOUCH_RIPPLE_LAUNCH)
    }));

    app.world_mut().write_message(UiModalResult {
        id: MODAL_TOUCH_RIPPLE_LAUNCH,
        action: ACTION_TOUCH_RIPPLE_SINGLE_PLAYER,
    });
    app.update();
    assert_eq!(
        *app.world().resource::<TouchLaunchMode>(),
        TouchLaunchMode::SinglePlayer
    );
    assert!(
        read_messages::<GameRouteCommand>(&app)
            .iter()
            .any(|command| {
                matches!(
                    command,
                    GameRouteCommand::ChangeMode(AppUiMode::WanfaTouchRipple)
                )
            })
    );
}

#[test]
fn lobby_account_actions_preserve_closed_myserver_commands() {
    let mut change_app = lobby_action_test_app();
    change_app.world_mut().write_message(lobby_dispatch(
        ACTION_CHANGE_CHARACTER,
        "lobby.change_character",
        BTreeMap::new(),
    ));
    change_app.update();
    assert!(
        read_messages::<MyServerCommand>(&change_app)
            .iter()
            .any(|command| matches!(command, MyServerCommand::SwitchCharacter))
    );
    assert!(
        read_messages::<GameRouteCommand>(&change_app)
            .iter()
            .any(|command| matches!(
                command,
                GameRouteCommand::ChangeMode(AppUiMode::CharacterSelect)
            ))
    );

    let mut logout_app = lobby_action_test_app();
    logout_app.world_mut().write_message(lobby_dispatch(
        ACTION_LOGOUT,
        "lobby.logout",
        BTreeMap::new(),
    ));
    logout_app.update();
    assert!(
        read_messages::<MyServerCommand>(&logout_app)
            .iter()
            .any(|command| matches!(command, MyServerCommand::Logout))
    );
    assert!(
        read_messages::<GameRouteCommand>(&logout_app)
            .iter()
            .any(|command| matches!(command, GameRouteCommand::ChangeMode(AppUiMode::Login)))
    );
}

#[test]
fn lobby_bindings_cover_collection_connection_and_maximum_list_states() {
    let mut state = LobbyUiState::default();
    state.entries = (0..usize::from(LOBBY_MAX_ENTRIES))
        .map(|index| LobbyEntry {
            entry_id: format!("audit:{index:02}"),
            title: format!("Entry {index:02}"),
            description: "fixture".to_owned(),
            badge: "TEST".to_owned(),
            target: LobbyEntryTarget::AuditOnly,
            enabled: false,
        })
        .collect();
    state.selected_entry_id = None;
    let mut app = lobby_binding_test_app(GameConnectionState::Disconnected, state);
    app.update();
    assert_eq!(
        lobby_binding_value(&app, "lobby.games.view_state"),
        crate::framework::ui::document::UiBindingValue::Enum("disconnected".to_owned())
    );
    let crate::framework::ui::document::UiBindingValue::List(items) =
        lobby_binding_value(&app, "lobby.games.items")
    else {
        panic!("Lobby items must remain a typed list");
    };
    assert_eq!(items.len(), usize::from(LOBBY_MAX_ENTRIES));

    {
        let mut state = app.world_mut().resource_mut::<LobbyUiState>();
        state.entries.clear();
        state.collection_state = LobbyCollectionState::Loading;
    }
    app.update();
    assert_eq!(
        lobby_binding_value(&app, "lobby.games.view_state"),
        crate::framework::ui::document::UiBindingValue::Enum("loading".to_owned())
    );

    {
        let mut state = app.world_mut().resource_mut::<LobbyUiState>();
        state.collection_state = LobbyCollectionState::Error;
        state.error_title = "failed".to_owned();
    }
    app.update();
    assert_eq!(
        lobby_binding_value(&app, "lobby.games.view_state"),
        crate::framework::ui::document::UiBindingValue::Enum("error".to_owned())
    );
}

#[test]
fn lobby_display_bindings_remove_hidden_sections_from_layout() {
    let mut app =
        lobby_binding_test_app(GameConnectionState::Authenticated, LobbyUiState::default());
    app.update();

    assert_eq!(
        lobby_binding_value(&app, "lobby.games.selected_display"),
        crate::framework::ui::document::UiBindingValue::Enum("flex".to_owned())
    );
    assert_eq!(
        lobby_binding_value(&app, "lobby.games.error_display"),
        crate::framework::ui::document::UiBindingValue::Enum("none".to_owned())
    );
    assert_eq!(
        lobby_binding_value(&app, "lobby.resources.notice_display"),
        crate::framework::ui::document::UiBindingValue::Enum("none".to_owned())
    );

    {
        let mut state = app.world_mut().resource_mut::<LobbyUiState>();
        state.selected_entry_id = None;
        state.collection_state = LobbyCollectionState::Error;
        state.resource_notice_visible = true;
    }
    app.update();

    assert_eq!(
        lobby_binding_value(&app, "lobby.games.selected_display"),
        crate::framework::ui::document::UiBindingValue::Enum("none".to_owned())
    );
    assert_eq!(
        lobby_binding_value(&app, "lobby.games.error_display"),
        crate::framework::ui::document::UiBindingValue::Enum("flex".to_owned())
    );
    assert_eq!(
        lobby_binding_value(&app, "lobby.resources.notice_display"),
        crate::framework::ui::document::UiBindingValue::Enum("flex".to_owned())
    );
}

#[test]
fn lobby_reload_events_report_committed_and_retained_updates() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<LobbyUiState>()
        .add_message::<UiDocumentReloadEvent>()
        .add_systems(Update, follow_lobby_document_reload_events);

    app.world_mut()
        .write_message(reload_event(UiDocumentReloadStatus::Committed, None));
    app.update();
    assert_eq!(
        app.world().resource::<LobbyUiState>().resource_title,
        "UI resources updated"
    );

    app.world_mut().write_message(reload_event(
        UiDocumentReloadStatus::Failed,
        Some(UiDocumentReloadError {
            code: "UI_DOCUMENT_INVALID".to_owned(),
            stage: UiDocumentReloadStage::Validation,
            document_path: None,
            node_id: None,
            field_path: None,
        }),
    ));
    app.update();
    let state = app.world().resource::<LobbyUiState>();
    assert_eq!(state.resource_title, "UI update retained previous version");
    assert!(state.resource_detail.contains("UI_DOCUMENT_INVALID"));
}

#[test]
fn lobby_scene_lifecycle_closes_public_loading_and_routes_or_reports_failure() {
    let mut app = lobby_scene_test_app();
    app.world_mut()
        .resource_mut::<LobbyUiState>()
        .pending_entry_id = Some(ENTRY_ROBOT_SYNC.to_owned());
    app.world_mut()
        .write_message(SceneEvent::Entered(SceneEntered {
            scene_id: ROBOT_SYNC_ARENA_SCENE_ID.into(),
            session_id: "robot-session".into(),
            content_version: None,
        }));
    app.update();
    assert!(read_messages::<UiPanelCommand>(&app).iter().any(|command| {
        matches!(command, UiPanelCommand::Close(id) if *id == UI_PANEL_GLOBAL_LOADING)
    }));
    assert!(
        read_messages::<GameRouteCommand>(&app)
            .iter()
            .any(|command| {
                matches!(
                    command,
                    GameRouteCommand::ChangeMode(AppUiMode::RobotSyncScene)
                )
            })
    );

    app.world_mut()
        .resource_mut::<LobbyUiState>()
        .pending_entry_id = Some(ENTRY_FANGYUAN_HOME.to_owned());
    app.world_mut().write_message(SceneEvent::Failed(
        SceneFailure::new(
            SceneFailureKind::SceneNotFound,
            SceneLifecycleState::Resolving,
        )
        .with_scene(FANGYUAN_HOME_SCENE_ID)
        .with_message("missing fixture"),
    ));
    app.update();
    assert!(
        app.world()
            .resource::<LobbyUiState>()
            .pending_entry_id
            .is_none()
    );
    assert!(!read_messages::<UiOverlayCommand>(&app).is_empty());

    app.world_mut()
        .write_message(SceneEvent::Exited(SceneExited {
            scene_id: FANGYUAN_HOME_SCENE_ID.into(),
            session_id: "home-session".into(),
        }));
    app.update();
}

#[test]
fn lobby_cleanup_clears_focus_transient_state_and_public_overlays() {
    let mut state = LobbyUiState::default();
    state.pending_entry_id = Some(ENTRY_SAMPLE_DUNGEON.to_owned());
    state.confirming_entry_id = Some(ENTRY_TOUCH_RIPPLE.to_owned());
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(state)
        .init_resource::<UiFocusState>()
        .init_resource::<MainWorldEntryState>()
        .add_message::<UiPanelCommand>()
        .add_message::<MainWorldEntryIntent>()
        .add_systems(Update, cleanup_lobby_screen);
    let focused = app.world_mut().spawn_empty().id();
    app.world_mut()
        .resource_mut::<UiFocusState>()
        .focused_entity = Some(focused);
    app.update();

    let state = app.world().resource::<LobbyUiState>();
    assert!(state.pending_entry_id.is_none());
    assert!(state.confirming_entry_id.is_none());
    assert!(
        app.world()
            .resource::<UiFocusState>()
            .focused_entity
            .is_none()
    );
    let panels = read_messages::<UiPanelCommand>(&app);
    assert!(panels.iter().any(|command| {
        matches!(command, UiPanelCommand::Close(id) if *id == UI_PANEL_CONFIRM_MODAL)
    }));
    assert!(panels.iter().any(|command| {
        matches!(command, UiPanelCommand::Close(id) if *id == UI_PANEL_GLOBAL_LOADING)
    }));
}

#[test]
fn lobby_cleanup_does_not_cancel_an_active_main_world_transition() {
    let mut entry = MainWorldEntryState::default();
    entry.phase = crate::game::scenes::main_world_entry::MainWorldEntryPhase::Active;
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<LobbyUiState>()
        .init_resource::<UiFocusState>()
        .insert_resource(entry)
        .add_message::<UiPanelCommand>()
        .add_message::<MainWorldEntryIntent>()
        .add_systems(Update, cleanup_lobby_screen);

    app.update();

    assert!(read_messages::<MainWorldEntryIntent>(&app).is_empty());
}

#[test]
fn lobby_cleanup_cancels_an_in_flight_main_world_transition() {
    let mut entry = MainWorldEntryState::default();
    entry.phase = crate::game::scenes::main_world_entry::MainWorldEntryPhase::JoiningRoom;
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<LobbyUiState>()
        .init_resource::<UiFocusState>()
        .insert_resource(entry)
        .add_message::<UiPanelCommand>()
        .add_message::<MainWorldEntryIntent>()
        .add_systems(Update, cleanup_lobby_screen);

    app.update();

    assert_eq!(
        read_messages::<MainWorldEntryIntent>(&app),
        vec![MainWorldEntryIntent::Cancel]
    );
}

fn find_document_node<'a>(node: &'a UiNode, id: &str) -> Option<&'a UiNode> {
    if node.id().as_str() == id {
        return Some(node);
    }
    node.children()
        .iter()
        .find_map(|child| find_document_node(child, id))
}

fn lobby_action_test_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, UiI18nPlugin))
        .init_resource::<LobbyUiState>()
        .init_resource::<TouchLaunchMode>()
        .add_message::<UiActionDispatch>()
        .add_message::<UiModalResult>()
        .add_message::<SceneCommand>()
        .add_message::<UiPanelCommand>()
        .add_message::<UiOverlayCommand>()
        .add_message::<GameRouteCommand>()
        .add_message::<MyServerCommand>()
        .add_message::<MainWorldEntryIntent>()
        .add_systems(Update, handle_lobby_document_actions);
    app
}

fn lobby_dispatch(
    action: &str,
    source: &str,
    params: BTreeMap<String, UiActionValue>,
) -> UiActionDispatch {
    UiActionDispatch {
        action: UiActionId::from_str(action).unwrap(),
        document_id: UiDocumentId::from_str(LOBBY_DOCUMENT_ID).unwrap(),
        owner: OWNER_LOBBY.as_str().to_owned(),
        source_node: UiNodeId::from_str(source).unwrap(),
        kind: UiRegisteredActionKind::BusinessCommand {
            target: action.to_owned(),
        },
        params,
    }
}

fn lobby_binding_test_app(connection: GameConnectionState, state: LobbyUiState) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<UiBindingValues>()
        .init_resource::<LobbyHostContract>()
        .init_resource::<MainWorldEntryState>()
        .insert_resource(MyServerSession {
            game_connection_state: connection,
            ..Default::default()
        })
        .insert_resource(state)
        .add_systems(Update, sync_lobby_document_bindings);
    app
}

fn lobby_binding_value(app: &App, path: &str) -> crate::framework::ui::document::UiBindingValue {
    let path = crate::framework::ui::document::UiBindingPath::from_str(path).unwrap();
    let declaration = app
        .world()
        .resource::<LobbyHostContract>()
        .bindings
        .get(&path)
        .unwrap();
    app.world()
        .resource::<UiBindingValues>()
        .scoped_value(LOBBY_DOCUMENT_ID, OWNER_LOBBY.as_str(), &path, declaration)
        .unwrap()
}

fn reload_event(
    status: UiDocumentReloadStatus,
    error: Option<UiDocumentReloadError>,
) -> UiDocumentReloadEvent {
    UiDocumentReloadEvent(UiDocumentReloadReport {
        report_version: 1,
        reload_id: UiDocumentReloadId(7),
        request_id: None,
        document_id: UiDocumentId::from_str(LOBBY_DOCUMENT_ID).unwrap(),
        owner: OWNER_LOBBY.as_str().to_owned(),
        source_path: LOBBY_DOCUMENT_SOURCE_PATH.to_owned(),
        status,
        previous_instance: Some(UiDocumentInstanceId(1)),
        current_instance: Some(UiDocumentInstanceId(1)),
        diff: Some(UiDocumentDiff {
            kind: UiDocumentDiffKind::NoChanges,
            in_place_nodes: Vec::new(),
            rebuild_subtrees: Vec::new(),
            page_reasons: Vec::new(),
        }),
        state_decisions: Vec::<UiDocumentStateDecision>::new(),
        error,
    })
}

fn lobby_scene_test_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, UiI18nPlugin))
        .init_resource::<LobbyUiState>()
        .add_message::<SceneEvent>()
        .add_message::<UiPanelCommand>()
        .add_message::<UiOverlayCommand>()
        .add_message::<GameRouteCommand>()
        .add_systems(Update, handle_lobby_scene_entry_events);
    app
}

fn read_messages<M>(app: &App) -> Vec<M>
where
    M: Message + Clone,
{
    let messages = app.world().resource::<Messages<M>>();
    let mut cursor = MessageCursor::default();
    cursor.read(messages).cloned().collect()
}
