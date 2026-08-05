use std::{collections::BTreeMap, str::FromStr};

use bevy::prelude::*;

#[cfg(all(debug_assertions, not(target_os = "android")))]
use crate::framework::ui::audit::UiAuditConfig;
#[cfg(all(debug_assertions, not(target_os = "android")))]
use crate::framework::ui::document::UiDocumentRuntime;

use crate::framework::{
    fangyuan::{FangyuanDebugPanelModule, FangyuanDebugPanelState},
    scene::prelude::{SceneCommand, SceneExitRequest},
    ui::{
        core::{UiOwnerId, binding::UiBindingValues, focus::UiFocusState},
        document::{
            UiActionDescriptor, UiActionDispatch, UiActionId, UiActionParamSchema,
            UiActionParamType, UiActionRegistry, UiActionValue, UiBindingDeclaration,
            UiBindingMissingBehavior, UiBindingPath, UiBindingScope, UiBindingType, UiBindingValue,
            UiDocumentId, UiDocumentLayer, UiDocumentPanel, UiHostBindingKey, UiNodeId,
            UiPageState, UiRegisteredActionKind,
        },
    },
};
#[cfg(all(debug_assertions, not(target_os = "android")))]
use crate::game::ui_ids::SCROLL_FANGYUAN_HOME_MAIN;
use crate::game::{
    declarative_screen::{
        DeclarativeScreenFailureDecision, DeclarativeScreenFailurePolicy, DeclarativeScreenHost,
        DeclarativeScreenHostEvent, DeclarativeScreenRegistry, DeclarativeScreenSource,
    },
    navigation::{AppUiMode, GameRouteCommand},
    scenes::{
        FangyuanHomeBlueprintCommand,
        main_world_entry::{MainWorldEntryIntent, MainWorldEntryPhase, MainWorldEntryState},
    },
    ui_ids::{
        OWNER_FANGYUAN_HOME, OWNER_FANGYUAN_PLAYER_PREVIEW, OWNER_MAIN_WORLD,
        OWNER_MAIN_WORLD_MAIL_PANEL, OWNER_MAIN_WORLD_SETTINGS_PANEL, OWNER_ROBOT_SYNC_SCENE,
        OWNER_SAMPLE_SCENE, OWNER_TOUCH_RIPPLE,
    },
};

pub(super) const TOUCH_RIPPLE_DOCUMENT_ID: &str = "game.touch_ripple_hud";
pub(super) const SAMPLE_SCENE_DOCUMENT_ID: &str = "game.sample_scene_hud";
pub(super) const ROBOT_SYNC_DOCUMENT_ID: &str = "game.robot_sync_hud";
pub(super) const FANGYUAN_PLAYER_PREVIEW_DOCUMENT_ID: &str = "game.fangyuan_player_preview_hud";
pub(super) const FANGYUAN_HOME_DOCUMENT_ID: &str = "game.fangyuan_home_hud";
pub(in crate::game) const MAIN_WORLD_HUD_DOCUMENT_ID: &str = "game.main_world_hud";

pub(in crate::game) const MAIN_WORLD_HUD_ROUTE: &str = "main_world";
pub(in crate::game) const MAIN_WORLD_HUD_ROUTE_ALIASES: &[&str] = &["main_world", "main-world"];
pub(in crate::game) const MAIN_WORLD_HUD_SOURCE_PATH: &str = "gameplay/main_world_hud.v1.json";
pub(super) const ACTION_MAIN_WORLD_OPEN_SETTINGS: &str = "main_world.open_settings";
pub(super) const ACTION_MAIN_WORLD_OPEN_MAIL: &str = "main_world.open_mail";
pub(super) const ACTION_MAIN_WORLD_ENTER_HOME: &str = "main_world.enter_home";
pub(super) const ACTION_MAIN_WORLD_RETURN_LOBBY: &str = "main_world.return_lobby";
/// Deferred from this checklist: developer tool pages, the production chat window,
/// and a complete character status bar remain outside the main-world HUD contract.
pub(in crate::game) const MAIN_WORLD_HUD_NON_GOALS: &[&str] = &[
    "developer_tool_pages",
    "production_chat_window",
    "complete_character_status_bar",
];

/// Scene-local document surfaces deliberately remain below the global route state.
/// Their open/close actions may affect UI focus and input blocking, but may not send
/// `GameRouteCommand` or `SceneCommand` directly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::game) enum MainWorldDocumentPanel {
    Settings,
    Mail,
}

impl MainWorldDocumentPanel {
    pub(in crate::game) const fn owner(self) -> UiOwnerId {
        match self {
            Self::Settings => OWNER_MAIN_WORLD_SETTINGS_PANEL,
            Self::Mail => OWNER_MAIN_WORLD_MAIL_PANEL,
        }
    }

    pub(in crate::game) const fn route(self) -> &'static str {
        match self {
            Self::Settings => "main_world_settings",
            Self::Mail => "main_world_mail",
        }
    }

    pub(in crate::game) const fn aliases(self) -> &'static [&'static str] {
        match self {
            Self::Settings => &["main_world_settings", "main-world-settings"],
            Self::Mail => &["main_world_mail", "main-world-mail", "mail"],
        }
    }

    pub(in crate::game) const fn panel(self) -> UiDocumentPanel {
        UiDocumentPanel::Floating
    }

    pub(in crate::game) const fn layer(self) -> UiDocumentLayer {
        UiDocumentLayer::Floating
    }
}

/// The entry coordinator may use this reason to request UI cleanup before it changes
/// a scene session. All reasons share the same panel-to-HUD cleanup order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::game) enum MainWorldUiTeardownCause {
    LeaveToLobby,
    SwitchToHome,
    Logout,
    EnvironmentChanged,
    SessionKicked,
}

impl MainWorldUiTeardownCause {
    pub(in crate::game) const fn cleanup_owners(self) -> &'static [UiOwnerId] {
        let _ = self;
        &MAIN_WORLD_UI_CLEANUP_OWNERS
    }
}

/// Close scene-local panels from top to bottom, then the persistent HUD. The scene
/// coordinator performs scene exit only after this UI cleanup has been requested.
const MAIN_WORLD_UI_CLEANUP_OWNERS: [UiOwnerId; 3] = [
    OWNER_MAIN_WORLD_MAIL_PANEL,
    OWNER_MAIN_WORLD_SETTINGS_PANEL,
    OWNER_MAIN_WORLD,
];

pub(in crate::game) const fn main_world_ui_cleanup_owners() -> &'static [UiOwnerId] {
    MainWorldUiTeardownCause::LeaveToLobby.cleanup_owners()
}

pub(super) const ACTION_TOUCH_RIPPLE_RETURN_LOBBY: &str = "touch_ripple.return_lobby";
pub(super) const ACTION_SAMPLE_SCENE_RETURN_LOBBY: &str = "sample_scene.return_lobby";
pub(super) const ACTION_ROBOT_SYNC_HIDE: &str = "robot_sync.hide_hud";
pub(super) const ACTION_ROBOT_SYNC_SHOW: &str = "robot_sync.show_hud";
pub(super) const ACTION_ROBOT_SYNC_RETURN_LOBBY: &str = "robot_sync.return_lobby";
pub(super) const ACTION_FANGYUAN_PREVIEW_RETURN_LOBBY: &str =
    "fangyuan_player_preview.return_lobby";
