use std::{collections::BTreeMap, str::FromStr};

use bevy::prelude::*;

#[cfg(all(debug_assertions, not(target_os = "android")))]
use crate::framework::ui::audit::UiAuditConfig;
#[cfg(all(debug_assertions, not(target_os = "android")))]
use crate::framework::ui::core::UiViewport;
#[cfg(all(debug_assertions, not(target_os = "android")))]
use crate::framework::ui::document::{
    UiAssetId, UiDocumentAssetPreflightOverrides, UiDocumentAssetPreflightStatus,
};
use crate::framework::{
    scene::prelude::{SceneCommand, SceneSwitchRequest},
    ui::{
        core::{
            UI_PANEL_CONFIRM_MODAL, UI_PANEL_GLOBAL_LOADING, UiPanelCommand, UiPanelRequest,
            binding::UiBindingValues, focus::UiFocusState,
        },
        document::{
            UiActionDescriptor, UiActionDispatch, UiActionId, UiActionParamSchema,
            UiActionParamType, UiActionRegistry, UiActionValue, UiBindingDeclaration,
            UiBindingMissingBehavior, UiBindingPath, UiBindingScope, UiBindingType, UiBindingValue,
            UiBindingVisibility, UiDocumentId, UiDocumentLayer, UiDocumentPanel,
            UiDocumentReloadEvent, UiDocumentReloadStatus, UiHostBindingKey, UiNodeId, UiPageState,
            UiRegisteredActionKind,
        },
        i18n::UiI18n,
        overlays::{
            UiConfirmModal, UiI18nTextSpec, UiLoading, UiModalActionSpec, UiModalActionStyle,
            UiModalResult, UiOverlayCommand, UiToast,
        },
    },
};
use crate::game::{
    declarative_screen::{
        DeclarativeScreenFailurePolicy, DeclarativeScreenHost, DeclarativeScreenRegistry,
        DeclarativeScreenSource,
    },
    features::touch_ripple::TouchLaunchMode,
    myserver::{GameConnectionState, MyServerCommand, MyServerSession},
    navigation::{AppUiMode, GameRouteCommand},
    scenes::{
        LOCKSTEP_SIM_ARENA_SCENE_ID, ROBOT_SYNC_ARENA_SCENE_ID, SAMPLE_DUNGEON_ROOM_SCENE_ID,
        main_world_entry::{MainWorldEntryIntent, MainWorldEntryState},
    },
    ui_ids::{
        ACTION_CANCEL, ACTION_CONFIRM, ACTION_TOUCH_RIPPLE_NETWORKED,
        ACTION_TOUCH_RIPPLE_SINGLE_PLAYER, MODAL_TOUCH_RIPPLE_LAUNCH, OWNER_LOBBY,
    },
};

use super::model::*;

pub(super) const LOBBY_DOCUMENT_ID: &str = "game.lobby";
pub(super) const LOBBY_DOCUMENT_SOURCE_PATH: &str = "lobby/lobby.v1.json";
pub(super) const LOBBY_DOCUMENT_SOURCE: &str =
    include_str!("../../../../assets/ui/documents/approved/lobby/lobby.v1.json");
const ENTRY_ID_MAX_BYTES: usize = 256;
#[cfg(all(debug_assertions, not(target_os = "android")))]
const LOBBY_BACKGROUND_ASSET_ID: &str = "lobby_background";
#[cfg(all(debug_assertions, not(target_os = "android")))]
const LOBBY_AUDIT_IMAGE_FAILURE_CODE: &str = "UI_DOCUMENT_AUDIT_IMAGE_LOAD_FAILED";

pub(super) const ACTION_SELECT_GAME: &str = "lobby.select_game";
pub(super) const ACTION_ENTER_GAME: &str = "lobby.enter_game";
pub(super) const ACTION_RELOAD_GAMES: &str = "lobby.reload_games";
pub(super) const ACTION_NAVIGATE: &str = "lobby.navigate";
pub(super) const ACTION_CHANGE_CHARACTER: &str = "lobby.change_character";
pub(super) const ACTION_LOGOUT: &str = "lobby.logout";

const DESTINATION_AUDIO_SETTINGS: &str = "audio_settings";
const DESTINATION_AUDIO_MONITOR: &str = "audio_monitor";
const DESTINATION_AUDIO_GALLERY: &str = "audio_gallery";
const DESTINATION_UI_GALLERY: &str = "ui_gallery";

const NAV_AUDIO_SETTINGS_NODE: &str = "lobby.nav.audio_settings";
const NAV_AUDIO_MONITOR_NODE: &str = "lobby.nav.audio_monitor";
const NAV_AUDIO_GALLERY_NODE: &str = "lobby.nav.audio_gallery";
const NAV_UI_GALLERY_NODE: &str = "lobby.nav.ui_gallery";

