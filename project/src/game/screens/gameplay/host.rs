use std::{
    collections::BTreeMap,
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use bevy::prelude::*;

#[cfg(all(debug_assertions, not(target_os = "android")))]
use crate::framework::ui::audit::UiAuditConfig;
#[cfg(all(debug_assertions, not(target_os = "android")))]
use crate::framework::ui::document::UiDocumentRuntime;

use crate::framework::{
    audio::AudioMixer,
    fangyuan::{FangyuanDebugPanelModule, FangyuanDebugPanelState},
    scene::prelude::{SceneCommand, SceneExitRequest},
    ui::{
        core::{UiOwnerId, UiPanelCommand, binding::UiBindingValues, focus::UiFocusState},
        document::{
            UiActionDescriptor, UiActionDispatch, UiActionId, UiActionParamSchema,
            UiActionParamType, UiActionRegistry, UiActionValue, UiBindingDeclaration,
            UiBindingMissingBehavior, UiBindingPath, UiBindingScope, UiBindingType, UiBindingValue,
            UiBindingVisibility, UiDocumentId, UiDocumentLayer, UiDocumentPanel,
            UiDocumentRuntimeCommand, UiHostBindingKey, UiNodeId, UiPageState,
            UiRegisteredActionKind,
        },
        i18n::UiI18n,
    },
};
#[cfg(all(debug_assertions, not(target_os = "android")))]
use crate::game::ui_ids::SCROLL_FANGYUAN_HOME_MAIN;
use crate::game::{
    declarative_screen::{
        DeclarativeScreenFailureDecision, DeclarativeScreenFailurePolicy, DeclarativeScreenHost,
        DeclarativeScreenHostCommand, DeclarativeScreenHostEvent, DeclarativeScreenRegistry,
        DeclarativeScreenSource,
    },
    myserver::{
        GameConnectionState, MyServerSession,
        mail::{
            MAIL_MAX_DETAIL_ATTACHMENTS, MailAttachment, MailAvailability, MailClaimWorkflow,
            MailClientCommand, MailClientError, MailClientState, MailDetailLoadState,
            MailListLoadState, MailListQuery, MailMarkReadState, MailSummary,
        },
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
pub(in crate::game) const MAIN_WORLD_MAIL_DOCUMENT_ID: &str = "game.main_world_mail";

pub(in crate::game) const MAIN_WORLD_HUD_ROUTE: &str = "main_world";
pub(in crate::game) const MAIN_WORLD_HUD_ROUTE_ALIASES: &[&str] = &["main_world", "main-world"];
pub(in crate::game) const MAIN_WORLD_HUD_SOURCE_PATH: &str = "gameplay/main_world_hud.v1.json";
pub(super) const ACTION_MAIN_WORLD_OPEN_SETTINGS: &str = "main_world.open_settings";
pub(super) const ACTION_MAIN_WORLD_OPEN_MAIL: &str = "main_world.open_mail";
pub(super) const ACTION_MAIN_WORLD_ENTER_HOME: &str = "main_world.enter_home";
pub(super) const ACTION_MAIN_WORLD_RETURN_LOBBY: &str = "main_world.return_lobby";
pub(super) const ACTION_MAIN_WORLD_MAIL_REFRESH: &str = "main_world_mail.refresh";
pub(super) const ACTION_MAIN_WORLD_MAIL_SELECT: &str = "main_world_mail.select";
pub(super) const ACTION_MAIN_WORLD_MAIL_LOAD_MORE: &str = "main_world_mail.load_more";
pub(super) const ACTION_MAIN_WORLD_MAIL_MARK_READ: &str = "main_world_mail.mark_read";
pub(super) const ACTION_MAIN_WORLD_MAIL_BACK_TO_LIST: &str = "main_world_mail.back_to_list";
pub(super) const ACTION_MAIN_WORLD_MAIL_CLAIM: &str = "main_world_mail.claim";
pub(super) const ACTION_MAIN_WORLD_MAIL_RETRY: &str = "main_world_mail.retry";
pub(super) const ACTION_MAIN_WORLD_MAIL_CLOSE: &str = "main_world_mail.close";
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

/// The authority coordinator requests this before changing the main-world scene
/// session. Route closure keeps detached host state in sync with runtime cleanup.
pub(in crate::game) fn request_main_world_ui_teardown(
    cause: MainWorldUiTeardownCause,
    bindings: &mut UiBindingValues,
    panel_commands: &mut MessageWriter<UiPanelCommand>,
    runtime_commands: &mut MessageWriter<UiDocumentRuntimeCommand>,
    screen_commands: &mut MessageWriter<DeclarativeScreenHostCommand>,
) {
    for owner in cause.cleanup_owners() {
        bindings.clear_owner(owner.as_str());
        panel_commands.write(UiPanelCommand::CloseAllForOwner(*owner));
        runtime_commands.write(UiDocumentRuntimeCommand::CloseAllForOwner {
            owner: owner.as_str().to_owned(),
        });
    }
    for route in [
        MainWorldDocumentPanel::Mail.route(),
        MainWorldDocumentPanel::Settings.route(),
        MAIN_WORLD_HUD_ROUTE,
    ] {
        screen_commands.write(DeclarativeScreenHostCommand::CloseRoute {
            route: route.to_owned(),
        });
    }
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
const MAIN_WORLD_MAIL_SOURCE: &str =
    include_str!("../../../../assets/ui/documents/approved/gameplay/main_world_mail.v1.json");
const MAIN_WORLD_MAIL_FALLBACK_SOURCE: &str = include_str!(
    "../../../../assets/ui/documents/approved/gameplay/main_world_mail_fallback.v1.json"
);

const MAIN_WORLD_SETTINGS_NODE: &str = "main_world.settings";
const MAIN_WORLD_MAIL_NODE: &str = "main_world.mail";
const MAIN_WORLD_HOME_NODE: &str = "main_world.home";
const MAIN_WORLD_RETURN_LOBBY_NODE: &str = "main_world.return_lobby";
const MAIN_WORLD_MAIL_REFRESH_NODE: &str = "main_world_mail.refresh";
const MAIN_WORLD_MAIL_SELECT_NODE: &str = "main_world_mail.item.open";
const MAIN_WORLD_MAIL_LOAD_MORE_NODE: &str = "main_world_mail.load_more";
const MAIN_WORLD_MAIL_MARK_READ_NODE: &str = "main_world_mail.mark_read";
const MAIN_WORLD_MAIL_BACK_TO_LIST_NODE: &str = "main_world_mail.back_to_list";
const MAIN_WORLD_MAIL_CLAIM_NODE: &str = "main_world_mail.claim";
const MAIN_WORLD_MAIL_RETRY_NODE: &str = "main_world_mail.retry";
const MAIN_WORLD_MAIL_CLOSE_NODE: &str = "main_world_mail.close";
const MAIN_WORLD_MAIL_ID_MAX_BYTES: usize = 64;
const MAIN_WORLD_MAIL_MAX_ITEMS: u16 = 50;
const MAIN_WORLD_MAIL_MAX_ATTACHMENTS: u16 = 32;
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
                (
                    MAIN_WORLD_HUD_DOCUMENT_ID,
                    binding_schema([
                        ("main_world.connection.status", UiBindingType::String),
                        ("main_world.character.summary", UiBindingType::String),
                        ("main_world.mail.disabled", UiBindingType::Bool),
                        ("main_world.mail.unread", UiBindingType::String),
                        (
                            "main_world.mail.unread_visibility",
                            UiBindingType::Visibility,
                        ),
                        ("main_world.settings.disabled", UiBindingType::Bool),
                        ("main_world.home.disabled", UiBindingType::Bool),
                        ("main_world.return_lobby.disabled", UiBindingType::Bool),
                        ("main_world.transition.loading", UiBindingType::Bool),
                        ("main_world.transition.status", UiBindingType::String),
                    ]),
                ),
                (
                    MAIN_WORLD_MAIL_DOCUMENT_ID,
                    main_world_mail_binding_schema(),
                ),
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
                        (
                            "fangyuan_home.debug.visibility",
                            UiBindingType::Enum {
                                values: ["flex", "none"].map(str::to_owned).to_vec(),
                            },
                        ),
                    ]),
                ),
            ]),
        }
    }
}