pub(super) const ACTION_FANGYUAN_HOME_RELOAD: &str = "fangyuan_home.reload";
pub(super) const ACTION_FANGYUAN_HOME_CLEAR: &str = "fangyuan_home.clear";
pub(super) const ACTION_FANGYUAN_HOME_RERUN_TRIAL: &str = "fangyuan_home.rerun_trial_audit";
pub(super) const ACTION_FANGYUAN_HOME_SWITCH_BUDGET: &str = "fangyuan_home.switch_trial_budget";
pub(super) const ACTION_FANGYUAN_HOME_TOGGLE_DEBUG: &str = "fangyuan_home.toggle_debug";
pub(super) const ACTION_FANGYUAN_HOME_TOGGLE_MODULE: &str = "fangyuan_home.toggle_debug_module";
pub(super) const ACTION_FANGYUAN_HOME_RETURN_LOBBY: &str = "fangyuan_home.return_lobby";

const DEFAULT_AUDIT_PROFILES: [&str; 4] = [
    "desktop",
    "phone-landscape",
    "phone-1080p-landscape",
    "tablet-landscape",
];

const TOUCH_RIPPLE_SOURCE_PATH: &str = "gameplay/touch_ripple_hud.v1.json";
const SAMPLE_SCENE_SOURCE_PATH: &str = "gameplay/sample_scene_hud.v1.json";
const ROBOT_SYNC_SOURCE_PATH: &str = "gameplay/robot_sync_hud.v1.json";
const FANGYUAN_PREVIEW_SOURCE_PATH: &str = "gameplay/fangyuan_player_preview_hud.v1.json";
const FANGYUAN_HOME_SOURCE_PATH: &str = "gameplay/fangyuan_home_hud.v1.json";

const TOUCH_RIPPLE_SOURCE: &str =
    include_str!("../../../../assets/ui/documents/approved/gameplay/touch_ripple_hud.v1.json");
const SAMPLE_SCENE_SOURCE: &str =
    include_str!("../../../../assets/ui/documents/approved/gameplay/sample_scene_hud.v1.json");
const ROBOT_SYNC_SOURCE: &str =
    include_str!("../../../../assets/ui/documents/approved/gameplay/robot_sync_hud.v1.json");
const FANGYUAN_PREVIEW_SOURCE: &str = include_str!(
    "../../../../assets/ui/documents/approved/gameplay/fangyuan_player_preview_hud.v1.json"
);
const FANGYUAN_HOME_SOURCE: &str =
    include_str!("../../../../assets/ui/documents/approved/gameplay/fangyuan_home_hud.v1.json");
const MAIN_WORLD_HUD_SOURCE: &str =
    include_str!("../../../../assets/ui/documents/approved/gameplay/main_world_hud.v1.json");

const MAIN_WORLD_SETTINGS_NODE: &str = "main_world.settings";
const MAIN_WORLD_MAIL_NODE: &str = "main_world.mail";
const MAIN_WORLD_HOME_NODE: &str = "main_world.home";
const MAIN_WORLD_RETURN_LOBBY_NODE: &str = "main_world.return_lobby";
const TOUCH_RETURN_NODE: &str = "touch_ripple.return_lobby";
const SAMPLE_RETURN_NODE: &str = "sample_scene.return_lobby";
const ROBOT_HIDE_NODE: &str = "robot_sync.hide_hud";
const ROBOT_SHOW_NODE: &str = "robot_sync.show_hud";
const ROBOT_RETURN_NODE: &str = "robot_sync.return_lobby";
const PREVIEW_RETURN_NODE: &str = "fangyuan_player_preview.return_lobby";
const HOME_RELOAD_NODE: &str = "fangyuan_home.reload";
const HOME_CLEAR_NODE: &str = "fangyuan_home.clear";
const HOME_RERUN_NODE: &str = "fangyuan_home.rerun_trial_audit";
const HOME_BUDGET_NODE: &str = "fangyuan_home.switch_trial_budget";
const HOME_DEBUG_NODE: &str = "fangyuan_home.toggle_debug";
const HOME_RETURN_NODE: &str = "fangyuan_home.return_lobby";

const HOME_MODULE_SOURCES: [(&str, FangyuanDebugPanelModule); 6] = [
    (
        "fangyuan_home.module.render",
        FangyuanDebugPanelModule::Render,
    ),
    ("fangyuan_home.module.lod", FangyuanDebugPanelModule::Lod),
    (
        "fangyuan_home.module.cache",
        FangyuanDebugPanelModule::Cache,
    ),
    ("fangyuan_home.module.bake", FangyuanDebugPanelModule::Bake),
    (
        "fangyuan_home.module.audit",
        FangyuanDebugPanelModule::Audit,
    ),
    (
        "fangyuan_home.module.trial",
        FangyuanDebugPanelModule::Trial,
    ),
];

#[derive(Resource)]
pub(super) struct GameplayHudHostContract {
    bindings: BTreeMap<&'static str, BTreeMap<UiBindingPath, UiBindingDeclaration>>,
}

impl Default for GameplayHudHostContract {
    fn default() -> Self {
        Self {
            bindings: BTreeMap::from([
                (MAIN_WORLD_HUD_DOCUMENT_ID, BTreeMap::new()),
                (TOUCH_RIPPLE_DOCUMENT_ID, BTreeMap::new()),
                (SAMPLE_SCENE_DOCUMENT_ID, BTreeMap::new()),
                (
                    ROBOT_SYNC_DOCUMENT_ID,
                    binding_schema([
                        ("robot_sync.title", UiBindingType::String),
                        ("robot_sync.status", UiBindingType::String),
                        ("robot_sync.details_visibility", UiBindingType::Visibility),
                        ("robot_sync.hide_visibility", UiBindingType::Visibility),
                        ("robot_sync.show_visibility", UiBindingType::Visibility),
                    ]),
                ),
                (FANGYUAN_PLAYER_PREVIEW_DOCUMENT_ID, BTreeMap::new()),
                (
                    FANGYUAN_HOME_DOCUMENT_ID,
                    binding_schema([
                        ("fangyuan_home.status", UiBindingType::String),
                        ("fangyuan_home.debug.text", UiBindingType::String),
                        ("fangyuan_home.debug.visibility", UiBindingType::Visibility),
                    ]),
                ),
            ]),
        }
    }
}

fn binding_schema<const N: usize>(
    specs: [(&str, UiBindingType); N],
) -> BTreeMap<UiBindingPath, UiBindingDeclaration> {
    specs
        .into_iter()
        .map(|(path, value_type)| {
            (
                UiBindingPath::from_str(path).expect("gameplay HUD binding path is static"),
                UiBindingDeclaration {
                    scope: UiBindingScope::Owner,
                    value_type,
                    default: None,
                    missing: UiBindingMissingBehavior::UseConsumerFallback,
                },
            )
        })
        .collect()
}

pub(super) fn register_gameplay_hud_contracts(
    contract: Res<GameplayHudHostContract>,
    mut actions: ResMut<UiActionRegistry>,
    mut screens: ResMut<DeclarativeScreenRegistry>,
) {
    for descriptor in gameplay_action_descriptors() {
        actions
            .register(descriptor)
            .expect("gameplay HUD action registration must be unique and valid");
    }
    for host in gameplay_declarative_screen_hosts(&contract) {
        screens
            .register(host)
            .expect("gameplay HUD screen registration must be unique and valid");
    }
}