#[derive(Resource)]
pub(super) struct LobbyHostContract {
    pub bindings: BTreeMap<UiBindingPath, UiBindingDeclaration>,
}

impl Default for LobbyHostContract {
    fn default() -> Self {
        Self {
            bindings: lobby_binding_schema(),
        }
    }
}

pub(super) fn lobby_binding_schema() -> BTreeMap<UiBindingPath, UiBindingDeclaration> {
    let entry = UiBindingType::Record {
        fields: BTreeMap::from([
            ("entry_id".to_owned(), UiBindingType::String),
            ("title".to_owned(), UiBindingType::String),
            ("description".to_owned(), UiBindingType::String),
            ("badge".to_owned(), UiBindingType::String),
            ("selected".to_owned(), UiBindingType::Bool),
            ("disabled".to_owned(), UiBindingType::Bool),
            ("loading".to_owned(), UiBindingType::Bool),
        ]),
    };
    binding_schema([
        (
            "lobby.games.items",
            UiBindingScope::Owner,
            UiBindingType::List {
                item: Box::new(entry),
                max_items: LOBBY_MAX_ENTRIES,
            },
        ),
        (
            "lobby.games.collection_state",
            UiBindingScope::Owner,
            UiBindingType::Enum {
                values: ["loading", "ready", "error"].map(str::to_owned).to_vec(),
            },
        ),
        (
            "lobby.games.view_state",
            UiBindingScope::Owner,
            UiBindingType::Enum {
                values: [
                    "loading",
                    "empty",
                    "ready",
                    "error",
                    "disconnected",
                    "confirming",
                ]
                .map(str::to_owned)
                .to_vec(),
            },
        ),
        (
            "lobby.games.status",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "lobby.connection.status",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "lobby.games.selected_id",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "lobby.games.selected_title",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "lobby.games.selected_detail",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "lobby.games.selected_visibility",
            UiBindingScope::Owner,
            UiBindingType::Visibility,
        ),
        (
            "lobby.games.error_title",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "lobby.games.error_detail",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "lobby.games.error_visibility",
            UiBindingScope::Owner,
            UiBindingType::Visibility,
        ),
        (
            "lobby.resources.title",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "lobby.resources.detail",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "lobby.resources.notice_visibility",
            UiBindingScope::Owner,
            UiBindingType::Visibility,
        ),
        (
            "lobby.games.reload_disabled",
            UiBindingScope::Owner,
            UiBindingType::Bool,
        ),
        (
            "lobby.games.reload_loading",
            UiBindingScope::Owner,
            UiBindingType::Bool,
        ),
        (
            "lobby.games.item.entry_id",
            UiBindingScope::Item,
            UiBindingType::String,
        ),
        (
            "lobby.games.item.title",
            UiBindingScope::Item,
            UiBindingType::String,
        ),
        (
            "lobby.games.item.description",
            UiBindingScope::Item,
            UiBindingType::String,
        ),
        (
            "lobby.games.item.badge",
            UiBindingScope::Item,
            UiBindingType::String,
        ),
        (
            "lobby.games.item.selected",
            UiBindingScope::Item,
            UiBindingType::Bool,
        ),
        (
            "lobby.games.item.disabled",
            UiBindingScope::Item,
            UiBindingType::Bool,
        ),
        (
            "lobby.games.item.loading",
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
                UiBindingPath::from_str(path).expect("Lobby binding paths are static and valid"),
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

pub(super) fn register_lobby_contract(
    contract: Res<LobbyHostContract>,
    mut actions: ResMut<UiActionRegistry>,
    mut screens: ResMut<DeclarativeScreenRegistry>,
) {
    for descriptor in lobby_action_descriptors() {
        actions
            .register(descriptor)
            .expect("Lobby action registration must be valid and unique");
    }
    screens
        .register(lobby_declarative_screen_host(contract.as_ref()))
        .expect("Lobby declarative screen registration must be valid and unique");
}

pub(super) fn lobby_declarative_screen_host(contract: &LobbyHostContract) -> DeclarativeScreenHost {
    let source =
        DeclarativeScreenSource::approved(LOBBY_DOCUMENT_SOURCE_PATH, LOBBY_DOCUMENT_SOURCE);
    DeclarativeScreenHost {
        document_id: UiDocumentId::from_str(LOBBY_DOCUMENT_ID)
            .expect("Lobby document ID is static and valid"),
        route: "lobby",
        route_aliases: &["lobby", "game_list", "game-list"],
        mode: Some(AppUiMode::Lobby),
        owner: OWNER_LOBBY,
        panel: UiDocumentPanel::Page,
        layer: UiDocumentLayer::Page,
        initial_state: UiPageState::initial(),
        binding_schema: contract
            .bindings
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
            .collect(),
        action_allowlist: [
            ACTION_SELECT_GAME,
            ACTION_ENTER_GAME,
            ACTION_RELOAD_GAMES,
            ACTION_NAVIGATE,
            ACTION_CHANGE_CHARACTER,
            ACTION_LOGOUT,
        ]
        .into_iter()
        .map(|action| UiActionId::from_str(action).expect("Lobby action IDs are static and valid"))
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

pub(super) fn lobby_action_descriptors() -> Vec<UiActionDescriptor> {
    vec![
        business_action(ACTION_SELECT_GAME, "lobby.entry.select").with_param(
            "entry_id",
            UiActionParamSchema::required(UiActionParamType::OpaqueId {
                max_bytes: ENTRY_ID_MAX_BYTES,
            }),
        ),
        business_action(ACTION_ENTER_GAME, "lobby.entry.enter").with_param(
            "entry_id",
            UiActionParamSchema::required(UiActionParamType::OpaqueId {
                max_bytes: ENTRY_ID_MAX_BYTES,
            }),
        ),
        business_action(ACTION_RELOAD_GAMES, "lobby.reload"),
        UiActionDescriptor::new(
            UiActionId::from_str(ACTION_NAVIGATE).unwrap(),
            UiDocumentId::from_str(LOBBY_DOCUMENT_ID).unwrap(),
            OWNER_LOBBY.as_str(),
            UiRegisteredActionKind::BusinessCommand {
                target: ACTION_NAVIGATE.to_owned(),
            },
        )
        .with_sources(
            [
                NAV_AUDIO_SETTINGS_NODE,
                NAV_AUDIO_MONITOR_NODE,
                NAV_AUDIO_GALLERY_NODE,
                NAV_UI_GALLERY_NODE,
            ]
            .into_iter()
            .map(|source| UiNodeId::from_str(source).unwrap()),
        )
        .with_param(
            "destination",
            UiActionParamSchema::required(UiActionParamType::Enum {
                values: [
                    DESTINATION_AUDIO_SETTINGS,
                    DESTINATION_AUDIO_MONITOR,
                    DESTINATION_AUDIO_GALLERY,
                    DESTINATION_UI_GALLERY,
                ]
                .map(str::to_owned)
                .into_iter()
                .collect(),
            }),
        ),
        business_action(ACTION_CHANGE_CHARACTER, "lobby.change_character"),
        business_action(ACTION_LOGOUT, "lobby.logout"),
    ]
}

fn business_action(action: &str, source: &str) -> UiActionDescriptor {
    UiActionDescriptor::new(
        UiActionId::from_str(action).expect("Lobby action IDs are static and valid"),
        UiDocumentId::from_str(LOBBY_DOCUMENT_ID).expect("Lobby document ID is static and valid"),
        OWNER_LOBBY.as_str(),
        UiRegisteredActionKind::BusinessCommand {
            target: action.to_owned(),
        },
    )
    .with_source(UiNodeId::from_str(source).expect("Lobby source node IDs are static and valid"))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_lobby_document_actions(
    mut actions: MessageReader<UiActionDispatch>,
    mut modal_results: MessageReader<UiModalResult>,
    mut ui_state: ResMut<LobbyUiState>,
    mut launch_mode: ResMut<TouchLaunchMode>,
    i18n: Res<UiI18n>,
    mut scene_commands: MessageWriter<SceneCommand>,
    mut panel_commands: MessageWriter<UiPanelCommand>,
    mut overlay_commands: MessageWriter<UiOverlayCommand>,
    mut route_commands: MessageWriter<GameRouteCommand>,
    mut myserver_commands: MessageWriter<MyServerCommand>,
    mut main_world_intents: MessageWriter<MainWorldEntryIntent>,
) {
    let mut command_sent = false;
    for action in actions.read() {
        if !is_lobby_business_action(action) {
            continue;
        }
        match action.action.as_str() {
            ACTION_SELECT_GAME if action.source_node.as_str() == "lobby.entry.select" => {
                if let Some(entry_id) = action_entry_id(action) {
                    ui_state.select(&entry_id);
                }
            }
            ACTION_ENTER_GAME
                if !command_sent && action.source_node.as_str() == "lobby.entry.enter" =>
            {
                let Some(entry_id) = action_entry_id(action) else {
                    continue;
                };
                let Some(entry) = ui_state.entry(&entry_id).cloned() else {
                    continue;
                };
                if !entry.enabled || ui_state.pending_entry_id.is_some() {
                    continue;
                }
                command_sent = enter_lobby_entry(
                    &entry,
                    &mut ui_state,
                    &i18n,
                    &mut scene_commands,
                    &mut panel_commands,
                    &mut route_commands,
                    &mut main_world_intents,
                );
            }
            ACTION_RELOAD_GAMES
                if !command_sent
                    && action.source_node.as_str() == "lobby.reload"
                    && action.params.is_empty() =>
            {
                command_sent = true;
                ui_state.begin_reload();
            }
            ACTION_NAVIGATE if !command_sent => {
                let Some(mode) = navigation_destination(action) else {
                    continue;
                };
                command_sent = true;
                route_commands.write(GameRouteCommand::ChangeMode(mode));
            }
            ACTION_CHANGE_CHARACTER
                if !command_sent
                    && action.source_node.as_str() == "lobby.change_character"
                    && action.params.is_empty() =>
            {
                command_sent = true;
                main_world_intents.write(MainWorldEntryIntent::CharacterChanged);
                myserver_commands.write(MyServerCommand::SwitchCharacter);
                route_commands.write(GameRouteCommand::ChangeMode(AppUiMode::CharacterSelect));
            }
            ACTION_LOGOUT
                if !command_sent
                    && action.source_node.as_str() == "lobby.logout"
                    && action.params.is_empty() =>
            {
                command_sent = true;
                main_world_intents.write(MainWorldEntryIntent::LoggedOut);
                myserver_commands.write(MyServerCommand::Logout);
                route_commands.write(GameRouteCommand::ChangeMode(AppUiMode::Login));
            }
            _ => {}
        }
    }

    for result in modal_results.read() {
        if result.id != MODAL_TOUCH_RIPPLE_LAUNCH
            || ui_state.confirming_entry_id.as_deref() != Some(ENTRY_TOUCH_RIPPLE)
        {
            continue;
        }
        ui_state.confirming_entry_id = None;
        match result.action {
            ACTION_CANCEL | ACTION_CONFIRM => {}
            ACTION_TOUCH_RIPPLE_SINGLE_PLAYER => {
                *launch_mode = TouchLaunchMode::SinglePlayer;
                overlay_commands.write(UiOverlayCommand::ShowToast(UiToast::new_key(
                    &i18n,
                    "lobby.touch_ripple.toast.local",
                    "Starting local Touch Ripple",
                )));
                route_commands.write(GameRouteCommand::ChangeMode(AppUiMode::WanfaTouchRipple));
            }
            ACTION_TOUCH_RIPPLE_NETWORKED => {
                *launch_mode = TouchLaunchMode::Auto;
                overlay_commands.write(UiOverlayCommand::ShowToast(UiToast::new_key(
                    &i18n,
                    "lobby.touch_ripple.toast.networked",
                    "Starting networked Touch Ripple",
                )));
                route_commands.write(GameRouteCommand::ChangeMode(AppUiMode::WanfaTouchRipple));
            }
            _ => {}
        }
    }
}

fn is_lobby_business_action(action: &UiActionDispatch) -> bool {
    action.document_id.as_str() == LOBBY_DOCUMENT_ID
        && action.owner == OWNER_LOBBY.as_str()
        && matches!(
            &action.kind,
            UiRegisteredActionKind::BusinessCommand { target }
                if target == action.action.as_str()
        )
}

fn action_entry_id(action: &UiActionDispatch) -> Option<String> {
    if action.params.len() != 1 {
        return None;
    }
    match action.params.get("entry_id") {
        Some(UiActionValue::String(entry_id))
            if !entry_id.is_empty() && entry_id.len() <= ENTRY_ID_MAX_BYTES =>
        {
            Some(entry_id.clone())
        }
        _ => None,
    }
}

fn navigation_destination(action: &UiActionDispatch) -> Option<AppUiMode> {
    if action.action.as_str() != ACTION_NAVIGATE || action.params.len() != 1 {
        return None;
    }
    match (
        action.source_node.as_str(),
        action.params.get("destination"),
    ) {
        (NAV_AUDIO_SETTINGS_NODE, Some(UiActionValue::Enum(value)))
            if value == DESTINATION_AUDIO_SETTINGS =>
        {
            Some(AppUiMode::AudioSettings)
        }
        (NAV_AUDIO_MONITOR_NODE, Some(UiActionValue::Enum(value)))
            if value == DESTINATION_AUDIO_MONITOR =>
        {
            Some(AppUiMode::AudioMonitor)
        }
        (NAV_AUDIO_GALLERY_NODE, Some(UiActionValue::Enum(value)))
            if value == DESTINATION_AUDIO_GALLERY =>
        {
            Some(AppUiMode::AudioGallery)
        }
        (NAV_UI_GALLERY_NODE, Some(UiActionValue::Enum(value)))
            if value == DESTINATION_UI_GALLERY =>
        {
            Some(AppUiMode::UiGallery)
        }
        _ => None,
    }
}

fn enter_lobby_entry(
    entry: &LobbyEntry,
    ui_state: &mut LobbyUiState,
    i18n: &UiI18n,
    scene_commands: &mut MessageWriter<SceneCommand>,
    panel_commands: &mut MessageWriter<UiPanelCommand>,
    route_commands: &mut MessageWriter<GameRouteCommand>,
    main_world_intents: &mut MessageWriter<MainWorldEntryIntent>,
) -> bool {
    ui_state.selected_entry_id = Some(entry.entry_id.clone());
    match entry.target {
        LobbyEntryTarget::MainWorld => {
            main_world_intents.write(MainWorldEntryIntent::Enter);
            true
        }
        LobbyEntryTarget::TouchRipple => {
            ui_state.confirming_entry_id = Some(ENTRY_TOUCH_RIPPLE.to_owned());
            panel_commands.write(UiPanelCommand::Open(UiPanelRequest::Confirm(
                touch_ripple_confirm_modal(i18n),
            )));
            true
        }
        LobbyEntryTarget::LockstepSim => {
            begin_scene_entry(
                entry,
                LOCKSTEP_SIM_ARENA_SCENE_ID,
                ui_state,
                i18n,
                scene_commands,
                panel_commands,
            );
            true
        }
        LobbyEntryTarget::SampleDungeon => {
            begin_scene_entry(
                entry,
                SAMPLE_DUNGEON_ROOM_SCENE_ID,
                ui_state,
                i18n,
                scene_commands,
                panel_commands,
            );
            true
        }
        LobbyEntryTarget::RobotSync => {
            begin_scene_entry(
                entry,
                ROBOT_SYNC_ARENA_SCENE_ID,
                ui_state,
                i18n,
                scene_commands,
                panel_commands,
            );
            true
        }
        LobbyEntryTarget::FangyuanHome => {
            main_world_intents.write(MainWorldEntryIntent::EnterHome);
            true
        }
        LobbyEntryTarget::FangyuanPlayerPreview => {
            route_commands.write(GameRouteCommand::ChangeMode(
                AppUiMode::FangyuanPlayerPreview,
            ));
            true
        }
        #[cfg(all(debug_assertions, not(target_os = "android")))]
        LobbyEntryTarget::AuditOnly => false,
    }
}

fn begin_scene_entry(
    entry: &LobbyEntry,
    scene_id: &str,
    ui_state: &mut LobbyUiState,
    i18n: &UiI18n,
    scene_commands: &mut MessageWriter<SceneCommand>,
    panel_commands: &mut MessageWriter<UiPanelCommand>,
) {
    ui_state.pending_entry_id = Some(entry.entry_id.clone());
    scene_commands.write(SceneCommand::Switch(SceneSwitchRequest::new(scene_id)));
    panel_commands.write(UiPanelCommand::Open(UiPanelRequest::Loading(
        UiLoading::new_key(i18n, "lobby.loading.entry", "Entering game..."),
    )));
}

pub(super) fn finish_lobby_reload(mut ui_state: ResMut<LobbyUiState>) {
    if ui_state.reload_frames_remaining == 0 || ui_state.audit_fixture_active {
        return;
    }
    ui_state.finish_reload();
}

pub(super) fn sync_lobby_document_bindings(
    session: Res<MyServerSession>,
    ui_state: Res<LobbyUiState>,
    main_world_entry: Res<MainWorldEntryState>,
    contract: Res<LobbyHostContract>,
    mut values: ResMut<UiBindingValues>,
) {
    let connection = ui_state
        .connection_override
        .unwrap_or(session.game_connection_state);
    let selected = ui_state
        .selected_entry_id
        .as_deref()
        .and_then(|entry_id| ui_state.entry(entry_id));
    let items = ui_state
        .entries
        .iter()
        .map(|entry| {
            let selected = ui_state.selected_entry_id.as_deref() == Some(entry.entry_id.as_str());
            let main_world_loading = matches!(entry.target, LobbyEntryTarget::MainWorld)
                && main_world_entry.is_in_flight();
            let loading = main_world_loading
                || ui_state.pending_entry_id.as_deref() == Some(entry.entry_id.as_str());
            UiBindingValue::Record(BTreeMap::from([
                (
                    "entry_id".to_owned(),
                    UiBindingValue::String(entry.entry_id.clone()),
                ),
                (
                    "title".to_owned(),
                    UiBindingValue::String(entry.title.clone()),
                ),
                (
                    "description".to_owned(),
                    UiBindingValue::String(entry.description.clone()),
                ),
                (
                    "badge".to_owned(),
                    UiBindingValue::String(entry.badge.clone()),
                ),
                ("selected".to_owned(), UiBindingValue::Bool(selected)),
                (
                    "disabled".to_owned(),
                    UiBindingValue::Bool(
                        !entry.enabled
                            || ui_state.pending_entry_id.is_some()
                            || main_world_entry.is_in_flight(),
                    ),
                ),
                ("loading".to_owned(), UiBindingValue::Bool(loading)),
            ]))
        })
        .collect::<Vec<_>>();
    let view_state = lobby_view_state(&ui_state, connection);
    let error_visible = ui_state.collection_state == LobbyCollectionState::Error;
    let selected_visible = selected.is_some();

    for (path, value) in [
        ("lobby.games.items", UiBindingValue::List(items)),
        (
            "lobby.games.collection_state",
            UiBindingValue::Enum(ui_state.collection_state.as_str().to_owned()),
        ),
        (
            "lobby.games.view_state",
            UiBindingValue::Enum(view_state.to_owned()),
        ),
        (
            "lobby.games.status",
            UiBindingValue::String(lobby_status_text(&ui_state)),
        ),
        (
            "lobby.connection.status",
            UiBindingValue::String(connection_status_text(connection)),
        ),
        (
            "lobby.games.selected_id",
            UiBindingValue::String(ui_state.selected_entry_id.clone().unwrap_or_default()),
        ),
        (
            "lobby.games.selected_title",
            UiBindingValue::String(
                selected
                    .map(|entry| entry.title.clone())
                    .unwrap_or_else(|| "No game selected".to_owned()),
            ),
        ),
        (
            "lobby.games.selected_detail",
            UiBindingValue::String(
                selected
                    .map(|entry| entry.description.clone())
                    .unwrap_or_default(),
            ),
        ),
        (
            "lobby.games.selected_visibility",
            UiBindingValue::Visibility(if selected_visible {
                UiBindingVisibility::Visible
            } else {
                UiBindingVisibility::Hidden
            }),
        ),
        (
            "lobby.games.error_title",
            UiBindingValue::String(ui_state.error_title.clone()),
        ),
        (
            "lobby.games.error_detail",
            UiBindingValue::String(ui_state.error_detail.clone()),
        ),
        (
            "lobby.games.error_visibility",
            UiBindingValue::Visibility(if error_visible {
                UiBindingVisibility::Visible
            } else {
                UiBindingVisibility::Hidden
            }),
        ),
        (
            "lobby.resources.title",
            UiBindingValue::String(ui_state.resource_title.clone()),
        ),
        (
            "lobby.resources.detail",
            UiBindingValue::String(ui_state.resource_detail.clone()),
        ),
        (
            "lobby.resources.notice_visibility",
            UiBindingValue::Visibility(if ui_state.resource_notice_visible {
                UiBindingVisibility::Visible
            } else {
                UiBindingVisibility::Hidden
            }),
        ),
        (
            "lobby.games.reload_disabled",
            UiBindingValue::Bool(ui_state.reload_frames_remaining > 0),
        ),
        (
            "lobby.games.reload_loading",
            UiBindingValue::Bool(ui_state.collection_state == LobbyCollectionState::Loading),
        ),
    ] {
        let path = UiBindingPath::from_str(path).unwrap();
        let declaration = contract
            .bindings
            .get(&path)
            .expect("Lobby binding schema contains every synchronized value");
        values.set_scoped(
            LOBBY_DOCUMENT_ID,
            OWNER_LOBBY.as_str(),
            &path,
            declaration,
            value,
        );
    }
}

fn lobby_view_state(ui_state: &LobbyUiState, connection: GameConnectionState) -> &'static str {
    if ui_state.collection_state == LobbyCollectionState::Loading
        || ui_state.pending_entry_id.is_some()
    {
        "loading"
    } else if ui_state.collection_state == LobbyCollectionState::Error {
        "error"
    } else if ui_state.confirming_entry_id.is_some() {
        "confirming"
    } else if matches!(
        connection,
        GameConnectionState::Disconnected | GameConnectionState::ReconnectFailed
    ) {
        "disconnected"
    } else if ui_state.entries.is_empty() {
        "empty"
    } else {
        "ready"
    }
}

fn lobby_status_text(ui_state: &LobbyUiState) -> String {
    match ui_state.collection_state {
        LobbyCollectionState::Loading => "Loading available games".to_owned(),
        LobbyCollectionState::Error => "Game list unavailable".to_owned(),
        LobbyCollectionState::Ready if ui_state.entries.is_empty() => {
            "No games are currently available".to_owned()
        }
        LobbyCollectionState::Ready => format!("{} games available", ui_state.entries.len()),
    }
}

fn connection_status_text(state: GameConnectionState) -> String {
    match state {
        GameConnectionState::NotConnected => "Game services offline".to_owned(),
        GameConnectionState::Connecting => "Connecting to game services".to_owned(),
        GameConnectionState::Connected => "Transport connected".to_owned(),
        GameConnectionState::Authenticating => "Authenticating game session".to_owned(),
        GameConnectionState::Authenticated => "Game session connected".to_owned(),
        GameConnectionState::Disconnected => {
            "Connection lost; local entries remain available".to_owned()
        }
        GameConnectionState::Reconnecting => "Reconnecting game session".to_owned(),
        GameConnectionState::ReconnectFailed => {
            "Reconnect failed; retry from the selected game".to_owned()
        }
    }
}

pub(super) fn follow_lobby_document_reload_events(
    mut events: MessageReader<UiDocumentReloadEvent>,
    mut ui_state: ResMut<LobbyUiState>,
) {
    for event in events.read() {
        let report = &event.0;
        if report.document_id.as_str() != LOBBY_DOCUMENT_ID
            || report.owner != OWNER_LOBBY.as_str()
            || ui_state.audit_fixture_active
        {
            continue;
        }
        match report.status {
            UiDocumentReloadStatus::Committed if report.previous_instance.is_some() => {
                ui_state.resource_title = "UI resources updated".to_owned();
                ui_state.resource_detail =
                    "Verified lobby update committed without replacing business state.".to_owned();
                ui_state.resource_notice_visible = true;
            }
            UiDocumentReloadStatus::Failed | UiDocumentReloadStatus::Cancelled
                if report.previous_instance.is_some() =>
            {
                ui_state.resource_title = "UI update retained previous version".to_owned();
                ui_state.resource_detail = report
                    .error
                    .as_ref()
                    .map(|error| format!("{} during {:?}.", error.code, error.stage))
                    .unwrap_or_else(|| "The verified previous lobby remains active.".to_owned());
                ui_state.resource_notice_visible = true;
            }
            _ => {}
        }
    }
}

pub(super) fn cleanup_lobby_screen(
    mut ui_state: ResMut<LobbyUiState>,
    mut focus_state: ResMut<UiFocusState>,
    mut panel_commands: MessageWriter<UiPanelCommand>,
    main_world_entry: Res<MainWorldEntryState>,
    mut main_world_intents: MessageWriter<MainWorldEntryIntent>,
) {
    ui_state.clear_transient();
    focus_state.focused_entity = None;
    panel_commands.write(UiPanelCommand::Close(UI_PANEL_CONFIRM_MODAL));
    panel_commands.write(UiPanelCommand::Close(UI_PANEL_GLOBAL_LOADING));
    if !matches!(
        main_world_entry.phase,
        crate::game::scenes::main_world_entry::MainWorldEntryPhase::Active
            | crate::game::scenes::main_world_entry::MainWorldEntryPhase::HomeActive
    ) {
        main_world_intents.write(MainWorldEntryIntent::Cancel);
    }
}

pub(super) fn close_lobby_loading(
    ui_state: &mut LobbyUiState,
    panel_commands: &mut MessageWriter<UiPanelCommand>,
) {
    ui_state.clear_pending();
    panel_commands.write(UiPanelCommand::Close(UI_PANEL_GLOBAL_LOADING));
}

pub(super) fn touch_ripple_confirm_modal(i18n: &UiI18n) -> UiConfirmModal {
    let title = UiI18nTextSpec::new(i18n, "lobby.touch_ripple.confirm.title", "Touch Ripple");
    let body = UiI18nTextSpec::new(
        i18n,
        "lobby.touch_ripple.confirm.body",
        "Choose how to start this session.",
    );
    let detail = UiI18nTextSpec::new(
        i18n,
        "lobby.touch_ripple.confirm.detail",
        "Single player uses local authority only.",
    );
    let cancel = UiI18nTextSpec::new(i18n, "common.cancel", "Cancel");
    let networked = UiI18nTextSpec::new(i18n, "lobby.touch_ripple.confirm.networked", "Networked");
    let single_player = UiI18nTextSpec::new(
        i18n,
        "lobby.touch_ripple.confirm.single_player",
        "Single Player",
    );
    UiConfirmModal {
        id: MODAL_TOUCH_RIPPLE_LAUNCH,
        title: title.text,
        body: body.text,
        detail: Some(detail.text),
        title_i18n_text: Some(title.i18n_text),
        body_i18n_text: Some(body.i18n_text),
        detail_i18n_text: Some(detail.i18n_text),
        actions: vec![
            UiModalActionSpec {
                label: cancel.text,
                action: ACTION_CANCEL,
                style: UiModalActionStyle::Secondary,
                i18n_text: Some(cancel.i18n_text),
            },
            UiModalActionSpec {
                label: networked.text,
                action: ACTION_TOUCH_RIPPLE_NETWORKED,
                style: UiModalActionStyle::Secondary,
                i18n_text: Some(networked.i18n_text),
            },
            UiModalActionSpec {
                label: single_player.text,
                action: ACTION_TOUCH_RIPPLE_SINGLE_PLAYER,
                style: UiModalActionStyle::Primary,
                i18n_text: Some(single_player.i18n_text),
            },
        ],
    }
}

#[cfg(all(debug_assertions, not(target_os = "android")))]
pub(super) fn prepare_lobby_audit_fixture(
    audit_config: Res<UiAuditConfig>,
    viewport: Res<UiViewport>,
    i18n: Res<UiI18n>,
    mut ui_state: ResMut<LobbyUiState>,
    mut panel_commands: MessageWriter<UiPanelCommand>,
    mut asset_overrides: ResMut<UiDocumentAssetPreflightOverrides>,
) {
    if !audit_config.targets_screen("lobby") || ui_state.audit_fixture_active {
        return;
    }
    let Some(fixture_id) = audit_config.stable_fixture_id() else {
        return;
    };
    set_lobby_audit_image_failure(
        &mut asset_overrides,
        fixture_id == "stage13_lobby_content" && lobby_content_audit_uses_image_failure(&viewport),
    );
    match fixture_id {
        "stage13_lobby_content" => {
            apply_content_audit_fixture(&viewport, &mut ui_state);
        }
        "stage13_lobby_overlays" => {
            ui_state.audit_fixture_active = true;
            if viewport.logical_width >= 1100.0 {
                ui_state.collection_state = LobbyCollectionState::Loading;
                ui_state.entries.clear();
                panel_commands.write(UiPanelCommand::Open(UiPanelRequest::Loading(
                    UiLoading::new("Loading audited lobby entries..."),
                )));
            } else {
                ui_state.confirming_entry_id = Some(ENTRY_TOUCH_RIPPLE.to_owned());
                panel_commands.write(UiPanelCommand::Open(UiPanelRequest::Confirm(
                    touch_ripple_confirm_modal(&i18n),
                )));
            }
        }
        _ => {}
    }
}

#[cfg(all(debug_assertions, not(target_os = "android")))]
pub(super) fn lobby_content_audit_uses_image_failure(viewport: &UiViewport) -> bool {
    viewport.logical_width < 900.0 && viewport.device_scale < 2.5
}

#[cfg(all(debug_assertions, not(target_os = "android")))]
pub(super) fn set_lobby_audit_image_failure(
    overrides: &mut UiDocumentAssetPreflightOverrides,
    failed: bool,
) {
    let document_id =
        UiDocumentId::from_str(LOBBY_DOCUMENT_ID).expect("Lobby document ID is static and valid");
    let asset_id = UiAssetId::from_str(LOBBY_BACKGROUND_ASSET_ID)
        .expect("Lobby background asset ID is static and valid");
    if failed {
        overrides.set(
            document_id,
            asset_id,
            UiDocumentAssetPreflightStatus::Failed {
                code: LOBBY_AUDIT_IMAGE_FAILURE_CODE.to_owned(),
            },
        );
    } else {
        overrides.remove(&document_id, &asset_id);
    }
}

#[cfg(all(debug_assertions, not(target_os = "android")))]
pub(super) fn clear_lobby_audit_image_failure(
    mut overrides: ResMut<UiDocumentAssetPreflightOverrides>,
) {
    set_lobby_audit_image_failure(&mut overrides, false);
}

#[cfg(all(debug_assertions, not(target_os = "android")))]
fn apply_content_audit_fixture(viewport: &UiViewport, ui_state: &mut LobbyUiState) {
    ui_state.audit_fixture_active = true;
    if viewport.logical_width >= 1100.0 && viewport.logical_height <= 740.0 {
        ui_state.entries.clear();
        ui_state.selected_entry_id = None;
        ui_state.connection_override = Some(GameConnectionState::Disconnected);
    } else if viewport.logical_width < 900.0 && viewport.device_scale < 2.5 {
        ui_state.entries = vec![LobbyEntry {
            entry_id: "route:lobby/long-unicode-角色-0000000000000001".to_owned(),
            title: "An Intentionally Long Cooperative Simulation Experience".to_owned(),
            description: "A very long description that must wrap without covering the selection and entry controls on a short landscape viewport.".to_owned(),
            badge: "LONG TEXT".to_owned(),
            target: LobbyEntryTarget::TouchRipple,
            enabled: true,
        }];
        ui_state.selected_entry_id = ui_state.entries.first().map(|entry| entry.entry_id.clone());
    } else if viewport.logical_width < 900.0 {
        ui_state.entries = (0..usize::from(LOBBY_MAX_ENTRIES))
            .map(|index| LobbyEntry {
                entry_id: format!("audit:max-entry:{index:02}"),
                title: format!("Audit Entry {:02}", index + 1),
                description: "Maximum-list deterministic fixture".to_owned(),
                badge: if index % 2 == 0 { "LOCAL" } else { "ONLINE" }.to_owned(),
                target: LobbyEntryTarget::AuditOnly,
                enabled: false,
            })
            .collect();
        ui_state.selected_entry_id = None;
    } else {
        ui_state.entries.clear();
        ui_state.selected_entry_id = None;
        ui_state.collection_state = LobbyCollectionState::Error;
        ui_state.error_title = "Game list request failed".to_owned();
        ui_state.error_detail =
            "Audit fixture: catalog unavailable while the previous UI update remains safe."
                .to_owned();
        ui_state.connection_override = Some(GameConnectionState::ReconnectFailed);
        ui_state.resource_title = "Verified hot update retained".to_owned();
        ui_state.resource_detail =
            "Resource update status remains visible beside the recoverable error.".to_owned();
        ui_state.resource_notice_visible = true;
    }
}