fn main_world_mail_binding_schema() -> BTreeMap<UiBindingPath, UiBindingDeclaration> {
    let mail_item = UiBindingType::Record {
        fields: BTreeMap::from([
            ("mail_id".to_owned(), UiBindingType::String),
            ("title".to_owned(), UiBindingType::String),
            ("sender".to_owned(), UiBindingType::String),
            ("sent_at".to_owned(), UiBindingType::String),
            ("status".to_owned(), UiBindingType::String),
            ("attachment_label".to_owned(), UiBindingType::String),
            ("unread".to_owned(), UiBindingType::Bool),
            ("expired".to_owned(), UiBindingType::Bool),
            ("selected".to_owned(), UiBindingType::Bool),
            ("disabled".to_owned(), UiBindingType::Bool),
        ]),
    };
    let attachment_item = UiBindingType::Record {
        fields: BTreeMap::from([
            ("attachment_id".to_owned(), UiBindingType::String),
            ("label".to_owned(), UiBindingType::String),
            ("count".to_owned(), UiBindingType::String),
            ("bind_status".to_owned(), UiBindingType::String),
        ]),
    };
    let specs = vec![
        (
            "main_world_mail.availability",
            UiBindingScope::Owner,
            UiBindingType::Enum {
                values: ["unavailable", "awaiting_session", "ready"]
                    .map(str::to_owned)
                    .to_vec(),
            },
        ),
        (
            "main_world_mail.collection_state",
            UiBindingScope::Owner,
            UiBindingType::Enum {
                values: ["loading", "ready", "error"].map(str::to_owned).to_vec(),
            },
        ),
        (
            "main_world_mail.view_mode",
            UiBindingScope::Owner,
            UiBindingType::Enum {
                values: ["list", "detail"].map(str::to_owned).to_vec(),
            },
        ),
        (
            "main_world_mail.items",
            UiBindingScope::Owner,
            UiBindingType::List {
                item: Box::new(mail_item),
                max_items: MAIN_WORLD_MAIL_MAX_ITEMS,
            },
        ),
        (
            "main_world_mail.status",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "main_world_mail.count",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "main_world_mail.has_more",
            UiBindingScope::Owner,
            UiBindingType::Bool,
        ),
        (
            "main_world_mail.load_more_visibility",
            UiBindingScope::Owner,
            UiBindingType::Visibility,
        ),
        (
            "main_world_mail.list_visibility",
            UiBindingScope::Owner,
            UiBindingType::Visibility,
        ),
        (
            "main_world_mail.load_more_disabled",
            UiBindingScope::Owner,
            UiBindingType::Bool,
        ),
        (
            "main_world_mail.load_more_loading",
            UiBindingScope::Owner,
            UiBindingType::Bool,
        ),
        (
            "main_world_mail.selected.mail_id",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "main_world_mail.detail_visibility",
            UiBindingScope::Owner,
            UiBindingType::Visibility,
        ),
        (
            "main_world_mail.detail.title",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "main_world_mail.detail.sender",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "main_world_mail.detail.sent_at",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "main_world_mail.detail.status",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "main_world_mail.detail.content",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "main_world_mail.detail.attachments",
            UiBindingScope::Owner,
            UiBindingType::List {
                item: Box::new(attachment_item),
                max_items: MAIN_WORLD_MAIL_MAX_ATTACHMENTS,
            },
        ),
        (
            "main_world_mail.claim.state",
            UiBindingScope::Owner,
            UiBindingType::Enum {
                values: [
                    "idle",
                    "available",
                    "submitting",
                    "processing",
                    "claimed",
                    "already_claimed",
                    "expired",
                    "retryable_failure",
                    "blocked_capacity",
                    "permanent_failure",
                    "reconciliation_pending",
                    "manual_review",
                    "unavailable",
                ]
                .map(str::to_owned)
                .to_vec(),
            },
        ),
        (
            "main_world_mail.claim.label",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "main_world_mail.mark_read_disabled",
            UiBindingScope::Owner,
            UiBindingType::Bool,
        ),
        (
            "main_world_mail.mark_read_loading",
            UiBindingScope::Owner,
            UiBindingType::Bool,
        ),
        (
            "main_world_mail.claim_disabled",
            UiBindingScope::Owner,
            UiBindingType::Bool,
        ),
        (
            "main_world_mail.claim_loading",
            UiBindingScope::Owner,
            UiBindingType::Bool,
        ),
        (
            "main_world_mail.error.code",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "main_world_mail.error.message",
            UiBindingScope::Owner,
            UiBindingType::String,
        ),
        (
            "main_world_mail.error_visibility",
            UiBindingScope::Owner,
            UiBindingType::Visibility,
        ),
        (
            "main_world_mail.retry_disabled",
            UiBindingScope::Owner,
            UiBindingType::Bool,
        ),
        (
            "main_world_mail.retry_loading",
            UiBindingScope::Owner,
            UiBindingType::Bool,
        ),
        (
            "main_world_mail.refresh_disabled",
            UiBindingScope::Owner,
            UiBindingType::Bool,
        ),
        (
            "main_world_mail.refresh_loading",
            UiBindingScope::Owner,
            UiBindingType::Bool,
        ),
        (
            "main_world_mail.item.mail_id",
            UiBindingScope::Item,
            UiBindingType::String,
        ),
        (
            "main_world_mail.item.title",
            UiBindingScope::Item,
            UiBindingType::String,
        ),
        (
            "main_world_mail.item.sender",
            UiBindingScope::Item,
            UiBindingType::String,
        ),
        (
            "main_world_mail.item.sent_at",
            UiBindingScope::Item,
            UiBindingType::String,
        ),
        (
            "main_world_mail.item.status",
            UiBindingScope::Item,
            UiBindingType::String,
        ),
        (
            "main_world_mail.item.attachment_label",
            UiBindingScope::Item,
            UiBindingType::String,
        ),
        (
            "main_world_mail.item.unread",
            UiBindingScope::Item,
            UiBindingType::Bool,
        ),
        (
            "main_world_mail.item.expired",
            UiBindingScope::Item,
            UiBindingType::Bool,
        ),
        (
            "main_world_mail.item.selected",
            UiBindingScope::Item,
            UiBindingType::Bool,
        ),
        (
            "main_world_mail.item.disabled",
            UiBindingScope::Item,
            UiBindingType::Bool,
        ),
        (
            "main_world_mail.attachment.item.attachment_id",
            UiBindingScope::Item,
            UiBindingType::String,
        ),
        (
            "main_world_mail.attachment.item.label",
            UiBindingScope::Item,
            UiBindingType::String,
        ),
        (
            "main_world_mail.attachment.item.count",
            UiBindingScope::Item,
            UiBindingType::String,
        ),
        (
            "main_world_mail.attachment.item.bind_status",
            UiBindingScope::Item,
            UiBindingType::String,
        ),
    ];
    specs
        .into_iter()
        .map(|(path, scope, value_type)| {
            (
                UiBindingPath::from_str(path).expect("main world mail binding path is static"),
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

#[derive(Default, Resource)]
pub(super) struct MainWorldHudBindingGeneration {
    generation: Option<u64>,
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
        main_world_mail_declarative_screen_host(contract),
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

fn main_world_mail_declarative_screen_host(
    contract: &GameplayHudHostContract,
) -> DeclarativeScreenHost {
    let source = DeclarativeScreenSource::approved(
        "gameplay/main_world_mail.v1.json",
        MAIN_WORLD_MAIL_SOURCE,
    );
    DeclarativeScreenHost {
        document_id: UiDocumentId::from_str(MAIN_WORLD_MAIL_DOCUMENT_ID)
            .expect("main world mail document ID is static"),
        route: MainWorldDocumentPanel::Mail.route(),
        route_aliases: MainWorldDocumentPanel::Mail.aliases(),
        mode: None,
        owner: MainWorldDocumentPanel::Mail.owner(),
        panel: MainWorldDocumentPanel::Mail.panel(),
        layer: MainWorldDocumentPanel::Mail.layer(),
        initial_state: UiPageState::initial(),
        binding_schema: contract.bindings[MAIN_WORLD_MAIL_DOCUMENT_ID]
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
            ACTION_MAIN_WORLD_MAIL_REFRESH,
            ACTION_MAIN_WORLD_MAIL_SELECT,
            ACTION_MAIN_WORLD_MAIL_LOAD_MORE,
            ACTION_MAIN_WORLD_MAIL_MARK_READ,
            ACTION_MAIN_WORLD_MAIL_BACK_TO_LIST,
            ACTION_MAIN_WORLD_MAIL_CLAIM,
            ACTION_MAIN_WORLD_MAIL_RETRY,
            ACTION_MAIN_WORLD_MAIL_CLOSE,
        ]
        .into_iter()
        .map(|action| UiActionId::from_str(action).expect("main world mail action ID is static"))
        .collect(),
        audit_profiles: DEFAULT_AUDIT_PROFILES.map(str::to_owned).to_vec(),
        source: source.clone(),
        fallback_source: Some(DeclarativeScreenSource::approved(
            "gameplay/main_world_mail_fallback.v1.json",
            MAIN_WORLD_MAIL_FALLBACK_SOURCE,
        )),
        failure_policy: DeclarativeScreenFailurePolicy::PackagedFallback,
    }
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
            MAIN_WORLD_MAIL_DOCUMENT_ID,
            OWNER_MAIN_WORLD_MAIL_PANEL.as_str(),
            ACTION_MAIN_WORLD_MAIL_REFRESH,
            MAIN_WORLD_MAIL_REFRESH_NODE,
        ),
        mail_id_action(ACTION_MAIN_WORLD_MAIL_SELECT, MAIN_WORLD_MAIL_SELECT_NODE),
        action(
            MAIN_WORLD_MAIL_DOCUMENT_ID,
            OWNER_MAIN_WORLD_MAIL_PANEL.as_str(),
            ACTION_MAIN_WORLD_MAIL_LOAD_MORE,
            MAIN_WORLD_MAIL_LOAD_MORE_NODE,
        ),
        mail_id_action(
            ACTION_MAIN_WORLD_MAIL_MARK_READ,
            MAIN_WORLD_MAIL_MARK_READ_NODE,
        ),
        action(
            MAIN_WORLD_MAIL_DOCUMENT_ID,
            OWNER_MAIN_WORLD_MAIL_PANEL.as_str(),
            ACTION_MAIN_WORLD_MAIL_BACK_TO_LIST,
            MAIN_WORLD_MAIL_BACK_TO_LIST_NODE,
        ),
        mail_id_action(ACTION_MAIN_WORLD_MAIL_CLAIM, MAIN_WORLD_MAIL_CLAIM_NODE),
        action(
            MAIN_WORLD_MAIL_DOCUMENT_ID,
            OWNER_MAIN_WORLD_MAIL_PANEL.as_str(),
            ACTION_MAIN_WORLD_MAIL_RETRY,
            MAIN_WORLD_MAIL_RETRY_NODE,
        ),
        action(
            MAIN_WORLD_MAIL_DOCUMENT_ID,
            OWNER_MAIN_WORLD_MAIL_PANEL.as_str(),
            ACTION_MAIN_WORLD_MAIL_CLOSE,
            MAIN_WORLD_MAIL_CLOSE_NODE,
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

fn mail_id_action(action_id: &str, source: &str) -> UiActionDescriptor {
    action(
        MAIN_WORLD_MAIL_DOCUMENT_ID,
        OWNER_MAIN_WORLD_MAIL_PANEL.as_str(),
        action_id,
        source,
    )
    .with_param(
        "mail_id",
        UiActionParamSchema::required(UiActionParamType::OpaqueId {
            max_bytes: MAIN_WORLD_MAIL_ID_MAX_BYTES,
        }),
    )
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

/// The full mailbox has a separate packaged fallback containing the same closed
/// close action. If even that source cannot mount, close only the detached route;
/// the global mail resource and any claim reconciliation continue to run.
pub(super) fn recover_from_main_world_mail_failure(
    mut host_events: MessageReader<DeclarativeScreenHostEvent>,
    mut screen_commands: MessageWriter<DeclarativeScreenHostCommand>,
    mut focus: Option<ResMut<UiFocusState>>,
) {
    let fallback_failed = host_events.read().any(|event| {
        matches!(
            event,
            DeclarativeScreenHostEvent::LoadFailed {
                document_id,
                owner,
                decision: DeclarativeScreenFailureDecision::NoFallbackAvailable,
                ..
            } if document_id.as_str() == MAIN_WORLD_MAIL_DOCUMENT_ID
                && owner == OWNER_MAIN_WORLD_MAIL_PANEL.as_str()
        )
    });
    if !fallback_failed {
        return;
    }
    if let Some(focus) = focus.as_deref_mut() {
        focus.focused_entity = None;
    }
    screen_commands.write(DeclarativeScreenHostCommand::CloseRoute {
        route: MainWorldDocumentPanel::Mail.route().to_owned(),
    });
}

pub(super) fn handle_gameplay_hud_document_actions(
    mut debug_panel_state: ResMut<FangyuanDebugPanelState>,
    mut robot_visibility: ResMut<super::robot_sync_scene::RobotSyncHudVisibility>,
    mut blueprint_commands: MessageWriter<FangyuanHomeBlueprintCommand>,
    mut scene_commands: MessageWriter<SceneCommand>,
    mut route_commands: MessageWriter<GameRouteCommand>,
    mut screen_commands: MessageWriter<DeclarativeScreenHostCommand>,
    entry: Option<Res<MainWorldEntryState>>,
    mail: Option<Res<MailClientState>>,
    mut mail_commands: MessageWriter<MailClientCommand>,
    mut focus: Option<ResMut<UiFocusState>>,
    mut main_world_intents: MessageWriter<MainWorldEntryIntent>,
    mut actions: MessageReader<UiActionDispatch>,
) {
    let mut main_world_transition_requested = false;
    for dispatch in actions.read() {
        if let Some(module) = debug_module_action(dispatch) {
            debug_panel_state.toggle_module(module);
            continue;
        }
        if dispatch.action.as_str() == ACTION_MAIN_WORLD_MAIL_SELECT {
            if main_world_actions_are_active(entry.as_deref())
                && let Some(mail) = mail.as_deref()
                && let Some(mail_id) = main_world_mail_select_id(dispatch)
                && mail.contains_authoritative_mail(mail_id)
            {
                mail_commands.write(MailClientCommand::LoadMail {
                    mail_id: mail_id.to_owned(),
                });
            }
            continue;
        }
        if dispatch.action.as_str() == ACTION_MAIN_WORLD_MAIL_MARK_READ {
            if main_world_actions_are_active(entry.as_deref())
                && let Some(mail) = mail.as_deref()
                && let Some(mail_id) = main_world_mail_id_action(
                    dispatch,
                    ACTION_MAIN_WORLD_MAIL_MARK_READ,
                    MAIN_WORLD_MAIL_MARK_READ_NODE,
                )
                && mail.selected_mail_id() == Some(mail_id)
                && mail.contains_authoritative_mail(mail_id)
            {
                mail_commands.write(MailClientCommand::MarkRead {
                    mail_id: mail_id.to_owned(),
                });
            }
            continue;
        }
        if dispatch.action.as_str() == ACTION_MAIN_WORLD_MAIL_CLAIM {
            if main_world_actions_are_active(entry.as_deref())
                && let Some(mail) = mail.as_deref()
                && let Some(mail_id) = main_world_mail_id_action(
                    dispatch,
                    ACTION_MAIN_WORLD_MAIL_CLAIM,
                    MAIN_WORLD_MAIL_CLAIM_NODE,
                )
                && mail.can_submit_claim(mail_id)
                && !mail.selected_mail.as_ref().is_some_and(|detail| {
                    detail
                        .summary
                        .expires_at
                        .as_deref()
                        .and_then(rfc3339_unix_seconds)
                        .is_some_and(|expires_at| {
                            SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .is_ok_and(|now| expires_at <= now.as_secs() as i64)
                        })
                })
            {
                mail_commands.write(MailClientCommand::Claim {
                    mail_id: mail_id.to_owned(),
                });
            }
            continue;
        }
        if !zero_param_action_matches_contract(dispatch) {
            continue;
        }
        match dispatch.action.as_str() {
            ACTION_MAIN_WORLD_OPEN_SETTINGS => {
                screen_commands.write(DeclarativeScreenHostCommand::OpenDetachedRoute {
                    route: crate::game::screens::settings::MAIN_WORLD_SETTINGS_ROUTE.to_owned(),
                });
            }
            ACTION_MAIN_WORLD_OPEN_MAIL
                if main_world_actions_are_active(entry.as_deref())
                    && mail.as_deref().is_some_and(MailClientState::is_available) =>
            {
                screen_commands.write(DeclarativeScreenHostCommand::OpenDetachedRoute {
                    route: MainWorldDocumentPanel::Mail.route().to_owned(),
                });
                mail_commands.write(MailClientCommand::LoadList {
                    query: MailListQuery::default(),
                });
            }
            ACTION_MAIN_WORLD_MAIL_REFRESH | ACTION_MAIN_WORLD_MAIL_RETRY
                if main_world_actions_are_active(entry.as_deref())
                    && mail.as_deref().is_some_and(MailClientState::is_available) =>
            {
                if dispatch.action.as_str() == ACTION_MAIN_WORLD_MAIL_RETRY
                    && let Some(mail_id) = mail
                        .as_deref()
                        .and_then(MailClientState::selected_mail_id)
                        .map(str::to_owned)
                {
                    mail_commands.write(MailClientCommand::LoadMail { mail_id });
                } else {
                    let query = mail
                        .as_deref()
                        .map_or_else(MailListQuery::default, MailClientState::refresh_query);
                    mail_commands.write(MailClientCommand::LoadList { query });
                }
            }
            ACTION_MAIN_WORLD_MAIL_LOAD_MORE if main_world_actions_are_active(entry.as_deref()) => {
                if let Some(query) = mail.as_deref().and_then(MailClientState::next_page_query) {
                    mail_commands.write(MailClientCommand::LoadList { query });
                }
            }
            ACTION_MAIN_WORLD_MAIL_CLOSE if main_world_actions_are_active(entry.as_deref()) => {
                if mail.as_deref().is_some_and(MailClientState::detail_is_open) {
                    mail_commands.write(MailClientCommand::DismissDetail);
                }
                if let Some(focus) = focus.as_deref_mut() {
                    focus.focused_entity = None;
                }
                screen_commands.write(DeclarativeScreenHostCommand::CloseRoute {
                    route: MainWorldDocumentPanel::Mail.route().to_owned(),
                });
            }
            ACTION_MAIN_WORLD_MAIL_BACK_TO_LIST
                if main_world_actions_are_active(entry.as_deref())
                    && mail.as_deref().is_some_and(MailClientState::detail_is_open) =>
            {
                mail_commands.write(MailClientCommand::DismissDetail);
            }
            ACTION_MAIN_WORLD_ENTER_HOME
                if !main_world_transition_requested
                    && main_world_actions_are_active(entry.as_deref()) =>
            {
                main_world_transition_requested = true;
                main_world_intents.write(MainWorldEntryIntent::EnterHome);
            }
            ACTION_MAIN_WORLD_RETURN_LOBBY
                if !main_world_transition_requested
                    && main_world_actions_are_active(entry.as_deref()) =>
            {
                main_world_transition_requested = true;
                main_world_intents.write(MainWorldEntryIntent::ExitToLobby);
            }
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

fn main_world_actions_are_active(entry: Option<&MainWorldEntryState>) -> bool {
    entry.is_some_and(|entry| entry.phase == MainWorldEntryPhase::Active)
}

fn main_world_mail_select_id(action: &UiActionDispatch) -> Option<&str> {
    main_world_mail_id_action(
        action,
        ACTION_MAIN_WORLD_MAIL_SELECT,
        MAIN_WORLD_MAIL_SELECT_NODE,
    )
}

fn main_world_mail_id_action<'a>(
    action: &'a UiActionDispatch,
    expected_action: &str,
    expected_source: &str,
) -> Option<&'a str> {
    (business_target_matches_action(action)
        && action.document_id.as_str() == MAIN_WORLD_MAIL_DOCUMENT_ID
        && action.owner == OWNER_MAIN_WORLD_MAIL_PANEL.as_str()
        && action.action.as_str() == expected_action
        && action.source_node.as_str() == expected_source
        && action.params.len() == 1)
        .then(|| action.params.get("mail_id"))
        .flatten()
        .and_then(|value| match value {
            UiActionValue::String(mail_id) => Some(mail_id.as_str()),
            _ => None,
        })
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
            MAIN_WORLD_MAIL_DOCUMENT_ID,
            "main_world_mail_panel",
            ACTION_MAIN_WORLD_MAIL_REFRESH,
            MAIN_WORLD_MAIL_REFRESH_NODE,
        ) | (
            MAIN_WORLD_MAIL_DOCUMENT_ID,
            "main_world_mail_panel",
            ACTION_MAIN_WORLD_MAIL_LOAD_MORE,
            MAIN_WORLD_MAIL_LOAD_MORE_NODE,
        ) | (
            MAIN_WORLD_MAIL_DOCUMENT_ID,
            "main_world_mail_panel",
            ACTION_MAIN_WORLD_MAIL_RETRY,
            MAIN_WORLD_MAIL_RETRY_NODE,
        ) | (
            MAIN_WORLD_MAIL_DOCUMENT_ID,
            "main_world_mail_panel",
            ACTION_MAIN_WORLD_MAIL_BACK_TO_LIST,
            MAIN_WORLD_MAIL_BACK_TO_LIST_NODE,
        ) | (
            MAIN_WORLD_MAIL_DOCUMENT_ID,
            "main_world_mail_panel",
            ACTION_MAIN_WORLD_MAIL_CLOSE,
            MAIN_WORLD_MAIL_CLOSE_NODE,
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

pub(super) fn sync_main_world_hud_bindings(
    entry: Res<MainWorldEntryState>,
    session: Option<Res<MyServerSession>>,
    mail: Option<Res<MailClientState>>,
    mixer: Option<Res<AudioMixer>>,
    i18n: Res<UiI18n>,
    contract: Res<GameplayHudHostContract>,
    mut synchronized_generation: ResMut<MainWorldHudBindingGeneration>,
    mut values: ResMut<UiBindingValues>,
) {
    if synchronized_generation.generation != Some(entry.generation) {
        values.clear_owner(OWNER_MAIN_WORLD.as_str());
        synchronized_generation.generation = Some(entry.generation);
    }

    let transition_loading = entry.is_in_flight();
    let mail_available = mail.as_deref().is_some_and(MailClientState::is_available);
    let unread_count = mail
        .as_deref()
        .and_then(MailClientState::authoritative_unread_count);
    let mail_disabled = transition_loading || !mail_available;
    let connection = session
        .as_deref()
        .map_or(GameConnectionState::NotConnected, |session| {
            session.game_connection_state
        });

    for (path, value) in [
        (
            "main_world.connection.status",
            UiBindingValue::String(localized_main_world_connection_status(connection, &i18n)),
        ),
        (
            "main_world.character.summary",
            UiBindingValue::String(localized_main_world_character_summary(
                entry.character_id.as_deref(),
                &i18n,
            )),
        ),
        (
            "main_world.mail.disabled",
            UiBindingValue::Bool(mail_disabled),
        ),
        (
            "main_world.mail.unread",
            UiBindingValue::String(
                unread_count.map_or_else(String::new, |count| count.to_string()),
            ),
        ),
        (
            "main_world.mail.unread_visibility",
            UiBindingValue::Visibility(if unread_count.is_some_and(|count| count > 0) {
                UiBindingVisibility::Visible
            } else {
                UiBindingVisibility::Hidden
            }),
        ),
        (
            "main_world.settings.disabled",
            UiBindingValue::Bool(transition_loading || mixer.is_none()),
        ),
        (
            "main_world.home.disabled",
            UiBindingValue::Bool(transition_loading),
        ),
        (
            "main_world.return_lobby.disabled",
            UiBindingValue::Bool(transition_loading),
        ),
        (
            "main_world.transition.loading",
            UiBindingValue::Bool(transition_loading),
        ),
        (
            "main_world.transition.status",
            UiBindingValue::String(localized_main_world_transition_status(entry.phase, &i18n)),
        ),
    ] {
        set_binding(
            &contract,
            &mut values,
            MAIN_WORLD_HUD_DOCUMENT_ID,
            OWNER_MAIN_WORLD.as_str(),
            path,
            value,
        );
    }
}

pub(super) fn sync_main_world_mail_bindings(
    mail: Option<Res<MailClientState>>,
    contract: Res<GameplayHudHostContract>,
    mut values: ResMut<UiBindingValues>,
) {
    let mail = mail.as_deref();
    let availability = match mail.map(|mail| &mail.availability) {
        Some(MailAvailability::Ready) => "ready",
        Some(MailAvailability::AwaitingCharacterTicket) => "awaiting_session",
        Some(MailAvailability::Unavailable { .. }) | None => "unavailable",
    };
    let list_load_state = mail.map_or(MailListLoadState::Failed, |mail| mail.list_load_state);
    let is_available = mail.is_some_and(MailClientState::is_available);
    let read_is_rate_limited = mail.is_some_and(MailClientState::is_read_rate_limited);
    let claim_is_rate_limited = mail.is_some_and(MailClientState::is_claim_rate_limited);
    let detail_open = mail.is_some_and(MailClientState::detail_is_open);
    let detail_load_state = mail.map_or(MailDetailLoadState::Idle, |mail| mail.detail_load_state);
    let mark_read_state = mail.map_or(MailMarkReadState::Idle, |mail| mail.mark_read_state);
    let list_is_busy = matches!(
        list_load_state,
        MailListLoadState::InitialLoading
            | MailListLoadState::Refreshing
            | MailListLoadState::LoadingMore
    );
    let collection_state = if !is_available
        || matches!(list_load_state, MailListLoadState::Failed)
            && mail.is_none_or(|mail| mail.mails.is_empty())
    {
        "error"
    } else if matches!(list_load_state, MailListLoadState::InitialLoading) {
        "loading"
    } else {
        "ready"
    };
    let status = if detail_open {
        match detail_load_state {
            MailDetailLoadState::Loading => "Loading mail detail",
            MailDetailLoadState::Ready => "Mail detail ready",
            MailDetailLoadState::NotFound => "Mail not found",
            MailDetailLoadState::Forbidden => "Mail access denied",
            MailDetailLoadState::Expired => "Mail expired",
            MailDetailLoadState::Failed => "Mail detail request failed",
            MailDetailLoadState::Idle => "Mail detail",
        }
    } else {
        match mail.map(|mail| (&mail.availability, mail.list_load_state)) {
            Some((MailAvailability::Ready, MailListLoadState::InitialLoading)) => "Loading mail",
            Some((MailAvailability::Ready, MailListLoadState::Refreshing)) => "Refreshing mail",
            Some((MailAvailability::Ready, MailListLoadState::LoadingMore)) => "Loading more mail",
            Some((MailAvailability::Ready, MailListLoadState::Empty)) => "No mail",
            Some((MailAvailability::Ready, MailListLoadState::Failed)) => "Mail request failed",
            Some((MailAvailability::Ready, _)) => "Mail ready",
            Some((MailAvailability::AwaitingCharacterTicket, _)) => "Waiting for character session",
            Some((MailAvailability::Unavailable { .. }, _)) | None => "Mail unavailable",
        }
    }
    .to_owned();
    let now_unix_seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(i64::MIN, |duration| duration.as_secs() as i64);
    let items = mail.map_or_else(Vec::new, |mail| {
        mail.mails
            .iter()
            .map(|summary| {
                main_world_mail_list_item(
                    summary,
                    now_unix_seconds,
                    mail.selected_mail_id() == Some(summary.mail_id.as_str()),
                )
            })
            .collect()
    });
    let count = mail.map_or_else(String::new, |mail| match mail.list_load_state {
        MailListLoadState::Ready | MailListLoadState::LoadingMore => {
            format!("{} messages", mail.mails.len())
        }
        MailListLoadState::Empty => "0 messages".to_owned(),
        _ => String::new(),
    });
    let has_more = mail.is_some_and(|mail| {
        !mail.list_stale
            && mail
                .pagination
                .as_ref()
                .is_some_and(|pagination| pagination.next_offset.is_some())
    });
    let can_load_more = mail.is_some_and(|mail| mail.next_page_query().is_some());
    let load_more_loading = matches!(list_load_state, MailListLoadState::LoadingMore);
    let detail = mail.and_then(|mail| mail.selected_mail.as_ref());
    let detail_expired = detail.is_some_and(|detail| {
        detail.summary.status.eq_ignore_ascii_case("expired")
            || detail
                .summary
                .expires_at
                .as_deref()
                .and_then(rfc3339_unix_seconds)
                .is_some_and(|expires_at| expires_at <= now_unix_seconds)
    });
    let attachments = detail.map_or_else(Vec::new, |detail| {
        detail
            .attachments
            .iter()
            .take(MAIL_MAX_DETAIL_ATTACHMENTS)
            .enumerate()
            .map(|(index, attachment)| main_world_mail_attachment(index, attachment))
            .collect()
    });
    let (error_code, error_message) = match mail.map(|mail| &mail.availability) {
        Some(MailAvailability::Unavailable { reason }) => {
            ("MAIL_UNAVAILABLE".to_owned(), reason.clone())
        }
        Some(MailAvailability::AwaitingCharacterTicket) => (
            "MAIL_AWAITING_CHARACTER_TICKET".to_owned(),
            "Character session is not ready".to_owned(),
        ),
        Some(MailAvailability::Ready) => mail
            .and_then(|mail| {
                if detail_open {
                    mail.detail_error.as_ref()
                } else {
                    mail.last_error.as_ref()
                }
            })
            .map_or_else(
                || (String::new(), String::new()),
                |error| (error.code.clone(), main_world_mail_error_message(error)),
            ),
        None => (
            "MAIL_UNAVAILABLE".to_owned(),
            "Mail service is unavailable".to_owned(),
        ),
    };
    let refresh_disabled = !is_available || read_is_rate_limited || list_is_busy;
    let mark_read_loading = matches!(mark_read_state, MailMarkReadState::Submitting);
    let mark_read_disabled = !is_available
        || read_is_rate_limited
        || !matches!(detail_load_state, MailDetailLoadState::Ready)
        || detail_expired
        || mark_read_loading
        || detail.is_none_or(|detail| !detail.summary.status.eq_ignore_ascii_case("unread"));
    let claim_workflow = mail.and_then(MailClientState::selected_claim_workflow);
    let claim_state = if !is_available {
        "unavailable"
    } else if detail_expired {
        "expired"
    } else {
        claim_workflow
            .map(|workflow| workflow.state.binding_value())
            .unwrap_or("idle")
    };
    let claim_loading = claim_state == "submitting";
    let claim_disabled = !is_available
        || claim_is_rate_limited
        || detail_expired
        || claim_workflow.is_none_or(|workflow| workflow.state.binding_value() != "available");
    let claim_label = main_world_mail_claim_label(claim_workflow, detail_expired);
    let error_visible = !error_message.is_empty();
    for (path, value) in vec![
        (
            "main_world_mail.availability",
            UiBindingValue::Enum(availability.to_owned()),
        ),
        (
            "main_world_mail.collection_state",
            UiBindingValue::Enum(collection_state.to_owned()),
        ),
        (
            "main_world_mail.view_mode",
            UiBindingValue::Enum(if detail_open { "detail" } else { "list" }.to_owned()),
        ),
        ("main_world_mail.items", UiBindingValue::List(items)),
        ("main_world_mail.status", UiBindingValue::String(status)),
        ("main_world_mail.count", UiBindingValue::String(count)),
        ("main_world_mail.has_more", UiBindingValue::Bool(has_more)),
        (
            "main_world_mail.load_more_visibility",
            UiBindingValue::Visibility(if has_more && !detail_open {
                UiBindingVisibility::Visible
            } else {
                UiBindingVisibility::Hidden
            }),
        ),
        (
            "main_world_mail.list_visibility",
            UiBindingValue::Visibility(if detail_open {
                UiBindingVisibility::Hidden
            } else {
                UiBindingVisibility::Visible
            }),
        ),
        (
            "main_world_mail.load_more_disabled",
            UiBindingValue::Bool(!can_load_more || read_is_rate_limited),
        ),
        (
            "main_world_mail.load_more_loading",
            UiBindingValue::Bool(load_more_loading),
        ),
        (
            "main_world_mail.selected.mail_id",
            UiBindingValue::String(
                mail.and_then(MailClientState::selected_mail_id)
                    .unwrap_or_default()
                    .to_owned(),
            ),
        ),
        (
            "main_world_mail.detail_visibility",
            UiBindingValue::Visibility(if detail_open {
                UiBindingVisibility::Visible
            } else {
                UiBindingVisibility::Hidden
            }),
        ),
        (
            "main_world_mail.detail.title",
            UiBindingValue::String(detail.map_or_else(String::new, |detail| {
                bounded_mail_label(&detail.summary.title, 96, "Untitled mail")
            })),
        ),
        (
            "main_world_mail.detail.sender",
            UiBindingValue::String(detail.map_or_else(String::new, |detail| {
                bounded_mail_label(
                    detail.summary.sender.name.as_deref().unwrap_or_default(),
                    48,
                    "System",
                )
            })),
        ),
        (
            "main_world_mail.detail.sent_at",
            UiBindingValue::String(detail.map_or_else(String::new, |detail| {
                bounded_mail_label(
                    detail.summary.created_at.as_deref().unwrap_or_default(),
                    40,
                    "",
                )
            })),
        ),
        (
            "main_world_mail.detail.status",
            UiBindingValue::String(detail.map_or_else(String::new, |detail| {
                main_world_mail_status_label(&detail.summary, detail_expired)
            })),
        ),
        (
            "main_world_mail.detail.content",
            UiBindingValue::String(
                detail.map_or_else(String::new, |detail| bounded_mail_content(&detail.content)),
            ),
        ),
        (
            "main_world_mail.detail.attachments",
            UiBindingValue::List(attachments),
        ),
        (
            "main_world_mail.claim.state",
            UiBindingValue::Enum(claim_state.to_owned()),
        ),
        (
            "main_world_mail.claim.label",
            UiBindingValue::String(claim_label),
        ),
        (
            "main_world_mail.mark_read_disabled",
            UiBindingValue::Bool(mark_read_disabled),
        ),
        (
            "main_world_mail.mark_read_loading",
            UiBindingValue::Bool(mark_read_loading),
        ),
        (
            "main_world_mail.claim_disabled",
            UiBindingValue::Bool(claim_disabled),
        ),
        (
            "main_world_mail.claim_loading",
            UiBindingValue::Bool(claim_loading),
        ),
        (
            "main_world_mail.error.code",
            UiBindingValue::String(error_code),
        ),
        (
            "main_world_mail.error.message",
            UiBindingValue::String(error_message),
        ),
        (
            "main_world_mail.error_visibility",
            UiBindingValue::Visibility(if error_visible {
                UiBindingVisibility::Visible
            } else {
                UiBindingVisibility::Hidden
            }),
        ),
        (
            "main_world_mail.retry_disabled",
            UiBindingValue::Bool(
                !is_available
                    || read_is_rate_limited
                    || if detail_open {
                        matches!(detail_load_state, MailDetailLoadState::Loading)
                            || mark_read_loading
                    } else {
                        list_is_busy
                    },
            ),
        ),
        (
            "main_world_mail.retry_loading",
            UiBindingValue::Bool(
                detail_open && matches!(detail_load_state, MailDetailLoadState::Loading),
            ),
        ),
        (
            "main_world_mail.refresh_disabled",
            UiBindingValue::Bool(refresh_disabled),
        ),
        (
            "main_world_mail.refresh_loading",
            UiBindingValue::Bool(matches!(
                list_load_state,
                MailListLoadState::InitialLoading | MailListLoadState::Refreshing
            )),
        ),
    ] {
        set_binding(
            &contract,
            &mut values,
            MAIN_WORLD_MAIL_DOCUMENT_ID,
            OWNER_MAIN_WORLD_MAIL_PANEL.as_str(),
            path,
            value,
        );
    }
}

fn main_world_mail_list_item(
    mail: &MailSummary,
    now_unix_seconds: i64,
    selected: bool,
) -> UiBindingValue {
    let expired = mail
        .expires_at
        .as_deref()
        .and_then(rfc3339_unix_seconds)
        .is_some_and(|expires_at| expires_at <= now_unix_seconds);
    let status = main_world_mail_status_label(mail, expired);
    UiBindingValue::Record(BTreeMap::from([
        (
            "mail_id".to_owned(),
            UiBindingValue::String(mail.mail_id.clone()),
        ),
        (
            "title".to_owned(),
            UiBindingValue::String(bounded_mail_label(&mail.title, 96, "Untitled mail")),
        ),
        (
            "sender".to_owned(),
            UiBindingValue::String(bounded_mail_label(
                mail.sender.name.as_deref().unwrap_or_default(),
                48,
                "System",
            )),
        ),
        (
            "sent_at".to_owned(),
            UiBindingValue::String(bounded_mail_label(
                mail.created_at.as_deref().unwrap_or_default(),
                40,
                "",
            )),
        ),
        ("status".to_owned(), UiBindingValue::String(status)),
        (
            "attachment_label".to_owned(),
            UiBindingValue::String(if mail.has_attachments {
                "Attachment".to_owned()
            } else {
                String::new()
            }),
        ),
        (
            "unread".to_owned(),
            UiBindingValue::Bool(mail.status.eq_ignore_ascii_case("unread")),
        ),
        ("expired".to_owned(), UiBindingValue::Bool(expired)),
        ("selected".to_owned(), UiBindingValue::Bool(selected)),
        ("disabled".to_owned(), UiBindingValue::Bool(false)),
    ]))
}

fn main_world_mail_attachment(index: usize, attachment: &MailAttachment) -> UiBindingValue {
    let label = match attachment.r#type.trim().to_ascii_lowercase().as_str() {
        "item" => "Item",
        "currency" => "Currency",
        "resource" => "Resource",
        _ => "Attachment",
    };
    UiBindingValue::Record(BTreeMap::from([
        (
            "attachment_id".to_owned(),
            UiBindingValue::String(format!("attachment-{index}")),
        ),
        ("label".to_owned(), UiBindingValue::String(label.to_owned())),
        (
            "count".to_owned(),
            UiBindingValue::String(attachment.count.max(0).to_string()),
        ),
        (
            "bind_status".to_owned(),
            UiBindingValue::String(if attachment.binded {
                "Bound".to_owned()
            } else {
                String::new()
            }),
        ),
    ]))
}

fn main_world_mail_error_message(error: &MailClientError) -> String {
    match error.code.as_str() {
        "MAIL_REQUEST_TIMEOUT" => return "Mail request timed out; try again later".to_owned(),
        "MAIL_RESPONSE_TOO_LARGE" => return "Mail response was too large to display".to_owned(),
        "MAIL_RESPONSE_INVALID" => return "Mail service returned an invalid response".to_owned(),
        _ => {}
    }
    match error.status {
        Some(400) => "Mail request was rejected".to_owned(),
        Some(401) => "Mail session expired; sign in again".to_owned(),
        Some(403) => "You no longer have access to this mail".to_owned(),
        Some(404) => "This mail is no longer available".to_owned(),
        Some(409) => "Mail state changed; refresh and try again".to_owned(),
        Some(410) => "This mail has expired".to_owned(),
        Some(429) => "Mail requests are limited; try again shortly".to_owned(),
        Some(503) => "Mail service is temporarily unavailable".to_owned(),
        _ => "Mail request could not be completed".to_owned(),
    }
}

fn main_world_mail_claim_label(workflow: Option<&MailClaimWorkflow>, expired: bool) -> String {
    if expired {
        return "Attachments expired".to_owned();
    }
    let Some(workflow) = workflow else {
        return String::new();
    };
    match workflow.state.binding_value() {
        "available" => "Attachments available".to_owned(),
        "submitting" => "Submitting claim".to_owned(),
        "processing" | "reconciliation_pending" => "Confirming claim result".to_owned(),
        "claimed" => "Attachments claimed".to_owned(),
        "already_claimed" => "Attachments were already claimed".to_owned(),
        "expired" => "Attachments expired".to_owned(),
        "retryable_failure" if workflow.player_retryable => {
            "Claim was not completed; retry is allowed later".to_owned()
        }
        "retryable_failure" => "Claim retry requires server confirmation".to_owned(),
        "blocked_capacity" if workflow.player_retryable => {
            "Free inventory space, then retry later".to_owned()
        }
        "blocked_capacity" => "Inventory capacity blocked this claim".to_owned(),
        "permanent_failure" => "Claim could not be completed".to_owned(),
        "manual_review" if workflow.exhausted => {
            "Claim result is unknown and needs confirmation".to_owned()
        }
        "manual_review" => "Claim is under manual review".to_owned(),
        "unavailable" => "Attachment claiming is temporarily unavailable".to_owned(),
        _ => String::new(),
    }
}

fn main_world_mail_status_label(mail: &MailSummary, expired: bool) -> String {
    if expired {
        return "Expired".to_owned();
    }
    let status = match mail.status.as_str() {
        "unread" => "Unread",
        "read" => "Read",
        "claiming" => "Claim pending",
        "claimed" => "Claimed",
        _ => "Mail",
    };
    mail.expires_at.as_deref().map_or_else(
        || status.to_owned(),
        |expires_at| {
            format!(
                "{status} | expires {}",
                bounded_mail_label(expires_at, 40, "")
            )
        },
    )
}

fn bounded_mail_label(value: &str, max_chars: usize, fallback: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        return fallback.to_owned();
    }
    let mut chars = value.chars();
    let bounded = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{bounded}...")
    } else {
        bounded
    }
}

fn bounded_mail_content(value: &str) -> String {
    const MAX_BINDING_BYTES: usize = 4 * 1024;
    const ELLIPSIS_BYTES: usize = 3;
    let mut output = String::new();
    let mut truncated = false;
    for (index, character) in value.chars().enumerate() {
        if index >= 8 * 1024
            || output.len() + character.len_utf8() > MAX_BINDING_BYTES - ELLIPSIS_BYTES
        {
            truncated = true;
            break;
        }
        output.push(character);
    }
    if truncated {
        output.push_str("...");
    }
    output
}

fn rfc3339_unix_seconds(value: &str) -> Option<i64> {
    let bytes = value.as_bytes();
    if bytes.len() < 20
        || bytes.get(4) != Some(&b'-')
        || bytes.get(7) != Some(&b'-')
        || bytes
            .get(10)
            .is_none_or(|byte| !matches!(byte, b'T' | b't' | b' '))
        || bytes.get(13) != Some(&b':')
        || bytes.get(16) != Some(&b':')
    {
        return None;
    }
    let year = parse_decimal(bytes, 0, 4)? as i64;
    let month = parse_decimal(bytes, 5, 2)?;
    let day = parse_decimal(bytes, 8, 2)?;
    let hour = parse_decimal(bytes, 11, 2)?;
    let minute = parse_decimal(bytes, 14, 2)?;
    let second = parse_decimal(bytes, 17, 2)?;
    if !(1..=12).contains(&month)
        || !(1..=days_in_month(year, month)).contains(&day)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }
    let mut suffix = 19;
    if bytes.get(suffix) == Some(&b'.') {
        suffix += 1;
        let fraction_start = suffix;
        while bytes.get(suffix).is_some_and(u8::is_ascii_digit) {
            suffix += 1;
        }
        if suffix == fraction_start {
            return None;
        }
    }
    let offset = match bytes.get(suffix) {
        Some(b'Z' | b'z') if suffix + 1 == bytes.len() => 0,
        Some(sign @ (b'+' | b'-')) if suffix + 6 == bytes.len() => {
            if bytes.get(suffix + 3) != Some(&b':') {
                return None;
            }
            let offset_hour = parse_decimal(bytes, suffix + 1, 2)?;
            let offset_minute = parse_decimal(bytes, suffix + 4, 2)?;
            if offset_hour > 23 || offset_minute > 59 {
                return None;
            }
            let seconds = i64::from(offset_hour * 3600 + offset_minute * 60);
            if *sign == b'+' { seconds } else { -seconds }
        }
        _ => return None,
    };
    let local_seconds =
        days_from_civil(year, month, day) * 86_400 + i64::from(hour * 3600 + minute * 60 + second);
    Some(local_seconds - offset)
}

fn parse_decimal(bytes: &[u8], start: usize, len: usize) -> Option<u32> {
    let mut value = 0_u32;
    for byte in bytes.get(start..start + len)? {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value * 10 + u32::from(byte - b'0');
    }
    Some(value)
}

fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        4 | 6 | 9 | 11 => 30,
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        _ => 31,
    }
}

fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let adjusted_month = i64::from(month) + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * adjusted_month + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn localized_main_world_connection_status(state: GameConnectionState, i18n: &UiI18n) -> String {
    let (key, fallback) = match state {
        GameConnectionState::NotConnected => ("offline", "Game services offline"),
        GameConnectionState::Connecting => ("connecting", "Connecting to game services"),
        GameConnectionState::Connected => ("connected", "Transport connected"),
        GameConnectionState::Authenticating => ("authenticating", "Authenticating game session"),
        GameConnectionState::Authenticated => ("authenticated", "Game session connected"),
        GameConnectionState::Disconnected => ("disconnected", "Connection lost"),
        GameConnectionState::Reconnecting => ("reconnecting", "Reconnecting game session"),
        GameConnectionState::ReconnectFailed => ("failed", "Reconnect failed"),
    };
    i18n.tr(&format!("main_world.connection.{key}"), fallback)
}

fn localized_main_world_character_summary(character_id: Option<&str>, i18n: &UiI18n) -> String {
    let Some(character_id) = character_id.filter(|character_id| !character_id.is_empty()) else {
        return i18n.tr("main_world.character.unavailable", "Character unavailable");
    };
    let compact_id = character_id.chars().take(8).collect::<String>();
    format!(
        "{} {compact_id}",
        i18n.tr("main_world.character", "Character")
    )
}

fn localized_main_world_transition_status(phase: MainWorldEntryPhase, i18n: &UiI18n) -> String {
    let (key, fallback) = match phase {
        MainWorldEntryPhase::Active => return String::new(),
        MainWorldEntryPhase::Exiting => ("returning_lobby", "Returning to lobby"),
        MainWorldEntryPhase::Recovering => ("recovering", "Recovering game session"),
        MainWorldEntryPhase::HomeLoading => ("entering_home", "Entering home"),
        MainWorldEntryPhase::HomeActive => ("home_active", "Home active"),
        MainWorldEntryPhase::ReturningFromHome => ("returning_home", "Returning from home"),
        MainWorldEntryPhase::Validating => ("validating", "Validating game session"),
        MainWorldEntryPhase::JoiningRoom => ("joining", "Joining game room"),
        MainWorldEntryPhase::LoadingScene => ("loading", "Loading main world"),
        MainWorldEntryPhase::WaitingSceneReady => ("preparing", "Preparing main world"),
        MainWorldEntryPhase::LobbyIdle => ("waiting", "Waiting for main world"),
        MainWorldEntryPhase::Failed => ("unavailable", "Main world unavailable"),
    };
    i18n.tr(&format!("main_world.transition.{key}"), fallback)
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
pub(super) fn prepare_main_world_mail_audit_fixture(
    audit: Res<UiAuditConfig>,
    mut mail: ResMut<MailClientState>,
    mut screen_commands: MessageWriter<DeclarativeScreenHostCommand>,
    mut opened: Local<bool>,
) {
    if !audit.targets_screen("main_world")
        || audit.stable_fixture_id() != Some("stage18_main_world_mail")
    {
        return;
    }
    *mail = MailClientState::main_world_mail_audit_fixture();
    if !*opened {
        screen_commands.write(DeclarativeScreenHostCommand::OpenDetachedRoute {
            route: MainWorldDocumentPanel::Mail.route().to_owned(),
        });
        *opened = true;
    }
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
    use crate::framework::audio::prelude::AudioCommand;
    #[cfg(all(debug_assertions, not(target_os = "android")))]
    use crate::framework::ui::audit::UiAuditConfig;
    use crate::framework::{
        network::NetworkCommand,
        scene::prelude::SceneCommand,
        ui::{
            core::{UiInputMode, UiMetrics, UiSafeArea, UiViewport, binding::UiBindingSystems},
            document::{
                UiActionDispatchContext, UiDocument, UiDocumentInputMode, UiDocumentPlatform,
                UiDocumentPreviewPlugin, UiDocumentRuntime, UiDocumentRuntimePlugin,
                UiDocumentRuntimeSystems, UiNode, UiSafeAreaClass, UiTargetProfile,
                parse_approved_document_registration,
            },
            style::{UiFontAssets, UiTheme},
            widgets::{UiButtonEvent, UiButtonEventKind},
        },
    };
    use crate::game::myserver::mail::MailClaimWorkflowState;
    use bevy::{
        ecs::message::{MessageCursor, Messages},
        picking::Pickable,
        state::app::StatesPlugin,
    };

    const DOCUMENTS: [(&str, &str); 7] = [
        (MAIN_WORLD_HUD_DOCUMENT_ID, MAIN_WORLD_HUD_SOURCE),
        (MAIN_WORLD_MAIL_DOCUMENT_ID, MAIN_WORLD_MAIL_SOURCE),
        (TOUCH_RIPPLE_DOCUMENT_ID, TOUCH_RIPPLE_SOURCE),
        (SAMPLE_SCENE_DOCUMENT_ID, SAMPLE_SCENE_SOURCE),
        (ROBOT_SYNC_DOCUMENT_ID, ROBOT_SYNC_SOURCE),
        (FANGYUAN_PLAYER_PREVIEW_DOCUMENT_ID, FANGYUAN_PREVIEW_SOURCE),
        (FANGYUAN_HOME_DOCUMENT_ID, FANGYUAN_HOME_SOURCE),
    ];

    const REGISTRATIONS: [&str; 7] = [
        include_str!(
            "../../../../assets/ui/documents/approved/gameplay/main_world_hud.promotion.v1.json"
        ),
        include_str!(
            "../../../../assets/ui/documents/approved/gameplay/main_world_mail.promotion.v1.json"
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
        assert_eq!(hosts.len(), 7);

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

            let (panel, layer) = if document_id == MAIN_WORLD_MAIL_DOCUMENT_ID {
                (UiDocumentPanel::Floating, UiDocumentLayer::Floating)
            } else {
                (UiDocumentPanel::Hud, UiDocumentLayer::Page)
            };
            assert_eq!(host.panel, panel);
            assert_eq!(host.layer, layer);
            assert_eq!(host.audit_profiles, DEFAULT_AUDIT_PROFILES);
            assert_eq!(registration.panel(), panel);
            assert_eq!(registration.layer(), layer);
            assert_eq!(registration.owner(), host.owner.as_str());
            assert_eq!(registration.route(), host.route);
            assert_eq!(audit.actions.len(), host.action_allowlist.len());
            assert_eq!(audit.bindings.len(), host.binding_schema.len());
        }
    }

    #[test]
    fn main_world_mail_host_contract_keeps_repeat_items_local_and_actions_closed() {
        let contract = GameplayHudHostContract::default();
        let host = main_world_mail_declarative_screen_host(&contract);
        assert_eq!(host.document_id.as_str(), MAIN_WORLD_MAIL_DOCUMENT_ID);
        assert_eq!(host.owner, OWNER_MAIN_WORLD_MAIL_PANEL);
        assert_eq!(host.route, "main_world_mail");
        assert_eq!(host.panel, UiDocumentPanel::Floating);
        assert_eq!(host.layer, UiDocumentLayer::Floating);
        assert_eq!(host.audit_profiles, DEFAULT_AUDIT_PROFILES);
        assert_eq!(host.action_allowlist.len(), 8);
        assert!(
            host.binding_schema
                .keys()
                .all(|key| key.scope != UiBindingScope::Item)
        );

        let mail = UiDocument::parse_and_validate_json(MAIN_WORLD_MAIL_SOURCE).unwrap();
        let list = find_document_node(&mail.document().root, "main_world_mail.list").unwrap();
        let repeat = list.repeat().unwrap();
        assert_eq!(repeat.source.as_str(), "main_world_mail.items");
        assert_eq!(repeat.key, "mail_id");
        assert_eq!(
            repeat.item_bindings[&UiBindingPath::from_str("main_world_mail.item.mail_id").unwrap()],
            "mail_id"
        );

        let select =
            find_document_node(&mail.document().root, MAIN_WORLD_MAIL_SELECT_NODE).unwrap();
        assert!(matches!(
            select,
            UiNode::Button {
                on_click: Some(invocation),
                ..
            } if matches!(
                invocation.params.get("mail_id"),
                Some(UiActionValue::ItemBinding(path))
                    if path.as_str() == "main_world_mail.item.mail_id"
            )
        ));
        for node_id in [MAIN_WORLD_MAIL_MARK_READ_NODE, MAIN_WORLD_MAIL_CLAIM_NODE] {
            let node = find_document_node(&mail.document().root, node_id).unwrap();
            assert!(matches!(
                node,
                UiNode::Button {
                    on_click: Some(invocation),
                    ..
                } if matches!(
                    invocation.params.get("mail_id"),
                    Some(UiActionValue::HostBinding(path))
                        if path.as_str() == "main_world_mail.selected.mail_id"
                )
            ));
        }
    }

    #[test]
    fn main_world_mail_uses_a_distinct_packaged_fallback_with_safe_close() {
        let host = main_world_mail_declarative_screen_host(&GameplayHudHostContract::default());
        let fallback = host.fallback_source.as_ref().unwrap();
        assert_ne!(host.source.source_path, fallback.source_path);
        assert_ne!(host.source.source_json, fallback.source_json);
        let fallback = UiDocument::parse_and_validate_json(&fallback.source_json).unwrap();
        assert_eq!(
            fallback.document().document_id.as_str(),
            MAIN_WORLD_MAIL_DOCUMENT_ID
        );
        let close =
            find_document_node(&fallback.document().root, MAIN_WORLD_MAIL_CLOSE_NODE).unwrap();
        assert!(matches!(
            close,
            UiNode::Button {
                on_click: Some(invocation),
                ..
            } if invocation.action.as_str() == ACTION_MAIN_WORLD_MAIL_CLOSE
                && invocation.params.is_empty()
        ));
    }

    #[test]
    fn main_world_mail_registry_rejects_wrong_document_owner_source_and_params() {
        let mut registry = UiActionRegistry::default();
        for descriptor in gameplay_action_descriptors()
            .into_iter()
            .filter(|descriptor| descriptor.document_id.as_str() == MAIN_WORLD_MAIL_DOCUMENT_ID)
        {
            registry.register(descriptor).unwrap();
        }
        let document = UiDocument::parse_and_validate_json(MAIN_WORLD_MAIL_SOURCE).unwrap();
        let context = UiActionDispatchContext {
            owner: OWNER_MAIN_WORLD_MAIL_PANEL.as_str().to_owned(),
            owner_alive: true,
            source_node: UiNodeId::from_str(MAIN_WORLD_MAIL_REFRESH_NODE).unwrap(),
        };
        assert!(registry.dispatch(&document, &context).is_ok());

        let mut wrong_owner = context.clone();
        wrong_owner.owner = OWNER_MAIN_WORLD.as_str().to_owned();
        assert_eq!(
            registry.dispatch(&document, &wrong_owner).unwrap_err().code,
            "UI_ACTION_OWNER_FORBIDDEN"
        );

        let mut wrong_source = context.clone();
        wrong_source.source_node = UiNodeId::from_str("main_world_mail.forged").unwrap();
        assert_eq!(
            registry
                .dispatch(&document, &wrong_source)
                .unwrap_err()
                .code,
            "UI_ACTION_SOURCE_NODE_UNKNOWN"
        );

        let wrong_document_source = MAIN_WORLD_MAIL_SOURCE.replacen(
            "\"document_id\": \"game.main_world_mail\"",
            "\"document_id\": \"game.forged_mail\"",
            1,
        );
        let wrong_document = UiDocument::parse_and_validate_json(&wrong_document_source).unwrap();
        assert_eq!(
            registry
                .dispatch(&wrong_document, &context)
                .unwrap_err()
                .code,
            "UI_ACTION_DOCUMENT_FORBIDDEN"
        );

        let malformed_params_source = MAIN_WORLD_MAIL_SOURCE.replacen(
            "\"on_click\": { \"action\": \"main_world_mail.refresh\" }",
            "\"on_click\": { \"action\": \"main_world_mail.refresh\", \"params\": { \"unexpected\": { \"kind\": \"string\", \"value\": \"forged\" } } }",
            1,
        );
        let malformed_params =
            UiDocument::parse_and_validate_json(&malformed_params_source).unwrap();
        assert_eq!(
            registry
                .dispatch(&malformed_params, &context)
                .unwrap_err()
                .code,
            "UI_ACTION_PARAM_UNKNOWN"
        );
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
        assert_eq!(descriptors.len(), 25);
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

        let mail_actions = descriptors
            .iter()
            .filter(|descriptor| descriptor.document_id.as_str() == MAIN_WORLD_MAIL_DOCUMENT_ID)
            .collect::<Vec<_>>();
        assert_eq!(mail_actions.len(), 8);
        for action_id in [
            ACTION_MAIN_WORLD_MAIL_SELECT,
            ACTION_MAIN_WORLD_MAIL_MARK_READ,
            ACTION_MAIN_WORLD_MAIL_CLAIM,
        ] {
            let descriptor = mail_actions
                .iter()
                .find(|descriptor| descriptor.id.as_str() == action_id)
                .unwrap();
            let mail_id = descriptor.params.get("mail_id").unwrap();
            assert!(mail_id.required);
            assert_eq!(
                mail_id.value_type,
                UiActionParamType::OpaqueId {
                    max_bytes: MAIN_WORLD_MAIL_ID_MAX_BYTES,
                }
            );
        }
        for action_id in [
            ACTION_MAIN_WORLD_MAIL_REFRESH,
            ACTION_MAIN_WORLD_MAIL_LOAD_MORE,
            ACTION_MAIN_WORLD_MAIL_RETRY,
            ACTION_MAIN_WORLD_MAIL_BACK_TO_LIST,
            ACTION_MAIN_WORLD_MAIL_CLOSE,
        ] {
            assert!(
                mail_actions
                    .iter()
                    .find(|descriptor| descriptor.id.as_str() == action_id)
                    .unwrap()
                    .params
                    .is_empty()
            );
        }

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
    fn active_main_world_mail_open_uses_only_the_host_and_mail_adapters() {
        let mut app = action_test_app();
        app.world_mut()
            .insert_resource(MailClientState::ready_for_test());
        app.world_mut().write_message(dispatch(
            MAIN_WORLD_HUD_DOCUMENT_ID,
            OWNER_MAIN_WORLD.as_str(),
            ACTION_MAIN_WORLD_OPEN_MAIL,
            MAIN_WORLD_MAIL_NODE,
            BTreeMap::new(),
        ));
        app.update();

        assert!(matches!(
            read_messages::<DeclarativeScreenHostCommand>(&app).as_slice(),
            [DeclarativeScreenHostCommand::OpenDetachedRoute { route }]
                if route == MainWorldDocumentPanel::Mail.route()
        ));
        assert!(matches!(
            read_messages::<MailClientCommand>(&app).as_slice(),
            [MailClientCommand::LoadList { query }] if query == &MailListQuery::default()
        ));
        assert!(read_messages::<NetworkCommand>(&app).is_empty());
        assert!(read_messages::<SceneCommand>(&app).is_empty());
        assert!(read_messages::<GameRouteCommand>(&app).is_empty());
        assert!(read_messages::<MainWorldEntryIntent>(&app).is_empty());
    }

    #[test]
    fn main_world_mail_select_revalidates_the_full_authoritative_mail_id() {
        let mail_id = "mail_0123456789abcdef_0123456789abcdef";
        let mut app = action_test_app();
        app.world_mut()
            .insert_resource(MailClientState::ready_with_list_for_test(
                vec![mail_summary_for_test(mail_id, "Authoritative")],
                1,
                crate::game::myserver::mail::MailPagination {
                    limit: 50,
                    offset: 0,
                    next_offset: None,
                },
            ));
        for candidate in [mail_id, "mail_stale"] {
            app.world_mut().write_message(dispatch(
                MAIN_WORLD_MAIL_DOCUMENT_ID,
                OWNER_MAIN_WORLD_MAIL_PANEL.as_str(),
                ACTION_MAIN_WORLD_MAIL_SELECT,
                MAIN_WORLD_MAIL_SELECT_NODE,
                BTreeMap::from([(
                    "mail_id".to_owned(),
                    UiActionValue::String(candidate.to_owned()),
                )]),
            ));
        }
        app.update();

        assert!(matches!(
            read_messages::<MailClientCommand>(&app).as_slice(),
            [MailClientCommand::LoadMail { mail_id: selected }] if selected == mail_id
        ));

        app.world_mut().write_message(dispatch(
            MAIN_WORLD_MAIL_DOCUMENT_ID,
            OWNER_MAIN_WORLD_MAIL_PANEL.as_str(),
            ACTION_MAIN_WORLD_MAIL_SELECT,
            MAIN_WORLD_MAIL_SELECT_NODE,
            BTreeMap::from([("mail_id".to_owned(), UiActionValue::Bool(true))]),
        ));
        app.update();
        assert_eq!(read_messages::<MailClientCommand>(&app).len(), 1);
    }

    #[test]
    fn main_world_mail_load_more_uses_only_the_authoritative_next_offset() {
        let mut app = action_test_app();
        app.world_mut()
            .insert_resource(MailClientState::ready_with_list_for_test(
                vec![mail_summary_for_test("mail_1", "First")],
                1,
                crate::game::myserver::mail::MailPagination {
                    limit: 25,
                    offset: 0,
                    next_offset: Some(25),
                },
            ));
        app.world_mut().write_message(dispatch(
            MAIN_WORLD_MAIL_DOCUMENT_ID,
            OWNER_MAIN_WORLD_MAIL_PANEL.as_str(),
            ACTION_MAIN_WORLD_MAIL_LOAD_MORE,
            MAIN_WORLD_MAIL_LOAD_MORE_NODE,
            BTreeMap::new(),
        ));
        app.update();

        assert!(matches!(
            read_messages::<MailClientCommand>(&app).as_slice(),
            [MailClientCommand::LoadList { query }]
                if query.status.is_none()
                    && query.limit == Some(25)
                    && query.offset == Some(25)
        ));
    }

    #[test]
    fn main_world_mail_mark_read_and_back_actions_revalidate_selected_detail() {
        let detail = mail_detail_for_test("mail_1", "unread", "Body");
        let mut app = action_test_app();
        app.world_mut()
            .insert_resource(MailClientState::ready_with_detail_for_test(
                vec![mail_summary_for_test("mail_1", "Selected")],
                1,
                crate::game::myserver::mail::MailPagination::default(),
                detail,
            ));
        app.world_mut().write_message(dispatch(
            MAIN_WORLD_MAIL_DOCUMENT_ID,
            OWNER_MAIN_WORLD_MAIL_PANEL.as_str(),
            ACTION_MAIN_WORLD_MAIL_MARK_READ,
            MAIN_WORLD_MAIL_MARK_READ_NODE,
            BTreeMap::from([(
                "mail_id".to_owned(),
                UiActionValue::String("mail_1".to_owned()),
            )]),
        ));
        app.world_mut().write_message(dispatch(
            MAIN_WORLD_MAIL_DOCUMENT_ID,
            OWNER_MAIN_WORLD_MAIL_PANEL.as_str(),
            ACTION_MAIN_WORLD_MAIL_MARK_READ,
            MAIN_WORLD_MAIL_MARK_READ_NODE,
            BTreeMap::from([(
                "mail_id".to_owned(),
                UiActionValue::String("mail_stale".to_owned()),
            )]),
        ));
        app.update();
        assert!(matches!(
            read_messages::<MailClientCommand>(&app).as_slice(),
            [MailClientCommand::MarkRead { mail_id }] if mail_id == "mail_1"
        ));

        app.world_mut().write_message(dispatch(
            MAIN_WORLD_MAIL_DOCUMENT_ID,
            OWNER_MAIN_WORLD_MAIL_PANEL.as_str(),
            ACTION_MAIN_WORLD_MAIL_BACK_TO_LIST,
            MAIN_WORLD_MAIL_BACK_TO_LIST_NODE,
            BTreeMap::new(),
        ));
        app.update();
        assert!(
            read_messages::<MailClientCommand>(&app)
                .iter()
                .any(|command| matches!(command, MailClientCommand::DismissDetail))
        );
    }

    #[test]
    fn main_world_mail_claim_revalidates_authoritative_attachment_and_expiry() {
        let mut detail = mail_detail_for_test("mail_1", "unread", "Reward");
        detail.summary.has_attachments = true;
        detail.attachments = vec![MailAttachment {
            r#type: "item".to_owned(),
            id: Some(1001),
            count: 2,
            binded: true,
        }];
        let mut app = action_test_app();
        app.world_mut()
            .insert_resource(MailClientState::ready_with_detail_for_test(
                vec![detail.summary.clone()],
                1,
                crate::game::myserver::mail::MailPagination::default(),
                detail,
            ));
        for (document, owner, source, candidate) in [
            (
                MAIN_WORLD_MAIL_DOCUMENT_ID,
                OWNER_MAIN_WORLD_MAIL_PANEL.as_str(),
                MAIN_WORLD_MAIL_CLAIM_NODE,
                "mail_1",
            ),
            (
                MAIN_WORLD_MAIL_DOCUMENT_ID,
                OWNER_MAIN_WORLD_MAIL_PANEL.as_str(),
                MAIN_WORLD_MAIL_CLAIM_NODE,
                "mail_stale",
            ),
            (
                MAIN_WORLD_HUD_DOCUMENT_ID,
                OWNER_MAIN_WORLD_MAIL_PANEL.as_str(),
                MAIN_WORLD_MAIL_CLAIM_NODE,
                "mail_1",
            ),
            (
                MAIN_WORLD_MAIL_DOCUMENT_ID,
                OWNER_MAIN_WORLD.as_str(),
                MAIN_WORLD_MAIL_CLAIM_NODE,
                "mail_1",
            ),
            (
                MAIN_WORLD_MAIL_DOCUMENT_ID,
                OWNER_MAIN_WORLD_MAIL_PANEL.as_str(),
                MAIN_WORLD_MAIL_SELECT_NODE,
                "mail_1",
            ),
        ] {
            app.world_mut().write_message(dispatch(
                document,
                owner,
                ACTION_MAIN_WORLD_MAIL_CLAIM,
                source,
                BTreeMap::from([(
                    "mail_id".to_owned(),
                    UiActionValue::String(candidate.to_owned()),
                )]),
            ));
        }
        app.update();
        assert!(matches!(
            read_messages::<MailClientCommand>(&app).as_slice(),
            [MailClientCommand::Claim { mail_id }] if mail_id == "mail_1"
        ));

        let mut expired = mail_detail_for_test("mail_expired", "unread", "Reward");
        expired.summary.has_attachments = true;
        expired.summary.expires_at = Some("2000-01-01T00:00:00Z".to_owned());
        expired.attachments = vec![MailAttachment {
            r#type: "item".to_owned(),
            id: Some(1002),
            count: 1,
            binded: true,
        }];
        let mut expired_app = action_test_app();
        expired_app
            .world_mut()
            .insert_resource(MailClientState::ready_with_detail_for_test(
                vec![expired.summary.clone()],
                1,
                crate::game::myserver::mail::MailPagination::default(),
                expired,
            ));
        expired_app.world_mut().write_message(dispatch(
            MAIN_WORLD_MAIL_DOCUMENT_ID,
            OWNER_MAIN_WORLD_MAIL_PANEL.as_str(),
            ACTION_MAIN_WORLD_MAIL_CLAIM,
            MAIN_WORLD_MAIL_CLAIM_NODE,
            BTreeMap::from([(
                "mail_id".to_owned(),
                UiActionValue::String("mail_expired".to_owned()),
            )]),
        ));
        expired_app.update();
        assert!(read_messages::<MailClientCommand>(&expired_app).is_empty());
    }

    #[test]
    fn main_world_home_and_lobby_actions_submit_one_coordinator_intent_without_direct_exit_work() {
        for (action, source, expected_intent) in [
            (
                ACTION_MAIN_WORLD_ENTER_HOME,
                MAIN_WORLD_HOME_NODE,
                MainWorldEntryIntent::EnterHome,
            ),
            (
                ACTION_MAIN_WORLD_RETURN_LOBBY,
                MAIN_WORLD_RETURN_LOBBY_NODE,
                MainWorldEntryIntent::ExitToLobby,
            ),
        ] {
            let mut app = action_test_app();
            for _ in 0..2 {
                app.world_mut().write_message(dispatch(
                    MAIN_WORLD_HUD_DOCUMENT_ID,
                    OWNER_MAIN_WORLD.as_str(),
                    action,
                    source,
                    BTreeMap::new(),
                ));
            }
            app.update();

            assert_eq!(
                read_messages::<MainWorldEntryIntent>(&app),
                [expected_intent]
            );
            assert!(read_messages::<NetworkCommand>(&app).is_empty());
            assert!(read_messages::<SceneCommand>(&app).is_empty());
            assert!(read_messages::<GameRouteCommand>(&app).is_empty());
            assert!(read_messages::<DeclarativeScreenHostCommand>(&app).is_empty());
        }
    }

    #[test]
    fn main_world_mail_close_only_closes_its_route_and_preserves_mail_state() {
        let mut app = action_test_app();
        app.world_mut()
            .insert_resource(MailClientState::ready_with_reconciliation_for_test(
                "mail-42",
            ));
        let focused = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<UiFocusState>()
            .focused_entity = Some(focused);
        app.world_mut().write_message(dispatch(
            MAIN_WORLD_MAIL_DOCUMENT_ID,
            OWNER_MAIN_WORLD_MAIL_PANEL.as_str(),
            ACTION_MAIN_WORLD_MAIL_CLOSE,
            MAIN_WORLD_MAIL_CLOSE_NODE,
            BTreeMap::new(),
        ));
        app.update();

        assert!(matches!(
            read_messages::<DeclarativeScreenHostCommand>(&app).as_slice(),
            [DeclarativeScreenHostCommand::CloseRoute { route }]
                if route == MainWorldDocumentPanel::Mail.route()
        ));
        assert_eq!(app.world().resource::<UiFocusState>().focused_entity, None);
        let reconciliation = app
            .world()
            .resource::<MailClientState>()
            .claim_reconciliation
            .as_ref()
            .unwrap();
        assert_eq!(reconciliation.mail_id, "mail-42");
        assert_eq!(reconciliation.polls_completed, 1);
        assert!(read_messages::<MailClientCommand>(&app).is_empty());
        assert!(read_messages::<NetworkCommand>(&app).is_empty());
        assert!(read_messages::<SceneCommand>(&app).is_empty());
        assert!(read_messages::<GameRouteCommand>(&app).is_empty());
        assert!(read_messages::<MainWorldEntryIntent>(&app).is_empty());
    }

    #[test]
    fn unavailable_main_world_mail_actions_leave_the_adapters_idle_and_show_errors() {
        for (availability, expected_status, expected_error) in [
            (
                MailAvailability::Unavailable {
                    reason: "Mail endpoint is unavailable".to_owned(),
                },
                "Mail unavailable",
                "Mail endpoint is unavailable",
            ),
            (
                MailAvailability::AwaitingCharacterTicket,
                "Waiting for character session",
                "Character session is not ready",
            ),
        ] {
            let mail = MailClientState::with_availability_for_test(availability);
            let mut actions = action_test_app();
            actions.world_mut().insert_resource(mail.clone());
            actions.world_mut().write_message(dispatch(
                MAIN_WORLD_HUD_DOCUMENT_ID,
                OWNER_MAIN_WORLD.as_str(),
                ACTION_MAIN_WORLD_OPEN_MAIL,
                MAIN_WORLD_MAIL_NODE,
                BTreeMap::new(),
            ));
            actions.world_mut().write_message(dispatch(
                MAIN_WORLD_MAIL_DOCUMENT_ID,
                OWNER_MAIN_WORLD_MAIL_PANEL.as_str(),
                ACTION_MAIN_WORLD_MAIL_REFRESH,
                MAIN_WORLD_MAIL_REFRESH_NODE,
                BTreeMap::new(),
            ));
            actions.update();

            assert!(read_messages::<DeclarativeScreenHostCommand>(&actions).is_empty());
            assert!(read_messages::<MailClientCommand>(&actions).is_empty());
            assert!(read_messages::<NetworkCommand>(&actions).is_empty());

            let mut bindings = main_world_mail_binding_test_app(mail);
            bindings.update();
            assert_eq!(
                main_world_mail_binding_value(&bindings, "main_world_mail.status"),
                UiBindingValue::String(expected_status.to_owned())
            );
            assert_eq!(
                main_world_mail_binding_value(&bindings, "main_world_mail.refresh_disabled"),
                UiBindingValue::Bool(true)
            );
            assert_eq!(
                main_world_mail_binding_value(&bindings, "main_world_mail.error.message"),
                UiBindingValue::String(expected_error.to_owned())
            );
            assert_ne!(
                main_world_mail_binding_value(&bindings, "main_world_mail.error.code"),
                UiBindingValue::String(String::new())
            );
            assert_eq!(
                main_world_mail_binding_value(&bindings, "main_world_mail.error_visibility"),
                UiBindingValue::Visibility(UiBindingVisibility::Visible)
            );
        }
    }

    #[test]
    fn stale_main_world_mail_actions_are_ignored_after_the_generation_stops_being_active() {
        let mut app = action_test_app();
        app.world_mut()
            .insert_resource(MailClientState::ready_for_test());
        app.world_mut().resource_mut::<MainWorldEntryState>().phase = MainWorldEntryPhase::Exiting;
        for (document_id, owner, action, source) in [
            (
                MAIN_WORLD_HUD_DOCUMENT_ID,
                OWNER_MAIN_WORLD.as_str(),
                ACTION_MAIN_WORLD_OPEN_MAIL,
                MAIN_WORLD_MAIL_NODE,
            ),
            (
                MAIN_WORLD_MAIL_DOCUMENT_ID,
                OWNER_MAIN_WORLD_MAIL_PANEL.as_str(),
                ACTION_MAIN_WORLD_MAIL_REFRESH,
                MAIN_WORLD_MAIL_REFRESH_NODE,
            ),
            (
                MAIN_WORLD_MAIL_DOCUMENT_ID,
                OWNER_MAIN_WORLD_MAIL_PANEL.as_str(),
                ACTION_MAIN_WORLD_MAIL_CLOSE,
                MAIN_WORLD_MAIL_CLOSE_NODE,
            ),
            (
                MAIN_WORLD_HUD_DOCUMENT_ID,
                OWNER_MAIN_WORLD.as_str(),
                ACTION_MAIN_WORLD_ENTER_HOME,
                MAIN_WORLD_HOME_NODE,
            ),
            (
                MAIN_WORLD_HUD_DOCUMENT_ID,
                OWNER_MAIN_WORLD.as_str(),
                ACTION_MAIN_WORLD_RETURN_LOBBY,
                MAIN_WORLD_RETURN_LOBBY_NODE,
            ),
        ] {
            app.world_mut().write_message(dispatch(
                document_id,
                owner,
                action,
                source,
                BTreeMap::new(),
            ));
        }
        app.update();

        assert!(read_messages::<DeclarativeScreenHostCommand>(&app).is_empty());
        assert!(read_messages::<MailClientCommand>(&app).is_empty());
        assert!(read_messages::<NetworkCommand>(&app).is_empty());
        assert!(read_messages::<MainWorldEntryIntent>(&app).is_empty());
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
    fn unchanged_main_world_snapshot_does_not_advance_binding_revision() {
        let mut app = main_world_binding_test_app();
        app.update();
        let revision = app.world().resource::<UiBindingValues>().revision();

        app.update();
        assert_eq!(
            app.world().resource::<UiBindingValues>().revision(),
            revision
        );
    }

    #[test]
    fn main_world_dynamic_status_uses_the_active_locale() {
        let i18n = UiI18n::test_with_texts(
            "zh_cn",
            &[
                ("main_world.character", "角色"),
                ("main_world.connection.authenticated", "游戏会话已连接"),
                ("main_world.transition.loading", "正在加载主世界"),
            ],
        );

        assert_eq!(
            localized_main_world_character_summary(Some("chr_27he_long"), &i18n),
            "角色 chr_27he"
        );
        assert_eq!(
            localized_main_world_connection_status(GameConnectionState::Authenticated, &i18n),
            "游戏会话已连接"
        );
        assert_eq!(
            localized_main_world_transition_status(MainWorldEntryPhase::LoadingScene, &i18n),
            "正在加载主世界"
        );
    }

    #[test]
    fn main_world_unknown_unread_count_is_hidden_instead_of_displaying_zero() {
        let mut app = main_world_binding_test_app();
        app.world_mut()
            .insert_resource(MailClientState::ready_for_test());
        app.update();

        assert_eq!(
            main_world_binding_value(&app, "main_world.mail.unread"),
            UiBindingValue::String(String::new())
        );
        assert_eq!(
            main_world_binding_value(&app, "main_world.mail.unread_visibility"),
            UiBindingValue::Visibility(UiBindingVisibility::Hidden)
        );

        app.world_mut()
            .insert_resource(MailClientState::ready_with_list_for_test(
                Vec::new(),
                0,
                crate::game::myserver::mail::MailPagination {
                    limit: 50,
                    offset: 0,
                    next_offset: None,
                },
            ));
        app.update();
        assert_eq!(
            main_world_binding_value(&app, "main_world.mail.unread"),
            UiBindingValue::String("0".to_owned())
        );
        assert_eq!(
            main_world_binding_value(&app, "main_world.mail.unread_visibility"),
            UiBindingValue::Visibility(UiBindingVisibility::Hidden)
        );

        app.world_mut()
            .resource_mut::<MailClientState>()
            .unread_count = Some(3);
        app.update();
        assert_eq!(
            main_world_binding_value(&app, "main_world.mail.unread"),
            UiBindingValue::String("3".to_owned())
        );
        assert_eq!(
            main_world_binding_value(&app, "main_world.mail.unread_visibility"),
            UiBindingValue::Visibility(UiBindingVisibility::Visible)
        );

        app.world_mut()
            .resource_mut::<MailClientState>()
            .list_load_state = MailListLoadState::Refreshing;
        app.world_mut().resource_mut::<MailClientState>().list_stale = true;
        app.update();
        assert_eq!(
            main_world_binding_value(&app, "main_world.mail.unread"),
            UiBindingValue::String(String::new())
        );
        assert_eq!(
            main_world_binding_value(&app, "main_world.mail.unread_visibility"),
            UiBindingValue::Visibility(UiBindingVisibility::Hidden)
        );
    }

    #[test]
    fn main_world_mail_bindings_render_bounded_authoritative_list_fields() {
        let title = "T".repeat(120);
        let mut summary = mail_summary_for_test("mail_full_id_0123456789", &title);
        summary.sender = crate::game::myserver::mail::MailSender {
            r#type: Some("internal_type".to_owned()),
            id: Some("internal_sender_id".to_owned()),
            name: Some("Live Ops".to_owned()),
        };
        summary.status = "unread".to_owned();
        summary.has_attachments = true;
        summary.created_at = Some("2026-08-05T12:34:56Z".to_owned());
        summary.expires_at = Some("2999-09-01T00:00:00Z".to_owned());
        let mail = MailClientState::ready_with_list_for_test(
            vec![summary],
            1,
            crate::game::myserver::mail::MailPagination {
                limit: 25,
                offset: 0,
                next_offset: Some(25),
            },
        );
        let mut app = main_world_mail_binding_test_app(mail);
        app.update();

        let UiBindingValue::List(items) =
            main_world_mail_binding_value(&app, "main_world_mail.items")
        else {
            panic!("mail items must remain a typed list");
        };
        let [UiBindingValue::Record(item)] = items.as_slice() else {
            panic!("one authoritative mail row must be synchronized");
        };
        assert_eq!(
            item["mail_id"],
            UiBindingValue::String("mail_full_id_0123456789".to_owned())
        );
        assert_eq!(
            item["sender"],
            UiBindingValue::String("Live Ops".to_owned())
        );
        assert!(matches!(
            &item["title"],
            UiBindingValue::String(value) if value.chars().count() == 99 && value.ends_with("...")
        ));
        assert!(matches!(
            &item["status"],
            UiBindingValue::String(value) if value.contains("Unread") && value.contains("expires")
        ));
        assert_eq!(
            item["attachment_label"],
            UiBindingValue::String("Attachment".to_owned())
        );
        assert!(!item.contains_key("player_id"));
        assert!(!item.contains_key("character_id"));
        assert!(!item.contains_key("sender_id"));
        assert!(!item.contains_key("sender_type"));
        assert_eq!(
            main_world_mail_binding_value(&app, "main_world_mail.has_more"),
            UiBindingValue::Bool(true)
        );
        assert_eq!(
            main_world_mail_binding_value(&app, "main_world_mail.load_more_visibility"),
            UiBindingValue::Visibility(UiBindingVisibility::Visible)
        );
        assert_eq!(
            main_world_mail_binding_value(&app, "main_world_mail.list_visibility"),
            UiBindingValue::Visibility(UiBindingVisibility::Visible)
        );
    }

    #[test]
    fn main_world_mail_detail_bindings_are_bounded_and_hide_attachment_identity() {
        let mut detail = mail_detail_for_test("mail_1", "unread", &"B".repeat(9000));
        detail.summary.sender.name = Some("Support".to_owned());
        detail.attachments = vec![MailAttachment {
            r#type: "item".to_owned(),
            id: Some(987654321),
            count: 3,
            binded: true,
        }];
        let mail = MailClientState::ready_with_detail_for_test(
            vec![detail.summary.clone()],
            1,
            crate::game::myserver::mail::MailPagination::default(),
            detail,
        );
        let mut app = main_world_mail_binding_test_app(mail);
        app.update();

        assert_eq!(
            main_world_mail_binding_value(&app, "main_world_mail.view_mode"),
            UiBindingValue::Enum("detail".to_owned())
        );
        assert_eq!(
            main_world_mail_binding_value(&app, "main_world_mail.list_visibility"),
            UiBindingValue::Visibility(UiBindingVisibility::Hidden)
        );
        assert_eq!(
            main_world_mail_binding_value(&app, "main_world_mail.load_more_visibility"),
            UiBindingValue::Visibility(UiBindingVisibility::Hidden)
        );
        assert!(matches!(
            main_world_mail_binding_value(&app, "main_world_mail.detail.content"),
            UiBindingValue::String(value) if value.len() == 4096 && value.ends_with("...")
        ));
        let UiBindingValue::List(attachments) =
            main_world_mail_binding_value(&app, "main_world_mail.detail.attachments")
        else {
            panic!("detail attachments must remain a typed list");
        };
        let [UiBindingValue::Record(attachment)] = attachments.as_slice() else {
            panic!("one attachment must be exposed");
        };
        assert_eq!(
            attachment["attachment_id"],
            UiBindingValue::String("attachment-0".to_owned())
        );
        assert_eq!(
            attachment["label"],
            UiBindingValue::String("Item".to_owned())
        );
        assert!(!format!("{attachment:?}").contains("987654321"));
        assert_eq!(
            main_world_mail_binding_value(&app, "main_world_mail.mark_read_disabled"),
            UiBindingValue::Bool(false)
        );
    }

    #[test]
    fn main_world_mail_claim_bindings_restore_workflow_states() {
        let mut detail = mail_detail_for_test("mail_1", "unread", "Reward");
        detail.summary.has_attachments = true;
        detail.attachments = vec![MailAttachment {
            r#type: "item".to_owned(),
            id: Some(1001),
            count: 2,
            binded: true,
        }];
        let mail = MailClientState::ready_with_detail_for_test(
            vec![detail.summary.clone()],
            1,
            crate::game::myserver::mail::MailPagination::default(),
            detail,
        );
        let mut app = main_world_mail_binding_test_app(mail);
        app.update();
        assert_eq!(
            main_world_mail_binding_value(&app, "main_world_mail.claim.state"),
            UiBindingValue::Enum("available".to_owned())
        );
        assert_eq!(
            main_world_mail_binding_value(&app, "main_world_mail.claim_disabled"),
            UiBindingValue::Bool(false)
        );

        for (state, player_retryable, exhausted, expected_label, loading) in [
            (
                MailClaimWorkflowState::Submitting,
                false,
                false,
                "Submitting claim",
                true,
            ),
            (
                MailClaimWorkflowState::Processing,
                false,
                false,
                "Confirming claim result",
                false,
            ),
            (
                MailClaimWorkflowState::Claimed,
                false,
                false,
                "Attachments claimed",
                false,
            ),
            (
                MailClaimWorkflowState::AlreadyClaimed,
                false,
                false,
                "Attachments were already claimed",
                false,
            ),
            (
                MailClaimWorkflowState::RetryableFailure,
                true,
                false,
                "Claim was not completed; retry is allowed later",
                false,
            ),
            (
                MailClaimWorkflowState::BlockedCapacity,
                true,
                false,
                "Free inventory space, then retry later",
                false,
            ),
            (
                MailClaimWorkflowState::PermanentFailure,
                false,
                false,
                "Claim could not be completed",
                false,
            ),
            (
                MailClaimWorkflowState::ManualReview,
                false,
                true,
                "Claim result is unknown and needs confirmation",
                false,
            ),
            (
                MailClaimWorkflowState::Unavailable,
                false,
                false,
                "Attachment claiming is temporarily unavailable",
                false,
            ),
        ] {
            {
                let mut mail = app.world_mut().resource_mut::<MailClientState>();
                let workflow = mail.claim_workflow.as_mut().unwrap();
                workflow.state = state;
                workflow.player_retryable = player_retryable;
                workflow.exhausted = exhausted;
            }
            app.update();
            assert_eq!(
                main_world_mail_binding_value(&app, "main_world_mail.claim.state"),
                UiBindingValue::Enum(state.binding_value().to_owned())
            );
            assert_eq!(
                main_world_mail_binding_value(&app, "main_world_mail.claim.label"),
                UiBindingValue::String(expected_label.to_owned())
            );
            assert_eq!(
                main_world_mail_binding_value(&app, "main_world_mail.claim_loading"),
                UiBindingValue::Bool(loading)
            );
            assert_eq!(
                main_world_mail_binding_value(&app, "main_world_mail.claim_disabled"),
                UiBindingValue::Bool(true)
            );
        }
    }

    #[test]
    fn main_world_mail_close_top_returns_to_list_before_closing_floating_route() {
        let mut app = main_world_hud_runtime_app();
        app.insert_resource(MailClientState::ready_for_test());
        app.world_mut().resource_mut::<MainWorldEntryState>().phase = MainWorldEntryPhase::Active;
        app.world_mut()
            .resource_mut::<NextState<AppUiMode>>()
            .set(AppUiMode::MainWorld);
        update_frames(&mut app, 6);
        app.world_mut()
            .write_message(DeclarativeScreenHostCommand::OpenDetachedRoute {
                route: MainWorldDocumentPanel::Mail.route().to_owned(),
            });
        update_frames(&mut app, 6);
        app.world_mut()
            .resource_mut::<MailClientState>()
            .show_detail_for_test(mail_detail_for_test("mail_1", "unread", "Body"));

        app.world_mut()
            .write_message(crate::framework::ui::core::UiDocumentCloseTopRequest);
        update_frames(&mut app, 2);
        assert!(!app.world().resource::<MailClientState>().detail_is_open());
        assert!(
            app.world()
                .resource::<UiDocumentRuntime>()
                .active_instance(
                    OWNER_MAIN_WORLD_MAIL_PANEL.as_str(),
                    &UiDocumentId::from_str(MAIN_WORLD_MAIL_DOCUMENT_ID).unwrap(),
                )
                .is_some()
        );

        app.world_mut()
            .write_message(crate::framework::ui::core::UiDocumentCloseTopRequest);
        update_frames(&mut app, 2);
        assert!(
            app.world()
                .resource::<UiDocumentRuntime>()
                .active_instance(
                    OWNER_MAIN_WORLD_MAIL_PANEL.as_str(),
                    &UiDocumentId::from_str(MAIN_WORLD_MAIL_DOCUMENT_ID).unwrap(),
                )
                .is_none()
        );
    }

    #[test]
    fn main_world_mail_expiration_uses_bounded_rfc3339_time() {
        assert_eq!(rfc3339_unix_seconds("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(rfc3339_unix_seconds("1970-01-01T08:00:00+08:00"), Some(0));
        assert!(rfc3339_unix_seconds("not-a-time").is_none());

        let mut expired = mail_summary_for_test("mail_expired", "Expired reward");
        expired.expires_at = Some("2000-01-01T00:00:00Z".to_owned());
        let UiBindingValue::Record(item) = main_world_mail_list_item(
            &expired,
            rfc3339_unix_seconds("2026-08-05T00:00:00Z").unwrap(),
            false,
        ) else {
            panic!("mail row must remain a typed record");
        };
        assert_eq!(item["expired"], UiBindingValue::Bool(true));
        assert_eq!(item["status"], UiBindingValue::String("Expired".to_owned()));
    }

    #[test]
    fn unavailable_mail_disables_only_the_mail_entry() {
        let mut app = main_world_binding_test_app();
        app.update();

        assert_eq!(
            main_world_binding_value(&app, "main_world.mail.disabled"),
            UiBindingValue::Bool(true)
        );
        assert_eq!(
            main_world_binding_value(&app, "main_world.settings.disabled"),
            UiBindingValue::Bool(false)
        );
        assert_eq!(
            main_world_binding_value(&app, "main_world.home.disabled"),
            UiBindingValue::Bool(false)
        );
        assert_eq!(
            main_world_binding_value(&app, "main_world.return_lobby.disabled"),
            UiBindingValue::Bool(false)
        );
    }

    #[test]
    fn main_world_generation_change_clears_stale_owner_bindings() {
        let mut app = main_world_binding_test_app();
        app.update();

        let stale_path = UiBindingPath::from_str("main_world.stale").unwrap();
        let stale_declaration = UiBindingDeclaration {
            scope: UiBindingScope::Owner,
            value_type: UiBindingType::String,
            default: None,
            missing: UiBindingMissingBehavior::UseConsumerFallback,
        };
        app.world_mut()
            .resource_mut::<UiBindingValues>()
            .set_scoped(
                MAIN_WORLD_HUD_DOCUMENT_ID,
                OWNER_MAIN_WORLD.as_str(),
                &stale_path,
                &stale_declaration,
                UiBindingValue::String("previous character".to_owned()),
            );
        app.world_mut()
            .resource_mut::<MainWorldEntryState>()
            .generation = 1;
        app.update();

        assert_eq!(
            app.world().resource::<UiBindingValues>().scoped_value(
                MAIN_WORLD_HUD_DOCUMENT_ID,
                OWNER_MAIN_WORLD.as_str(),
                &stale_path,
                &stale_declaration,
            ),
            None
        );
    }

    #[test]
    fn main_world_transition_disables_conflicting_entries_and_sets_loading() {
        let mut app = main_world_binding_test_app();
        app.world_mut().resource_mut::<MainWorldEntryState>().phase =
            MainWorldEntryPhase::HomeLoading;
        app.update();

        for path in [
            "main_world.home.disabled",
            "main_world.return_lobby.disabled",
        ] {
            assert_eq!(
                main_world_binding_value(&app, path),
                UiBindingValue::Bool(true)
            );
        }
        assert_eq!(
            main_world_binding_value(&app, "main_world.transition.loading"),
            UiBindingValue::Bool(true)
        );
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
    fn active_main_world_settings_button_opens_its_floating_document() {
        let mut app = main_world_hud_runtime_app();
        app.init_resource::<UiAuditConfig>()
            .init_resource::<FangyuanDebugPanelState>()
            .init_resource::<super::super::robot_sync_scene::RobotSyncHudVisibility>()
            .init_resource::<MainWorldHudBindingGeneration>()
            .insert_resource(AudioMixer::default())
            .add_message::<AudioCommand>()
            .add_message::<GameRouteCommand>()
            .add_message::<FangyuanHomeBlueprintCommand>()
            .add_message::<SceneCommand>()
            .add_message::<MailClientCommand>()
            .add_message::<MainWorldEntryIntent>()
            .add_systems(
                Update,
                handle_gameplay_hud_document_actions.after(UiDocumentRuntimeSystems::Reconcile),
            )
            .add_systems(
                Update,
                sync_main_world_hud_bindings.before(UiBindingSystems::Apply),
            )
            .add_plugins(crate::game::screens::settings::SettingsScreensPlugin);
        app.world_mut().resource_mut::<MainWorldEntryState>().phase = MainWorldEntryPhase::Active;
        app.world_mut()
            .resource_mut::<NextState<AppUiMode>>()
            .set(AppUiMode::MainWorld);
        update_frames(&mut app, 6);

        let settings = app
            .world()
            .resource::<UiDocumentRuntime>()
            .node_entity(
                app.world()
                    .resource::<UiDocumentRuntime>()
                    .active_instance(
                        OWNER_MAIN_WORLD.as_str(),
                        &UiDocumentId::from_str(MAIN_WORLD_HUD_DOCUMENT_ID).unwrap(),
                    )
                    .unwrap(),
                &UiNodeId::from_str(MAIN_WORLD_SETTINGS_NODE).unwrap(),
            )
            .unwrap();
        let mut host_events = MessageCursor::<DeclarativeScreenHostEvent>::default();
        app.world_mut().write_message(UiButtonEvent {
            entity: settings,
            kind: UiButtonEventKind::Click,
            button: None,
        });
        app.update();
        assert!(
            read_messages::<UiActionDispatch>(&app)
                .iter()
                .any(|dispatch| {
                    dispatch.document_id.as_str() == MAIN_WORLD_HUD_DOCUMENT_ID
                        && dispatch.owner == OWNER_MAIN_WORLD.as_str()
                        && dispatch.action.as_str() == ACTION_MAIN_WORLD_OPEN_SETTINGS
                        && dispatch.source_node.as_str() == MAIN_WORLD_SETTINGS_NODE
                })
        );
        assert!(
            read_messages::<DeclarativeScreenHostCommand>(&app)
                .iter()
                .any(|command| matches!(
                    command,
                    DeclarativeScreenHostCommand::OpenDetachedRoute { route }
                        if route == crate::game::screens::settings::MAIN_WORLD_SETTINGS_ROUTE
                ))
        );
        app.update();
        assert!(
            host_events
                .read(
                    app.world()
                        .resource::<Messages<DeclarativeScreenHostEvent>>()
                )
                .all(|event| !matches!(event, DeclarativeScreenHostEvent::LoadFailed { .. })),
            "{:#?}",
            read_messages::<DeclarativeScreenHostEvent>(&app)
        );
        update_frames(&mut app, 2);

        assert!(
            app.world()
                .resource::<UiDocumentRuntime>()
                .active_instance(
                    OWNER_MAIN_WORLD_SETTINGS_PANEL.as_str(),
                    &UiDocumentId::from_str("game.main_world_audio_settings").unwrap(),
                )
                .is_some()
        );
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
    fn main_world_hud_and_mail_keep_accessible_controls_in_all_audit_profiles() {
        let profiles = main_world_audit_profiles();
        assert_eq!(
            profiles
                .iter()
                .filter(|profile| profile.safe_area() == UiSafeAreaClass::Inset)
                .count(),
            3
        );
        let hud = UiDocument::parse_and_validate_json(MAIN_WORLD_HUD_SOURCE).unwrap();
        let mail = UiDocument::parse_and_validate_json(MAIN_WORLD_MAIL_SOURCE).unwrap();

        for (profile, (width, height)) in profiles.into_iter().zip([
            (1280.0, 720.0),
            (800.0, 360.0),
            (800.0, 360.0),
            (1280.0, 800.0),
        ]) {
            assert_eq!(profile.logical_width(), width);
            assert_eq!(profile.logical_height(), height);
            let hud = hud
                .effective_document(&profile, &UiPageState::initial())
                .unwrap();
            let mail = mail
                .effective_document(&profile, &UiPageState::initial())
                .unwrap();
            for node in [
                MAIN_WORLD_SETTINGS_NODE,
                MAIN_WORLD_MAIL_NODE,
                MAIN_WORLD_HOME_NODE,
                MAIN_WORLD_RETURN_LOBBY_NODE,
            ] {
                assert!(
                    find_document_node(&hud.document.root, node).is_some(),
                    "{node}"
                );
            }
            for node in [
                MAIN_WORLD_MAIL_REFRESH_NODE,
                MAIN_WORLD_MAIL_SELECT_NODE,
                MAIN_WORLD_MAIL_LOAD_MORE_NODE,
                MAIN_WORLD_MAIL_MARK_READ_NODE,
                MAIN_WORLD_MAIL_CLAIM_NODE,
                MAIN_WORLD_MAIL_RETRY_NODE,
                MAIN_WORLD_MAIL_CLOSE_NODE,
            ] {
                assert!(
                    find_document_node(&mail.document.root, node).is_some(),
                    "{node}"
                );
            }
        }
    }

    #[test]
    fn main_world_dynamic_status_and_mail_error_stay_outside_action_groups() {
        let hud = UiDocument::parse_and_validate_json(MAIN_WORLD_HUD_SOURCE).unwrap();
        let hud = hud.document();
        let status = find_document_node(&hud.root, "main_world.status").unwrap();
        let buttons = find_document_node(&hud.root, "main_world.buttons").unwrap();
        assert_eq!(status.children().len(), 3);
        assert_eq!(buttons.children().len(), 4);

        let mail = UiDocument::parse_and_validate_json(MAIN_WORLD_MAIL_SOURCE).unwrap();
        let mail = mail.document();
        let panel = find_document_node(&mail.root, "main_world_mail.panel").unwrap();
        let error_panel = find_document_node(&mail.root, "main_world_mail.error_panel").unwrap();
        let actions = find_document_node(&mail.root, "main_world_mail.actions").unwrap();
        assert!(
            panel
                .children()
                .iter()
                .any(|node| node.id().as_str() == "main_world_mail.error_panel")
        );
        assert!(find_document_node(error_panel, "main_world_mail.error_text").is_some());
        assert!(find_document_node(error_panel, MAIN_WORLD_MAIL_RETRY_NODE).is_some());
        assert!(actions.children().iter().all(|node| matches!(
            node.id().as_str(),
            MAIN_WORLD_MAIL_REFRESH_NODE | MAIN_WORLD_MAIL_CLOSE_NODE
        )));
    }

    #[test]
    fn main_world_short_landscape_documents_compact_without_hiding_controls() {
        let profile = main_world_short_landscape_stress_profile();
        let hud = UiDocument::parse_and_validate_json(MAIN_WORLD_HUD_SOURCE)
            .unwrap()
            .effective_document(&profile, &UiPageState::initial())
            .unwrap();
        let mail = UiDocument::parse_and_validate_json(MAIN_WORLD_MAIL_SOURCE)
            .unwrap()
            .effective_document(&profile, &UiPageState::initial())
            .unwrap();

        assert!(
            hud.applied_overrides
                .iter()
                .any(|item| item.source_id == "short_landscape")
        );
        assert!(
            mail.applied_overrides
                .iter()
                .any(|item| item.source_id == "short_landscape")
        );
        assert_eq!(
            find_document_node(&mail.document.root, "main_world_mail.panel")
                .unwrap()
                .layout()
                .width,
            crate::framework::ui::document::UiLength::Percent(100.0)
        );
    }

    #[test]
    fn main_world_documents_exclude_development_and_connection_diagnostics() {
        for source in [MAIN_WORLD_HUD_SOURCE, MAIN_WORLD_MAIL_SOURCE] {
            for forbidden in ["ticket", "endpoint", "room_id", "developer", "debug"] {
                assert!(
                    !source.to_ascii_lowercase().contains(forbidden),
                    "document must not expose {forbidden}"
                );
            }
        }
    }

    #[test]
    fn main_world_hud_touch_controls_keep_the_framework_minimum_on_short_landscape() {
        let parse = UiDocument::parse_and_validate_json(MAIN_WORLD_HUD_SOURCE);
        assert!(parse.is_ok(), "{parse:#?}");
        let viewport = main_world_short_landscape_touch_viewport();
        let metrics = UiMetrics::from_viewport_and_theme(&viewport, &UiTheme::default());
        let mut app = main_world_hud_runtime_app();
        app.world_mut().insert_resource(viewport);
        app.world_mut().insert_resource(metrics);
        app.world_mut().resource_mut::<MainWorldEntryState>().phase = MainWorldEntryPhase::Active;
        app.world_mut()
            .resource_mut::<NextState<AppUiMode>>()
            .set(AppUiMode::MainWorld);
        update_frames(&mut app, 6);

        let runtime = app.world().resource::<UiDocumentRuntime>();
        let instance = runtime
            .active_instance(
                OWNER_MAIN_WORLD.as_str(),
                &UiDocumentId::from_str(MAIN_WORLD_HUD_DOCUMENT_ID).unwrap(),
            )
            .unwrap();
        let touch_target_min = app.world().resource::<UiMetrics>().touch_target_min;
        for node_id in [
            MAIN_WORLD_SETTINGS_NODE,
            MAIN_WORLD_MAIL_NODE,
            MAIN_WORLD_HOME_NODE,
            MAIN_WORLD_RETURN_LOBBY_NODE,
        ] {
            let entity = runtime
                .node_entity(instance, &UiNodeId::from_str(node_id).unwrap())
                .unwrap();
            assert!(matches!(
                app.world().get::<Node>(entity).unwrap().height,
                Val::Px(height) if height >= touch_target_min
            ));
        }
    }

    #[test]
    fn main_world_mail_touch_controls_keep_the_framework_minimum_on_short_landscape() {
        let viewport = main_world_short_landscape_touch_viewport();
        let metrics = UiMetrics::from_viewport_and_theme(&viewport, &UiTheme::default());
        let mut app = main_world_hud_runtime_app();
        app.world_mut().insert_resource(viewport);
        app.world_mut().insert_resource(metrics);
        app.world_mut().resource_mut::<MainWorldEntryState>().phase = MainWorldEntryPhase::Active;
        app.world_mut()
            .resource_mut::<NextState<AppUiMode>>()
            .set(AppUiMode::MainWorld);
        update_frames(&mut app, 6);
        app.world_mut()
            .write_message(DeclarativeScreenHostCommand::OpenDetachedRoute {
                route: MainWorldDocumentPanel::Mail.route().to_owned(),
            });
        update_frames(&mut app, 6);

        let runtime = app.world().resource::<UiDocumentRuntime>();
        let instance = runtime
            .active_instance(
                OWNER_MAIN_WORLD_MAIL_PANEL.as_str(),
                &UiDocumentId::from_str(MAIN_WORLD_MAIL_DOCUMENT_ID).unwrap(),
            )
            .unwrap();
        let root = runtime.instance(instance).unwrap().root;
        assert_eq!(app.world().get::<ZIndex>(root), Some(&ZIndex(80)));
        let touch_target_min = app.world().resource::<UiMetrics>().touch_target_min;
        for node_id in [MAIN_WORLD_MAIL_REFRESH_NODE, MAIN_WORLD_MAIL_CLOSE_NODE] {
            let entity = runtime
                .node_entity(instance, &UiNodeId::from_str(node_id).unwrap())
                .unwrap();
            assert!(matches!(
                app.world().get::<Node>(entity).unwrap().height,
                Val::Px(height) if height >= touch_target_min
            ));
        }

        let document = UiDocument::parse_and_validate_json(MAIN_WORLD_MAIL_SOURCE).unwrap();
        for node_id in [
            MAIN_WORLD_MAIL_REFRESH_NODE,
            MAIN_WORLD_MAIL_SELECT_NODE,
            MAIN_WORLD_MAIL_LOAD_MORE_NODE,
            MAIN_WORLD_MAIL_BACK_TO_LIST_NODE,
            MAIN_WORLD_MAIL_MARK_READ_NODE,
            MAIN_WORLD_MAIL_CLAIM_NODE,
            MAIN_WORLD_MAIL_RETRY_NODE,
            MAIN_WORLD_MAIL_CLOSE_NODE,
        ] {
            let node = find_document_node(&document.document().root, node_id).unwrap();
            assert!(matches!(
                node.layout().height,
                crate::framework::ui::document::UiLength::Px(height)
                    if height >= touch_target_min
            ));
        }

        let fallback =
            UiDocument::parse_and_validate_json(MAIN_WORLD_MAIL_FALLBACK_SOURCE).unwrap();
        let close =
            find_document_node(&fallback.document().root, MAIN_WORLD_MAIL_CLOSE_NODE).unwrap();
        assert!(matches!(
            close.layout().height,
            crate::framework::ui::document::UiLength::Px(height)
                if height >= touch_target_min
        ));
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

    #[cfg(all(debug_assertions, not(target_os = "android")))]
    #[test]
    fn startup_audit_config_makes_the_main_world_hud_document_root_ready() {
        let mut app = main_world_hud_runtime_app();
        app.insert_resource(UiAuditConfig::from_test_env(&[
            ("MYBEVY_UI_AUDIT", "1"),
            ("MYBEVY_UI_AUDIT_SCREEN", "main_world"),
            (
                "MYBEVY_UI_AUDIT_STABLE_FIXTURE_ID",
                "stage16_main_world_hud",
            ),
        ]))
        .add_plugins(crate::game::scenes::main_world_entry::MainWorldEntryPlugin);
        update_frames(&mut app, 6);

        let runtime = app.world().resource::<UiDocumentRuntime>();
        let document_id = UiDocumentId::from_str(MAIN_WORLD_HUD_DOCUMENT_ID).unwrap();
        let instance = runtime
            .active_instance(OWNER_MAIN_WORLD.as_str(), &document_id)
            .unwrap();
        assert!(
            runtime
                .node_entity(instance, &UiNodeId::from_str("main_world.root").unwrap())
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
    fn main_world_mail_missing_fallback_closes_only_the_panel_and_preserves_reconciliation() {
        let mut app = App::new();
        app.init_resource::<UiFocusState>()
            .insert_resource(MailClientState::ready_with_reconciliation_for_test(
                "mail-42",
            ))
            .add_message::<DeclarativeScreenHostEvent>()
            .add_message::<DeclarativeScreenHostCommand>()
            .add_message::<MainWorldEntryIntent>()
            .add_message::<MailClientCommand>()
            .add_systems(Update, recover_from_main_world_mail_failure);
        let focused = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<UiFocusState>()
            .focused_entity = Some(focused);
        app.world_mut()
            .write_message(DeclarativeScreenHostEvent::LoadFailed {
                code: "UI_DECLARATIVE_SCREEN_LOAD_FAILED".to_owned(),
                cause: "mail fallback unavailable".to_owned(),
                route: MainWorldDocumentPanel::Mail.route().to_owned(),
                document_id: UiDocumentId::from_str(MAIN_WORLD_MAIL_DOCUMENT_ID).unwrap(),
                owner: OWNER_MAIN_WORLD_MAIL_PANEL.as_str().to_owned(),
                decision: DeclarativeScreenFailureDecision::NoFallbackAvailable,
            });
        app.update();

        assert!(matches!(
            read_messages::<DeclarativeScreenHostCommand>(&app).as_slice(),
            [DeclarativeScreenHostCommand::CloseRoute { route }]
                if route == MainWorldDocumentPanel::Mail.route()
        ));
        assert_eq!(app.world().resource::<UiFocusState>().focused_entity, None);
        let reconciliation = app
            .world()
            .resource::<MailClientState>()
            .claim_reconciliation
            .as_ref()
            .unwrap();
        assert_eq!(reconciliation.mail_id, "mail-42");
        assert!(read_messages::<MainWorldEntryIntent>(&app).is_empty());
        assert!(read_messages::<MailClientCommand>(&app).is_empty());
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
        app.add_plugins((
            MinimalPlugins,
            StatesPlugin,
            crate::framework::ui::i18n::UiI18nPlugin,
        ))
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
            .unwrap_or_else(|| {
                panic!(
                    "{:#?}",
                    read_messages::<crate::framework::ui::document::UiDocumentReloadEvent>(&app)
                )
            });
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

    fn mail_summary_for_test(mail_id: &str, title: &str) -> MailSummary {
        MailSummary {
            mail_id: mail_id.to_owned(),
            sender: crate::game::myserver::mail::MailSender::default(),
            title: title.to_owned(),
            mail_type: "system".to_owned(),
            status: "unread".to_owned(),
            has_attachments: false,
            created_at: None,
            read_at: None,
            claimed_at: None,
            expires_at: None,
        }
    }

    fn mail_detail_for_test(
        mail_id: &str,
        status: &str,
        content: &str,
    ) -> crate::game::myserver::mail::MailDetail {
        let mut summary = mail_summary_for_test(mail_id, "Detail title");
        summary.status = status.to_owned();
        crate::game::myserver::mail::MailDetail {
            summary,
            content: content.to_owned(),
            attachments: Vec::new(),
            claim: crate::game::myserver::mail::MailClaimSummary::default(),
        }
    }

    fn action_test_app() -> App {
        let mut app = App::new();
        app.init_resource::<FangyuanDebugPanelState>()
            .init_resource::<super::super::robot_sync_scene::RobotSyncHudVisibility>()
            .init_resource::<MainWorldEntryState>()
            .init_resource::<UiFocusState>()
            .add_message::<UiActionDispatch>()
            .add_message::<FangyuanHomeBlueprintCommand>()
            .add_message::<SceneCommand>()
            .add_message::<GameRouteCommand>()
            .add_message::<DeclarativeScreenHostCommand>()
            .add_message::<MailClientCommand>()
            .add_message::<NetworkCommand>()
            .add_message::<MainWorldEntryIntent>()
            .add_systems(Update, handle_gameplay_hud_document_actions);
        app.world_mut().resource_mut::<MainWorldEntryState>().phase = MainWorldEntryPhase::Active;
        app
    }

    fn main_world_binding_test_app() -> App {
        let mut app = App::new();
        app.init_resource::<MainWorldEntryState>()
            .init_resource::<GameplayHudHostContract>()
            .init_resource::<MainWorldHudBindingGeneration>()
            .init_resource::<UiBindingValues>()
            .insert_resource(AudioMixer::default())
            .insert_resource(UiI18n::test_with_texts("en_us", &[]))
            .add_systems(Update, sync_main_world_hud_bindings);
        app
    }

    fn main_world_binding_value(app: &App, path: &str) -> UiBindingValue {
        let path = UiBindingPath::from_str(path).unwrap();
        let contract = app.world().resource::<GameplayHudHostContract>();
        let declaration = contract
            .bindings
            .get(MAIN_WORLD_HUD_DOCUMENT_ID)
            .and_then(|bindings| bindings.get(&path))
            .unwrap();
        app.world()
            .resource::<UiBindingValues>()
            .scoped_value(
                MAIN_WORLD_HUD_DOCUMENT_ID,
                OWNER_MAIN_WORLD.as_str(),
                &path,
                declaration,
            )
            .unwrap()
    }

    fn main_world_mail_binding_test_app(mail: MailClientState) -> App {
        let mut app = App::new();
        app.init_resource::<GameplayHudHostContract>()
            .init_resource::<UiBindingValues>()
            .insert_resource(mail)
            .add_systems(Update, sync_main_world_mail_bindings);
        app
    }

    fn main_world_mail_binding_value(app: &App, path: &str) -> UiBindingValue {
        let path = UiBindingPath::from_str(path).unwrap();
        let contract = app.world().resource::<GameplayHudHostContract>();
        let declaration = contract
            .bindings
            .get(MAIN_WORLD_MAIL_DOCUMENT_ID)
            .and_then(|bindings| bindings.get(&path))
            .unwrap();
        app.world()
            .resource::<UiBindingValues>()
            .scoped_value(
                MAIN_WORLD_MAIL_DOCUMENT_ID,
                OWNER_MAIN_WORLD_MAIL_PANEL.as_str(),
                &path,
                declaration,
            )
            .unwrap()
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
                crate::framework::ui::i18n::UiI18nPlugin,
                UiDocumentRuntimePlugin,
                UiDocumentPreviewPlugin,
                crate::game::declarative_screen::DeclarativeScreenHostPlugin,
            ))
            .add_systems(Startup, register_gameplay_hud_contracts);
        app
    }

    fn main_world_audit_profiles() -> [UiTargetProfile; 4] {
        // UiTargetProfile stores runner logical geometry only. The two 800x360
        // phone captures differ by physical scale (2x and 3x) in run-ui-audit.
        [
            UiTargetProfile::new(
                1280.0,
                720.0,
                UiSafeAreaClass::None,
                UiDocumentInputMode::MouseKeyboard,
                UiDocumentPlatform::Windows,
            )
            .unwrap(),
            UiTargetProfile::new(
                800.0,
                360.0,
                UiSafeAreaClass::Inset,
                UiDocumentInputMode::Touch,
                UiDocumentPlatform::Android,
            )
            .unwrap(),
            UiTargetProfile::new(
                800.0,
                360.0,
                UiSafeAreaClass::Inset,
                UiDocumentInputMode::Touch,
                UiDocumentPlatform::Android,
            )
            .unwrap(),
            UiTargetProfile::new(
                1280.0,
                800.0,
                UiSafeAreaClass::Inset,
                UiDocumentInputMode::Touch,
                UiDocumentPlatform::Android,
            )
            .unwrap(),
        ]
    }

    fn main_world_short_landscape_stress_profile() -> UiTargetProfile {
        UiTargetProfile::new(
            800.0,
            360.0,
            UiSafeAreaClass::Inset,
            UiDocumentInputMode::Touch,
            UiDocumentPlatform::Android,
        )
        .unwrap()
    }

    fn main_world_short_landscape_touch_viewport() -> UiViewport {
        UiViewport::from_device_logical_size(
            800.0,
            360.0,
            UiInputMode::Touch,
            UiSafeArea {
                left: 12.0,
                right: 12.0,
                top: 10.0,
                bottom: 10.0,
            },
        )
    }

    fn find_document_node<'a>(node: &'a UiNode, id: &str) -> Option<&'a UiNode> {
        if node.id().as_str() == id {
            return Some(node);
        }
        node.children()
            .iter()
            .find_map(|child| find_document_node(child, id))
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

    #[test]
    fn main_world_mail_errors_have_stable_actionable_messages() {
        use crate::game::myserver::mail::MailOperation;

        for (status, expected) in [
            (400, "Mail request was rejected"),
            (401, "Mail session expired; sign in again"),
            (403, "You no longer have access to this mail"),
            (404, "This mail is no longer available"),
            (409, "Mail state changed; refresh and try again"),
            (410, "This mail has expired"),
            (429, "Mail requests are limited; try again shortly"),
            (503, "Mail service is temporarily unavailable"),
        ] {
            assert_eq!(
                main_world_mail_error_message(&MailClientError {
                    operation: MailOperation::List,
                    status: Some(status),
                    code: format!("MAIL_HTTP_{status}"),
                }),
                expected
            );
        }
        for (code, expected) in [
            (
                "MAIL_REQUEST_TIMEOUT",
                "Mail request timed out; try again later",
            ),
            (
                "MAIL_RESPONSE_TOO_LARGE",
                "Mail response was too large to display",
            ),
            (
                "MAIL_RESPONSE_INVALID",
                "Mail service returned an invalid response",
            ),
        ] {
            assert_eq!(
                main_world_mail_error_message(&MailClientError {
                    operation: MailOperation::Detail,
                    status: None,
                    code: code.to_owned(),
                }),
                expected
            );
        }
    }

    #[cfg(all(debug_assertions, not(target_os = "android")))]
    #[test]
    fn main_world_mail_audit_fixture_opens_once_with_stress_content() {
        let mut app = App::new();
        app.init_resource::<MailClientState>()
            .add_message::<DeclarativeScreenHostCommand>()
            .insert_resource(UiAuditConfig::from_test_env(&[
                ("MYBEVY_UI_AUDIT", "1"),
                ("MYBEVY_UI_AUDIT_SCREEN", "main_world"),
                (
                    "MYBEVY_UI_AUDIT_STABLE_FIXTURE_ID",
                    "stage18_main_world_mail",
                ),
            ]))
            .add_systems(Update, prepare_main_world_mail_audit_fixture);
        let mut cursor = MessageCursor::<DeclarativeScreenHostCommand>::default();

        app.update();
        let commands = cursor
            .read(
                app.world()
                    .resource::<Messages<DeclarativeScreenHostCommand>>(),
            )
            .cloned()
            .collect::<Vec<_>>();
        assert!(matches!(
            commands.as_slice(),
            [DeclarativeScreenHostCommand::OpenDetachedRoute { route }]
                if route == MainWorldDocumentPanel::Mail.route()
        ));
        let mail = app.world().resource::<MailClientState>();
        assert!(mail.detail_is_open());
        assert_eq!(
            mail.selected_mail.as_ref().unwrap().attachments.len(),
            MAIL_MAX_DETAIL_ATTACHMENTS
        );

        app.update();
        assert_eq!(
            cursor
                .read(
                    app.world()
                        .resource::<Messages<DeclarativeScreenHostCommand>>(),
                )
                .count(),
            0
        );
    }
}