pub(super) fn gameplay_declarative_screen_hosts(
    contract: &GameplayHudHostContract,
) -> Vec<DeclarativeScreenHost> {
    vec![
        host(
            contract,
            MAIN_WORLD_HUD_DOCUMENT_ID,
            MAIN_WORLD_HUD_ROUTE,
            MAIN_WORLD_HUD_ROUTE_ALIASES,
            AppUiMode::MainWorld,
            OWNER_MAIN_WORLD,
            MAIN_WORLD_HUD_SOURCE_PATH,
            MAIN_WORLD_HUD_SOURCE,
            &[
                ACTION_MAIN_WORLD_OPEN_SETTINGS,
                ACTION_MAIN_WORLD_OPEN_MAIL,
                ACTION_MAIN_WORLD_ENTER_HOME,
                ACTION_MAIN_WORLD_RETURN_LOBBY,
            ],
        ),
        host(
            contract,
            TOUCH_RIPPLE_DOCUMENT_ID,
            "touch_ripple_hud",
            &["touch_ripple_hud", "touch-ripple-hud"],
            AppUiMode::WanfaTouchRipple,
            OWNER_TOUCH_RIPPLE,
            TOUCH_RIPPLE_SOURCE_PATH,
            TOUCH_RIPPLE_SOURCE,
            &[ACTION_TOUCH_RIPPLE_RETURN_LOBBY],
        ),
        host(
            contract,
            SAMPLE_SCENE_DOCUMENT_ID,
            "sample_scene_hud",
            &["sample_scene_hud", "sample-scene-hud"],
            AppUiMode::SampleScene,
            OWNER_SAMPLE_SCENE,
            SAMPLE_SCENE_SOURCE_PATH,
            SAMPLE_SCENE_SOURCE,
            &[ACTION_SAMPLE_SCENE_RETURN_LOBBY],
        ),
        host(
            contract,
            ROBOT_SYNC_DOCUMENT_ID,
            "robot_sync_hud",
            &["robot_sync_hud", "robot-sync-hud"],
            AppUiMode::RobotSyncScene,
            OWNER_ROBOT_SYNC_SCENE,
            ROBOT_SYNC_SOURCE_PATH,
            ROBOT_SYNC_SOURCE,
            &[
                ACTION_ROBOT_SYNC_HIDE,
                ACTION_ROBOT_SYNC_SHOW,
                ACTION_ROBOT_SYNC_RETURN_LOBBY,
            ],
        ),
        host(
            contract,
            FANGYUAN_PLAYER_PREVIEW_DOCUMENT_ID,
            "fangyuan_player_preview_hud",
            &["fangyuan_player_preview_hud", "fangyuan-player-preview-hud"],
            AppUiMode::FangyuanPlayerPreview,
            OWNER_FANGYUAN_PLAYER_PREVIEW,
            FANGYUAN_PREVIEW_SOURCE_PATH,
            FANGYUAN_PREVIEW_SOURCE,
            &[ACTION_FANGYUAN_PREVIEW_RETURN_LOBBY],
        ),
        host(
            contract,
            FANGYUAN_HOME_DOCUMENT_ID,
            "fangyuan_home_hud",
            &["fangyuan_home_hud", "fangyuan-home-hud"],
            AppUiMode::FangyuanHome,
            OWNER_FANGYUAN_HOME,
            FANGYUAN_HOME_SOURCE_PATH,
            FANGYUAN_HOME_SOURCE,
            &[
                ACTION_FANGYUAN_HOME_RELOAD,
                ACTION_FANGYUAN_HOME_CLEAR,
                ACTION_FANGYUAN_HOME_RERUN_TRIAL,
                ACTION_FANGYUAN_HOME_SWITCH_BUDGET,
                ACTION_FANGYUAN_HOME_TOGGLE_DEBUG,
                ACTION_FANGYUAN_HOME_TOGGLE_MODULE,
                ACTION_FANGYUAN_HOME_RETURN_LOBBY,
            ],
        ),
    ]
}

#[allow(clippy::too_many_arguments)]
fn host(
    contract: &GameplayHudHostContract,
    document_id: &'static str,
    route: &'static str,
    route_aliases: &'static [&'static str],
    mode: AppUiMode,
    owner: crate::framework::ui::core::UiOwnerId,
    source_path: &'static str,
    source_json: &'static str,
    action_ids: &[&str],
) -> DeclarativeScreenHost {
    let source = DeclarativeScreenSource::approved(source_path, source_json);
    let bindings = contract
        .bindings
        .get(document_id)
        .expect("every gameplay HUD has a host binding schema");
    DeclarativeScreenHost {
        document_id: UiDocumentId::from_str(document_id).expect("gameplay document ID is static"),
        route,
        route_aliases,
        mode: Some(mode),
        owner,
        panel: UiDocumentPanel::Hud,
        layer: UiDocumentLayer::Page,
        initial_state: UiPageState::initial(),
        binding_schema: bindings
            .iter()
            .map(|(path, declaration)| {
                (
                    UiHostBindingKey::new(declaration.scope, path.clone()),
                    declaration.value_type.clone(),
                )
            })
            .collect(),
        action_allowlist: action_ids
            .iter()
            .map(|id| UiActionId::from_str(id).expect("gameplay action ID is static"))
            .collect(),
        audit_profiles: DEFAULT_AUDIT_PROFILES.map(str::to_owned).to_vec(),
        source: source.clone(),
        fallback_source: Some(source),
        failure_policy: DeclarativeScreenFailurePolicy::PackagedFallback,
    }
}

pub(super) fn gameplay_action_descriptors() -> Vec<UiActionDescriptor> {
    let mut descriptors = vec![
        action(
            MAIN_WORLD_HUD_DOCUMENT_ID,
            OWNER_MAIN_WORLD.as_str(),
            ACTION_MAIN_WORLD_OPEN_SETTINGS,
            MAIN_WORLD_SETTINGS_NODE,
        ),
        action(
            MAIN_WORLD_HUD_DOCUMENT_ID,
            OWNER_MAIN_WORLD.as_str(),
            ACTION_MAIN_WORLD_OPEN_MAIL,
            MAIN_WORLD_MAIL_NODE,
        ),
        action(
            MAIN_WORLD_HUD_DOCUMENT_ID,
            OWNER_MAIN_WORLD.as_str(),
            ACTION_MAIN_WORLD_ENTER_HOME,
            MAIN_WORLD_HOME_NODE,
        ),
        action(
            MAIN_WORLD_HUD_DOCUMENT_ID,
            OWNER_MAIN_WORLD.as_str(),
            ACTION_MAIN_WORLD_RETURN_LOBBY,
            MAIN_WORLD_RETURN_LOBBY_NODE,
        ),
        action(
            TOUCH_RIPPLE_DOCUMENT_ID,
            OWNER_TOUCH_RIPPLE.as_str(),
            ACTION_TOUCH_RIPPLE_RETURN_LOBBY,
            TOUCH_RETURN_NODE,
        ),
        action(
            SAMPLE_SCENE_DOCUMENT_ID,
            OWNER_SAMPLE_SCENE.as_str(),
            ACTION_SAMPLE_SCENE_RETURN_LOBBY,
            SAMPLE_RETURN_NODE,
        ),
        action(
            ROBOT_SYNC_DOCUMENT_ID,
            OWNER_ROBOT_SYNC_SCENE.as_str(),
            ACTION_ROBOT_SYNC_HIDE,
            ROBOT_HIDE_NODE,
        ),
        action(
            ROBOT_SYNC_DOCUMENT_ID,
            OWNER_ROBOT_SYNC_SCENE.as_str(),
            ACTION_ROBOT_SYNC_SHOW,
            ROBOT_SHOW_NODE,
        ),
        action(
            ROBOT_SYNC_DOCUMENT_ID,
            OWNER_ROBOT_SYNC_SCENE.as_str(),
            ACTION_ROBOT_SYNC_RETURN_LOBBY,
            ROBOT_RETURN_NODE,
        ),
        action(
            FANGYUAN_PLAYER_PREVIEW_DOCUMENT_ID,
            OWNER_FANGYUAN_PLAYER_PREVIEW.as_str(),
            ACTION_FANGYUAN_PREVIEW_RETURN_LOBBY,
            PREVIEW_RETURN_NODE,
        ),
        action(
            FANGYUAN_HOME_DOCUMENT_ID,
            OWNER_FANGYUAN_HOME.as_str(),
            ACTION_FANGYUAN_HOME_RELOAD,
            HOME_RELOAD_NODE,
        ),
        action(
            FANGYUAN_HOME_DOCUMENT_ID,
            OWNER_FANGYUAN_HOME.as_str(),
            ACTION_FANGYUAN_HOME_CLEAR,
            HOME_CLEAR_NODE,
        ),
        action(
            FANGYUAN_HOME_DOCUMENT_ID,
            OWNER_FANGYUAN_HOME.as_str(),
            ACTION_FANGYUAN_HOME_RERUN_TRIAL,
            HOME_RERUN_NODE,
        ),
        action(
            FANGYUAN_HOME_DOCUMENT_ID,
            OWNER_FANGYUAN_HOME.as_str(),
            ACTION_FANGYUAN_HOME_SWITCH_BUDGET,
            HOME_BUDGET_NODE,
        ),
        action(
            FANGYUAN_HOME_DOCUMENT_ID,
            OWNER_FANGYUAN_HOME.as_str(),
            ACTION_FANGYUAN_HOME_TOGGLE_DEBUG,
            HOME_DEBUG_NODE,
        ),
        action(
            FANGYUAN_HOME_DOCUMENT_ID,
            OWNER_FANGYUAN_HOME.as_str(),
            ACTION_FANGYUAN_HOME_RETURN_LOBBY,
            HOME_RETURN_NODE,
        ),
    ];
    descriptors.push(
        UiActionDescriptor::new(
            UiActionId::from_str(ACTION_FANGYUAN_HOME_TOGGLE_MODULE).unwrap(),
            UiDocumentId::from_str(FANGYUAN_HOME_DOCUMENT_ID).unwrap(),
            OWNER_FANGYUAN_HOME.as_str(),
            business_command(ACTION_FANGYUAN_HOME_TOGGLE_MODULE),
        )
        .with_sources(
            HOME_MODULE_SOURCES
                .iter()
                .map(|(source, _)| UiNodeId::from_str(source).unwrap()),
        )
        .with_param(
            "module",
            UiActionParamSchema::required(UiActionParamType::Enum {
                values: HOME_MODULE_SOURCES
                    .iter()
                    .map(|(_, module)| module.as_str().to_owned())
                    .collect(),
            }),
        ),
    );
    descriptors
}

fn action(document_id: &str, owner: &str, action_id: &str, source: &str) -> UiActionDescriptor {
    UiActionDescriptor::new(
        UiActionId::from_str(action_id).unwrap(),
        UiDocumentId::from_str(document_id).unwrap(),
        owner,
        business_command(action_id),
    )
    .with_source(UiNodeId::from_str(source).unwrap())
}

fn business_command(target: &str) -> UiRegisteredActionKind {
    UiRegisteredActionKind::BusinessCommand {
        target: target.to_owned(),
    }
}

/// The fixed HUD already has a packaged fallback. If that fallback cannot load for
/// the active generation, delegate the controlled exit to the entry coordinator.
pub(super) fn recover_from_main_world_hud_failure(
    entry: Option<Res<MainWorldEntryState>>,
    mut host_events: MessageReader<DeclarativeScreenHostEvent>,
    mut intents: MessageWriter<MainWorldEntryIntent>,
) {
    let fallback_failed = host_events.read().any(|event| {
        matches!(
            event,
            DeclarativeScreenHostEvent::LoadFailed {
                document_id,
                owner,
                decision: DeclarativeScreenFailureDecision::NoFallbackAvailable,
                ..
            } if document_id.as_str() == MAIN_WORLD_HUD_DOCUMENT_ID
                && owner == OWNER_MAIN_WORLD.as_str()
        )
    });
    if entry.is_some_and(|entry| entry.phase == MainWorldEntryPhase::Active) && fallback_failed {
        intents.write(MainWorldEntryIntent::ExitToLobby);
    }
}

pub(super) fn handle_gameplay_hud_document_actions(
    mut debug_panel_state: ResMut<FangyuanDebugPanelState>,
    mut robot_visibility: ResMut<super::robot_sync_scene::RobotSyncHudVisibility>,
    mut blueprint_commands: MessageWriter<FangyuanHomeBlueprintCommand>,
    mut scene_commands: MessageWriter<SceneCommand>,
    mut route_commands: MessageWriter<GameRouteCommand>,
    mut main_world_intents: MessageWriter<MainWorldEntryIntent>,
    mut actions: MessageReader<UiActionDispatch>,
) {
    for dispatch in actions.read() {
        if let Some(module) = debug_module_action(dispatch) {
            debug_panel_state.toggle_module(module);
            continue;
        }
        if !zero_param_action_matches_contract(dispatch) {
            continue;
        }
        match dispatch.action.as_str() {
            ACTION_TOUCH_RIPPLE_RETURN_LOBBY | ACTION_FANGYUAN_PREVIEW_RETURN_LOBBY => {
                route_commands.write(GameRouteCommand::ChangeMode(AppUiMode::Lobby));
            }
            ACTION_SAMPLE_SCENE_RETURN_LOBBY | ACTION_ROBOT_SYNC_RETURN_LOBBY => {
                scene_commands.write(SceneCommand::Exit(SceneExitRequest::default()));
                route_commands.write(GameRouteCommand::ChangeMode(AppUiMode::Lobby));
            }
            ACTION_FANGYUAN_HOME_RETURN_LOBBY => {
                main_world_intents.write(MainWorldEntryIntent::ReturnFromHome);
            }
            ACTION_ROBOT_SYNC_HIDE => robot_visibility.show_details = false,
            ACTION_ROBOT_SYNC_SHOW => robot_visibility.show_details = true,
            ACTION_FANGYUAN_HOME_RELOAD => {
                blueprint_commands.write(FangyuanHomeBlueprintCommand::Reload);
            }
            ACTION_FANGYUAN_HOME_CLEAR => {
                blueprint_commands.write(FangyuanHomeBlueprintCommand::Clear);
            }
            ACTION_FANGYUAN_HOME_RERUN_TRIAL => {
                blueprint_commands.write(FangyuanHomeBlueprintCommand::RerunTrialAudit);
            }
            ACTION_FANGYUAN_HOME_SWITCH_BUDGET => {
                blueprint_commands.write(FangyuanHomeBlueprintCommand::SwitchTrialBudget);
            }
            ACTION_FANGYUAN_HOME_TOGGLE_DEBUG => {
                debug_panel_state.toggle_visible();
            }
            _ => {}
        }
    }
}

fn zero_param_action_matches_contract(action: &UiActionDispatch) -> bool {
    if !action.params.is_empty() || !business_target_matches_action(action) {
        return false;
    }
    matches!(
        (
            action.document_id.as_str(),
            action.owner.as_str(),
            action.action.as_str(),
            action.source_node.as_str(),
        ),
        (
            MAIN_WORLD_HUD_DOCUMENT_ID,
            "main_world",
            ACTION_MAIN_WORLD_OPEN_SETTINGS,
            MAIN_WORLD_SETTINGS_NODE,
        ) | (
            MAIN_WORLD_HUD_DOCUMENT_ID,
            "main_world",
            ACTION_MAIN_WORLD_OPEN_MAIL,
            MAIN_WORLD_MAIL_NODE,
        ) | (
            MAIN_WORLD_HUD_DOCUMENT_ID,
            "main_world",
            ACTION_MAIN_WORLD_ENTER_HOME,
            MAIN_WORLD_HOME_NODE,
        ) | (
            MAIN_WORLD_HUD_DOCUMENT_ID,
            "main_world",
            ACTION_MAIN_WORLD_RETURN_LOBBY,
            MAIN_WORLD_RETURN_LOBBY_NODE,
        ) | (
            TOUCH_RIPPLE_DOCUMENT_ID,
            "wanfa_touch_ripple",
            ACTION_TOUCH_RIPPLE_RETURN_LOBBY,
            TOUCH_RETURN_NODE,
        ) | (
            SAMPLE_SCENE_DOCUMENT_ID,
            "sample_scene",
            ACTION_SAMPLE_SCENE_RETURN_LOBBY,
            SAMPLE_RETURN_NODE,
        ) | (
            ROBOT_SYNC_DOCUMENT_ID,
            "robot_sync_scene",
            ACTION_ROBOT_SYNC_HIDE,
            ROBOT_HIDE_NODE,
        ) | (
            ROBOT_SYNC_DOCUMENT_ID,
            "robot_sync_scene",
            ACTION_ROBOT_SYNC_SHOW,
            ROBOT_SHOW_NODE,
        ) | (
            ROBOT_SYNC_DOCUMENT_ID,
            "robot_sync_scene",
            ACTION_ROBOT_SYNC_RETURN_LOBBY,
            ROBOT_RETURN_NODE,
        ) | (
            FANGYUAN_PLAYER_PREVIEW_DOCUMENT_ID,
            "fangyuan_player_preview",
            ACTION_FANGYUAN_PREVIEW_RETURN_LOBBY,
            PREVIEW_RETURN_NODE,
        ) | (
            FANGYUAN_HOME_DOCUMENT_ID,
            "fangyuan_home",
            ACTION_FANGYUAN_HOME_RELOAD,
            HOME_RELOAD_NODE,
        ) | (
            FANGYUAN_HOME_DOCUMENT_ID,
            "fangyuan_home",
            ACTION_FANGYUAN_HOME_CLEAR,
            HOME_CLEAR_NODE,
        ) | (
            FANGYUAN_HOME_DOCUMENT_ID,
            "fangyuan_home",
            ACTION_FANGYUAN_HOME_RERUN_TRIAL,
            HOME_RERUN_NODE,
        ) | (
            FANGYUAN_HOME_DOCUMENT_ID,
            "fangyuan_home",
            ACTION_FANGYUAN_HOME_SWITCH_BUDGET,
            HOME_BUDGET_NODE,
        ) | (
            FANGYUAN_HOME_DOCUMENT_ID,
            "fangyuan_home",
            ACTION_FANGYUAN_HOME_TOGGLE_DEBUG,
            HOME_DEBUG_NODE,
        ) | (
            FANGYUAN_HOME_DOCUMENT_ID,
            "fangyuan_home",
            ACTION_FANGYUAN_HOME_RETURN_LOBBY,
            HOME_RETURN_NODE,
        )
    )
}

fn business_target_matches_action(action: &UiActionDispatch) -> bool {
    matches!(
        &action.kind,
        UiRegisteredActionKind::BusinessCommand { target }
            if target == action.action.as_str()
    )
}

fn debug_module_action(action: &UiActionDispatch) -> Option<FangyuanDebugPanelModule> {
    if !business_target_matches_action(action)
        || action.document_id.as_str() != FANGYUAN_HOME_DOCUMENT_ID
        || action.owner.as_str() != OWNER_FANGYUAN_HOME.as_str()
        || action.action.as_str() != ACTION_FANGYUAN_HOME_TOGGLE_MODULE
        || action.params.len() != 1
    {
        return None;
    }
    let UiActionValue::Enum(module_name) = action.params.get("module")? else {
        return None;
    };
    HOME_MODULE_SOURCES.iter().find_map(|(source, module)| {
        (action.source_node.as_str() == *source && module.as_str() == module_name)
            .then_some(*module)
    })
}

pub(super) fn set_binding(
    contract: &GameplayHudHostContract,
    values: &mut UiBindingValues,
    document_id: &'static str,
    owner: &str,
    path: &str,
    value: UiBindingValue,
) {
    let path = UiBindingPath::from_str(path).unwrap();
    let declaration = contract
        .bindings
        .get(document_id)
        .and_then(|bindings| bindings.get(&path))
        .expect("gameplay HUD binding schema contains synchronized value");
    values.set_scoped(document_id, owner, &path, declaration, value);
}

pub(super) fn reset_robot_sync_hud_visibility(
    mut visibility: ResMut<super::robot_sync_scene::RobotSyncHudVisibility>,
) {
    visibility.show_details = true;
}

pub(super) fn cleanup_gameplay_hud_focus(mut focus: ResMut<UiFocusState>) {
    focus.focused_entity = None;
}

#[cfg(all(debug_assertions, not(target_os = "android")))]
pub(super) fn prepare_gameplay_hud_audit_fixture(
    audit: Res<UiAuditConfig>,
    mut robot_visibility: ResMut<super::robot_sync_scene::RobotSyncHudVisibility>,
    mut debug_panel_state: ResMut<FangyuanDebugPanelState>,
) {
    if audit.stable_fixture_id() != Some("stage15_gameplay_hud_key_states") {
        return;
    }
    if audit.targets_screen("robot_sync_scene") {
        robot_visibility.show_details = false;
    }
    if audit.targets_screen("fangyuan_home") {
        debug_panel_state.visible = true;
    }
}

#[cfg(all(debug_assertions, not(target_os = "android")))]
pub(super) fn mark_fangyuan_home_audit_scroll(
    audit: Res<UiAuditConfig>,
    runtime: Res<UiDocumentRuntime>,
    mut commands: Commands,
) {
    if !audit.targets_screen("fangyuan_home") {
        return;
    }
    let document_id =
        UiDocumentId::from_str(FANGYUAN_HOME_DOCUMENT_ID).expect("Fangyuan document ID is static");
    let Some(instance) = runtime.active_instance(OWNER_FANGYUAN_HOME.as_str(), &document_id) else {
        return;
    };
    let node_id = if audit.stable_fixture_id() == Some("stage15_gameplay_hud_key_states") {
        "fangyuan_home.debug_panel"
    } else {
        "fangyuan_home.status_panel"
    };
    let node_id = UiNodeId::from_str(node_id).expect("Fangyuan scroll node ID is static");
    let Some(scroll) = runtime.node_entity(instance, &node_id) else {
        return;
    };
    commands.entity(scroll).insert(SCROLL_FANGYUAN_HOME_MAIN);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framework::{
        scene::prelude::SceneCommand,
        ui::{
            core::{UiMetrics, UiViewport},
            document::{
                UiDocument, UiDocumentPreviewPlugin, UiDocumentRuntime, UiDocumentRuntimePlugin,
                parse_approved_document_registration,
            },
            style::{UiFontAssets, UiTheme},
        },
    };
    use bevy::{
        ecs::message::{MessageCursor, Messages},
        picking::Pickable,
        state::app::StatesPlugin,
    };

    const DOCUMENTS: [(&str, &str); 6] = [
        (MAIN_WORLD_HUD_DOCUMENT_ID, MAIN_WORLD_HUD_SOURCE),
        (TOUCH_RIPPLE_DOCUMENT_ID, TOUCH_RIPPLE_SOURCE),
        (SAMPLE_SCENE_DOCUMENT_ID, SAMPLE_SCENE_SOURCE),
        (ROBOT_SYNC_DOCUMENT_ID, ROBOT_SYNC_SOURCE),
        (FANGYUAN_PLAYER_PREVIEW_DOCUMENT_ID, FANGYUAN_PREVIEW_SOURCE),
        (FANGYUAN_HOME_DOCUMENT_ID, FANGYUAN_HOME_SOURCE),
    ];

    const REGISTRATIONS: [&str; 6] = [
        include_str!(
            "../../../../assets/ui/documents/approved/gameplay/main_world_hud.promotion.v1.json"
        ),
        include_str!(
            "../../../../assets/ui/documents/approved/gameplay/touch_ripple_hud.promotion.v1.json"
        ),
        include_str!(
            "../../../../assets/ui/documents/approved/gameplay/sample_scene_hud.promotion.v1.json"
        ),
        include_str!(
            "../../../../assets/ui/documents/approved/gameplay/robot_sync_hud.promotion.v1.json"
        ),
        include_str!(
            "../../../../assets/ui/documents/approved/gameplay/fangyuan_player_preview_hud.promotion.v1.json"
        ),
        include_str!(
            "../../../../assets/ui/documents/approved/gameplay/fangyuan_home_hud.promotion.v1.json"
        ),
    ];

    #[test]
    fn approved_gameplay_hud_documents_and_promotions_match_fixed_hosts() {
        let contract = GameplayHudHostContract::default();
        let hosts = gameplay_declarative_screen_hosts(&contract);
        assert_eq!(hosts.len(), 6);

        for ((document_id, source), registration_source) in DOCUMENTS.into_iter().zip(REGISTRATIONS)
        {
            let validation = UiDocument::validate_json(source);
            assert!(
                validation.report.valid,
                "{document_id}: {:#?}",
                validation.report.diagnostics
            );
            let registration = parse_approved_document_registration(registration_source).unwrap();
            let audit = registration.audit_report(source).unwrap();
            let host = hosts
                .iter()
                .find(|host| host.document_id.as_str() == document_id)
                .unwrap();

            assert_eq!(host.panel, UiDocumentPanel::Hud);
            assert_eq!(host.layer, UiDocumentLayer::Page);
            assert_eq!(host.audit_profiles, DEFAULT_AUDIT_PROFILES);
            assert_eq!(registration.panel(), UiDocumentPanel::Hud);
            assert_eq!(registration.layer(), UiDocumentLayer::Page);
            assert_eq!(registration.owner(), host.owner.as_str());
            assert_eq!(registration.route(), host.route);
            assert_eq!(audit.actions.len(), host.action_allowlist.len());
            assert_eq!(audit.bindings.len(), host.binding_schema.len());
        }
    }

    #[test]
    fn main_world_ui_contract_keeps_scene_panels_below_the_fixed_hud_route() {
        assert_eq!(MAIN_WORLD_HUD_DOCUMENT_ID, "game.main_world_hud");
        assert_eq!(MAIN_WORLD_HUD_ROUTE, "main_world");
        assert_eq!(MAIN_WORLD_HUD_ROUTE_ALIASES, ["main_world", "main-world"]);
        assert_eq!(
            MAIN_WORLD_HUD_SOURCE_PATH,
            "gameplay/main_world_hud.v1.json"
        );
        assert_eq!(AppUiMode::MainWorld.ui_owner(), OWNER_MAIN_WORLD);

        for panel in [
            MainWorldDocumentPanel::Settings,
            MainWorldDocumentPanel::Mail,
        ] {
            assert_eq!(panel.panel(), UiDocumentPanel::Floating);
            assert_eq!(panel.layer(), UiDocumentLayer::Floating);
            assert!(panel.aliases().contains(&panel.route()));
        }
        assert_eq!(
            main_world_ui_cleanup_owners(),
            [
                OWNER_MAIN_WORLD_MAIL_PANEL,
                OWNER_MAIN_WORLD_SETTINGS_PANEL,
                OWNER_MAIN_WORLD,
            ]
        );
        for cause in [
            MainWorldUiTeardownCause::LeaveToLobby,
            MainWorldUiTeardownCause::SwitchToHome,
            MainWorldUiTeardownCause::Logout,
            MainWorldUiTeardownCause::EnvironmentChanged,
            MainWorldUiTeardownCause::SessionKicked,
        ] {
            assert_eq!(cause.cleanup_owners(), main_world_ui_cleanup_owners());
        }
        assert_eq!(
            MAIN_WORLD_HUD_NON_GOALS,
            [
                "developer_tool_pages",
                "production_chat_window",
                "complete_character_status_bar",
            ]
        );
    }

    #[test]
    fn gameplay_actions_are_closed_to_exact_sources_and_module_values() {
        let descriptors = gameplay_action_descriptors();
        assert_eq!(descriptors.len(), 17);
        assert!(
            descriptors
                .iter()
                .all(|descriptor| !descriptor.sources.is_empty())
        );

        let main_world_actions = descriptors
            .iter()
            .filter(|descriptor| descriptor.document_id.as_str() == MAIN_WORLD_HUD_DOCUMENT_ID)
            .collect::<Vec<_>>();
        assert_eq!(main_world_actions.len(), 4);
        assert!(
            main_world_actions
                .iter()
                .all(|descriptor| descriptor.params.is_empty())
        );

        let module = descriptors
            .iter()
            .find(|descriptor| descriptor.id.as_str() == ACTION_FANGYUAN_HOME_TOGGLE_MODULE)
            .unwrap();
        assert_eq!(module.sources.len(), 6);
        assert_eq!(module.params.len(), 1);
    }

    #[test]
    fn gameplay_action_adapter_revalidates_sources_and_parameters() {
        let mut app = action_test_app();
        app.world_mut().write_message(dispatch(
            SAMPLE_SCENE_DOCUMENT_ID,
            OWNER_SAMPLE_SCENE.as_str(),
            ACTION_SAMPLE_SCENE_RETURN_LOBBY,
            "sample_scene.forged",
            BTreeMap::new(),
        ));
        app.world_mut().write_message(dispatch(
            FANGYUAN_HOME_DOCUMENT_ID,
            OWNER_FANGYUAN_HOME.as_str(),
            ACTION_FANGYUAN_HOME_TOGGLE_MODULE,
            "fangyuan_home.module.cache",
            BTreeMap::from([("module".to_owned(), UiActionValue::Enum("cache".to_owned()))]),
        ));
        app.update();

        assert!(read_messages::<SceneCommand>(&app).is_empty());
        assert!(read_messages::<GameRouteCommand>(&app).is_empty());
        assert!(
            !app.world()
                .resource::<FangyuanDebugPanelState>()
                .toggles
                .cache
        );

        app.world_mut().write_message(dispatch(
            SAMPLE_SCENE_DOCUMENT_ID,
            OWNER_SAMPLE_SCENE.as_str(),
            ACTION_SAMPLE_SCENE_RETURN_LOBBY,
            SAMPLE_RETURN_NODE,
            BTreeMap::new(),
        ));
        app.update();
        assert_eq!(read_messages::<SceneCommand>(&app).len(), 1);
        assert!(matches!(
            read_messages::<GameRouteCommand>(&app).as_slice(),
            [GameRouteCommand::ChangeMode(AppUiMode::Lobby)]
        ));
    }

    #[test]
    fn fangyuan_cache_and_bake_actions_toggle_only_their_named_modules() {
        let mut app = action_test_app();
        app.world_mut().write_message(dispatch(
            FANGYUAN_HOME_DOCUMENT_ID,
            OWNER_FANGYUAN_HOME.as_str(),
            ACTION_FANGYUAN_HOME_TOGGLE_MODULE,
            "fangyuan_home.module.cache",
            BTreeMap::from([("module".to_owned(), UiActionValue::Enum("cache".to_owned()))]),
        ));
        app.update();

        let after_cache = app.world().resource::<FangyuanDebugPanelState>();
        assert!(!after_cache.toggles.cache);
        assert!(after_cache.toggles.bake);

        app.world_mut().write_message(dispatch(
            FANGYUAN_HOME_DOCUMENT_ID,
            OWNER_FANGYUAN_HOME.as_str(),
            ACTION_FANGYUAN_HOME_TOGGLE_MODULE,
            "fangyuan_home.module.bake",
            BTreeMap::from([("module".to_owned(), UiActionValue::Enum("bake".to_owned()))]),
        ));
        app.update();

        let after_bake = app.world().resource::<FangyuanDebugPanelState>();
        assert!(!after_bake.toggles.cache);
        assert!(!after_bake.toggles.bake);
        assert!(after_bake.toggles.render);
        assert!(after_bake.toggles.lod);
        assert!(after_bake.toggles.audit);
        assert!(after_bake.toggles.trial);
    }

    #[test]
    fn cross_document_action_splicing_has_no_gameplay_side_effects() {
        let mut app = action_test_app();
        app.world_mut()
            .resource_mut::<super::super::robot_sync_scene::RobotSyncHudVisibility>()
            .show_details = true;
        let initial_debug = app.world().resource::<FangyuanDebugPanelState>().clone();

        for forged in [
            dispatch(
                FANGYUAN_HOME_DOCUMENT_ID,
                OWNER_FANGYUAN_HOME.as_str(),
                ACTION_ROBOT_SYNC_HIDE,
                ROBOT_HIDE_NODE,
                BTreeMap::new(),
            ),
            dispatch(
                ROBOT_SYNC_DOCUMENT_ID,
                OWNER_ROBOT_SYNC_SCENE.as_str(),
                ACTION_FANGYUAN_HOME_RELOAD,
                HOME_RELOAD_NODE,
                BTreeMap::new(),
            ),
            dispatch(
                ROBOT_SYNC_DOCUMENT_ID,
                OWNER_ROBOT_SYNC_SCENE.as_str(),
                ACTION_FANGYUAN_HOME_TOGGLE_MODULE,
                "fangyuan_home.module.cache",
                BTreeMap::from([("module".to_owned(), UiActionValue::Enum("cache".to_owned()))]),
            ),
            dispatch(
                SAMPLE_SCENE_DOCUMENT_ID,
                OWNER_SAMPLE_SCENE.as_str(),
                ACTION_TOUCH_RIPPLE_RETURN_LOBBY,
                TOUCH_RETURN_NODE,
                BTreeMap::new(),
            ),
        ] {
            app.world_mut().write_message(forged);
        }
        app.update();

        assert_eq!(
            *app.world().resource::<FangyuanDebugPanelState>(),
            initial_debug
        );
        assert!(
            app.world()
                .resource::<super::super::robot_sync_scene::RobotSyncHudVisibility>()
                .show_details
        );
        assert!(read_messages::<FangyuanHomeBlueprintCommand>(&app).is_empty());
        assert!(read_messages::<SceneCommand>(&app).is_empty());
        assert!(read_messages::<GameRouteCommand>(&app).is_empty());
    }

    #[test]
    fn unchanged_high_frequency_binding_does_not_advance_revision() {
        let contract = GameplayHudHostContract::default();
        let mut values = UiBindingValues::default();
        set_binding(
            &contract,
            &mut values,
            ROBOT_SYNC_DOCUMENT_ID,
            OWNER_ROBOT_SYNC_SCENE.as_str(),
            "robot_sync.status",
            UiBindingValue::String("frame=42".to_owned()),
        );
        let revision = values.revision();
        set_binding(
            &contract,
            &mut values,
            ROBOT_SYNC_DOCUMENT_ID,
            OWNER_ROBOT_SYNC_SCENE.as_str(),
            "robot_sync.status",
            UiBindingValue::String("frame=42".to_owned()),
        );
        assert_eq!(values.revision(), revision);
    }

    #[test]
    fn touch_hud_root_does_not_block_gameplay_picking_and_lifecycle_closes_it() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .init_state::<AppUiMode>()
            .insert_resource(UiTheme::default())
            .insert_resource(UiMetrics::default())
            .insert_resource(UiFontAssets::test_registry())
            .init_resource::<UiFocusState>()
            .init_resource::<UiViewport>()
            .init_resource::<GameplayHudHostContract>()
            .add_plugins((
                UiDocumentRuntimePlugin,
                UiDocumentPreviewPlugin,
                crate::game::declarative_screen::DeclarativeScreenHostPlugin,
            ))
            .add_systems(Startup, register_gameplay_hud_contracts)
            .add_systems(
                OnExit(AppUiMode::WanfaTouchRipple),
                cleanup_gameplay_hud_focus,
            );
        app.world_mut()
            .resource_mut::<NextState<AppUiMode>>()
            .set(AppUiMode::WanfaTouchRipple);
        update_frames(&mut app, 6);

        let document_id = UiDocumentId::from_str(TOUCH_RIPPLE_DOCUMENT_ID).unwrap();
        let runtime = app.world().resource::<UiDocumentRuntime>();
        let instance = runtime
            .active_instance(OWNER_TOUCH_RIPPLE.as_str(), &document_id)
            .unwrap();
        let root = runtime
            .node_entity(instance, &UiNodeId::from_str("touch_ripple.root").unwrap())
            .unwrap();
        let button = runtime
            .node_entity(
                instance,
                &UiNodeId::from_str("touch_ripple.return_lobby").unwrap(),
            )
            .unwrap();
        assert!(app.world().get::<Pickable>(root).is_none());
        assert!(app.world().get::<Button>(button).is_some());
        assert!(!TOUCH_RIPPLE_SOURCE.contains("\"block_lower\": true"));
        app.world_mut()
            .resource_mut::<UiFocusState>()
            .focused_entity = Some(button);

        app.world_mut()
            .resource_mut::<NextState<AppUiMode>>()
            .set(AppUiMode::Lobby);
        update_frames(&mut app, 6);
        assert!(
            app.world()
                .resource::<UiDocumentRuntime>()
                .active_instance(OWNER_TOUCH_RIPPLE.as_str(), &document_id)
                .is_none()
        );
        assert_eq!(app.world().resource::<UiFocusState>().focused_entity, None);
    }

    #[test]
    fn main_world_hud_exposes_only_its_four_buttons_without_blocking_gameplay() {
        let mut app = main_world_hud_runtime_app();
        app.world_mut().resource_mut::<MainWorldEntryState>().phase = MainWorldEntryPhase::Active;
        app.world_mut()
            .resource_mut::<NextState<AppUiMode>>()
            .set(AppUiMode::MainWorld);
        update_frames(&mut app, 6);

        let document_id = UiDocumentId::from_str(MAIN_WORLD_HUD_DOCUMENT_ID).unwrap();
        let runtime = app.world().resource::<UiDocumentRuntime>();
        let instance = runtime
            .active_instance(OWNER_MAIN_WORLD.as_str(), &document_id)
            .unwrap();
        let root = runtime
            .node_entity(instance, &UiNodeId::from_str("main_world.root").unwrap())
            .unwrap();
        assert!(app.world().get::<Pickable>(root).is_none());
        assert!(!MAIN_WORLD_HUD_SOURCE.contains("\"block_lower\": true"));
        for node_id in [
            MAIN_WORLD_SETTINGS_NODE,
            MAIN_WORLD_MAIL_NODE,
            MAIN_WORLD_HOME_NODE,
            MAIN_WORLD_RETURN_LOBBY_NODE,
        ] {
            let button = runtime
                .node_entity(instance, &UiNodeId::from_str(node_id).unwrap())
                .unwrap();
            assert!(app.world().get::<Button>(button).is_some());
        }
    }

    #[test]
    fn main_world_hud_waits_for_the_active_entry_generation() {
        let mut app = main_world_hud_runtime_app();
        app.world_mut()
            .resource_mut::<NextState<AppUiMode>>()
            .set(AppUiMode::MainWorld);
        update_frames(&mut app, 6);

        let document_id = UiDocumentId::from_str(MAIN_WORLD_HUD_DOCUMENT_ID).unwrap();
        assert!(
            app.world()
                .resource::<UiDocumentRuntime>()
                .active_instance(OWNER_MAIN_WORLD.as_str(), &document_id)
                .is_none()
        );

        app.world_mut().resource_mut::<MainWorldEntryState>().phase = MainWorldEntryPhase::Active;
        update_frames(&mut app, 6);
        assert!(
            app.world()
                .resource::<UiDocumentRuntime>()
                .active_instance(OWNER_MAIN_WORLD.as_str(), &document_id)
                .is_some()
        );
    }

    #[test]
    fn main_world_hud_fallback_exits_only_for_the_active_generation() {
        let mut app = App::new();
        app.init_resource::<MainWorldEntryState>()
            .add_message::<DeclarativeScreenHostEvent>()
            .add_message::<MainWorldEntryIntent>()
            .add_systems(Update, recover_from_main_world_hud_failure);
        app.world_mut().resource_mut::<MainWorldEntryState>().phase = MainWorldEntryPhase::Active;
        app.world_mut()
            .write_message(DeclarativeScreenHostEvent::LoadFailed {
                code: "UI_DECLARATIVE_SCREEN_LOAD_FAILED".to_owned(),
                cause: "fallback unavailable".to_owned(),
                route: MAIN_WORLD_HUD_ROUTE.to_owned(),
                document_id: UiDocumentId::from_str(MAIN_WORLD_HUD_DOCUMENT_ID).unwrap(),
                owner: OWNER_MAIN_WORLD.as_str().to_owned(),
                decision: DeclarativeScreenFailureDecision::NoFallbackAvailable,
            });
        app.update();
        assert_eq!(
            read_messages::<MainWorldEntryIntent>(&app),
            [MainWorldEntryIntent::ExitToLobby]
        );
    }

    #[test]
    fn inactive_main_world_hud_failure_is_drained_before_a_later_generation_activates() {
        let mut app = App::new();
        app.init_resource::<MainWorldEntryState>()
            .add_message::<DeclarativeScreenHostEvent>()
            .add_message::<MainWorldEntryIntent>()
            .add_systems(Update, recover_from_main_world_hud_failure);
        app.world_mut()
            .write_message(DeclarativeScreenHostEvent::LoadFailed {
                code: "UI_DECLARATIVE_SCREEN_LOAD_FAILED".to_owned(),
                cause: "stale fallback unavailable".to_owned(),
                route: MAIN_WORLD_HUD_ROUTE.to_owned(),
                document_id: UiDocumentId::from_str(MAIN_WORLD_HUD_DOCUMENT_ID).unwrap(),
                owner: OWNER_MAIN_WORLD.as_str().to_owned(),
                decision: DeclarativeScreenFailureDecision::NoFallbackAvailable,
            });
        app.update();
        assert!(read_messages::<MainWorldEntryIntent>(&app).is_empty());

        app.world_mut().resource_mut::<MainWorldEntryState>().phase = MainWorldEntryPhase::Active;
        app.update();
        assert!(read_messages::<MainWorldEntryIntent>(&app).is_empty());
    }

    #[test]
    fn fangyuan_home_status_and_debug_panels_are_runtime_scroll_views() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .init_state::<AppUiMode>()
            .insert_resource(UiTheme::default())
            .insert_resource(UiMetrics::default())
            .insert_resource(UiFontAssets::test_registry())
            .init_resource::<UiFocusState>()
            .init_resource::<UiViewport>()
            .init_resource::<GameplayHudHostContract>()
            .add_plugins((
                UiDocumentRuntimePlugin,
                UiDocumentPreviewPlugin,
                crate::game::declarative_screen::DeclarativeScreenHostPlugin,
            ))
            .add_systems(Startup, register_gameplay_hud_contracts);
        app.world_mut()
            .resource_mut::<NextState<AppUiMode>>()
            .set(AppUiMode::FangyuanHome);
        update_frames(&mut app, 6);

        let document_id = UiDocumentId::from_str(FANGYUAN_HOME_DOCUMENT_ID).unwrap();
        let runtime = app.world().resource::<UiDocumentRuntime>();
        let instance = runtime
            .active_instance(OWNER_FANGYUAN_HOME.as_str(), &document_id)
            .unwrap();
        let root = runtime
            .node_entity(instance, &UiNodeId::from_str("fangyuan_home.root").unwrap())
            .unwrap();
        assert!(app.world().get::<Pickable>(root).is_none());

        for node_id in ["fangyuan_home.status_panel", "fangyuan_home.debug_panel"] {
            let entity = runtime
                .node_entity(instance, &UiNodeId::from_str(node_id).unwrap())
                .unwrap();
            assert!(app.world().get::<ScrollPosition>(entity).is_some());
            assert_eq!(
                app.world().get::<Node>(entity).unwrap().overflow.y,
                OverflowAxis::Scroll
            );
        }
    }

    fn action_test_app() -> App {
        let mut app = App::new();
        app.init_resource::<FangyuanDebugPanelState>()
            .init_resource::<super::super::robot_sync_scene::RobotSyncHudVisibility>()
            .add_message::<UiActionDispatch>()
            .add_message::<FangyuanHomeBlueprintCommand>()
            .add_message::<SceneCommand>()
            .add_message::<GameRouteCommand>()
            .add_systems(Update, handle_gameplay_hud_document_actions);
        app
    }

    fn main_world_hud_runtime_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .init_state::<AppUiMode>()
            .init_resource::<MainWorldEntryState>()
            .insert_resource(UiTheme::default())
            .insert_resource(UiMetrics::default())
            .insert_resource(UiFontAssets::test_registry())
            .init_resource::<UiFocusState>()
            .init_resource::<UiViewport>()
            .init_resource::<GameplayHudHostContract>()
            .add_plugins((
                UiDocumentRuntimePlugin,
                UiDocumentPreviewPlugin,
                crate::game::declarative_screen::DeclarativeScreenHostPlugin,
            ))
            .add_systems(Startup, register_gameplay_hud_contracts);
        app
    }

    fn dispatch(
        document_id: &str,
        owner: &str,
        action: &str,
        source: &str,
        params: BTreeMap<String, UiActionValue>,
    ) -> UiActionDispatch {
        UiActionDispatch {
            action: UiActionId::from_str(action).unwrap(),
            document_id: UiDocumentId::from_str(document_id).unwrap(),
            owner: owner.to_owned(),
            source_node: UiNodeId::from_str(source).unwrap(),
            kind: business_command(action),
            params,
        }
    }

    fn read_messages<M>(app: &App) -> Vec<M>
    where
        M: Message + Clone,
    {
        let messages = app.world().resource::<Messages<M>>();
        let mut cursor = MessageCursor::default();
        cursor.read(messages).cloned().collect()
    }

    fn update_frames(app: &mut App, frames: usize) {
        for _ in 0..frames {
            app.update();
        }
    }
}
